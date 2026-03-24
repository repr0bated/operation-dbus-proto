# ICUS Container Management Script
# Manage Chimera Linux containers

set -euo pipefail

ICUS_NAME="${1:-}"
COMMAND="${2:-}"
ICUS_ROOT="/var/lib/icus"

if [[ -z "$ICUS_NAME" ]] || [[ -z "$COMMAND" ]]; then
    echo "Usage: $0 <icus-name> <command>"
    echo ""
    echo "Commands:"
    echo "  start      Start the ICUS container"
    echo "  stop       Stop the ICUS container"
    echo "  restart    Restart the ICUS container"
    echo "  status     Check container status"
    echo "  shell      Enter container shell"
    echo "  logs       View container logs"
    echo "  wg-status  Check WireGuard connection status"
    echo "  wg-restart Restart WireGuard tunnel"
    echo "  backup     Create backup of container"
    echo "  destroy    Destroy container (DANGER!)"
    echo ""
    echo "Examples:"
    echo "  $0 chimera-icus start"
    echo "  $0 chimera-icus shell"
    exit 1
fi

ICUS_PATH="${ICUS_ROOT}/${ICUS_NAME}"

if [[ ! -d "$ICUS_PATH" ]]; then
    echo "ERROR: ICUS container not found: $ICUS_NAME"
    exit 1
fi

case "$COMMAND" in
    start)
        echo "Starting ICUS container: $ICUS_NAME"
        systemctl start "icus-${ICUS_NAME}"
        ;;
    stop)
        echo "Stopping ICUS container: $ICUS_NAME"
        systemctl stop "icus-${ICUS_NAME}"
        ;;
    restart)
        echo "Restarting ICUS container: $ICUS_NAME"
        systemctl restart "icus-${ICUS_NAME}"
        ;;
    status)
        systemctl status "icus-${ICUS_NAME}"
        ;;
    shell)
        echo "Entering ICUS container shell..."
        exec chroot "$ICUS_PATH/rootfs" /bin/sh
        ;;
    logs)
        journalctl -u "icus-${ICUS_NAME}" -f
        ;;
    wg-status)
        echo "WireGuard Status:"
        chroot "$ICUS_PATH/rootfs" wg show
        ;;
    wg-restart)
        echo "Restarting WireGuard..."
        chroot "$ICUS_PATH/rootfs" wg-quick down wg0 2>/dev/null || true
        chroot "$ICUS_PATH/rootfs" wg-quick up wg0
        ;;
    backup)
        BACKUP_FILE="${ICUS_NAME}-backup-$(date +%Y%m%d-%H%M%S).tar.gz"
        echo "Creating backup: $BACKUP_FILE"
        tar -czf "$BACKUP_FILE" -C "$ICUS_ROOT" "$ICUS_NAME"
        echo "Backup created: $BACKUP_FILE"
        ;;
    destroy)
        echo "WARNING: This will DESTROY the container $ICUS_NAME"
        read -p "Type 'DESTROY' to confirm: " confirm
        if [[ "$confirm" == "DESTROY" ]]; then
            systemctl stop "icus-${ICUS_NAME}" 2>/dev/null || true
            rm -rf "$ICUS_PATH"
            rm -f "/usr/local/bin/icus-${ICUS_NAME}"
            rm -f "/etc/systemd/system/icus-${ICUS_NAME}.service"
            systemctl daemon-reload
            echo "Container destroyed: $ICUS_NAME"
        else
            echo "Aborted."
        fi
        ;;
    *)
        echo "Unknown command: $COMMAND"
        exit 1
        ;;
esac
