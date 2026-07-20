#!/usr/bin/env bash
#
# 3tched-network-bootstrap.sh — focused network boot unit (s6 + OVS + netmaker)
#
# The comprehensive installer already ran on this server and MUST NOT be
# re-run (HANDOFF-2026-07-20-NETWORK-GRPC.md).  This script touches ONLY the
# network boot path:
#
#   1. Installs /usr/local/bin/3tched-bridge-up — THE one network command:
#        dhcp -> seed (bridge creation + eth0 enslavement in ONE OVSDB
#        transact) -> uplink migration onto ovsbr0 -> netmaker ready
#        (readiness over D-Bus with busctl, never systemctl — netmaker is
#        deliberately NIC-less, reached via rust-network-mgr's socket + a
#        host-side Incus proxy device, never bridge enslavement) -> service
#        addresses 10.200.0.1 + 10.0.0.2 + 10.200.0.2 applied LAST, still
#        strictly after netmaker is ready, to avoid racing the container
#        bring-up.
#   2. Regenerates the network s6 services around that command:
#        uplink-dhcp -> ovsdb-server -> ovs-vswitchd (seed) -> ovsbr0-uplink
#        -> incusd -> incus-ct-netmaker -> ovsbr0-svc-addr
#        -> op-of-controller / opdbus
#   3. Retires the backwards oneshots (ovsbr0-addr, ovsbr0-uplink-addr):
#      the host uplink now completes BEFORE netmaker, service addresses
#      strictly AFTER netmaker.
#   4. Strips any network dependency from op-session-bus / op-grpc-bridge so
#      the D-Bus session bus and the gRPC bridge (UDS + loopback :8090) load
#      early, in parallel with the network chain.
#   5. Commits the touched services via `service6 enable`/`disable` (the only
#      commands that actually change s6 set prescriptions) and installs the
#      result.  `s6 live install` does NOT just stage a database: per its own
#      manual it stops anything absent from the new set and s6-rc-update
#      restarts any EXISTING service whose definition changed — so this step
#      IS the live cutover of the whole touched subgraph, executed the moment
#      this script runs, not staged for a later reboot.  Run it only when you
#      mean to cut over now, ideally with the noVNC console/mirror available
#      for recovery.
#
# Rollback contract: before any change, /etc/s6/sv, network.conf and the old
# 3tched-bridge-up are copied to
#   /var/lib/op-dbus/network-bootstrap-backup-<timestamp>/
# Restore (console): cp -a <backup>/sv/. /etc/s6/sv/, then `service6 enable`
# the old service names so the working set's prescriptions actually change —
# restoring the source alone does not revert what is live.

set -euo pipefail

SV=/etc/s6/sv
NET_CONF=/etc/op-dbus/network.conf
BIN=/usr/local/bin
BACKUP="/var/lib/op-dbus/network-bootstrap-backup-$(date +%Y%m%d-%H%M%S)"

# Every service whose source dir (type/up/dependencies.d) this script creates
# or edits, and every retired service, gets recorded here.  `s6 set commit`
# only ever compiles what the working set's prescriptions say is enabled —
# editing /etc/s6/sv directly does NOT change those prescriptions.  Without
# an explicit `service6 enable`/`disable` pass over these exact names, a
# `repository sync && set commit && live install` silently recompiles the
# OLD set unchanged (no error, no new services, nothing done) because as far
# as `s6 set commit` is concerned nothing about the set itself changed.
declare -a TOUCHED_SERVICES=()
declare -a RETIRED_SERVICES=()

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
log()  { echo -e "${BLUE}[net-bootstrap]${NC} $*"; }
ok()   { echo -e "${GREEN}[  ok  ]${NC} $*"; }
warn() { echo -e "${YELLOW}[ warn ]${NC} $*"; }
die()  { echo -e "${RED}[ fail ]${NC} $*" >&2; exit 1; }

# ============================================================================
# PREFLIGHT + BACKUP
# ============================================================================

preflight() {
    [[ $EUID -eq 0 ]] || die "run as root (sudo)"
    [[ -d $SV ]] || die "$SV missing — not an s6-rc system"
    command -v service6 >/dev/null 2>&1 || die "service6 not found — host s6 services must be managed only through it (paru -S service6-git)"
    [[ -x ${BIN}/op-ovsbr0-setup ]] || die "${BIN}/op-ovsbr0-setup missing — build/install op-network first"
    [[ -r $NET_CONF ]] || die "$NET_CONF missing — this bootstrap only adjusts an installed system"
    log "Preflight OK"
}

backup() {
    log "Backing up to $BACKUP ..."
    install -d -m 0700 "$BACKUP"
    cp -a "$SV" "$BACKUP/sv"
    cp -a "$NET_CONF" "$BACKUP/network.conf"
    [[ -f ${BIN}/3tched-bridge-up ]] && cp -a "${BIN}/3tched-bridge-up" "$BACKUP/"
    ok "Backup complete (restore: cp -a $BACKUP/sv/. $SV/ && service6 enable <affected services>; see summary output for the exact list)"
}

# ============================================================================
# S6 TREE HYGIENE — standard services that must exist for s6-rc-compile
# ============================================================================

ensure_standard_oneshots() {
    # The base Artix s6 tree on this host is missing a couple of standard
    # oneshots that bundles still reference.  Without them s6-rc-compile
    # (and the modern s6 set commit) hard-fails with "undefined service".
    # We create minimal, safe stubs only when they are absent and referenced.
    local svc
    if [[ -f ${SV}/misc/contents.d/rc-local && ! -d ${SV}/rc-local ]]; then
        log "Creating minimal rc-local oneshot (referenced by misc bundle) ..."
        install -d "${SV}/rc-local"
        echo oneshot > "${SV}/rc-local/type"
        cat > "${SV}/rc-local/up" <<'EOF'
#!/bin/sh
[ -x /etc/rc.local ] && /etc/rc.local
EOF
        chmod 0755 "${SV}/rc-local/up"
        ok "rc-local stub created"
    fi
    if [[ -f ${SV}/mount/contents.d/mount-filesystems && ! -d ${SV}/mount-filesystems ]]; then
        log "Creating minimal mount-filesystems oneshot (referenced by mount bundle) ..."
        install -d "${SV}/mount-filesystems"
        echo oneshot > "${SV}/mount-filesystems/type"
        cat > "${SV}/mount-filesystems/up" <<'EOF'
#!/bin/sh
mount -a -O no_netdev 2>/dev/null || true
EOF
        chmod 0755 "${SV}/mount-filesystems/up"
        ok "mount-filesystems stub created"
    fi
}

# ============================================================================
# NETWORK.CONF — post-netmaker service addresses + service6 name
# ============================================================================

update_net_conf() {
    log "Updating $NET_CONF ..."
    # VSWITCHD_SVC is a service6 NAME now, not a scandir path
    if grep -q '^VSWITCHD_SVC=' "$NET_CONF"; then
        sed -i 's|^VSWITCHD_SVC=.*|VSWITCHD_SVC=ovs-vswitchd|' "$NET_CONF"
    else
        echo 'VSWITCHD_SVC=ovs-vswitchd' >> "$NET_CONF"
    fi
    if ! grep -q '^BRIDGE_SVC_ADDRS=' "$NET_CONF"; then
        cat >> "$NET_CONF" <<'EOF'

# Host service addresses, applied to the bridge ONLY AFTER the netmaker
# container is enslaved and its service manager is ready (ovsbr0-svc-addr):
#   10.200.0.1/24 — control plane bind (gRPC :50051, OpenFlow :6653)
#   10.0.0.2/24    — host side of the netmaker egress path
#   10.200.0.2/24  — netmaker mesh bridge address (formerly the separate,
#                    never-consumed NETMAKER_BRIDGE_ADDR — a config value
#                    nothing read is how this address silently dropped off
#                    the bridge during the 2026-07-20 cutover)
# Applying these before the netmaker veth exists races the container's own
# bring-up.
BRIDGE_SVC_ADDRS="10.200.0.1/24 10.0.0.2/24 10.200.0.2/24"

# Container whose enslavement gates the service addresses
NETMAKER_CT=netmaker
EOF
    fi
    ok "network.conf updated"
}

# ============================================================================
# THE ONE COMMAND — 3tched-bridge-up
# ============================================================================

install_bridge_up() {
    log "Installing ${BIN}/3tched-bridge-up ..."
    cat > "${BIN}/3tched-bridge-up" <<'BRIDGEUP'
#!/bin/sh
# 3tched-bridge-up — the ONE network bring-up command.
#
# Ordered contract (each phase is an idempotent s6 oneshot body):
#
#   dhcp           acquire an IPv4 lease on UPLINK while it is still a plain
#                  physical NIC, then snapshot addresses + default gateway
#                  to tmpfs.  Later phases only ever consume the snapshot —
#                  the kernel strips a NIC's IPs the instant OVS captures it.
#   seed           create BRIDGE and enslave UPLINK in ONE OVSDB transact
#                  (op-ovsbr0-setup --seed-only).  Called from the
#                  ovs-vswitchd run script BEFORE vswitchd starts so
#                  vswitchd reads bridge + port together and uplink capture
#                  starts correctly.  Never split into create-then-add-port.
#   uplink         migrate the snapshot addresses and default route onto
#                  BRIDGE.  The host uplink is COMPLETE here, strictly
#                  before any container starts.
#   netmaker-wait  bounded wait for the netmaker container to be RUNNING
#                  and its internal service manager ready — probed over
#                  D-Bus with busctl, never systemctl.
#   service-addrs  apply BRIDGE_SVC_ADDRS (10.200.0.1 + 10.0.0.2 + 10.200.0.2)
#                  + netmaker route + NAT, strictly AFTER netmaker is enslaved so the
#                  host service addresses never race the container bring-up.
#   all            full sequence for manual/recovery use.  Boot never runs
#                  this: the s6 graph calls the individual phases
#                  (uplink-dhcp -> seed inside ovs-vswitchd run ->
#                  ovsbr0-uplink -> incus-ct-netmaker -> ovsbr0-svc-addr).
#                  Supervised pieces are started through `service6` only.
set -eu

NET_CONF=/etc/op-dbus/network.conf
SNAPSHOT=/run/opdbus/uplink-migration.env
OVSBR0_SETUP=/usr/local/bin/op-ovsbr0-setup

[ "$(id -u)" -eq 0 ] || { echo "3tched-bridge-up: run as root" >&2; exit 1; }
set -a; [ -r "$NET_CONF" ] && . "$NET_CONF"; set +a
BRIDGE="${BRIDGE:-ovsbr0}"
UPLINK="${UPLINK:-eth0}"
NETMAKER_CT="${NETMAKER_CT:-netmaker}"

phase_dhcp() {
    [ -n "$UPLINK" ] || return 0
    ip link show "$UPLINK" >/dev/null 2>&1 || { echo "uplink $UPLINK not present"; exit 1; }
    ip link set "$UPLINK" up
    # A restored static/retained lease is already valid input.  Otherwise
    # obtain one synchronously while the NIC is still a normal interface.
    if ! ip -4 -o addr show dev "$UPLINK" scope global | grep -q .; then
        command -v dhcpcd >/dev/null 2>&1 || { echo "dhcpcd is required for $UPLINK"; exit 1; }
        echo "acquiring IPv4 DHCP lease on $UPLINK before OVS attachment"
        # -1 -B: one synchronous boot transaction, no lingering supervisor
        dhcpcd -4 -1 -B -q "$UPLINK"
    fi
    addrs="$(ip -4 -o addr show dev "$UPLINK" scope global | awk '{print $4}' | tr '\n' ' ')"
    addrs="${addrs% }"
    [ -n "$addrs" ] || { echo "$UPLINK has no global IPv4 lease"; exit 1; }
    gw="$(ip -4 route show default dev "$UPLINK" | awk '{print $3; exit}')"
    install -d -m 0755 "$(dirname "$SNAPSHOT")"
    tmp="${SNAPSHOT}.new"
    {
        printf 'UPLINK_ADDRS="%s"\n' "$addrs"
        printf 'UPLINK_GW="%s"\n' "$gw"
    } >"$tmp"
    chmod 0600 "$tmp"
    mv -f "$tmp" "$SNAPSHOT"
    echo "captured $addrs (gw ${gw:-none}) before OVS attachment"
}

phase_seed() {
    # bridge + UPLINK enslavement in ONE OVSDB transact — this IS the atomic
    # bridge-creation step; never done as two transactions.
    [ -x "$OVSBR0_SETUP" ] || { echo "$OVSBR0_SETUP missing"; exit 1; }
    export BRIDGE UPLINK FAIL_MODE SHARED_MAC OVSDB_SOCKET VSWITCHD_SVC
    exec "$OVSBR0_SETUP" --seed-only
}

phase_uplink() {
    export BRIDGE UPLINK FAIL_MODE SHARED_MAC OVSDB_SOCKET VSWITCHD_SVC
    if [ -x "$OVSBR0_SETUP" ]; then
        "$OVSBR0_SETUP" || echo "op-ovsbr0-setup failed (continuing)"
    fi
    i=0
    until ip link show "$BRIDGE" >/dev/null 2>&1; do
        i=$((i+1))
        [ "$i" -ge 30 ] && { echo "$BRIDGE did not appear"; exit 1; }
        sleep 1
    done
    ip link set "$BRIDGE" up
    # UPLINK is already an OVS port.  Consume the pre-enslavement snapshot;
    # never query the enslaved NIC.
    if [ -r "$SNAPSHOT" ]; then
        UPLINK_ADDRS=""
        UPLINK_GW=""
        . "$SNAPSHOT"
        if [ -n "$UPLINK_ADDRS" ]; then
            echo "migrating $UPLINK_ADDRS (gw ${UPLINK_GW:-none}) from $UPLINK to $BRIDGE"
            for a in $UPLINK_ADDRS; do
                ip addr replace "$a" dev "$BRIDGE"
            done
            [ -n "$UPLINK" ] && ip addr flush dev "$UPLINK" scope global
            [ -n "$UPLINK_GW" ] && ip route replace default via "$UPLINK_GW" dev "$BRIDGE"
        fi
    else
        echo "warning: no snapshot at $SNAPSHOT — uplink addresses not migrated"
    fi
    if [ -n "$UPLINK" ] && ip link show "$UPLINK" >/dev/null 2>&1; then
        ip link set "$UPLINK" up
    fi
    sysctl -qw net.ipv4.ip_forward=1 || true
    echo "$BRIDGE uplink complete — host network stable before containers"
}

phase_netmaker_wait() {
    command -v incus >/dev/null 2>&1 || return 0
    incus info >/dev/null 2>&1 || { echo "incusd not reachable — skipping netmaker wait"; return 0; }
    if ! incus list -f csv -c n 2>/dev/null | grep -qx "$NETMAKER_CT"; then
        echo "no $NETMAKER_CT container — skipping netmaker wait"
        return 0
    fi
    echo "waiting for $NETMAKER_CT (RUNNING + service manager ready over D-Bus)"
    i=0
    until incus list -f csv -c ns 2>/dev/null | grep -q "^${NETMAKER_CT},RUNNING"; do
        i=$((i+1))
        [ "$i" -ge 60 ] && { echo "$NETMAKER_CT not RUNNING after 60s — continuing without it"; return 0; }
        sleep 1
    done
    # container service-manager readiness over D-Bus — busctl, NEVER systemctl
    i=0
    state=""
    while :; do
        state="$(incus exec "$NETMAKER_CT" -- busctl --system get-property \
            org.freedesktop.systemd1 /org/freedesktop/systemd1 \
            org.freedesktop.systemd1.Manager SystemState 2>/dev/null |
            tr -d '"' | awk '{print $2}')" || true
        case "$state" in
            running|degraded) break ;;
        esac
        i=$((i+1))
        [ "$i" -ge 180 ] && { echo "$NETMAKER_CT service manager not ready after 180s (state: ${state:-none}) — continuing"; return 0; }
        sleep 1
    done
    echo "$NETMAKER_CT ready (service manager: $state)"
}

phase_service_addrs() {
    # STRICTLY after netmaker: the host service addresses must never race
    # the container's own bring-up on the bridge.
    phase_netmaker_wait
    for a in ${BRIDGE_SVC_ADDRS:-10.200.0.1/24 10.0.0.2/24 10.200.0.2/24}; do
        ip addr replace "$a" dev "$BRIDGE"
    done
    # netmaker net rides the bridge; this route lets 10.0.0.2 egress reach
    # host xray for identity header injection
    ip route replace "${NETMAKER_NET:-10.0.0.0/24}" dev "$BRIDGE" 2>/dev/null || true
    sysctl -qw net.ipv4.ip_forward=1 || true
    if [ "${NAT_ENABLE:-1}" = "1" ] && command -v iptables >/dev/null 2>&1; then
        iptables -t nat -C POSTROUTING -s "${BRIDGE_NET:-10.200.0.0/24}" ! -d "${BRIDGE_NET:-10.200.0.0/24}" -j MASQUERADE 2>/dev/null ||
        iptables -t nat -A POSTROUTING -s "${BRIDGE_NET:-10.200.0.0/24}" ! -d "${BRIDGE_NET:-10.200.0.0/24}" -j MASQUERADE
    fi
    echo "$BRIDGE service addresses applied after netmaker: ${BRIDGE_SVC_ADDRS:-10.200.0.1/24 10.0.0.2/24 10.200.0.2/24}"
}

phase_all() {
    install -d -m 0755 /run/opdbus
    phase_dhcp
    if ! pgrep -x ovs-vswitchd >/dev/null 2>&1; then
        # supervised pieces go through the sanctioned wrapper only
        service6 start ovs-vswitchd-pipeline ||
            service6 start ovs-vswitchd ||
            { echo "service6 start ovs-vswitchd failed"; exit 1; }
    fi
    phase_uplink
    if [ -d "/etc/s6/sv/incus-ct-${NETMAKER_CT}" ]; then
        service6 start "incus-ct-${NETMAKER_CT}-pipeline" ||
            service6 start "incus-ct-${NETMAKER_CT}" || true
    fi
    phase_service_addrs
}

case "${1:-all}" in
    dhcp)          phase_dhcp ;;
    seed)          phase_seed ;;
    uplink)        phase_uplink ;;
    netmaker-wait) phase_netmaker_wait ;;
    service-addrs) phase_service_addrs ;;
    all)           phase_all ;;
    *)
        echo "usage: 3tched-bridge-up [dhcp|seed|uplink|netmaker-wait|service-addrs|all]" >&2
        exit 2
        ;;
esac
BRIDGEUP
    chmod 0755 "${BIN}/3tched-bridge-up"
    ok "3tched-bridge-up installed"
}

# ============================================================================
# S6 GRAPH HELPERS (source-dir surgery only; no lifecycle calls)
# ============================================================================

mk_oneshot() {
    # mk_oneshot <name> <up-line> [dep...]
    local svc="$1" up="$2"; shift 2
    local dir="${SV}/${svc}"
    install -d "$dir" "${dir}/dependencies.d"
    echo oneshot > "${dir}/type"
    echo "$up" > "${dir}/up"
    local d
    for d in "$@"; do
        if [[ -d ${SV}/$d ]]; then
            touch "${dir}/dependencies.d/$d"
        else
            warn "$svc: skipping missing dependency $d"
        fi
    done
    TOUCHED_SERVICES+=("$svc")
}

add_dep() {
    # add_dep <svc> <dep> — only when both exist
    [[ -d ${SV}/$1 && -d ${SV}/$2 ]] || return 0
    install -d "${SV}/$1/dependencies.d"
    touch "${SV}/$1/dependencies.d/$2"
    # $1 gained a new dependency edge: it must be re-enabled alongside that
    # dependency in the same `service6 enable` call, or s6 set enable's
    # default -I warn behavior leaves the set inconsistent (dependency not
    # started at boot) instead of failing loudly.
    TOUCHED_SERVICES+=("$1")
}

del_dep() {
    rm -f "${SV}/$1/dependencies.d/$2"
}

bundle_del_everywhere() {
    # remove an entry from every bundle's contents.d
    local entry="$1" b
    for b in "$SV"/*/; do
        [[ -f ${b}type && $(<"${b}type") == bundle ]] || continue
        rm -f "${b}contents.d/${entry}"
        if [[ -f ${b}contents ]]; then
            sed -i "/^${entry}\$/d" "${b}contents"
        fi
    done
}

bundle_add_3tched() {
    local entry="$1"
    local dir="${SV}/3tched"
    [[ -d $dir ]] || { warn "no 3tched bundle — add $entry to a boot bundle manually"; return 0; }
    install -d "${dir}/contents.d"
    touch "${dir}/contents.d/${entry}"
}

retire_service() {
    # remove a service from the source tree, every bundle, and every
    # dependencies.d that references it
    local svc="$1" d
    bundle_del_everywhere "$svc"
    for d in "$SV"/*/dependencies.d; do
        rm -f "${d}/${svc}"
    done
    if [[ -d ${SV}/${svc} ]]; then
        rm -rf "${SV:?}/${svc}"
        ok "retired s6 service: $svc"
    fi
    RETIRED_SERVICES+=("$svc")
}

# ============================================================================
# S6 GRAPH — corrected network boot order
# ============================================================================

write_network_services() {
    log "Rewriting the network s6 graph ..."

    # -- retire the backwards oneshots (the deployed ovsbr0-uplink-addr
    #    waited for netmaker before the uplink — that dependency was
    #    backwards; ovsbr0-addr applied service addresses too early) --------
    retire_service ovsbr0-addr
    retire_service ovsbr0-uplink-addr

    # -- uplink-dhcp: lease + immutable migration snapshot, BEFORE OVS ------
    mk_oneshot uplink-dhcp "${BIN}/3tched-bridge-up dhcp" opdbus-rundirs
    add_dep ovsdb-server uplink-dhcp

    # -- ovs-vswitchd: seed bridge + eth0 in ONE transact, then exec --------
    if [[ -d ${SV}/ovs-vswitchd ]]; then
        cat > "${SV}/ovs-vswitchd/run" <<'EOF'
#!/bin/sh
exec 2>&1
set -a; [ -r /etc/op-dbus/network.conf ] && . /etc/op-dbus/network.conf; set +a
export OVSDB_SOCKET="${OVSDB_SOCKET:-/run/openvswitch/db.sock}"
# bridge creation + UPLINK enslavement in ONE OVSDB transact, before vswitchd
/usr/local/bin/3tched-bridge-up seed || echo "bridge seed failed (continuing)"
exec ovs-vswitchd "unix:${OVSDB_SOCKET}"
EOF
        chmod 0755 "${SV}/ovs-vswitchd/run"
        add_dep ovs-vswitchd uplink-dhcp
    else
        warn "no ovs-vswitchd service in $SV — seed phase not wired"
    fi

    # -- ovsbr0-uplink: host uplink complete BEFORE any container -----------
    mk_oneshot ovsbr0-uplink "${BIN}/3tched-bridge-up uplink" ovs-vswitchd

    # -- ovsbr0-svc-addr: 10.200.0.1 + 10.0.0.2 + 10.200.0.2 strictly AFTER netmaker -----
    mk_oneshot ovsbr0-svc-addr "${BIN}/3tched-bridge-up service-addrs" ovsbr0-uplink

    # -- container runtime sits between uplink and service addresses --------
    add_dep incusd ovsbr0-uplink

    # -- consumers of the service addresses ---------------------------------
    add_dep op-of-controller ovsbr0-svc-addr   # binds 10.200.0.1:6653
    add_dep opdbus ovsbr0-svc-addr             # binds 10.200.0.1:50051
    add_dep xray ovsbr0-uplink                 # binds the public uplink IP

    # -- EARLY BOOT GUARANTEE: session bus + gRPC bridge never wait on the
    #    network (UDS + loopback only; blobs sealed by opdbus-rundirs) ------
    local early svc
    for early in op-session-bus op-grpc-bridge; do
        [[ -d ${SV}/${early} ]] || continue
        for svc in uplink-dhcp ovsdb-server ovs-vswitchd ovsbr0-uplink ovsbr0-svc-addr incusd op-of-controller; do
            del_dep "$early" "$svc"
        done
        rm -f "${SV}/${early}"/dependencies.d/incus-ct-* 2>/dev/null || true
    done

    bundle_add_3tched uplink-dhcp
    bundle_add_3tched ovsbr0-uplink
    bundle_add_3tched ovsbr0-svc-addr
    ok "network s6 graph rewritten"
}

# ============================================================================
# NETMAKER CONTAINER SUPERVISOR — busctl readiness, no systemctl
# ============================================================================

write_netmaker_service() {
    local nm bridge svc dir
    nm="$(set -a; . "$NET_CONF" 2>/dev/null; set +a; echo "${NETMAKER_CT:-netmaker}")"
    bridge="$(set -a; . "$NET_CONF" 2>/dev/null; set +a; echo "${BRIDGE:-ovsbr0}")"
    svc="incus-ct-${nm}"
    dir="${SV}/${svc}"
    if [[ ! -d $dir ]]; then
        warn "no $svc service — generate it first, then re-run this bootstrap"
        return 0
    fi
    log "Rewriting $svc supervisor (busctl readiness, s6 notification) ..."

    cat > "${dir}/run" <<'EOF'
#!/bin/sh
# s6-supervised Incus container '@NM@' — s6 owns the lifecycle, not Incus.
# Internal service-manager operations go over D-Bus with busctl; systemctl
# is banned in this path.
exec 2>&1
NAME='@NM@'
ATTACH_BRIDGE='@BRIDGE@'
i=0
until incus info >/dev/null 2>&1; do
    i=$((i+1))
    [ "$i" -ge 60 ] && { echo "incusd not ready"; exit 1; }
    sleep 1
done
incus config set "$NAME" boot.autostart=false 2>/dev/null || true
# Never invent a NIC. Most identity containers (including netmaker as of the
# NIC removal below) are deliberately NIC-less: all I/O goes through
# rust-network-mgr's socket + host-side Incus proxy devices, never a bridged
# eth0. If a NIC IS configured (existing eth0 only, never created here),
# incus enslaves its veth into ATTACH_BRIDGE on start; if not, that's the
# expected NIC-less path and this is informational only, not an error.
if incus config device show "$NAME" 2>/dev/null | grep -q '^eth0:'; then
    HAS_NIC=1
else
    HAS_NIC=0
fi
if ! incus list -f csv -c ns | grep -q "^${NAME},RUNNING"; then
    echo "starting container $NAME"
    incus start "$NAME"
fi
# readiness: internal service manager over D-Bus (busctl), never systemctl
j=0
state=""
while :; do
    state="$(incus exec "$NAME" -- busctl --system get-property \
        org.freedesktop.systemd1 /org/freedesktop/systemd1 \
        org.freedesktop.systemd1.Manager SystemState 2>/dev/null |
        tr -d '"' | awk '{print $2}')" || true
    case "$state" in
        running|degraded) break ;;
    esac
    j=$((j+1))
    [ "$j" -ge 180 ] && { echo "$NAME service manager not ready after 180s (state: ${state:-none})"; break; }
    sleep 1
done
if [ "$HAS_NIC" = 1 ]; then
    echo "$NAME enslaved on $ATTACH_BRIDGE (service manager: ${state:-unknown})"
else
    echo "$NAME ready — NIC-less (rust-network-mgr socket + host proxy device), service manager: ${state:-unknown}"
fi
# s6 readiness notification — dependents (ovsbr0-svc-addr) start only now
{ echo; } >&3 2>/dev/null || true
# hold supervision and stream the container console into the log pipeline;
# stopping this service stops the container ('script' provides the pty
# incus console needs)
script -qefc "incus console $NAME" /dev/null &
pid=$!
trap 'incus stop "$NAME" --timeout 30 2>/dev/null || true; kill "$pid" 2>/dev/null || true' TERM INT
wait "$pid"
EOF
    sed -i "s/@NM@/${nm}/g; s/@BRIDGE@/${bridge}/g" "${dir}/run"
    chmod 0755 "${dir}/run"
    echo 3 > "${dir}/notification-fd"

    # correct ordering: after incusd + host uplink; NEVER after the OpenFlow
    # controller (which now waits on the post-netmaker service addresses —
    # keeping that old dependency would be a cycle)
    del_dep "$svc" op-of-controller
    del_dep "$svc" ovsbr0-addr
    del_dep "$svc" ovsbr0-uplink-addr
    add_dep "$svc" incusd
    add_dep "$svc" ovsbr0-uplink

    # the service addresses land only after this container is ready
    add_dep ovsbr0-svc-addr "$svc"
    ok "$svc rewritten (busctl readiness, notification-fd 3)"
}

# ============================================================================
# COMPILE (no starts) + SUMMARY
# ============================================================================

compile_db() {
    log "Committing the s6 service set (service6 only — raw s6/s6-rc calls are blocked for this account) ..."

    # De-duplicate TOUCHED_SERVICES, keep only names that still have a
    # source dir (retired ones are handled separately, below).
    local -A seen=()
    local -a enable_list=()
    local s
    for s in "${TOUCHED_SERVICES[@]}"; do
        [[ -n ${seen[$s]:-} ]] && continue
        seen[$s]=1
        [[ -d ${SV}/$s ]] && enable_list+=("$s")
    done

    if ((${#RETIRED_SERVICES[@]})); then
        log "Disabling retired services in the working set: ${RETIRED_SERVICES[*]}"
        # Best-effort: if `repository sync`/a prior run already pruned these
        # from the reference database (source is gone), service6 reports it
        # and moves on rather than failing — that is the expected steady
        # state, not an error.
        service6 disable "${RETIRED_SERVICES[@]}" ||
            warn "service6 disable reported issues for some retired services (likely already pruned) — continuing"
    fi

    ((${#enable_list[@]})) || die "no touched services recorded — nothing to enable/commit"

    # `service6 enable` runs, in order: s6 set enable <names> -> s6 set check
    # -> s6 set commit -> s6 live install.  ALL of the services that gained a
    # new dependency edge on a not-yet-enabled service must be listed in this
    # SAME call — s6 set enable's default (-I warn) does not pull in or fail
    # on unlisted cross-dependencies, it just warns and leaves the set
    # inconsistent (dependency silently not started at boot).  TOUCHED_SERVICES
    # is exactly that full set, recorded as mk_oneshot/add_dep/retire_service
    # ran above, so nothing needs to be enumerated by hand here.
    #
    # IMPORTANT: `s6 live install` does not just register the new database —
    # per its own manual, "services that are not defined in the new set are
    # stopped before the live database is replaced", and s6-rc-update also
    # restarts any EXISTING service whose definition changed (e.g. rewritten
    # run scripts). In practice this means the call below performs a REAL,
    # LIVE cutover of the whole touched dependency subgraph — including
    # already-running services like ovs-vswitchd/ovsdb-server/xray/incusd —
    # not a side-effect-free compile. Do not run this while unsure whether
    # you want that cutover to happen right now.
    warn "the next step live-restarts every touched service (${enable_list[*]}) — this is a real cutover, not a dry compile"
    log "Enabling + committing + installing: ${enable_list[*]}"
    service6 enable "${enable_list[@]}"
    # service6 swallows internal CalledProcessErrors and can still exit 0 on
    # a failed s6 set/live step; always read its own printed output above,
    # then confirm with `service6 list` and `service6 log <svc>`.

    ok "s6-rc database committed and installed — verify with: service6 list"
}

summary() {
    echo
    echo "============================================================================"
    echo -e "${GREEN}NETWORK BOOTSTRAP COMMITTED AND INSTALLED${NC}"
    echo -e "${YELLOW}service6 enable's 'live install' step already cut the running system over"
    echo -e "to this graph — it does not just stage a database. Verify below, don't assume.${NC}"
    echo "============================================================================"
    echo
    echo "Boot order now:"
    echo "  uplink-dhcp (lease + snapshot on eth0, before OVS)"
    echo "  -> ovsdb-server -> ovs-vswitchd (bridge + eth0 in ONE OVSDB transact)"
    echo "  -> ovsbr0-uplink (public addrs + default route migrate to ovsbr0)"
    echo "  -> incusd -> incus-ct-netmaker (busctl readiness over D-Bus)"
    echo "  -> ovsbr0-svc-addr (10.200.0.1 + 10.0.0.2 + 10.200.0.2 AFTER netmaker)"
    echo "  -> op-of-controller / opdbus"
    echo "  (op-session-bus + op-grpc-bridge stay network-independent: early)"
    echo
    echo "One command (manual/recovery): 3tched-bridge-up [phase|all]"
    echo
    echo "Verify now:"
    echo "  service6 list                   # uplink-dhcp/ovsbr0-uplink/ovsbr0-svc-addr should show ✔"
    echo "  ip addr show ovsbr0             # expect the uplink addr + BRIDGE_SVC_ADDRS"
    echo "  ping -c2 <default gateway>"
    echo
    echo "Rollback (restores source only — you still need to re-enable the old"
    echo "service names with service6 for it to take live effect):"
    echo "  cp -a $BACKUP/sv/. $SV/"
    echo "============================================================================"
}

main() {
    preflight
    backup
    ensure_standard_oneshots
    update_net_conf
    install_bridge_up
    write_network_services
    write_netmaker_service
    compile_db
    summary
}

main "$@"
