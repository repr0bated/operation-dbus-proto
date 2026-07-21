#!/usr/bin/env bash
#
# 3tched-network-bootstrap.sh — focused network boot unit (runit + OVS + netmaker)
#
# Network-only install for Artix runit. Does NOT run the comprehensive
# installer and does NOT build the full Rust workspace. If op-ovsbr0-setup is
# missing it builds only the op-network binaries; if network.conf is missing
# it seeds a minimal runit-oriented defaults file.
#
# This script touches ONLY the network boot path:
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
#   2. Regenerates the network runit services around that command:
#        uplink-dhcp -> ovsdb-server -> ovs-vswitchd (seed) -> ovsbr0-uplink
#        -> incusd -> incus-ct-netmaker -> ovsbr0-svc-addr
#        -> op-of-controller / opdbus
#   3. Retires the backwards oneshots (ovsbr0-addr, ovsbr0-uplink-addr):
#      the host uplink now completes BEFORE netmaker, service addresses
#      strictly AFTER netmaker.
#   4. Strips any network dependency from op-session-bus / op-grpc-bridge so
#      the D-Bus session bus and the gRPC bridge (UDS + loopback :8090) load
#      early, in parallel with the network chain.
#   5. Enables the touched services via symlink into
#      /etc/runit/runsvdir/default and starts them with `sv`.  That IS the
#      live cutover of the touched subgraph.  Run it only when you mean to
#      cut over now, ideally with console access for recovery.
#
# Rollback contract: before any change, /etc/runit/sv, network.conf and the
# old 3tched-bridge-up are copied to
#   /var/lib/op-dbus/network-bootstrap-backup-<timestamp>/
# Restore (console): cp -a <backup>/sv/. /etc/runit/sv/, re-link retired
# services if needed, then `sv restart` the affected names.

set -euo pipefail

SV=/etc/runit/sv
RUNSVDIR=/etc/runit/runsvdir/default
NET_CONF=/etc/op-dbus/network.conf
BIN=/usr/local/bin
LIBEXEC=/usr/local/libexec/3tched
LOG_ROOT=/var/log/op-dbus
BACKUP="/var/lib/op-dbus/network-bootstrap-backup-$(date +%Y%m%d-%H%M%S)"

SCRIPT_PATH=$(readlink -f "${BASH_SOURCE[0]}" 2>/dev/null || printf '%s' "$0")
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$SCRIPT_PATH")" && pwd)
if [[ -f $SCRIPT_DIR/Cargo.toml ]]; then
    REPO_ROOT=$SCRIPT_DIR
elif [[ -f $SCRIPT_DIR/../Cargo.toml ]]; then
    REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
else
    REPO_ROOT=${OP_DBUS_ROOT:-}
fi

BUILD_USER=${BUILD_USER:-$(logname 2>/dev/null || printf '%s' "${SUDO_USER:-${USER:-root}}")}

declare -a TOUCHED_SERVICES=()
declare -a RETIRED_SERVICES=()

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
log()  { echo -e "${BLUE}[net-bootstrap]${NC} $*"; }
ok()   { echo -e "${GREEN}[  ok  ]${NC} $*"; }
warn() { echo -e "${YELLOW}[ warn ]${NC} $*"; }
die()  { echo -e "${RED}[ fail ]${NC} $*" >&2; exit 1; }

# ============================================================================
# PREFLIGHT + NETWORK-ONLY BINARIES + BACKUP
# ============================================================================

seed_network_conf() {
    [[ -r $NET_CONF ]] && return 0
    log "Seeding $NET_CONF (was missing) ..."
    install -d -m 0755 /etc/op-dbus
    cat > "$NET_CONF" <<'EOF'
# 3tched network configuration — consumed by the runit network services and the
# op-network binaries (op-ovsbr0-setup / op-of-controller).

BRIDGE=ovsbr0
BRIDGE_ADDR=10.200.0.1/24
BRIDGE_NET=10.200.0.0/24
FAIL_MODE=standalone
OVSDB_SOCKET=/run/openvswitch/db.sock
VSWITCHD_SVC=/run/runit/service/ovs-vswitchd
UPLINK=eth0
NAT_ENABLE=1
NETMAKER_NET=10.0.0.0/24
EOF
    chmod 0640 "$NET_CONF"
    ok "seeded $NET_CONF"
}

ensure_network_bins() {
    local need=()
    [[ -x ${BIN}/op-ovsbr0-setup ]] || need+=(op-ovsbr0-setup)
    [[ -x ${BIN}/op-of-controller ]] || need+=(op-of-controller)

    ((${#need[@]})) || return 0

    [[ -n $REPO_ROOT && -f $REPO_ROOT/Cargo.toml ]] || \
        die "missing ${need[*]} under $BIN — set OP_DBUS_ROOT to the repo (or place this script in it) so network-only cargo build can run"

    command -v cargo >/dev/null 2>&1 || die "cargo not found — install rust or place ${need[*]} in $BIN"
    command -v sudo >/dev/null 2>&1 || die "sudo not found (needed to build as $BUILD_USER)"

    log "Building network-only bins as $BUILD_USER: ${need[*]}"
    (
        cd "$REPO_ROOT"
        local args=(-p op-network --release)
        local b
        for b in "${need[@]}"; do
            args+=(--bin "$b")
        done
        sudo -u "$BUILD_USER" cargo build "${args[@]}"
    )
    local b
    for b in "${need[@]}"; do
        [[ -x $REPO_ROOT/target/release/$b ]] || die "build did not produce target/release/$b"
        install -Dm755 "$REPO_ROOT/target/release/$b" "$BIN/$b"
        ok "installed $BIN/$b"
    done
}

preflight() {
    [[ $EUID -eq 0 ]] || die "run as root (sudo)"
    [[ -d $SV ]] || die "$SV missing — not a runit system"
    command -v sv >/dev/null 2>&1 || die "sv not found — host is not runit"
    [[ -d $RUNSVDIR ]] || die "$RUNSVDIR missing — Artix runit runsvdir not present"
    seed_network_conf
    ensure_network_bins
    [[ -x ${BIN}/op-ovsbr0-setup ]] || die "${BIN}/op-ovsbr0-setup missing after network-only build"
    [[ -r $NET_CONF ]] || die "$NET_CONF missing"
    install -d -m 0755 "$LIBEXEC"
    install -d -m 0755 -o root -g root "$LOG_ROOT" 2>/dev/null || install -d -m 0755 "$LOG_ROOT"
    log "Preflight OK (runit network-only)"
}

backup() {
    log "Backing up to $BACKUP ..."
    install -d -m 0700 "$BACKUP"
    cp -a "$SV" "$BACKUP/sv"
    [[ -r $NET_CONF ]] && cp -a "$NET_CONF" "$BACKUP/network.conf"
    [[ -f ${BIN}/3tched-bridge-up ]] && cp -a "${BIN}/3tched-bridge-up" "$BACKUP/"
    ok "Backup complete (restore: cp -a $BACKUP/sv/. $SV/ && re-link + sv restart; see summary)"
}

# ============================================================================
# NETWORK.CONF — post-netmaker service addresses + runit supervise path
# ============================================================================

update_net_conf() {
    log "Updating $NET_CONF ..."
    if grep -q '^VSWITCHD_SVC=' "$NET_CONF"; then
        sed -i 's|^VSWITCHD_SVC=.*|VSWITCHD_SVC=/run/runit/service/ovs-vswitchd|' "$NET_CONF"
    else
        echo 'VSWITCHD_SVC=/run/runit/service/ovs-vswitchd' >> "$NET_CONF"
    fi
    if ! grep -q '^BRIDGE_SVC_ADDRS=' "$NET_CONF"; then
        cat >> "$NET_CONF" <<'EOF'

# Host service addresses, applied to the bridge ONLY AFTER the netmaker
# container is ready (ovsbr0-svc-addr):
#   10.200.0.1/24 — control plane bind (gRPC :50051, OpenFlow :6653)
#   10.0.0.2/24    — host side of the netmaker egress path
#   10.200.0.2/24  — netmaker mesh bridge address
# Applying these before the netmaker veth exists races the container's own
# bring-up.
BRIDGE_SVC_ADDRS="10.200.0.1/24 10.0.0.2/24 10.200.0.2/24"

# Container whose readiness gates the service addresses
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
# Ordered contract (each phase is an idempotent runit oneshot body):
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
#                  + netmaker route + NAT, strictly AFTER netmaker is ready.
#   all            full sequence for manual/recovery use.  Boot never runs
#                  this: the runit graph calls the individual phases
#                  (uplink-dhcp -> seed inside ovs-vswitchd run ->
#                  ovsbr0-uplink -> incus-ct-netmaker -> ovsbr0-svc-addr).
#                  Supervised pieces are started through `sv` only.
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
    if ! ip -4 -o addr show dev "$UPLINK" scope global | grep -q .; then
        command -v dhcpcd >/dev/null 2>&1 || { echo "dhcpcd is required for $UPLINK"; exit 1; }
        echo "acquiring IPv4 DHCP lease on $UPLINK before OVS attachment"
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
    phase_netmaker_wait
    for a in ${BRIDGE_SVC_ADDRS:-10.200.0.1/24 10.0.0.2/24 10.200.0.2/24}; do
        ip addr replace "$a" dev "$BRIDGE"
    done
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
        sv start ovs-vswitchd || { echo "sv start ovs-vswitchd failed"; exit 1; }
    fi
    phase_uplink
    if [ -d "/etc/runit/sv/incus-ct-${NETMAKER_CT}" ]; then
        sv start "incus-ct-${NETMAKER_CT}" || true
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
# RUNIT HELPERS
# ============================================================================

mk_logger() {
    local svc="$1"
    local dir="${SV}/${svc}/log"
    install -d "$dir"
    cat > "${dir}/run" <<EOF
#!/bin/sh
exec 2>&1
install -d -o s6log -g s6log ${LOG_ROOT}/${svc}
echo "s4000000" > ${LOG_ROOT}/${svc}/config
echo "n10" >> ${LOG_ROOT}/${svc}/config
exec chpst -u s6log svlogd ${LOG_ROOT}/${svc}
EOF
    chmod 0755 "${dir}/run"
}

# Wait for deps via `sv start` + `sv check`, then run body forever (longrun)
# or oneshot-then-pause.
write_run_with_deps() {
    # write_run_with_deps <svc> <deps-space-sep> <<'BODY' ... BODY
    local svc="$1" deps="$2"
    local dir="${SV}/${svc}"
    local body
    body="$(cat)"
    install -d "$dir"
    {
        echo '#!/bin/sh'
        echo 'exec 2>&1'
        echo
        if [[ -n $deps ]]; then
            local d
            for d in $deps; do
                [[ -d ${SV}/$d ]] || { warn "$svc: skipping missing dependency $d"; continue; }
                cat <<DEP
sv start "$d" >/dev/null 2>&1 || true
until sv check "$d" >/dev/null 2>&1; do sleep 1; done
DEP
            done
            echo
        fi
        # strip shebang from body if present
        printf '%s\n' "$body" | sed '/^#!\/bin\/sh$/d; /^#!\/usr\/bin\/env bash$/d'
    } > "${dir}/run"
    chmod 0755 "${dir}/run"
    TOUCHED_SERVICES+=("$svc")
}

mk_oneshot() {
    # mk_oneshot <name> <deps> <up-cmd...>
    local svc="$1" deps="$2"; shift 2
    local up="$*"
    local dir="${SV}/${svc}"
    install -d "$dir"
    {
        echo '#!/bin/sh'
        echo 'exec 2>&1'
        echo
        if [[ -n $deps ]]; then
            local d
            for d in $deps; do
                [[ -d ${SV}/$d ]] || { warn "$svc: skipping missing dependency $d"; continue; }
                cat <<DEP
sv start "$d" >/dev/null 2>&1 || true
until sv check "$d" >/dev/null 2>&1; do sleep 1; done
DEP
            done
            echo
        fi
        cat <<EOF
# oneshot UP
$up

# stay up so dependents can sv check us
exec pause || exec sleep infinity
EOF
    } > "${dir}/run"
    chmod 0755 "${dir}/run"
    TOUCHED_SERVICES+=("$svc")
}

# Rewrite an existing longrun's dep wait prefix (keeps rest of run if possible
# is hard — callers pass full body). Used to strip/add network waits.
set_longrun_deps() {
    # set_longrun_deps <svc> <deps> — only adjusts the wait prefix of an
    # existing run by regenerating waits; body after first non-wait line kept
    # is fragile, so callers that need body changes rewrite run entirely.
    :
}

strip_network_waits() {
    # Remove sv start/check loops for network services from early-boot
    # longruns so they do not wait on the network chain.
    local svc="$1"
    local run="${SV}/${svc}/run"
    [[ -f $run ]] || return 0
    local tmp
    tmp="$(mktemp)"
    awk '
        BEGIN { skip=0 }
        /^sv start "(uplink-dhcp|ovsdb-server|ovs-vswitchd|ovsbr0-uplink|ovsbr0-svc-addr|ovsbr0-addr|ovsbr0-uplink-addr|incusd|op-of-controller|incus-ct-)/ {
            skip=1; next
        }
        skip && /^until sv check / { skip=0; next }
        skip { next }
        { print }
    ' "$run" > "$tmp"
    mv -f "$tmp" "$run"
    chmod 0755 "$run"
    TOUCHED_SERVICES+=("$svc")
}

retire_service() {
    local svc="$1"
    if [[ -L ${RUNSVDIR}/${svc} || -e ${RUNSVDIR}/${svc} ]]; then
        sv stop "$svc" 2>/dev/null || true
        rm -f "${RUNSVDIR}/${svc}"
    fi
    if [[ -d ${SV}/${svc} ]]; then
        rm -rf "${SV:?}/${svc}"
        ok "retired runit service: $svc"
    fi
    RETIRED_SERVICES+=("$svc")
}

# ============================================================================
# RUNIT GRAPH — corrected network boot order
# ============================================================================

write_network_services() {
    log "Rewriting the network runit graph ..."

    retire_service ovsbr0-addr
    retire_service ovsbr0-uplink-addr

    # -- uplink-dhcp --------------------------------------------------------
    mk_oneshot uplink-dhcp "opdbus-rundirs" "${BIN}/3tched-bridge-up dhcp"

    # -- ovs-vswitchd: seed then exec ---------------------------------------
    if [[ -d ${SV}/ovs-vswitchd ]]; then
        write_run_with_deps ovs-vswitchd "ovsdb-server uplink-dhcp" <<'EOF'
set -a; [ -r /etc/op-dbus/network.conf ] && . /etc/op-dbus/network.conf; set +a
export OVSDB_SOCKET="${OVSDB_SOCKET:-/run/openvswitch/db.sock}"
/usr/local/bin/3tched-bridge-up seed || echo "bridge seed failed (continuing)"
exec ovs-vswitchd "unix:${OVSDB_SOCKET}"
EOF
        mk_logger ovs-vswitchd
    else
        warn "no ovs-vswitchd service in $SV — seed phase not wired"
    fi

    # ensure ovsdb-server waits on uplink-dhcp
    if [[ -d ${SV}/ovsdb-server ]]; then
        write_run_with_deps ovsdb-server "uplink-dhcp" <<'EOF'
DB=/etc/openvswitch/conf.db
SCHEMA=/usr/share/openvswitch/vswitch.ovsschema
install -d /etc/openvswitch /run/openvswitch
[ -f "$DB" ] || ovsdb-tool create "$DB" "$SCHEMA"
exec ovsdb-server "$DB" \
    --remote=punix:/run/openvswitch/db.sock \
    --remote=db:Open_vSwitch,Open_vSwitch,manager_options
EOF
        mk_logger ovsdb-server
    fi

    # -- ovsbr0-uplink: host uplink BEFORE containers -----------------------
    mk_oneshot ovsbr0-uplink "ovs-vswitchd" "${BIN}/3tched-bridge-up uplink"

    # -- ovsbr0-svc-addr: service addrs AFTER netmaker ----------------------
    # dep on incus-ct-netmaker added in write_netmaker_service when present
    mk_oneshot ovsbr0-svc-addr "ovsbr0-uplink" "${BIN}/3tched-bridge-up service-addrs"

    # -- rewrite dependent longruns to wait on the corrected oneshots -------
    if [[ -d ${SV}/incusd ]]; then
        write_run_with_deps incusd "ovsbr0-uplink" <<'EOF'
exec incusd --group incus-admin
EOF
        mk_logger incusd
    fi

    if [[ -d ${SV}/op-of-controller ]]; then
        write_run_with_deps op-of-controller "ovsbr0-svc-addr" <<'EOF'
set -a
[ -r /etc/op-dbus/environment ] && . /etc/op-dbus/environment
[ -r /etc/op-dbus/network.conf ] && . /etc/op-dbus/network.conf
set +a
export OF_CONTROLLER_LISTEN="${OF_CONTROLLER_LISTEN:-10.200.0.1:6653}"
export RUST_LOG="${RUST_LOG:-info}"
exec /usr/local/bin/op-of-controller
EOF
        mk_logger op-of-controller
    fi

    if [[ -d ${SV}/opdbus ]]; then
        write_run_with_deps opdbus "op-session-bus ovsbr0-svc-addr" <<'EOF'
set -a; [ -r /etc/op-dbus/environment ] && . /etc/op-dbus/environment; set +a
export RUST_LOG="${RUST_LOG:-info}"
exec /usr/local/bin/opdbus
EOF
        mk_logger opdbus
    fi

    if [[ -d ${SV}/xray ]]; then
        # xray binds public uplink IP — after uplink migration
        local xray_body
        xray_body="$(sed '1{/^#!/d;}; /^sv start /d; /^until sv check /d' "${SV}/xray/run" 2>/dev/null || true)"
        if [[ -n ${xray_body:-} ]]; then
            write_run_with_deps xray "ovsbr0-uplink" <<<"$xray_body"
            mk_logger xray
        else
            warn "xray run body empty — left untouched"
        fi
    fi

    # -- EARLY BOOT: session bus + gRPC bridge never wait on network --------
    local early
    for early in op-session-bus op-grpc-bridge; do
        strip_network_waits "$early"
    done

    ok "network runit graph rewritten"
}

# ============================================================================
# NETMAKER CONTAINER SUPERVISOR — busctl readiness
# ============================================================================

write_netmaker_service() {
    local nm bridge svc dir
    nm="$(set -a; . "$NET_CONF" 2>/dev/null; set +a; echo "${NETMAKER_CT:-netmaker}")"
    bridge="$(set -a; . "$NET_CONF" 2>/dev/null; set +a; echo "${BRIDGE:-ovsbr0}")"
    svc="incus-ct-${nm}"
    dir="${SV}/${svc}"
    if [[ ! -d $dir ]]; then
        warn "no $svc service — generate it first (3tched-incus-svcgen), then re-run this bootstrap"
        return 0
    fi
    log "Rewriting $svc supervisor (busctl readiness) ..."

    write_run_with_deps "$svc" "incusd ovsbr0-uplink" <<EOF
NAME='${nm}'
ATTACH_BRIDGE='${bridge}'
i=0
until incus info >/dev/null 2>&1; do
    i=\$((i+1))
    [ "\$i" -ge 60 ] && { echo "incusd not ready"; exit 1; }
    sleep 1
done
incus config set "\$NAME" boot.autostart=false 2>/dev/null || true
if incus config device show "\$NAME" 2>/dev/null | grep -q '^eth0:'; then
    HAS_NIC=1
else
    HAS_NIC=0
fi
if ! incus list -f csv -c ns | grep -q "^\${NAME},RUNNING"; then
    echo "starting container \$NAME"
    incus start "\$NAME"
fi
j=0
state=""
while :; do
    state="\$(incus exec "\$NAME" -- busctl --system get-property \\
        org.freedesktop.systemd1 /org/freedesktop/systemd1 \\
        org.freedesktop.systemd1.Manager SystemState 2>/dev/null |
        tr -d '"' | awk '{print \$2}')" || true
    case "\$state" in
        running|degraded) break ;;
    esac
    j=\$((j+1))
    [ "\$j" -ge 180 ] && { echo "\$NAME service manager not ready after 180s (state: \${state:-none})"; break; }
    sleep 1
done
if [ "\$HAS_NIC" = 1 ]; then
    echo "\$NAME enslaved on \$ATTACH_BRIDGE (service manager: \${state:-unknown})"
else
    echo "\$NAME ready — NIC-less (rust-network-mgr socket + host proxy device), service manager: \${state:-unknown}"
fi
# touch a ready flag so ovsbr0-svc-addr's wait (via sv check on this
# longrun) only succeeds after busctl readiness — runit has no
# notification-fd, so we stay up from here via console hold.
script -qefc "incus console \$NAME" /dev/null &
pid=\$!
trap 'incus stop "\$NAME" --timeout 30 2>/dev/null || true; kill "\$pid" 2>/dev/null || true' TERM INT
wait "\$pid"
EOF
    mk_logger "$svc"

    # service addresses wait on netmaker being up
    mk_oneshot ovsbr0-svc-addr "ovsbr0-uplink ${svc}" "${BIN}/3tched-bridge-up service-addrs"
    ok "$svc rewritten (busctl readiness)"
}

# ============================================================================
# ENABLE + START (live cutover)
# ============================================================================

enable_and_start() {
    log "Enabling + starting touched runit services ..."

    local -A seen=()
    local -a enable_list=()
    local s
    for s in "${TOUCHED_SERVICES[@]}"; do
        [[ -n ${seen[$s]:-} ]] && continue
        seen[$s]=1
        [[ -d ${SV}/$s ]] && enable_list+=("$s")
    done

    if ((${#RETIRED_SERVICES[@]})); then
        log "Retired (stopped + unlinked): ${RETIRED_SERVICES[*]}"
    fi

    ((${#enable_list[@]})) || die "no touched services recorded — nothing to enable"

    warn "the next step live-restarts every touched service (${enable_list[*]}) — real cutover"
    for s in "${enable_list[@]}"; do
        ln -sfn "${SV}/${s}" "${RUNSVDIR}/${s}"
    done

    # brief settle for runsv to pick up new links
    sleep 1

    for s in "${enable_list[@]}"; do
        sv restart "$s" || sv start "$s" || warn "sv start/restart failed for $s"
    done

    ok "runit services enabled and started — verify with: sv status <svc>"
}

summary() {
    echo
    echo "============================================================================"
    echo -e "${GREEN}NETWORK BOOTSTRAP COMMITTED (runit)${NC}"
    echo -e "${YELLOW}Touched services were linked into $RUNSVDIR and restarted — live cutover.${NC}"
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
    echo "  sv status uplink-dhcp ovsbr0-uplink ovsbr0-svc-addr"
    echo "  ip addr show ovsbr0             # uplink addr + BRIDGE_SVC_ADDRS"
    echo "  ping -c2 <default gateway>"
    echo
    echo "Rollback:"
    echo "  cp -a $BACKUP/sv/. $SV/"
    echo "  # re-link retired services if needed, then sv restart <names>"
    echo "============================================================================"
}

main() {
    preflight
    backup
    update_net_conf
    install_bridge_up
    write_network_services
    write_netmaker_service
    enable_and_start
    summary
}

main "$@"
