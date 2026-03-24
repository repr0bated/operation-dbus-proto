#!/bin/bash
# Complete Setup Script for op-dbus-v2
# Builds, installs binaries to /usr/local/sbin, configures services
# 
# Usage:
#   sudo ./deploy/setup-complete.sh
#   sudo DOMAIN=example.com ./deploy/setup-complete.sh
#
# Environment Variables:
#   DOMAIN          - Your domain name (prompted if not set)
#   SUBDOMAINS      - Comma-separated subdomains (default: proxmox,op-web,chat,mcp-tools,agents)
#   SERVICE_USER    - User to run services as (default: jeremy)
#   PROJECT_DIR     - Project directory (default: /home/jeremy/op-dbus-v2)
#   INSTALL_DIR     - Binary install location (default: /usr/local/sbin)
#   SETUP_TLS       - Enable TLS setup (default: true)
#   SETUP_NGINX     - Enable nginx setup (default: true)
#   SETUP_SYSTEMD   - Enable systemd setup (default: true)
#   BUILD_RELEASE   - Build in release mode (default: true)
#
# Version: 1.0.0

set -e

#===============================================================================
# COLORS AND HELPERS
#===============================================================================
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }
log_step() { echo -e "\n${MAGENTA}[$1/$TOTAL_STEPS]${NC} ${CYAN}$2${NC}"; }

#===============================================================================
# DEFAULT CONFIGURATION
#===============================================================================

# Detect project directory (script location or current directory)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "$SCRIPT_DIR/../Cargo.toml" ]; then
    DEFAULT_PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
elif [ -f "./Cargo.toml" ]; then
    DEFAULT_PROJECT_DIR="$(pwd)"
else
    DEFAULT_PROJECT_DIR="/home/jeremy/op-dbus-v2"
fi

# Paths
PROJECT_DIR="${PROJECT_DIR:-$DEFAULT_PROJECT_DIR}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/sbin}"
CONFIG_DIR="/etc/op-dbus"
LOG_DIR="/var/log/op-dbus"
DATA_DIR="/var/lib/op-dbus"

# Service user
SERVICE_USER="${SERVICE_USER:-jeremy}"
SERVICE_GROUP="${SERVICE_GROUP:-$SERVICE_USER}"

# Domain (for TLS)
DOMAIN="${DOMAIN:-}"
SUBDOMAINS="${SUBDOMAINS:-proxmox,op-web,chat,mcp-tools,agents}"

# Feature flags
SETUP_TLS="${SETUP_TLS:-true}"
SETUP_NGINX="${SETUP_NGINX:-true}"
SETUP_SYSTEMD="${SETUP_SYSTEMD:-true}"
BUILD_RELEASE="${BUILD_RELEASE:-true}"

# Binaries to build
BINARY_PACKAGES=(
    "op-web"
    "op-mcp"
)

TOTAL_STEPS=10

#===============================================================================
# BANNER
#===============================================================================

echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║           op-dbus-v2 Complete Setup Script                      ║${NC}"
echo -e "${GREEN}║           Build • Install • Configure • Deploy                  ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""

#===============================================================================
# STEP 0: PRE-FLIGHT CHECKS
#===============================================================================

log_step 0 "Pre-flight checks..."

# Check root
if [ "$EUID" -ne 0 ]; then
    log_error "This script must be run as root (use sudo)"
    log_info "Usage: sudo $0"
    exit 1
fi

# Check project directory
if [ ! -d "$PROJECT_DIR" ]; then
    log_error "Project directory not found: $PROJECT_DIR"
    log_info "Set PROJECT_DIR environment variable or run from project root"
    exit 1
fi

if [ ! -f "$PROJECT_DIR/Cargo.toml" ]; then
    log_error "Not a valid Rust project: $PROJECT_DIR/Cargo.toml missing"
    exit 1
fi

log_info "Project directory: $PROJECT_DIR"

# Check for cargo (try user's cargo first)
CARGO_BIN=""
if [ -x "/home/$SERVICE_USER/.cargo/bin/cargo" ]; then
    CARGO_BIN="/home/$SERVICE_USER/.cargo/bin/cargo"
elif command -v cargo &> /dev/null; then
    CARGO_BIN="cargo"
else
    log_error "Cargo not found. Install Rust first."
    log_info "Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

log_success "Using cargo: $CARGO_BIN"

# Check for required tools
for cmd in curl jq openssl; do
    if ! command -v $cmd &> /dev/null; then
        log_warning "$cmd not found, installing..."
        apt-get update -qq && apt-get install -y -qq $cmd
    fi
done

# Check service user exists
if ! id "$SERVICE_USER" &>/dev/null; then
    log_error "Service user does not exist: $SERVICE_USER"
    log_info "Create with: useradd -m $SERVICE_USER"
    exit 1
fi

log_success "Pre-flight checks passed"

#===============================================================================
# STEP 1: INTERACTIVE CONFIGURATION
#===============================================================================

log_step 1 "Configuration..."

echo -e "${CYAN}Domain Configuration${NC}"
echo "─────────────────────────────────────────────────────────────────"

if [ -z "$DOMAIN" ]; then
    DEFAULT_DOMAIN="ghostbridge.tech"
    read -p "Enter your domain name [$DEFAULT_DOMAIN]: " DOMAIN
    DOMAIN=${DOMAIN:-$DEFAULT_DOMAIN}
fi

# Validate domain format
if [[ ! "$DOMAIN" =~ ^[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)*\.[a-zA-Z]{2,}$ ]]; then
    log_warning "Domain format may be invalid: $DOMAIN"
fi

log_success "Domain: $DOMAIN"

echo ""
echo -e "${CYAN}Subdomain Configuration${NC}"
echo "─────────────────────────────────────────────────────────────────"
read -p "Subdomains (comma-separated) [$SUBDOMAINS]: " INPUT_SUBDOMAINS
SUBDOMAINS=${INPUT_SUBDOMAINS:-$SUBDOMAINS}

log_success "Subdomains: $SUBDOMAINS"

echo ""
echo -e "${CYAN}Installation Summary${NC}"
echo "─────────────────────────────────────────────────────────────────"
echo -e "  Project:           ${YELLOW}$PROJECT_DIR${NC}"
echo -e "  Install directory: ${YELLOW}$INSTALL_DIR${NC}"
echo -e "  Config directory:  ${YELLOW}$CONFIG_DIR${NC}"
echo -e "  Log directory:     ${YELLOW}$LOG_DIR${NC}"
echo -e "  Service user:      ${YELLOW}$SERVICE_USER${NC}"
echo -e "  Domain:            ${YELLOW}$DOMAIN${NC}"
echo -e "  Setup TLS:         ${YELLOW}$SETUP_TLS${NC}"
echo -e "  Setup nginx:       ${YELLOW}$SETUP_NGINX${NC}"
echo -e "  Setup systemd:     ${YELLOW}$SETUP_SYSTEMD${NC}"
echo ""
read -p "Proceed with installation? [Y/n]: " CONFIRM

if [[ "$CONFIRM" =~ ^[Nn]$ ]]; then
    log_info "Installation cancelled"
    exit 0
fi

#===============================================================================
# STEP 2: CREATE DIRECTORIES
#===============================================================================

log_step 2 "Creating directories..."

mkdir -p "$INSTALL_DIR"
mkdir -p "$CONFIG_DIR"
mkdir -p "$LOG_DIR"
mkdir -p "$DATA_DIR"
mkdir -p /etc/nginx/ssl
mkdir -p /etc/ssl/cloudflare

# Set ownership
chown -R "$SERVICE_USER:$SERVICE_GROUP" "$LOG_DIR"
chown -R "$SERVICE_USER:$SERVICE_GROUP" "$DATA_DIR"

log_success "Directories created"

#===============================================================================
# STEP 3: BUILD BINARIES
#===============================================================================

log_step 3 "Building binaries (this may take several minutes)..."

cd "$PROJECT_DIR"

# Build mode
if [ "$BUILD_RELEASE" = "true" ]; then
    BUILD_FLAGS="--release"
    TARGET_DIR="target/release"
else
    BUILD_FLAGS=""
    TARGET_DIR="target/debug"
fi

# Build as service user to avoid permission issues
log_info "Building workspace..."
if sudo -u "$SERVICE_USER" "$CARGO_BIN" build $BUILD_FLAGS 2>&1 | tail -20; then
    log_success "Build complete"
else
    log_error "Build failed"
    exit 1
fi

#===============================================================================
# STEP 4: INSTALL BINARIES TO /usr/local/sbin
#===============================================================================

log_step 4 "Installing binaries to $INSTALL_DIR..."

INSTALLED_COUNT=0

# Known binary names to look for
BINARY_NAMES=(
    "op-web-server"
    "op-web"
    "op-mcp-server"
    "op-mcp"
    "op-agents-server"
    "op-chat-server"
)

for binary in "${BINARY_NAMES[@]}"; do
    BINARY_PATH="$PROJECT_DIR/$TARGET_DIR/$binary"
    
    if [ -f "$BINARY_PATH" ] && [ -x "$BINARY_PATH" ]; then
        log_info "Installing $binary..."
        
        # Stop service if running
        SERVICE_NAME=$(echo "$binary" | sed 's/-server$//')
        if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
            log_info "Stopping $SERVICE_NAME service..."
            systemctl stop "$SERVICE_NAME" || true
        fi
        
        # Install binary
        cp "$BINARY_PATH" "$INSTALL_DIR/$binary"
        chmod 755 "$INSTALL_DIR/$binary"
        chown root:root "$INSTALL_DIR/$binary"
        
        log_success "Installed: $INSTALL_DIR/$binary"
        ((INSTALLED_COUNT++))
    fi
done

if [ $INSTALLED_COUNT -eq 0 ]; then
    log_warning "No binaries were installed. Checking build output..."
    ls -la "$PROJECT_DIR/$TARGET_DIR/" | grep -E '^-rwx' | head -10
else
    log_success "Installed $INSTALLED_COUNT binaries to $INSTALL_DIR"
fi

# List installed binaries
echo ""
echo -e "${CYAN}Installed Binaries:${NC}"
ls -la "$INSTALL_DIR"/op* 2>/dev/null | sed 's/^/  /' || echo "  (none found)"

#===============================================================================
# STEP 5: CREATE ENVIRONMENT FILE
#===============================================================================

log_step 5 "Creating environment configuration..."

ENV_FILE="$CONFIG_DIR/op-web.env"
SAFE_DOMAIN=$(echo "$DOMAIN" | tr '.' '-')

# Source existing environment variables if available
if [ -f "/home/$SERVICE_USER/.bashrc" ]; then
    # Extract key environment variables
    HF_TOKEN=$(sudo -u "$SERVICE_USER" bash -c 'source ~/.bashrc 2>/dev/null; echo $HF_TOKEN' 2>/dev/null || echo "")
    GH_TOKEN=$(sudo -u "$SERVICE_USER" bash -c 'source ~/.bashrc 2>/dev/null; echo $GH_TOKEN' 2>/dev/null || echo "")
    CF_DNS_ZONE_TOKEN=$(sudo -u "$SERVICE_USER" bash -c 'source ~/.bashrc 2>/dev/null; echo $CF_DNS_ZONE_TOKEN' 2>/dev/null || echo "")
    CF_ACCOUNT_ID=$(sudo -u "$SERVICE_USER" bash -c 'source ~/.bashrc 2>/dev/null; echo $CF_ACCOUNT_ID' 2>/dev/null || echo "")
    CF_ZONE_ID=$(sudo -u "$SERVICE_USER" bash -c 'source ~/.bashrc 2>/dev/null; echo $CF_ZONE_ID' 2>/dev/null || echo "")
    PINECONE_API_KEY=$(sudo -u "$SERVICE_USER" bash -c 'source ~/.bashrc 2>/dev/null; echo $PINECONE_API_KEY' 2>/dev/null || echo "")
fi

cat > "$ENV_FILE" << EOF
# op-dbus-v2 Environment Configuration
# Generated: $(date)
# Domain: $DOMAIN

# Domain
DOMAIN=$DOMAIN

# API Keys
HF_TOKEN=${HF_TOKEN:-}
GITHUB_PERSONAL_ACCESS_TOKEN=${GH_TOKEN:-}
HUGGINGFACE_API_KEY=${HF_TOKEN:-}
PINECONE_API_KEY=${PINECONE_API_KEY:-}

# Cloudflare
CLOUDFLARE_API_TOKEN=${CF_DNS_ZONE_TOKEN:-}
CLOUDFLARE_ACCOUNT_ID=${CF_ACCOUNT_ID:-}
CF_ZONE_ID=${CF_ZONE_ID:-}

# MCP Configuration
MCP_CONFIG_FILE=$PROJECT_DIR/crates/op-mcp/mcp-config.json

# TLS Certificates
SSL_CERT_PATH=/etc/nginx/ssl/$SAFE_DOMAIN.crt
SSL_KEY_PATH=/etc/nginx/ssl/$SAFE_DOMAIN.key

# Server Configuration
OP_WEB_HOST=127.0.0.1
OP_WEB_PORT=8081
OP_MCP_PORT=8082
OP_AGENTS_PORT=8083

# Logging
RUST_LOG=info,op_web=debug,op_mcp=debug

# Data directories
OP_DATA_DIR=$DATA_DIR
OP_LOG_DIR=$LOG_DIR
EOF

chmod 600 "$ENV_FILE"
chown root:root "$ENV_FILE"

# Also create user-readable copy
cp "$ENV_FILE" "/home/$SERVICE_USER/.op-web.env"
chown "$SERVICE_USER:$SERVICE_GROUP" "/home/$SERVICE_USER/.op-web.env"
chmod 600 "/home/$SERVICE_USER/.op-web.env"

log_success "Environment file created: $ENV_FILE"

#===============================================================================
# STEP 6: SETUP SYSTEMD SERVICES
#===============================================================================

if [ "$SETUP_SYSTEMD" = "true" ]; then
    log_step 6 "Setting up systemd services..."
    
    # Determine which binary to use for op-web
    OP_WEB_BINARY=""
    if [ -f "$INSTALL_DIR/op-web-server" ]; then
        OP_WEB_BINARY="$INSTALL_DIR/op-web-server"
    elif [ -f "$INSTALL_DIR/op-web" ]; then
        OP_WEB_BINARY="$INSTALL_DIR/op-web"
    fi
    
    if [ -n "$OP_WEB_BINARY" ]; then
        cat > /etc/systemd/system/op-web.service << EOF
[Unit]
Description=op-dbus Web Server
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=$SERVICE_USER
Group=$SERVICE_GROUP
WorkingDirectory=$PROJECT_DIR
EnvironmentFile=$ENV_FILE
ExecStart=$OP_WEB_BINARY
Restart=on-failure
RestartSec=5
StandardOutput=append:$LOG_DIR/op-web.log
StandardError=append:$LOG_DIR/op-web-error.log

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ReadWritePaths=$LOG_DIR $DATA_DIR

[Install]
WantedBy=multi-user.target
EOF
        log_success "Created op-web.service"
    else
        log_warning "op-web binary not found, skipping service creation"
    fi

    # op-mcp service
    OP_MCP_BINARY=""
    if [ -f "$INSTALL_DIR/op-mcp-server" ]; then
        OP_MCP_BINARY="$INSTALL_DIR/op-mcp-server"
    elif [ -f "$INSTALL_DIR/op-mcp" ]; then
        OP_MCP_BINARY="$INSTALL_DIR/op-mcp"
    fi
    
    if [ -n "$OP_MCP_BINARY" ]; then
        cat > /etc/systemd/system/op-mcp.service << EOF
[Unit]
Description=op-dbus MCP Server
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=$SERVICE_USER
Group=$SERVICE_GROUP
WorkingDirectory=$PROJECT_DIR
EnvironmentFile=$ENV_FILE
ExecStart=$OP_MCP_BINARY
Restart=on-failure
RestartSec=5
StandardOutput=append:$LOG_DIR/op-mcp.log
StandardError=append:$LOG_DIR/op-mcp-error.log

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ReadWritePaths=$LOG_DIR $DATA_DIR

[Install]
WantedBy=multi-user.target
EOF
        log_success "Created op-mcp.service"
    fi

    # Reload systemd
    systemctl daemon-reload
    
    # Enable services
    [ -f /etc/systemd/system/op-web.service ] && systemctl enable op-web.service
    [ -f /etc/systemd/system/op-mcp.service ] && systemctl enable op-mcp.service
    
    log_success "Systemd services configured"
else
    log_step 6 "Skipping systemd setup (SETUP_SYSTEMD=false)"
fi

#===============================================================================
# STEP 7: SETUP NGINX
#===============================================================================

if [ "$SETUP_NGINX" = "true" ]; then
    log_step 7 "Setting up nginx..."
    
    # Install nginx if not present
    if ! command -v nginx &> /dev/null; then
        log_info "Installing nginx..."
        apt-get update -qq && apt-get install -y -qq nginx
    fi
    
    # Create nginx configuration
    cat > /etc/nginx/sites-available/op-web << EOF
# op-dbus-v2 nginx configuration
# Domain: $DOMAIN
# Generated: $(date)

# Upstream servers
upstream op_web_backend {
    server 127.0.0.1:8081;
    keepalive 32;
}

upstream op_mcp_backend {
    server 127.0.0.1:8082;
    keepalive 32;
}

# HTTP redirect to HTTPS
server {
    listen 80;
    listen [::]:80;
    server_name $DOMAIN *.$DOMAIN;
    return 301 https://\$host\$request_uri;
}

# Main HTTPS server
server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name $DOMAIN *.$DOMAIN;

    # SSL Configuration
    ssl_certificate /etc/nginx/ssl/$SAFE_DOMAIN.crt;
    ssl_certificate_key /etc/nginx/ssl/$SAFE_DOMAIN.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 1d;

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;

    # Logging
    access_log $LOG_DIR/nginx-access.log;
    error_log $LOG_DIR/nginx-error.log;

    # Root location
    location / {
        proxy_pass http://op_web_backend;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    # WebSocket support
    location /ws {
        proxy_pass http://op_web_backend/ws;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_read_timeout 86400;
    }

    # MCP endpoint
    location /mcp/ {
        proxy_pass http://op_mcp_backend/;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    # Health check
    location /health {
        proxy_pass http://op_web_backend/api/health;
        proxy_http_version 1.1;
    }

    # Static files
    location /static/ {
        alias $PROJECT_DIR/static/;
        expires 1d;
        add_header Cache-Control "public, immutable";
    }
}
EOF

    # Enable site
    ln -sf /etc/nginx/sites-available/op-web /etc/nginx/sites-enabled/op-web
    
    # Remove default site if exists
    rm -f /etc/nginx/sites-enabled/default
    
    log_success "Nginx configured"
else
    log_step 7 "Skipping nginx setup (SETUP_NGINX=false)"
fi

#===============================================================================
# STEP 8: SETUP TLS CERTIFICATES
#===============================================================================

if [ "$SETUP_TLS" = "true" ]; then
    log_step 8 "Setting up TLS certificates..."
    
    CERT_PATH="/etc/nginx/ssl/$SAFE_DOMAIN.crt"
    KEY_PATH="/etc/nginx/ssl/$SAFE_DOMAIN.key"
    
    # Check if certificate already exists
    if [ -f "$CERT_PATH" ] && [ -f "$KEY_PATH" ]; then
        # Verify it's valid
        if openssl x509 -in "$CERT_PATH" -noout -checkend 86400 2>/dev/null; then
            log_success "Valid certificate already exists: $CERT_PATH"
        else
            log_warning "Certificate exists but is expired or invalid"
            rm -f "$CERT_PATH" "$KEY_PATH"
        fi
    fi
    
    # Generate certificate if needed
    if [ ! -f "$CERT_PATH" ]; then
        # Check for Cloudflare setup script
        if [ -f "$PROJECT_DIR/deploy/setup-cloudflare-tls.sh" ] && [ -n "$CF_DNS_ZONE_TOKEN" ]; then
            log_info "Running Cloudflare TLS setup..."
            bash "$PROJECT_DIR/deploy/setup-cloudflare-tls.sh" || {
                log_warning "Cloudflare TLS setup failed, creating self-signed certificate..."
            }
        fi
        
        # Fallback to self-signed if still no cert
        if [ ! -f "$CERT_PATH" ]; then
            log_info "Generating self-signed certificate..."
            openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
                -keyout "$KEY_PATH" \
                -out "$CERT_PATH" \
                -subj "/CN=$DOMAIN/O=op-dbus/C=US" \
                2>/dev/null
            chmod 600 "$KEY_PATH"
            chmod 644 "$CERT_PATH"
            log_success "Self-signed certificate created"
            log_warning "For production, run: sudo $PROJECT_DIR/deploy/setup-cloudflare-tls.sh"
        fi
    fi
else
    log_step 8 "Skipping TLS setup (SETUP_TLS=false)"
fi

#===============================================================================
# STEP 9: START SERVICES
#===============================================================================

log_step 9 "Starting services..."

# Start op-web
if [ -f /etc/systemd/system/op-web.service ]; then
    log_info "Starting op-web..."
    systemctl start op-web.service
    sleep 2
    if systemctl is-active --quiet op-web.service; then
        log_success "op-web started"
    else
        log_warning "op-web failed to start. Check: journalctl -u op-web -n 50"
    fi
fi

# Start op-mcp
if [ -f /etc/systemd/system/op-mcp.service ]; then
    log_info "Starting op-mcp..."
    systemctl start op-mcp.service
    sleep 2
    if systemctl is-active --quiet op-mcp.service; then
        log_success "op-mcp started"
    else
        log_warning "op-mcp failed to start"
    fi
fi

# Start/reload nginx
if command -v nginx &> /dev/null; then
    log_info "Starting nginx..."
    systemctl enable nginx
    if nginx -t 2>/dev/null; then
        systemctl reload nginx 2>/dev/null || systemctl start nginx
        log_success "Nginx started"
    else
        log_warning "Nginx config invalid - check certificates"
    fi
fi

#===============================================================================
# STEP 10: VERIFICATION
#===============================================================================

log_step 10 "Verifying installation..."

echo ""
echo -e "${CYAN}Service Status:${NC}"

for service in op-web op-mcp nginx; do
    echo -n "  $service: "
    if systemctl is-active --quiet $service 2>/dev/null; then
        echo -e "${GREEN}running${NC}"
    elif systemctl is-enabled --quiet $service 2>/dev/null; then
        echo -e "${YELLOW}enabled but not running${NC}"
    else
        echo -e "${RED}not configured${NC}"
    fi
done

echo ""
echo -e "${CYAN}Connectivity Tests:${NC}"

# Test local backend
echo -n "  Backend (127.0.0.1:8081): "
if curl -sf --connect-timeout 5 http://127.0.0.1:8081/api/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${YELLOW}⚠ Not responding${NC}"
fi

# Test HTTPS
echo -n "  HTTPS (localhost:443): "
if curl -sfk --connect-timeout 5 https://localhost/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${YELLOW}⚠ Not responding${NC}"
fi

#===============================================================================
# SUMMARY
#===============================================================================

echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║                    Installation Complete!                       ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${CYAN}Installed Binaries:${NC}"
ls "$INSTALL_DIR"/op* 2>/dev/null | while read f; do
    echo -e "  ${YELLOW}$f${NC}"
done || echo "  (none)"
echo ""
echo -e "${CYAN}Configuration Files:${NC}"
echo -e "  Environment:  ${YELLOW}$ENV_FILE${NC}"
echo -e "  Nginx:        ${YELLOW}/etc/nginx/sites-available/op-web${NC}"
echo ""
echo -e "${CYAN}Access Points:${NC}"
echo -e "  Local:        ${YELLOW}http://localhost:8081/${NC}"
echo -e "  HTTPS:        ${YELLOW}https://$DOMAIN/${NC}"
echo ""
echo -e "${CYAN}Useful Commands:${NC}"
echo -e "  Status:       ${YELLOW}systemctl status op-web op-mcp${NC}"
echo -e "  Logs:         ${YELLOW}journalctl -u op-web -f${NC}"
echo -e "  Restart:      ${YELLOW}systemctl restart op-web${NC}"
echo -e "  Upgrade:      ${YELLOW}$PROJECT_DIR/deploy/upgrade.sh${NC}"
echo ""
