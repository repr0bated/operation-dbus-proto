#!/usr/bin/env bash
# ====================================================================
# Standalone Installation Script for End-4 / Illogical Impulse
# Target OS: Arch / Artix Linux
# Requirements: 'paru' installed via Cargo in user home space.
# DO NOT RUN THIS SCRIPT WITH SUDO. It requests elevation when needed.
# ====================================================================

set -euo pipefail

# 1. Block Execution as Root User
if [ "$EUID" -eq 0 ]; then
    echo "[-] Error: Do NOT run this script with 'sudo' or as root."
    echo "    Run it as your normal user: bash end-4.sh"
    exit 1
fi

# 2. Path & Environment Resolution
USER_HOME=$(eval echo "~${USER}")
export PATH="$USER_HOME/.cargo/bin:$PATH"
PARU_BIN="$USER_HOME/.cargo/bin/paru"

echo "==> Validating environment for user: ${USER}..."

if ! command -v git &> /dev/null; then
    echo "[-] Error: 'git' is required but not installed."
    echo "    Install it first using: sudo pacman -S git"
    exit 1
fi

if [ ! -x "$PARU_BIN" ]; then
    echo "[-] Error: 'paru' was not found or is not executable at: $PARU_BIN"
    echo "    Ensure paru is installed under your normal user's Cargo bin directory."
    exit 1
fi

echo "[+] Confirmed paru executable at: $PARU_BIN"

# 3. Clean Legacy Conflicts
echo "==> Cleaning up potential legacy package conflicts..."
if pacman -Q | grep -q "illogical-impulse-"; then
    echo "[!] Legacy packages found. Removing via sudo pacman..."
    sudo pacman -Rs $(pacman -Q | grep "illogical-impulse-" | awk '{print $1}') || true
fi

# 4. Setup Target Cache Workspace in User Space
TARGET_DIR="$USER_HOME/.cache/dots-hyprland"
echo "==> Preparing target directory: $TARGET_DIR"

if [ -d "$TARGET_DIR" ]; then
    echo "[!] Repository already exists at $TARGET_DIR."
    echo "    Backing up existing repository directory to avoid conflicts..."
    mv "$TARGET_DIR" "${TARGET_DIR}_backup_$(date +%F_%T)"
fi

# 5. Clone the Upstream Configurations
echo "==> Cloning end-4/dots-hyprland recursively..."
git clone --recursive https://github.com/end-4/dots-hyprland "$TARGET_DIR"

cd "$TARGET_DIR"

# 6. Fire the Installer Framework Non-Interactively
echo "==> Initiating automated setup execution..."
echo "    Note: paru will prompt you for your sudo password to install packages."

# Setup prompts logic passed into standard input:
# - First "y": Authorizes backing up your default configuration folders.
# - "n": Rejects step-by-step confirmation mode, enabling full automation.
printf "y\nn\n" | ./setup install

echo "===================================================================="
echo "[+] Setup completed successfully!"
echo "    Log out of your current session, switch to a TTY, and run: hyprland"
echo "===================================================================="
