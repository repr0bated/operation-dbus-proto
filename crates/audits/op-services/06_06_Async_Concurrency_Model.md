# OP-SERVICES Production Security & Quality Audit

## 1. Executive Summary

This production security and quality audit evaluates the `op-services` crate, a system-wide service manager designed as a systemd alternative using a dinit backend. The codebase has been analyzed for asynchronous safety, concurrency bugs, OS-level resource leaks, API contract integrity (Schema-as-Code discipline), and exploitable security vulnerabilities.

A critical security flaw was identified in the gRPC server binding, which exposes an unauthenticated, unencrypted remote command execution vector to the network. Furthermore, a severe system resource leak was found in the fallback process manager where all spawned processes are detached upon creation and never reaped, permanently spawning zombie processes and exhausting the OS PID space. Ad-hoc JSON serialization bypasses Schema-as-Code guarantees in both SQLite storage and D-Bus interfaces.

---

## 2. Quantitative Async & Concurrency Analysis

The `op-services` codebase heavily relies on asynchronous programming using the Tokio runtime, SQLite (`sqlx` with Tokio driver), and `zbus` (D-Bus). The following metrics represent the async footprint across the crate:

* **`async fn` count:** 50 instances
* **`tokio::spawn` count:** 2 instances
* **`spawn_blocking` count:** 0 instances

---

## 3. Critical Vulnerabilities

### 3.1. Unauthenticated Remote gRPC API Exposes Root Command Execution
* **File:** `crates/op-services/src/bin/op-services.rs`
* **Lines:** 34-44
* **Impact:** Critical (Directly Exploitable)
* **Description:** 
  The gRPC server serves the `ServiceManagerServer` on a TCP socket binding to all network interfaces (`[::]:50053` by default) without configuring Transport Layer Security (TLS), token authentication, or role-based access control. 
  ```rust
  Server::builder()
      .add_service(ServiceManagerServer::new(grpc_server))
      .serve(addr)
      .await?;
  ```
  Because the daemon manages system services (creating dinit files in `/etc/dinit.d` and executing processes via `ProcessManager`), it likely runs as `root`. Any remote attacker who can route TCP packets to port `50053` can invoke the `Create` and `Start` RPC endpoints to register and execute arbitrary payloads with root privileges.
* **Remediation:** 
  1. Bind the gRPC server exclusively to localhost (`127.0.0.1` or `::1`) or a UNIX domain socket unless external access is strictly required.
  2. Implement TLS using `.tls_config()`.
  3. Add a gRPC interceptor that validates authentication tokens (such as mTLS or secure bearer tokens) before executing any service management RPC.

---

## 4. Async & Concurrency Findings

### 4.1. Dropped `Child` Future in Process Manager Generates Zombie Processes
* **File:** `crates/op-services/src/manager/process.rs`
* **Lines:** 35-43
* **Impact:** High
* **Description:** 
  Inside `ProcessManager::start`, a child process is spawned asynchronously using `tokio::process::Command`:
  ```rust
  let child = cmd.spawn()?;
  let pid = child.id().unwrap_or(0);
  ...
  let mut procs = self.processes.write().await;
  procs.insert(service.name.clone(), pid);
  ```
  The `child` instance (which represents the spawned OS process handle) is dropped when it goes out of scope at the end of `start`. When a `tokio::process::Child` is dropped, it detaches the process. However, because no background task or reaper awaits its exit status via `.wait()`, the process will permanently remain a **zombie process** upon termination. Over time, as fallback services exit and restart, the host system's PID space will be entirely exhausted.
* **Remediation:** 
  Do not discard the `Child` future. Instead, spawn an asynchronous monitoring task in the background to reap the process when it exits:
  ```rust
  let mut child = cmd.spawn()?;
  let pid = child.id().unwrap_or(0);
  let name_clone = service.name.clone();
  tokio::spawn(async move {
      let _ = child.wait().await;
      // Handle cleanup, update service state to Stopped/Failed
  });
  ```

### 4.2. Blocking Synchronous Disk I/O Blocks Async Executor
* **File:** `crates/op-services/src/manager/service_manager.rs`
* **Line:** 121
* **Impact:** Medium
* **Description:** 
  The function `pub async fn create` is an asynchronous method running on the Tokio thread pool. However, it invokes `service.install()`, which is a synchronous operation that writes the service file to the disk (`/etc/dinit.d/`):
  ```rust
  self.store.save_service(service).await?;
  if let Err(e) = service.install() { ... } // Synchronous write blocking the executor
  ```
  Calling synchronous file-system I/O within an asynchronous context blocks the current executor thread, preventing other futures scheduled on that thread from executing, leading to high latency and thread starvation.
* **Remediation:** 
  Wrap the synchronous file creation in `tokio::task::spawn_blocking`:
  ```rust
  let service_clone = service.clone();
  tokio::task::spawn_blocking(move || service_clone.install()).await??;
  ```

### 4.3. Silently Dropped D-Bus Daemon `JoinHandle`
* **File:** `crates/op-services/src/bin/op-services.rs`
* **Lines:** 27-33
* **Impact:** Medium
* **Description:** 
  The D-Bus server is spawned in the background via `tokio::spawn`:
  ```rust
  tokio::spawn(async move {
      if let Err(e) = run_dbus_server(dbus_manager).await {
          tracing::error!("D-Bus server error: {}", e);
      }
  });
  ```
  The returned `JoinHandle` is dropped. If `run_dbus_server` fails during initialization (e.g., due to name ownership conflicts, lack of permission on the system bus, or daemon termination), the parent task continues running the gRPC server unaware that its D-Bus control plane has failed.
* **Remediation:** 
  Store the handle and use `tokio::select!` or a supervisor task to exit or restart the daemon if either the gRPC or D-Bus tasks fail.

### 4.4. TOCTOU Concurrency Race in Fallback Process Spawning
* **File:** `crates/op-services/src/manager/service_manager.rs`
* **Lines:** 48-69
* **Impact:** Medium
* **Description:** 
  `ServiceManager::start` blindly executes the spawn operation without verifying if the service's current state is already `Starting` or `Running`. If two clients concurrently call `start` on the same service, or if a single client calls `start` twice in quick succession, both operations will proceed. The fallback process manager will spawn two duplicate instances of the process, overwrite the PID mapping, and permanently lose control of the first process (leaking it in the background).
* **Remediation:** 
  Acquire a lock on the specific service's state, and verify if the state is already `Starting` or `Running` before initiating the process spawn. Return an error or a no-op if the service is already active.

---

## 5. Schema-as-Code Violations

The codebase bypasses the Schema-as-Code discipline by utilizing ad-hoc JSON serialization over IPC boundaries, untyped raw structures, and unversioned serialization in storage.

### 5.1. Unversioned JSON Blobs Stored as TEXT in Database
* **File:** `crates/op-services/src/store/mod.rs`
* **Lines:** 60-75
* **Impact:** Low-Medium
* **Description:** 
  Service definitions are persisted in SQLite by serializing the `ServiceDef` struct directly into a `definition TEXT` column as raw JSON:
  ```rust
  let json = serde_json::to_string(service)?;
  // INSERT OR REPLACE INTO services ... VALUES (?, ?, ?, CURRENT_TIMESTAMP)
  ```
  This ad-hoc serialization bypasses structural schema enforcement. Any update to the `ServiceDef` struct structure (such as field addition, deletion, or type changes) will break database deserialization of previously stored configurations with no schema evolution or version-migration guarantees.
* **Remediation:** 
  Use versioned schemas (such as Protocol Buffers) to store definitions, or store the individual configuration properties in properly typed database columns with structured SQL migrations.

### 5.2. Ad-hoc JSON Serialization Over D-Bus Interfaces
* **File:** `crates/op-services/src/dbus/interface.rs`
* **Lines:** 23-68
* **Impact:** Medium
* **Description:** 
  D-Bus methods like `start`, `stop`, `restart`, and `get_status` return service states as unstructured JSON-encoded strings:
  ```rust
  async fn get_status(&self, name: &str) -> zbus::fdo::Result<String> {
      ...
      Ok(serde_json::to_string(&status).unwrap_or_default())
  }
  ```
  This completely breaks the D-Bus type system and type safety guarantees. Clients (such as `systemctl-native`) are forced to parse raw JSON strings blindly. There is no machine-readable API versioning, making backward-compatibility checks impossible.
* **Remediation:** 
  Use structured D-Bus signatures with strongly-typed properties, or pass standard Protocol Buffer message payloads.

### 5.3. External IPC Contracts Expressed as Ad-hoc Anonymous Tuples
* **File:** `crates/op-services/src/manager/dinit_proxy.rs`
* **Lines:** 11-23
* **Impact:** Low-Medium
* **Description:** 
  The interface definitions for communication with the `dinit` manager are expressed as ad-hoc anonymous Rust tuple aliases:
  ```rust
  type DinitStatusRecord = (String, String, String, String, DinitFlags, u32, i32, i32);
  type DinitServiceRecord = (String, String, String, String, String, DinitFlags, u32, i32, i32);
  ```
  These anonymous structures rely entirely on positional offset correctness. If the upstream `dinit` D-Bus service changes its signature or field order, compilation will not catch the mismatch, resulting in runtime memory/type deserialization errors.
* **Remediation:** 
  Define named, version-tagged structs for these interface contracts, mapping the D-Bus fields explicitly.

---

## 6. Code Quality & Insecure Coding Practices

### 6.1. Ad-hoc Path Formatting Bypasses Boundary Validation
* **File:** `crates/op-services/src/manager/service_manager.rs`
* **Line:** 135
* **Impact:** Medium
* **Description:** 
  When deleting a service, the system manager constructs file-system paths using ad-hoc string formatting:
  ```rust
  let path = format!("/etc/dinit.d/{}", name);
  if let Err(e) = tokio::fs::remove_file(&path).await { ... }
  ```
  If `ServiceName::new` does not perform strict validation to reject directory traversal characters (e.g., `../`), this formatting pattern enables path traversal attacks, allowing an attacker to delete arbitrary files on the host system (e.g., `../../../etc/shadow`).
* **Remediation:** 
  Construct paths securely using path manipulation APIs, ensuring the resulting path is a child of the intended base directory:
  ```rust
  let base_dir = std::path::Path::new("/etc/dinit.d");
  let path = base_dir.join(name.as_str());
  if !path.starts_with(base_dir) {
      return Err(anyhow::anyhow!("Invalid service name: path traversal attempt"));
  }
  ```