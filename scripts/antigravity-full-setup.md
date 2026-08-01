# Antigravity Complete Setup

## What Antigravity Does

Antigravity on port 8045 acts as a **universal intercepting proxy** that:

✓ Intercepts ALL HTTP/HTTPS traffic routed through it
✓ Logs requests/responses (visible in Traffic Logs tab)
✓ Provides OpenAI-compatible API for any LLM backend
✓ Can route to multiple providers (Gemini, Claude, OpenAI, etc.)
✓ Caches responses, tracks tokens, rate limits

## Current Configuration (from screenshot)

- **Port**: 8045
- **Authentication**: Auto (Recommended) - smart detection
- **API Key**: `sk-28da1542217448069593b22690c561ca`
- **Allow LAN Access**: Enabled (127.0.0.1 only)

## Use Cases

### 1. Intercept VSCode Cloud Code OAuth

Set VSCode to use Antigravity as proxy:

```bash
# Launch VSCode with proxy
HTTP_PROXY=http://127.0.0.1:8045 \
HTTPS_PROXY=http://127.0.0.1:8045 \
code
```

Or in VSCode settings.json:
```json
{
  "http.proxy": "http://127.0.0.1:8045",
  "http.proxyStrictSSL": false
}
```

Now when Cloud Code does OAuth:
- All requests go through Antigravity
- View in **Traffic Logs** tab
- See OAuth URLs, tokens, callbacks
- Can export/save for analysis

### 2. Proxy Gemini API (OpenAI-compatible)

Configure Gemini in Antigravity's **EXTERNAL PROVIDERS**:

1. Add Gemini provider with your API key
2. Antigravity translates OpenAI format → Gemini format
3. Use from any OpenAI-compatible client:

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:8045/v1",
    api_key="sk-28da1542217448069593b22690c561ca"
)

# This goes to Gemini via Antigravity
response = client.chat.completions.create(
    model="gemini-2.0-flash-exp",
    messages=[{"role": "user", "content": "Hello"}]
)
```

### 3. MCP Server Integration

Antigravity can expose MCP servers. Configure in **MCP Servers** section:

```json
{
  "mcpServers": {
    "gemini": {
      "command": "python3",
      "args": ["scripts/gemini-mcp-server.py"],
      "env": {
        "GEMINI_BASE_URL": "http://127.0.0.1:8045/v1",
        "GEMINI_API_KEY": "sk-28da1542217448069593b22690c561ca"
      }
    }
  }
}
```

## Authentication Modes Explained

### Off (Open)
- No authentication required
- Anyone can use the proxy
- Good for: Local testing only

### All (Strict)
- API key required for ALL requests
- Blocks unauthorized access
- Good for: Production, shared environments

### All except Health
- API key required except `/health` endpoint
- Allows monitoring without auth
- Good for: Load balancers, monitoring

### Auto (Recommended) ⭐
- Smart detection:
  - Recognizes OpenAI API format → requires auth
  - Passes through OAuth flows → no auth
  - Health checks → no auth
  - Unknown requests → requires auth
- Good for: Mixed use cases (your scenario!)

## Complete Workflow

### Step 1: Start Antigravity Service
Click **"Start Service"** button (blue button in screenshot)

### Step 2: Add Gemini Provider
Scroll to **EXTERNAL PROVIDERS** → Add Gemini with API key

### Step 3: Configure VSCode Proxy (for OAuth interception)
```bash
# Option A: Environment variables
export HTTP_PROXY=http://127.0.0.1:8045
export HTTPS_PROXY=http://127.0.0.1:8045
code

# Option B: VSCode settings
# Add to ~/.config/Code/User/settings.json:
{
  "http.proxy": "http://127.0.0.1:8045",
  "http.proxyStrictSSL": false
}
```

### Step 4: Test Everything

```bash
# Test 1: Gemini API
curl http://127.0.0.1:8045/v1/chat/completions \
  -H "Authorization: Bearer sk-28da1542217448069593b22690c561ca" \
  -H "Content-Type: application/json" \
  -d '{"model":"gemini-2.0-flash-exp","messages":[{"role":"user","content":"Hi"}]}'

# Test 2: Trigger Cloud Code OAuth in VSCode
# Watch Traffic Logs tab in Antigravity to see OAuth flow

# Test 3: Check what's being intercepted
# Open Antigravity → Traffic Logs tab
# You'll see all HTTP/HTTPS requests
```

## What You'll See in Traffic Logs

When Cloud Code does OAuth:
```
GET https://accounts.google.com/v3/signin/accountchooser?...
  → Authorization URL with client_id, redirect_uri, etc.

GET http://localhost:42393/oauth2redirect?code=...
  → Callback with authorization code

POST https://oauth2.googleapis.com/token
  → Token exchange (code → access_token)
```

When using Gemini API:
```
POST http://127.0.0.1:8045/v1/chat/completions
  → Your request (OpenAI format)

POST https://generativelanguage.googleapis.com/v1beta/...
  → Antigravity's request to Gemini (translated)

Response: 200 OK
  → Gemini's response (translated back to OpenAI format)
```

## Benefits

✓ **Single proxy** for everything
✓ **Unified logging** - see all traffic in one place
✓ **OpenAI compatibility** - use any OpenAI SDK with Gemini
✓ **OAuth inspection** - capture tokens, analyze flows
✓ **Token tracking** - see usage across all providers
✓ **Caching** - reduce API calls
✓ **Rate limiting** - protect against overuse
✓ **Multi-provider** - switch between Gemini, Claude, OpenAI easily

## Security Notes

- API key `sk-28da1542217448069593b22690c561ca` is for Antigravity proxy auth
- Your Gemini API key is stored in Antigravity's provider config
- OAuth tokens are visible in Traffic Logs (be careful!)
- Use `http.proxyStrictSSL: false` only for local development
- Don't expose port 8045 to the internet (127.0.0.1 only)
