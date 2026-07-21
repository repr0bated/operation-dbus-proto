#!/usr/bin/env bash
#
# 3tched / OP-DBUS — comprehensive Artix Linux s6 installer
#
# Target: a fresh Artix Linux s6 system installed from
#         artix-base-s6-20260402-x86_64.iso (s6 + s6-rc init, pacman).
#
# What this does:
#   1. Installs every package needed to build and run operation-dbus-proto
#      (Rust toolchain, protoc, node, OVS, WireGuard, Incus, xray, D-Bus, ...)
#   2. Installs a Wayland desktop: Hyprland + greetd/tuigreet + pipewire,
#      seat management via elogind (seatd fallback), all supervised by s6.
#   3. Builds the workspace from crates/ and installs the binaries.
#   4. Generates s6-rc service definitions for the whole 3tched control
#      plane and the network, every longrun paired with a dedicated s6-log
#      consumer (producer-for / consumer-for / pipeline-name), logging to
#      /var/log/op-dbus/<service>/ as the s6log user.
#   5. Brings the network up via s6 through ONE command (3tched-bridge-up),
#      each phase an idempotent oneshot body:
#        dhcp (lease + snapshot on the physical NIC, before OVS owns it)
#        -> ovsdb-server -> ovs-vswitchd (bridge creation and eth0
#        enslavement in ONE OVSDB transact via op-ovsbr0-setup --seed-only)
#        -> uplink migration onto ovsbr0 (host network stable BEFORE any
#        container) -> netmaker enslaved -> service addresses (10.200.0.1 +
#        10.0.0.2) applied LAST, after netmaker, to avoid racing its
#        bring-up -> OpenFlow controller.  NO WireGuard and NO AF_XDP on
#      the host: the netmaker mesh is self-contained in the netmaker
#      container (WG terminates at the decoy server; 10.0.0.2 egress
#      forwards to host xray, which injects the identity header — traffic
#      leaves as gRPC).  The D-Bus session bus and the gRPC bridge (UDS +
#      loopback :8090) start EARLY, in parallel with the network chain.
#   6. Supervises Incus containers with s6 (+ log pipelines) via the
#      generated 3tched-incus-svcgen helper — never Incus autostart; the
#      netmaker container joins the bridge before the service addresses,
#      readiness probed over D-Bus with busctl (never systemctl).
#
# The deploy/ tree is deprecated; this script derives everything from the
# crates/ workspace (op-core config constants, op-network binaries,
# op-s6-systemctl paths) and generates its own service definitions.
#
# Usage:
#   sudo ./install/3tched-artix-s6-install.sh [options]
#
# Options:
#   --user NAME          Operator user (default: jeremy)
#   --repo DIR           Existing repo checkout (default: autodetect / clone)
#   --skip-desktop       Skip Hyprland/Wayland desktop installation
#   --skip-build         Skip cargo/npm builds (system + services only)
#   --skip-start         Install and enable services but do not start them
#   --with-headless-gui  Install weston + wayvnc headless compositor services
#   --with-dev-ui        Also npm-build the primary UI dev tree in crates/
#   --help               Show this help
#
# Environment overrides:
#   ZBUS_GIT_URL   zbus checkout URL for the [patch.crates-io] pin
#                  (default: https://github.com/dbus2/zbus.git)
#   REPO_GIT_URL   operation-dbus-proto clone URL
#                  (default: https://github.com/repr0bated/operation-dbus-proto.git)

set -euo pipefail

# ============================================================================
# CONFIGURATION
# ============================================================================

OP_USER="jeremy"
REPO_DIR=""
REPO_GIT_URL="${REPO_GIT_URL:-https://github.com/repr0bated/operation-dbus-proto.git}"
ZBUS_GIT_URL="${ZBUS_GIT_URL:-https://github.com/dbus2/zbus.git}"

SKIP_DESKTOP=0
SKIP_BUILD=0
SKIP_START=0
WITH_HEADLESS_GUI=0
WITH_DEV_UI=0

# Canonical paths (from op-core::config and op-s6-systemctl::dbus)
ENV_FILE="/etc/op-dbus/environment"
NET_CONF="/etc/op-dbus/network.conf"
SESSION_BUS_SOCKET="/run/opdbus/session-bus.sock"        # op_core::config::SESSION_BUS_SOCKET_PATH
SESSION_BUS_ADDRESS="unix:path=${SESSION_BUS_SOCKET}"    # op_core::config::SESSION_BUS_ADDRESS
BLOB_SHM_DIR="/dev/shm/opdbus/plugin-blobs"              # op_blob::catalog::DEFAULT_SHM_DIR
S6_SV_DIR="/etc/s6/sv"                                   # s6 source dir (op-s6-systemctl default)
S6_SCAN_DIR="/run/service"                               # s6 scan dir (op-s6-systemctl default)
STATE_DIR="/var/lib/op-dbus"
LOG_ROOT="/var/log/op-dbus"
LIBEXEC_DIR="/usr/local/libexec/3tched"
BIN_DIR="/usr/local/bin"

# Log rotation directives for every s6-log consumer
LOG_DIRECTIVES="n10 s4000000 T"

MISSING_PKGS=()
ARCH_SUPPORT_TRIED=0

# ============================================================================
# HELPERS
# ============================================================================

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
log()  { echo -e "${BLUE}[3tched]${NC} $*"; }
ok()   { echo -e "${GREEN}[  ok  ]${NC} $*"; }
warn() { echo -e "${YELLOW}[ warn ]${NC} $*"; }
die()  { echo -e "${RED}[ fail ]${NC} $*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

usage() { sed -n '2,50p' "$0" | sed 's/^# \{0,1\}//'; exit 0; }

while [[ $# -gt 0 ]]; do
    case "$1" in
        --user)              OP_USER="$2"; shift 2 ;;
        --repo)              REPO_DIR="$2"; shift 2 ;;
        --skip-desktop)      SKIP_DESKTOP=1; shift ;;
        --skip-build)        SKIP_BUILD=1; shift ;;
        --skip-start)        SKIP_START=1; shift ;;
        --with-headless-gui) WITH_HEADLESS_GUI=1; shift ;;
        --with-dev-ui)       WITH_DEV_UI=1; shift ;;
        --help|-h)           usage ;;
        *) die "unknown option: $1 (see --help)" ;;
    esac
done

OP_HOME="/home/${OP_USER}"

as_user() {
    # Run a command as the operator user with a sane environment.
    sudo -u "$OP_USER" env HOME="$OP_HOME" USER="$OP_USER" LOGNAME="$OP_USER" \
        PATH="${OP_HOME}/.cargo/bin:/usr/local/bin:/usr/bin:/bin" "$@"
}

# ----------------------------------------------------------------------------
# pacman helpers — try the Artix repos first; if a package is missing, enable
# the Arch package support layer (artix-archlinux-support + [extra]) once and
# retry.  Failures are collected, not fatal.
# ----------------------------------------------------------------------------

enable_arch_support() {
    [[ $ARCH_SUPPORT_TRIED -eq 1 ]] && return 0
    ARCH_SUPPORT_TRIED=1
    log "Enabling Arch package support (artix-archlinux-support + [extra])..."
    pacman -S --needed --noconfirm artix-archlinux-support || {
        warn "artix-archlinux-support not installable; Arch repos unavailable"
        return 1
    }
    if ! grep -q '^\[extra\]' /etc/pacman.conf; then
        cat >> /etc/pacman.conf <<'EOF'

# Added by 3tched-artix-s6-install.sh — Arch package support
[extra]
Include = /etc/pacman.d/mirrorlist-arch
EOF
    fi
    pacman-key --populate archlinux || true
    pacman -Sy || true
}

pac() {
    # pac <pkg>... — install packages, retrying individually and via the Arch
    # support repos before giving up on any of them.
    local failed=()
    pacman -S --needed --noconfirm "$@" 2>/dev/null && return 0
    for p in "$@"; do
        pacman -S --needed --noconfirm "$p" 2>/dev/null && continue
        enable_arch_support || true
        pacman -S --needed --noconfirm "$p" 2>/dev/null && continue
        failed+=("$p")
    done
    if [[ ${#failed[@]} -gt 0 ]]; then
        warn "packages not installable: ${failed[*]}"
        MISSING_PKGS+=("${failed[@]}")
    fi
}

# ============================================================================
# PHASE 0 — PREFLIGHT
# ============================================================================

preflight() {
    [[ $EUID -eq 0 ]] || die "run as root (sudo)"
    [[ -f /etc/artix-release ]] || warn "/etc/artix-release not found — this targets Artix Linux"
    have s6-rc || die "s6-rc not found — this targets the Artix s6 init (artix-base-s6 ISO)"
    [[ -d $S6_SV_DIR ]] || die "$S6_SV_DIR missing — is this an Artix s6 system?"
    log "Preflight OK: Artix s6 system detected"
    log "Synchronising package databases..."
    pacman -Syu --noconfirm || warn "pacman -Syu failed — continuing with stale databases"
}

# ============================================================================
# PHASE 1 — PACKAGES
# ============================================================================

install_packages() {
    log "Installing build toolchain and core system packages..."
    pac base-devel sudo git curl wget jq pkgconf cmake clang llvm lld \
        protobuf openssl sqlite rustup nodejs npm \
        dbus dbus-s6

    log "Installing network stack packages..."
    # No wireguard-tools and no AF_XDP tooling: there is NO WireGuard on this
    # host (the netmaker mesh is self-contained inside the netmaker container).
    # The uplink NIC is enslaved into ovsbr0 as a plain OVS port — atomically
    # with bridge creation (op-ovsbr0-setup UPLINK env, one OVSDB transact).
    pac openvswitch iproute2 iptables-nft dhcpcd \
        incus btrfs-progs xray

    if [[ $SKIP_DESKTOP -eq 0 ]]; then
        log "Installing Wayland / Hyprland desktop packages..."
        pac hyprland xdg-desktop-portal-hyprland xdg-desktop-portal-gtk \
            xorg-xwayland qt5-wayland qt6-wayland \
            waybar foot wofi mako grim slurp wl-clipboard \
            hyprpaper hyprlock hypridle \
            polkit pipewire pipewire-alsa pipewire-pulse wireplumber \
            noto-fonts ttf-dejavu \
            greetd greetd-tuigreet
        # Seat management: prefer elogind (creates XDG_RUNTIME_DIR, standard
        # for desktop Artix); fall back to bare seatd.
        pac elogind elogind-s6
        if ! pacman -Qi elogind >/dev/null 2>&1; then
            warn "elogind unavailable — falling back to seatd"
            pac seatd seatd-s6
        fi
    fi

    if [[ $WITH_HEADLESS_GUI -eq 1 ]]; then
        log "Installing headless GUI packages (weston + wayvnc)..."
        pac weston wayvnc
    fi

    ok "Package installation done"
}

# ============================================================================
# PHASE 2 — USERS, GROUPS, DIRECTORIES
# ============================================================================

setup_users() {
    if ! id "$OP_USER" >/dev/null 2>&1; then
        log "Creating operator user: $OP_USER"
        useradd -m -G wheel -s /bin/bash "$OP_USER"
        warn "set a password for $OP_USER: passwd $OP_USER"
    fi
    for grp in video input audio seat incus-admin; do
        getent group "$grp" >/dev/null 2>&1 || groupadd -r "$grp" 2>/dev/null || true
        usermod -aG "$grp" "$OP_USER" 2>/dev/null || true
    done

    # Dedicated log-writer for every s6-log consumer
    if ! id s6log >/dev/null 2>&1; then
        useradd --system --user-group --no-create-home --shell /usr/bin/nologin s6log
    fi

    install -d -m 0755 /etc/op-dbus
    install -d -m 0750 -o root -g root "$STATE_DIR"
    install -d -m 0755 -o s6log -g s6log "$LOG_ROOT"
    install -d -m 0755 "$LIBEXEC_DIR"
    ok "Users and directories ready"
}

# ============================================================================
# PHASE 3 — RUST TOOLCHAIN, ZBUS PIN, SESSION BUS BROKER
# ============================================================================

setup_rust() {
    log "Installing Rust toolchain (stable) for $OP_USER..."
    as_user rustup default stable >/dev/null 2>&1 || as_user rustup default stable ||
        die "rustup default stable failed for $OP_USER"

    # [patch.crates-io] pins zbus to a local checkout at /home/jeremy/git/zbus.
    # The path is hard-coded in Cargo.toml, so make sure it exists even when
    # the operator user is not 'jeremy'.
    local zbus_dir="${OP_HOME}/git/zbus"
    if [[ ! -d $zbus_dir ]]; then
        log "Cloning zbus checkout for the [patch.crates-io] pin..."
        as_user install -d "${OP_HOME}/git"
        as_user git clone --depth 1 "$ZBUS_GIT_URL" "$zbus_dir" ||
            warn "zbus clone failed — the workspace will NOT build without $zbus_dir"
    fi
    if [[ $OP_HOME != "/home/jeremy" && ! -e /home/jeremy/git/zbus ]]; then
        install -d /home/jeremy/git
        ln -sfn "$zbus_dir" /home/jeremy/git/zbus
        log "Symlinked /home/jeremy/git/zbus -> $zbus_dir (Cargo.toml patch path)"
    fi

    # busd — Rust D-Bus broker used for the opdbus SESSION bus.
    if [[ ! -x ${BIN_DIR}/busd ]]; then
        log "Installing busd (session bus broker)..."
        if as_user cargo install busd --locked; then
            install -m 0755 "${OP_HOME}/.cargo/bin/busd" "${BIN_DIR}/busd"
        else
            warn "cargo install busd failed — session bus will fall back to dbus-daemon"
        fi
    fi

    # zbusctl — operator CLI, lives in ~/.cargo/bin (not built from this repo).
    if ! as_user sh -c 'command -v zbusctl' >/dev/null 2>&1; then
        if [[ -d ${zbus_dir}/zbusctl ]]; then
            as_user cargo install --path "${zbus_dir}/zbusctl" ||
                warn "zbusctl build failed — install it manually into ~/.cargo/bin"
        else
            warn "zbusctl not found and not present in the zbus checkout — install it manually (~/.cargo/bin/zbusctl)"
        fi
    fi
    ok "Rust toolchain ready"
}

# ============================================================================
# PHASE 4 — REPO CHECKOUT + BUILD
# ============================================================================

locate_repo() {
    if [[ -z $REPO_DIR ]]; then
        local script_dir
        script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
        if [[ -f "${script_dir}/../Cargo.toml" ]] &&
           grep -q 'crates/op-plugins' "${script_dir}/../Cargo.toml" 2>/dev/null; then
            REPO_DIR="$(cd "${script_dir}/.." && pwd)"
        else
            REPO_DIR="${OP_HOME}/git/operation-dbus-proto"
        fi
    fi
    if [[ ! -d $REPO_DIR ]]; then
        log "Cloning operation-dbus-proto into $REPO_DIR..."
        as_user install -d "$(dirname "$REPO_DIR")"
        as_user git clone "$REPO_GIT_URL" "$REPO_DIR" || die "repo clone failed"
    fi
    log "Repo: $REPO_DIR"
}

build_project() {
    [[ $SKIP_BUILD -eq 1 ]] && { warn "--skip-build: not building"; return 0; }

    # Build as the repo owner: a root-owned checkout can't be built by the
    # operator user, and vice versa.
    local repo_owner
    repo_owner="$(stat -c %U "$REPO_DIR")"
    if [[ $repo_owner == "$OP_USER" ]]; then
        build_as() { as_user "$@"; }
    else
        log "Repo is owned by $repo_owner — building as $repo_owner"
        build_as() { "$@"; }
        # root builds need root's own toolchain
        rustup default stable >/dev/null 2>&1 || rustup default stable ||
            die "rustup default stable failed for root"
    fi

    # op-web release builds RustEmbed crates/op-web/ui/dist — build that UI
    # first or the cargo build panics.
    log "Building op-web embedded UI (crates/op-web/ui)..."
    if [[ -d ${REPO_DIR}/crates/op-web/ui ]]; then
        ( cd "${REPO_DIR}/crates/op-web/ui" &&
          build_as npm install --no-audit --no-fund &&
          build_as npx vite build ) || die "op-web UI build failed — the release build of op-web panics without ui/dist"
    fi

    if [[ $WITH_DEV_UI -eq 1 ]]; then
        log "Building primary UI dev tree (crates/)..."
        ( cd "${REPO_DIR}/crates" &&
          build_as npm install --no-audit --no-fund &&
          build_as npm run build ) || warn "dev UI build failed (non-fatal)"
    fi

    log "Building Rust workspace (release) — this takes a while..."
    ( cd "$REPO_DIR" &&
      build_as env PROTOC="$(command -v protoc || true)" \
        cargo build --release \
            -p op-web \
            -p op-projection \
            -p op-grpc-bridge \
            -p op-cognitive-mcp \
            -p op-mcp \
            -p op-s6-systemctl \
            -p op-xray-daemon \
            -p op-network \
            -p op-plugins \
            -p op-dbus-mirror
    ) || die "cargo build failed (check the zbus checkout at ${OP_HOME}/git/zbus matches the fork the workspace expects; override with ZBUS_GIT_URL=...)"

    log "Installing binaries to ${BIN_DIR}..."
    local bins=(
        op-web-server opdbus
        projection_server
        op-grpc-bridge
        op-cognitive-mcp rag-ingest op-cog-admin
        op-mcp-compact op-mcp-server op-mcp-agents
        s6d op-s6-systemctl
        op-xray-daemon
        op-of-controller op-ovsbr0-setup
        opblob
        op-dbus-mirror ovs-dbus-init
    )
    for b in "${bins[@]}"; do
        if [[ -f ${REPO_DIR}/target/release/$b ]]; then
            install -m 0755 "${REPO_DIR}/target/release/$b" "${BIN_DIR}/$b"
            ok "installed $b"
        else
            warn "binary not built: $b"
        fi
    done
}

# ============================================================================
# PHASE 5 — CONFIGURATION FILES
# ============================================================================

write_environment() {
    if [[ ! -f $ENV_FILE ]]; then
        log "Writing $ENV_FILE..."
        cat > "$ENV_FILE" <<EOF
# 3tched / OP-DBUS canonical environment  (op-core reads this at startup)
# Values grounded in crates/op-core/src/config.rs and the op-network binaries.

# SESSION bus — the WG-identity-gated plugin surface (busd)
DBUS_SESSION_BUS_ADDRESS=${SESSION_BUS_ADDRESS}

# projection_server bus selection: "session" or "system"
OP_DBUS_PROJECTION_BUS=session

# State + memory
OP_DBUS_STATE_DIR=${STATE_DIR}
OP_MEMORY_DB=${STATE_DIR}/cognitive.db
COGNITIVE_MCP_DB_PATH=${STATE_DIR}/cognitive.db

# gRPC state manager (opdbus binds the IPv4 of OP_DBUS_GRPC_IFACE:50051)
OP_DBUS_GRPC_IFACE=ovsbr0
OP_DBUS_GRPC_ADDR=http://10.200.0.1:50051

# zero-trust gRPC bridge (xray redirects gRPC traffic here)
GRPC_BIND=127.0.0.1:8090
ZEROCLAW_TLS_BIND_ADDR=127.0.0.1:8443

# OpenFlow controller for ovsbr0
OF_CONTROLLER_LISTEN=10.200.0.1:6653
OPENFLOW_CONTROLLER=tcp:10.200.0.1:6653

# HTTP API + embedded UI
PORT=8080

RUST_LOG=info
EOF
        chmod 0640 "$ENV_FILE"
    else
        log "Keeping existing $ENV_FILE"
    fi

    if [[ ! -f $NET_CONF ]]; then
        log "Writing $NET_CONF..."
        cat > "$NET_CONF" <<'EOF'
# 3tched network configuration — consumed by the s6 network services and the
# op-network binaries (op-ovsbr0-setup / op-of-controller).

# OVS bridge (kernel datapath, seeded natively over OVSDB JSON-RPC)
BRIDGE=ovsbr0
BRIDGE_ADDR=10.200.0.1/24
BRIDGE_NET=10.200.0.0/24
FAIL_MODE=standalone
# SHARED_MAC intentionally unset: op-ovsbr0-setup defaults to UPLINK's own
# MAC so the bridge presents the same address upstream as the NIC always
# did. Only set this to override — most hosting providers filter by MAC on
# the virtual NIC, so a mismatched bridge MAC silently blackholes traffic.
OVSDB_SOCKET=/run/openvswitch/db.sock
VSWITCHD_SVC=/run/service/ovs-vswitchd

# Physical NIC enslaved into the bridge.  Bridge creation and enslavement
# happen in ONE OVSDB transact (op-ovsbr0-setup UPLINK env) — done as two
# steps the uplink capture does not start correctly.  The NIC's IPv4 and
# default route are migrated onto the bridge by ovsbr0-addr.  Set empty to
# skip enslavement (e.g. when the NIC is not named eth0 yet).
UPLINK=eth0

# NAT 10.200.0.0/24 out through the default route (1 = enable)
NAT_ENABLE=1

# Netmaker network, routed over the bridge — NO WireGuard on this host.
# The netmaker container is self-contained: its bridge interface carries both
# 10.0.0.2 and 10.200.0.2, the WG protocol terminates at the decoy server,
# and 10.0.0.2 egress forwards to host xray for identity header injection
# (traffic leaves xray as gRPC with the header).
NETMAKER_NET=10.0.0.0/24
EOF
        chmod 0640 "$NET_CONF"
    else
        log "Keeping existing $NET_CONF"
    fi

    # sysctl + modules
    cat > /etc/sysctl.d/90-3tched.conf <<'EOF'
net.ipv4.ip_forward = 1
EOF
    sysctl -q -p /etc/sysctl.d/90-3tched.conf || true
    cat > /etc/modules-load.d/3tched.conf <<'EOF'
tun
EOF
    modprobe tun 2>/dev/null || true

    ok "Configuration files in place"
}

write_dbus_policy() {
    log "Installing D-Bus system policy + s6d activation..."
    install -d /etc/dbus-1/system.d
    cat > /etc/dbus-1/system.d/org.opdbus.conf <<EOF
<!DOCTYPE busconfig PUBLIC
 "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <policy user="root">
    <allow own_prefix="org.opdbus"/>
    <allow send_destination_prefix="org.opdbus"/>
  </policy>
  <policy user="${OP_USER}">
    <allow own_prefix="org.opdbus"/>
    <allow send_destination_prefix="org.opdbus"/>
  </policy>
  <policy context="default">
    <allow send_destination_prefix="org.opdbus"
           send_interface="org.freedesktop.DBus.Introspectable"/>
    <allow send_destination_prefix="org.opdbus"
           send_interface="org.freedesktop.DBus.Properties"/>
  </policy>
</busconfig>
EOF

    # D-Bus activation for the s6 service-management surface
    # (org.opdbus.v1.S6.Systemctl — crates/op-s6-systemctl).
    install -d /usr/share/dbus-1/system-services
    cat > /usr/share/dbus-1/system-services/org.opdbus.v1.S6.Systemctl.service <<EOF
[D-BUS Service]
Name=org.opdbus.v1.S6.Systemctl
Exec=${BIN_DIR}/op-s6-systemctl
User=root
EOF
    ok "D-Bus policy installed"
}

# ============================================================================
# PHASE 6 — S6 SERVICE GENERATION
# All longruns get a dedicated s6-log consumer:
#   <svc>/producer-for = <svc>-log
#   <svc>-log/consumer-for = <svc>, pipeline-name = <svc>-pipeline
# Enabling the pipeline bundle pulls both sides up together.
# ============================================================================

SVC_PIPELINES=()   # pipeline bundles to add to the 3tched bundle
SVC_ONESHOTS=()    # oneshots to add to the 3tched bundle

mk_logger() {
    local svc="$1"
    local dir="${S6_SV_DIR}/${svc}-log"
    install -d "$dir"
    echo longrun > "${dir}/type"
    echo "$svc" > "${dir}/consumer-for"
    echo "${svc}-pipeline" > "${dir}/pipeline-name"
    echo 3 > "${dir}/notification-fd"
    cat > "${dir}/run" <<EOF
#!/bin/execlineb -P
foreground { install -d -o s6log -g s6log ${LOG_ROOT}/${svc} }
s6-setuidgid s6log
exec -c s6-log -d3 -b -- ${LOG_DIRECTIVES} ${LOG_ROOT}/${svc}
EOF
    chmod 0755 "${dir}/run"
}

mk_longrun() {
    # mk_longrun <name> <deps-space-separated> <<'RUN' ... RUN
    local svc="$1" deps="$2"
    local dir="${S6_SV_DIR}/${svc}"
    install -d "$dir"
    echo longrun > "${dir}/type"
    echo "${svc}-log" > "${dir}/producer-for"
    cat > "${dir}/run"
    chmod 0755 "${dir}/run"
    if [[ -n $deps ]]; then
        install -d "${dir}/dependencies.d"
        local d
        for d in $deps; do
            # never reference a service that does not exist — s6-rc-compile
            # hard-fails on dangling dependencies
            if [[ -d ${S6_SV_DIR}/$d ]]; then
                touch "${dir}/dependencies.d/$d"
            else
                warn "$svc: skipping missing dependency $d"
            fi
        done
    fi
    mk_logger "$svc"
    SVC_PIPELINES+=("${svc}-pipeline")
}

mk_oneshot() {
    # mk_oneshot <name> <deps> <up-script-path> [down-script-path]
    local svc="$1" deps="$2" up="$3" down="${4:-}"
    local dir="${S6_SV_DIR}/${svc}"
    install -d "$dir"
    echo oneshot > "${dir}/type"
    echo "$up" > "${dir}/up"
    [[ -n $down ]] && echo "$down" > "${dir}/down"
    if [[ -n $deps ]]; then
        install -d "${dir}/dependencies.d"
        local d
        for d in $deps; do
            if [[ -d ${S6_SV_DIR}/$d ]]; then
                touch "${dir}/dependencies.d/$d"
            else
                warn "$svc: skipping missing dependency $d"
            fi
        done
    fi
    SVC_ONESHOTS+=("$svc")
}

install_bridge_up_cmd() {
    # 3tched-bridge-up — THE one network bring-up command.  Every network
    # oneshot is a thin call into one phase of this script, so the whole
    # sequence is reviewable (and manually runnable) as a single artifact.
    # Paths are the canonical constants (must match NET_CONF / BIN_DIR).
    log "Installing 3tched-bridge-up (single-command network bring-up)..."
    cat > "${BIN_DIR}/3tched-bridge-up" <<'BRIDGEUP'
#!/bin/sh
# 3tched-bridge-up — the ONE network bring-up command.
#
# Ordered contract (each phase is an idempotent s6 oneshot body):
#
#   dhcp           acquire an IPv4 lease on UPLINK while it is still a plain
#                  physical NIC, then snapshot addresses + default gateway
#                  to tmpfs.  Later phases only ever consume the snapshot —
#                  the kernel strips a NIC's IPs the instant OVS captures it.
#   seed           create BRIDGE and enslave UPLINK in ONE OVSDB transact
#                  (op-ovsbr0-setup --seed-only).  Called from the
#                  ovs-vswitchd run script BEFORE vswitchd starts, so
#                  vswitchd reads bridge + port together and uplink capture
#                  starts correctly.  Never split into create-then-add-port.
#   uplink         migrate the snapshot addresses and default route onto
#                  BRIDGE.  The host uplink is COMPLETE here, strictly
#                  before any container starts.
#   netmaker-wait  bounded wait for the netmaker container to be RUNNING
#                  and its internal service manager ready — probed over
#                  D-Bus with busctl, never systemctl.
#   service-addrs  apply BRIDGE_SVC_ADDRS (10.200.0.1 + 10.0.0.2) + netmaker
#                  route + NAT, strictly AFTER netmaker is enslaved so the
#                  host service addresses never race the container bring-up.
#   all            full sequence for manual/recovery use.  Boot never runs
#                  this: the s6 graph calls the individual phases
#                  (uplink-dhcp -> seed inside ovs-vswitchd run ->
#                  ovsbr0-uplink -> incus-ct-netmaker -> ovsbr0-svc-addr).
#                  Supervised pieces are started through `service6` only.
set -eu

NET_CONF=/etc/op-dbus/network.conf
SNAPSHOT=/run/opdbus/uplink-migration.env
OVSBR0_SETUP=/usr/local/bin/op-ovsbr0-setup

[ "$(id -u)" -eq 0 ] || { echo "3tched-bridge-up: run as root" >&2; exit 1; }
set -a; [ -r "$NET_CONF" ] && . "$NET_CONF"; set +a
BRIDGE="${BRIDGE:-ovsbr0}"
UPLINK="${UPLINK:-eth0}"
NETMAKER_CT="${NETMAKER_CT:-netmaker}"

phase_dhcp() {
    [ -n "$UPLINK" ] || return 0
    ip link show "$UPLINK" >/dev/null 2>&1 || { echo "uplink $UPLINK not present"; exit 1; }
    ip link set "$UPLINK" up
    # A restored static/retained lease is already valid input.  Otherwise
    # obtain one synchronously while the NIC is still a normal interface.
    if ! ip -4 -o addr show dev "$UPLINK" scope global | grep -q .; then
        command -v dhcpcd >/dev/null 2>&1 || { echo "dhcpcd is required for $UPLINK"; exit 1; }
        echo "acquiring IPv4 DHCP lease on $UPLINK before OVS attachment"
        # -1 -B: one synchronous boot transaction, no lingering supervisor
        dhcpcd -4 -1 -B -q "$UPLINK"
    fi
    addrs="$(ip -4 -o addr show dev "$UPLINK" scope global | awk '{print $4}' | tr '\n' ' ')"
    addrs="${addrs% }"
    [ -n "$addrs" ] || { echo "$UPLINK has no global IPv4 lease"; exit 1; }
    gw="$(ip -4 route show default dev "$UPLINK" | awk '{print $3; exit}')"
    tmp="${SNAPSHOT}.new"
    {
        printf 'UPLINK_ADDRS="%s"\n' "$addrs"
        printf 'UPLINK_GW="%s"\n' "$gw"
    } >"$tmp"
    chmod 0600 "$tmp"
    mv -f "$tmp" "$SNAPSHOT"
    echo "captured $addrs (gw ${gw:-none}) before OVS attachment"
}

phase_seed() {
    # bridge + UPLINK enslavement in ONE OVSDB transact — this IS the atomic
    # bridge-creation step (op-ovsbr0-setup refuses to do it as two steps).
    [ -x "$OVSBR0_SETUP" ] || { echo "$OVSBR0_SETUP missing"; exit 1; }
    export BRIDGE UPLINK FAIL_MODE SHARED_MAC OVSDB_SOCKET VSWITCHD_SVC
    exec "$OVSBR0_SETUP" --seed-only
}

phase_uplink() {
    export BRIDGE UPLINK FAIL_MODE SHARED_MAC OVSDB_SOCKET VSWITCHD_SVC
    if [ -x "$OVSBR0_SETUP" ]; then
        "$OVSBR0_SETUP" || echo "op-ovsbr0-setup failed (continuing)"
    fi
    i=0
    until ip link show "$BRIDGE" >/dev/null 2>&1; do
        i=$((i+1))
        [ "$i" -ge 30 ] && { echo "$BRIDGE did not appear"; exit 1; }
        sleep 1
    done
    ip link set "$BRIDGE" up
    # UPLINK is already an OVS port.  Consume the pre-enslavement snapshot;
    # never query the enslaved NIC.
    if [ -r "$SNAPSHOT" ]; then
        UPLINK_ADDRS=""
        UPLINK_GW=""
        . "$SNAPSHOT"
        if [ -n "$UPLINK_ADDRS" ]; then
            echo "migrating $UPLINK_ADDRS (gw ${UPLINK_GW:-none}) from $UPLINK to $BRIDGE"
            for a in $UPLINK_ADDRS; do
                ip addr replace "$a" dev "$BRIDGE"
            done
            [ -n "$UPLINK" ] && ip addr flush dev "$UPLINK" scope global
            [ -n "$UPLINK_GW" ] && ip route replace default via "$UPLINK_GW" dev "$BRIDGE"
        fi
    else
        echo "warning: no snapshot at $SNAPSHOT — uplink addresses not migrated"
    fi
    if [ -n "$UPLINK" ] && ip link show "$UPLINK" >/dev/null 2>&1; then
        ip link set "$UPLINK" up
    fi
    sysctl -qw net.ipv4.ip_forward=1 || true
    echo "$BRIDGE uplink complete — host network stable before containers"
}

phase_netmaker_wait() {
    command -v incus >/dev/null 2>&1 || return 0
    incus info >/dev/null 2>&1 || { echo "incusd not reachable — skipping netmaker wait"; return 0; }
    if ! incus list -f csv -c n 2>/dev/null | grep -qx "$NETMAKER_CT"; then
        echo "no $NETMAKER_CT container — skipping netmaker wait"
        return 0
    fi
    echo "waiting for $NETMAKER_CT (RUNNING + service manager ready over D-Bus)"
    i=0
    until incus list -f csv -c ns 2>/dev/null | grep -q "^${NETMAKER_CT},RUNNING"; do
        i=$((i+1))
        [ "$i" -ge 60 ] && { echo "$NETMAKER_CT not RUNNING after 60s — continuing without it"; return 0; }
        sleep 1
    done
    # container service-manager readiness over D-Bus — busctl, NEVER systemctl
    i=0
    state=""
    while :; do
        state="$(incus exec "$NETMAKER_CT" -- busctl --system get-property \
            org.freedesktop.systemd1 /org/freedesktop/systemd1 \
            org.freedesktop.systemd1.Manager SystemState 2>/dev/null |
            tr -d '"' | awk '{print $2}')" || true
        case "$state" in
            running|degraded) break ;;
        esac
        i=$((i+1))
        [ "$i" -ge 180 ] && { echo "$NETMAKER_CT service manager not ready after 180s (state: ${state:-none}) — continuing"; return 0; }
        sleep 1
    done
    echo "$NETMAKER_CT enslaved and ready (service manager: $state)"
}

phase_service_addrs() {
    # STRICTLY after netmaker: the host service addresses must never race
    # the container's own bring-up on the bridge.
    phase_netmaker_wait
    for a in ${BRIDGE_SVC_ADDRS:-10.200.0.1/24 10.0.0.2/24}; do
        ip addr replace "$a" dev "$BRIDGE"
    done
    # netmaker net rides the bridge; this route lets 10.0.0.2 egress reach
    # host xray for identity header injection
    ip route replace "${NETMAKER_NET:-10.0.0.0/24}" dev "$BRIDGE" 2>/dev/null || true
    sysctl -qw net.ipv4.ip_forward=1 || true
    if [ "${NAT_ENABLE:-1}" = "1" ] && command -v iptables >/dev/null 2>&1; then
        iptables -t nat -C POSTROUTING -s "${BRIDGE_NET:-10.200.0.0/24}" ! -d "${BRIDGE_NET:-10.200.0.0/24}" -j MASQUERADE 2>/dev/null ||
        iptables -t nat -A POSTROUTING -s "${BRIDGE_NET:-10.200.0.0/24}" ! -d "${BRIDGE_NET:-10.200.0.0/24}" -j MASQUERADE
    fi
    echo "$BRIDGE service addresses applied after netmaker: ${BRIDGE_SVC_ADDRS:-10.200.0.1/24 10.0.0.2/24}"
}

phase_all() {
    install -d -m 0755 /run/opdbus
    phase_dhcp
    if ! pgrep -x ovs-vswitchd >/dev/null 2>&1; then
        # supervised pieces go through the sanctioned wrapper only
        service6 start ovs-vswitchd-pipeline ||
            service6 start ovs-vswitchd ||
            { echo "service6 start ovs-vswitchd failed"; exit 1; }
    fi
    phase_uplink
    if [ -d "/etc/s6/sv/incus-ct-${NETMAKER_CT}" ]; then
        service6 start "incus-ct-${NETMAKER_CT}-pipeline" ||
            service6 start "incus-ct-${NETMAKER_CT}" || true
    fi
    phase_service_addrs
}

case "${1:-all}" in
    dhcp)          phase_dhcp ;;
    seed)          phase_seed ;;
    uplink)        phase_uplink ;;
    netmaker-wait) phase_netmaker_wait ;;
    service-addrs) phase_service_addrs ;;
    all)           phase_all ;;
    *)
        echo "usage: 3tched-bridge-up [dhcp|seed|uplink|netmaker-wait|service-addrs|all]" >&2
        exit 2
        ;;
esac
BRIDGEUP
    chmod 0755 "${BIN_DIR}/3tched-bridge-up"
    ok "3tched-bridge-up installed"
}

write_s6_services() {
    log "Generating s6-rc service definitions..."

    # ---- opdbus-rundirs (oneshot): runtime dirs + sealed-blob SHM catalog --
    cat > "${LIBEXEC_DIR}/opdbus-rundirs-up" <<EOF
#!/bin/sh
set -eu
install -d -m 0755 /run/opdbus
install -d -m 0755 /run/op-dbus
install -d -m 0755 ${BLOB_SHM_DIR}
install -d -m 0755 -o s6log -g s6log ${LOG_ROOT}
# Seal the plugin blobs BEFORE op-grpc-bridge starts: the bridge hydrates its
# gRPC reflection from this catalog and enters a ~40s retry loop against an
# empty tmpfs otherwise.  This is what lets the gRPC bridge come up early.
if [ -x ${BIN_DIR}/opblob ]; then
    ${BIN_DIR}/opblob seal-shm >/dev/null
else
    echo "warning: ${BIN_DIR}/opblob missing — blob catalog not sealed"
fi
cat > /dev/shm/xray_config.json <<'XRAY_CONFIG_EOF'
{
  "log": { "loglevel": "warning" },
  "inbounds": [
    {
      "tag": "egress-proxy",
      "listen": "127.0.0.1",
      "port": 10809,
      "protocol": "http",
      "settings": {}
    },
    {
      "tag": "grpc-uplink",
      "listen": "188.68.58.237",
      "port": 8443,
      "protocol": "dokodemo-door",
      "settings": {
        "address": "127.0.0.1",
        "port": 8443,
        "network": "tcp"
      }
    },
    {
      "tag": "netmaker-api-uplink",
      "listen": "188.68.58.237",
      "port": 8081,
      "protocol": "dokodemo-door",
      "settings": {
        "address": "127.0.0.1",
        "port": 8081,
        "network": "tcp"
      }
    }
  ],
  "outbounds": [
    { "tag": "direct", "protocol": "freedom", "settings": {} }
  ]
}
XRAY_CONFIG_EOF
chown root:xray /dev/shm/xray_config.json
chmod 0640 /dev/shm/xray_config.json
EOF
    chmod 0755 "${LIBEXEC_DIR}/opdbus-rundirs-up"
    mk_oneshot opdbus-rundirs "" "${LIBEXEC_DIR}/opdbus-rundirs-up"

    # ---- op-session-bus: busd on the canonical SESSION bus socket ----------
    # EARLY BY DESIGN: depends only on opdbus-rundirs, never on the network
    # chain — the whole D-Bus control plane and the gRPC bridge must be up
    # while OVS/netmaker are still converging.
    mk_longrun op-session-bus "opdbus-rundirs" <<EOF
#!/bin/sh
# 3tched SESSION bus — the WG-identity-gated plugin surface.
exec 2>&1
set -a; [ -r ${ENV_FILE} ] && . ${ENV_FILE}; set +a
: "\${DBUS_SESSION_BUS_ADDRESS:=${SESSION_BUS_ADDRESS}}"
SOCKET_PATH="\${DBUS_SESSION_BUS_ADDRESS#unix:path=}"
install -d -m 0755 "\$(dirname "\$SOCKET_PATH")"
rm -f "\$SOCKET_PATH"
# the bus runs as root; loosen the socket so the operator's tools (zbusctl)
# can reach it
umask 000
if command -v busd >/dev/null 2>&1; then
    exec busd -a "\$DBUS_SESSION_BUS_ADDRESS"
fi
exec dbus-daemon --session --nofork --nopidfile --address="\$DBUS_SESSION_BUS_ADDRESS"
EOF

    # ======================= NETWORK (all via s6) ==========================

    # ---- uplink-dhcp (oneshot): lease + immutable migration snapshot -------
    # This must complete before any OVS row can enslave UPLINK.  The snapshot
    # is the hand-off contract: later stages never try to rediscover an address
    # from a physical interface after it has become an OVS port.
    cat > "${LIBEXEC_DIR}/uplink-dhcp-up" <<EOF
#!/bin/sh
set -eu
set -a; [ -r ${NET_CONF} ] && . ${NET_CONF}; set +a
UPLINK="\${UPLINK:-eth0}"
SNAPSHOT=/run/opdbus/uplink-migration.env
[ -n "\$UPLINK" ] || exit 0
ip link show "\$UPLINK" >/dev/null 2>&1 || { echo "uplink \$UPLINK not present"; exit 1; }
ip link set "\$UPLINK" up

# A restored static/retained lease is already valid input.  Otherwise obtain
# one synchronously while the NIC is still a normal physical interface.
if ! ip -4 -o addr show dev "\$UPLINK" scope global | grep -q .; then
    command -v dhcpcd >/dev/null 2>&1 || { echo "dhcpcd is required for \$UPLINK"; exit 1; }
    echo "acquiring IPv4 DHCP lease on \$UPLINK before OVS attachment"
    # -B keeps this as a synchronous boot transaction rather than leaving a
    # second network supervisor behind after the address is handed to OVS.
    dhcpcd -4 -1 -B -q "\$UPLINK"
fi

UPLINK_ADDRS="\$(ip -4 -o addr show dev "\$UPLINK" scope global | awk '{print \$4}')"
[ -n "\$UPLINK_ADDRS" ] || { echo "\$UPLINK has no global IPv4 lease"; exit 1; }
UPLINK_GW="\$(ip -4 route show default dev "\$UPLINK" | awk '{print \$3; exit}')"
tmp="\${SNAPSHOT}.new"
{
    printf 'UPLINK_ADDRS="%s"\\n' "\$(printf '%s' "\$UPLINK_ADDRS" | tr '\\n' ' ')"
    printf 'UPLINK_GW="%s"\\n' "\$UPLINK_GW"
} >"\$tmp"
chmod 0600 "\$tmp"
mv -f "\$tmp" "\$SNAPSHOT"
echo "captured \$(printf '%s' "\$UPLINK_ADDRS" | tr '\\n' ' ') (gw \${UPLINK_GW:-none}) before OVS attachment"
EOF
    chmod 0755 "${LIBEXEC_DIR}/uplink-dhcp-up"
    mk_oneshot uplink-dhcp "opdbus-rundirs" "${LIBEXEC_DIR}/uplink-dhcp-up"

    # ---- ovsdb-server ------------------------------------------------------
    mk_longrun ovsdb-server "uplink-dhcp" <<'EOF'
#!/bin/sh
# OVSDB — the native JSON-RPC control surface for OVS (no ovs-vsctl anywhere).
exec 2>&1
DB=/etc/openvswitch/conf.db
SCHEMA=/usr/share/openvswitch/vswitch.ovsschema
install -d /etc/openvswitch /run/openvswitch
[ -f "$DB" ] || ovsdb-tool create "$DB" "$SCHEMA"
exec ovsdb-server "$DB" \
    --remote=punix:/run/openvswitch/db.sock \
    --remote=db:Open_vSwitch,Open_vSwitch,manager_options
EOF

    # ---- ovs-vswitchd ------------------------------------------------------
    # op-ovsbr0-setup --seed-only writes the system-datapath bridge/port rows
    # over OVSDB JSON-RPC *before* vswitchd starts (crates/op-network contract).
    mk_longrun ovs-vswitchd "ovsdb-server" <<EOF
#!/bin/sh
exec 2>&1
set -a; [ -r ${NET_CONF} ] && . ${NET_CONF}; set +a
export OVSDB_SOCKET="\${OVSDB_SOCKET:-/run/openvswitch/db.sock}"
if [ -x ${BIN_DIR}/op-ovsbr0-setup ]; then
    ${BIN_DIR}/op-ovsbr0-setup --seed-only || echo "op-ovsbr0-setup --seed-only failed (continuing)"
fi
exec ovs-vswitchd "unix:\${OVSDB_SOCKET}"
EOF

    # ---- ovsbr0-addr (oneshot): bridge exists + addressed + NAT ------------
    cat > "${LIBEXEC_DIR}/ovsbr0-addr-up" <<EOF
#!/bin/sh
set -eu
set -a; [ -r ${NET_CONF} ] && . ${NET_CONF}; set +a
BRIDGE="\${BRIDGE:-ovsbr0}"
export BRIDGE UPLINK FAIL_MODE SHARED_MAC OVSDB_SOCKET VSWITCHD_SVC
if [ -x ${BIN_DIR}/op-ovsbr0-setup ]; then
    ${BIN_DIR}/op-ovsbr0-setup || echo "op-ovsbr0-setup failed (continuing)"
fi
i=0
until ip link show "\$BRIDGE" >/dev/null 2>&1; do
    i=\$((i+1))
    [ "\$i" -ge 30 ] && { echo "\$BRIDGE did not appear"; exit 1; }
    sleep 1
done
ip addr replace "\${BRIDGE_ADDR:-10.200.0.1/24}" dev "\$BRIDGE"
ip link set "\$BRIDGE" up
# netmaker net rides the bridge (container iface holds 10.0.0.2 + 10.200.0.2);
# this route lets 10.0.0.2 egress reach host xray for identity header injection
ip route replace "\${NETMAKER_NET:-10.0.0.0/24}" dev "\$BRIDGE" 2>/dev/null || true
# UPLINK is already an OVS port.  Consume the snapshot made before ovsdb-server
# was allowed to start; never race by querying the enslaved NIC here.
SNAPSHOT=/run/opdbus/uplink-migration.env
if [ -r "\$SNAPSHOT" ]; then
    UPLINK_ADDRS=""
    UPLINK_GW=""
    . "\$SNAPSHOT"
    if [ -n "\$UPLINK_ADDRS" ]; then
        echo "migrating \$(echo "\$UPLINK_ADDRS" | tr '\\n' ' ') (gw \${UPLINK_GW:-none}) from \$UPLINK to \$BRIDGE"
        printf '%s\\n' \$UPLINK_ADDRS | while IFS= read -r a; do
            [ -n "\$a" ] && ip addr replace "\$a" dev "\$BRIDGE"
        done
        [ -n "\${UPLINK:-}" ] && ip addr flush dev "\$UPLINK" scope global
        [ -n "\$UPLINK_GW" ] && ip route replace default via "\$UPLINK_GW" dev "\$BRIDGE"
    fi
fi
if [ -n "\${UPLINK:-}" ] && ip link show "\$UPLINK" >/dev/null 2>&1; then
    ip link set "\$UPLINK" up
fi
sysctl -qw net.ipv4.ip_forward=1 || true
if [ "\${NAT_ENABLE:-1}" = "1" ] && command -v iptables >/dev/null 2>&1; then
    iptables -t nat -C POSTROUTING -s "\${BRIDGE_NET:-10.200.0.0/24}" ! -d "\${BRIDGE_NET:-10.200.0.0/24}" -j MASQUERADE 2>/dev/null || \
    iptables -t nat -A POSTROUTING -s "\${BRIDGE_NET:-10.200.0.0/24}" ! -d "\${BRIDGE_NET:-10.200.0.0/24}" -j MASQUERADE
fi
echo "\$BRIDGE ready at \${BRIDGE_ADDR:-10.200.0.1/24}"
EOF
    chmod 0755 "${LIBEXEC_DIR}/ovsbr0-addr-up"
    mk_oneshot ovsbr0-addr "ovs-vswitchd" "${LIBEXEC_DIR}/ovsbr0-addr-up"

    # ---- op-of-controller: OpenFlow 1.3 controller for ovsbr0 --------------
    mk_longrun op-of-controller "ovsbr0-addr" <<EOF
#!/bin/sh
exec 2>&1
set -a
[ -r ${ENV_FILE} ] && . ${ENV_FILE}
[ -r ${NET_CONF} ] && . ${NET_CONF}
set +a
export OF_CONTROLLER_LISTEN="\${OF_CONTROLLER_LISTEN:-10.200.0.1:6653}"
export RUST_LOG="\${RUST_LOG:-info}"
exec ${BIN_DIR}/op-of-controller
EOF

    # ---- incusd: container runtime (all container I/O is UDS) --------------
    if have incusd; then
        mk_longrun incusd "ovsbr0-addr" <<'EOF'
#!/bin/sh
exec 2>&1
exec incusd --group incus-admin
EOF
    fi

    # ======================= OP-DBUS CONTROL PLANE =========================

    # ---- opdbus: gRPC state manager (binds ovsbr0 IPv4 :50051) -------------
    mk_longrun opdbus "op-session-bus ovsbr0-addr" <<EOF
#!/bin/sh
exec 2>&1
set -a; [ -r ${ENV_FILE} ] && . ${ENV_FILE}; set +a
export RUST_LOG="\${RUST_LOG:-info}"
exec ${BIN_DIR}/opdbus
EOF

    # ---- op-projection: SchemaEngine / plugin projection -------------------
    mk_longrun op-projection "op-session-bus" <<EOF
#!/bin/sh
exec 2>&1
set -a; [ -r ${ENV_FILE} ] && . ${ENV_FILE}; set +a
: "\${DBUS_SESSION_BUS_ADDRESS:?must be set in ${ENV_FILE}}"
export RUST_LOG="\${RUST_LOG:-op_projection=info,info}"
exec ${BIN_DIR}/projection_server
EOF

    # ---- op-web: HTTP API + embedded UI ------------------------------------
    mk_longrun op-web "op-projection opdbus" <<EOF
#!/bin/sh
exec 2>&1
set -a; [ -r ${ENV_FILE} ] && . ${ENV_FILE}; set +a
: "\${DBUS_SESSION_BUS_ADDRESS:?must be set in ${ENV_FILE}}"
export PORT="\${PORT:-8080}"
export RUST_LOG="\${RUST_LOG:-op_web=info,info}"
exec ${BIN_DIR}/op-web-server
EOF

    # ---- op-grpc-bridge: zero-trust gRPC bridge (xray redirects here) ------
    # Keep the SHM blob seal explicit even though op-session-bus also depends on
    # opdbus-rundirs.  The bridge must never enter its retry loop against an
    # empty tmpfs catalog after reboot or a regenerated service graph.
    mk_longrun op-grpc-bridge "opdbus-rundirs op-session-bus" <<EOF
#!/bin/sh
exec 2>&1
set -a; [ -r ${ENV_FILE} ] && . ${ENV_FILE}; set +a
: "\${DBUS_SESSION_BUS_ADDRESS:?must be set in ${ENV_FILE}}"
export GRPC_BIND="\${GRPC_BIND:-127.0.0.1:8090}"
export ZEROCLAW_TLS_BIND_ADDR="\${ZEROCLAW_TLS_BIND_ADDR:-127.0.0.1:8443}"
export RUST_LOG="\${GRPC_RUST_LOG:-off}"
exec ${BIN_DIR}/op-grpc-bridge
EOF

    # ---- op-cognitive-mcp: universal external MCP gateway ------------------
    mk_longrun op-cognitive-mcp "op-session-bus" <<EOF
#!/bin/sh
exec 2>&1
set -a; [ -r ${ENV_FILE} ] && . ${ENV_FILE}; set +a
export COGNITIVE_MCP_DB_PATH="\${COGNITIVE_MCP_DB_PATH:-${STATE_DIR}/cognitive.db}"
export RUST_LOG="\${RUST_LOG:-info}"
exec ${BIN_DIR}/op-cognitive-mcp --db "\$COGNITIVE_MCP_DB_PATH"
EOF

    # ---- op-mcp-compact: loopback-only MCP for the chatbot -----------------
    mk_longrun op-mcp-compact "op-session-bus" <<EOF
#!/bin/sh
exec 2>&1
set -a; [ -r ${ENV_FILE} ] && . ${ENV_FILE}; set +a
export RUST_LOG="\${RUST_LOG:-info}"
exec ${BIN_DIR}/op-mcp-compact
EOF

    # ---- op-dbus-mirror: OVSDB/ObjectManager mirror surfaces ---------------
    if [[ -x ${BIN_DIR}/op-dbus-mirror ]]; then
        mk_longrun op-dbus-mirror "op-session-bus" <<EOF
#!/bin/sh
exec 2>&1
set -a; [ -r ${ENV_FILE} ] && . ${ENV_FILE}; set +a
export RUST_LOG="\${RUST_LOG:-info}"
exec ${BIN_DIR}/op-dbus-mirror
EOF
    fi

    # ---- op-xray-daemon: xray router (identity header injection) -----------
    # xray runs on the HOST (the old wg-xray container path is deprecated).
    # Config arrives at /dev/shm/xray_config.json via the D-Bus surface.
    mk_longrun op-xray-daemon "op-session-bus" <<EOF
#!/bin/sh
exec 2>&1
set -a; [ -r ${ENV_FILE} ] && . ${ENV_FILE}; set +a
export RUST_LOG="\${RUST_LOG:-info}"
exec ${BIN_DIR}/op-xray-daemon
EOF

    # ---- xray: live config is SHM-only; static bootstrap is materialized by
    # opdbus-rundirs until the validated model routing generator replaces it.
    install -d "${S6_SV_DIR}/xray/dependencies.d"
    echo longrun > "${S6_SV_DIR}/xray/type"
    touch "${S6_SV_DIR}/xray/dependencies.d/opdbus-rundirs"
    cat > "${S6_SV_DIR}/xray/run" <<'EOF'
#!/bin/sh
exec 2>&1
: "${XRAY_CONFIG_PATH:=/dev/shm/xray_config.json}"
[ -r "$XRAY_CONFIG_PATH" ] || { echo "missing SHM Xray config: $XRAY_CONFIG_PATH"; exit 1; }
exec /usr/bin/xray run -config "$XRAY_CONFIG_PATH"
EOF
    chmod 0755 "${S6_SV_DIR}/xray/run"

    ok "s6 service definitions generated"
}

write_desktop_services() {
    [[ $SKIP_DESKTOP -eq 1 ]] && return 0
    log "Configuring greetd -> Hyprland (Wayland) under s6..."

    # greetd config: tuigreet launching Hyprland
    install -d /etc/greetd
    if [[ -f /etc/greetd/config.toml ]] && ! grep -q 3tched /etc/greetd/config.toml; then
        cp /etc/greetd/config.toml /etc/greetd/config.toml.bak
    fi
    if [[ ! -f /etc/greetd/config.toml ]] || ! grep -q 3tched /etc/greetd/config.toml; then
        cat > /etc/greetd/config.toml <<EOF
# 3tched greetd configuration — tuigreet -> Hyprland
[terminal]
vt = 2

[default_session]
command = "tuigreet --time --remember --cmd Hyprland"
user = "greeter"
EOF
    fi
    getent group greeter >/dev/null 2>&1 || groupadd -r greeter 2>/dev/null || true
    id greeter >/dev/null 2>&1 || useradd -r -g greeter -s /usr/bin/nologin greeter 2>/dev/null || true

    # greetd longrun (with log pipeline).  Depends on the system dbus and the
    # seat manager when their s6 services exist (package-provided).
    local greetd_deps=""
    [[ -d ${S6_SV_DIR}/dbus ]] && greetd_deps="dbus"
    [[ -d ${S6_SV_DIR}/elogind ]] && greetd_deps="$greetd_deps elogind"
    [[ -d ${S6_SV_DIR}/seatd ]] && greetd_deps="$greetd_deps seatd"
    mk_longrun greetd "$greetd_deps" <<'EOF'
#!/bin/sh
exec 2>&1
exec greetd --config /etc/greetd/config.toml
EOF

    # Minimal Hyprland config for the operator (only when absent)
    local hypr_dir="${OP_HOME}/.config/hypr"
    if [[ ! -f ${hypr_dir}/hyprland.conf ]]; then
        install -d -o "$OP_USER" -g "$OP_USER" "${OP_HOME}/.config" "$hypr_dir"
        cat > "${hypr_dir}/hyprland.conf" <<'EOF'
# 3tched default Hyprland configuration
monitor = ,preferred,auto,1

$mod = SUPER
$terminal = foot
$menu = wofi --show drun

exec-once = waybar
exec-once = mako
exec-once = hyprpaper

env = XCURSOR_SIZE,24
env = QT_QPA_PLATFORM,wayland

input {
    kb_layout = us
    follow_mouse = 1
}

general {
    gaps_in = 4
    gaps_out = 8
    border_size = 2
    layout = dwindle
}

decoration {
    rounding = 6
}

animations {
    enabled = true
}

bind = $mod, RETURN, exec, $terminal
bind = $mod, D, exec, $menu
bind = $mod, Q, killactive,
bind = $mod SHIFT, E, exit,
bind = $mod, F, fullscreen,
bind = $mod, V, togglefloating,
bind = $mod, S, exec, grim -g "$(slurp)" - | wl-copy

bind = $mod, left, movefocus, l
bind = $mod, right, movefocus, r
bind = $mod, up, movefocus, u
bind = $mod, down, movefocus, d

bind = $mod, 1, workspace, 1
bind = $mod, 2, workspace, 2
bind = $mod, 3, workspace, 3
bind = $mod, 4, workspace, 4
bind = $mod, 5, workspace, 5
bind = $mod SHIFT, 1, movetoworkspace, 1
bind = $mod SHIFT, 2, movetoworkspace, 2
bind = $mod SHIFT, 3, movetoworkspace, 3
bind = $mod SHIFT, 4, movetoworkspace, 4
bind = $mod SHIFT, 5, movetoworkspace, 5

bindm = $mod, mouse:272, movewindow
bindm = $mod, mouse:273, resizewindow
EOF
        chown "$OP_USER:$OP_USER" "${hypr_dir}/hyprland.conf"
    fi
    ok "Desktop services configured"
}

write_headless_gui_services() {
    [[ $WITH_HEADLESS_GUI -eq 1 ]] || return 0
    log "Configuring headless Wayland sled (weston + wayvnc)..."

    # Headless weston compositor owning WAYLAND_DISPLAY=3tched-wayland;
    # never exports DISPLAY.  wayvnc exposes it loopback-only.
    mk_longrun weston-headless "opdbus-rundirs" <<EOF
#!/bin/sh
exec 2>&1
OP_GUI_USER="${OP_USER}"
uid="\$(id -u "\$OP_GUI_USER")"
runtime_dir="/run/user/\${uid}"
state_dir="/run/op-dbus/weston-headless"
install -d -m 0700 -o "\$uid" -g "\$(id -g "\$OP_GUI_USER")" "\$runtime_dir"
install -d -m 0750 -o "\$uid" -g "\$(id -g "\$OP_GUI_USER")" "\$state_dir"
rm -f "\${runtime_dir}/3tched-wayland" "\${runtime_dir}/3tched-wayland.lock"
exec s6-setuidgid "\$OP_GUI_USER" env \
    HOME="/home/\${OP_GUI_USER}" \
    XDG_RUNTIME_DIR="\$runtime_dir" \
    WAYLAND_DISPLAY=3tched-wayland \
    weston --backend=headless --socket=3tched-wayland --idle-time=0
EOF

    mk_longrun wayvnc "weston-headless" <<EOF
#!/bin/sh
exec 2>&1
OP_GUI_USER="${OP_USER}"
uid="\$(id -u "\$OP_GUI_USER")"
runtime_dir="/run/user/\${uid}"
i=0
until [ -S "\${runtime_dir}/3tched-wayland" ]; do
    i=\$((i+1)); [ "\$i" -ge 30 ] && { echo "wayland socket missing"; exit 1; }
    sleep 1
done
exec s6-setuidgid "\$OP_GUI_USER" env \
    XDG_RUNTIME_DIR="\$runtime_dir" \
    WAYLAND_DISPLAY=3tched-wayland \
    wayvnc 127.0.0.1 5901
EOF
    ok "Headless GUI services configured"
}

write_incus_svcgen() {
    # Containers come up under s6 supervision with log pipelines — NEVER via
    # Incus autostart.  This helper generates an incus-ct-<name> longrun (+
    # s6-log consumer) for a container; the installer runs it for every
    # existing container and the operator runs it for future ones.
    log "Installing 3tched-incus-svcgen (s6-supervised Incus containers)..."
    cat > "${BIN_DIR}/3tched-incus-svcgen" <<'SVCGEN'
#!/bin/sh
# 3tched-incus-svcgen — generate an s6-supervised service (with an s6-log
# pipeline) for an Incus container.  Containers come up with s6, not Incus
# autostart, and their console streams into /var/log/op-dbus/.
#
# Usage: 3tched-incus-svcgen <container> [--attach-bridge <bridge>] [--no-start]
#
# --attach-bridge: join the container to the OVS bridge LAST — after its
#   internal service manager reports readiness over D-Bus (used for netmaker: the container is
#   self-contained; its bridge interface carries 10.0.0.2 + 10.200.0.2 and
#   its 10.0.0.2 egress forwards to host xray for identity header injection).

set -eu

SV_DIR=/etc/s6/sv
LOG_ROOT=/var/log/op-dbus
LOG_DIRECTIVES="n10 s4000000 T"

name=""
bridge=""
start=1
while [ $# -gt 0 ]; do
    case "$1" in
        --attach-bridge) bridge="$2"; shift 2 ;;
        --no-start)      start=0; shift ;;
        -*) echo "unknown option: $1" >&2; exit 2 ;;
        *)  name="$1"; shift ;;
    esac
done
[ -n "$name" ] || {
    echo "usage: 3tched-incus-svcgen <container> [--attach-bridge <bridge>] [--no-start]" >&2
    exit 2
}

svc="incus-ct-${name}"
dir="${SV_DIR}/${svc}"

install -d "$dir" "${dir}/dependencies.d"
echo longrun > "${dir}/type"
echo "${svc}-log" > "${dir}/producer-for"
[ -d "${SV_DIR}/incusd" ] && touch "${dir}/dependencies.d/incusd"
if [ -n "$bridge" ] && [ -d "${SV_DIR}/op-of-controller" ]; then
    # bridge-attached containers come up after the network is fully up
    touch "${dir}/dependencies.d/op-of-controller"
fi

cat > "${dir}/run" <<EOF
#!/bin/sh
# s6-supervised Incus container '${name}' — s6 owns the lifecycle, not Incus.
exec 2>&1
NAME='${name}'
ATTACH_BRIDGE='${bridge}'
i=0
until incus info >/dev/null 2>&1; do
    i=\$((i+1)); [ "\$i" -ge 60 ] && { echo "incusd not ready"; exit 1; }
    sleep 1
done
incus config set "\$NAME" boot.autostart=false 2>/dev/null || true
if ! incus list -f csv -c ns | grep -q "^\${NAME},RUNNING"; then
    echo "starting container \$NAME"
    incus start "\$NAME"
fi
if [ -n "\$ATTACH_BRIDGE" ]; then
    incus config device show "\$NAME" 2>/dev/null | grep -q '^eth0:' || {
        echo "\$NAME has no configured eth0; refusing to create a NIC implicitly"
        exit 1
    }
    echo "\$NAME started last with its existing eth0 on \$ATTACH_BRIDGE"
fi
# hold supervision and stream the container console into the log pipeline;
# stopping this service stops the container ('script' provides the pty
# incus console needs).
script -qefc "incus console \$NAME" /dev/null &
pid=\$!
trap 'incus stop "\$NAME" --timeout 30 2>/dev/null || true; kill "\$pid" 2>/dev/null || true' TERM INT
wait "\$pid"
EOF

ldir="${SV_DIR}/${svc}-log"
install -d "$ldir"
echo longrun > "${ldir}/type"
echo "$svc" > "${ldir}/consumer-for"
echo "${svc}-pipeline" > "${ldir}/pipeline-name"
echo 3 > "${ldir}/notification-fd"
cat > "${ldir}/run" <<EOF
#!/bin/execlineb -P
foreground { install -d -o s6log -g s6log ${LOG_ROOT}/${svc} }
s6-setuidgid s6log
exec -c s6-log -d3 -b -- ${LOG_DIRECTIVES} ${LOG_ROOT}/${svc}
EOF
chmod 0755 "${dir}/run" "${ldir}/run"

bdir="${SV_DIR}/3tched"
install -d "${bdir}/contents.d"
[ -f "${bdir}/type" ] || echo bundle > "${bdir}/type"
touch "${bdir}/contents.d/${svc}-pipeline"

if command -v s6-db-reload >/dev/null 2>&1; then
    s6-db-reload
else
    echo "warning: s6-db-reload not found — recompile the s6-rc database manually" >&2
fi
if [ "$start" = 1 ]; then
    s6-rc -u change "${svc}-pipeline" || true
fi
echo "generated ${svc} (+ log pipeline); logs: ${LOG_ROOT}/${svc}/current"
SVCGEN
    chmod 0755 "${BIN_DIR}/3tched-incus-svcgen"
    ok "3tched-incus-svcgen installed"
}

generate_container_services() {
    # s6-supervise every container that already exists (netmaker gets the
    # settle-then-bridge-attach treatment).  New containers: run
    # 3tched-incus-svcgen <name> after creating them.
    have incus || return 0
    if ! incus info >/dev/null 2>&1; then
        warn "incusd not reachable — run 3tched-incus-svcgen <name> per container later"
        return 0
    fi
    local bridge cts ct
    bridge="$(set -a; . "$NET_CONF" 2>/dev/null; set +a; echo "${BRIDGE:-ovsbr0}")"
    cts="$(incus list -f csv -c n 2>/dev/null || true)"
    if [[ -z $cts ]]; then
        log "no Incus containers yet — use 3tched-incus-svcgen <name> after creating them"
        return 0
    fi
    for ct in $cts; do
        case "$ct" in
            netmaker*) "${BIN_DIR}/3tched-incus-svcgen" "$ct" --attach-bridge "$bridge" ;;
            *)         "${BIN_DIR}/3tched-incus-svcgen" "$ct" ;;
        esac
    done
    ok "container services generated"
}

# ============================================================================
# PHASE 7 — ENABLE + COMPILE + START
# ============================================================================

add_to_bundle() {
    # add_to_bundle <bundle> <entry>
    local bundle="$1" entry="$2"
    local dir="${S6_SV_DIR}/${bundle}"
    install -d "$dir" "${dir}/contents.d"
    echo bundle > "${dir}/type"
    touch "${dir}/contents.d/${entry}"
}

enable_default() {
    # add an entry to the boot-time default bundle
    local svc="$1"
    if have s6-service; then
        s6-service add default "$svc" 2>/dev/null && return 0
    fi
    if [[ -d ${S6_SV_DIR}/default/contents.d ]]; then
        touch "${S6_SV_DIR}/default/contents.d/${svc}"
    elif [[ -f ${S6_SV_DIR}/default/contents ]]; then
        grep -qx "$svc" "${S6_SV_DIR}/default/contents" ||
            echo "$svc" >> "${S6_SV_DIR}/default/contents"
    else
        warn "no default bundle found — enable $svc manually"
    fi
}

enable_services() {
    log "Assembling the 3tched bundle and enabling services..."

    local entry
    for entry in "${SVC_ONESHOTS[@]}"; do
        add_to_bundle 3tched "$entry"
    done
    for entry in "${SVC_PIPELINES[@]}"; do
        add_to_bundle 3tched "$entry"
    done

    enable_default 3tched
    # System bus + seat manager come from the Artix -s6 packages
    [[ -d ${S6_SV_DIR}/dbus ]] && enable_default dbus
    [[ -d ${S6_SV_DIR}/elogind ]] && enable_default elogind
    [[ -d ${S6_SV_DIR}/seatd && ! -d ${S6_SV_DIR}/elogind ]] && enable_default seatd

    log "Recompiling the s6-rc database..."
    if have s6-db-reload; then
        s6-db-reload || warn "s6-db-reload failed — fix the service definitions and re-run it"
    else
        warn "s6-db-reload not found — recompile the s6-rc database manually or reboot"
    fi
}

start_services() {
    [[ $SKIP_START -eq 1 ]] && { warn "--skip-start: services enabled but not started"; return 0; }
    log "Starting the 3tched bundle..."
    s6-rc -u change 3tched || warn "s6-rc -u change 3tched reported errors (see logs below)"
    if [[ $SKIP_DESKTOP -eq 0 ]]; then
        [[ -d ${S6_SV_DIR}/dbus ]] && s6-rc -u change dbus 2>/dev/null || true
        [[ -d ${S6_SV_DIR}/elogind ]] && s6-rc -u change elogind 2>/dev/null || true
        s6-rc -u change greetd-pipeline 2>/dev/null || true
    fi
}

verify() {
    log "Verifying installation..."
    local bad=0

    for b in op-web-server opdbus projection_server op-grpc-bridge s6d; do
        if [[ -x ${BIN_DIR}/$b ]]; then ok "binary: $b"; else warn "missing binary: $b"; bad=1; fi
    done

    [[ $SKIP_START -eq 1 ]] && return 0
    sleep 3

    if [[ -S $SESSION_BUS_SOCKET ]]; then
        ok "SESSION bus socket: $SESSION_BUS_SOCKET"
    else
        warn "SESSION bus socket missing"; bad=1
    fi
    if [[ -S /run/openvswitch/db.sock ]]; then
        ok "OVSDB socket: /run/openvswitch/db.sock"
    else
        warn "OVSDB socket missing"; bad=1
    fi
    if ip link show ovsbr0 >/dev/null 2>&1; then
        ok "ovsbr0 bridge present"
    else
        warn "ovsbr0 not present yet"
    fi
    if curl -fsS -o /dev/null "http://127.0.0.1:8080/" 2>/dev/null; then
        ok "op-web responding on :8080"
    else
        warn "op-web not responding on :8080 (may still be starting)"
    fi
    s6-rc -a list 2>/dev/null | sed 's/^/    /' || true
    [[ $bad -eq 0 ]] && ok "verification passed" || warn "verification finished with warnings"
}

summary() {
    echo
    echo "============================================================================"
    echo -e "${GREEN}3TCHED / OP-DBUS — ARTIX S6 INSTALL COMPLETE${NC}"
    echo "============================================================================"
    echo
    echo "Service management (all through the D-Bus control plane):"
    echo "  s6d status <svc> | s6d start <svc> | s6d stop <svc>"
    echo "  s6d journalctl <svc>          # reads ${LOG_ROOT}/<svc>/current"
    echo "  s6-rc -a list                 # everything currently up"
    echo
    echo "s6 layout:"
    echo "  Source definitions:  ${S6_SV_DIR}/<svc>  (+ <svc>-log consumer)"
    echo "  Bundle:              3tched  (in the default boot bundle)"
    echo "  Longrun logs:        ${LOG_ROOT}/<svc>/current  (s6-log: ${LOG_DIRECTIVES})"
    echo "  Oneshot output:      s6-rc catch-all logger (/run/uncaught-logs)"
    echo
    echo "Network (s6-managed):"
    echo "  uplink-dhcp -> ovsdb-server -> ovs-vswitchd (system, rovs-seeded) -> ovsbr0-addr"
    echo "  ovsbr0: 10.200.0.1/24 · OpenFlow controller: 10.200.0.1:6653"
    echo "  Uplink (UPLINK in network.conf) enslaved atomically with bridge"
    echo "  creation — ONE OVSDB transact — and its IP migrated to the bridge"
    echo "  No WireGuard and no AF_XDP on this host — the netmaker mesh is"
    echo "  self-contained in the netmaker container (iface: 10.0.0.2 +"
    echo "  10.200.0.2, WG terminates at the decoy server; 10.0.0.2 egress ->"
    echo "  host xray -> gRPC with identity header)"
    echo
    echo "Containers (s6-supervised with log pipelines — never Incus autostart):"
    echo "  3tched-incus-svcgen <name>                        # any container"
    echo "  3tched-incus-svcgen netmaker --attach-bridge ovsbr0"
    echo "  (netmaker joins the bridge LAST, after its internal systemd settles)"
    echo
    echo "Next steps:"
    echo "  1. Containers: incus admin init, create containers, then run"
    echo "     3tched-incus-svcgen per container (sockets via zbusctl createsocket)"
    if [[ $SKIP_DESKTOP -eq 0 ]]; then
        echo "  2. Desktop: greetd/tuigreet on VT2 -> Hyprland (Super+Enter = foot)"
    fi
    if [[ ${#MISSING_PKGS[@]} -gt 0 ]]; then
        echo
        warn "packages that could not be installed: ${MISSING_PKGS[*]}"
        warn "install them manually (check the Artix galaxy/world repos or enable [extra])"
    fi
    echo
    echo "  UI:        http://127.0.0.1:8080/ (op-web, embedded UI)"
    echo "  MCP:       op-cognitive-mcp (external gateway) · op-mcp-compact (loopback)"
    echo "  Blob SHM:  ${BLOB_SHM_DIR}"
    echo "============================================================================"
}

# ============================================================================
# MAIN
# ============================================================================

main() {
    echo
    echo "============================================================================"
    echo "3TCHED / OP-DBUS — ARTIX LINUX S6 INSTALLER"
    echo "target: artix-base-s6-20260402-x86_64.iso (s6 + s6-rc)"
    echo "============================================================================"
    echo

    preflight
    install_packages
    setup_users
    setup_rust
    locate_repo
    build_project
    write_environment
    write_dbus_policy
    write_s6_services
    write_desktop_services
    write_headless_gui_services
    write_incus_svcgen
    enable_services
    start_services
    generate_container_services
    verify
    summary
}

main "$@"
