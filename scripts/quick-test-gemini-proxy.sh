#!/bin/bash
set -euo pipefail

# Quick test of Gemini API through Antigravity proxy

PROXY_URL="http://127.0.0.1:8045/v1"
API_KEY="sk-28da1542217448069593b22690c561ca"
MODEL="gemini-2.0-flash-exp"

echo "=== Testing Gemini API Proxy ==="
echo "Proxy: $PROXY_URL"
echo "Model: $MODEL"
echo ""

# Test 1: List models
echo "1. Testing /v1/models endpoint..."
curl -s "$PROXY_URL/models" \
  -H "Authorization: Bearer $API_KEY" | jq -r '.data[]?.id // "No models found"' | head -5

echo ""

# Test 2: Simple chat completion
echo "2. Testing chat completion..."
RESPONSE=$(curl -s "$PROXY_URL/chat/completions" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d "{
    \"model\": \"$MODEL\",
    \"messages\": [{\"role\": \"user\", \"content\": \"Say 'Hello from Gemini!'\"}]
  }")

if echo "$RESPONSE" | jq -e '.choices[0].message.content' > /dev/null 2>&1; then
    echo "✓ Success!"
    echo "$RESPONSE" | jq -r '.choices[0].message.content'
else
    echo "❌ Failed!"
    echo "$RESPONSE" | jq '.'
fi

echo ""

# Test 3: Streaming (if supported)
echo "3. Testing streaming..."
curl -s "$PROXY_URL/chat/completions" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d "{
    \"model\": \"$MODEL\",
    \"messages\": [{\"role\": \"user\", \"content\": \"Count to 3\"}],
    \"stream\": true
  }" | while IFS= read -r line; do
    if [[ "$line" == data:* ]]; then
        echo "$line" | sed 's/^data: //' | jq -r '.choices[0]?.delta?.content // empty' 2>/dev/null || true
    fi
done

echo ""
echo "=== Test Complete ==="
