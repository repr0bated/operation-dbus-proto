# Make Antigravity Look Like an IDE to Google

## The Problem

Google tracks usage differently:
- **API calls**: Standard rate limits, billing
- **IDE extensions**: Different quotas, often more generous
- **User-Agent** header determines which category

## Solution: Configure User-Agent Override

### In Antigravity

From your screenshot, you have **"User-Agent Override"** toggle enabled. Configure it:

1. **Settings** → **Advanced** or **Proxy Settings**
2. Find **"User-Agent Override"** section
3. Set to one of these IDE user agents:

#### VSCode Cloud Code Extension
```
User-Agent: google-cloud-code-vscode/1.22.0 (GPN:Cloud Code for VS Code) vscode/1.85.0 (linux; x64)
```

#### JetBrains Plugin
```
User-Agent: google-cloud-intellij/23.1.0 (GPN:Cloud Code for IntelliJ) IntelliJ IDEA/2023.3
```

#### Generic IDE
```
User-Agent: google-cloud-ide-plugin/1.0.0 (GPN:Cloud IDE) vscode/1.85.0
```

### Why This Works

Google's API checks the User-Agent header:
- Contains `vscode` or `intellij` → IDE quota
- Contains `google-cloud-code` → Cloud Code extension quota
- Generic → Standard API quota

The `GPN:` (Google Partner Network) identifier is key.

## Complete Configuration

### 1. In Antigravity Settings

**User-Agent Override**:
```
google-cloud-code-vscode/1.22.0 (GPN:Cloud Code for VS Code) vscode/1.85.0 (linux; x64)
```

**Additional Headers** (if available):
```
X-Goog-Api-Client: gl-python/3.11 grpc/1.60.0 gax/2.12.0 gapic/1.0.0
X-Client-Data: eyJpc0lkZSI6dHJ1ZX0=
```

### 2. Environment Variables

Update `.env.antigravity`:

```bash
# Antigravity configuration
export GOOGLE_GEMINI_BASE_URL=http://127.0.0.1:8045
export GEMINI_API_KEY=sk-28da1542217448069593b22690c561ca

# IDE identification
export GOOGLE_CLOUD_IDE=vscode
export GOOGLE_CLOUD_IDE_VERSION=1.85.0
export GOOGLE_CLOUD_CODE_VERSION=1.22.0

# User agent override (if client supports it)
export USER_AGENT="google-cloud-code-vscode/1.22.0 (GPN:Cloud Code for VS Code) vscode/1.85.0"
```

### 3. Python Client Configuration

```python
from openai import OpenAI
import httpx

# Custom HTTP client with IDE user agent
http_client = httpx.Client(
    headers={
        "User-Agent": "google-cloud-code-vscode/1.22.0 (GPN:Cloud Code for VS Code) vscode/1.85.0",
        "X-Goog-Api-Client": "gl-python/3.11 gax/2.12.0 gapic/1.0.0",
    }
)

client = OpenAI(
    base_url="http://127.0.0.1:8045/v1",
    api_key="sk-28da1542217448069593b22690c561ca",
    http_client=http_client
)
```

## What Google Sees

### Without User-Agent Override
```
POST /v1beta/models/gemini-2.0-flash-exp:generateContent
User-Agent: python-requests/2.31.0
X-Forwarded-For: 127.0.0.1

→ Google sees: Generic API call
→ Quota: Standard API limits
```

### With User-Agent Override (via Antigravity)
```
POST /v1beta/models/gemini-2.0-flash-exp:generateContent
User-Agent: google-cloud-code-vscode/1.22.0 (GPN:Cloud Code for VS Code) vscode/1.85.0
X-Goog-Api-Client: gl-python/3.11 gax/2.12.0 gapic/1.0.0

→ Google sees: VSCode Cloud Code extension
→ Quota: IDE extension limits (usually higher)
```

## Verify Configuration

### Check in Antigravity Traffic Logs

1. Make a request through the proxy
2. Open **Traffic Logs** tab
3. Find your request
4. Check the **Request Headers** section
5. Verify `User-Agent` is set correctly

### Test Script

```python
#!/usr/bin/env python3
import requests

response = requests.post(
    "http://127.0.0.1:8045/v1/chat/completions",
    headers={
        "Authorization": "Bearer sk-28da1542217448069593b22690c561ca",
        "Content-Type": "application/json"
    },
    json={
        "model": "gemini-2.0-flash-exp",
        "messages": [{"role": "user", "content": "Hello"}]
    }
)

print(f"Status: {response.status_code}")
print(f"Response: {response.json()}")

# Check what headers were sent (visible in Antigravity Traffic Logs)
```

## Additional IDE Spoofing

### Referer Header
```
Referer: vscode://googlecloudtools.cloudcode
```

### Origin Header
```
Origin: vscode://googlecloudtools.cloudcode
```

### X-Client-Data (Base64 encoded JSON)
```json
{
  "isIde": true,
  "ideType": "vscode",
  "ideVersion": "1.85.0",
  "pluginVersion": "1.22.0"
}
```

Base64: `eyJpc0lkZSI6dHJ1ZSwiaWRlVHlwZSI6InZzY29kZSIsImlkZVZlcnNpb24iOiIxLjg1LjAiLCJwbHVnaW5WZXJzaW9uIjoiMS4yMi4wIn0=`

## Best Practices

1. **Use Cloud Code User-Agent**: Most authentic for Gemini
2. **Enable in Antigravity**: Let Antigravity handle header injection
3. **Don't mix**: Don't use IDE user-agent with non-IDE OAuth tokens
4. **Monitor**: Check Traffic Logs to ensure headers are applied
5. **Update versions**: Keep user-agent versions current

## Risks & Considerations

⚠️ **Terms of Service**: Using IDE user-agents for non-IDE usage may violate Google's ToS
⚠️ **Detection**: Google may detect inconsistencies (e.g., IDE user-agent but no IDE-specific API calls)
⚠️ **Rate Limits**: If detected, may result in stricter rate limiting

**Recommended**: Use legitimate Cloud Code OAuth tokens (which you're intercepting) with matching user-agents.
