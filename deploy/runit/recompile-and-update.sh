#!/bin/sh
# Safely build, install, and refresh the canonical Artix runit service set.
#
# Runit has no compiled service database: definitions under /etc/runit/sv are
# live. After installing new binaries (and optional service trees from
# deploy/runit/), this script enables missing links in the default runsvdir
# and restarts already-enabled services so they pick up the new binaries.
set -eu

SCRIPT_PATH=$(readlink -f "$0" 2>/dev/null || printf '%s' "$0")
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$SCRIPT_PATH")" && pwd)
if [ -f "$SCRIPT_DIR/../../Cargo.toml" ]; then
    PROJECT_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
else
    PROJECT_ROOT=${OP_DBUS_ROOT:-$(pwd)}
fi

RUNIT_SV_DIR=${RUNIT_SV_DIR:-/etc/runit/sv}
RUNIT_RUNSVDIR=${RUNIT_RUNSVDIR:-/etc/runit/runsvdir/default}
INSTALL_SBIN=${INSTALL_SBIN:-/usr/local/sbin}
INSTALL_BIN=${INSTALL_BIN:-/usr/local/bin}

# Build the Rust workspace as the unprivileged user who owns the source tree,
# then install every binary target to /usr/local/bin. This keeps the cargo
# cache and target directory owned by the developer while allowing the final
# install and runit refresh to run as root.
BUILD_USER=${BUILD_USER:-$(logname 2>/dev/null || printf '%s' "${SUDO_USER:-${USER:-root}}")}

# Optional space-separated service names to restart. Empty = auto-detect
# enabled services whose run scripts reference $INSTALL_BIN.
RESTART_SERVICES=${RESTART_SERVICES:-}

build_workspace() {
    echo "Building workspace binaries as $BUILD_USER..."
    cd "$PROJECT_ROOT"
    if [ ! -f Cargo.toml ]; then
        echo "PROJECT_ROOT=$PROJECT_ROOT has no Cargo.toml (set OP_DBUS_ROOT or run from the repo)" >&2
        exit 1
    fi
    sudo -u "$BUILD_USER" cargo build --workspace --release
}

install_binaries() {
    echo "Installing built binaries to $INSTALL_BIN..."
    cd "$PROJECT_ROOT"
    if [ ! -d target/release ]; then
        echo "target/release missing under $PROJECT_ROOT — build first or unset SKIP_BUILD" >&2
        exit 1
    fi
    find target/release -maxdepth 1 -type f -executable -print0 |
        while IFS= read -r -d '' bin; do
            install -Dm755 "$bin" "$INSTALL_BIN/$(basename "$bin")"
        done
}

install_control_scripts() {
    echo "Installing runit control scripts to $INSTALL_SBIN..."
    install -Dm755 "$SCRIPT_PATH" "$INSTALL_SBIN/op-runit-recompile-and-update"
    # Keep the historical s6d reload path working on runit hosts.
    install -Dm755 "$SCRIPT_PATH" "$INSTALL_SBIN/op-s6-recompile-and-update"

    # systemd compatibility layer. Third-party installers assume systemd and
    # call `systemctl enable --now X`; without these they fail and leave a
    # half-installed service. The shim goes in $INSTALL_BIN because
    # sudo's secure_path searches /usr/local/bin before /usr/bin, so it wins
    # for installers that shell out to `systemctl`.
    if [ -f "$SCRIPT_DIR/systemd-unit-to-runit" ]; then
        echo "Installing systemd->runit compatibility layer..."
        install -Dm755 "$SCRIPT_DIR/systemd-unit-to-runit" \
            "$INSTALL_SBIN/systemd-unit-to-runit"
        install -Dm755 "$SCRIPT_DIR/op-convert-systemd-units" \
            "$INSTALL_SBIN/op-convert-systemd-units"
        install -Dm755 "$SCRIPT_DIR/systemctl-shim" "$INSTALL_BIN/systemctl"
        # Pacman hook: convert units that packages drop, without enabling them.
        if [ -d /etc/pacman.d/hooks ] || mkdir -p /etc/pacman.d/hooks 2>/dev/null; then
            install -Dm644 "$SCRIPT_DIR/99-systemd-unit-to-runit.hook" \
                /etc/pacman.d/hooks/99-systemd-unit-to-runit.hook
        fi
    fi
}

sync_repo_service_defs() {
    # Copy any deploy/runit/<svc>/run trees into the live sv directory and
    # enable them in the default runsvdir.
    repo_sv="$PROJECT_ROOT/deploy/runit"
    [ -d "$repo_sv" ] || return 0

    echo "Synchronizing service definitions from $repo_sv"
    for svc_dir in "$repo_sv"/*/; do
        [ -d "$svc_dir" ] || continue
        name=$(basename "$svc_dir")
        [ -f "${svc_dir}run" ] || continue

        dest="$RUNIT_SV_DIR/$name"
        install -d "$dest"
        # Preserve log/ and finish/ alongside run; do not clobber supervise/.
        for f in run finish check; do
            if [ -f "${svc_dir}${f}" ]; then
                install -Dm755 "${svc_dir}${f}" "${dest}/${f}"
            fi
        done
        if [ -d "${svc_dir}log" ]; then
            install -d "${dest}/log"
            if [ -f "${svc_dir}log/run" ]; then
                install -Dm755 "${svc_dir}log/run" "${dest}/log/run"
            fi
        fi

        ln -sfn "$dest" "$RUNIT_RUNSVDIR/$name"
        echo "  enabled: $name"
    done
}

service_uses_local_bin() {
    svc="$1"
    runfile="$RUNIT_SV_DIR/$svc/run"
    [ -f "$runfile" ] || return 1
    grep -qF "$INSTALL_BIN/" "$runfile" 2>/dev/null
}

list_restart_targets() {
    if [ -n "$RESTART_SERVICES" ]; then
        printf '%s\n' $RESTART_SERVICES
        return 0
    fi

    for link in "$RUNIT_RUNSVDIR"/*; do
        [ -e "$link" ] || continue
        name=$(basename "$link")
        case "$name" in
            agetty*|sulogin|udevd|dhcpcd|sshd|dbus|elogind|logind|seatd)
                continue
                ;;
        esac
        if service_uses_local_bin "$name"; then
            printf '%s\n' "$name"
        fi
    done
}

refresh_runit_services() {
    echo "Refreshing enabled runit services under $RUNIT_RUNSVDIR"
    # Give runsv a moment to notice newly created symlinks.
    sleep 1

    targets=$(list_restart_targets || true)
    if [ -z "$targets" ]; then
        echo "  no OP/local-bin services enabled yet — binaries installed only"
        return 0
    fi

    printf '%s\n' "$targets" | while IFS= read -r name; do
        [ -n "$name" ] || continue
        if sv restart "$name" 2>/dev/null || sv start "$name" 2>/dev/null; then
            echo "  restarted: $name"
        else
            echo "  warning: sv restart/start failed for $name" >&2
        fi
    done

    echo "Runit refresh complete (sv status <svc> to verify)"
}

if ! command -v sv >/dev/null 2>&1 || ! command -v runsv >/dev/null 2>&1; then
    echo "runit tools (sv/runsv) not found" >&2
    exit 127
fi

if [ ! -d "$RUNIT_SV_DIR" ] || [ ! -d "$RUNIT_RUNSVDIR" ]; then
    echo "Artix runit paths missing: $RUNIT_SV_DIR or $RUNIT_RUNSVDIR" >&2
    exit 1
fi

if [ "${SKIP_BUILD:-}" != "1" ]; then
    build_workspace
fi
install_binaries
install_control_scripts
sync_repo_service_defs
refresh_runit_services

echo "Activated runit service set from $PROJECT_ROOT"
