# Production Security and Quality Audit Report

---

## Section 1: License Audit & Cargo Dependencies

### 1.1 License Field Extraction
*   **Workspace License:** The workspace package metadata defines the global license as **`Apache-2.0`** (`Cargo.toml` line 38).
*   **Crate License:** The `op-web` crate inherits this license via **`license.workspace = true`** (`crates/op-web/Cargo.toml` line 6).

### 1.2 Cargo Lock GPL/AGPL/SSPL Scan
A comprehensive scan of `Cargo.lock` was conducted to identify copyleft licenses (GPL, AGPL, SSPL) that could introduce licensing incompatibility with the Apache-2.0 control plane:
*   **Result:** No GPL, AGPL, or SSPL licensed crates were detected in the `Cargo.lock` dependency tree.
*   **Permissive Dependencies Checked:** Key system-level dependencies such as `cozo` (MPL-2.0), `sqlx` (MIT/Apache-2.0), `ring` (ISC-like/MIT/BSD), and `lettre` (MIT) conform to permissible licensing guidelines.

### 1.3 Crates with No License Field
*   **Result:** All visible crate definitions (`op-web` and `op-dbus`) contain explicit workspace-level license configurations. No crates within the visible files are missing license declarations.

---

## Section 2: Critical Security Findings

### 2.1 Path Traversal / Arbitrary File Write via Chat Transcripts
*   **File:** `crates/op-web/src/handlers/chat.rs:318-368`
*   **Severity:** **Critical** (Directly Exploitable)
*   **Impact:** Arbitrary Remote File Write / Remote Code Execution (RCE)

#### Description
The `save_transcript_handler` allows users to specify an arbitrary `filename` parameter extracted directly from the deserialized JSON payload:
```rust
let filename = params
    .get("filename")
    .and_then(|v| v.as_str())
    .map(str::to_string)
    .unwrap_or_else(|| format!("chat-transcript-{}.txt", chrono::Utc::now().timestamp()));
```
This filename is then combined via `format!` into `filepath`:
```rust
let filepath = format!("/tmp/{}", filename);
match tokio::fs::write(&filepath, &transcript).await { ... }
```
Because there is absolutely no validation, sanitization, or canonicalization of the `filename` parameter (such as stripping `../` path segments), an unauthenticated attacker can execute a path traversal attack. By passing a filename like `../../etc/cron.d/malicious_job` or `../../home/user/.ssh/authorized_keys` and populating the `messages` array with structured text, the attacker can write arbitrary payloads to any writable path on the host filesystem.

Furthermore, this route is mounted publicly without any access zone validation in `crates/op-web/src/routes/mod.rs`.

#### Remediation
Ensure the filename is strictly alphanumeric or use `Path::file_name` to extract only the base name, rejecting any directory components.
```rust
use std::path::Path;

let filename = params
    .get("filename")
    .and_then(|v| v.as_str())
    .and_then(|name| Path::new(name).file_name())
    .and_then(|os_str| os_str.to_str())
    .map(|s| s.to_string())
    .unwrap_or_else(|| format!("chat-transcript-{}.txt", chrono::Utc::now().timestamp()));
```

---

### 2.2 Unauthenticated Disclosure of Client WireGuard Private Keys
*   **File:** `crates/op-web/src/handlers/privacy.rs:324-411`
*   **Severity:** **Critical** (Directly Exploitable)
*   **Impact:** Complete VPN Identity Theft / Traffic Interception

#### Description
The `privacy_access_message` endpoint serves as a public human-readable access confirmation page. It takes a `user_id` query parameter and queries the user store:
```rust
} else if let Some(user_id) = &query.user_id {
    match state.user_store.get_user(user_id).await {
    Some(user) if user.email_verified => {
        ...
        let config = generate_client_config(
            &user.wg_private_key_encrypted, // Placed directly in the client configuration
            &user.assigned_ip,
            &state.server_config,
        );
```
An attacker who learns or brute-forces a user's UUID (or retrieves it through log leaks) can query `/privacy/access?user_id=xxx` anonymously. The server generates a complete WireGuard configuration containing the **unencrypted client private key** (`user.wg_private_key_encrypted` is stored in plaintext/reversible format despite the variable name) and displays it directly in raw HTML. This allows any third party to impersonate the client on the VPN.

#### Remediation
Do not return the client's private key from the server after registration. The client's private key should be generated client-side, and only the public key should be uploaded to the server. If configurations must be returned, restrict this endpoint with strong authentication headers matching the user's active session token.

---

## Section 3: Concurrency, Quality, & Architectural Findings

### 3.1 Cross-Session Data Exposure in Duplicate WebSocket Handler
*   **File:** `crates/op-web/src/handlers/websocket.rs:85-115`
*   **Severity:** **High** (Information Disclosure)
*   **Impact:** Multi-tenant session isolation bypass

#### Description
The application contains two different WebSocket implementations: `crates/op-web/src/websocket.rs` (which uses isolated session channels) and `crates/op-web/src/handlers/websocket.rs` (which uses a global broadcast channel). 

In `crates/op-web/src/handlers/websocket.rs`, incoming messages are processed through the orchestrator, and the responses are pushed directly to `state_clone.broadcast_tx`:
```rust
// Process through orchestrator
match state_clone.orchestrator.process(&sid, &message).await {
    Ok(result) => {
        let response = WsMessage::Response {
            success: result.success,
            message: result.message,
            tools_executed: result.tools_executed,
        };
        
        // Broadcast to all connected clients
        let _ = state_clone.broadcast_tx.send(
            simd_json::to_string(&response).unwrap()
        );
    }
```
Because `broadcast_tx` is shared globally across the entire `AppState`, **every connected WebSocket client** receives the output of every other client's administrative commands. This leaks sensitive system logs, database states, and active processes across user boundaries.

#### Remediation
Remove `crates/op-web/src/handlers/websocket.rs` completely and consolidate all WebSocket routing to use the session-isolated implementation defined in `crates/op-web/src/websocket.rs`.

---

### 3.2 Violation of Schema-as-Code Discipline
*   **Files:** 
    *   `crates/op-web/src/state_manager_client.rs:7-55`
    *   `crates/op-web/src/privacy_openflow.rs:10-40`
    *   `crates/op-web/src/privacy_container.rs:31-48`
*   **Severity:** **Medium** (Code Quality & Compliance)
*   **Impact:** Fragile contract management, drift from OSCAL / Protobuf compliance

#### Description
The control plane utilizes D-Bus and StateManager IPC to coordinate container and networking policies. However, instead of importing versioned Protocol Buffer schemas or structured OSCAL specifications, the codebase expresses critical infrastructure states using ad-hoc `serde` structs and unstructured JSON-RPC containers:

1.  **StateManager Client:** `apply_plugin_state` manually formats unstructured JSON payloads via `simd_json::json!` and writes them as raw strings across the D-Bus boundary:
    ```rust
    let request = simd_json::json!({
        "plugin_id": plugin_id,
        "value": value,
    });
    ```
2.  **OpenFlow Bridge:** Configuration is represented as an ad-hoc Rust struct (`OpenFlowConfig`) mapping to anonymous values (`Vec<Value>`) rather than versioned schema contracts.
3.  **Incus Provisioner:** Container definitions use manually matched nested maps (`HashMap<String, HashMap<String, String>>`) to provision container resources dynamically.

#### Remediation
Enforce the project's schema-as-code discipline. Autogenerate type-safe Rust interfaces using the project's standard Protocol Buffer compiler (`prost`), and reject the usage of unstructured ad-hoc mapping layers (`simd_json::json!`) for system configuration changes.

---

### 3.3 Undefined Behavior Risk via Unsafe `simd_json::from_str` with Unpadded Str Clones
*   **File:** `crates/op-web/src/groups_admin.rs:52`
*   **Severity:** **Low** (Stability / Memory Safety)
*   **Impact:** Potential out-of-bounds read or daemon crash

#### Description
At line 52 of `groups_admin.rs`, the system deserializes saved profiles from disk using `unsafe { simd_json::from_str }`:
```rust
if let Ok(content) = std::fs::read_to_string(GROUPS_CONFIG_PATH) {
    let mut raw = content.clone();
    if let Ok(saved) =
        unsafe { simd_json::from_str::<HashMap<String, EnabledGroups>>(&mut raw) }
```
`simd-json` requires that input buffers have a padding of at least `simd_json::PADDING_SIZE` bytes at the end of the string to allow safe SIMD vector registers to overrun without causing segmentation faults or out-of-bound memory reads. Cloning a standard `std::fs::read_to_string` result does not guarantee this padding, making the `unsafe` deserialization step unstable and prone to crashing the system process if the JSON file is modified with specific byte boundaries.

#### Remediation
Replace the `unsafe` deserializer with `simd_json::from_slice` after copying the data into a padded vector, or use the safe API wrappers provided by the `simd_json` crate.