# Configure Antigravity for Gemini API Proxy

## Current Settings (from screenshot)

- **Listen Port**: 8045 ✓
- **Request Timeout**: 120s ✓
- **Allow LAN Access**: Enabled (listening on 127.0.0.1)
- **Authentication**: Auto (Recommended) ✓
- **API Key**: `sk-28da1542217448069593b22690c561ca` ✓
- **User-Agent Override**: Enabled

## Setup Steps

### 1. Add Gemini as GLM Provider

**Note**: ONE-CLICK CLI SYNC is for external providers like Claude. For Gemini, configure as GLM provider:

1. In Antigravity, scroll to **EXTERNAL PROVIDERS** section
2. Look for **Gemini / Google AI** or **GLM Provider** option
3. Click **Add** or **Configure**
4. Enter configuration:
   - **Provider Type**: `Google Gemini` or `GLM`
   - **API Key**: Your Gemini API key (from Google AI Studio)
   - **Project ID**: `geminidev-479406` (optional for AI Studio keys)
   - **Region**: `us-central1` (if using Vertex AI)
   - **Endpoint**: 
     - AI Studio: `https://generativelanguage.googleapis.com/v1beta`
     - Vertex AI: `https://us-central1-aiplatform.googleapis.com/v1`
   - **Available Models**: 
     - `gemini-2.0-flash-exp`
     - `gemini-1.5-pro-latest`
     - `gemini-1.5-flash-latest`

**Alternative: Manual Config File**

If Antigravity supports config files, create `~/.config/antigravity/providers.json`:

```json
{
  "providers": {
    "gemini": {
      "type": "google-gemini",
      "api_key": "${GEMINI_API_KEY}",
      "project_id": "geminidev-479406",
      "base_url": "https://generativelanguage.googleapis.com/v1beta",
      "models": [
        "gemini-2.0-flash-exp",
        "gemini-1.5-pro-latest",
        "gemini-1.5-flash-latest"
      ]
    }
  }
}
```

### 2. Get Gemini API Key

```bash
# Option 1: From Google AI Studio (easiest)
# Visit: https://aistudio.google.com/app/apikey

# Option 2: From GCP Console
gcloud auth application-default login
gcloud config set project geminidev-479406

# Option 3: Create service account
gcloud iam service-accounts create gemini-api-user \
  --project=geminidev-479406 \
  --display-name="Gemini API User"

gcloud iam service-accounts keys create ~/gemini-key.json \
  --iam-account=gemini-api-user@geminidev-479406.iam.gserviceaccount.com

# Grant permissions
gcloud projects add-iam-policy-binding geminidev-479406 \
  --member="serviceAccount:gemini-api-user@geminidev-479406.iam.gserviceaccount.com" \
  --role="roles/aiplatform.user"
```

### 3. Test the Proxy

```bash
# Test with curl
curl http://127.0.0.1:8045/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-28da1542217448069593b22690c561ca" \
  -d '{
    "model": "gemini-2.0-flash-exp",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'

# Test with Python
python3 << 'EOF'
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:8045/v1",
    api_key="sk-28da1542217448069593b22690c561ca"
)

response = client.chat.completions.create(
    model="gemini-2.0-flash-exp",
    messages=[{"role": "user", "content": "Hello!"}]
)

print(response.choices[0].message.content)
EOF
```

### 4. Configure MCP Server

Add to `.kiro/settings/mcp.json`:

```json
{
  "mcpServers": {
    "gemini-proxy": {
      "command": "python3",
      "args": ["scripts/gemini-mcp-server.py"],
      "env": {
        "GEMINI_BASE_URL": "http://127.0.0.1:8045/v1",
        "GEMINI_API_KEY": "sk-28da1542217448069593b22690c561ca",
        "GEMINI_MODEL": "gemini-2.0-flash-exp"
      },
      "disabled": false,
      "autoApprove": ["gemini_chat", "gemini_analyze_code"]
    }
  }
}
```

### 5. Install MCP Dependencies

```bash
pip install mcp openai
```

## Troubleshooting

### Provider Not Showing Up
- Click "Sync Config Now" in Antigravity
- Restart Antigravity service
- Check logs in Traffic Logs tab

### Authentication Errors
- Verify API key is correct
- Check project permissions
- Enable Generative Language API in GCP Console:
  ```bash
  gcloud services enable generativelanguage.googleapis.com \
    --project=geminidev-479406
  ```

### Connection Refused
- Verify Antigravity is running: `ps aux | grep antigravity`
- Check port is listening: `ss -tlnp | grep 8045`
- Test locally: `curl http://127.0.0.1:8045/v1/models`

## Features Available

✓ **OpenAI-compatible API** - Use any OpenAI SDK
✓ **Request logging** - See all requests in Antigravity Traffic Logs
✓ **Token counting** - Track usage in Token Stats
✓ **Rate limiting** - Configure in Antigravity settings
✓ **Caching** - Enable response caching
✓ **MCP integration** - Use Gemini through Model Context Protocol
✓ **Multi-model** - Switch between Gemini models easily

## Next Steps

1. Click "Start Service" button (blue button in screenshot)
2. Add Gemini provider in EXTERNAL PROVIDERS section
3. Test with the curl command above
4. Configure MCP server for IDE integration
