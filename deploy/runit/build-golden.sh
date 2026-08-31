#!/bin/sh
# build-golden.sh — publish a release two ways, from one build.
#
#   1. GOLDEN  — populate the `golden` btrfs subvolume: the deployable content.
#                Taking the read-only snapshot and running `btrfs send` belongs
#                to the deployment process, not here. This script only makes
#                `golden` correct and current.
#   2. LIVE    — install the same binaries into this host's runtime
#                (/usr/local/bin, /etc/runit/sv) and restart only the runit
#                services whose binary or release-owned definition changed.
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
#     etc/                       environment defaults, pacman hooks
#     MANIFEST                   commit, build time, sha256 per binary
#
# Usage:
#   build-golden.sh                 # both paths (default)
#   build-golden.sh --golden-only   # skip touching the running host
#   build-golden.sh --live-only     # skip the subvolume
#   build-golden.sh --no-restart    # install live but leave services running old code
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

SCRIPT_PATH=$(readlink -f "$0" 2>/dev/null || printf '%s' "$0")
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$SCRIPT_PATH")" && pwd)
if [ -f "$SCRIPT_DIR/../../Cargo.toml" ]; then
    PROJECT_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
else
    PROJECT_ROOT=${OP_DBUS_ROOT:-$(pwd)}
fi
RELEASE_DIR="$PROJECT_ROOT/target/release"
RETIRED_SERVICES_FILE="$SCRIPT_DIR/retired-services"
RETIRED_BINARIES_FILE="$SCRIPT_DIR/retired-binaries"
MANAGED_SERVICES_FILE="$SCRIPT_DIR/managed-services"
ENABLED_SERVICES_FILE="$SCRIPT_DIR/enabled-services"

is_retired_service() {
    svc=$1
    [ -f "$RETIRED_SERVICES_FILE" ] &&
        grep -Ev '^[[:space:]]*(#|$)' "$RETIRED_SERVICES_FILE" | grep -qx "$svc"
}

is_managed_service() {
    svc=$1
    [ -f "$MANAGED_SERVICES_FILE" ] &&
        grep -Ev '^[[:space:]]*(#|$)' "$MANAGED_SERVICES_FILE" | grep -qx "$svc"
}

is_retired_binary() {
    binary=$1
    [ -f "$RETIRED_BINARIES_FILE" ] &&
        grep -Ev '^[[:space:]]*(#|$)' "$RETIRED_BINARIES_FILE" | grep -qx "$binary"
}

log()  { printf '\033[0;34m[golden]\033[0m %s\n' "$*"; }
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
        --dry-run)     DRY_RUN=1 ;;
        -h|--help)     sed -n '2,35p' "$SCRIPT_PATH"; exit 0 ;;
        *)             die "unknown option: $1" ;;
    esac
    shift
done

# ── Preconditions ───────────────────────────────────────────────────────────
[ -d "$RELEASE_DIR" ] || die "no $RELEASE_DIR — build first:
  CXXFLAGS=\"-include cstdint\" cargo build --workspace --release"

BINARIES=$(find "$RELEASE_DIR" -maxdepth 1 -type f -executable ! -name '*.d' ! -name '*.so' | sort |
    while IFS= read -r bin; do
        is_retired_binary "$(basename "$bin")" || printf '%s\n' "$bin"
    done)
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

    run mkdir -p "$GOLDEN_DIR/bin" "$GOLDEN_DIR/sbin" "$GOLDEN_DIR/sv" "$GOLDEN_DIR/etc"

    # Binaries. A reused golden subvolume must not retain listener executables
    # that were retired when their implementation moved behind the bridge.
    if [ -f "$RETIRED_BINARIES_FILE" ]; then
        grep -Ev '^[[:space:]]*(#|$)' "$RETIRED_BINARIES_FILE" | while IFS= read -r name; do
            [ -n "$name" ] || continue
            [ ! -e "$GOLDEN_DIR/bin/$name" ] || run rm -- "$GOLDEN_DIR/bin/$name"
        done
    fi
    printf '%s\n' "$BINARIES" | while IFS= read -r bin; do
        run install -Dm755 "$bin" "$GOLDEN_DIR/bin/$(basename "$bin")"
    done
    ok "staged $BIN_COUNT binaries into golden/bin"

    # Control scripts + the systemd compatibility layer.
    for script in systemd-unit-to-runit op-convert-systemd-units systemctl-shim \
                  build-golden.sh; do
        [ -f "$SCRIPT_DIR/$script" ] &&
            run install -Dm755 "$SCRIPT_DIR/$script" "$GOLDEN_DIR/sbin/$script"
    done
    [ -f "$SCRIPT_DIR/../agent-runit-guard.sh" ] &&
        run install -Dm755 "$SCRIPT_DIR/../agent-runit-guard.sh" \
            "$GOLDEN_DIR/sbin/agent-runit-guard.sh"
    [ -f "$SCRIPT_DIR/libexec-3tched/xray-config-mount-up" ] &&
        run install -Dm755 "$SCRIPT_DIR/libexec-3tched/xray-config-mount-up" \
            "$GOLDEN_DIR/libexec/3tched/xray-config-mount-up"
    ok "staged control scripts into golden/sbin"

    # Runit service definitions tracked in the repo. Explicit retirement is
    # subtractive so a reused golden subvolume cannot resurrect an old unit.
    if [ -f "$RETIRED_SERVICES_FILE" ]; then
        grep -Ev '^[[:space:]]*(#|$)' "$RETIRED_SERVICES_FILE" | while IFS= read -r svc; do
            [ -n "$svc" ] || continue
            [ ! -e "$GOLDEN_DIR/sv/$svc" ] || run rm -r -- "$GOLDEN_DIR/sv/$svc"
        done
    fi
    for svc_dir in "$SCRIPT_DIR"/*/; do
        [ -f "${svc_dir}run" ] || continue
        svc=$(basename "$svc_dir")
        is_retired_service "$svc" && continue
        run mkdir -p "$GOLDEN_DIR/sv/$svc"
        run install -Dm755 "${svc_dir}run" "$GOLDEN_DIR/sv/$svc/run"
        [ -f "${svc_dir}log/run" ] &&
            run install -Dm755 "${svc_dir}log/run" "$GOLDEN_DIR/sv/$svc/log/run"
        [ -f "${svc_dir}finish" ] &&
            run install -Dm755 "${svc_dir}finish" "$GOLDEN_DIR/sv/$svc/finish"
    done
    ok "staged runit service definitions into golden/sv"

    # Config the target needs alongside the binaries.
    [ -f "$SCRIPT_DIR/../environment.default" ] &&
        run install -Dm644 "$SCRIPT_DIR/../environment.default" \
            "$GOLDEN_DIR/etc/environment.default"
    [ -f "$SCRIPT_DIR/99-systemd-unit-to-runit.hook" ] &&
        run install -Dm644 "$SCRIPT_DIR/99-systemd-unit-to-runit.hook" \
            "$GOLDEN_DIR/etc/pacman-hooks/99-systemd-unit-to-runit.hook"
    [ -f "$SCRIPT_DIR/../99-agent-runit-guard.hook" ] &&
        run install -Dm644 "$SCRIPT_DIR/../99-agent-runit-guard.hook" \
            "$GOLDEN_DIR/etc/pacman-hooks/99-agent-runit-guard.hook"

    # Network configuration shipped by this release. These paths mirror the
    # live locations below so golden and the running host remain one artifact.
    for mapping in \
        "nftables.conf:nftables.conf" \
        "iptables.rules:iptables/iptables.rules" \
        "network.conf:op-dbus/network.conf" \
        "openflow-static-flows.json:op-dbus/openflow-static-flows.json" \
        "sshd/sshd_config:ssh/sshd_config" \
        "sshd/10-loopback-only.conf:ssh/sshd_config.d/10-loopback-only.conf" \
        "sshd/90-password-public.conf:ssh/sshd_config.d/90-singleuser-password.conf"
    do
        src=${mapping%%:*}
        rel=${mapping#*:}
        [ -f "$SCRIPT_DIR/../config/$src" ] &&
            run install -Dm644 "$SCRIPT_DIR/../config/$src" "$GOLDEN_DIR/etc/$rel"
    done

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
uplink-dhcp op-session-bus opdbus-rundirs dbus"

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

listener_present() {
    address=$1
    ss -H -lnt 2>/dev/null | awk -v wanted="$address" '
        $4 == wanted { found = 1 }
        END { exit(found ? 0 : 1) }
    '
}

local_ipv4_present() {
    address=$1
    ip -4 -o addr show 2>/dev/null | awk -v wanted="$address" '
        {
            split($4, parts, "/")
            if (parts[1] == wanted) found = 1
        }
        END { exit(found ? 0 : 1) }
    '
}

bridge_netmaker_listener() (
    set +u
    set -a
    [ -r /etc/op-dbus/environment ] && . /etc/op-dbus/environment
    [ -r /etc/op-dbus/netmaker-broker.env ] && . /etc/op-dbus/netmaker-broker.env
    set +a
    NETMAKER_MESH_IP=${NETMAKER_MESH_IP:-100.69.0.1}
    if local_ipv4_present "$NETMAKER_MESH_IP"; then
        printf '%s:8090\n' "$NETMAKER_MESH_IP"
    fi
)

legacy_8090_relay_running() {
    pgrep -f '/usr/local/libexec/3tched/socket-relay .*tcp-listen .* 8090' >/dev/null 2>&1
}

wait_legacy_8090_relays_down() {
    attempts=0
    while legacy_8090_relay_running; do
        attempts=$((attempts + 1))
        [ "$attempts" -lt 30 ] || die "legacy :8090 relay still running after retirement"
        sleep 1
    done
}

bridge_ready() {
    sv status op-grpc-bridge 2>/dev/null | grep -q '^run:' || return 1
    pgrep -x op-grpc-bridge >/dev/null 2>&1 || return 1
    [ -S /run/opdbus/grpc.sock ] || return 1
    listener_present 127.0.0.1:8090 || return 1
    listener_present 10.0.0.3:8090 || return 1
    netmaker_listener=$(bridge_netmaker_listener)
    [ -z "$netmaker_listener" ] || listener_present "$netmaker_listener" || return 1
    ! legacy_8090_relay_running
}

wait_bridge_ready() {
    attempts=0
    until bridge_ready; do
        attempts=$((attempts + 1))
        [ "$attempts" -lt 45 ] || return 1
        sleep 1
    done
}

install_live() {
    log "installing into the live runtime"

    changed_binaries=""
    changed_services=""
    bridge_cutover_needed=0
    for legacy_bridge_relay in fwd-8090 fwd-nm-mesh-8090; do
        if [ -L "$RUNIT_RUNSVDIR/$legacy_bridge_relay" ] ||
           [ -d "$RUNIT_SV_DIR/$legacy_bridge_relay" ]; then
            bridge_cutover_needed=1
        fi
    done
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
    for script in systemd-unit-to-runit op-convert-systemd-units; do
        [ -f "$SCRIPT_DIR/$script" ] &&
            run install -Dm755 "$SCRIPT_DIR/$script" "$INSTALL_SBIN/$script"
    done
    [ -f "$SCRIPT_DIR/systemctl-shim" ] &&
        run install -Dm755 "$SCRIPT_DIR/systemctl-shim" "$INSTALL_BIN/systemctl"
    [ -f "$SCRIPT_DIR/../agent-runit-guard.sh" ] &&
        run install -Dm755 "$SCRIPT_DIR/../agent-runit-guard.sh" \
            "$INSTALL_SBIN/agent-runit-guard"
    if [ -f "$SCRIPT_DIR/libexec-3tched/xray-config-mount-up" ]; then
        xray_helper="/usr/local/libexec/3tched/xray-config-mount-up"
        if [ -f "$xray_helper" ] && ! cmp -s \
            "$SCRIPT_DIR/libexec-3tched/xray-config-mount-up" "$xray_helper"; then
            run mkdir -p "/var/tmp/op-control-pre-golden-$STAMP"
            run install -Dm755 "$xray_helper" \
                "/var/tmp/op-control-pre-golden-$STAMP/xray-config-mount-up"
        fi
        run install -Dm755 "$SCRIPT_DIR/libexec-3tched/xray-config-mount-up" "$xray_helper"
    fi
    ok "installed control scripts + systemd compat layer"

    if [ -d /etc/pacman.d/hooks ] || mkdir -p /etc/pacman.d/hooks 2>/dev/null; then
        [ -f "$SCRIPT_DIR/99-systemd-unit-to-runit.hook" ] &&
            run install -Dm644 "$SCRIPT_DIR/99-systemd-unit-to-runit.hook" \
                /etc/pacman.d/hooks/99-systemd-unit-to-runit.hook
    fi

    # Retire exact service definitions recoverably. Removing the enabled
    # symlink is runit's source-of-truth operation; /run/runit/service is never
    # edited directly. Old definitions remain under /var/tmp for rollback.
    retired_backup="/var/tmp/op-runit-retired-$STAMP"
    if [ -f "$RETIRED_SERVICES_FILE" ]; then
        grep -Ev '^[[:space:]]*(#|$)' "$RETIRED_SERVICES_FILE" | while IFS= read -r svc; do
            [ -n "$svc" ] || continue
            [ ! -L "$RUNIT_RUNSVDIR/$svc" ] || run rm -- "$RUNIT_RUNSVDIR/$svc"
            if [ -d "$RUNIT_SV_DIR/$svc" ]; then
                run mkdir -p "$retired_backup"
                run mv -- "$RUNIT_SV_DIR/$svc" "$retired_backup/$svc"
            fi
        done
    fi

    # The crate libraries remain part of the bridge, but their old standalone
    # listener executables are not deployable surfaces. Preserve any installed
    # copies alongside the service rollback backup, then remove them live.
    if [ -f "$RETIRED_BINARIES_FILE" ]; then
        grep -Ev '^[[:space:]]*(#|$)' "$RETIRED_BINARIES_FILE" | while IFS= read -r name; do
            [ -n "$name" ] || continue
            if [ -f "$INSTALL_BIN/$name" ]; then
                run mkdir -p "$retired_backup/bin"
                run mv -- "$INSTALL_BIN/$name" "$retired_backup/bin/$name"
            fi
        done
    fi

    # Install the release-owned network config and preserve replaced host
    # copies under one timestamped rollback directory.
    config_backup="/var/tmp/op-config-pre-golden-$STAMP"
    legacy_sshd_dropin="/etc/ssh/sshd_config.d/10-netmaker-only.conf"
    if [ -f "$legacy_sshd_dropin" ]; then
        run mkdir -p "$config_backup/etc/ssh/sshd_config.d"
        run mv -- "$legacy_sshd_dropin" \
            "$config_backup/etc/ssh/sshd_config.d/10-netmaker-only.conf"
    fi
    for mapping in \
        "nftables.conf:/etc/nftables.conf" \
        "iptables.rules:/etc/iptables/iptables.rules" \
        "network.conf:/etc/op-dbus/network.conf" \
        "openflow-static-flows.json:/etc/op-dbus/openflow-static-flows.json" \
        "sshd/sshd_config:/etc/ssh/sshd_config" \
        "sshd/10-loopback-only.conf:/etc/ssh/sshd_config.d/10-loopback-only.conf" \
        "sshd/90-password-public.conf:/etc/ssh/sshd_config.d/90-singleuser-password.conf"
    do
        src=${mapping%%:*}
        dest=${mapping#*:}
        source_file="$SCRIPT_DIR/../config/$src"
        [ -f "$source_file" ] || continue
        if [ -f "$dest" ] && ! cmp -s "$source_file" "$dest"; then
            run mkdir -p "$config_backup$(dirname "$dest")"
            run install -Dm644 "$dest" "$config_backup$dest"
        fi
        run install -Dm644 "$source_file" "$dest"
    done

    # Service definitions normally preserve a hand-tuned host copy. Services
    # listed in managed-services are deliberately release-owned and replaced
    # with a recoverable backup when they differ.
    service_backup="/var/tmp/op-runit-pre-golden-$STAMP"
    for svc_dir in "$SCRIPT_DIR"/*/; do
        [ -f "${svc_dir}run" ] || continue
        svc=$(basename "$svc_dir")
        is_retired_service "$svc" && continue
        dest="$RUNIT_SV_DIR/$svc/run"
        definition_changed=0
        if [ -f "$dest" ] && ! cmp -s "${svc_dir}run" "$dest"; then
            if is_managed_service "$svc"; then
                run mkdir -p "$service_backup/$svc"
                run install -Dm755 "$dest" "$service_backup/$svc/run"
                definition_changed=1
            else
                warn "$dest differs from the repo copy — leaving the host version alone"
                continue
            fi
        elif [ ! -f "$dest" ]; then
            definition_changed=1
        fi
        run install -Dm755 "${svc_dir}run" "$dest"
        [ -f "${svc_dir}log/run" ] &&
            run install -Dm755 "${svc_dir}log/run" "$RUNIT_SV_DIR/$svc/log/run"
        if [ "$definition_changed" = 1 ] && is_managed_service "$svc"; then
            changed_services="$changed_services $svc"
        fi
    done

    # Enable only explicitly declared new services. Existing enablement is
    # otherwise untouched.
    if [ -f "$ENABLED_SERVICES_FILE" ]; then
        grep -Ev '^[[:space:]]*(#|$)' "$ENABLED_SERVICES_FILE" | while IFS= read -r svc; do
            [ -n "$svc" ] || continue
            if [ "$DRY_RUN" = 1 ]; then
                [ -f "$SCRIPT_DIR/$svc/run" ] || die "enabled service has no definition: $svc"
            else
                [ -d "$RUNIT_SV_DIR/$svc" ] || die "enabled service has no definition: $svc"
            fi
            [ -e "$RUNIT_RUNSVDIR/$svc" ] ||
                run ln -s "$RUNIT_SV_DIR/$svc" "$RUNIT_RUNSVDIR/$svc"
        done
    fi

    if [ "$bridge_cutover_needed" = 1 ] && [ -e "$RUNIT_RUNSVDIR/op-grpc-bridge" ]; then
        case " $changed_services " in
            *" op-grpc-bridge "*) ;;
            *) changed_services="$changed_services op-grpc-bridge" ;;
        esac
    fi

    if [ "$DO_RESTART" != 1 ]; then
        warn "--no-restart: services still running the previous binaries"
        return 0
    fi
    if [ -z "$changed_binaries" ] && [ -z "$changed_services" ]; then
        log "nothing changed; no restarts needed"
        if [ -e "$RUNIT_RUNSVDIR/op-grpc-bridge" ]; then
            wait_legacy_8090_relays_down
            wait_bridge_ready || die "op-grpc-bridge failed direct-bind readiness"
        fi
        return 0
    fi

    # Restart enabled services whose release-owned run definition or binary
    # changed. Network/session-bus carriers remain held back below.
    restart_list=""
    held_back=""
    for svc in $changed_services; do
        [ -e "$RUNIT_RUNSVDIR/$svc" ] || continue
        case " $NEVER_AUTO_RESTART " in
            *" $svc "*)
                held_back="$held_back $svc"
                continue
                ;;
        esac
        case " $restart_list " in
            *" $svc "*) ;;
            *) restart_list="$restart_list $svc" ;;
        esac
    done
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
        if [ "$svc" = op-grpc-bridge ]; then
            if [ "$DRY_RUN" = 1 ]; then
                run sv restart "$svc"
                continue
            fi
            wait_legacy_8090_relays_down
            sv restart "$svc" || die "sv restart $svc failed"
            wait_bridge_ready || die "op-grpc-bridge failed direct-bind readiness"
            ok "op-grpc-bridge ready on required direct :8090 listeners"
        else
            run sv restart "$svc" || warn "sv restart $svc failed"
        fi
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
