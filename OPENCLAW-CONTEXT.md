

## What We're Doing

Integrating operation-dbus with OpenClaw gateway running in an Incus container. The goal is full integration of:
1. **gcloud ADC authentication** (already working on both sides)
2. **Chatbot LLM backend** - route op-dbus chatbot through OpenClaw's gateway
3. **Model switching** - expose all OpenClaw models to op-dbus chatbot

## OpenClaw Gateway (Running)

- **Endpoint:** `http://127.0.0.1:18789/v1/chat/completions` (OpenAI-compatible)
- **Auth token:** `b6fb8429d8fd9e615305261050aa67d97489d566842a8cec`
- **Service:** `openclaw.service` (systemd), runs as root
- **Config:** `/root/.openclaw/openclaw.json`
- **Codebase:** `/root/openclaw/` (Node.js/TypeScript)
- **GCP Project:** `dbus-enterprise-2026`
- **gcloud ADC:** Working, credentials at `/root/.config/gcloud/application_default_credentials.json`

### Available Models (from openclaw.json)

| Model ID | Alias |
|----------|-------|
| `google-gemini-cli/gemini-2.5-pro` | Pro |
| `google-gemini-cli/gemini-2.5-flash` | Flash |
| `google-gemini-cli/gemini-3-pro-preview` | G3 |
| `opencode/claude-opus-4-6` | Opus |
| `opencode/kimi-k2.5-free` | Kimi |

Primary model is currently `opencode/kimi-k2.5-free`.

## operation-dbus Architecture (Relevant Parts)

### LLM Provider System (`crates/op-llm/src/`)

- **`provider.rs`** - Defines `LlmProvider` trait (async_trait) with methods:
  - `provider_type()` -> `ProviderType` enum
  - `list_models()` -> `Vec<ModelInfo>`
  - `chat()` / `chat_with_request()` - chat completions with tool support
  - `chat_stream()` - streaming
- **`ProviderType` enum** - `Anthropic`, `Antigravity`, `Gemini`, `HuggingFace`, `OpenAI`, `Perplexity`
- **`gcloud_adc.rs`** - `GCloudADCProvider` hits Cloud AI Companion API using `gcloud auth application-default print-access-token`. Uses Gemini native format (not OpenAI-compatible).
- **`gemini.rs`** - Direct Gemini API client
- **`anthropic.rs`** - Anthropic API client
- **`antigravity.rs`** - Antigravity headless OAuth provider
- **`lib.rs`** - Re-exports all providers

### Chat System (`crates/op-chat/src/`)

- **`llm.rs`** - Has its OWN `LlmProvider` trait (different from op-llm's!) and `OpenAiProvider` struct for OpenAI-compatible APIs. Has `create_provider()` factory that currently only handles `"openai"` type.
- **`chat_loop.rs`** - `ForcedToolChatLoop<P: LlmProvider>` - forced tool execution loop (anti-hallucination). Default model: `deepseek-ai/DeepSeek-V2.5`.
- **`actor.rs`** - `ChatActor` central message processor
- **`system_prompt.rs`** - System prompt generation
- **`tool_loader.rs`** - Loads tools for chat

### Chatbot (`src/chatbot/`)

- **`mod.rs`** - `Chatbot` struct, cognitive control plane. Routes intents to handlers. Uses MCP dispatchers, NOT direct LLM calls.
- **`session.rs`** - Session management with `ChatSession`, `SessionManager`
- **`intent.rs`** - Intent classification
- **`cognitive.rs`** - Cognitive reasoning
- **`planner.rs`** - Workflow planning

### Key Design Principles (from AGENTS.md)

- **DBUS FIRST** - if it can be done with D-Bus, do it
- **gRPC for internal comms** where possible
- **simd-json** instead of serde for serialization
- **Native protocols only** - no shell commands (no ovs-vsctl, systemctl, etc.)
- Tools are the ONLY mutation mechanism
- Chatbot NEVER executes directly: `Chatbot -> MCP -> Orchestrator -> Tools`

## Implementation Plan

### 1. Add OpenClaw provider to `op-llm`

Create `crates/op-llm/src/openclaw.rs` implementing the `LlmProvider` trait from `provider.rs`. This wraps the OpenAI-compatible endpoint at `127.0.0.1:18789`.

Key details:
- Use reqwest to hit `/v1/chat/completions`
- Trust internal Incus/WireGuard reachability instead of gateway bearer auth
- `list_models()` should return the configured models
- `chat_with_request()` must handle tools (OpenAI function calling format)
- Add `OpenClaw` variant to `ProviderType` enum

### 2. Extend `op-chat` provider factory

Update `crates/op-chat/src/llm.rs` `create_provider()` to handle `"openclaw"` type, creating an `OpenAiProvider` pointed at the gateway URL.

### 3. Wire model switching

The chatbot needs a way to switch models at runtime. OpenClaw models use prefixed IDs like `google-gemini-cli/gemini-2.5-flash`. The `ChatLoopConfig.model` field controls this.

### 4. Environment variables

```bash
OPENCLAW_BASE_URL=http://127.0.0.1:18789/v1
OPENCLAW_DEFAULT_MODEL=google-gemini-cli/gemini-2.5-flash
LLM_PROVIDER=openclaw
```

## Files to Modify

| File | Change |
|------|--------|
| `crates/op-llm/src/openclaw.rs` | **NEW** - OpenClaw provider |
| `crates/op-llm/src/provider.rs` | Add `OpenClaw` to `ProviderType` enum |
| `crates/op-llm/src/lib.rs` | Add `pub mod openclaw;` and re-exports |
| `crates/op-chat/src/llm.rs` | Add `"openclaw"` to `create_provider()` factory |
| `crates/op-chat/src/chat_loop.rs` | Update default model config |
| `src/chatbot/mod.rs` | Wire OpenClaw provider option |
| `src/main.rs` | Add CLI flags/env for provider selection |

## Important: Do NOT push changes

The user explicitly said not to push any changes that aren't the latest version. Only commit locally, never push.

## Existing Access Flow

gcloud ADC is already authenticated and working for the agent-side model stack:
```
gcloud auth application-default print-access-token  # returns valid ya29.xxx token
```

Both OpenClaw and operation-dbus share the same GCP project credentials. OpenClaw's systemd service already has `GOOGLE_APPLICATION_CREDENTIALS` pointed at the ADC file. Gateway access itself is trusted via internal network isolation, not an extra bearer token.

## MCP Servers (Already Connected)

The op-dbus MCP servers run on the host (`10.149.181.1:8080`) and are already
registered in both OpenClaw (`/root/.openclaw/workspace/config/mcporter.json`)
and Claude Code (`~/.claude.json`) inside the Incus container `openclaw`.

| Display Name | Endpoint | Description |
|---|---|---|
| **compact** | `http://10.149.181.1:8080/mcp/compact` | 4 meta-tools for efficient tool discovery |
| **cognitive** | `http://10.149.181.1:8080/mcp/agents` | Memory, sequential thinking, context agents |
| **agents** | `http://10.149.181.1:8080/mcp/sse` | Full 145+ tools (D-Bus, OVS, rtnetlink, etc.) |

All use SSE transport. Host gateway IP is `10.149.181.1` (Incus bridge).

### Container Network Context

- Container hostname: `openclaw`
- Container IP: `10.149.181.114`
- Host gateway: `10.149.181.1`
- Tailscale IP: `100.89.214.55`
- OpenClaw gateway: `127.0.0.1:18789` (local) / `0.0.0.0:18789` (LAN)
- op-dbus MCP: `10.149.181.1:8080` (host)

### Cognitive MCP Crate (`crates/op-cognitive-mcp/`)

Separate binary (`op-cognitive-mcp`) for standalone cognitive memory server.
Currently NOT running as a service — the cognitive tools are served through
the agents endpoint on the main MCP server at `:8080/mcp/agents`.

Source files:
- `src/server.rs` - `CognitiveMcpServer`
- `src/memory_store.rs` - `CognitiveMemoryStore`
- `src/cognitive_tools.rs` - `CognitiveToolRegistry`
- `src/main.rs` - Standalone binary entry point
root@openclaw:/home/jeremy# 
