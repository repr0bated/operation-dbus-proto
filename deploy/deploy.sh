#!/bin/bash
# deploy/deploy.sh
# Operation D-Bus — full system deployment
#
# Order of operations:
#   1. Install all system file artifacts (dinit services, scripts, netplan)
#   2. Bootstrap network via deploy-network.sh (wgcf → OVS → Incus → NextDNS)
#   3. [PENDING NETWORK VERIFICATION] Build and deploy app services
#
# Usage:
#   sudo ./deploy/deploy.sh [--exclude-xray-server] [SERVICE|all]

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEPLOY_DIR="${PROJECT_ROOT}/deploy"

# App service components: "crate_name:binary_name:service_name"
SERVICES=(
    "op-web:op-dbus:op-dbus"        # gRPC server (op-grpc-bridge), binary in op-web crate
    "op-web:op-web-server:op-web"   # HTTP/WS server
    "op-services:op-services:op-services"
    "op-chat:op-chat:op-chat"
    "op-projection:projection_server:op-projection"
)

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

EXCLUDE_XRAY_SERVER=false
TARGET=""
SKIP_NETWORK=false

for arg in "$@"; do
    case "$arg" in
        --exclude-xray-server)
            EXCLUDE_XRAY_SERVER=true
            ;;
        --skip-network)
            SKIP_NETWORK=true
            ;;
        --help)
            echo "Usage: sudo $0 [--exclude-xray-server] [--skip-network] [SERVICE|all]"
            exit 0
            ;;
        -*)
            echo "Unknown flag: $arg" >&2
            exit 1
            ;;
        *)
            TARGET="$arg"
            ;;
    esac
done

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------

log()   { echo -e "\033[0;32m[DEPLOY]\033[0m $*"; }
warn()  { echo -e "\033[1;33m[WARN]\033[0m $*"; }
error() { echo -e "\033[0;31m[ERROR]\033[0m $*"; exit 1; }

# ---------------------------------------------------------------------------
# Mode detection (system vs user)
# ---------------------------------------------------------------------------

if [[ "$EUID" -ne 0 ]]; then
    log "Running in USER mode (installing to ${PROJECT_ROOT}/deploy/bin)"
    INSTALL_DIR="${PROJECT_ROOT}/deploy/bin"
    SERVICE_DIR="${PROJECT_ROOT}/deploy/services"
    SOCKET_PATH="${PROJECT_ROOT}/deploy/dinit.socket"
    DINITCTL="dinitctl -p ${SOCKET_PATH}"

    mkdir -p "$INSTALL_DIR" "$SERVICE_DIR"

    if [[ ! -S "$SOCKET_PATH" ]]; then
        log "Starting local dinit instance..."
        echo "type = internal" > "${SERVICE_DIR}/boot"
        dinit --user -p "$SOCKET_PATH" -d "$SERVICE_DIR" \
              -l "${PROJECT_ROOT}/deploy/dinit.log" &
        sleep 5
    fi
else
    log "Running in SYSTEM mode (installing to /usr/local/sbin)"
    INSTALL_DIR="/usr/local/sbin"
    SERVICE_DIR="/etc/dinit.d"
    DINITCTL="dinitctl"
    mkdir -p "$INSTALL_DIR" "$SERVICE_DIR"
fi

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

is_started() { $DINITCTL status "$1" 2>/dev/null | grep -q "State: STARTED"; }

enable_boot() {
    local service="$1"
    mkdir -p "${SERVICE_DIR}/boot.d"
    ln -sfn "../${service}" "${SERVICE_DIR}/boot.d/${service}"
}

append_env_if_missing() {
    local file="$1" key="$2" value="$3"
    if ! grep -q "^${key}=" "$file" 2>/dev/null; then
        printf '%s=%s\n' "$key" "$value" >> "$file"
        log "Added ${key} to ${file}"
    fi
}

read_env_value() {
    local file="$1" key="$2"
    [[ -f "$file" ]] || return 0
    awk -F= -v wanted="$key" '$1 == wanted { print substr($0, index($0, "=") + 1); exit }' "$file"
}

ensure_dhcpcd_ovs_denyinterfaces() {
    local dhcpcd_conf="/etc/dhcpcd.conf"

    [[ -f "$dhcpcd_conf" ]] || return 0

    if ! grep -q '^# op-ovs internal interfaces$' "$dhcpcd_conf"; then
        cat >> "$dhcpcd_conf" <<'EOF'

# op-ovs internal interfaces
# dhcpcd must not claim IPv4LL on OVS-managed or wg-quick-managed links.
denyinterfaces ovsbr0 wgcf grpc-bridge ovsbr0-mgmt ovsbr0-sock priv_wg priv_warp priv_xray
EOF
        log "Updated ${dhcpcd_conf} with OVS/internal interface deny list"
    fi
}

normalize_system_runtime_environment() {
    local env_file="/etc/op-dbus/environment"
    local state_dir cache_dir

    state_dir="$(read_env_value "$env_file" "OP_DBUS_STATE_DIR")"
    [[ -n "$state_dir" ]] || state_dir="/var/lib/op-dbus"

    cache_dir="$(read_env_value "$env_file" "OP_DBUS_CACHE_DIR")"
    [[ -n "$cache_dir" ]] || cache_dir="${state_dir}/cache"

    # Runtime authority is persisted state, not in-memory bootstrap data.
    # Ensure the directories behind the canonical SQLite-backed control-plane
    # store exist before services start writing plugin catalog/state documents.
    install -d -m 0750 "$cache_dir" /run/op-dbus

    append_env_if_missing "$env_file" "OP_DBUS_CACHE_DIR" "$cache_dir"

    # deploy.sh installs and wires the shared session-bus service, so the app
    # environment must explicitly point at it instead of depending on ambient
    # login-session state.
    append_env_if_missing "$env_file" "OP_DBUS_SESSION_BUS" "1"
    append_env_if_missing \
        "$env_file" \
        "DBUS_SESSION_BUS_ADDRESS" \
        "unix:path=/run/op-dbus/session-bus"
}

install_user_support_files() {
    [[ "$EUID" -ne 0 ]] || return 0

    local run_dir="${PROJECT_ROOT}/deploy/run"
    local launcher="${INSTALL_DIR}/op-session-bus"

    mkdir -p "$run_dir" "$INSTALL_DIR" "$SERVICE_DIR"

    cat > "$launcher" <<EOF
#!/bin/sh
set -eu
mkdir -p "${run_dir}"
exec /usr/bin/dbus-daemon --session --nofork --address=unix:path=${run_dir}/session-bus
EOF
    chmod 0755 "$launcher"

    cat > "${SERVICE_DIR}/op-session-bus" <<EOF
type = process
command = ${launcher}
log-type = buffer
smooth-recovery = true
EOF
}

service_command_for() {
    local service="$1" binary="$2"

    if [[ "$EUID" -ne 0 ]]; then
        printf '%s\n' "${INSTALL_DIR}/${binary}"
        return 0
    fi

    case "$service" in
        op-dbus) printf '%s\n' "/usr/local/sbin/op-dbus-dinit.sh" ;;
        op-web) printf '%s\n' "/usr/local/sbin/op-web-dinit.sh" ;;
        *) printf '%s\n' "${INSTALL_DIR}/${binary}" ;;
    esac
}

# ---------------------------------------------------------------------------
# STEP 1: Install all system file artifacts
# ---------------------------------------------------------------------------
# Single source of truth for every file that lives under /etc/dinit.d,
# /etc/netplan, /etc/systemd/network, and /usr/local/sbin.
# deploy-network.sh does NOT create files — it reads what is installed here.
# ---------------------------------------------------------------------------

install_system_files() {
    if [[ "$EUID" -ne 0 ]]; then
        warn "Skipping system file install (non-root). Re-run with sudo for host networking changes."
        return 0
    fi

    log "Installing system file artifacts..."

    install -d "$SERVICE_DIR" \
               "${SERVICE_DIR}/boot.d" \
               "${SERVICE_DIR}/scripts" \
               /etc/systemd/network \
               /etc/netplan \
               /etc/wireguard \
               /usr/local/sbin

    # --- Launcher scripts ---
    install -m 0755 "${DEPLOY_DIR}/dinit/op-session-bus.sh"      /usr/local/sbin/op-session-bus
    install -m 0755 "${DEPLOY_DIR}/dinit/op-dbus-dinit.sh"       /usr/local/sbin/op-dbus-dinit.sh
    install -m 0755 "${DEPLOY_DIR}/dinit/op-web-dinit.sh"        /usr/local/sbin/op-web-dinit.sh
    install -m 0755 "${DEPLOY_DIR}/dinit/op-web-start.sh"        /usr/local/sbin/op-web-start.sh
    rm -f /usr/local/sbin/op-networkd-dinit.sh   # stale

    # --- Dinit service definitions (network boot chain) ---
    install -m 0644 "${DEPLOY_DIR}/dinit/services0-sockets"  "${SERVICE_DIR}/services0-sockets"
    install -m 0644 "${DEPLOY_DIR}/dinit/wg-quick-all"       "${SERVICE_DIR}/wg-quick-all"
    install -m 0644 "${DEPLOY_DIR}/dinit/systemd-networkd"   "${SERVICE_DIR}/systemd-networkd"
    install -m 0644 "${DEPLOY_DIR}/dinit/op-ovs-services"    "${SERVICE_DIR}/op-ovs-services"
    install -m 0644 "${DEPLOY_DIR}/dinit/op-ovsdb-seed"      "${SERVICE_DIR}/op-ovsdb-seed"
    install -m 0644 "${DEPLOY_DIR}/dinit/netplan-apply"      "${SERVICE_DIR}/netplan-apply"
    install -m 0644 "${DEPLOY_DIR}/dinit/ovs-attach-ports"   "${SERVICE_DIR}/ovs-attach-ports"
    install -m 0644 "${DEPLOY_DIR}/dinit/xray-client"        "${SERVICE_DIR}/xray-client"

    # --- Dinit service definitions (app chain — installed now, started after network verify) ---
    install -m 0644 "${DEPLOY_DIR}/dinit/op-session-bus"     "${SERVICE_DIR}/op-session-bus"

    # --- Dinit scripts ---
    install -m 0755 "${DEPLOY_DIR}/dinit/scripts/services0-sockets.sh"   "${SERVICE_DIR}/scripts/services0-sockets.sh"
    install -m 0755 "${DEPLOY_DIR}/dinit/scripts/ovs-attach-ports.sh"    "${SERVICE_DIR}/scripts/ovs-attach-ports.sh"
    install -m 0755 "${DEPLOY_DIR}/dinit/op-ovs-services-start.sh"       "${SERVICE_DIR}/scripts/op-ovs-services-start.sh"
    install -m 0755 "${DEPLOY_DIR}/dinit/op-ovsdb-seed.sh"               "${SERVICE_DIR}/scripts/op-ovsdb-seed.sh"

    # --- systemd-networkd units (ens3 DHCP standalone) ---
    if [[ -d "${DEPLOY_DIR}/systemd/networkd" ]]; then
        find "${DEPLOY_DIR}/systemd/networkd" -maxdepth 1 -type f | while read -r f; do
            install -m 0644 "$f" "/etc/systemd/network/$(basename "$f")"
        done
        log "Installed systemd-networkd units"
    else
        warn "No systemd/networkd units found in deploy dir — skipping"
    fi

    ensure_dhcpcd_ovs_denyinterfaces

    # --- Netplan (OVS bridge) ---
    install -m 0600 "${DEPLOY_DIR}/netplan/01-ovsbr0.yaml" /etc/netplan/01-ovsbr0.yaml
    log "Installed /etc/netplan/01-ovsbr0.yaml"

    # --- wgcf: source of truth is /etc/wireguard/wgcf.conf (real key, premium sub)
    # wgcf-profile.conf in the repo is a template only — do not extract from it.
    if [[ ! -f /etc/wireguard/wgcf.conf ]]; then
        error "/etc/wireguard/wgcf.conf not found. Provision the WARP key before deploying."
    fi
    log "/etc/wireguard/wgcf.conf present — not overwriting"

    # --- Xray client config (install only if not yet present — server config is manual) ---
    if [[ ! -f /etc/xray/client.json ]]; then
        if [[ -f "${DEPLOY_DIR}/xray/client.json" ]]; then
            install -d /etc/xray
            install -m 0600 "${DEPLOY_DIR}/xray/client.json" /etc/xray/client.json
            log "Installed /etc/xray/client.json"
        else
            warn "deploy/xray/client.json not found — skipping xray config install"
        fi
    else
        log "/etc/xray/client.json already present — not overwriting"
    fi

    if [[ -f /etc/xray/client.json ]] && grep -q "REPLACE_WITH_" /etc/xray/client.json; then
        warn "Xray config contains placeholder values - service will not start correctly"
    fi

    # --- App environment (gRPC addr, OpenFlow controller, SMTP, etc.) ---
    install -d /etc/op-dbus
    # Only install if not already present — local overrides (API keys, passwords) are preserved.
    if [[ ! -f /etc/op-dbus/environment ]]; then
        install -m 0640 "${DEPLOY_DIR}/environment.default" /etc/op-dbus/environment
        log "Installed /etc/op-dbus/environment from environment.default"
    else
        # Merge: add keys from default that are not yet in the installed file.
        # Skip git conflict markers if the file was committed mid-merge.
        while IFS= read -r line; do
            [[ "$line" =~ ^#.*$    ]] && continue
            [[ "$line" == "<<<<<<<"*  ]] && continue
            [[ "$line" == "======="*  ]] && continue
            [[ "$line" == ">>>>>>>"*  ]] && continue
            [[ -z "$line"          ]] && continue
            key="${line%%=*}"
            if ! grep -q "^${key}=" /etc/op-dbus/environment 2>/dev/null; then
                echo "$line" >> /etc/op-dbus/environment
                log "Added ${key} to /etc/op-dbus/environment"
            fi
        done < "${DEPLOY_DIR}/environment.default"

        # Strip any conflict markers already in the installed file.
        if grep -qE "^(<<<<<<<|=======|>>>>>>>)" /etc/op-dbus/environment 2>/dev/null; then
            warn "Removing git conflict markers from /etc/op-dbus/environment"
            python3 -c "
import re, sys
content = open('/etc/op-dbus/environment').read()
content = re.sub(r'^(<<<<<<<|=======|>>>>>>>)[^\n]*\n', '', content, flags=re.MULTILINE)
open('/etc/op-dbus/environment', 'w').write(content)
"
        fi
    fi

    normalize_system_runtime_environment

    # --- Remove stale services ---
    rm -f "${SERVICE_DIR}/wgcf" "${SERVICE_DIR}/boot.d/wgcf"      # old wg-quick-era name
    rm -f "${SERVICE_DIR}/boot.d/stalwart" "${SERVICE_DIR}/stalwart"
    rm -f "${SERVICE_DIR}/op-ovsdb-bridge" "${SERVICE_DIR}/boot.d/op-ovsdb-bridge"
    rm -f "${SERVICE_DIR}/scripts/op-ovsdb-bridge-start.sh"
    rm -f "${SERVICE_DIR}/disabled/op-ovsdb-bridge" "${SERVICE_DIR}/disabled/op-ovsdb-seed"
    rm -f "${SERVICE_DIR}/op-web.backup" "${SERVICE_DIR}/op-web.broken" "${SERVICE_DIR}/incus.bak"
    rm -f "${SERVICE_DIR}/scripts/ovs-attach-ports.sh.bak" "${SERVICE_DIR}/scripts/ovs-attach-ports.sh.bak3"
    rm -f "${SERVICE_DIR}"/op-dbus.bak.* "${SERVICE_DIR}"/op-web.bak.* "${SERVICE_DIR}"/networkd.bak.* "${SERVICE_DIR}"/qdrant.bak.*
    $DINITCTL stop stalwart >/dev/null 2>&1 || true

    # --- Enable network boot chain ---
    enable_boot services0-sockets
    enable_boot wg-quick-all
    enable_boot systemd-networkd
    enable_boot op-ovs-services
    enable_boot op-ovsdb-seed
    enable_boot netplan-apply
    enable_boot ovs-attach-ports
    enable_boot xray-client
    enable_boot op-session-bus

    log "System file artifacts installed."
}

# ---------------------------------------------------------------------------
# STEP 2: Network bootstrap
# ---------------------------------------------------------------------------

run_network_bootstrap() {
    if [[ "$SKIP_NETWORK" == "true" ]]; then
        log "Skipping network bootstrap (--skip-network provided)."
        return 0
    fi

    if [[ "$EUID" -ne 0 ]]; then
        warn "Skipping network bootstrap (non-root)."
        return 0
    fi

    local network_script="${DEPLOY_DIR}/deploy-network.sh"
    if [[ ! -x "$network_script" ]]; then
        error "${network_script} not found or not executable."
    fi

    local flags=()
    [[ "$EXCLUDE_XRAY_SERVER" == "true" ]] && flags+=(--exclude-xray-server)

    log "Running network bootstrap: ${network_script} ${flags[*]}"
    "$network_script" "${flags[@]}"
}

# ---------------------------------------------------------------------------
# STEP 3: App service build + deploy
# [COMMENTED OUT — pending network verification]
# Uncomment this section once deploy-network.sh has been verified on the VPS.
# ---------------------------------------------------------------------------

build_and_install() {
    local crate=$1 binary=$2

    log "Building ${crate}..."
    # stat -c "%U" gives owner username on Linux (Chimera). stat -f is BSD only.
    local build_user
    build_user=$(stat -c "%U" "$PROJECT_ROOT" 2>/dev/null || echo "")
    local cargo_target
    if [[ "$EUID" -eq 0 ]]; then
        cargo_target="/var/cache/op-dbus-build"
    else
        cargo_target="${PROJECT_ROOT}/deploy/target"
    fi
    mkdir -p "$cargo_target"
    if [[ "$EUID" -eq 0 ]]; then
        chown "${build_user:-root}:${build_user:-root}" "$cargo_target" 2>/dev/null || true
    fi

    if [[ "$EUID" -eq 0 && -n "$build_user" && "$build_user" != "root" ]]; then
        su -l "$build_user" -c \
            "cd '${PROJECT_ROOT}' && CARGO_TARGET_DIR='${cargo_target}' cargo build --release -p '${crate}' --bin '${binary}'" \
            || error "Build failed for ${crate}"
    else
        CARGO_TARGET_DIR="$cargo_target" cargo build --release -p "$crate" --bin "$binary" \
            || error "Build failed for ${crate}"
    fi

    local staged="${INSTALL_DIR}/${binary}.new.$$"
    install -m 755 "${cargo_target}/release/${binary}" "$staged"
    mv -f "$staged" "${INSTALL_DIR}/${binary}"
    log "Installed ${binary} → ${INSTALL_DIR}/${binary}"
}

generate_service_file() {
    local binary=$1 service=$2
    local file="${SERVICE_DIR}/${service}"
    local command
    command="$(service_command_for "$service" "$binary")"

    log "Generating dinit service for ${service}..."
    case "$service" in
        op-dbus)
            cat > "$file" <<EOF
type = process
command = ${command}
log-type = buffer
smooth-recovery = true
depends-on = op-session-bus
EOF
            ;;
        op-web)
            cat > "$file" <<EOF
type = process
command = /usr/local/sbin/op-web-start.sh
log-type = buffer
smooth-recovery = true
depends-on = op-dbus
EOF
            ;;
        op-services|op-chat)
            cat > "$file" <<EOF
type = process
command = ${INSTALL_DIR}/${binary}
log-type = buffer
smooth-recovery = true
depends-on = op-web
EOF
            ;;
        op-projection)
            cat > "$file" <<EOF
type = process
command = ${INSTALL_DIR}/${binary}
log-type = buffer
smooth-recovery = true
depends-on = op-dbus
EOF
            ;;
        *)
            cat > "$file" <<EOF
type = process
command = ${INSTALL_DIR}/${binary}
log-type = buffer
smooth-recovery = true
EOF
            ;;
    esac

    if [[ "$EUID" -ne 0 ]]; then
        local DATA_DIR="${PROJECT_ROOT}/deploy/data"
        local RUN_DIR="${PROJECT_ROOT}/deploy/run"
        mkdir -p "${DATA_DIR}/cache" "${RUN_DIR}"
        cat >> "$file" <<EOF
env = OP_DBUS_CACHE_DIR=${DATA_DIR}/cache
env = OP_DBUS_WEB_PORT=8081
env = OP_DBUS_SESSION_BUS=1
env = DBUS_SESSION_BUS_ADDRESS=unix:path=${RUN_DIR}/session-bus
EOF
    fi
}

flush_session_bus_names() {
    # Kill any process still holding org.opdbus.* names on the session bus so
    # the new binary can claim them cleanly.  Without this, "name already taken"
    # causes start() to abort before registering /host and /plugins.
    local bus_addr="/run/op-dbus/session-bus"
    [[ -S "$bus_addr" ]] || return 0

    local pids
    pids=$(busctl --address="unix:path=${bus_addr}" list --no-legend 2>/dev/null \
           | awk '/org\.opdbus/ { print $NF }' \
           | grep -v '^-$' | sort -u || true)

    for pid in $pids; do
        [[ "$pid" =~ ^[0-9]+$ ]] || continue
        local comm
        comm=$(cat /proc/"$pid"/comm 2>/dev/null || echo "?")
        log "Killing stale bus holder: pid=${pid} (${comm})"
        kill -TERM "$pid" 2>/dev/null || true
    done

    [[ -n "$pids" ]] && sleep 1 || true
}

deploy_service() {
    local crate=$1 binary=$2 service=$3
    local stopped_dependents=()

    was_stopped() {
        local t="$1" e
        for e in "${stopped_dependents[@]}"; do [[ "$e" == "$t" ]] && return 0; done
        return 1
    }

    build_and_install "$crate" "$binary"
    generate_service_file "$binary" "$service"
    [[ "$EUID" -eq 0 ]] && enable_boot "$service"

    case "$service" in
        op-dbus)
            for dep in op-chat op-services op-web op-projection code-assist-gateway op-of-controller ovs-dbus-init xray-client ovs-attach-ports systemd-networkd; do
                if is_started "$dep"; then
                    $DINITCTL stop "$dep" || true; stopped_dependents+=("$dep")
                fi
            done
            flush_session_bus_names
            ;;
        op-web)
            for dep in op-chat op-services; do
                if is_started "$dep"; then
                    $DINITCTL stop "$dep" || true; stopped_dependents+=("$dep")
                fi
            done ;;
    esac

    if is_started "$service"; then
        $DINITCTL restart "$service" || echo "Failed to restart $service"
    else
        $DINITCTL start "$service" || echo "Failed to start $service"
    fi

    if [[ "${#stopped_dependents[@]}" -gt 0 ]]; then
        for dep in systemd-networkd ovs-attach-ports op-of-controller xray-client op-web op-projection op-services op-chat; do
            if was_stopped "$dep"; then $DINITCTL start "$dep" || echo "Failed to start $dep"; fi
        done
    fi
    log "✅ ${service} deployed"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

command -v cargo    >/dev/null || warn "cargo not found — service builds will fail"
command -v dinitctl >/dev/null || warn "dinitctl not found"

install_system_files
run_network_bootstrap
install_user_support_files

# --- App services: uncomment after network is verified on VPS ---
# command -v cargo >/dev/null || error "Cargo not found — cannot build services"
for entry in "${SERVICES[@]}"; do
    IFS=':' read -r crate binary service <<< "$entry"
    if [[ -z "$TARGET" || "$TARGET" == "all" || "$TARGET" == "$crate" ]]; then
        deploy_service "$crate" "$binary" "$service"
    fi
done

log "Deployment complete."
