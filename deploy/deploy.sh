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
)

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------

EXCLUDE_XRAY_SERVER=false
TARGET=""

for arg in "$@"; do
    case "$arg" in
        --exclude-xray-server)
            EXCLUDE_XRAY_SERVER=true
            ;;
        --help)
            echo "Usage: sudo $0 [--exclude-xray-server] [SERVICE|all]"
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
    rm -f /usr/local/sbin/op-networkd-dinit.sh   # stale

    # --- Dinit service definitions (network boot chain) ---
    install -m 0644 "${DEPLOY_DIR}/dinit/wg-quick-all"       "${SERVICE_DIR}/wg-quick-all"
    install -m 0644 "${DEPLOY_DIR}/dinit/systemd-networkd"   "${SERVICE_DIR}/systemd-networkd"
    install -m 0644 "${DEPLOY_DIR}/dinit/op-ovsdb-seed"      "${SERVICE_DIR}/op-ovsdb-seed"
    install -m 0644 "${DEPLOY_DIR}/dinit/netplan-apply"      "${SERVICE_DIR}/netplan-apply"
    install -m 0644 "${DEPLOY_DIR}/dinit/ovs-attach-ports"   "${SERVICE_DIR}/ovs-attach-ports"
    install -m 0644 "${DEPLOY_DIR}/dinit/xray-client"        "${SERVICE_DIR}/xray-client"

    # --- Dinit service definitions (app chain — installed now, started after network verify) ---
    install -m 0644 "${DEPLOY_DIR}/dinit/op-session-bus"     "${SERVICE_DIR}/op-session-bus"
    install -m 0644 "${DEPLOY_DIR}/dinit/op-ovsdb-bridge"    "${SERVICE_DIR}/op-ovsdb-bridge"

    # --- Dinit scripts ---
    install -m 0755 "${DEPLOY_DIR}/dinit/scripts/ovs-attach-ports.sh"    "${SERVICE_DIR}/scripts/ovs-attach-ports.sh"
    install -m 0755 "${DEPLOY_DIR}/dinit/op-ovsdb-seed.sh"               "${SERVICE_DIR}/scripts/op-ovsdb-seed.sh"
    install -m 0755 "${DEPLOY_DIR}/dinit/op-ovsdb-bridge-start.sh"       "${SERVICE_DIR}/scripts/op-ovsdb-bridge-start.sh"

    # --- systemd-networkd units (ens3 DHCP standalone) ---
    if [[ -d "${DEPLOY_DIR}/systemd/networkd" ]]; then
        find "${DEPLOY_DIR}/systemd/networkd" -maxdepth 1 -type f | while read -r f; do
            install -m 0644 "$f" "/etc/systemd/network/$(basename "$f")"
        done
        log "Installed systemd-networkd units"
    else
        warn "No systemd/networkd units found in deploy dir — skipping"
    fi

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
        while IFS= read -r line; do
            [[ "$line" =~ ^#.*$ || -z "$line" ]] && continue
            key="${line%%=*}"
            if ! grep -q "^${key}=" /etc/op-dbus/environment 2>/dev/null; then
                echo "$line" >> /etc/op-dbus/environment
                log "Added ${key} to /etc/op-dbus/environment"
            fi
        done < "${DEPLOY_DIR}/environment.default"
    fi

    # --- Remove stale services ---
    rm -f "${SERVICE_DIR}/wgcf" "${SERVICE_DIR}/boot.d/wgcf"      # old wg-quick-era name
    rm -f "${SERVICE_DIR}/boot.d/stalwart" "${SERVICE_DIR}/stalwart"
    $DINITCTL stop stalwart >/dev/null 2>&1 || true

    # --- Enable network boot chain ---
    enable_boot wg-quick-all
    enable_boot systemd-networkd
    enable_boot op-ovsdb-seed
    enable_boot netplan-apply
    enable_boot ovs-attach-ports
    enable_boot xray-client
    enable_boot op-session-bus
    enable_boot op-ovsdb-bridge

    log "System file artifacts installed."
}

# ---------------------------------------------------------------------------
# STEP 2: Network bootstrap
# ---------------------------------------------------------------------------

run_network_bootstrap() {
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
    local cargo_target="/var/cache/op-dbus-build"
    mkdir -p "$cargo_target"
    chown "${build_user:-root}:${build_user:-root}" "$cargo_target" 2>/dev/null || true

    if [[ "$EUID" -eq 0 && -n "$build_user" && "$build_user" != "root" ]]; then
        su -l "$build_user" -c \
            "cd '${PROJECT_ROOT}' && CARGO_TARGET_DIR='${cargo_target}' cargo build --release -p '${crate}'" \
            || error "Build failed for ${crate}"
    else
        CARGO_TARGET_DIR="$cargo_target" cargo build --release -p "$crate" \
            || error "Build failed for ${crate}"
    fi

    local staged="${INSTALL_DIR}/${binary}.new.$$"
    install -m 755 "${cargo_target}/release/${binary}" "$staged"
    mv -f "$staged" "${INSTALL_DIR}/${binary}"
    log "Installed ${binary} → ${INSTALL_DIR}/${binary}"
}
#
# generate_service_file() {
#     local binary=$1 service=$2
#     local file="${SERVICE_DIR}/${service}"
#
#     log "Generating dinit service for ${service}..."
#     case "$service" in
#         op-dbus)
#             cat > "$file" <<EOF
# type = process
# command = /usr/local/sbin/op-dbus-dinit.sh
# log-type = buffer
# smooth-recovery = true
# depends-on = op-session-bus
# EOF
#             ;;
#         op-web)
#             cat > "$file" <<EOF
# type = process
# command = /usr/local/sbin/op-web-dinit.sh
# log-type = buffer
# smooth-recovery = true
# depends-on = op-dbus
# depends-on = op-ovsdb-bridge
# EOF
#             ;;
#         op-services|op-chat)
#             cat > "$file" <<EOF
# type = process
# command = ${INSTALL_DIR}/${binary}
# log-type = buffer
# smooth-recovery = true
# depends-on = op-web
# EOF
#             ;;
#         *)
#             cat > "$file" <<EOF
# type = process
# command = ${INSTALL_DIR}/${binary}
# log-type = buffer
# smooth-recovery = true
# EOF
#             ;;
#     esac
#
#     if [[ "$EUID" -ne 0 ]]; then
#         local DATA_DIR="${PROJECT_ROOT}/deploy/data"
#         mkdir -p "${DATA_DIR}/cache"
#         cat >> "$file" <<EOF
# env = OP_DBUS_DATABASE_URL=sqlite://${DATA_DIR}/state.db
# env = OP_DBUS_CACHE_DIR=${DATA_DIR}/cache
# env = OP_DBUS_WEB_PORT=8081
# env = OP_DBUS_SESSION_BUS=1
# EOF
    fi
# }
#
# deploy_service() {
#     local crate=$1 binary=$2 service=$3
#     local stopped_dependents=()
#
#     was_stopped() {
#         local t="$1" e
#         for e in "${stopped_dependents[@]}"; do [[ "$e" == "$t" ]] && return 0; done
#         return 1
#     }
#
#     build_and_install "$crate" "$binary"
#     generate_service_file "$binary" "$service"
#     [[ "$EUID" -eq 0 ]] && enable_boot "$service"
#
#     case "$service" in
#         op-dbus)
#             for dep in op-chat op-services op-web systemd-networkd op-ovsdb-bridge ovs-attach-ports; do
#                 if is_started "$dep"; then
#                     $DINITCTL stop "$dep"; stopped_dependents+=("$dep")
#                 fi
#             done ;;
#         op-web)
#             for dep in op-chat op-services; do
#                 if is_started "$dep"; then
#                     $DINITCTL stop "$dep"; stopped_dependents+=("$dep")
#                 fi
#             done ;;
#     esac
#
#     if is_started "$service"; then
#         $DINITCTL restart "$service"
#     else
#         $DINITCTL start "$service"
    fi
#
#     if [[ "${#stopped_dependents[@]}" -gt 0 ]]; then
#         for dep in ovs-attach-ports op-ovsdb-bridge systemd-networkd op-web op-services op-chat; do
#             was_stopped "$dep" && $DINITCTL start "$dep"
#         done
    fi
#     log "✅ ${service} deployed"
# }

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

command -v cargo    >/dev/null || warn "cargo not found — service builds will fail"
command -v dinitctl >/dev/null || warn "dinitctl not found"

install_system_files
run_network_bootstrap

# --- App services: uncomment after network is verified on VPS ---
command -v cargo >/dev/null || error "Cargo not found — cannot build services"
for entry in "${SERVICES[@]}"; do
    IFS=':' read -r crate binary service <<< "$entry"
    if [[ -z "$TARGET" || "$TARGET" == "all" || "$TARGET" == "$crate" ]]; then
        deploy_service "$crate" "$binary" "$service"
    fi
done

log "Deployment complete."
