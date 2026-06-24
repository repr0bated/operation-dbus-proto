#!/usr/bin/env bash
# cutover-afxdp.sh - noVNC-only AF_XDP cutover with libxdp dispatcher chaining.
#
# Run from noVNC:
#   sudo /home/jeremy/git/operation-dbus-proto/deploy/cutover-afxdp.sh --novnc
#
# Required XDP chain:
#   xdp_steer (prio 50, XDP_PASS) -> OVS AF_XDP/XSK redirect
#
# This script keeps the host recoverable:
# - refuses to run without --novnc
# - loads xdp_steer through xdp-loader after AF_XDP attach
# - checks gateway reachability through ovsbr0 before flushing eth0
# - rolls back eth0 public IP/default route if any step fails
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="${REPO_DIR:-$(cd "$SCRIPT_DIR/.." && pwd)}"
OVS_SRC_DIR="${OVS_SRC_DIR:-/home/jeremy/git/ovs}"

BR="${BR:-ovsbr0}"
UPLINK="${UPLINK:-eth0}"
VETH="${VETH:-grpc-uplink}"
MGMT_ADDR="${MGMT_ADDR:-148.113.204.83}"
MGMT_PREFIX="${MGMT_PREFIX:-32}"
GW="${GW:-148.113.204.1}"
CT="${CT:-wg-xray}"
SHARED_MAC="${SHARED_MAC:-fa:16:3e:f1:71:d2}"

VSWITCHD_SVC="${VSWITCHD_SVC:-/run/service/ovs-vswitchd}"
OP_DBUS_SVC="${OP_DBUS_SVC:-/run/service/op-dbus}"
OP_XDP_WG="${OP_XDP_WG:-/usr/local/sbin/op-xdp-wg}"
OP_AFXDP="${OP_AFXDP:-/usr/local/bin/op-ovsbr0-afxdp}"
OP_SETUP="${OP_SETUP:-/usr/local/bin/op-ovsbr0-setup}"
LOG_DIR="${LOG_DIR:-/var/log/op-afxdp-cutover}"
VSWITCHD_ETC_RUN="${VSWITCHD_ETC_RUN:-/etc/s6/sv/ovs-vswitchd/run}"
OP_DBUS_ETC_RUN="${OP_DBUS_ETC_RUN:-/etc/s6/sv/op-dbus/run}"
SEED_SKIP_FLAG="${SEED_SKIP_FLAG:-/run/op-ovsbr0-skip-seed}"

SUCCESS=0
ROLLBACK_STARTED=0

log() { printf '\033[1;32m[afxdp]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[warn]\033[0m %s\n' "$*" >&2; }
die() { printf '\033[1;31m[ERROR]\033[0m %s\n' "$*" >&2; exit 1; }

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

dhcpcd_denies_bridge() {
    [ -f /etc/dhcpcd.conf ] || return 1
    awk -v br="$BR" '
        $1 == "denyinterfaces" {
            for (i = 2; i <= NF; i++) {
                if ($i == br) {
                    found = 1
                }
            }
        }
        END { exit found ? 0 : 1 }
    ' /etc/dhcpcd.conf
}

disable_bridge_dhcp_clients() {
    if [ -f /etc/dhcpcd.conf ] && ! dhcpcd_denies_bridge; then
        cat >>/etc/dhcpcd.conf <<EOF

# op-afxdp-cutover
# dhcpcd must not claim IPv4LL on the OVS bridge during AF_XDP cutover.
denyinterfaces $BR
EOF
        log "added dhcpcd denyinterfaces entry for $BR"
    fi

    if command -v dhcpcd >/dev/null 2>&1; then
        dhcpcd -x "$BR" 2>/dev/null || true
        dhcpcd -k "$BR" 2>/dev/null || true
    fi
    while read -r pid _; do
        [ -n "${pid:-}" ] || continue
        kill "$pid" 2>/dev/null || true
    done < <(pgrep -af "dhcpcd: .*${BR}" 2>/dev/null || true)
}

install_repo_binaries() {
    if [ -x "$REPO_DIR/target/release/op-ovsbr0-setup" ]; then
        install -m 755 "$REPO_DIR/target/release/op-ovsbr0-setup" /usr/local/bin/op-ovsbr0-setup
        log "installed repo op-ovsbr0-setup release binary"
    fi
    if [ -x "$REPO_DIR/target/release/op-ovsbr0-afxdp" ]; then
        install -m 755 "$REPO_DIR/target/release/op-ovsbr0-afxdp" /usr/local/bin/op-ovsbr0-afxdp
        install -m 755 "$REPO_DIR/target/release/op-ovsbr0-afxdp" /usr/local/sbin/op-ovsbr0-afxdp
        log "installed repo op-ovsbr0-afxdp release binary"
    fi
    if [ -x "$REPO_DIR/target/release/op-xdp-wg" ]; then
        install -m 755 "$REPO_DIR/target/release/op-xdp-wg" /usr/local/sbin/op-xdp-wg
        log "installed repo op-xdp-wg release binary"
    fi
    if [ -x "$OVS_SRC_DIR/vswitchd/ovs-vswitchd" ]; then
        install -m 755 "$OVS_SRC_DIR/vswitchd/ovs-vswitchd" /usr/local/sbin/ovs-vswitchd
        log "installed patched ovs-vswitchd with libxdp AF_XDP chaining"
    fi
    if [ -x "$REPO_DIR/target/release/opdbus" ]; then
        install -m 755 "$REPO_DIR/target/release/opdbus" /usr/local/bin/op-dbus
        log "installed repo op-dbus release binary"
    fi
}

clear_kernel_datapath() {
    ovs-dpctl del-dp system@ovs-system 2>/dev/null || true
    ip link del "$BR" 2>/dev/null || true
}

clear_bridge_l3() {
    [ -e "/sys/class/net/$BR" ] || return 0
    ip -4 route flush dev "$BR" 2>/dev/null || true
    ip -4 addr flush dev "$BR" 2>/dev/null || true
    ip -4 route flush dev "$BR" 2>/dev/null || true
}

remove_bridge_defaults() {
    [ -e "/sys/class/net/$BR" ] || return 0
    while read -r route; do
        [ -n "$route" ] || continue
        ip -4 route del $route dev "$BR" 2>/dev/null \
            || ip -4 route del default dev "$BR" 2>/dev/null \
            || true
    done < <(ip -4 route show default dev "$BR" 2>/dev/null)
}

remove_bridge_ipv4ll() {
    [ -e "/sys/class/net/$BR" ] || return 0
    while read -r addr; do
        [ -n "$addr" ] || continue
        case "$addr" in
            169.254.*)
                warn "removing stale IPv4LL address $addr from $BR"
                ip -4 addr del "$addr" dev "$BR" 2>/dev/null || true
                ;;
        esac
    done < <(ip -o -4 addr show dev "$BR" 2>/dev/null | awk '{print $4}')
    ip -4 route del 169.254.0.0/16 dev "$BR" 2>/dev/null || true
    remove_bridge_defaults
}

assert_no_bridge_ipv4ll() {
    [ -e "/sys/class/net/$BR" ] || return 0
    remove_bridge_ipv4ll
    if ip -o -4 addr show dev "$BR" 2>/dev/null | awk '{print $4}' | grep -q '^169\.254\.'; then
        die "$BR still has an IPv4LL 169.254 address after cleanup"
    fi
}

vswitchd_ctl() {
    ls /usr/local/var/run/openvswitch/ovs-vswitchd.*.ctl 2>/dev/null | tail -n 1
}

enable_ovs_afxdp_debug() {
    ctl="$(vswitchd_ctl || true)"
    [ -n "${ctl:-}" ] || return 0
    ovs-appctl -t "$ctl" vlog/set netdev_afxdp::dbg 2>/dev/null || true
}

restore_xdp_chain() {
    [ -x "$OP_XDP_WG" ] || return 0
    "$OP_XDP_WG" chain 2>/dev/null || true
}

stage_bridge_l3() {
    disable_bridge_dhcp_clients
    log "clearing stale IPv4 state from $BR"
    clear_bridge_l3
    ip link set "$BR" up
    ip addr add "${MGMT_ADDR}/${MGMT_PREFIX}" dev "$BR"
    ip route replace "$GW" dev "$BR" scope link src "$MGMT_ADDR"
    assert_no_bridge_ipv4ll
}

wait_no_kernel_bridge() {
    i=0
    while [ -e "/sys/class/net/$BR" ]; do
        i=$((i + 1))
        [ "$i" -le 50 ] || die "$BR still exists in the kernel; refusing netdev cutover"
        if [ "$i" -eq 10 ] || [ "$i" -eq 25 ]; then
            clear_kernel_datapath
        fi
        sleep 0.2
    done
}

install_vswitchd_netdev_run() {
    for dst in "$VSWITCHD_ETC_RUN" "$VSWITCHD_SVC/run"; do
        dir="$(dirname "$dst")"
        [ -d "$dir" ] || continue
        tmp="$dst.tmp.$$"
        cat >"$tmp" <<'RUN_SCRIPT'
#!/bin/sh
exec 2>&1

BR="${BR:-ovsbr0}"
VETH_HOST="${VETH_HOST:-grpc-uplink}"
FAIL_MODE="${FAIL_MODE:-standalone}"
SHARED_MAC="${SHARED_MAC:-fa:16:3e:f1:71:d2}"
OVSDB_SOCK="${OVSDB_SOCK:-/usr/local/var/run/openvswitch/db.sock}"
SEED_SKIP_FLAG="${SEED_SKIP_FLAG:-/run/op-ovsbr0-skip-seed}"

mkdir -p /run/openvswitch
while [ ! -S "$OVSDB_SOCK" ]; do sleep 0.5; done

if [ -e "$SEED_SKIP_FLAG" ]; then
    rm -f "$SEED_SKIP_FLAG"
    echo "ovs-vswitchd: seed skipped; caller already wrote OVSDB"
else
    ovs-dpctl del-dp system@ovs-system 2>/dev/null || true
    BRIDGE="$BR" \
    VETH_HOST="$VETH_HOST" \
    FAIL_MODE="$FAIL_MODE" \
    SHARED_MAC="$SHARED_MAC" \
    OVSDB_SOCKET="$OVSDB_SOCK" \
        /usr/local/bin/op-ovsbr0-setup --seed-only || exit 1

    echo "ovs-vswitchd: seeded $BR datapath_type=netdev via op-ovsbr0-setup --seed-only"
fi
exec /usr/local/sbin/ovs-vswitchd "unix:$OVSDB_SOCK"
RUN_SCRIPT
        chmod +x "$tmp"
        mv "$tmp" "$dst"
        log "installed netdev-safe ovs-vswitchd run script: $dst"
    done
}

install_op_dbus_netdev_run() {
    for dst in "$OP_DBUS_ETC_RUN" "$OP_DBUS_SVC/run"; do
        dir="$(dirname "$dst")"
        [ -d "$dir" ] || continue
        tmp="$dst.tmp.$$"
        cat >"$tmp" <<'RUN_SCRIPT'
#!/bin/sh
export PRIVACY_DATAPATH_TYPE="${PRIVACY_DATAPATH_TYPE:-netdev}"
export PRIVACY_FAIL_MODE="${PRIVACY_FAIL_MODE:-standalone}"
export PRIVACY_UPLINK_PORT="${PRIVACY_UPLINK_PORT:-eth0}"
exec /usr/local/bin/op-dbus
RUN_SCRIPT
        chmod +x "$tmp"
        mv "$tmp" "$dst"
        log "installed netdev-safe op-dbus run script: $dst"
    done
}

install_ovsbr0_netdev_verify() {
    for dst in /etc/s6/sv/ovsbr0-netdev-verify /run/service/ovsbr0-netdev-verify; do
        [ -d "$(dirname "$dst")" ] || continue
        mkdir -p "$dst/dependencies.d"
        install -m 644 "$REPO_DIR/deploy/s6/ovsbr0-netdev-verify/type" "$dst/type"
        install -m 755 "$REPO_DIR/deploy/s6/ovsbr0-netdev-verify/up" "$dst/up"
        install -m 755 "$REPO_DIR/deploy/s6/ovsbr0-netdev-verify/shell_up" "$dst/shell_up"
        touch "$dst/dependencies.d/ovsbr0-setup"
        log "installed ovsbr0-netdev-verify service: $dst"
    done
    if [ -d /etc/s6/sv/ovs-stack/contents.d ]; then
        touch /etc/s6/sv/ovs-stack/contents.d/ovsbr0-netdev-verify
        log "added ovsbr0-netdev-verify to ovs-stack bundle"
    fi
}

restore_eth0_l3() {
    clear_bridge_l3
    ip link set "$UPLINK" up 2>/dev/null || true
    ip addr replace "${MGMT_ADDR}/${MGMT_PREFIX}" dev "$UPLINK" 2>/dev/null \
        || ip addr add "${MGMT_ADDR}/${MGMT_PREFIX}" dev "$UPLINK" 2>/dev/null \
        || true
    ip route replace "$GW" dev "$UPLINK" scope link src "$MGMT_ADDR" 2>/dev/null || true
    ip route replace default via "$GW" dev "$UPLINK" src "$MGMT_ADDR" metric 1002 2>/dev/null || true
}

stop_ovs_mutators() {
    for svc in \
        "$OP_DBUS_SVC" \
        /run/service/ovsbr0-afxdp \
        /run/service/ovsbr0-dhcp \
        /run/service/ovsbr0-static \
        /run/service/ovsbr0-setup
    do
        [ -e "$svc" ] || continue
        s6-svc -d "$svc" 2>/dev/null || true
    done
}

rollback() {
    [ "$SUCCESS" -eq 0 ] || return 0
    [ "$ROLLBACK_STARTED" -eq 0 ] || return 0
    ROLLBACK_STARTED=1

    warn "cutover failed or was interrupted; rolling back $UPLINK"
    "$OP_AFXDP" detach-only 2>/dev/null || ovs-vsctl --if-exists del-port "$BR" "$UPLINK" 2>/dev/null || true
    restore_eth0_l3
    restore_xdp_chain
    s6-svc -u "$VSWITCHD_SVC" 2>/dev/null || true
    ip route show default >&2 || true
    warn "rollback complete; verify from noVNC: ping -c2 $GW"
}

trap rollback EXIT INT TERM

[ "${1:-}" = "--novnc" ] || die "refusing remote cutover; run from noVNC with: sudo $0 --novnc"
[ "$(id -u)" = 0 ] || die "run as root"

mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/cutover-$(date +%Y%m%d-%H%M%S).log"
exec > >(tee -a "$LOG_FILE") 2>&1
log "capturing cutover transcript in $LOG_FILE"

require_cmd ip
require_cmd ovs-vsctl
require_cmd ovs-dpctl
require_cmd s6-svc
require_cmd xdp-loader
require_cmd clang
require_cmd ping
require_cmd install

install_repo_binaries

[ -x "$OP_XDP_WG" ] || die "missing $OP_XDP_WG"
if [ ! -x "$OP_AFXDP" ] && [ -x /usr/local/sbin/op-ovsbr0-afxdp ]; then
    OP_AFXDP=/usr/local/sbin/op-ovsbr0-afxdp
fi
[ -x "$OP_AFXDP" ] || die "missing op-ovsbr0-afxdp in /usr/local/bin or /usr/local/sbin"
if [ ! -x "$OP_SETUP" ] && [ -x /usr/local/sbin/op-ovsbr0-setup ]; then
    OP_SETUP=/usr/local/sbin/op-ovsbr0-setup
fi
[ -x "$OP_SETUP" ] || die "missing op-ovsbr0-setup in /usr/local/bin or /usr/local/sbin"

log "making live s6 ovs-vswitchd service seed netdev through rovs"
install_vswitchd_netdev_run
install_op_dbus_netdev_run
install_ovsbr0_netdev_verify

log "preventing DHCP/IPv4LL clients from claiming $BR"
disable_bridge_dhcp_clients

log "preflight: preserving current $UPLINK route"
restore_eth0_l3
assert_no_bridge_ipv4ll
ping -c1 -W2 "$GW" >/dev/null || die "gateway $GW is not reachable before cutover"

log "stopping s6 services that mutate OVS during cutover"
stop_ovs_mutators

log "stopping ovs-vswitchd via s6"
s6-svc -d "$VSWITCHD_SVC" 2>/dev/null || true
s6-svc -k "$VSWITCHD_SVC" 2>/dev/null || true
sleep 1

log "clearing stale kernel datapath state"
clear_kernel_datapath
wait_no_kernel_bridge

log "creating $BR with rovs-backed setup binary"
touch "$SEED_SKIP_FLAG"
BRIDGE="$BR" \
VETH_HOST="$VETH" \
SHARED_MAC="$SHARED_MAC" \
VSWITCHD_SVC="$VSWITCHD_SVC" \
PRIVACY_DATAPATH_TYPE=netdev \
PRIVACY_FAIL_MODE=standalone \
"$OP_SETUP"

DT_LIVE="$(ovs-vsctl get bridge "$BR" datapath_type 2>/dev/null || echo unknown)"
[ "$DT_LIVE" = "netdev" ] || die "vswitchd reset $BR to datapath_type=$DT_LIVE"
log "$BR is netdev"
enable_ovs_afxdp_debug

log "checking XDP state before AF_XDP attach"
XDP_PRE="$(xdp-loader status "$UPLINK" 2>/dev/null || true)"
printf '%s\n' "$XDP_PRE"

log "pre-staging ${MGMT_ADDR}/${MGMT_PREFIX} on $BR without flushing $UPLINK"
stage_bridge_l3

log "setting container $CT MAC if container is available"
if incus info "$CT" >/dev/null 2>&1; then
    incus exec "$CT" -- ip link set eth0 address "$SHARED_MAC" 2>/dev/null || true
fi

log "bringing $VETH up"
ip link set "$VETH" up 2>/dev/null || true

log "attaching $UPLINK as AF_XDP through patched libxdp dispatcher path"
"$OP_AFXDP" detach-only 2>/dev/null || true
"$OP_AFXDP" attach-only
sleep 1

ERR="$(ovs-vsctl get interface "$UPLINK" error 2>/dev/null || true)"
[ -z "$ERR" ] || [ "$ERR" = "[]" ] || die "OVS reported AF_XDP interface error: $ERR"

log "re-chaining xdp_steer after OVS AF_XDP attach"
"$OP_XDP_WG" chain

XDP_POST="$(xdp-loader status "$UPLINK" 2>/dev/null || true)"
printf '%s\n' "$XDP_POST"
printf '%s\n' "$XDP_POST" | grep -q 'xdp_dispatcher' || die "dispatcher disappeared after AF_XDP attach"
printf '%s\n' "$XDP_POST" | grep -q 'xdp_steer' || die "xdp_steer disappeared after AF_XDP attach"

log "testing gateway through $BR before moving default route"
assert_no_bridge_ipv4ll
ip route replace "$GW" dev "$BR" scope link src "$MGMT_ADDR"
ping -I "$MGMT_ADDR" -c2 -W2 "$GW" >/dev/null || die "gateway is not reachable through $BR"

log "moving default route to $BR and flushing $UPLINK address"
remove_bridge_defaults
ip route replace default via "$GW" dev "$BR" onlink src "$MGMT_ADDR" metric 100
ip addr flush dev "$UPLINK" 2>/dev/null || true
ping -c2 -W2 "$GW" >/dev/null || die "gateway failed after default route move"

SUCCESS=1
trap - EXIT INT TERM

log "cutover complete"
ovs-vsctl show
ip -4 addr show dev "$BR"
ip route show default
xdp-loader status "$UPLINK" 2>/dev/null || true
