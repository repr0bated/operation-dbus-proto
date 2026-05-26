# Production Security and Configuration Audit: op-services

---

### 1. Standard Environment Variable (`std::env::var`) Reads

This section details all instances of `std::env::var` found in the audited code.

| File Path | Line Number | Environment Variable | Default Value | Error Handling Strategy | Status / Security Analysis |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `crates/op-services/src/bin/op-services.rs` | 42 | `OP_SERVICES_GRPC_ADDR` | `"[::]:50053"` | `.unwrap_or_else` provides default fallback. Parsing errors bubble up to `main` via `?`. | **Vulnerable Default Interface**: The default bind is set to `[::]` (all interfaces), exposing an unauthenticated administration port to the local network (see Section 5). |

#### Flagged Env Vars with No Default and No Error Handling
* **None**: The single explicit read of `std::env::var` includes fallback handling using `.unwrap_or_else()`.

---

### 2. Cargo Features and Additivity Analysis

#### Workspace Package Features (`Cargo.toml`)
In the workspace-level `Cargo.toml`, features are defined for the primary namespace package (`op-dbus`):
```toml
[features]
default = ["grpc"]
grpc = []
```

#### Crate-Specific Features (`crates/op-services/Cargo.toml`)
The `op-services` crate does not declare any custom features under a `[features]` section. It relies on the feature configurations of its transitive dependencies:
* `zbus` is configured with the `["tokio"]` feature.
* `sqlx` is configured with `["runtime-tokio", "sqlite"]`.
* `tokio` is configured with `["full", "signal"]`.

#### Additivity Analysis
* Rust Cargo features are **strictly additive**. If another crate in the workspace compiles with a dependency on `op-dbus` or `op-services`, Cargo will compile the union of all active features.
* The presence of `default = ["grpc"]` in the workspace root package means the gRPC server compilation will run by default unless `--no-default-features` is explicitly passed.

---

### 3. Schema-as-Code Violations (Data Contract Discipline)

This codebase fails strict schema-as-code discipline by utilizing unstructured and unversioned data transfers where formal versioned schemas (such as Protocol Buffers or OSCAL) are expected.

#### I. D-Bus IPC Serialized via Unstructured JSON Strings
* **File**: `crates/op-services/src/dbus/interface.rs`
* **Lines**: 35, 48, 61, 74
* **Ad-hoc Serialization**:
  ```rust
  Ok(serde_json::to_string(&status).unwrap_or_default())
  ```
* **Analysis**: Rather than defining versioned D-Bus structures or passing typed variables, methods like `start`, `stop`, `restart`, and `get_status` serialize internal memory structures into ad-hoc JSON strings over the bus. Any changes to the fields of `ServiceStatus` will cause downstream client parsing errors or silent failures without any compile-time contract enforcement.

#### II. Database Serialization of Configurations to JSON Text
* **File**: `crates/op-services/src/store/mod.rs`
* **Lines**: 40–51, 54–66
* **Ad-hoc Storage**:
  ```rust
  let json = serde_json::to_string(service)?;
  // ...
  sqlx::query_as("SELECT definition FROM services WHERE name = ?")
  ```
* **Analysis**: The SQLite store persists the service definition inside a `definition TEXT NOT NULL` column. Storing nested configurations as unstructured JSON blobs introduces a "schema-on-read" vulnerability. If the structure of `ServiceDef` evolves, existing database rows will fail to parse on startup, introducing potential system-wide denial-of-service vectors. These configurations should instead map directly to structured SQLite tables or rely on versioned Proto schemas.

---

### 4. Hardcoded Paths, Ports, and Addresses

| File Path | Line Number | Hardcoded Value | Type | Context & Security Analysis |
| :--- | :--- | :--- | :--- | :--- |
| `crates/op-services/src/bin/op-services.rs` | 27 | `"/var/lib/op-dbus/services.db"` | Path | Embedded SQLite database file path. Restricts deployments on read-only filesystems or environments lacking root access to `/var/lib`. |
| `crates/op-services/src/bin/op-services.rs` | 43 | `"[::]:50053"` | Network Address | Default fallback listen address of the gRPC server. Binds to **all local network interfaces** over IPv6 and IPv4, leaving the server widely exposed. |
| `crates/op-services/src/bin/systemctl.rs` | 19 | `"http://[::1]:50053"` | Network Address | Hardcoded target connection URI for the `systemctl` client. Prevents querying remote managers or custom listener configurations. |
| `crates/op-services/src/bin/systemctl-native.rs` | 18–21 | `"org.opdbus.services"`, `"/org/opdbus/services"`, `"org.opdbus.services.v1.Manager"` | D-Bus Identifier | Well-known name, object path, and interface target on the system bus. |
| `crates/op-services/src/dbus/interface.rs` | 106 | `"/org/opdbus/services"` | D-Bus Path | Registered server-side D-Bus object path. |
| `crates/op-services/src/dbus/interface.rs` | 109 | `"org.opdbus.services"` | D-Bus Name | Registered well-known D-Bus connection name. |
| `crates/op-services/src/manager/dinit_proxy.rs` | 20–24 | `"org.chimera.dinit.Manager"`, `"org.chimera.dinit"`, `"/org/chimera/dinit"` | D-Bus Identifier | Well-known target properties for interacting with the backend `dinit` system. |
| `crates/op-services/src/manager/service_manager.rs` | 188 | `"/etc/dinit.d/{}"` | Path | Sandbox directory for dinit configuration files. Directly formatted with service names without canonicalization validation. |

---

### 5. Security and Quality Findings

#### [CRITICAL] Remote Arbitrary Code Execution (RCE) via Unauthenticated gRPC Interface
* **Location**: `crates/op-services/src/bin/op-services.rs` (Lines 41-52), `crates/op-services/src/grpc/server.rs` (Lines 98-115, 33-47)
* **Exploitability**: **Directly Exploitable (Remote)**
* **Description**:
  1. The system daemon runs as a replacement for systemd/dinit and handles high-privilege operations (typically running as `root` or an administrator).
  2. The gRPC server defaults to binding to `[::]:50053` on all network interfaces:
     ```rust
     Server::builder()
         .add_service(ServiceManagerServer::new(grpc_server))
         .serve(addr)
         .await?;
     ```
  3. No authentication layer, TLS client certificates (mTLS), token verification, or encryption has been configured on the Tonic transport server.
  4. The `create` RPC method in `server.rs` permits any caller to submit a `CreateRequest` containing a custom `ServiceDef`. This configuration specifies a program path (`start_program`), execution arguments, environment variables, and ownership fields.
  5. The `start` RPC method then executes the stored service definition using the local execution backend:
     ```rust
     let mut cmd = TokioCommand::new(&service.exec_start.program);
     cmd.args(&service.exec_start.args);
     let child = cmd.spawn()?;
     ```
  6. **Impact**: Any adjacent network attacker can submit a malicious service registration targeting `/bin/sh` or `/usr/bin/python3` (containing arbitrary reverse shell payloads) and immediately trigger execution under the daemon's host privilege level.
* **Remediation**:
  * Bind the gRPC interface strictly to the local loopback address (`127.0.0.1` / `[::1]`) or utilize a Unix Domain Socket (UDS) with strict filesystem permission sets (`chmod 0600`).
  * Implement mutual TLS (mTLS) with client certificate verification on the Tonic server builder.
  * Integrate an authentication/authorization middleware (e.g. validating local system tokens, JWTs, or polkit credentials) before processing operations inside `GrpcServer`.

#### [HIGH] Arbitrary File Deletion via Directory Traversal in Service Removal
* **Location**: `crates/op-services/src/manager/service_manager.rs` (Lines 188-193)
* **Exploitability**: **Directly Exploitable**
* **Description**:
  When a service is deleted, the manager removes its associated `dinit` config file from `/etc/dinit.d/` by directly formatting the service name into the file path:
  ```rust
  let path = format!("/etc/dinit.d/{}", name);
  if let Err(e) = tokio::fs::remove_file(&path).await {
  ```
  The string representation of `ServiceName` is re-exported from `op-plugins`. If this type fails to sanitize path inputs to reject folder traversal tokens (such as `../`), an authenticated D-Bus or gRPC caller can trigger arbitrary file deletion (e.g., requesting the deletion of service `../../etc/shadow` or `../../boot/vmlinuz`).
* **Remediation**:
  * Enforce strict alphanumeric-only checks on `ServiceName` inputs.
  * Ensure that the resolved path is canonicalized and verify that it remains nested under the target `/etc/dinit.d/` prefix before invoking `remove_file`.

#### [MEDIUM] Process Hijacking / Denial of Service via PID Recycling
* **Location**: `crates/op-services/src/manager/process.rs` (Lines 52-64)
* **Exploitability**: **Indirectly Exploitable (Local)**
* **Description**:
  When a process is stopped, the manager retrieves its tracked PID from a local in-memory lookup table and issues a termination signal:
  ```rust
  if let Some(pid) = procs.remove(name) {
      if let Err(e) = kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
  ```
  In Linux systems, PIDs are finite resources and are recycled. If a managed service crashes or exits and its PID is subsequently reassigned to an unrelated system process, calling `stop` on the service name will send `SIGTERM` to the reassigned PID, resulting in unexpected termination of critical system processes.
* **Remediation**:
  * Track processes using stable file descriptors (e.g., `pidfd` on Linux systems) or verify process identity (such as matching start times from `/proc/<pid>/stat`) before issuing signals.