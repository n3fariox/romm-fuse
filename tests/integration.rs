use std::collections::HashMap;
use std::path::PathBuf;

use clap::Parser;
use romm_fuse::api::types::SimpleRom;
use romm_fuse::config::{Args, ProfileConfig, ResolvedConfig};
use romm_fuse::fs::tree::FileTree;

fn make_rom(id: u64, name: &str, platform_slug: &str, multi: bool) -> SimpleRom {
    let ext = name.rsplit('.').next().unwrap_or("bin").to_string();
    let no_ext = name
        .strip_suffix(&format!(".{ext}"))
        .unwrap_or(name)
        .to_string();
    let files = if multi {
        vec![
            romm_fuse::api::types::RomFile {
                id: id * 10,
                rom_id: id,
                file_name: format!("{name}.disc1"),
                file_path: String::new(),
                file_size_bytes: 1024,
                is_top_level: false,
            },
            romm_fuse::api::types::RomFile {
                id: id * 10 + 1,
                rom_id: id,
                file_name: format!("{name}.disc2"),
                file_path: String::new(),
                file_size_bytes: 2048,
                is_top_level: false,
            },
        ]
    } else {
        vec![romm_fuse::api::types::RomFile {
            id: id * 10,
            rom_id: id,
            file_name: name.to_string(),
            file_path: String::new(),
            file_size_bytes: 4096,
            is_top_level: true,
        }]
    };

    SimpleRom {
        id,
        name: Some(no_ext.clone()),
        slug: None,
        fs_name: name.to_string(),
        fs_name_no_ext: no_ext,
        fs_extension: ext,
        fs_size_bytes: if multi { 3072 } else { 4096 },
        platform_id: 1,
        platform_slug: platform_slug.to_string(),
        platform_fs_slug: platform_slug.to_string(),
        is_top_level: true,
        has_simple_single_file: !multi,
        has_nested_single_file: false,
        has_multiple_files: multi,
        files,
        updated_at: None,
    }
}

#[test]
fn test_single_file_rom() {
    let mut tree = FileTree::new();
    let root = 1;
    let nes_dir = tree.allocate_inode();
    tree.add_dir(root, "NES".to_string(), nes_dir);

    let roms = vec![make_rom(1, "Super Mario Bros. (USA).nes", "nes", false)];
    tree.build_from_roms(&HashMap::from([("nes".to_string(), nes_dir)]), &roms);

    let children = tree.children(root).unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].0, "NES");

    let children = tree.children(nes_dir).unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].0, "Super Mario Bros. (USA).nes");

    assert!(tree.lookup(root, "NES").is_some());
    assert!(tree
        .lookup(nes_dir, "Super Mario Bros. (USA).nes")
        .is_some());
    assert!(tree.lookup(nes_dir, "nonexistent.nes").is_none());
}

#[test]
fn test_multi_file_rom() {
    let mut tree = FileTree::new();
    let root = 1;
    let psx_dir = tree.allocate_inode();
    tree.add_dir(root, "PSX".to_string(), psx_dir);

    let roms = vec![make_rom(2, "Final Fantasy VII", "playstation", true)];
    tree.build_from_roms(
        &HashMap::from([("playstation".to_string(), psx_dir)]),
        &roms,
    );

    let children = tree.children(psx_dir).unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].0, "Final Fantasy VII");
    let ff7_dir = children[0].1;

    let children = tree.children(ff7_dir).unwrap();
    assert_eq!(children.len(), 2);
}

#[test]
fn test_unmapped_platform_ignored() {
    let mut tree = FileTree::new();
    let root = 1;

    let roms = vec![make_rom(3, "game.pce", "tgfx16", false)];
    tree.build_from_roms(&HashMap::new(), &roms);

    let children = tree.children(root).unwrap();
    assert_eq!(children.len(), 0);
}

#[test]
fn test_profile_loading() {
    let args = ResolvedConfig {
        mountpoint: Some(PathBuf::from("/tmp/test")),
        romm_url: "http://localhost:3000".to_string(),
        token: "test".to_string(),
        profile: "mister".to_string(),
        config: None,
        cache_dir: PathBuf::from("/tmp/cache"),
        allow_other: false,
        ttl: 300,
        chunk_size: 262144,
        foreground: false,
    };

    let config = ProfileConfig::load(&args).unwrap();
    assert_eq!(config.profile.name, "mister");
    assert_eq!(config.profile.prefix.as_deref(), Some("games"));
    assert_eq!(config.platforms.get("nes").unwrap(), "NES");
    assert_eq!(config.platforms.get("snes").unwrap(), "SNES");
}

#[test]
fn test_config_file_loading() {
    use romm_fuse::config::ConfigFile;

    // Non-existent file returns defaults
    let config = ConfigFile::load(Some(std::path::Path::new("/nonexistent/config.toml"))).unwrap();
    assert!(config.romm_url.is_none());
    assert!(config.token.is_none());
}

#[test]
fn test_priority_chain() {
    // When ROMM_URL env is set, clap's env feature picks it up via parse()
    std::env::set_var("ROMM_URL", "http://env-test:3000");
    std::env::set_var("ROMM_TOKEN", "env-token");

    let args = Args::parse_from(["romm-fuse", "/tmp/test"]);
    let resolved = ResolvedConfig::resolve(&args).unwrap();
    assert_eq!(resolved.romm_url, "http://env-test:3000");
    assert_eq!(resolved.token, "env-token");
    assert_eq!(resolved.profile, "mister"); // default

    // CLI flag should override env var
    let args = Args::parse_from([
        "romm-fuse",
        "/tmp/test",
        "--romm-url",
        "http://cli-override:3000",
    ]);
    let resolved = ResolvedConfig::resolve(&args).unwrap();
    assert_eq!(resolved.romm_url, "http://cli-override:3000");
    assert_eq!(resolved.token, "env-token"); // env still used

    std::env::remove_var("ROMM_URL");
    std::env::remove_var("ROMM_TOKEN");
}
