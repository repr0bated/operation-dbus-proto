#!/bin/bash
# Build functions

build_release() {
    local project_dir="$1"
    local user="$2"
    
    cd "$project_dir"
    
    if is_dry_run; then
        log_info "Would build release binaries in $project_dir"
        return 0
    fi
    
    log_info "Building release binaries (this may take a few minutes)..."
    
    if sudo -u "$user" "$CARGO_BIN" build --release 2>&1 | tail -20; then
        log_success "Build complete"
        return 0
    else
        log_error "Build failed"
        return 1
    fi
}
