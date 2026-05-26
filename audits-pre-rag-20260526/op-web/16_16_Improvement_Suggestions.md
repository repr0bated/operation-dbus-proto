1. **Critical Security Finding: Hardcoded Bypass API Keys**
   * **Suggestion**: Remove hardcoded API tokens from the source code and transition to loading them dynamically from secure environment variables or a key vault.
   * **Rationale**: Bypassing IP-based security zones via hardcoded static credentials allows any client with access to the codebase (or compiled binary containing static strings) to gain `TrustedMesh` access. This gives administrative privileges over the network and tools without checking origin IPs.
   * **Example**: `crates/op-web/src/middleware/security.rs:14`

2. **Performance: Avoid Creating Reqwest Clients Inside Handlers**
   * **Suggestion**: Instantiate a single `reqwest::Client` during server initialization and share it across HTTP handlers via `AppState` or request extensions.
   * **Rationale**: Re-instantiating a client on every HTTP request completely bypasses connection pooling, increases handshake latencies due to TCP/TLS reconnects, and risks socket/file descriptor exhaustion under heavy traffic.
   * **Example**: `crates/op-web/src/handlers/openclaw.rs:94` and `crates/op-web/src/handlers/openclaw.rs:167`

3. **Storage: Transition from In-Memory JSON RWLocks to SQLite or CozoDB**
   * **Suggestion**: Replace the flat JSON-file storage backend (`RwLock<HashMap<String, PrivacyUser>>`) with a production-ready database like SQLite or CozoDB.
   * **Rationale**: Rewriting the entire JSON file to disk on every profile update or user registration blocks threads, does not guarantee write atomicity (risking corruption on sudden shutdowns), and causes severe lock contention as the user base grows.
   * **Example**: `crates/op-web/src/users.rs:96` and `crates/op-web/src/groups_admin.rs:109`

4. **Performance: Prevent Blocking Synchronous File IO in Async Executor**
   * **Suggestion**: Use `tokio::fs::write` instead of synchronous `std::fs::write` when persisting configurations inside async functions.
   * **Rationale**: Synchronous filesystem operations block the active Tokio thread pool, preventing other async tasks from running and severely degrading overall system throughput.
   * **Example**: `crates/op-web/src/mcp_agents.rs:595`

5. **Architecture: Decouple Monolithic Server Crate**
   * **Suggestion**: Split `op-web` into three distinct crates: `op-web-api` (routing, handlers, SSE, websockets), `op-mcp-server` (meta-tools, MCP compact, and discovery), and `op-vpn-provisioner` (WireGuard generation, Incus container orchestration, OVS/OpenFlow flow management).
   * **Rationale**: `op-web` is currently violating single-responsibility principles by handling administrative UIs, mail server checking, container creation, OpenFlow routing logic, and LLM orchestration. Separation allows independent deployment, faster build times, and narrower attack surfaces.
   * **Example**: `crates/op-web/src/lib.rs:1`

6. **Observability: Implement Tracing Spans for WebSocket Connections**
   * **Suggestion**: Instrument the async spawned task in `handle_socket` with a structured `tracing::Span` that captures the unique `session_id`.
   * **Rationale**: Without an active span contextualizing the spawned task, diagnostic logs produced during downstream tool executions or LLM orchestration turns cannot be correlated back to the originating WebSocket session.
   * **Example**: `crates/op-web/src/websocket.rs:41`

7. **API Ergonomics: Replace Raw `Value` Params with Strongly-Typed Structs**
   * **Suggestion**: Use typed structs with Axum's `Json` extractor rather than extracting fields from untyped raw `simd_json::OwnedValue`.
   * **Rationale**: Slicing strings and extracting fields manually from untyped values (like `arguments` or `params`) leads to verbose boilerplate, fragile validation logic, and a higher probability of silent runtime parsing bugs.
   * **Example**: `crates/op-web/src/mcp_compact.rs:167`

8. **Performance: Eliminate Unnecessary JSON Serialization/Deserialization Roundtrips**
   * **Suggestion**: Refactor `AgentTask` to accept native AST or `simd_json::OwnedValue` directly rather than converting values into String buffers.
   * **Rationale**: Converting a parsed JSON structure back into a string just to pass it to `AgentTrait::execute` consumes CPU cycles and increases heap allocations on every single tool execution.
   * **Example**: `crates/op-web/src/mcp_agents.rs:327`

9. **Observability: Prefer Structured Logging over Interpolated Strings**
   * **Suggestion**: Use structured tracing key-value pairs (e.g., `info!(user_id = %user_id, "Set credentials")`) instead of interpolating strings directly in log messages.
   * **Rationale**: Structured logging formats (like JSON) can be indexed and queried efficiently by production log aggregation tools (e.g., Loki, ElasticSearch), whereas raw interpolated text requires expensive regex parsing.
   * **Example**: `crates/op-web/src/handlers/privacy.rs:480`