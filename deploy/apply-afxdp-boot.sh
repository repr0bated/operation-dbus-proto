#!/bin/sh
# apply-afxdp-boot.sh — fix AF_XDP boot sequence and install ovsbr0-static
#
# Run as root in novnc.  Safe to re-run.
# After running, reboot to verify the full boot sequence.
set -eu

REPO="$(cd "$(dirname "$0")/.." && pwd)"
REPO_DIR="$REPO"
log() { printf '\033[1;32m[apply]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[ERROR]\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" = 0 ] || die "run as root (sudo sh deploy/apply-afxdp-boot.sh)"

# ── 1. Build the fixed op-ovsbr0-afxdp binary ────────────────────────────────
log "building op-ovsbr0-afxdp (reordered: set IP on bridge BEFORE adding eth0)"
cd "$REPO"
sudo -u jeremy cargo build --release -p op-network --bin op-ovsbr0-afxdp
install -m 755 target/release/op-ovsbr0-afxdp /usr/local/bin/op-ovsbr0-afxdp
log "installed /usr/local/bin/op-ovsbr0-afxdp"

# ── 2. Fix ovsbr0-afxdp shell_up — remove --dhcp flag ────────────────────────
# The --dhcp flag caused the binary to skip all IP/route setup after adding
# eth0 to the bridge, leaving the host with no default route.
log "fixing /etc/s6/sv/ovsbr0-afxdp/shell_up (removing --dhcp)"
cat > /etc/s6/sv/ovsbr0-afxdp/shell_up <<'SHELL'
#!/bin/sh
set -eu
exec /usr/local/bin/op-ovsbr0-afxdp up
SHELL
chmod +x /etc/s6/sv/ovsbr0-afxdp/shell_up

# ── 3. Install ovsbr0-netdev-verify oneshot service ───────────────────────────
# Verifies that ovsbr0 survived boot with datapath_type=netdev before we move
# the host IP onto the bridge.
log "installing ovsbr0-netdev-verify s6 service"
mkdir -p /etc/s6/sv/ovsbr0-netdev-verify/dependencies.d
install -m 644 "$REPO_DIR/deploy/s6/ovsbr0-netdev-verify/type" /etc/s6/sv/ovsbr0-netdev-verify/type
install -m 755 "$REPO_DIR/deploy/s6/ovsbr0-netdev-verify/up" /etc/s6/sv/ovsbr0-netdev-verify/up
install -m 755 "$REPO_DIR/deploy/s6/ovsbr0-netdev-verify/shell_up" /etc/s6/sv/ovsbr0-netdev-verify/shell_up
touch /etc/s6/sv/ovsbr0-netdev-verify/dependencies.d/ovsbr0-setup

# ── 4. Install ovsbr0-static oneshot service ─────────────────────────────────
# Runs after ovsbr0-netdev-verify and before ovsbr0-afxdp (eth0 migration).
# Sets 148.113.204.83/32 + default route onlink on ovsbr0 so there is no window
# where the kernel has no valid path when eth0 enters AF_XDP mode.
log "installing ovsbr0-static s6 service"
mkdir -p /etc/s6/sv/ovsbr0-static/dependencies.d

printf 'oneshot\n' > /etc/s6/sv/ovsbr0-static/type

cat > /etc/s6/sv/ovsbr0-static/up <<'EXECLINE'
#!/bin/execlineb -P
foreground { sh /etc/s6/sv/ovsbr0-static/shell_up }
exit 0
EXECLINE
chmod +x /etc/s6/sv/ovsbr0-static/up

cat > /etc/s6/sv/ovsbr0-static/shell_up <<'SHELL'
#!/bin/sh
set -eu
MGMT_ADDR="${MGMT_ADDR:-148.113.204.83}"
MGMT_PREFIX="${MGMT_PREFIX:-32}"
GW="${GW:-148.113.204.1}"
BR="${BR:-ovsbr0}"

ip link set "$BR" up 2>/dev/null || true
ip addr replace "${MGMT_ADDR}/${MGMT_PREFIX}" dev "$BR" 2>/dev/null \
    || ip addr add "${MGMT_ADDR}/${MGMT_PREFIX}" dev "$BR" 2>/dev/null \
    || true
ip route replace "$GW" dev "$BR" scope link 2>/dev/null || true
ip route replace default via "$GW" dev "$BR" onlink 2>/dev/null || true
echo "ovsbr0-static: ${MGMT_ADDR}/${MGMT_PREFIX} on ${BR}, default via ${GW} onlink"
SHELL
chmod +x /etc/s6/sv/ovsbr0-static/shell_up

# ovsbr0-static must run after ovsbr0-netdev-verify.
touch /etc/s6/sv/ovsbr0-static/dependencies.d/ovsbr0-netdev-verify

# ── 5. ovsbr0-afxdp must run after ovsbr0-static ─────────────────────────────
log "adding ovsbr0-static → ovsbr0-afxdp dependency"
mkdir -p /etc/s6/sv/ovsbr0-afxdp/dependencies.d
touch /etc/s6/sv/ovsbr0-afxdp/dependencies.d/ovsbr0-static

# ── 6. Add ovsbr0-netdev-verify and ovsbr0-static to ovs-stack bundle ─────────
log "adding ovsbr0-netdev-verify to ovs-stack bundle"
mkdir -p /etc/s6/sv/ovs-stack/contents.d
touch /etc/s6/sv/ovs-stack/contents.d/ovsbr0-netdev-verify
log "adding ovsbr0-static to ovs-stack bundle"
touch /etc/s6/sv/ovs-stack/contents.d/ovsbr0-static

# ── 7. Recompile s6-rc database ──────────────────────────────────────────────
log "recompiling s6-rc database"
tmp="$(mktemp -d)"
s6-rc-compile "$tmp/db" /etc/s6/sv /etc/s6/adminsv
s6-rc-db -c "$tmp/db" check && log "database check passed"
stamp="$(date +%Y%m%d%H%M%S)"
[ -e /etc/s6/rc/compiled ] && mv /etc/s6/rc/compiled "/etc/s6/rc/compiled.prev-${stamp}" || true
cp -a "$tmp/db" /etc/s6/rc/compiled
rm -rf "$tmp"
log "s6-rc database updated"

log ""
log "===== Done ====="
log "Boot sequence after reboot:"
log "  ovsbr0-setup   — creates OVS bridge (ovs-vswitchd dependency)"
log "  ovsbr0-netdev-verify — confirms ovsbr0 stayed netdev"
log "  ovsbr0-static  — sets 148.113.204.83/32 on ovsbr0, default route onlink"
log "  ovsbr0-afxdp   — adds eth0 as AF_XDP (IP already on bridge, no gap)"
log ""
log "To test NOW without rebooting:"
log "  sh /etc/s6/sv/ovsbr0-static/shell_up"
log "  /usr/local/bin/op-ovsbr0-afxdp up"
log "  ip addr show ovsbr0 && ip route show"
