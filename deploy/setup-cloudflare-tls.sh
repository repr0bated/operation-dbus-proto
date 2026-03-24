#!/bin/bash
# Cloudflare TLS Setup for op-dbus-v2
# Interactive setup - prompts for domain and subdomains
# Version: 2.0

set -e

#===============================================================================
# COLORS
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
# INTERACTIVE CONFIGURATION
#===============================================================================

echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║       Cloudflare TLS Setup for op-dbus-v2                       ║${NC}"
echo -e "${GREEN}║       Interactive Configuration                                 ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check root
if [ "$EUID" -ne 0 ]; then
    log_error "This script must be run as root (use sudo)"
    exit 1
fi

#-------------------------------------------------------------------------------
# Domain Configuration
#-------------------------------------------------------------------------------
echo -e "${CYAN}Domain Configuration${NC}"
echo "─────────────────────────────────────────────────────────────────"

# Get domain name
DEFAULT_DOMAIN="ghostbridge.tech"
read -p "Enter your domain name [$DEFAULT_DOMAIN]: " DOMAIN
DOMAIN=${DOMAIN:-$DEFAULT_DOMAIN}

# Validate domain format
if [[ ! "$DOMAIN" =~ ^[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)*\.[a-zA-Z]{2,}$ ]]; then
    log_error "Invalid domain format: $DOMAIN"
    exit 1
fi

log_success "Domain: $DOMAIN"
echo ""

#-------------------------------------------------------------------------------
# Subdomain Configuration
#-------------------------------------------------------------------------------
echo -e "${CYAN}Subdomain Configuration${NC}"
echo "─────────────────────────────────────────────────────────────────"
echo "Enter subdomains to include in the certificate."
echo "Separate multiple subdomains with commas (no spaces)."
echo "Example: proxmox,op-web,chat,api,mcp-tools"
echo ""

DEFAULT_SUBDOMAINS="proxmox,op-web,mcp-tools,agents,mcp-servers,chat"
read -p "Subdomains [$DEFAULT_SUBDOMAINS]: " SUBDOMAINS
SUBDOMAINS=${SUBDOMAINS:-$DEFAULT_SUBDOMAINS}

# Parse subdomains into array
IFS=',' read -ra SUBDOMAIN_ARRAY <<< "$SUBDOMAINS"

echo ""
log_info "Certificate will cover:"
echo -e "  ${YELLOW}$DOMAIN${NC} (root domain)"
echo -e "  ${YELLOW}*.$DOMAIN${NC} (wildcard)"
for sub in "${SUBDOMAIN_ARRAY[@]}"; do
    sub=$(echo "$sub" | xargs)  # Trim whitespace
    echo -e "  ${YELLOW}$sub.$DOMAIN${NC}"
done
echo ""

#-------------------------------------------------------------------------------
# Cloudflare Credentials
#-------------------------------------------------------------------------------
echo -e "${CYAN}Cloudflare Credentials${NC}"
echo "─────────────────────────────────────────────────────────────────"

# Try to get from environment first
CF_API_TOKEN="${CF_DNS_ZONE_TOKEN:-${CF_API_TOKEN:-}}"
CF_ZONE_ID="${CF_ZONE_ID:-}"
CF_ACCOUNT_ID="${CF_ACCOUNT_ID:-}"
CF_EMAIL="${CF_EMAIL:-}"

# Prompt for missing credentials
if [ -z "$CF_API_TOKEN" ]; then
    echo "Cloudflare API Token not found in environment."
    echo "Create one at: https://dash.cloudflare.com/profile/api-tokens"
    echo "Required permissions: Zone:DNS:Edit, Zone:SSL and Certificates:Edit"
    read -sp "Enter Cloudflare API Token: " CF_API_TOKEN
    echo ""
fi

if [ -z "$CF_API_TOKEN" ]; then
    log_error "Cloudflare API Token is required"
    exit 1
fi

log_success "API Token configured"

# Get Zone ID if not set
if [ -z "$CF_ZONE_ID" ]; then
    log_info "Fetching Zone ID for $DOMAIN..."
    ZONE_RESPONSE=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones?name=$DOMAIN" \
        -H "Authorization: Bearer $CF_API_TOKEN" \
        -H "Content-Type: application/json")
    
    CF_ZONE_ID=$(echo "$ZONE_RESPONSE" | jq -r '.result[0].id')
    
    if [ "$CF_ZONE_ID" = "null" ] || [ -z "$CF_ZONE_ID" ]; then
        log_error "Could not find Zone ID for $DOMAIN"
        log_info "Available zones:"
        echo "$ZONE_RESPONSE" | jq -r '.result[] | "  - \(.name): \(.id)"'
        echo ""
        read -p "Enter Zone ID manually: " CF_ZONE_ID
    fi
fi

if [ -z "$CF_ZONE_ID" ]; then
    log_error "Zone ID is required"
    exit 1
fi

log_success "Zone ID: $CF_ZONE_ID"
echo ""

#-------------------------------------------------------------------------------
# Certificate Options
#-------------------------------------------------------------------------------
echo -e "${CYAN}Certificate Options${NC}"
echo "─────────────────────────────────────────────────────────────────"

echo "Certificate validity period:"
echo "  1) 7 days"
echo "  2) 30 days"
echo "  3) 90 days"
echo "  4) 365 days (1 year)"
echo "  5) 730 days (2 years)"
echo "  6) 1095 days (3 years)"
echo "  7) 5475 days (15 years) - Maximum for Origin Certificates"
echo ""
read -p "Select validity period [7]: " VALIDITY_CHOICE

case ${VALIDITY_CHOICE:-7} in
    1) CERT_VALIDITY_DAYS=7 ;;
    2) CERT_VALIDITY_DAYS=30 ;;
    3) CERT_VALIDITY_DAYS=90 ;;
    4) CERT_VALIDITY_DAYS=365 ;;
    5) CERT_VALIDITY_DAYS=730 ;;
    6) CERT_VALIDITY_DAYS=1095 ;;
    7) CERT_VALIDITY_DAYS=5475 ;;
    *) CERT_VALIDITY_DAYS=5475 ;;
esac

log_success "Certificate validity: $CERT_VALIDITY_DAYS days"
echo ""

#-------------------------------------------------------------------------------
# Confirm Configuration
#-------------------------------------------------------------------------------
echo -e "${CYAN}Configuration Summary${NC}"
echo "─────────────────────────────────────────────────────────────────"
echo -e "  Domain:      ${YELLOW}$DOMAIN${NC}"
echo -e "  Subdomains:  ${YELLOW}$SUBDOMAINS${NC}"
echo -e "  Zone ID:     ${YELLOW}$CF_ZONE_ID${NC}"
echo -e "  Validity:    ${YELLOW}$CERT_VALIDITY_DAYS days${NC}"
echo ""
read -p "Proceed with this configuration? [Y/n]: " CONFIRM

if [[ "$CONFIRM" =~ ^[Nn]$ ]]; then
    log_info "Setup cancelled"
    exit 0
fi

echo ""

#===============================================================================
# CERTIFICATE PATHS
#===============================================================================
CF_SSL_DIR="/etc/ssl/cloudflare"
NGINX_SSL_DIR="/etc/nginx/ssl"
CERT_FILE="origin.pem"
KEY_FILE="origin.key"

#===============================================================================
# HELPER FUNCTIONS
#===============================================================================

cf_api() {
    local method=$1
    local endpoint=$2
    local data=$3
    
    if [ "$method" = "GET" ]; then
        curl -s -X GET "https://api.cloudflare.com/client/v4$endpoint" \
            -H "Authorization: Bearer $CF_API_TOKEN" \
            -H "Content-Type: application/json"
    else
        curl -s -X "$method" "https://api.cloudflare.com/client/v4$endpoint" \
            -H "Authorization: Bearer $CF_API_TOKEN" \
            -H "Content-Type: application/json" \
            -d "$data"
    fi
}

#===============================================================================
# STEP 1: VERIFY API ACCESS
#===============================================================================

TOTAL_STEPS=7

log_step 1 "Verifying Cloudflare API access..."

# Check required tools
for cmd in curl jq openssl; do
    if ! command -v $cmd &> /dev/null; then
        log_error "$cmd is required but not installed"
        exit 1
    fi
done

# Verify API token
VERIFY=$(cf_api GET "/user/tokens/verify")
if [ "$(echo "$VERIFY" | jq -r '.success')" != "true" ]; then
    log_error "API token verification failed"
    echo "$VERIFY" | jq '.errors'
    exit 1
fi

log_success "API token verified"

#===============================================================================
# STEP 2: CHECK FOR EXISTING CERTIFICATES
#===============================================================================

log_step 2 "Checking for existing certificates..."

EXISTING_CERT=""
EXISTING_KEY=""

# Check common locations
CERT_LOCATIONS=(
    "$CF_SSL_DIR/$CERT_FILE"
    "$NGINX_SSL_DIR/cloudflare.crt"
    "/etc/cloudflare/origin.pem"
    "/home/*/certs/cloudflare_origin.pem"
)

for cert_path in "${CERT_LOCATIONS[@]}"; do
    # Handle glob patterns
    for expanded_path in $cert_path; do
        if [ -f "$expanded_path" ]; then
            if openssl x509 -in "$expanded_path" -noout 2>/dev/null; then
                # Check if it's a Cloudflare cert and not expired
                issuer=$(openssl x509 -in "$expanded_path" -noout -issuer 2>/dev/null)
                if echo "$issuer" | grep -qi "cloudflare"; then
                    if openssl x509 -in "$expanded_path" -noout -checkend 0 2>/dev/null; then
                        log_info "Found valid Cloudflare certificate: $expanded_path"
                        expiry=$(openssl x509 -in "$expanded_path" -noout -enddate 2>/dev/null | sed 's/notAfter=//')
                        log_info "Expires: $expiry"
                        
                        # Find matching key
                        key_dir=$(dirname "$expanded_path")
                        for key_pattern in "origin.key" "privkey.pem" "*.key"; do
                            for key_file in "$key_dir"/$key_pattern; do
                                if [ -f "$key_file" ]; then
                                    # Verify key matches cert
                                    cert_mod=$(openssl x509 -in "$expanded_path" -noout -modulus 2>/dev/null | md5sum | awk '{print $1}')
                                    key_mod=$(openssl rsa -in "$key_file" -noout -modulus 2>/dev/null | md5sum | awk '{print $1}')
                                    if [ "$cert_mod" = "$key_mod" ]; then
                                        EXISTING_CERT="$expanded_path"
                                        EXISTING_KEY="$key_file"
                                        log_success "Found matching key: $key_file"
                                        break 3
                                    fi
                                fi
                            done
                        done
                    fi
                fi
            fi
        fi
    done
done

if [ -n "$EXISTING_CERT" ] && [ -n "$EXISTING_KEY" ]; then
    echo ""
    read -p "Use existing certificate? [Y/n]: " USE_EXISTING
    if [[ ! "$USE_EXISTING" =~ ^[Nn]$ ]]; then
        CERT_PATH="$EXISTING_CERT"
        KEY_PATH="$EXISTING_KEY"
        log_success "Using existing certificate"
    else
        EXISTING_CERT=""
        EXISTING_KEY=""
    fi
fi

#===============================================================================
# STEP 3: GENERATE NEW CERTIFICATE (if needed)
#===============================================================================

if [ -z "$EXISTING_CERT" ]; then
    log_step 3 "Generating Cloudflare Origin Certificate..."
    
    # Create SSL directory
    mkdir -p "$CF_SSL_DIR"
    chmod 700 "$CF_SSL_DIR"
    
    # Build hostnames list for JSON
    HOSTNAMES_JSON="\"$DOMAIN\", \"*.$DOMAIN\""
    for sub in "${SUBDOMAIN_ARRAY[@]}"; do
        sub=$(echo "$sub" | xargs)  # Trim whitespace
        HOSTNAMES_JSON="$HOSTNAMES_JSON, \"$sub.$DOMAIN\""
    done
    
    log_info "Requesting certificate for: $HOSTNAMES_JSON"
    
    # Generate private key locally
    log_info "Generating private key..."
    openssl genrsa -out "$CF_SSL_DIR/$KEY_FILE" 2048 2>/dev/null
    chmod 600 "$CF_SSL_DIR/$KEY_FILE"
    
    # Generate CSR
    CSR_FILE="$CF_SSL_DIR/origin.csr"
    openssl req -new -key "$CF_SSL_DIR/$KEY_FILE" \
        -out "$CSR_FILE" \
        -subj "/CN=$DOMAIN/O=Cloudflare Origin/C=US" \
        2>/dev/null
    
    CSR_CONTENT=$(cat "$CSR_FILE" | sed ':a;N;$!ba;s/\n/\\n/g')
    
    # Request certificate from Cloudflare
    log_info "Requesting Origin certificate from Cloudflare..."
    
    REQUEST_DATA="{
        \"hostnames\": [$HOSTNAMES_JSON],
        \"requested_validity\": $CERT_VALIDITY_DAYS,
        \"request_type\": \"origin-rsa\",
        \"csr\": \"$CSR_CONTENT\"
    }"
    
    RESPONSE=$(cf_api POST "/zones/$CF_ZONE_ID/origin_tls_client_auth" "$REQUEST_DATA")
    SUCCESS=$(echo "$RESPONSE" | jq -r '.success')
    
    if [ "$SUCCESS" != "true" ]; then
        log_warning "CSR-based request failed, trying Cloudflare-generated key..."
        
        # Fallback: Let Cloudflare generate the key
        REQUEST_DATA="{
            \"hostnames\": [$HOSTNAMES_JSON],
            \"requested_validity\": $CERT_VALIDITY_DAYS,
            \"request_type\": \"origin-rsa\"
        }"
        
        RESPONSE=$(cf_api POST "/certificates" "$REQUEST_DATA")
        SUCCESS=$(echo "$RESPONSE" | jq -r '.success')
        
        if [ "$SUCCESS" = "true" ]; then
            # Extract certificate and key from response
            CERT=$(echo "$RESPONSE" | jq -r '.result.certificate')
            KEY=$(echo "$RESPONSE" | jq -r '.result.private_key')
            
            if [ "$CERT" != "null" ] && [ -n "$CERT" ]; then
                echo "$CERT" > "$CF_SSL_DIR/$CERT_FILE"
            fi
            
            if [ "$KEY" != "null" ] && [ -n "$KEY" ]; then
                echo "$KEY" > "$CF_SSL_DIR/$KEY_FILE"
                chmod 600 "$CF_SSL_DIR/$KEY_FILE"
            fi
        else
            log_error "Failed to generate Origin certificate"
            echo "$RESPONSE" | jq '.errors'
            log_info ""
            log_info "You may need to generate the certificate manually:"
            log_info "1. Go to Cloudflare Dashboard"
            log_info "2. Select your domain: $DOMAIN"
            log_info "3. Go to SSL/TLS → Origin Server"
            log_info "4. Click 'Create Certificate'"
            log_info "5. Save the certificate and key to:"
            log_info "   Certificate: $CF_SSL_DIR/$CERT_FILE"
            log_info "   Key: $CF_SSL_DIR/$KEY_FILE"
            exit 1
        fi
    else
        # CSR-based request succeeded
        CERT=$(echo "$RESPONSE" | jq -r '.result.certificate')
        echo "$CERT" > "$CF_SSL_DIR/$CERT_FILE"
    fi
    
    # Clean up CSR
    rm -f "$CSR_FILE"
    
    # Verify certificate was saved
    if [ -f "$CF_SSL_DIR/$CERT_FILE" ] && [ -s "$CF_SSL_DIR/$CERT_FILE" ]; then
        chmod 644 "$CF_SSL_DIR/$CERT_FILE"
        CERT_PATH="$CF_SSL_DIR/$CERT_FILE"
        KEY_PATH="$CF_SSL_DIR/$KEY_FILE"
        log_success "Certificate saved to $CERT_PATH"
    else
        log_error "Certificate file is empty or missing"
        exit 1
    fi
else
    log_step 3 "Skipping certificate generation (using existing)"
fi

#===============================================================================
# STEP 4: VALIDATE CERTIFICATE
#===============================================================================

log_step 4 "Validating certificate..."

# Verify certificate format
if ! openssl x509 -in "$CERT_PATH" -noout 2>/dev/null; then
    log_error "Invalid certificate format"
    exit 1
fi
log_success "Certificate format valid"

# Verify key format
if ! openssl rsa -in "$KEY_PATH" -noout 2>/dev/null; then
    log_error "Invalid private key format"
    exit 1
fi
log_success "Private key format valid"

# Verify key matches certificate
CERT_MOD=$(openssl x509 -in "$CERT_PATH" -noout -modulus 2>/dev/null | md5sum | awk '{print $1}')
KEY_MOD=$(openssl rsa -in "$KEY_PATH" -noout -modulus 2>/dev/null | md5sum | awk '{print $1}')

if [ "$CERT_MOD" != "$KEY_MOD" ]; then
    log_error "Private key does not match certificate!"
    exit 1
fi
log_success "Private key matches certificate"

# Show certificate details
echo -e "\n${CYAN}Certificate Details:${NC}"
openssl x509 -in "$CERT_PATH" -noout -subject -issuer -dates -ext subjectAltName 2>/dev/null | sed 's/^/  /'

#===============================================================================
# STEP 5: INSTALL FOR NGINX
#===============================================================================

log_step 5 "Installing certificate for nginx..."

mkdir -p "$NGINX_SSL_DIR"

# Copy certificate and key (use domain name in filename)
SAFE_DOMAIN=$(echo "$DOMAIN" | tr '.' '-')
cp "$CERT_PATH" "$NGINX_SSL_DIR/$SAFE_DOMAIN.crt"
cp "$KEY_PATH" "$NGINX_SSL_DIR/$SAFE_DOMAIN.key"

# Also create standard names
cp "$CERT_PATH" "$NGINX_SSL_DIR/cloudflare.crt"
cp "$KEY_PATH" "$NGINX_SSL_DIR/cloudflare.key"

# Set permissions
chmod 644 "$NGINX_SSL_DIR/$SAFE_DOMAIN.crt" "$NGINX_SSL_DIR/cloudflare.crt"
chmod 600 "$NGINX_SSL_DIR/$SAFE_DOMAIN.key" "$NGINX_SSL_DIR/cloudflare.key"
chown root:root "$NGINX_SSL_DIR"/*

log_success "Certificate installed to $NGINX_SSL_DIR"
log_info "Files created:"
echo "  $NGINX_SSL_DIR/$SAFE_DOMAIN.crt"
echo "  $NGINX_SSL_DIR/$SAFE_DOMAIN.key"
echo "  $NGINX_SSL_DIR/cloudflare.crt (symlink-style copy)"
echo "  $NGINX_SSL_DIR/cloudflare.key (symlink-style copy)"

# Test nginx configuration if nginx is installed
if command -v nginx &> /dev/null; then
    log_info "Testing nginx configuration..."
    if nginx -t 2>&1; then
        log_success "Nginx configuration valid"
    else
        log_warning "Nginx configuration test failed - check your nginx config"
    fi
fi

#===============================================================================
# STEP 6: CONFIGURE CLOUDFLARE SSL SETTINGS
#===============================================================================

log_step 6 "Configuring Cloudflare SSL settings..."

# Set SSL mode to Full (Strict)
log_info "Setting SSL mode to Full (Strict)..."
RESPONSE=$(cf_api PATCH "/zones/$CF_ZONE_ID/settings/ssl" '{"value":"strict"}')
if [ "$(echo "$RESPONSE" | jq -r '.success')" = "true" ]; then
    log_success "SSL mode set to Full (Strict)"
else
    log_warning "Could not set SSL mode (may require different permissions)"
fi

# Enable Always Use HTTPS
log_info "Enabling Always Use HTTPS..."
RESPONSE=$(cf_api PATCH "/zones/$CF_ZONE_ID/settings/always_use_https" '{"value":"on"}')
if [ "$(echo "$RESPONSE" | jq -r '.success')" = "true" ]; then
    log_success "Always Use HTTPS enabled"
fi

# Enable Automatic HTTPS Rewrites
log_info "Enabling Automatic HTTPS Rewrites..."
RESPONSE=$(cf_api PATCH "/zones/$CF_ZONE_ID/settings/automatic_https_rewrites" '{"value":"on"}')
if [ "$(echo "$RESPONSE" | jq -r '.success')" = "true" ]; then
    log_success "Automatic HTTPS Rewrites enabled"
fi

# Set minimum TLS version
log_info "Setting minimum TLS version to 1.2..."
RESPONSE=$(cf_api PATCH "/zones/$CF_ZONE_ID/settings/min_tls_version" '{"value":"1.2"}')
if [ "$(echo "$RESPONSE" | jq -r '.success')" = "true" ]; then
    log_success "Minimum TLS version set to 1.2"
fi

#===============================================================================
# STEP 7: UPDATE ENVIRONMENT AND RESTART SERVICES
#===============================================================================

log_step 7 "Updating environment and restarting services..."

# Find user home directory (for non-root user running the service)
SERVICE_USER="jeremy"  # Default, can be changed
if [ -d "/home/$SERVICE_USER" ]; then
    ENV_FILE="/home/$SERVICE_USER/.op-web.env"
else
    ENV_FILE="$HOME/.op-web.env"
fi

if [ -f "$ENV_FILE" ]; then
    # Remove old SSL paths
    sed -i '/^SSL_CERT_PATH=/d' "$ENV_FILE"
    sed -i '/^SSL_KEY_PATH=/d' "$ENV_FILE"
    sed -i '/^DOMAIN=/d' "$ENV_FILE"
    
    # Add new paths
    echo "DOMAIN=$DOMAIN" >> "$ENV_FILE"
    echo "SSL_CERT_PATH=$NGINX_SSL_DIR/$SAFE_DOMAIN.crt" >> "$ENV_FILE"
    echo "SSL_KEY_PATH=$NGINX_SSL_DIR/$SAFE_DOMAIN.key" >> "$ENV_FILE"
    
    log_success "Updated $ENV_FILE"
else
    log_info "No environment file found at $ENV_FILE - skipping"
fi

# Reload nginx if running
if systemctl is-active --quiet nginx; then
    log_info "Reloading nginx..."
    systemctl reload nginx
    log_success "Nginx reloaded"
fi

# Restart op-web if running
if systemctl is-active --quiet op-web; then
    log_info "Restarting op-web..."
    systemctl restart op-web
    log_success "op-web restarted"
fi

#===============================================================================
# VERIFICATION
#===============================================================================

echo ""
echo -e "${BLUE}Verifying TLS setup...${NC}"

sleep 2

# Test local HTTPS
echo -n "  Local HTTPS (localhost:443): "
if curl -sfk --connect-timeout 5 https://localhost/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${YELLOW}⚠ Not responding (may be expected)${NC}"
fi

# Test subdomains via Cloudflare
for sub in "${SUBDOMAIN_ARRAY[@]}"; do
    sub=$(echo "$sub" | xargs)
    echo -n "  https://$sub.$DOMAIN: "
    if curl -sf --connect-timeout 10 "https://$sub.$DOMAIN/health" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ OK${NC}"
    elif curl -sf --connect-timeout 10 "https://$sub.$DOMAIN/" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ OK (no /health)${NC}"
    else
        echo -e "${YELLOW}⚠ Not responding${NC}"
    fi
done

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
echo -e "${CYAN}Certificate Locations:${NC}"
echo -e "  Origin:  ${YELLOW}$CERT_PATH${NC}"
echo -e "  Nginx:   ${YELLOW}$NGINX_SSL_DIR/$SAFE_DOMAIN.crt${NC}"
echo ""
echo -e "${CYAN}Certificate Expiry:${NC}"
openssl x509 -in "$CERT_PATH" -noout -enddate 2>/dev/null | sed 's/notAfter=/  /'
echo ""
echo -e "${CYAN}Cloudflare Settings:${NC}"
echo -e "  SSL Mode:      ${YELLOW}Full (Strict)${NC}"
echo -e "  Always HTTPS:  ${YELLOW}Enabled${NC}"
echo -e "  Min TLS:       ${YELLOW}1.2${NC}"
echo ""
echo -e "${CYAN}Access Points:${NC}"
for sub in "${SUBDOMAIN_ARRAY[@]}"; do
    sub=$(echo "$sub" | xargs)
    echo -e "  ${YELLOW}https://$sub.$DOMAIN/${NC}"
done
echo ""
echo -e "${CYAN}Useful Commands:${NC}"
echo -e "  Check certificate: ${YELLOW}openssl x509 -in $CERT_PATH -noout -text${NC}"
echo -e "  Test connection:   ${YELLOW}curl -vI https://${SUBDOMAIN_ARRAY[0]}.$DOMAIN/${NC}"
echo -e "  Nginx logs:        ${YELLOW}tail -f /var/log/nginx/*.log${NC}"
echo ""
