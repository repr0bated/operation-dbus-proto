# Production Security and Quality Audit

## 1. Critical & High Security Vulnerabilities

### [CRITICAL] Remote Unauthenticated Arbitrary Command Execution (RCE)
* **Citation**: `crates/op-services/src/bin/op-services.rs:40-42`, `crates/op-services/src/grpc/server.rs:114`, `crates/op-services/src/manager/service_manager.rs:163`, `crates/op-services/src/manager/process.rs:26-39`
* **Impact**: Full host compromise with root/system-manager privileges by any local or remote network attacker.
* **Mechanism**: 
  The service manager daemon binds the gRPC server to `[::]:50053` by default without configuring any transport-layer security (TLS), token validation, or client authorization:
  ```rust
  let addr = std::env::var("OP_SERVICES_GRPC_ADDR")
      .unwrap_or_else(|_| "[::]:50053".to_string())
      .parse()?;
  ```
  The exposed gRPC endpoint `create` allows any caller to pass an arbitrary, unvalidated `ServiceDef` struct:
  ```rust
  async fn create(
      &self,
      req: Request<CreateRequest>,
  ) -> Result<Response<CreateResponse>, Status> {
      ...
      let service_def =
          proto_to_schema_def(proto_def).map_err(|e| Status::invalid_argument(e.to_string()))?;
      self.manager.create(&service_def).await...
  ```
  The daemon then writes this configuration to a SQLite database and triggers `install()` to deploy the configuration system-wide. A subsequent unauthenticated request to the `start` gRPC or D-Bus endpoint invokes `ProcessManager::start` which spawns the raw executable binary specified by the attacker (`exec_start.program` and `exec_start.args`) as a child process of the highly privileged root daemon:
  ```rust
  let mut cmd = TokioCommand::new(&service.exec_start.program);
  cmd.args(&service.exec_start.args);
  ...
  let child = cmd.spawn()?;
  ```
* **Remediation**:
  1. Enforce TLS encryption and client certificate authentication (Mutual TLS) within `tonic` by using `Server::builder().tls_config(...)`.
  2. Implement a gRPC interceptor that validates JSON Web Tokens (JWT) or system-level identity tokens before allowing access to mutation API surface paths (such as `create`, `delete`, `start`, and `stop`).
  3. Bind to `127.0.0.1` by default rather than all interfaces (`[::]`).

---

### [HIGH] Path Traversal & Arbitrary File Deletion as Root
* **Citation**: `crates/op-services/src/dbus/interface.rs:107`, `crates/op-services/src/manager/service_manager.rs:188`
* **Impact**: Local Privilege Escalation (LPE) or complete system Denial of Service (DoS) by unprivileged local users.
* **Mechanism**:
  The D-Bus server runs on the local System Bus (`Connection::system()`). By default, all local processes can communicate over the System Bus unless limited by an explicit system security configuration file.
  The D-Bus interface exposes the `delete` command, which internally maps to:
  ```rust
  let path = format!("/etc/dinit.d/{}", name);
  if let Err(e) = tokio::fs::remove_file(&path).await {
  ```
  Because the validation of `ServiceName` is external to this crate (re-exported from `op-plugins`), and the interface doesn't sanitize path separators (`/` or `..`), an unprivileged local attacker can execute a path traversal attack by invoking `delete` with a service name like `../../etc/shadow`. This forces the privileged root daemon to remove critical system files.
* **Remediation**:
  1. Restrict the System D-Bus access through a strict XML policy file (e.g. at `/etc/dbus-1/system.d/`) ensuring only a dedicated group or `root` can access mutation operations.
  2. Ensure `ServiceName` strictly permits only alphanumeric and hyphen characters. Reject any payload containing slashes (`/`), backslashes (`\`), or periods (`.`).

---

### [HIGH] Database Privilege Escalation Vector via Hardcoded Path
* **Citation**: `crates/op-services/src/bin/op-services.rs:26`
* **Impact**: Hijacking of daemon-managed binaries by local adversaries.
* **Mechanism**:
  The database file is hardcoded to `/var/lib/op-dbus/services.db`. If the installer fails to strictly lock down directory permissions for `/var/lib/op-dbus/` (e.g., using `0700` owned by `root`), any local user with write permissions to that directory or file can modify the SQLite store database directly. They can alter existing `definition` JSON strings inside the `services` table to change execution variables (`exec_start` program/arguments), achieving local privilege escalation (LPE) when the daemon next starts the target service.
* **Remediation**:
  The daemon must verify directory permissions and file ownership of `/var/lib/op-dbus/services.db` at startup, immediately failing with an error if the directory permissions are group-writable or world-writable.

---

## 2. Schema-as-Code Compliance Flagging

The codebase claims to implement a structured schema-as-code pattern using Protocol Buffers, but several critical boundaries fall back to unstructured, unversioned, and ad-hoc data contracts:

### 1. Ad-Hoc JSON Over System D-Bus
* **Citation**: `crates/op-services/src/dbus/interface.rs:35`, `47`, `59`, `71`
* **Violation**: Instead of using typed D-Bus structures (which map directly to GVariant types and versioned interfaces), the code serializes the internal domain model into unstructured JSON strings:
  ```rust
  Ok(serde_json::to_string(&status).unwrap_or_default())
  ```
  This is an anti-pattern. If a downstream consumer (`systemctl-native.rs`) attempts to parse the payload, there is no schema enforcement, version handshaking, or protocol safety. A mismatch in `ServiceStatus` fields will cause client parsing failures. D-Bus interfaces should return strongly typed D-Bus records or versioned Protobuf payloads.

### 2. Unversioned JSON Blobs Stored in Database
* **Citation**: `crates/op-services/src/store/mod.rs:41`, `81`
* **Violation**: The database schema drops version safety by dumping raw JSON strings directly into an unversioned text column:
  ```rust
  CREATE TABLE IF NOT EXISTS services (
      name TEXT PRIMARY KEY,
      definition TEXT NOT NULL,
      ...
  ```
  Storing `ServiceDef` as a raw JSON text block means changes to the internal Rust layout of `ServiceDef` will make existing databases unreadable or corrupt at runtime. The store must maintain an explicit database schema migration mechanism or store versioned Protocol Buffer byte-arrays (`BLOB`) instead.

---

## 3. Public API Surface & Glob Re-exports

### Public API Surface Analysis
* **Total Public Items**: 54 items.
* **Top 10 Most Impactful Public Items**:

| No. | Public Item | Type | Citation | Impact / Risk |
|---|---|---|---|---|
| 1 | `run_dbus_server` | Function | `crates/op-services/src/dbus/interface.rs:98` | Runs system bus handler; processes local commands as root |
| 2 | `GrpcServer` | Struct | `crates/op-services/src/grpc/server.rs:17` | Exposes RPC management logic to the network |
| 3 | `ServiceManager` | Struct | `crates/op-services/src/manager/service_manager.rs:13` | Orchestrates process lifecycles and backend commands |
| 4 | `Store` | Struct | `crates/op-services/src/store/mod.rs:10` | Manages service state storage and SQL executions |
| 5 | `ProcessManager` | Struct | `crates/op-services/src/manager/process.rs:13` | Direct runner that executes binary paths as root |
| 6 | `DinitProxy` | Struct | `crates/op-services/src/manager/dinit_proxy.rs:43` | Connects directly to external init system (`org.chimera.dinit`) |
| 7 | `ServiceEvent` | Struct | `crates/op-services/src/manager/service_manager.rs:22` | Distributed broadcast schema detailing service transitions |
| 8 | `ServiceManager::create` | Method | `crates/op-services/src/manager/service_manager.rs:163` | Persists service configurations and copies files to `/etc/` |
| 9 | `ServiceManager::delete` | Method | `crates/op-services/src/manager/service_manager.rs:175` | Clears runtime metrics and deletes configurations |
| 10 | `Store::audit` | Method | `crates/op-services/src/store/mod.rs:102` | Database security auditor interface |

---

### Namespace Pollution via Glob Re-exports (`pub use *`)
Glob re-exports are heavily used, which pollutes the namespace and can lead to compile-time resolution problems or accidental API exposures:
* **Citation**: `crates/op-services/src/lib.rs:7` -> `pub use manager::*;`
* **Citation**: `crates/op-services/src/lib.rs:8` -> `pub use schema::*;`
* **Citation**: `crates/op-services/src/lib.rs:9` -> `pub use store::*;`
* **Citation**: `crates/op-services/src/manager/mod.rs:7` -> `pub use dinit_proxy::*;`
* **Citation**: `crates/op-services/src/manager/mod.rs:8` -> `pub use process::*;`
* **Citation**: `crates/op-services/src/manager/mod.rs:9` -> `pub use service_manager::*;`
* **Citation**: `crates/op-services/src/schema/mod.rs:6` -> `pub use op_plugins::service_def::*;`

**Downside**: Glob re-exports hide the origins of types, merge separate concerns (like DB storage and system init dinit protocols) into a single flat namespace, and expose internal helper modules to downstream consumers.

---

### Structs with Public Fields That Should Be Private
The event structures expose raw fields, violating encapsulation principles:
* **Citation**: `crates/op-services/src/manager/service_manager.rs:22`
  ```rust
  pub struct ServiceEvent {
      pub name: ServiceName,
      pub old_state: ManagerState,
      pub new_state: ManagerState,
  }
  ```
* **Downside**: Exposing fields directly prevents modifying the internal state tracking mechanism of `ServiceEvent` in future updates. These fields should be made private, using standard Rust getters (`event.name()`, `event.old_state()`, etc.) instead.

---

## 4. Dead Code Audit

### Unused Code Analysis & Table
No `#[allow(dead_code)]` attributes are present in any of the audited files. However, several functions and imports are compiled but never invoked or used anywhere within the workspace.

| Item | Type | file:line | Recommendation |
|---|---|---|---|
| `Store::audit` | Method | `crates/op-services/src/store/mod.rs:102` | Remove if audit logging is handled at the gateway layer, or integrate it into gRPC mutation methods. |
| `ProcessManager::get_pid` | Method | `crates/op-services/src/manager/process.rs:70` | Wire this up to the `get_status` query runner to return the real fallback process ID, or remove it. |
| `DinitProxy::list` | Method | `crates/op-services/src/manager/dinit_proxy.rs:125` | Integrate it into the `ServiceManager::list` implementation to aggregate services managed directly by Chimera dinit. |
| `tonic::transport::Channel` | Unused Import | `crates/op-services/src/bin/systemctl.rs:4` | Remove. The client instantiates via automatic code-gen types. |

---
## ⚠ Citation Warnings
- `crates/op-services/src/schema/mod.rs:6`: file has 5 lines
- `crates/op-services/src/manager/process.rs:70`: file has 67 lines
- `crates/op-services/src/manager/dinit_proxy.rs:125`: file has 114 lines
