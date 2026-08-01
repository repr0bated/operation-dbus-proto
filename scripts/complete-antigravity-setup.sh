#!/bin/bash
set -euo pipefail

# Complete Antigravity Setup for IDE Spoofing
# This configures BOTH proxies to make Google see requests as coming from VSCode

echo "=== Antigravity Complete Setup ==="
echo ""

# Configuration
API_PROXY_PORT=8045
UPSTREAM_PROXY_PORT=7890
API_KEY="sk-28da1542217448069593b22690c561ca"
IDE_USER_AGENT="google-cloud-code-vscode/1.22.0 (GPN:Cloud Code for VS Code) vscode/1.85.0 (linux; x64)"

echo "Configuration:"
echo "  API Proxy: http://127.0.0.1:$API_PROXY_PORT"
echo "  Upstream Proxy: http://127.0.0.1:$UPSTREAM_PROXY_PORT"
echo "  User-Agent: $IDE_USER_AGENT"
echo ""

# Check if Antigravity is running
if ! pgrep -f "antigravity" > /dev/null; then
    echo "❌ Antigravity is not running"
    echo "Please start Antigravity Tools first"
    exit 1
fi

echo "✓ Antigravity is running"

# Create environment file
cat > ~/.config/antigravity-proxy.env <<EOF
# Antigravity Proxy Configuration
# Source this file: source ~/.config/antigravity-proxy.env

# API Proxy (OpenAI-compatible endpoint)
export GOOGLE_GEMINI_BASE_URL=http://127.0.0.1:$API_PROXY_PORT
export GEMINI_API_KEY=$API_KEY
export OPENAI_API_BASE=http://127.0.0.1:$API_PROXY_PORT/v1
export OPENAI_API_KEY=$API_KEY

# Global Upstream Proxy (for header injection)
export HTTP_PROXY=http://127.0.0.1:$UPSTREAM_PROXY_PORT
export HTTPS_PROXY=http://127.0.0.1:$UPSTREAM_PROXY_PORT
export NO_PROXY=localhost,127.0.0.1

# IDE identification
export USER_AGENT="$IDE_USER_AGENT"
export GOOGLE_CLOUD_IDE=vscode
export GOOGLE_CLOUD_CODE_VERSION=1.22.0

# Project
export GOOGLE_CLOUD_PROJECT=geminidev-479406
EOF

echo "✓ Created ~/.config/antigravity-proxy.env"

# Create shell wrapper for launching apps with proxy
cat > ~/.local/bin/with-antigravity <<'WRAPPER'
#!/bin/bash
# Launch applications through Antigravity proxy

# Load configuration
if [ -f ~/.config/antigravity-proxy.env ]; then
    source ~/.config/antigravity-proxy.env
fi

# Launch the application
exec "$@"
WRAPPER

chmod +x ~/.local/bin/with-antigravity

echo "✓ Created ~/.local/bin/with-antigravity"

# Create VSCode launcher
cat > ~/.local/bin/vscode-with-antigravity <<'VSCODE'
#!/bin/bash
# Launch VSCode with Antigravity proxy

source ~/.config/antigravity-proxy.env

echo "Launching VSCode with Antigravity proxy..."
echo "  HTTP_PROXY: $HTTP_PROXY"
echo "  HTTPS_PROXY: $HTTPS_PROXY"
echo "  API Base: $GOOGLE_GEMINI_BASE_URL"
echo ""

exec code "$@"
VSCODE

chmod +x ~/.local/bin/vscode-with-antigravity

echo "✓ Created ~/.local/bin/vscode-with-antigravity"

# Create test script
cat > ~/.local/bin/test-antigravity <<'TEST'
#!/bin/bash
# Test Antigravity proxy configuration

source ~/.config/antigravity-proxy.env

echo "=== Testing Antigravity Proxy ==="
echo ""

# Test 1: Check proxy is accessible
echo "1. Testing upstream proxy..."
if curl -s -x "$HTTP_PROXY" http://example.com > /dev/null; then
    echo "   ✓ Upstream proxy is working"
else
    echo "   ❌ Upstream proxy failed"
fi

# Test 2: Check API proxy
echo "2. Testing API proxy..."
RESPONSE=$(curl -s -w "%{http_code}" -o /dev/null \
    -H "Authorization: Bearer $GEMINI_API_KEY" \
    "$GOOGLE_GEMINI_BASE_URL/v1/models")

if [ "$RESPONSE" = "200" ]; then
    echo "   ✓ API proxy is working"
else
    echo "   ❌ API proxy failed (HTTP $RESPONSE)"
fi

# Test 3: Test Gemini API
echo "3. Testing Gemini API..."
RESULT=$(curl -s \
    -H "Authorization: Bearer $GEMINI_API_KEY" \
    -H "Content-Type: application/json" \
    "$GOOGLE_GEMINI_BASE_URL/v1/chat/completions" \
    -d '{
        "model": "gemini-2.0-flash-exp",
        "messages": [{"role": "user", "content": "Say OK"}]
    }' | jq -r '.choices[0].message.content // "ERROR"')

if [ "$RESULT" != "ERROR" ]; then
    echo "   ✓ Gemini API is working: $RESULT"
else
    echo "   ❌ Gemini API failed"
fi

echo ""
echo "=== Configuration ==="
echo "HTTP_PROXY: $HTTP_PROXY"
echo "HTTPS_PROXY: $HTTPS_PROXY"
echo "API Base: $GOOGLE_GEMINI_BASE_URL"
echo "User-Agent: $USER_AGENT"
TEST

chmod +x ~/.local/bin/test-antigravity

echo "✓ Created ~/.local/bin/test-antigravity"

echo ""
echo "=== Setup Complete! ==="
echo ""
echo "Next steps:"
echo ""
echo "1. Configure Antigravity:"
echo "   - Open Antigravity Tools"
echo "   - Settings → Proxy Settings"
echo "   - Enable 'Global Upstream Proxy' on port $UPSTREAM_PROXY_PORT"
echo "   - Set User-Agent Override to:"
echo "     $IDE_USER_AGENT"
echo ""
echo "2. Start the API Proxy:"
echo "   - Go to API Proxy tab"
echo "   - Click 'Start Service'"
echo ""
echo "3. Test the setup:"
echo "   test-antigravity"
echo ""
echo "4. Launch VSCode with proxy:"
echo "   vscode-with-antigravity"
echo ""
echo "5. Or launch any app with proxy:"
echo "   with-antigravity <command>"
echo ""
echo "6. Or manually source the environment:"
echo "   source ~/.config/antigravity-proxy.env"
echo ""
