#!/usr/bin/env bash
# setup-assistant-gui-devices.sh
# Configure Incus devices for X11 / Wayland socket pass-through into the assistant container.
# Guidelines: socket networking only — no NICs are added.

set -euo pipefail

CONTAINER="assistant"

if [[ $EUID -ne 0 ]]; then
  echo "This script must be run as root (sudo)." >&2
  exit 1
fi

echo "==> Adding X11 socket passthrough"
incus config device add "${CONTAINER}" x11 disk \
    source=/tmp/.X11-unix \
    path=/tmp/.X11-unix \
    readonly=false || true

echo "==> Adding Wayland socket passthrough (host user 1000)"
incus config device add "${CONTAINER}" wayland disk \
    source=/run/user/1000 \
    path=/run/user/1000 \
    readonly=false || true

echo "==> Setting container environment variables for display"
incus config set "${CONTAINER}" environment.DISPLAY ":0"
incus config set "${CONTAINER}" environment.WAYLAND_DISPLAY "wayland-0"
incus config set "${CONTAINER}" environment.XDG_RUNTIME_DIR "/run/user/1000"

echo "==> GUI socket devices configured."
echo "    You can now enter the container and run the egui application:"
echo "      incus exec ${CONTAINER} -- bash"
echo "      cd /path/to/crates/op-web/ui"
echo "      cargo run"
echo ""
echo "    Make sure the host has a running X11 or Wayland session before launching the UI."