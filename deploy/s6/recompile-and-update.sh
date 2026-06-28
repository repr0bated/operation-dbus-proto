#!/bin/sh
# 🟢 🛷 Safely build, commit and activate the canonical Artix s6 service set.
#
# The s6 frontend generates the boot bundle during `s6 set commit`. Compiling
# /etc/s6/sv directly bypasses that step and can produce a database without the
# `default` runlevel, which makes the next boot fail in rc.init.
set -eu

PROJECT_ROOT=$(cd "$(dirname "$0")/../.." && pwd)
S6_COMPILED_LINK=${S6_COMPILED_LINK:-/etc/s6/rc/compiled}
DEFAULT_BUNDLE=${DEFAULT_BUNDLE:-default}
INSTALL_SBIN=${INSTALL_SBIN:-/usr/local/sbin}

# Build the Rust workspace as the unprivileged user who owns the source tree,
# then install every binary target to /usr/local/bin. This keeps the cargo
# cache and target directory owned by the developer while allowing the final
# install and s6 activation to run as root.
BUILD_USER=${BUILD_USER:-$(logname 2>/dev/null || printf '%s' "${SUDO_USER:-$USER}")}

build_workspace() {
    echo "Building workspace binaries as $BUILD_USER..."
    cd "$PROJECT_ROOT"
    sudo -u "$BUILD_USER" cargo build --workspace --release
}

install_binaries() {
    echo "Installing built binaries to /usr/local/bin..."
    cd "$PROJECT_ROOT"
    find target/release -maxdepth 1 -type f -executable -print0 |
        while IFS= read -r -d '' bin; do
            install -Dm755 "$bin" "/usr/local/bin/$(basename "$bin")"
        done
}

install_control_scripts() {
    echo "Installing s6 control scripts to $INSTALL_SBIN..."
    install -Dm755 "$PROJECT_ROOT/deploy/s6/recompile-and-update.sh" \
        "$INSTALL_SBIN/op-s6-recompile-and-update"
}

if ! command -v s6 >/dev/null 2>&1 || ! command -v s6-rc-db >/dev/null 2>&1; then
    echo "s6 frontend or s6-rc-db not found" >&2
    exit 127
fi

if [ "${SKIP_BUILD:-}" != "1" ]; then
    build_workspace
fi
install_binaries
install_control_scripts

old_db="$(readlink -f "$S6_COMPILED_LINK" 2>/dev/null || true)"

echo "Synchronizing the canonical s6 repository"
s6 repository sync
s6 set check -F -u
s6 set commit -f -D "$DEFAULT_BUNDLE"

new_db="$(readlink -f /etc/s6/repo/compiled/current)"
if [ -z "$new_db" ] || [ ! -f "$new_db/db" ]; then
    echo "s6 frontend did not produce a compiled database" >&2
    exit 1
fi

s6-rc-db -c "$new_db" check
if ! s6-rc-db -c "$new_db" list bundles | grep -Fxq "$DEFAULT_BUNDLE"; then
    echo "refusing unsafe database: missing boot bundle '$DEFAULT_BUNDLE'" >&2
    exit 1
fi

echo "Updating live s6-rc state to: $new_db"
if ! s6 live install -b; then
    echo "s6 live install failed; leaving $S6_COMPILED_LINK unchanged" >&2
    [ -n "$old_db" ] && echo "current compiled DB remains: $old_db" >&2
    exit 1
fi

ln -sfnT "$new_db" "$S6_COMPILED_LINK"
if [ "$(readlink -f "$S6_COMPILED_LINK")" != "$new_db" ]; then
    echo "failed to point bootdb at the committed database" >&2
    exit 1
fi

echo "Activated s6-rc database: $new_db"
