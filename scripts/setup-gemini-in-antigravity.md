# Setup Gemini in Antigravity Tools

## What You're Looking At

The **API Proxy** tab shows:
- **ONE-CLICK CLI SYNC**: Sync configs from CLI tools (gcloud, claude, etc.)
- **GLM Providers**: Configured LLM providers (you have z.ai already)
- **MCP Servers**: Model Context Protocol servers

## Add Gemini Provider

### Option 1: One-Click CLI Sync (Easiest)

1. **Install gcloud CLI** (if not already):
   ```bash
   # Check if installed
   which gcloud
   
   # If not, install
   curl https://sdk.cloud.google.com | bash
   exec -l $SHELL
   ```

2. **Authenticate with Google**:
   ```bash
   gcloud auth login
   gcloud config set project geminidev-479406
   gcloud auth application-default login
   ```

3. **In Antigravity**:
   - Click **"Gemini CLI Config"** card
   - Click **"Sync Config Now"** button
   - Antigravity will read from `~/.config/gcloud/`
   - Status should change to "Synced" (green)

### Option 2: Manual GLM Provider (Like z.ai)

1. **Get Gemini API Key**:
   - Visit: https://aistudio.google.com/app/apikey
   - Click "Create API Key"
   - Copy the key

2. **In Antigravity**:
   - Scroll down below z.ai provider
   - Look for **"Add Provider"** or **"+"** button
   - Configure:
     - **Provider Name**: `gemini`
     - **Provider Type**: `Google Gemini` or `GLM`
     - **Base URL**: `https://generativelanguage.googleapis.com/v1beta`
     - **API Key**: [paste your key]
     - **Dispatch Mode**: `Off` (or `On` for load balancing)
     - **Models**: 
       - `gemini-2.0-flash-exp`
       - `gemini-1.5-pro-latest`
       - `gemini-1.5-flash-latest`

3. **Enable the provider** (toggle on the right)

### Option 3: Use Vertex AI (GCP Project)

If you want to use your GCP project `geminidev-479406`:

1. **Enable Vertex AI API**:
   ```bash
   gcloud services enable aiplatform.googleapis.com \
     --project=geminidev-479406
   ```

2. **Create service account**:
   ```bash
   gcloud iam service-accounts create antigravity-gemini \
     --project=geminidev-479406
   
   gcloud projects add-iam-policy-binding geminidev-479406 \
     --member="serviceAccount:antigravity-gemini@geminidev-479406.iam.gserviceaccount.com" \
     --role="roles/aiplatform.user"
   
   gcloud iam service-accounts keys create ~/.config/antigravity/gemini-key.json \
     --iam-account=antigravity-gemini@geminidev-479406.iam.gserviceaccount.com
   ```

3. **In Antigravity**:
   - Add provider with:
     - **Base URL**: `https://us-central1-aiplatform.googleapis.com/v1`
     - **Project ID**: `geminidev-479406`
     - **Service Account Key**: `~/.config/antigravity/gemini-key.json`

## Test the Setup

Once configured, test with:

```bash
curl http://127.0.0.1:8045/v1/chat/completions \
  -H "Authorization: Bearer sk-28da1542217448069593b22690c561ca" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gemini-2.0-flash-exp",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

Or with Python:

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:8045/v1",
    api_key="sk-28da1542217448069593b22690c561ca"
)

response = client.chat.completions.create(
    model="gemini-2.0-flash-exp",
    messages=[{"role": "user", "content": "Hello from Gemini!"}]
)

print(response.choices[0].message.content)
```

## Understanding the Interface

### ONE-CLICK CLI SYNC
- **Claude Code Config**: Syncs from `~/.claude/` or Claude CLI
- **Codex AI Config**: Syncs from Codex CLI config
- **Gemini CLI Config**: Syncs from `~/.config/gcloud/`

When you click "Sync Config Now":
- Antigravity reads credentials from CLI tool configs
- Automatically configures the provider
- No need to manually enter API keys

### GLM Provider (like z.ai)
This is a **manually configured provider**:
- **Base URL**: API endpoint
- **API Key**: Authentication
- **Dispatch Mode**: 
  - `Off`: Direct routing
  - `On`: Load balancing across multiple accounts
- **Model Mapping**: Which models are available
- **Fetch models**: Auto-discover available models

### MCP Servers
Model Context Protocol servers that can be exposed through the proxy.

## Recommended Approach

**For quick testing**: Use Option 1 (One-Click CLI Sync)
- Fastest setup
- Uses gcloud credentials
- Auto-syncs

**For production**: Use Option 2 (Manual GLM Provider)
- More control
- Dedicated API key
- Better for rate limiting

**For enterprise**: Use Option 3 (Vertex AI)
- GCP project integration
- Better quotas
- Audit logging
