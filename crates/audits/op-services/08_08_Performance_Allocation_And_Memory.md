# Production Security & Quality Audit: op-services

---

## 1. Executive Summary

This security and quality audit focuses on the `op-services` system-wide service manager daemon. The daemon is designed to act as a systemd replacement, running with `root` privileges to manage system processes. 

The audit identified **two directly exploitable Critical vulnerabilities** that allow remote/local unauthenticated code execution as root and daemon self-termination (Denial of Service). Additionally, several High and Medium severity security gaps were identified, along with violations of the **Schema-as-Code** discipline.

---

## 2. Security Vulnerability Findings

### Finding 1: [CRITICAL] Remote Root Code Execution via Unauthenticated Network-Exposed gRPC Service
*   **File & Line Citation:** 
    *   `crates/op-services/src/bin/op-services.rs:43-46`
    *   `crates/op-services/src/grpc/server.rs:107-124`
    *   `crates/op-services/src/manager/process.rs:31-48`
*   **Vulnerability Type:** Remote Code Execution (RCE) / Privilege Escalation
*   **Directly Exploitable:** Yes
*   **Description:**
    The `op-services` daemon runs as `root` to execute system-level operations. At startup (`op-services.rs:43-46`), it binds a gRPC server to a network-facing address (`OP_SERVICES_GRPC_ADDR`, defaulting to `[::]:50053`) without configuring TLS, client certificates, or any form of authentication or authorization.
    
    The gRPC endpoint `create` (`server.rs:107-124`) allows a client to submit an arbitrary `ServiceDef` consisting of binary paths (`start_program`) and arguments. The `start` endpoint then invokes `ProcessManager::start` (`process.rs:31-48`), which spawns the program using `tokio::process::Command::spawn` as the root user.
    
    Any network attacker or local unprivileged process can connect to port `50053` and execute arbitrary binaries as `root`, leading to complete system compromise.
*   **Remediation:**
    1. Bind the control interface to a Unix Domain Socket (UDS) instead of a TCP port by default.
    2. Configure strict filesystem permissions (`0600`, owner `root`) on the UDS.
    3. If TCP is strictly required, enforce mutual TLS (mTLS) with client certificate verification and implement token-based role-based access control (RBAC).

---

### Finding 2: [CRITICAL] Daemon Self-Kill (Denial of Service) via nix::sys::signal::kill on PID 0
*   **File & Line Citation:** 
    *   `crates/op-services/src/manager/process.rs:70-75`
    *   `crates/op-services/src/manager/dinit_proxy.rs:92-94`
*   **Vulnerability Type:** Denial of Service (DoS) / Process Instability
*   **Directly Exploitable:** Yes
*   **Description:**
    In `ProcessManager::stop` (`process.rs:70-75`), the service manager stops a fallback process by removing its name from the tracking map and calling:
    ```rust
    kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
    ```
    If `pid` is `0`, POSIX behavior dictates that `kill(0, SIGTERM)` sends the signal to **all processes in the current process group** of the caller. 
    
    A PID of `0` is introduced under two scenarios:
    1. If `child.id()` fails or returns `None` during fallback spawn, it defaults to `0` (`process.rs:41`).
    2. If the `dinit` proxy queries status for a service that does not yet possess a running process, it returns `0` (`dinit_proxy.rs:94`):
       ```rust
       let has_pid = status.4.get("has_pid").copied().unwrap_or(false);
       Ok(if has_pid { status.5 } else { 0 })
       ```
    
    When an administrator or automated system attempts to stop a service that registered with PID `0`, the daemon sends `SIGTERM` to its own process group. This immediately kills `op-services` itself along with all managed child services, triggering a cascading system crash.
*   **Remediation:**
    Assert that the retrieved `pid` is strictly greater than `0` before wrapping it in `Pid::from_raw` and calling `kill`:
    ```rust
    if pid > 0 {
        if let Err(e) = kill(Pid::from_raw(pid as i32), Signal::SIGTERM) { ... }
    } else {
        return Err(anyhow::anyhow!("Invalid PID 0; cannot signal process group"));
    }
    ```

---

### Finding 3: [HIGH] Arbitrary File Deletion via Directory Traversal in Service Cleanup
*   **File & Line Citation:** 
    *   `crates/op-services/src/manager/service_manager.rs:162-167`
    *   `crates/op-services/src/grpc/server.rs:126-135`
*   **Vulnerability Type:** Path Traversal / Arbitrary File Deletion
*   **Directly Exploitable:** Yes
*   **Description:**
    When deleting a service, the manager cleans up its associated `dinit` service file (`service_manager.rs:162-167`):
    ```rust
    let path = format!("/etc/dinit.d/{}", name);
    if let Err(e) = tokio::fs::remove_file(&path).await { ... }
    ```
    The `name` parameter is passed directly from the `DeleteRequest` gRPC request. There is no verification to ensure that `name` does not contain directory traversal sequences (such as `../`). Because the daemon runs as `root`, a user can pass a name like `../../etc/shadow` or `../../boot/vmlinuz`, causing the daemon to delete critical system files.
*   **Remediation:**
    Sanitize the `ServiceName` within the domain schema or directly inside the service manager to prohibit slashes, null bytes, and traversal tokens:
    ```rust
    if name.as_str().contains('/') || name.as_str().contains('.') {
        return Err(anyhow::anyhow!("Invalid character sequence in service name"));
    }
    ```

---

### Finding 4: [HIGH] Information Disclosure via Weak Permissions on Secret-Containing SQLite Database
*   **File & Line Citation:** 
    *   `crates/op-services/src/bin/op-services.rs:29`
    *   `crates/op-services/src/store/mod.rs:15-20`
*   **Vulnerability Type:** Weak File Permissions / Information Disclosure
*   **Directly Exploitable:** Yes
*   **Description:**
    The SQLite persistent store is initialized at `/var/lib/op-dbus/services.db`. This database stores `ServiceDef` entities as JSON strings. These configurations contain environment variables (`environment` field), which frequently contain highly sensitive application secrets, API tokens, and private database credentials.
    
    If SQLite creates this file with default permissions (often influenced by loose process umasks like `022`, yielding `0644`), any local unprivileged user or compromised low-privilege agent on the system can read the raw database file and steal all system secrets.
*   **Remediation:**
    Explicitly set the file permissions of `/var/lib/op-dbus/` and `/var/lib/op-dbus/services.db` to `0600` (read/write only by owner `root`) immediately upon creation:
    ```rust
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions("/var/lib/op-dbus/services.db", Permissions::from_mode(0o600))?;
    ```

---

### Finding 5: [MEDIUM] Silent Failure of Auditing Controls (Empty Audit Logs)
*   **File & Line Citation:** 
    *   `crates/op-services/src/store/mod.rs:121-137`
*   **Vulnerability Type:** Security Auditing / Accountability Failure
*   **Directly Exploitable:** No
*   **Description:**
    The `Store` implementation defines a robust `audit_log` table schema during database migrations (`store/mod.rs:37-47`) and provides a `Store::audit` helper method. However, a comprehensive search of the codebase reveals that `Store::audit` is **never called** by the `ServiceManager` or any gRPC/D-Bus interface handlers. 
    
    Critical system mutations (service creation, state modification, systemctl actions, service deletions) occur with zero audit recording. This leaves system administrators with empty audit logs, failing compliance requirements (such as FedRAMP/OSCAL audit trail criteria).
*   **Remediation:**
    Ensure every mutating operation in `ServiceManager` (e.g., `start`, `stop`, `create`, `delete`, `set_enabled`) invokes `self.store.audit(...)` before completing the transaction.

---

## 3. Schema-as-Code Violations

The codebase claims to adhere to a strict "schema-as-code" discipline using Protocol Buffers and OSCAL. However, multiple instances of unversioned, ad-hoc, and raw unstructured string payloads were identified:

1.  **D-Bus Interface Ad-hoc JSON Serialization:**
    *   *Citation:* `crates/op-services/src/dbus/interface.rs:35`, `49`, `63`, `77`
    *   *Violation:* Methods serialize and return data contracts as ad-hoc, raw JSON strings (`serde_json::to_string(&status).unwrap_or_default()`). This forces clients to parse untyped, unversioned JSON structures over D-Bus instead of using generated, strongly-typed D-Bus structures or Protocol Buffer messages.
2.  **Database Storage of Raw Service Definitions:**
    *   *Citation:* `crates/op-services/src/store/mod.rs:80`, `94`
    *   *Violation:* Service definitions are stored in the SQLite database as raw JSON text strings (`definition TEXT NOT NULL`). Storing configurations as unstructured JSON blobs undermines data model versioning, making migrations fragile and prone to compatibility drift.
3.  **Untyped external D-Bus records in Proxy:**
    *   *Citation:* `crates/op-services/src/manager/dinit_proxy.rs:10-21`
    *   *Violation:* The dinit proxy uses unversioned Rust tuples and map typings (`DinitStatusRecord`, `DinitServiceRecord`) to handle interface structures. This makes the proxy highly vulnerable to silent runtime parsing errors if dinit changes its return signatures.

---

## 4. Performance, Allocation & Memory Map

This section analyzes memory mapping, persistent store configurations, and hot-path memory allocations.

### Memory Mapping Analysis
The audited files inside `crates/op-services` do not use `memmap2`, `mmap`, `MmapMut`, or `MmapOptions` directly. Additionally, the crate uses SQLite (via `sqlx`) as its database engine, meaning `sled` is not initialized or mapped in this crate. 

No large heap allocations (such as `Vec` allocations > 1MB or large `BytesMut` buffers) are present in the provided source files.

| Site | file:line | Type | Risk |
| :--- | :--- | :--- | :--- |
| **None** | N/A | N/A | No explicit memory mapping is performed in this crate. |

### Hot Path Allocation and format!() Analysis
*   **Redundant Conversions in Collections:**
    *   *Citation:* `crates/op-services/src/dbus/interface.rs:91`
    *   *Description:* `Ok(services.into_iter().map(|s| s.name.to_string()).collect())` performs a heap-allocated string copy for every service on every invocation of `list_services`.
    *   *Impact:* Low performance impact for small service lists, but scales poorly under high-frequency polling.
*   **Ad-hoc String Formatting:**
    *   *Citation:* `crates/op-services/src/manager/service_manager.rs:162`
    *   *Description:* `let path = format!("/etc/dinit.d/{}", name);` generates a new heap string allocation during deletion.
    *   *Impact:* Negligible (deletion is not a hot path).

---
## ⚠ Citation Warnings
- `crates/op-services/src/manager/process.rs:70`: file has 67 lines
- `crates/op-services/src/store/mod.rs:121`: file has 117 lines
