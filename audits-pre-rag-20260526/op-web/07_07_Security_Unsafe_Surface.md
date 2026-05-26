### 1. Unsafe Blocks Audit

All `unsafe` blocks in the audited files are listed below. Every single `unsafe` block identified is **missing** the required `// SAFETY:` documentation comment explaining its correctness.

*   **`crates/op-web/src/groups_admin.rs:47`**
    ```rust
    if let Ok(saved) = unsafe { simd_json::from_str::<HashMap<String, EnabledGroups>>(&mut raw) }
    ```
    *   **Flag**: Missing `// SAFETY:` comment.

*   **`crates/op-web/src/state_manager_client.rs:31`**
    ```rust
    let query_state: QueryStateResponse = unsafe { simd_json::from_str(&mut state_json) }
    ```
    *   **Flag**: Missing `// SAFETY:` comment.

*   **`crates/op-web/src/websocket.rs:98`**
    ```rust
    let ws_msg: Result<WsMessage, _> = unsafe { simd_json::from_str(&mut raw) };
    ```
    *   **Flag**: Missing `// SAFETY:` comment.

*   **`crates/op-web/src/users.rs:95`**
    ```rust
    let data: StoredData = unsafe { simd_json::from_str(&mut raw) }?;
    ```
    *   **Flag**: Missing `// SAFETY:` comment.

*   **`crates/op-web/src/handlers/websocket.rs:64`**
    ```rust
    let ws_msg: Result<WsMessage, _> = unsafe { simd_json::from_str(&mut raw) };
    ```
    *   **Flag**: Missing `// SAFETY:` comment.

*   **`crates/op-web/src/orchestrator/execution.rs:60`**
    ```rust
    unsafe { simd_json::from_str(&mut raw) }.unwrap_or(json!({}))
    ```
    *   **Flag**: Missing `// SAFETY:` comment.

*   **`crates/op-web/src/orchestrator/parsing.rs:31`**
    ```rust
    if let Ok(parsed) = unsafe { simd_json::from_str(&mut raw) } {
    ```
    *   **Flag**: Missing `// SAFETY:` comment.

*   **`crates/op-web/src/orchestrator/parsing.rs:80`**
    ```rust
    if let Ok(parsed) = unsafe { simd_json::from_str(&mut raw) } {
    ```
    *   **Flag**: Missing `// SAFETY:` comment.

*   **`crates/op-web/src/orchestrator/parsing.rs:104`**
    ```rust
    unsafe { simd_json::from_str(&mut raw) }.unwrap_or(json!({}))
    ```
    *   **Flag**: Missing `// SAFETY:` comment.

*   **`crates/op-web/src/orchestrator/parsing.rs:120`**
    ```rust
    if let Ok(parsed) = unsafe { simd_json::from_str::<Value>(&mut raw) } {
    ```
    *   **Flag**: Missing `// SAFETY:` comment.

---

### 2. Command Spawning Audit

A total of **8** command spawning sites (`Command::new` or `tokio::process::Command::new`) were identified. 

None of the spawned commands are controlled directly by user HTTP input, but environment variable overrides represent minor external entry points. There is no active shell interpolation, preventing typical command injection, though flag injection is theoretically possible if the environment variables are manipulated.

*   **`crates/op-web/src/wireguard.rs:69`**
    ```rust
    Command::new("wg").args(["show", interface, "public-key"])
    ```
    *   **Analysis**: `interface` is loaded via `std::env::var("WG_INTERFACE")` (defaults to `"wg0"`). No direct user control, but an environment variable override can alter arguments.

*   **`crates/op-web/src/handlers/dashboard.rs:47`**
    ```rust
    Command::new("wg").args(&["show", "wg0", "peers"])
    ```
    *   **Analysis**: Arguments are hardcoded and static. No user control.

*   **`crates/op-web/src/handlers/logs.rs:31`**
    ```rust
    Command::new("tail").args(&["-n", "50", log_path])
    ```
    *   **Analysis**: `log_path` is selected from a hardcoded whitelist (`/var/log/op-web.log`, `/var/log/op-dbus.log`, `/tmp/op-web.log`). No user control.

*   **`crates/op-web/src/handlers/status.rs:175`**
    ```rust
    tokio::process::Command::new("doas").arg("dinitctl").arg("list")
    ```
    *   **Analysis**: Arguments are completely hardcoded. No user control.

*   **`crates/op-web/src/handlers/vpn.rs:37`**
    ```rust
    Command::new("wg").args(&["show", interface])
    ```
    *   **Analysis**: `interface` is hardcoded to `"wg0"`. No user control.

*   **`crates/op-web/src/handlers/vpn.rs:50`**
    ```rust
    Command::new("wg").args(&["show", interface, "dump"])
    ```
    *   **Analysis**: `interface` is hardcoded to `"wg0"`. No user control.

*   **`crates/op-web/src/handlers/vpn.rs:98`**
    ```rust
    Command::new("wg").args(&["show", interface, "public-key"])
    ```
    *   **Analysis**: `interface` is hardcoded to `"wg0"`. No user control.

*   **`crates/op-web/src/handlers/mail.rs:26`**
    ```rust
    Command::new("incus").args(&["exec", "crd-astral", "--", "systemctl", "is-active", "maddy"])
    ```
    *   **Analysis**: Arguments are completely hardcoded. No user control.

---

### 3. Forbidden Commands

No direct invocations of the forbidden command patterns (`ovs-*`, `dpctl`, shell interpreters like `sh` or `bash`, or data exfiltration utilities like `curl`/`wget`) were found in the compiled Rust codebase. 

*(Note: Although `systemctl` is referenced inside an `incus` container command in `handlers/mail.rs:26` and `doas` is executed in `handlers/status.rs:175`, none of these violate the specific forbidden command patterns).*

---

### 4. Hardcoded Secrets, IPs, and Tokens

*   **`crates/op-web/src/middleware/security.rs:15-18`** [CRITICAL]
    ```rust
    const BYPASS_API_KEYS: &[&str] = &[
        "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
        "test-key-huggingface-2024",            // Hugging Face test key
    ];
    ```
    *   **Vulnerability**: Statically compiled bypass tokens. Any client supplying these tokens via `x-api-key`, `authorization`, or `x-op-mcp-token` headers bypasses all IP-based restriction zones and is immediately granted `TrustedMesh` privileges. This allows complete administrative control over the Control Plane.

*   **`crates/op-web/src/wireguard.rs:45`** [HIGH]
    ```rust
    .unwrap_or_else(|_| "148.113.204.83:51820".to_string());
    ```
    *   **Vulnerability**: Hardcoded public IP address (`148.113.204.83`) used as the default fallback VPN endpoint.

*   **`crates/op-web/src/handlers/vpn.rs:113`** [HIGH]
    ```rust
    std::env::var("VPN_ENDPOINT").unwrap_or_else(|_| "148.113.204.83:51820".to_string());
    ```
    *   **Vulnerability**: Duplicate hardcoded fallback public IP address (`148.113.204.83`).

*   **`crates/op-web/src/bin/op-dbus.rs:24`** [MEDIUM]
    ```rust
    std::env::var("OP_DBUS_GRPC_LISTEN").unwrap_or_else(|_| "10.200.0.2:50051".to_string());
    ```
    *   **Vulnerability**: Hardcoded default private listen IP (`10.200.0.2:50051`).

*   **`crates/op-web/src/state.rs:232`** [MEDIUM]
    ```rust
    std::env::var("OP_DBUS_GRPC_ADDR").unwrap_or_else(|_| "http://10.200.0.2:50051".to_string());
    ```
    *   **Vulnerability**: Hardcoded default gRPC destination IP (`10.200.0.2:50051`).

*   **`crates/op-web/src/privacy_network.rs:34`** [MEDIUM]
    ```rust
    std::env::var("XRAY_INGRESS_IP").unwrap_or_else(|_| "10.200.0.1".to_string()),
    ```
    *   **Vulnerability**: Hardcoded default ingress IP address (`10.200.0.1`).

*   **`crates/op-web/src/system_prompt_loader.rs:17-18`** [LOW]
    ```rust
    "/home/jeremy/git/gemini-op-dbus/LLM-SYSTEM-PROMPT-COMPLETE.txt",
    "/home/jeremy/op-dbus-v2/LLM-SYSTEM-PROMPT-COMPLETE.txt",
    ```
    *   **Vulnerability**: Exposure of local home directories containing developer username (`jeremy`) hardcoded into production search paths.

---

### 5. D-Bus Method Exposure

The `op-web` crate does **not** expose any system D-Bus services or methods directly to peers on the system bus. 

Instead, it functions as a pure D-Bus client connecting to `org.opdbus.StateManager` at the object path `/org/opdbus/v1/state` (D-Bus destination `org.opdbus.v1`). It invokes the following methods:

*   **`QueryState`** (`state_manager_client.rs:33`) — Used to pull desired configuration state for the `incus` and `privacy_routes` plugins.
*   **`ApplyContractMutation`** (`state_manager_client.rs:61`) — Used to push mutated contract configurations (such as creating new container instances or declaring routing policies) to the central StateManager control loop.