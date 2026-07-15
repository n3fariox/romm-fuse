mod api;
mod config;
mod fs;
mod profiles;

use anyhow::Result;
use clap::Parser;
use log::info;

use crate::config::Args;

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    let resolved = config::ResolvedConfig::resolve(&args)?;

    if args.test {
        return test_connection(&resolved);
    }

    info!("romm-fuse starting");
    info!("RomM URL: {}", resolved.romm_url);
    info!("Profile: {}", resolved.profile);
    info!("Cache dir: {}", resolved.cache_dir.display());

    let mountpoint = resolved.mountpoint.clone();

    fs::mount(resolved)?;

    info!("romm-fuse unmounted from {}", mountpoint.display());
    Ok(())
}

fn test_connection(resolved: &config::ResolvedConfig) -> Result<()> {
    use crate::api::client::RommClient;

    println!("Testing connection to {}", resolved.romm_url);
    println!();

    let client = RommClient::new(&resolved.romm_url, &resolved.token)?;

    // Test platforms endpoint
    println!("=== Platforms ===");
    let platforms = match client.list_platforms() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("FAIL: could not list platforms: {e}");
            return Err(e);
        }
    };
    println!("Found {} platforms", platforms.len());
    for p in &platforms {
        println!("  {:>4}  {:<20}  {}  ({} ROMs)", p.id, p.slug, p.name, p.rom_count);
    }
    println!();

    // Test ROMs for first platform with ROMs
    let platform = platforms.iter().find(|p| p.rom_count > 0);
    let platform = match platform {
        Some(p) => p,
        None => {
            println!("No platforms with ROMs found — nothing more to test.");
            return Ok(());
        }
    };

    println!("=== ROMs for '{}' (id={}) ===", platform.name, platform.id);
    let roms = match client.list_all_roms(platform.id) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("FAIL: could not list ROMs for '{}': {e}", platform.name);
            return Err(e);
        }
    };
    println!("Found {} ROMs", roms.len());
    for rom in roms.iter().take(10) {
        let files = if rom.files.is_empty() {
            "no files".to_string()
        } else {
            rom.files.iter()
                .map(|f| f.file_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!("  {:>4}  {:<30}  {} bytes  [{files}]", rom.id, rom.fs_name, rom.fs_size_bytes);
    }
    if roms.len() > 10 {
        println!("  ... and {} more", roms.len() - 10);
    }
    println!();

    // Test download of first ROM
    if let Some(rom) = roms.first() {
        if let Some(file) = rom.files.first() {
            println!("=== Download test: '{}' ===", file.file_name);
            let tmp = std::env::temp_dir().join("romm-fuse-test.bin");
            match client.download_rom_content(rom.id, &file.file_name, &tmp) {
                Ok(bytes) => {
                    println!("OK: downloaded {bytes} bytes to {}", tmp.display());
                    std::fs::remove_file(&tmp).ok();
                }
                Err(e) => {
                    eprintln!("FAIL: could not download: {e}");
                }
            }
        }
    }

    println!();
    println!("All checks passed.");
    Ok(())
}
