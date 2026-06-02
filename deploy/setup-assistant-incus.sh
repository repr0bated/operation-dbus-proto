#!/bin/bash
# Setup Assistant Incus container.
#
# Overlay model (OverlayFS inside the container):
#   lower  = base gateway install (curl installer, read-only, upgradeable)
#   upper  = host ~/.assistant/  (your layer — masks files, never erases them)
#   merged = /opt/assistant/     (what the service runs from)
#
# Upgrades: re-run the base installer against the lower dir. Upper layer
# persists on the host; OverlayFS re-merges automatically on next start.
# Files you've masked in upper stay masked; unmasked files pick up the upgrade.
#
# domain: assistant.3tched.com
# port:   18789 (internal)
#
# NO NETWORK INTERFACE — sealed container. Traffic via Unix socket proxies
# (Incus proxy devices) bridged by Xray ds outbound / OpenFlow on the host.
#
# Usage: sudo bash deploy/setup-assistant-incus.sh
set -euo pipefail

CONTAINER="assistant"
DOMAIN="assistant.3tched.com"
GATEWAY_PORT=18789

# Host paths
OVERLAY_HOST="${HOME}/.assistant"              # user customisation layer
LOWER_HOST="${HOME}/.assistant-base"           # base install — ro btrfs subvolume
CONFIG_LOWER_HOST="${HOME}/.assistant-config-base"  # base default config — ro btrfs subvolume
CONFIG_UPPER_HOST="${HOME}/.assistant/config-overlay"  # user config overrides (rw)
CONFIG_WORK_HOST="${HOME}/.assistant/config-work"      # overlayfs workdir for config

# Container paths
LOWER_CT="/opt/assistant-base"             # base install inside container
UPPER_CT="/opt/assistant-upper"            # bind-mounted from OVERLAY_HOST/overlay/
WORK_CT="/opt/assistant-work"              # bind-mounted from OVERLAY_HOST/work/
MERGED_CT="/opt/assistant"                 # OverlayFS merged view (service runs here)
CONFIG_LOWER_CT="/opt/assistant-config-base"   # base default config (ro)
CONFIG_UPPER_CT="/opt/assistant-config-upper"  # user config overrides
CONFIG_WORK_CT="/opt/assistant-config-work"    # overlayfs workdir
CONFIG_CT="/root/.assistant"               # merged config — what the service reads

log() { echo "[assistant-setup] $*"; }
die() { echo "[assistant-setup] ERROR: $*" >&2; exit 1; }

# ─── preflight ────────────────────────────────────────────────────────────────

[[ $EUID -eq 0 ]] || die "Run as root: sudo bash $0"
command -v incus &>/dev/null || die "incus not found"

# ─── host overlay directory structure ─────────────────────────────────────────

log "Ensuring host overlay directories exist"
mkdir -p \
    "$OVERLAY_HOST"/{agents,canvas,completions,credentials,cron,devices,identity,logs,sandboxes} \
    "$OVERLAY_HOST"/workspace/config \
    "$OVERLAY_HOST"/overlay \
    "$OVERLAY_HOST"/work \
    "$CONFIG_UPPER_HOST" \
    "$CONFIG_WORK_HOST"
mkdir -p "$LOWER_HOST"
mkdir -p "$CONFIG_LOWER_HOST"

# Lock down credential and identity dirs — only owner can read
chmod 700 \
    "$OVERLAY_HOST/credentials" \
    "$OVERLAY_HOST/identity" \
    "$CONFIG_UPPER_HOST"

# Seed assistant config if not present (gateway reads this)
if [[ ! -f "$OVERLAY_HOST/assistant.json" ]]; then
    cat > "$OVERLAY_HOST/assistant.json" <<'JSON'
{
  "name": "assistant",
  "version": "1.0.0",
  "gateway": { "host": "0.0.0.0", "port": 18789, "trust_proxy": true },
  "agents": { "workspace": "/root/.assistant/workspace", "default": "main" },
  "models": {
    "primary": "opencode/kimi-k2.5-free",
    "available": [
      { "id": "google-gemini-cli/gemini-2.5-pro",       "alias": "Pro"   },
      { "id": "google-gemini-cli/gemini-2.5-flash",    "alias": "Flash" },
      { "id": "google-gemini-cli/gemini-3-pro-preview", "alias": "G3"    },
      { "id": "opencode/claude-opus-4-6",              "alias": "Opus"  },
      { "id": "opencode/kimi-k2.5-free",              "alias": "Kimi"  }
    ]
  },
  "mcp": { "config_path": "/root/.assistant/workspace/config/mcporter.json" },
  "logging": { "level": "info", "path": "/root/.assistant/logs" }
}
JSON
fi

# ─── container lifecycle ───────────────────────────────────────────────────────

if incus info "$CONTAINER" &>/dev/null; then
    log "Container '$CONTAINER' exists — stopping and recreating"
    incus stop "$CONTAINER" --force 2>/dev/null || true
    incus delete "$CONTAINER"
fi

log "Creating container '$CONTAINER' (Debian trixie, NO NIC)"
incus init images:debian/trixie "$CONTAINER"
incus config set "$CONTAINER" security.privileged true
incus config set "$CONTAINER" security.nesting true
incus config device add "$CONTAINER" root disk path=/ pool=default

# ─── chroot install ────────────────────────────────────────────────────────────
#
# Use host network via chroot so the sealed container never needs a NIC.

CT_ROOTFS="/var/lib/incus/storage-pools/default/containers/$CONTAINER/rootfs"
[[ -d "$CT_ROOTFS" ]] || die "Container rootfs not found: $CT_ROOTFS"

log "Mounting chroot binds"
mount --bind /proc           "$CT_ROOTFS/proc"
mount --bind /sys            "$CT_ROOTFS/sys"
mount --bind /dev            "$CT_ROOTFS/dev"
mount --bind /etc/resolv.conf "$CT_ROOTFS/etc/resolv.conf"

# Create overlay dirs inside rootfs before installing
mkdir -p \
    "$CT_ROOTFS$LOWER_CT" \
    "$CT_ROOTFS$UPPER_CT" \
    "$CT_ROOTFS$WORK_CT" \
    "$CT_ROOTFS$MERGED_CT" \
    "$CT_ROOTFS$CONFIG_LOWER_CT" \
    "$CT_ROOTFS$CONFIG_UPPER_CT" \
    "$CT_ROOTFS$CONFIG_WORK_CT" \
    "$CT_ROOTFS$CONFIG_CT"

# Bind overlay dirs so installer can see existing config
mount --bind "$OVERLAY_HOST/overlay" "$CT_ROOTFS$UPPER_CT"
mount --bind "$OVERLAY_HOST"         "$CT_ROOTFS$CONFIG_CT"

log "Installing base packages"
chroot "$CT_ROOTFS" apt-get update -qq
chroot "$CT_ROOTFS" apt-get install -y -qq \
    curl ca-certificates git systemd systemd-sysv \
    build-essential python3 nginx

log "Installing Node.js 22 LTS"
chroot "$CT_ROOTFS" bash -c \
    'curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && apt-get install -y -qq nodejs'

log "Cloning OpenClaw source"
_GIT_DIR="$CT_ROOTFS/opt/openclaw-src"
_GIT_REF="${GATEWAY_GIT_REF:-main}"
if [[ -d "$_GIT_DIR/.git" ]]; then
    git -C "$_GIT_DIR" pull --ff-only
else
    git clone --depth=1 --branch "$_GIT_REF" \
        https://github.com/openclaw/openclaw.git "$_GIT_DIR"
fi

log "Rebinding install paths and binary invocations to assistant"
ASSISTANT_BRAND_NAME=assistant "$(dirname "$0")/bin/openclaw-rebind" "$_GIT_DIR"

log "Installing gateway base layer into $LOWER_CT"
_INSTALL_URL="${GATEWAY_INSTALL_URL:-https://openclaw.ai/install.sh}"
chroot "$CT_ROOTFS" bash -c \
    "OPENCLAW_HOME='$LOWER_CT' OPENCLAW_SOURCE_DIR='/opt/openclaw-src' \
     curl -fsSL '$_INSTALL_URL' | bash -s -- --install-method git --dir /opt/openclaw-src --no-onboard"

# Discover binary inside the chroot lower dir
_BIN=$(chroot "$CT_ROOTFS" bash -c "
    for d in '$LOWER_CT/bin' /usr/bin /usr/local/bin /opt/openclaw/bin; do
        [[ -x \"\$d/openclaw\" ]] && echo \"\$d/openclaw\" && break
    done
")
[[ -n "$_BIN" ]] || { umount -R "$CT_ROOTFS" 2>/dev/null || true; die "Gateway binary not found after install"; }

_VER=$(chroot "$CT_ROOTFS" bash -c "$_BIN --version 2>/dev/null || echo unknown")
log "Gateway version: $_VER  (binary: $_BIN)"

# Copy installed files into lower dir if installer placed them elsewhere
_INSTALL_DIR=$(dirname "$(dirname "$_BIN")")   # e.g. /usr/local
if [[ "$_INSTALL_DIR" != "$LOWER_CT" ]]; then
    log "Mirroring install from $_INSTALL_DIR into $LOWER_CT (lower layer)"
    cp -a "$CT_ROOTFS$_INSTALL_DIR/." "$CT_ROOTFS$LOWER_CT/"
fi

log "Installing log-filter into base layer"
mkdir -p "$CT_ROOTFS$LOWER_CT/bin"
cp "$(dirname "$0")/bin/log-filter" "$CT_ROOTFS$LOWER_CT/bin/log-filter"
chmod +x "$CT_ROOTFS$LOWER_CT/bin/log-filter"

umount "$CT_ROOTFS$UPPER_CT"  2>/dev/null || true
umount "$CT_ROOTFS$CONFIG_CT" 2>/dev/null || true
umount -R "$CT_ROOTFS"        2>/dev/null || true
log "Chroot binds unmounted"

# ─── immutable base layer (Fedora-style) ──────────────────────────────────────
#
# Mirror the populated lower dir to the host as a btrfs read-only subvolume,
# then bind-mount it into the container readonly=true. The container's internal
# copy becomes unreachable under the bind. Upgrades: delete subvolume, re-run.

log "Sealing base layer as immutable host subvolume"

# If previous run left a writable subvolume, make it writable to delete/replace
if btrfs subvolume show "$LOWER_HOST" &>/dev/null; then
    btrfs property set "$LOWER_HOST" ro false 2>/dev/null || true
    btrfs subvolume delete "$LOWER_HOST"
fi

# Create fresh subvolume and populate from container rootfs
btrfs subvolume create "$LOWER_HOST"
rsync -a --delete "$CT_ROOTFS$LOWER_CT/" "$LOWER_HOST/"

# Seal read-only — nothing inside the container can write to it
btrfs property set "$LOWER_HOST" ro true
log "Base layer sealed: $LOWER_HOST (ro)"

# ─── start container ──────────────────────────────────────────────────────────

incus start "$CONTAINER"
sleep 2

# ─── Incus disk devices (bind mounts) ─────────────────────────────────────────

log "Attaching overlay disk devices"

# Config dir: host ~/.assistant → container /root/.assistant
incus config device remove "$CONTAINER" assistant-config 2>/dev/null || true
incus config device add "$CONTAINER" assistant-config disk \
    source="$OVERLAY_HOST" \
    path="$CONFIG_CT"

# Base layer: host ~/.assistant-base (ro btrfs subvolume) → container /opt/assistant-base
# readonly=true enforced at the Incus/kernel level — container cannot write through it
incus config device remove "$CONTAINER" assistant-base 2>/dev/null || true
incus config device add "$CONTAINER" assistant-base disk \
    source="$LOWER_HOST" \
    path="$LOWER_CT" \
    readonly=true

# Upper layer: host ~/.assistant/overlay → container /opt/assistant-upper
incus config device remove "$CONTAINER" assistant-upper 2>/dev/null || true
incus config device add "$CONTAINER" assistant-upper disk \
    source="$OVERLAY_HOST/overlay" \
    path="$UPPER_CT"

# Work dir: host ~/.assistant/work → container /opt/assistant-work
incus config device remove "$CONTAINER" assistant-work 2>/dev/null || true
incus config device add "$CONTAINER" assistant-work disk \
    source="$OVERLAY_HOST/work" \
    path="$WORK_CT"

# ─── OverlayFS mount unit ─────────────────────────────────────────────────────
#
# Merges: lower (base install, read-only) + upper (your layer) → merged (service)
# Files in upper shadow lower. Unmasked files come from lower (picks up upgrades).

log "Installing base-layer read-only bind remount unit"
incus exec "$CONTAINER" -- bash -c "cat > /etc/systemd/system/opt-assistant-base.mount" <<'MOUNT'
[Unit]
Description=Assistant base layer (immutable)
DefaultDependencies=no
After=local-fs-pre.target

[Mount]
What=/opt/assistant-base
Where=/opt/assistant-base
Type=none
Options=bind,ro

[Install]
WantedBy=local-fs.target
MOUNT

incus exec "$CONTAINER" -- systemctl daemon-reload
incus exec "$CONTAINER" -- systemctl enable opt-assistant-base.mount

log "Installing OverlayFS mount unit inside container"
incus exec "$CONTAINER" -- bash -c "cat > /etc/systemd/system/opt-assistant.mount" <<'MOUNT'
[Unit]
Description=Assistant overlay filesystem
After=opt-assistant-base.mount
Requires=opt-assistant-base.mount
Before=assistant.service

[Mount]
What=overlay
Where=/opt/assistant
Type=overlay
Options=lowerdir=/opt/assistant-base,upperdir=/opt/assistant-upper,workdir=/opt/assistant-work

[Install]
WantedBy=multi-user.target
MOUNT

incus exec "$CONTAINER" -- systemctl daemon-reload
incus exec "$CONTAINER" -- systemctl enable opt-assistant.mount
incus exec "$CONTAINER" -- systemctl start  opt-assistant.mount || \
    log "WARNING: overlayfs mount failed — service will fall back to lower dir"

# ─── nav overlay CSS (injected by nginx sub_filter) ───────────────────────────
#
# Drop a CSS file into the upper layer to mask/extend openclaw's nav.
# Edit ~/.assistant/overlay/<path-to-nav-asset> to shadow individual JS files.
# This CSS layer is always applied via nginx regardless of what openclaw ships.

OVERLAY_CSS="$OVERLAY_HOST/overlay/assistant-nav.css"
if [[ ! -f "$OVERLAY_CSS" ]]; then
    cat > "$OVERLAY_CSS" <<'CSS'
/* Assistant nav overlay — runs on top of base nav. Add rules here to
   mask base nav items or inject assistant-specific navigation.
   This file is in the upper layer and survives gateway upgrades. */

/* Example: hide a base nav item by its data attribute or class */
/* [data-nav="settings"] { display: none; } */

/* Example: assistant branding in nav */
/* .nav-logo-text::after { content: " · assistant"; font-size: 0.75em; opacity: 0.6; } */
CSS
fi

# ─── nginx config (inside container) ──────────────────────────────────────────
#
# Proxies assistant.3tched.com → gateway port.
# sub_filter injects the overlay CSS into every HTML response.

log "Writing nginx config for $DOMAIN"
incus exec "$CONTAINER" -- bash -c "cat > /etc/nginx/sites-available/assistant" <<NGINX
server {
    listen 80;
    server_name $DOMAIN;

    location /assistant-nav.css {
        alias /opt/assistant-upper/assistant-nav.css;
        add_header Cache-Control "no-cache";
    }

    location / {
        proxy_pass         http://127.0.0.1:$GATEWAY_PORT;
        proxy_http_version 1.1;
        proxy_set_header   Host \$host;
        proxy_set_header   X-Real-IP \$remote_addr;
        proxy_set_header   Upgrade \$http_upgrade;
        proxy_set_header   Connection "upgrade";

        # Inject nav overlay CSS into every HTML page
        sub_filter '</head>'
            '<link rel="stylesheet" href="/assistant-nav.css"></head>';
        sub_filter_once on;
    }
}
NGINX

incus exec "$CONTAINER" -- ln -sf \
    /etc/nginx/sites-available/assistant \
    /etc/nginx/sites-enabled/assistant
incus exec "$CONTAINER" -- rm -f /etc/nginx/sites-enabled/default
incus exec "$CONTAINER" -- nginx -t
incus exec "$CONTAINER" -- systemctl enable nginx
incus exec "$CONTAINER" -- systemctl restart nginx

# ─── assistant.service ────────────────────────────────────────────────────────

log "Installing assistant.service inside container"
incus exec "$CONTAINER" -- bash -c "cat > /etc/systemd/system/assistant.service" <<EOF
[Unit]
Description=Assistant Gateway
After=network.target opt-assistant.mount

[Service]
Type=simple
User=root
Environment=NODE_ENV=production
Environment=HOME=/root
Environment=OPENCLAW_DEV_MODE=\${OPENCLAW_DEV_MODE:-0}
ExecStart=$_BIN gateway run
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

incus exec "$CONTAINER" -- systemctl daemon-reload
incus exec "$CONTAINER" -- systemctl enable assistant.service
incus exec "$CONTAINER" -- systemctl restart assistant.service || \
    log "WARNING: service failed — running doctor and retrying"

if ! incus exec "$CONTAINER" -- systemctl is-active assistant.service &>/dev/null; then
    incus exec "$CONTAINER" -- bash -c "HOME=/root $_BIN doctor --fix" || true
    incus exec "$CONTAINER" -- systemctl restart assistant.service || \
        log "WARNING: service still failed after doctor --fix"
fi

sleep 2
if incus exec "$CONTAINER" -- ss -tlnp 2>/dev/null | grep -q ":$GATEWAY_PORT"; then
    log "Gateway listening on port $GATEWAY_PORT"
else
    log "WARNING: gateway may not be listening on port $GATEWAY_PORT yet"
fi

# ─── proxy devices ────────────────────────────────────────────────────────────

log "Configuring host proxy devices"

# TCP: host 127.0.0.1:18789 → container:18789
incus config device remove "$CONTAINER" assistant-tcp 2>/dev/null || true
incus config device add "$CONTAINER" assistant-tcp proxy \
    listen=tcp:127.0.0.1:$GATEWAY_PORT \
    connect=tcp:127.0.0.1:$GATEWAY_PORT \
    bind=host

# Unix socket: host /run/assistant.sock → container:50051 (gRPC)
incus config device remove "$CONTAINER" assistant-grpc 2>/dev/null || true
incus config device add "$CONTAINER" assistant-grpc proxy \
    listen=unix:/run/assistant.sock \
    connect=tcp:127.0.0.1:50051 \
    bind=host \
    mode=0660 \
    gid="$(id -g)"

# D-Bus session bus
incus config device remove "$CONTAINER" assistant-dbus 2>/dev/null || true
incus config device add "$CONTAINER" assistant-dbus disk \
    source=/run/op-dbus/session-bus.sock \
    path=/run/host-op-dbus/session-bus.sock

# ─── seal NICs ────────────────────────────────────────────────────────────────

log "Sealing container (removing NICs)"
for dev in $(incus config device list "$CONTAINER" 2>/dev/null | grep -E "^eth|^enp" || true); do
    incus config device remove "$CONTAINER" "$dev" 2>/dev/null && log "  Removed NIC: $dev" || true
done

# ─── summary ─────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════════════════════════"
echo " $CONTAINER  ·  $DOMAIN"
echo "══════════════════════════════════════════════════════════════════"
echo " Base OS:   Debian 13 (trixie)   NICs: NONE (sealed)"
echo " Gateway:   v$_VER"
echo ""
echo " Overlay layers:"
echo "   lower  $LOWER_CT        (base install — read-only)"
echo "   upper  $OVERLAY_HOST/overlay  (your layer — host)"
echo "   merged $MERGED_CT       (what the service sees)"
echo ""
echo " Config:    $OVERLAY_HOST  →  $CONFIG_CT"
echo " Nav CSS:   $OVERLAY_HOST/overlay/assistant-nav.css"
echo ""
echo " Proxies:"
echo "   TCP   : host 127.0.0.1:$GATEWAY_PORT → container:$GATEWAY_PORT"
echo "   Unix  : host /run/assistant.sock → container:50051 (gRPC)"
echo "   D-Bus : host /run/op-dbus/session-bus.sock → container"
echo "──────────────────────────────────────────────────────────────────"
echo " Upgrade base (overlay data survives):"
echo "   incus exec $CONTAINER -- bash -c \\"
echo "     'OPENCLAW_HOME=$LOWER_CT curl -fsSL \$GATEWAY_INSTALL_URL | bash'"
echo "   incus exec $CONTAINER -- systemctl restart opt-assistant.mount assistant"
echo ""
echo " Full recreate (host overlay data always survives):"
echo "   incus stop $CONTAINER --force && incus delete $CONTAINER"
echo "   sudo bash deploy/setup-assistant-incus.sh"
echo ""
echo " Diagnostics:"
echo "   incus exec $CONTAINER -- systemctl status assistant"
echo "   incus exec $CONTAINER -- journalctl -u assistant -n 50"
echo "   incus exec $CONTAINER -- ls $MERGED_CT"
echo "══════════════════════════════════════════════════════════════════"
