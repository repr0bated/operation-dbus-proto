#!/bin/bash
#
# Setup Chimera Linux rootfs in ICUS container with WireGuard, SSH, and DHCP
# Uses existing local WireGuard server
#

set -euo pipefail

# Configuration
ICUS_NAME="${ICUS_NAME:-chimera-icus}"
CHIMERA_VERSION="${CHIMERA_VERSION:-20251220}"
ROOTFS_URL="https://repo.chimera-linux.org/live/latest/chimera-linux-x86_64-ROOTFS-${CHIMERA_VERSION}-bootstrap.tar.gz"
ICUS_ROOT="/var/lib/icus"
ICUS_PATH="${ICUS_ROOT}/${ICUS_NAME}"
WIREGUARD_ENDPOINT="${WIREGUARD_ENDPOINT:-}"  # e.g., wg.example.com:51820
WIREGUARD_PUBLIC_KEY="${WIREGUARD_PUBLIC_KEY:-}"
WIREGUARD_ALLOWED_IPS="${WIREGUARD_ALLOWED_IPS:-0.0.0.0/0, ::/0}"
MCP_SSH_HOST="${MCP_SSH_HOST:-}"  # Existing MCP SSH server
MCP_SSH_PORT="${MCP_SSH_PORT:-22}"
DHCP_INTERFACE="${DHCP_INTERFACE:-eth0}"

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

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        exit 1
    fi
    
    # Check for required tools
    for cmd in wget tar ip wg ipset; do
        if ! command -v "$cmd" &> /dev/null; then
            log_warn "$cmd not found, attempting to install..."
            apt-get update && apt-get install -y "$cmd" || true
        fi
    done
    
    # Verify WireGuard endpoint
    if [[ -z "$WIREGUARD_ENDPOINT" ]]; then
        log_error "WIREGUARD_ENDPOINT not set. Please set it to your existing WireGuard server (e.g., wg.example.com:51820)"
        exit 1
    fi
    
    # Verify MCP SSH host
    if [[ -z "$MCP_SSH_HOST" ]]; then
        log_error "MCP_SSH_HOST not set. Please set it to your existing MCP SSH server"
        exit 1
    fi
    
    log_success "Prerequisites checked"
}

# Download Chimera Linux rootfs
download_rootfs() {
    log_info "Downloading Chimera Linux rootfs..."
    
    mkdir -p /tmp/chimera-download
    cd /tmp/chimera-download
    
    if [[ -f "chimera-rootfs.tar.gz" ]]; then
        log_info "Rootfs already downloaded, skipping..."
    else
        log_info "Downloading from: $ROOTFS_URL"
        wget --progress=bar:force "$ROOTFS_URL" -O chimera-rootfs.tar.gz || {
            log_error "Failed to download rootfs"
            exit 1
        }
    fi
    
    log_success "Rootfs downloaded"
}

# Setup ICUS container
setup_icus_container() {
    log_info "Setting up ICUS container: $ICUS_NAME"
    
    # Create ICUS directory structure
    mkdir -p "$ICUS_PATH"/{rootfs,overlay,work,config,logs}
    
    # Extract rootfs
    log_info "Extracting rootfs to $ICUS_PATH/rootfs..."
    tar -xzf /tmp/chimera-download/chimera-rootfs.tar.gz -C "$ICUS_PATH/rootfs" --strip-components=1
    
    # Setup basic directories
    mkdir -p "$ICUS_PATH/rootfs"/{proc,sys,dev,run,tmp,etc/wireguard,root/.ssh}
    chmod 755 "$ICUS_PATH/rootfs"
    
    log_success "ICUS container created at $ICUS_PATH"
}

# Generate WireGuard keys
generate_wireguard_keys() {
    log_info "Generating WireGuard keys..."
    
    mkdir -p "$ICUS_PATH/config/wireguard"
    
    # Generate private key
    wg genkey | tee "$ICUS_PATH/config/wireguard/private.key" | wg pubkey > "$ICUS_PATH/config/wireguard/public.key"
    
    # Generate pre-shared key for extra security
    wg genpsk > "$ICUS_PATH/config/wireguard/preshared.key"
    
    local private_key=$(cat "$ICUS_PATH/config/wireguard/private.key")
    local public_key=$(cat "$ICUS_PATH/config/wireguard/public.key")
    local preshared_key=$(cat "$ICUS_PATH/config/wireguard/preshared.key")
    
    log_success "WireGuard keys generated"
    log_info "Container Public Key: $public_key"
    log_info "Add this key to your WireGuard server configuration"
}

# Create WireGuard configuration
create_wireguard_config() {
    log_info "Creating WireGuard configuration..."
    
    local private_key=$(cat "$ICUS_PATH/config/wireguard/private.key")
    local preshared_key=$(cat "$ICUS_PATH/config/wireguard/preshared.key")
    
    # Parse endpoint
    local wg_host=$(echo "$WIREGUARD_ENDPOINT" | cut -d':' -f1)
    local wg_port=$(echo "$WIREGUARD_ENDPOINT" | cut -d':' -f2)
    
    # Create WireGuard config for container
    cat > "$ICUS_PATH/rootfs/etc/wireguard/wg0.conf" << EOF
[Interface]
PrivateKey = ${private_key}
Address = 10.200.200.2/32, fd00:7::2/128
DNS = 1.1.1.1, 2606:4700:4700::1111

[Peer]
PublicKey = ${WIREGUARD_PUBLIC_KEY}
PresharedKey = ${preshared_key}
AllowedIPs = ${WIREGUARD_ALLOWED_IPS}
Endpoint = ${wg_host}:${wg_port}
PersistentKeepalive = 25
EOF
    
    chmod 600 "$ICUS_PATH/rootfs/etc/wireguard/wg0.conf"
    
    log_success "WireGuard configuration created"
}

# Create dinit service for WireGuard
create_wireguard_service() {
    log_info "Creating WireGuard dinit service..."
    
    mkdir -p "$ICUS_PATH/rootfs/etc/dinit.d"
    
    cat > "$ICUS_PATH/rootfs/etc/dinit.d/wireguard" << 'EOF'
type = scripted
description = "WireGuard VPN tunnel"

script = <<END
#!/bin/bash
# Load WireGuard module
modprobe wireguard 2>/dev/null || true

# Bring up WireGuard interface
wg-quick up wg0

# Wait for interface to be ready
sleep 2

# Verify connection
if ip link show wg0 &>/dev/null; then
    echo "WireGuard tunnel established"
    exit 0
else
    echo "Failed to establish WireGuard tunnel"
    exit 1
fi
END
EOF

    # Create stop script
    cat > "$ICUS_PATH/rootfs/etc/dinit.d/wireguard-stop" << 'EOF'
type = scripted
description = "Stop WireGuard VPN tunnel"

script = <<END
#!/bin/bash
wg-quick down wg0 2>/dev/null || true
exit 0
END
EOF

    chmod +x "$ICUS_PATH/rootfs/etc/dinit.d/wireguard"
    chmod +x "$ICUS_PATH/rootfs/etc/dinit.d/wireguard-stop"
    
    # Enable service
    mkdir -p "$ICUS_PATH/rootfs/etc/dinit.d/boot.d"
    ln -sf ../wireguard "$ICUS_PATH/rootfs/etc/dinit.d/boot.d/wireguard" 2>/dev/null || true
    
    log_success "WireGuard dinit service created"
}

# Create DHCP dinit service
create_dhcp_service() {
    log_info "Creating DHCP dinit service..."
    
    cat > "$ICUS_PATH/rootfs/etc/dinit.d/dhcpcd" << EOF
type = scripted
description = "DHCP Client"
depends-on = network

script = <<END
#!/bin/bash
# Wait for network interface
while ! ip link show $DHCP_INTERFACE &>/dev/null; do
    sleep 1
done

# Start DHCP client
dhcpcd -b $DHCP_INTERFACE
exit 0
END
EOF

    chmod +x "$ICUS_PATH/rootfs/etc/dinit.d/dhcpcd"
    
    # Enable service
    ln -sf ../dhcpcd "$ICUS_PATH/rootfs/etc/dinit.d/boot.d/dhcpcd" 2>/dev/null || true
    
    log_success "DHCP dinit service created"
}

# Setup SSH client and MCP connection
setup_ssh_mcp() {
    log_info "Setting up SSH for MCP connection..."
    
    # Create SSH directory
    mkdir -p "$ICUS_PATH/rootfs/root/.ssh"
    chmod 700 "$ICUS_PATH/rootfs/root/.ssh"
    
    # Generate SSH key pair for MCP connection
    if [[ ! -f "$ICUS_PATH/config/ssh/mcp_key" ]]; then
        mkdir -p "$ICUS_PATH/config/ssh"
        ssh-keygen -t ed25519 -f "$ICUS_PATH/config/ssh/mcp_key" -N "" -C "icus-mcp-$(date +%Y%m%d)"
    fi
    
    # Copy public key to container
    cp "$ICUS_PATH/config/ssh/mcp_key.pub" "$ICUS_PATH/rootfs/root/.ssh/authorized_keys"
    chmod 600 "$ICUS_PATH/rootfs/root/.ssh/authorized_keys"
    
    # Create SSH config
    cat > "$ICUS_PATH/rootfs/root/.ssh/config" << EOF
Host mcp-server
    HostName $MCP_SSH_HOST
    Port $MCP_SSH_PORT
    User root
    IdentityFile ~/.ssh/mcp_key
    StrictHostKeyChecking accept-new
    ServerAliveInterval 60
    ServerAliveCountMax 3
EOF
    chmod 600 "$ICUS_PATH/rootfs/root/.ssh/config"
    
    # Create MCP connection service
    cat > "$ICUS_PATH/rootfs/etc/dinit.d/mcp-ssh" << EOF
type = scripted
description = "MCP SSH Tunnel"
depends-on = wireguard

script = <<END
#!/bin/bash
# Ensure WireGuard is up
while ! ip link show wg0 &>/dev/null; do
    sleep 1
done

# Establish persistent SSH connection to MCP server
# This creates a reverse tunnel for MCP access
autossh -M 0 -N -R 2222:localhost:22 -o "ServerAliveInterval=60" -o "ServerAliveCountMax=3" mcp-server &

exit 0
END
EOF

    chmod +x "$ICUS_PATH/rootfs/etc/dinit.d/mcp-ssh"
    ln -sf ../mcp-ssh "$ICUS_PATH/rootfs/etc/dinit.d/boot.d/mcp-ssh" 2>/dev/null || true
    
    log_success "SSH MCP connection configured"
    log_info "Add this public key to your MCP server:"
    cat "$ICUS_PATH/config/ssh/mcp_key.pub"
}

# Create ICUS launcher script
create_icus_launcher() {
    log_info "Creating ICUS launcher..."
    
    cat > "/usr/local/bin/icus-${ICUS_NAME}" << EOF
#!/bin/bash
# ICUS Container Launcher for ${ICUS_NAME}

ICUS_PATH="${ICUS_PATH}"

# Mount essential filesystems
mount --bind /dev "\$ICUS_PATH/rootfs/dev"
mount --bind /dev/pts "\$ICUS_PATH/rootfs/dev/pts"
mount --bind /proc "\$ICUS_PATH/rootfs/proc"
mount --bind /sys "\$ICUS_PATH/rootfs/sys"
mount -t tmpfs tmpfs "\$ICUS_PATH/rootfs/run"
mount -t tmpfs tmpfs "\$ICUS_PATH/rootfs/tmp"

# Mount overlay for writeable layers
mount -t overlay overlay \
    -o lowerdir=\$ICUS_PATH/rootfs,upperdir=\$ICUS_PATH/overlay,workdir=\$ICUS_PATH/work \
    "\$ICUS_PATH/rootfs"

# Copy resolv.conf for DNS
cp /etc/resolv.conf "\$ICUS_PATH/rootfs/etc/resolv.conf"

# Enter container
exec chroot "\$ICUS_PATH/rootfs" /sbin/dinit
EOF

    chmod +x "/usr/local/bin/icus-${ICUS_NAME}"
    
    log_success "ICUS launcher created at /usr/local/bin/icus-${ICUS_NAME}"
}

# Create systemd service for ICUS container
create_systemd_service() {
    log_info "Creating systemd service..."
    
    cat > "/etc/systemd/system/icus-${ICUS_NAME}.service" << EOF
[Unit]
Description=ICUS Container: ${ICUS_NAME}
After=network.target
Wants=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/icus-${ICUS_NAME}
ExecStopPost=/bin/bash -c 'umount -R ${ICUS_PATH}/rootfs/* 2>/dev/null || true'
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable "icus-${ICUS_NAME}.service"
    
    log_success "Systemd service created and enabled"
}

# Print summary
print_summary() {
    echo
    echo "============================================================================"
    echo -e "${GREEN}ICUS Container Setup Complete${NC}"
    echo "============================================================================"
    echo
    echo "Container Name:    $ICUS_NAME"
    echo "Container Path:    $ICUS_PATH"
    echo "Rootfs:            Chimera Linux $CHIMERA_VERSION"
    echo
    echo "WireGuard Config:  $ICUS_PATH/rootfs/etc/wireguard/wg0.conf"
    echo "WireGuard Public:  $(cat $ICUS_PATH/config/wireguard/public.key)"
    echo
    echo "SSH MCP Key:       $ICUS_PATH/config/ssh/mcp_key.pub"
    echo "MCP Server:        $MCP_SSH_HOST:$MCP_SSH_PORT"
    echo
    echo "Services Enabled:"
    echo "  - wireguard      (VPN tunnel to $WIREGUARD_ENDPOINT)"
    echo "  - dhcpcd         (DHCP client on $DHCP_INTERFACE)"
    echo "  - mcp-ssh        (SSH tunnel to MCP server)"
    echo
    echo "Commands:"
    echo "  Start:   sudo systemctl start icus-${ICUS_NAME}"
    echo "  Stop:    sudo systemctl stop icus-${ICUS_NAME}"
    echo "  Status:  sudo systemctl status icus-${ICUS_NAME}"
    echo "  Shell:   sudo chroot $ICUS_PATH/rootfs /bin/sh"
    echo
    echo "Next Steps:"
    echo "  1. Add the WireGuard public key to your server:"
    echo "     $(cat $ICUS_PATH/config/wireguard/public.key)"
    echo
    echo "  2. Add the SSH public key to your MCP server:"
    echo "     $(cat $ICUS_PATH/config/ssh/mcp_key.pub)"
    echo
    echo "  3. Start the container:"
    echo "     sudo systemctl start icus-${ICUS_NAME}"
    echo
    echo "============================================================================"
}

# Main execution
main() {
    echo
    echo "============================================================================"
    echo "Chimera Linux ICUS Container Setup"
    echo "============================================================================"
    echo
    
    check_prerequisites
    download_rootfs
    setup_icus_container
    generate_wireguard_keys
    create_wireguard_config
    create_wireguard_service
    create_dhcp_service
    setup_ssh_mcp
    create_icus_launcher
    create_systemd_service
    print_summary
}

main "$@"
