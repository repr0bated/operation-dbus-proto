#!/bin/bash
# Uninstall script
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"

KEEP_DATA="false"
[[ "${1:-}" == "--keep-data" ]] && KEEP_DATA="true"

echo -e "${YELLOW}op-dbus-v2 Uninstall${NC}"
echo ""

[[ $EUID -ne 0 ]] && { log_error "Run as root"; exit 1; }

echo -e "${RED}This will remove:${NC}"
echo "  - Binaries from /usr/local/sbin/op-*"
echo "  - Systemd services"
echo "  - Nginx configuration"
echo "  - /etc/op-dbus"
[[ "$KEEP_DATA" == "false" ]] && echo "  - /var/log/op-dbus, /var/lib/op-dbus"
echo ""
read -p "Continue? [y/N]: " confirm
[[ ! "$confirm" =~ ^[Yy]$ ]] && { echo "Cancelled"; exit 0; }

log_step "Stopping services"
for svc in op-web op-dbus-service; do
    systemctl stop "$svc" 2>/dev/null || true
    systemctl disable "$svc" 2>/dev/null || true
    rm -f "/etc/systemd/system/${svc}.service"
done
systemctl daemon-reload

log_step "Removing binaries"
rm -f /usr/local/sbin/op-*
rm -f /usr/local/sbin/dbus-agent

log_step "Removing nginx config"
rm -f /etc/nginx/sites-enabled/op-web
rm -f /etc/nginx/sites-available/op-web
nginx -t 2>/dev/null && systemctl reload nginx 2>/dev/null || true

log_step "Removing config"
rm -rf /etc/op-dbus

[[ "$KEEP_DATA" == "false" ]] && {
    log_step "Removing data"
    rm -rf /var/log/op-dbus /var/lib/op-dbus
}

log_success "Uninstall complete"
