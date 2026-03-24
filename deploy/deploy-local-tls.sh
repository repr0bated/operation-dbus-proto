#!/bin/bash
# Deploy op-dbus-v2 on local server with TLS
# Checks /media/ and standard locations for existing certificates

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  op-dbus-v2 Local Deployment with TLS${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""

# Configuration
PROJECT_DIR="/home/jeremy/op-dbus-v2"
SSL_DIR="/etc/nginx/ssl"
SERVICE_USER="jeremy"
BACKEND_PORT="8081"

# Check if running as root for system operations
if [ "$EUID" -ne 0 ]; then
    echo -e "${YELLOW}Note: Some operations require sudo. You'll be prompted.${NC}"
fi

#===============================================================================
# STEP 1: Find TLS Certificates
#===============================================================================
echo -e "\n${BLUE}[1/6] Searching for TLS certificates...${NC}"

CERT_FOUND=false
CERT_PATH=""
KEY_PATH=""

# Priority 1: Check /media/ for previous TLS configuration
echo "  Checking /media/ for certificates..."
for media_dir in /media/*; do
    if [ -d "$media_dir" ]; then
        # Look for common certificate patterns
        for pattern in "ssl" "certs" "certificates" "tls" ".ssl" "letsencrypt"; do
            if [ -d "$media_dir/$pattern" ]; then
                # Check for cert files
                for cert in "$media_dir/$pattern"/*.{crt,pem,cert}; do
                    if [ -f "$cert" ]; then
                        # Try to find matching key
                        base=$(basename "$cert" | sed 's/\.[^.]*$//')
                        for key in "$media_dir/$pattern"/${base}*.{key,pem}; do
                            if [ -f "$key" ] && [ "$key" != "$cert" ]; then
                                echo -e "  ${GREEN}✓ Found certificates in $media_dir/$pattern${NC}"
                                CERT_PATH="$cert"
                                KEY_PATH="$key"
                                CERT_FOUND=true
                                break 3
                            fi
                        done
                    fi
                done
            fi
        done
        
        # Check root of media for certs
        for cert in "$media_dir"/*.{crt,pem}; do
            if [ -f "$cert" ]; then
                base=$(basename "$cert" | sed 's/\.[^.]*$//')
                for key in "$media_dir"/${base}*.key "$media_dir"/*.key; do
                    if [ -f "$key" ]; then
                        echo -e "  ${GREEN}✓ Found certificates in $media_dir${NC}"
                        CERT_PATH="$cert"
                        KEY_PATH="$key"
                        CERT_FOUND=true
                        break 2
                    fi
                done
            fi
        done
    fi
done 2>/dev/null

# Priority 2: Check existing nginx SSL directory
if [ "$CERT_FOUND" = false ]; then
    echo "  Checking /etc/nginx/ssl/..."
    if [ -f "/etc/nginx/ssl/ghostbridge.crt" ] && [ -f "/etc/nginx/ssl/ghostbridge.key" ]; then
        echo -e "  ${GREEN}✓ Found existing nginx certificates${NC}"
        CERT_PATH="/etc/nginx/ssl/ghostbridge.crt"
        KEY_PATH="/etc/nginx/ssl/ghostbridge.key"
        CERT_FOUND=true
    fi
fi

# Priority 3: Check Proxmox PVE certificates
if [ "$CERT_FOUND" = false ]; then
    echo "  Checking Proxmox certificates..."
    HOSTNAME=$(hostname)
    if [ -f "/etc/pve/nodes/$HOSTNAME/pve-ssl.pem" ]; then
        echo -e "  ${GREEN}✓ Found Proxmox certificates${NC}"
        CERT_PATH="/etc/pve/nodes/$HOSTNAME/pve-ssl.pem"
        KEY_PATH="/etc/pve/nodes/$HOSTNAME/pve-ssl.key"
        CERT_FOUND=true
    fi
fi

# Priority 4: Check Let's Encrypt
if [ "$CERT_FOUND" = false ]; then
    echo "  Checking Let's Encrypt..."
    for domain in proxmox.ghostbridge.tech ghostbridge.tech $(hostname -f) $(hostname); do
        if [ -f "/etc/letsencrypt/live/$domain/fullchain.pem" ]; then
            echo -e "  ${GREEN}✓ Found Let's Encrypt certificates for $domain${NC}"
            CERT_PATH="/etc/letsencrypt/live/$domain/fullchain.pem"
            KEY_PATH="/etc/letsencrypt/live/$domain/privkey.pem"
            CERT_FOUND=true
            break
        fi
    done
fi

# Priority 5: Generate self-signed if nothing found
if [ "$CERT_FOUND" = false ]; then
    echo -e "  ${YELLOW}No certificates found. Generating self-signed...${NC}"
    sudo mkdir -p "$SSL_DIR"
    sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
        -keyout "$SSL_DIR/ghostbridge.key" \
        -out "$SSL_DIR/ghostbridge.crt" \
        -subj "/CN=$(hostname)/O=op-dbus-v2/C=US" 2>/dev/null
    CERT_PATH="$SSL_DIR/ghostbridge.crt"
    KEY_PATH="$SSL_DIR/ghostbridge.key"
    CERT_FOUND=true
    echo -e "  ${GREEN}✓ Generated self-signed certificate${NC}"
fi

# Copy certificates to nginx SSL directory if needed
if [ "$CERT_PATH" != "$SSL_DIR/ghostbridge.crt" ]; then
    echo "  Copying certificates to $SSL_DIR..."
    sudo mkdir -p "$SSL_DIR"
    sudo cp "$CERT_PATH" "$SSL_DIR/ghostbridge.crt"
    sudo cp "$KEY_PATH" "$SSL_DIR/ghostbridge.key"
    sudo chmod 644 "$SSL_DIR/ghostbridge.crt"
    sudo chmod 600 "$SSL_DIR/ghostbridge.key"
fi

echo -e "  ${GREEN}Certificate: $CERT_PATH${NC}"
echo -e "  ${GREEN}Key: $KEY_PATH${NC}"

# Verify certificate
echo "  Verifying certificate..."
if openssl x509 -in "$SSL_DIR/ghostbridge.crt" -noout -dates 2>/dev/null; then
    EXPIRY=$(openssl x509 -in "$SSL_DIR/ghostbridge.crt" -noout -enddate | cut -d= -f2)
    echo -e "  ${GREEN}✓ Certificate valid until: $EXPIRY${NC}"
else
    echo -e "  ${RED}✗ Certificate verification failed${NC}"
fi

#===============================================================================
# STEP 2: Build Binaries
#===============================================================================
echo -e "\n${BLUE}[2/6] Building binaries...${NC}"

cd "$PROJECT_DIR"

# Check if rebuild needed
if [ ! -f "target/release/op-web-server" ] || [ "$(find crates -newer target/release/op-web-server -name '*.rs' 2>/dev/null | head -1)" ]; then
    echo "  Building op-web and op-mcp..."
    cargo build --release -p op-web -p op-mcp 2>&1 | tail -5
    echo -e "  ${GREEN}✓ Build complete${NC}"
else
    echo -e "  ${GREEN}✓ Binaries up to date${NC}"
fi

# Verify binaries exist
for binary in op-web-server op-mcp-server; do
    if [ -f "target/release/$binary" ]; then
        echo -e "  ${GREEN}✓ $binary exists${NC}"
    else
        echo -e "  ${RED}✗ $binary not found${NC}"
        exit 1
    fi
done

#===============================================================================
# STEP 3: Create Environment File
#===============================================================================
echo -e "\n${BLUE}[3/6] Creating environment file...${NC}"

ENV_FILE="/home/$SERVICE_USER/.op-web.env"

cat > "$ENV_FILE" << EOF
# op-web Environment Variables
# Generated by deploy-local-tls.sh on $(date)

# API Keys (loaded from user environment)
HF_TOKEN=${HF_TOKEN:-}
GITHUB_PERSONAL_ACCESS_TOKEN=${GH_TOKEN:-}
HUGGINGFACE_API_KEY=${HF_TOKEN:-}
CLOUDFLARE_API_TOKEN=${CF_DNS_ZONE_TOKEN:-}
CLOUDFLARE_ACCOUNT_ID=${CF_ACCOUNT_ID:-}
PINECONE_API_KEY=${PINECONE_API_KEY:-}

# Server Configuration
MCP_CONFIG_FILE=$PROJECT_DIR/crates/op-mcp/mcp-config.json
RUST_LOG=info,op_web=debug,op_chat=debug,op_tools=debug

# TLS Configuration (for direct HTTPS mode)
SSL_CERT_PATH=$SSL_DIR/ghostbridge.crt
SSL_KEY_PATH=$SSL_DIR/ghostbridge.key
HTTPS_ENABLED=true

# Server Ports
HTTP_PORT=8080
HTTPS_PORT=8443
BIND_HOST=0.0.0.0
EOF

chmod 600 "$ENV_FILE"
echo -e "  ${GREEN}✓ Environment file created: $ENV_FILE${NC}"

#===============================================================================
# STEP 4: Configure Nginx
#===============================================================================
echo -e "\n${BLUE}[4/6] Configuring Nginx...${NC}"

# Check if nginx is installed
if ! command -v nginx &> /dev/null; then
    echo "  Installing nginx..."
    sudo apt update && sudo apt install -y nginx
fi

# Create nginx configuration
sudo tee /etc/nginx/sites-available/op-web > /dev/null << 'NGINX_EOF'
# op-web Chat Server with TLS
# Auto-generated by deploy-local-tls.sh

# Rate limiting zones
limit_req_zone $binary_remote_addr zone=api_limit:10m rate=10r/s;
limit_req_zone $binary_remote_addr zone=chat_limit:10m rate=5r/s;

# Backend upstream
upstream op_web_backend {
    server 127.0.0.1:8081;
    keepalive 32;
}

# HTTP -> HTTPS redirect
server {
    listen 80 default_server;
    listen [::]:80 default_server;
    server_name _;
    
    # Allow ACME challenges for Let's Encrypt
    location /.well-known/acme-challenge/ {
        root /var/www/certbot;
    }
    
    # Redirect everything else to HTTPS
    location / {
        return 301 https://$host$request_uri;
    }
}

# HTTPS server
server {
    listen 443 ssl http2 default_server;
    listen [::]:443 ssl http2 default_server;
    server_name _;
    
    # SSL Configuration
    ssl_certificate /etc/nginx/ssl/ghostbridge.crt;
    ssl_certificate_key /etc/nginx/ssl/ghostbridge.key;
    
    # Modern SSL settings
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:DHE-RSA-AES128-GCM-SHA256:DHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 1d;
    ssl_session_tickets off;
    
    # OCSP Stapling (if using real certificates)
    # ssl_stapling on;
    # ssl_stapling_verify on;
    
    # Security headers
    add_header Strict-Transport-Security "max-age=63072000" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
    
    # Logging
    access_log /var/log/nginx/op-web-access.log;
    error_log /var/log/nginx/op-web-error.log;
    
    # Root redirect to chat
    location = / {
        return 301 /chat/;
    }
    
    # Chat interface
    location /chat/ {
        limit_req zone=chat_limit burst=20 nodelay;
        
        proxy_pass http://op_web_backend/;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket support
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        
        # Timeouts for long-running requests
        proxy_read_timeout 300s;
        proxy_connect_timeout 75s;
        proxy_send_timeout 300s;
        
        # Disable buffering for streaming
        proxy_buffering off;
        proxy_cache off;
    }
    
    # API endpoints
    location /api/ {
        limit_req zone=api_limit burst=50 nodelay;
        
        proxy_pass http://op_web_backend/api/;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # SSE support
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 86400s;
    }
    
    # WebSocket endpoint
    location /ws {
        proxy_pass http://op_web_backend/ws;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Long timeout for WebSocket
        proxy_read_timeout 86400s;
        proxy_send_timeout 86400s;
    }
    
    # MCP endpoints
    location /mcp {
        proxy_pass http://op_web_backend/mcp;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # SSE support for MCP
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 86400s;
    }
    
    # SSE streaming endpoint
    location /sse {
        proxy_pass http://op_web_backend/sse;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # SSE specific settings
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 86400s;
        chunked_transfer_encoding off;
    }
    
    # Static assets with caching
    location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2|ttf|eot)$ {
        proxy_pass http://op_web_backend;
        expires 7d;
        add_header Cache-Control "public, immutable";
    }
    
    # Health check (no rate limiting)
    location /health {
        proxy_pass http://op_web_backend/api/health;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
    }
}
NGINX_EOF

# Enable the site
sudo ln -sf /etc/nginx/sites-available/op-web /etc/nginx/sites-enabled/
sudo rm -f /etc/nginx/sites-enabled/default 2>/dev/null || true

# Test nginx configuration
if sudo nginx -t 2>&1; then
    echo -e "  ${GREEN}✓ Nginx configuration valid${NC}"
else
    echo -e "  ${RED}✗ Nginx configuration error${NC}"
    exit 1
fi

#===============================================================================
# STEP 5: Install Systemd Service
#===============================================================================
echo -e "\n${BLUE}[5/6] Installing systemd service...${NC}"

sudo tee /etc/systemd/system/op-web.service > /dev/null << EOF
[Unit]
Description=op-dbus-v2 Web Server
After=network.target dbus.service
Wants=dbus.service

[Service]
Type=simple
User=$SERVICE_USER
Group=$SERVICE_USER
WorkingDirectory=$PROJECT_DIR
EnvironmentFile=/home/$SERVICE_USER/.op-web.env
ExecStart=$PROJECT_DIR/target/release/op-web-server
Restart=always
RestartSec=5

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ReadWritePaths=/var/cache/op-dbus

# Resource limits
LimitNOFILE=65535
MemoryMax=2G

[Install]
WantedBy=multi-user.target
EOF

# Create cache directory
sudo mkdir -p /var/cache/op-dbus
sudo chown $SERVICE_USER:$SERVICE_USER /var/cache/op-dbus

# Reload systemd
sudo systemctl daemon-reload
echo -e "  ${GREEN}✓ Systemd service installed${NC}"

#===============================================================================
# STEP 6: Start Services
#===============================================================================
echo -e "\n${BLUE}[6/6] Starting services...${NC}"

# Enable and start op-web
sudo systemctl enable op-web
sudo systemctl restart op-web
sleep 2

if sudo systemctl is-active --quiet op-web; then
    echo -e "  ${GREEN}✓ op-web service started${NC}"
else
    echo -e "  ${RED}✗ op-web service failed to start${NC}"
    echo "  Check logs: sudo journalctl -u op-web -n 50"
fi

# Restart nginx
sudo systemctl enable nginx
sudo systemctl restart nginx

if sudo systemctl is-active --quiet nginx; then
    echo -e "  ${GREEN}✓ nginx started${NC}"
else
    echo -e "  ${RED}✗ nginx failed to start${NC}"
fi

#===============================================================================
# VERIFICATION
#===============================================================================
echo -e "\n${BLUE}Verifying deployment...${NC}"
sleep 2

# Test backend directly
echo -n "  Backend (localhost:8081): "
if curl -sf http://localhost:8081/api/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${YELLOW}⚠ Not responding yet${NC}"
fi

# Test HTTPS
echo -n "  HTTPS (localhost:443): "
if curl -sfk https://localhost/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${YELLOW}⚠ Not responding yet${NC}"
fi

# Show certificate info
echo -e "\n${BLUE}Certificate Information:${NC}"
openssl x509 -in "$SSL_DIR/ghostbridge.crt" -noout -subject -dates 2>/dev/null | sed 's/^/  /'

#===============================================================================
# SUMMARY
#===============================================================================
echo ""
echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  Deployment Complete!${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""
echo -e "${GREEN}Access Points:${NC}"
echo -e "  HTTPS:     ${BLUE}https://localhost/chat/${NC}"
echo -e "  HTTP:      ${BLUE}http://localhost/chat/${NC} (redirects to HTTPS)"
echo -e "  WebSocket: ${BLUE}wss://localhost/ws${NC}"
echo -e "  API:       ${BLUE}https://localhost/api/health${NC}"
echo -e "  MCP SSE:   ${BLUE}https://localhost/sse${NC}"
echo ""
echo -e "${GREEN}If you have a domain configured:${NC}"
HOSTNAME=$(hostname -f 2>/dev/null || hostname)
echo -e "  ${BLUE}https://$HOSTNAME/chat/${NC}"
echo ""
echo -e "${GREEN}Service Management:${NC}"
echo -e "  Status:    ${YELLOW}sudo systemctl status op-web nginx${NC}"
echo -e "  Logs:      ${YELLOW}sudo journalctl -u op-web -f${NC}"
echo -e "  Restart:   ${YELLOW}sudo systemctl restart op-web nginx${NC}"
echo ""
echo -e "${GREEN}TLS Certificate:${NC}"
echo -e "  Path:      ${BLUE}$SSL_DIR/ghostbridge.crt${NC}"
if openssl x509 -in "$SSL_DIR/ghostbridge.crt" -noout -checkend 0 2>/dev/null; then
    echo -e "  Status:    ${GREEN}Valid${NC}"
else
    echo -e "  Status:    ${RED}Expired or Invalid${NC}"
fi
echo ""
