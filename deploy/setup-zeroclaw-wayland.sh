#!/bin/sh
# ⚖️ 🟢 Zeroclaw headless Wayland setup — installs isolated GUI services.
#
# Requirements:
# - D-Bus service control goes through s6d / org.opdbus.v1.S6.Systemctl.
# - DISPLAY is never exported; CRD's X11 session is not modified.
# - zeroclaw-wgui receives only WAYLAND_DISPLAY=zeroclaw-wayland.
#
# Design:
# - zeroclaw-wayland owns the headless Weston compositor socket.
# - zeroclaw-wgui waits for that socket and starts under the same runtime user.
# - zeroclaw-wayvnc exposes a loopback-only VNC endpoint for that Wayland socket.

set -eu

repo_dir="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
s6_src="${repo_dir}/deploy/s6"
config_src="${repo_dir}/deploy/config/zeroclaw-wayland.env.example"

log() { printf '%s\n' "[zeroclaw-wayland] $*"; }
die() { printf '%s\n' "[zeroclaw-wayland] ERROR: $*" >&2; exit 1; }
need_root() { [ "$(id -u)" -eq 0 ] || die "run as root so service definitions can be installed"; }
need_cmd() { command -v "$1" >/dev/null 2>&1 || die "$1 is required"; }

install_service() {
  svc="$1"
  install -d "/etc/s6/sv/${svc}"
  cp -a "${s6_src}/${svc}/." "/etc/s6/sv/${svc}/"
  chmod 0755 "/etc/s6/sv/${svc}/run" 2>/dev/null || true
  chmod 0755 "/etc/s6/sv/${svc}/up" 2>/dev/null || true
  chmod 0755 "/etc/s6/sv/${svc}/shell_up" 2>/dev/null || true
  log "installed ${svc}"
}

ensure_s6d() {
  need_cmd cargo

  build_user="${SUDO_USER:-${USER:-root}}"
  log "building D-Bus s6 wrapper"
  if [ "$(id -u)" -eq 0 ] && [ "$build_user" != "root" ]; then
    sudo -u "$build_user" cargo build --release -p op-s6-systemctl --bin s6d --bin op-s6-systemctl
  else
    cargo build --release -p op-s6-systemctl --bin s6d --bin op-s6-systemctl
  fi
  install -m 0755 "${repo_dir}/target/release/s6d" /usr/local/bin/s6d
  install -m 0755 "${repo_dir}/target/release/op-s6-systemctl" /usr/local/bin/op-s6-systemctl

  install -d /usr/share/dbus-1/system-services
  cat > /usr/share/dbus-1/system-services/org.opdbus.v1.S6.Systemctl.service <<'DBUS_SERVICE'
[D-BUS Service]
Name=org.opdbus.v1.S6.Systemctl
Exec=/usr/local/bin/op-s6-systemctl
User=root
DBUS_SERVICE

  dbus-send --system --type=method_call \
    --dest=org.freedesktop.DBus /org/freedesktop/DBus \
    org.freedesktop.DBus.ReloadConfig 2>/dev/null || true

  # Confirm the D-Bus activation path before service changes are attempted.
  /usr/local/bin/s6d daemon-status >/dev/null
}

s6d_call() {
  USER="${USER:-root}" BUILD_USER="${SUDO_USER:-${USER:-root}}" /usr/local/bin/s6d "$@"
}

need_root
need_cmd weston
need_cmd wayvnc
need_cmd s6-setuidgid
ensure_s6d

install -d -m 0750 /etc/ghostbridge
if [ ! -f /etc/ghostbridge/zeroclaw-wayland.env ]; then
  install -m 0640 "$config_src" /etc/ghostbridge/zeroclaw-wayland.env
  log "installed /etc/ghostbridge/zeroclaw-wayland.env"
else
  log "kept existing /etc/ghostbridge/zeroclaw-wayland.env"
fi

install_service zeroclaw-wayland
install_service zeroclaw-wayland-log
install_service zeroclaw-wgui
install_service zeroclaw-wgui-log
install_service zeroclaw-wayvnc
install_service zeroclaw-wayvnc-log

s6d_call daemon-reload
s6d_call enable zeroclaw-wayland
s6d_call enable zeroclaw-wgui
s6d_call enable zeroclaw-wayvnc
s6d_call start zeroclaw-wayland
s6d_call start zeroclaw-wgui
s6d_call start zeroclaw-wayvnc

log "status: zeroclaw-wayland"
s6d_call status zeroclaw-wayland || true
log "status: zeroclaw-wgui"
s6d_call status zeroclaw-wgui || true
log "status: zeroclaw-wayvnc"
s6d_call status zeroclaw-wayvnc || true

log "Wayland socket: /run/user/\$(id -u \$(. /etc/ghostbridge/zeroclaw-wayland.env; printf %s \"\$OP_ZEROCLAW_USER\"))/zeroclaw-wayland"
log "VNC endpoint: \$(. /etc/ghostbridge/zeroclaw-wayland.env; printf '%s:%s' \"\$ZEROCLAW_WAYVNC_HOST\" \"\$ZEROCLAW_WAYVNC_PORT\")"

# Logs flow through the s6 -log pipelines into /var/log/op-dbus/<service>/current,
# which the s6d D-Bus wrapper (org.opdbus.v1.S6.Systemctl.journalctl) reads back.
# Always read logs through s6d so access stays on the D-Bus control plane.
log "logs (D-Bus): s6d journalctl zeroclaw-wayland"
log "logs (D-Bus): s6d journalctl zeroclaw-wgui"
log "logs (D-Bus): s6d journalctl zeroclaw-wayvnc"
log "compositor detail: /run/op-dbus/zeroclaw-wayland/weston.log"
