#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INCUS_COMPOSE="${HOME}/go/bin/incus-compose"

# ── 1. Pre-flight ─────────────────────────────────────────────────────────────

if [[ -z "${MQ_PASSWORD:-}" ]]; then
  echo "ERROR: MQ_PASSWORD env var required" >&2
  exit 1
fi

sudo mkdir -p /run/netmaker /etc/netmaker
sudo cp "$SCRIPT_DIR/mosquitto.conf" /etc/netmaker/mosquitto.conf

# ── 2. Create containers via incus-compose ────────────────────────────────────

cd "$SCRIPT_DIR"
"$INCUS_COMPOSE" up --cwd "$SCRIPT_DIR"

# ── 3. Stop containers before rewiring network ────────────────────────────────

for ct in netmaker netmaker-mq netmaker-ui; do
  sudo incus stop "$ct" --force 2>/dev/null || true
done

# ── 4. Remove default NIC from each container ─────────────────────────────────

for ct in netmaker netmaker-mq netmaker-ui; do
  if sudo incus config device show "$ct" 2>/dev/null | grep -q "^eth0:"; then
    sudo incus config device remove "$ct" eth0
    echo "[$ct] removed eth0"
  fi
done

# ── 5. Add unix socket proxy devices on each container ───────────────────────
#   listen=unix:// on host  →  connect=tcp:127.0.0.1:port inside container

sudo incus config device add netmaker api-sock \
  proxy listen=unix:/run/netmaker/api.sock connect=tcp:127.0.0.1:8081 \
  uid=0 gid=0 mode=0660

sudo incus config device add netmaker-mq mqtt-sock \
  proxy listen=unix:/run/netmaker/mq.sock connect=tcp:127.0.0.1:1883 \
  uid=0 gid=0 mode=0660

sudo incus config device add netmaker-mq mqtts-sock \
  proxy listen=unix:/run/netmaker/mqtts.sock connect=tcp:127.0.0.1:8883 \
  uid=0 gid=0 mode=0660

sudo incus config device add netmaker-ui ui-sock \
  proxy listen=unix:/run/netmaker/ui.sock connect=tcp:127.0.0.1:80 \
  uid=0 gid=0 mode=0660

echo "unix socket proxy devices installed"

# ── 6. Wire wg-xray: mount /run/netmaker + container-side TCP→unix proxies ───

if ! sudo incus config device show wg-xray 2>/dev/null | grep -q "^netmaker-socks:"; then
  sudo incus config device add wg-xray netmaker-socks \
    disk source=/run/netmaker path=/run/netmaker
fi

while IFS='|' read -r dev port sock; do
  dev="${dev// /}"
  port="${port// /}"
  sock="${sock// /}"
  if ! sudo incus config device show wg-xray 2>/dev/null | grep -q "^${dev}:"; then
    sudo incus config device add wg-xray "$dev" \
      proxy bind=container \
      "listen=tcp:127.0.0.1:${port}" \
      "connect=unix:/run/netmaker/${sock}"
  fi
done << 'DEVICES'
netmaker-api-tcp   | 18081 | api.sock
netmaker-mqtt-tcp  | 11883 | mq.sock
netmaker-mqtts-tcp | 18883 | mqtts.sock
netmaker-ui-tcp    | 18082 | ui.sock
DEVICES

echo "wg-xray devices wired"

# ── 7. Start containers ───────────────────────────────────────────────────────

for ct in netmaker netmaker-mq netmaker-ui; do
  sudo incus start "$ct"
done

# ── 8. Reload xray inside wg-xray ────────────────────────────────────────────

sudo incus exec wg-xray -- systemctl reload xray 2>/dev/null \
  || sudo incus exec wg-xray -- s6-svc -h /run/service/xray 2>/dev/null \
  || echo "WARN: could not signal xray — reload manually inside wg-xray"

echo "netmaker install complete"
echo "  API:  http://api.ghostbridge.tech  (→ unix:/run/netmaker/api.sock)"
echo "  MQTT: broker.ghostbridge.tech:1883 (→ unix:/run/netmaker/mq.sock)"
echo "  UI:   http://app.ghostbridge.tech  (→ unix:/run/netmaker/ui.sock)"
