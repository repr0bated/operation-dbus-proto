#!/bin/sh
set -eu

ROOT="${ROOT:-/}"
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(CDPATH='' cd -- "$SCRIPT_DIR/../.." && pwd)"

echo "Installing dinit op-dbus service files..."

install -d "$ROOT/etc/dinit.d" "$ROOT/etc/dinit.d/boot.d" "$ROOT/etc/dinit.d/scripts" "$ROOT/etc/op-dbus" "$ROOT/etc/systemd/network" "$ROOT/usr/local/bin" "$ROOT/usr/local/sbin"
install -m 0644 "$SCRIPT_DIR/op-dbus" "$ROOT/etc/dinit.d/op-dbus"
install -m 0644 "$SCRIPT_DIR/op-session-bus" "$ROOT/etc/dinit.d/op-session-bus"
install -m 0644 "$SCRIPT_DIR/op-ovsdb-bridge" "$ROOT/etc/dinit.d/op-ovsdb-bridge"
install -m 0644 "$SCRIPT_DIR/systemd-networkd" "$ROOT/etc/dinit.d/systemd-networkd"
install -m 0755 "$SCRIPT_DIR/op-dbus-dinit.sh" "$ROOT/usr/local/bin/op-dbus-dinit.sh"
install -m 0755 "$SCRIPT_DIR/op-dbus-dinit.sh" "$ROOT/usr/local/sbin/op-dbus-dinit.sh"
install -m 0755 "$SCRIPT_DIR/op-networkd-dinit.sh" "$ROOT/usr/local/sbin/op-networkd-dinit.sh"
install -m 0755 "$SCRIPT_DIR/op-web-dinit.sh" "$ROOT/usr/local/sbin/op-web-dinit.sh"
install -m 0755 "$SCRIPT_DIR/op-mcp-proxy-select3" "$ROOT/usr/local/bin/op-mcp-proxy-select3"
install -m 0755 "$SCRIPT_DIR/op-session-bus.sh" "$ROOT/usr/local/sbin/op-session-bus"
install -m 0755 "$SCRIPT_DIR/op-ovsdb-bridge-start.sh" "$ROOT/etc/dinit.d/scripts/op-ovsdb-bridge-start.sh"
install -m 0644 "$REPO_ROOT/deploy/systemd/networkd/10-ens3.network" "$ROOT/etc/systemd/network/10-ens3.network"
install -m 0644 "$REPO_ROOT/deploy/systemd/networkd/20-ovsbr0.network" "$ROOT/etc/systemd/network/20-ovsbr0.network"

if [ ! -f "$ROOT/etc/op-dbus/environment" ]; then
  install -m 0644 "$SCRIPT_DIR/environment.op-dbus.template" "$ROOT/etc/op-dbus/environment"
  echo "Wrote new environment template to $ROOT/etc/op-dbus/environment"
else
  echo "Keeping existing $ROOT/etc/op-dbus/environment"
fi

ln -sfn ../op-dbus "$ROOT/etc/dinit.d/boot.d/op-dbus"
ln -sfn ../op-session-bus "$ROOT/etc/dinit.d/boot.d/op-session-bus"
ln -sfn ../op-ovsdb-bridge "$ROOT/etc/dinit.d/boot.d/op-ovsdb-bridge"
ln -sfn ../systemd-networkd "$ROOT/etc/dinit.d/boot.d/systemd-networkd"
rm -f "$ROOT/etc/dinit.d/boot.d/stalwart" "$ROOT/etc/dinit.d/stalwart"

if command -v dinitctl >/dev/null 2>&1 && [ "$ROOT" = "/" ]; then
  dinitctl stop stalwart || true
  dinitctl start op-session-bus || true
  dinitctl restart op-dbus || dinitctl start op-dbus || true
  dinitctl restart op-ovsdb-bridge || dinitctl start op-ovsdb-bridge || true
  dinitctl restart systemd-networkd || dinitctl start systemd-networkd || true
fi

echo "Done."
echo "If needed, copy your op-dbus binaries:"
echo "  install -m 0755 \"$REPO_ROOT/target/release/op-dbus\" /usr/local/bin/op-dbus"
echo "  install -m 0755 \"$REPO_ROOT/target/release/op-mcp-proxy\" /usr/local/bin/op-mcp-proxy"
