#!/bin/sh

set -eu

check_file() {
    file="$1"

    if [ ! -f "$file" ]; then
        printf 'missing file: %s\n' "$file" >&2
        return 1
    fi

    if ! grep -Eq 'upstream[[:space:]]+op_dbus_grpc_backend' "$file"; then
        printf 'missing gRPC upstream in %s\n' "$file" >&2
        return 1
    fi

    if ! grep -Eq 'location[[:space:]]+~[[:space:]]+\^/operation\\\.' "$file"; then
        printf 'missing gRPC-Web route in %s\n' "$file" >&2
        return 1
    fi

    if ! grep -Eq 'proxy_pass[[:space:]]+http://op_dbus_grpc_backend' "$file"; then
        printf 'missing gRPC proxy target in %s\n' "$file" >&2
        return 1
    fi
}

project_root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)

check_file "$project_root/deploy/lib/nginx.sh"
check_file "$project_root/deploy/nginx/op-web.conf"
check_file "$project_root/deploy/nginx/op-web-3etched.com.conf"
check_file "$project_root/deploy/nginx/ghostbridge-public.conf"
check_file "$project_root/deploy/deploy-local-tls.sh"
check_file "$project_root/deploy/setup-complete.sh"

printf 'gRPC-Web nginx routes verified\n'