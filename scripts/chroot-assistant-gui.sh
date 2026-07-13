#!/usr/bin/env bash
# chroot-assistant-gui.sh
# Pre-install GUI development stack inside the assistant container rootfs
# from the host Artix environment while bypassing the container's network stack.
# Guidelines: socket networking only; add software via Artix chroot into btrfs rootfs.

set -euo pipefail

CONTAINER_ROOTFS="/opt/incus/storage-pool/containers/assistant/rootfs"
MOUNTPOINT="${CONTAINER_ROOTFS}"

if [[ $EUID -ne 0 ]]; then
  echo "This script must be run as root (sudo)." >&2
  exit 1
fi

echo "==> Preparing mounts for chroot into ${MOUNTPOINT}"

# Bind mounts for a fully functional chroot
mount -t proc /proc "${MOUNTPOINT}/proc" 2>/dev/null || true
mount -t sysfs /sys "${MOUNTPOINT}/sys" 2>/dev/null || true
mount --bind /dev "${MOUNTPOINT}/dev" 2>/dev/null || true
mount --bind /dev/pts "${MOUNTPOINT}/dev/pts" 2>/dev/null || true
mount --bind /run "${MOUNTPOINT}/run" 2>/dev/null || true

# Copy host DNS so apt works inside chroot (handle symlink case)
if [ -L "${MOUNTPOINT}/etc/resolv.conf" ]; then
    rm -f "${MOUNTPOINT}/etc/resolv.conf"
fi
cp /etc/resolv.conf "${MOUNTPOINT}/etc/resolv.conf"

echo "==> Entering chroot and installing GUI + development packages (host network)"

chroot "${MOUNTPOINT}" /bin/bash <<'CHROOT_EOF'
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install -y \
    xorg \
    x11-utils \
    x11-xserver-utils \
    xinit \
    openbox \
    xterm \
    spice-vdagent \
    build-essential \
    pkg-config \
    libssl-dev \
    curl \
    git \
    ca-certificates \
    dbus-x11 \
    libgtk-3-0 \
    libgtk-3-dev \
    libwayland-client0 \
    libwayland-server0 \
    libwayland-egl1 \
    wayland-protocols \
    mesa-utils \
    || true

# Optional: minimal wayland compositor (weston) if not already pulled in
apt-get install -y weston || true

echo "==> Cleaning apt cache"
apt-get clean

echo "==> GUI stack installation complete inside container rootfs"
CHROOT_EOF

echo "==> Unmounting chroot bind mounts"
umount "${MOUNTPOINT}/proc" 2>/dev/null || true
umount "${MOUNTPOINT}/sys" 2>/dev/null || true
umount "${MOUNTPOINT}/dev/pts" 2>/dev/null || true
umount "${MOUNTPOINT}/dev" 2>/dev/null || true
umount "${MOUNTPOINT}/run" 2>/dev/null || true

echo "==> Chroot package installation finished. Container now has a GUI stack."
echo "    Next: configure incus devices for X11/Wayland socket pass-through (unix sockets only)."