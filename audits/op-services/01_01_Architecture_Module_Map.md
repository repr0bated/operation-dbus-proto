# Architecture & Module Map

### Overview
`op-services` is designed as a system-wide service manager (intended as a systemd replacement) utilizing a `dinit` D-Bus backend, with a direct process-spawning fallback system. It exposes two primary communication interfaces:
1. **gRPC API**: A port-based service configuration and control plane.
2. **D-Bus Interface**: A native system-bus control plane operating on `org.opdbus.services`.

The architecture persists system service configurations inside an embedded SQLite database and manages operational status transitions in-memory with event subscription broadcasts.

### Module Tree
```
op-services (lib)
├── dbus
│   └── interface (D-Bus server registration & method handlers)
├── grpc
│   └── server (gRPC service manager server implementation)
├── manager
│   ├── dinit_proxy (Chimera dinit D-Bus integration)
│   ├── process (Direct POSIX process spawning fallback)
│   └── service_manager (Coordinator orchestrating DB/D-Bus/process state)
├── schema (Integration wrapper re-exporting op-plugins types)
└── store (SQLite storage layer for service registration & auditing)
```

### Entry Points & Binaries
*   **Library Entry Point**: `crates/op-services/src/lib.rs`
*   **Daemon Binary**: `crates/op-services/src/bin/op-services.rs` — Starts the core daemon, registers SQLite database connection pool, launches the background D-Bus listener, and binds the unauthenticated gRPC listener.
*   **systemctl Binary**: `crates/op-services/src/bin/systemctl.rs` — Compatibility wrapper acting as a gRPC client to the daemon.
*   **systemctl-native Binary**: `crates/op-services/src/bin/systemctl-native.rs` — CLI tool utilizing direct system D-Bus calls, bypassing network/gRPC dependencies.

---

# Production Security and Quality Audit

## Critical Severity Findings

### 1. Unauthenticated Remote & Local gRPC API Allows Privilege Escalation and Arbitrary Code Execution
*   **File Citation**: `crates/op-services/src/bin/op-services.rs:43-52`, `crates/op-services/src/grpc/server.rs:101-118`, and `crates/op-services/src/manager/process.rs:27-47`
*   **Vulnerability Type**: Privilege Escalation / Remote Code Execution (RCE) / Missing Authentication and Authorization
*   **Exploit Mechanism**:
    The daemon binds to a public TCP port (defaulting to `[::]:50053` or defined by `OP_SERVICES_GRPC_ADDR`) and starts the gRPC server using a plain `tonic::transport::Server` with absolutely no transport-level encryption (TLS), client certificate verification, or token-based authentication (`crates/op-services/src/bin/op-services.rs:43-52`). 
    
    Any network-adjacent attacker (or local unprivileged user) can send a gRPC `CreateRequest` containing a custom `ServiceDef` payload (`crates/op-services/src/grpc/server.rs:101-118`). When this service is subsequently started, the fallback `ProcessManager::start` spawns the target program (`crates/op-services/src/manager/process.rs:27-47`).
    
    Importantly, although `ServiceDef` has fields for `user` and `group` (`crates/op-services/src/grpc/server.rs:194`), the `ProcessManager` fallback executor completely ignores these fields and does **not** call `.uid()` or `.gid()` on the command, nor does it drop privileges via `setuid`/`setgid` before executing the spawned binary. Because the daemon runs as `root` (confirmed by its ability to write to `/etc/dinit.d/` in `crates/op-services/src/manager/service_manager.rs:125`), the arbitrary command provided by the untrusted gRPC caller is executed directly with `root` privileges. This is a directly exploitable remote and local root compromise.
*   **Remediation**:
    1. Force gRPC listener connection security by requiring mutual TLS (mTLS) with client certificate verification via `Server::builder().tls_config(...)`.
    2. Add an authorization middleware/interceptor to the gRPC router to validate caller tokens.
    3. Modify `ProcessManager::start` to strictly enforce `setuid`/`setgid` using the `user` and `group` fields of the service definition using the `std::os::unix::process::CommandExt` extension.

---

## High Severity Findings

### 2. PID 0 Group Kill Vulnerability leading to Local Denial of Service
*   **File Citation**: `crates/op-services/src/manager/process.rs:43-45` and `crates/op-services/src/manager/process.rs:56-60`
*   **Vulnerability Type**: Local Denial of Service (DoS) / Process Group Termination
*   **Exploit Mechanism**:
    When a fallback process is spawned, `ProcessManager` retrieves its ID using `child.id().unwrap_or(0)` and stores it in the active process map (`crates/op-services/src/manager/process.rs:43-45`). If `child.id()` returns `None` (for example, if the process is extremely short-lived, immediately defunct, or if platform-specific errors prevent ID extraction), a PID value of `0` is committed to state.
    
    When a caller attempts to stop this service, `ProcessManager::stop` retrieves the PID from the state map and passes it directly to `nix::sys::signal::kill` via `Pid::from_raw(0)` (`crates/op-services/src/manager/process.rs:56-60`). 
    
    Under POSIX standards, passing `0` as the target PID to `kill` sends the signal (here, `SIGTERM`) to **every process in the process group of the calling process**. Since the `op-services` manager runs as a single process daemon, this causes the service manager to send `SIGTERM` to itself and all of its spawned services/subprocesses, instantaneously crashing the entire system control plane.
*   **Remediation**:
    Reject startup or fail immediately if `child.id()` is `None` or resolves to `0`. Never allow a PID of `0` to be registered or passed to `kill`.
    ```rust
    let pid = child.id().ok_or_else(|| anyhow::anyhow!("Failed to acquire child PID"))?;
    if pid == 0 {
        return Err(anyhow::anyhow!("Invalid PID 0 returned by child"));
    }
    ```

---

## Medium Severity Findings

### 3. Missing Caller Authorization Policies on System D-Bus Daemon Connection
*   **File Citation**: `crates/op-services/src/dbus/interface.rs:105-115`
*   **Vulnerability Type**: Missing Access Control / Local Privilege Escalation
*   **Exploit Mechanism**:
    The daemon registers its interface on the system-wide D-Bus bus via `Connection::system().await?` and requests the well-known name `org.opdbus.services` (`crates/op-services/src/dbus/interface.rs:105-115`). 
    
    By default, unless restricted by an explicit system bus security configuration policy file (typically located under `/etc/dbus-1/system.d/`), any unprivileged local user logged into the system can invoke methods on registered interfaces. The `DbusInterface` implementation does not validate the credentials of incoming message senders (such as checking if the peer UID matches `0` or an authorized service user). Consequently, unprivileged local system users can stop, start, restart, and manipulate critical root system services.
*   **Remediation**:
    Use the `zbus::Connection::object_server` context or inspection helpers to retrieve the message sender's credentials (UID). Verify that the calling process possesses the appropriate permissions before executing actions.
    ```rust
    // In zbus methods, retrieve the connection or message context and extract the peer UID:
    let header = ctx.message().header();
    // Validate caller UID is 0 or matches an administrative group
    ```

### 4. Database Locking and Busy States due to Lack of WAL Mode and Busy Timeout
*   **File Citation**: `crates/op-services/src/store/mod.rs:15-22`
*   **Vulnerability Type**: Resource Contention / Denial of Service
*   **Exploit Mechanism**:
    The SQLite pool is initialized with a maximum of 5 connections using the standard connection URL `sqlite:<path>?mode=rwc` (`crates/op-services/src/store/mod.rs:15-22`). 
    
    By default, SQLite initializes in rollback-journal mode, which locks the entire database during write transactions. Because the service manager performs concurrent write operations (e.g., storing service statuses, updating enabled flags, and registering definitions) from multiple threads managed by the asynchronous gRPC and D-Bus runtimes, this configuration will trigger `SQLITE_BUSY` errors. 
    
    Without setting a busy timeout (`PRAGMA busy_timeout`) or migrating to Write-Ahead Logging (WAL) mode, connection requests will fail instantly instead of waiting for locks to clear, leading to failed state transitions and manager instability.
*   **Remediation**:
    Configure the SQLite connection string to force WAL mode and define a reasonable busy timeout (e.g., 5000 milliseconds) to prevent concurrent writes from failing:
    ```rust
    let url = format!(
        "sqlite:{}?mode=rwc&_journal_mode=WAL&_busy_timeout=5000",
        path.as_ref().display()
    );
    ```

---

## Low Severity & Compliance Findings

### 5. Dead Code / Missing Compliance Audit Logging
*   **File Citation**: `crates/op-services/src/store/mod.rs:112-127`
*   **Vulnerability Type**: Non-Compliance (OSCAL System and Information Integrity / System Auditing)
*   **Exploit Mechanism**:
    The database migrations correctly define and generate an `audit_log` table (`crates/op-services/src/store/mod.rs:43-53`), and the `Store` struct exposes a public async `audit` method (`crates/op-services/src/store/mod.rs:112-127`). 
    
    However, this `audit` function is never invoked anywhere else in the repository. No operations—such as service creation, deletion, start, stop, enable, or disable—write records to the `audit_log` table. This creates a compliance gap where security-relevant administrative actions are not audit-trailed, directly violating OSCAL security control requirements for continuous compliance monitoring.
*   **Remediation**:
    Integrate calls to `self.store.audit(...)` into the core service manager actions inside `ServiceManager::start`, `stop`, `create`, `delete`, and `set_enabled` in `crates/op-services/src/manager/service_manager.rs`.

### 6. Schema-as-Code Violation: Ad-Hoc JSON Payload Serialization Over D-Bus
*   **File Citation**: `crates/op-services/src/dbus/interface.rs:34`, `interface.rs:46`, `interface.rs:58`, and `interface.rs:70`
*   **Vulnerability Type**: Architectural / Schema-as-Code Violation
*   **Description**:
    The system follows a strict schema-as-code discipline using Protocol Buffers (`opdbus.services.v1`) and OSCAL component representations. However, inside the native D-Bus interface (`crates/op-services/src/dbus/interface.rs`), data contracts are violated by dumping operational state as unstructured ad-hoc strings:
    ```rust
    Ok(serde_json::to_string(&status).unwrap_or_default())
    ```
    Returning raw JSON strings bypasses versioned schemas and typed contracts, forcing D-Bus consumers to implement ad-hoc parsing logic. If the underlying `ServiceStatus` structure changes, D-Bus clients will silently break without schema detection.
*   **Remediation**:
    Model the D-Bus return structures natively using derived `zbus::zvariant::Type` and `serde::Serialize` on versioned structs, or strictly utilize Protocol Buffer serialized byte arrays mapped directly to system contract definitions.