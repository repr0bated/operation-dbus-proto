# Unified Security & Quality Audit: op-web

---

## Critical Findings

### 1. Arbitrary File Write / Path Traversal in `save_transcript_handler`
*   **Vulnerability Type:** Path Traversal / Arbitrary File Write
*   **Location:** `crates/op-web/src/handlers/chat.rs:245` and `crates/op-web/src/handlers/chat.rs:377`
*   **Impact:** Critical (Directly exploitable for Privilege Escalation / Remote Code Execution)
*   **Description:** 
    The `save_transcript_handler` accepts a client-provided JSON payload. It extracts the `filename` parameter without any sanitization or validation (e.g., checking for path traversal segments like `..` or `/`):
    ```rust
    // crates/op-web/src/handlers/chat.rs:245
    let filename = params
        .get("filename")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("chat-transcript-{}.txt", chrono::Utc::now().timestamp()));
    ```
    This unsanitized filename is passed directly to `save_transcript_to_file`, which concatenates it to create the target path:
    ```rust
    // crates/op-web/src/handlers/chat.rs:377-378
    let filepath = format!("/tmp/{}", filename);
    match tokio::fs::write(&filepath, &transcript).await {
    ```
    An attacker can supply a filename like `../../etc/cron.d/malicious_job` or `../../root/.ssh/authorized_keys` containing custom payloads inside the conversation transcript. Because the process executes with high privileges (often managing system configurations and running root-level D-Bus/Incus/OVS operations), this leads directly to a write-what-where vulnerability, permitting immediate privilege escalation or arbitrary code execution.
*   **Remediation:** 
    Extract only the base filename using `Path::file_name` and reject any inputs containing path traversal components or absolute paths. Ensure the resolved path remains strictly within the intended directory.
    ```rust
    let base_name = Path::new(&filename)
        .file_name()
        .context("Invalid filename")?;
    let filepath = Path::new("/tmp").join(base_name);
    ```

---

### 2. Denial of Service (SIGSEGV) / Out-of-bounds Read in WebSocket via Unsafe `simd-json` Parsing
*   **Vulnerability Type:** Undefined Behavior / Out-of-bounds Memory Read / Denial of Service
*   **Location:** `crates/op-web/src/websocket.rs:102` and `crates/op-web/src/handlers/websocket.rs:76`
*   **Impact:** Critical (Directly exploitable for Denial of Service)
*   **Description:** 
    The system reads text messages from untrusted WebSocket connections and parses them using the `unsafe` API of `simd_json`:
    ```rust
    // crates/op-web/src/websocket.rs:102
    let ws_msg: Result<WsMessage, _> = unsafe { simd_json::from_str(&mut raw) };
    ```
    ```rust
    // crates/op-web/src/handlers/websocket.rs:76
    let ws_msg: Result<WsMessage, _> = unsafe { simd_json::from_str(&mut raw) };
    ```
    `simd_json` relies on strict memory layout guarantees to execute high-performance SIMD instructions. In particular, the input buffer *must* have a padding of `simd_json::PADDING_SIZE` (typically 32 or 64 bytes) beyond the end of the string.
    By sending a short WebSocket payload that resides at the end of a allocated memory page, an attacker can trigger SIMD instructions that read past the boundary of the allocated buffer, leading to an immediate segmentation fault (`SIGSEGV`) and causing a complete Denial of Service of the unified system server.
*   **Remediation:** 
    Avoid unsafe in-place parsing of strings without checking and allocating required padding bytes. Use the safe `simd_json::from_slice` API over a `std::vec::Vec<u8>` which handles the necessary padding buffers transparently, or switch to standard `serde_json::from_str`.
    ```rust
    // Safe alternative using serde_json
    let ws_msg: Result<WsMessage, _> = serde_json::from_str(&text);
    ```

---

### 3. Hardcoded Security Bypass API Keys
*   **Vulnerability Type:** Hardcoded Credentials / Backdoor Access
*   **Location:** `crates/op-web/src/middleware/security.rs:16`
*   **Impact:** Critical (Directly exploitable)
*   **Description:** 
    The security middleware defines static, hardcoded API keys that instantly bypass all IP/Zone restrictions and grant full `AccessZone::TrustedMesh` privileges to any client:
    ```rust
    // crates/op-web/src/middleware/security.rs:16-19
    const BYPASS_API_KEYS: &[&str] = &[
        "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
        "test-key-huggingface-2024",            // Hugging Face test key
    ];
    ```
    An attacker who extracts these keys from the source code or binary can pass them via `x-api-key`, `Authorization: Bearer`, or `x-op-mcp-token` headers and gain administrative mesh privileges, allowing access to any system administration tools.
*   **Remediation:** 
    Remove all hardcoded API keys. Load authorized API keys dynamically from a cryptographically secure, restricted-access configuration file, or validate authentication tokens against users stored dynamically inside the system's state database.

---

## High & Medium Severity Findings

### 4. Global CSRF Token Pool Linkage in Google OAuth Callback
*   **Vulnerability Type:** Cryptographic / Logic Error (Cross-Site Request Forgery / Session Hijacking)
*   **Location:** `crates/op-web/src/handlers/privacy.rs:446` and `crates/op-web/src/handlers/privacy.rs:468`
*   **Impact:** High
*   **Description:** 
    Google OAuth uses a global in-memory state token pool in `AppState`:
    ```rust
    // crates/op-web/src/state.rs
    pub csrf_tokens: Arc<RwLock<HashMap<String, String>>>,
    ```
    When initiating OAuth, the CSRF token is inserted into this global pool:
    ```rust
    // crates/op-web/src/handlers/privacy.rs:446
    tokens.insert(csrf_token.secret().clone(), csrf_token.secret().clone());
    ```
    During the callback phase, the server validates the incoming state simply by verifying its existence in the global pool and removing it:
    ```rust
    // crates/op-web/src/handlers/privacy.rs:468
    if tokens.remove(&query.state).is_none() { ... }
    ```
    Because this verification token is not cryptographically or logically tied to the specific initiating user's browser session (e.g. via a secure, HTTP-only session cookie), an attacker can initiate a Google OAuth login on their own machine, capture the valid CSRF state token generated for them, and trick a victim's browser into visiting the callback URL with that token. The server will find the token in the global map, accept it, and link the victim's session/container to the attacker's Google identity.
*   **Remediation:** 
    Generate session cookies mapped specifically to the initiated CSRF token. Verify that the incoming CSRF token in the callback matches the state stored in the user's browser cookie.

---

### 5. Plaintext WireGuard Private Key Storage
*   **Vulnerability Type:** Insecure Credential Storage
*   **Location:** `crates/op-web/src/users.rs:32` and `crates/op-web/src/handlers/privacy.rs:188`
*   **Impact:** High
*   **Description:** 
    The `PrivacyUser` struct defines a field named `wg_private_key_encrypted`:
    ```rust
    // crates/op-web/src/users.rs:32
    pub wg_private_key_encrypted: String,
    ```
    However, the signup handler currently stores the unencrypted plaintext WireGuard private key directly in this field:
    ```rust
    // crates/op-web/src/handlers/privacy.rs:188-189
    // Create user (we'll encrypt the private key later, for now just store it)
    match state.user_store.create_user(&email, keypair.public_key, keypair.private_key).await
    ```
    This plaintext private key is persisted inside the public-facing directory `/var/lib/op-dbus/privacy-users.json`. Any compromise of this file exposes all users' WireGuard private keys, permitting unauthorized access to their private container networks.
*   **Remediation:** 
    Implement robust envelope encryption on user private keys prior to storing them on disk. Use a master key loaded securely at startup from environment variables or a hardware-protected KMS.

---

### 6. Resource Exhaustion / Unauthenticated Disk Bloat in Signup Flow
*   **Vulnerability Type:** Denial of Service / Resource Exhaustion
*   **Location:** `crates/op-web/src/handlers/privacy.rs:60`
*   **Impact:** Medium
*   **Description:** 
    The signup flow restricts signup frequency only *per email address*. 
    ```rust
    // crates/op-web/src/handlers/privacy.rs:60
    pub async fn signup(...)
    ```
    An attacker can make thousands of requests to this endpoint using distinct email strings. For every unique email address submitted, the server will:
    1.  Compute a CPU-intensive WireGuard keypair.
    2.  Write a new unverified user entry into `/var/lib/op-dbus/privacy-users.json`.
    3.  Generate and store a new magic link.
    This enables attackers to quickly exhaust the server's CPU and disk space, degrading system performance and crashing database reads.
*   **Remediation:** 
    Implement global and IP-based rate limiting on the `/api/privacy/signup` endpoint using the `tower_governor` middleware. Do not write dummy user rows to disk until email verification is complete.

---

### 7. Sensitive Credential Exposure via Logs and Console
*   **Vulnerability Type:** Sensitive Information Leak
*   **Location:** `crates/op-web/src/email.rs:64-75`
*   **Impact:** Medium
*   **Description:** 
    If SMTP credentials are not configured, the email sender logs the plaintext verification URLs and tokens directly to `stdout` and tracing logs:
    ```rust
    // crates/op-web/src/email.rs:64-71
    info!("🔗 MAGIC LINK (no SMTP configured):");
    info!("   To: {}", to_email);
    info!("   URL: {}", magic_url);
    info!("   Token: {}", token);
    println!("\n=== MAGIC LINK FOR TESTING ===");
    println!("Email: {}", to_email);
    println!("Link: {}", magic_url);
    ```
    If SMTP is misconfigured or disabled in production, these tokens leak directly to central log collectors (such as `systemd-journald` or external SIEM systems), allowing attackers with read-only log access to compromise user accounts.
*   **Remediation:** 
    Disable console logging of verification tokens if the application is not compiled in debug mode (`#[cfg(debug_assertions)]`), or mask the token value to prevent inadvertent leaks.

---

## Public API Surface & Dead Code

### Structural Review of Crate API Surface
*   **Glob Re-exports:** No glob re-exports (`pub use *`) were found in the examined codebase.
*   **Public Fields on Structs that should be private:**
    1.  `EmailConfig` in `crates/op-web/src/email.rs:16-24` exposes all of its fields publicly (`smtp_host`, `smtp_port`, `smtp_user`, `smtp_pass`, `from_email`, `from_name`, `base_url`). Since this configuration is initialized strictly via `from_env()`, these fields should be private with public read-only getter methods to prevent accidental or malicious configuration drift.
    2.  `AppState` in `crates/op-web/src/state.rs:77-111` exposes all inner system managers publicly (e.g., `orchestrator`, `tool_registry`, `chat_manager`, `user_store`, `state_store`). This allows arbitrary handler routes to manipulate core system states directly, bypassing orchestrated pathways and anti-hallucination guardrails.

---

### Top 10 Impactful Public API Elements
The following table details the most critical public API elements based on their footprint and potential impact on system architecture:

| Item | Type | file:line | Impact Analysis |
| :--- | :--- | :--- | :--- |
| `UnifiedOrchestrator` | `struct` | `crates/op-web/src/orchestrator/mod.rs:19` | Central cognitive engine. Coordinates tool routing and execution. |
| `AppState` | `struct` | `crates/op-web/src/state.rs:77` | Master dependency injection target. Shares all state engines across the HTTP system. |
| `UserStore` | `struct` | `crates/op-web/src/users.rs:56` | Master credential database. Handles user authentication and WireGuard mappings. |
| `GROUPS_CONFIG` | `static ref` | `crates/op-web/src/groups_admin.rs:108` | Global tool selector configuration. Directly impacts what capabilities are active. |
| `create_router` | `fn` | `crates/op-web/src/routes/mod.rs:30` | Master route composer. Binds security middlewares and routes. |
| `ensure_host_privacy_network` | `fn` | `crates/op-web/src/privacy_network.rs:60` | Network setup utility. Modifies system routing table and Open vSwitch states. |
| `publish_user_privacy_route` | `fn` | `crates/op-web/src/privacy_routes.rs:37` | Core route publisher. Mutates host-level kernel networking configurations. |
| `ensure_user_container` | `fn` | `crates/op-web/src/privacy_container.rs:59` | Incus container provisioning entrypoint. Modifies virtualization states. |
| `query_plugin_state` | `fn` | `crates/op-web/src/state_manager_client.rs:20` | Dynamic system contract reader. Accesses critical network states over system D-Bus. |
| `apply_plugin_state` | `fn` | `crates/op-web/src/state_manager_client.rs:41` | Dynamic state mutator. Writes operational changes to the host's StateManager. |

---

### Dead Code Table

The following items are defined but never referenced within any of the provided project files, or utilize `#[allow(dead_code)]` to suppress compilation warnings:

| Item / Identifier | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `broadcast` | `fn` (on `SseEventBroadcaster`) | `crates/op-web/src/sse.rs:30` | Remove or utilize in handlers (marked `#[allow(dead_code)]`). |
| `chat` | `mod` | `crates/op-web/src/routes/mod.rs:22` | Remove if replaced by newer unified handlers (marked `#[allow(dead_code)]`). |
| `llm` | `mod` | `crates/op-web/src/routes/mod.rs:24` | Remove if unused by current router layout (marked `#[allow(dead_code)]`). |
| `WebServiceRouter` | `struct` | `crates/op-web/src/router.rs:15` | Remove. Router creation is fully delegated to functional `create_router` calls. |
| `parse_journalctl_logs` | `fn` | `crates/op-web/src/handlers/logs.rs:120` | Remove. Code has been fully replaced by `linemux` log tailing. |
| `parse_wg_peers` | `fn` | `crates/op-web/src/handlers/vpn.rs:184` | Remove or integrate with active status page checks. |
| `format_duration` | `fn` | `crates/op-web/src/handlers/vpn.rs:222` | Remove since its only caller (`parse_wg_peers`) is dead code. |
| `mail_queue_handler` | `fn` | `crates/op-web/src/handlers/mail.rs:41` | Expose and test, or remove. Currently returns a hardcoded empty list placeholder. |
| `delete_memory_handler` | `fn` | `crates/op-web/src/handlers/mcp.rs:180` | Complete implementation or remove. Contains only placeholder logic. |
| `memory_stats_handler` | `fn` | `crates/op-web/src/handlers/mcp.rs:191` | Integrate with real memory store or remove. |

---

## Schema-as-Code Discipline Compliance

The project enforces a Schema-as-Code philosophy to maintain structured contracts instead of ad-hoc formatting. The codebase fails to meet this discipline in several locations, expressing critical system configurations as unstructured types:

### 1. Incus Container Configuration States
*   **File Location:** `crates/op-web/src/privacy_container.rs:25-50`
*   **Ad-hoc Structs:** `IncusState`, `IncusInstance`
*   **Audit Finding:** 
    The target state of virtualization containers is represented using ad-hoc, untyped structures with nested HashMaps for configuration options and device layouts:
    ```rust
    config: Option<HashMap<String, String>>,
    devices: Option<HashMap<String, HashMap<String, String>>>,
    ```
    This lacks version control, validation schemas, or schema-defined contracts, violating the Protocol Buffer architecture.
*   **Remediation:** 
    Define the container target state as a versioned Protocol Buffer schema (e.g. `IncusInstanceTargetState.proto`) and generate safe Rust structures using `prost`.

---

### 2. OpenFlow Routing Configurations
*   **File Location:** `crates/op-web/src/privacy_openflow.rs:11-53`
*   **Ad-hoc Structs:** `OpenFlowConfig`, `BridgeFlowConfig`, `FlowEntry`, `FlowAction`, `SocketPort`
*   **Audit Finding:** 
    Kernel flow tables, OpenFlow priorities, and matching properties are modeled as ad-hoc serializable structures:
    ```rust
    pub match_fields: HashMap<String, String>,
    pub flow_policies: Option<Vec<Value>>,
    ```
    This unstructured format bypasses schema boundaries and does not guarantee correctness when communicating policies to state controllers.
*   **Remediation:** 
    Consolidate OpenFlow and bridge configuration boundaries into formal, versioned protobuf schemas compiled and managed centrally.

---

### 3. Dynamic User Routing State
*   **File Location:** `crates/op-web/src/privacy_routes.rs:14-30`
*   **Ad-hoc Struct:** `PrivacyRoute`
*   **Audit Finding:** 
    Operational network routing details (ingress ports, network device associations, WireGuard configurations) are serialized using custom, unversioned JSON structures. 
*   **Remediation:** 
    Define standard, versioned network route models utilizing version-controlled Protocol Buffers or standardized OSCAL system configurations to ensure long-term structure integrity.