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
#     2     svc0    10.0.0.1/24 + 10.0.0.2/24  entrance / tonic helpers
#     3     grpc0   10.200.0.2/24   gRPC plane (op-grpc-bridge :8090)
#     4     eth0    physical uplink, enslaved LAST, no L3 of its own
#   controller  tcp:10.200.0.1:6653 (op-of-controller), one flow:
#     table=0 priority=0 actions=NORMAL cookie=0x3344434800000001
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
#   5. Attaches the OpenFlow controller and installs the static flow set from
#      deploy/config/openflow-static-flows.json.
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
#     --dry-run                 print what would change, touch nothing
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
BACKUP="/var/lib/op-dbus/network-install-backup-$(date +%Y%m%d-%H%M%S)"

CAPTURE=false
DO_START=true
DO_CONTAINERS=true
STRIP_LEGACY=false
DRY_RUN=false

for arg in "$@"; do
    case "$arg" in
        --capture)              CAPTURE=true ;;
        --no-start)             DO_START=false ;;
        --skip-containers)      DO_CONTAINERS=false ;;
        --strip-legacy-devices) STRIP_LEGACY=true ;;
        --dry-run|--print)      DRY_RUN=true ;;
        -h|--help)              sed -n '2,50p' "${BASH_SOURCE[0]}"; exit 0 ;;
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
INTERNAL_PORTS="svc0 grpc0"
GRPC_PORT=grpc0
GRPC_ADDR=10.200.0.2/24
BRIDGE_ADDR=10.200.0.1/24
BRIDGE_SVC_ADDRS="10.0.0.2/24@svc0 10.0.0.1/24@svc0"
OF_CONTROLLER_LISTEN=10.200.0.1:6653
OF_CONTROLLER_ENDPOINT=tcp:10.200.0.1:6653
NETMAKER_NET=100.69.0.0/16
NETMAKER_CT=netmaker
NETMAKER_INCUS_NAME=NetMaker
NETMAKER_EGRESS_NET=10.0.0.0/24
NETMAKER_EGRESS_DEV=svc0

capture_live() {
    log "Capturing live network state..."
    local a
    a="$(ip -4 -o addr show dev "$PUBLIC_PORT" scope global 2>/dev/null | awk '{print $4; exit}')"
    [ -n "$a" ] && PUBLIC_ADDR="$a"
    a="$(ip -4 route show default 2>/dev/null | awk '/dev '"$PUBLIC_PORT"'/ {print $3; exit}')"
    [ -n "$a" ] && PUBLIC_GW="$a"
    a="$(ip -4 -o addr show dev "$GRPC_PORT" scope global 2>/dev/null | awk '{print $4; exit}')"
    [ -n "$a" ] && GRPC_ADDR="$a"
    a="$(ip -4 -o addr show dev "$BRIDGE" scope global 2>/dev/null | awk '{print $4; exit}')"
    [ -n "$a" ] && BRIDGE_ADDR="$a"
    local svc=""
    while read -r addr; do
        [ -n "$addr" ] && svc="$svc $addr@svc0"
    done < <(ip -4 -o addr show dev svc0 scope global 2>/dev/null | awk '{print $4}')
    [ -n "$svc" ] && BRIDGE_SVC_ADDRS="${svc# }"
    log "  public  $PUBLIC_ADDR via $PUBLIC_GW on $PUBLIC_PORT"
    log "  grpc    $GRPC_ADDR on $GRPC_PORT"
    log "  fabric  $BRIDGE_ADDR on $BRIDGE"
    log "  svc     $BRIDGE_SVC_ADDRS"
}

# ── Preflight ─────────────────────────────────────────────────────────────────
[[ $EUID -eq 0 ]] || $DRY_RUN || die "must run as root"
[ -d "$REPO/deploy/runit" ] || die "$REPO/deploy/runit missing — run from the repo checkout"
command -v sv >/dev/null || die "sv not found — this host is not running runit"
command -v zcall >/dev/null || die "zcall not found — install the operator CLI"
[ -d "/sys/class/net/$UPLINK_PHYS" ] || die "uplink $UPLINK_PHYS does not exist"

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
    echo "  would write $NET_CONF (BRIDGE=$BRIDGE PUBLIC_ADDR=$PUBLIC_ADDR GRPC_ADDR=$GRPC_ADDR)"
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
#   - svc0: entrance (10.0.0.1/10.0.0.2) — tonic helpers (mail, Netmaker)
#   - grpc0: gRPC plane (10.200.0.2:8090) — op-grpc-bridge + cognitive MCP
#   - ovsbr0 LOCAL: fabric address, also the OpenFlow controller address
#   - every container is socket-only: /run/ghostbridge bind mount, no NIC,
#     no incus proxy devices. The WG iface "netmaker" must NOT join ovsbr0.
#   - gRPC port is :8090 — never :50051

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
GRPC_PORT=$GRPC_PORT
GRPC_ADDR=$GRPC_ADDR

BRIDGE_ADDR=$BRIDGE_ADDR
BRIDGE_SVC_ADDRS="$BRIDGE_SVC_ADDRS"

# OpenFlow: OVS dials the controller, never the other way around.
OF_CONTROLLER_LISTEN=$OF_CONTROLLER_LISTEN
OPENFLOW_CONTROLLER=$OF_CONTROLLER_ENDPOINT
OF_STATIC_FLOWS_FILE=$STATIC_FLOWS
OF_FLOW_PAIRS=

NETMAKER_NET=$NETMAKER_NET
NETMAKER_CT=$NETMAKER_CT
NETMAKER_INCUS_NAME=$NETMAKER_INCUS_NAME
NETMAKER_EGRESS_NET=$NETMAKER_EGRESS_NET
NETMAKER_EGRESS_DEV=$NETMAKER_EGRESS_DEV
EOF
    chmod 0644 "$NET_CONF"
fi

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
# Container socket relays — the only network path any container has.
SOCK_SERVICES=(
    xsock-web
    xsock-qdrant
    xsock-netmaker
    xsock-netmaker-broker
    xsock-decoy
    uds-assistant
    uds-cozo-chat
    uds-netmaker-api
    uds-netmaker-broker
    uds-qdrant-grpc
    uds-qdrant-http
    uds-qdrant-http-svc0
    uds-xray-reality
)

install_service() {
    local name="$1" src="$REPO/deploy/runit/$1"
    [ -d "$src" ] || { warn "no service definition for $name in repo — skipping"; return 1; }
    run install -d -m 0755 "$SV/$name"
    local f
    for f in run finish check conf; do
        [ -f "$src/$f" ] && run install -m 0755 "$src/$f" "$SV/$name/$f"
    done
    if [ -d "$src/log" ]; then
        run install -d -m 0755 "$SV/$name/log"
        [ -f "$src/log/run" ] && run install -m 0755 "$src/log/run" "$SV/$name/log/run"
    fi
    return 0
}

log "Installing network services"
for s in "${NET_SERVICES[@]}"; do install_service "$s" && log "  $s"; done
log "Installing container socket relays"
for s in "${SOCK_SERVICES[@]}"; do install_service "$s" && log "  $s"; done

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

# ── 4. Enable + start ─────────────────────────────────────────────────────────
log "Enabling services"
for s in "${NET_SERVICES[@]}" "${SOCK_SERVICES[@]}"; do
    [ -d "$SV/$s" ] || continue
    [ -e "$RUNSVDIR/$s" ] || run ln -s "$SV/$s" "$RUNSVDIR/$s"
done

if $DO_START; then
    log "Starting the network chain in dependency order"
    for s in "${NET_SERVICES[@]}"; do
        [ -d "$SV/$s" ] || continue
        run sv start "$s" || warn "$s did not start"
        if ! $DRY_RUN; then
            i=0
            until sv check "$s" >/dev/null 2>&1; do
                i=$((i + 1))
                [ "$i" -ge 60 ] && { warn "$s not ready after 60s — continuing"; break; }
                sleep 1
            done
        fi
        log "  $s up"
    done
    log "Starting container socket relays"
    for s in "${SOCK_SERVICES[@]}"; do
        [ -d "$SV/$s" ] || continue
        run sv start "$s" || warn "$s did not start"
    done
else
    log "--no-start: services installed and enabled, not started"
fi

# ── 5. OpenFlow ───────────────────────────────────────────────────────────────
log "Installing static flows -> $STATIC_FLOWS"
if [ -f "$REPO/deploy/config/openflow-static-flows.json" ]; then
    run install -m 0644 "$REPO/deploy/config/openflow-static-flows.json" "$STATIC_FLOWS"
else
    warn "deploy/config/openflow-static-flows.json missing — controller will run with no static flows"
fi

if $DO_START; then
    log "Attaching OpenFlow controller $OF_CONTROLLER_ENDPOINT to $BRIDGE"
    # attach-controller-safe waits for the plugin tree and rolls back on failure.
    run "$LIBEXEC/attach-controller-safe" "$BRIDGE" "$OF_CONTROLLER_ENDPOINT" \
        || warn "controller attach failed — bridge left on fail_mode=$FAIL_MODE"
    # The priority=0 NORMAL fallback is what keeps the datapath forwarding if
    # the controller is not connected. Idempotent, cookied.
    run zcall openflow ensure_fallback_normal -a "{\"bridge\":\"$BRIDGE\"}" \
        || warn "ensure_fallback_normal failed"
    # Reassert fail_mode last: attaching a controller is what makes it matter.
    run zcall openflow set_fail_mode -a "{\"bridge\":\"$BRIDGE\",\"mode\":\"$FAIL_MODE\"}" \
        || warn "set_fail_mode failed — bridge keeps its current fail mode"
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
log "Datapath health:"
"$LIBEXEC/get-datapath-health" "$BRIDGE" 2>/dev/null || warn "  health check failed"
log "Kernel L3:"
for dev in "$BRIDGE" "$PUBLIC_PORT" $INTERNAL_PORTS; do
    ip -4 -o addr show dev "$dev" scope global 2>/dev/null | awk '{print "    " $2 " " $4}'
done
ip -4 route show default | sed 's/^/    default: /'

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
