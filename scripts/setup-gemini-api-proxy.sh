#!/bin/bash
set -euo pipefail

# Setup Gemini API Proxy through Antigravity Tools
# Provides OpenAI-compatible API + MCP support

ANTIGRAVITY_PORT=8045
GEMINI_PROJECT="geminidev-479406"
CONFIG_DIR="$HOME/.config/antigravity-api"

echo "=== Gemini API Proxy Setup ==="
echo "Project: $GEMINI_PROJECT"
echo "Proxy Port: $ANTIGRAVITY_PORT"
echo ""

# Create config directory
mkdir -p "$CONFIG_DIR"

# Check if Antigravity is running
if ! pgrep -f "antigravity" > /dev/null; then
    echo "⚠ Antigravity Tools is not running"
    echo "Please start Antigravity Tools first"
    exit 1
fi

# Check if port is listening
if ! ss -tlnp 2>/dev/null | grep -q ":$ANTIGRAVITY_PORT"; then
    echo "⚠ Port $ANTIGRAVITY_PORT is not listening"
    echo "Please configure Antigravity API Proxy on port $ANTIGRAVITY_PORT"
    exit 1
fi

echo "✓ Antigravity is running on port $ANTIGRAVITY_PORT"

# Create OpenAI client test script
cat > "$CONFIG_DIR/test_gemini_proxy.py" <<'PYTHON'
#!/usr/bin/env python3
"""Test Gemini API through Antigravity proxy"""
from openai import OpenAI
import sys

# Configure client to use Antigravity proxy
client = OpenAI(
    base_url="http://127.0.0.1:8045/v1",
    api_key="sk-28da1542217448069593b22690c561ca"  # Antigravity API key
)

def test_chat():
    """Test chat completion"""
    print("Testing Gemini via Antigravity proxy...")
    
    try:
        response = client.chat.completions.create(
            model="gemini-2.0-flash-exp",  # or gemini-1.5-pro
            messages=[
                {"role": "user", "content": "Hello! What model are you?"}
            ]
        )
        
        print(f"✓ Response: {response.choices[0].message.content}")
        return True
    except Exception as e:
        print(f"❌ Error: {e}")
        return False

def test_streaming():
    """Test streaming response"""
    print("\nTesting streaming...")
    
    try:
        stream = client.chat.completions.create(
            model="gemini-2.0-flash-exp",
            messages=[{"role": "user", "content": "Count to 5"}],
            stream=True
        )
        
        print("✓ Stream: ", end="", flush=True)
        for chunk in stream:
            if chunk.choices[0].delta.content:
                print(chunk.choices[0].delta.content, end="", flush=True)
        print()
        return True
    except Exception as e:
        print(f"❌ Error: {e}")
        return False

if __name__ == "__main__":
    success = test_chat() and test_streaming()
    sys.exit(0 if success else 1)
PYTHON

chmod +x "$CONFIG_DIR/test_gemini_proxy.py"

# Create MCP server config for Gemini
cat > "$CONFIG_DIR/gemini-mcp-server.json" <<JSON
{
  "mcpServers": {
    "gemini-proxy": {
      "command": "python3",
      "args": ["-m", "mcp_server_gemini"],
      "env": {
        "GEMINI_API_KEY": "\${GEMINI_API_KEY}",
        "GEMINI_PROJECT": "$GEMINI_PROJECT",
        "GEMINI_BASE_URL": "http://127.0.0.1:$ANTIGRAVITY_PORT/v1"
      }
    }
  }
}
JSON

# Create environment file
cat > "$CONFIG_DIR/gemini.env" <<ENV
# Gemini API Configuration
export GEMINI_PROJECT="$GEMINI_PROJECT"
export GEMINI_API_KEY="your-api-key-here"
export OPENAI_API_BASE="http://127.0.0.1:$ANTIGRAVITY_PORT/v1"
export OPENAI_API_KEY="sk-28da1542217448069593b22690c561ca"
ENV

# Create usage guide
cat > "$CONFIG_DIR/README.md" <<'README'
# Gemini API Proxy via Antigravity

## Setup

1. **Get Gemini API Key**:
   ```bash
   # From Google Cloud Console or AI Studio
   export GEMINI_API_KEY="your-key-here"
   ```

2. **Configure Antigravity**:
   - Open Antigravity Tools
   - Go to Settings → API Proxy
   - Enable "API Proxy" on port 8045
   - Add Gemini provider with your API key

3. **Test the proxy**:
   ```bash
   python3 ~/.config/antigravity-api/test_gemini_proxy.py
   ```

## Usage

### Python (OpenAI SDK)
```python
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:8045/v1",
    api_key="sk-28da1542217448069593b22690c561ca"
)

response = client.chat.completions.create(
    model="gemini-2.0-flash-exp",
    messages=[{"role": "user", "content": "Hello"}]
)
```

### cURL
```bash
curl http://127.0.0.1:8045/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-28da1542217448069593b22690c561ca" \
  -d '{
    "model": "gemini-2.0-flash-exp",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

### MCP Integration

Add to your `.kiro/settings/mcp.json`:
```json
{
  "mcpServers": {
    "gemini": {
      "command": "python3",
      "args": ["-m", "mcp_server_gemini"],
      "env": {
        "GEMINI_BASE_URL": "http://127.0.0.1:8045/v1",
        "GEMINI_API_KEY": "your-key-here"
      }
    }
  }
}
```

## Available Models

- `gemini-2.0-flash-exp` - Latest experimental
- `gemini-1.5-pro` - Production stable
- `gemini-1.5-flash` - Fast responses

## Features

✓ OpenAI-compatible API
✓ Streaming support
✓ Function calling
✓ Vision (image inputs)
✓ MCP server integration
✓ Request/response logging in Antigravity
✓ Token usage tracking
README

echo ""
echo "✓ Configuration created in: $CONFIG_DIR"
echo ""
echo "Next steps:"
echo "1. Configure Antigravity API Proxy:"
echo "   - Open Antigravity Tools"
echo "   - Settings → API Proxy → Enable on port $ANTIGRAVITY_PORT"
echo "   - Add Gemini provider with your API key"
echo ""
echo "2. Test the setup:"
echo "   python3 $CONFIG_DIR/test_gemini_proxy.py"
echo ""
echo "3. View full guide:"
echo "   cat $CONFIG_DIR/README.md"
echo ""
