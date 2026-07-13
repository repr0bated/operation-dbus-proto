#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
zcall_bin="$repo_root/bin/zcall"

request_json() {
    "$zcall_bin" "$@" | sed -n '/^{/,$p'
}

expanded_command() {
    "$zcall_bin" "$@" | sed -n '1p'
}

assert_request() {
    local name="$1"
    local filter="$2"
    shift 2

    if ! request_json "$@" | jq -e "$filter" >/dev/null; then
        printf 'FAIL %s\n' "$name" >&2
        request_json "$@" >&2
        exit 1
    fi
    printf 'PASS %s\n' "$name"
}

assert_request \
    "global call options after method" \
    '. == {}' \
    identity-sled get-identity \
    --arguments '{}' \
    --capability test.read \
    --actor audit \
    --dry-run

assert_request \
    "method argument after method" \
    '. == {"interface":"netmaker"}' \
    wireguard get-device \
    --interface netmaker \
    --dry-run

if ! expanded_command rovs-commands list-bridges --dry-run \
    | rg -Fq 'operation.plugin.v1.RovsCommandsPluginMethods/ListBridges'; then
    printf 'FAIL direct typed gRPC service\n' >&2
    exit 1
fi
printf 'PASS direct typed gRPC service\n'
