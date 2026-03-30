#!/bin/bash
# deploy/deploy.sh
# Tight, Dinit-focused deployment for Operation D-Bus

set -e

# --- Configuration ---
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

# Components: "crate_name:binary_name:service_name"
SERVICES=(
    "op-dbus:op-dbus:op-dbus"
    "op-web:op-web-server:op-web"
    "op-services:op-services:op-services"
    "op-chat:op-chat:op-chat"
)

# Detect Mode (System vs User)
if [ "$EUID" -ne 0 ]; then
    echo "Running in USER mode (installing to $PROJECT_ROOT/deploy/bin)"
    INSTALL_DIR="$PROJECT_ROOT/deploy/bin"
    SERVICE_DIR="$PROJECT_ROOT/deploy/services"
    SOCKET_PATH="$PROJECT_ROOT/deploy/dinit.socket"
    SUDO=""
    DINITCTL="dinitctl -p $SOCKET_PATH"
    
    mkdir -p "$INSTALL_DIR" "$SERVICE_DIR"
    
    # Start dinit if not running
    if [ ! -S "$SOCKET_PATH" ]; then
        echo "Starting local dinit instance..."
        # Create a boot service with correct type
        echo "type = internal" > "$SERVICE_DIR/boot"
        
        dinit --user -p "$SOCKET_PATH" -d "$SERVICE_DIR" -l "$PROJECT_ROOT/deploy/dinit.log" &
        sleep 5
    fi
else
    echo "Running in SYSTEM mode (installing to /usr/local/sbin)"
    INSTALL_DIR="/usr/local/sbin"
    SERVICE_DIR="/etc/dinit.d"
    SUDO="" # Already root
    DINITCTL="dinitctl"
    mkdir -p "$INSTALL_DIR" "$SERVICE_DIR"
fi

# --- Functions ---

log() { echo -e "\033[0;32m[DEPLOY]\033[0m $1"; }
warn() { echo -e "\033[1;33m[WARN]\033[0m $1"; }
error() { echo -e "\033[0;31m[ERROR]\033[0m $1"; exit 1; }
is_started() { $DINITCTL status "$1" 2>/dev/null | grep -q "State: STARTED"; }
enable_boot() {
    local service="$1"
    mkdir -p "$SERVICE_DIR/boot.d"
    ln -sfn "../$service" "$SERVICE_DIR/boot.d/$service"
}

ensure_resolver_manager() {
    [ "$EUID" -eq 0 ] || return 0
    command -v apk >/dev/null 2>&1 || return 0

    if ! apk info -e resolvconf >/dev/null 2>&1; then
        log "Installing resolvconf..."
        apk add resolvconf
    fi
    if ! apk info -e resolvconf-openresolv >/dev/null 2>&1; then
        log "Installing resolvconf-openresolv..."
        apk add resolvconf-openresolv
    fi
}

ensure_system_runtime_units() {
    if [ "$EUID" -ne 0 ]; then
        warn "Skipping system runtime unit install (non-root); /etc/systemd/network and /etc/dinit.d are unchanged. Re-run with doas/sudo for host networking changes."
        return 0
    fi

    log "Installing D-Bus runtime units..."
    install -d "$SERVICE_DIR" "$SERVICE_DIR/boot.d" "$SERVICE_DIR/scripts" /etc/systemd/network /etc/netplan /usr/local/sbin

    # Clean old networkd units and install new ones
    find /etc/systemd/network -maxdepth 1 -type f \
        \( -name '*.network' -o -name '*.netdev' -o -name '*.link' \) -delete
    ensure_resolver_manager

    # Launcher scripts
    install -m 0755 "$PROJECT_ROOT/deploy/dinit/op-session-bus.sh" /usr/local/sbin/op-session-bus
    install -m 0755 "$PROJECT_ROOT/deploy/dinit/op-dbus-dinit.sh" /usr/local/sbin/op-dbus-dinit.sh
    rm -f /usr/local/sbin/op-networkd-dinit.sh
    install -m 0755 "$PROJECT_ROOT/deploy/dinit/op-web-dinit.sh" /usr/local/sbin/op-web-dinit.sh

    # Dinit service definitions
    install -m 0644 "$PROJECT_ROOT/deploy/dinit/op-session-bus" "$SERVICE_DIR/op-session-bus"
    install -m 0644 "$PROJECT_ROOT/deploy/dinit/op-ovsdb-seed" "$SERVICE_DIR/op-ovsdb-seed"
    install -m 0644 "$PROJECT_ROOT/deploy/dinit/op-ovsdb-bridge" "$SERVICE_DIR/op-ovsdb-bridge"
    install -m 0644 "$PROJECT_ROOT/deploy/dinit/systemd-networkd" "$SERVICE_DIR/systemd-networkd"
    install -m 0644 "$PROJECT_ROOT/deploy/dinit/netplan-apply" "$SERVICE_DIR/netplan-apply"
    install -m 0644 "$PROJECT_ROOT/deploy/dinit/ovs-attach-ports" "$SERVICE_DIR/ovs-attach-ports"
    install -m 0644 "$PROJECT_ROOT/deploy/dinit/xray-client" "$SERVICE_DIR/xray-client"

    # Dinit scripts
    install -m 0755 "$PROJECT_ROOT/deploy/dinit/op-ovsdb-seed.sh" "$SERVICE_DIR/scripts/op-ovsdb-seed.sh"
    install -m 0755 "$PROJECT_ROOT/deploy/dinit/op-ovsdb-bridge-start.sh" "$SERVICE_DIR/scripts/op-ovsdb-bridge-start.sh"
    install -m 0755 "$PROJECT_ROOT/deploy/dinit/scripts/ovs-attach-ports.sh" "$SERVICE_DIR/scripts/ovs-attach-ports.sh"

    # systemd-networkd units (ens3 DHCP standalone)
    for network_file in "$PROJECT_ROOT"/deploy/systemd/networkd/*; do
        install -m 0644 "$network_file" "/etc/systemd/network/$(basename "$network_file")"
    done

    # Netplan config (OVS bridge — execution path)
    install -m 0600 "$PROJECT_ROOT/deploy/netplan/01-ovsbr0.yaml" /etc/netplan/01-ovsbr0.yaml

    # Install wgcf private key for networkd WireGuard tunnel.
    # Always refresh it from wgcf-profile.conf so profile rotations are applied.
    install -d -m 750 -o root -g systemd-network /etc/wireguard
    grep '^PrivateKey' "$PROJECT_ROOT/wgcf-profile.conf" | awk '{print $3}' \
        > /tmp/wgcf.key.tmp
    install -m 640 -o root -g systemd-network /tmp/wgcf.key.tmp /etc/wireguard/wgcf.key
    rm -f /tmp/wgcf.key.tmp
    log "Installed /etc/wireguard/wgcf.key for networkd"

    # Install Xray client config if not already present
    if [ ! -f /etc/xray/client.json ]; then
        install -d /etc/xray
        install -m 0600 "$PROJECT_ROOT/deploy/xray/client.json" /etc/xray/client.json
        log "Installed Xray client config — edit /etc/xray/client.json with server details"
    fi

    # Remove stale wg-quick-era dinit service if present.
    rm -f "$SERVICE_DIR/wgcf" "$SERVICE_DIR/boot.d/wgcf"

    enable_boot op-session-bus
    enable_boot op-ovsdb-seed
    enable_boot op-ovsdb-bridge
    enable_boot systemd-networkd
    enable_boot netplan-apply
    enable_boot ovs-attach-ports
    enable_boot xray-client

    if [ -e "$SERVICE_DIR/boot.d/stalwart" ] || [ -e "$SERVICE_DIR/stalwart" ]; then
        log "Removing stale stalwart service from dinit boot set..."
    fi
    rm -f "$SERVICE_DIR/boot.d/stalwart" "$SERVICE_DIR/stalwart"
    $DINITCTL stop stalwart >/dev/null 2>&1 || true
}

build_and_install() {
    local crate=$1
    local binary=$2
    local service=$3

    log "Building $crate..."
    # Run cargo as the repo owner to avoid root-owned target/ artifacts.
    # Use /var/cache/op-dbus-build as CARGO_TARGET_DIR since the repo is on a
    # noexec btrfs subvolume that prevents build script execution.
    local build_user
    build_user=$(stat -c "%U" "$PROJECT_ROOT" 2>/dev/null || stat -f "%Su" "$PROJECT_ROOT" 2>/dev/null || echo "")
    local cargo_target="/var/cache/op-dbus-build"
    mkdir -p "$cargo_target"
    chown "${build_user:-root}:${build_user:-root}" "$cargo_target" 2>/dev/null || true
    if [ "$EUID" -eq 0 ] && [ -n "$build_user" ] && [ "$build_user" != "root" ]; then
        if ! su -l "$build_user" -c "cd '$PROJECT_ROOT' && CARGO_TARGET_DIR='$cargo_target' cargo build --release -p '$crate'"; then
            error "Build failed for $crate"
        fi
    else
        if ! CARGO_TARGET_DIR="$cargo_target" cargo build --release -p "$crate"; then
            error "Build failed for $crate"
        fi
    fi

    log "Installing $binary..."
    local staged="$INSTALL_DIR/${binary}.new.$$"
    install -m 755 "$cargo_target/release/$binary" "$staged"
    mv -f "$staged" "$INSTALL_DIR/$binary"
}

generate_service_file() {
    local binary=$1
    local service=$2
    local file="$SERVICE_DIR/$service"

    log "Generating dinit service for $service..."

    case "$service" in
        op-dbus)
            cat <<EOF > "$file"
type = process
command = /usr/local/sbin/op-dbus-dinit.sh
log-type = buffer
smooth-recovery = true
depends-on = op-session-bus
EOF
            ;;
        op-web)
            cat <<EOF > "$file"
type = process
command = /usr/local/sbin/op-web-dinit.sh
log-type = buffer
smooth-recovery = true
depends-on = op-dbus
depends-on = op-ovsdb-bridge
EOF
            ;;
        op-services|op-chat)
            cat <<EOF > "$file"
type = process
command = $INSTALL_DIR/$binary
log-type = buffer
smooth-recovery = true
depends-on = op-web
EOF
            ;;
        *)
            cat <<EOF > "$file"
type = process
command = $INSTALL_DIR/$binary
log-type = buffer
smooth-recovery = true
EOF
            ;;
    esac

    # Local environment overrides for user-mode deploys
    if [ "$EUID" -ne 0 ]; then
        local DATA_DIR="$PROJECT_ROOT/deploy/data"
        mkdir -p "$DATA_DIR/cache"
        cat <<EOF >> "$file"
env = OP_DBUS_DATABASE_URL=sqlite://$DATA_DIR/state.db
env = OP_DBUS_CACHE_DIR=$DATA_DIR/cache
env = OP_DBUS_WEB_PORT=8081
env = OP_DBUS_SESSION_BUS=1
EOF
    fi
}

deploy_service() {
    local crate=$1
    local binary=$2
    local service=$3
    local stopped_dependents=()
    local dep

    was_stopped() {
        local target="$1"
        local entry
        for entry in "${stopped_dependents[@]}"; do
            if [ "$entry" = "$target" ]; then
                return 0
            fi
        done
        return 1
    }

    # Build & Install
    build_and_install "$crate" "$binary" "$service"

    # Generate Service Config
    generate_service_file "$binary" "$service"

    # Ensure services start on boot in system mode
    if [ "$EUID" -eq 0 ]; then
        enable_boot "$service"
    fi

    # dinit will block restart if dependents are active; stop them in reverse chain.
    case "$service" in
        op-dbus)
            for dep in op-chat op-services op-web systemd-networkd op-ovsdb-bridge ovs-attach-ports; do
                if is_started "$dep"; then
                    log "Stopping dependent $dep..."
                    $DINITCTL stop "$dep"
                    stopped_dependents+=("$dep")
                fi
            done
            ;;
        op-web)
            for dep in op-chat op-services; do
                if is_started "$dep"; then
                    log "Stopping dependent $dep..."
                    $DINITCTL stop "$dep"
                    stopped_dependents+=("$dep")
                fi
            done
            ;;
        *)
            ;;
    esac

    # Restart only when already started, otherwise start
    if is_started "$service"; then
        log "Restarting $service..."
        $DINITCTL restart "$service"
    else
        log "Starting $service..."
        $DINITCTL start "$service"
    fi

    # Restore dependents in dependency order.
    if [ "${#stopped_dependents[@]}" -gt 0 ]; then
        for dep in ovs-attach-ports op-ovsdb-bridge systemd-networkd op-web op-services op-chat; do
            if was_stopped "$dep"; then
                log "Starting dependent $dep..."
                $DINITCTL start "$dep"
            fi
        done
    fi
    
    log "✅ $service deployed"
}

# --- Main ---

# Check dependencies
command -v cargo >/dev/null || error "Cargo not found"
command -v dinitctl >/dev/null || warn "dinitctl not found"

ensure_system_runtime_units

# Deploy selected or all
TARGET=$1

for entry in "${SERVICES[@]}"; do
    IFS=':' read -r crate binary service <<< "$entry"
    
    if [ -z "$TARGET" ] || [ "$TARGET" == "all" ] || [ "$TARGET" == "$crate" ]; then
        deploy_service "$crate" "$binary" "$service"
    fi
done

log "Deployment Complete."
