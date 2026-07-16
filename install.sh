#!/bin/bash
set -euo pipefail

# romm-fuse installer — works with: curl -sSL https://.../install.sh | bash
# Supports local and remote (SSH) installation on MiSTer and Linux systems.

REPO="n3fariox/romm-fuse"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/romm-fuse"
SERVICE_FILE="/etc/systemd/system/romm-fuse.service"
DEFAULT_CACHE_DIR="/tmp/romm-fuse-cache"

# ─── Colors ──────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}▸${NC} $*"; }
ok()    { echo -e "${GREEN}✓${NC} $*"; }
warn()  { echo -e "${YELLOW}!${NC} $*"; }
err()   { echo -e "${RED}✗${NC} $*" >&2; }
die()   { err "$@"; exit 1; }

require_root() {
    if [[ $EUID -ne 0 ]]; then
        if [[ -f "$0" ]]; then
            info "Re-running with sudo..."
            exec sudo env __ROMM_FUSE_REEXEC=1 "$0" "$@"
        else
            die "This script requires root. Re-run with: sudo bash"
        fi
    fi
}

# ─── Helpers ─────────────────────────────────────────────────────────────────

remote_exec() {
    if [[ -n "${REMOTE_SSH:-}" ]]; then
        ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new "$REMOTE_SSH" "$@"
    else
        eval "$@"
    fi
}

remote_copy() {
    if [[ -n "${REMOTE_SSH:-}" ]]; then
        scp -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new "$1" "${REMOTE_SSH}:$2"
    else
        cp "$1" "$2"
    fi
}

cleanup() {
    rm -rf "${TMP_DIR:-}"
}
trap cleanup EXIT

# ─── Prompts ─────────────────────────────────────────────────────────────────

prompt_cache_dir() {
    echo ""
    info "Cache directory"
    echo "  ROMs are cached locally to avoid repeated API calls."
    echo "  Default: ${DEFAULT_CACHE_DIR}"
    echo ""
    read -rp "  Cache dir (default: ${DEFAULT_CACHE_DIR}): " CACHE_DIR
    CACHE_DIR="${CACHE_DIR:-$DEFAULT_CACHE_DIR}"
    ok "Cache dir: ${CACHE_DIR}"
}

prompt_mount_point() {
    echo ""
    info "Mount point"
    echo "  romm-fuse will mount your RomM library at this path."
    echo ""

    # Show existing MiSTer directories if /media/fat exists
    if [[ -d "/media/fat" ]] || remote_exec "test -d '/media/fat'" 2>/dev/null; then
        echo "  Existing MiSTer directories:"
        while IFS= read -r dir; do
            [[ -n "$dir" ]] && echo "    ${dir}"
        done < <(remote_exec "ls -1d /media/fat/*/ 2>/dev/null | head -20" 2>/dev/null || true)
        echo ""
    fi

    while true; do
        read -rp "  Mount point (default: /media/fat): " MOUNT_POINT
        MOUNT_POINT="${MOUNT_POINT:-/media/fat}"

        # Check if already mounted
        if remote_exec "mountpoint -q '${MOUNT_POINT}'" 2>/dev/null; then
            warn "${MOUNT_POINT} is already mounted!"
            echo "    Unmount first, or choose a different path."
            echo ""
            continue
        fi
        break
    done
    ok "Mount point: ${MOUNT_POINT}"
}

# ─── Architecture Detection ──────────────────────────────────────────────────

detect_arch() {
    local arch
    arch=$(remote_exec "uname -m")
    case "$arch" in
        armv7l|armv7|armhf)
            RELEASE_ARCH="armv7"
            LIBC="musleabihf"
            ;;
        x86_64|amd64)
            RELEASE_ARCH="x86_64"
            LIBC="musl"
            ;;
        aarch64|arm64)
            RELEASE_ARCH="aarch64"
            LIBC="musl"
            ;;
        *)
            die "Unsupported architecture: $arch"
            ;;
    esac
    ok "Detected architecture: ${RELEASE_ARCH} (libc: ${LIBC})"
}

# ─── GitHub Release ──────────────────────────────────────────────────────────

get_latest_version() {
    local version
    version=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' | head -1 | sed -E 's/.*"tag_name": *"v?([^"]+)".*/v\1/')
    if [[ -z "$version" ]]; then
        die "Could not fetch latest release from GitHub"
    fi
    echo "$version"
}

download_release() {
    local version="$1"
    local base="romm-fuse-${version}-${RELEASE_ARCH}-linux-${LIBC}"
    local tarball="${base}.tar.gz"
    local checksum="${base}.tar.gz.sha256"
    local url="https://github.com/${REPO}/releases/download/${version}/${tarball}"
    local sha_url="https://github.com/${REPO}/releases/download/${version}/${checksum}"

    TMP_DIR=$(mktemp -d)
    info "Downloading ${tarball}..."

    if ! curl -fsSL -o "${TMP_DIR}/${tarball}" "$url"; then
        die "Failed to download release. Check that version ${version} has a binary for ${RELEASE_ARCH}-linux-${LIBC}"
    fi

    if curl -fsSL -o "${TMP_DIR}/${checksum}" "$sha_url" 2>/dev/null; then
        info "Verifying checksum..."
        (cd "$TMP_DIR" && echo "$(cat "${checksum}")  ${tarball}" | sha256sum -c -) \
            || die "Checksum verification failed"
        ok "Checksum verified"
    else
        warn "No checksum file found, skipping verification"
    fi

    info "Extracting..."
    tar -xzf "${TMP_DIR}/${tarball}" -C "$TMP_DIR" --strip-components=1
    if [[ ! -f "${TMP_DIR}/romm-fuse" ]]; then
        die "Binary 'romm-fuse' not found in tarball"
    fi
    chmod +x "${TMP_DIR}/romm-fuse"
    ok "Release extracted"
}

# ─── Config Setup ────────────────────────────────────────────────────────────

setup_config() {
    remote_exec "mkdir -p '${CONFIG_DIR}'"

    local config_exists
    config_exists=$(remote_exec "test -f '${CONFIG_DIR}/config.toml' && echo yes || echo no")

    if [[ "$config_exists" == "yes" ]]; then
        ok "Config already exists at ${CONFIG_DIR}/config.toml, skipping"
        return
    fi

    echo ""
    info "Setting up configuration..."
    read -rp "  RomM URL (e.g., http://192.168.1.100:3000): " romm_url
    read -rp "  RomM API Token: " romm_token

    if [[ -z "$romm_url" || -z "$romm_token" ]]; then
        die "URL and token are required"
    fi

    local config_content="# romm-fuse configuration
romm_url = \"${romm_url}\"
token = \"${romm_token}\"
profile = \"mister\"
cache_dir = \"${CACHE_DIR}\"
allow_other = true
"
    if [[ -n "${REMOTE_SSH:-}" ]]; then
        echo "$config_content" | ssh "$REMOTE_SSH" "cat > '${CONFIG_DIR}/config.toml'"
    else
        echo "$config_content" > "${CONFIG_DIR}/config.toml"
    fi
    ok "Config written to ${CONFIG_DIR}/config.toml"
}

# ─── Systemd Setup ───────────────────────────────────────────────────────────

setup_systemd() {
    if ! remote_exec "command -v systemctl" &>/dev/null; then
        warn "systemd not found on target — skipping service setup"
        return
    fi

    local service_content="[Unit]
Description=RomM FUSE Filesystem
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStartPre=/bin/mkdir -p ${MOUNT_POINT}
ExecStartPre=/bin/chmod 755 ${MOUNT_POINT}
ExecStart=${INSTALL_DIR}/romm-fuse ${MOUNT_POINT} --profile mister --foreground --allow-other
ExecStop=/usr/bin/fusermount3 -u ${MOUNT_POINT}
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
"
    info "Installing systemd service..."
    if [[ -n "${REMOTE_SSH:-}" ]]; then
        echo "$service_content" | ssh "$REMOTE_SSH" "cat > '${SERVICE_FILE}'"
    else
        echo "$service_content" > "${SERVICE_FILE}"
    fi

    remote_exec "systemctl daemon-reload"
    remote_exec "systemctl enable romm-fuse"
    ok "Systemd service installed and enabled"
}

# ─── Main ────────────────────────────────────────────────────────────────────

main() {
    echo ""
    echo -e "${CYAN}romm-fuse installer${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""

    # Check dependencies
    for cmd in curl tar; do
        command -v "$cmd" &>/dev/null || die "'$cmd' is required but not found"
    done

    REMOTE_SSH=""
    if [[ "${__ROMM_FUSE_REEXEC:-}" == "1" ]]; then
        info "Running as root (local mode)"
    else
        # Prompt: local or remote
        echo "Install mode:"
        echo "  [1] Local  — install on this machine"
        echo "  [2] Remote — install on another machine via SSH"
        echo ""
        read -rp "  Choose (1/2, default: 1): " install_mode
        install_mode="${install_mode:-1}"

        case "$install_mode" in
            1)
                require_root "$@"
                info "Installing locally..."
                ;;
            2)
                read -rp "  SSH host (IP or hostname): " ssh_host
                [[ -z "$ssh_host" ]] && die "Host is required"
                read -rp "  SSH user (default: root): " ssh_user
                ssh_user="${ssh_user:-root}"
                REMOTE_SSH="${ssh_user}@${ssh_host}"

                info "Testing SSH connection to ${REMOTE_SSH}..."
                if ! ssh -o ConnectTimeout=5 "$REMOTE_SSH" "echo ok" &>/dev/null; then
                    die "Cannot connect to ${REMOTE_SSH}. Check SSH access."
                fi
                ok "SSH connection verified"
                ;;
            *)
                die "Invalid choice: $install_mode"
                ;;
        esac
    fi

    # Detect architecture
    detect_arch

    # Fetch latest release
    info "Checking latest release..."
    local version
    version=$(get_latest_version)
    ok "Latest version: ${version}"

    # Download and extract
    download_release "$version"

    # Install binary
    info "Installing binary to ${INSTALL_DIR}/romm-fuse..."
    remote_exec "mkdir -p '${INSTALL_DIR}'"
    remote_copy "${TMP_DIR}/romm-fuse" "${INSTALL_DIR}/romm-fuse"
    remote_exec "chmod 755 '${INSTALL_DIR}/romm-fuse'"
    ok "Binary installed"

    # Verify installation
    local installed_version
    installed_version=$(remote_exec "${INSTALL_DIR}/romm-fuse --version 2>/dev/null" || true)
    if [[ -n "$installed_version" ]]; then
        ok "Installed: ${installed_version}"
    fi

    # Setup config
    prompt_cache_dir
    prompt_mount_point
    setup_config

    # Setup systemd
    setup_systemd

    # Done
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo -e "${GREEN}Installation complete!${NC}"
    echo ""
    echo "Quick start:"
    echo "  1. Edit config: ${CONFIG_DIR}/config.toml"
    echo "  2. Mount:       romm-fuse ${MOUNT_POINT}"
    echo "  3. Or use systemd: systemctl start romm-fuse"
    echo ""
}

main "$@"
