#!/usr/bin/env bash
#
# 3tched-network-install.sh — install the network control plane as it is
# actually running on this host (188.68.58.237, Artix + runit, 2026-08-13).
#
# Replaces 3tched-network-bootstrap.sh, whose header described a topology that
# no longer exists (netmaker reached via a host-side Incus proxy device; per-CT
# proxy devices generally). Every value below was read off the live host rather
# than carried forward from a doc:
#
#   ovsbr0  fail_mode=standalone datapath_type=system
#     LOCAL ovsbr0  10.200.0.1/24   fabric + OpenFlow controller address
#     1     pub0    188.68.58.237/22 public L3, wears eth0's MAC, default route
#     2     3tched  10.0.0.2/24     incoming identity
#                    10.200.0.2/24  incoming identity
#     3     svc0    10.0.0.3/24     Tonic service (:8090)
#                    10.0.0.1/24 belongs on decoy wg0, not this host
#     4     eth0    physical uplink, enslaved LAST, no L3 of its own
#   controller  tcp:10.200.0.1:6653 (op-of-controller), two cookie classes:
#     FALLBACK 0x3344434800000001 — table=0 priority=0 actions=NORMAL, the
#       standalone safety net that keeps the datapath forwarding with no
#       controller attached (ensure_fallback_normal).
#     MANAGED  0x3344434800000002 — the 4 durable static flows from
#       deploy/config/openflow-static-flows.json: priority=100 NORMAL for
#       netmaker API tcp/10.200.0.1:8081 and QUIC udp/188.68.58.237:443,
#       each direction. Deleted-by-cookie and reinstalled on every OVS
#       reconnect, so they are the set the controller actually owns.
#     NOTE the live table held only the FALLBACK flow between 2026-08-10
#     22:17 and this script's first run: /etc/op-dbus/openflow-static-flows.json
#     had been truncated to `[]`, so the controller logged "Loaded 0 static
#     flow(s)" on every start. Recovered from the btrfs snapshot
#     /.snapshots/root-20260806-062641-postcommit-1afa6c25.
#   containers  socket-only, every one of them: the sole network path is the
#     /run/ghostbridge bind mount. No NICs, no incus proxy devices, anywhere.
#
# What this script does, in order:
#   1. Backs up /etc/runit/sv, network.conf and the libexec tree.
#   2. Writes /etc/op-dbus/network.conf from the values above (or, with
#      --capture, re-derives them from the live host at run time).
#   3. Installs the libexec helpers and the network runit services from this
#      repo (deploy/runit/, deploy/runit/libexec-3tched/).
#   4. Enables and starts the network chain in dependency order.
#   5. Stages the static flow set from deploy/config/openflow-static-flows.json
#      BEFORE starting op-of-controller (it reads that file only at startup),
#      refuses a 0-flow set, attaches the controller, and reads back both
#      cookie classes to prove the table is what was asked for.
#   6. Converges every Incus container onto the shared-socket model: ensures
#      the ghostbridge-socket mount exists, and reports (or, with
#      --strip-legacy-devices, removes) any nic/proxy device it finds.
#
# OVS is driven through the plugin tree (zcall ovsdb_bridge / openflow), never
# ovs-vsctl or ovs-ofctl. Container devices go through the incus plugin, never
# `incus config device add`.
#
# Usage:
#   sudo ./3tched-network-install.sh [options]
#     --capture                 re-derive network.conf from live host state
#     --no-start                install files + enable, do not sv start/restart
#     --skip-containers         leave Incus containers untouched
#     --strip-legacy-devices    remove nic/proxy devices from containers
#     --allow-empty-flows       permit a 0-flow static set (normally fatal)
#     --skip-plugin-check       install files even if the plugin tree is down
#                               (steps 5 and 6 will not do anything)
#     --dry-run                 print what would change, touch nothing
#
# Exit status is 1 if any load-bearing step did not complete, with the list
# printed at the end. A 0 exit means the datapath really is what the summary
# says it is — earlier versions exited 0 having attached no controller and
# installed no flows.
#
# Rollback (console): the backup directory printed at the top holds the
# previous /etc/runit/sv, network.conf and libexec tree. Restore with
#   cp -a <backup>/sv/. /etc/runit/sv/ && cp -a <backup>/libexec/. /usr/local/libexec/3tched/
# then `sv restart` the affected names.

set -euo pipefail

# ── Paths ─────────────────────────────────────────────────────────────────────
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SV=/etc/runit/sv
RUNSVDIR=/etc/runit/runsvdir/default
CONF_DIR=/etc/op-dbus
NET_CONF="$CONF_DIR/network.conf"
STATIC_FLOWS="$CONF_DIR/openflow-static-flows.json"
LIBEXEC=/usr/local/libexec/3tched
OF_LOG=/var/log/op-dbus/op-of-controller
BACKUP="/var/lib/op-dbus/network-install-backup-$(date +%Y%m%d-%H%M%S)"

CAPTURE=false
DO_START=true
DO_CONTAINERS=true
STRIP_LEGACY=false
DRY_RUN=false
ALLOW_EMPTY_FLOWS=false
SKIP_PLUGIN_CHECK=false

for arg in "$@"; do
    case "$arg" in
        --capture)              CAPTURE=true ;;
        --no-start)             DO_START=false ;;
        --skip-containers)      DO_CONTAINERS=false ;;
        --strip-legacy-devices) STRIP_LEGACY=true ;;
        --allow-empty-flows)    ALLOW_EMPTY_FLOWS=true ;;
        --skip-plugin-check)    SKIP_PLUGIN_CHECK=true ;;
        --dry-run|--print)      DRY_RUN=true ;;
        -h|--help)              sed -n '2,65p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
log()  { echo -e "${GREEN}[INFO]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()  { echo -e "${RED}[ERROR]${NC} $*" >&2; }
die()  { err "$*"; exit 1; }

run() {
    if $DRY_RUN; then echo "  would run: $*"; else "$@"; fi
}

# Steps that go through the plugin tree used to be `|| warn` and nothing else,
# so a run in which the controller was never attached and not one flow was
# installed still exited 0. Failures are recorded and reported, and the script
# exits non-zero if anything load-bearing did not happen.
FAILURES=()
fail() { err "$*"; FAILURES+=("$*"); }
# Service dirs under $SV that are symlinks — see the ovsbr0-eth0 note below.
SYMLINKED_SERVICES=()
# Services whose run/check/finish this run actually rewrote. Only these get
# restarted; restarting the whole chain would needlessly cycle the datapath.
CHANGED_SERVICES=()

# ── Live topology, as read off this host ──────────────────────────────────────
# Defaults are the current live values. --capture re-reads them instead, so the
# script stays correct if the host is re-addressed before it is next run.
BRIDGE=ovsbr0
FAIL_MODE=standalone
DATAPATH_TYPE=system
UPLINK_PHYS=eth0
PUBLIC_PORT=pub0
PUBLIC_ADDR=188.68.58.237/22
PUBLIC_GW=188.68.56.1
INTERNAL_PORTS="3tched svc0"
BRIDGE_ADDR=10.200.0.1/24
BRIDGE_SVC_ADDRS="10.0.0.2/24@3tched 10.200.0.2/24@3tched 10.0.0.3/24@svc0"
OF_CONTROLLER_LISTEN=10.200.0.1:6653
OF_CONTROLLER_ENDPOINT=tcp:10.200.0.1:6653
OF_PROTOCOLS=OpenFlow10,OpenFlow13,OpenFlow15
OF_STATIC_FDB=ovsbr0:eth0:0:00:00:5e:00:01:0a
NETMAKER_NET=100.69.0.0/16
NETMAKER_CT=netmaker
NETMAKER_INCUS_NAME=NetMaker

capture_live() {
    log "Capturing live network state..."
    local a
    a="$(ip -4 -o addr show dev "$PUBLIC_PORT" scope global 2>/dev/null | awk '{print $4; exit}')"
    [ -n "$a" ] && PUBLIC_ADDR="$a"
    a="$(ip -4 route show default 2>/dev/null | awk '/dev '"$PUBLIC_PORT"'/ {print $3; exit}')"
    [ -n "$a" ] && PUBLIC_GW="$a"
    a="$(ip -4 -o addr show dev "$BRIDGE" scope global 2>/dev/null | awk '{print $4; exit}')"
    [ -n "$a" ] && BRIDGE_ADDR="$a"
    local svc=""
    local port addr
    for port in $INTERNAL_PORTS; do
        while read -r addr; do
            # 10.0.0.1 belongs on decoy wg0. Never capture it onto the host.
            case "$addr" in
                10.0.0.1/*|10.0.0.1) continue ;;
            esac
            [ -n "$addr" ] && svc="$svc $addr@$port"
        done < <(ip -4 -o addr show dev "$port" scope global 2>/dev/null | awk '{print $4}')
    done
    [ -n "$svc" ] && BRIDGE_SVC_ADDRS="${svc# }"
    log "  public  $PUBLIC_ADDR via $PUBLIC_GW on $PUBLIC_PORT"
    log "  fabric  $BRIDGE_ADDR on $BRIDGE"
    log "  internal $BRIDGE_SVC_ADDRS"
}

# ── Preflight ─────────────────────────────────────────────────────────────────
[[ $EUID -eq 0 ]] || $DRY_RUN || die "must run as root"
[ -d "$REPO/deploy/runit" ] || die "$REPO/deploy/runit missing — run from the repo checkout"
command -v sv >/dev/null || die "sv not found — this host is not running runit"
command -v zcall >/dev/null || die "zcall not found — install the operator CLI"
[ -d "/sys/class/net/$UPLINK_PHYS" ] || die "uplink $UPLINK_PHYS does not exist"

# `command -v zcall` proves the binary exists, not that it can reach anything.
# Every mutation in steps 5 and 6 goes through the plugin tree, so if the tree
# is not on the bus this run can only produce warnings — and used to do exactly
# that, silently. zcall talks to the session bus at
# /run/opdbus/session-bus.sock, where op-grpc-bridge publishes the objects and
# claims org.opdbus.v1.plugins. Note `zcall methods <plugin>` answers from the
# sealed blob catalog in SHM and works even when the bus is empty, so it is not
# a liveness probe; `zcall list` is.
if ! zcall list >/dev/null 2>&1; then
    msg="plugin tree unreachable — 'zcall list' fails against the session bus.
       op-grpc-bridge is what publishes the tree and claims org.opdbus.v1.plugins;
       check 'sv status op-grpc-bridge' and its log for a wait_dep timeout.
       Without it, controller attach, flow install and container convergence
       cannot run. Re-run with --skip-plugin-check to install files anyway."
    if $SKIP_PLUGIN_CHECK; then warn "$msg"; else die "$msg"; fi
fi

$CAPTURE && capture_live

# ── 1. Backup ─────────────────────────────────────────────────────────────────
if ! $DRY_RUN; then
    install -d -m 0755 "$BACKUP"
    [ -d "$SV" ]      && cp -a "$SV"      "$BACKUP/sv"
    [ -d "$LIBEXEC" ] && cp -a "$LIBEXEC" "$BACKUP/libexec"
    [ -f "$NET_CONF" ] && cp -a "$NET_CONF" "$BACKUP/network.conf"
    log "backup: $BACKUP"
else
    log "backup: (dry-run, would be $BACKUP)"
fi

# ── 2. network.conf ───────────────────────────────────────────────────────────
log "Writing $NET_CONF"
if $DRY_RUN; then
    echo "  would write $NET_CONF (BRIDGE=$BRIDGE PUBLIC_ADDR=$PUBLIC_ADDR INTERNAL=$BRIDGE_SVC_ADDRS)"
else
    install -d -m 0755 "$CONF_DIR"
    cat >"$NET_CONF" <<EOF
# 3tched network configuration — generated by 3tched-network-install.sh
# $(date -Is)
#
# Boot order (intentional):
#   opdbus-rundirs -> uplink-dhcp -> ovsdb-server -> ovs-vswitchd (seed:
#   bridge + internal ports, NO eth0) -> ovsbr0-uplink -> ovsbr0-eth0
#   (enslave physical uplink, migrate L3 onto pub0) -> ovsbr0-svc-addr
#   -> op-of-controller
#
# Model:
#   - eth0 enslaved last, no L3 on eth0
#   - pub0: public uplink L3, wears eth0's MAC (provider filters on it)
#   - 3tched: single incoming port carrying 10.0.0.2 and 10.200.0.2
#   - svc0: Tonic service at 10.0.0.3:8090
#   - 10.0.0.1/24 is decoy wg0, never a host internal-port address
#   - ovsbr0 LOCAL: fabric address, also the OpenFlow controller address
#   - every container is socket-only: /run/ghostbridge bind mount, no NIC,
#     no incus proxy devices. That rule is about CONTAINERS.
#   - "netmaker" is a WireGuard interface managed by netclient. It BELONGS
#     on ovsbr0 as a system port (netmaker-ovs-attach, after netclient has
#     100.69.0.1). Not a wg0-style identity interface — this host has no wg0.
#   - gRPC/HTTP entrance is tonic :8090 (op-grpc-bridge). Never :50051.

BRIDGE=$BRIDGE
FAIL_MODE=$FAIL_MODE
DATAPATH_TYPE=$DATAPATH_TYPE
OVSDB_SOCKET=/run/openvswitch/db.sock
VSWITCHD_SVC=ovs-vswitchd

# Physical NIC. Seed/uplink leave UPLINK empty; ovsbr0-eth0 enslaves this last.
UPLINK_PHYS=$UPLINK_PHYS
UPLINK=

PUBLIC_PORT=$PUBLIC_PORT
PUBLIC_ADDR=$PUBLIC_ADDR
PUBLIC_GW=$PUBLIC_GW

INTERNAL_PORTS="$INTERNAL_PORTS"

BRIDGE_ADDR=$BRIDGE_ADDR
BRIDGE_SVC_ADDRS="$BRIDGE_SVC_ADDRS"

# OpenFlow: OVS dials the controller, never the other way around.
OF_CONTROLLER_LISTEN=$OF_CONTROLLER_LISTEN
OPENFLOW_CONTROLLER=$OF_CONTROLLER_ENDPOINT
OF_PROTOCOLS=$OF_PROTOCOLS
OF_STATIC_FLOWS_FILE=$STATIC_FLOWS
OF_FLOW_PAIRS=
OF_STATIC_FDB=$OF_STATIC_FDB

NETMAKER_NET=$NETMAKER_NET
NETMAKER_CT=$NETMAKER_CT
NETMAKER_INCUS_NAME=$NETMAKER_INCUS_NAME
NETMAKER_PORT=netmaker
BRIDGE_MTU=1420
NETMAKER_MTU=1500
IDENTITY_HOST=10.10.0.5
IDENTITY_VIA=100.69.0.2
IDENTITY_DEV=netmaker
EOF
    chmod 0644 "$NET_CONF"
fi

# One authoritative MQTT/WebSocket door shared by the host runit graph and the
# NetMaker container convergence tool.
run install -d -m 0755 "$CONF_DIR"
run install -m 0644 "$REPO/deploy/config/netmaker-broker.env" \
    "$CONF_DIR/netmaker-broker.env"

# ── 3. libexec helpers + runit services ───────────────────────────────────────
log "Installing libexec helpers into $LIBEXEC"
run install -d -m 0755 "$LIBEXEC"
for f in "$REPO"/deploy/runit/libexec-3tched/*; do
    [ -f "$f" ] || continue
    run install -m 0755 "$f" "$LIBEXEC/$(basename "$f")"
done
# opdbus-rundirs-up ships under scripts/ as well; the runit copy wins if both exist.
[ -f "$REPO/scripts/opdbus-rundirs-up" ] && [ ! -f "$REPO/deploy/runit/libexec-3tched/opdbus-rundirs-up" ] \
    && run install -m 0755 "$REPO/scripts/opdbus-rundirs-up" "$LIBEXEC/opdbus-rundirs-up"

# The network chain, in dependency order. Each name must exist under
# deploy/runit/ — anything missing is a repo gap, not something to paper over.
NET_SERVICES=(
    opdbus-rundirs
    uplink-dhcp
    ovsdb-server
    ovs-vswitchd
    ovsbr0-uplink
    ovsbr0-eth0
    ovsbr0-svc-addr
    op-of-controller
)
# Mesh: NetMaker CT sockets -> netclient IP -> enslave netmaker on ovsbr0.
MESH_SERVICES=(
    incus-ct-netmaker
    uds-netmaker-api
    nm-api-tls
    netclient
    netmaker-ovs-attach
    nm-identity-route
)
# Container socket relays — the only network path any container has.
# 8090 tonic (grpc + http) is the shared door; mail-port-fabric is SMTP/IMAP.
SOCK_SERVICES=(
    xsock-web
    xsock-qdrant
    xsock-netmaker
    xsock-decoy
    uds-assistant
    uds-cozo-chat
    uds-qdrant-grpc
    uds-qdrant-http
    uds-qdrant-http-svc0
    uds-xray-reality
    mail-port-fabric
    mail-web-socket
    fwd-8090
    fwd-nm-mesh-8090
)

install_service() {
    local name="$1" src="$REPO/deploy/runit/$1"
    [ -d "$src" ] || { warn "no service definition for $name in repo — skipping"; return 1; }

    # A service dir under $SV can be a symlink to a `.retired-<ts>` copy left
    # behind by a half-finished retirement (ovsbr0-eth0 is one on this host).
    # `install` follows it, so the files land in the retired dir — which is also
    # where runsv is chdir'd, so it does work, but `ls $SV` tells you nothing
    # about where the live definition lives. Install through it and say so.
    if [ -L "$SV/$name" ]; then
        warn "  $name: $SV/$name is a symlink -> $(readlink "$SV/$name")"
        SYMLINKED_SERVICES+=("$name -> $(readlink "$SV/$name")")
    fi

    run install -d -m 0755 "$SV/$name"
    local f changed=false
    for f in run finish check conf; do
        [ -f "$src/$f" ] || continue
        cmp -s "$src/$f" "$SV/$name/$f" || changed=true
        run install -m 0755 "$src/$f" "$SV/$name/$f"
    done
    if [ -d "$src/log" ]; then
        run install -d -m 0755 "$SV/$name/log"
        if [ -f "$src/log/run" ]; then
            cmp -s "$src/log/run" "$SV/$name/log/run" || changed=true
            run install -m 0755 "$src/log/run" "$SV/$name/log/run"
        fi
    fi
    $changed && CHANGED_SERVICES+=("$name")
    return 0
}

# `sv start` on a service runsv already has up is a no-op: the run script that
# is executing stays executing, so a definition installed a moment ago does not
# take effect until something restarts it. That is how this host ended up with
# a correct ovsbr0-eth0 on disk while runsv kept executing the previous one,
# blocked forever on a dependency the new script does not even have. Restart
# what changed; plain start for the rest, so an unchanged datapath is not cycled.
start_service() {
    local name="$1"
    if [[ " ${CHANGED_SERVICES[*]-} " == *" $name "* ]]; then
        log "  $name: definition changed — restarting"
        run sv restart "$name" || { fail "$name failed to restart"; return 1; }
    else
        run sv start "$name" || { fail "$name failed to start"; return 1; }
    fi
    return 0
}

log "Installing network services"
for s in "${NET_SERVICES[@]}"; do install_service "$s" && log "  $s"; done
log "Installing mesh + netclient attach"
for s in "${MESH_SERVICES[@]}"; do install_service "$s" && log "  $s"; done
log "Installing container socket relays"
for s in "${SOCK_SERVICES[@]}"; do install_service "$s" && log "  $s"; done

# ── Static flows — installed BEFORE op-of-controller starts ───────────────────
# op-of-controller reads OF_STATIC_FLOWS_FILE exactly once, in main(), at
# startup. Installing the file after the service is running is a silent no-op
# until the next restart, which is how the table can look wrong while the file
# on disk looks right. So it lands here, ahead of the start loop.
#
# The guard exists because of what actually happened on 2026-08-10 22:17: the
# file was truncated to `[]`, the controller logged "Loaded 0 static flow(s)",
# and the datapath ran on the priority=0 fallback alone for three days without
# anything reporting a fault. An empty flow set is almost never intended — it
# has to be asked for explicitly.
SRC_FLOWS="$REPO/deploy/config/openflow-static-flows.json"
log "Installing static flows -> $STATIC_FLOWS"
if [ ! -f "$SRC_FLOWS" ]; then
    warn "  $SRC_FLOWS missing — controller will start with no static flows"
elif ! python3 -c "import json,sys; json.load(open(sys.argv[1]))" "$SRC_FLOWS" 2>/dev/null; then
    die "  $SRC_FLOWS is not valid JSON — refusing to install a file the controller cannot parse"
else
    FLOW_COUNT="$(python3 -c "import json,sys; print(len(json.load(open(sys.argv[1]))))" "$SRC_FLOWS")"
    if [ "$FLOW_COUNT" -eq 0 ] && ! $ALLOW_EMPTY_FLOWS; then
        die "  $SRC_FLOWS has 0 flows — this is the 2026-08-10 truncation signature.
       Restore it (btrfs snapshots under /.snapshots hold known-good copies) or
       pass --allow-empty-flows if a bare fallback-only table is really intended."
    fi
    run install -m 0644 "$SRC_FLOWS" "$STATIC_FLOWS"
    log "  $FLOW_COUNT flow(s) staged for op-of-controller"
fi

# ── Retire ovsbr0-addr ────────────────────────────────────────────────────────
# It is still enabled and "running" on this host, but its helper
# (/usr/local/libexec/3tched/ovsbr0-addr-up) does not exist — the run script has
# no `set -e`, so it falls straight through to `exec pause` and supervises
# nothing. ovsbr0-eth0 (uplink L3) and ovsbr0-svc-addr (internal L3) own that
# work now. Leaving it enabled is a false green in `sv status`.
if [ -L "$RUNSVDIR/ovsbr0-addr" ] || [ -d "$RUNSVDIR/ovsbr0-addr" ]; then
    warn "retiring ovsbr0-addr (dangling: ovsbr0-addr-up helper does not exist)"
    run sv stop ovsbr0-addr || true
    run rm -f "$RUNSVDIR/ovsbr0-addr"
fi

# :8090 is the only externally published NetMaker broker door. The bridge
# connects `/mqtt` directly to broker.sock, so the old 8083 TCP and secondary
# UDS relays would bypass the demux and advertise two conflicting mechanisms.
for retired in uds-netmaker-broker xsock-netmaker-broker fwd-nm-broker-8083 fwd-nm-mesh-8083; do
    if [ -L "$RUNSVDIR/$retired" ] || [ -d "$RUNSVDIR/$retired" ]; then
        warn "retiring $retired (broker WebSocket now uses op-grpc-bridge :8090/mqtt)"
        run sv stop "$retired" || true
        run rm -f "$RUNSVDIR/$retired"
    fi
done

# ── 4. Enable + start ─────────────────────────────────────────────────────────
log "Enabling services"
for s in "${NET_SERVICES[@]}" "${MESH_SERVICES[@]}" "${SOCK_SERVICES[@]}"; do
    [ -d "$SV/$s" ] || continue
    [ -e "$RUNSVDIR/$s" ] || run ln -s "$SV/$s" "$RUNSVDIR/$s"
done

if $DO_START; then
    log "Starting the network chain in dependency order"
    for s in "${NET_SERVICES[@]}"; do
        [ -d "$SV/$s" ] || continue
        start_service "$s"
        if ! $DRY_RUN; then
            i=0
            ready=true
            until sv check "$s" >/dev/null 2>&1; do
                i=$((i + 1))
                # A service can sit here indefinitely: these run scripts poll
                # `sv check` on their own dependencies, so one unsatisfiable
                # dependency deep in the chain stalls everything above it while
                # `sv status` still shows a live pid. Not ready is a failure —
                # the steps below assume the datapath this was supposed to build.
                [ "$i" -ge 60 ] && { ready=false; break; }
                sleep 1
            done
            if $ready; then
                log "  $s up"
            else
                fail "$s never passed its check (60s) — it is probably blocked in
       a wait_dep loop on a dependency that will not come ready. Look at
       'pstree -alp \$(cat $SV/$s/supervise/pid)' to see which one."
            fi
        else
            log "  $s up"
        fi
    done
    log "Starting mesh + netclient attach"
    for s in "${MESH_SERVICES[@]}"; do
        [ -d "$SV/$s" ] || continue
        start_service "$s"
    done
    log "Starting container socket relays"
    for s in "${SOCK_SERVICES[@]}"; do
        [ -d "$SV/$s" ] || continue
        start_service "$s"
    done
else
    log "--no-start: services installed and enabled, not started"
fi

# ── 5. OpenFlow ───────────────────────────────────────────────────────────────
# The file was staged before the start loop, so a controller started above
# already read it. If op-of-controller was ALREADY running when this script
# began, it is running on whatever it loaded at its own start — restart it so
# the staged set is the live set. Safe: fail_mode=standalone plus the cookied
# priority=0 fallback means the datapath keeps forwarding while it cycles.
if $DO_START; then
    if [ -f "$STATIC_FLOWS" ]; then
        log "Restarting op-of-controller so it reloads $STATIC_FLOWS"
        run sv restart op-of-controller || warn "op-of-controller restart failed"
        $DRY_RUN || sleep 2
    fi

    log "Attaching OpenFlow controller $OF_CONTROLLER_ENDPOINT to $BRIDGE"
    # attach-controller-safe waits for the plugin tree and rolls back on failure.
    run "$LIBEXEC/attach-controller-safe" "$BRIDGE" "$OF_CONTROLLER_ENDPOINT" \
        || fail "controller attach failed — OVS will never dial the controller,
       so the static flows it loaded are never pushed and the table keeps only
       whatever fail_mode=$FAIL_MODE gives it."
    # The priority=0 NORMAL fallback is what keeps the datapath forwarding if
    # the controller is not connected. Idempotent, cookied. Note this is NOT the
    # same rule as the cookie=0x0 priority=0 NORMAL that OVS installs itself in
    # standalone mode — if the table has only that one, this step did not run.
    run zcall openflow ensure_fallback_normal -a "{\"bridge\":\"$BRIDGE\"}" \
        || fail "ensure_fallback_normal failed — no cookied fallback flow"
    # Reassert fail_mode last: attaching a controller is what makes it matter.
    run zcall openflow set_fail_mode -a "{\"bridge\":\"$BRIDGE\",\"mode\":\"$FAIL_MODE\"}" \
        || warn "set_fail_mode failed — bridge keeps its current fail mode"

    # Confirm the controller actually loaded them. "Loaded 0 static flow(s)"
    # here is the failure this whole section exists to make loud — it is what
    # ran unnoticed from 2026-08-10 22:17 onward.
    if ! $DRY_RUN && [ -r "$OF_LOG/current" ]; then
        loaded="$(sed 's/\x1b\[[0-9;]*m//g' "$OF_LOG/current" 2>/dev/null \
            | grep -oE 'Loaded [0-9]+ static flow' | tail -1 | grep -oE '[0-9]+' || true)"
        if [ -n "$loaded" ]; then
            if [ "$loaded" -gt 0 ]; then
                log "  controller loaded $loaded static flow(s)"
            else
                fail "controller loaded 0 static flow(s) — the table is fallback-only"
            fi
        else
            warn "  no 'Loaded N static flow(s)' line yet in $OF_LOG/current"
        fi
    fi
fi

# ── 6. Containers: socket-only convergence ────────────────────────────────────
# Every container's entire network path is the /run/ghostbridge bind mount.
# There is no NIC and no proxy device on any of them, including xray — public
# TLS arrives on the host's pub0 and is relayed in over the shared socket.
GHOSTBRIDGE_SRC=/run/ghostbridge
GHOSTBRIDGE_PATH=/opt/run-mounts/ghostbridge
GHOSTBRIDGE_DEV=ghostbridge-socket

converge_containers() {
    command -v incus >/dev/null || { warn "incus not found — skipping container convergence"; return 0; }
    [ -d "$GHOSTBRIDGE_SRC" ] || warn "$GHOSTBRIDGE_SRC does not exist yet (opdbus-rundirs creates it)"

    local names
    names="$(incus list -c n --format csv 2>/dev/null || true)"
    [ -n "$names" ] || { warn "no containers listed — skipping"; return 0; }

    local ct devs legacy
    while read -r ct; do
        [ -n "$ct" ] || continue
        devs="$(incus config device show "$ct" 2>/dev/null || true)"

        # a. the shared socket mount
        if echo "$devs" | grep -q "^${GHOSTBRIDGE_DEV}:"; then
            log "  $ct: $GHOSTBRIDGE_DEV present"
        else
            warn "  $ct: missing $GHOSTBRIDGE_DEV — adding"
            run zcall incus add_device -a "$(printf '{"instance_name":"%s","device_name":"%s","device":{"type":"disk","source":"%s","path":"%s"}}' \
                "$ct" "$GHOSTBRIDGE_DEV" "$GHOSTBRIDGE_SRC" "$GHOSTBRIDGE_PATH")" \
                || warn "  $ct: add_device failed"
        fi

        # b. anything that is not a socket — a NIC or a per-port TCP proxy.
        # These are what the shared-socket model exists to remove. Removing one
        # from a running container cuts whatever is using it, so it is opt-in.
        legacy="$(echo "$devs" | awk '
            /^[a-zA-Z0-9_.-]+:$/ { name = substr($1, 1, length($1)-1) }
            /^ *type: (nic|proxy)$/ { print name, $2 }
        ')"
        if [ -n "$legacy" ]; then
            while read -r dev kind; do
                [ -n "$dev" ] || continue
                if $STRIP_LEGACY; then
                    warn "  $ct: removing $kind device '$dev'"
                    run zcall incus remove_device -a "$(printf '{"instance_name":"%s","device_name":"%s"}' "$ct" "$dev")" \
                        || warn "  $ct: remove_device $dev failed"
                else
                    warn "  $ct: $kind device '$dev' violates socket-only model (re-run with --strip-legacy-devices to remove)"
                fi
            done <<<"$legacy"
        fi
    done <<<"$names"
}

if $DO_CONTAINERS; then
    log "Converging containers onto the shared-socket model"
    converge_containers
else
    log "--skip-containers: containers untouched"
fi

# ── Verification ──────────────────────────────────────────────────────────────
$DRY_RUN && { log "dry-run complete — nothing was changed"; exit 0; }

log ""
log "=== Verification ==="
log "Bridges:"
zcall rovs_commands list_bridges 2>/dev/null || warn "  list_bridges failed"
log "Ports on $BRIDGE:"
zcall rovs_commands list_ports -a "{\"bridge_name\":\"$BRIDGE\"}" 2>/dev/null || warn "  list_ports failed"
# Datapath readback goes through openflow.get_datapath_health — the only read
# path there is. It reports `controllers` (proving the attach landed) and
# `fallback_normal` (the cookie 0x…01 rule), both straight off OVSDB.
#
# ovs-ofctl is forbidden, so the MANAGED (0x…02) static set cannot be counted
# here: the openflow plugin exposes no flow-dump read method. Its presence is
# inferred from the controller being connected plus the "Loaded N static
# flow(s)" readback above, which is weaker than counting the rules. See
# SIGNALS.md — the plugin needs a dump_flows read method to close this.
log "Datapath health:"
health="$("$LIBEXEC/get-datapath-health" "$BRIDGE" 2>/dev/null || true)"
if [ -z "$health" ]; then
    fail "get_datapath_health returned nothing — cannot verify the datapath"
else
    echo "$health" | sed 's/^/    /'
    case "$health" in
        *'"controllers":[]'*|*'"controllers": []'*)
            fail "no controller on $BRIDGE — OVS is not dialing $OF_CONTROLLER_ENDPOINT,
       so op-of-controller never pushes its static flows" ;;
    esac
    case "$health" in
        *'"fallback_normal":true'*|*'"fallback_normal": true'*)
            log "    fallback NORMAL (cookie 0x…01) present" ;;
        *)
            fail "fallback NORMAL (cookie 0x…01) missing — if the table still
       forwards it is on OVS's own cookie=0x0 standalone rule, not ours" ;;
    esac
fi
log "Kernel L3:"
for dev in "$BRIDGE" "$PUBLIC_PORT" $INTERNAL_PORTS; do
    ip -4 -o addr show dev "$dev" scope global 2>/dev/null | awk '{print "    " $2 " " $4}'
done
ip -4 route show default | sed 's/^/    default: /'

# The cutover is the one step whose failure looks like success: if ovsbr0-eth0
# never ran, eth0 keeps its address and the box stays reachable, so nothing
# complains — but the bridge has no uplink and pub0 is a dark port.
if ! zcall rovs_commands list_ports -a "{\"bridge_name\":\"$BRIDGE\"}" 2>/dev/null \
    | grep -qw "$UPLINK_PHYS"; then
    fail "$UPLINK_PHYS is not a port on $BRIDGE — the ovsbr0-eth0 cutover never
       ran. Public L3 is still on the raw NIC, $PUBLIC_PORT is unused."
elif ! ip -4 -o addr show dev "$PUBLIC_PORT" scope global 2>/dev/null | grep -q .; then
    fail "$UPLINK_PHYS is enslaved but $PUBLIC_PORT has no address — the cutover
       half-applied. This is the state that takes the host off the network."
fi

log ""
log "=== Summary ==="
log "  Bridge:       $BRIDGE (fail_mode=$FAIL_MODE datapath=$DATAPATH_TYPE)"
log "  Uplink:       $UPLINK_PHYS -> enslaved last, L3 on $PUBLIC_PORT ($PUBLIC_ADDR)"
log "  Internal:     $INTERNAL_PORTS"
log "  Controller:   $OF_CONTROLLER_ENDPOINT (static flows: $STATIC_FLOWS)"
log "  Containers:   socket-only via $GHOSTBRIDGE_SRC -> $GHOSTBRIDGE_PATH"
log "  Backup:       $BACKUP"
log ""
log "Verify reachability: ping -c1 -W2 $PUBLIC_GW"

if [ ${#SYMLINKED_SERVICES[@]} -gt 0 ]; then
    log ""
    warn "Service dirs installed through a symlink (finish the retirement or"
    warn "replace the link with a real directory):"
    for s in "${SYMLINKED_SERVICES[@]}"; do warn "    $s"; done
fi

if [ ${#FAILURES[@]} -gt 0 ]; then
    log ""
    err "=== ${#FAILURES[@]} step(s) did not complete ==="
    for f in "${FAILURES[@]}"; do err "  - $f"; done
    err ""
    err "Files are installed and the backup is at $BACKUP, but the network is"
    err "not in the state this script describes. Do not treat this run as done."
    exit 1
fi

log ""
log "All steps completed."
