#!/usr/bin/env bash
# PoC bash completion adapter for op-inspector's IntrospectiveGadget.
#
# Purpose: prove out a completion strategy that handles non-uniform argument
# shapes, using op-inspector as the test subject because its own schema is
# deliberately non-uniform in a way zcall's arg_flags_for() cannot currently
# handle (crates/op-plugins/../zcall or bin/zcall's `--complete args`, which
# dead-ends whenever a method's schema isn't a flat {"properties": {...}}
# object — see crates/op-inspector/src/introspective_gadget.rs for the real
# shapes this PoC models):
#
#   inspect_object(InspectionInput)
#     InspectionInput { source: InspectionSource, data: Option<String>, metadata: HashMap<String,String> }
#     InspectionSource is a 4-variant enum, each variant a DIFFERENT shape:
#       File(String)                              -- single positional value
#       Url(String)                                -- single positional value
#       DockerContainer(String)                    -- single positional value
#       RawData { format_hint: Option<String>, description: String }  -- 2-field struct
#
#   inspect_docker_container(container_name: &str)  -- single positional value
#   inspect_xml_data(...)                            -- see source for exact signature
#   inspect_legacy_data(...)                         -- see source for exact signature
#
# Strategy that avoids the dead-end: instead of assuming every method/variant
# has a flat `properties` dict (zcall's assumption), this table declares each
# leaf's *shape kind* explicitly (positional | struct | enum) and dispatches
# completion generation per-kind. An enum variant is completed as a nested
# selection: first complete the variant name, THEN re-dispatch completion
# using that variant's own shape (which may itself be positional or struct).
# This composes correctly for arbitrary non-uniform nesting, which a single
# "flatten to properties" pass cannot.

_op_inspector_methods() {
    printf '%s\n' inspect_object inspect_docker_container inspect_xml_data inspect_legacy_data
}

# Declares the shape kind for each method. This is the piece zcall's
# arg_flags_for() is missing: a per-method (or per-variant) shape descriptor,
# rather than assuming one uniform "properties" dict shape for everything.
_op_inspector_shape_kind() {
    case "$1" in
        inspect_object) printf 'enum_wrapped_struct\n' ;;          # takes InspectionInput, whose `source` field is itself an enum
        inspect_docker_container) printf 'positional\n' ;;         # single &str arg
        inspect_xml_data) printf 'unknown\n' ;;                    # not modeled in this PoC; real adapter would introspect the source file
        inspect_legacy_data) printf 'unknown\n' ;;
        *) printf 'unknown\n' ;;
    esac
}

# InspectionSource's 4 variants and each variant's OWN shape kind.
_op_inspector_source_variants() {
    printf '%s\n' File Url DockerContainer RawData
}

_op_inspector_variant_shape_kind() {
    case "$1" in
        File|Url|DockerContainer) printf 'positional\n' ;;   # tuple variant, single value
        RawData) printf 'struct\n' ;;                        # struct variant, named fields
        *) printf 'unknown\n' ;;
    esac
}

# RawData's own fields (a plain struct shape, this part DOES look like
# zcall's flat-properties assumption -- proving the fix isn't "throw away
# flat-properties completion," it's "don't assume it's the ONLY shape.")
_op_inspector_rawdata_fields() {
    printf -- '--format-hint\n--description\n'
}

_op_inspector_complete() {
    # Self-contained: no dependency on the bash-completion package's
    # _init_completion helper (not installed on every target host).
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local words=("${COMP_WORDS[@]}")
    local cword="${COMP_CWORD}"

    local method="${words[1]:-}"

    # Word 1: complete the method/subcommand name.
    if ((cword == 1)); then
        COMPREPLY=($(compgen -W "$(_op_inspector_methods)" -- "$cur"))
        return
    fi

    local kind
    kind="$(_op_inspector_shape_kind "$method")"

    case "$kind" in
        positional)
            # e.g. inspect_docker_container <container_name> -- nothing to
            # complete but the value itself; correctly return empty rather
            # than silently offering nothing that looks like a dead end
            # (zcall's failure mode looks identical to "no more args needed"
            #  even when it actually means "couldn't figure out the shape" --
            #  this PoC keeps those two cases visibly distinct in comments/
            #  --debug output, see below).
            COMPREPLY=()
            return
            ;;
        enum_wrapped_struct)
            # word 2 is the InspectionSource variant name.
            if ((cword == 2)); then
                COMPREPLY=($(compgen -W "$(_op_inspector_source_variants)" -- "$cur"))
                return
            fi
            # word 3+: re-dispatch based on the CHOSEN variant's own shape.
            local variant="${words[2]:-}"
            local variant_kind
            variant_kind="$(_op_inspector_variant_shape_kind "$variant")"
            case "$variant_kind" in
                positional)
                    COMPREPLY=()  # File/Url/DockerContainer just take one value
                    ;;
                struct)
                    COMPREPLY=($(compgen -W "$(_op_inspector_rawdata_fields)" -- "$cur"))
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            return
            ;;
        unknown)
            # Explicit, not silent: distinguishable from "nothing more to
            # complete" if this script is run with OP_INSPECTOR_COMPLETE_DEBUG=1.
            if [[ -n "${OP_INSPECTOR_COMPLETE_DEBUG:-}" ]]; then
                printf '\n[completion adapter: no shape descriptor for "%s" -- add one to _op_inspector_shape_kind instead of silently returning empty]\n' "$method" >&2
            fi
            COMPREPLY=()
            return
            ;;
    esac
}

complete -F _op_inspector_complete op-inspector 2>/dev/null || true

# --- Standalone smoke test (run this file directly, not sourced, to exercise
# the dispatch logic without a real terminal/readline session) ---
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "method shape kinds:"
    for m in $(_op_inspector_methods); do
        printf '  %-28s %s\n' "$m" "$(_op_inspector_shape_kind "$m")"
    done
    echo
    echo "InspectionSource variant shape kinds:"
    for v in $(_op_inspector_source_variants); do
        printf '  %-16s %s\n' "$v" "$(_op_inspector_variant_shape_kind "$v")"
    done
    echo
    echo "RawData struct fields:"
    _op_inspector_rawdata_fields | sed 's/^/  /'
fi
