#!/usr/bin/env bash
# Create a macOS-capable Incus VM using OpenCore and installer media.
#
# Sources used for compatibility defaults:
# - macOS-on-Incus guide (raw.qemu/raw.apparmor/scriptlet flow)
# - OSX-KVM / OSX-PROXMOX (Apple SMC OSK, SMBIOS type 2, q35-style assumptions)
# - OpenCore-ISO (boot media workflow and CPU/machine recommendations)

set -euo pipefail

SCRIPT_NAME="$(basename "$0")"

# Defaults
VM_NAME="macos-incus"
STORAGE_POOL=""
CREATE_BTRFS_POOL=1
BTRFS_POOL_NAME="macos-btrfs"
BTRFS_POOL_SIZE="600GiB"
VCPU="6"
MEMORY="12GiB"
DISK_SIZE="220GiB"
STATE_SIZE=""
ROOT_BUS="nvme"
MEDIA_BUS="virtio-scsi"
OPENCORE_MEDIA=""
INSTALLER_MEDIA=""
FETCH_OPENCORE=0
DOWNLOAD_DIR="${PWD}"
START_VM=1
RECREATE=0
BOOT_AUTOSTART="true"
USE_SCRIPTLET=1
SCRIPTLET_PATH=""
SCRIPTLET_URL="https://raw.githubusercontent.com/macOS-on-Incus/QEMU-Scriptlet/refs/heads/main/scriptlet.py"

# Publicly documented Apple SMC key used in OSX-KVM style setups.
APPLE_OSK="ourhardworkbythesewordsguardedpleasedontsteal(c)AppleComputerInc"

log() {
    printf '[osx-incus] %s\n' "$*"
}

warn() {
    printf '[osx-incus][warn] %s\n' "$*" >&2
}

die() {
    printf '[osx-incus][error] %s\n' "$*" >&2
    exit 1
}

usage() {
    cat <<USAGE
Usage:
  $SCRIPT_NAME --opencore <path> --installer <path> [options]

Required:
  --opencore <path>        Path to OpenCore boot media (ISO preferred; IMG/QCOW2 also works)
  --installer <path>       Path to macOS installer/recovery media (ISO or BaseSystem.img)

Options:
  --name <vm-name>         VM name (default: ${VM_NAME})
  --pool <name>            Incus storage pool name
  --disk-size <size>       Root disk size (default: ${DISK_SIZE})
  --state-size <size>      VM state size override (Btrfs pools)
  --cpu <count>            vCPU count (default: ${VCPU})
  --memory <size>          Memory (default: ${MEMORY})
  --root-bus <bus>         Root disk bus: nvme|virtio-blk|virtio-scsi|auto (default: ${ROOT_BUS})
  --fetch-opencore         Download latest OpenCore ISO from LongQT-sea/OpenCore-ISO
  --download-dir <dir>     Download directory for --fetch-opencore (default: current dir)
  --no-scriptlet           Skip QEMU scriptlet injection
  --scriptlet <path>       Use local raw.qemu.scriptlet file instead of downloading
  --scriptlet-url <url>    Override scriptlet URL (default: macOS-on-Incus scriptlet)
  --no-start               Create/configure VM but do not start it
  --recreate               Delete and recreate VM if it already exists
  --no-create-btrfs-pool   Do not auto-create a Btrfs pool when none exists
  --autostart <true|false> Set boot.autostart (default: ${BOOT_AUTOSTART})
  -h, --help               Show this help

Example:
  $SCRIPT_NAME \\
    --name sonoma \\
    --opencore ./OpenCore_1.0.5.iso \\
    --installer ./BaseSystem.img \\
    --cpu 8 --memory 16GiB --disk-size 300GiB
USAGE
}

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

abs_path() {
    local input="$1"

    if command -v realpath >/dev/null 2>&1; then
        realpath "$input"
        return
    fi

    if command -v readlink >/dev/null 2>&1; then
        readlink -f "$input"
        return
    fi

    local dir
    dir="$(cd "$(dirname "$input")" && pwd)"
    printf '%s/%s\n' "$dir" "$(basename "$input")"
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --name)
                VM_NAME="$2"
                shift 2
                ;;
            --pool)
                STORAGE_POOL="$2"
                shift 2
                ;;
            --disk-size)
                DISK_SIZE="$2"
                shift 2
                ;;
            --state-size)
                STATE_SIZE="$2"
                shift 2
                ;;
            --cpu)
                VCPU="$2"
                shift 2
                ;;
            --memory)
                MEMORY="$2"
                shift 2
                ;;
            --root-bus)
                ROOT_BUS="$2"
                shift 2
                ;;
            --opencore)
                OPENCORE_MEDIA="$2"
                shift 2
                ;;
            --installer)
                INSTALLER_MEDIA="$2"
                shift 2
                ;;
            --fetch-opencore)
                FETCH_OPENCORE=1
                shift
                ;;
            --download-dir)
                DOWNLOAD_DIR="$2"
                shift 2
                ;;
            --no-scriptlet)
                USE_SCRIPTLET=0
                shift
                ;;
            --scriptlet)
                SCRIPTLET_PATH="$2"
                USE_SCRIPTLET=1
                shift 2
                ;;
            --scriptlet-url)
                SCRIPTLET_URL="$2"
                USE_SCRIPTLET=1
                shift 2
                ;;
            --no-start)
                START_VM=0
                shift
                ;;
            --recreate)
                RECREATE=1
                shift
                ;;
            --no-create-btrfs-pool)
                CREATE_BTRFS_POOL=0
                shift
                ;;
            --autostart)
                BOOT_AUTOSTART="$2"
                shift 2
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                die "unknown argument: $1"
                ;;
        esac
    done
}

incus_reachable() {
    incus info >/dev/null 2>&1
}

storage_driver_for_pool() {
    local pool="$1"
    incus storage list -f csv -c nD | awk -F, -v p="$pool" '$1 == p { print $2; exit }'
}

first_pool_by_driver() {
    local driver="$1"
    incus storage list -f csv -c nD | awk -F, -v d="$driver" '$2 == d { print $1; exit }'
}

first_pool_any() {
    incus storage list -f csv -c n | head -n 1
}

pool_exists() {
    incus storage show "$1" >/dev/null 2>&1
}

ensure_btrfs_pool_if_requested() {
    local existing_btrfs
    existing_btrfs="$(first_pool_by_driver btrfs || true)"

    if [[ -n "$existing_btrfs" ]]; then
        STORAGE_POOL="$existing_btrfs"
        log "Using existing Btrfs pool: ${STORAGE_POOL}"
        return
    fi

    if [[ "$CREATE_BTRFS_POOL" -eq 1 ]]; then
        log "No Btrfs pool found. Creating '${BTRFS_POOL_NAME}' (size=${BTRFS_POOL_SIZE})..."
        if incus storage create "$BTRFS_POOL_NAME" btrfs "size=${BTRFS_POOL_SIZE}" >/dev/null 2>&1; then
            STORAGE_POOL="$BTRFS_POOL_NAME"
            log "Created Btrfs pool: ${STORAGE_POOL}"
            return
        fi
        warn "Failed to create Btrfs pool automatically. Falling back to an existing pool."
    fi

    local any_pool
    any_pool="$(first_pool_any || true)"
    [[ -n "$any_pool" ]] || die "no Incus storage pool found; create one first"
    STORAGE_POOL="$any_pool"
    warn "Using non-Btrfs pool: ${STORAGE_POOL}"
}

double_size_if_integer_unit() {
    local size="$1"
    if [[ "$size" =~ ^([0-9]+)(TiB|GiB|MiB)$ ]]; then
        local n="${BASH_REMATCH[1]}"
        local unit="${BASH_REMATCH[2]}"
        printf '%s%s\n' "$((n * 2))" "$unit"
        return 0
    fi

    return 1
}

fetch_latest_opencore_iso() {
    require_cmd curl
    mkdir -p "$DOWNLOAD_DIR"

    local api_url="https://api.github.com/repos/LongQT-sea/OpenCore-ISO/releases/latest"
    local release_json
    local iso_url
    local output

    log "Fetching latest OpenCore ISO release metadata..."
    release_json="$(curl -fsSL "$api_url")"

    iso_url="$(printf '%s\n' "$release_json" | awk -F'"' '/browser_download_url/ && /\.iso/ {print $4; exit}')"
    [[ -n "$iso_url" ]] || die "could not find an ISO asset in latest OpenCore-ISO release"

    output="${DOWNLOAD_DIR}/$(basename "$iso_url")"
    log "Downloading OpenCore ISO to: $output"
    curl -fL "$iso_url" -o "$output"

    OPENCORE_MEDIA="$output"
}

vm_exists() {
    incus info "$VM_NAME" >/dev/null 2>&1
}

remove_vm_if_needed() {
    if vm_exists; then
        if [[ "$RECREATE" -eq 1 ]]; then
            log "VM '${VM_NAME}' already exists; recreating..."
            incus stop "$VM_NAME" --force >/dev/null 2>&1 || true
            incus delete "$VM_NAME" >/dev/null 2>&1
        else
            die "VM '${VM_NAME}' already exists (use --recreate to replace it)"
        fi
    fi
}

create_vm() {
    log "Creating empty VM '${VM_NAME}' on pool '${STORAGE_POOL}'..."
    incus init "$VM_NAME" --empty --vm --storage "$STORAGE_POOL" \
        -c "image.os=macOS" \
        -c "limits.cpu=${VCPU}" \
        -c "limits.memory=${MEMORY}" \
        -c "security.secureboot=false" \
        -c "boot.autostart=${BOOT_AUTOSTART}"

    incus config device set "$VM_NAME" root size "$DISK_SIZE"
    incus config device set "$VM_NAME" root io.bus "$ROOT_BUS"

    local pool_driver
    pool_driver="$(storage_driver_for_pool "$STORAGE_POOL")"

    if [[ "$pool_driver" == "btrfs" ]]; then
        local resolved_state_size="$STATE_SIZE"

        if [[ -z "$resolved_state_size" ]]; then
            if resolved_state_size="$(double_size_if_integer_unit "$DISK_SIZE")"; then
                log "Btrfs pool detected; setting size.state=${resolved_state_size}"
            else
                warn "Btrfs pool detected but DISK_SIZE '${DISK_SIZE}' is not a simple integer unit; skipping size.state"
                resolved_state_size=""
            fi
        fi

        if [[ -n "$resolved_state_size" ]]; then
            incus config device set "$VM_NAME" root size.state "$resolved_state_size"
        fi
    fi

    # macOS-specific QEMU args translated from OSX-KVM/OSX-PROXMOX guidance.
    incus config set "$VM_NAME" \
        "raw.qemu=-device isa-applesmc,osk=${APPLE_OSK} -smbios type=2 -global ICH9-LPC.acpi-pci-hotplug-with-bridge-support=off -global nec-usb-xhci.msi=off"

    # Needed by current macOS-on-Incus scriptlet flow.
    incus config set "$VM_NAME" \
        "raw.apparmor=mount options=(rw,bind) /dev/null/ -> /run/incus_agent.*"
}

remove_device_if_present() {
    local dev="$1"
    if incus config device get "$VM_NAME" "$dev" type >/dev/null 2>&1; then
        incus config device remove "$VM_NAME" "$dev"
    fi
}

attach_media_disk() {
    local device_name="$1"
    local media_path="$2"
    local boot_priority="$3"

    remove_device_if_present "$device_name"

    incus config device add "$VM_NAME" "$device_name" disk \
        source="$media_path" \
        readonly=true \
        io.bus="$MEDIA_BUS" \
        boot.priority="$boot_priority"
}

apply_scriptlet() {
    [[ "$USE_SCRIPTLET" -eq 1 ]] || {
        log "Skipping raw.qemu.scriptlet (--no-scriptlet)"
        return
    }

    local source_file="$SCRIPTLET_PATH"
    local temp_file=""

    if [[ -z "$source_file" ]]; then
        require_cmd curl
        temp_file="$(mktemp)"
        log "Downloading QEMU scriptlet: ${SCRIPTLET_URL}"
        curl -fsSL "$SCRIPTLET_URL" -o "$temp_file"
        source_file="$temp_file"
    fi

    [[ -f "$source_file" ]] || die "scriptlet file not found: $source_file"

    incus config set "$VM_NAME" raw.qemu.scriptlet - < "$source_file"

    if [[ -n "$temp_file" ]]; then
        rm -f "$temp_file"
    fi
}

validate_media() {
    if [[ "$FETCH_OPENCORE" -eq 1 ]] && [[ -z "$OPENCORE_MEDIA" ]]; then
        fetch_latest_opencore_iso
    fi

    [[ -n "$OPENCORE_MEDIA" ]] || die "missing required --opencore <path> (or use --fetch-opencore)"
    [[ -n "$INSTALLER_MEDIA" ]] || die "missing required --installer <path>"

    OPENCORE_MEDIA="$(abs_path "$OPENCORE_MEDIA")"
    INSTALLER_MEDIA="$(abs_path "$INSTALLER_MEDIA")"

    [[ -f "$OPENCORE_MEDIA" ]] || die "OpenCore media not found: $OPENCORE_MEDIA"
    [[ -f "$INSTALLER_MEDIA" ]] || die "Installer media not found: $INSTALLER_MEDIA"
}

main() {
    parse_args "$@"

    require_cmd incus
    incus_reachable || die "Incus server is unreachable (start incus daemon or check permissions)"

    validate_media

    if [[ -n "$STORAGE_POOL" ]]; then
        pool_exists "$STORAGE_POOL" || die "storage pool not found: $STORAGE_POOL"
    else
        ensure_btrfs_pool_if_requested
    fi

    remove_vm_if_needed
    create_vm

    log "Attaching OpenCore media: ${OPENCORE_MEDIA}"
    attach_media_disk "opencore" "$OPENCORE_MEDIA" "20"

    log "Attaching installer media: ${INSTALLER_MEDIA}"
    attach_media_disk "installer" "$INSTALLER_MEDIA" "10"

    apply_scriptlet

    if [[ "$START_VM" -eq 1 ]]; then
        log "Starting VM '${VM_NAME}'..."
        if ! incus start "$VM_NAME"; then
            if [[ "$USE_SCRIPTLET" -eq 1 ]]; then
                warn "VM failed to start with raw.qemu.scriptlet; retrying without scriptlet"
                incus config unset "$VM_NAME" raw.qemu.scriptlet || true
                incus start "$VM_NAME"
            else
                die "failed to start VM '${VM_NAME}'"
            fi
        fi
    else
        log "VM creation complete (not started)."
    fi

    cat <<SUMMARY

[osx-incus] Done.
VM name:        ${VM_NAME}
Storage pool:   ${STORAGE_POOL}
OpenCore media: ${OPENCORE_MEDIA}
Installer media:${INSTALLER_MEDIA}

Next commands:
  incus console ${VM_NAME} --type=vga
  incus config show ${VM_NAME} --expanded

SUMMARY
}

main "$@"
