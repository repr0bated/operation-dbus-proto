#!/bin/bash
set -euo pipefail

# Get Cloud Code API credentials via OAuth interception
# This captures the enterprise tokens needed for daily-cloudcode-pa.sandbox.googleapis.com

echo "=== Cloud Code Credential Capture ==="
echo ""
echo "This will intercept OAuth tokens from Google Cloud Code extension"
echo ""

# Check if mitmproxy is installed
if ! command -v mitmproxy &> /dev/null; then
    echo "Installing mitmproxy for OAuth interception..."
    pip3 install mitmproxy
fi

# Create interception script
cat > /tmp/cloudcode_intercept.py <<'PYTHON'
from mitmproxy import http
import json
import os

def response(flow: http.HTTPFlow) -> None:
    # Intercept Cloud Code API responses
    if "cloudcode-pa.sandbox.googleapis.com" in flow.request.pretty_host:
        print(f"\n🎯 Cloud Code API Request: {flow.request.path}")
        
        # Log headers (contains auth)
        if "authorization" in flow.request.headers:
            auth = flow.request.headers["authorization"]
            print(f"   Auth: {auth[:50]}...")
            
            # Save to file
            with open("/tmp/cloudcode_token.txt", "w") as f:
                f.write(auth)
        
        # Log response
        if flow.response:
            print(f"   Status: {flow.response.status_code}")
            if flow.response.content:
                try:
                    data = json.loads(flow.response.content)
                    print(f"   Response: {json.dumps(data, indent=2)[:200]}...")
                except:
                    pass
    
    # Also intercept OAuth token exchanges
    if "oauth2.googleapis.com" in flow.request.pretty_host:
        if flow.response and flow.response.content:
            try:
                data = json.loads(flow.response.content)
                if "access_token" in data:
                    print(f"\n🔑 OAuth Token Captured!")
                    with open("/tmp/oauth_tokens.json", "w") as f:
                        json.dump(data, f, indent=2)
            except:
                pass
PYTHON

echo "Starting proxy on port 7890..."
echo ""
echo "Next steps:"
echo "1. Configure your IDE/VSCode to use proxy:"
echo "   export HTTP_PROXY=http://127.0.0.1:7890"
echo "   export HTTPS_PROXY=http://127.0.0.1:7890"
echo ""
echo "2. Open VSCode with Cloud Code extension"
echo "3. Sign in to Google Cloud"
echo "4. Try to use Gemini Code Assist"
echo ""
echo "Tokens will be saved to:"
echo "  - /tmp/cloudcode_token.txt"
echo "  - /tmp/oauth_tokens.json"
echo ""
echo "Press Ctrl+C when done"
echo ""

# Start mitmproxy with interception script
mitmdump -s /tmp/cloudcode_intercept.py -p 7890 --set confdir=~/.mitmproxy
