#!/bin/sh
# install-ovsbr0-netdev.sh — permanent fix for OVS netdev boot sequence
#
# Replaces /usr/local/bin/op-ovsbr0-setup with a shell script that creates
# the OVS bridge with datapath_type=netdev (AF_XDP requirement).
# The original binary (source deleted) always created system datapath.
#
# Also fixes /etc/s6/sv/ovs-vswitchd/run to pre-configure OVSDB with netdev
# BEFORE vswitchd starts, so vswitchd initializes the netdev dpif at boot.
#
# Run as root.  Safe to re-run.
set -eu

log() { printf '\033[1;32m[install]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[ERROR]\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" = 0 ] || die "run as root"

# ── 1. Replace op-ovsbr0-setup with netdev-aware shell script ─────────────────
log "installing new /usr/local/bin/op-ovsbr0-setup (netdev-aware)"

# Back up original only once
if [ -f /usr/local/bin/op-ovsbr0-setup ] && \
   [ ! -f /usr/local/bin/op-ovsbr0-setup.orig-bak ]; then
    cp /usr/local/bin/op-ovsbr0-setup /usr/local/bin/op-ovsbr0-setup.orig-bak
    log "backed up original to op-ovsbr0-setup.orig-bak"
fi

cat > /usr/local/bin/op-ovsbr0-setup << 'SETUP_SCRIPT'
#!/bin/sh
# op-ovsbr0-setup — create OVS bridge with datapath_type=netdev for AF_XDP
# Replacement for deleted Rust binary. Reads same env vars as original.
set -eu

BRIDGE="${BRIDGE:-ovsbr0}"
VETH_HOST="${VETH_HOST:-grpc-uplink}"
FAIL_MODE="${FAIL_MODE:-standalone}"
SHARED_MAC="${SHARED_MAC:-fa:16:3e:f1:71:d2}"

# Wait for OVSDB socket
i=0
while ! ovs-vsctl show >/dev/null 2>&1; do
    i=$((i+1)); [ $i -lt 20 ] || { echo "op-ovsbr0-setup: OVSDB not ready" >&2; exit 1; }
    sleep 0.5
done

DT=$(ovs-vsctl get bridge "$BRIDGE" datapath_type 2>/dev/null || echo "missing")

if [ "$DT" = "netdev" ]; then
    echo "op-ovsbr0-setup: $BRIDGE already has datapath_type=netdev"
elif [ "$DT" = "missing" ]; then
    # Bridge does not exist — create with netdev
    ovs-vsctl \
        add-br "$BRIDGE" \
        -- set bridge "$BRIDGE" datapath_type=netdev \
        -- set bridge "$BRIDGE" fail_mode="$FAIL_MODE" \
        -- set bridge "$BRIDGE" other_config:hwaddr="$SHARED_MAC"
    echo "op-ovsbr0-setup: created $BRIDGE with datapath_type=netdev fail_mode=$FAIL_MODE"
else
    # Bridge exists with wrong datapath — update fail_mode and hwaddr only.
    # datapath_type can only be changed by restarting vswitchd; the run script
    # handles that via pre-configuration before vswitchd starts.
    ovs-vsctl set bridge "$BRIDGE" fail_mode="$FAIL_MODE" \
        other_config:hwaddr="$SHARED_MAC" 2>/dev/null || true
    echo "op-ovsbr0-setup: WARNING: $BRIDGE has datapath_type=$DT (expected netdev) — vswitchd restart required"
fi

# Add VETH_HOST to bridge (grpc-uplink connects to wg-xray container)
if ovs-vsctl port-to-br "$VETH_HOST" 2>/dev/null | grep -q "^${BRIDGE}$"; then
    echo "op-ovsbr0-setup: $VETH_HOST already in $BRIDGE"
else
    ovs-vsctl --if-exists del-port "$BRIDGE" "$VETH_HOST" 2>/dev/null || true
    ovs-vsctl add-port "$BRIDGE" "$VETH_HOST" 2>/dev/null \
        && echo "op-ovsbr0-setup: added $VETH_HOST to $BRIDGE" \
        || echo "op-ovsbr0-setup: WARNING: could not add $VETH_HOST (may not exist yet)"
fi

ip link set "$VETH_HOST" up 2>/dev/null || true
SETUP_SCRIPT

chmod +x /usr/local/bin/op-ovsbr0-setup
log "installed /usr/local/bin/op-ovsbr0-setup"

# ── 2. Fix ovs-vswitchd run script to pre-configure OVSDB before startup ──────
# vswitchd initializes whichever datapath_type is in OVSDB when it starts.
# Pre-configuring netdev here (before exec) means vswitchd starts with netdev dpif.
# del-br + add-br must be SEPARATE ovs-vsctl calls — combined atomic transaction
# silently rolls back on OVS 3.x due to Bridge table unique-name constraint.
log "patching /etc/s6/sv/ovs-vswitchd/run"
cat > /etc/s6/sv/ovs-vswitchd/run << 'RUN_SCRIPT'
#!/bin/sh
exec 2>&1

mkdir -p /run/openvswitch
while [ ! -S /usr/local/var/run/openvswitch/db.sock ]; do sleep 0.5; done
OVSDB_SOCKET=/usr/local/var/run/openvswitch/db.sock /usr/local/bin/op-ovsdb-init

# Pre-configure ovsbr0 with datapath_type=netdev BEFORE vswitchd starts.
# ovs-vsctl --no-wait writes to ovsdb-server directly (vswitchd not needed).
# vswitchd reads this at startup and initializes the netdev (userspace) dpif.
# The two commands MUST be separate — del+add in one atomic transaction rolls back.
ovs-vsctl --no-wait del-br ovsbr0 2>/dev/null || true
sleep 0.3
ovs-vsctl --no-wait \
    add-br ovsbr0 \
    -- set bridge ovsbr0 datapath_type=netdev \
    -- set bridge ovsbr0 fail_mode=standalone \
    -- set bridge ovsbr0 other_config:hwaddr=fa:16:3e:f1:71:d2
echo "ovs-vswitchd: pre-configured ovsbr0 datapath_type=netdev"

exec /usr/local/sbin/ovs-vswitchd unix:/usr/local/var/run/openvswitch/db.sock
RUN_SCRIPT

chmod +x /etc/s6/sv/ovs-vswitchd/run
cp /etc/s6/sv/ovs-vswitchd/run /run/service/ovs-vswitchd/run
chmod +x /run/service/ovs-vswitchd/run
log "vswitchd run script updated"

# ── 3. Remove the broken wrapper if it exists ─────────────────────────────────
if [ -f /usr/local/bin/op-ovsbr0-setup.orig ]; then
    log "removing stale op-ovsbr0-setup.orig wrapper"
    rm -f /usr/local/bin/op-ovsbr0-setup.orig
fi

log ""
log "===== Done ====="
log "Now run:  sh deploy/cutover-afxdp.sh"
log ""
log "On next boot, the sequence will be:"
log "  ovsdb-server starts"
log "  ovs-vswitchd run script: pre-configures ovsbr0 as netdev in OVSDB"
log "  ovs-vswitchd starts: reads netdev, initializes netdev dpif"
log "  ovsbr0-setup: finds bridge already netdev, adds grpc-uplink"
log "  ovsbr0-afxdp: adds eth0 as type=afxdp (chains under xdp-dispatcher)"
