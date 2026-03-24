#!/bin/bash
# TLS certificate setup

setup_tls() {
    local domain="${DOMAIN:-localhost}"
    local safe_domain=$(get_safe_domain)
    local cert_path="/etc/nginx/ssl/${safe_domain}.crt"
    local key_path="/etc/nginx/ssl/${safe_domain}.key"
    
    # Check existing certificate
    if [[ -f "$cert_path" && -f "$key_path" ]]; then
        if openssl x509 -in "$cert_path" -noout -checkend 86400 2>/dev/null; then
            log_success "Valid certificate exists: $cert_path"
            return 0
        else
            log_warning "Certificate expired or invalid, regenerating..."
            rm -f "$cert_path" "$key_path"
        fi
    fi
    
    if is_dry_run; then
        log_info "Would generate certificate for: $domain"
        return 0
    fi
    
    # Try Cloudflare first
    local cf_token=$(get_user_env "CF_DNS_ZONE_TOKEN")
    if [[ -n "$cf_token" && -f "${SCRIPT_DIR:-./deploy}/setup-cloudflare-tls.sh" ]]; then
        log_info "Attempting Cloudflare Origin Certificate..."
        if bash "${SCRIPT_DIR:-./deploy}/setup-cloudflare-tls.sh" 2>/dev/null; then
            if [[ -f "$cert_path" ]]; then
                log_success "Cloudflare certificate generated"
                return 0
            fi
        fi
        log_warning "Cloudflare setup failed, falling back to self-signed"
    fi
    
    # Self-signed fallback
    log_info "Generating self-signed certificate..."
    openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
        -keyout "$key_path" \
        -out "$cert_path" \
        -subj "/CN=$domain/O=op-dbus/C=US" \
        2>/dev/null
    
    chmod 600 "$key_path"
    chmod 644 "$cert_path"
    
    log_success "Self-signed certificate created"
    log_warning "For production, use: certbot --nginx -d $domain"
}
