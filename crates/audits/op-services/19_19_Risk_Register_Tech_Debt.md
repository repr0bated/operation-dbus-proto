| Severity | Issue | Evidence (file:line) | Recommendation |
| :--- | :--- | :--- | :--- |
| **Critical** | **Remote Code Execution (RCE) via Unauthenticated, Unencrypted gRPC Control Plane** | `crates/op-services/src/bin/op-services.rs:45`<br>`crates/op-services/src/grpc/server.rs:30`<br>`crates/op-services/src/grpc/server.rs:92` | Secure the tonic transport layer. Configure Mutual TLS (mTLS) via `Server::builder().tls_config(...)`. Implement a gRPC interceptor or middleware to validate caller identity (SPIFFE IDs, JWTs, or Unix socket peer credentials) before authorizing `create`, `start`, or `stop` RPC methods. Default binding address should be `127.0.0.1` or a Unix Domain Socket instead of wildcard `[::]`. |
| **High** | **Privilege Escalation via Fallback Process Manager Ignoring User/Group Dropping** | `crates/op-services/src/manager/process.rs:24`<br>`crates/op-services/src/grpc/server.rs:233` | Modify `ProcessManager::start` to resolve `service.user` and `service.group` names to numeric UIDs/GIDs. Use `std::os::unix::process::CommandExt` on the `TokioCommand` instance to call `.uid(uid)` and `.gid(gid)` before `.spawn()`, ensuring spawned processes do not run with implicit `root` privileges. |
| **High** | **Ad-hoc JSON Contracts Over D-Bus (Schema-as-Code Violation)** | `crates/op-services/src/dbus/interface.rs:29`<br>`crates/op-services/src/bin/systemctl-native.rs:55` | Declare structured, versioned D-Bus payloads by implementing `zbus::zvariant::Type` and `serde::Serialize`/`Deserialize` on shared contract structs, instead of encoding internal Rust types to opaque, untyped JSON strings (`serde_json::to_string`). |
| **High** | **Ad-hoc Database Schema Persistence & Lack of OSCAL Compliance** | `crates/op-services/src/store/mod.rs:33`<br>`crates/op-services/src/store/mod.rs:74` | Transition SQLite column storage from an unstructured JSON string (`definition TEXT NOT NULL`) to a versioned Protocol Buffer or FlatBuffers payload. Integrate a formal OSCAL (Open Security Controls Assessment Language) Component Definition schema to map system components and services directly to regulatory controls (e.g., FedRAMP/NIST 800-53) for continuous compliance auditing. |
| **Medium** | **Unhandled Critical Subsystem Failures (Silent Denial of Service)** | `crates/op-services/src/bin/op-services.rs:31`<br>`crates/op-services/src/grpc/server.rs:201` | Replace blind `tokio::spawn` tasks with a robust supervisor pattern. Use coordination primitives like `tokio::select!` or `tokio::try_join!` in the daemon root thread to guarantee that if either the D-Bus or gRPC server loops crash, the entire daemon tears down cleanly, allowing systemd/dinit to trigger a restart. |
| **Medium** | **Hardcoded Database Path and Missing Parent Directory Initialization** | `crates/op-services/src/bin/op-services.rs:25`<br>`crates/op-services/src/store/mod.rs:15` | Extract the database path to a configurable variable via environmental variables or a configuration file. Ensure `tokio::fs::create_dir_all` is executed on the parent directory (e.g., `/var/lib/op-dbus`) before initializing the SQLite connection pool to prevent initialization panics on fresh system installations. |

---

### Detailed Findings & Technical Remediation

#### 1. Remote Code Execution (RCE) via Unauthenticated, Unencrypted gRPC Control Plane
* **Vulnerability Analysis:** The system manager daemon binds to `[::]:50053` by default and exposes a gRPC service manager interface with zero authentication or encryption. Because the gRPC server accepts service creation requests (`create`) with arbitrary executable programs, and immediately allows them to be executed (`start`), any network-adjacent or local adversary can register a malicious payload (e.g., a reverse shell) and execute it as the daemon's user (`root`).
* **Remediation:** Apply TLS and peer verification.
```rust
// In crates/op-services/src/bin/op-services.rs
use tonic::transport::{Identity, Server, ServerTlsConfig};

let cert = std::fs::read_to_string("/etc/op-services/certs/server.pem")?;
let key = std::fs::read_to_string("/etc/op-services/certs/server.key")?;
let client_ca = std::fs::read_to_string("/etc/op-services/certs/client_ca.pem")?;

let tls_config = ServerTlsConfig::new()
    .identity(Identity::from_pem(cert, key))
    .client_ca_root_ca_certificate(tonic::transport::Certificate::from_pem(client_ca));

Server::builder()
    .tls_config(tls_config)?
    .add_service(ServiceManagerServer::new(grpc_server))
    .serve(addr)
    .await?;
```

#### 2. Privilege Escalation via Fallback Process Manager Ignoring User/Group Dropping
* **Vulnerability Analysis:** When `dinit-dbus` is unavailable, `op-services` falls back to its internal `ProcessManager`. Although the proto conversion (`proto_to_schema_def`) maps the requested `user` and `group` values into the target `ServiceDef` struct, the fallback process manager entirely ignores these security constraints when spawning processes, leading to all fallback services executing with root access.
* **Remediation:** Drop privileges inside the child pre-exec hook:
```rust
// In crates/op-services/src/manager/process.rs
use std::os::unix::process::CommandExt;

// Resolve uid/gid from service.user / service.group using nix/libc
if let Some(uid) = resolved_uid {
    cmd.uid(uid.as_raw());
}
if let Some(gid) = resolved_gid {
    cmd.gid(gid.as_raw());
}
```

#### 3. Ad-hoc JSON Contracts Over D-Bus (Schema-as-Code Violation)
* **Vulnerability Analysis:** The system D-Bus interface serializes highly structured runtime state structs to arbitrary, unstructured JSON strings (`zbus::fdo::Result<String>`). This defeats the compile-time safety guarantees of Rust and violates schema-as-code principles, leaving the system client (`systemctl-native`) vulnerable to parsing errors if service state definitions change without synchronized updates.
* **Remediation:** Expose structured, type-safe structures natively in `zbus`:
```rust
// Instead of String, return the structured and decorated representation
#[derive(zbus::zvariant::Type, serde::Serialize, serde::Deserialize)]
pub struct ServiceStatusDto {
    pub name: String,
    pub state: String,
    pub pid: Option<u32>,
}

// In the D-Bus interface:
async fn get_status(&self, name: &str) -> zbus::fdo::Result<ServiceStatusDto>;
```

#### 4. Ad-hoc Database Schema Persistence & Lack of OSCAL Compliance
* **Vulnerability Analysis:** Service definitions are persisted as raw, unstructured JSON text inside SQLite (`definition TEXT NOT NULL`). There are no migrations, version markers, or integrity assertions. This completely isolates the systems architecture from OSCAL tracking, complicating automated compliance assessments.
* **Remediation:** Define database schemas mapped directly to a versioned protobuf format or formal OSCAL Component Definition models. Store version flags inside the database to enforce schema compatibility migrations. Use SQLx query binding to validate structure before parsing.