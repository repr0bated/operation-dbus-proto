#!/bin/bash
# makepkg PreBuild hook — auto-patch libsystemd → libelogind
# Install to /usr/local/lib/makepkg-hooks/ and reference from a hook in /etc/makepkg.d/
#
# This hook runs before every makepkg build and transparently replaces
# libsystemd dependencies with libelogind on Artix/s6 systems.

set -euo pipefail

# Only patch if elogind is actually available
if ! pkg-config --exists libelogind 2>/dev/null; then
    echo "makepkg-hook: libelogind not found — skipping systemd→elogind patch"
    exit 0
fi

SRCDIR="${SRCDIR:-$PWD/src}"

# If src/ doesn't exist yet, nothing to patch
[[ -d "$SRCDIR" ]] || exit 0

echo "makepkg-hook: scanning for libsystemd references in $SRCDIR ..."

# ── 1. Patch meson.build ─────────────────────────────────────────────────────
find "$SRCDIR" -name "meson.build" -type f -print0 2>/dev/null | \
    while IFS= read -r -d '' f; do
        if grep -q "libsystemd" "$f" 2>/dev/null; then
            echo "makepkg-hook: patching meson.build: $f"
            perl -pi -e "s/dependency\(['\"]libsystemd['\"]\)/dependency('libelogind')/g" "$f"
            perl -pi -e 's/libsystemd/libelogind/g' "$f"
        fi
    done

# ── 2. Patch CMakeLists.txt ────────────────────────────────────────────────
find "$SRCDIR" -name "CMakeLists.txt" -type f -print0 2>/dev/null | \
    while IFS= read -r -d '' f; do
        if grep -qi "libsystemd" "$f" 2>/dev/null; then
            echo "makepkg-hook: patching CMakeLists.txt: $f"
            perl -pi -e 's/libsystemd/libelogind/gi' "$f"
        fi
    done

# ── 3. Patch configure.ac / configure.in ───────────────────────────────────
find "$SRCDIR" -type f \( -name "configure.ac" -o -name "configure.in" \) -print0 2>/dev/null | \
    while IFS= read -r -d '' f; do
        if grep -q "libsystemd" "$f" 2>/dev/null; then
            echo "makepkg-hook: patching configure.ac: $f"
            perl -pi -e 's/libsystemd/libelogind/g' "$f"
        fi
    done

# ── 4. Patch pkg-config .pc files ──────────────────────────────────────────
find "$SRCDIR" -name "*.pc.in" -type f -print0 2>/dev/null | \
    while IFS= read -r -d '' f; do
        if grep -q "libsystemd" "$f" 2>/dev/null; then
            echo "makepkg-hook: patching .pc file: $f"
            perl -pi -e 's/libsystemd/libelogind/g' "$f"
        fi
    done

# ── 5. Patch PKGBUILD depends ──────────────────────────────────────────────
if [[ -f PKGBUILD ]]; then
    if grep -q 'systemd-libs' PKGBUILD 2>/dev/null; then
        echo "makepkg-hook: patching PKGBUILD depends (systemd-libs → elogind)"
        perl -pi -e 's/"systemd-libs"/"elogind"/g' PKGBUILD
        perl -pi -e "s/'systemd-libs'/'elogind'/g" PKGBUILD
    fi
fi

# ── 6. Patch header includes (systemd/*.h → elogind/systemd/*.h) ────────────
find "$SRCDIR" -type f \( -name "*.c" -o -name "*.cpp" -o -name "*.h" -o -name "*.hpp" \) -print0 2>/dev/null | \
    while IFS= read -r -d '' f; do
        if grep -q '#include <systemd/' "$f" 2>/dev/null && \
           ! grep -q '#include <elogind/systemd/' "$f" 2>/dev/null; then
            # Only patch if elogind actually has the systemd compat headers
            if [[ -d /usr/include/elogind/systemd ]]; then
                echo "makepkg-hook: patching includes in: $f"
                perl -pi -e 's|#include <systemd/|#include <elogind/systemd/|g' "$f"
            fi
        fi
    done

echo "makepkg-hook: systemd→elogind patching complete"
