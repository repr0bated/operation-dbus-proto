#!/bin/sh
# Non-secret regression test for the runit-owned NotebookLM MCP singleton.
# Point NOTEBOOKLM_PROVIDER_ROOT at an extracted provider tree with the tracked
# transport overlay applied. The test uses an isolated HOME and loopback only.
set -eu

PROVIDER_ROOT=${NOTEBOOKLM_PROVIDER_ROOT:-/opt/op-mcp-providers/notebooklm-mcp}
PROVIDER_ENTRY="$PROVIDER_ROOT/node_modules/notebooklm-mcp/dist/index.js"
NODE_BINARY=${NODE_BINARY:-/usr/bin/node}
TEST_PORT=${NOTEBOOKLM_TEST_PORT:-39101}
TEST_URL="http://127.0.0.1:$TEST_PORT"

[ -f "$PROVIDER_ENTRY" ] || {
    echo "notebooklm singleton test: provider entry is missing: $PROVIDER_ENTRY" >&2
    exit 1
}
[ -x "$NODE_BINARY" ] || {
    echo "notebooklm singleton test: node is missing: $NODE_BINARY" >&2
    exit 1
}
case "$TEST_PORT" in
    ''|*[!0-9]*)
        echo "notebooklm singleton test: NOTEBOOKLM_TEST_PORT must be numeric" >&2
        exit 1
        ;;
esac

if ss -H -ltn | awk -v endpoint="127.0.0.1:$TEST_PORT" \
    '$4 == endpoint { found = 1 } END { exit(found ? 0 : 1) }'; then
    echo "notebooklm singleton test: loopback port $TEST_PORT is already in use" >&2
    exit 1
fi

test_root=$(mktemp -d)
provider_pid=0
cleanup() {
    if [ "$provider_pid" -gt 0 ]; then
        kill -TERM "$provider_pid" 2>/dev/null || true
        wait "$provider_pid" 2>/dev/null || true
    fi
    rm -r -- "$test_root"
}
trap cleanup HUP INT TERM EXIT

mkdir -p "$test_root/home" "$test_root/config" "$test_root/data"
initialize_body='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"op-dbus-mcp-aggregator","version":"0.1.0"}}}'
foreign_initialize_body='{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"unexpected-loopback-probe","version":"1"}}}'

start_provider() {
    HOME="$test_root/home" \
    XDG_CONFIG_HOME="$test_root/config" \
    XDG_DATA_HOME="$test_root/data" \
    NOTEBOOKLM_TRANSPORT=http \
    NOTEBOOKLM_HOST=127.0.0.1 \
    NOTEBOOKLM_PORT="$TEST_PORT" \
    NOTEBOOKLM_PROFILE=standard \
    NOTEBOOK_PROFILE_STRATEGY=auto \
        "$NODE_BINARY" "$PROVIDER_ENTRY" \
        --transport http --host 127.0.0.1 --port "$TEST_PORT" \
        >"$test_root/provider.log" 2>&1 &
    provider_pid=$!

    attempt=0
    until curl -fsS --max-time 1 "$TEST_URL/healthz" >/dev/null 2>&1; do
        attempt=$((attempt + 1))
        if ! kill -0 "$provider_pid" 2>/dev/null; then
            echo "notebooklm singleton test: provider exited before health" >&2
            return 1
        fi
        [ "$attempt" -lt 100 ] || {
            echo "notebooklm singleton test: provider health timed out" >&2
            return 1
        }
        sleep 0.1
    done
}

initialize_session() {
    label=$1
    headers="$test_root/$label.headers"
    body="$test_root/$label.body"
    status=$(curl -sS --max-time 10 -D "$headers" -o "$body" -w '%{http_code}' \
        -H 'content-type: application/json' \
        -H 'accept: application/json, text/event-stream' \
        -H 'mcp-protocol-version: 2025-03-26' \
        --data "$initialize_body" "$TEST_URL/mcp")
    [ "$status" = 200 ] || {
        echo "notebooklm singleton test: $label initialize returned HTTP $status" >&2
        return 1
    }
    awk 'tolower($1) == "mcp-session-id:" { gsub("\\r", "", $2); print $2; exit }' "$headers"
}

start_provider
first_session=$(initialize_session first)
[ -n "$first_session" ] || {
    echo "notebooklm singleton test: first initialize returned no session id" >&2
    exit 1
}

foreign_status=$(curl -sS --max-time 10 -o "$test_root/foreign.body" -w '%{http_code}' \
    -H 'content-type: application/json' \
    -H 'accept: application/json, text/event-stream' \
    -H 'mcp-protocol-version: 2025-03-26' \
    --data "$foreign_initialize_body" "$TEST_URL/mcp")
[ "$foreign_status" = 409 ] || {
    echo "notebooklm singleton test: foreign initialize returned HTTP $foreign_status, expected 409" >&2
    exit 1
}
"$NODE_BINARY" -e '
const fs = require("node:fs");
const value = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
if (value?.error?.code !== -32002) process.exit(1);
' "$test_root/foreign.body" || {
    echo "notebooklm singleton test: foreign initialize returned the wrong MCP error" >&2
    exit 1
}

# A fresh initialize from the canonical bridge owner represents recovery after
# a bridge crash or lost response.  It must request a clean runit restart
# instead of leaving the old singleton wedged forever.
replacement_status=$(curl -sS --max-time 10 -o "$test_root/stale.body" -w '%{http_code}' \
    -H 'content-type: application/json' \
    -H 'accept: application/json, text/event-stream' \
    -H 'mcp-protocol-version: 2025-03-26' \
    --data "$initialize_body" "$TEST_URL/mcp")
[ "$replacement_status" = 503 ] || {
    echo "notebooklm singleton test: stale-owner replacement returned HTTP $replacement_status, expected 503" >&2
    exit 1
}
"$NODE_BINARY" -e '
const fs = require("node:fs");
const value = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
if (value?.error?.code !== -32003) process.exit(1);
' "$test_root/stale.body" || {
    echo "notebooklm singleton test: stale-owner replacement returned the wrong MCP error" >&2
    exit 1
}

attempt=0
while kill -0 "$provider_pid" 2>/dev/null; do
    attempt=$((attempt + 1))
    [ "$attempt" -lt 100 ] || {
        echo "notebooklm singleton test: provider did not exit for stale-owner recovery" >&2
        exit 1
    }
    sleep 0.1
done
wait "$provider_pid"
provider_pid=0

# Model runit's restart: a fresh process must accept a fresh owner session.
start_provider
first_session=$(initialize_session restarted)
[ -n "$first_session" ] || {
    echo "notebooklm singleton test: restarted initialize returned no session id" >&2
    exit 1
}

delete_status=$(curl -sS --max-time 10 -o /dev/null -w '%{http_code}' \
    -X DELETE \
    -H "mcp-session-id: $first_session" \
    "$TEST_URL/mcp")
[ "$delete_status" = 200 ] || {
    echo "notebooklm singleton test: owner DELETE returned HTTP $delete_status" >&2
    exit 1
}

attempt=0
while kill -0 "$provider_pid" 2>/dev/null; do
    attempt=$((attempt + 1))
    [ "$attempt" -lt 100 ] || {
        echo "notebooklm singleton test: provider did not exit after owner session closed" >&2
        exit 1
    }
    sleep 0.1
done
wait "$provider_pid"
provider_pid=0

# Model runit's second restart after a graceful owner DELETE.
start_provider
replacement_session=$(initialize_session replacement)
[ -n "$replacement_session" ] || {
    echo "notebooklm singleton test: replacement initialize returned no session id" >&2
    exit 1
}

kill -TERM "$provider_pid"
wait "$provider_pid"
provider_pid=0
trap - HUP INT TERM EXIT
rm -r -- "$test_root"
echo "notebooklm singleton transport regression: PASS"
