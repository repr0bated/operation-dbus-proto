_zcall_complete_words() {
    local mode="$1"
    local plugin="${2:-}"
    local method="${3:-}"
    local zcall_bin
    zcall_bin="$(command -v zcall 2>/dev/null || true)"
    if [[ -z "$zcall_bin" && -x ./bin/zcall ]]; then
        zcall_bin="./bin/zcall"
    fi
    [[ -n "$zcall_bin" ]] || return 0
    if [[ "$mode" == "plugins" ]]; then
        "$zcall_bin" --complete plugins 2>/dev/null
    elif [[ "$mode" == "methods" ]]; then
        "$zcall_bin" --complete methods "$plugin" 2>/dev/null
    else
        "$zcall_bin" --complete args "$plugin" "$method" 2>/dev/null
    fi
}

_zcall() {
    local cur prev words cword
    COMPREPLY=()
    _get_comp_words_by_ref -n : cur prev words cword 2>/dev/null || {
        cur="${COMP_WORDS[COMP_CWORD]}"
        prev="${COMP_WORDS[COMP_CWORD-1]}"
        words=("${COMP_WORDS[@]}")
        cword="$COMP_CWORD"
    }

    case "$prev" in
        --source)
            COMPREPLY=( $(compgen -W "blob auto schema dbus" -- "$cur") )
            return 0
            ;;
        --arguments|-a|--capability|-c|--actor|--object|--interface|--endpoint|--blob-dir)
            return 0
            ;;
    esac

    local first="${words[1]:-}"
    case "$first" in
        expand)
            if (( cword == 2 )); then
                COMPREPLY=( $(compgen -W "$(_zcall_complete_words plugins)" -- "$cur") )
            elif (( cword == 3 )); then
                COMPREPLY=( $(compgen -W "$(_zcall_complete_words methods "${words[2]}")" -- "$cur") )
            else
                COMPREPLY=( $(compgen -W "$(_zcall_complete_words args "${words[2]}" "${words[3]}")" -- "$cur") )
            fi
            return 0
            ;;
    esac

    if (( cword == 1 )); then
        COMPREPLY=( $(compgen -W "$(_zcall_complete_words plugins)" -- "$cur") )
    elif (( cword == 2 )); then
        COMPREPLY=( $(compgen -W "$(_zcall_complete_words methods "$first")" -- "$cur") )
    elif [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "$(_zcall_complete_words args "$first" "${words[2]}")" -- "$cur") )
    else
        COMPREPLY=( $(compgen -W "$(_zcall_complete_words args "$first" "${words[2]}")" -- "$cur") )
    fi
}

complete -F _zcall zcall
