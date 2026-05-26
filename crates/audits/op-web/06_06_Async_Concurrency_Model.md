# Production Security and Quality Audit Report
**Crate:** op-web  
**Target:** Unified Web Server for op-dbus-v2

---

## 1. Executive Summary

This security and quality audit evaluates the design and implementation of the unified HTTP/WebSocket control plane server (`op-web`). 

Multiple **Critical** security vulnerabilities were discovered in the routing layer, chat transcript handler, and system management APIs. Most notably, unauthenticated users can execute arbitrary control plane tools (including shell commands as `root`), read/write arbitrary files outside the sandboxed path using directory traversal, and extract plaintext WireGuard client private keys. 

Additionally, the async engine exhibits severe performance blockages due to synchronous file I/O and external process spawns, and suffers from a resource-exhaustion memory leak when streaming system logs.

---

## 2. Async & Concurrency Performance Analysis

### Metric Counts
*   **Async Functions (`async fn`):** ~115 declarations across endpoints, storage subsystems, and D-Bus adapters.
*   **Tokio Task Spawns (`tokio::spawn`):** 10 occurrences (system metrics loops, gRPC-to-SSE event bridges, WebSocket session receivers, and log watchers).
*   **Blocking Task Spawns (`spawn_blocking`):** **0 occurrences.**

### Thread-Blocking Reactor Violations
The codebase completely lacks `tokio::task::spawn_blocking` wrappers. As a result, heavy disk and system control operations run directly on the multi-threaded asynchronous reactor thread pool. This starves the cooperative scheduler and severely degrades concurrent throughput:

1.  **Synchronous Process Spawning:**
    *   `crates/op-web/src/handlers/dashboard.rs:37-47`: Synchronously executes `wg show wg0 peers` in a blocking subprocess call, blocking the worker thread for tens of milliseconds during metrics queries.
    *   `crates/op-web/src/handlers/logs.rs:43-52`: Spawns synchronous `tail` commands to parse log files, blocking the reactor on disk read completions.
    *   `crates/op-web/src/handlers/vpn.rs:49-76` & `114-124`: Spawns multiple synchronous `wg` queries to retrieve interface stats and dumps.
    *   `crates/op-web/src/handlers/mail.rs:34-45`: Spawns `incus exec` to query systemd in a container, a blocking network/IPC process invocation that can stall the reactor thread for hundreds of milliseconds.
2.  **Synchronous File I/O inside Async Contexts:**
    *   `crates/op-web/src/handlers/dashboard.rs:56-78`: Uses `std::fs::read_to_string` inside an async endpoint handler to read `/proc/loadavg` and `/proc/meminfo`.
    *   `crates/op-web/src/mcp_agents.rs:704-726`: Uses `std::fs::create_dir_all` and `std::fs::write` inside `save_agent_config`, which is invoked directly by the async function `set_cognitive_agents`.
    *   `crates/op-web/src/privacy_network.rs:81-125`: Uses blocking `Path::exists` checks inside `ensure_host_privacy_network_with_config` to query the `/sys/class/net` filesystem.

---

## 3. Critical Security Vulnerabilities (Directly Exploitable)

### Finding 1: Unauthenticated Remote Tool/Shell Code Execution
*   **Citations:** 
    *   `crates/op-web/src/routes/mod.rs:118-124`
    *   `crates/op-web/src/handlers/tools.rs:85-115`
*   **Severity:** Critical (RCE)
*   **Threat Model:** External Network Adversary  
*   **Description:** The HTTP endpoints `/api/tool` and `/api/tools/:name/execute` execute tools directly from the `ToolRegistry` via `execute_tool_internal`. These routes are mounted without authentication checks or zone validation:
    ```rust
    // crates/op-web/src/routes/mod.rs
    .route("/tool", post(handlers::tools::execute_tool_handler))
    .route(
        "/tools/:name/execute",
        post(handlers::tools::execute_named_tool_handler),
    )
    ```
    While `security::ip_security_middleware` computes an `AccessZone` and attaches it as a request extension, the tool execution handlers do not extract or validate this extension. A remote attacker can issue a POST request to execute any system tool, including `shell_exec` with arbitrary arguments (e.g., `rm -rf /` or spawning a reverse shell), leading to total host takeover.

---

### Finding 2: Arbitrary File Write via Path Traversal
*   **Citations:** 
    *   `crates/op-web/src/handlers/chat.rs:317-362`
    *   `crates/op-web/src/handlers/chat.rs:434-454`
*   **Severity:** Critical (Arbitrary File Write / Privilege Escalation)
*   **Threat Model:** Authenticated/Unauthenticated Chat Client  
*   **Description:** The chat transcript endpoint `/api/chat/transcript` takes a user-supplied JSON payload containing a `filename` string:
    ```rust
    // crates/op-web/src/handlers/chat.rs (save_transcript_handler)
    let filename = params
        .get("filename")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        ...
    ```
    The filename is passed directly to `save_transcript_to_file`, which constructs a file path on disk without sanitizing directory traversal characters:
    ```rust
    // crates/op-web/src/handlers/chat.rs (save_transcript_to_file)
    let filepath = format!("/tmp/{}", filename);
    match tokio::fs::write(&filepath, &transcript).await { ... }
    ```
    An attacker can supply a filename like `../../etc/cron.d/malicious_job`. The server will resolve this to `/etc/cron.d/malicious_job`. By structuring the `messages` array in the request body, the attacker can write a payload containing a crontab entry (e.g., executing a reverse shell as root), leading to complete compromise of the underlying operating system.

---

### Finding 3: Unauthenticated Private Key Disclosure
*   **Citations:** 
    *   `crates/op-web/src/routes/mod.rs:194-197`
    *   `crates/op-web/src/handlers/privacy.rs:448-522`
*   **Severity:** Critical (Information Disclosure / Cryptographic Key Leak)
*   **Threat Model:** Unauthorized External Attacker  
*   **Description:** While the JSON configuration retrieval endpoint `/api/privacy/config/:user_id` implements bearer token validation, the human-readable HTML equivalent `/privacy/access` is completely public.
    The handler extracts `user_id` from the URL query parameters and immediately loads the corresponding `PrivacyUser` from storage:
    ```rust
    // crates/op-web/src/handlers/privacy.rs
    } else if let Some(user_id) = &query.user_id {
        match state.user_store.get_user(user_id).await {
        Some(user) if user.email_verified => {
            ...
            let config = generate_client_config(
                &user.wg_private_key_encrypted, // Stored as plaintext in this implementation
                &user.assigned_ip,
                &state.server_config,
            );
    ```
    The private key is rendered directly into the HTML response inside a `<pre>` block. Any unauthenticated network client who discovers, guesses, or intercepts a valid user UUID can retrieve their full WireGuard configuration, including the plaintext private key, completely bypassing the auth token check implemented on the JSON API.

---

### Finding 4: Inotify File Descriptor & Tokio Task Leak
*   **Citations:** 
    *   `crates/op-web/src/handlers/logs.rs:136-193`
*   **Severity:** Medium (Denial of Service via Resource Exhaustion)
*   **Threat Model:** External Client connecting to Log Stream  
*   **Description:** The SSE handler `logs_stream_handler` spawns a background thread task to monitor log files with `linemux::MuxedLines` on every client connection:
    ```rust
    // crates/op-web/src/handlers/logs.rs
    pub async fn logs_stream_handler(...) {
        ...
        tokio::spawn(async move {
            use linemux::MuxedLines;
            let mut lines = MuxedLines::new().expect("Failed to create MuxedLines");
            ...
            while let Ok(Some(line)) = lines.next_line().await {
                ...
                broadcaster.broadcast("log", &payload);
            }
        });
    }
    ```
    This spawned loop runs indefinitely and contains no check to determine if the client connection has closed, nor does it listen to any shutdown/cancellation signals. When an SSE client disconnects, the spawned task is left running on the executor. Repeating this request leaks tokio tasks and `inotify` file watches on `/var/log/op-web.log` and `/var/log/op-dbus.log`. This rapidly exhausts the system's process watch limits, resulting in a Denial of Service.

---

### Finding 5: Plaintext WireGuard Private Key Storage
*   **Citations:** 
    *   `crates/op-web/src/users.rs:252-282`
    *   `crates/op-web/src/handlers/privacy.rs:360-369`
*   **Severity:** High (Cryptographic Key Exposure)
*   **Threat Model:** Local File Disclosure / Unauthorized Server Read  
*   **Description:** The database structure is represented by `crates/op-web/src/users.rs:20`. Although the field is named `wg_private_key_encrypted`, no encryption is performed. Plaintext keys generated by `generate_keypair()` are saved directly into the field and serialized to disk in plaintext:
    ```rust
    // crates/op-web/src/handlers/privacy.rs (signup)
    let keypair = generate_keypair();
    // Create user (we'll encrypt the private key later, for now just store it)
    match state
        .user_store
        .create_user(&email, keypair.public_key, keypair.private_key) // Plaintext passed directly
        .await
    ```
    The plaintext keys are stored in JSON format at `/var/lib/op-dbus/privacy-users.json`. Any access to this database file completely compromises the WireGuard VPN infrastructure.

---

## 4. Schema-as-Code Compliance Violations

The codebase consistently violates the **schema-as-code** discipline. No Protocol Buffers (`.proto`) or OSCAL schemas are used to define the core system boundaries or data contracts. Instead, the interface relies on ad-hoc Rust structs, manually parsed JSON structures, and untyped dynamic serialization via `simd_json`:

1.  **Ad-Hoc Model Configuration & Routing Contracts:**
    *   `crates/op-web/src/mcp.rs:60-95`: `McpRequest`, `McpResponse`, and `McpError` are defined as ad-hoc Rust structs rather than generated from versioned Protocol Buffer schema schemas.
    *   `crates/op-web/src/mcp_agents.rs:55-85`: `JsonRpcRequest`, `JsonRpcResponse`, and `JsonRpcError` implement ad-hoc JSON-RPC parsing for the cognitive agents MCP layer.
    *   `crates/op-web/src/mcp_compact.rs:37-64`: Duplicate definition of JSON-RPC schema contracts.
2.  **Unversioned State Contracts:**
    *   `crates/op-web/src/users.rs:18-47`: `PrivacyUser` and `UserApiCredentials` are stored directly in a file database using ad-hoc JSON fields. Schema changes will lead to unversioned deserialization failures at runtime.
    *   `crates/op-web/src/privacy_container.rs:26-55`: `IncusState` and `IncusInstance` represent ad-hoc JSON contracts designed to mock complex infrastructure states without centralized API specification rules.
    *   `crates/op-web/src/privacy_openflow.rs:12-45`: Ad-hoc serialization definitions representing complex OpenFlow bridge, routing, and table action configurations.
    *   `crates/op-web/src/groups_admin.rs:26-30`: Ad-hoc `EnabledGroups` serialization contract used to configure D-Bus access groups.

---

## 5. Security & Quality Action Plan

| Rank | Vulnerability / Violation | Impact | Remediation Action |
| :--- | :--- | :--- | :--- |
| **1** | Unauthenticated Remote Tool Exec (`routes/mod.rs:118-124`) | Critical (RCE) | Guard `/api/tool` and `/api/tools/:name/execute` with an authorization middleware checking `AccessZone` and bearer keys. |
| **2** | Path Traversal File Write (`handlers/chat.rs:434-454`) | Critical | Sanitize `filename` using `Path::file_name` to prevent directory traversal (`../`). |
| **3** | Unauthenticated Key Disclosure (`handlers/privacy.rs:448-522`) | Critical | Restrict `/privacy/access` with secure token validation; never display plaintext keys in client-side HTML. |
| **4** | Plaintext Key Storage (`users.rs:252-282`) | High | Encrypt `wg_private_key_encrypted` on the host side using a key derived from an HSM or secure environment key before writing. |
| **5** | Inotify/Task Leak (`handlers/logs.rs:136-193`) | Medium | Monitor the client connection state within the log stream task; terminate the `linemux` file watcher loop immediately when the subscriber drops. |
| **6** | Async Reactor Blocking (`handlers/dashboard.rs:37-47`) | Medium | Wrap all synchronous file I/O operations and subprocess command spawns (`wg`, `incus`) with `tokio::task::spawn_blocking`. |
| **7** | Schema-as-Code Compliance | Low | Migrate serializable structs (e.g. `McpRequest`, `PrivacyUser`) to Protocol Buffers with compiled Rust bindings. |