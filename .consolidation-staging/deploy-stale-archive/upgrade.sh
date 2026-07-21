#!/bin/bash
# Quick upgrade script
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"

export SERVICE_USER="${SERVICE_USER:-jeremy}"
export PROJECT_DIR="${PROJECT_DIR:-$(cd "$SCRIPT_DIR/.." && pwd)}"
export INSTALL_DIR="${INSTALL_DIR:-/usr/local/sbin}"
export DRY_RUN="false"

RESTART="true"
[[ "${1:-}" == "--no-restart" ]] && RESTART="false"

echo -e "${BLUE}op-dbus-v2 Upgrade${NC}"
echo ""

[[ $EUID -ne 0 ]] && { log_error "Run as root"; exit 1; }
[[ ! -f "$PROJECT_DIR/Cargo.toml" ]] && { log_error "Invalid project dir"; exit 1; }

find_cargo || { log_error "Cargo not found"; exit 1; }

log_step "Pulling changes"
cd "$PROJECT_DIR"
[[ -d .git ]] && sudo -u "$SERVICE_USER" git pull 2>/dev/null || true

log_step "Building"
source "$SCRIPT_DIR/lib/build.sh"
build_release "$PROJECT_DIR" "$SERVICE_USER"

log_step "Installing"
source "$SCRIPT_DIR/lib/install-binaries.sh"
install_binaries "$PROJECT_DIR" "$INSTALL_DIR"

if [[ "$RESTART" == "true" ]]; then
    log_step "Restarting services"
    for svc in op-web op-dbus-service; do
        [[ -f "/etc/systemd/system/${svc}.service" ]] && systemctl restart "$svc" || true
    done
fi

log_success "Upgrade complete"
