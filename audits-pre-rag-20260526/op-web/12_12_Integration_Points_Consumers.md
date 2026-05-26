### Integration Report

#### Crates in Workspace Cargo.toml Depending on `op-web`
* **`op-dbus`** (root package): Declares a workspace dependency on `op-web` via `op-web.workspace = true` in its `[dependencies]`.

---

#### Registered D-Bus Service Names and Object Paths
The `op-web` crate does not register its own D-Bus service names or object paths. However, it acts as a D-Bus client proxy and interacts with the following external service:
* **Service Name**: `org.opdbus.v1`
* **Object Path**: `/org/opdbus/v1/state`
* **Interface**: `org.opdbus.StateManager`
* **Citations**: `crates/op-web/src/state_manager_client.rs:15`

---

#### Exposed HTTP/gRPC Endpoints

##### HTTP REST API & SSE Endpoints (`crates/op-web/src/routes/mod.rs`)
* `GET /api/health` — Health check status
* `GET /api/status` — Comprehensive system status
* `GET /api/dashboard/metrics` — Dashboard overview metrics
* `GET /api/users` — List registered users
* `GET /api/users/:id` — Get specific user details
* `GET /api/vpn/status` — WireGuard VPN server status
* `GET /api/vpn/connections` — List active VPN connections
* `GET /api/vpn/config` — Retrieve global WireGuard configuration
* `GET /api/mail/status` — Mail server status
* `GET /api/mail/queue` — Retrieve outgoing mail queue
* `GET /api/mail/accounts` — List email accounts
* `GET /api/logs` — Fetch recent logs from disk
* `GET /api/logs/stream` — SSE endpoint for live log streaming
* `POST /api/chat` — Unified orchestrator blocking chat response
* `POST /api/chat/stream` — SSE endpoint for streaming chat responses
* `GET /api/chat/sessions` — List conversation sessions
* `POST /api/chat/sessions` — Create a new conversation session
* `DELETE /api/chat/sessions/:id` — Delete a conversation session
* `POST /api/chat/message` — Send message (creates a session if none provided)
* `GET /api/chat/history/:session_id` — Fetch history for a session
* `POST /api/chat/transcript` — Save session transcript to disk
* `GET /api/chat/system-prompt` — View system prompt (immutable & custom)
* `PUT /api/chat/system-prompt` — Update custom system prompt
* `GET /api/tools` — List all registered tools and categories
* `GET /api/tools/:name` — Get schema for a specific tool
* `POST /api/tool` — Execute a tool directly by payload
* `POST /api/tools/:name/execute` — Execute a named tool with arguments
* `GET /api/agents` — List running cognitive agent instances
* `POST /api/agents` — Spawn a new cognitive agent instance
* `GET /api/agents/types` — List available agent templates
* `GET /api/agents/:id` — Get agent runtime status
* `DELETE /api/agents/:id` — Terminate a running agent instance
* `GET /api/llm/status` — Get active LLM provider and model
* `GET /api/llm/providers` — List configured LLM providers
* `GET /api/llm/models` — List available models for current provider
* `GET /api/llm/models/:provider` — List models for a specified provider
* `POST /api/llm/provider` — Switch current active LLM provider
* `POST /api/llm/model` — Switch active LLM model
* `GET /api/openclaw/status` — Health check OpenClaw gateway
* `GET /api/openclaw/config` — View OpenClaw configuration
* `POST /api/openclaw/chat` — Proxy chat requests directly to OpenClaw
* `GET /api/openclaw/models` — List OpenClaw route keys
* `GET /api/mcp/servers` — List running Model Context Protocol servers
* `GET /api/mcp/servers/:id` — Fetch status for a specific MCP server
* `GET /api/mcp/cognitive/agents` — List available cognitive agents
* `POST /api/mcp/cognitive/agents` — Configure enabled/active cognitive agents
* `POST /api/mcp/cognitive/memory` — Query cognitive memory store
* `DELETE /api/mcp/cognitive/memory/:key` — Delete memory entry
* `GET /api/mcp/cognitive/memory/stats` — Get memory storage statistics
* `GET /api/mcp/_config` — Generate legacy configuration for clients
* `GET /api/events` — Standard SSE system event broadcaster
* `POST /api/privacy/signup` — Register email and dispatch magic link
* `GET /api/privacy/verify` — Token verification endpoint returning WireGuard credentials
* `GET /api/privacy/config/:user_id` — Fetch WireGuard config for authorized users
* `GET /api/privacy/status` — Verify privacy router configuration status
* `POST /api/privacy/credentials` — Assign LLM API keys to a specific user
* `GET /api/privacy/google/auth` — Trigger Google OAuth flow
* `GET /api/privacy/google/callback` — Google OAuth authentication callback
* `GET /privacy/verify` — User-facing token verification redirect
* `GET /privacy/access` — HTML page displaying client WireGuard configuration
* `POST /jsonrpc` & `POST /rpc` — JSON-RPC compatibility mirrors of standard MCP
* `GET /.well-known/mcp.json` — Auto-discovery configuration for MCP clients

##### Model Context Protocol (MCP) Endpoints (`crates/op-web/src/mcp.rs`)
* `POST /mcp` — Standard MCP handler
* `GET /mcp/sse` — SSE transport connection endpoint
* `POST /mcp/message` — Receive messages over SSE transport
* `GET /mcp/_config` — Standard configuration helper
* `GET /mcp/compact` — SSE transport for compact meta-tool mode
* `POST /mcp/compact/message` — Message handler for compact mode
* `GET /mcp/agents` — SSE transport for cognitive agents mode
* `POST /mcp/agents/message` — Message handler for cognitive agents mode

##### WebSocket Endpoints (`crates/op-web/src/websocket.rs`)
* `GET /ws` — Bidirectional real-time chat interface

##### Tool Groups Admin UI Endpoints (`crates/op-web/src/groups_admin.rs`)
* `GET /groups-admin/` — Admin web interface HTML
* `GET /groups-admin/api/groups` — Get all domain-sorted tool groups
* `GET /groups-admin/api/presets` — List built-in presets
* `GET /groups-admin/api/profiles` — List saved profiles
* `GET /groups-admin/api/profiles/:name` — Fetch configuration for a profile
* `POST /groups-admin/api/profiles/:name` — Save configuration for a profile
* `GET /groups-admin/api/access-zone` — Check active IP access zone status
* `GET /groups-admin/api/trusted-networks` — List customized trusted networks
* `POST /groups-admin/api/trusted-networks` — Save a trusted network prefix

##### Admin UI Endpoints (`crates/op-web/src/routes/admin.rs`)
* `GET /admin/prompt` — Retrieve generated system prompt components
* `GET /admin/prompt/custom` — Fetch custom system prompt part
* `POST /admin/prompt/custom` — Save custom system prompt part
* `POST /admin/prompt/test` — Test prompt changes for length and vulnerabilities
* `POST /admin/prompt/reload` — Clear custom prompt cache
* `GET /admin/config` — Fetch general administrative configuration

##### PTY Auth Bridge Endpoints (`crates/op-web/src/handlers/auth_bridge.rs`)
* `GET /auth-bridge` — Admin web interface HTML
* `GET /api/auth-bridge/pending` — Fetch pending auth requirements
* `POST /api/auth-bridge/webhook` — Webhook for incoming headless requests
* `POST /api/auth-bridge/:id/complete` — Mark an auth request as completed

##### gRPC Endpoints (`crates/op-web/src/bin/op-dbus.rs`)
The `op-dbus` binary binds a gRPC server at `10.200.0.2:50051` (controlled via `OP_DBUS_GRPC_LISTEN`). This exposes the following protocols internally via `op-grpc-bridge`:
* System state subscription and query APIs.
* Central execution event tracking and streaming APIs.

---

#### Circular Dependency Risks
* **Binary-to-Library Structural Separation**: The binary `crates/op-web/src/bin/op-dbus.rs` resides within the `op-web` library crate. While cargo allows binaries to depend on their sibling libraries, the root package `op-dbus` (workspace Cargo.toml) also depends on `op-web`. This creates namespace confusion and raises circular structural dependency risks if any other crate depends on the root `op-dbus` package.
* **Implicit Cycles via Workspace Crates**: `op-web` depends on `op-grpc-bridge`, which depends on `op-cognitive-mcp`. If `op-cognitive-mcp` is modified to depend back on `op-web` to invoke rest APIs, a silent compile-time dependency loop will manifest.

---

### Security and Quality Audit Findings

#### CRITICAL: Unauthenticated Arbitrary File Write & Path Traversal via Transcript Handler
* **File/Line**: `crates/op-web/src/handlers/chat.rs:482`
* **Exploitability**: Directly exploitable. The `/api/chat/transcript` endpoint is exposed with no authentication checks. It accepts a user-provided JSON payload containing a `filename` string and a `messages` array. The handler maps the `filename` directly to a file path via `format!("/tmp/{}", filename)` and writes the formatted transcript using `tokio::fs::write`. An attacker can pass directory traversal sequences (such as `"filename": "../../../etc/cron.d/malicious_job"`) and write arbitrary files anywhere on the filesystem. Since the server runs with elevated privileges to orchestrate D-Bus and container operations, this allows an immediate unauthenticated remote attacker to gain Root Code Execution (RCE).
* **Vulnerable Code**:
```rust
let filepath = format!("/tmp/{}", filename);
match tokio::fs::write(&filepath, &transcript).await { ... }
```

#### CRITICAL: Complete Server-Side Authentication Bypass on Sensitive APIs
* **File/Line**: `crates/op-web/src/routes/mod.rs:37`
* **Exploitability**: Directly exploitable. The `ip_security_middleware` applied to the router is purely informational; it determines the client's `AccessZone` and inserts it into the request extensions, but *never* aborts the request or returns an error. The actual backend handlers (such as `/api/tool`, `/api/tools/:name/execute`, `/api/agents`, `/admin/prompt/custom`, and `/groups-admin/api/profiles/:name`) never read or enforce the `AccessZone` extension. As a result, any public internet client can invoke arbitrary D-Bus tools (including `shell_exec`), spawn malicious agents, modify system configurations, or write system prompts with zero authentication.
* **Vulnerable Code**:
```rust
router.layer(axum::middleware::from_fn(security::ip_security_middleware))
```

#### CRITICAL: Dead Middleware Implementation Bypassing Rate Limiting & Security Layers
* **File/Line**: `crates/op-web/src/main.rs:31`
* **Exploitability**: Directly exploitable. The main entry-point binary `op-web-server` initializes by calling `routes::create_router(state)` and serves it directly. This completely bypasses the `WebServer` wrapper implemented in `server.rs`, which is responsible for applying the rate limiter (`tower_governor`), CORS configurations, and tracing layers. Because `WebServer` is dead code, the production server runs without any denial-of-service protections or CORS restrictions.
* **Vulnerable Code**:
```rust
let app = routes::create_router(state);
// ...
axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
```

#### HIGH: Infinite Background Thread / Resource Leak in SSE Log Streaming
* **File/Line**: `crates/op-web/src/handlers/logs.rs:136`
* **Exploitability**: Every time a client initiates a connection to `/api/logs/stream`, a background `tokio::spawn` task is created to watch files using `linemux`. The task runs an infinite loop `while let Ok(Some(line)) = lines.next_line().await`. However, there is no mechanism to detect when the HTTP client disconnects. The spawned thread runs indefinitely, continuously polling the file system. An attacker can repeatedly open connections to exhaust system memory, CPU cycles, and inotify file descriptors, causing a system-wide Denial of Service.

#### HIGH: Storing Plaintext WireGuard Private Keys on Disk
* **File/Line**: `crates/op-web/src/handlers/privacy.rs:129`
* **Exploitability**: During user sign-up, the server generates a fresh WireGuard keypair. The comment states `// Create user (we'll encrypt the private key later, for now just store it)`, but the code passes the plaintext `keypair.private_key` directly into the database's `wg_private_key_encrypted` parameter. This plaintext key is written directly to `/var/lib/op-dbus/privacy-users.json` on disk, leaving private keys permanently exposed to any unauthorized process or local user with read access to the database.

#### HIGH: Hardcoded Bypass API Keys in Production Security Middleware
* **File/Line**: `crates/op-web/src/middleware/security.rs:16`
* **Exploitability**: The security middleware contains hardcoded API keys (`"4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f"`, `"test-key-huggingface-2024"`) intended to bypass IP security restrictions and grant full `TrustedMesh` access. Any client supplying these keys in the `X-API-Key` or `Authorization` headers will be automatically granted administrative capabilities, regardless of their source network zone.

#### MEDIUM: Global Shared OAuth CSRF Token Pool fixation
* **File/Line**: `crates/op-web/src/handlers/privacy.rs:448`
* **Exploitability**: The Google OAuth flow uses a shared global token map (`state.csrf_tokens`) to validate authorization states. Because the state is not bound to a specific browser session or secure cookie, any client can initiate an OAuth flow, register a token, and use it to complete a callback for any other user's session. This makes the implementation vulnerable to OAuth state fixation and Cross-Site Request Forgery (CSRF) attacks.

#### MEDIUM: Unsafe Deserialization of Untrusted Network Inputs
* **File/Line**: `crates/op-web/src/websocket.rs:102` (and others)
* **Exploitability**: The WebSocket and REST API handlers make frequent use of `unsafe { simd_json::from_str(...) }` on raw, untrusted strings received directly from the network. While `simd_json` is highly optimized, `unsafe` parsing bypasses key memory safety checks. If an attacker sends malformed UTF-8 payloads or exploits memory layout parsing flaws within the parser, it can result in undefined behavior, memory corruption, or unexpected crashes.

---
## ⚠ Citation Warnings
- `crates/op-web/src/handlers/chat.rs:482`: file has 453 lines
