# Production Security and Quality Audit Report: `op-services`

## 1. Executive Summary

This audit evaluates the quality, security posture, and schema discipline of the `op-services` crate, a system-wide service manager intended as a native control plane and `systemd` replacement with a `dinit` backend.

Three major architectural and security deficiencies were identified:
1. **Critical Privilege Containment Failure**: The process execution fallback mechanism completely ignores user and group credentials specified in service definitions, executing all processes as the user running the service manager (typically `root`).
2. **High-Risk Path Traversal**: Unsanitized service name interpolation in file deletion routines poses an arbitrary file-deletion risk.
3. **Schema-as-Code Violations**: Inter-Process Communication (IPC) boundaries use ad-hoc JSON serialization over `zbus` (D-Bus) and raw string storage in SQLite rather than versioned schema contracts.

---

## 2. Storage Backend Inventory

The codebase defines storage mechanics through local SQLite pools. No implementation of the workspace-defined `CozoDB` (Graph) or Sled backends is utilized within `op-services`.

| Backend | Found at File:Line | Role (KV/Graph/Cache/Queue) | Audit Observations |
| :--- | :--- | :--- | :--- |
| **SQLite (SQLx)** | `crates/op-services/src/store/mod.rs:14` | Relational Storage & Audit Log | Uses an embedded SQLite pool to persist service configurations and audit logs. Service configurations are serialized to ad-hoc JSON strings inside text columns. |

---

## 3. Schema-as-Code Analysis

The project purports to use a schema-as-code discipline. While Protocol Buffers are defined and generated for the gRPC server layer (`opdbus.services.v1` via `tonic-build`), there are multiple areas where data contracts are expressed as ad-hoc strings, violating strict schema-as-code principles.

### IPC Ad-Hoc Data Contracts
* **D-Bus Return Payloads (`crates/op-services/src/dbus/interface.rs` lines 27, 39, 51, 63)**: 
  The interface methods return serialized JSON strings:
  ```rust
  async fn start(&self, name: &str) -> zbus::fdo::Result<String> { ... }
  ```
  Instead of utilizing `zvariant` traits to expose strongly-typed structures or dictionaries natively over the D-Bus bus (enabling native introspection and type validation), the daemon relies on ad-hoc JSON strings (`serde_json::to_string(&status).unwrap_or_default()`). This creates a loose string-ly typed contract that bypasses interface-level validation and contract schema validation.

### Database Ad-Hoc Storage
* **SQLite JSON Serialization (`crates/op-services/src/store/mod.rs` lines 50-60, 63-74)**:
  Service definitions are serialized and stored as text fields in the SQLite database (`definition TEXT NOT NULL`) using JSON. 
  ```rust
  let json = serde_json::to_string(service)?;
  ```
  Storing versioned service schema definitions inside unvalidated SQLite text columns prevents schema evolution management (such as default values for new fields, type checks, and relational lookups) and makes database migration operations highly fragile and prone to decoding errors during runtime.

---

## 4. Vulnerability & Quality Findings

### CRITICAL: Privilege Containment Failure / Root Execution Bypass
* **Location**: `crates/op-services/src/manager/process.rs` lines 26–52
* **Impact**: Local Privilege Escalation / Arbitrary Code Execution as Root
* **Vulnerability Description**:
  The direct process management fallback (`ProcessManager::start`) spawns child processes using `tokio::process::Command`. 
  ```rust
  pub async fn start(&self, service: &ServiceDef) -> anyhow::Result<u32> {
      let mut cmd = TokioCommand::new(&service.exec_start.program);
      cmd.args(&service.exec_start.args);
      cmd.stdin(Stdio::null());
      cmd.stdout(Stdio::null());
      cmd.stderr(Stdio::null());

      if let Some(ref dir) = service.working_dir {
          cmd.current_dir(dir);
      }

      for (k, v) in &service.environment {
          cmd.env(k, v);
      }

      let child = cmd.spawn()?;
  ```
  Although the incoming `ServiceDef` contains parsed user and group context fields (`user` and `group` extracted at `crates/op-services/src/grpc/server.rs:252`), `ProcessManager::start` completely fails to apply privilege-dropping operations (such as calling `pre_exec` to invoke `setuid`/`setgid` or configuring the UID/GID on the command builder). 
  
  Because the `op-services` daemon runs as `root` (as is necessary to perform system-wide service initialization and D-Bus system bus registration), any process spawned via this process manager fallback will execute with full `root` privileges. Any local unprivileged attacker who is permitted to register a service can specify `user = "nobody"` in their request, yet have their registered binary execute with unrestricted root authority on the host system. This is a directly exploitable local privilege escalation vector.

* **Remediation**:
  Use `std::os::unix::process::CommandExt` within the command builder to safely set the UID and GID to target unprivileged users before spawning:
  ```rust
  #[cfg(unix)]
  {
      use std::os::unix::process::CommandExt;
      // Resolve UID/GID from service.user/service.group and inject:
      cmd.uid(target_uid).gid(target_gid);
  }
  ```

---

### HIGH: Arbitrary File Deletion via Path Traversal in Service Removal
* **Location**: `crates/op-services/src/manager/service_manager.rs` lines 144–145
* **Impact**: Denial of Service (DoS) / System Corruption
* **Vulnerability Description**:
  In `ServiceManager::delete`, the path to the configuration file to be removed is constructed by direct, unvalidated string interpolation of the service's name:
  ```rust
  // Remove the dinit service file if it exists
  let path = format!("/etc/dinit.d/{}", name);
  if let Err(e) = tokio::fs::remove_file(&path).await {
  ```
  While `ServiceName` is instantiated from raw string inputs via `ServiceName::new(name)`, its exact validation logic resides inside the opaque `op-plugins` dependency. If `ServiceName::new` does not perform strict sanitization against path traversal sequences (such as `../`), an attacker calling `Delete` with a name like `../../etc/shadow` or `../../boot/vmlinuz` will cause the system daemon (running as root) to delete critical system files, resulting in severe denial of service or compromise of system integrity.

* **Remediation**:
  Before constructing the path, explicitly sanitize the name to ensure it contains no directory separators (`/` or `\`) and cannot resolve to a parent directory:
  ```rust
  let path_buf = std::path::Path::new(&name.to_string());
  if path_buf.components().count() > 1 || name.to_string().contains('/') {
      return Err(anyhow::anyhow!("Invalid service name: contains path traversal characters"));
  }
  ```

---

### MEDIUM: Missing IPC Error Propagation / Silent Serialization Failures
* **Location**: `crates/op-services/src/dbus/interface.rs` lines 32, 44, 56, 68
* **Impact**: IPC Desynchronization / Silent Failures
* **Vulnerability Description**:
  Inside the D-Bus interface methods, the return status is converted to JSON strings using `serde_json::to_string(&status).unwrap_or_default()`. If serialization fails (for example, due to memory pressure or unexpected data types), the method silently returns an empty string `""` to the caller instead of raising a proper IPC-level error. This behavior hides serialization bugs and forces client applications to either panic or misinterpret the empty payload as a successful operation.
* **Remediation**:
  Propagate serialization errors to the D-Bus context:
  ```rust
  let serialized = serde_json::to_string(&status)
      .map_err(|e| zbus::fdo::Error::Failed(format!("Serialization failed: {}", e)))?;
  Ok(serialized)
  ```

---

### MEDIUM: Hardcoded Database Path
* **Location**: `crates/op-services/src/bin/op-services.rs` line 26
* **Impact**: Privilege Escalation / Local Denial of Service
* **Vulnerability Description**:
  The service daemon hardcodes the SQLite database path:
  ```rust
  let store = Arc::new(Store::new("/var/lib/op-dbus/services.db").await?);
  ```
  If `/var/lib/op-dbus` does not exist or has incorrect directory permissions (e.g., if it is writable by non-root users), an unprivileged actor can construct a symlink at `/var/lib/op-dbus/services.db` pointing to arbitrary system files. When `op-services` starts up as root, it will perform SQL migrations and write operations on the target file, leading to file corruption or arbitrary writes to host files.
* **Remediation**:
  Ensure the parent directory `/var/lib/op-dbus` is created with strict root-only permissions (`0700` or `0755`) prior to database initialization, and make the directory path configurable via command-line arguments or environmental variables rather than a hardcoded string.

---

### LOW: Silent and Unsupervised Fallback to Non-isolated Process Manager
* **Location**: `crates/op-services/src/manager/service_manager.rs` lines 30–40
* **Impact**: Bypassed Security & Supervision Policies
* **Vulnerability Description**:
  During startup, if `DinitProxy::new()` fails (e.g., if the `dinit-dbus` interface is not loaded or has crashed), the daemon prints a warning and silently falls back to direct process management via `ProcessManager`:
  ```rust
  let dinit = match DinitProxy::new().await {
      Ok(d) => {
          info!("Connected to dinit-dbus");
          Some(d)
      }
      Err(e) => {
          warn!("dinit-dbus unavailable, using fallback: {}", e);
          None
      }
  };
  ```
  This silent fallback leads to massive architectural and security shifts without administrative consent. In fallback mode, none of the `dinit` security policies, system-level process isolation, chain loading, or dependency trees are enforced; processes are instead spawned as direct child processes of the daemon. This leaves the system in a highly inconsistent and non-deterministic security posture.
* **Remediation**:
  Require explicit configuration authorization (such as an `--allow-fallback` command-line switch or environment variable) to run in process fallback mode, rather than failing open by default.