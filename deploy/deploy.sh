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
    "op-dbus:op-dbus:op-dbus"       # main control-plane binary (root crate)
    "op-web:op-web-server:op-web"   # HTTP/WS server
    "op-services:op-services:op-services"
    "op-chat:op-chat:op-chat"
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

BUILD_SESSION_ID="$(date +%Y%m%d%H%M%S)"
TARGET_RETENTION_COUNT="${OP_DBUS_TARGET_RETENTION_COUNT:-3}"

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

path_owner_user() {
    local path="$1"

    if stat -c "%U" "$path" >/dev/null 2>&1; then
        stat -c "%U" "$path"
        return 0
    fi

    if stat -f "%Su" "$path" >/dev/null 2>&1; then
        stat -f "%Su" "$path"
        return 0
    fi

    return 1
}

build_cache_root() {
    if [[ "$EUID" -eq 0 ]]; then
        printf '%s\n' "/var/cache/op-dbus-build"
    else
        printf '%s\n' "${PROJECT_ROOT}/deploy/target-cache"
    fi
}

cleanup_legacy_cargo_target_layout() {
    local root="$1"
    local legacy_entries=(
        "build"
        "deps"
        "examples"
        "incremental"
        "release"
        "debug"
        "doc"
        "tmp"
        ".fingerprint"
        ".rustc_info.json"
        "CACHEDIR.TAG"
        ".cargo-lock"
        ".future-incompat-report.json"
    )
    local found=false
    local entry

    for entry in "${legacy_entries[@]}"; do
        if [[ -e "${root}/${entry}" ]]; then
            found=true
            break
        fi
    done

    if [[ "$found" == true ]]; then
        log "Removing legacy flat cargo cache layout in ${root}"
        rm -rf \
            "${root}/build" \
            "${root}/deps" \
            "${root}/examples" \
            "${root}/incremental" \
            "${root}/release" \
            "${root}/debug" \
            "${root}/doc" \
            "${root}/tmp" \
            "${root}/.fingerprint" \
            "${root}/.rustc_info.json" \
            "${root}/CACHEDIR.TAG" \
            "${root}/.cargo-lock" \
            "${root}/.future-incompat-report.json"
    fi
}

prepare_cargo_target_dir() {
    local root
    root="$(build_cache_root)"

    mkdir -p "$root"
    cleanup_legacy_cargo_target_layout "$root"

    local target="${root}/build-${BUILD_SESSION_ID}"
    mkdir -p "$target"
    printf '%s\n' "$target"
}

prune_old_cargo_target_dirs() {
    local root="$1"
    local keep_count="$2"
    local dirs=()
    local dir

    while IFS= read -r dir; do
        dirs+=("$dir")
    done < <(find "$root" -mindepth 1 -maxdepth 1 -type d -name 'build-*' | sort)

    while (( ${#dirs[@]} > keep_count )); do
        log "Pruning old cargo target cache ${dirs[0]}"
        rm -rf "${dirs[0]}"
        dirs=("${dirs[@]:1}")
    done
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
    install -d -m 0750 "$state_dir" "$cache_dir" /run/op-dbus

    append_env_if_missing "$env_file" "OP_DBUS_DATABASE_URL" "sqlite://${state_dir}/state.db"
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
    install -m 0644 "${DEPLOY_DIR}/dinit/op-ovsdb-bridge"    "${SERVICE_DIR}/op-ovsdb-bridge"

    # --- Dinit scripts ---
    install -m 0755 "${DEPLOY_DIR}/dinit/scripts/services0-sockets.sh"   "${SERVICE_DIR}/scripts/services0-sockets.sh"
    install -m 0755 "${DEPLOY_DIR}/dinit/scripts/ovs-attach-ports.sh"    "${SERVICE_DIR}/scripts/ovs-attach-ports.sh"
    install -m 0755 "${DEPLOY_DIR}/dinit/op-ovs-services-start.sh"       "${SERVICE_DIR}/scripts/op-ovs-services-start.sh"
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
        while IFS= read -r line; do
            [[ "$line" =~ ^#.*$ || -z "$line" ]] && continue
            key="${line%%=*}"
            if ! grep -q "^${key}=" /etc/op-dbus/environment 2>/dev/null; then
                echo "$line" >> /etc/op-dbus/environment
                log "Added ${key} to /etc/op-dbus/environment"
            fi
        done < "${DEPLOY_DIR}/environment.default"
    fi

    normalize_system_runtime_environment

    # --- Remove stale services ---
    rm -f "${SERVICE_DIR}/wgcf" "${SERVICE_DIR}/boot.d/wgcf"      # old wg-quick-era name
    rm -f "${SERVICE_DIR}/boot.d/stalwart" "${SERVICE_DIR}/stalwart"
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
    enable_boot op-ovsdb-bridge

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
# ---------------------------------------------------------------------------

build_embedded_ui() {
    local ui_dir="${PROJECT_ROOT}/crates/op-web/ui"

    if [[ ! -d "$ui_dir" ]]; then
        warn "op-web/ui not found — skipping embedded UI build"
        return 0
    fi

    log "Building embedded UI (${ui_dir})..."

    local build_user
    build_user=$(path_owner_user "$PROJECT_ROOT" 2>/dev/null || echo "")

    if [[ "$EUID" -eq 0 && -n "$build_user" && "$build_user" != "root" ]]; then
        su -l "$build_user" -c \
            "export PATH=\"\$HOME/.bun/bin:\$PATH\" && \
             cd '${ui_dir}' && \
             if command -v bun >/dev/null 2>&1; then \
                 bun install && bun run build; \
             elif command -v npm >/dev/null 2>&1; then \
                 npm install && npm run build; \
             else \
                 echo 'Neither bun nor npm found — cannot build embedded UI' >&2; \
                 exit 127; \
             fi" \
            || error "Embedded UI build failed"
    else
        (export PATH="${HOME}/.bun/bin:${PATH}" && \
         cd "$ui_dir" && \
         if command -v bun >/dev/null 2>&1; then
             bun install && bun run build
         elif command -v npm >/dev/null 2>&1; then
             npm install && npm run build
         else
             echo "Neither bun nor npm found — cannot build embedded UI" >&2
             exit 127
         fi) \
            || error "Embedded UI build failed"
    fi

    log "Embedded UI built → ${ui_dir}/dist"
}

build_and_install() {
    local crate=$1 binary=$2

    # Build embedded UI before cargo so rust-embed picks up fresh assets.
    if [[ "$crate" == "op-web" ]]; then
        build_embedded_ui
    fi

    log "Building ${crate}..."
    local build_user
    build_user=$(path_owner_user "$PROJECT_ROOT" 2>/dev/null || echo "")
    local cargo_target
    cargo_target="$(prepare_cargo_target_dir)"
    local cargo_cmd="cargo"

    if [[ "$EUID" -eq 0 ]]; then
        chown -R "${build_user:-root}:${build_user:-root}" "$cargo_target" 2>/dev/null || true
    fi

    if [[ "$EUID" -eq 0 && -n "$build_user" && "$build_user" != "root" ]]; then
        cargo_cmd=$(su -l "$build_user" -c 'command -v cargo')
        su -l "$build_user" -c \
            "cd '${PROJECT_ROOT}' && OP_DBUS_DISABLE_MANAGED_CARGO=1 CARGO_TARGET_DIR='${cargo_target}' '${cargo_cmd}' build --release -p '${crate}' --bin '${binary}'" \
            || error "Build failed for ${crate}"
    else
        OP_DBUS_DISABLE_MANAGED_CARGO=1 CARGO_TARGET_DIR="$cargo_target" "$cargo_cmd" build --release -p "$crate" --bin "$binary" \
            || error "Build failed for ${crate}"
    fi

    local staged="${INSTALL_DIR}/${binary}.new.$$"
    install -m 755 "${cargo_target}/release/${binary}" "$staged"
    mv -f "$staged" "${INSTALL_DIR}/${binary}"
    log "Installed ${binary} → ${INSTALL_DIR}/${binary}"

    prune_old_cargo_target_dirs "$(build_cache_root)" "$TARGET_RETENTION_COUNT"
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
depends-on = ovs-attach-ports
env-file = /etc/op-dbus/environment
EOF
            ;;
        op-web)
            cat > "$file" <<EOF
type = process
command = /usr/local/sbin/op-web-start.sh
log-type = buffer
smooth-recovery = true
depends-on = op-dbus
env-file = /etc/op-dbus/environment
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
env = OP_DBUS_DATABASE_URL=sqlite://${DATA_DIR}/state.db
env = OP_DBUS_CACHE_DIR=${DATA_DIR}/cache
env = OP_DBUS_WEB_PORT=8081
env = OP_DBUS_SESSION_BUS=1
env = DBUS_SESSION_BUS_ADDRESS=unix:path=${RUN_DIR}/session-bus
EOF
    fi
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
            for dep in ovs-dbus-init op-chat op-services op-web op-ovsdb-bridge; do
                if is_started "$dep"; then
                    $DINITCTL stop "$dep"; stopped_dependents+=("$dep")
                fi
            done ;;
        op-web)
            for dep in op-chat op-services; do
                if is_started "$dep"; then
                    $DINITCTL stop "$dep"; stopped_dependents+=("$dep")
                fi
            done ;;
    esac

    if is_started "$service"; then
        $DINITCTL restart "$service"
    else
        $DINITCTL start "$service"
    fi

    if [[ "${#stopped_dependents[@]}" -gt 0 ]]; then
        for dep in systemd-networkd ovs-attach-ports xray-client op-web op-services op-chat ovs-dbus-init; do
            was_stopped "$dep" && $DINITCTL start "$dep"
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

command -v cargo >/dev/null || error "Cargo not found — cannot build services"
for entry in "${SERVICES[@]}"; do
    IFS=':' read -r crate binary service <<< "$entry"
    if [[ -z "$TARGET" || "$TARGET" == "all" || "$TARGET" == "$crate" ]]; then
        deploy_service "$crate" "$binary" "$service"
    fi
done

log "Deployment complete."
