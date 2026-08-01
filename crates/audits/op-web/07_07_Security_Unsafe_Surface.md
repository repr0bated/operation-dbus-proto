# Production Security and Quality Audit: op-web

## SECTION 1: UNSAFE BLOCKS AUDIT

### 1. `crates/op-web/src/groups_admin.rs:53`
*   **Context**:
    ```rust
    if let Ok(saved) =
        unsafe { simd_json::from_str::<HashMap<String, EnabledGroups>>(&mut raw) }
    ```
*   **Flag**: Missing `// SAFETY:` comment.
*   **Analysis**: This function parses the tool groups profile from disk. While the file path `/var/lib/op-dbus/tool-groups.json` is typically owned by root, the use of `simd_json::from_str` on a string slice requires the input string to have a 32-byte padding. Since `raw` is a standard clone of a read string, it may lack the required SIMD padding, raising concerns of minor out-of-bounds reads depending on allocation bounds.

### 2. `crates/op-web/src/state_manager_client.rs:38`
*   **Context**:
    ```rust
    let query_state: QueryStateResponse = unsafe { simd_json::from_str(&mut state_json) }
    ```
*   **Flag**: Missing `// SAFETY:` comment.
*   **Analysis**: Parses JSON output fetched from a D-Bus system proxy. If a malicious system-bus peer can spoof the D-Bus StateManager response or hijack its namespace, this unsafe deserialization could trigger memory safety issues since the buffer is modified in place and lacks verification of the SIMD padding alignment.

### 3. `crates/op-web/src/websocket.rs:92`
*   **Context**:
    ```rust
    let ws_msg: Result<WsMessage, _> = unsafe { simd_json::from_str(&mut raw) };
    ```
*   **Flag**: Missing `// SAFETY:` comment.
*   **Severity**: **CRITICAL** (Directly Exploitable)
*   **Analysis**: The `text` parameter is a standard `String` populated directly from the WebSocket client (`Message::Text(text)`). Calling `simd_json::from_str` on a cloned, non-padded standard string is inherently unsafe. `simd_json` relies on a 32-byte padding buffer zone at the end of the parsed string to execute vector instructions safely. Passing un-padded, user-controlled heap strings can result in out-of-bounds reads and trigger a segmentation fault, allowing any unauthenticated external attacker to crash the web server via a crafted WebSocket payload.

### 4. `crates/op-web/src/users.rs:117`
*   **Context**:
    ```rust
    let data: StoredData = unsafe { simd_json::from_str(&mut raw) }?;
    ```
*   **Flag**: Missing `// SAFETY:` comment.
*   **Analysis**: Deserializes the local persistence database. If local attackers can write to the JSON user database (e.g., if permissions are weak), they can exploit the un-padded SIMD deserialization to trigger crashes.

### 5. `crates/op-web/src/handlers/websocket.rs:68`
*   **Context**:
    ```rust
    let ws_msg: Result<WsMessage, _> = unsafe { simd_json::from_str(&mut raw) };
    ```
*   **Flag**: Missing `// SAFETY:` comment.
*   **Severity**: **CRITICAL** (Directly Exploitable)
*   **Analysis**: Identical to `src/websocket.rs:92`. This endpoint is exposed globally on the WebSocket router and receives raw client text. It executes an unsafe, un-padded SIMD JSON parse directly on untrusted network data, resulting in a severe Denial of Service vector.

### 6. `crates/op-web/src/orchestrator/execution.rs:44`
*   **Context**:
    ```rust
    let args: Value = if parts.len() > 1 {
        let mut raw = parts[1].trim().to_string();
        unsafe { simd_json::from_str(&mut raw) }.unwrap_or(json!({}))
    ```
*   **Flag**: Missing `// SAFETY:` comment.
*   **Analysis**: This executes direct tool input from users. If user-submitted commands bypass standard filtering and execute directly on non-padded raw string slices, the underlying SIMD instructions can read arbitrary memory pages past the buffer boundary.

### 7. `crates/op-web/src/orchestrator/parsing.rs:33`
*   **Context**:
    ```rust
    if let Ok(parsed) = unsafe { simd_json::from_str(&mut raw) } {
    ```
*   **Flag**: Missing `// SAFETY:` comment.
*   **Analysis**: Parsed within the LLM agent workflow. If the LLM generates a tool call containing malformed or crafted payload strings without padding, this can cause the orchestration daemon to crash.

### 8. `crates/op-web/src/orchestrator/parsing.rs:92`
*   **Context**:
    ```rust
    if let Ok(parsed) = unsafe { simd_json::from_str(&mut raw) } {
    ```
*   **Flag**: Missing `// SAFETY:` comment.
*   **Analysis**: Executes unsafe JSON parsing on regex-extracted matches from LLM text responses, which lack any safety bounds or padding.

### 9. `crates/op-web/src/orchestrator/parsing.rs:126`
*   **Context**:
    ```rust
    let args: Value = if call.arguments.is_str() {
        {
            let mut raw = call.arguments.as_str().unwrap().to_string();
            unsafe { simd_json::from_str(&mut raw) }.unwrap_or(json!({}))
    ```
*   **Flag**: Missing `// SAFETY:` comment.
*   **Analysis**: Deserializes argument strings directly inside nested tool calls. Lacks memory boundary verification and padding.

### 10. `crates/op-web/src/orchestrator/parsing.rs:144`
*   **Context**:
    ```rust
    if let Ok(parsed) = unsafe { simd_json::from_str::<Value>(&mut raw) } {
    ```
*   **Flag**: Missing `// SAFETY:` comment.
*   **Analysis**: Parsed from regex content captures of assistant/tool communication logs.

---

## SECTION 2: COMMAND EXECUTION & FORBIDDEN COMMANDS AUDIT

### Command Execution Statistics
There are **8** instances of `Command::new` or similar command execution patterns in this codebase:

1.  `crates/op-web/src/wireguard.rs:69`:
    *   Command: `Command::new("wg").args(["show", interface, "public-key"])`
    *   User-Control: Low. The `interface` variable is populated from `std::env::var("WG_INTERFACE")` (defaulting to `"wg0"`).
2.  `crates/op-web/src/handlers/dashboard.rs:44`:
    *   Command: `Command::new("wg").args(&["show", "wg0", "peers"])`
    *   User-Control: None. Fully hardcoded arguments.
3.  `crates/op-web/src/handlers/logs.rs:41`:
    *   Command: `Command::new("tail").args(&["-n", "50", log_path])`
    *   User-Control: None. Uses a static list of files (`/var/log/op-web.log`, `/var/log/op-dbus.log`, `/tmp/op-web.log`).
4.  `crates/op-web/src/handlers/status.rs:196`:
    *   Command: `tokio::process::Command::new("doas").arg("dinitctl").arg("list")`
    *   User-Control: None. Fully hardcoded.
5.  `crates/op-web/src/handlers/vpn.rs:45`:
    *   Command: `Command::new("wg").args(&["show", interface])`
    *   User-Control: None. Hardcoded interface value (`"wg0"`).
6.  `crates/op-web/src/handlers/vpn.rs:60`:
    *   Command: `Command::new("wg").args(&["show", interface, "dump"])`
    *   User-Control: None. Hardcoded interface value (`"wg0"`).
7.  `crates/op-web/src/handlers/vpn.rs:113`:
    *   Command: `Command::new("wg").args(&["show", interface, "public-key"])`
    *   User-Control: None. Hardcoded interface value (`"wg0"`).
8.  `crates/op-web/src/handlers/mail.rs:44`:
    *   Command: `Command::new("incus").args(&["exec", "crd-astral", "--", "systemctl", "is-active", "maddy"])`
    *   User-Control: None. Fully hardcoded arguments.

### Forbidden Command Audit
*   **Result**: No forbidden command-spawns are executed directly in the audited Rust code. The use of shell-bypass commands (like `sh`, `bash`), exfiltration tools, or raw `ovs-*` commands was not observed in any `Command::new` invocation.
*   **Exclusion Note**: While `crates/op-web/src/orchestrator/anti_hallucination.rs` lists forbidden command patterns (e.g., `ovs-vsctl`, `systemctl`, `ip addr`), these patterns are strictly utilized inside static lists for pattern-matching and LLM output verification; they are never executed on the host system.

---

## SECTION 3: HARDCODED CREDENTIALS, IPS, & BYPASS KEYS

### 1. Hardcoded Administrative Bypass API Keys
*   **Citation**: `crates/op-web/src/middleware/security.rs:17-20`
*   **Code**:
    ```rust
    const BYPASS_API_KEYS: &[&str] = &[
        "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
        "test-key-huggingface-2024",            // Hugging Face test key
    ];
    ```
*   **Severity**: **CRITICAL** (Directly Exploitable Backdoor)
*   **Analysis**: This array hardcodes static API keys directly into the source code. The security middleware `ip_security_middleware` checks for these keys inside request headers (`x-api-key`, `authorization` Bearer, or `x-op-mcp-token`). If any matched key is found, the connection is instantly promoted to `AccessZone::TrustedMesh`. This bypasses all IP-based firewalls, local network checks, and security boundaries. Any external client sending the public bypass token `"4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f"` gains full administrative access to the entire tool suite.

### 2. Hardcoded Default WireGuard and gRPC IPs
*   **Citation**: `crates/op-web/src/wireguard.rs:43-45`
*   **Code**:
    ```rust
    let endpoint = std::env::var("WG_SERVER_ENDPOINT")
        .or_else(|_| std::env::var("VPN_ENDPOINT"))
        .unwrap_or_else(|_| "148.113.204.83:51820".to_string());
    ```
*   **Analysis**: Hardcodes the public IP Address `148.113.204.83:51820` as the fallback endpoint for WireGuard.
*   **Citation**: `crates/op-web/src/state.rs:253-254`
*   **Code**:
    ```rust
    let grpc_addr = std::env::var("OP_DBUS_GRPC_ADDR")
        .unwrap_or_else(|_| "http://10.200.0.2:50051".to_string());
    ```
*   **Analysis**: Hardcodes `10.200.0.2:50051` as the internal gRPC listener for D-Bus proxy routing.
*   **Citation**: `crates/op-web/src/bin/op-dbus.rs:17-18`
*   **Code**:
    ```rust
    let listen =
        std::env::var("OP_DBUS_GRPC_LISTEN").unwrap_or_else(|_| "10.200.0.2:50051".to_string());
    ```
*   **Analysis**: Hardcodes the host internal network bind address `10.200.0.2`.

---

## SECTION 4: SCHEMA-AS-CODE COMPLIANCE

The codebase utilizes ad-hoc structures and dynamic string maps rather than formal, versioned schemas (such as Protocol Buffers or OSCAL) for contract management and state storage.

### 1. Ad-Hoc Incus State Representation
*   **Citation**: `crates/op-web/src/privacy_container.rs:32-52`
*   **Analysis**: `IncusState` and `IncusInstance` are mapped as ad-hoc Rust structs serialized into raw JSON. Instead of relying on a versioned schema file, properties like custom profiles and OVS configuration flags are appended as unversioned keys inside the `config` dynamic hashmap (e.g., `user.opdbus.route_id` and `user.opdbus.assigned_ip`).

### 2. Ad-Hoc OpenFlow Configuration Contracts
*   **Citation**: `crates/op-web/src/privacy_openflow.rs:10-53`
*   **Analysis**: `OpenFlowConfig`, `BridgeFlowConfig`, and `FlowEntry` define network-level forwarding actions and matching rules directly as ad-hoc Rust schemas. Modifying or extending these structs can silently break compatibility with existing system state stores, as there are no versioning headers or validation constraints.

### 3. Ad-Hoc Dynamic JSON-RPC MCP Definitions
*   **Citation**: `crates/op-web/src/mcp.rs:48-93`
*   **Analysis**: `McpRequest`, `McpResponse`, and `McpError` use standard JSON-RPC structures mapped directly inside Rust code. Dynamic fields such as the request `params` are stored as raw `simd_json::OwnedValue`, preventing compile-time validation of arguments.

### 4. Dynamic User Metadata Contracts
*   **Citation**: `crates/op-web/src/users.rs:14-46`
*   **Analysis**: The `PrivacyUser` record defines core user-access metadata, containing active state markers and sensitive cryptographic keys. It is serialized directly into a flat JSON format (`/var/lib/op-dbus/privacy-users.json`). Changing the structural layout of this Rust struct will cause serialization failures or state loss on legacy deployments due to the lack of schema-as-code version migrations.

---

## SECTION 5: SYSTEM D-BUS EXPOSURE

`op-web` does not register, host, or expose any custom D-Bus methods directly to system-bus peers. It functions exclusively as a client consuming D-Bus services.

### D-Bus Client Consumption Details
*   **Proxy Target**: `org.opdbus.StateManager` at path `/org/opdbus/v1/state` via the interface `org.opdbus.StateManager` over the **System Bus** (`Connection::system()`).
*   **Citations**:
    *   `crates/op-web/src/state_manager_client.rs:13-19` (Proxy initialization)
    *   `crates/op-web/src/state_manager_client.rs:31` (Querying state using `QueryState`)
    *   `crates/op-web/src/state_manager_client.rs:64` (Applying state changes via `ApplyContractMutation`)
*   **Access Control**: Since `op-web` connects to the system bus to query and apply state mutations (including creating containers and managing routes), it relies on the D-Bus configuration daemon (`/etc/dbus-1/system.d/`) to enforce policy controls on the target `org.opdbus.StateManager` service. `op-web` itself does not expose listening entry points on the D-Bus system loop.