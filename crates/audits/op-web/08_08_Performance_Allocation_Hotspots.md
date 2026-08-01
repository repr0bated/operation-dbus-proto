### 1. Unsafe `simd_json` / `serde_json` Usage

*   **`crates/op-web/src/groups_admin.rs:48`**: Uses `unsafe { simd_json::from_str(...) }` on a mutated clone of config file contents. If `raw` contains invalid UTF-8 (or is structurally mutated in-place maliciously), this triggers undefined behavior.
*   **`crates/op-web/src/state_manager_client.rs:33`**: Uses `unsafe { simd_json::from_str(...) }` on the D-Bus system response string. Parsing untrusted system D-Bus output with unsafe string mutation mechanics is highly volatile.
*   **`crates/op-web/src/orchestrator/parsing.rs:32`**: Uses `unsafe { simd_json::from_str(...) }` on raw arguments extracted from LLM XML output. This parses untrusted LLM input using unsafe mutating APIs.
*   **`crates/op-web/src/orchestrator/parsing.rs:82`**: Uses `unsafe { simd_json::from_str(...) }` to parse raw arguments extracted from LLM code block outputs.
*   **`crates/op-web/src/orchestrator/parsing.rs:124`**: Uses `unsafe { simd_json::from_str(...) }` on JSON strings embedded inside LLM markdown responses.
*   **`crates/op-web/src/websocket.rs:103`**: Uses `unsafe { simd_json::from_str(...) }` to parse incoming raw WebSocket text frames.
*   **`crates/op-web/src/handlers/websocket.rs:84`**: Duplicated WebSocket handler uses `unsafe { simd_json::from_str(...) }` on raw incoming frame strings.

---

### 2. High-Frequency / Hot Path Allocations

#### Large Registry Cloning (O(N) Vector Copying)
*   **`crates/op-web/src/handlers/tools.rs:17`**: Invokes `state.tool_registry.list().await` which clones all registry definitions. In standalone/D-Bus projection mode, this processes up to 16,000+ tools. Mapping this inside `list_tools_handler` allocates megabytes of memory per request and blocks the async reactor thread.
*   **`crates/op-web/src/mcp_compact.rs:247`**: Calling `registry.list().await` duplicates the entire 16k+ tool registry just to paginate a subset.
*   **`crates/op-web/src/mcp_compact.rs:297`**: Calling `registry.list().await` clones the entire registry for a basic filter operation.
*   **`crates/op-web/src/mcp_compact.rs:327`**: Calling `registry.list().await` clones the entire registry just to find one schema by name. This is an $O(N)$ allocation anti-pattern that could be $O(1)$ via a direct lookup map.
*   **`crates/op-web/src/orchestrator/execution.rs:74`**: Duplicates the entire registry list to filter by category on meta-tool queries.
*   **`crates/op-web/src/orchestrator/execution.rs:122`**: Duplicates the entire registry list to execute a search query.

#### On-The-Fly Regex Compilation
*   **`crates/op-web/src/orchestrator/formatting.rs:193`**: Compiles `Regex::new(r"<tool_call>.*?</tool_call>")` on *every* single LLM text cleaning step inside `clean_llm_text`.
*   **`crates/op-web/src/orchestrator/formatting.rs:198`**: Compiles `Regex::new(r"\w+\(\s*\{{[^}}]*\}}\s*\)")` on the fly.
*   **`crates/op-web/src/orchestrator/formatting.rs:203`**: Compiles `Regex::new(r"\n{{3,}}")` on the fly.
*   **`crates/op-web/src/orchestrator/parsing.rs:21`**: Compiles `Regex::new(r"(?s)<tool_call>...")` on the fly inside the parser.
*   **`crates/op-web/src/orchestrator/parsing.rs:49`**: Compiles `Regex::new(r"(?s)```(?:tool|tool_code)...")` on the fly.
*   **`crates/op-web/src/orchestrator/parsing.rs:71`**: Compiles `Regex::new(r"(?s)\b...")` on the fly.

#### Forking Processes in API Handlers
*   **`crates/op-web/src/handlers/dashboard.rs:40`**: Spawns a shell command `Command::new("wg").args(&["show", "wg0", "peers"])` on every dashboard metrics poll. This is extremely slow and susceptible to process exhaustion (fork-bombing) under load.
*   **`crates/op-web/src/handlers/logs.rs:35`**: Spawns a shell process `Command::new("tail")` inside a REST handler on every request.
*   **`crates/op-web/src/handlers/status.rs:170`**: Spawns `doas dinitctl list` via shell process inside a status API endpoint.

#### Redundant HTML & JSON Allocations
*   **`crates/op-web/src/groups_admin.rs:124`**: Calling `GROUPS_ADMIN_HTML.to_string()` allocates and copies a large static template (~20KB) on every single GET request.
*   **`crates/op-web/src/mcp_compact.rs:90`**: Re-allocates the `json!` structure list containing static metadata on *every* call to `get_compact_tools()`. This should be lazily evaluated or defined as static bytes.
*   **`crates/op-web/src/mcp_discovery.rs:20`**: Re-allocates a massive nested `json!` structure on every discovery request.
*   **`crates/op-web/src/handlers/chat.rs:241`**: Re-allocates, formats, and structures long conversation strings to write to `/tmp` dynamically on transcript generation.

---

### 3. Clone Abuse & Unnecessary Copies

*   **`crates/op-web/src/groups_admin.rs:45`**: `let mut raw = content.clone();` clones the entire file contents of the JSON configuration on disk before parsing.
*   **`crates/op-web/src/groups_admin.rs:76`**: `.cloned()` clones the `EnabledGroups` struct containing a `HashSet<String>` on every read lock.
*   **`crates/op-web/src/groups_admin.rs:93`**: `self.trusted_networks.read().await.clone()` clones the entire `Vec<String>`.
*   **`crates/op-web/src/groups_admin.rs:133`**: `group.domain.clone()` clones the string inside a hot parsing loop.
*   **`crates/op-web/src/email.rs:114`**: `Credentials::new(self.config.smtp_user.clone(), self.config.smtp_pass.clone())` unnecessarily clones credentials on every email dispatch.
*   **`crates/op-web/src/mcp_agents.rs:207`**: Clones `agent_type` keys during configuration sync.
*   **`crates/op-web/src/mcp_agents.rs:347`**: Clones descriptor names, descriptions, and capabilities lists for every agent mapped on the listing endpoint.
*   **`crates/op-web/src/privacy_container.rs:128`**: Clones structural properties `user.id`, `user.email`, `user.assigned_ip`, and `user.wg_public_key` to build container parameters.

---

### 4. Public API Leakage & Panic Hazards (`unwrap`)

#### Panics in Production Configuration
*   **`crates/op-web/src/server.rs:114`**: Uses `.unwrap()` on `GovernorConfigBuilder::finish()`. If the configuration limits or burst bounds are invalid, the entire web server panics on startup.
*   **`crates/op-web/src/bin/op-dbus.rs:26`**: Uses double-nested `.unwrap()` on IP address parsing: `.parse().unwrap()`. An invalid configuration value in `OP_DBUS_GRPC_LISTEN` crashes the entire backend service.
*   **`crates/op-web/src/handlers/openclaw.rs:94`**: `Client::builder().build().unwrap()` can panic if the TLS backend initialization fails.
*   **`crates/op-web/src/handlers/privacy.rs:547`**: Uses `.unwrap()` on several critical OAuth URLs. If Google client environment variables contain invalid URL structures, the signup route will panic the server.

#### Silent Panic Risks on JSON Serialization
*   **`crates/op-web/src/websocket.rs:77`**: `simd_json::to_string(&welcome).unwrap()`
*   **`crates/op-web/src/websocket.rs:111`**: `simd_json::to_string(&pong).unwrap()`
*   **`crates/op-web/src/handlers/websocket.rs:56`**: `simd_json::to_string(&welcome).unwrap()`
*   **`crates/op-web/src/handlers/websocket.rs:111`**: `simd_json::to_string(&pong).unwrap()`
    *   *Audit*: While these are simple system payloads, using `.unwrap()` on serialization steps inside active WebSocket loops introduces crash vectors if internal payload memory layouts change. Use `?` or handle errors gracefully.