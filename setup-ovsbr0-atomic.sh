#!/bin/bash
# Atomic OVS Bridge Setup: ovsbr0 with management port + physical uplink
#
# Drives the live plugin tree through `zcall` (operator wrapper over
# org.opdbus.v1.plugins / /org/opdbus/v1/plugins / org.opdbus.v1.PluginV1).
# No busctl plumbing, no OvsdbV1 interface — those are gone.
#
# ATOMICITY, both halves:
#   1. OVSDB — the bridge, its internal management port, and the physical
#      uplink go in as ONE `ovsdb_bridge transact`. Creating the bridge and
#      then enslaving the uplink as a second mutation is the boot-time race:
#      vswitchd reconfigures the datapath at the moment the host's only path
#      to the network joins it. One transaction means vswitchd's first read of
#      OVSDB sees the finished topology and brings the datapath up once.
#   2. Kernel — the address/route move off the uplink onto the bridge goes
#      through a single `ip -batch`.
#
# Usage: sudo ./setup-ovsbr0-atomic.sh [--no-ip-migrate] [--dry-run]

set -euo pipefail

export PATH="/usr/local/bin:/usr/bin:/usr/sbin:${PATH:-}"
set -a; [ -r /etc/op-dbus/network.conf ] && . /etc/op-dbus/network.conf; set +a

BRIDGE="${BRIDGE:-ovsbr0}"
UPLINK="${UPLINK_PHYS:-eth0}"
FAIL_MODE="${FAIL_MODE:-standalone}"
DATAPATH="${DATAPATH_TYPE:-system}"
# OpenFlow versions the bridge accepts. Set at creation so a fresh provision
# matches a long-running host: op-of-controller speaks 1.5, while ovs-ofctl and
# other CLI clients may still default to 1.0. Allow all deployed versions, or
# those clients fail
# version negotiation, vswitchd drops the socket, and the caller sees only the
# misleading "failed to connect to socket (Broken pipe)". Leaving the column
# empty is not equivalent — OVS then defaults to OpenFlow10 alone and the
# controller cannot connect at all.
OF_PROTOCOLS="${OF_PROTOCOLS:-OpenFlow10,OpenFlow13,OpenFlow15}"
MIGRATE_IP=true
DRY_RUN=false

for arg in "$@"; do
    case "$arg" in
        --no-ip-migrate) MIGRATE_IP=false ;;
        --dry-run|--print) DRY_RUN=true ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

die() { log_error "$1"; exit 1; }

# --- Preflight checks ---
[[ $EUID -eq 0 ]] || die "Must run as root"
command -v zcall &>/dev/null || die "zcall not found — install the operator CLI"
[[ -d "/sys/class/net/$UPLINK" ]] || die "Uplink interface $UPLINK does not exist"

# The plugin tree has to be answering before any of this means anything.
zcall list &>/dev/null || die "plugin tree not answering — start op-grpc-bridge first"
log_info "plugin tree is live"

# Bridge absence — read the source, not the kernel.
if zcall rovs_commands list_bridges 2>/dev/null | grep -qw "$BRIDGE"; then
    die "Bridge '$BRIDGE' already exists in OVSDB"
fi
if [[ -d "/sys/class/net/$BRIDGE" ]]; then
    die "Bridge '$BRIDGE' already exists in kernel"
fi

# --- Capture uplink IP config before we enslave it ---
UPLINK_ADDRS=()
UPLINK_GATEWAY=""

if $MIGRATE_IP; then
    log_info "Capturing IP configuration from $UPLINK..."

    while IFS= read -r addr; do
        [[ -n "$addr" ]] && UPLINK_ADDRS+=("$addr")
    done < <(ip -4 -o addr show dev "$UPLINK" scope global | awk '{print $4}')

    UPLINK_GATEWAY=$(ip -4 route show default dev "$UPLINK" 2>/dev/null | awk '{print $3}' | head -1)

    if [[ ${#UPLINK_ADDRS[@]} -eq 0 ]]; then
        log_warn "$UPLINK has no global IPv4 addresses — IP migration will be skipped"
        MIGRATE_IP=false
    else
        log_info "  Addresses: ${UPLINK_ADDRS[*]}"
        log_info "  Gateway:   ${UPLINK_GATEWAY:-none}"
    fi
fi

# --- Build ONE transaction: bridge + management port + uplink ---
# Each port is an Interface row and a Port row joined by named-uuid; the Bridge
# row references them all, so the bridge cannot come into existence without its
# uplink already in its ports set.
OPS=""
REFS=""

# Comma-separated OF_PROTOCOLS -> an OVSDB set literal.
PROTO_ITEMS=""
IFS=',' read -ra _protos <<< "$OF_PROTOCOLS"
for _v in "${_protos[@]}"; do
    [[ -n $_v ]] && PROTO_ITEMS="${PROTO_ITEMS}\"${_v}\","
done
[[ -n $PROTO_ITEMS ]] || die "OF_PROTOCOLS is empty — refusing to create a bridge with no OpenFlow version"
PROTOCOLS="[\"set\",[${PROTO_ITEMS%,}]]"

add_port() {
    local name="$1" type="$2" tag="$3"
    OPS="${OPS}
{\"op\":\"insert\",\"table\":\"Interface\",\"uuid-name\":\"i_${tag}\",
 \"row\":{\"name\":\"${name}\",\"type\":\"${type}\"}},
{\"op\":\"insert\",\"table\":\"Port\",\"uuid-name\":\"p_${tag}\",
 \"row\":{\"name\":\"${name}\",\"interfaces\":[\"set\",[[\"named-uuid\",\"i_${tag}\"]]]}},"
    REFS="${REFS}[\"named-uuid\",\"p_${tag}\"],"
}

# The bridge's own internal management port.
add_port "$BRIDGE" "internal" "br"
# The physical uplink — an ordinary system netdev, same transaction.
add_port "$UPLINK" "system" "uplink"

PAYLOAD="{\"db_name\":\"Open_vSwitch\",\"operations\":[${OPS}
{\"op\":\"insert\",\"table\":\"Bridge\",\"uuid-name\":\"br\",
 \"row\":{\"name\":\"${BRIDGE}\",\"datapath_type\":\"${DATAPATH}\",
          \"fail_mode\":\"${FAIL_MODE}\",
          \"protocols\":${PROTOCOLS},
          \"ports\":[\"set\",[${REFS%,}]]}},
{\"op\":\"mutate\",\"table\":\"Open_vSwitch\",\"where\":[],
 \"mutations\":[[\"bridges\",\"insert\",[\"set\",[[\"named-uuid\",\"br\"]]]]]}]}"

if $DRY_RUN; then
    log_info "Transaction that would be submitted:"
    zcall expand ovsdb_bridge transact -a "$PAYLOAD"
    exit 0
fi

log_info "Creating '$BRIDGE' with management port and '$UPLINK' enslaved, in one transact..."
zcall ovsdb_bridge transact -a "$PAYLOAD" >/dev/null \
    || die "transact failed — bridge not created, uplink untouched"
log_info "  transaction committed"

# --- Wait for the kernel to catch up ---
log_info "Waiting for kernel interfaces..."
for _ in $(seq 1 30); do
    [[ -d "/sys/class/net/$BRIDGE" ]] && break
    sleep 0.1
done
[[ -d "/sys/class/net/$BRIDGE" ]] || die "Bridge interface $BRIDGE did not appear in kernel"
log_info "  $BRIDGE is up in kernel"

# --- Atomic IP migration: uplink -> bridge ---
if $MIGRATE_IP; then
    log_info "Migrating IP configuration from $UPLINK to $BRIDGE..."

    BATCH_FILE=$(mktemp /tmp/ovs-ip-migrate.XXXXXX)
    trap 'rm -f "$BATCH_FILE"' EXIT

    echo "link set dev $BRIDGE up" >> "$BATCH_FILE"

    for addr in "${UPLINK_ADDRS[@]}"; do
        echo "addr del $addr dev $UPLINK" >> "$BATCH_FILE"
        echo "addr add $addr dev $BRIDGE" >> "$BATCH_FILE"
    done

    if [[ -n "$UPLINK_GATEWAY" ]]; then
        echo "route replace default via $UPLINK_GATEWAY dev $BRIDGE" >> "$BATCH_FILE"
    fi

    log_info "  Executing ip batch:"
    while IFS= read -r line; do
        log_info "    ip $line"
    done < "$BATCH_FILE"

    ip -batch "$BATCH_FILE"
    log_info "IP migration complete"
fi

# --- Bring uplink up (OVS needs it for forwarding) ---
ip link set dev "$UPLINK" up

# --- Verification ---
# Nothing to reconcile or force-refresh: present state is republished by the
# mutation that just ran. These are plain reads of the same source.
log_info ""
log_info "=== Verification ==="

log_info "Bridges:"
zcall rovs_commands list_bridges 2>/dev/null || log_warn "  list_bridges failed"

log_info ""
log_info "Ports on $BRIDGE:"
zcall rovs_commands list_ports -a "{\"bridge_name\":\"$BRIDGE\"}" 2>/dev/null \
    || log_warn "  list_ports failed"

log_info ""
log_info "Kernel state:"
ip -4 addr show dev "$BRIDGE" 2>/dev/null || log_warn "  $BRIDGE has no IPv4 address"
ip link show dev "$UPLINK" 2>/dev/null | head -2

log_info ""
log_info "=== Summary ==="
log_info "  Bridge:         $BRIDGE"
log_info "  Management:     $BRIDGE (type=internal)"
log_info "  Uplink:         $UPLINK (enslaved, same transaction)"
log_info "  Datapath:       $DATAPATH"
log_info "  Fail mode:      $FAIL_MODE"
log_info "  Method:         zcall ovsdb_bridge transact (single OVSDB transaction)"
if $MIGRATE_IP; then
    log_info "  IP migrated:    ${UPLINK_ADDRS[*]}"
    [[ -n "$UPLINK_GATEWAY" ]] && log_info "  Default route:  via $UPLINK_GATEWAY dev $BRIDGE"
fi
log_info ""
log_info "Done. Verify connectivity: ping -c1 -W2 ${UPLINK_GATEWAY:-8.8.8.8}"
