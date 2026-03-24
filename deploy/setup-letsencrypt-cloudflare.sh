#!/bin/bash
# Let's Encrypt with Cloudflare DNS Challenge
# Interactive setup - prompts for domain and subdomains
# Version: 2.0

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }

echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║   Let's Encrypt + Cloudflare DNS Challenge Setup               ║${NC}"
echo -e "${GREEN}║   Interactive Configuration                                     ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check root
if [ "$EUID" -ne 0 ]; then
    log_error "Run as root (sudo)"
    exit 1
fi

#===============================================================================
# INTERACTIVE CONFIGURATION
#===============================================================================

echo -e "${CYAN}Domain Configuration${NC}"
echo "─────────────────────────────────────────────────────────────────"

# Get domain name
DEFAULT_DOMAIN="ghostbridge.tech"
read -p "Enter your domain name [$DEFAULT_DOMAIN]: " DOMAIN
DOMAIN=${DOMAIN:-$DEFAULT_DOMAIN}

# Validate domain
if [[ ! "$DOMAIN" =~ ^[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)*\.[a-zA-Z]{2,}$ ]]; then
    log_error "Invalid domain format: $DOMAIN"
    exit 1
fi

log_success "Domain: $DOMAIN"
echo ""

# Get subdomains
echo -e "${CYAN}Subdomain Configuration${NC}"
echo "─────────────────────────────────────────────────────────────────"
echo "Enter subdomains (comma-separated, no spaces)."
echo "Example: proxmox,op-web,chat,api"
echo ""

DEFAULT_SUBDOMAINS="proxmox,op-web,mcp-tools,agents,chat"
read -p "Subdomains [$DEFAULT_SUBDOMAINS]: " SUBDOMAINS
SUBDOMAINS=${SUBDOMAINS:-$DEFAULT_SUBDOMAINS}

IFS=',' read -ra SUBDOMAIN_ARRAY <<< "$SUBDOMAINS"

log_success "Subdomains: $SUBDOMAINS"
echo ""

# Get email
echo -e "${CYAN}Contact Email${NC}"
echo "─────────────────────────────────────────────────────────────────"
DEFAULT_EMAIL="admin@$DOMAIN"
read -p "Email for Let's Encrypt notifications [$DEFAULT_EMAIL]: " EMAIL
EMAIL=${EMAIL:-$DEFAULT_EMAIL}

log_success "Email: $EMAIL"
echo ""

# Get Cloudflare API token
echo -e "${CYAN}Cloudflare Credentials${NC}"
echo "─────────────────────────────────────────────────────────────────"

CF_API_TOKEN="${CF_DNS_ZONE_TOKEN:-${CF_API_TOKEN:-}}"

if [ -z "$CF_API_TOKEN" ]; then
    echo "Cloudflare API Token not found in environment."
    echo "Create one at: https://dash.cloudflare.com/profile/api-tokens"
    echo "Required permissions: Zone:DNS:Edit"
    read -sp "Enter Cloudflare API Token: " CF_API_TOKEN
    echo ""
fi

if [ -z "$CF_API_TOKEN" ]; then
    log_error "Cloudflare API Token is required"
    exit 1
fi

log_success "API Token configured"
echo ""

# Confirm
echo -e "${CYAN}Configuration Summary${NC}"
echo "─────────────────────────────────────────────────────────────────"
echo -e "  Domain:      ${YELLOW}$DOMAIN${NC}"
echo -e "  Subdomains:  ${YELLOW}$SUBDOMAINS${NC}"
echo -e "  Email:       ${YELLOW}$EMAIL${NC}"
echo ""
read -p "Proceed? [Y/n]: " CONFIRM

if [[ "$CONFIRM" =~ ^[Nn]$ ]]; then
    log_info "Setup cancelled"
    exit 0
fi

echo ""

#===============================================================================
# STEP 1: Install certbot and cloudflare plugin
#===============================================================================

log_info "Installing certbot and cloudflare plugin..."

if ! command -v certbot &> /dev/null; then
    apt update
    apt install -y certbot python3-certbot-dns-cloudflare
    log_success "Certbot installed"
else
    log_success "Certbot already installed"
    apt install -y python3-certbot-dns-cloudflare 2>/dev/null || true
fi

#===============================================================================
# STEP 2: Create Cloudflare credentials file
#===============================================================================

log_info "Creating Cloudflare credentials file..."

CREDS_DIR="/root/.secrets"
CREDS_FILE="$CREDS_DIR/cloudflare.ini"

mkdir -p "$CREDS_DIR"
chmod 700 "$CREDS_DIR"

cat > "$CREDS_FILE" << EOF
# Cloudflare API credentials for certbot
# Domain: $DOMAIN
# Generated: $(date)

dns_cloudflare_api_token = $CF_API_TOKEN
EOF

chmod 600 "$CREDS_FILE"
log_success "Credentials file created: $CREDS_FILE"

#===============================================================================
# STEP 3: Request certificate
#===============================================================================

log_info "Requesting Let's Encrypt certificate..."

# Build domain list
DOMAIN_ARGS="-d $DOMAIN -d *.$DOMAIN"
for sub in "${SUBDOMAIN_ARRAY[@]}"; do
    sub=$(echo "$sub" | xargs)
    DOMAIN_ARGS="$DOMAIN_ARGS -d $sub.$DOMAIN"
done

log_info "Domains: $DOMAIN_ARGS"

certbot certonly \
    --dns-cloudflare \
    --dns-cloudflare-credentials "$CREDS_FILE" \
    --dns-cloudflare-propagation-seconds 30 \
    $DOMAIN_ARGS \
    --email "$EMAIL" \
    --agree-tos \
    --non-interactive \
    --preferred-challenges dns-01

if [ $? -eq 0 ]; then
    log_success "Certificate obtained successfully!"
else
    log_error "Certificate request failed"
    exit 1
fi

#===============================================================================
# STEP 4: Copy to nginx location
#===============================================================================

log_info "Installing certificate for nginx..."

NGINX_SSL_DIR="/etc/nginx/ssl"
mkdir -p "$NGINX_SSL_DIR"

CERT_DIR="/etc/letsencrypt/live/$DOMAIN"
SAFE_DOMAIN=$(echo "$DOMAIN" | tr '.' '-')

if [ -d "$CERT_DIR" ]; then
    ln -sf "$CERT_DIR/fullchain.pem" "$NGINX_SSL_DIR/$SAFE_DOMAIN.crt"
    ln -sf "$CERT_DIR/privkey.pem" "$NGINX_SSL_DIR/$SAFE_DOMAIN.key"
    
    # Also create standard names
    ln -sf "$CERT_DIR/fullchain.pem" "$NGINX_SSL_DIR/letsencrypt.crt"
    ln -sf "$CERT_DIR/privkey.pem" "$NGINX_SSL_DIR/letsencrypt.key"
    
    log_success "Certificate linked to $NGINX_SSL_DIR"
else
    log_error "Certificate directory not found: $CERT_DIR"
    exit 1
fi

#===============================================================================
# STEP 5: Setup auto-renewal
#===============================================================================

log_info "Setting up auto-renewal..."

HOOK_DIR="/etc/letsencrypt/renewal-hooks/deploy"
mkdir -p "$HOOK_DIR"

cat > "$HOOK_DIR/reload-services.sh" << 'EOF'
#!/bin/bash
# Reload services after certificate renewal

if systemctl is-active --quiet nginx; then
    systemctl reload nginx
    echo "Nginx reloaded"
fi

if systemctl is-active --quiet op-web; then
    systemctl restart op-web
    echo "op-web restarted"
fi
EOF

chmod +x "$HOOK_DIR/reload-services.sh"
log_success "Renewal hook created"

# Test renewal
log_info "Testing renewal process..."
certbot renew --dry-run

if [ $? -eq 0 ]; then
    log_success "Renewal test passed"
else
    log_warning "Renewal test failed - check configuration"
fi

#===============================================================================
# STEP 6: Reload services
#===============================================================================

log_info "Reloading services..."

if systemctl is-active --quiet nginx; then
    systemctl reload nginx
    log_success "Nginx reloaded"
fi

if systemctl is-active --quiet op-web; then
    systemctl restart op-web
    log_success "op-web restarted"
fi

#===============================================================================
# SUMMARY
#===============================================================================

echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║                    Setup Complete!                              ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${CYAN}Domain:${NC} ${YELLOW}$DOMAIN${NC}"
echo -e "${CYAN}Subdomains:${NC} ${YELLOW}$SUBDOMAINS${NC}"
echo ""
echo -e "${CYAN}Certificate Details:${NC}"
openssl x509 -in "$CERT_DIR/fullchain.pem" -noout -subject -dates | sed 's/^/  /'
echo ""
echo -e "${CYAN}Certificate Locations:${NC}"
echo -e "  Let's Encrypt: ${YELLOW}$CERT_DIR/${NC}"
echo -e "  Nginx:         ${YELLOW}$NGINX_SSL_DIR/$SAFE_DOMAIN.crt${NC}"
echo ""
echo -e "${CYAN}Auto-Renewal:${NC}"
echo -e "  Status:  ${GREEN}Enabled${NC} (via certbot timer)"
echo -e "  Hook:    ${YELLOW}$HOOK_DIR/reload-services.sh${NC}"
echo ""
echo -e "${CYAN}Commands:${NC}"
echo -e "  Check status:  ${YELLOW}certbot certificates${NC}"
echo -e "  Force renew:   ${YELLOW}certbot renew --force-renewal${NC}"
echo -e "  Test renewal:  ${YELLOW}certbot renew --dry-run${NC}"
echo ""
echo -e "${CYAN}Access Points:${NC}"
for sub in "${SUBDOMAIN_ARRAY[@]}"; do
    sub=$(echo "$sub" | xargs)
    echo -e "  ${YELLOW}https://$sub.$DOMAIN/${NC}"
done
echo ""
