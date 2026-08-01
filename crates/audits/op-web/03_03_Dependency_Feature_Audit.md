# Production Security & Quality Audit: op-web

---

## SECTION 1: DEPENDENCIES & FEATURE INVENTORY

This section documents all direct dependencies specified in `crates/op-web/Cargo.toml` along with their enabled features, workspace inheritance, and a security posture assessment of potential risks.

### Direct Dependencies

| Dependency | Declared Version | Features Explicitly Enabled | Inherited Features / Posture Notes |
| :--- | :--- | :--- | :--- |
| **op-core** | `workspace = true` | N/A | Core OS interfaces & execution. |
| **op-chat** | `workspace = true` | N/A | Prompt compilation and history tracking. |
| **op-llm** | `{ path = "../op-llm" }` | N/A | Handles upstream AI model API integration. |
| **op-tools** | `{ path = "../op-tools" }` | N/A | Linux management tools registry. |
| **op-agents** | `{ path = "../op-agents" }` | N/A | Cognitive execution agent definitions. |
| **op-state** | `workspace = true` | N/A | Shared state synchronization. |
| **op-network** | `workspace = true` | N/A | OVSDB and OpenFlow controllers. |
| **op-mcp** | `{ path = "../op-mcp" }` | N/A | Model Context Protocol engine. |
| **op-mcp-aggregator** | `{ path = "../op-mcp-aggregator" }` | N/A | Consolidates tools across endpoints. |
| **op-state-store** | `{ path = "../op-state-store" }` | N/A | SQL and key-value storage engine. |
| **op-identity** | `workspace = true` | N/A | Cryptographic key pair generation. |
| **op-introspection** | `workspace = true` | N/A | System d-bus introspector. |
| **op-grpc-bridge** | `{ path = "../op-grpc-bridge" }` | N/A | Client/server gRPC interface. |
| **op-jsonrpc** | `{ path = "../op-jsonrpc" }` | N/A | JSON-RPC protocol parser. |
| **tower_governor** | `0.4` | None | Axum rate-limiting wrapper. |
| **axum** | `workspace = true` | `"ws"`, `"macros"`, `"tokio"` | Axum router & WebSocket support. |
| **tokio** | `workspace = true` | `"full"`, `"signal"` | Full async runtime executor. |
| **tower** | `workspace = true` | None | Tower Service traits. |
| **tower-http** | `workspace = true` | `"cors"`, `"fs"`, `"compression-gzip"`, `"trace"`, `"timeout"` | Compression & file serving. |
| **hyper** | `workspace = true` | None | Base HTTP server. |
| **serde** | `workspace = true` | None | Deserialization frameworks. |
| **simd-json** | `workspace = true` | None | High-performance JSON parser. |
| **toml** | `workspace = true` | None | Configuration files format. |
| **futures** | `workspace = true` | None | Future combinators. |
| **async-trait** | `workspace = true` | None | Async trait method support. |
| **tokio-stream** | `workspace = true` | `"sync"` | Stream capabilities for event loops. |
| **async-stream** | `0.3` | None | Generator macros for axum streams. |
| **uuid** | `workspace = true` | `"v4"`, `"serde"` | Unique identifier generation. |
| **chrono** | `workspace = true` | `"serde"` | DateTime formatting and parsing. |
| **tracing** | `workspace = true` | None | Application tracing infrastructure. |
| **tracing-subscriber**| `workspace = true` | `"env-filter"` | Environment variable filtering. |
| **anyhow** | `workspace = true` | None | General error utility framework. |
| **thiserror** | `workspace = true` | None | Strongly typed error macro. |
| **sysinfo** | `0.30` | None | Queries OS resource utilization metrics. |
| **gethostname** | `workspace = true` | None | Detects active system hostname. |
| **lazy_static** | `1.4` | None | Thread-safe lazily evaluated statics. |
| **regex** | `workspace = true` | None | Regular expression match engines. |
| **qrcode** | `0.14` | None | PNG generation for client profiles. |
| **image** | `0.25` | `default-features = false`, `"png"`| Formats raw QR buffers into PNG binaries. |
| **base64** | `0.22` | None | Base64 config encoding. |
| **lettre** | `0.11` | `"tokio1-native-tls"`, `"builder"` | Client for sending outbound SMTP emails. |
| **hex** | `workspace = true` | None | Formatting hashes into hex. |
| **zbus** | `workspace = true` | None | Native IPC interface to System D-Bus. |
| **ring** | `0.17` | None | Cryptographic keys & HKDF entropy. |
| **oauth2** | `4.4` | None | OAuth2 flow controller. |
| **reqwest** | `0.11` | `"json"` | External HTTP queries. |
| **rust-embed** | `8` | `"compression"` | Compiles static files into the binary. |
| **axum-embed** | `0.1` | None | Bridges Axum with `rust-embed` resources. |
| **mime_guess** | `2` | None | Resolves browser mime types for raw files. |
| **linemux** | `0.3.0` | None | File watching with efficient `inotify`. |

### Workspace-Level Resolved Versions

From the main workspace configuration (`Cargo.toml` and lock files):
*   **axum**: `0.7`
*   **tokio**: `1` (features: `full`)
*   **tower**: `0.4` / `0.5`
*   **tower-http**: `0.5` / `0.6`
*   **hyper**: `1.0`
*   **serde**: `1` (features: `derive`)
*   **simd-json**: `0.13` (features: `serde`, `serde_impl`)
*   **chrono**: `0.4`
*   **zbus**: `4.0`
*   **reqwest**: `0.11`
*   **ring**: `0.17.14`

### Crate features

Crate `op-web` has **none defined** in its own `[features]` block inside `crates/op-web/Cargo.toml`.

---

## SECTION 2: STORAGE BACKEND ANALYSIS

The codebase utilizes several files and system sockets to read and write state. Below is an inventory of all storage mechanisms directly accessed by the `op-web` server.

### Storage Backend Inventory

| Backend | Found at File:Line | Role (KV / Graph / Cache / Queue / File) | Architectural Violation & Security Gaps |
| :--- | :--- | :--- | :--- |
| **JSON File** (`tool-groups.json`) | `crates/op-web/src/groups_admin.rs:43` | **KV/File**: Tool Group and Active Profile Storage. | **Violation**: Bypasses transaction-safe database architectures. Direct, uncoordinated writes lead to file corruptions if concurrent administrative actions occur. |
| **JSON File** (`cognitive-mcp-agents.json`) | `crates/op-web/src/mcp_agents.rs:556` | **KV/File**: Cognitive Agent Prewarming Selections. | **Violation**: Relies on direct filesystem serialization. Lack of transaction support or atomic locks can cause a race condition when rewriting active configurations. |
| **JSON File** (`privacy-users.json`) | `crates/op-web/src/users.rs:75` | **KV/File**: Privacy User Accounts, WireGuard Keys, and Quota metrics. | **Critical Violation**: Stores absolute cryptographic parameters and traffic quotas in an ad-hoc JSON document. Represents a massive data safety hazard. |
| **SQLite DB** (`state.db`) | `crates/op-web/src/state.rs:188` | **KV/Cache**: Local job tracking & execution audit logging via `SqliteStore`. | **Violation**: Safe. SQLite is bounded to local tracking of tool jobs, avoiding direct conflicts. |
| **D-Bus System Bus** (`StateManager`) | `crates/op-web/src/state_manager_client.rs:24` | **IPC State Store**: Publishes container profiles (Incus), OpenFlow rules, and WireGuard route states to the parent control plane. | **Standard**: This is the designated, declarative routing mechanism to coordinate network changes via `StateManager`. |

---

## SECTION 3: CRITICAL EXPLOITABLE VULNERABILITIES

The following issues are directly exploitable within the provided codebase and present immediate security threats to the server or target host.

### Finding 1: Path Traversal & Arbitrary File Overwrite via Unsanitized Chat Transcript Export
*   **Severity**: Critical
*   **File:Line**: `crates/op-web/src/handlers/chat.rs:432` (Tracing back to input extraction at `crates/op-web/src/handlers/chat.rs:360`)
*   **Exploit Vector**:
    The endpoint `/api/chat/transcript` takes user input as a JSON request body and maps the `filename` value:
    ```rust
    let filename = params
        .get("filename")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("chat-transcript-{}.txt", chrono::Utc::now().timestamp()));
    ```
    This parameter is then passed directly to the `save_transcript_to_file` function, which maps and writes the output payload:
    ```rust
    let filepath = format!("/tmp/{}", filename);
    match tokio::fs::write(&filepath, &transcript).await { ... }
    ```
    Because the application does not validate, sanitize, or restrict path characters in `filename`, a malicious user can provide path traversal strings such as `../../etc/cron.d/exploit` or `../../var/lib/op-dbus/state.db`. Since the underlying server executes commands requiring root/escalated privileges (interacting with `dinitctl`, `incus`, and `wg` interfaces), the process has write permissions over critical system configurations. This allows an unauthenticated remote attacker to overwrite arbitrary files, drop malicious cron jobs, or corrupt the local database.

*   **Remediation**:
    Apply strict input validation on `filename`. Force the output path to use a sanitized, base-named string without directory separators (`/` or `\`) or traversal elements (`..`), and restrict the output strictly to a dedicated, bounded directory.
    ```rust
    let safe_filename = Path::new(&filename)
        .file_name()
        .context("Invalid filename format")?;
    let filepath = Path::new("/tmp").join(safe_filename);
    ```

---

### Finding 2: Unauthenticated Tool Execution & Command Injection Bypass on Main Router
*   **Severity**: Critical
*   **File:Line**: `crates/op-web/src/middleware/security.rs:114` and `crates/op-web/src/routes/mod.rs:251`
*   **Exploit Vector**:
    The application introduces an IP-based security middleware (`ip_security_middleware`) applied to the main routing structure:
    ```rust
    router
        .layer(Extension(state))
        .layer(axum::middleware::from_fn(security::ip_security_middleware))
    ```
    However, looking closely at the implementation of `ip_security_middleware`, its role is entirely restricted to evaluating the client's `AccessZone` and storing it as a request extension:
    ```rust
    request.extensions_mut().insert(zone);
    next.run(request).await
    ```
    The middleware **never** drops, rejects, or halts requests belonging to public/unauthenticated zones.
    
    Worse, the core execution endpoints (such as `execute_tool_handler` at `crates/op-web/src/handlers/tools.rs:112` and `execute_named_tool_handler` at `121`) **completely ignore** the attached `AccessZone` request extension. They immediately route the payload to `execute_tool_internal` which executes any designated tool (including raw shell execution via `shell_exec` / `shell_exec_tool` and file edits) on the host machine. 
    
    This leaves the entire tool execution pipeline wide open to the public internet. Anyone who can reach the web port can execute arbitrary commands as root via POST requests to `/api/tool` or `/api/tools/:name/execute`.

*   **Remediation**:
    The `ip_security_middleware` must perform explicit policy enforcement. Add an inspection check within the middleware (or via a separate routing guard layer) to block any incoming request whose evaluated `AccessZone` does not possess the required `SecurityLevel` needed to execute tools or access administrative profiles.
    ```rust
    if zone != AccessZone::Localhost && zone != AccessZone::TrustedMesh {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::from("Unauthorized access zone"))
            .unwrap();
    }
    ```

---

## SECTION 4: HIGH & MEDIUM SEVERITY VULNERABILITIES

### Finding 3: Google OAuth CSRF State Flooding & Denial of Service (DoS)
*   **Severity**: High
*   **File:Line**: `crates/op-web/src/handlers/privacy.rs:496`
*   **Description**:
    When initiating a Google OAuth transaction, the server generates a random CSRF token and writes it into the `csrf_tokens` static state map:
    ```rust
    {
        let mut tokens = state.csrf_tokens.write().await;
        tokens.insert(csrf_token.secret().clone(), csrf_token.secret().clone());

        // Cleanup old tokens (simple heuristic)
        if tokens.len() > 1000 {
            tokens.clear();
        }
    }
    ```
    This logic has two critical flaws:
    1.  There is no TTL (Time-To-Live) eviction or timestamp validation for these tokens. Unused tokens leak memory indefinitely.
    2.  The cleanup strategy simply executes `tokens.clear();` when the collection exceeds 1,000 items. An attacker can easily trigger this threshold by sending 1,001 fake authentication requests to `/api/privacy/google/auth`. This immediately clears the token cache for **all** legitimate users in the middle of logging in, causing an instant denial of service for the authentication portal.

*   **Remediation**:
    Replace the flat `HashMap` with an LRU cache or a map that records insertion timestamps, evicting only expired states. Never wipe the entire cache during routine cleanup.

---

### Finding 4: Cryptographic Private Key Leakage via Plaintext Disk Storage
*   **Severity**: High
*   **File:Line**: `crates/op-web/src/users.rs:141` and `crates/op-web/src/handlers/privacy.rs:175`
*   **Description**:
    The user storage system defines a structure property named `wg_private_key_encrypted` on the `PrivacyUser` object. However, looking at how users are created:
    At `crates/op-web/src/handlers/privacy.rs:172`, a plaintext WireGuard keypair is generated:
    ```rust
    let keypair = generate_keypair();
    ```
    At `crates/op-web/src/handlers/privacy.rs:175`, this plaintext private key is passed directly to `create_user` as the `wg_private_key_encrypted` parameter:
    ```rust
    state.user_store.create_user(&email, keypair.public_key, keypair.private_key).await
    ```
    Inside `create_user` (`crates/op-web/src/users.rs:141`), it is written without any encryption to the database object and written in plaintext to the local JSON file on disk at `/var/lib/op-dbus/privacy-users.json` during the `save()` routine.
    
    This exposes all users' WireGuard private keys to disk in plaintext, allowing local privilege escalation or administrative compromise if the file is read.

*   **Remediation**:
    Perform proper cryptographic encryption (e.g., using AES-GCM with a secret key derived from a master host key) on `keypair.private_key` before placing it into the `wg_private_key_encrypted` persistence parameter.

---

### Finding 5: Local Privilege Escalation via Weak Permission Tail Commands
*   **Severity**: Medium
*   **File:Line**: `crates/op-web/src/handlers/logs.rs:25`
*   **Description**:
    The server attempts to stream logs by launching external system sub-processes using `tail`:
    ```rust
    if let Ok(output) = Command::new("tail").args(&["-n", "50", log_path]).output() { ... }
    ```
    If `op-web` is running under an escalated context to access d-bus or incus sockets, this pattern invites local privilege escalation vectors. If a local adversary is able to create a symbolic link from `/var/log/op-web.log` or `/tmp/op-web.log` to a privileged file (like `/etc/shadow`), the `tail` process will happily read the contents and expose them through the web log viewer endpoint.
*   **Remediation**:
    Avoid shell execution or direct shell piping. Utilize native Rust file reading APIs with strict symbol-link dereference checking (`fs::canonicalize`) to verify that the file being read belongs to the expected logging boundary.

---

## SECTION 5: SCHEMA-AS-CODE & OSCAL COMPLIANCE GAPS

Under strict schema-as-code engineering disciplines, all structured data contracts, protocol exchanges, and administrative states must be expressed as versioned schemas (such as Protocol Buffer schemas or OSCAL XML/JSON compliance boundaries) to ensure structural guarantees and prevent drift.

The audited codebase has several critical schema-as-code gaps where data contracts are defined as ad-hoc Rust structs or raw maps:

### 1. Ad-Hoc JSON-RPC & MCP Payloads
*   **Citations**: `crates/op-web/src/mcp.rs:59` (`McpRequest`), `crates/op-web/src/mcp_agents.rs:37` (`JsonRpcRequest`)
*   **Gap**: The JSON-RPC and Model Context Protocol messages are designed using manual, ad-hoc struct deserializations mapping raw `simd_json::OwnedValue` objects. 
*   **Consequence**: There is no schema validation or structural conformance testing. Upstream model changes or malicious parameters can cause parsing failures or panics. These should be defined using versioned, schema-generated protobuf structures.

### 2. Declarative Virtualization (Incus) and Network Topology Models
*   **Citations**: `crates/op-web/src/privacy_container.rs:32` (`IncusState`), `crates/op-web/src/privacy_openflow.rs:10` (`OpenFlowConfig`), `crates/op-web/src/privacy_routes.rs:14` (`PrivacyRoutesState`)
*   **Gap**: High-risk control configurations representing active virtualization profiles, OpenFlow routing tables, and interface links are modeled as raw Rust vectors containing nested `HashMap<String, String>` structures.
*   **Consequence**: Since these configurations manage active system boundaries and network choke points, using raw ad-hoc JSON serializations introduces contract drift. A single validation error (such as a missing or corrupt JSON property) can result in firewall bypasses, invalid routing loops, or failure to isolate network zones. These configurations must be managed via versioned, validated schemas or formal OSCAL control profiles.

### 3. Tool Group Profile Persistence
*   **Citations**: `crates/op-web/src/groups_admin.rs:30` (`EnabledGroups`)
*   **Gap**: Tool access limitations, presets, and domain configurations are written directly as arbitrary `HashSet<String>` components mapped to `/var/lib/op-dbus/tool-groups.json`.
*   **Consequence**: There is no mechanism to enforce tool inventory boundaries. The lack of versioned schema contracts means that a compromised or manually altered JSON file on disk can silently enable highly privileged tools for restricted network zones, bypassing expected access levels.

---
## ⚠ Citation Warnings
- `crates/op-web/src/routes/mod.rs:251`: file has 248 lines
