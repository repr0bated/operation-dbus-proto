# Production Security, Quality, and License Audit

## 1. License Compliance & Inventory

### License Extraction
* **Workspace Crate (`op-dbus`)**: `Apache-2.0` (specified in `Cargo.toml:43`)
* **Subject Crate (`op-services`)**: No license field is defined in `crates/op-services/Cargo.toml`. It does not inherit the workspace license via `license.workspace = true`.

### Crates with No License Field
* `op-services` (`crates/op-services/Cargo.toml`)

### GPL/AGPL/SSPL Crate Scan
* No GPL, AGPL, or SSPL licensed crates were detected in the `Cargo.lock` dependency tree.
* **Note on Weak Copyleft**: The workspace uses `cozo` version `0.7.6` (specified in `Cargo.lock:431`), which is licensed under the **Mozilla Public License 2.0 (MPL-2.0)**. MPL-2.0 is a weak copyleft license. While it is generally compatible with Apache-2.0, any modifications to `cozo` source files themselves must be made available under the MPL-2.0.

---

## 2. Schema-as-Code Compliance Audit

The codebase violates the schema-as-code discipline by passing unstructured, ad-hoc JSON strings over IPC interfaces rather than utilizing versioned schemas or strongly typed interfaces.

### Ad-hoc JSON Serialization Over D-Bus
* **Citations**: 
  * `crates/op-services/src/dbus/interface.rs:32`
  * `crates/op-services/src/dbus/interface.rs:45`
  * `crates/op-services/src/dbus/interface.rs:58`
  * `crates/op-services/src/dbus/interface.rs:71`
* **Description**: The D-Bus interface methods `start`, `stop`, `restart`, and `get_status` serialize internal structures to unstructured JSON strings via `serde_json::to_string(&status)` rather than defining versioned Protocol Buffer schemas or structured D-Bus signatures. This makes the interface fragile, difficult to version, and susceptible to parsing errors if contracts change.

---

## 3. Security & Quality Findings

### Finding 1: Unauthenticated Remote Code Execution & Privilege Escalation (gRPC)
* **Severity**: Critical (Directly Exploitable)
* **Citations**: 
  * `crates/op-services/src/bin/op-services.rs:43`
  * `crates/op-services/src/grpc/server.rs:140`
* **Description**: 
  The `op-services` system daemon runs with high privileges (managing system services/dinit) and binds its gRPC server to all interfaces (`[::]:50053`) by default. The gRPC server implements no TLS, no authentication, and no authorization interceptors. 
  
  An unauthenticated remote or local attacker can invoke the `create` RPC to register a new service with an arbitrary `exec_start.program` and `exec_start.args`, and then invoke `start` to execute arbitrary commands as the root user.

---

### Finding 2: Denial of Service / Self-Termination via `kill(0, SIGTERM)`
* **Severity**: Critical (Directly Exploitable)
* **Citations**: 
  * `crates/op-services/src/manager/process.rs:37`
  * `crates/op-services/src/manager/process.rs:56`
* **Description**: 
  In the fallback `ProcessManager`, if `child.id()` fails or returns `None` during process spawning, the PID is set to `0`. 
  
  When `stop` is subsequently called on that service, it executes `kill(Pid::from_raw(0), Signal::SIGTERM)`. In Unix systems, sending a signal to PID `0` sends it to every process in the process group of the calling process. Because `op-services` runs as a system daemon, calling `stop` on a service with PID `0` will terminate the daemon itself and all sibling services in its process group, causing an immediate system-wide crash.

---

### Finding 3: PID Recycling Race Condition (TOCTOU)
* **Severity**: High
* **Citation**: `crates/op-services/src/manager/process.rs:56`
* **Description**: 
  The fallback `ProcessManager` tracks active services using a simple numeric mapping `HashMap<ServiceName, u32>`. 
  
  When stopping a service, it sends `SIGTERM` to the cached PID. If the child process has exited and the operating system has recycled its PID for an unrelated system process, `op-services` will send `SIGTERM` to the recycled PID. Because the daemon runs with root privileges, this can terminate critical system services.

---

### Finding 4: Path Traversal via Unsanitized Service Deletion
* **Severity**: High
* **Citations**: 
  * `crates/op-services/src/manager/service_manager.rs:161`
  * `crates/op-services/src/grpc/server.rs:170`
* **Description**: 
  When deleting a service, the manager constructs the path to remove as `format!("/etc/dinit.d/{}", name)`. The `name` parameter is retrieved directly from the gRPC/D-Bus payload. 
  
  If the `ServiceName` validation (implemented externally) does not strictly prohibit directory traversal sequences (e.g., `../../`), an attacker can cause the daemon to delete arbitrary files on the system (for example, `../../etc/shadow`) via `tokio::fs::remove_file`.

---

### Finding 5: Silent Stream Termination on Lagged Subscribers
* **Severity**: Medium
* **Citation**: `crates/op-services/src/grpc/server.rs:233`
* **Description**: 
  The `watch_status` gRPC implementation spawns a task to forward events from a tokio broadcast channel to the client. 
  
  If the client lags behind the broadcast rate, `sub.recv().await` will return `Err(RecvError::Lagged)`. Because the forwarding loop terminates on any non-Ok result (`while let Ok(event) = sub.recv().await`), a lagged subscriber's stream will silently terminate without warning or error reporting.