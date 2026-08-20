#!/bin/sh
# build-golden.sh — publish a release two ways, from one build.
#
#   1. GOLDEN  — populate the `golden` btrfs subvolume: the deployable content.
#                Taking the read-only snapshot and running `btrfs send` belongs
#                to the deployment process, not here. This script only makes
#                `golden` correct and current.
#   2. LIVE    — install the same binaries into this host's runtime
#                (/usr/local/bin, /etc/runit/sv) and restart only the runit
#                services whose binary actually changed.
#
# Both read the same `target/release`, so the golden subvolume and the running
# host are provably the same build — the MANIFEST records per-binary sha256 so
# that can be checked after the fact.
#
# Layout created under $OPDBUS_ROOT (default /opt/op-dbus):
#
#   golden/                      the deployable subvolume
#     bin/                       release binaries
#     sbin/                      control scripts + systemd compat layer
#     sv/<service>/run           runit service definitions
#     libexec/3tched/            runit helper scripts
#     etc/                       environment defaults, pacman hooks
#     MANIFEST                   commit, build time, sha256 per binary
#
# Usage:
#   build-golden.sh                 # both paths (default)
#   build-golden.sh --golden-only   # skip touching the running host
#   build-golden.sh --live-only     # skip the subvolume
#   build-golden.sh --no-restart    # install live but leave services running old code
#   build-golden.sh --replace-service NAME  # replace one hand-tuned run definition
#   build-golden.sh --dry-run
#
# Requires root for the subvolume and the live install. Never builds: run
# `CXXFLAGS="-include cstdint" cargo build --workspace --release` first (the
# vendored RocksDB in cozorocks needs that flag on modern GCC).
set -eu

OPDBUS_ROOT=${OPDBUS_ROOT:-/opt/op-dbus}
GOLDEN_DIR="$OPDBUS_ROOT/golden"

INSTALL_BIN=${INSTALL_BIN:-/usr/local/bin}
INSTALL_SBIN=${INSTALL_SBIN:-/usr/local/sbin}
RUNIT_SV_DIR=${RUNIT_SV_DIR:-/etc/runit/sv}
RUNIT_RUNSVDIR=${RUNIT_RUNSVDIR:-/etc/runit/runsvdir/default}

DO_GOLDEN=1
DO_LIVE=1
DO_RESTART=1
DRY_RUN=0
REPLACE_SERVICES=""

SCRIPT_PATH=$(readlink -f "$0" 2>/dev/null || printf '%s' "$0")
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$SCRIPT_PATH")" && pwd)
if [ -f "$SCRIPT_DIR/../../Cargo.toml" ]; then
    PROJECT_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
else
    PROJECT_ROOT=${OP_DBUS_ROOT:-$(pwd)}
fi
RELEASE_DIR="$PROJECT_ROOT/target/release"

log()  { printf '\033[1;36m[golden]\033[0m %s\n' "$*"; }
ok()   { printf '\033[0;32m[ ok  ]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[warn ]\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[0;31m[fail ]\033[0m %s\n' "$*" >&2; exit 1; }

run() {
    if [ "$DRY_RUN" = 1 ]; then
        printf '  would run: %s\n' "$*"
    else
        "$@"
    fi
}

while [ $# -gt 0 ]; do
    case "$1" in
        --golden-only) DO_LIVE=0 ;;
        --live-only)   DO_GOLDEN=0 ;;
        --no-restart)  DO_RESTART=0 ;;
        --replace-service)
            shift
            [ $# -gt 0 ] || die "--replace-service requires a service name"
            REPLACE_SERVICES="$REPLACE_SERVICES $1"
            ;;
        --dry-run)     DRY_RUN=1 ;;
        -h|--help)     sed -n '2,35p' "$SCRIPT_PATH"; exit 0 ;;
        *)             die "unknown option: $1" ;;
    esac
    shift
done

# ── Preconditions ───────────────────────────────────────────────────────────
[ -d "$RELEASE_DIR" ] || die "no $RELEASE_DIR — build first:
  CXXFLAGS=\"-include cstdint\" cargo build --workspace --release"

BINARIES=$(find "$RELEASE_DIR" -maxdepth 1 -type f -executable ! -name '*.d' ! -name '*.so' | sort)
[ -n "$BINARIES" ] || die "$RELEASE_DIR contains no executables"
BIN_COUNT=$(printf '%s\n' "$BINARIES" | wc -l)
log "$BIN_COUNT release binaries in $RELEASE_DIR"

if [ "$DRY_RUN" != 1 ] && [ "$(id -u)" != 0 ]; then
    die "must run as root (subvolume creation and install to $INSTALL_BIN)"
fi

COMMIT=$(git -C "$PROJECT_ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)
STAMP=$(date -u '+%Y%m%dT%H%M%SZ')

# ═══ Path 1: golden subvolume ═══════════════════════════════════════════════
build_golden() {
    log "building golden subvolume at $GOLDEN_DIR"

    # /opt must be btrfs for subvolumes; refuse rather than silently making a
    # plain directory that cannot be snapshotted or sent.
    fstype=$(stat -f -c %T "$OPDBUS_ROOT" 2>/dev/null || stat -f -c %T "$(dirname "$OPDBUS_ROOT")")
    case "$fstype" in
        btrfs) ;;
        *) die "$OPDBUS_ROOT is on '$fstype', not btrfs — snapshots and send need btrfs" ;;
    esac

    run mkdir -p "$OPDBUS_ROOT"

    if [ ! -d "$GOLDEN_DIR" ]; then
        run btrfs subvolume create "$GOLDEN_DIR"
        ok "created subvolume $GOLDEN_DIR"
    else
        log "reusing existing subvolume $GOLDEN_DIR"
    fi

    run mkdir -p "$GOLDEN_DIR/bin" "$GOLDEN_DIR/sbin" "$GOLDEN_DIR/sv" \
        "$GOLDEN_DIR/etc" "$GOLDEN_DIR/libexec/3tched"

    # Binaries
    printf '%s\n' "$BINARIES" | while IFS= read -r bin; do
        run install -Dm755 "$bin" "$GOLDEN_DIR/bin/$(basename "$bin")"
    done
    ok "staged $BIN_COUNT binaries into golden/bin"

    # Control scripts + the systemd compatibility layer.
    for script in systemd-unit-to-runit op-convert-systemd-units systemctl-shim \
                  build-golden.sh 3tched-incus-svcgen; do
        [ -f "$SCRIPT_DIR/$script" ] &&
            run install -Dm755 "$SCRIPT_DIR/$script" "$GOLDEN_DIR/sbin/$script"
    done
    [ -f "$SCRIPT_DIR/../agent-runit-guard.sh" ] &&
        run install -Dm755 "$SCRIPT_DIR/../agent-runit-guard.sh" \
            "$GOLDEN_DIR/sbin/agent-runit-guard.sh"
    [ -f "$PROJECT_ROOT/scripts/opdbus-rundirs-up" ] &&
        run install -Dm755 "$PROJECT_ROOT/scripts/opdbus-rundirs-up" \
            "$GOLDEN_DIR/libexec/3tched/opdbus-rundirs-up"
    [ -f "$PROJECT_ROOT/deploy/netmaker/configure-broker-8090.sh" ] &&
        run install -Dm755 "$PROJECT_ROOT/deploy/netmaker/configure-broker-8090.sh" \
            "$GOLDEN_DIR/sbin/configure-netmaker-broker-8090"
    for asset in emqx-ws-8090.hocon op-uds-relay-8090.conf; do
        [ -f "$PROJECT_ROOT/deploy/netmaker/$asset" ] &&
            run install -Dm644 "$PROJECT_ROOT/deploy/netmaker/$asset" \
                "$GOLDEN_DIR/etc/op-dbus/netmaker/$asset"
    done
    ok "staged control scripts into golden/sbin"

    # Runit service definitions tracked in the repo.
    for svc_dir in "$SCRIPT_DIR"/*/; do
        [ -f "${svc_dir}run" ] || continue
        svc=$(basename "$svc_dir")
        run mkdir -p "$GOLDEN_DIR/sv/$svc"
        run install -Dm755 "${svc_dir}run" "$GOLDEN_DIR/sv/$svc/run"
        [ -f "${svc_dir}log/run" ] &&
            run install -Dm755 "${svc_dir}log/run" "$GOLDEN_DIR/sv/$svc/log/run"
        [ -f "${svc_dir}finish" ] &&
            run install -Dm755 "${svc_dir}finish" "$GOLDEN_DIR/sv/$svc/finish"
        [ -f "${svc_dir}check" ] &&
            run install -Dm755 "${svc_dir}check" "$GOLDEN_DIR/sv/$svc/check"
    done
    for helper in "$SCRIPT_DIR/libexec-3tched"/*; do
        [ -f "$helper" ] || continue
        run install -Dm755 "$helper" "$GOLDEN_DIR/libexec/3tched/$(basename "$helper")"
    done
    ok "staged runit service definitions into golden/sv"

    # Config the target needs alongside the binaries.
    [ -f "$SCRIPT_DIR/../environment.default" ] &&
        run install -Dm644 "$SCRIPT_DIR/../environment.default" \
            "$GOLDEN_DIR/etc/environment.default"
    [ -f "$SCRIPT_DIR/../config/zeroclaw-runtime.toml" ] &&
        run install -Dm644 "$SCRIPT_DIR/../config/zeroclaw-runtime.toml" \
            "$GOLDEN_DIR/etc/zeroclaw-runtime.toml"
    [ -f "$SCRIPT_DIR/../config/tched-router-runtime.toml" ] &&
        run install -Dm644 "$SCRIPT_DIR/../config/tched-router-runtime.toml" \
            "$GOLDEN_DIR/etc/tched-router-runtime.toml"
    [ -f "$SCRIPT_DIR/../config/netmaker-broker.env" ] &&
        run install -Dm644 "$SCRIPT_DIR/../config/netmaker-broker.env" \
            "$GOLDEN_DIR/etc/netmaker-broker.env"
    [ -f "$SCRIPT_DIR/99-systemd-unit-to-runit.hook" ] &&
        run install -Dm644 "$SCRIPT_DIR/99-systemd-unit-to-runit.hook" \
            "$GOLDEN_DIR/etc/pacman-hooks/99-systemd-unit-to-runit.hook"
    [ -f "$SCRIPT_DIR/../99-agent-runit-guard.hook" ] &&
        run install -Dm644 "$SCRIPT_DIR/../99-agent-runit-guard.hook" \
            "$GOLDEN_DIR/etc/pacman-hooks/99-agent-runit-guard.hook"

    # Network config. Without these, golden reproduces the programs but not the
    # network they need to work — the binaries come up on a host with no MSS
    # clamp and an empty OpenFlow table. Both lived only under /etc until
    # 2026-08-13, which is exactly how the flow set was lost for three days and
    # how the mesh MSS clamp stayed mistuned. Staged into golden/etc; installing
    # them onto a running host is deliberately NOT part of the live path, since
    # nftables.conf is partly netclient-generated and blindly overwriting it
    # would clobber the NETMAKER-ACL chains.
    [ -f "$SCRIPT_DIR/../config/nftables.conf" ] &&
        run install -Dm644 "$SCRIPT_DIR/../config/nftables.conf" \
            "$GOLDEN_DIR/etc/nftables.conf"
    [ -f "$SCRIPT_DIR/../config/openflow-static-flows.json" ] &&
        run install -Dm644 "$SCRIPT_DIR/../config/openflow-static-flows.json" \
            "$GOLDEN_DIR/etc/openflow-static-flows.json"
    [ -f "$SCRIPT_DIR/../config/network.conf" ] &&
        run install -Dm644 "$SCRIPT_DIR/../config/network.conf" \
            "$GOLDEN_DIR/etc/network.conf"

    # MANIFEST: what this snapshot is, and the hashes to prove the running host
    # matches it.
    if [ "$DRY_RUN" != 1 ]; then
        {
            printf 'golden-build: %s\n' "$STAMP"
            printf 'commit: %s\n' "$COMMIT"
            printf 'source: %s\n' "$PROJECT_ROOT"
            printf 'init: runit (sv)\n'
            printf 'binaries: %s\n' "$BIN_COUNT"
            printf '\n[sha256]\n'
            printf '%s\n' "$BINARIES" | while IFS= read -r bin; do
                printf '%s  %s\n' "$(sha256sum "$bin" | cut -d' ' -f1)" "$(basename "$bin")"
            done
        } > "$GOLDEN_DIR/MANIFEST"
    fi
    ok "wrote golden/MANIFEST (commit $COMMIT)"
    log "golden is ready; the read-only snapshot and btrfs send happen at deploy time"
}

# ═══ Path 2: live runtime ═══════════════════════════════════════════════════
# Services that are never auto-restarted: they carry the host's network and
# session bus, and bouncing them from a deploy script can cut remote access or
# reparent the control plane. They are reported so an operator can restart them
# deliberately, on the console, in a chosen order.
NEVER_AUTO_RESTART="ovs-vswitchd ovsbr0-addr ovsbr0-svc-addr ovsbr0-uplink \
uplink-dhcp op-grpc-bridge op-session-bus opdbus-rundirs dbus"

# Which enabled services actually exec a given binary?
#
# Matches the installed path with a trailing word boundary, not the bare name:
# a substring match would tie `opdbus` to `opdbus-rundirs` and drag unrelated
# services into the restart set.
services_using() {
    binary=$1
    # `.` is the only ERE metacharacter in these paths.
    escaped=$(printf '%s' "$INSTALL_BIN/$binary" | sed 's/\./\\./g')
    for link in "$RUNIT_RUNSVDIR"/*; do
        [ -e "$link" ] || continue
        svc=$(basename "$link")
        run_script="$RUNIT_SV_DIR/$svc/run"
        [ -f "$run_script" ] || continue
        if grep -qE "${escaped}([^A-Za-z0-9_.-]|\$)" "$run_script" 2>/dev/null; then
            printf '%s\n' "$svc"
        fi
    done
}

install_live() {
    log "installing into the live runtime"

    rundirs_helper_changed=0

    changed_binaries=""
    printf '%s\n' "$BINARIES" | while IFS= read -r bin; do
        name=$(basename "$bin")
        target="$INSTALL_BIN/$name"
        if [ -f "$target" ] && cmp -s "$bin" "$target"; then
            continue
        fi
        run install -Dm755 "$bin" "$target"
        printf '%s\n' "$name" >> /tmp/golden-changed.$$
    done
    [ -f "/tmp/golden-changed.$$" ] && changed_binaries=$(cat "/tmp/golden-changed.$$")
    rm -f "/tmp/golden-changed.$$"

    if [ -z "$changed_binaries" ]; then
        ok "all $BIN_COUNT binaries already current in $INSTALL_BIN"
    else
        ok "updated $(printf '%s\n' "$changed_binaries" | wc -l) binaries in $INSTALL_BIN"
    fi

    # Control scripts. The systemctl shim goes in $INSTALL_BIN because sudo's
    # secure_path searches it before /usr/bin, so it wins for installers.
    for script in systemd-unit-to-runit op-convert-systemd-units \
                  3tched-incus-svcgen; do
        [ -f "$SCRIPT_DIR/$script" ] &&
            run install -Dm755 "$SCRIPT_DIR/$script" "$INSTALL_SBIN/$script"
    done
    [ -f "$SCRIPT_DIR/systemctl-shim" ] &&
        run install -Dm755 "$SCRIPT_DIR/systemctl-shim" "$INSTALL_BIN/systemctl"
    [ -f "$SCRIPT_DIR/../agent-runit-guard.sh" ] &&
        run install -Dm755 "$SCRIPT_DIR/../agent-runit-guard.sh" \
            "$INSTALL_SBIN/agent-runit-guard"
    [ -f "$PROJECT_ROOT/deploy/netmaker/configure-broker-8090.sh" ] &&
        run install -Dm755 "$PROJECT_ROOT/deploy/netmaker/configure-broker-8090.sh" \
            "$INSTALL_SBIN/configure-netmaker-broker-8090"
    for asset in emqx-ws-8090.hocon op-uds-relay-8090.conf; do
        [ -f "$PROJECT_ROOT/deploy/netmaker/$asset" ] &&
            run install -Dm644 "$PROJECT_ROOT/deploy/netmaker/$asset" \
                "/etc/op-dbus/netmaker/$asset"
    done
    if [ -f "$PROJECT_ROOT/scripts/opdbus-rundirs-up" ]; then
        rundirs_helper_target=/usr/local/libexec/3tched/opdbus-rundirs-up
        if [ ! -f "$rundirs_helper_target" ] || \
            ! cmp -s "$PROJECT_ROOT/scripts/opdbus-rundirs-up" "$rundirs_helper_target"; then
            rundirs_helper_changed=1
        fi
        run install -Dm755 "$PROJECT_ROOT/scripts/opdbus-rundirs-up" \
            "$rundirs_helper_target"
    fi
    ok "installed control scripts + systemd compat layer"

    if [ -d /etc/pacman.d/hooks ] || mkdir -p /etc/pacman.d/hooks 2>/dev/null; then
        [ -f "$SCRIPT_DIR/99-systemd-unit-to-runit.hook" ] &&
            run install -Dm644 "$SCRIPT_DIR/99-systemd-unit-to-runit.hook" \
                /etc/pacman.d/hooks/99-systemd-unit-to-runit.hook
    fi

    [ -f "$SCRIPT_DIR/../config/zeroclaw-runtime.toml" ] &&
        run install -Dm644 "$SCRIPT_DIR/../config/zeroclaw-runtime.toml" \
            /etc/op-dbus/zeroclaw-runtime.toml
    [ -f "$SCRIPT_DIR/../config/tched-router-runtime.toml" ] &&
        run install -Dm644 "$SCRIPT_DIR/../config/tched-router-runtime.toml" \
            /etc/op-dbus/tched-router-runtime.toml
    [ -f "$SCRIPT_DIR/../config/netmaker-broker.env" ] &&
        run install -Dm644 "$SCRIPT_DIR/../config/netmaker-broker.env" \
            /etc/op-dbus/netmaker-broker.env
    [ -f "$SCRIPT_DIR/../config/openflow-static-flows.json" ] &&
        run install -Dm644 "$SCRIPT_DIR/../config/openflow-static-flows.json" \
            /etc/op-dbus/openflow-static-flows.json
    [ -f "$SCRIPT_DIR/../config/network.conf" ] &&
        run install -Dm644 "$SCRIPT_DIR/../config/network.conf" \
            /etc/op-dbus/network.conf

    # Service definitions: install new ones, never clobber a hand-tuned run
    # script that differs (the host copy is authoritative until an operator says
    # otherwise).
    for svc_dir in "$SCRIPT_DIR"/*/; do
        [ -f "${svc_dir}run" ] || continue
        svc=$(basename "$svc_dir")
        dest="$RUNIT_SV_DIR/$svc/run"
        if [ -f "$dest" ] && ! cmp -s "${svc_dir}run" "$dest"; then
            case " $REPLACE_SERVICES " in
                *" $svc "*)
                    warn "$dest differs from the repo copy — replacing by explicit request"
                    ;;
                *)
                    warn "$dest differs from the repo copy — leaving the host version alone"
                    continue
                    ;;
            esac
        fi
        run install -Dm755 "${svc_dir}run" "$dest"
        [ -f "${svc_dir}log/run" ] &&
            run install -Dm755 "${svc_dir}log/run" "$RUNIT_SV_DIR/$svc/log/run"
        [ -f "${svc_dir}check" ] &&
            run install -Dm755 "${svc_dir}check" "$RUNIT_SV_DIR/$svc/check"
    done
    for helper in "$SCRIPT_DIR/libexec-3tched"/*; do
        [ -f "$helper" ] || continue
        run install -Dm755 "$helper" \
            "/usr/local/libexec/3tched/$(basename "$helper")"
    done

    if [ "$DO_RESTART" != 1 ]; then
        warn "--no-restart: services still running the previous binaries"
        return 0
    fi
    if [ "$rundirs_helper_changed" = 1 ]; then
        warn "updated opdbus-rundirs-up; opdbus-rundirs is not auto-restarted"
        warn "  apply it deliberately from the console: sudo sv restart opdbus-rundirs"
    fi
    if [ -z "$changed_binaries" ]; then
        log "nothing changed; no restarts needed"
        return 0
    fi

    # Restart only services whose binary actually changed.
    restart_list=""
    held_back=""
    for name in $changed_binaries; do
        for svc in $(services_using "$name"); do
            # Network and session-bus carriers are reported, never bounced here.
            case " $NEVER_AUTO_RESTART " in
                *" $svc "*)
                    case " $held_back " in
                        *" $svc "*) ;;
                        *) held_back="$held_back $svc" ;;
                    esac
                    continue
                    ;;
            esac
            case " $restart_list " in
                *" $svc "*) ;;
                *) restart_list="$restart_list $svc" ;;
            esac
        done
    done

    if [ -n "$held_back" ]; then
        warn "NOT restarted (network/session-bus critical):$held_back"
        warn "  restart these deliberately from the console, e.g.: sudo sv restart <svc>"
    fi

    if [ -z "$restart_list" ]; then
        log "no enabled service references the changed binaries"
        return 0
    fi

    log "restarting:$restart_list"
    for svc in $restart_list; do
        run sv restart "$svc" || warn "sv restart $svc failed"
    done

    # Report real state rather than assuming the restart worked.
    if [ "$DRY_RUN" != 1 ]; then
        sleep 3
        for svc in $restart_list; do
            printf '  %s\n' "$(sv status "$svc" 2>&1 | head -1)"
        done
    fi
}

# ═══ Run ════════════════════════════════════════════════════════════════════
[ "$DO_GOLDEN" = 1 ] && build_golden
[ "$DO_LIVE" = 1 ] && install_live

log "commit $COMMIT, build $STAMP"
[ "$DO_GOLDEN" = 1 ] && log "deployable subvolume: $GOLDEN_DIR"
[ "$DO_LIVE" = 1 ] && log "live runtime: $INSTALL_BIN"
ok "done"
