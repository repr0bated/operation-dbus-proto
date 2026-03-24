#!/bin/bash
#
# BTRFS Subvolume Layout for OP-DBUS Release Server
#
# This script sets up the BTRFS subvolume structure for:
# - Base system (non-modular, always present)
# - Modules (chatbot-managed, snapshotable)
# - Snapshots (for deployment via btrfs send/receive)
# - Staging (chatbot staging area for changes)
#

set -euo pipefail

# ============================================================================
# CONFIGURATION
# ============================================================================

# BTRFS mount point (adjust for your system)
BTRFS_ROOT="/mnt/btrfs-root"

# Subvolume names
SUBVOL_BASE="@op-dbus-base"
SUBVOL_MODULES="@op-dbus-modules"
SUBVOL_SNAPSHOTS="@op-dbus-snapshots"
SUBVOL_STAGING="@op-dbus-staging"

# Mount points
MOUNT_BASE="/opt/op-dbus"
MOUNT_MODULES="/opt/op-dbus/modules"
MOUNT_SNAPSHOTS="/opt/op-dbus/snapshots"
MOUNT_STAGING="/opt/op-dbus/staging"

# Module subvolumes (nested under @op-dbus-modules)
MODULE_SUBVOLS=(
    "agents"        # Dynamic LLM agents
    "commands"      # Dynamic commands
    "mcp-servers"   # External MCP servers
    "workflows"     # Custom workflows
    "templates"     # VM/container templates
)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# ============================================================================
# FUNCTIONS
# ============================================================================

check_btrfs() {
    log_info "Checking BTRFS filesystem..."
    
    # Find BTRFS filesystem
    local btrfs_dev=$(findmnt -n -o SOURCE -T / | head -1)
    local fs_type=$(findmnt -n -o FSTYPE -T /)
    
    if [[ "$fs_type" != "btrfs" ]]; then
        log_error "Root filesystem is not BTRFS (found: $fs_type)"
        log_error "This setup requires BTRFS for subvolume management"
        exit 1
    fi
    
    log_success "BTRFS filesystem detected: $btrfs_dev"
    
    # Mount BTRFS root if not already mounted
    if ! mountpoint -q "$BTRFS_ROOT" 2>/dev/null; then
        log_info "Mounting BTRFS root at $BTRFS_ROOT..."
        mkdir -p "$BTRFS_ROOT"
        mount -o subvolid=5 "$btrfs_dev" "$BTRFS_ROOT"
    fi
    
    log_success "BTRFS root mounted at $BTRFS_ROOT"
}

create_subvolume() {
    local name="$1"
    local path="$BTRFS_ROOT/$name"
    
    if btrfs subvolume show "$path" &>/dev/null; then
        log_info "Subvolume already exists: $name"
    else
        log_info "Creating subvolume: $name"
        btrfs subvolume create "$path"
        log_success "Created subvolume: $name"
    fi
}

create_nested_subvolume() {
    local parent="$1"
    local name="$2"
    local path="$BTRFS_ROOT/$parent/$name"
    
    if btrfs subvolume show "$path" &>/dev/null; then
        log_info "Nested subvolume already exists: $parent/$name"
    else
        log_info "Creating nested subvolume: $parent/$name"
        btrfs subvolume create "$path"
        log_success "Created nested subvolume: $parent/$name"
    fi
}

setup_subvolumes() {
    log_info "Setting up BTRFS subvolume structure..."
    
    # Create main subvolumes
    create_subvolume "$SUBVOL_BASE"
    create_subvolume "$SUBVOL_MODULES"
    create_subvolume "$SUBVOL_SNAPSHOTS"
    create_subvolume "$SUBVOL_STAGING"
    
    # Create module subvolumes (nested)
    for module in "${MODULE_SUBVOLS[@]}"; do
        create_nested_subvolume "$SUBVOL_MODULES" "$module"
    done
    
    log_success "Subvolume structure created"
}

setup_mounts() {
    log_info "Setting up mount points..."
    
    # Create mount directories
    mkdir -p "$MOUNT_BASE"
    mkdir -p "$MOUNT_MODULES"
    mkdir -p "$MOUNT_SNAPSHOTS"
    mkdir -p "$MOUNT_STAGING"
    
    # Get BTRFS device
    local btrfs_dev=$(findmnt -n -o SOURCE -T / | head -1 | sed 's/\[.*\]//')
    
    # Add to fstab if not already present
    local fstab_entries=(
        "$btrfs_dev $MOUNT_BASE btrfs subvol=$SUBVOL_BASE,defaults 0 0"
        "$btrfs_dev $MOUNT_MODULES btrfs subvol=$SUBVOL_MODULES,defaults 0 0"
        "$btrfs_dev $MOUNT_SNAPSHOTS btrfs subvol=$SUBVOL_SNAPSHOTS,defaults 0 0"
        "$btrfs_dev $MOUNT_STAGING btrfs subvol=$SUBVOL_STAGING,defaults 0 0"
    )
    
    for entry in "${fstab_entries[@]}"; do
        local mount_point=$(echo "$entry" | awk '{print $2}')
        if ! grep -q "$mount_point" /etc/fstab; then
            echo "$entry" >> /etc/fstab
            log_info "Added fstab entry for $mount_point"
        fi
    done
    
    # Mount all
    mount -a
    
    log_success "Mount points configured"
}

setup_directory_structure() {
    log_info "Setting up directory structure within subvolumes..."
    
    # Base subvolume directories
    mkdir -p "$MOUNT_BASE"/{bin,lib,share,etc,var}
    mkdir -p "$MOUNT_BASE/etc"/{agents,plugins,mcp}
    mkdir -p "$MOUNT_BASE/var"/{cache,sessions,log}
    
    # Module directories (each is a subvolume)
    # They're already created as subvolumes, just ensure they're mounted
    for module in "${MODULE_SUBVOLS[@]}"; do
        local module_path="$MOUNT_MODULES/$module"
        if [[ ! -d "$module_path" ]]; then
            # Mount the nested subvolume
            mkdir -p "$module_path"
        fi
    done
    
    # Staging directories
    mkdir -p "$MOUNT_STAGING"/{pending,approved,rejected}
    
    # Snapshot directories
    mkdir -p "$MOUNT_SNAPSHOTS"/{base,full,deploy}
    
    log_success "Directory structure created"
}

create_initial_snapshot() {
    log_info "Creating initial base snapshot..."
    
    local timestamp=$(date +%Y%m%d-%H%M%S)
    local snapshot_name="base-$timestamp"
    local snapshot_path="$BTRFS_ROOT/$SUBVOL_SNAPSHOTS/$snapshot_name"
    
    # Create read-only snapshot of base
    btrfs subvolume snapshot -r \
        "$BTRFS_ROOT/$SUBVOL_BASE" \
        "$snapshot_path"
    
    log_success "Created initial snapshot: $snapshot_name"
    
    # Record snapshot metadata
    cat > "$MOUNT_SNAPSHOTS/base/$snapshot_name.json" << EOF
{
    "name": "$snapshot_name",
    "type": "base",
    "created_at": "$(date -Iseconds)",
    "description": "Initial base installation",
    "components": {
        "chat_server": true,
        "mcp_server": true,
        "dbus_integration": true,
        "introspection": true,
        "agents": "embedded",
        "plugins": "embedded"
    }
}
EOF
}

print_summary() {
    echo
    echo "============================================================================"
    echo -e "${GREEN}BTRFS SUBVOLUME LAYOUT COMPLETE${NC}"
    echo "============================================================================"
    echo
    echo "Subvolumes created:"
    btrfs subvolume list "$BTRFS_ROOT" | grep op-dbus | while read line; do
        echo "  $line"
    done
    echo
    echo "Mount points:"
    echo "  $MOUNT_BASE        - Base system (non-modular)"
    echo "  $MOUNT_MODULES     - Chatbot-managed modules"
    echo "  $MOUNT_SNAPSHOTS   - Deployment snapshots"
    echo "  $MOUNT_STAGING     - Staging area for changes"
    echo
    echo "Module subvolumes:"
    for module in "${MODULE_SUBVOLS[@]}"; do
        echo "  $MOUNT_MODULES/$module"
    done
    echo
    echo "Usage:"
    echo "  # Create snapshot of current state"
    echo "  btrfs subvolume snapshot -r $BTRFS_ROOT/$SUBVOL_BASE $BTRFS_ROOT/$SUBVOL_SNAPSHOTS/snap-name"
    echo
    echo "  # Send snapshot to target"
    echo "  btrfs send $BTRFS_ROOT/$SUBVOL_SNAPSHOTS/snap-name | ssh target 'btrfs receive /mnt/target'"
    echo
    echo "  # Incremental send (faster)"
    echo "  btrfs send -p $BTRFS_ROOT/$SUBVOL_SNAPSHOTS/parent-snap $BTRFS_ROOT/$SUBVOL_SNAPSHOTS/new-snap | ssh target 'btrfs receive /mnt/target'"
    echo
}

# ============================================================================
# MAIN
# ============================================================================

main() {
    echo
    echo "============================================================================"
    echo "OP-DBUS BTRFS SUBVOLUME SETUP"
    echo "============================================================================"
    echo
    
    # Check we're root
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        exit 1
    fi
    
    check_btrfs
    setup_subvolumes
    setup_mounts
    setup_directory_structure
    create_initial_snapshot
    print_summary
}

main "$@"
