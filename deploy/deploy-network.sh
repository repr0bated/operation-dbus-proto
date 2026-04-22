#!/usr/bin/env bash
# deploy-network.sh — Idempotent network bootstrap for operation-dbus-proto VPS
#
# Topology:
#   ens3 (148.113.204.83/32, standalone public uplink) → Host
#   ovsbr0 (10.88.88.1/24, private OVS bridge)          → privacy datapath
#   incusbr0 (10.149.181.1/24, Incus LAN)               → containers
#
# Containers:
#   services             10.149.181.188  — NextDNS DNS server
#   privacy-xray-ingress 10.149.181.167  — WireGuard/xray ingress
#   xray-server          10.149.181.100  — Xray server (optional)
#
# Usage:
#   sudo ./deploy-network.sh [--exclude-xray-server] [--help]
#
# Environment variables:
#   NEXTDNS_API_KEY  — Required only when NextDNS is not yet configured in the
#                      services container and cannot be discovered automatically.

set -euo pipefail

# ---------------------------------------------------------------------------
# 1. ARGUMENT PARSING
# ---------------------------------------------------------------------------

EXCLUDE_XRAY_SERVER=false

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  --exclude-xray-server   Skip creating/starting the xray-server container
  --help                  Show this help message and exit
EOF
}

for arg in "$@"; do
    case "$arg" in
        --exclude-xray-server)
            EXCLUDE_XRAY_SERVER=true
            ;;
        --help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg" >&2
            usage >&2
            exit 1
            ;;
    esac
done

# ---------------------------------------------------------------------------
# 2. LOGGING SETUP
# ---------------------------------------------------------------------------

LOG_FILE="/var/log/deploy-network.log"

# Ensure log file is writable. If /var/log is not yet writable as the current
# user, we redirect to a temp file and note this.  The calling convention
# requires sudo, so this should not be an issue in normal use.
log() {
    local ts
    ts="$(date '+%Y-%m-%d %H:%M:%S')"
    local msg="[${ts}] $*"
    echo "$msg"
    # Append to log file — this may require root; fail gracefully if not.
    echo "$msg" >> "$LOG_FILE" 2>/dev/null || true
}

die() {
    log "ERROR: $*"
    exit 1
}

start_service_via_dinit_dbus() {
    local service="$1"

    if command -v gdbus &>/dev/null; then
        local output
        if output="$(gdbus call \
            --system \
            --dest org.chimera.dinit \
            --object-path /org/chimera/dinit \
            --method org.chimera.dinit.Manager.StartService \
            "$service" \
            false 2>&1)"; then
            return 0
        fi

        if grep -q 'org.chimera.dinit.Error.ServiceAlready' <<<"$output"; then
            return 0
        fi
    fi

    if command -v dinitctl &>/dev/null; then
        dinitctl start "$service" 2>/dev/null && return 0
    fi

    return 1
}

stop_service_via_dinit_dbus() {
    local service="$1"

    if command -v gdbus &>/dev/null; then
        local output
        if output="$(gdbus call \
            --system \
            --dest org.chimera.dinit \
            --object-path /org/chimera/dinit \
            --method org.chimera.dinit.Manager.StopService \
            "$service" \
            false \
            false \
            false 2>&1)"; then
            return 0
        fi

        if grep -q 'org.chimera.dinit.Error.ServiceAlready' <<<"$output"; then
            return 0
        fi
    fi

    if command -v dinitctl &>/dev/null; then
        dinitctl stop "$service" 2>/dev/null && return 0
    fi

    return 1
}

restart_service_via_dinit_dbus() {
    local service="$1"
    stop_service_via_dinit_dbus "$service" || true
    start_service_via_dinit_dbus "$service"
}

# ---------------------------------------------------------------------------
# 3. PREFLIGHT CHECKS
# ---------------------------------------------------------------------------

log "=== deploy-network.sh starting ==="
log "exclude-xray-server: ${EXCLUDE_XRAY_SERVER}"

# Must be root (sudo provides this; Chimera has sudo→doas alias)
if [[ "$(id -u)" -ne 0 ]]; then
    die "This script must be run as root (use: sudo $0 $*)"
fi

# wgcf config
if [[ ! -f /etc/wireguard/wgcf.conf ]]; then
    die "/etc/wireguard/wgcf.conf not found or not readable. Provision the WARP key before running this script."
fi
log "Preflight: /etc/wireguard/wgcf.conf present"

# incus binary
if ! command -v incus &>/dev/null; then
    die "incus binary not found. Install Incus before running this script."
fi
log "Preflight: incus found at $(command -v incus)"

# wg-quick
if ! command -v wg-quick &>/dev/null; then
    die "wg-quick not found. Install wireguard-tools before running this script."
fi
log "Preflight: wg-quick found at $(command -v wg-quick)"

# netplan
if ! command -v netplan &>/dev/null; then
    die "netplan not found. Install netplan.io before running this script."
fi
log "Preflight: netplan found at $(command -v netplan)"

# curl
if ! command -v curl &>/dev/null; then
    die "curl not found. Install curl before running this script."
fi
log "Preflight: curl found at $(command -v curl)"

# jq
if ! command -v jq &>/dev/null; then
    die "jq not found. Install jq before running this script."
fi
log "Preflight: jq found at $(command -v jq)"

# dig (for DNS verification in section 16)
if ! command -v dig &>/dev/null; then
    die "dig not found. Install bind-tools (or dig equivalent) before running this script."
fi
log "Preflight: dig found at $(command -v dig)"

log "All preflight checks passed."

# ---------------------------------------------------------------------------
# 4. HELPER: wait_for_iface
# ---------------------------------------------------------------------------

wait_for_iface() {
    local iface=$1
    local timeout=$2
    local i=0
    while ! ip link show "$iface" &>/dev/null; do
        sleep 1
        ((i++))
        [[ $i -ge $timeout ]] && die "Interface $iface did not appear after ${timeout}s"
    done
    log "Interface $iface is up"
}

# ---------------------------------------------------------------------------
#    HELPER: wait_for_container
# ---------------------------------------------------------------------------

wait_for_container() {
    local name=$1
    local timeout=$2
    local i=0
    while [[ "$(incus info "$name" 2>/dev/null | awk -F': ' '$1 == "Status" { print toupper($2); exit }' || true)" != "RUNNING" ]]; do
        sleep 2
        ((i+=2))
        [[ $i -ge $timeout ]] && die "Container $name did not reach Running state after ${timeout}s"
    done
    log "Container $name is Running"
}

# ---------------------------------------------------------------------------
# 4. WG-QUICK: Bring up wgcf
# ---------------------------------------------------------------------------

log "--- Section 4: wg-quick up wgcf ---"

# wg-quick returns exit code 1 if the interface already exists ("already exists")
# and a non-1 code on real errors. We capture the output to distinguish.
WG_OUTPUT=""
WG_EXIT=0
WG_OUTPUT=$(wg-quick up wgcf 2>&1) || WG_EXIT=$?

if [[ $WG_EXIT -ne 0 ]]; then
    # "already exists" is not a failure
    if echo "$WG_OUTPUT" | grep -qi "already exists\|already in use"; then
        log "wgcf: interface already up (ignored)"
    else
        die "wg-quick up wgcf failed (exit ${WG_EXIT}): ${WG_OUTPUT}"
    fi
else
    log "wg-quick up wgcf: success"
fi

wait_for_iface wgcf 30

# ---------------------------------------------------------------------------
# 5. DINIT SERVICE: wg-quick-all
# ---------------------------------------------------------------------------

log "--- Section 5: dinit wg-quick-all service ---"

DINIT_D="/etc/dinit.d"
WG_ALL_SERVICE="${DINIT_D}/wg-quick-all"

# The service file is installed by deploy.sh (source: deploy/dinit/wg-quick-all).
# When running standalone, create it as a fallback only.
if [[ ! -f "$WG_ALL_SERVICE" ]]; then
    log "WARNING: ${WG_ALL_SERVICE} not found — creating fallback (run deploy.sh for authoritative install)"
    cat > "$WG_ALL_SERVICE" <<'DINIT_EOF'
# /etc/dinit.d/wg-quick-all — fallback created by deploy-network.sh
# For authoritative install run deploy.sh which installs from deploy/dinit/wg-quick-all
type = scripted
command = /usr/bin/wg-quick up wgcf
stop-command = /usr/bin/wg-quick down wgcf
DINIT_EOF
    log "Created fallback ${WG_ALL_SERVICE}"
else
    log "${WG_ALL_SERVICE} present (installed by deploy.sh)"
fi

# Patch ovs-attach-ports to depend on wg-quick-all
OVS_ATTACH_SERVICE="${DINIT_D}/ovs-attach-ports"
if [[ -f "$OVS_ATTACH_SERVICE" ]]; then
    if ! grep -q "depends-on = wg-quick-all" "$OVS_ATTACH_SERVICE"; then
        log "Patching ${OVS_ATTACH_SERVICE}: adding 'depends-on = wg-quick-all'"
        echo "depends-on = wg-quick-all" >> "$OVS_ATTACH_SERVICE"
        log "Patched ${OVS_ATTACH_SERVICE}"
    else
        log "${OVS_ATTACH_SERVICE} already has 'depends-on = wg-quick-all'"
    fi
else
    log "WARNING: ${OVS_ATTACH_SERVICE} not found — cannot patch; you must install it manually"
fi

# Enable the service with dinitctl if available and daemon is running
if command -v dinitctl &>/dev/null; then
    dinitctl enable wg-quick-all 2>/dev/null || log "dinitctl enable wg-quick-all: already enabled or daemon not running (non-fatal)"
fi

# ---------------------------------------------------------------------------
# 5.5. ENSURE OVS SERVICES ARE RUNNING
# ---------------------------------------------------------------------------
# netplan apply fails if ovsdb-server is not already up (it tries to call
# ovs-vsctl to configure the OVS bridge). Start the repo-managed op-ovs-services
# unit via dinit D-Bus before calling netplan so OVS comes up through a
# persistent, declarative service path.
# ---------------------------------------------------------------------------

log "--- Section 5.5: Ensure ovsdb-server + ovs-vswitchd ---"

OVS_SOCKET=""
for candidate in /run/openvswitch/db.sock /var/run/openvswitch/db.sock; do
    if [[ -S "$candidate" ]]; then
        OVS_SOCKET="$candidate"
        break
    fi
done

if [[ -z "$OVS_SOCKET" ]]; then
    log "ovsdb-server socket not found — starting OVS services via dinit D-Bus"
    start_service_via_dinit_dbus op-ovs-services \
        || log "WARNING: could not start op-ovs-services via dinit D-Bus"
    # Wait up to 30 s for the socket to appear
    OVS_WAIT=0
    until [[ -S /run/openvswitch/db.sock || -S /var/run/openvswitch/db.sock || $OVS_WAIT -ge 30 ]]; do
        sleep 1
        ((OVS_WAIT++))
    done
    for candidate in /run/openvswitch/db.sock /var/run/openvswitch/db.sock; do
        if [[ -S "$candidate" ]]; then OVS_SOCKET="$candidate"; break; fi
    done
    if [[ -z "$OVS_SOCKET" ]]; then
        die "ovsdb-server socket still not present after 30 s. Cannot run netplan apply."
    fi
    log "ovsdb-server socket available at ${OVS_SOCKET}"
else
    log "ovsdb-server socket already present at ${OVS_SOCKET}"
fi

# ---------------------------------------------------------------------------
# 6. OVS BRIDGE + NETPLAN
# ---------------------------------------------------------------------------

log "--- Section 6: OVS bridge + netplan ---"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NETPLAN_SRC="${SCRIPT_DIR}/netplan/01-ovsbr0.yaml"
NETPLAN_DEST="/etc/netplan/01-ovsbr0.yaml"

if [[ ! -f "$NETPLAN_SRC" ]]; then
    die "Netplan source file not found: ${NETPLAN_SRC}"
fi

# Copy only if different (idempotent)
if ! cmp -s "$NETPLAN_SRC" "$NETPLAN_DEST" 2>/dev/null; then
    log "Copying ${NETPLAN_SRC} → ${NETPLAN_DEST}"
    cp "$NETPLAN_SRC" "$NETPLAN_DEST"
    chmod 600 "$NETPLAN_DEST"
else
    log "${NETPLAN_DEST} is already up to date"
fi

log "Running: netplan apply"
netplan apply
log "netplan apply completed"

wait_for_iface ovsbr0 30

# ---------------------------------------------------------------------------
# 7. OVS ATTACH PORTS
# ---------------------------------------------------------------------------

log "--- Section 7: ovs-attach-ports ---"

OVS_SCRIPT_SYSTEM="/etc/dinit.d/scripts/ovs-attach-ports.sh"
OVS_SCRIPT_REPO="${SCRIPT_DIR}/dinit/scripts/ovs-attach-ports.sh"

if [[ -x "$OVS_SCRIPT_SYSTEM" ]]; then
    OVS_SCRIPT="$OVS_SCRIPT_SYSTEM"
elif [[ -x "$OVS_SCRIPT_REPO" ]]; then
    OVS_SCRIPT="$OVS_SCRIPT_REPO"
else
    die "ovs-attach-ports.sh not found at ${OVS_SCRIPT_SYSTEM} or ${OVS_SCRIPT_REPO}. Install the script before running."
fi

log "Running: ${OVS_SCRIPT}"
OVS_EXIT=0
"$OVS_SCRIPT" || OVS_EXIT=$?
if [[ $OVS_EXIT -ne 0 ]]; then
    die "ovs-attach-ports.sh failed (exit ${OVS_EXIT})"
fi
log "ovs-attach-ports completed successfully"

# ---------------------------------------------------------------------------
# 8. NEXTDNS PROFILE DISCOVERY
# ---------------------------------------------------------------------------

log "--- Section 8: NextDNS profile discovery ---"

discover_nextdns_profile() {
    local profile=""

    # Method 1: nextdns config show inside the services container
    profile=$(incus exec services -- nextdns config show 2>/dev/null \
        | awk -F': ' '$1 == "Profile" { print $2; exit }' || true)
    if [[ -n "$profile" ]]; then
        log "NextDNS profile discovered via 'nextdns config show': ${profile}" >&2
        echo "$profile"
        return 0
    fi

    # Method 2: read /etc/nextdns.conf in the services container
    profile=$(incus exec services -- cat /etc/nextdns.conf 2>/dev/null \
        | awk '$1 == "profile" { print $2; exit }' || true)
    if [[ -n "$profile" ]]; then
        log "NextDNS profile discovered via /etc/nextdns.conf in container: ${profile}" >&2
        echo "$profile"
        return 0
    fi

    # Method 3: host-managed config written by a previous run
    if [[ -f /etc/nextdns/nextdns.conf ]]; then
        profile=$(awk '$1 == "profile" { print $2; exit }' /etc/nextdns/nextdns.conf || true)
        if [[ -n "$profile" ]]; then
            log "NextDNS profile discovered via host /etc/nextdns/nextdns.conf: ${profile}" >&2
            echo "$profile"
            return 0
        fi
    fi

    # Method 4: NextDNS API — requires NEXTDNS_API_KEY in environment
    if [[ -n "${NEXTDNS_API_KEY:-}" ]]; then
        profile=$(curl -sf -H "X-Api-Key: ${NEXTDNS_API_KEY}" \
            https://api.nextdns.io/profiles \
            | jq -r '.data[0].id // empty' || true)
        if [[ -n "$profile" ]]; then
            log "NextDNS profile discovered via API: ${profile}" >&2
            echo "$profile"
            return 0
        fi
        die "NextDNS API key provided but API returned no profiles. Check NEXTDNS_API_KEY."
    fi

    die "Cannot discover NextDNS profile ID. Options:
  1. Set NEXTDNS_API_KEY environment variable before running this script.
  2. Pre-configure nextdns in the services container (/etc/nextdns.conf).
  3. Run: NEXTDNS_API_KEY=<your-key> sudo -E $0"
}

# Ensure the services container exists and is running before discovery
# (section 10 creates containers, but we need services up for profile discovery —
# so we do a pre-check and start it here, full idempotent creation happens below)
if ! incus info services &>/dev/null; then
    log "services container does not exist yet — will create in section 10; skipping discovery pre-check"
    NEXTDNS_PROFILE=""
else
    NEXTDNS_PROFILE="$(discover_nextdns_profile)"
fi

# ---------------------------------------------------------------------------
# 9. WRITE HOST-MANAGED CONFIG FILES
# (Deferred until after profile is known; may be written again after section 10)
# ---------------------------------------------------------------------------

write_nextdns_conf() {
    local profile=$1

    log "--- Section 9: Writing host-managed NextDNS config (profile: ${profile}) ---"

    mkdir -p /etc/nextdns

    cat > /etc/nextdns/nextdns.conf <<EOF
# Managed by deploy-network.sh — do not edit manually
profile ${profile}
listen 10.149.181.188:53
report-client-info yes
hardened-privacy yes
cache-size 10MB
EOF
    log "Wrote /etc/nextdns/nextdns.conf (profile: ${profile})"
}

write_host_resolv() {
    # Called only after NextDNS is verified listening — do not move earlier
    log "Writing /etc/resolv.conf (NextDNS verified)"
    cat > /etc/resolv.conf <<EOF
# Managed by deploy-network.sh — do not edit manually
# Primary: NextDNS via services container (verified listening)
nameserver 10.149.181.188
# Fallback: if NextDNS is down
nameserver 1.1.1.1
nameserver 9.9.9.9
EOF
    log "Wrote /etc/resolv.conf (primary: 10.149.181.188, fallbacks: 1.1.1.1 9.9.9.9)"
}

# ---------------------------------------------------------------------------
# 10. INCUS CONTAINERS
# ---------------------------------------------------------------------------

log "--- Section 10: Incus containers ---"

ensure_container_config() {
    local name=$1

    ensure_container_config_value() {
        local config_name=$1
        local config_value=$2
        local current_value

        current_value="$(incus config get "$name" "$config_name" 2>/dev/null || true)"
        if [[ "$current_value" != "$config_value" ]]; then
            log "Setting ${config_name}=${config_value} on ${name}"
            incus config set "$name" "$config_name" "$config_value"
        fi
    }

    case "$name" in
        services)
            ensure_container_config_value security.privileged true
            ensure_container_config_value security.nesting true
            ;;
        privacy-xray-ingress)
            ensure_container_config_value security.privileged true
            ensure_container_config_value linux.kernel_modules wireguard
            ;;
        xray-server)
            ensure_container_config_value security.privileged true
            ;;
    esac
}

ensure_proxy_device() {
    local container=$1
    local device_name=$2
    local listen=$3
    local connect=$4
    local current_type
    local current_listen
    local current_connect

    current_type="$(incus config device get "$container" "$device_name" type 2>/dev/null || true)"
    current_listen="$(incus config device get "$container" "$device_name" listen 2>/dev/null || true)"
    current_connect="$(incus config device get "$container" "$device_name" connect 2>/dev/null || true)"

    if [[ "$current_type" == "proxy" && "$current_listen" == "$listen" && "$current_connect" == "$connect" ]]; then
        log "Proxy device ${container}/${device_name} already present (${listen} -> ${connect})"
        return
    fi

    incus config device remove "$container" "$device_name" 2>/dev/null || true
    incus config device add "$container" "$device_name" proxy \
        listen="$listen" \
        connect="$connect"
    log "Proxy device ${container}/${device_name} configured (${listen} -> ${connect})"
}

ensure_container() {
    local name=$1
    local ip=$2

    if ! incus info "$name" &>/dev/null; then
        log "Creating container ${name} (IP: ${ip})..."
        incus init images:debian/trixie "$name"
        ensure_container_config "$name"

        # Assign static IP. If the device override fails (e.g., NIC not named
        # eth0 in the profile), fall back to letting Incus/DHCP assign it.
        if incus config device override "$name" eth0 \
                ipv4.address="$ip" 2>/dev/null; then
            log "Static IP ${ip} assigned to ${name} eth0"
        else
            # Try adding a new NIC device explicitly
            log "WARNING: device override failed — attempting explicit NIC add for ${name}"
            incus config device add "$name" eth0 nic \
                nictype=bridged \
                parent=incusbr0 \
                ipv4.address="$ip" 2>/dev/null \
                || log "WARNING: could not set static IP for ${name} — Incus will assign via DHCP"
        fi

        incus start "$name"
        wait_for_container "$name" 60
        log "Container ${name} created and Running"
    else
        log "Container ${name} already exists"
        local status
        status="$(incus info "$name" 2>/dev/null | awk -F': ' '$1 == "Status" { print toupper($2); exit }' || true)"
        if [[ "$status" != "RUNNING" ]]; then
            ensure_container_config "$name"
            log "Starting container ${name} (current status: ${status})..."
            incus start "$name"
            wait_for_container "$name" 60
        else
            log "Container ${name} is already Running"
        fi
    fi
}

# Track state for the summary
SERVICES_STATE="UNKNOWN"
PRIV_XRAY_STATE="UNKNOWN"
XRAY_SERVER_STATE="SKIPPED"

ensure_container services              10.149.181.188
ensure_proxy_device services smtp25 tcp:0.0.0.0:25 tcp:10.149.181.188:25
SERVICES_STATE="RUNNING"

ensure_container privacy-xray-ingress  10.149.181.167
PRIV_XRAY_STATE="RUNNING"

if [[ "$EXCLUDE_XRAY_SERVER" == "false" ]]; then
    ensure_container xray-server 10.149.181.100
    XRAY_SERVER_STATE="RUNNING"
else
    log "Skipping xray-server container (--exclude-xray-server)"
    XRAY_SERVER_STATE="SKIPPED"
fi

# Now that services container is guaranteed to exist, discover profile if we
# haven't yet.
if [[ -z "${NEXTDNS_PROFILE:-}" ]]; then
    NEXTDNS_PROFILE="$(discover_nextdns_profile)"
fi

# Write NextDNS config now that we have the profile and the container exists
write_nextdns_conf "$NEXTDNS_PROFILE"

# ---------------------------------------------------------------------------
# 11. MOUNT CONFIG FILES INTO SERVICES CONTAINER
# ---------------------------------------------------------------------------

log "--- Section 11: Mounting config files into services container ---"

mount_device() {
    local container=$1
    local device_name=$2
    local source=$3
    local path=$4
    local existing_device

    while IFS= read -r existing_device; do
        [[ -z "$existing_device" ]] && continue
        incus config device remove "$container" "$existing_device" 2>/dev/null || true
    done < <(
        incus config device show "$container" 2>/dev/null \
            | awk -v target_path="$path" '
                /^[^[:space:]][^:]*:$/ {
                    device=$1
                    sub(/:$/, "", device)
                }
                $1 == "path:" && $2 == target_path { print device }
            '
    )

    # Remove existing device with the target name (force refresh — idempotent)
    incus config device remove "$container" "$device_name" 2>/dev/null || true

    # Add fresh mount
    incus config device add "$container" "$device_name" disk \
        source="$source" \
        path="$path"
    log "Mounted ${source} → ${container}:${path}"
}

mount_device services nextdns-conf /etc/nextdns/nextdns.conf /etc/nextdns.conf
mount_device services resolv-conf  /etc/resolv.conf          /etc/resolv.conf

# ---------------------------------------------------------------------------
# 12. DISABLE SYSTEMD-RESOLVED IN SERVICES CONTAINER
# ---------------------------------------------------------------------------

log "--- Section 12: Disable systemd-resolved in services container ---"

incus exec services -- systemctl disable --now systemd-resolved 2>/dev/null || true
# Remove resolv.conf symlink (typically → /run/systemd/resolve/stub-resolv.conf)
incus exec services -- bash -c 'rm -f /etc/resolv.conf && echo "removed /etc/resolv.conf symlink"' 2>/dev/null || true
log "systemd-resolved disabled in services container"

# ---------------------------------------------------------------------------
# 13. CONFIGURE INCUSBR0 DNS
# ---------------------------------------------------------------------------

log "--- Section 13: incusbr0 DNS nameserver ---"

incus network set incusbr0 dns.nameservers=10.149.181.188
log "incusbr0 DNS nameserver set to 10.149.181.188"

# ---------------------------------------------------------------------------
# 14. INSTALL AND START NEXTDNS IN SERVICES CONTAINER
# ---------------------------------------------------------------------------

log "--- Section 14: Install and start NextDNS in services container ---"

if ! incus exec services -- which nextdns &>/dev/null; then
    log "Installing nextdns in services container..."
    incus exec services -- bash -c "curl -sf https://nextdns.io/install | sh"
    log "nextdns installation complete"
else
    log "nextdns already installed in services container"
fi

# Restart nextdns (uses the host-managed config now mounted at /etc/nextdns.conf)
log "Restarting nextdns in services container..."
if incus exec services -- systemctl restart nextdns 2>/dev/null; then
    log "nextdns restarted via systemctl"
elif incus exec services -- service nextdns restart 2>/dev/null; then
    log "nextdns restarted via service"
else
    # nextdns CLI start as fallback
    incus exec services -- nextdns start \
        || die "All methods to start nextdns in services container failed"
    log "nextdns started via nextdns CLI"
fi

# ---------------------------------------------------------------------------
# 15. VERIFY NEXTDNS IS LISTENING
# ---------------------------------------------------------------------------

log "--- Section 15: Verify NextDNS is listening on 10.149.181.188:53 ---"

verify_dns_listening() {
    local attempt=0
    local max=15
    while [[ $attempt -lt $max ]]; do
        if incus exec services -- ss -lnup 2>/dev/null | grep -q '10.149.181.188:53'; then
            log "NextDNS is listening on 10.149.181.188:53"
            return 0
        fi
        log "Waiting for NextDNS to bind 10.149.181.188:53 (attempt $((attempt+1))/${max})..."
        sleep 2
        ((attempt++))
    done
    die "NextDNS is NOT listening on 10.149.181.188:53 after ${max} attempts.
Diagnose with: incus exec services -- journalctl -u nextdns"
}

verify_dns_listening

# ---------------------------------------------------------------------------
# 16. VERIFY DNS QUERY SUCCESS
# ---------------------------------------------------------------------------

log "--- Section 16: DNS query verification ---"

DNS_TEST_RESULT="FAIL"
if dig @10.149.181.188 google.com +short +timeout=5 &>/dev/null; then
    log "DNS query to 10.149.181.188 (google.com) succeeded"
    DNS_TEST_RESULT="PASS"
    # Only update host resolver after DNS is confirmed working
    write_host_resolv
else
    die "DNS query to 10.149.181.188 failed. NextDNS may be running but not forwarding.
Check: incus exec services -- nextdns status
Check: incus exec services -- journalctl -u nextdns -n 50"
fi

# ---------------------------------------------------------------------------
# 17. XRAY-CLIENT START
# ---------------------------------------------------------------------------

log "--- Section 17: xray-client ---"

XRAY_CLIENT_STATE="NOT CONFIGURED"

if [[ -f /etc/xray/client.json ]]; then
    if restart_service_via_dinit_dbus xray-client 2>/dev/null; then
        log "xray-client restarted via dinit D-Bus"
        XRAY_CLIENT_STATE="RUNNING (restarted)"
    elif start_service_via_dinit_dbus xray-client 2>/dev/null; then
        log "xray-client started via dinit D-Bus"
        XRAY_CLIENT_STATE="RUNNING (started)"
    else
        log "WARNING: could not start xray-client via dinit D-Bus"
        XRAY_CLIENT_STATE="WARNING: could not start"
    fi
else
    log "WARNING: /etc/xray/client.json not found — skipping xray-client start"
    XRAY_CLIENT_STATE="SKIPPED (no client.json)"
fi

# ---------------------------------------------------------------------------
# 18. SUMMARY
# ---------------------------------------------------------------------------

log "--- Section 18: Deploy Summary ---"

# Collect live state for the summary
WGCF_STATUS="DOWN"
ip link show wgcf &>/dev/null && WGCF_STATUS="UP (wgcf interface present)"

OVSBR0_STATUS="DOWN"
ip link show ovsbr0 &>/dev/null && OVSBR0_STATUS="UP (10.88.88.1/24)"

cat <<EOF

=== DEPLOY SUMMARY ===
wgcf tunnel:          ${WGCF_STATUS}
ovsbr0 bridge:        ${OVSBR0_STATUS}
services container:   ${SERVICES_STATE} (10.149.181.188)
NextDNS profile:      ${NEXTDNS_PROFILE}
NextDNS listening:    10.149.181.188:53
DNS test (google.com): ${DNS_TEST_RESULT}
incusbr0 DNS:         10.149.181.188
Host resolv.conf:     10.149.181.188 (primary), 1.1.1.1 (fallback)
privacy-xray-ingress: ${PRIV_XRAY_STATE}
xray-server:          ${XRAY_SERVER_STATE}
xray-client:          ${XRAY_CLIENT_STATE}
======================
EOF

log "deploy-network.sh completed successfully."
