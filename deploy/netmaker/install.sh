#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INCUS_COMPOSE="${HOME}/go/bin/incus-compose"
MCP_ENDPOINT="${MCP_ENDPOINT:-http://100.90.37.254:3003/mcp}"
BRIDGE_GRPC_ADDR="${BRIDGE_GRPC_ADDR:-127.0.0.1:18789}"

register_socket_via_grpc() {
  local name="$1"
  local ports_csv="$2"

  if ! command -v grpcurl >/dev/null 2>&1; then
    echo "WARN: grpcurl not found; skipping socket registration for ${name}" >&2
    return 0
  fi

  grpcurl -plaintext \
    -d "{
      \"plugin_id\": \"unix_socket\",
      \"object_path\": \"/org/opdbus/v1/plugins/unix_socket\",
      \"interface_name\": \"org.opdbus.PluginV1\",
      \"method_name\": \"createunixsocket\",
      \"arguments\": [{\"name\":\"$name\",\"ports\":[$ports_csv]}],
      \"actor_id\": \"netmaker-install\",
      \"capability_id\": \"netmaker.socket.registration\"
    }" \
    "$BRIDGE_GRPC_ADDR" \
    operation.v1.PluginService/CallMethod >/dev/null 2>&1 || true
}

# ── 1. Pre-flight ─────────────────────────────────────────────────────────────

if [[ -z "${MQ_PASSWORD:-}" ]]; then
  echo "ERROR: MQ_PASSWORD env var required" >&2
  exit 1
fi

sudo mkdir -p /run/netmaker /etc/netmaker
sudo cp "$SCRIPT_DIR/mosquitto.conf" /etc/netmaker/mosquitto.conf

# ── 2. Create containers via incus-compose ────────────────────────────────────
#
# This is the provisioning entrypoint. The follow-up wiring is D-Bus-owned:
# the socket objects are created by the bridge/plugin path, not by Incus proxy
# devices. Keep the container runtime Debian/Ubuntu-compatible so the install
# flow matches the rest of the estate.

cd "$SCRIPT_DIR"
# "$INCUS_COMPOSE" up --cwd "$SCRIPT_DIR"

# ── 3. Stop containers before rewiring network ────────────────────────────────
# Netmaker stays on the pro license path; the install only rewires the socket
# projection and removes the default container NICs.

for ct in netmaker netmaker-mq netmaker-ui prometheus grafana netmaker-exporter; do
  sudo incus stop "$ct" --force 2>/dev/null || true
done

# ── 4. Remove default NIC from each container ─────────────────────────────────

for ct in netmaker netmaker-mq netmaker-ui prometheus grafana netmaker-exporter; do
  if sudo incus config device show "$ct" 2>/dev/null | grep -q "^eth0:"; then
    sudo incus config device remove "$ct" eth0
    echo "[$ct] removed eth0"
  fi
done

# ── 5. Start containers ───────────────────────────────────────────────────────

register_socket_via_grpc "netmaker" "8081"
register_socket_via_grpc "netmaker-mq" "1883,8883"
register_socket_via_grpc "netmaker-ui" "80"
register_socket_via_grpc "prometheus" "9090"
register_socket_via_grpc "grafana" "3000"
register_socket_via_grpc "netmaker-exporter" "8085"

for ct in netmaker netmaker-mq netmaker-ui prometheus grafana netmaker-exporter; do
  sudo incus start "$ct"
done

# The bridge / projection layer owns socket registration. The installer only
# provisions the containers and asks the bridge to project the sockets into the
# authoritative runtime path.

# ── 6. Reload xray inside wg-xray ────────────────────────────────────────────

sudo incus exec wg-xray -- systemctl reload xray 2>/dev/null \
  || sudo incus exec wg-xray -- s6-svc -h /run/service/xray 2>/dev/null \
  || echo "WARN: could not signal xray — reload manually inside wg-xray"

echo "netmaker install complete"
echo "  API:  http://api.ghostbridge.tech"
echo "  MQTT: broker.ghostbridge.tech:1883"
echo "  UI:   http://app.ghostbridge.tech"
