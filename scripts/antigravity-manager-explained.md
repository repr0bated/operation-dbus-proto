# Antigravity Manager - GitHub App

## What It Is

Antigravity Manager is a GitHub app that:
- **Manages AI sessions** - Can swap between different AI providers/accounts
- **Has 2 proxy servers**:
  1. **API Proxy** (port 8045) - OpenAI-compatible API endpoint
  2. **Global Upstream Proxy** - HTTP/HTTPS intercepting proxy
- **Session Management** - Import/export accounts, switch providers on the fly
- **MCP Support** - Can expose MCP servers

## The Two Proxies

### 1. API Proxy (Port 8045)
**What**: OpenAI-compatible API endpoint
**Purpose**: Unified API for multiple LLM providers

```python
# Use any provider through OpenAI SDK
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:8045/v1",
    api_key="sk-28da1542217448069593b22690c561ca"
)

# Antigravity routes to configured provider (Gemini, Claude, etc.)
response = client.chat.completions.create(
    model="gemini-2.0-flash-exp",
    messages=[{"role": "user", "content": "Hello"}]
)
```

**Features**:
- OpenAI format → any provider
- Token tracking
- Request logging
- Caching
- Rate limiting

### 2. Global Upstream Proxy
**What**: HTTP/HTTPS intercepting proxy
**Purpose**: Intercept ALL traffic from applications

**Enable in**: Settings → Proxy Settings → "Enable Upstream Proxy"

```bash
# Set system-wide or per-app
export HTTP_PROXY=http://127.0.0.1:7890
export HTTPS_PROXY=http://127.0.0.1:7890

# Launch VSCode with proxy
code
```

**Features**:
- Intercepts OAuth flows
- Logs all HTTP/HTTPS traffic
- Can modify requests/responses
- SSL/TLS inspection (with cert install)

## Session Management

The key feature: **Swap accounts/providers on the fly**

### Accounts Tab
- Import accounts from:
  - OAuth (Google, GitHub, etc.)
  - Refresh tokens
  - Database files
- Export accounts for backup
- Switch active account instantly

### How It Works

1. **Add Account** (from your screenshot):
   - **OAuth**: Browser-based login
   - **Refresh Token**: Paste token directly
   - **Import DB**: Import from file

2. **Switch Sessions**:
   - Select account from dropdown
   - All API calls use that account
   - No need to restart apps

3. **Multiple Providers**:
   - Gemini account 1
   - Gemini account 2
   - Claude account
   - OpenAI account
   - Switch between them instantly

## Your Use Case: Cloud Code OAuth

### Goal
Intercept VSCode Cloud Code OAuth to get tokens for Antigravity

### Setup

#### Step 1: Enable Global Upstream Proxy
Settings → Proxy Settings → Enable Upstream Proxy
- Port: 7890 (or custom)
- Enable SSL interception

#### Step 2: Configure VSCode to use proxy
```bash
# Option A: Launch with proxy
HTTP_PROXY=http://127.0.0.1:7890 \
HTTPS_PROXY=http://127.0.0.1:7890 \
code

# Option B: VSCode settings.json
{
  "http.proxy": "http://127.0.0.1:7890",
  "http.proxyStrictSSL": false
}
```

#### Step 3: Trigger Cloud Code OAuth
- Open Cloud Code extension
- Click "Sign in with Google"
- OAuth flow goes through Antigravity proxy

#### Step 4: Capture tokens in Antigravity
- Go to **Traffic Logs** tab
- Find OAuth requests:
  - `accounts.google.com/v3/signin/accountchooser`
  - `localhost:42393/oauth2redirect?code=...`
  - `oauth2.googleapis.com/token` (token exchange)
- Copy the tokens

#### Step 5: Import to Antigravity
- Go to **Accounts** tab
- Click **Add Account** → **Refresh Token**
- Paste the refresh token
- Or use **Import DB** if you have the token database

#### Step 6: Use in API Proxy
Now the API Proxy (port 8045) can use that account:

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:8045/v1",
    api_key="sk-28da1542217448069593b22690c561ca"
)

# Uses the imported Google account
response = client.chat.completions.create(
    model="gemini-2.0-flash-exp",
    messages=[{"role": "user", "content": "Hello"}]
)
```

## The Two Proxies Working Together

```
┌─────────────────────────────────────────────────────────┐
│                    Your Application                      │
│                  (VSCode, Python, etc.)                  │
└────────────────────┬────────────────────────────────────┘
                     │
                     ├─ HTTP/HTTPS traffic
                     │  (OAuth, web requests)
                     ↓
         ┌───────────────────────────┐
         │  Global Upstream Proxy    │
         │     (Port 7890)           │
         │  - Intercepts OAuth       │
         │  - Logs all traffic       │
         │  - SSL inspection         │
         └───────────┬───────────────┘
                     │
                     ├─ OpenAI API calls
                     │  (chat completions)
                     ↓
         ┌───────────────────────────┐
         │     API Proxy             │
         │     (Port 8045)           │
         │  - OpenAI compatible      │
         │  - Routes to providers    │
         │  - Uses imported accounts │
         └───────────┬───────────────┘
                     │
                     ↓
         ┌───────────────────────────┐
         │   LLM Providers           │
         │  - Gemini                 │
         │  - Claude                 │
         │  - OpenAI                 │
         └───────────────────────────┘
```

## Configuration Summary

### For OAuth Interception
- **Use**: Global Upstream Proxy (port 7890)
- **Enable**: Settings → Proxy Settings
- **Configure**: VSCode to use proxy
- **View**: Traffic Logs tab

### For API Usage
- **Use**: API Proxy (port 8045)
- **Enable**: Settings → API Proxy → Start Service
- **Configure**: Add providers in EXTERNAL PROVIDERS
- **Use**: OpenAI SDK pointing to localhost:8045

### For Session Management
- **Use**: Accounts tab
- **Import**: OAuth tokens from Traffic Logs
- **Switch**: Select account from dropdown
- **Export**: Backup accounts to file

## Quick Start

1. **Start both services**:
   - API Proxy: Click "Start Service" (port 8045)
   - Upstream Proxy: Enable in Proxy Settings (port 7890)

2. **Configure VSCode**:
   ```bash
   HTTP_PROXY=http://127.0.0.1:7890 code
   ```

3. **Trigger OAuth** in Cloud Code

4. **Check Traffic Logs** for tokens

5. **Import account** in Accounts tab

6. **Use API Proxy**:
   ```python
   client = OpenAI(base_url="http://127.0.0.1:8045/v1", ...)
   ```

Done! You now have OAuth tokens captured and can use them through the API proxy.
