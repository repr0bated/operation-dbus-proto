#!/bin/sh
# Deploy op-assistant-grpc s6-rc service on Artix Linux.
#
# Copies service sources into /etc/s6/sv, config into /etc/s6/config,
# compiles a new s6-rc database, and atomically activates it.
set -eu

S6_SV=/etc/s6/sv
S6_CFG=/etc/s6/config
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LABEL="${1:-op-assistant-grpc-$(date +%s)}"

echo "==> Building op-assistant-grpc binary..."
cargo build --release -p op-assistant-grpc --bins 2>/dev/null
echo "==> Installing binary to /usr/local/bin..."
doas install -m 0755 target/release/op-assistant-grpc /usr/local/bin/op-assistant-grpc

echo "==> Installing s6 service sources..."
doas cp -r "$SCRIPT_DIR/s6/op-assistant-grpc-srv"  "$S6_SV/op-assistant-grpc-srv"
doas cp -r "$SCRIPT_DIR/s6/op-assistant-grpc-log" "$S6_SV/op-assistant-grpc-log"

echo "==> Installing s6 config..."
doas install -d "$S6_CFG"
doas install -m 0644 "$SCRIPT_DIR/s6/config/op-assistant-grpc.conf" "$S6_CFG/op-assistant-grpc.conf"

echo "==> Creating CozoDB data directory..."
doas install -d -o root -g root /var/lib/op-dbus

echo "==> Compiling s6-rc database..."
"$SCRIPT_DIR/s6/recompile-and-update.sh" "$LABEL"

echo ""
echo "==> op-assistant-grpc deployed. Activate with:"
echo "    doas s6-rc -u change op-assistant-grpc-srv"
echo ""
echo "    Logs: /var/log/op-assistant-grpc/"
echo "    Config: /etc/s6/config/op-assistant-grpc.conf"
