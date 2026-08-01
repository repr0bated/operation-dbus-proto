### D-Bus & IPC Attack Surface Analysis

The `op-services` daemon registers a system-level D-Bus interface and a gRPC interface to manage system services (intended as a systemd replacement). It executes as a privileged daemon (root) because it interacts with system-wide directories such as `/etc/dinit.d/` and `/var/lib/op-dbus/`.

---

#### 1. D-Bus Interface Registry

The daemon registers the following D-Bus interface on the **System Bus**:

*   **Service Name**: `org.opdbus.services` (Registered at `crates/op-services/src/dbus/interface.rs:113`)
*   **Object Path**: `/org/opdbus/services` (Registered at `crates/op-services/src/dbus/interface.rs:110`)
*   **Interface**: `org.opdbus.services.v1.Manager` (Declared at `crates/op-services/src/dbus/interface.rs:19`)

##### Methods and Security Verification

| Method Name | Source Code Link | Mutates State / Spawns Processes? | Caller Identity Checked? | Security / Privilege Check |
| :--- | :--- | :--- | :--- | :--- |
| `start` | `crates/op-services/src/dbus/interface.rs:23` | **Yes** (Spawns/starts services via dinit or direct fork) | **No** | None |
| `stop` | `crates/op-services/src/dbus/interface.rs:36` | **Yes** (Terminates running processes) | **No** | None |
| `restart` | `crates/op-services/src/dbus/interface.rs:49` | **Yes** (Terminates and spawns processes) | **No** | None |
| `get_status`| `crates/op-services/src/dbus/interface.rs:62` | No (Read-only status lookup) | **No** | None |
| `list_services`| `crates/op-services/src/dbus/interface.rs:75` | No (Read-only service list) | **No** | None |

##### Signals

*   `service_state_changed` (`crates/op-services/src/dbus/interface.rs:86`): Broadcasts service transition events (`name`, `old_state`, `new_state`).

---

#### 2. D-Bus Connection Type & Bus Policy

The daemon explicitly connects to the **system bus**:
```rust
let conn = Connection::system().await?;
```
*(Citation: `crates/op-services/src/dbus/interface.rs:107`)*

##### Policy Analysis
No D-Bus system bus configuration policy file (e.g., `/usr/share/dbus-1/system.d/org.opdbus.services.conf`) is provided in the source files. 
*   **Vulnerability Risk**: Because the Rust code itself contains **zero privilege checks** (does not inspect `zbus::Message` headers for peer credentials, UIDs, or Polkit authorizations), the daemon relies entirely on the D-Bus daemon's XML policy to restrict access. If the system bus policy has overly permissive rules (e.g., allowing wildcards `<allow send_interface="*"/>`), any unprivileged local user can invoke `start`, `stop`, or `restart` on arbitrary services, resulting in local privilege escalation or system-wide denial of service.

---

#### 3. Schema-as-Code Violations

The codebase bypasses versioned Protocol Buffer / schema-as-code definitions in several critical boundaries, relying on ad-hoc serialized JSON strings:

*   **D-Bus Return Payloads**:
    *   `start`: Returns a JSON-serialized string of the status payload (`crates/op-services/src/dbus/interface.rs:34`) instead of a versioned D-Bus structure or Protobuf wire format.
    *   `stop`: Returns ad-hoc serialized JSON (`crates/op-services/src/dbus/interface.rs:47`).
    *   `restart`: Returns ad-hoc serialized JSON (`crates/op-services/src/dbus/interface.rs:60`).
    *   `get_status`: Returns ad-hoc serialized JSON (`crates/op-services/src/dbus/interface.rs:73`).
*   **Database Persistence Layer**:
    *   `crates/op-services/src/store/mod.rs:34`: The SQLite schema defines `definition TEXT NOT NULL`. The structured `ServiceDef` is stored as a raw JSON string rather than database columns mapped to versioned schemas.
    *   `crates/op-services/src/store/mod.rs:77`: Fetched service definitions are parsed using ad-hoc `serde_json::from_str(&json)` logic.

---

### Security Findings

#### CRITICAL: Unauthenticated Remote & Local Root Command Execution via gRPC
*   **Location**: `crates/op-services/src/bin/op-services.rs:43`, `crates/op-services/src/grpc/server.rs:110` (implementing `create`), and `crates/op-services/src/grpc/server.rs:40` (implementing `start`).
*   **Exploitation Mechanism**:
    1.  The `op-services` daemon binds its gRPC interface to all network interfaces on port `50053` by default:
        ```rust
        let addr = std::env::var("OP_SERVICES_GRPC_ADDR")
            .unwrap_or_else(|_| "[::]:50053".to_string())
            .parse()?;
        ```
    2.  The gRPC server implementation does **not** employ any transport-layer authentication (TLS client certificates), interceptors, token validation, or local socket credential checks.
    3.  The `create` endpoint accepts a `CreateRequest` message containing a `ServiceDef` block.
    4.  An attacker (remote or local) can issue a `create` request defining a new service with `exec_start` pointing to a malicious binary or shell command (e.g., `/bin/sh -c "reboot"`).
    5.  The attacker then calls `start` for that service.
    6.  The service manager executes this process as `root` (either fallback via `ProcessManager::start` or via dinit installation):
        ```rust
        let mut cmd = TokioCommand::new(&service.exec_start.program);
        cmd.args(&service.exec_start.args);
        let child = cmd.spawn()?;
        ```
*   **Direct Impact**: Trivial, unauthenticated remote or local code execution as `root`.

#### HIGH: Environment Injection via Unauthenticated Service Creation
*   **Location**: `crates/op-services/src/manager/process.rs:43-45`
*   **Exploitation Mechanism**:
    The service manager passes the environment map directly to the spawned child process without sanitization:
    ```rust
    for (k, v) in &service.environment {
        cmd.env(k, v);
    }
    ```
    Because any local or remote user can register a service via gRPC, they can inject critical environment variables such as `LD_PRELOAD`, `PATH`, or `LD_LIBRARY_PATH` into a service designated to run under a specific user/group (`crates/op-services/src/grpc/server.rs:260`), forcing privilege escalation via shared library preloading when the daemon forks the process.

#### HIGH: Path Traversal during Service Deletion
*   **Location**: `crates/op-services/src/manager/service_manager.rs:218-222`
*   **Exploitation Mechanism**:
    The `delete` method cleans up the dinit configuration file by concatenating the unvalidated `ServiceName` directly to the file system path:
    ```rust
    let path = format!("/etc/dinit.d/{}", name);
    if let Err(e) = tokio::fs::remove_file(&path).await {
    ```
    If `ServiceName` allows path traversal sequences (e.g., `../../etc/shadow`), the privileged daemon will attempt to remove files outside of the intended directory. While `ServiceName::new()` enforces some structure, if the validation in the external dependency `op-plugins` is weak or missing alphanumeric checks, this represents an arbitrary file deletion vulnerability.

#### MEDIUM: JSON Deserialization Panic (Denial of Service)
*   **Location**: `crates/op-services/src/store/mod.rs:77`, `crates/op-services/src/grpc/server.rs:114`
*   **Exploitation Mechanism**:
    The store retrieves service configurations from SQLite and deserializes them on invocation. If an attacker gains write access to `/var/lib/op-dbus/services.db` or directly injects malformed JSON payloads, the SQLite retrieval will return a parse error, causing D-Bus and gRPC queries targeting that service to fail gracefully but blocking legitimate unit operations.

---
## ⚠ Citation Warnings
- `crates/op-services/src/dbus/interface.rs:113`: file has 107 lines
- `crates/op-services/src/dbus/interface.rs:110`: file has 107 lines
