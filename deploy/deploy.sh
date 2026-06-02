#!/bin/bash
# deploy/deploy.sh — Operation D-Bus system deployment (s6-rc)
#
# Usage:
#   sudo ./deploy/deploy.sh [--skip-network] [SERVICE|all]
#
# Services: op-dbus, op-web-server, op-chat, op-services, op-projection, op-dbus-mirror

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEPLOY_DIR="${PROJECT_ROOT}/deploy"
INSTALL_BIN="/usr/local/bin"
S6_SV="/etc/s6/sv"
S6_DB="/etc/s6/db"

SERVICES=(
    "op-web:op-dbus:op-dbus"
    "op-web:op-web-server:op-web-server"
    "op-chat:op-chat:chatmanager"
    "op-dbus-mirror:ovs-dbus-init:op-dbus-mirror"
    "op-projection:projection_server:op-projection"
    "op-cognitive-mcp:op-cognitive-mcp:op-cognitive-mcp"
)

# ---------------------------------------------------------------------------
# Args
# ---------------------------------------------------------------------------

SKIP_NETWORK=false
TARGET=""

for arg in "$@"; do
    case "$arg" in
        --skip-network) SKIP_NETWORK=true ;;
        --help) echo "Usage: sudo $0 [--skip-network] [SERVICE|all]"; exit 0 ;;
        -*) echo "Unknown flag: $arg" >&2; exit 1 ;;
        *) TARGET="$arg" ;;
    esac
done

[[ "$EUID" -eq 0 ]] || { echo "Run with sudo." >&2; exit 1; }

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------

log()   { echo -e "\033[0;32m[DEPLOY]\033[0m $*"; }
warn()  { echo -e "\033[1;33m[WARN]\033[0m $*"; }
error() { echo -e "\033[0;31m[ERROR]\033[0m $*"; exit 1; }

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

append_env_if_missing() {
    local file="$1" key="$2" value="$3"
    grep -q "^${key}=" "$file" 2>/dev/null || printf '%s=%s\n' "$key" "$value" >> "$file"
}

s6_restart() {
    local svc="$1"
    if [[ -d "/run/service/${svc}" ]]; then
        s6-svc -r "/run/service/${svc}" && log "Restarted s6 service: ${svc}"
    else
        warn "s6 service not running: ${svc}"
    fi
}

# ---------------------------------------------------------------------------
# STEP 1: System files
# ---------------------------------------------------------------------------

install_system_files() {
    log "Installing system files..."

    # D-Bus policy
    install -d /etc/dbus-1/system.d
    install -m 0644 "${DEPLOY_DIR}/dbus/org.opdbus.conf"  /etc/dbus-1/system.d/org.opdbus.conf
    install -m 0644 "${DEPLOY_DIR}/dbus/org.dbusmcp.conf" /etc/dbus-1/system.d/org.dbusmcp.conf
    dbus-send --system --type=method_call \
        --dest=org.freedesktop.DBus /org/freedesktop/DBus \
        org.freedesktop.DBus.ReloadConfig 2>/dev/null || true
    log "Installed D-Bus policy"

    # s6 service definitions
    for svc_dir in "${DEPLOY_DIR}/s6"/*/; do
        svc="$(basename "$svc_dir")"
        if [ ! -f "${svc_dir}/type" ]; then
            continue
        fi
        install -d "${S6_SV}/${svc}"
        cp -a "${svc_dir}/." "${S6_SV}/${svc}/"
        chmod 0755 "${S6_SV}/${svc}/run" 2>/dev/null || true
        chmod 0755 "${S6_SV}/${svc}/up" 2>/dev/null || true
        chmod 0755 "${S6_SV}/${svc}/shell_up" 2>/dev/null || true
        log "Installed s6 service: ${svc}"
    done

    # systemd-networkd units
    if [[ -d "${DEPLOY_DIR}/systemd/networkd" ]]; then
        install -d /etc/systemd/network
        find "${DEPLOY_DIR}/systemd/networkd" -maxdepth 1 -type f | while read -r f; do
            install -m 0644 "$f" "/etc/systemd/network/$(basename "$f")"
        done
        log "Installed systemd-networkd units"
    fi

    # Netplan
    install -d /etc/netplan
    install -m 0600 "${DEPLOY_DIR}/netplan/01-ovsbr0.yaml" /etc/netplan/01-ovsbr0.yaml
    log "Installed /etc/netplan/01-ovsbr0.yaml"

    # wgcf — never overwrite the live key
    if [[ -f /etc/wireguard/wgcf.conf ]]; then
        log "/etc/wireguard/wgcf.conf present — not overwriting"
    else
        warn "/etc/wireguard/wgcf.conf not found. Skipping WARP checks."
    fi

    # App environment
    install -d /etc/op-dbus
    if [[ ! -f /etc/op-dbus/environment ]]; then
        install -m 0640 "${DEPLOY_DIR}/environment.default" /etc/op-dbus/environment
        log "Installed /etc/op-dbus/environment"
    else
        # Merge: add keys from default that aren't already present
        while IFS= read -r line; do
            [[ "$line" =~ ^[[:space:]]*# ]] && continue
            [[ -z "$line" ]] && continue
            key="${line%%=*}"
            grep -q "^${key}=" /etc/op-dbus/environment 2>/dev/null \
                || { echo "$line" >> /etc/op-dbus/environment; log "Added ${key} to environment"; }
        done < "${DEPLOY_DIR}/environment.default"
    fi

    # Ensure runtime dirs exist
    local state_dir
    state_dir="$(awk -F= '$1=="OP_DBUS_STATE_DIR"{print $2;exit}' /etc/op-dbus/environment)"
    state_dir="${state_dir:-/var/lib/op-dbus}"
    install -d -m 0750 "${state_dir}/cache" /run/op-dbus
    append_env_if_missing /etc/op-dbus/environment "OP_DBUS_CACHE_DIR" "${state_dir}/cache"

    log "System files installed."
}

# ---------------------------------------------------------------------------
# STEP 2: Network bootstrap
# ---------------------------------------------------------------------------

run_network_bootstrap() {
    [[ "$SKIP_NETWORK" == "true" ]] && { log "Skipping network bootstrap."; return 0; }
    local script="${DEPLOY_DIR}/deploy-network.sh"
    [[ -x "$script" ]] || error "${script} not found or not executable"
    log "Running network bootstrap..."
    "$script"
}

# ---------------------------------------------------------------------------
# STEP 3: Build + install service binaries
# ---------------------------------------------------------------------------

build_and_install() {
    local crate="$1" binary="$2"
    log "Building ${binary}..."
    cargo build --release -p "$crate" --bin "$binary" \
        || error "Build failed: ${binary}"
    install -m 0755 "${PROJECT_ROOT}/target/release/${binary}" "${INSTALL_BIN}/${binary}"
    log "Installed ${binary} → ${INSTALL_BIN}/${binary}"
}

deploy_service() {
    local crate="$1" binary="$2" service="$3"
    build_and_install "$crate" "$binary"
    s6_restart "$service"
    log "✅ ${service} deployed"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

command -v cargo >/dev/null || warn "cargo not found — service builds will fail"
command -v s6-svc >/dev/null || warn "s6-svc not found"

install_system_files
run_network_bootstrap

for entry in "${SERVICES[@]}"; do
    IFS=':' read -r crate binary service <<< "$entry"
    if [[ -z "$TARGET" || "$TARGET" == "all" || "$TARGET" == "$service" || "$TARGET" == "$crate" ]]; then
        deploy_service "$crate" "$binary" "$service"
    fi
done

log "Deployment complete."
