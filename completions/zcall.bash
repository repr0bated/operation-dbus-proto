# bash completion for zcall.
#
# Candidates come from zcall itself, which reads the live D-Bus tree
# (org.opdbus.v1.plugins on unix:path=/run/opdbus/session-bus.sock) for plugin
# objects and the sealed blob catalog schema for declared methods and args.

_zcall_bin() {
    local bin
    bin="$(command -v zcall 2>/dev/null || true)"
    if [[ -z "$bin" && -x ./bin/zcall ]]; then
        bin="./bin/zcall"
    fi
    printf '%s' "$bin"
}

_zcall_complete_words() {
    local mode="$1"
    local plugin="${2:-}"
    local method="${3:-}"
    local bin
    bin="$(_zcall_bin)"
    [[ -n "$bin" ]] || return 0
    case "$mode" in
        plugins | subcommands) "$bin" --complete "$mode" 2>/dev/null ;;
        methods) "$bin" --complete methods "$plugin" 2>/dev/null ;;
        args) "$bin" --complete args "$plugin" "$method" 2>/dev/null ;;
    esac
}

_zcall() {
    local cur prev words cword
    COMPREPLY=()
    _get_comp_words_by_ref -n : cur prev words cword 2>/dev/null || {
        cur="${COMP_WORDS[COMP_CWORD]}"
        prev="${COMP_WORDS[COMP_CWORD - 1]}"
        words=("${COMP_WORDS[@]}")
        cword="$COMP_CWORD"
    }

    case "$prev" in
        --address | --blob-dir | --arguments | -a)
            return 0
            ;;
    esac

    local base=1
    case "${words[1]:-}" in
        expand)
            base=2
            ;;
        tree | list)
            return 0
            ;;
        introspect | methods)
            if ((cword == 2)); then
                mapfile -t COMPREPLY < <(compgen -W "$(_zcall_complete_words plugins)" -- "$cur")
            fi
            return 0
            ;;
        help)
            if ((cword == 2)); then
                mapfile -t COMPREPLY < <(compgen -W "$(_zcall_complete_words plugins)" -- "$cur")
            elif ((cword == 3)); then
                mapfile -t COMPREPLY < <(compgen -W "$(_zcall_complete_words methods "${words[2]}")" -- "$cur")
            fi
            return 0
            ;;
        props | get | set)
            if ((cword == 2)); then
                mapfile -t COMPREPLY < <(compgen -W "$(_zcall_complete_words plugins)" -- "$cur")
            fi
            return 0
            ;;
    esac

    local plugin_pos=$base
    local method_pos=$((base + 1))

    if ((cword == plugin_pos)); then
        local candidates
        candidates="$(_zcall_complete_words plugins)"
        if ((base == 1)); then
            candidates+=$'\n'"$(_zcall_complete_words subcommands)"
        fi
        mapfile -t COMPREPLY < <(compgen -W "$candidates" -- "$cur")
    elif ((cword == method_pos)); then
        mapfile -t COMPREPLY < <(compgen -W "$(_zcall_complete_words methods "${words[plugin_pos]}")" -- "$cur")
    else
        mapfile -t COMPREPLY < <(compgen -W \
            "$(_zcall_complete_words args "${words[plugin_pos]}" "${words[method_pos]}")" -- "$cur")
    fi
}

complete -F _zcall zcall
