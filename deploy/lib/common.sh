#!/bin/bash
# Common functions for deployment scripts

# Colors
export RED='\033[0;31m'
export GREEN='\033[0;32m'
export YELLOW='\033[1;33m'
export BLUE='\033[0;34m'
export MAGENTA='\033[0;35m'
export CYAN='\033[0;36m'
export NC='\033[0m'

# Logging functions
log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error()   { echo -e "${RED}[✗]${NC} $1"; }
log_step()    { echo -e "\n${MAGENTA}▶${NC} ${CYAN}$1${NC}"; }

# Find cargo binary
find_cargo() {
    if [[ -x "/home/${SERVICE_USER:-jeremy}/.cargo/bin/cargo" ]]; then
        export CARGO_BIN="/home/${SERVICE_USER:-jeremy}/.cargo/bin/cargo"
        return 0
    elif command -v cargo &>/dev/null; then
        export CARGO_BIN="cargo"
        return 0
    fi
    return 1
}

# Get environment variable from user's shell
get_user_env() {
    local var_name="$1"
    local user="${SERVICE_USER:-jeremy}"
    sudo -u "$user" bash -c "source ~/.bashrc 2>/dev/null; echo \${$var_name}" 2>/dev/null || echo ""
}

# Check if running in dry-run mode
is_dry_run() {
    [[ "${DRY_RUN:-false}" == "true" ]]
}

# Run command (respects dry-run)
run_cmd() {
    if is_dry_run; then
        log_info "Would run: $*"
    else
        "$@"
    fi
}

# Install package if not present
ensure_package() {
    local pkg="$1"
    if ! command -v "$pkg" &>/dev/null; then
        if [[ -x "/usr/sbin/$pkg" || -x "/usr/local/sbin/$pkg" ]]; then
            return 0
        fi
        if is_dry_run; then
            log_info "Would install: $pkg"
        else
            log_info "Installing $pkg..."
            if ! apt-get update -qq; then
                log_warning "apt-get update failed; cannot install $pkg"
                return 1
            fi
            if ! apt-get install -y -qq "$pkg"; then
                log_warning "apt-get install failed for $pkg"
                return 1
            fi
        fi
    fi
}

# Stop service if running
stop_service_if_running() {
    local service="$1"
    if systemctl is-active --quiet "$service" 2>/dev/null; then
        if is_dry_run; then
            log_info "Would stop: $service"
        else
            systemctl stop "$service" || true
        fi
    fi
}

# Validate domain format
validate_domain() {
    local domain="$1"
    if [[ ! "$domain" =~ ^[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?)*\.[a-zA-Z]{2,}$ ]]; then
        return 1
    fi
    return 0
}

# Get safe domain name (dots replaced with dashes)
get_safe_domain() {
    echo "${DOMAIN:-localhost}" | tr '.' '-'
}
