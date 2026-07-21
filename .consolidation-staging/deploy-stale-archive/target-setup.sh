#!/bin/bash
#
# OP-DBUS TARGET SYSTEM SETUP
#
# Run this on target systems to prepare them for receiving deployments
# from the release server.
#
# Prerequisites:
# - BTRFS filesystem
# - SSH access from release server
# - Root access
#

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# ============================================================================
# CONFIGURATION
# ============================================================================

DEPLOY_DIR="/opt/op-dbus-deploy"
INSTALL_DIR="/opt/op-dbus"
CONFIG_DIR="/etc/op-dbus"
DATA_DIR="/var/lib/op-dbus"
LOG_DIR="/var/log/op-dbus"

# ============================================================================
# CHECKS
# ============================================================================

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        exit 1
    fi
}

check_btrfs() {
    local fs_type=$(findmnt -n -o FSTYPE -T /opt)
    
    if [[ "$fs_type" != "btrfs" ]]; then
        log_warn "Filesystem at /opt is not BTRFS (found: $fs_type)"
        log_warn "BTRFS is recommended for atomic deployments"
        log_warn "Falling back to rsync-based deployment"
        USE_BTRFS=false
    else
        log_success "BTRFS filesystem detected"
        USE_BTRFS=true
    fi
}

# ============================================================================
# SETUP
# ============================================================================

setup_directories() {
    log_info "Creating directory structure..."
    
    mkdir -p "$DEPLOY_DIR"
    mkdir -p "$INSTALL_DIR"
    mkdir -p "$CONFIG_DIR"
    mkdir -p "$DATA_DIR"/{cache,sessions}
    mkdir -p "$LOG_DIR"
    
    log_success "Directories created"
}

setup_user() {
    log_info "Creating op-dbus user..."
    
    if id "op-dbus" &>/dev/null; then
        log_info "User op-dbus already exists"
    else
        useradd --system --no-create-home --shell /usr/sbin/nologin op-dbus
        log_success "User op-dbus created"
    fi
    
    chown -R op-dbus:op-dbus "$DATA_DIR" "$LOG_DIR"
}

setup_systemd() {
    log_info "Installing systemd services..."
    
    # op-chat-server service
    cat > /etc/systemd/system/op-chat-server.service << 'EOF'
[Unit]
Description=OP-DBUS Chat Server
After=network.target

[Service]
Type=simple
User=op-dbus
Group=op-dbus
EnvironmentFile=-/etc/op-dbus/environment
EnvironmentFile=-/etc/op-dbus/secrets.env
ExecStart=/opt/op-dbus/bin/op-web-server
Restart=always
RestartSec=5

NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=yes
ReadWritePaths=/var/lib/op-dbus /var/log/op-dbus

StandardOutput=journal
StandardError=journal
SyslogIdentifier=op-chat-server

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    
    log_success "Systemd services installed"
}

setup_deploy_receiver() {
    log_info "Setting up deployment receiver..."
    
    # Create deployment receiver script
    cat > /usr/local/bin/op-receive-deploy << 'SCRIPT'
#!/bin/bash
#
# Receive deployment from release server
# Called via: ssh target op-receive-deploy < snapshot.stream
#

set -euo pipefail

DEPLOY_DIR="/opt/op-dbus-deploy"
INSTALL_DIR="/opt/op-dbus"

# Receive BTRFS stream
if command -v btrfs &>/dev/null; then
    btrfs receive "$DEPLOY_DIR"
else
    # Fallback: receive as tar
    tar -xzf - -C "$DEPLOY_DIR"
fi

# Find the received snapshot
SNAPSHOT=$(ls -t "$DEPLOY_DIR" | head -1)

if [[ -z "$SNAPSHOT" ]]; then
    echo "No snapshot received"
    exit 1
fi

echo "Received snapshot: $SNAPSHOT"

# Atomic switch
if [[ -d "$INSTALL_DIR" ]]; then
    mv "$INSTALL_DIR" "${INSTALL_DIR}.old.$(date +%Y%m%d%H%M%S)"
fi

mv "$DEPLOY_DIR/$SNAPSHOT" "$INSTALL_DIR"

# Restart services
systemctl restart op-chat-server || true

# Cleanup old versions (keep last 3)
ls -dt ${INSTALL_DIR}.old.* 2>/dev/null | tail -n +4 | xargs rm -rf 2>/dev/null || true

echo "Deployment complete: $SNAPSHOT"
SCRIPT

    chmod +x /usr/local/bin/op-receive-deploy
    
    log_success "Deployment receiver configured"
}

setup_environment() {
    log_info "Creating environment configuration..."
    
    if [[ ! -f "$CONFIG_DIR/environment" ]]; then
        cat > "$CONFIG_DIR/environment" << 'EOF'
# OP-DBUS Target System Configuration

PORT=8080
BIND_ADDRESS=127.0.0.1

OP_DBUS_DATA_DIR=/var/lib/op-dbus
OP_DBUS_CACHE_DIR=/var/lib/op-dbus/cache
OP_DBUS_LOG_DIR=/var/log/op-dbus

RUST_LOG=info,op_chat=debug

# LLM Provider
LLM_PROVIDER=gemini
EOF
        log_success "Environment file created"
    else
        log_info "Environment file already exists"
    fi
    
    if [[ ! -f "$CONFIG_DIR/secrets.env" ]]; then
        cat > "$CONFIG_DIR/secrets.env" << 'EOF'
# OP-DBUS Secrets
# Fill in your API keys

HF_TOKEN=
GITHUB_PERSONAL_ACCESS_TOKEN=
EOF
        chmod 600 "$CONFIG_DIR/secrets.env"
        log_success "Secrets template created"
    fi
}

# ============================================================================
# MAIN
# ============================================================================

print_summary() {
    echo
    echo "============================================================================"
    echo -e "${GREEN}TARGET SYSTEM SETUP COMPLETE${NC}"
    echo "============================================================================"
    echo
    echo "This system is ready to receive deployments from the release server."
    echo
    echo "On the release server, add this target:"
    echo "  op-deploy add-target $(hostname) root@$(hostname -I | awk '{print $1}')"
    echo
    echo "Then deploy:"
    echo "  op-deploy send <snapshot-name> $(hostname)"
    echo
    echo "Configuration:"
    echo "  Environment: $CONFIG_DIR/environment"
    echo "  Secrets:     $CONFIG_DIR/secrets.env (add your API keys)"
    echo
    echo "After first deployment, start services:"
    echo "  systemctl enable --now op-chat-server"
    echo
}

main() {
    echo
    echo "============================================================================"
    echo "OP-DBUS TARGET SYSTEM SETUP"
    echo "============================================================================"
    echo
    
    check_root
    check_btrfs
    setup_directories
    setup_user
    setup_systemd
    setup_deploy_receiver
    setup_environment
    print_summary
}

main "$@"
