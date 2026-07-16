# romm-fuse

FUSE filesystem that bridges a [RomM](https://romm.app/) instance to a plain directory tree for emulation systems (MiSTer FPGA, RetroArch, EmulationStation, etc.).

## Install

```bash
curl -sSL https://raw.githubusercontent.com/n3fariox/romm-fuse/main/install.sh | bash
```

This will:
- Detect your architecture (armv7, x86_64, aarch64)
- Download the latest release from GitHub
- Prompt for RomM URL, API token, cache dir, and mount point
- Install a systemd service

### Remote install (e.g. MiSTer over SSH)

```bash
curl -sSL https://raw.githubusercontent.com/n3fariox/romm-fuse/main/install.sh | bash
# Choose [2] Remote when prompted
```

### Uninstall

```bash
curl -sSL https://raw.githubusercontent.com/n3fariox/romm-fuse/main/uninstall.sh | bash
```

## Usage

### Manual

```bash
romm-fuse /media/fat --profile mister
```

### Systemd

```bash
sudo systemctl start romm-fuse
```

## Configuration

Config is loaded from (highest priority first):
1. CLI flags: `--romm-url`, `--token`, `--profile`
2. Environment variables: `ROMM_URL`, `ROMM_TOKEN`, `ROMM_PROFILE`
3. Config file: `~/.config/romm-fuse/config.toml` or `/etc/romm-fuse/config.toml`

### Config file

```toml
romm_url = "http://192.168.1.100:3000"
token = "rmm_your_token_here"
profile = "mister"

# Optional
cache_dir = "/tmp/romm-fuse-cache"
ttl = 300
allow_other = false
```

## Profiles

| Profile | Layout | Notes |
|---------|--------|-------|
| `mister` | `games/NES/rom.nes` | MiSTer FPGA core folder names |
| `retroarch` | `nes/rom.nes` | RetroArch content folders |
| `emulationstation` | `nes/rom.nes` | ES-style short names |
| `custom` | user-defined | Any mapping via `--config my.toml` |

## Building from source

```bash
cargo build --release
./scripts/local_install.sh
```

## License

MIT
