#!/bin/bash
set -euo pipefail

# romm-fuse uninstaller — removes binary, config, and systemd service.

INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/romm-fuse"
SERVICE_FILE="/etc/systemd/system/romm-fuse.service"
MOUNT_POINT="/media/fat"

# ─── Colors ──────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}▸${NC} $*"; }
ok()    { echo -e "${GREEN}✓${NC} $*"; }
warn()  { echo -e "${YELLOW}!${NC} $*"; }
die()   { echo -e "${RED}✗${NC} $*" >&2; exit 1; }

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

remote_exec() {
    if [[ -n "${REMOTE_SSH:-}" ]]; then
        ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new "$REMOTE_SSH" "$@"
    else
        eval "$@"
    fi
}

# ─── Main ────────────────────────────────────────────────────────────────────

main() {
    echo ""
    echo -e "${CYAN}romm-fuse uninstaller${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""

    REMOTE_SSH=""
    if [[ "${__ROMM_FUSE_REEXEC:-}" == "1" ]]; then
        info "Running as root (local mode)"
    else
        # Prompt: local or remote
        echo "Uninstall mode:"
        echo "  [1] Local  — uninstall from this machine"
        echo "  [2] Remote — uninstall from another machine via SSH"
        echo ""
        read -rp "  Choose (1/2, default: 1): " install_mode
        install_mode="${install_mode:-1}"

        case "$install_mode" in
            1)
                require_root "$@"
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
        esac
    fi

    # Check if installed
    if ! remote_exec "command -v romm-fuse" &>/dev/null; then
        warn "romm-fuse not found in PATH"
    fi

    # Stop and disable service
    if remote_exec "command -v systemctl" &>/dev/null; then
        if remote_exec "systemctl is-active romm-fuse" &>/dev/null; then
            info "Stopping romm-fuse service..."
            remote_exec "systemctl stop romm-fuse"
            ok "Service stopped"
        fi
        if remote_exec "systemctl is-enabled romm-fuse" &>/dev/null; then
            info "Disabling romm-fuse service..."
            remote_exec "systemctl disable romm-fuse"
            ok "Service disabled"
        fi
    fi

    # Unmount if mounted
    if remote_exec "mountpoint -q '${MOUNT_POINT}'" 2>/dev/null; then
        info "Unmounting ${MOUNT_POINT}..."
        remote_exec "fusermount3 -u '${MOUNT_POINT}' 2>/dev/null || fusermount -u '${MOUNT_POINT}' 2>/dev/null || true"
        ok "Unmounted"
    fi

    # Remove files
    for path in \
        "${INSTALL_DIR}/romm-fuse" \
        "${SERVICE_FILE}" \
        "${CONFIG_DIR}"; do
        if remote_exec "test -e '${path}'" 2>/dev/null; then
            info "Removing ${path}..."
            remote_exec "rm -rf '${path}'"
            ok "Removed"
        fi
    done

    # Reload systemd
    if remote_exec "command -v systemctl" &>/dev/null; then
        remote_exec "systemctl daemon-reload" 2>/dev/null || true
    fi

    # Done
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo -e "${GREEN}Uninstall complete!${NC}"
    echo ""
    echo "Removed:"
    echo "  • Binary:   ${INSTALL_DIR}/romm-fuse"
    echo "  • Config:   ${CONFIG_DIR}/"
    echo "  • Service:  ${SERVICE_FILE}"
    echo ""
}

main "$@"
