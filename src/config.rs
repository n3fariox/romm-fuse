use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;

#[derive(Parser)]
#[command(name = "romm-fuse", about = "FUSE filesystem bridging RomM to emulation systems")]
pub struct Args {
    /// Mountpoint directory
    pub mountpoint: Option<PathBuf>,

    /// RomM instance URL
    #[arg(long, env = "ROMM_URL")]
    pub romm_url: Option<String>,

    /// Bearer token for RomM API
    #[arg(long, env = "ROMM_TOKEN")]
    pub token: Option<String>,

    /// Built-in profile: mister, retroarch, emulationstation
    #[arg(long, env = "ROMM_PROFILE", conflicts_with = "config")]
    pub profile: Option<String>,

    /// Custom profile TOML file
    #[arg(long, conflicts_with = "profile")]
    pub config: Option<PathBuf>,

    /// Config file path (default: ~/.config/romm-fuse/config.toml)
    #[arg(long)]
    pub config_file: Option<PathBuf>,

    /// Local cache directory
    #[arg(long, env = "ROMM_CACHE_DIR", default_value = "/tmp/romm-fuse-cache")]
    pub cache_dir: PathBuf,

    /// Pass allow_other to FUSE mount
    #[arg(long)]
    pub allow_other: bool,

    /// How long to cache API responses in seconds
    #[arg(long, env = "ROMM_TTL", default_value_t = 300)]
    pub ttl: u64,

    /// Chunk size for partial reads in bytes (default: 262144 = 256KB)
    #[arg(long, env = "ROMM_CHUNK_SIZE", default_value_t = 262144)]
    pub chunk_size: u64,

    /// Test RomM connection and exit
    #[arg(long)]
    pub test: bool,

    /// Don't daemonize, stay in foreground
    #[arg(long)]
    pub foreground: bool,
}

/// Config file structure (TOML)
#[derive(Debug, Deserialize, Default)]
pub struct ConfigFile {
    pub romm_url: Option<String>,
    pub token: Option<String>,
    pub profile: Option<String>,
    #[allow(dead_code)]
    pub cache_dir: Option<PathBuf>,
    #[allow(dead_code)]
    pub allow_other: Option<bool>,
    #[allow(dead_code)]
    pub ttl: Option<u64>,
    #[allow(dead_code)]
    pub chunk_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileConfig {
    pub profile: ProfileMeta,
    pub platforms: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileMeta {
    #[allow(dead_code)]
    pub name: String,
    #[serde(default)]
    pub prefix: Option<String>,
}

impl ConfigFile {
    /// Load config file from path, or try default locations
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let paths = if let Some(p) = path {
            vec![p.to_path_buf()]
        } else {
            let mut candidates = Vec::new();

            // ~/.config/romm-fuse/config.toml
            if let Some(home) = std::env::var_os("HOME") {
                candidates.push(PathBuf::from(home)
                    .join(".config").join("romm-fuse").join("config.toml"));
            }

            // /etc/romm-fuse/config.toml
            candidates.push(PathBuf::from("/etc/romm-fuse/config.toml"));

            candidates
        };

        for p in &paths {
            if p.exists() {
                let content = std::fs::read_to_string(p)
                    .with_context(|| format!("reading config file {}", p.display()))?;
                let config: ConfigFile = toml::from_str(&content)
                    .with_context(|| format!("parsing config file {}", p.display()))?;
                log::info!("loaded config from {}", p.display());
                return Ok(config);
            }
        }

        Ok(ConfigFile::default())
    }
}

/// Resolve final values: CLI flags > env vars > config file > defaults
#[allow(dead_code)]
pub struct ResolvedConfig {
    pub romm_url: String,
    pub token: String,
    pub profile: String,
    pub config: Option<PathBuf>,
    pub cache_dir: PathBuf,
    pub allow_other: bool,
    pub ttl: u64,
    pub chunk_size: u64,
    pub foreground: bool,
    pub mountpoint: PathBuf,
}

impl ResolvedConfig {
    pub fn resolve(args: &Args) -> Result<Self> {
        let config_file = ConfigFile::load(args.config_file.as_deref())?;

        // Prompt for missing values interactively
        let romm_url = args.romm_url.clone()
            .or(config_file.romm_url)
            .or_else(|| prompt("RomM URL (e.g. http://192.168.1.100:3000): "));

        let token = args.token.clone()
            .or(config_file.token)
            .or_else(|| prompt("API Token (rmm_...): "));

        let profile = args.profile.clone()
            .or(config_file.profile)
            .unwrap_or_else(|| "mister".to_string());

        let mountpoint = args.mountpoint.clone()
            .ok_or_else(|| anyhow::anyhow!("mountpoint is required"))?;

        let romm_url = romm_url
            .ok_or_else(|| anyhow::anyhow!("RomM URL is required (--romm-url, ROMM_URL, or config file)"))?;
        let token = token
            .ok_or_else(|| anyhow::anyhow!("API token is required (--token, ROMM_TOKEN, or config file)"))?;

        Ok(ResolvedConfig {
            romm_url,
            token,
            profile,
            config: args.config.clone(),
            cache_dir: args.cache_dir.clone(),
            allow_other: args.allow_other || config_file.allow_other.unwrap_or(false),
            ttl: args.ttl,
            chunk_size: config_file.chunk_size.unwrap_or(args.chunk_size),
            foreground: args.foreground,
            mountpoint,
        })
    }
}

fn prompt(label: &str) -> Option<String> {
    use std::io::{self, Write};

    // Don't prompt if stdin is not a terminal
    if !atty::is(atty::Stream::Stdin) {
        return None;
    }

    eprint!("{label}");
    io::stderr().flush().ok()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

impl ProfileConfig {
    pub fn load(args: &ResolvedConfig) -> Result<Self> {
        if let Some(config_path) = &args.config {
            let content = std::fs::read_to_string(config_path)
                .with_context(|| format!("reading config file {}", config_path.display()))?;
            let config: ProfileConfig =
                toml::from_str(&content).with_context(|| "parsing config TOML")?;
            Ok(config)
        } else {
            let name = &args.profile;
            let content = crate::profiles::get_builtin(name)
                .with_context(|| format!("unknown built-in profile: {name}"))?;
            let config: ProfileConfig =
                toml::from_str(content).with_context(|| "parsing built-in profile TOML")?;
            Ok(config)
        }
    }
}
