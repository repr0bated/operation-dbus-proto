### D-Bus & IPC Attack Surface Registry

#### Exposed Services and Interfaces
The following D-Bus interface is registered and exposed as a service by the codebase:

*   **D-Bus Interface**: `org.dbusmcp.Agent`
    *   **Registered in**: `crates/op-tools/src/builtin/agent_tool.rs:280`
    *   **D-Bus Service Name**: `org.dbusmcp.Agent.{PascalCaseAgentName}` (dynamically registered on lines 132–153)
    *   **D-Bus Object Path**: `/org/dbusmcp/Agent/{PascalCaseAgentName}`
    *   **Service Bus Connection**: Connects to either the **System Bus** or the **Session Bus** depending on the environment. Line 218 queries the `OP_AGENT_BUS` environment variable and falls back to checking `DBUS_SESSION_BUS_ADDRESS`. If neither suggests a session bus, it defaults to the system bus.

##### Interface Methods:
1.  **`name(&self) -> &str`** (line 282)
    *   **Caller Identity Checked?**: No.
    *   **Mutates State / Spawns Processes?**: No.
2.  **`description(&self) -> &str`** (line 286)
    *   **Caller Identity Checked?**: No.
    *   **Mutates State / Spawns Processes?**: No.
3.  **`operations(&self) -> Vec<String>`** (line 290)
    *   **Caller Identity Checked?**: No.
    *   **Mutates State / Spawns Processes?**: No.
4.  **`execute(&self, task_json: &str) -> String`** (line 294)
    *   **Caller Identity Checked?**: No.
    *   **Mutates State / Spawns Processes?**: The method itself contains a mock placeholder implementation. However, the architecture is designed to delegate this to actual system agent logic (which executes shell commands, writes files, and modifies systemd services). Exposing an execution entry point on D-Bus with zero caller credentials or policy checks poses a high risk of local privilege escalation.
    *   **Deserialises Caller-Supplied Bytes Without Validation?**: Yes. The caller-provided `task_json: &str` is parsed directly via `unsafe { simd_json::from_str }` on line 298.

---

### Audit Findings

#### [CRITICAL] Memory Corruption via Unpadded Unsafe SIMD JSON Deserialisation
*   **File/Line**: `crates/op-tools/src/builtin/agent_tool.rs:298`
*   **Vulnerability Type**: Memory Unsafety / Out-of-Bounds Read & Write
*   **Description**: 
    The `execute` D-Bus method accepts a `task_json: &str` from an untrusted IPC caller and converts it to a standard `String` (`task_json_mut`). It then parses it using:
    ```rust
    let task: Value = match unsafe { simd_json::from_str(&mut task_json_mut) }
    ```
    `simd_json` utilizes architecture-specific SIMD instructions (e.g., AVX2, SSE) to parse JSON in chunks. To avoid segmentation faults, `simd_json` strictly requires that its input buffer has a padding of `simd_json::SIMDJSON_PADDING` bytes (usually 32 or 64 bytes) at the end. Standard Rust `String` allocations do not guarantee this padding. When the parser scans the mutable string, it may read and write past the end of the allocated buffer, leading to memory corruption, daemon crashes, or arbitrary code execution by a local unprivileged user on the D-Bus.

#### [HIGH] Missing Peer Authentication on Exposed `org.dbusmcp.Agent` Service
*   **File/Line**: `crates/op-tools/src/builtin/agent_tool.rs:280`
*   **Vulnerability Type**: Missing Access Control / Authentication Bypass
*   **Description**:
    The system registers the `org.dbusmcp.Agent` interface on the D-Bus system or session bus without checking the caller's credentials. There is no verification of the peer connection's UID/GID (e.g., using `zbus::Connection::peer_credentials` or validating the incoming message sender). Since the system bus is accessible by all local users on a Linux system, any unprivileged local user can call the `execute` method to command the agent to perform actions, resulting in a complete bypass of the `SecurityValidator` controls defined in `crates/op-tools/src/security.rs`.

#### [HIGH] Dynamic D-Bus Tool Projections Default to ReadOnly Security Level
*   **File/Line**: `crates/op-tools/src/dynamic_tool.rs:90` and `crates/op-tools/src/builtin/dbus_hybrid.rs:114`
*   **Vulnerability Type**: Privilege Escalation / Insufficient Authorization Mapping
*   **Description**:
    The dynamically projected D-Bus tools (`DynamicDbusTool` and `DbusMethodTool`) do not override the `security_level()` method of the `Tool` trait. Consequently, they default to `SecurityLevel::ReadOnly`.
    
    However, the system registers mutating system D-Bus commands such as `StartUnit`, `StopUnit`, and `RestartUnit` on `org.freedesktop.systemd1.Manager` (see `crates/op-tools/src/builtin/dbus_hybrid.rs:188` and surrounding lines) under these exact tool types. Because these critical system modifications are classified as `ReadOnly`, they bypass the system's elevated authorization checks and approvals.

#### [LOW] Missing D-Bus System Bus Policy configuration for Audit
*   **File/Line**: N/A (Project Configuration)
*   **Vulnerability Type**: Security Configuration Gap
*   **Description**:
    No D-Bus system bus configuration policy (`/usr/share/dbus-1/system.d/*.conf`) was provided for audit. When registering services on the system bus (such as `org.dbusmcp.Agent`), D-Bus defaults to blocking service name acquisition and method calls unless an explicit system configuration policy allows them. Without an audited policy, it is impossible to verify whether the system permits over-permissioned wildcard `allow` rules for local users.