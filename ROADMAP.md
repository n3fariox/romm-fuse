# romm-fuse: FUSE Filesystem Bridging RomM to Any Directory-Based Emulation System

## Overview

A read-only FUSE filesystem in Rust that presents a RomM instance's ROM library as a
plain directory tree. Comes with built-in mapping profiles for common systems
(MiSTer FPGA, RetroArch, EmulationStation, etc.) and supports custom TOML profiles
for any directory-based frontend. Runs directly on the target device's Linux
environment (MiSTer Debian, Raspberry Pi, etc.).

## Architecture

```
romm-fuse
├── main.rs              # CLI args + mount
├── api/
│   ├── client.rs        # reqwest HTTP client for RomM API (bearer auth)
│   └── types.rs         # Serde structs for Platform/Rom/File schemas
├── fs/
│   ├── romm_fs.rs       # fuser::Filesystem impl (the FUSE bridge)
│   ├── inodes.rs        # inode allocator & path<->inode mapping
│   ├── cache.rs         # local disk cache for downloaded ROMs
│   └── tree.rs          # in-memory directory tree (platforms→ROMs→files)
├── config.rs            # CLI args + profile loading
└── profiles/
    ├── mod.rs           # Profile registry + built-in profile loader
    ├── mister.toml      # MiSTer FPGA mapping
    ├── retroarch.toml   # RetroArch mapping
    └── emulationstation.toml
```

## Virtual Directory Layout

The directory layout depends on which **mapping profile** is active. Different
frontends expect different structures:

```
# --profile mister
<mountpoint>/games/NES/Super Mario Bros. (USA).nes

# --profile retroarch
<mountpoint>/nes/Super Mario Bros. (USA).nes

# --profile custom --config my-setup.toml
<mountpoint>/<whatever you define>
```

| Profile | Example layout | Notes |
|---|---|---|
| `mister` | `games/NES/rom.nes` | MiSTer FPGA core folder names |
| `retroarch` | `nes/rom.nes` | RetroArch content folder convention |
| `emulationstation` | `nes/rom.nes` | ES-style short names |
| `custom` | user-defined | Any arbitrary mapping via TOML |

## Key Design Decisions

1. **Read-only now, extensible later** — tree/inode system designed so firmware/saves can layer on without restructuring
2. **Local cache, eventual chunk streaming** — ROMs download on first `open()` to `<cache-dir>/<sha256>`. Future: HTTP Range-based chunked reads for large ISOs (PSX/Saturn)
3. **Profile-based platform mapping** — Built-in profiles for MiSTer, RetroArch, EmulationStation, plus custom TOML profiles. A profile maps RomM platform slugs to directory names and can customize path prefixes, file filtering, and naming rules.
4. **In-memory tree + TTL refresh** — platforms fetched on mount, ROMs listed per-platform with pagination, tree refreshed after configurable TTL (default 300s)
5. **Multi-disc as subfolders** — ROMs with multiple files become subdirectories (convention for CD-based games)

## FUSE Operations

| Op | Behavior |
|---|---|
| `lookup` | Find inode by parent + name from in-memory tree |
| `getattr` | Return stat (size, mode 0444, mtime from API) |
| `readdir` | List entries from tree |
| `open` | Trigger cache download if not cached |
| `read` | Read from cached file at offset |
| `statfs` | Return free space from cache dir |

Write ops (`write`, `mkdir`, `unlink`, etc.) return `EROFS`.

## CLI

```
romm-fuse [OPTIONS] <MOUNTPOINT>

Options:
  --romm-url <URL>          RomM instance URL (e.g. http://192.168.1.100:3000)
  --token <TOKEN>           Bearer token for RomM API (rmm_...)
  --profile <NAME>          Built-in profile: mister, retroarch, emulationstation
  --config <FILE>           Custom profile TOML (use instead of --profile)
  --cache-dir <DIR>         Local cache directory (default: /tmp/romm-fuse-cache)
  --allow-other             Pass allow_other to FUSE mount
  --ttl <SECONDS>           How long to cache API responses (default: 300)
```

## Profile Format (TOML)

```toml
[profile]
name = "mister"
prefix = "games"           # optional path prefix prepended to all platforms

[platforms]
# RomM slug = MiSTer core folder name
"nes"        = "NES"
"snes"       = "SNES"
"genesis"    = "Genesis"
"mega-drive" = "MegaDrive"
"game-boy"   = "GAMEBOY"
"gba"        = "GBA"
"master-system" = "SMS"
"mega-cd"    = "MegaCD"
"n64"        = "N64"
"playstation" = "PSX"
"saturn"     = "Saturn"
"tgfx16"     = "TGFX16"
"tgfx16-cd"  = "TGFX16-CD"
"neo-geo"    = "NeoGeo"
"atari-2600" = "ATARI2600"
"atari-5200" = "ATARI5200"
"atari-7800" = "ATARI7800"
"game-gear"  = "SMS"
"s32x"       = "S32X"
"wonderswan" = "WonderSwan"
```

Platforms not listed in the mapping are skipped (not exposed in the filesystem).

## Dependencies

- **`fuser` 0.17** — FUSE impl (sync, well-tested)
- **`reqwest` 0.12** (blocking + json) — RomM API calls
- **`clap` 4** — CLI parsing
- **`toml` 0.8** — profile config
- **`serde`/`serde_json`** — API response deserialization
- **`sha2`** — cache key hashing

## Roadmap

### v0.1 — Minimal Working FS

- [ ] Project skeleton (`cargo init`, deps, directory structure, clap CLI)
- [ ] RomM API client — types.rs (Platform, SimpleRom, RomFile schemas), client.rs (list_platforms, list_roms with pagination, download_rom)
- [ ] Profile system — built-in mister/retroarch/emulationstation profiles, custom TOML loader
- [ ] Tree + inodes — build in-memory tree from API, allocate inodes, path lookup
- [ ] ROM cache — disk-backed cache with SHA256 keys, download-on-demand
- [ ] FUSE impl — wire everything into `fuser::Filesystem` trait
- [ ] Testing — unit tests + integration test (mount, ls, cat a ROM)

### v0.2 — Chunk-Based Streaming

- [ ] HTTP Range support for partial reads (fetch only requested byte ranges)
- [ ] Chunked cache with LRU eviction for large ISOs (PSX/Saturn/SegaCD)
- [ ] Configurable chunk size (default 256KB)
- [ ] Performance tuning for sequential reads (read-ahead)

### v0.3 — Firmware Support

- [ ] Expose RomM firmware files via `/api/firmware`
- [ ] Profile-configurable firmware placement (e.g. MiSTer `bootrom/`, RetroArch `system/`)
- [ ] Firmware cache with separate TTL

### v0.4 — Save/State Bridge

- [ ] Map save/state paths back to RomM save/state APIs
- [ ] Write-back support: saves sync to RomM on unmount or periodically
- [ ] Profile-configurable save folder convention

### v0.5 — Polish & Hardening

- [ ] Graceful handling of API downtime (serve from cache only)
- [ ] Background refresh without blocking FUSE operations
- [ ] Systemd service file
- [ ] Installation script for common platforms (MiSTer, RPi, etc.)
- [ ] Man page / usage docs
