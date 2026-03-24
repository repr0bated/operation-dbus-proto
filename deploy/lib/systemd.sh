#!/bin/bash
# Systemd service setup

setup_systemd_services() {
    local config_dir="${CONFIG_DIR:-/etc/op-dbus}"
    local install_dir="${INSTALL_DIR:-/usr/local/sbin}"
    local log_dir="${LOG_DIR:-/var/log/op-dbus}"
    local data_dir="${DATA_DIR:-/var/lib/op-dbus}"
    local user="${SERVICE_USER:-jeremy}"
    local project_dir="${PROJECT_DIR:-/home/jeremy/op-dbus-v2}"
    
    # op-web service
    local op_web_bin=""
    [[ -f "$install_dir/op-web-server" ]] && op_web_bin="$install_dir/op-web-server"
    
    if [[ -n "$op_web_bin" ]]; then
        create_service_file "op-web" "$op_web_bin" "op-dbus Web Server"
    else
        log_warning "op-web binary not found, skipping service"
    fi
    
    # op-dbus-service
    local op_dbus_bin=""
    [[ -f "$install_dir/op-dbus-service" ]] && op_dbus_bin="$install_dir/op-dbus-service"
    
    if [[ -n "$op_dbus_bin" ]]; then
        create_service_file "op-dbus-service" "$op_dbus_bin" "op-dbus Unified D-Bus Service"
    else
        log_warning "op-dbus-service binary not found, skipping service"
    fi
    
    # Reload and enable
    if ! is_dry_run; then
        systemctl daemon-reload
        [[ -f /etc/systemd/system/op-web.service ]] && systemctl enable op-web.service
        [[ -f /etc/systemd/system/op-dbus-service.service ]] && systemctl enable op-dbus-service.service
        log_success "Systemd services configured"
    fi
}

create_service_file() {
    local name="$1"
    local exec_path="$2"
    local description="$3"
    local service_file="/etc/systemd/system/${name}.service"
    
    if is_dry_run; then
        log_info "Would create: $service_file"
        return 0
    fi
    
    local exec_start="$exec_path"
    if [[ "$name" == "op-dbus-service" ]]; then
        exec_start="$exec_path --bind \${OP_DBUS_BIND}"
    fi

    cat > "$service_file" << EOF
[Unit]
Description=$description
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=${SERVICE_USER:-jeremy}
Group=${SERVICE_USER:-jeremy}
WorkingDirectory=${PROJECT_DIR:-/home/jeremy/op-dbus-v2}
EnvironmentFile=${CONFIG_DIR:-/etc/op-dbus}/op-web.env
ExecStart=$exec_start
Restart=on-failure
RestartSec=5
StandardOutput=append:${LOG_DIR:-/var/log/op-dbus}/${name}.log
StandardError=append:${LOG_DIR:-/var/log/op-dbus}/${name}-error.log

# Security
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ReadWritePaths=${LOG_DIR:-/var/log/op-dbus} ${DATA_DIR:-/var/lib/op-dbus}

[Install]
WantedBy=multi-user.target
EOF
    
    log_success "Created: $service_file"
}
