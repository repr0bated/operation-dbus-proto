#!/bin/sh
# Installs the control-plane-network bootstrap for Artix s6-rc.
set -eu

SRC="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
PREFIX="${PREFIX:-/}"

install -d \
  "$PREFIX/usr/local/sbin" \
  "$PREFIX/etc/control-plane-network"

if [ ! -e "$PREFIX/etc/control-plane-network/network.conf" ]; then
  install -m 0644 "$SRC/etc/control-plane-network/network.conf" \
    "$PREFIX/etc/control-plane-network/network.conf"
fi

for svc in control-plane-network ovsbr0-controller; do
  install -d "$PREFIX/etc/s6/sv/$svc/dependencies.d"
  install -m 0755 "$SRC/bin/$svc" "$PREFIX/usr/local/sbin/$svc"
  install -m 0644 "$SRC/s6-sv/$svc/type" "$PREFIX/etc/s6/sv/$svc/type"
  install -m 0755 "$SRC/s6-sv/$svc/up" "$PREFIX/etc/s6/sv/$svc/up"
  for dep in "$SRC"/s6-sv/"$svc"/dependencies.d/*; do
    [ -e "$dep" ] || continue
    : > "$PREFIX/etc/s6/sv/$svc/dependencies.d/$(basename "$dep")"
  done
done

# Remove the old (inert) s6-overlay layout from a previous install.
rm -rf "$PREFIX/etc/s6-overlay/s6-rc.d/control-plane-network" \
  "$PREFIX/etc/s6-overlay/s6-rc.d/user/contents.d/control-plane-network" 2>/dev/null || true

echo "Installed control-plane-network + ovsbr0-controller."
echo "Set NODE_ROLE and any layout overrides in:"
echo "  /etc/control-plane-network/network.conf"
echo "Then register them (commit and live install MUST run back-to-back):"
echo "  s6 set enable control-plane-network ovsbr0-controller"
echo "  s6 set commit -D default && s6 live install"
