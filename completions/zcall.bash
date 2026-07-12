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

    # Handle completion values for global flags that require arguments
    case "$prev" in
        --source)
            COMPREPLY=( $(compgen -W "blob auto schema dbus" -- "$cur") )
            return 0
            ;;
        --arguments|-a|--capability|-c|--actor|--object|--interface|--endpoint|--blob-dir|--timeout)
            return 0
            ;;
    esac

    # Global options list
    local global_options="--arguments -a --capability -c --actor --object --interface --endpoint --timeout --blob-dir --source --print --dry-run --show-headers --help -h"
    local builtins="list methods help check-catalog doctor expand"

    # Find the index of the first word that is not a global option or option value
    local cmd_idx=1
    while (( cmd_idx < cword )); do
        case "${words[cmd_idx]}" in
            --arguments|-a|--capability|-c|--actor|--object|--interface|--endpoint|--blob-dir|--timeout|--source)
                ((cmd_idx += 2))
                ;;
            --print|--dry-run|--show-headers|--help|-h)
                ((cmd_idx += 1))
                ;;
            -*)
                # Unknown option? skip
                ((cmd_idx += 1))
                ;;
            *)
                # Found the command/plugin!
                break
                ;;
        esac
    done

    # If the user is completing a word before specifying the command/plugin
    if (( cmd_idx >= cword )); then
        if [[ "$cur" == -* ]]; then
            COMPREPLY=( $(compgen -W "$global_options" -- "$cur") )
        else
            COMPREPLY=( $(compgen -W "$builtins $(_zcall_complete_words plugins)" -- "$cur") )
        fi
        return 0
    fi

    # We found the command/plugin at words[cmd_idx]
    local first="${words[cmd_idx]}"
    local relative_cword=$(( cword - cmd_idx ))

    case "$first" in
        methods|help)
            if (( relative_cword == 1 )); then
                COMPREPLY=( $(compgen -W "$(_zcall_complete_words plugins)" -- "$cur") )
            elif (( relative_cword == 2 )); then
                COMPREPLY=( $(compgen -W "$(_zcall_complete_words methods "${words[cmd_idx+1]}")" -- "$cur") )
            fi
            return 0
            ;;
        list|check-catalog|doctor)
            return 0
            ;;
        expand)
            if (( relative_cword == 1 )); then
                COMPREPLY=( $(compgen -W "$(_zcall_complete_words plugins)" -- "$cur") )
            elif (( relative_cword == 2 )); then
                COMPREPLY=( $(compgen -W "$(_zcall_complete_words methods "${words[cmd_idx+1]}")" -- "$cur") )
            else
                COMPREPLY=( $(compgen -W "$(_zcall_complete_words args "${words[cmd_idx+1]}" "${words[cmd_idx+2]}")" -- "$cur") )
            fi
            return 0
            ;;
    esac

    # If the first non-option word was not a builtin, it is a plugin!
    local plugin="$first"
    if (( relative_cword == 1 )); then
        COMPREPLY=( $(compgen -W "$(_zcall_complete_words methods "$plugin")" -- "$cur") )
    else
        local method="${words[cmd_idx+1]}"
        # Only suggest method-specific flags, no global options!
        COMPREPLY=( $(compgen -W "$(_zcall_complete_words args "$plugin" "$method")" -- "$cur") )
    fi
}

complete -F _zcall zcall
