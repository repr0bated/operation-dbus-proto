### D-Bus & IPC Attack Surface Analysis

#### Registered D-Bus Interfaces, Methods, and Signals
The audited files in the `op-web` crate **do not register** (i.e., define, expose, or implement) any D-Bus interfaces, methods, or signals of their own. 

However, `op-web` acts as a highly privileged D-Bus client that interacts directly with external D-Bus interfaces on both the **System Bus** and the **Session Bus**.

#### Outbound D-Bus Method Calls
`op-web` makes outbound RPC calls to the following system service:
* **Destination (Service)**: `org.opdbus.v1`
* **Object Path**: `/org/opdbus/v1/state`
* **Interface**: `org.opdbus.StateManager`
* **Connected Bus**: System Bus (`Connection::system()`)
* **Invoked Methods**:
  1. **`QueryState`** (`crates/op-web/src/state_manager_client.rs:25`):
     * *Purpose*: Retrieves the current operational state of a specified plugin.
     * *Caller Identity Check*: None. This is an outbound client call; however, the HTTP handlers in `op-web` that trigger this call do not authenticate the remote HTTP client before forwarding.
  2. **`ApplyContractMutation`** (`crates/op-web/src/state_manager_client.rs:52`):
     * *Purpose*: Mutates system states or executes configurations for plugins (such as `incus`, `privacy_routes`, or `openflow`).
     * *Caller Identity Check*: None. Any remote user capable of triggering the HTTP endpoints can cause arbitrary contract mutations on the system control plane.

Additionally, `op-web` triggers automated D-Bus introspection and projection discovery on both buses:
* **System Bus** (`crates/op-web/src/state.rs:783`): Discovers and projects system services.
* **Session Bus** (`crates/op-web/src/state.rs:796`): Discovers and projects user-level services.

---

### Security Findings

#### [CRITICAL] No Enforcement of Access Zones in `ip_security_middleware`
* **File:Line**: `crates/op-web/src/middleware/security.rs:136` (referenced in `crates/op-web/src/routes/mod.rs:281`)
* **Description**: The `ip_security_middleware` determines the caller's IP-based `AccessZone` (e.g., `Localhost`, `TrustedMesh`, `Public`) and successfully inserts it into the request extensions. However, the middleware **never rejects** unauthorized requests. It always completes by calling `next.run(request).await`. Because the target route handlers (such as tool execution, agent spawning, and configuration updates) fail to extract or enforce this `AccessZone` extension, all highly privileged endpoints are fully public over the network.
* **Remediation**: Update the middleware to return `StatusCode::FORBIDDEN` or `StatusCode::UNAUTHORIZED` if the request originates from an untrusted zone and lacks valid authentication headers.

#### [CRITICAL] Unauthenticated Privilege Escalation and Remote Code Execution via Direct Tool API
* **File:Line**: `crates/op-web/src/handlers/tools.rs:72` (and `crates/op-web/src/handlers/tools.rs:82`)
* **Description**: The `execute_tool_handler` and `execute_named_tool_handler` expose direct execution of any registered system tool (including `shell_exec`, file modification, OpenFlow manipulation, and dinit service modification) with arbitrary client-supplied arguments. Because no authentication or authorization checks are performed on these handlers, an external attacker can execute arbitrary shell commands or take over the host operating system.
* **Remediation**: Implement strict token validation or session-based authentication checks before invoking `tool.execute(arguments)`.

#### [CRITICAL] Memory Safety Violation / Undefined Behavior on Untrusted WebSocket Input via Unsafe `simd_json::from_str`
* **File:Line**: `crates/op-web/src/websocket.rs:107` and `crates/op-web/src/handlers/websocket.rs:64`
* **Description**: WebSocket message handlers parse incoming string payloads using `unsafe { simd_json::from_str(&mut raw) }`. In `simd_json`, the mutating `from_str` parser is marked `unsafe` because it relies on strict string alignment, padding, and null-termination guarantees. Invoking this on raw, unvalidated network packets supplied directly by remote clients violates Rust's safety guarantees and can trigger memory corruption, undefined behavior, or process crashes.
* **Remediation**: Replace the unsafe parser with safe parsing methods such as `simd_json::from_slice` or standard `serde_json::from_str`.

#### [HIGH] Path Traversal and Arbitrary File Write via Chat Transcripts
* **File:Line**: `crates/op-web/src/handlers/chat.rs:269` (invoking `save_transcript_to_file` at line 341)
* **Description**: The `save_transcript_handler` accepts a client-defined `filename` string parameter and writes the session transcript to `format!("/tmp/{}", filename)`. Because `filename` is not sanitized against directory traversal characters (e.g., `../../etc/cron.d/malicious`), an attacker can overwrite critical system configuration files, leading to remote code execution.
* **Remediation**: Strip path-traversal sequences (e.g., `..`, `/`) from the `filename` parameter, or restrict file writes to a strictly verified sandbox directory.

#### [HIGH] Hardcoded Bypass API Keys
* **File:Line**: `crates/op-web/src/middleware/security.rs:14`
* **Description**: The system configuration contains hardcoded API bypass keys (`4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f` and `test-key-huggingface-2024`). Anyone with access to these keys can bypass all IP-based restriction zones to obtain `TrustedMesh` system access.
* **Remediation**: Remove hardcoded credentials. Manage authentication tokens dynamically using a secure configuration store or cryptographically signed tokens.

#### [HIGH] Unauthenticated State Mutation of Cognitive Agents Selection
* **File:Line**: `crates/op-web/src/handlers/mcp.rs:125`
* **Description**: The endpoint `set_agents_handler` modifies which cognitive agents are active and running on the host system. It accepts arbitrary JSON payloads and writes state updates to `/var/lib/op-dbus/cognitive-mcp-agents.json` without verifying the caller's credentials or IP zone.
* **Remediation**: Protect the `/api/mcp/cognitive/agents` route with authentication middleware.

#### [HIGH] Unauthenticated Overwriting of Tool Group Configuration
* **File:Line**: `crates/op-web/src/groups_admin.rs:282`
* **Description**: The `/api/profiles/:name` HTTP POST route lets remote clients save enabled tool groups and presets to the disk path `/var/lib/op-dbus/tool-groups.json`. There are no authentication checks, allowing any remote actor to alter active system toolsets.
* **Remediation**: Restrict access to local administration interfaces or require high-privilege credentials.

#### [MEDIUM] Unsafe Deserialization of Configuration Files and State Payloads
* **File:Line**: `crates/op-web/src/groups_admin.rs:49`, `crates/op-web/src/users.rs:114`, and `crates/op-web/src/state_manager_client.rs:31`
* **Description**: `GroupsConfig::new`, `UserStore::load`, and `query_plugin_state` utilize `unsafe { simd_json::from_str }` to deserialize configurations and state payloads loaded from disk or received via the system D-Bus. If these local files or D-Bus payloads are corrupted or modified by a lower-privileged process on the host, parsing will result in memory unsafety.
* **Remediation**: Use safe parsing variants (`simd_json::from_slice` or `serde_json::from_str`) for disk and IPC deserialization.

---
## ⚠ Citation Warnings
- `crates/op-web/src/state.rs:783`: file has 672 lines
- `crates/op-web/src/state.rs:796`: file has 672 lines
- `crates/op-web/src/routes/mod.rs:281`: file has 248 lines
