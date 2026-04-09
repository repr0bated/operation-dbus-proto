#!/bin/bash
# Deploy Qdrant vector database in its own Incus container.
#
# Container: qdrant
# Static IP:  10.149.181.190 (on incusbr0)
# Ports:      6333 (REST), 6334 (gRPC)
# Storage:    BTRFS-backed volume at /var/lib/qdrant
#
# Usage:
#   sudo ./qdrant.sh          # Create and start
#   sudo ./qdrant.sh teardown  # Remove container + volume

set -euo pipefail

CONTAINER="qdrant"
STATIC_IP="10.149.181.190"
STORAGE_POOL="default"
VOLUME_NAME="qdrant-data"
QDRANT_VERSION="latest"
QDRANT_STORAGE="/var/lib/qdrant"

log() { echo "[qdrant] $*"; }

teardown() {
    log "Tearing down Qdrant container..."
    incus stop "$CONTAINER" --force 2>/dev/null || true
    incus delete "$CONTAINER" 2>/dev/null || true
    incus storage volume delete "$STORAGE_POOL" "$VOLUME_NAME" 2>/dev/null || true
    log "Done."
    exit 0
}

[[ "${1:-}" == "teardown" ]] && teardown

# --- Idempotent creation ---

if incus info "$CONTAINER" &>/dev/null; then
    log "Container '$CONTAINER' already exists"
    if [[ "$(incus info "$CONTAINER" | awk '/^Status:/{print $2}')" != "RUNNING" ]]; then
        log "Starting container..."
        incus start "$CONTAINER"
    fi
else
    log "Creating container '$CONTAINER' (Debian 12)..."
    incus launch images:debian/12 "$CONTAINER"
    sleep 3
fi

# --- Static IP ---

if ! incus config device get "$CONTAINER" eth0 ipv4.address 2>/dev/null | grep -q "$STATIC_IP"; then
    log "Assigning static IP $STATIC_IP..."
    incus config device override "$CONTAINER" eth0 ipv4.address="$STATIC_IP" 2>/dev/null \
        || incus config device add "$CONTAINER" eth0 nic \
            nictype=bridged parent=incusbr0 ipv4.address="$STATIC_IP" 2>/dev/null \
        || log "WARNING: could not assign static IP — check incusbr0 config"
fi

# --- BTRFS storage volume ---

if ! incus storage volume show "$STORAGE_POOL" "$VOLUME_NAME" &>/dev/null; then
    log "Creating storage volume '$VOLUME_NAME'..."
    incus storage volume create "$STORAGE_POOL" "$VOLUME_NAME"
fi

if ! incus config device show "$CONTAINER" | grep -q "$VOLUME_NAME"; then
    log "Attaching storage volume..."
    incus config device add "$CONTAINER" "$VOLUME_NAME" disk \
        pool="$STORAGE_POOL" source="$VOLUME_NAME" path="$QDRANT_STORAGE"
fi

# --- Install Qdrant binary ---

if ! incus exec "$CONTAINER" -- test -f /usr/local/bin/qdrant; then
    log "Installing Qdrant binary..."
    incus exec "$CONTAINER" -- bash -c '
        apt-get update -qq && apt-get install -y -qq curl ca-certificates >/dev/null 2>&1
        curl -sL "https://github.com/qdrant/qdrant/releases/latest/download/qdrant-x86_64-unknown-linux-musl.tar.gz" \
            | tar xz -C /usr/local/bin/
        chmod +x /usr/local/bin/qdrant
    '
    log "Qdrant binary installed"
else
    log "Qdrant binary already present"
fi

# --- Systemd service ---

incus exec "$CONTAINER" -- bash -c "cat > /etc/systemd/system/qdrant.service << 'EOF'
[Unit]
Description=Qdrant Vector Database
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/qdrant --storage-path /var/lib/qdrant/storage --host 0.0.0.0
Restart=always
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF"

incus exec "$CONTAINER" -- systemctl daemon-reload
incus exec "$CONTAINER" -- systemctl enable --now qdrant

# --- Health check ---

log "Waiting for Qdrant to be ready..."
for i in $(seq 1 15); do
    if curl -sf "http://$STATIC_IP:6333/collections" >/dev/null 2>&1; then
        log "Qdrant is running at http://$STATIC_IP:6333"
        exit 0
    fi
    sleep 2
done

log "WARNING: Qdrant did not respond within 30s — check 'incus exec $CONTAINER -- journalctl -u qdrant'"
exit 1
