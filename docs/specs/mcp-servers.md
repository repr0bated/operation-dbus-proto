# MCP Servers — Requirements & Detailed Specification

## Overview

This project contains four MCP (Model Context Protocol) server implementations, each serving a distinct role in the system. They are all written in Rust and built on top of the shared `op-mcp` crate infrastructure.

---

## 1. op-mcp — Unified MCP Protocol Server

**Crate**: `crates/op-mcp`  
**Version**: 0.4.0  
**Binaries**: `op-mcp-server`, `op-mcp-compact`, `op-mcp-agents`

### Purpose

The primary MCP server for the project. It is a thin protocol adapter that exposes op-dbus system tools to MCP clients (Claude, Cursor, Gemini CLI, etc.) via JSON-RPC 2.0. All tool logic is delegated to `op-tools`, `op-introspection`, and `op-chat`.

### Requirements

- **REQ-MCP-01**: Implement MCP protocol version `2024-11-05` over JSON-RPC 2.0.
- **REQ-MCP-02**: Support four transport modes: stdio, HTTP+SSE, WebSocket, and gRPC.
- **REQ-MCP-03**: Support three server modes: `compact`, `full`, and `agents`.
- **REQ-MCP-04**: In compact mode, expose exactly 4 meta-tools (`list_tools`, `search_tools`, `get_tool_schema`, `execute_tool`) to stay under Cursor's 40-tool limit.
- **REQ-MCP-05**: In full mode, expose all registered tools up to a configurable `max_tools` limit (default: 500).
- **REQ-MCP-06**: In agents mode, connect to D-Bus (system or session bus) and expose agent tools.
- **REQ-MCP-07**: Auto-detect compact mode for known clients: Gemini, Claude, Cursor.
- **REQ-MCP-08**: Block a configurable set of destructive tools by default (`shell_execute`, `write_file`, `systemd_start/stop/restart/enable/disable`).
- **REQ-MCP-09**: Support running multiple transports simultaneously via `--all` flag.
- **REQ-MCP-10**: All log output must go to stderr; JSON-RPC responses go to stdout.
- **REQ-MCP-11**: gRPC transport must be feature-gated behind the `grpc` feature flag.
- **REQ-MCP-12**: Expose MCP `resources/list` and `resources/read` endpoints backed by a `ResourceRegistry`.

### Server Modes

| Mode | Tools Exposed | Use Case |
|------|--------------|----------|
| `compact` | 4 meta-tools | LLM clients (default) |
| `full` | All registered tools | Direct tool access |
| `agents` | D-Bus agent tools | Agent orchestration |
| `grpc` | Compact via gRPC | High-performance internal |
| `grpc-agents` | Agents via gRPC | Internal agent calls |

### Transport Addresses (defaults)

| Transport | Default Address |
|-----------|----------------|
| stdio | stdin/stdout |
| HTTP+SSE | `0.0.0.0:3001` |
| WebSocket | `0.0.0.0:3002` |
| gRPC | `0.0.0.0:50051` |
| gRPC-agents | `0.0.0.0:50052` |

### MCP Protocol Methods

| Method | Handler |
|--------|---------|
| `initialize` | Handshake, detect client, set compact mode |
| `initialized` | Acknowledge (no-op) |
| `ping` | Health check |
| `tools/list` | Return tool list (compact or full) |
| `tools/call` | Execute a named tool |
| `resources/list` | List documentation resources |
| `resources/read` | Read a resource by URI |
| `list_tools` | Compact meta-tool: browse tools |
| `search_tools` | Compact meta-tool: keyword search |
| `get_tool_schema` | Compact meta-tool: get input schema |
| `execute_tool` | Compact meta-tool: run any tool |

### Tool Categories (built-in)

- `filesystem` — file read/write/list
- `shell` — command execution
- `system` — system info, process management
- `systemd` — service management
- `ovs` — Open vSwitch operations
- `plugin` — plugin management
- `qdrant` — vector store queries

### McpServerConfig Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | `Option<String>` | None | Server name override |
| `compact_mode` | `bool` | false | Force compact mode |
| `allowed_categories` | `Option<Vec<String>>` | None | Whitelist categories |
| `blocked_patterns` | `Vec<String>` | (see above) | Tool name blocklist |
| `max_tools` | `usize` | 500 | Max tools in full mode |

### Key Dependencies

- `op-core`, `op-tools`, `op-plugins`, `op-introspection`, `op-state`, `op-state-store`, `op-chat`
- `axum 0.7` (HTTP+WebSocket), `tonic` (gRPC), `tokio`, `simd-json`, `serde`

---

## 2. op-mcp-aggregator — Multi-Server Aggregator

**Crate**: `crates/op-mcp-aggregator`  
**Version**: workspace  
**Binaries**: none (library crate)

### Purpose

Aggregates tools from multiple upstream MCP servers behind a single endpoint. Solves the Cursor 40-tool limit by providing profile-based tool filtering and compact mode. Deployed at `https://op-dbus.ghostbridge.tech/mcp/compact`.

### Requirements

- **REQ-AGG-01**: Connect to multiple upstream MCP servers via SSE, stdio, or WebSocket transports.
- **REQ-AGG-02**: Cache tool schemas with LRU eviction and configurable TTL.
- **REQ-AGG-03**: Support named profiles that select subsets of tools from upstream servers.
- **REQ-AGG-04**: Enforce a per-profile `max_tools` limit (default: 40) to stay within Cursor's constraint.
- **REQ-AGG-05**: Support tool name prefixing to avoid collisions between upstream servers.
- **REQ-AGG-06**: Support wildcard patterns (`github_*`) in `include_tools` and `exclude_tools`.
- **REQ-AGG-07**: Support compact mode exposing 4 meta-tools over all aggregated tools.
- **REQ-AGG-08**: Support bearer token, basic auth, and custom header authentication per upstream server.
- **REQ-AGG-09**: Support `${VAR_NAME}` environment variable substitution in config values.
- **REQ-AGG-10**: Provide background schema refresh to keep tool lists current.
- **REQ-AGG-11**: Load configuration from `/etc/op-dbus/aggregator.json`.
- **REQ-AGG-12**: Route `tools/call` requests to the correct upstream server based on tool origin.

### Architecture

```
MCP Client (Cursor/Claude)
        │
        ▼
op-mcp-aggregator
  ├── Profile Manager  (/mcp/profile/<name>)
  ├── Tool Cache (LRU + TTL)
  └── Upstream Clients
        ├── local op-dbus  (SSE)
        ├── GitHub MCP     (SSE + bearer token)
        ├── Postgres MCP   (SSE)
        └── Custom servers (any transport)
```

### Module Structure

| Module | Responsibility |
|--------|---------------|
| `config` | Load and parse `aggregator.json` |
| `aggregator` | Core aggregation logic, tool routing |
| `client` | Upstream MCP server HTTP client |
| `cache` | LRU + TTL tool schema cache |
| `profile` | Named profile management |
| `compact` | 4 meta-tool compact mode |
| `groups` | (unused) Tool group management |

### Server Configuration Schema

```json
{
  "servers": [{
    "id": "string",
    "name": "string",
    "url": "string",
    "transport": "sse | stdio | websocket",
    "enabled": true,
    "tool_prefix": "string?",
    "include_tools": ["string"],
    "exclude_tools": ["string"],
    "priority": 100,
    "timeout_secs": 30,
    "auth": { "type": "bearer | basic | header", "token": "string" }
  }],
  "profiles": {
    "<name>": {
      "description": "string",
      "servers": ["server_id"],
      "include_tools": ["pattern"],
      "exclude_tools": ["pattern"],
      "include_categories": ["string"],
      "include_namespaces": ["string"],
      "max_tools": 35
    }
  },
  "default_profile": "string",
  "max_tools_per_profile": 40
}
```

### Profile URL Routing

| URL Path | Description |
|----------|-------------|
| `/mcp/profile/sysadmin` | System admin tools |
| `/mcp/profile/dev` | Development tools |
| `/mcp/profile/minimal` | Essential tools only |
| `/mcp/compact` | Compact 4 meta-tool mode |

### Key Dependencies

- `op-core`, `op-tools`, `op-plugins`
- `reqwest` (upstream HTTP client), `tokio`, `serde`, `serde_yaml`, `simd-json`

---

## 3. op-mcp-proxy — LLM Proxy / gRPC Bridge

**Crate**: `crates/op-mcp-proxy`  
**Version**: 0.1.0  
**Binaries**: `op-mcp-proxy` (main.rs is the binary)

### Purpose

A thin stdio-based MCP proxy with two operating modes:

1. **Proxy mode** (default): Forwards MCP JSON-RPC requests to the op-dbus daemon over gRPC.
2. **Direct mode** (`DIRECT_MODE=1`): Handles LLM generation requests directly via Google Cloud AI Companion (`cloudcode-pa.googleapis.com`), bypassing the daemon.

### Requirements

- **REQ-PROXY-01**: Read JSON-RPC requests from stdin, write responses to stdout (stdio transport only).
- **REQ-PROXY-02**: In proxy mode, forward all requests to the op-dbus gRPC daemon at `OP_DBUS_ADDR` (default: `http://[::1]:50051`).
- **REQ-PROXY-03**: In direct mode (`DIRECT_MODE` env var set), handle `completion/complete`, `sampling/createMessage`, and `generate` methods locally via Cloud AI Companion.
- **REQ-PROXY-04**: In direct mode, expose a single `generate` tool that calls Gemini via Cloud AI Companion.
- **REQ-PROXY-05**: Authenticate to Cloud AI Companion using Google Cloud credentials with automatic token refresh.
- **REQ-PROXY-06**: Persist session state in SQLite (via `rusqlite`) for credential caching.
- **REQ-PROXY-07**: Support `initialize` and `tools/list` in direct mode with minimal capability surface.
- **REQ-PROXY-08**: Return proper JSON-RPC 2.0 error responses for unsupported methods.
- **REQ-PROXY-09**: All log output must go to stderr.

### Operating Modes

| Mode | Trigger | Behavior |
|------|---------|----------|
| Proxy | Default | Forward all requests to gRPC daemon |
| Direct | `DIRECT_MODE=1` | Handle LLM calls locally via Cloud AI Companion |

### Direct Mode Tool

```json
{
  "name": "generate",
  "description": "Generate text using Gemini via Cloud AI Companion",
  "inputSchema": {
    "type": "object",
    "properties": {
      "prompt": { "type": "string" },
      "model": { "type": "string" }
    },
    "required": ["prompt"]
  }
}
```

### Module Structure

| Module | Responsibility |
|--------|---------------|
| `main` | Stdio loop, request routing |
| `direct_llm` | Cloud AI Companion LLM client |
| `cloudaicompanion` | API types and request formatting |
| `gcloud_auth` | Google Cloud credential management + auto-refresh |
| `session` | SQLite-backed session/credential persistence |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DIRECT_MODE` | unset | Enable direct LLM mode |
| `OP_DBUS_ADDR` | `http://[::1]:50051` | gRPC daemon address |

### Key Dependencies

- `op-cache` (gRPC proto types), `op-identity`
- `tonic 0.11` (gRPC client), `reqwest`, `rusqlite`, `tokio`
- `chrono`, `uuid`, `dirs`, `hostname`

---

## 4. op-cognitive-mcp — Cognitive Memory Server

**Crate**: `crates/op-cognitive-mcp`  
**Version**: 0.1.0  
**Binaries**: `op-cognitive-mcp` (deployed to `/usr/local/bin/op-cognitive-mcp`)

### Purpose

An MCP server providing the authoritative persistent memory and knowledge graph subsystem. It owns immutable event capture, graph composition in CozoDB, vector indexing in Qdrant, and graph-backed memory/query surfaces for agents, OpenClaw, and platform consumers. It replaces the earlier SQLite namespace/key-value cognitive memory design.

### Requirements

- **REQ-COG-01**: Implement the persistent memory spec as the sole authoritative memory server.
- **REQ-COG-02**: Use CozoDB for graph nodes, edges, graph schemas, and graph queries.
- **REQ-COG-03**: Use Qdrant for vector storage and semantic similarity search.
- **REQ-COG-04**: Maintain an append-only immutable event ledger with cryptographic hash chaining.
- **REQ-COG-05**: Support isolated user memory stores and control-plane memory views over the same event stream.
- **REQ-COG-06**: Replace the prior SQLite namespace/key-value cognitive memory implementation.
- **REQ-COG-07**: Serve via MCP transport and integrate cleanly with `op-mcp`, `op-mcp-aggregator`, `op-chat`, and `op-web`.
- **REQ-COG-08**: Do not use SQL as the storage model for graph nodes, edges, graph queries, or semantic memory.
- **REQ-COG-09**: Treat SQL-backed domains such as users, WireGuard keys, auth/session state, and future hosted applications as out of scope for this subsystem.

### Core Subsystems

| Subsystem | Responsibility |
|-----------|----------------|
| Immutable ledger | Capture all mutations as append-only, hash-chained events |
| CozoDB graph | Graph-native node/edge storage and traversal |
| Qdrant index | Reusable embeddings and semantic search |
| Consumer APIs | Memory stores, semantic search, chatbot recall, audit views |

### SQL Boundary

SQL is not part of `op-cognitive-mcp`'s memory or graph implementation. If the platform later hosts CRM-style apps, Slack-like apps, user directories, WireGuard key management, or auth/session tables, those may use SQL independently. They are not the persistent memory subsystem.

### Module Structure

| Module | Responsibility |
|--------|---------------|
| `server` | `CognitiveMcpServer` — wires ledger, graph, vector, and MCP transport |
| `cognitive_tools` | Graph/event/vector-aware cognitive memory tool surface |
| `memory_store` | Legacy SQLite CRUD store to be removed/replaced |
| `activity_filter` | (supplementary) Activity-based filtering |
| `embedding_worker` | (supplementary) Background embedding generation |

### Deployment

Registered in `deploy/config/mcp-servers.json` as:

```json
{
  "name": "op-cognitive-mcp",
  "description": "Persistent memory, knowledge graph, and cognitive context tools",
  "command": "/usr/local/bin/op-cognitive-mcp",
  "transport": "stdio",
  "enabled": true
}
```

### Key Dependencies

- `op-core`, `op-mcp`, `op-dynamic-loader`, `op-cache`
- `CozoDB`, `Qdrant`, `axum`, `tokio`, `simd-json`, `uuid`, `chrono`

---

## Cross-Cutting Requirements

- **REQ-CC-01**: All servers must implement MCP protocol version `2024-11-05`.
- **REQ-CC-02**: All servers must use `simd-json` for JSON serialization/deserialization.
- **REQ-CC-03**: All servers must write logs to stderr, never stdout.
- **REQ-CC-04**: All servers must handle malformed JSON-RPC requests gracefully with error code `-32700`.
- **REQ-CC-05**: All servers must respond to `ping` with an empty result object.
- **REQ-CC-06**: Tool execution errors must be returned as `isError: true` content responses, not JSON-RPC errors.

---

## Deployment Configuration

The `deploy/config/mcp-servers.json` file defines the full set of MCP servers available to the system:

| Server | Transport | Enabled | Notes |
|--------|-----------|---------|-------|
| `op-dbus` | HTTP SSE (`127.0.0.1:8080`) | Yes | Main system tools |
| `filesystem` | stdio (npx) | Yes | File system access |
| `memory` | stdio (npx) | No / remove | Legacy duplicate memory server |
| `sequential-thinking` | stdio (npx) | Yes | Step-by-step reasoning |
| `fetch` | stdio (npx) | Yes | HTTP requests |
| `github` | stdio (npx) | No | Requires `GITHUB_PERSONAL_ACCESS_TOKEN` |
| `brave-search` | stdio (npx) | No | Requires `BRAVE_API_KEY` |
| `puppeteer` | stdio (npx) | No | Requires headless Chrome |
| `postgres` | stdio (npx) | No | Requires connection string |
| `op-cognitive-mcp` | stdio | Yes | Cognitive memory |

---

## Relationship Between Servers

```
MCP Client
    │
    ├──► op-mcp-server (compact/full/agents)
    │         └── delegates to op-tools, op-chat, op-introspection
    │
    ├──► op-mcp-aggregator
    │         └── proxies to op-mcp-server + external MCP servers
    │
    ├──► op-mcp-proxy
    │         ├── proxy mode → gRPC → op-dbus daemon
    │         └── direct mode → Cloud AI Companion (Gemini)
    │
    └──► op-cognitive-mcp
              └── Immutable ledger + CozoDB graph + Qdrant vectors
```
