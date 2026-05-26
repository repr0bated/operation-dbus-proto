# Production Security & Quality Audit: op-services

---

## 1. Unsafe Code Analysis

An exhaustive review of all provided source files was conducted to identify blocks containing the `unsafe` keyword.

* **Total `unsafe` blocks found:** 0

The codebase strictly adheres to safe Rust for all components defined within the provided `crates/op-services` files. Since no `unsafe` blocks exist, there are no missing `// SAFETY:` comment violations to report.

---

## 2. Process Spawning & Command Execution Analysis

### Process Spawning Inventory
* **Total instantiations of `Command::new()` or `TokioCommand::new()`:** 1

### Citation
* **File:** `crates/op-services/src/manager/process.rs:29`
* **Code Context:**
  ```rust
  let mut cmd = TokioCommand::new(&service.exec_start.program);
  cmd.args(&service.exec_start.args);
  ```

### Argument Validation & Control Assessment
The program binary (`service.exec_start.program`) and its arguments (`service.exec_start.args`) are **completely user-controlled**. 
* They are transmitted via the gRPC `CreateRequest` payload containing a `ServiceDef` struct.
* The conversions in `crates/op-services/src/grpc/server.rs:260-264` extract these values as raw strings and convert them into `PathBuf` and `String` vectors without any sanitization, path restriction, or validation.
* This direct execution of unvalidated, user-controlled commands by a daemon (running with elevated system privileges) presents a severe security risk (detailed in Section 5).

---

## 3. Schema-as-Code & Data Contract Discipline

The codebase uses Protocol Buffers (`opdbus.services.v1`) for some of its gRPC interfaces, but exhibits several key violations of the schema-as-code discipline where structured data contracts are degraded into ad-hoc strings, raw JSON, or unstructured DB text fields.

### Ad-hoc JSON over D-Bus Interfaces
Rather than using compiled, versioned D-Bus/GVariant schemas or structured GVariant payloads, the D-Bus interface serializes internal Rust models to raw JSON strings and exposes them as unstructured `String` return values.
* **`crates/op-services/src/dbus/interface.rs:34`**:
  ```rust
  Ok(serde_json::to_string(&status).unwrap_or_default())
  ```
* **`crates/op-services/src/dbus/interface.rs:46`**:
  ```rust
  Ok(serde_json::to_string(&status).unwrap_or_default())
  ```
* **`crates/op-services/src/dbus/interface.rs:58`**:
  ```rust
  Ok(serde_json::to_string(&status).unwrap_or_default())
  ```
* **`crates/op-services/src/dbus/interface.rs:70`**:
  ```rust
  Ok(serde_json::to_string(&status).unwrap_or_default())
  ```

### Unstructured Database Schema Storage
The SQLite database persistence layer stores the entire structured `ServiceDef` as a serialized JSON blob in a generic `TEXT` column rather than using versioned schemas or normalized SQL tables.
* **`crates/op-services/src/store/mod.rs:30-34`**:
  ```rust
  CREATE TABLE IF NOT EXISTS services (
      name TEXT PRIMARY KEY,
      definition TEXT NOT NULL,
      ...
  ```
* **`crates/op-services/src/store/mod.rs:73-82`**:
  ```rust
  let row: Option<(String,)> =
      sqlx::query_as("SELECT definition FROM services WHERE name = ?")
  ```

---

## 4. Hardcoded Network Configurations & Bus Method Exposures

### Hardcoded IPs and Bind Addresses
* **`crates/op-services/src/bin/op-services.rs:41`**:
  ```rust
  .unwrap_or_else(|_| "[::]:50053".to_string())
  ```
  * *Risk:* Binds the service manager gRPC interface to all available network interfaces (`[::]`) by default, exposing a highly privileged system daemon to the local network.
* **`crates/op-services/src/bin/systemctl.rs:21`**:
  ```rust
  let mut client = ServiceManagerClient::connect("http://[::1]:50053").await?;
  ```
  * *Risk:* Hardcoded client connection address (`[::1]`).

### D-Bus Method Exposure
The system daemon registers on the **system bus** (`Connection::system().await?`) under the name `org.opdbus.services` at path `/org/opdbus/services`.
* **File:** `crates/op-services/src/dbus/interface.rs:114-118`
* **Exposed Methods:**
  * `Start(name: &str) -> String`
  * `Stop(name: &str) -> String`
  * `Restart(name: &str) -> String`
  * `GetStatus(name: &str) -> String`
  * `ListServices() -> Vec<String>`

* **Exposure Analysis:** Without an accompanying D-Bus XML configuration policy (typically installed in `/etc/dbus-1/system.d/`), any unprivileged local user or process connected to the D-Bus system bus can invoke these methods. This allows unauthorized control of system-wide service states.

---

## 5. Detailed Security Findings

### CRITICAL: Remote Code Execution & Local Privilege Escalation via Unauthenticated gRPC Server
* **Citations:**
  * `crates/op-services/src/bin/op-services.rs:40-47`
  * `crates/op-services/src/grpc/server.rs:98-118` (gRPC `create` method)
  * `crates/op-services/src/grpc/server.rs:24-38` (gRPC `start` method)
  * `crates/op-services/src/manager/process.rs:29-31` (Process Spawning)

* **Vulnerability Analysis:**
  1. The `op-services` daemon runs with system-level privileges (implied by service management, dinit file installation, and writing to `/etc/dinit.d/`).
  2. The gRPC server binds to all interfaces (`[::]:50053`) by default, without any transport layer security (TLS), token validation, or client authentication.
  3. A remote or local adversary can make a gRPC request to the `create` method with a custom `ServiceDef` payload containing an arbitrary binary and arguments in `ExecConfig` (e.g., `/bin/sh` or a reverse shell payload).
  4. The adversary then calls `start` with the created service name.
  5. The daemon executes the process as the daemon's user (typically `root`) via `TokioCommand::new(&service.exec_start.program).args(&service.exec_start.args).spawn()`.
  
* **Remediation:**
  * Enforce local-only socket binding (e.g., Unix Domain Sockets) or bind strictly to localhost `127.0.0.1` / `::1` if TCP is required.
  * Implement authentication middleware (e.g., Mutual TLS or token-based authorization checks) for all gRPC endpoints.
  * Restrict process execution binaries to a strict whitelist or authenticated service definitions.

---

### HIGH: Local Privilege Escalation via D-Bus System Bus Exposure
* **Citations:**
  * `crates/op-services/src/dbus/interface.rs:114-118`
  * `crates/op-services/src/dbus/interface.rs:27-72`

* **Vulnerability Analysis:**
  1. The `run_dbus_server` function initializes the D-Bus connection using `Connection::system()`.
  2. The D-Bus interface exposes administrative capability methods (`start`, `stop`, `restart`) to the system bus.
  3. If no restrictive D-Bus security policy is deployed to `/etc/dbus-1/system.d/`, any unprivileged user or sandboxed process on the host can connect to the system bus and call these methods.
  
* **Remediation:**
  * Ensure a strict D-Bus security XML configuration file is packaged with `op-services` that denies method calls to all users except `root` or members of a privileged group (e.g., `wheel` or `opdbus`).
  * Verify client credentials inside the method implementation using `zbus`'s caller UID/GID retrieval APIs if supported by the transport.

---

### MEDIUM: Potential Path Traversal on Service Deletion
* **Citation:** `crates/op-services/src/manager/service_manager.rs:165-171`
* **Vulnerability Analysis:**
  The `delete` method formats a file path using the provided `ServiceName`:
  ```rust
  let path = format!("/etc/dinit.d/{}", name);
  if let Err(e) = tokio::fs::remove_file(&path).await {
  ```
  If `ServiceName` validation in `op-plugins` allows directory traversal characters (e.g., `../`), an attacker calling the gRPC `delete` method could delete arbitrary files on the filesystem (e.g., `/etc/dinit.d/../../etc/shadow`).

* **Remediation:**
  * Sanitize and canonicalize `ServiceName` within `op-services` prior to formatting file paths.
  * Ensure path prefix validation occurs by verifying that the resolved path strictly resides inside `/etc/dinit.d/`.

---
## ⚠ Citation Warnings
- `crates/op-services/src/dbus/interface.rs:114`: file has 107 lines
- `crates/op-services/src/dbus/interface.rs:114`: file has 107 lines
