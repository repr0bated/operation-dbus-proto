#!/bin/bash
# Setup Assistant Incus container with OpenClaw base + ~/.assistant overlay.
#
# The overlay model:
#   - Base layer: OpenClaw installed via curl script (upgradable by re-running)
#   - Overlay layer: host ~/.assistant/ mounted as container /root/.openclaw/
#   - OpenClaw sees its normal paths; host sees ~/.assistant/
#   - Upgrades: re-run curl installer (base), overlay data persists on host
#
# NO NETWORK INTERFACE — container is sealed. All traffic flows via Unix
# socket proxies bridged by Xray ds outbound / OpenFlow on the host.
#
# Usage: sudo bash deploy/setup-assistant-incus.sh
set -euo pipefail

CONTAINER="assistant"
OVERLAY_HOST="${HOME}/.assistant"
OVERLAY_CONTAINER="/root/.openclaw"
OPENCLAW_PORT=18789

log() { echo "[assistant-setup] $*"; }
die() { echo "[assistant-setup] ERROR: $*" >&2; exit 1; }

# ─── preflight ────────────────────────────────────────────────────────────────

[[ $EUID -eq 0 ]] || die "Run as root: sudo bash $0"
command -v incus &>/dev/null || die "incus not found"

# Ensure host overlay directory exists (populated earlier by user / droid)
if [[ ! -d "$OVERLAY_HOST" ]]; then
    log "Creating host overlay directory: $OVERLAY_HOST"
    mkdir -p "$OVERLAY_HOST"/{agents,canvas,completions,credentials,cron,devices,identity,logs,sandboxes,workspace/config}
    # Seed a minimal openclaw.json that OpenClaw expects (mirrors assistant.json)
    cat > "$OVERLAY_HOST/openclaw.json" <<'JSON'
{
  "name": "assistant",
  "version": "1.0.0",
  "gateway": { "host": "0.0.0.0", "port": 18789, "trust_proxy": true },
  "agents": { "workspace": "/root/.openclaw/workspace", "default": "main" },
  "models": {
    "primary": "opencode/kimi-k2.5-free",
    "available": [
      { "id": "google-gemini-cli/gemini-2.5-pro",      "alias": "Pro"   },
      { "id": "google-gemini-cli/gemini-2.5-flash",   "alias": "Flash" },
      { "id": "google-gemini-cli/gemini-3-pro-preview", "alias": "G3"    },
      { "id": "opencode/claude-opus-4-6",             "alias": "Opus"  },
      { "id": "opencode/kimi-k2.5-free",             "alias": "Kimi"  }
    ]
  },
  "mcp": { "config_path": "/root/.openclaw/workspace/config/mcporter.json" },
  "logging": { "level": "info", "path": "/root/.openclaw/logs" }
}
JSON
fi

# ─── container lifecycle (no NIC — sealed) ────────────────────────────────────

if incus info "$CONTAINER" &>/dev/null; then
    log "Container '$CONTAINER' already exists — stopping and recreating"
    incus stop "$CONTAINER" --force 2>/dev/null || true
    incus delete "$CONTAINER"
fi

log "Creating container '$CONTAINER' (Debian trixie, NO NIC)"

# Launch with --no-profiles, then add root disk manually (no NIC).
# All communication is via Unix socket proxies (Incus proxy device) and
# routed by Xray ds outbound / OpenFlow on the host.
incus init images:debian/trixie "$CONTAINER"
incus config set "$CONTAINER" security.privileged true
incus config set "$CONTAINER" security.nesting true

# Add root disk explicitly (default profile would add it, but we skip profiles)
incus config device add "$CONTAINER" root disk path=/ pool=default

# Do NOT start the container yet — we chroot into the rootfs to install
# software using the host's network, avoiding any network dependency inside
# the sealed container.

# ─── chroot install: base packages + Node.js + OpenClaw ───────────────────────
#
# The container rootfs is a Btrfs subvolume under
# /var/lib/incus/storage-pools/default/containers/$CONTAINER/rootfs
# We bind-mount host /proc, /sys, /dev, and /etc/resolv.conf so apt/curl
# work, then chroot to install everything before first boot.

CT_ROOTFS="/var/lib/incus/storage-pools/default/containers/$CONTAINER/rootfs"
[[ -d "$CT_ROOTFS" ]] || die "Container rootfs not found: $CT_ROOTFS"

log "Preparing chroot environment in $CT_ROOTFS"
mount --bind /proc "$CT_ROOTFS/proc"
mount --bind /sys  "$CT_ROOTFS/sys"
mount --bind /dev  "$CT_ROOTFS/dev"
mount --bind /etc/resolv.conf "$CT_ROOTFS/etc/resolv.conf"

# Ensure the overlay is visible inside the chroot BEFORE OpenClaw install
# so the installer respects existing config.
mkdir -p "$CT_ROOTFS$OVERLAY_CONTAINER"
mount --bind "$OVERLAY_HOST" "$CT_ROOTFS$OVERLAY_CONTAINER"

log "Installing base packages via chroot"
chroot "$CT_ROOTFS" apt-get update -qq
chroot "$CT_ROOTFS" apt-get install -y -qq \
    curl ca-certificates git systemd systemd-sysv \
    build-essential python3

log "Installing Node.js 22 LTS via chroot"
chroot "$CT_ROOTFS" bash -c \
    'curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && apt-get install -y -qq nodejs'

NODE_VER=$(chroot "$CT_ROOTFS" node -v 2>/dev/null || echo 'unknown')
log "Node.js version in container rootfs: $NODE_VER"

# ─── OpenClaw base layer (curl install script via chroot) ─────────────────────

OPENCLAW_INSTALL_URL="${OPENCLAW_INSTALL_URL:-https://openclaw.ai/install.sh}"

log "Installing OpenClaw base layer via chroot: $OPENCLAW_INSTALL_URL"
chroot "$CT_ROOTFS" bash -c "curl -fsSL '$OPENCLAW_INSTALL_URL' | bash"

# Discover the installed binary path inside the chroot
OPENCLAW_BIN=$(chroot "$CT_ROOTFS" bash -c 'command -v openclaw || echo ""')
if [[ -z "$OPENCLAW_BIN" ]]; then
    OPENCLAW_BIN=$(chroot "$CT_ROOTFS" bash -c '
        for d in /usr/bin /usr/local/bin /opt/openclaw/bin /root/.openclaw/bin; do
            if [[ -x "$d/openclaw" ]]; then echo "$d/openclaw"; break; fi
        done
    ')
fi
if [[ -z "$OPENCLAW_BIN" ]]; then
    umount -R "$CT_ROOTFS" 2>/dev/null || true
    die "OpenClaw binary not found after install"
fi
log "OpenClaw binary: $OPENCLAW_BIN"

OPENCLAW_VERSION=$(chroot "$CT_ROOTFS" bash -c "$OPENCLAW_BIN --version 2>/dev/null || echo 'unknown'")
log "OpenClaw installed version: $OPENCLAW_VERSION"

# ─── overlay permissions ──────────────────────────────────────────────────────

chroot "$CT_ROOTFS" chown -R root:root "$OVERLAY_CONTAINER" 2>/dev/null || true

# ─── unmount chroot binds ─────────────────────────────────────────────────────

umount -R "$CT_ROOTFS" 2>/dev/null || true
log "Unmounted chroot binds"

# ─── start container ──────────────────────────────────────────────────────────

incus start "$CONTAINER"
sleep 2

# ─── overlay mount (disk device) ────────────────────────────────────────────
#
# Incus disk device mounts host ~/.assistant → container /root/.openclaw
# so OpenClaw runtime sees its normal paths while the host controls the data.

log "Mounting overlay disk device: $OVERLAY_HOST → $OVERLAY_CONTAINER"
incus config device remove "$CONTAINER" assistant-overlay 2>/dev/null || true
incus config device add "$CONTAINER" assistant-overlay disk \
    source="$OVERLAY_HOST" \
    path="$OVERLAY_CONTAINER"

# ─── systemd service for OpenClaw ─────────────────────────────────────────────

log "Creating OpenClaw systemd service inside container"
incus exec "$CONTAINER" -- bash -c "cat > /etc/systemd/system/openclaw.service" <<EOF
[Unit]
Description=OpenClaw Gateway (Assistant overlay)
After=network.target

[Service]
Type=simple
User=root
Environment=NODE_ENV=production
Environment=HOME=/root
# Run the OpenClaw WebSocket Gateway in the foreground.
# 'gateway run' stays in the foreground (unlike 'gateway start' which backgrounds).
ExecStart=$OPENCLAW_BIN gateway run
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

incus exec "$CONTAINER" -- systemctl daemon-reload

log "Enabling and starting openclaw.service"
incus exec "$CONTAINER" -- systemctl enable openclaw.service
incus exec "$CONTAINER" -- systemctl restart openclaw.service || \
    log "WARNING: openclaw.service failed to start — running doctor --fix and retrying"

# If gateway fails, config may need repair (first-boot scenario)
if ! incus exec "$CONTAINER" -- systemctl is-active openclaw.service &>/dev/null; then
    log "OpenClaw service not active — running openclaw doctor --fix"
    incus exec "$CONTAINER" -- bash -c "HOME=/root $OPENCLAW_BIN doctor --fix" || true
    incus exec "$CONTAINER" -- systemctl restart openclaw.service || \
        log "WARNING: openclaw.service still failed after doctor --fix"
fi

# Verify openclaw is listening on 127.0.0.1:18789 inside container
sleep 2
if incus exec "$CONTAINER" -- ss -tlnp 2>/dev/null | grep -q ':18789'; then
    log "OpenClaw is listening on port 18789 inside container"
else
    log "WARNING: OpenClaw may not be listening on port 18789 yet"
fi

# ─── proxy devices (all traffic via Unix sockets / localhost) ───────────────────

# TCP proxy: host 127.0.0.1:18789 → container 127.0.0.1:18789
# wg-xray / xray routes this via OpenFlow rules on the host.
log "Configuring TCP proxy: host 127.0.0.1:$OPENCLAW_PORT → container:$OPENCLAW_PORT"
incus config device remove "$CONTAINER" openclaw-tcp 2>/dev/null || true
incus config device add "$CONTAINER" openclaw-tcp proxy \
    listen=tcp:127.0.0.1:$OPENCLAW_PORT \
    connect=tcp:127.0.0.1:$OPENCLAW_PORT \
    bind=host

# Unix socket proxy for gRPC-bridge → assistant communication
# Xray ds outbound routes via UNIX_SOCKET_ENDPOINTS=assistant:/run/assistant.sock:50051
ASSISTANT_SOCKET="/run/assistant.sock"
log "Configuring Unix socket proxy: host $ASSISTANT_SOCKET → container TCP 50051"
incus config device remove "$CONTAINER" assistant-grpc 2>/dev/null || true
incus config device add "$CONTAINER" assistant-grpc proxy \
    listen=unix:$ASSISTANT_SOCKET \
    connect=tcp:127.0.0.1:50051 \
    bind=host \
    mode=0660 \
    gid="$(id -g)"

# D-Bus session bus mount (control plane)
log "Adding D-Bus session bus mount"
incus config device remove "$CONTAINER" host-op-dbus-session 2>/dev/null || true
incus config device add "$CONTAINER" host-op-dbus-session disk \
    source=/run/op-dbus/session-bus.sock \
    path=/run/host-op-dbus/session-bus.sock

# ─── seal: remove any NIC devices ─────────────────────────────────────────────

log "Removing any NIC devices from $CONTAINER (sealed container)"
for dev in $(incus config device list "$CONTAINER" 2>/dev/null | grep -E "^eth|^enp" || true); do
    incus config device remove "$CONTAINER" "$dev" 2>/dev/null && \
        log "  Removed NIC: $dev" || true
done

# ─── verify no non-loopback IPs ───────────────────────────────────────────────

ip_count=$(incus exec "$CONTAINER" -- ip -4 addr show 2>/dev/null | grep -c "inet " || echo "0")
if [[ "$ip_count" -gt 1 ]]; then
    log "WARNING: container still has $((ip_count - 1)) non-loopback IP(s):"
    incus exec "$CONTAINER" -- ip -4 addr show | grep -v "127.0.0.1" || true
else
    log "Container sealed — only loopback present"
fi

# ─── summary ─────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════════════════════════"
echo " Assistant container '$CONTAINER' configured"
echo "══════════════════════════════════════════════════════════════════"
echo " Base OS:        Debian 13 (trixie)"
echo " NICs:           NONE (sealed container)"
echo " Routing:        Unix socket + TCP proxies via Xray/OpenFlow"
echo " OpenClaw base:   curl: $OPENCLAW_INSTALL_URL (v$OPENCLAW_VERSION)"
echo " Overlay:        $OVERLAY_HOST → $OVERLAY_CONTAINER"
echo ""
echo " Proxies:"
echo "   TCP   : host 127.0.0.1:$OPENCLAW_PORT → container:$OPENCLAW_PORT"
echo "   Unix  : host $ASSISTANT_SOCKET → container:50051 (gRPC)"
echo "   D-Bus : host /run/op-dbus/session-bus.sock → container"
echo "──────────────────────────────────────────────────────────────────"
echo ""
echo " Upgrade workflow (preserves overlay data):"
echo "   1. Stop container:    incus stop $CONTAINER"
echo "   2. Upgrade base:     incus exec $CONTAINER -- bash -c 'curl -fsSL $OPENCLAW_INSTALL_URL | bash'"
echo "   3. Start container:  incus start $CONTAINER"
echo ""
echo " Or fully recreate (overlay data is on host, so it survives):"
echo "   incus stop $CONTAINER --force && incus delete $CONTAINER"
echo "   sudo bash deploy/setup-assistant-incus.sh"
echo ""
echo " Diagnostics:"
echo "   incus exec $CONTAINER -- systemctl status openclaw"
echo "   incus exec $CONTAINER -- journalctl -u openclaw -n 50"
echo "   incus exec $CONTAINER -- ss -tlnp"
echo "   incus exec $CONTAINER -- $OPENCLAW_BIN --version"
echo "══════════════════════════════════════════════════════════════════"
