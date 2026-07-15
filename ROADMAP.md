# romm-fuse: FUSE Filesystem Bridging RomM to Any Directory-Based Emulation System

## Overview

A read-only FUSE filesystem in Rust that presents a RomM instance's ROM library as a
plain directory tree. Comes with built-in mapping profiles for common systems
(MiSTer FPGA, RetroArch, EmulationStation, etc.) and supports custom TOML profiles
for any directory-based frontend. Runs directly on the target device's Linux
environment (MiSTer Debian, Raspberry Pi, etc.).

## Quick Start

```bash
# Install dependencies (Debian/Ubuntu)
sudo apt install fuse3 libfuse3-dev

# Build
cargo build --release

# Run (interactive — will prompt for URL and token)
./target/release/romm-fuse /mnt/games

# Or with CLI flags
./target/release/romm-fuse /mnt/games \
  --romm-url http://192.168.1.100:3000 \
  --token rmm_your_token_here \
  --profile mister
```

## Configuration

Credentials are resolved with this priority chain (highest first):

1. **CLI flags** — `--romm-url`, `--token`, `--profile`
2. **Environment variables** — `ROMM_URL`, `ROMM_TOKEN`, `ROMM_PROFILE`
3. **Config file** — `~/.config/romm-fuse/config.toml` or `/etc/romm-fuse/config.toml`
4. **Interactive prompt** — prompts on stdin if nothing else is set

### Config file (`~/.config/romm-fuse/config.toml`)

```toml
romm_url = "http://192.168.1.100:3000"
token = "rmm_your_token_here"
profile = "mister"

# Optional overrides
# cache_dir = "/var/cache/romm-fuse"
# ttl = 300
# allow_other = false
```

### Environment variables

```bash
export ROMM_URL="http://192.168.1.100:3000"
export ROMM_TOKEN="rmm_your_token_here"
export ROMM_PROFILE="mister"
```

## Installation

### Manual

```bash
cargo build --release
sudo install -m 755 target/release/romm-fuse /usr/local/bin/romm-fuse
sudo mkdir -p /etc/romm-fuse
sudo cp examples/config.toml /etc/romm-fuse/config.toml
# Edit config.toml with your details
```

### Using install script

```bash
./install.sh
```

### Systemd service

```bash
sudo cp examples/systemd/romm-fuse.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now romm-fuse
```

### fstab (for boot-time mount)

```
/usr/local/bin/romm-fuse /media/fat/games fuse.ro,_netdev,auto,x-systemd.requires=network-online.target 0 0
```

## Architecture

```
romm-fuse
├── main.rs              # CLI args + mount
├── lib.rs               # Library re-exports for integration tests
├── api/
│   ├── client.rs        # reqwest HTTP client for RomM API (bearer auth)
│   └── types.rs         # Serde structs for Platform/Rom/File schemas
├── fs/
│   ├── romm_fs.rs       # fuser::Filesystem impl (the FUSE bridge)
│   ├── tree.rs          # in-memory directory tree (platforms→ROMs→files)
│   └── cache.rs         # local disk cache for downloaded ROMs
├── config.rs            # CLI args, env vars, config file resolution
└── profiles/
    ├── builtins.rs      # Built-in profile loader
    ├── mister.toml      # MiSTer FPGA mapping
    ├── retroarch.toml   # RetroArch mapping
    └── emulationstation.toml
```

## Virtual Directory Layout

The directory layout depends on which **mapping profile** is active:

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
  --romm-url <URL>          RomM instance URL (env: ROMM_URL)
  --token <TOKEN>           Bearer token for RomM API (env: ROMM_TOKEN)
  --profile <NAME>          Built-in profile: mister, retroarch, emulationstation (env: ROMM_PROFILE)
  --config <FILE>           Custom profile TOML (use instead of --profile)
  --config-file <FILE>      Config file path (default: ~/.config/romm-fuse/config.toml)
  --cache-dir <DIR>         Local cache directory (default: /tmp/romm-fuse-cache)
  --chunk-size <BYTES>      Chunk size for HTTP Range reads (default: 262144 = 256KB)
  --allow-other             Pass allow_other to FUSE mount
  --ttl <SECONDS>           How long to cache API responses (default: 300)
  --foreground              Don't daemonize, stay in foreground
```

## Profile Format (TOML)

```toml
[profile]
name = "mister"
prefix = "games"           # optional path prefix prepended to all platforms

[platforms]
# RomM slug = directory folder name
"nes"        = "NES"
"snes"       = "SNES"
"genesis"    = "Genesis"
"game-boy"   = "GAMEBOY"
"gba"        = "GBA"
"playstation" = "PSX"
# ... any RomM platform slug
```

Platforms not listed in the mapping are skipped (not exposed in the filesystem).

## Dependencies

- **`fuser` 0.17** — FUSE impl (sync, well-tested)
- **`reqwest` 0.12** (blocking + json) — RomM API calls
- **`clap` 4** (with `env` feature) — CLI parsing + env var support
- **`toml` 0.8** — profile config
- **`serde`/`serde_json`** — API response deserialization
- **`sha2`** — cache key hashing
- **`atty`** — terminal detection for interactive prompts

## Roadmap

### v0.1 — Minimal Working FS ✓

- [x] Project skeleton (`cargo init`, deps, directory structure, clap CLI)
- [x] RomM API client — types.rs (Platform, SimpleRom, RomFile schemas), client.rs
- [x] Profile system — built-in mister/retroarch/emulationstation profiles, custom TOML loader
- [x] Tree + inodes — build in-memory tree from API, allocate inodes, path lookup
- [x] ROM cache — disk-backed cache with SHA256 keys, download-on-demand
- [x] FUSE impl — wire everything into `fuser::Filesystem` trait
- [x] Config system — CLI flags > env vars > config file > interactive prompt
- [x] Systemd service file + install script
- [x] Testing — 6 integration tests passing

### v0.2 — Chunk-Based Streaming (in progress)

- [x] HTTP Range support for partial reads (`download_range()` in client)
- [x] Chunked cache with LRU eviction for large ISOs
- [x] Configurable chunk size (default 256KB, `--chunk-size` flag)
- [x] `open()` no longer blocks on full download
- [ ] Read-ahead / sequential read optimization

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
- [ ] Man page / usage docs
