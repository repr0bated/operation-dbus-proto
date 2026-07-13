#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$repo_root/bin:$PATH"

source "$repo_root/completions/zcall.bash"

get_completions() {
    local line="$1"
    local -a words=()

    COMP_LINE="$line"
    COMP_POINT=${#COMP_LINE}

    # shellcheck disable=SC2206
    words=($line)
    if [[ "${line: -1}" == " " ]]; then
        words+=("")
    fi

    COMP_WORDS=("${words[@]}")
    COMP_CWORD=$((${#COMP_WORDS[@]} - 1))
    COMPREPLY=()
    _zcall
    printf '%s\n' "${COMPREPLY[@]}" | LC_ALL=C sort
}

assert_lines() {
    local name="$1"
    local actual="$2"
    local expected="$3"

    if [[ "$actual" != "$expected" ]]; then
        printf 'FAIL %s\nexpected:\n%s\nactual:\n%s\n' "$name" "$expected" "$actual" >&2
        exit 1
    fi
    printf 'PASS %s\n' "$name"
}

assert_contains_only_prefix() {
    local name="$1"
    local actual="$2"
    local expected="$3"

    assert_lines "$name" "$actual" "$expected"
}

assert_contains_only_prefix \
    "top-level plugins" \
    "$(get_completions 'zcall ')" \
    "$( (echo "list"; echo "methods"; echo "help"; echo "check-catalog"; echo "doctor"; echo "expand"; zcall --complete plugins) | LC_ALL=C sort )"

assert_lines \
    "top-level options" \
    "$(get_completions 'zcall -')" \
    $'--actor\n--arguments\n--blob-dir\n--capability\n--dry-run\n--endpoint\n--help\n--print\n--show-headers\n--source\n--timeout\n-a\n-c\n-h'


assert_contains_only_prefix \
    "plugin prefix a" \
    "$(get_completions 'zcall a')" \
    "agent-config"

assert_lines \
    "unix-socket methods" \
    "$(get_completions 'zcall unix-socket ')" \
    $'accept\nbind\nclose\nlisten'

assert_lines \
    "bind args" \
    "$(get_completions 'zcall unix-socket bind ')" \
    $'--name\n--path\n--ports\n--protocol'

assert_lines \
    "bind arg prefix" \
    "$(get_completions 'zcall unix-socket bind --p')" \
    $'--path\n--ports\n--protocol'
