#!/bin/bash
# Verify Factory/Droid connection to cognitive-mcp gateway
# Per AGENTS.md Section 4b: cognitive-mcp (:3003) is universal gateway for ALL external clients

set -e

echo "=========================================="
echo "Factory/Droid Cognitive-MCP Verifier"
echo "=========================================="
echo ""

COGNITIVE_MCP_URL="${COGNITIVE_MCP_URL:-http://127.0.0.1:3003}"
WG_INTERFACE="${WG_INTERFACE:-netmaker}"

echo "Configuration:"
echo "  Cognitive-MCP URL: $COGNITIVE_MCP_URL"
echo "  WireGuard Interface: $WG_INTERFACE"
echo ""

# Check WireGuard interface
echo "[1/4] Checking WireGuard interface ($WG_INTERFACE)..."
if ! ip link show "$WG_INTERFACE" &>/dev/null; then
    echo "  ⚠️  Warning: WireGuard interface '$WG_INTERFACE' not found"
    echo "      Ghostbridge authentication may fail"
else
    WG_IP=$(ip addr show "$WG_INTERFACE" | grep -oP 'inet \K[\d.]+' || echo "unknown")
    echo "  ✅ WireGuard interface active (IP: $WG_IP)"
fi
echo ""

# Check cognitive-mcp health
echo "[2/4] Checking cognitive-mcp gateway health..."
if curl -s --max-time 5 "$COGNITIVE_MCP_URL/health" &>/dev/null; then
    echo "  ✅ Cognitive-MCP gateway is reachable"
else
    echo "  ⚠️  Warning: Cannot reach cognitive-mcp at $COGNITIVE_MCP_URL"
    echo "      Ensure the server is running: systemctl status op-cognitive-mcp"
fi
echo ""

# Check capabilities endpoint
echo "[3/4] Fetching available capabilities..."
CAPABILITIES=$(curl -s --max-time 10 "$COGNITIVE_MCP_URL/tools" 2>/dev/null || echo "null")
if [ "$CAPABILITIES" != "null" ] && [ -n "$CAPABILITIES" ]; then
    TOOL_COUNT=$(echo "$CAPABILITIES" | jq 'if type == "array" then length else 0 end' 2>/dev/null || echo "0")
    echo "  ✅ Found $TOOL_COUNT tools available"
    
    # List first few tools
    echo ""
    echo "  Available tools (first 5):"
    echo "$CAPABILITIES" | jq -r '.[:5] | .[] | "    - " + (.name // "unknown")' 2>/dev/null || true
else
    echo "  ⚠️  Could not fetch capabilities"
fi
echo ""

# Check Factory/Droid configuration
echo "[4/4] Checking Factory configuration..."
if [ -f "$HOME/.factory/mcp.json" ]; then
    echo "  ✅ Factory MCP config found: ~/.factory/mcp.json"
    
    # Check if cognitive-mcp is in config
    if grep -q "cognitive-mcp" "$HOME/.factory/mcp.json"; then
        echo "  ✅ Cognitive-MCP configured in Factory"
    else
        echo "  ⚠️  Cognitive-MCP not found in Factory config"
        echo "      Run: cp deploy/config/factory-mcp.json ~/.factory/mcp.json"
    fi
else
    echo "  ❌ No Factory MCP config found at ~/.factory/mcp.json"
    echo "     Create it with: cp deploy/config/factory-mcp.json ~/.factory/mcp.json"
fi
echo ""

# Summary
echo "=========================================="
echo "Summary"
echo "=========================================="
echo ""
echo "Factory/Droid should now connect to cognitive-mcp at:"
echo "  $COGNITIVE_MCP_URL"
echo ""
echo "Per AGENTS.md Section 4b:"
echo "  ✅ cognitive-mcp (:3003) - Universal gateway for external clients"
echo "  ❌ compact-mcp (127.0.0.1:11436) - Loopback ONLY (not for external)"
echo ""
echo "Next steps:"
echo "  1. Restart Factory/Droid to pick up MCP configuration"
echo "  2. Verify connection in Factory UI: Settings > MCP Servers"
echo "  3. Test with: curl $COGNITIVE_MCP_URL/tools"
echo ""
