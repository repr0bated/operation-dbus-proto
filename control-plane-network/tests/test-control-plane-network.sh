#!/bin/sh
set -eu

TEST_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ROOT_DIR="$(CDPATH= cd -- "$TEST_DIR/.." && pwd)"
SCRIPT="$ROOT_DIR/bin/control-plane-network"
INSTALLER="$ROOT_DIR/install.sh"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/control-plane-network-test.XXXXXX")"
SOCKET_PID=""
LOCK_PID=""
TESTS=0

cleanup() {
    [ -z "$LOCK_PID" ] || kill "$LOCK_PID" 2>/dev/null || true
    [ -z "$SOCKET_PID" ] || kill "$SOCKET_PID" 2>/dev/null || true
    rm -rf "$TMP_ROOT"
}
trap cleanup 0 HUP INT TERM

fail() {
    printf 'not ok %s - %s\n' "$TESTS" "$*" >&2
    exit 1
}

pass() {
    TESTS=$((TESTS + 1))
    printf 'ok %s - %s\n' "$TESTS" "$1"
}

assert_file() {
    [ -f "$1" ] || fail "expected file: $1"
}

assert_no_file() {
    [ ! -e "$1" ] || fail "unexpected file: $1"
}

assert_log_count() {
    _alc_expected="$1"
    _alc_pattern="$2"
    _alc_actual="$(grep -c "$_alc_pattern" "$MOCK_DIR/calls.log" 2>/dev/null || true)"
    [ "$_alc_actual" -eq "$_alc_expected" ] \
        || fail "expected $_alc_expected matches for $_alc_pattern, got $_alc_actual"
}

stop_socket_server() {
    if [ -n "$SOCKET_PID" ]; then
        kill "$SOCKET_PID" 2>/dev/null || true
        wait "$SOCKET_PID" 2>/dev/null || true
        SOCKET_PID=""
    fi
}

write_mocks() {
    mkdir -p "$MOCK_BIN" "$MOCK_DIR"
    : > "$MOCK_DIR/calls.log"

    cat > "$MOCK_BIN/busctl" <<'EOF'
#!/bin/sh
set -eu
printf 'busctl|%s\n' "$*" >> "$MOCK_DIR/calls.log"
case " $* " in
    *" NameHasOwner "*)
        printf '%s\n' '{"type":"b","data":[true]}'
        exit 0
        ;;
esac
method="${9:-}"
args="${10:-}"
printf '%s|%s\n' "$method" "$args" >> "$MOCK_DIR/calls.log"
case "$method" in
    list_bridges)
        if [ -f "$MOCK_DIR/bridge" ]; then
            printf '%s\n' '{"type":"s","data":["{\"bridges\":[\"ovsbr0\"]}"]}'
        else
            printf '%s\n' '{"type":"s","data":["{\"bridges\":[]}"]}'
        fi
        ;;
    list_ports)
        if [ -f "$MOCK_DIR/eth0-port" ] && [ -f "$MOCK_DIR/netmaker-port" ]; then
            printf '%s\n' '{"type":"s","data":["{\"ports\":[\"eth0\",\"netmaker\"]}"]}'
        elif [ -f "$MOCK_DIR/netmaker-port" ]; then
            printf '%s\n' '{"type":"s","data":["{\"ports\":[\"netmaker\"]}"]}'
        else
            printf '%s\n' '{"type":"s","data":["{\"ports\":[]}"]}'
        fi
        ;;
    ensure_bridge_ports)
        : > "$MOCK_DIR/bridge"
        : > "$MOCK_DIR/eth0-port"
        : > "$MOCK_DIR/netmaker-port"
        printf '%s\n' '{"type":"s","data":["{\"added\":[\"eth0\",\"netmaker\"],\"already_attached\":[],\"bridge\":\"ovsbr0\",\"created_bridge\":true}"]}'
        ;;
    remove_port)
        case "$args" in *'"port_name":"eth0"'*) rm -f "$MOCK_DIR/eth0-port" ;; esac
        printf '%s\n' '{"type":"s","data":["{\"removed\":\"eth0\",\"source\":\"ovsbr0\"}"]}'
        ;;
    list_registrations)
        printf '%s\n' '{"type":"s","data":["{\"sockets\":[]}"]}'
        ;;
    add_flow)
        printf '%s\n' '{"type":"s","data":["{\"success\":true}"]}'
        ;;
    *)
        printf '%s\n' '{"type":"s","data":["{\"success\":true}"]}'
        ;;
esac
EOF

    cat > "$MOCK_BIN/ip" <<'EOF'
#!/bin/sh
set -eu
printf 'ip|%s\n' "$*" >> "$MOCK_DIR/calls.log"
case "$*" in
    "-batch "*)
        count=0
        [ ! -r "$MOCK_DIR/batch-count" ] || count="$(cat "$MOCK_DIR/batch-count")"
        count=$((count + 1))
        printf '%s\n' "$count" > "$MOCK_DIR/batch-count"
        if [ "${MOCK_FAIL_SECOND_BATCH:-0}" = 1 ] && [ "$count" -eq 2 ]; then
            exit 1
        fi
        ;;
    "link show netmaker"|"link show dev netmaker")
        [ -f "$MOCK_DIR/netmaker-link" ]
        ;;
    "link add netmaker type wireguard")
        : > "$MOCK_DIR/netmaker-link"
        ;;
    "link del netmaker")
        rm -f "$MOCK_DIR/netmaker-link"
        ;;
    "link show ovsbr0"|"link show dev ovsbr0")
        [ -f "$MOCK_DIR/bridge" ]
        ;;
    "-o link show ovsbr0"|"-o link show dev ovsbr0")
        printf '%s\n' '4: ovsbr0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 link/ether 02:00:00:00:00:01 brd ff:ff:ff:ff:ff:ff'
        ;;
    "-o link show netmaker"|"-o link show dev netmaker")
        printf '%s\n' '5: netmaker: <POINTOPOINT,NOARP,UP,LOWER_UP> mtu 1420 link/none'
        ;;
    "-o addr show dev eth0 scope global")
        printf '%s\n' \
          '2: eth0 inet 148.113.204.83/32 scope global eth0' \
          '2: eth0 inet 10.200.0.2/32 scope global eth0' \
          '2: eth0 inet 10.0.0.2/32 scope global eth0'
        ;;
    "-o addr show dev ovsbr0 scope global")
        printf '%s\n' '4: ovsbr0 inet 10.200.0.1/30 scope global ovsbr0'
        ;;
    "-o addr show dev netmaker scope global")
        printf '%s\n' \
          '5: netmaker inet 10.0.0.3/32 scope global netmaker' \
          '5: netmaker inet 100.90.37.254/32 scope global netmaker' \
          '5: netmaker inet6 fd3c:8a39:2d9e:9577:ffff:ffff:ffff:fffe/128 scope global'
        ;;
    "-4 route show default")
        printf '%s\n' 'default via 148.113.204.1 dev eth0 src 148.113.204.83'
        ;;
    "-4 route show 148.113.204.1/32")
        printf '%s\n' '148.113.204.1 dev eth0 scope link src 148.113.204.83'
        ;;
    "-4 route show 10.0.0.0/24")
        printf '%s\n' '10.0.0.0/24 dev netmaker scope link src 10.0.0.3'
        ;;
    "-4 route show 100.90.37.0/24")
        printf '%s\n' '100.90.37.0/24 dev netmaker scope link src 100.90.37.254'
        ;;
    "-6 route show fd3c:8a39:2d9e:9577::/64")
        printf '%s\n' 'fd3c:8a39:2d9e:9577::/64 dev netmaker src fd3c:8a39:2d9e:9577:ffff:ffff:ffff:fffe'
        ;;
    "-brief address")
        printf '%s\n' 'eth0 UP 148.113.204.83/32' 'ovsbr0 UP 10.200.0.1/30' 'netmaker UP 10.0.0.3/32'
        ;;
    "-4 route"|"route")
        printf '%s\n' 'default via 148.113.204.1 dev eth0'
        ;;
    "-6 route")
        printf '%s\n' 'fd3c:8a39:2d9e:9577::/64 dev netmaker'
        ;;
esac
exit 0
EOF

    cat > "$MOCK_BIN/wg-quick" <<'EOF'
#!/bin/sh
set -eu
printf 'wg-quick|%s\n' "$*" >> "$MOCK_DIR/calls.log"
printf '%s\n' '[Interface]' 'PrivateKey = test-private-key' '[Peer]' 'PublicKey = test-peer'
EOF

    cat > "$MOCK_BIN/wg" <<'EOF'
#!/bin/sh
set -eu
printf 'wg|%s\n' "$*" >> "$MOCK_DIR/calls.log"
case "$1" in
    setconf)
        stat -c %a "$3" > "$MOCK_DIR/wg-temp-mode"
        ;;
    show)
        if [ "${3:-}" = peers ]; then
            printf '%s\n' test-peer
        else
            printf '%s\n' 'interface: netmaker' 'peer: test-peer'
        fi
        ;;
esac
EOF

    cat > "$MOCK_BIN/ping" <<'EOF'
#!/bin/sh
printf 'ping|%s\n' "$*" >> "$MOCK_DIR/calls.log"
exit "${MOCK_PING_RC:-0}"
EOF

    cat > "$MOCK_BIN/sleep" <<'EOF'
#!/bin/sh
exit 0
EOF

    for command in ss who last; do
        cat > "$MOCK_BIN/$command" <<'EOF'
#!/bin/sh
exit 0
EOF
    done
    chmod 0755 "$MOCK_BIN"/*
}

start_socket_server() {
    python3 - "$DBUS_SOCKET" "$OVSDB_SOCKET" <<'PY' &
import os
import signal
import socket
import sys
import time

sockets = []
for path in sys.argv[1:]:
    try:
        os.unlink(path)
    except FileNotFoundError:
        pass
    sock = socket.socket(socket.AF_UNIX)
    sock.bind(path)
    sock.listen(1)
    sockets.append(sock)
signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
while True:
    time.sleep(1)
PY
    SOCKET_PID=$!
    attempts=0
    while [ ! -S "$DBUS_SOCKET" ] || [ ! -S "$OVSDB_SOCKET" ]; do
        attempts=$((attempts + 1))
        [ "$attempts" -lt 100 ] || fail "socket fixture did not start"
        /usr/bin/sleep 0.01
    done
}

new_fixture() {
    stop_socket_server
    FIXTURE="$TMP_ROOT/$1"
    MOCK_BIN="$FIXTURE/bin"
    MOCK_DIR="$FIXTURE/mock"
    STATE_DIR="$FIXTURE/state"
    REPORT_DIR="$FIXTURE/report"
    WG_DIR="$FIXTURE/wireguard"
    DBUS_SOCKET="$FIXTURE/dbus.sock"
    OVSDB_SOCKET="$FIXTURE/ovsdb.sock"
    CONF="$FIXTURE/network.conf"
    mkdir -p "$STATE_DIR" "$REPORT_DIR" "$WG_DIR"
    write_mocks
    cat > "$WG_DIR/netmaker.conf" <<'EOF'
[Interface]
PrivateKey = test-private-key
MTU = 1420
[Peer]
PublicKey = test-peer
AllowedIPs = 10.0.0.0/24
EOF
    chmod 0600 "$WG_DIR/netmaker.conf"
    cat > "$CONF" <<EOF
NODE_ROLE=main
STATE_DIR=$STATE_DIR
REPORT_DIR=$REPORT_DIR
DBUS_SOCKET=$DBUS_SOCKET
OVSDB_SOCK=$OVSDB_SOCKET
DHCPCD_CONF=$FIXTURE/dhcpcd.conf
TAME_DHCPCD=no
WIREGUARD_CONF_DIR=$WG_DIR
DBUS_WAIT=2
OVSDB_WAIT=2
PLUGIN_BUS_WAIT=2
CONTROLLER_WAIT=2
BRIDGE_NETDEV_WAIT=2
COMMAND_TIMEOUT=2
GATEWAY_PROBE_ATTEMPTS=2
GATEWAY_PROBE_TIMEOUT=1
FLOW_RETRY_ATTEMPTS=2
FLOW_RETRY_PAUSE=0
EOF
    start_socket_server
}

run_script() {
    CONTROL_PLANE_NETWORK_TEST_MODE=1 \
    CONTROL_PLANE_NETWORK_TEST_ROOT="$FIXTURE" \
    CONTROL_PLANE_NETWORK_TEST_PATH="$MOCK_BIN" \
    CONTROL_PLANE_NETWORK_CONF="$CONF" \
    MOCK_DIR="$MOCK_DIR" \
    MOCK_FAIL_SECOND_BATCH="${MOCK_FAIL_SECOND_BATCH:-0}" \
    "$SCRIPT" "$@"
}

new_fixture check
run_script --check-config > "$FIXTURE/output" 2>&1 || {
    cat "$FIXTURE/output" >&2
    fail "check-config failed"
}
assert_log_count 0 '^busctl|'
assert_log_count 0 '^ip|'
pass "check-config is read-only"

new_fixture bootstrap
run_script > "$FIXTURE/output" 2>&1 || {
    cat "$FIXTURE/output" >&2
    fail "bootstrap failed"
}
assert_log_count 1 '^ensure_bridge_ports|'
assert_log_count 0 '^add_flow|'
assert_log_count 1 '^set_controller|'
assert_file "$STATE_DIR/main.ready"
assert_no_file "$STATE_DIR/main.bootstrap.failed"
[ "$(cat "$MOCK_DIR/wg-temp-mode")" = 600 ] || fail "WireGuard temporary file was not mode 0600"
[ -z "$(find "$STATE_DIR" -maxdepth 1 \( -name 'wg-netmaker.*' -o -name 'uplink-replumb.*' \) -print)" ] \
    || fail "sensitive temporary files survived bootstrap"
pass "bootstrap uses one bridge transaction and no flow calls"

new_fixture rollback
: > "$STATE_DIR/main.ready"
MOCK_FAIL_SECOND_BATCH=1
set +e
run_script > "$FIXTURE/output" 2>&1
rc=$?
set -e
MOCK_FAIL_SECOND_BATCH=0
[ "$rc" -ne 0 ] || fail "post-attach batch failure unexpectedly succeeded"
assert_log_count 1 '^ensure_bridge_ports|'
assert_log_count 1 '^remove_port|'
assert_no_file "$STATE_DIR/main.ready"
assert_file "$STATE_DIR/main.bootstrap.failed"
pass "post-attach failure rolls back and clears stale readiness"

new_fixture flows
run_script > "$FIXTURE/bootstrap-output" 2>&1 || {
    cat "$FIXTURE/bootstrap-output" >&2
    fail "flow prerequisite bootstrap failed"
}
: > "$MOCK_DIR/calls.log"
run_script --flows-only > "$FIXTURE/flow-output" 2>&1 || {
    cat "$FIXTURE/flow-output" >&2
    fail "flow reconciliation failed"
}
assert_log_count 0 '^ensure_bridge_ports|'
assert_log_count 2 '^add_flow|'
assert_file "$STATE_DIR/main.flows-ready"
pass "flow methods run only in the post-controller mode"

new_fixture invalid
printf '%s\n' "UPLINK_NIC='eth0;bad'" >> "$CONF"
set +e
run_script --check-config > "$FIXTURE/output" 2>&1
rc=$?
set -e
[ "$rc" -eq 2 ] || fail "invalid interface config returned $rc instead of 2"
assert_log_count 0 '^busctl|'
pass "unsafe interface configuration is rejected before mutation"

new_fixture declarative
injection_marker="$FIXTURE/config-was-executed"
printf '%s\n' "BIRTHDAY='\$(touch $injection_marker)'" >> "$CONF"
run_script --check-config > "$FIXTURE/output" 2>&1 || {
    cat "$FIXTURE/output" >&2
    fail "declarative config check failed"
}
assert_no_file "$injection_marker"
assert_log_count 0 '^busctl|'
pass "configuration values are parsed without shell execution"

new_fixture wg_symlink
mv "$WG_DIR/netmaker.conf" "$WG_DIR/netmaker.real"
ln -s netmaker.real "$WG_DIR/netmaker.conf"
set +e
run_script --check-config > "$FIXTURE/output" 2>&1
rc=$?
set -e
[ "$rc" -eq 2 ] || fail "symlinked WireGuard config returned $rc instead of 2"
assert_log_count 0 '^busctl|'
pass "symlinked WireGuard private-key sources are rejected"

new_fixture duplicate_mode
set +e
run_script --flows-only --controller-only > "$FIXTURE/output" 2>&1
rc=$?
set -e
[ "$rc" -eq 2 ] || fail "duplicate mode returned $rc instead of 2"
assert_log_count 0 '^busctl|'
pass "conflicting command modes are rejected before mutation"

new_fixture locking
lock_ready="$FIXTURE/lock-ready"
(
    exec 8> "$STATE_DIR/control-plane-network.lock"
    flock -n 8
    : > "$lock_ready"
    /usr/bin/sleep 30
) &
LOCK_PID=$!
while [ ! -f "$lock_ready" ]; do /usr/bin/sleep 0.01; done
set +e
run_script --controller-only > "$FIXTURE/output" 2>&1
rc=$?
set -e
kill "$LOCK_PID" 2>/dev/null || true
wait "$LOCK_PID" 2>/dev/null || true
LOCK_PID=""
[ "$rc" -eq 75 ] || fail "lock contention returned $rc instead of 75"
pass "concurrent reconciliation is rejected with EX_TEMPFAIL"

PREFIX_ROOT="$TMP_ROOT/install-root"
PREFIX="$PREFIX_ROOT" "$INSTALLER" > "$TMP_ROOT/install-1.log"
printf '%s\n' '# preserved-local-value' >> "$PREFIX_ROOT/etc/control-plane-network/network.conf"
PREFIX="$PREFIX_ROOT" "$INSTALLER" > "$TMP_ROOT/install-2.log"
grep -q '^# preserved-local-value$' "$PREFIX_ROOT/etc/control-plane-network/network.conf" \
    || fail "installer overwrote persistent config"
assert_file "$PREFIX_ROOT/usr/share/control-plane-network/network.conf.example"
assert_file "$PREFIX_ROOT/etc/s6/sv/control-plane-network-flows/dependencies.d/op-of-controller-srv"
[ -x "$PREFIX_ROOT/usr/local/sbin/control-plane-network-flows" ] \
    || fail "flow entry point is not executable"
pass "installer preserves policy and installs the split service graph"

printf '1..%s\n' "$TESTS"
