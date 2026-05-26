# Production Security & Quality Audit: `op-services`

---

## 1. Data Structures & Concurrency Analysis

### Sync / Concurrency and Clone Counts by File

| File Path | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell` | `.clone()` Calls | Large Structs (>5 Pub Fields) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- |
| `crates/op-services/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None |
| `crates/op-services/src/bin/op-services.rs` | 3 | 0 | 0 | 0 | 0 | 0 | 1 | None |
| `crates/op-services/src/bin/systemctl-native.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None |
| `crates/op-services/src/bin/systemctl.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 4 | None |
| `crates/op-services/src/dbus/interface.rs` | 4 | 0 | 0 | 0 | 0 | 0 | 0 | None |
| `crates/op-services/src/dbus/mod.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None |
| `crates/op-services/src/grpc/mod.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None |
| `crates/op-services/src/grpc/server.rs` | 3 | 0 | 0 | 0 | 0 | 0 | 5 | None |
| `crates/op-services/src/manager/dinit_proxy.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None |
| `crates/op-services/src/manager/mod.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None |
| `crates/op-services/src/manager/process.rs` | 0 | 0 | 0 | 1 | 0 | 0 | 1 | None |
| `crates/op-services/src/manager/service_manager.rs` | 4 | 0 | 0 | 2 | 0 | 0 | 5 | None |
| `crates/op-services/src/schema/mod.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None |
| `crates/op-services/src/store/mod.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None |

### Struct & State Flags

* **Large Structs (> 5 Public Fields)**: None in the audited files. (Note: `ServiceDef` from `op-plugins` is not part of the provided source files).
* **Globally Mutable State**: No instances of `static mut` or `lazy_static` exist in the provided source files.

---

## 2. Production Security & Quality Findings

### [CRITICAL] Finding 1: Unauthenticated Remote Code Execution (RCE) via Default gRPC Binding
* **Reference**: `crates/op-services/src/bin/op-services.rs:38-40`, `crates/op-services/src/grpc/server.rs:92-108`
* **Exploitability**: Directly exploitable
* **Description**:
  The system-wide service manager daemon (`op-services`) binds its gRPC API to `[::]:50053` (all interfaces) by default without any authentication, TLS, or client authorization controls. Any client with network access can connect and invoke the `create` RPC to persist arbitrary service definitions, followed by invoking the `start` RPC. Because the daemon runs as `root` (intended as a systemd replacement), this enables unauthenticated remote command execution with full root privileges.

---

### [HIGH] Finding 2: Failure to Drop Privileges in Process Manager Fallback
* **Reference**: `crates/op-services/src/manager/process.rs:24-38`
* **Exploitability**: Directly exploitable
* **Description**:
  The gRPC translation layer correctly parses `user` and `group` fields into the internal `ServiceDef` structure (`crates/op-services/src/grpc/server.rs:211-212`). However, the process manager fallback implementation in `ProcessManager::start` completely ignores these configurations. It spawns target executables via `TokioCommand` without calling POSIX user/group switching APIs. Consequently, every service spawned by this fallback runs under the root privileges of the daemon, violating least-privilege security designs and escalating administrative privileges.

---

### [HIGH] Finding 3: Denial of Service via PID 0 Process Group Signaling
* **Reference**: `crates/op-services/src/manager/process.rs:38-40`, `crates/op-services/src/manager/process.rs:51`
* **Exploitability**: Local DoS
* **Description**:
  If a child process fails to spawn or returns a fallback PID of `0` at `child.id().unwrap_or(0)` (line 39), `0` is inserted into the active processes map. When `stop` is subsequently called on that service, it attempts to kill the process via:
  ```rust
  kill(Pid::from_raw(0), Signal::SIGTERM)
  ```
  In POSIX compliance, calling `kill` with PID `0` signals **every process within the sender's current process group**. This instantly terminates the main `op-services` daemon and all sister processes in its group, resulting in an unrecoverable local denial of service.

---

### [MEDIUM] Finding 4: Schema-as-Code Violation: Ad-hoc JSON Serialization over D-Bus
* **Reference**: `crates/op-services/src/dbus/interface.rs:29`, `crates/op-services/src/dbus/interface.rs:43`, `crates/op-services/src/dbus/interface.rs:57`, `crates/op-services/src/dbus/interface.rs:71`
* **Exploitability**: Non-exploitable (Quality / Contract Defect)
* **Description**:
  In violation of the schema-as-code discipline, the system's native D-Bus interface expresses its data contracts as raw JSON strings rather than strongly-typed, versioned schema definitions. For instance:
  ```rust
  async fn get_status(&self, name: &str) -> zbus::fdo::Result<String> {
      ...
      Ok(serde_json::to_string(&status).unwrap_or_default())
  }
  ```
  Clients must parse these serialized strings ad-hoc (`crates/op-services/src/bin/systemctl-native.rs:52`), bypassing compile-time validation, API versioning controls, and robust serialization safety.

---

### [MEDIUM] Finding 5: Disruptive Configuration Reload Sequence
* **Reference**: `crates/op-services/src/grpc/server.rs:121-137`
* **Exploitability**: Non-exploitable (Operational Defect)
* **Description**:
  The gRPC `reload` service method is implemented as a full stop-and-start lifecycle sequence:
  ```rust
  // Reload by performing a stop + start cycle, since neither dinit proxy
  // nor the process manager exposes a dedicated reload operation.
  let status = self.manager.restart(&name).await...
  ```
  This violates the standard operational expectation of a non-disruptive, zero-downtime configuration reload. Spawning a full stop/start cycle forces downtime and drops active connections on critical system services.

---

### [LOW] Finding 6: SQLite Connection URL Parameter Injection
* **Reference**: `crates/op-services/src/store/mod.rs:17`
* **Exploitability**: Non-exploitable (Quality Defect)
* **Description**:
  The connection string for the SQLite pool is formatted directly as a string from an input path:
  ```rust
  let url = format!("sqlite:{}?mode=rwc", path.as_ref().display());
  ```
  If the target database path contains special characters such as `?`, `&`, or `#`, the formatted string will cause incorrect parsing or parameter injection into the SQLite connection setup. It is recommended to use `SqliteConnectOptions` to specify connection details cleanly.

---

### [LOW] Finding 7: Missing Auditing Execution
* **Reference**: `crates/op-services/src/store/mod.rs:36-47`, `crates/op-services/src/store/mod.rs:87-101`
* **Exploitability**: Non-exploitable (Audit/Defense Defect)
* **Description**:
  Although the database migration properly provisions an `audit_log` table and the `Store` struct exposes an asynchronous `audit` helper method, the method is never actually invoked in any operational pathways of the `ServiceManager` or the gRPC/D-Bus interfaces. Critical operations like service creation, manual stops, restarts, and deletions occur silently without generating any audit records.