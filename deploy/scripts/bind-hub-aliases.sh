#!/bin/sh
# Bind decoy + mesh hub aliases on ovsbr0 (idempotent).
#   10.0.0.2/32  — decoy ingress (op-web UI :8080)
#   10.200.0.2/32 — grpc-uplink / tonic-web mesh entry (:8090)
#
# Sourced by s6 run scripts; safe to call multiple times.

HUB_DEV="${HUB_DEV:-ovsbr0}"
DECOY_ALIAS="${DECOY_HUB_ALIAS:-10.0.0.2/32}"
GRPC_ALIAS="${GRPC_UPLINK_ALIAS:-10.200.0.2/32}"

bind_hub_aliases() {
    i=0
    while [ "$i" -lt 30 ]; do
        if ip link show "$HUB_DEV" >/dev/null 2>&1; then
            ip addr replace "$DECOY_ALIAS" dev "$HUB_DEV" 2>/dev/null || true
            ip addr replace "$GRPC_ALIAS" dev "$HUB_DEV" 2>/dev/null || true
            if ip -4 addr show dev "$HUB_DEV" 2>/dev/null | grep -q '10\.0\.0\.2'; then
                echo "hub-aliases: $DECOY_ALIAS on $HUB_DEV (UI http://10.0.0.2:8080)"
            fi
            if ip -4 addr show dev "$HUB_DEV" 2>/dev/null | grep -q '10\.200\.0\.2'; then
                echo "hub-aliases: $GRPC_ALIAS on $HUB_DEV (tonic-web http://10.200.0.2:8090)"
            fi
            return 0
        fi
        i=$((i + 1))
        sleep 1
    done
    echo "hub-aliases: WARNING — $HUB_DEV not ready; aliases not bound" >&2
    return 1
}

bind_hub_aliases
