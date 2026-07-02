#!/bin/bash
# Setup Qdrant in an Incus container with no NIC, exposed via a single
# Unix socket at /run/qdrant.sock (gRPC port 6334).
#
# Usage: sudo bash deploy/setup-qdrant-incus.sh
set -euo pipefail

CONTAINER="qdrant"
QDRANT_VERSION="v1.18.1"
QDRANT_URL="https://github.com/qdrant/qdrant/releases/download/${QDRANT_VERSION}/qdrant-x86_64-unknown-linux-musl.tar.gz"
STORAGE_POOL="default"
# Local gRPC port — xray proxies 127.0.0.1:$QDRANT_PORT → container via ds transport.
QDRANT_PORT=6334

log() { echo "[qdrant-setup] $*"; }
die() { echo "[qdrant-setup] ERROR: $*" >&2; exit 1; }

# ─── preflight ────────────────────────────────────────────────────────────────

[[ $EUID -eq 0 ]] || die "Run as root: sudo bash $0"
command -v incus &>/dev/null || die "incus not found"

# Initialise Incus storage if this is a fresh system
if ! incus storage list --format csv 2>/dev/null | grep -q .; then
    log "Initialising Incus with default storage pool (dir backend)"
    incus admin init --preseed <<'PRESEED'
config: {}
networks: []
storage_pools:
- config: {}
  description: ""
  name: default
  driver: dir
profiles:
- config: {}
  description: ""
  devices:
    root:
      path: /
      pool: default
      type: disk
  name: default
PRESEED
fi

# ─── download Qdrant on the host ──────────────────────────────────────────────

QDRANT_TMP=$(mktemp -d)
trap "rm -rf $QDRANT_TMP" EXIT

if [[ ! -f "$QDRANT_TMP/qdrant" ]]; then
    log "Downloading Qdrant ${QDRANT_VERSION} ..."
    curl -fL "$QDRANT_URL" | tar -xz -C "$QDRANT_TMP"
fi

log "Qdrant binary: $(file $QDRANT_TMP/qdrant | cut -d: -f2 | xargs)"

# ─── create container (no NIC) ────────────────────────────────────────────────

if incus info "$CONTAINER" &>/dev/null; then
    log "Container '$CONTAINER' already exists — stopping and recreating"
    incus stop "$CONTAINER" --force 2>/dev/null || true
    incus delete "$CONTAINER"
fi

log "Creating container '$CONTAINER' (Alpine 3.20, no NIC)"

# Launch with default profile to get storage, then strip the NIC
incus launch images:alpine/3.20 "$CONTAINER" \
    --storage "$STORAGE_POOL" \
    --no-profiles \
    -d root,path=/,pool="$STORAGE_POOL",size=4GiB

# Give Alpine a moment to boot
sleep 3

# ─── add Unix socket proxy BEFORE removing NIC ────────────────────────────────
# Incus proxy: host listens on /run/qdrant.sock, bridges to container TCP 6334.
# Xray's ds outbound then routes qdrant.ghostbridge.tech → /run/qdrant.sock
# via UNIX_SOCKET_ENDPOINTS=qdrant:/run/qdrant.sock:qdrant.ghostbridge.tech.

SOCKET_PATH="/run/qdrant.sock"

incus config device add "$CONTAINER" qdrant proxy \
    listen=unix:"$SOCKET_PATH" \
    connect=tcp:127.0.0.1:6334 \
    bind=host

log "Proxy device: $SOCKET_PATH -> container:6334 (xray ds outbound will use this)"

# ─── install Qdrant inside the container ──────────────────────────────────────

log "Pushing Qdrant binary into container"
incus file push "$QDRANT_TMP/qdrant" "$CONTAINER/usr/local/bin/qdrant" --mode 0755

log "Configuring Qdrant"
incus exec "$CONTAINER" -- mkdir -p /var/lib/qdrant /etc/qdrant

incus exec "$CONTAINER" -- sh -c 'cat > /etc/qdrant/config.yaml' <<'CONF'
storage:
  # Qdrant's storage root must contain collections/ directly. The host mount
  # for this system is /var/lib/qdrant/storage, which contains
  # collections/repomix_rag.
  storage_path: /var/lib/qdrant/storage

service:
  host: 127.0.0.1
  http_port: 6333
  grpc_port: 6334
  enable_cors: true

cluster:
  enabled: false

telemetry_disabled: true
CONF

# ─── OpenRC service ───────────────────────────────────────────────────────────

incus exec "$CONTAINER" -- sh -c 'cat > /etc/init.d/qdrant' <<'INIT'
#!/sbin/openrc-run

name="qdrant"
description="Qdrant vector database"

command="/usr/local/bin/qdrant"
command_args="--config-path /etc/qdrant/config.yaml"
command_background="yes"
pidfile="/run/qdrant.pid"
output_log="/var/log/qdrant.log"
error_log="/var/log/qdrant.log"

depend() {
    need localmount
    after bootmisc
}
INIT

incus exec "$CONTAINER" -- chmod +x /etc/init.d/qdrant
incus exec "$CONTAINER" -- rc-update add qdrant default
incus exec "$CONTAINER" -- rc-service qdrant start

# ─── verify Qdrant is up inside the container ─────────────────────────────────

log "Waiting for Qdrant to start..."
for i in $(seq 1 15); do
    if incus exec "$CONTAINER" -- wget -qO- http://127.0.0.1:6333/healthz 2>/dev/null | grep -q ok; then
        log "Qdrant is healthy inside the container"
        break
    fi
    sleep 1
done

# ─── remove NIC (container is now sealed, no network) ────────────────────────

log "Removing container NIC (sealing network)"
# Alpine has eth0 added by default profile; remove any NIC devices
for dev in eth0 eth1 enp0 enp1; do
    incus config device remove "$CONTAINER" "$dev" 2>/dev/null && \
        log "  Removed NIC: $dev" || true
done

# ─── set socket permissions ───────────────────────────────────────────────────

# The proxy device creates the socket as root; make it group-accessible
SOCKET_GID=$(id -g jeremy 2>/dev/null || echo 0)
chown root:"$SOCKET_GID" "$SOCKET_PATH" 2>/dev/null || true
chmod 660 "$SOCKET_PATH" 2>/dev/null || true

# ─── test connectivity from host via Unix socket ──────────────────────────────

log "Testing gRPC health via Unix socket..."
if command -v grpc_health_probe &>/dev/null; then
    grpc_health_probe -addr unix:"$SOCKET_PATH" && log "gRPC health OK" || true
else
    # Fall back to socat + HTTP check
    if command -v socat &>/dev/null; then
        result=$(echo -e "GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n" \
            | socat - UNIX-CONNECT:"$SOCKET_PATH" 2>/dev/null | tail -1 || true)
        log "Socket check: ${result:-no HTTP response (gRPC-only socket is normal)}"
    fi
fi

# ─── summary ─────────────────────────────────────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════════════"
echo " Qdrant ${QDRANT_VERSION} installed in Incus container '${CONTAINER}'"
echo " Unix socket      : ${SOCKET_PATH}  (Incus proxy → container:6334)"
echo " Container NIC    : none (sealed)"
echo " Storage          : /var/lib/qdrant/storage (inside container)"
echo "═══════════════════════════════════════════════════════"
echo ""
echo " Xray routes 127.0.0.1:${QDRANT_PORT} → ${SOCKET_PATH} via ds outbound."
echo " Set in your xray environment:"
echo "   UNIX_SOCKET_ENDPOINTS=qdrant:${SOCKET_PATH}:qdrant.ghostbridge.tech"
echo ""
echo " rag-ingest / cognitive-mcp connects to:"
echo "   COGNITIVE_MCP_QDRANT_URL=http://127.0.0.1:${QDRANT_PORT}  (default)"
echo ""
echo " Check status:"
echo "   incus exec ${CONTAINER} -- rc-service qdrant status"
echo "   incus exec ${CONTAINER} -- wget -qO- http://127.0.0.1:6333/collections"
