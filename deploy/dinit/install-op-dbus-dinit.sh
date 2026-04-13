#!/bin/sh
set -eu

ROOT="${ROOT:-/}"
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(CDPATH='' cd -- "$SCRIPT_DIR/../.." && pwd)"

ensure_dhcpcd_ovs_denyinterfaces() {
  dhcpcd_conf="$ROOT/etc/dhcpcd.conf"
  [ -f "$dhcpcd_conf" ] || return 0

  if ! grep -q '^# op-ovs internal interfaces$' "$dhcpcd_conf"; then
    cat >> "$dhcpcd_conf" <<'EOF'

# op-ovs internal interfaces
# dhcpcd must not claim IPv4LL on OVS-managed or wg-quick-managed links.
denyinterfaces ovsbr0 wgcf grpc-bridge ovsbr0-mgmt ovsbr0-sock priv_wg priv_warp priv_xray
EOF
    echo "Updated $dhcpcd_conf with OVS/internal interface deny list"
  fi
}

cleanup_stale_dinit_artifacts() {
  rm -f "$ROOT/etc/dinit.d/boot.d/stalwart" "$ROOT/etc/dinit.d/stalwart"
  rm -f "$ROOT/etc/dinit.d/wgcf" "$ROOT/etc/dinit.d/boot.d/wgcf"
  rm -f "$ROOT/etc/dinit.d/disabled/op-ovsdb-bridge" "$ROOT/etc/dinit.d/disabled/op-ovsdb-seed"

  for stale in \
    "$ROOT"/etc/dinit.d/op-dbus.bak.* \
    "$ROOT"/etc/dinit.d/op-web.bak.* \
    "$ROOT"/etc/dinit.d/op-web.backup \
    "$ROOT"/etc/dinit.d/op-web.broken \
    "$ROOT"/etc/dinit.d/networkd.bak.* \
    "$ROOT"/etc/dinit.d/incus.bak \
    "$ROOT"/etc/dinit.d/qdrant.bak.* \
    "$ROOT"/etc/dinit.d/scripts/ovs-attach-ports.sh.bak*; do
    [ -e "$stale" ] || continue
    rm -f "$stale"
  done
}

dinit_dbus_start_service() {
  service="$1"

  if command -v gdbus >/dev/null 2>&1; then
    output="$(gdbus call \
      --system \
      --dest org.chimera.dinit \
      --object-path /org/chimera/dinit \
      --method org.chimera.dinit.Manager.StartService \
      "$service" \
      false 2>&1)" && return 0

    printf '%s' "$output" | grep -q 'org.chimera.dinit.Error.ServiceAlready' && return 0
  fi

  if command -v dinitctl >/dev/null 2>&1; then
    dinitctl start "$service" >/dev/null 2>&1 && return 0
  fi

  return 1
}

dinit_dbus_stop_service() {
  service="$1"

  if command -v gdbus >/dev/null 2>&1; then
    output="$(gdbus call \
      --system \
      --dest org.chimera.dinit \
      --object-path /org/chimera/dinit \
      --method org.chimera.dinit.Manager.StopService \
      "$service" \
      false \
      false \
      false 2>&1)" && return 0

    printf '%s' "$output" | grep -q 'org.chimera.dinit.Error.ServiceAlready' && return 0
  fi

  if command -v dinitctl >/dev/null 2>&1; then
    dinitctl stop "$service" >/dev/null 2>&1 && return 0
  fi

  return 1
}

dinit_dbus_restart_service() {
  service="$1"
  dinit_dbus_stop_service "$service" || true
  dinit_dbus_start_service "$service"
}

echo "Installing dinit op-dbus service files..."

install -d "$ROOT/etc/dinit.d" "$ROOT/etc/dinit.d/boot.d" "$ROOT/etc/dinit.d/scripts" "$ROOT/etc/op-dbus" "$ROOT/etc/systemd/network" "$ROOT/usr/local/bin" "$ROOT/usr/local/sbin"
find "$ROOT/etc/systemd/network" -maxdepth 1 -type f \
  \( -name '*.network' -o -name '*.netdev' -o -name '*.link' \) -delete
if command -v apk >/dev/null 2>&1; then
  apk info -e resolvconf >/dev/null 2>&1 || apk add resolvconf
  apk info -e resolvconf-openresolv >/dev/null 2>&1 || apk add resolvconf-openresolv
fi
install -m 0644 "$SCRIPT_DIR/op-dbus" "$ROOT/etc/dinit.d/op-dbus"
install -m 0644 "$SCRIPT_DIR/op-session-bus" "$ROOT/etc/dinit.d/op-session-bus"
install -m 0644 "$SCRIPT_DIR/op-ovs-services" "$ROOT/etc/dinit.d/op-ovs-services"
install -m 0644 "$SCRIPT_DIR/op-ovsdb-seed" "$ROOT/etc/dinit.d/op-ovsdb-seed"
install -m 0644 "$SCRIPT_DIR/op-ovsdb-bridge" "$ROOT/etc/dinit.d/op-ovsdb-bridge"
install -m 0644 "$SCRIPT_DIR/services0-sockets" "$ROOT/etc/dinit.d/services0-sockets"
install -m 0644 "$SCRIPT_DIR/wg-quick-all" "$ROOT/etc/dinit.d/wg-quick-all"
install -m 0644 "$SCRIPT_DIR/netplan-apply" "$ROOT/etc/dinit.d/netplan-apply"
install -m 0644 "$SCRIPT_DIR/ovs-attach-ports" "$ROOT/etc/dinit.d/ovs-attach-ports"
install -m 0644 "$SCRIPT_DIR/xray-client" "$ROOT/etc/dinit.d/xray-client"
install -m 0644 "$SCRIPT_DIR/systemd-networkd" "$ROOT/etc/dinit.d/systemd-networkd"
install -m 0755 "$SCRIPT_DIR/op-dbus-dinit.sh" "$ROOT/usr/local/bin/op-dbus-dinit.sh"
install -m 0755 "$SCRIPT_DIR/op-dbus-dinit.sh" "$ROOT/usr/local/sbin/op-dbus-dinit.sh"
rm -f "$ROOT/usr/local/sbin/op-networkd-dinit.sh"
install -m 0755 "$SCRIPT_DIR/op-web-dinit.sh" "$ROOT/usr/local/sbin/op-web-dinit.sh"
install -m 0755 "$SCRIPT_DIR/op-mcp-proxy-select3" "$ROOT/usr/local/bin/op-mcp-proxy-select3"
install -m 0755 "$SCRIPT_DIR/op-session-bus.sh" "$ROOT/usr/local/sbin/op-session-bus"
install -m 0755 "$SCRIPT_DIR/scripts/services0-sockets.sh" "$ROOT/etc/dinit.d/scripts/services0-sockets.sh"
install -m 0755 "$SCRIPT_DIR/op-ovs-services-start.sh" "$ROOT/etc/dinit.d/scripts/op-ovs-services-start.sh"
install -m 0755 "$SCRIPT_DIR/op-ovsdb-seed.sh" "$ROOT/etc/dinit.d/scripts/op-ovsdb-seed.sh"
install -m 0755 "$SCRIPT_DIR/op-ovsdb-bridge-start.sh" "$ROOT/etc/dinit.d/scripts/op-ovsdb-bridge-start.sh"
install -m 0755 "$SCRIPT_DIR/scripts/ovs-attach-ports.sh" "$ROOT/etc/dinit.d/scripts/ovs-attach-ports.sh"
for network_file in "$REPO_ROOT"/deploy/systemd/networkd/*; do
  install -m 0644 "$network_file" "$ROOT/etc/systemd/network/$(basename "$network_file")"
done
ensure_dhcpcd_ovs_denyinterfaces
cleanup_stale_dinit_artifacts

if [ ! -f "$ROOT/etc/op-dbus/environment" ]; then
  install -m 0644 "$SCRIPT_DIR/environment.op-dbus.template" "$ROOT/etc/op-dbus/environment"
  echo "Wrote new environment template to $ROOT/etc/op-dbus/environment"
else
  echo "Keeping existing $ROOT/etc/op-dbus/environment"
fi

ln -sfn ../op-dbus "$ROOT/etc/dinit.d/boot.d/op-dbus"
ln -sfn ../op-session-bus "$ROOT/etc/dinit.d/boot.d/op-session-bus"
ln -sfn ../op-ovs-services "$ROOT/etc/dinit.d/boot.d/op-ovs-services"
ln -sfn ../op-ovsdb-seed "$ROOT/etc/dinit.d/boot.d/op-ovsdb-seed"
ln -sfn ../op-ovsdb-bridge "$ROOT/etc/dinit.d/boot.d/op-ovsdb-bridge"
ln -sfn ../services0-sockets "$ROOT/etc/dinit.d/boot.d/services0-sockets"
ln -sfn ../wg-quick-all "$ROOT/etc/dinit.d/boot.d/wg-quick-all"
ln -sfn ../netplan-apply "$ROOT/etc/dinit.d/boot.d/netplan-apply"
ln -sfn ../ovs-attach-ports "$ROOT/etc/dinit.d/boot.d/ovs-attach-ports"
ln -sfn ../xray-client "$ROOT/etc/dinit.d/boot.d/xray-client"
ln -sfn ../systemd-networkd "$ROOT/etc/dinit.d/boot.d/systemd-networkd"

if [ "$ROOT" = "/" ]; then
  dinit_dbus_stop_service stalwart || true
  dinit_dbus_start_service op-session-bus || true
  dinit_dbus_start_service op-ovs-services || true
  dinit_dbus_restart_service systemd-networkd || dinit_dbus_start_service systemd-networkd || true
  dinit_dbus_restart_service op-dbus || dinit_dbus_start_service op-dbus || true
  dinit_dbus_restart_service op-ovsdb-seed || dinit_dbus_start_service op-ovsdb-seed || true
  dinit_dbus_restart_service op-ovsdb-bridge || dinit_dbus_start_service op-ovsdb-bridge || true
fi

echo "Done."
echo "If needed, copy your op-dbus binaries:"
echo "  install -m 0755 \"$REPO_ROOT/target/release/op-dbus\" /usr/local/bin/op-dbus"
echo "  install -m 0755 \"$REPO_ROOT/target/release/op-mcp-proxy\" /usr/local/bin/op-mcp-proxy"
