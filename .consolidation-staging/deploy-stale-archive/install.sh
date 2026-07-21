#!/bin/bash
# op-dbus-v2 Installation Script
# Main entry point for deployment
#
# Usage:
#   sudo ./deploy/install.sh              # Full installation
#   sudo ./deploy/install.sh --dry-run    # Preview changes
#   sudo ./deploy/install.sh --skip-tls   # Skip TLS setup
#   sudo ./deploy/install.sh --skip-nginx # Skip nginx setup

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Source common functions
source "$SCRIPT_DIR/lib/common.sh"

# Default configuration
export DOMAIN="${DOMAIN:-}"
export SERVICE_USER="${SERVICE_USER:-jeremy}"
export INSTALL_DIR="${INSTALL_DIR:-/usr/local/sbin}"
export CONFIG_DIR="${CONFIG_DIR:-/etc/op-dbus}"
export LOG_DIR="${LOG_DIR:-/var/log/op-dbus}"
export DATA_DIR="${DATA_DIR:-/var/lib/op-dbus}"
export DRY_RUN="${DRY_RUN:-false}"
export SKIP_TLS="${SKIP_TLS:-false}"
export SKIP_NGINX="${SKIP_NGINX:-false}"
export SKIP_SYSTEMD="${SKIP_SYSTEMD:-false}"
export SKIP_BUILD="${SKIP_BUILD:-false}"
export AUTO_APPROVE="${AUTO_APPROVE:-false}"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)     DRY_RUN=true; shift ;;
        --skip-tls)    SKIP_TLS=true; shift ;;
        --skip-nginx)  SKIP_NGINX=true; shift ;;
        --skip-systemd) SKIP_SYSTEMD=true; shift ;;
        --skip-build)  SKIP_BUILD=true; shift ;;
        --domain)      DOMAIN="$2"; shift 2 ;;
        --user)        SERVICE_USER="$2"; shift 2 ;;
        --yes)         AUTO_APPROVE=true; shift ;;
        --help|-h)     show_help; exit 0 ;;
        *)             log_error "Unknown option: $1"; exit 1 ;;
    esac
done

show_help() {
    cat << EOF
op-dbus-v2 Installation Script

Usage: sudo ./deploy/install.sh [OPTIONS]

Options:
  --dry-run       Preview changes without applying
  --skip-tls      Skip TLS certificate setup
  --skip-nginx    Skip nginx configuration
  --skip-systemd  Skip systemd service setup
  --skip-build    Skip building (use existing binaries)
  --domain DOMAIN Set domain name
  --user USER     Set service user (default: jeremy)
  --yes           Non-interactive (skip confirmation)
  --help          Show this help

Environment Variables:
  DOMAIN          Domain name for TLS
  SERVICE_USER    User to run services as
  INSTALL_DIR     Binary installation directory
  CONFIG_DIR      Configuration directory
  LOG_DIR         Log directory
  DATA_DIR        Data directory

Examples:
  sudo ./deploy/install.sh
  sudo ./deploy/install.sh --domain example.com
  sudo ./deploy/install.sh --dry-run --skip-tls
EOF
}

# Banner
print_banner() {
    echo -e "${GREEN}"
    cat << 'EOF'
  ___  ____        _ _                    ____  
 / _ \|  _ \      | | |__  _   _ ___    _|___ \ 
| | | | |_) |_____| | '_ \| | | / __|  (_) __) |
| |_| |  __/______| | |_) | |_| \__ \   _/ __/ 
 \___/|_|        |_|_.__/ \__,_|___/  (_)_____|
                                                
EOF
    echo -e "${NC}"
    echo -e "${CYAN}Installation Script v1.0${NC}"
    echo ""
}

# Pre-flight checks
preflight_checks() {
    log_step "Pre-flight checks"
    
    # Root check
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root (use sudo)"
        exit 1
    fi
    
    # Project directory check
    if [[ ! -f "$PROJECT_DIR/Cargo.toml" ]]; then
        log_error "Not a valid project directory: $PROJECT_DIR"
        exit 1
    fi
    
    # Service user check
    if ! id "$SERVICE_USER" &>/dev/null; then
        log_error "Service user does not exist: $SERVICE_USER"
        exit 1
    fi
    
    # Cargo check
    if [[ "$SKIP_BUILD" != "true" ]]; then
        if ! find_cargo; then
            log_error "Cargo not found. Install Rust first."
            exit 1
        fi
    fi
    
    log_success "Pre-flight checks passed"
}

# Interactive configuration
configure() {
    log_step "Configuration"
    
    # Domain
    if [[ -z "$DOMAIN" ]]; then
        local default="localhost"
        if [[ "$AUTO_APPROVE" == "true" ]]; then
            DOMAIN="$default"
        else
            read -p "Enter domain name [$default]: " DOMAIN
            DOMAIN="${DOMAIN:-$default}"
        fi
    fi
    export DOMAIN
    
    # Summary
    echo ""
    echo -e "${CYAN}Installation Summary${NC}"
    echo "────────────────────────────────────────"
    echo -e "  Project:      ${YELLOW}$PROJECT_DIR${NC}"
    echo -e "  Install dir:  ${YELLOW}$INSTALL_DIR${NC}"
    echo -e "  Config dir:   ${YELLOW}$CONFIG_DIR${NC}"
    echo -e "  Log dir:      ${YELLOW}$LOG_DIR${NC}"
    echo -e "  Service user: ${YELLOW}$SERVICE_USER${NC}"
    echo -e "  Domain:       ${YELLOW}$DOMAIN${NC}"
    echo -e "  Dry run:      ${YELLOW}$DRY_RUN${NC}"
    echo -e "  Skip TLS:     ${YELLOW}$SKIP_TLS${NC}"
    echo -e "  Skip nginx:   ${YELLOW}$SKIP_NGINX${NC}"
    echo -e "  Skip systemd: ${YELLOW}$SKIP_SYSTEMD${NC}"
    echo ""
    
    if [[ "$DRY_RUN" == "true" ]]; then
        log_warning "DRY RUN MODE - No changes will be made"
        echo ""
    fi
    
    if [[ "$AUTO_APPROVE" != "true" ]]; then
        read -p "Proceed? [Y/n]: " confirm
        if [[ "$confirm" =~ ^[Nn]$ ]]; then
            log_info "Installation cancelled"
            exit 0
        fi
    fi
}

# Create directories
create_directories() {
    log_step "Creating directories"
    
    local dirs=(
        "$INSTALL_DIR"
        "$CONFIG_DIR"
        "$LOG_DIR"
        "$DATA_DIR"
        "/etc/nginx/ssl"
    )
    
    for dir in "${dirs[@]}"; do
        if [[ "$DRY_RUN" == "true" ]]; then
            log_info "Would create: $dir"
        else
            mkdir -p "$dir"
            log_success "Created: $dir"
        fi
    done
    
    # Set ownership
    if [[ "$DRY_RUN" != "true" ]]; then
        chown -R "$SERVICE_USER:$SERVICE_USER" "$LOG_DIR" "$DATA_DIR"
    fi
}

# Build
do_build() {
    if [[ "$SKIP_BUILD" == "true" ]]; then
        log_info "Skipping build (--skip-build)"
        return 0
    fi
    
    log_step "Building binaries"
    source "$SCRIPT_DIR/lib/build.sh"
    build_release "$PROJECT_DIR" "$SERVICE_USER"
}

# Install binaries
do_install_binaries() {
    log_step "Installing binaries"
    source "$SCRIPT_DIR/lib/install-binaries.sh"
    install_binaries "$PROJECT_DIR" "$INSTALL_DIR"
}

# Create environment file
create_env_file() {
    log_step "Creating environment file"
    
    local env_file="$CONFIG_DIR/op-web.env"
    local safe_domain=$(echo "$DOMAIN" | tr '.' '-')
    
    # Try to get existing tokens from user's environment
    local hf_token=$(get_user_env "HF_TOKEN")
    local gh_token=$(get_user_env "GH_TOKEN")
    local cf_token=$(get_user_env "CF_DNS_ZONE_TOKEN")
    local cf_account=$(get_user_env "CF_ACCOUNT_ID")
    local cf_zone=$(get_user_env "CF_ZONE_ID")
    local pinecone=$(get_user_env "PINECONE_API_KEY")
    
    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "Would create: $env_file"
        return 0
    fi
    
    cat > "$env_file" << EOF
# op-dbus-v2 Environment Configuration
# Generated: $(date -Iseconds)
# Domain: $DOMAIN

# Domain
DOMAIN=$DOMAIN
PORT=8081
OP_DBUS_BIND=127.0.0.1:8082
OP_WEB_STATIC_DIR=$PROJECT_DIR/crates/op-web/static

# API Keys
HF_TOKEN=${hf_token:-}
GITHUB_PERSONAL_ACCESS_TOKEN=${gh_token:-}
HUGGINGFACE_API_KEY=${hf_token:-}
PINECONE_API_KEY=${pinecone:-}

# Cloudflare
CLOUDFLARE_API_TOKEN=${cf_token:-}
CLOUDFLARE_ACCOUNT_ID=${cf_account:-}
CF_ZONE_ID=${cf_zone:-}

# Paths
SSL_CERT_PATH=/etc/nginx/ssl/${safe_domain}.crt
SSL_KEY_PATH=/etc/nginx/ssl/${safe_domain}.key

# Logging
RUST_LOG=info,op_web=debug,op_mcp=debug

# Data
OP_DATA_DIR=$DATA_DIR
OP_LOG_DIR=$LOG_DIR
EOF
    
    chmod 600 "$env_file"
    log_success "Created: $env_file"
    
    # User copy
    cp "$env_file" "/home/$SERVICE_USER/.op-web.env"
    chown "$SERVICE_USER:$SERVICE_USER" "/home/$SERVICE_USER/.op-web.env"
    chmod 600 "/home/$SERVICE_USER/.op-web.env"
}

# Setup systemd
do_systemd() {
    if [[ "$SKIP_SYSTEMD" == "true" ]]; then
        log_info "Skipping systemd (--skip-systemd)"
        return 0
    fi
    
    log_step "Setting up systemd services"
    source "$SCRIPT_DIR/lib/systemd.sh"
    setup_systemd_services
}

# Setup nginx
do_nginx() {
    if [[ "$SKIP_NGINX" == "true" ]]; then
        log_info "Skipping nginx (--skip-nginx)"
        return 0
    fi
    
    log_step "Setting up nginx"
    source "$SCRIPT_DIR/lib/nginx.sh"
    setup_nginx
}

# Setup TLS
do_tls() {
    if [[ "$SKIP_TLS" == "true" ]]; then
        log_info "Skipping TLS (--skip-tls)"
        return 0
    fi
    
    log_step "Setting up TLS"
    source "$SCRIPT_DIR/lib/tls.sh"
    setup_tls
}

# Start services
start_services() {
    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "Would start services"
        return 0
    fi
    
    log_step "Starting services"
    
    for service in op-web op-dbus-service; do
        if [[ -f "/etc/systemd/system/${service}.service" ]]; then
            systemctl start "$service" || true
            sleep 2
            if systemctl is-active --quiet "$service"; then
                log_success "$service started"
            else
                log_warning "$service failed to start"
            fi
        fi
    done
    
    # Nginx
    if command -v nginx &>/dev/null && [[ "$SKIP_NGINX" != "true" ]]; then
        if nginx -t &>/dev/null; then
            systemctl reload nginx || systemctl start nginx
            log_success "nginx started"
        else
            log_warning "nginx config invalid"
        fi
    fi
}

# Verify installation
verify() {
    log_step "Verification"
    
    echo ""
    echo -e "${CYAN}Service Status:${NC}"
    for service in op-web op-dbus-service nginx; do
        printf "  %-12s: " "$service"
        if systemctl is-active --quiet "$service" 2>/dev/null; then
            echo -e "${GREEN}running${NC}"
        elif systemctl is-enabled --quiet "$service" 2>/dev/null; then
            echo -e "${YELLOW}enabled (not running)${NC}"
        else
            echo -e "${RED}not configured${NC}"
        fi
    done
    
    echo ""
    echo -e "${CYAN}Connectivity:${NC}"
    printf "  %-20s: " "Web (8081)"
    if curl -sf --connect-timeout 3 http://127.0.0.1:8081/api/health &>/dev/null; then
        echo -e "${GREEN}OK${NC}"
    else
        echo -e "${YELLOW}not responding${NC}"
    fi

    printf "  %-20s: " "D-Bus HTTP (8082)"
    if curl -sf --connect-timeout 3 http://127.0.0.1:8082/health &>/dev/null; then
        echo -e "${GREEN}OK${NC}"
    else
        echo -e "${YELLOW}not responding${NC}"
    fi
    
    printf "  %-20s: " "HTTPS (443)"
    if curl -sfk --connect-timeout 3 https://localhost/health &>/dev/null; then
        echo -e "${GREEN}OK${NC}"
    else
        echo -e "${YELLOW}not responding${NC}"
    fi
}

# Print summary
print_summary() {
    echo ""
    echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}                  Installation Complete!${NC}"
    echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
    echo ""
    
    if [[ "$DRY_RUN" == "true" ]]; then
        echo -e "${YELLOW}This was a dry run - no changes were made${NC}"
        echo ""
        return
    fi
    
    echo -e "${CYAN}Binaries:${NC}"
    ls "$INSTALL_DIR"/op-* 2>/dev/null | while read f; do
        echo -e "  ${YELLOW}$f${NC}"
    done || echo "  (none installed)"
    
    echo ""
    echo -e "${CYAN}Access:${NC}"
    echo -e "  Local:  ${YELLOW}http://localhost:8081/${NC}"
    echo -e "  HTTPS:  ${YELLOW}https://$DOMAIN/${NC}"
    
    echo ""
    echo -e "${CYAN}Commands:${NC}"
    echo -e "  Status:   ${YELLOW}systemctl status op-web op-dbus-service${NC}"
    echo -e "  Logs:     ${YELLOW}journalctl -u op-web -f${NC}"
    echo -e "  Restart:  ${YELLOW}systemctl restart op-web${NC}"
    echo -e "  Upgrade:  ${YELLOW}$SCRIPT_DIR/upgrade.sh${NC}"
    echo ""
}

# Main
main() {
    print_banner
    preflight_checks
    configure
    create_directories
    do_build
    do_install_binaries
    create_env_file
    do_systemd
    do_nginx
    do_tls
    start_services
    verify
    print_summary
}

main
