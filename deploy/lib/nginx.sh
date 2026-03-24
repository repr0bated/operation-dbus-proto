#!/bin/bash
# Nginx configuration

setup_nginx() {
    # Install if needed
    if ! ensure_package nginx; then
        log_warning "Skipping nginx setup (nginx not installed and install failed)"
        return 0
    fi
    
    local domain="${DOMAIN:-localhost}"
    local safe_domain=$(get_safe_domain)
    local log_dir="${LOG_DIR:-/var/log/op-dbus}"
    local project_dir="${PROJECT_DIR:-/home/jeremy/op-dbus-v2}"
    local config_file="/etc/nginx/sites-available/op-web"
    
    if is_dry_run; then
        log_info "Would create nginx config: $config_file"
        return 0
    fi
    
    cat > "$config_file" << EOF
# op-dbus-v2 nginx configuration
# Domain: $domain
# Generated: $(date -Iseconds)

upstream op_web_backend {
    server 127.0.0.1:8081;
    keepalive 32;
}

# HTTP -> HTTPS redirect
server {
    listen 80;
    listen [::]:80;
    server_name $domain *.$domain;
    return 301 https://\$host\$request_uri;
}

# Main HTTPS server
server {
    listen 443 ssl;
    listen [::]:443 ssl;
    http2 on;
    server_name $domain *.$domain;

    # SSL
    ssl_certificate /etc/nginx/ssl/${safe_domain}.crt;
    ssl_certificate_key /etc/nginx/ssl/${safe_domain}.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 1d;

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;

    # Logging
    access_log $log_dir/nginx-access.log;
    error_log $log_dir/nginx-error.log;

    # Root
    location / {
        proxy_pass http://op_web_backend;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    # WebSocket
    location /ws {
        proxy_pass http://op_web_backend/ws;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
        proxy_read_timeout 86400;
    }

    # SSE events
    location /events {
        proxy_pass http://op_web_backend/api/events;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 3600;
    }

    # Health
    location /health {
        proxy_pass http://op_web_backend/api/health;
    }
}
EOF
    
    # Enable site
    ln -sf "$config_file" /etc/nginx/sites-enabled/op-web
    rm -f /etc/nginx/sites-enabled/default
    
    log_success "Nginx configured: $config_file"
}
