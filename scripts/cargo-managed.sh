#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TARGET_ROOT="${OP_DBUS_LOCAL_TARGET_ROOT:-${PROJECT_ROOT}/target-cache}"
KEEP_COUNT="${OP_DBUS_TARGET_RETENTION_COUNT:-3}"
BUILD_SESSION_ID="$(date +%Y%m%d%H%M%S)"
TARGET_DIR="${TARGET_ROOT}/build-${BUILD_SESSION_ID}"

cleanup_legacy_cargo_target_layout() {
    local root="$1"
    local legacy_entries=(
        "build"
        "deps"
        "examples"
        "incremental"
        "release"
        ".fingerprint"
        ".rustc_info.json"
        "CACHEDIR.TAG"
    )
    local entry

    for entry in "${legacy_entries[@]}"; do
        if [[ -e "${root}/${entry}" ]]; then
            echo "[cargo-managed] removing legacy flat cargo cache layout in ${root}" >&2
            rm -rf \
                "${root}/build" \
                "${root}/deps" \
                "${root}/examples" \
                "${root}/incremental" \
                "${root}/release" \
                "${root}/.fingerprint" \
                "${root}/.rustc_info.json" \
                "${root}/CACHEDIR.TAG"
            break
        fi
    done
}

prune_old_target_dirs() {
    local root="$1"
    local keep_count="$2"
    local dirs=()
    local dir

    while IFS= read -r dir; do
        dirs+=("$dir")
    done < <(find "$root" -mindepth 1 -maxdepth 1 -type d -name 'build-*' | sort)

    while (( ${#dirs[@]} > keep_count )); do
        echo "[cargo-managed] pruning old cargo target cache ${dirs[0]}" >&2
        rm -rf "${dirs[0]}"
        dirs=("${dirs[@]:1}")
    done
}

mkdir -p "$TARGET_ROOT"
cleanup_legacy_cargo_target_layout "$TARGET_ROOT"
mkdir -p "$TARGET_DIR"

echo "[cargo-managed] CARGO_TARGET_DIR=${TARGET_DIR}" >&2
CARGO_TARGET_DIR="$TARGET_DIR" cargo "$@"
prune_old_target_dirs "$TARGET_ROOT" "$KEEP_COUNT"

