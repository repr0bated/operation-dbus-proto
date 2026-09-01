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
GRANTS_SOURCE="$SCRIPT_DIR/../security/capability-grants.json"
MCP_AUDIENCE_POLICY_SOURCE="$SCRIPT_DIR/../config/mcp-audience-policy.json"
MCP_TOOLSETS_SOURCE="$SCRIPT_DIR/../config/mcp-toolsets.json"
RETIRED_SERVICES_FILE="$SCRIPT_DIR/retired-services"
RETIRED_BINARIES_FILE="$SCRIPT_DIR/retired-binaries"
MANAGED_SERVICES_FILE="$SCRIPT_DIR/managed-services"
ENABLED_SERVICES_FILE="$SCRIPT_DIR/enabled-services"
EMQX_VERSION_FILE="$SCRIPT_DIR/../config/emqx.version"
EMQX_CONFIG_DIR="$SCRIPT_DIR/../emqx"
EMQX_ARTIFACT_CACHE_DIR=${EMQX_ARTIFACT_CACHE_DIR:-/var/cache/op-dbus/source-artifacts}
MCP_PROVIDER_ARTIFACT_CACHE_DIR=${MCP_PROVIDER_ARTIFACT_CACHE_DIR:-/var/cache/op-dbus/source-artifacts}
NOTEBOOKLM_MCP_VERSION_FILE="$SCRIPT_DIR/../config/notebooklm-mcp.version"
MONGODB_MCP_VERSION_FILE="$SCRIPT_DIR/../config/mongodb-mcp-server.version"
NOTEBOOKLM_MCP_OVERLAY_SOURCE="$SCRIPT_DIR/provider-overlays/notebooklm-mcp/http.js"
NOTEBOOKLM_MCP_OVERLAY_TARGET=node_modules/notebooklm-mcp/dist/transport/http.js

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

is_enabled_service() {
    svc=$1
    [ -f "$ENABLED_SERVICES_FILE" ] &&
        grep -Ev '^[[:space:]]*(#|$)' "$ENABLED_SERVICES_FILE" | grep -qx "$svc"
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

[ -r "$EMQX_VERSION_FILE" ] || die "missing EMQX artifact declaration: $EMQX_VERSION_FILE"
# shellcheck disable=SC1090 -- release-owned, fixed-key declaration validated below.
. "$EMQX_VERSION_FILE"
case "${EMQX_VERSION:-}" in *[!0-9.]*|'') die "invalid EMQX_VERSION" ;; esac
case "${EMQX_ARTIFACT:-}" in *[!A-Za-z0-9._-]*|'') die "invalid EMQX_ARTIFACT" ;; esac
case "${EMQX_SHA256:-}" in *[!0-9a-f]*|'') die "invalid EMQX_SHA256" ;; esac
[ "${#EMQX_SHA256}" -eq 64 ] || die "EMQX_SHA256 must contain 64 lowercase hex characters"
EMQX_ARTIFACT_PATH="$EMQX_ARTIFACT_CACHE_DIR/$EMQX_ARTIFACT"
EMQX_INSTALL_NAME="emqx-$EMQX_VERSION-$EMQX_SHA256"

[ -r "$NOTEBOOKLM_MCP_VERSION_FILE" ] ||
    die "missing NotebookLM MCP artifact declaration: $NOTEBOOKLM_MCP_VERSION_FILE"
# shellcheck disable=SC1090 -- release-owned, fixed-key declaration validated below.
. "$NOTEBOOKLM_MCP_VERSION_FILE"
case "${NOTEBOOKLM_MCP_VERSION:-}" in *[!0-9.]*|'') die "invalid NOTEBOOKLM_MCP_VERSION" ;; esac
case "${NOTEBOOKLM_MCP_ARTIFACT:-}" in *[!A-Za-z0-9._-]*|'') die "invalid NOTEBOOKLM_MCP_ARTIFACT" ;; esac
case "${NOTEBOOKLM_MCP_SHA256:-}" in *[!0-9a-f]*|'') die "invalid NOTEBOOKLM_MCP_SHA256" ;; esac
[ "${#NOTEBOOKLM_MCP_SHA256}" -eq 64 ] ||
    die "NOTEBOOKLM_MCP_SHA256 must contain 64 lowercase hex characters"
[ -f "$NOTEBOOKLM_MCP_OVERLAY_SOURCE" ] ||
    die "missing tracked NotebookLM MCP transport overlay: $NOTEBOOKLM_MCP_OVERLAY_SOURCE"
NOTEBOOKLM_MCP_OVERLAY_SHA256=$(sha256sum "$NOTEBOOKLM_MCP_OVERLAY_SOURCE" | cut -d' ' -f1)
NOTEBOOKLM_MCP_DEPLOY_SHA256=$(
    printf '%s\n' \
        "$NOTEBOOKLM_MCP_SHA256" \
        "$NOTEBOOKLM_MCP_OVERLAY_TARGET" \
        "$NOTEBOOKLM_MCP_OVERLAY_SHA256" |
        sha256sum | cut -d' ' -f1
)
NOTEBOOKLM_MCP_ARTIFACT_PATH="$MCP_PROVIDER_ARTIFACT_CACHE_DIR/$NOTEBOOKLM_MCP_ARTIFACT"
NOTEBOOKLM_MCP_INSTALL_NAME="notebooklm-mcp-$NOTEBOOKLM_MCP_VERSION-$NOTEBOOKLM_MCP_DEPLOY_SHA256"

[ -r "$MONGODB_MCP_VERSION_FILE" ] ||
    die "missing MongoDB MCP artifact declaration: $MONGODB_MCP_VERSION_FILE"
# shellcheck disable=SC1090 -- release-owned, fixed-key declaration validated below.
. "$MONGODB_MCP_VERSION_FILE"
case "${MONGODB_MCP_VERSION:-}" in *[!0-9.]*|'') die "invalid MONGODB_MCP_VERSION" ;; esac
case "${MONGODB_MCP_ARTIFACT:-}" in *[!A-Za-z0-9._-]*|'') die "invalid MONGODB_MCP_ARTIFACT" ;; esac
case "${MONGODB_MCP_SHA256:-}" in *[!0-9a-f]*|'') die "invalid MONGODB_MCP_SHA256" ;; esac
[ "${#MONGODB_MCP_SHA256}" -eq 64 ] ||
    die "MONGODB_MCP_SHA256 must contain 64 lowercase hex characters"
MONGODB_MCP_ARTIFACT_PATH="$MCP_PROVIDER_ARTIFACT_CACHE_DIR/$MONGODB_MCP_ARTIFACT"
MONGODB_MCP_INSTALL_NAME="mongodb-mcp-server-$MONGODB_MCP_VERSION-$MONGODB_MCP_SHA256"

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
[ -f "$EMQX_ARTIFACT_PATH" ] || die "missing pinned EMQX artifact: $EMQX_ARTIFACT_PATH"
[ "$(sha256sum "$EMQX_ARTIFACT_PATH" | cut -d' ' -f1)" = "$EMQX_SHA256" ] ||
    die "EMQX artifact digest does not match deploy/config/emqx.version"
tar -tzf "$EMQX_ARTIFACT_PATH" | awk '
    /^\// || /(^|\/)\.\.($|\/)/ { bad = 1 }
    END { exit bad ? 1 : 0 }
' || die "EMQX artifact contains an unsafe path"
for required in bin/emqx etc/emqx.conf etc/base.hocon; do
    tar -tzf "$EMQX_ARTIFACT_PATH" | grep -qx "$required" ||
        die "EMQX artifact is missing $required"
done
for required_config in emqx.conf base.hocon acl.conf; do
    [ -f "$EMQX_CONFIG_DIR/$required_config" ] ||
        die "missing tracked EMQX config: $EMQX_CONFIG_DIR/$required_config"
done
[ -f "$NOTEBOOKLM_MCP_ARTIFACT_PATH" ] ||
    die "missing pinned NotebookLM MCP artifact: $NOTEBOOKLM_MCP_ARTIFACT_PATH"
[ "$(sha256sum "$NOTEBOOKLM_MCP_ARTIFACT_PATH" | cut -d' ' -f1)" = "$NOTEBOOKLM_MCP_SHA256" ] ||
    die "NotebookLM MCP artifact digest does not match deploy/config/notebooklm-mcp.version"
tar -tzf "$NOTEBOOKLM_MCP_ARTIFACT_PATH" | awk '
    /^\// || /(^|\/)\.\.($|\/)/ { bad = 1 }
    END { exit bad ? 1 : 0 }
' || die "NotebookLM MCP artifact contains an unsafe path"
tar -tzf "$NOTEBOOKLM_MCP_ARTIFACT_PATH" |
    grep -qx 'node_modules/notebooklm-mcp/dist/index.js' ||
    die "NotebookLM MCP artifact is missing its provider entry point"
tar -tzf "$NOTEBOOKLM_MCP_ARTIFACT_PATH" |
    grep -qx "$NOTEBOOKLM_MCP_OVERLAY_TARGET" ||
    die "NotebookLM MCP artifact is missing its overlaid HTTP transport"
/usr/bin/node --check "$NOTEBOOKLM_MCP_OVERLAY_SOURCE" >/dev/null ||
    die "NotebookLM MCP transport overlay is not valid JavaScript"

[ -f "$MONGODB_MCP_ARTIFACT_PATH" ] ||
    die "missing pinned MongoDB MCP artifact: $MONGODB_MCP_ARTIFACT_PATH"
[ "$(sha256sum "$MONGODB_MCP_ARTIFACT_PATH" | cut -d' ' -f1)" = "$MONGODB_MCP_SHA256" ] ||
    die "MongoDB MCP artifact digest does not match deploy/config/mongodb-mcp-server.version"
tar -tzf "$MONGODB_MCP_ARTIFACT_PATH" | awk '
    /^\// || /(^|\/)\.\.($|\/)/ { bad = 1 }
    END { exit bad ? 1 : 0 }
' || die "MongoDB MCP artifact contains an unsafe path"
tar -tzf "$MONGODB_MCP_ARTIFACT_PATH" |
    grep -qx 'node_modules/mongodb-mcp-server/dist/esm/index.js' ||
    die "MongoDB MCP artifact is missing its provider entry point"

if [ -f "$GRANTS_SOURCE" ]; then
    [ -x "$RELEASE_DIR/op-grants-materializer" ] ||
        die "release is missing op-grants-materializer"
    "$RELEASE_DIR/op-grants-materializer" validate "$GRANTS_SOURCE" >/dev/null ||
        die "capability grants are not principal-only and valid"
fi
[ -x "$RELEASE_DIR/op-grants-materializer" ] ||
    die "release is missing op-grants-materializer"
"$RELEASE_DIR/op-grants-materializer" validate-audience "$MCP_AUDIENCE_POLICY_SOURCE" >/dev/null ||
    die "MCP audience policy is invalid"
"$RELEASE_DIR/op-grants-materializer" validate-toolsets "$MCP_TOOLSETS_SOURCE" >/dev/null ||
    die "MCP tool-set manifest is invalid"
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
        "$GOLDEN_DIR/etc" "$GOLDEN_DIR/opt"
    run install -d -m 0700 -o root -g root \
        "$GOLDEN_DIR/var/lib/op-dbus/identity-cozo"

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
    for helper in xray-config-mount-up emqx-prepare mcp-provider-probe; do
        [ -f "$SCRIPT_DIR/libexec-3tched/$helper" ] &&
            run install -Dm755 "$SCRIPT_DIR/libexec-3tched/$helper" \
                "$GOLDEN_DIR/libexec/3tched/$helper"
    done
    ok "staged control scripts into golden/sbin"

    # Standalone EMQX is part of the release artifact and is never copied from
    # a running Netmaker container. Reused golden trees are accepted only when
    # their pinned source digest still matches.
    golden_emqx="$GOLDEN_DIR/opt/$EMQX_INSTALL_NAME"
    golden_emqx_marker="$golden_emqx/.opdbus-artifact-sha256"
    golden_emqx_current=0
    if [ -f "$golden_emqx_marker" ] &&
       [ "$(sed -n '1p' "$golden_emqx_marker")" = "$EMQX_SHA256" ]; then
        golden_emqx_current=1
    fi
    if [ "$golden_emqx_current" != 1 ]; then
        [ ! -e "$golden_emqx" ] || run rm -r -- "$golden_emqx"
        run mkdir -p "$golden_emqx"
        run tar -xzf "$EMQX_ARTIFACT_PATH" -C "$golden_emqx"
        if [ "$DRY_RUN" != 1 ]; then
            printf '%s\n' "$EMQX_SHA256" > "$golden_emqx_marker"
        fi
    fi
    run chmod 0755 "$golden_emqx"
    run mkdir -p "$GOLDEN_DIR/etc/emqx"
    if [ -d "$golden_emqx/etc" ] && [ ! -L "$golden_emqx/etc" ]; then
        run cp -a "$golden_emqx/etc/." "$GOLDEN_DIR/etc/emqx/"
        run mv -- "$golden_emqx/etc" "$golden_emqx/etc.dist"
        run ln -s ../../etc/emqx "$golden_emqx/etc"
    fi
    if [ ! -L "$GOLDEN_DIR/opt/emqx" ] ||
       [ "$(readlink "$GOLDEN_DIR/opt/emqx" 2>/dev/null || true)" != "$EMQX_INSTALL_NAME" ]; then
        [ ! -e "$GOLDEN_DIR/opt/emqx" ] || run rm -r -- "$GOLDEN_DIR/opt/emqx"
        run ln -s "$EMQX_INSTALL_NAME" "$GOLDEN_DIR/opt/emqx"
    fi
    for config_name in emqx.conf base.hocon acl.conf; do
        run install -Dm644 "$EMQX_CONFIG_DIR/$config_name" \
            "$GOLDEN_DIR/etc/emqx/$config_name"
    done
    ok "staged pinned standalone EMQX $EMQX_VERSION into golden/opt"

    # Provider packages are immutable, digest-pinned release inputs. Their
    # runit services talk to these versioned trees through stable local links;
    # neither provider creates another externally reachable MCP endpoint.
    stage_golden_mcp_provider() {
        provider_alias=$1
        provider_install_name=$2
        provider_artifact_path=$3
        provider_sha256=$4
        provider_entry=$5
        provider_overlay_source=${6:-}
        provider_overlay_target=${7:-}
        provider_root="$GOLDEN_DIR/opt/op-mcp-providers/$provider_install_name"
        provider_marker="$provider_root/.opdbus-artifact-sha256"
        provider_current=0
        if [ -f "$provider_marker" ] &&
           [ "$(sed -n '1p' "$provider_marker")" = "$provider_sha256" ]; then
            provider_current=1
            if [ -n "$provider_overlay_source" ] &&
               ! cmp -s "$provider_overlay_source" "$provider_root/$provider_overlay_target"; then
                die "$provider_alias content-addressed overlay does not match its digest marker"
            fi
        fi
        if [ "$provider_current" != 1 ]; then
            [ ! -e "$provider_root" ] || run rm -r -- "$provider_root"
            run mkdir -p "$provider_root"
            run tar -xzf "$provider_artifact_path" -C "$provider_root"
            if [ -n "$provider_overlay_source" ]; then
                run install -Dm644 "$provider_overlay_source" \
                    "$provider_root/$provider_overlay_target"
            fi
            if [ "$DRY_RUN" != 1 ]; then
                [ -f "$provider_root/$provider_entry" ] ||
                    die "$provider_alias staged tree is missing $provider_entry"
                if [ -n "$provider_overlay_source" ]; then
                    cmp -s "$provider_overlay_source" \
                        "$provider_root/$provider_overlay_target" ||
                        die "$provider_alias staged overlay verification failed"
                fi
                printf '%s\n' "$provider_sha256" > "$provider_marker"
            fi
        fi
        run chmod 0755 "$provider_root"
        provider_link="$GOLDEN_DIR/opt/op-mcp-providers/$provider_alias"
        if [ ! -L "$provider_link" ] ||
           [ "$(readlink "$provider_link" 2>/dev/null || true)" != "$provider_install_name" ]; then
            if [ -e "$provider_link" ] || [ -L "$provider_link" ]; then
                run rm -r -- "$provider_link"
            fi
            run ln -s "$provider_install_name" "$provider_link"
        fi
    }
    run mkdir -p "$GOLDEN_DIR/opt/op-mcp-providers"
    stage_golden_mcp_provider \
        notebooklm-mcp "$NOTEBOOKLM_MCP_INSTALL_NAME" \
        "$NOTEBOOKLM_MCP_ARTIFACT_PATH" "$NOTEBOOKLM_MCP_DEPLOY_SHA256" \
        node_modules/notebooklm-mcp/dist/index.js \
        "$NOTEBOOKLM_MCP_OVERLAY_SOURCE" "$NOTEBOOKLM_MCP_OVERLAY_TARGET"
    stage_golden_mcp_provider \
        mongodb-mcp-server "$MONGODB_MCP_INSTALL_NAME" \
        "$MONGODB_MCP_ARTIFACT_PATH" "$MONGODB_MCP_SHA256" \
        node_modules/mongodb-mcp-server/dist/esm/index.js
    run mkdir -p \
        "$GOLDEN_DIR/var/log/runit/notebooklm-mcp" \
        "$GOLDEN_DIR/var/log/runit/mongodb-mcp-server"
    ok "staged pinned NotebookLM and MongoDB MCP providers into golden/opt"

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
        [ -f "${svc_dir}check" ] &&
            run install -Dm755 "${svc_dir}check" "$GOLDEN_DIR/sv/$svc/check"
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
    [ -f "$GRANTS_SOURCE" ] &&
        run install -Dm600 "$GRANTS_SOURCE" \
            "$GOLDEN_DIR/etc/opdbus/capability-grants.json"
    run install -Dm600 "$MCP_AUDIENCE_POLICY_SOURCE" \
        "$GOLDEN_DIR/etc/opdbus/mcp-audience-policy.json"
    run install -Dm600 "$MCP_TOOLSETS_SOURCE" \
        "$GOLDEN_DIR/etc/opdbus/mcp-toolsets.json"

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
            printf 'emqx-artifact: %s\n' "$EMQX_ARTIFACT"
            printf 'emqx-sha256: %s\n' "$EMQX_SHA256"
            printf 'notebooklm-mcp-artifact: %s\n' "$NOTEBOOKLM_MCP_ARTIFACT"
            printf 'notebooklm-mcp-sha256: %s\n' "$NOTEBOOKLM_MCP_SHA256"
            printf 'notebooklm-mcp-overlay-sha256: %s\n' "$NOTEBOOKLM_MCP_OVERLAY_SHA256"
            printf 'notebooklm-mcp-deploy-sha256: %s\n' "$NOTEBOOKLM_MCP_DEPLOY_SHA256"
            printf 'mongodb-mcp-artifact: %s\n' "$MONGODB_MCP_ARTIFACT"
            printf 'mongodb-mcp-sha256: %s\n' "$MONGODB_MCP_SHA256"
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
    listener_present 127.0.0.1:9000 || return 1
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

runit_manager_ready() {
    sv status op-runit-systemctl 2>/dev/null | grep -q '^run:' || return 1
    busctl --system introspect \
        org.opdbus.v1.Runit.Systemctl \
        /org/opdbus/v1/plugins/runit/systemctl \
        org.opdbus.v1.Runit.Systemctl >/dev/null 2>&1
}

emqx_ready() {
    sv status emqx 2>/dev/null | grep -q '^run:' || return 1
    listener_present 127.0.0.1:1883 || return 1
    listener_present 127.0.0.1:18083 || return 1
    ! listener_present 0.0.0.0:1883
}

notebooklm_mcp_ready() {
    sv status notebooklm-mcp 2>/dev/null | grep -q '^run:' || return 1
    curl -fsS --max-time 2 http://127.0.0.1:3101/healthz >/dev/null 2>&1 || return 1
    listener_present 127.0.0.1:3101 || return 1
    ! listener_present 0.0.0.0:3101
}

mongodb_mcp_ready() {
    sv status mongodb-mcp-server 2>/dev/null | grep -q '^run:' || return 1
    curl -fsS --max-time 2 http://127.0.0.1:3103/health >/dev/null 2>&1 || return 1
    listener_present 127.0.0.1:3102 || return 1
    listener_present 127.0.0.1:3103 || return 1
    ! listener_present 0.0.0.0:3102 || return 1
    ! listener_present 0.0.0.0:3103
}

wait_service_ready() {
    ready_fn=$1
    attempts=0
    until "$ready_fn"; do
        attempts=$((attempts + 1))
        [ "$attempts" -lt 60 ] || return 1
        sleep 1
    done
}

wait_runit_supervision() {
    svc=$1
    attempts=0
    until sv status "$svc" >/dev/null 2>&1; do
        attempts=$((attempts + 1))
        [ "$attempts" -lt 30 ] || return 1
        sleep 1
    done
}

ensure_enabled_service_ready() {
    svc=$1
    ready_fn=$2
    [ -e "$RUNIT_RUNSVDIR/$svc" ] || [ -L "$RUNIT_RUNSVDIR/$svc" ] || return 0
    "$ready_fn" && return 0
    warn "$svc is enabled but not ready; restarting it to complete the release"
    wait_runit_supervision "$svc" ||
        die "$svc is enabled but runsv did not establish supervision"
    if [ "$svc" = op-grpc-bridge ]; then
        wait_legacy_8090_relays_down
    fi
    sv restart "$svc" || die "sv restart $svc failed"
    if [ "$svc" = op-grpc-bridge ]; then
        wait_bridge_ready || die "op-grpc-bridge failed direct-bind readiness"
    else
        wait_service_ready "$ready_fn" || die "$svc failed readiness"
    fi
}

install_live() {
    log "installing into the live runtime"

    changed_binaries=""
    changed_services=""
    config_backup="/var/tmp/op-config-pre-golden-$STAMP"
    bridge_cutover_needed=0
    for legacy_bridge_relay in fwd-8090 fwd-nm-mesh-8090; do
        if [ -L "$RUNIT_RUNSVDIR/$legacy_bridge_relay" ] ||
           [ -d "$RUNIT_SV_DIR/$legacy_bridge_relay" ]; then
            bridge_cutover_needed=1
        fi
    done
    # A redirected loop stays in the current shell, preserving both the
    # accumulated change list and one exact executable path per input line.
    # This avoids a predictable root-owned /tmp scratch file, including during
    # --dry-run, without regressing whitespace-safe path handling.
    while IFS= read -r bin; do
        [ -n "$bin" ] || continue
        name=$(basename "$bin")
        target="$INSTALL_BIN/$name"
        if [ -f "$target" ] && cmp -s "$bin" "$target"; then
            continue
        fi
        run install -Dm755 "$bin" "$target"
        if [ -z "$changed_binaries" ]; then
            changed_binaries=$name
        else
            changed_binaries="$changed_binaries
$name"
        fi
    done <<EOF
$BINARIES
EOF

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
    for helper_name in xray-config-mount-up emqx-prepare mcp-provider-probe; do
        helper_source="$SCRIPT_DIR/libexec-3tched/$helper_name"
        [ -f "$helper_source" ] || continue
        helper_target="/usr/local/libexec/3tched/$helper_name"
        helper_changed=0
        if [ ! -f "$helper_target" ] || ! cmp -s "$helper_source" "$helper_target"; then
            helper_changed=1
            if [ -f "$helper_target" ]; then
                run mkdir -p "/var/tmp/op-control-pre-golden-$STAMP"
                run install -Dm755 "$helper_target" \
                    "/var/tmp/op-control-pre-golden-$STAMP/$helper_name"
            fi
        fi
        run install -Dm755 "$helper_source" "$helper_target"
        if [ "$helper_changed" = 1 ]; then
            case "$helper_name" in
                xray-config-mount-up) helper_services="xray-config-mount" ;;
                emqx-prepare) helper_services="emqx" ;;
                mcp-provider-probe)
                    helper_services="notebooklm-mcp mongodb-mcp-server"
                    ;;
                *) helper_services="" ;;
            esac
            for helper_service in $helper_services; do
                case " $changed_services " in
                    *" $helper_service "*) ;;
                    *) changed_services="$changed_services $helper_service" ;;
                esac
            done
        fi
    done
    ok "installed control scripts + systemd compat layer"

    # Install the same digest-pinned portable EMQX tree used by the golden
    # image. The immutable root name includes the full artifact digest, so a
    # new artifact never replaces the tree referenced by the stable link.
    live_emqx="/opt/$EMQX_INSTALL_NAME"
    live_emqx_marker="$live_emqx/.opdbus-artifact-sha256"
    emqx_changed=0
    live_emqx_current=0
    if [ -f "$live_emqx_marker" ] &&
       [ "$(sed -n '1p' "$live_emqx_marker")" = "$EMQX_SHA256" ]; then
        live_emqx_current=1
    fi
    if [ "$live_emqx_current" != 1 ]; then
        emqx_changed=1
        [ ! -e "$live_emqx" ] ||
            die "content-addressed EMQX root exists with an invalid digest marker: $live_emqx"
        if [ "$DRY_RUN" = 1 ]; then
            printf '  would stage: %s -> %s\n' "$EMQX_ARTIFACT_PATH" "$live_emqx"
        else
            emqx_stage=$(mktemp -d /opt/.emqx-stage.XXXXXX)
            tar -xzf "$EMQX_ARTIFACT_PATH" -C "$emqx_stage"
            [ -x "$emqx_stage/bin/emqx" ] || die "staged EMQX tree has no executable bin/emqx"
            printf '%s\n' "$EMQX_SHA256" > "$emqx_stage/.opdbus-artifact-sha256"
            chmod 0755 "$emqx_stage"
            mv -- "$emqx_stage" "$live_emqx"
        fi
    fi
    run chmod 0755 "$live_emqx"

    run install -d -m 0750 -o root -g root /etc/emqx
    if [ -d "$live_emqx/etc" ] && [ ! -L "$live_emqx/etc" ]; then
        run cp -an "$live_emqx/etc/." /etc/emqx/
        run mv -- "$live_emqx/etc" "$live_emqx/etc.dist"
        run ln -s /etc/emqx "$live_emqx/etc"
    fi
    if [ -e /opt/emqx ] && [ ! -L /opt/emqx ]; then
        run mkdir -p "/opt/.opdbus-rollback/$STAMP"
        run mv -- /opt/emqx "/opt/.opdbus-rollback/$STAMP/emqx-current"
        emqx_changed=1
    fi
    if [ ! -L /opt/emqx ] || [ "$(readlink /opt/emqx 2>/dev/null || true)" != "$EMQX_INSTALL_NAME" ]; then
        if [ "$DRY_RUN" = 1 ]; then
            printf '  would atomically link: /opt/emqx -> %s\n' "$EMQX_INSTALL_NAME"
        else
            emqx_link_tmp="/opt/.emqx-link.$$"
            rm -f -- "$emqx_link_tmp"
            ln -s "$EMQX_INSTALL_NAME" "$emqx_link_tmp"
            mv -Tf -- "$emqx_link_tmp" /opt/emqx
        fi
        emqx_changed=1
    fi

    for config_name in emqx.conf base.hocon acl.conf; do
        source_file="$EMQX_CONFIG_DIR/$config_name"
        dest="/etc/emqx/$config_name"
        [ -f "$source_file" ] || die "missing EMQX configuration: $source_file"
        if [ ! -f "$dest" ] || ! cmp -s "$source_file" "$dest"; then
            emqx_changed=1
            if [ -f "$dest" ]; then
                run mkdir -p "$config_backup/etc/emqx"
                run install -Dm644 "$dest" "$config_backup$dest"
            fi
        fi
        run install -Dm644 "$source_file" "$dest"
    done
    run /usr/local/libexec/3tched/emqx-prepare --check-config
    if [ "$emqx_changed" = 1 ]; then
        case " $changed_services " in
            *" emqx "*) ;;
            *) changed_services="$changed_services emqx" ;;
        esac
    fi

    # Stage the pinned local MCP providers atomically. The versioned tree is
    # never overwritten in place; the stable link changes only after the
    # complete archive and expected entry point have been verified.
    install_live_mcp_provider() {
        provider_service=$1
        provider_install_name=$2
        provider_artifact_path=$3
        provider_sha256=$4
        provider_entry=$5
        provider_overlay_source=${6:-}
        provider_overlay_target=${7:-}
        provider_base=/opt/op-mcp-providers
        provider_root="$provider_base/$provider_install_name"
        provider_link="$provider_base/$provider_service"
        provider_marker="$provider_root/.opdbus-artifact-sha256"
        provider_changed=0
        if [ -f "$provider_marker" ] &&
           [ "$(sed -n '1p' "$provider_marker" 2>/dev/null || true)" = "$provider_sha256" ] &&
           [ -n "$provider_overlay_source" ] &&
           ! cmp -s "$provider_overlay_source" "$provider_root/$provider_overlay_target"; then
            die "$provider_service content-addressed overlay does not match its digest marker"
        fi
        if [ ! -f "$provider_marker" ] ||
           [ "$(sed -n '1p' "$provider_marker" 2>/dev/null || true)" != "$provider_sha256" ]; then
            provider_changed=1
            [ ! -e "$provider_root" ] ||
                die "content-addressed provider root has an invalid digest marker: $provider_root"
            if [ "$DRY_RUN" = 1 ]; then
                printf '  would stage: %s -> %s\n' "$provider_artifact_path" "$provider_root"
            else
                provider_stage=$(mktemp -d "$provider_base/.${provider_service}-stage.XXXXXX")
                tar -xzf "$provider_artifact_path" -C "$provider_stage"
                [ -f "$provider_stage/$provider_entry" ] ||
                    die "$provider_service staged tree is missing $provider_entry"
                if [ -n "$provider_overlay_source" ]; then
                    install -Dm644 "$provider_overlay_source" \
                        "$provider_stage/$provider_overlay_target"
                    cmp -s "$provider_overlay_source" \
                        "$provider_stage/$provider_overlay_target" ||
                        die "$provider_service staged overlay verification failed"
                fi
                printf '%s\n' "$provider_sha256" > "$provider_stage/.opdbus-artifact-sha256"
                chmod 0755 "$provider_stage"
                mv -- "$provider_stage" "$provider_root"
            fi
        fi
        run chmod 0755 "$provider_root"
        if [ ! -L "$provider_link" ] ||
           [ "$(readlink "$provider_link" 2>/dev/null || true)" != "$provider_install_name" ]; then
            provider_changed=1
            if [ -e "$provider_link" ] && [ ! -L "$provider_link" ]; then
                run mkdir -p "$provider_base/.opdbus-rollback/$STAMP"
                run mv -- "$provider_link" \
                    "$provider_base/.opdbus-rollback/$STAMP/${provider_service}-previous"
            fi
            if [ "$DRY_RUN" = 1 ]; then
                printf '  would atomically link: %s -> %s\n' \
                    "$provider_link" "$provider_install_name"
            else
                provider_link_tmp="$provider_base/.${provider_service}-link.$$"
                rm -f -- "$provider_link_tmp"
                ln -s "$provider_install_name" "$provider_link_tmp"
                mv -Tf -- "$provider_link_tmp" "$provider_link"
            fi
        fi
        if [ "$provider_changed" = 1 ]; then
            case " $changed_services " in
                *" $provider_service "*) ;;
                *) changed_services="$changed_services $provider_service" ;;
            esac
        fi
    }
    run install -d -m 0755 -o root -g root /opt/op-mcp-providers
    install_live_mcp_provider \
        notebooklm-mcp "$NOTEBOOKLM_MCP_INSTALL_NAME" \
        "$NOTEBOOKLM_MCP_ARTIFACT_PATH" "$NOTEBOOKLM_MCP_DEPLOY_SHA256" \
        node_modules/notebooklm-mcp/dist/index.js \
        "$NOTEBOOKLM_MCP_OVERLAY_SOURCE" "$NOTEBOOKLM_MCP_OVERLAY_TARGET"
    install_live_mcp_provider \
        mongodb-mcp-server "$MONGODB_MCP_INSTALL_NAME" \
        "$MONGODB_MCP_ARTIFACT_PATH" "$MONGODB_MCP_SHA256" \
        node_modules/mongodb-mcp-server/dist/esm/index.js
    run install -d -m 0755 \
        /var/log/runit/notebooklm-mcp \
        /var/log/runit/mongodb-mcp-server
    run install -d -m 0750 -o root -g secrets /etc/opdbus/secrets
    # OIB1 is durable inside the identity Cozo relations. Keep that database
    # root-only; blob-aware clients read only the private tmpfs projection.
    run install -d -m 0700 -o root -g root /var/lib/op-dbus/identity-cozo
    run find /var/lib/op-dbus/identity-cozo -xdev -type d -exec chmod 0700 {} +
    run find /var/lib/op-dbus/identity-cozo -xdev -type f -exec chmod 0600 {} +

    if [ -f "$SCRIPT_DIR/99-systemd-unit-to-runit.hook" ]; then
        run install -d -m 0755 /etc/pacman.d/hooks
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

    # Capability authority is release-owned, principal-only, and materialized
    # into tmpfs by opdbus-grants. Preserve the previous durable document for
    # rollback and restart the materializer when it changes.
    grants_dest="/etc/opdbus/capability-grants.json"
    if [ -f "$GRANTS_SOURCE" ]; then
        grants_changed=0
        if [ ! -f "$grants_dest" ] || ! cmp -s "$GRANTS_SOURCE" "$grants_dest"; then
            grants_changed=1
            if [ -f "$grants_dest" ]; then
                run mkdir -p "$config_backup$(dirname "$grants_dest")"
                run install -Dm600 "$grants_dest" "$config_backup$grants_dest"
            fi
        fi
        run install -Dm600 "$GRANTS_SOURCE" "$grants_dest"
        if [ "$grants_changed" = 1 ]; then
            case " $changed_services " in
                *" opdbus-grants "*) ;;
                *) changed_services="$changed_services opdbus-grants" ;;
            esac
        fi
    fi

    # Projection policy is durable, root-only release content. It contains no
    # credentials or identity assertions, but is protected against live edits
    # because it defines what each authenticated MCP audience can discover.
    for mapping in \
        "$MCP_AUDIENCE_POLICY_SOURCE:/etc/opdbus/mcp-audience-policy.json" \
        "$MCP_TOOLSETS_SOURCE:/etc/opdbus/mcp-toolsets.json"
    do
        source_file=${mapping%%:*}
        dest=${mapping#*:}
        policy_changed=0
        if [ ! -f "$dest" ] || ! cmp -s "$source_file" "$dest"; then
            policy_changed=1
            if [ -f "$dest" ]; then
                run mkdir -p "$config_backup$(dirname "$dest")"
                run install -Dm600 "$dest" "$config_backup$dest"
            fi
        fi
        run install -Dm600 "$source_file" "$dest"
        if [ "$policy_changed" = 1 ]; then
            case " $changed_services " in
                *" op-grpc-bridge "*) ;;
                *) changed_services="$changed_services op-grpc-bridge" ;;
            esac
        fi
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
        [ -f "${svc_dir}check" ] &&
            run install -Dm755 "${svc_dir}check" "$RUNIT_SV_DIR/$svc/check"
        [ -f "${svc_dir}finish" ] &&
            run install -Dm755 "${svc_dir}finish" "$RUNIT_SV_DIR/$svc/finish"
        if [ "$definition_changed" = 1 ] && is_managed_service "$svc"; then
            changed_services="$changed_services $svc"
        fi
    done

    # Enable only explicitly declared new services. Existing enablement is
    # otherwise untouched.
    if [ -f "$ENABLED_SERVICES_FILE" ]; then
        while IFS= read -r svc || [ -n "$svc" ]; do
            case "$svc" in ''|\#*) continue ;; esac
            if [ "$DRY_RUN" = 1 ]; then
                [ -f "$SCRIPT_DIR/$svc/run" ] || die "enabled service has no definition: $svc"
            else
                [ -d "$RUNIT_SV_DIR/$svc" ] || die "enabled service has no definition: $svc"
            fi
            enabled_target="$RUNIT_RUNSVDIR/$svc"
            enable_changed=0
            if [ -e "$enabled_target" ] && [ ! -L "$enabled_target" ]; then
                die "enabled runit service path is not a symlink: $enabled_target"
            fi
            if [ ! -L "$enabled_target" ] ||
               [ "$(readlink "$enabled_target" 2>/dev/null || true)" != "$RUNIT_SV_DIR/$svc" ]; then
                enable_changed=1
                [ ! -L "$enabled_target" ] || run rm -- "$enabled_target"
                run ln -s "$RUNIT_SV_DIR/$svc" "$RUNIT_RUNSVDIR/$svc"
            fi
            if [ "$enable_changed" = 1 ]; then
                case " $changed_services " in
                    *" $svc "*) ;;
                    *) changed_services="$changed_services $svc" ;;
                esac
            fi
        done < "$ENABLED_SERVICES_FILE"
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
        if [ "$DRY_RUN" != 1 ]; then
            ensure_enabled_service_ready op-runit-systemctl runit_manager_ready
            ensure_enabled_service_ready notebooklm-mcp notebooklm_mcp_ready
            ensure_enabled_service_ready mongodb-mcp-server mongodb_mcp_ready
            ensure_enabled_service_ready op-grpc-bridge bridge_ready
            ensure_enabled_service_ready emqx emqx_ready
        fi
        return 0
    fi

    # Restart enabled services whose release-owned run definition or binary
    # changed. Network/session-bus carriers remain held back below.
    restart_list=""
    held_back=""
    for svc in $changed_services; do
        if [ "$DRY_RUN" = 1 ]; then
            [ -e "$RUNIT_RUNSVDIR/$svc" ] || is_enabled_service "$svc" || continue
        else
            [ -e "$RUNIT_RUNSVDIR/$svc" ] || [ -L "$RUNIT_RUNSVDIR/$svc" ] || continue
        fi
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

    # A prior interrupted publish may have installed content before restarting
    # its consumer. Queue any unhealthy release dependency so an idempotent
    # rerun completes the cutover instead of merely reporting stale state.
    for readiness in \
        op-runit-systemctl:runit_manager_ready \
        notebooklm-mcp:notebooklm_mcp_ready \
        mongodb-mcp-server:mongodb_mcp_ready \
        op-grpc-bridge:bridge_ready \
        emqx:emqx_ready
    do
        svc=${readiness%%:*}
        ready_fn=${readiness#*:}
        [ -e "$RUNIT_RUNSVDIR/$svc" ] || [ -L "$RUNIT_RUNSVDIR/$svc" ] || continue
        "$ready_fn" && continue
        case " $restart_list " in
            *" $svc "*) ;;
            *) restart_list="$restart_list $svc" ;;
        esac
    done

    if [ -z "$restart_list" ]; then
        log "no enabled service references the changed binaries"
        return 0
    fi

    # Dependency order is explicit: local WARM providers become healthy before
    # the bridge projects them, and EMQX registers after the bridge's ExHook
    # listener is ready.
    ordered_restart_list=""
    for svc in op-runit-systemctl notebooklm-mcp mongodb-mcp-server op-grpc-bridge emqx $restart_list; do
        case " $restart_list " in *" $svc "*) ;; *) continue ;; esac
        case " $ordered_restart_list " in
            *" $svc "*) ;;
            *) ordered_restart_list="$ordered_restart_list $svc" ;;
        esac
    done
    restart_list=$ordered_restart_list

    log "restarting:$restart_list"
    for svc in $restart_list; do
        if [ "$DRY_RUN" != 1 ]; then
            wait_runit_supervision "$svc" ||
                die "$svc was enabled but runsv did not establish supervision"
        fi
        if [ "$svc" = op-grpc-bridge ]; then
            if [ "$DRY_RUN" = 1 ]; then
                run sv restart "$svc"
                continue
            fi
            wait_legacy_8090_relays_down
            sv restart "$svc" || die "sv restart $svc failed"
            wait_bridge_ready || die "op-grpc-bridge failed direct-bind readiness"
            ok "op-grpc-bridge ready on :8090 plus loopback-mTLS ExHook :9000"
        elif [ "$svc" = op-runit-systemctl ]; then
            run sv restart "$svc" || die "sv restart $svc failed"
            if [ "$DRY_RUN" != 1 ]; then
                wait_service_ready runit_manager_ready || die "op-runit-systemctl failed D-Bus readiness"
                ok "op-runit-systemctl ready on the system bus"
            fi
        elif [ "$svc" = emqx ]; then
            run sv restart "$svc" || die "sv restart $svc failed"
            if [ "$DRY_RUN" != 1 ]; then
                wait_service_ready emqx_ready || die "standalone EMQX failed loopback readiness"
                ok "standalone EMQX ready on loopback MQTT and management listeners"
            fi
        elif [ "$svc" = notebooklm-mcp ]; then
            run sv restart "$svc" || die "sv restart $svc failed"
            if [ "$DRY_RUN" != 1 ]; then
                wait_service_ready notebooklm_mcp_ready ||
                    die "NotebookLM MCP provider failed loopback readiness"
                ok "NotebookLM MCP provider ready on loopback :3101"
            fi
        elif [ "$svc" = mongodb-mcp-server ]; then
            run sv restart "$svc" || die "sv restart $svc failed"
            if [ "$DRY_RUN" != 1 ]; then
                wait_service_ready mongodb_mcp_ready ||
                    die "MongoDB MCP provider failed loopback readiness"
                ok "MongoDB MCP provider ready read-only on loopback :3102"
            fi
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
