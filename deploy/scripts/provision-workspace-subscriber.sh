#!/bin/sh
# Provision a workspace subscriber — full container + memory setup.
#
# Creates an Incus container, configures networking, provisions memory
# namespaces and feature flags. This is the template run at signup.
#
# Identity chain: MAC address (user-provided shared key) -> WG keypair ->
#   full pubkey stored in CozoDB -> MCP token = UUID v5(pubkey).
# The MAC is less identifying than IP but hardware-bound to the user.
#
# Email is stored ONLY when GhostBridge is disabled. GhostBridge users
# route through the privacy layer and must not have email in CozoDB.
#
# Usage:
#   sudo ./provision-workspace-subscriber.sh <username> --mac <aa:bb:cc:dd:ee:ff> --psk <key> [--email <addr>] [--ghostbridge] [--semantic]
#
set -eu

USERNAME="${1:?username required}"
shift

MAC_ADDR=""
PSK=""
EMAIL=""
GHOSTBRIDGE=false
SEMANTIC=false
while [ $# -gt 0 ]; do
    case "$1" in
        --mac)         shift; MAC_ADDR="${1:?--mac requires a MAC address}" ;;
        --psk)         shift; PSK="${1:?--psk requires a pre-shared key}" ;;
        --email)       shift; EMAIL="${1:?--email requires an address}" ;;
        --ghostbridge) GHOSTBRIDGE=true ;;
        --semantic)    SEMANTIC=true ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
    shift
done

CONTAINER_ID="ws-${USERNAME}"
IMAGE="images:debian/12"
MCP="http://100.90.37.254:3003/mcp"

echo "=== Provisioning workspace subscriber: $USERNAME ==="
echo "    Container  : $CONTAINER_ID"
echo "    MAC address: ${MAC_ADDR:-<not provided>}"
echo "    PSK        : ${PSK:+<provided>}${PSK:-<not provided>}"
echo "    Email      : ${EMAIL:-<not provided>}"
echo "    GhostBridge: $GHOSTBRIDGE"
echo "    Semantic   : $SEMANTIC"
echo ""

# ── 1. Create Incus container ─────────────────────────────────────────────────
echo "[1] Creating container..."
if incus info "$CONTAINER_ID" >/dev/null 2>&1; then
    echo "    Already exists — skipping"
else
    incus launch "$IMAGE" "$CONTAINER_ID"
    echo "    Launched $CONTAINER_ID"
fi

# Wait for container to be ready
echo "    Waiting for init..."
sleep 3
incus exec "$CONTAINER_ID" -- sh -c "until [ -f /run/systemd/private ] || [ -d /run/s6 ] || pgrep -x init >/dev/null 2>&1 || sleep 1; do sleep 1; done" 2>/dev/null || sleep 2

# ── 2. Base packages ──────────────────────────────────────────────────────────
echo "[2] Installing base packages..."
incus exec "$CONTAINER_ID" -- apt-get update -qq
incus exec "$CONTAINER_ID" -- apt-get install -y --no-install-recommends \
    curl ca-certificates wireguard-tools iproute2 \
    nodejs npm 2>/dev/null || true

# ── 3. WireGuard identity — always provisioned ─────────────────────────────────
# The WG pubkey IS the identity. The MCP token is a UUID v5 derivative.
# The full pubkey is stored in the user database (CozoDB).
echo "[3] Generating WireGuard identity..."
incus exec "$CONTAINER_ID" -- sh -c "
    mkdir -p /etc/wireguard
    wg genkey | tee /etc/wireguard/privkey | wg pubkey > /etc/wireguard/pubkey
    chmod 600 /etc/wireguard/privkey
"
PEER_PUBKEY=$(incus exec "$CONTAINER_ID" -- cat /etc/wireguard/pubkey)
echo "    Peer pubkey: $PEER_PUBKEY"

# Derive MCP bearer token: UUID v5 from WG pubkey (one-way, not the raw key)
TOKEN=$(uuidgen -v5 -n @oid -N "$PEER_PUBKEY")
echo "    MCP token : $TOKEN (derived from pubkey)"

# GhostBridge networking (optional WG peer registration)
if [ "$GHOSTBRIDGE" = "true" ]; then
    echo "    GhostBridge: registering WireGuard peer..."
    # TODO: register peer with Netmaker API
    echo "    (Netmaker registration: TODO — wire to Netmaker API)"
else
    echo "    GhostBridge: skipped (networking only — identity already provisioned)"
fi

# ── 4. Semantic search — Qdrant collection ────────────────────────────────────
if [ "$SEMANTIC" = "true" ]; then
    echo "[4] Semantic search: enabled — Qdrant collection will be created on first use"
else
    echo "[4] Semantic search: skipped (disabled)"
fi

# ── 5. Memory namespaces via cognitive-mcp ────────────────────────────────────
echo "[5] Provisioning memory namespaces..."

remember() {
    NS="$1"; KEY="$2"; VAL="$3"
    RESULT=$(curl -sf -X POST "$MCP" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $TOKEN" \
        -H "User-Agent: op-dbus/1.0" \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"cognitive_memory\",\"arguments\":{\"operation\":\"store\",\"namespace\":\"$NS\",\"key\":\"$KEY\",\"value\":$VAL}}}" 2>&1) \
        && echo "    ok: $NS/$KEY" \
        || echo "    WARN: could not write $NS/$KEY (cognitive-mcp may be unreachable)"
}

NOW=$(date -u +%Y-%m-%dT%H:%M:%SZ)

# Identity — cryptographic + hardware keys in CozoDB
remember "container:${CONTAINER_ID}:identity" "wireguard_pubkey" \
    "\"$PEER_PUBKEY\""
remember "container:${CONTAINER_ID}:identity" "mcp_token" \
    "\"$TOKEN\""
if [ -n "$MAC_ADDR" ]; then
    remember "container:${CONTAINER_ID}:identity" "mac_address" \
        "\"$MAC_ADDR\""
fi
if [ -n "$PSK" ]; then
    remember "container:${CONTAINER_ID}:identity" "psk" \
        "\"$PSK\""
fi

# Email: store ONLY when GhostBridge is disabled.
# GhostBridge users route through the privacy layer — email must not leak into CozoDB.
if [ -n "$EMAIL" ] && [ "$GHOSTBRIDGE" = "false" ]; then
    remember "container:${CONTAINER_ID}:identity" "email" \
        "\"$EMAIL\""
fi

# Soul
remember "container:${CONTAINER_ID}:soul" "profile" \
    "{\"kind\":\"soul\",\"container_id\":\"$CONTAINER_ID\",\"username\":\"$USERNAME\",\"created_at\":\"$NOW\"}"

# Domains
for DOMAIN in work personal home; do
    remember "container:${CONTAINER_ID}:domain:${DOMAIN}" "MEMORY_INDEX" \
        "{\"kind\":\"domain\",\"domain\":\"$DOMAIN\",\"container_id\":\"$CONTAINER_ID\",\"entries\":[]}"
done

# Index
remember "container:${CONTAINER_ID}:index" "MEMORY_INDEX" \
    "{\"kind\":\"index\",\"container_id\":\"$CONTAINER_ID\",\"namespaces\":[\"identity\",\"soul\",\"domain:work\",\"domain:personal\",\"domain:home\"]}"

# Feature flags
remember "container:${CONTAINER_ID}:features" "ghostbridge" \
    "{\"enabled\":$GHOSTBRIDGE}"
remember "container:${CONTAINER_ID}:features" "semantic_search" \
    "{\"enabled\":$SEMANTIC}"

# ── 6. Summary ──────────────────────────────────────────────────────────────
echo ""
echo "=== Done ==="
echo ""
incus list "$CONTAINER_ID" 2>/dev/null || true
echo ""
echo "Identity:"
echo "  WG pubkey : $PEER_PUBKEY"
echo "  MCP token : $TOKEN (UUID v5 derivative)"
if [ -n "$MAC_ADDR" ]; then
echo "  MAC addr  : $MAC_ADDR (shared key)"
fi
if [ -n "$PSK" ]; then
echo "  PSK       : <stored>"
fi
if [ -n "$EMAIL" ] && [ "$GHOSTBRIDGE" = "false" ]; then
echo "  Email     : $EMAIL"
elif [ "$GHOSTBRIDGE" = "true" ]; then
echo "  Email     : <withheld — GhostBridge privacy>"
fi
echo ""
echo "Memory namespaces:"
echo "  container:${CONTAINER_ID}:identity"
echo "  container:${CONTAINER_ID}:soul"
echo "  container:${CONTAINER_ID}:domain:work"
echo "  container:${CONTAINER_ID}:domain:personal"
echo "  container:${CONTAINER_ID}:domain:home"
echo "  container:${CONTAINER_ID}:index"
echo "  container:${CONTAINER_ID}:features"
echo ""
echo "To enable GhostBridge:    sudo $0 $USERNAME --mac $MAC_ADDR --psk <key> --ghostbridge"
echo "To enable semantic search: sudo $0 $USERNAME --mac $MAC_ADDR --semantic"
echo "To enter container:        incus exec $CONTAINER_ID -- bash"
