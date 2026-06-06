#!/bin/bash
# Install the systemd→elogind makepkg auto-patch hook
# Run: sudo ./install-makepkg-hook.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOK_DIR="/usr/local/lib/makepkg-hooks"
CONFIG_DIR="/etc/makepkg.d"

echo "=== Installing systemd→elogind makepkg auto-patch hook ==="

# 1. Install the hook script
mkdir -p "$HOOK_DIR"
cp "$SCRIPT_DIR/hooks/autopatch-systemd-to-elogind.sh" "$HOOK_DIR/"
chmod +x "$HOOK_DIR/autopatch-systemd-to-elogind.sh"
echo "Installed: $HOOK_DIR/autopatch-systemd-to-elogind.sh"

# 2. Install the hook definition
mkdir -p "$CONFIG_DIR"
cp "$SCRIPT_DIR/hooks/50-autopatch-systemd-to-elogind.hook" "$CONFIG_DIR/"
echo "Installed: $CONFIG_DIR/50-autopatch-systemd-to-elogind.hook"

# 3. Verify makepkg supports hooks
if ! makepkg --help 2>&1 | grep -q "hook"; then
    echo ""
    echo "WARNING: Your makepkg may not support hooks natively."
    echo "If using pacman/makepkg from Artix, hooks should work."
    echo "Alternative: add to ~/.makepkg.conf:"
    echo '  PREBUILD="/usr/local/lib/makepkg-hooks/autopatch-systemd-to-elogind.sh"'
fi

echo ""
echo "=== Done ==="
echo "The hook will now auto-patch any AUR/package build that references libsystemd."
echo "It replaces libsystemd with libelogind in:"
echo "  - meson.build files"
echo "  - CMakeLists.txt files"
echo "  - configure.ac files"
echo "  - .pc pkg-config files"
echo "  - PKGBUILD depends arrays"
echo "  - C/C++ header includes (#include <systemd/...> → <elogind/systemd/...>)"
echo ""
echo "To test: cd /home/jeremy/.cache/paru/clone/sdbusplus-git && makepkg -si --nocheck"
