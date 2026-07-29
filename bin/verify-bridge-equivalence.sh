#!/usr/bin/env bash
# verify-bridge-equivalence.sh — prove the bridge path returns the same shape as the
# deprecated direct :3003 listener, for the cognitive_mcp tool surface.
#
# Phase 1 of .kiro/specs/cognitive-mcp-bridge-only-door. Both paths are alive during
# Phase 1 by design, which is what makes this comparison possible. Run it before and
# after the cutover; the "after" run is the oracle that the bridge is a faithful
# replacement.
#
# Compares JSON *shape* (key sets and value types), not values: several tools are
# time- or state-dependent, so equal values are not expected.
#
# jq only — no Python (NFR-4).

set -uo pipefail

BRIDGE_CALL="${BRIDGE_CALL:-./bin/zcall}"
MCP_URL="${COGNITIVE_MCP_MCP_URL:-http://10.200.0.2:3003/mcp}"
FOOTPRINT="${GHOSTBRIDGE_FOOTPRINT:-verify-equivalence}"
TRACE="${GHOSTBRIDGE_TRACE_ID:-verify-equivalence}"

for dep in jq curl; do
    command -v "$dep" >/dev/null 2>&1 || { echo "FATAL: $dep is required" >&2; exit 2; }
done
[ -x "$BRIDGE_CALL" ] || { echo "FATAL: $BRIDGE_CALL not executable" >&2; exit 2; }

pass=0; fail=0; skip=0

# Reduce a JSON document to a sorted list of "path:type" pairs. Two documents with
# the same shape produce identical output regardless of scalar values or key order.
shape() {
    jq -S 'def walk_shape($p):
             if   type == "object" then
                    if (keys|length) == 0 then ["\($p):object{}"]
                    else [ to_entries[] | .key as $k | (.value | walk_shape("\($p).\($k)")) ] | flatten
                    end
             elif type == "array"  then
                    if length == 0 then ["\($p):array[]"]
                    else (.[0] | walk_shape("\($p)[]"))
                    end
             else ["\($p):\(type)"]
             end;
           walk_shape("$") | sort | unique' 2>/dev/null
}

# Invoke one tool through the bridge and return just the tool result.
via_bridge() {
    local tool="$1" args="$2"
    "$BRIDGE_CALL" cognitive_mcp invoke_tool \
        -a "$(jq -nc --arg t "$tool" --argjson a "$args" '{tool_name:$t, arguments:$a}')" \
        2>/dev/null | jq -c '.result // empty' 2>/dev/null
}

# Invoke the same tool through the deprecated direct listener.
via_direct() {
    local tool="$1" args="$2"
    curl -s -m 30 "$MCP_URL" \
        -H 'Content-Type: application/json' \
        -H "X-Ghostbridge-Footprint: $FOOTPRINT" \
        -H "X-Ghostbridge-Trace-ID: $TRACE" \
        -d "$(jq -nc --arg t "$tool" --argjson a "$args" \
              '{jsonrpc:"2.0", id:1, method:"tools/call", params:{name:$t, arguments:$a}}')" \
        2>/dev/null | jq -c '.result // empty' 2>/dev/null
}

compare_tool() {
    local tool="$1" args="$2"
    printf '%-34s ' "$tool"

    local b d
    b="$(via_bridge "$tool" "$args")"
    d="$(via_direct "$tool" "$args")"

    if [ -z "$d" ]; then
        echo "SKIP  (direct path returned nothing; cannot form an oracle)"
        skip=$((skip+1)); return
    fi
    if [ -z "$b" ]; then
        echo "FAIL  (bridge returned nothing, direct returned data)"
        fail=$((fail+1)); return
    fi

    local bs ds
    bs="$(printf '%s' "$b" | shape)"
    ds="$(printf '%s' "$d" | shape)"

    if [ -z "$bs" ] || [ -z "$ds" ]; then
        echo "SKIP  (shape could not be derived)"
        skip=$((skip+1)); return
    fi

    if [ "$bs" = "$ds" ]; then
        echo "PASS"
        pass=$((pass+1))
    else
        echo "FAIL  (shape differs)"
        diff <(printf '%s\n' "$ds") <(printf '%s\n' "$bs") \
            | sed 's/^/      /' | head -20
        fail=$((fail+1))
    fi
}

echo "bridge : $BRIDGE_CALL cognitive_mcp invoke_tool"
echo "direct : $MCP_URL"
echo

# Read-only, side-effect-free tools. Deliberately excludes anything that mutates
# state or shells out (e.g. agent_shell_executor_exec).
compare_tool cognitive_memory '{"operation":"list_namespaces"}'
compare_tool get_health       '{}'
compare_tool blob_catalog     '{}'
compare_tool dbus_list_namespaces '{}'

echo
echo "pass=$pass fail=$fail skip=$skip"
[ "$fail" -eq 0 ] || exit 1
