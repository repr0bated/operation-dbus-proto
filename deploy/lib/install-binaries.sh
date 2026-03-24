#!/bin/bash
# Binary installation functions

# Known binary names
BINARY_NAMES=(
    "op-web-server"
    "op-mcp-server"
    "op-dbus-service"
    "dbus-agent"
)

install_binaries() {
    local project_dir="$1"
    local install_dir="$2"
    local target_dir="$project_dir/target/release"
    local count=0
    
    for binary in "${BINARY_NAMES[@]}"; do
        local src="$target_dir/$binary"
        local dst="$install_dir/$binary"
        
        if [[ -f "$src" && -x "$src" ]]; then
            if is_dry_run; then
                log_info "Would install: $dst"
            else
                # Stop service before replacing
                local service_name=$(echo "$binary" | sed 's/-server$//')
                stop_service_if_running "$service_name"
                
                cp "$src" "$dst"
                chmod 755 "$dst"
                chown root:root "$dst"
                log_success "Installed: $dst"
            fi
            count=$((count + 1))
        fi
    done
    
    if [[ $count -eq 0 ]]; then
        log_warning "No binaries found to install"
        log_info "Available in $target_dir:"
        ls -la "$target_dir/" 2>/dev/null | grep -E '^-rwx' | head -10 || echo "  (none)"
    else
        log_success "Installed $count binaries"
    fi
}
