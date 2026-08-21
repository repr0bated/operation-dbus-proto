#!/bin/sh
# Verify the canonical Cognitive ingress without printing identity material.
#
# Canonical topology:
#   TLS TCP  https://127.0.0.1:8090
#   host UDS /run/opdbus/grpc.sock
#   CT UDS   /run/ghostbridge/container.sock
# The CognitiveMcpServer is owned in-process by op-grpc-bridge.  The standalone
# op-cognitive-mcp service must remain down and legacy network ports must not
# listen.

set -eu

failures=0

pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1" >&2; failures=$((failures + 1)); }

if sudo sv check op-grpc-bridge >/dev/null 2>&1; then
    pass "op-grpc-bridge is supervised and running"
else
    fail "op-grpc-bridge is not healthy under runit"
fi

if sudo sv status op-cognitive-mcp 2>&1 | grep -q '^down:'; then
    pass "standalone op-cognitive-mcp is down"
else
    fail "standalone op-cognitive-mcp must remain down"
fi

if ss -ltn 2>/dev/null | grep -Eq ':(3003|50051|50052)[[:space:]]'; then
    fail "a retired cognitive/control-plane TCP port is listening"
else
    pass "retired ports 3003, 50051, and 50052 are closed"
fi

services=$(grpcurl -insecure 127.0.0.1:8090 list 2>/dev/null || true)
if printf '%s\n' "$services" | grep -qx 'operation.cognitive.v1.CognitiveToolService'; then
    pass "CognitiveToolService is reflected on TLS :8090"
else
    fail "CognitiveToolService is absent from TLS :8090 reflection"
fi

method_count=$(printf '%s\n' "$services" \
    | grep -c '^operation\.method\.cognitive_mcp\.' || true)
if [ "$method_count" -ge 16 ]; then
    pass "schema-derived cognitive services are reflected ($method_count)"
else
    fail "expected at least 16 schema-derived cognitive services, found $method_count"
fi

unauth=$(grpcurl -insecure -d '{"deep_check":false}' 127.0.0.1:8090 \
    operation.cognitive.v1.CognitiveToolService/GetHealth 2>&1 || true)
if printf '%s\n' "$unauth" | grep -q 'Unauthenticated'; then
    pass "TLS cognitive calls reject missing Ghostbridge identity"
else
    fail "TLS cognitive call did not produce the expected identity rejection"
fi

if [ -s /etc/op-dbus/host-session-id ]; then
    pass "canonical host session id is configured"
else
    fail "/etc/op-dbus/host-session-id is missing or empty"
fi

if grpcurl -plaintext -unix -d '{"deep_check":false}' /run/opdbus/grpc.sock \
    operation.cognitive.v1.CognitiveToolService/GetHealth >/dev/null 2>&1; then
    pass "authenticated host UDS reaches CognitiveToolService"
else
    fail "host UDS could not authenticate or reach CognitiveToolService"
fi

if [ "$failures" -ne 0 ]; then
    printf '\n%d canonical-ingress check(s) failed\n' "$failures" >&2
    exit 1
fi

printf '\nCanonical Cognitive ingress is healthy\n'
