# System-Wide Service Manager (`op-services`) Security & Quality Audit

---

## 1. ROLE: Build Check & Schema-As-Code

### Build Configuration Summary
*   **Edition:** `2021` (specified in both `crates/op-services/Cargo.toml` and root `Cargo.toml`).
*   **Rust Version:** No `rust-version` is specified in `crates/op-services/Cargo.toml` or the root `Cargo.toml`.
*   **Binaries:** Three binaries are declared in `crates/op-services/Cargo.toml`:
    1.  `op-services` (`src/bin/op-services.rs`) - The service manager daemon.
    2.  `systemctl` (`src/bin/systemctl.rs`) - gRPC CLI compatibility wrapper.
    3.  `systemctl-native` (`src/bin/systemctl-native.rs`) - Local D-Bus CLI client.
*   **Examples:** None declared or present.

### Workspace Inheritance vs. Local Overrides
The root `Cargo.toml` defines a workspace and a central set of dependencies under `[workspace.dependencies]`. However, `crates/op-services/Cargo.toml` **does not inherit** these dependencies. Instead of using `workspace = true`, it defines local version overrides for nearly every dependency:
*   `tonic = "0.12"`
*   `prost = "0.13"`
*   `prost-types = "0.13"`
*   `zbus = { version = "4.0", features = ["tokio"] }`
*   `sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }`
*   `tokio = { version = "1", features = ["full", "signal"] }`

This bypasses the workspace's version-pinning and dependency management, introducing compilation bloat and risks of duplicate-dependency linking conflicts.

### Schema-As-Code Build Check
*   **`build.rs` Analysis:** No `build.rs` file is included in the provided `crates/op-services/` directory files. However, `crates/op-services/Cargo.toml` lists `tonic-build = "0.12"` in its `[build-dependencies]`, and `crates/op-services/src/grpc/mod.rs:8` invokes `tonic::include_proto!("opdbus.services.v1")`. This implies a build script is executed during compilation to generate the Rust structures.
*   **`.proto` Source of Truth:** No `.proto` files are present in the provided FILES section.
*   **Runtime Compilation:** Proto compilation does not occur at runtime; it is performed at build-time using `tonic-build` (implied by `include_proto!`).
*   **Ad-Hoc Data Contracts (VIOLATION):** 
    *   **File:** `crates/op-services/src/dbus/interface.rs:33,46,59,72`
    *   **Description:** The D-Bus interface bypasses versioned schema enforcement by serializing internal types to raw JSON strings (`serde_json::to_string(&status).unwrap_or_default()`) and returning them as untyped D-Bus `String` objects. 
    *   **File:** `crates/op-services/src/bin/systemctl-native.rs:33,42,51,55`
    *   **Description:** The D-Bus client receives these unstructured JSON strings, expecting the caller and receiver to implicitly agree on the JSON layout without a compiled, versioned schema contract. This violates schema-as-code discipline.

---

## 2. SECURITY AUDIT FINDINGS

### Summary of Findings
| Finding Reference | Severity | Category | Exploitable? |
| :--- | :--- | :--- | :--- |
| **OS-01** | Critical | Remote Code Execution | Yes (Network-facing root RCE) |
| **OS-02** | Critical | Self-Denial of Service | Yes (Local/Remote daemon termination) |
| **OS-03** | High | Path Traversal File Deletion | Yes (Privileged file deletion) |
| **OS-04** | Medium | Silent Thread Termination | Yes (Spurious channel closure) |

---

### OS-01: Unauthenticated Network-Facing gRPC Interface Allowing Arbitrary Process Spawn
*   **Severity:** Critical (Exploitable)
*   **Citations:**
    *   `crates/op-services/src/bin/op-services.rs:43-53`
    *   `crates/op-services/src/grpc/server.rs:136-150`
    *   `crates/op-services/src/manager/process.rs:26-48`
*   **Description:**
    The `op-services` daemon (designed to run as a systemd replacement with elevated system privileges) starts a gRPC server that binds to all network interfaces (`[::]:50053` by default) without any authentication, authorization, or transport-layer security (TLS). 
    
    An attacker on the network can connect to the gRPC port and call the `create` method to register a new service with an arbitrary `exec_start.program` path and `exec_start.args`. They can then invoke the `start` method, forcing the privileged system daemon to execute arbitrary commands as the user running the daemon (typically `root`).

```rust
// crates/op-services/src/bin/op-services.rs:43-53
let addr = std::env::var("OP_SERVICES_GRPC_ADDR")
    .unwrap_or_else(|_| "[::]:50053".to_string())
    .parse()?;

info!("gRPC server listening on {}", addr);

Server::builder()
    .add_service(ServiceManagerServer::new(grpc_server)) // No auth/TLS middleware
    .serve(addr)
    .await?;
```

*   **Remediation:**
    1. Bind to a local UNIX domain socket (`/run/op-services.sock`) by default instead of a TCP port.
    2. If TCP is mandatory, enforce Mutual TLS (mTLS) and add token-based authorization metadata interceptors to all gRPC endpoints.

---

### OS-02: Self-Denial of Service via `kill(0, SIGTERM)` Process Group Termination
*   **Severity:** Critical (Exploitable)
*   **Citations:**
    *   `crates/op-services/src/manager/process.rs:43`
    *   `crates/op-services/src/manager/process.rs:65`
*   **Description:**
    When spawning a service in fallback mode, the process manager attempts to track the child PID. If the process spawn fails or does not yield a PID, the variable is assigned a default fallback value of `0`:
    ```rust
    // crates/op-services/src/manager/process.rs:43
    let pid = child.id().unwrap_or(0);
    ```
    This value `0` is successfully inserted into the tracking map. When a user requests to `stop` this service, the daemon retrieves the PID (`0`) and executes a raw UNIX `kill` system call:
    ```rust
    // crates/op-services/src/manager/process.rs:65
    if let Err(e) = kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
    ```
    In UNIX systems, calling `kill(0, SIGTERM)` sends `SIGTERM` to **all processes in the current process group of the calling process**. Because `op-services` is the calling process, this immediately terminates the service manager daemon itself and all other services sharing its process group. This allows any unauthenticated gRPC client or D-Bus client to trigger a total shutdown of the core service manager.

*   **Remediation:**
    Validate that `pid > 0` before calling `kill`. If `pid` is `0` or `None`, handle the termination as a failure or skip the `kill` system call entirely.

```rust
let pid_i32 = pid as i32;
if pid_i32 <= 0 {
    return Err(anyhow::anyhow!("Invalid process ID: {}", pid_i32));
}
kill(Pid::from_raw(pid_i32), Signal::SIGTERM)?;
```

---

### OS-03: Arbitrary File Deletion via Service Name Path Traversal
*   **Severity:** High (Exploitable)
*   **Citations:**
    *   `crates/op-services/src/manager/service_manager.rs:177-183`
*   **Description:**
    The `delete` method of the service manager constructs a configuration filepath using the unvalidated `ServiceName` value provided by the client:
    ```rust
    // crates/op-services/src/manager/service_manager.rs:177-183
    let path = format!("/etc/dinit.d/{}", name);
    if let Err(e) = tokio::fs::remove_file(&path).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!("Failed to remove dinit service file {}: {}", path, e);
        }
    }
    ```
    If `ServiceName` allows path traversal sequences (such as `../../etc/shadow` or `../../usr/lib/libc.so`), a privileged deletion is triggered. Because the daemon runs with system permissions, an attacker can delete arbitrary files across the operating system filesystem, leading to complete system corruption or privilege escalation.

*   **Remediation:**
    1. Sanitize and canonicalize `ServiceName` inputs to ensure they contain only alphanumeric characters, dashes, and underscores.
    2. Prevent any directory traversal components (such as `/` or `.`) in the name validator before attempting filesystem modifications.

---

### OS-04: Fragile Client Stream Dropping via Unhandled `RecvError::Lagged`
*   **Severity:** Medium (Quality & Denial of Service)
*   **Citations:**
    *   `crates/op-services/src/grpc/server.rs:219-236`
*   **Description:**
    In the `watch_status` gRPC implementation, a status subscriber is established via a tokio broadcast channel:
    ```rust
    // crates/op-services/src/grpc/server.rs:219-236
    let (tx, rx) = mpsc::channel(128);
    let mut sub = self.manager.subscribe();

    tokio::spawn(async move {
        while let Ok(event) = sub.recv().await {
            if tx.send(Ok(event.into())).await.is_err() {
                break;
            }
        }
    });
    ```
    `sub.recv()` returns a `Result<T, RecvError>`. If the system is under heavy load and a client cannot drain the gRPC buffer fast enough, the bounded internal `tx` channel fills up, causing the worker task to block. 
    
    If the manager continues to push status updates to the broadcast channel while the task is blocked, the subscriber will fall behind the broadcast lag limit (256 elements). This causes `sub.recv().await` to return `Err(RecvError::Lagged)`. 
    
    Because the loop condition specifically tests `while let Ok(event) = sub.recv().await`, the `Err(RecvError::Lagged)` immediately **terminates the loop and drops the client connection**, causing silent disconnects under high load.

*   **Remediation:**
    Handle the `RecvError::Lagged` error explicitly by logging a warning, skipping lost messages, or returning a gRPC status error instead of silently exiting the stream loop.

```rust
loop {
    match sub.recv().await {
        Ok(event) => {
            if tx.send(Ok(event.into())).await.is_err() {
                break;
            }
        }
        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
            warn!("Subscriber lagged behind. Missed {} messages.", skipped);
            // Continue execution; do not terminate the loop
        }
        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
            break;
        }
    }
}
```

---
## ⚠ Citation Warnings
- `crates/op-services/src/grpc/mod.rs:8`: file has 7 lines
