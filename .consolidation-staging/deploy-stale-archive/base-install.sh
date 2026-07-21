#!/bin/bash
#
# OP-DBUS BASE INSTALLATION SCRIPT
# For fresh Proxmox installations
#
# This installs the complete non-modular base system:
# - Chat server with LLM integration
# - MCP protocol server
# - D-Bus integration service
# - All agents, plugins, and tools
# - Introspection and orchestration
#
# After this, the chatbot handles modular additions.
#

set -euo pipefail

# ============================================================================
# CONFIGURATION
# ============================================================================

INSTALL_DIR="/opt/op-dbus"
CONFIG_DIR="/etc/op-dbus"
DATA_DIR="/var/lib/op-dbus"
LOG_DIR="/var/log/op-dbus"
RUN_DIR="/run/op-dbus"
USER="op-dbus"
GROUP="op-dbus"

# Source directory (where we're installing from)
SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && cd .. && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# ============================================================================
# HELPER FUNCTIONS
# ============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

install_packages() {
    if command -v op-packagekit-install &> /dev/null; then
        if ! op-packagekit-install "$@"; then
            log_warn "op-packagekit-install failed, falling back to apt-get"
            apt-get install -y "$@"
        fi
        return
    fi

    apt-get install -y "$@"
}

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        exit 1
    fi
}

check_proxmox() {
    if ! command -v pveversion &> /dev/null; then
        log_warn "Proxmox not detected - continuing anyway"
    else
        log_info "Proxmox version: $(pveversion)"
    fi
}

# ============================================================================
# PHASE 1: SYSTEM PREPARATION
# ============================================================================

install_system_deps() {
    log_info "Installing system dependencies..."
    
    apt-get update
    
    # Core build tools
    install_packages \
        build-essential \
        pkg-config \
        libssl-dev \
        libsqlite3-dev \
        libdbus-1-dev \
        git \
        curl \
        wget \
        jq
    
    # Runtime dependencies
    install_packages \
        nginx \
        sqlite3 \
        dbus \
        systemd
    
    # Node.js for external MCP servers
    if ! command -v node &> /dev/null; then
        log_info "Installing Node.js..."
        curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
        install_packages nodejs
    fi
    
    log_success "System dependencies installed"
}

install_rust() {
    if command -v rustc &> /dev/null; then
        log_info "Rust already installed: $(rustc --version)"
        return
    fi
    
    log_info "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    
    log_success "Rust installed: $(rustc --version)"
}

create_user() {
    if id "$USER" &>/dev/null; then
        log_info "User $USER already exists"
        return
    fi
    
    log_info "Creating system user: $USER"
    useradd --system --no-create-home --shell /usr/sbin/nologin "$USER"
    usermod -aG dbus "$USER"
    
    log_success "User $USER created"
}

create_directories() {
    log_info "Creating directory structure..."
    
    mkdir -p "$INSTALL_DIR"/{bin,lib,share}
    mkdir -p "$CONFIG_DIR"/{agents,plugins,mcp}
    mkdir -p "$DATA_DIR"/{cache,sessions,snapshots}
    mkdir -p "$LOG_DIR"
    mkdir -p "$RUN_DIR"
    mkdir -p /var/www/op-dbus/static
    
    chown -R "$USER:$GROUP" "$DATA_DIR" "$LOG_DIR" "$RUN_DIR"
    chmod 750 "$DATA_DIR" "$LOG_DIR"
    chmod 755 "$RUN_DIR"
    
    log_success "Directories created"
}

# ============================================================================
# PHASE 2: BUILD AND INSTALL BINARIES
# ============================================================================

build_project() {
    log_info "Building op-dbus project..."
    
    cd "$SOURCE_DIR"
    
    # Ensure we have the latest Rust toolchain
    source "$HOME/.cargo/env" 2>/dev/null || true
    
    # Build all binaries in release mode
    cargo build --release \
        -p op-web \
        -p op-mcp \
        -p op-chat \
        -p op-agents \
        -p op-introspection \
        -p op-tools
    
    log_success "Project built successfully"
}

install_binaries() {
    log_info "Installing binaries..."
    
    cd "$SOURCE_DIR"
    
    # Main binaries
    local binaries=(
        "op-web-server"
        "op-mcp-server"
    )
    
    for bin in "${binaries[@]}"; do
        if [[ -f "target/release/$bin" ]]; then
            cp "target/release/$bin" "$INSTALL_DIR/bin/"
            chmod 755 "$INSTALL_DIR/bin/$bin"
            log_success "Installed: $bin"
        else
            log_warn "Binary not found: $bin"
        fi
    done
    
    # Create symlinks in /usr/local/bin
    for bin in "${binaries[@]}"; do
        if [[ -f "$INSTALL_DIR/bin/$bin" ]]; then
            ln -sf "$INSTALL_DIR/bin/$bin" "/usr/local/bin/$bin"
        fi
    done
    
    log_success "Binaries installed"
}

install_static_files() {
    log_info "Installing static files..."
    
    cd "$SOURCE_DIR"
    
    # Copy static web files
    if [[ -d "static" ]]; then
        cp -r static/* /var/www/op-dbus/static/
        chown -R www-data:www-data /var/www/op-dbus
    fi
    
    # Copy embedded agents and prompts
    if [[ -d "crates/op-agents/src/agents" ]]; then
        cp -r crates/op-agents/src/agents/* "$CONFIG_DIR/agents/"
    fi
    
    log_success "Static files installed"
}

# ============================================================================
# PHASE 3: CONFIGURE SERVICES
# ============================================================================

install_systemd_services() {
    log_info "Installing systemd services..."
    
    # op-chat-server.service
    cat > /etc/systemd/system/op-chat-server.service << 'EOF'
[Unit]
Description=OP-DBUS Chat Server
Documentation=https://github.com/ghostbridgetech/op-dbus
After=network.target dbus.service
Wants=dbus.service

[Service]
Type=simple
User=op-dbus
Group=op-dbus
EnvironmentFile=-/etc/op-dbus/environment
ExecStart=/opt/op-dbus/bin/op-web-server
Restart=always
RestartSec=5
WatchdogSec=30

# Security hardening
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=yes
ReadWritePaths=/var/lib/op-dbus /var/log/op-dbus /run/op-dbus

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=op-chat-server

[Install]
WantedBy=multi-user.target
EOF

    # op-mcp-server.service (for stdio-based MCP)
    cat > /etc/systemd/system/op-mcp-server.service << 'EOF'
[Unit]
Description=OP-DBUS MCP Protocol Server
Documentation=https://github.com/ghostbridgetech/op-dbus
After=network.target op-chat-server.service

[Service]
Type=simple
User=op-dbus
Group=op-dbus
EnvironmentFile=-/etc/op-dbus/environment
ExecStart=/opt/op-dbus/bin/op-mcp-server
Restart=always
RestartSec=5

# Security
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=yes
ReadWritePaths=/var/lib/op-dbus /var/log/op-dbus

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=op-mcp-server

[Install]
WantedBy=multi-user.target
EOF

    # op-dbus-agents.service (D-Bus agent spawner)
    cat > /etc/systemd/system/op-dbus-agents.service << 'EOF'
[Unit]
Description=OP-DBUS Agent Manager
Documentation=https://github.com/ghostbridgetech/op-dbus
After=dbus.service
Requires=dbus.service

[Service]
Type=dbus
BusName=org.opdbus.AgentManager
User=root
Group=root
EnvironmentFile=-/etc/op-dbus/environment
ExecStart=/opt/op-dbus/bin/op-agent-manager
Restart=on-failure
RestartSec=5

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=op-dbus-agents

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    
    log_success "Systemd services installed"
}

install_dbus_config() {
    log_info "Installing D-Bus configuration..."
    
    # D-Bus policy for op-dbus services
    cat > /etc/dbus-1/system.d/op-dbus.conf << 'EOF'
<!DOCTYPE busconfig PUBLIC
 "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <!-- Allow op-dbus user to own our bus names -->
  <policy user="op-dbus">
    <allow own_prefix="org.opdbus"/>
    <allow send_destination="org.opdbus.AgentManager"/>
    <allow send_destination="org.opdbus.Orchestrator"/>
    <allow send_destination="org.opdbus.Introspection"/>
  </policy>

  <!-- Allow root to own and send to our services -->
  <policy user="root">
    <allow own_prefix="org.opdbus"/>
    <allow send_destination="org.opdbus.AgentManager"/>
    <allow send_destination="org.opdbus.Orchestrator"/>
    <allow send_destination="org.opdbus.Introspection"/>
  </policy>

  <!-- Allow any user to call our methods (for chat interface) -->
  <policy context="default">
    <allow send_destination="org.opdbus.AgentManager"
           send_interface="org.opdbus.AgentManager"/>
    <allow send_destination="org.opdbus.Orchestrator"
           send_interface="org.opdbus.Orchestrator"/>
    <allow send_destination="org.opdbus.Introspection"
           send_interface="org.opdbus.Introspection"/>
    
    <!-- Allow introspection -->
    <allow send_destination="org.opdbus.AgentManager"
           send_interface="org.freedesktop.DBus.Introspectable"/>
    <allow send_destination="org.opdbus.Orchestrator"
           send_interface="org.freedesktop.DBus.Introspectable"/>
  </policy>
</busconfig>
EOF

    # Reload D-Bus config
    systemctl reload dbus || true
    
    log_success "D-Bus configuration installed"
}

install_nginx_config() {
    log_info "Installing Nginx configuration..."
    
    cat > /etc/nginx/sites-available/op-dbus << 'EOF'
# OP-DBUS Chat Server
server {
    listen 80;
    listen [::]:80;
    server_name _;
    
    # Redirect to HTTPS
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name _;
    
    # SSL certificates (will be configured by certbot or use Proxmox certs)
    ssl_certificate /etc/nginx/ssl/server.crt;
    ssl_certificate_key /etc/nginx/ssl/server.key;
    
    # SSL settings
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ecdh_curve X25519:prime256v1:secp384r1;
    ssl_prefer_server_ciphers on;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256;
    
    # Chat interface
    location /chat/ {
        proxy_pass http://127.0.0.1:8080/;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket timeout
        proxy_read_timeout 86400;
    }
    
    # API endpoints
    location /api/ {
        proxy_pass http://127.0.0.1:8080/api/;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
    
    # Static files
    location /static/ {
        alias /var/www/op-dbus/static/;
        expires 1d;
        add_header Cache-Control "public, immutable";
    }
    
    # Health check
    location /health {
        proxy_pass http://127.0.0.1:8080/api/health;
    }
    
    # Root redirect to chat
    location = / {
        return 302 /chat/;
    }
}
EOF

    # Create SSL directory
    mkdir -p /etc/nginx/ssl
    
    # Copy Proxmox certs if available, otherwise generate self-signed
    if [[ -f /etc/pve/nodes/$(hostname)/pve-ssl.pem ]]; then
        cp /etc/pve/nodes/$(hostname)/pve-ssl.pem /etc/nginx/ssl/server.crt
        cp /etc/pve/nodes/$(hostname)/pve-ssl.key /etc/nginx/ssl/server.key
        log_info "Using Proxmox SSL certificates"
    else
        # Generate self-signed cert
        openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
            -keyout /etc/nginx/ssl/server.key \
            -out /etc/nginx/ssl/server.crt \
            -subj "/CN=$(hostname)/O=OP-DBUS/C=US"
        log_info "Generated self-signed SSL certificate"
    fi
    
    chmod 600 /etc/nginx/ssl/server.key
    chmod 644 /etc/nginx/ssl/server.crt
    
    # Enable site
    ln -sf /etc/nginx/sites-available/op-dbus /etc/nginx/sites-enabled/
    rm -f /etc/nginx/sites-enabled/default
    
    # Test and reload
    nginx -t
    systemctl reload nginx
    
    log_success "Nginx configuration installed"
}

# ============================================================================
# PHASE 4: CONFIGURE ENVIRONMENT
# ============================================================================

create_environment_file() {
    log_info "Creating environment configuration..."
    
    cat > /etc/op-dbus/environment << 'EOF'
# OP-DBUS Environment Configuration
# This file is sourced by all op-dbus services

# Server settings
PORT=8080
BIND_ADDRESS=127.0.0.1
STATIC_DIR=/var/www/op-dbus/static

# Data directories
OP_DBUS_DATA_DIR=/var/lib/op-dbus
OP_DBUS_CACHE_DIR=/var/lib/op-dbus/cache
OP_DBUS_LOG_DIR=/var/log/op-dbus

# MCP configuration
MCP_CONFIG_FILE=/etc/op-dbus/mcp/servers.json

# Introspection cache
INTROSPECTION_CACHE_PATH=/var/lib/op-dbus/cache/introspection.db

# Logging
RUST_LOG=info,op_chat=debug,op_mcp=debug

# LLM Provider (default to gemini, can be changed to huggingface, anthropic)
LLM_PROVIDER=gemini

# API Keys (loaded from separate secrets file)
# Source additional secrets if they exist
EOF

    # Create secrets template
    cat > /etc/op-dbus/secrets.env.template << 'EOF'
# OP-DBUS Secrets Configuration
# Copy this to /etc/op-dbus/secrets.env and fill in your API keys

# HuggingFace
HF_TOKEN=

# GitHub
GITHUB_PERSONAL_ACCESS_TOKEN=

# Pinecone (for vector memory)
PINECONE_API_KEY=

# Brave Search
BRAVE_API_KEY=

# Cloudflare
CF_ACCOUNT_ID=
CF_API_TOKEN=
EOF

    chmod 640 /etc/op-dbus/environment
    chmod 640 /etc/op-dbus/secrets.env.template
    chown root:op-dbus /etc/op-dbus/environment
    
    log_success "Environment configuration created"
}

create_mcp_config() {
    log_info "Creating MCP server configuration..."
    
    cat > /etc/op-dbus/mcp/servers.json << 'EOF'
[
  {
    "name": "filesystem",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home", "/etc", "/var/log"],
    "auth_method": "none",
    "enabled": true
  },
  {
    "name": "memory",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-memory"],
    "auth_method": "none",
    "enabled": true
  },
  {
    "name": "sequential-thinking",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-sequential-thinking"],
    "auth_method": "none",
    "enabled": true
  },
  {
    "name": "github",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-github"],
    "api_key_env": "GITHUB_PERSONAL_ACCESS_TOKEN",
    "auth_method": "env_var",
    "enabled": false,
    "comment": "Enable after setting GITHUB_PERSONAL_ACCESS_TOKEN in secrets.env"
  },
  {
    "name": "brave-search",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-brave-search"],
    "api_key_env": "BRAVE_API_KEY",
    "auth_method": "env_var",
    "enabled": false,
    "comment": "Enable after setting BRAVE_API_KEY in secrets.env"
  }
]
EOF

    chmod 644 /etc/op-dbus/mcp/servers.json
    
    log_success "MCP configuration created"
}

# ============================================================================
# PHASE 5: INSTALL EXTERNAL MCP SERVERS
# ============================================================================

install_external_mcp_servers() {
    log_info "Pre-installing external MCP servers..."
    
    # Install commonly used MCP servers globally
    npm install -g \
        @modelcontextprotocol/server-filesystem \
        @modelcontextprotocol/server-memory \
        @modelcontextprotocol/server-sequential-thinking \
        @modelcontextprotocol/server-github \
        @modelcontextprotocol/server-brave-search \
        @modelcontextprotocol/server-fetch \
        @modelcontextprotocol/server-puppeteer \
        2>/dev/null || log_warn "Some MCP servers failed to install (optional)"
    
    log_success "External MCP servers installed"
}

# ============================================================================
# PHASE 6: INITIALIZE DATABASE AND CACHE
# ============================================================================

initialize_databases() {
    log_info "Initializing databases..."
    
    # Create introspection cache database
    sqlite3 "$DATA_DIR/cache/introspection.db" << 'EOF'
CREATE TABLE IF NOT EXISTS introspection_cache (
    service_name TEXT NOT NULL,
    object_path TEXT NOT NULL,
    interface_name TEXT NOT NULL,
    cached_at INTEGER NOT NULL,
    introspection_json TEXT NOT NULL,
    PRIMARY KEY (service_name, object_path, interface_name)
);

CREATE INDEX IF NOT EXISTS idx_service ON introspection_cache(service_name);
CREATE INDEX IF NOT EXISTS idx_cached_at ON introspection_cache(cached_at);

CREATE TABLE IF NOT EXISTS service_methods (
    service_name TEXT NOT NULL,
    interface_name TEXT NOT NULL,
    method_name TEXT NOT NULL,
    signature_json TEXT NOT NULL,
    PRIMARY KEY (service_name, interface_name, method_name)
);

CREATE TABLE IF NOT EXISTS service_properties (
    service_name TEXT NOT NULL,
    interface_name TEXT NOT NULL,
    property_name TEXT NOT NULL,
    type_json TEXT NOT NULL,
    access TEXT NOT NULL,
    PRIMARY KEY (service_name, interface_name, property_name)
);
EOF

    # Create session database
    sqlite3 "$DATA_DIR/sessions/sessions.db" << 'EOF'
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    metadata_json TEXT
);

CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    tool_calls_json TEXT,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_session_messages ON messages(session_id, timestamp);
EOF

    chown -R "$USER:$GROUP" "$DATA_DIR"
    
    log_success "Databases initialized"
}

# ============================================================================
# PHASE 7: START SERVICES
# ============================================================================

enable_and_start_services() {
    log_info "Enabling and starting services..."
    
    # Enable services
    systemctl enable op-chat-server
    systemctl enable nginx
    
    # Start services
    systemctl start op-chat-server
    systemctl start nginx
    
    # Wait for services to be ready
    sleep 3
    
    # Check status
    if systemctl is-active --quiet op-chat-server; then
        log_success "op-chat-server is running"
    else
        log_error "op-chat-server failed to start"
        journalctl -u op-chat-server -n 20 --no-pager
    fi
    
    if systemctl is-active --quiet nginx; then
        log_success "nginx is running"
    else
        log_error "nginx failed to start"
        journalctl -u nginx -n 20 --no-pager
    fi
}

# ============================================================================
# PHASE 8: POST-INSTALL VERIFICATION
# ============================================================================

verify_installation() {
    log_info "Verifying installation..."
    
    local errors=0
    
    # Check binaries
    for bin in op-web-server op-mcp-server; do
        if [[ -x "$INSTALL_DIR/bin/$bin" ]]; then
            log_success "Binary exists: $bin"
        else
            log_error "Binary missing: $bin"
            ((errors++))
        fi
    done
    
    # Check services
    for svc in op-chat-server nginx; do
        if systemctl is-active --quiet "$svc"; then
            log_success "Service running: $svc"
        else
            log_warn "Service not running: $svc"
        fi
    done
    
    # Check ports
    if ss -tlnp | grep -q ':8080'; then
        log_success "Port 8080 is listening (chat server)"
    else
        log_warn "Port 8080 not listening"
    fi
    
    if ss -tlnp | grep -q ':443'; then
        log_success "Port 443 is listening (nginx)"
    else
        log_warn "Port 443 not listening"
    fi
    
    # Check health endpoint
    if curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8080/api/health | grep -q '200'; then
        log_success "Health endpoint responding"
    else
        log_warn "Health endpoint not responding (may need a moment)"
    fi
    
    if [[ $errors -gt 0 ]]; then
        log_error "Installation completed with $errors errors"
        return 1
    fi
    
    log_success "Installation verified successfully"
}

print_summary() {
    local ip=$(hostname -I | awk '{print $1}')
    
    echo
    echo "============================================================================"
    echo -e "${GREEN}OP-DBUS BASE INSTALLATION COMPLETE${NC}"
    echo "============================================================================"
    echo
    echo "Access Points:"
    echo "  Chat Interface:  https://$ip/chat/"
    echo "  API Health:      https://$ip/api/health"
    echo "  Proxmox UI:      https://$ip:8006 (unchanged)"
    echo
    echo "Service Management:"
    echo "  systemctl status op-chat-server"
    echo "  systemctl status nginx"
    echo "  journalctl -u op-chat-server -f"
    echo
    echo "Configuration Files:"
    echo "  Environment:     /etc/op-dbus/environment"
    echo "  Secrets:         /etc/op-dbus/secrets.env (create from template)"
    echo "  MCP Servers:     /etc/op-dbus/mcp/servers.json"
    echo "  Nginx:           /etc/nginx/sites-available/op-dbus"
    echo
    echo "Data Directories:"
    echo "  Cache:           /var/lib/op-dbus/cache"
    echo "  Sessions:        /var/lib/op-dbus/sessions"
    echo "  Logs:            /var/log/op-dbus"
    echo
    echo "Next Steps:"
    echo "  1. Copy /etc/op-dbus/secrets.env.template to /etc/op-dbus/secrets.env"
    echo "  2. Add your API keys (HF_TOKEN, GITHUB_TOKEN, etc.)"
    echo "  3. Restart services: systemctl restart op-chat-server"
    echo "  4. Open chat interface and let the AI complete setup"
    echo
    echo "The chatbot can now help you with:"
    echo "  • Installing additional MCP servers"
    echo "  • Loading dynamic agents from ~/agents/"
    echo "  • Creating VM/container templates"
    echo "  • Configuring custom workflows"
    echo "  • Taking system snapshots"
    echo
    echo "============================================================================"
}

# ============================================================================
# MAIN
# ============================================================================

main() {
    echo
    echo "============================================================================"
    echo "OP-DBUS BASE INSTALLATION"
    echo "============================================================================"
    echo
    
    check_root
    check_proxmox
    
    # Phase 1: System Preparation
    install_system_deps
    install_rust
    create_user
    create_directories
    
    # Phase 2: Build and Install
    build_project
    install_binaries
    install_static_files
    
    # Phase 3: Configure Services
    install_systemd_services
    install_dbus_config
    install_nginx_config
    
    # Phase 4: Environment
    create_environment_file
    create_mcp_config
    
    # Phase 5: External MCP
    install_external_mcp_servers
    
    # Phase 6: Databases
    initialize_databases
    
    # Phase 7: Start Services
    enable_and_start_services
    
    # Phase 8: Verify
    verify_installation
    print_summary
}

# Run main function
main "$@"
