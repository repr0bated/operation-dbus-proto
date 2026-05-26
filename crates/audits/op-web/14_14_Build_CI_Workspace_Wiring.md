# op-web Production Security and Quality Audit

## 1. Build and Schema-as-Code Audit

### Cargo.toml Analysis
*   **Edition**: Inherited from workspace (`crates/op-web/Cargo.toml:4`), which is set to `2021` in the workspace `Cargo.toml:39`.
*   **Rust Version**: Not explicitly specified in the provided `Cargo.toml` files.
*   **Binaries**: 
    *   `op-web-server` with path `src/main.rs` (`crates/op-web/Cargo.toml:8-9`).
    *   Workspace binary `op-dbus` with path `crates/op-web/src/bin/op-dbus.rs`.
*   **Workspace inheritance vs Local overrides**: `op-web` extensively inherits its version, edition, authors, and license from the workspace. Key dependencies like `axum`, `tokio`, `tower`, `serde`, `simd-json`, `futures`, `tracing`, `anyhow`, `thiserror`, and `zbus` are inherited from the workspace dependencies defined in the root `Cargo.toml`.

### Schema-as-Code Build Check
*   No custom `build.rs` is present or invoked in the audited `op-web` crate.
*   The workspace `Cargo.toml` lists code generation tools `prost-build` and `tonic-build` under its workspace dependencies, but `op-web` does not use them directly at build time to compile `.proto` files.
*   **Flagged Violations of Schema-as-Code**:
    *   **Ad-hoc D-Bus contracts**: System state contracts are defined using ad-hoc, untyped Rust serialization structures instead of versioned Protobuf or OSCAL schemas. For example, `IncusInstance` (`crates/op-web/src/privacy_container.rs:24`) and `OpenFlowConfig` (`crates/op-web/src/privacy_openflow.rs:10`) are modeled as ad-hoc Serde JSON structures mapped manually to StateManager mutations.
    *   **Ad-hoc API Payload contracts**: JSON-RPC and REST API contracts in `mcp_compact.rs`, `mcp_agents.rs`, and `handlers/tools.rs` manipulate loose `simd_json::OwnedValue` (untyped JSON objects) rather than relying on schema-validated strongly-typed contracts generated from a single versioned source of truth.

---

## 2. Critical Findings (Directly Exploitable)

### [CRITICAL] Unauthenticated Arbitrary System Tool and Command Execution
*   **Citations**: `crates/op-web/src/routes/mod.rs:91`, `crates/op-web/src/handlers/tools.rs:76-118`
*   **Threat Model**: Remote Code Execution (RCE) / Full System Takeover.
*   **Description**:
    The HTTP server exposes direct system tool execution endpoints (`POST /api/tool` and `POST /api/tools/:name/execute`) that are completely unauthenticated. The `ip_security_middleware` attached to the router only extracts the client IP address and inserts an `AccessZone` extension into the request, but **never rejects** or filters requests based on this zone.
    
    Consequently, any unauthenticated attacker on the public internet can send arbitrary JSON payloads to `/api/tool` to execute critical system tools. For example, they can call the `shell_exec` tool with arbitrary bash scripts, or utilize the `file_write` tool to overwrite system files, leading to immediate remote code execution with root privileges (as system tools are configured to run with elevated permissions).
*   **Exploit Vector**:
    ```bash
    curl -X POST http://<target>:8080/api/tool \
      -H "Content-Type: application/json" \
      -d '{"tool_name": "shell_exec", "arguments": {"command": "rm -rf / || echo owned"}}'
    ```

### [CRITICAL] Unauthenticated Leak of Sensitive User Metadata and WireGuard Keys
*   **Citations**: `crates/op-web/src/routes/mod.rs:42`, `crates/op-web/src/handlers/users.rs:20-41`
*   **Threat Model**: Massive Information Disclosure and Privacy Breach.
*   **Description**:
    The `/api/users` endpoint lists all registered users in the privacy router system, including their cleartext emails, allocated WireGuard private IPs, and public keys. Like the tool execution route, this endpoint has no authentication checks or zone-based access controls. Any public client can query `/api/users` to harvest user emails and network topology mappings.
*   **Exploit Vector**:
    ```bash
    curl http://<target>:8080/api/users
    ```

### [CRITICAL] Log Streaming SSE Endpoint Quadratic Resource Exhaustion (DoS)
*   **Citations**: `crates/op-web/src/handlers/logs.rs:135-224`
*   **Threat Model**: Denial of Service (DoS) via CPU/Memory Exhaustion and Message Storm.
*   **Description**:
    Every time a client requests `/api/logs/stream`, the handler subscribes to a global broadcaster and spawns a *new* asynchronous background thread (`tokio::spawn`) that creates a unique `linemux::MuxedLines` instance to watch the system log files. 
    
    When a log line is read, the connection-specific thread calls the global broadcaster: `broadcaster.broadcast("log", &payload)`. Because the broadcaster is shared globally across *all* active connection tasks, each individual watcher task broadcasts newly detected log lines to *all* active subscribers. 
    
    If $N$ connections are open, there will be $N$ concurrent file-watching loops running. When a single log line is written to disk, it is picked up by all $N$ watchers, resulting in $N^2$ messages being dispatched. An attacker can open a few dozen connections to this endpoint and write a log message to trigger a quadratic message storm, exhausting CPU, memory, and file descriptors.
*   **Exploit Vector**:
    Establish 100 concurrent SSE connections to `http://<target>:8080/api/logs/stream` and then trigger any log-generating endpoint (like a failed login or public status query).

---

## 3. High/Medium Findings

### [HIGH] Lock Order Inversion Deadlock in `UserStore`
*   **Citations**: `crates/op-web/src/users.rs:104-123`, `crates/op-web/src/users.rs:141-157`, `crates/op-web/src/users.rs:159-183`
*   **Threat Model**: System-wide Thread Blockage / Denial of Service.
*   **Description**:
    A classic Lock Order Inversion exists between the locks of `UserStore::users` and `UserStore::next_ip`.
    *   In `UserStore::load` and `UserStore::save`, the lock acquisition order is:
        1. `self.users` (Write or Read Lock)
        2. `self.next_ip` (Write or Read Lock)
    *   In `UserStore::allocate_ip`, the lock acquisition order is:
        1. `self.next_ip` (Write Lock)
        2. `self.users` (Read Lock)
        
    If one thread initiates a `save` operation (which occurs during any user creation or verification) and holds the read lock on `self.users` while waiting for `self.next_ip`, and another thread simultaneously initiates `allocate_ip` (acquiring the write lock on `self.next_ip` and waiting for a read lock on `self.users`), both threads will permanently deadlock. This freezes all login, registration, and user-query flows.

### [HIGH] Storage of Unencrypted Private Keys in User Database
*   **Citations**: `crates/op-web/src/users.rs:24`, `crates/op-web/src/handlers/privacy.rs:136-146`, `crates/op-web/src/users.rs:141-157`
*   **Threat Model**: Cryptographic Key Compromise.
*   **Description**:
    During registration (`signup`), the server generates a WireGuard client keypair. The raw client private key (`keypair.private_key`) is stored directly in the `wg_private_key_encrypted` field in plaintext as confirmed by the code comments: `// Create user (we'll encrypt the private key later, for now just store it)`. This database is subsequently written in cleartext JSON format to `/var/lib/op-dbus/privacy-users.json`.
    
    This violates the fundamental security model of WireGuard (where the server should never possess the client's private key) and exposes the private keys of all users to any attacker who reads the database file or accesses the unauthenticated `/api/users` dump.

### [MEDIUM] Sensitive Token Leak in Console and Log Output
*   **Citations**: `crates/op-web/src/email.rs:62-72`
*   **Threat Model**: Account Hijacking via Log Inspection.
*   **Description**:
    If the server's SMTP settings are unconfigured, `EmailSender::send_magic_link` defaults to printing active magic link URLs and raw authentication tokens to standard output and standard error logs (`println!`). In production environments, standard output is routinely indexed by centralized log-aggregation tools, exposing single-use magic login tokens to unauthorized operators and monitoring agents.

### [MEDIUM] OAuth CSRF Map Exhaustion Denial of Service
*   **Citations**: `crates/op-web/src/handlers/privacy.rs:480-485`
*   **Threat Model**: Session Invalidation and Login Flow Disruption.
*   **Description**:
    To prevent unbounded memory growth of the Google OAuth CSRF tokens map, the server implements a naive cleaning heuristic: `if tokens.len() > 1000 { tokens.clear(); }`.
    
    This allows an external attacker to easily initiate 1,001 OAuth login flows sequentially. Doing so triggers the purge condition, wiping the CSRF token map completely clean. This instantly invalidates all active, legitimate pending Google OAuth login attempts across the entire platform.

---

## 4. Code Quality & Compilation Failures

### Non-Compiling References to `state.auth_bridge`
*   **Citations**: `crates/op-web/src/handlers/auth_bridge.rs:77`, `crates/op-web/src/handlers/auth_bridge.rs:84`, `crates/op-web/src/state.rs:86-135`
*   **Description**:
    The handlers in `auth_bridge.rs` attempt to read pending authentications from `state.auth_bridge`. However, the `AppState` struct defined in `state.rs` does not contain any field named `auth_bridge`. This causes a critical compilation failure when attempting to build `op-web`.

### Non-Compiling Calls and Incorrect Signatures in `mcp_smart_router.rs`
*   **Citations**: `crates/op-web/src/mcp_smart_router.rs:78-95`
*   **Description**:
    The smart router implementation contains several critical syntax and reference errors that prevent compilation:
    1.  It attempts to call `crate::mcp_compact::mcp_compact_message_handler` with only one argument (`axum::extract::Json(...)`), but that handler requires two parameters: `Extension(state)` and `Json(request)`.
    2.  It attempts to call `crate::mcp_agents::mcp_agents_message_handler_stateless` with a single argument, ignoring the expected extractor signatures.
    3.  It attempts to call `crate::mcp::jsonrpc_handler` by passing `State(crate::mcp::get_app_state())` as the first parameter. However, `jsonrpc_handler` expects `Extension<Arc<AppState>>`, and there is no function named `get_app_state()` anywhere in `mcp.rs`.