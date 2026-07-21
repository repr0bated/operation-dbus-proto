#!/bin/bash
#
# OP-DBUS RELEASE SERVER INSTALLATION
#
# This script installs the base system and sets up the release server
# for BTRFS-based deployments.
#
# The release server:
# 1. Runs as a live system with full op-dbus functionality
# 2. Has chatbot-managed modules in snapshotable subvolumes
# 3. Creates deployment snapshots for target systems
# 4. Deploys via btrfs send/receive
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${CYAN}[STEP]${NC} $1"; }

# ============================================================================
# PHASE 1: BTRFS SETUP
# ============================================================================

phase_btrfs_setup() {
    log_step "Phase 1: Setting up BTRFS subvolume layout..."
    
    if [[ -x "$SCRIPT_DIR/btrfs-layout.sh" ]]; then
        "$SCRIPT_DIR/btrfs-layout.sh"
    else
        log_error "btrfs-layout.sh not found or not executable"
        exit 1
    fi
    
    log_success "BTRFS layout complete"
}

# ============================================================================
# PHASE 2: BASE INSTALLATION
# ============================================================================

phase_base_install() {
    log_step "Phase 2: Installing base system..."
    
    # Run the base installation script
    if [[ -x "$SCRIPT_DIR/base-install.sh" ]]; then
        "$SCRIPT_DIR/base-install.sh"
    else
        log_error "base-install.sh not found or not executable"
        exit 1
    fi
    
    log_success "Base installation complete"
}

# ============================================================================
# PHASE 3: RELEASE SERVER CONFIGURATION
# ============================================================================

phase_release_config() {
    log_step "Phase 3: Configuring release server..."
    
    # Create release server configuration
    cat > /etc/op-dbus/release-server.conf << 'EOF'
# OP-DBUS Release Server Configuration

# This server is a release/deployment server
RELEASE_SERVER=true

# BTRFS subvolume paths
BTRFS_ROOT=/mnt/btrfs-root
SUBVOL_BASE=@op-dbus-base
SUBVOL_MODULES=@op-dbus-modules
SUBVOL_SNAPSHOTS=@op-dbus-snapshots
SUBVOL_STAGING=@op-dbus-staging

# Snapshot retention
SNAPSHOT_RETENTION_BASE=10
SNAPSHOT_RETENTION_FULL=20
SNAPSHOT_RETENTION_DEPLOY=50

# Deployment targets (managed by chatbot)
DEPLOY_TARGETS_FILE=/etc/op-dbus/deploy-targets.json

# Auto-snapshot on module changes
AUTO_SNAPSHOT=true
AUTO_SNAPSHOT_PREFIX=auto
EOF

    # Create deployment targets file
    cat > /etc/op-dbus/deploy-targets.json << 'EOF'
{
  "targets": [],
  "groups": {
    "production": [],
    "staging": [],
    "development": []
  },
  "default_group": "staging"
}
EOF

    # Create snapshot management script
    cat > /opt/op-dbus/bin/op-snapshot << 'SCRIPT'
#!/bin/bash
#
# OP-DBUS Snapshot Management
#
# Usage:
#   op-snapshot create [type] [name] [description]
#   op-snapshot list [type]
#   op-snapshot send <snapshot> <target>
#   op-snapshot delete <snapshot>
#   op-snapshot rollback <snapshot>
#

set -euo pipefail

source /etc/op-dbus/release-server.conf

BTRFS_MOUNT="/mnt/btrfs-root"
SNAPSHOT_BASE="$BTRFS_MOUNT/$SUBVOL_SNAPSHOTS"

cmd_create() {
    local type="${1:-full}"
    local name="${2:-$(date +%Y%m%d-%H%M%S)}"
    local description="${3:-Manual snapshot}"
    
    local snapshot_name="${type}-${name}"
    local snapshot_path="$SNAPSHOT_BASE/$snapshot_name"
    
    case "$type" in
        base)
            btrfs subvolume snapshot -r "$BTRFS_MOUNT/$SUBVOL_BASE" "$snapshot_path"
            ;;
        modules)
            btrfs subvolume snapshot -r "$BTRFS_MOUNT/$SUBVOL_MODULES" "$snapshot_path"
            ;;
        full)
            # Create a combined snapshot (base + modules)
            local temp_dir=$(mktemp -d)
            cp -a /opt/op-dbus/* "$temp_dir/" 2>/dev/null || true
            cp -a /opt/op-dbus/modules/* "$temp_dir/modules/" 2>/dev/null || true
            
            # Create snapshot from temp
            btrfs subvolume create "$snapshot_path"
            cp -a "$temp_dir"/* "$snapshot_path/"
            btrfs property set "$snapshot_path" ro true
            
            rm -rf "$temp_dir"
            ;;
        *)
            echo "Unknown snapshot type: $type"
            exit 1
            ;;
    esac
    
    # Record metadata
    cat > "$SNAPSHOT_BASE/${snapshot_name}.json" << EOF
{
    "name": "$snapshot_name",
    "type": "$type",
    "created_at": "$(date -Iseconds)",
    "description": "$description",
    "created_by": "$(whoami)",
    "hostname": "$(hostname)"
}
EOF
    
    echo "Created snapshot: $snapshot_name"
}

cmd_list() {
    local type="${1:-}"
    
    echo "Available snapshots:"
    echo
    
    if [[ -n "$type" ]]; then
        btrfs subvolume list "$BTRFS_MOUNT" | grep "$SUBVOL_SNAPSHOTS/$type" | while read line; do
            local name=$(echo "$line" | awk '{print $NF}' | sed "s|$SUBVOL_SNAPSHOTS/||")
            local meta_file="$SNAPSHOT_BASE/${name}.json"
            if [[ -f "$meta_file" ]]; then
                local created=$(jq -r '.created_at' "$meta_file")
                local desc=$(jq -r '.description' "$meta_file")
                printf "  %-30s %s\n    %s\n" "$name" "$created" "$desc"
            else
                echo "  $name"
            fi
        done
    else
        btrfs subvolume list "$BTRFS_MOUNT" | grep "$SUBVOL_SNAPSHOTS" | while read line; do
            local name=$(echo "$line" | awk '{print $NF}' | sed "s|$SUBVOL_SNAPSHOTS/||")
            echo "  $name"
        done
    fi
}

cmd_send() {
    local snapshot="$1"
    local target="$2"
    local parent="${3:-}"
    
    local snapshot_path="$SNAPSHOT_BASE/$snapshot"
    
    if [[ ! -d "$snapshot_path" ]]; then
        echo "Snapshot not found: $snapshot"
        exit 1
    fi
    
    echo "Sending snapshot $snapshot to $target..."
    
    if [[ -n "$parent" ]]; then
        local parent_path="$SNAPSHOT_BASE/$parent"
        echo "Using incremental send with parent: $parent"
        btrfs send -p "$parent_path" "$snapshot_path" | ssh "$target" "btrfs receive /opt/op-dbus-deploy"
    else
        btrfs send "$snapshot_path" | ssh "$target" "btrfs receive /opt/op-dbus-deploy"
    fi
    
    echo "Snapshot sent successfully"
}

cmd_delete() {
    local snapshot="$1"
    local snapshot_path="$SNAPSHOT_BASE/$snapshot"
    
    if [[ ! -d "$snapshot_path" ]]; then
        echo "Snapshot not found: $snapshot"
        exit 1
    fi
    
    echo "Deleting snapshot: $snapshot"
    btrfs subvolume delete "$snapshot_path"
    rm -f "$SNAPSHOT_BASE/${snapshot}.json"
    
    echo "Snapshot deleted"
}

cmd_rollback() {
    local snapshot="$1"
    local snapshot_path="$SNAPSHOT_BASE/$snapshot"
    
    if [[ ! -d "$snapshot_path" ]]; then
        echo "Snapshot not found: $snapshot"
        exit 1
    fi
    
    echo "Rolling back to snapshot: $snapshot"
    echo "This will replace the current system state."
    read -p "Are you sure? (yes/no): " confirm
    
    if [[ "$confirm" != "yes" ]]; then
        echo "Rollback cancelled"
        exit 0
    fi
    
    # Stop services
    systemctl stop op-chat-server nginx 2>/dev/null || true
    
    # Create backup of current state
    local backup_name="pre-rollback-$(date +%Y%m%d-%H%M%S)"
    cmd_create "full" "$backup_name" "Backup before rollback to $snapshot"
    
    # Restore from snapshot
    # This is a simplified version - real implementation would be more careful
    cp -a "$snapshot_path"/* /opt/op-dbus/
    
    # Restart services
    systemctl start op-chat-server nginx
    
    echo "Rollback complete. Backup created: $backup_name"
}

case "${1:-help}" in
    create)  cmd_create "${2:-}" "${3:-}" "${4:-}" ;;
    list)    cmd_list "${2:-}" ;;
    send)    cmd_send "$2" "$3" "${4:-}" ;;
    delete)  cmd_delete "$2" ;;
    rollback) cmd_rollback "$2" ;;
    *)
        echo "Usage: op-snapshot <command> [args]"
        echo
        echo "Commands:"
        echo "  create [type] [name] [description]  - Create a snapshot"
        echo "  list [type]                         - List snapshots"
        echo "  send <snapshot> <target> [parent]   - Send snapshot to target"
        echo "  delete <snapshot>                   - Delete a snapshot"
        echo "  rollback <snapshot>                 - Rollback to snapshot"
        echo
        echo "Types: base, modules, full"
        ;;
esac
SCRIPT

    chmod +x /opt/op-dbus/bin/op-snapshot
    ln -sf /opt/op-dbus/bin/op-snapshot /usr/local/bin/op-snapshot
    
    log_success "Release server configuration complete"
}

# ============================================================================
# PHASE 4: CHATBOT MODULE MANAGEMENT TOOLS
# ============================================================================

phase_module_tools() {
    log_step "Phase 4: Installing module management tools..."
    
    # Create module management script for chatbot
    cat > /opt/op-dbus/bin/op-module << 'SCRIPT'
#!/bin/bash
#
# OP-DBUS Module Management
# Used by chatbot to manage modules in subvolumes
#
# Usage:
#   op-module install <type> <name> <source>
#   op-module remove <type> <name>
#   op-module list [type]
#   op-module stage <type> <name> <source>
#   op-module commit <staging-id>
#   op-module reject <staging-id>
#

set -euo pipefail

source /etc/op-dbus/release-server.conf 2>/dev/null || true

MODULES_DIR="/opt/op-dbus/modules"
STAGING_DIR="/opt/op-dbus/staging"

cmd_install() {
    local type="$1"  # agents, commands, mcp-servers, workflows, templates
    local name="$2"
    local source="$3"
    
    local target_dir="$MODULES_DIR/$type/$name"
    
    if [[ -d "$target_dir" ]]; then
        echo "Module already exists: $type/$name"
        echo "Use 'op-module remove $type $name' first to replace"
        exit 1
    fi
    
    mkdir -p "$target_dir"
    
    # Handle different source types
    if [[ "$source" == http* ]]; then
        # Download from URL
        if [[ "$source" == *.git ]]; then
            git clone "$source" "$target_dir"
        else
            curl -sL "$source" -o "$target_dir/module.tar.gz"
            tar -xzf "$target_dir/module.tar.gz" -C "$target_dir"
            rm "$target_dir/module.tar.gz"
        fi
    elif [[ "$source" == npm:* ]]; then
        # Install from npm
        local pkg="${source#npm:}"
        cd "$target_dir"
        npm init -y > /dev/null
        npm install "$pkg"
    elif [[ -d "$source" ]]; then
        # Copy from local directory
        cp -a "$source"/* "$target_dir/"
    elif [[ -f "$source" ]]; then
        # Copy single file
        cp "$source" "$target_dir/"
    else
        echo "Unknown source: $source"
        exit 1
    fi
    
    # Create metadata
    cat > "$target_dir/.module.json" << EOF
{
    "name": "$name",
    "type": "$type",
    "installed_at": "$(date -Iseconds)",
    "source": "$source",
    "installed_by": "$(whoami)"
}
EOF
    
    echo "Installed module: $type/$name"
    
    # Auto-snapshot if enabled
    if [[ "${AUTO_SNAPSHOT:-false}" == "true" ]]; then
        op-snapshot create modules "${AUTO_SNAPSHOT_PREFIX:-auto}-$name" "Installed $type/$name"
    fi
}

cmd_remove() {
    local type="$1"
    local name="$2"
    
    local target_dir="$MODULES_DIR/$type/$name"
    
    if [[ ! -d "$target_dir" ]]; then
        echo "Module not found: $type/$name"
        exit 1
    fi
    
    rm -rf "$target_dir"
    echo "Removed module: $type/$name"
    
    # Auto-snapshot if enabled
    if [[ "${AUTO_SNAPSHOT:-false}" == "true" ]]; then
        op-snapshot create modules "${AUTO_SNAPSHOT_PREFIX:-auto}-remove-$name" "Removed $type/$name"
    fi
}

cmd_list() {
    local type="${1:-}"
    
    if [[ -n "$type" ]]; then
        echo "Modules of type '$type':"
        if [[ -d "$MODULES_DIR/$type" ]]; then
            for module in "$MODULES_DIR/$type"/*; do
                if [[ -d "$module" ]]; then
                    local name=$(basename "$module")
                    local meta="$module/.module.json"
                    if [[ -f "$meta" ]]; then
                        local installed=$(jq -r '.installed_at' "$meta")
                        printf "  %-30s %s\n" "$name" "$installed"
                    else
                        echo "  $name"
                    fi
                fi
            done
        fi
    else
        for t in agents commands mcp-servers workflows templates; do
            if [[ -d "$MODULES_DIR/$t" ]]; then
                local count=$(find "$MODULES_DIR/$t" -maxdepth 1 -mindepth 1 -type d | wc -l)
                echo "$t: $count modules"
            fi
        done
    fi
}

cmd_stage() {
    local type="$1"
    local name="$2"
    local source="$3"
    
    local staging_id="$(date +%Y%m%d%H%M%S)-$$"
    local staging_path="$STAGING_DIR/pending/$staging_id"
    
    mkdir -p "$staging_path"
    
    # Create staging metadata
    cat > "$staging_path/staging.json" << EOF
{
    "staging_id": "$staging_id",
    "type": "$type",
    "name": "$name",
    "source": "$source",
    "staged_at": "$(date -Iseconds)",
    "staged_by": "$(whoami)",
    "status": "pending"
}
EOF
    
    # Copy/download to staging
    mkdir -p "$staging_path/content"
    if [[ -d "$source" ]]; then
        cp -a "$source"/* "$staging_path/content/"
    elif [[ -f "$source" ]]; then
        cp "$source" "$staging_path/content/"
    elif [[ "$source" == http* ]]; then
        curl -sL "$source" -o "$staging_path/content/download"
    fi
    
    echo "Staged for review: $staging_id"
    echo "Use 'op-module commit $staging_id' to install"
    echo "Use 'op-module reject $staging_id' to discard"
}

cmd_commit() {
    local staging_id="$1"
    local staging_path="$STAGING_DIR/pending/$staging_id"
    
    if [[ ! -d "$staging_path" ]]; then
        echo "Staging not found: $staging_id"
        exit 1
    fi
    
    local meta="$staging_path/staging.json"
    local type=$(jq -r '.type' "$meta")
    local name=$(jq -r '.name' "$meta")
    
    # Install from staging
    local target_dir="$MODULES_DIR/$type/$name"
    mkdir -p "$target_dir"
    cp -a "$staging_path/content"/* "$target_dir/"
    
    # Update metadata
    jq '.status = "committed" | .committed_at = "'$(date -Iseconds)'"' "$meta" > "$meta.tmp"
    mv "$meta.tmp" "$meta"
    
    # Move to approved
    mv "$staging_path" "$STAGING_DIR/approved/"
    
    echo "Committed: $type/$name"
}

cmd_reject() {
    local staging_id="$1"
    local staging_path="$STAGING_DIR/pending/$staging_id"
    
    if [[ ! -d "$staging_path" ]]; then
        echo "Staging not found: $staging_id"
        exit 1
    fi
    
    # Update metadata
    local meta="$staging_path/staging.json"
    jq '.status = "rejected" | .rejected_at = "'$(date -Iseconds)'"' "$meta" > "$meta.tmp"
    mv "$meta.tmp" "$meta"
    
    # Move to rejected
    mv "$staging_path" "$STAGING_DIR/rejected/"
    
    echo "Rejected: $staging_id"
}

case "${1:-help}" in
    install) cmd_install "$2" "$3" "$4" ;;
    remove)  cmd_remove "$2" "$3" ;;
    list)    cmd_list "${2:-}" ;;
    stage)   cmd_stage "$2" "$3" "$4" ;;
    commit)  cmd_commit "$2" ;;
    reject)  cmd_reject "$2" ;;
    *)
        echo "Usage: op-module <command> [args]"
        echo
        echo "Commands:"
        echo "  install <type> <name> <source>  - Install a module"
        echo "  remove <type> <name>            - Remove a module"
        echo "  list [type]                     - List modules"
        echo "  stage <type> <name> <source>    - Stage for review"
        echo "  commit <staging-id>             - Commit staged module"
        echo "  reject <staging-id>             - Reject staged module"
        echo
        echo "Types: agents, commands, mcp-servers, workflows, templates"
        echo
        echo "Sources:"
        echo "  - Local directory: /path/to/module"
        echo "  - Local file: /path/to/file.md"
        echo "  - Git URL: https://github.com/user/repo.git"
        echo "  - HTTP URL: https://example.com/module.tar.gz"
        echo "  - NPM package: npm:@modelcontextprotocol/server-github"
        ;;
esac
SCRIPT

    chmod +x /opt/op-dbus/bin/op-module
    ln -sf /opt/op-dbus/bin/op-module /usr/local/bin/op-module
    
    log_success "Module management tools installed"
}

# ============================================================================
# PHASE 5: DEPLOYMENT TOOLS
# ============================================================================

phase_deploy_tools() {
    log_step "Phase 5: Installing deployment tools..."
    
    # Create deployment script
    cat > /opt/op-dbus/bin/op-deploy << 'SCRIPT'
#!/bin/bash
#
# OP-DBUS Deployment Tool
# Deploy snapshots to target systems via btrfs send/receive
#
# Usage:
#   op-deploy create <name> [description]     - Create deployment snapshot
#   op-deploy send <snapshot> <target>        - Send to single target
#   op-deploy broadcast <snapshot> <group>    - Send to target group
#   op-deploy status <target>                 - Check target status
#   op-deploy targets                         - List deployment targets
#   op-deploy add-target <name> <host>        - Add deployment target
#

set -euo pipefail

source /etc/op-dbus/release-server.conf 2>/dev/null || true

TARGETS_FILE="${DEPLOY_TARGETS_FILE:-/etc/op-dbus/deploy-targets.json}"
SNAPSHOT_BASE="/mnt/btrfs-root/${SUBVOL_SNAPSHOTS:-@op-dbus-snapshots}"

cmd_create() {
    local name="$1"
    local description="${2:-Deployment snapshot}"
    
    local snapshot_name="deploy-$name-$(date +%Y%m%d-%H%M%S)"
    
    echo "Creating deployment snapshot: $snapshot_name"
    
    # Create full snapshot (base + modules)
    op-snapshot create full "$snapshot_name" "$description"
    
    echo "Deployment snapshot ready: $snapshot_name"
}

cmd_send() {
    local snapshot="$1"
    local target="$2"
    
    # Get target info
    local host=$(jq -r ".targets[] | select(.name == \"$target\") | .host" "$TARGETS_FILE")
    
    if [[ -z "$host" || "$host" == "null" ]]; then
        echo "Target not found: $target"
        echo "Use 'op-deploy add-target $target <host>' to add it"
        exit 1
    fi
    
    echo "Deploying $snapshot to $target ($host)..."
    
    # Find parent snapshot for incremental send
    local last_deploy=$(jq -r ".targets[] | select(.name == \"$target\") | .last_snapshot" "$TARGETS_FILE")
    
    if [[ -n "$last_deploy" && "$last_deploy" != "null" && -d "$SNAPSHOT_BASE/$last_deploy" ]]; then
        echo "Using incremental send from: $last_deploy"
        op-snapshot send "$snapshot" "$host" "$last_deploy"
    else
        echo "Full send (no parent snapshot)"
        op-snapshot send "$snapshot" "$host"
    fi
    
    # Update target's last snapshot
    local temp_file=$(mktemp)
    jq "(.targets[] | select(.name == \"$target\")).last_snapshot = \"$snapshot\" | (.targets[] | select(.name == \"$target\")).last_deploy = \"$(date -Iseconds)\"" "$TARGETS_FILE" > "$temp_file"
    mv "$temp_file" "$TARGETS_FILE"
    
    echo "Deployment complete: $target"
}

cmd_broadcast() {
    local snapshot="$1"
    local group="$2"
    
    local targets=$(jq -r ".groups.\"$group\"[]" "$TARGETS_FILE" 2>/dev/null)
    
    if [[ -z "$targets" ]]; then
        echo "No targets in group: $group"
        exit 1
    fi
    
    echo "Broadcasting $snapshot to group: $group"
    
    for target in $targets; do
        echo "---"
        cmd_send "$snapshot" "$target" || echo "Failed: $target"
    done
    
    echo "---"
    echo "Broadcast complete"
}

cmd_status() {
    local target="$1"
    
    local host=$(jq -r ".targets[] | select(.name == \"$target\") | .host" "$TARGETS_FILE")
    
    if [[ -z "$host" || "$host" == "null" ]]; then
        echo "Target not found: $target"
        exit 1
    fi
    
    echo "Checking status of $target ($host)..."
    
    # Check SSH connectivity
    if ssh -o ConnectTimeout=5 "$host" "echo ok" &>/dev/null; then
        echo "  SSH: OK"
    else
        echo "  SSH: FAILED"
        exit 1
    fi
    
    # Check op-dbus status
    ssh "$host" "systemctl is-active op-chat-server 2>/dev/null || echo 'not running'" | while read status; do
        echo "  op-chat-server: $status"
    done
    
    # Check last deployment
    local last=$(jq -r ".targets[] | select(.name == \"$target\") | .last_deploy" "$TARGETS_FILE")
    echo "  Last deploy: ${last:-never}"
}

cmd_targets() {
    echo "Deployment targets:"
    echo
    jq -r '.targets[] | "  \(.name): \(.host) (last: \(.last_deploy // "never"))"' "$TARGETS_FILE"
    echo
    echo "Groups:"
    jq -r '.groups | to_entries[] | "  \(.key): \(.value | length) targets"' "$TARGETS_FILE"
}

cmd_add_target() {
    local name="$1"
    local host="$2"
    local group="${3:-${DEFAULT_GROUP:-staging}}"
    
    local temp_file=$(mktemp)
    
    # Add target
    jq ".targets += [{\"name\": \"$name\", \"host\": \"$host\", \"added_at\": \"$(date -Iseconds)\"}]" "$TARGETS_FILE" > "$temp_file"
    mv "$temp_file" "$TARGETS_FILE"
    
    # Add to group
    jq ".groups.\"$group\" += [\"$name\"]" "$TARGETS_FILE" > "$temp_file"
    mv "$temp_file" "$TARGETS_FILE"
    
    echo "Added target: $name ($host) to group: $group"
}

case "${1:-help}" in
    create)     cmd_create "$2" "${3:-}" ;;
    send)       cmd_send "$2" "$3" ;;
    broadcast)  cmd_broadcast "$2" "$3" ;;
    status)     cmd_status "$2" ;;
    targets)    cmd_targets ;;
    add-target) cmd_add_target "$2" "$3" "${4:-}" ;;
    *)
        echo "Usage: op-deploy <command> [args]"
        echo
        echo "Commands:"
        echo "  create <name> [description]     - Create deployment snapshot"
        echo "  send <snapshot> <target>        - Send to single target"
        echo "  broadcast <snapshot> <group>    - Send to target group"
        echo "  status <target>                 - Check target status"
        echo "  targets                         - List deployment targets"
        echo "  add-target <name> <host> [group] - Add deployment target"
        ;;
esac
SCRIPT

    chmod +x /opt/op-dbus/bin/op-deploy
    ln -sf /opt/op-dbus/bin/op-deploy /usr/local/bin/op-deploy
    
    log_success "Deployment tools installed"
}

# ============================================================================
# PHASE 6: CREATE INITIAL SNAPSHOTS
# ============================================================================

phase_initial_snapshots() {
    log_step "Phase 6: Creating initial deployment snapshots..."
    
    # Create base snapshot
    op-snapshot create base "initial" "Initial base installation"
    
    # Create full snapshot (base + empty modules)
    op-snapshot create full "initial" "Initial full installation"
    
    log_success "Initial snapshots created"
}

# ============================================================================
# MAIN
# ============================================================================

print_summary() {
    local ip=$(hostname -I | awk '{print $1}')
    
    echo
    echo "============================================================================"
    echo -e "${GREEN}OP-DBUS RELEASE SERVER INSTALLATION COMPLETE${NC}"
    echo "============================================================================"
    echo
    echo "This server is now configured as a release/deployment server."
    echo
    echo "Access Points:"
    echo "  Chat Interface:  https://$ip/chat/"
    echo "  API Health:      https://$ip/api/health"
    echo
    echo "Management Commands:"
    echo "  op-snapshot   - Manage BTRFS snapshots"
    echo "  op-module     - Manage chatbot modules"
    echo "  op-deploy     - Deploy to target systems"
    echo
    echo "BTRFS Subvolumes:"
    echo "  @op-dbus-base      - Base system (non-modular)"
    echo "  @op-dbus-modules   - Chatbot-managed modules"
    echo "  @op-dbus-snapshots - Deployment snapshots"
    echo "  @op-dbus-staging   - Staging area"
    echo
    echo "Workflow:"
    echo "  1. Use chatbot to install modules:"
    echo "     'Install the GitHub MCP server'"
    echo "     'Load agents from ~/agents/'"
    echo
    echo "  2. Chatbot stages changes in @staging subvolume"
    echo
    echo "  3. Review and commit staged changes:"
    echo "     op-module list"
    echo "     op-module commit <staging-id>"
    echo
    echo "  4. Create deployment snapshot:"
    echo "     op-deploy create prod-v1.0 'Production release'"
    echo
    echo "  5. Deploy to targets:"
    echo "     op-deploy add-target prod-server-1 user@10.0.0.10"
    echo "     op-deploy send deploy-prod-v1.0 prod-server-1"
    echo
    echo "  Or broadcast to group:"
    echo "     op-deploy broadcast deploy-prod-v1.0 production"
    echo
    echo "============================================================================"
}

main() {
    echo
    echo "============================================================================"
    echo "OP-DBUS RELEASE SERVER INSTALLATION"
    echo "============================================================================"
    echo
    
    # Check we're root
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        exit 1
    fi
    
    phase_btrfs_setup
    phase_base_install
    phase_release_config
    phase_module_tools
    phase_deploy_tools
    phase_initial_snapshots
    
    print_summary
}

main "$@"
