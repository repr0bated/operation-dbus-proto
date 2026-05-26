### License Audit

*   **Extracted License Field**: `Apache-2.0` (declared in workspace `Cargo.toml` as `license = "Apache-2.0"` and inherited by the `op-web` crate via `license.workspace = true`).
*   **GPL/AGPL/SSPL Scan**: No GPL, AGPL, or SSPL licensed crates were found in the scanned portion of `Cargo.lock`. The embedded `cozo` crate (v0.7.6) is licensed under the weak-copyleft `MPL-2.0` license, which is compatible with your proprietary/Apache-2.0 workspace.
*   **Missing Licenses**: All visible workspace member crates and external dependencies contain valid license declarations or inherit workspace-level licenses.

---

### Security & Quality Audit

#### [Critical] Path Traversal and Arbitrary File Write/Overwrite
*   **File:Line**: `crates/op-web/src/handlers/chat.rs:434` (invoked via `crates/op-web/src/handlers/chat.rs:376`)
*   **Details**: The `save_transcript_handler` accepts a client-provided JSON payload containing a `filename` field. This filename is concatenated directly into a path string: `let filepath = format!("/tmp/{}", filename);` without any validation or sanitization. 
*   **Impact**: An unauthenticated attacker can supply path traversal sequences (such as `../../etc/cron.d/malicious_job` or `../../root/.ssh/authorized_keys`) to write arbitrary content anywhere on the host filesystem that the web server process has write permissions to. This leads to immediate and trivial Remote Code Execution (RCE).
*   **Remediation**: Sanitize the filename to strip path traversal components (`..`, `/`), or force a securely generated random UUID/timestamp for the filename.

---

#### [Critical] Complete Authorization Bypass on Tool-Execution and MCP Endpoints
*   **File:Line**: `crates/op-web/src/mcp.rs:291` and `crates/op-web/src/mcp_compact.rs:387`
*   **Details**: The `ip_security_middleware` identifies the client's `AccessZone` and inserts it into the request extensions, but it **does not block or filter** any requests. The Model Context Protocol (MCP) endpoints (`/mcp`, `/mcp/compact`, and `/mcp/agents`) do not extract or validate the request's `AccessZone`.
*   **Impact**: Unauthenticated remote actors on the public internet can send JSON-RPC payloads to `/mcp` or `/mcp/compact/message` to call `execute_tool` with arbitrary registered tools (including powerful tools like `shell_exec`, systemd service controls, or OVS networking commands). This allows complete, unauthenticated remote control of the host.
*   **Remediation**: Check request extensions for `AccessZone` inside the JSON-RPC handlers or within a dedicated router guard layer, and reject requests that fall below `SecurityLevel::Standard` or `SecurityLevel::Elevated`.

---

#### [Critical] Missing Authentication and Access Control on Admin & Tool Group Interfaces
*   **File:Line**: `crates/op-web/src/groups_admin.rs:265`, `crates/op-web/src/groups_admin.rs:327`, and `crates/op-web/src/routes/admin.rs:135`
*   **Details**: The endpoints under `/groups-admin` and `/admin` (such as `save_profile`, `add_trusted_network`, and `set_custom_prompt`) lack any form of session verification, token checks, or `AccessZone` validation.
*   **Impact**: Any client with network access to the web server can modify enabled tool groups, add trusted networks, or overwrite the LLM's custom system prompt (allowing system prompt injection or instruction hijacking).
*   **Remediation**: Apply authentication guards or restrict `/groups-admin` and `/admin` routes using router-level middleware that validates API keys or enforces that the client belongs to a trusted network/localhost zone.

---

#### [Defect] Compilation Failure: Syntax Error in WebSocket Event Streaming
*   **File:Line**: `crates/op-web/src/websocket.rs:105`
*   **Details**: The channel creation code contains a syntax error:
    ```rust
    let event_tx, mut event_rx) = mpsc::channel::<OrchestratorEvent>(100);
    ```
    The opening parenthesis `(` for the destructuring tuple is missing.
*   **Impact**: The `op-web` crate will fail to compile.
*   **Remediation**: Correct the line to:
    ```rust
    let (event_tx, mut event_rx) = mpsc::channel::<OrchestratorEvent>(100);
    ```

---

#### [Defect] Compilation Failure: Non-Existent Fields in Auth Bridge State
*   **File:Line**: `crates/op-web/src/handlers/auth_bridge.rs:69`, `crates/op-web/src/handlers/auth_bridge.rs:79`, and `crates/op-web/src/handlers/auth_bridge.rs:124`
*   **Details**: The auth bridge route handlers attempt to access `state.auth_bridge`:
    ```rust
    let bridge = &state.auth_bridge;
    ```
    However, the `AppState` struct defined in `crates/op-web/src/state.rs` does not contain any field named `auth_bridge`.
*   **Impact**: The crate will fail to compile.
*   **Remediation**: Add the `auth_bridge: Arc<AuthBridgeState>` field to `AppState` in `state.rs` and initialize it during `AppState::new_with_registry`.

---

#### [Defect] Compilation Failure: Arity Mismatch on Orchestrator Process Calls
*   **File:Line**: `crates/op-web/src/handlers/websocket.rs:77` and `crates/op-web/src/handlers/websocket.rs:104`
*   **Details**: The WebSocket handler calls `state_clone.orchestrator.process(&sid, &message)` passing only 2 arguments. However, the `process` function in `crates/op-web/src/orchestrator/process.rs` requires 3 arguments:
    ```rust
    pub async fn process(
        &self,
        _session_id: &str,
        input: &str,
        event_tx: Option<mpsc::Sender<OrchestratorEvent>>,
    ) -> Result<OrchestratorResponse>
    ```
*   **Impact**: The crate will fail to compile.
*   **Remediation**: Pass `None` as the third parameter to match the function signature:
    ```rust
    state_clone.orchestrator.process(&sid, &message, None).await
    ```

---

#### [Defect] Compilation Failure: Type Mismatch and Missing Functions in Smart Router
*   **File:Line**: `crates/op-web/src/mcp_smart_router.rs:81` and `crates/op-web/src/mcp_smart_router.rs:91`
*   **Details**:
    *   Line 81 passes `serde_json::Value::Null` to `mcp_agents_message_handler_stateless` which expects a custom `JsonRpcRequest` type, resulting in a type mismatch.
    *   Line 91 references `crate::mcp::get_app_state()`, which is not defined in `crates/op-web/src/mcp.rs`. It also passes `axum::extract::Json(serde_json::Value::Null)` instead of `Json<McpRequest>`.
*   **Impact**: The `mcp_smart_router` module is entirely broken and will not compile.
*   **Remediation**: Correctly pass the extracted request payloads of the appropriate type and extract the global `AppState` from the request extension or state wrapper rather than calling non-existent helper functions.

---

#### [Quality] Non-Atomic File Overwrites for System Configurations
*   **File:Line**: `crates/op-web/src/groups_admin.rs:106` and `crates/op-web/src/mcp_agents.rs:629`
*   **Details**: Both `save_to_disk` and `save_agent_config` overwrite vital system configurations using `tokio::fs::write` / `std::fs::write` directly on the target path.
*   **Impact**: If the system crashes, runs out of disk space, or loses power mid-write, the target JSON files will be left partially written or truncated, resulting in persistent corruption and boot-looping of the services on next start.
*   **Remediation**: Use a temporary file in the same directory, write the JSON content, call `sync_all()`, and then perform an atomic rename using `std::fs::rename` (similar to the correct implementation used in `UserStore::save` at `users.rs:163`).