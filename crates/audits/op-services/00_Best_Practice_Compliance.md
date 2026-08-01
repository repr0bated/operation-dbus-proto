| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `command_new` | `crates/op-services/src/grpc/server.rs:229` | Instantiates `schema::ExecCommand` from unstructured raw string fields (`exec.start_program` and `exec.start_args`). | Express executable definitions via strict versioned schemas or predefined enum-based command registries. | Bypasses Schema-as-Code discipline by allowing arbitrary, unvalidated program paths as strings. | Major Gap |
| `command_new` | `crates/op-services/src/grpc/server.rs:232` | Converts raw optional strings to `PathBuf` and executes via `ExecCommand::new` without validating format. | Leverage strongly typed command definitions with secure path resolution models. | Lack of strict schema-level constraints on binary paths allowing arbitrary command specification. | Major Gap |
| `format_json_manual` | `crates/op-services/src/grpc/server.rs:142` | Formats gRPC error strings dynamically via `format!("service not found: {}", name)`. | Utilize structured, schema-defined error response codes or gRPC rich status types (`google.rpc.Status`). | String interpolation for errors leaks runtime parameters and bypasses versioned error schemas. | Minor Gap |
| `command_new` | `crates/op-services/src/manager/process.rs:25` | Spawns system processes with `TokioCommand::new` using paths extracted directly from `ServiceDef`. | Restrict command invocation to a pre-validated system execution allowlist or sandboxed binaries. | Dynamic command execution from mutable config files without path sanitization/allowlist verification. | Major Gap |
| `format_json_manual` | `crates/op-services/src/manager/service_manager.rs:145` | Direct path construction using `format!("/etc/dinit.d/{}", name)` and invoking `tokio::fs::remove_file`. | Validate path components using strict sanitization patterns or verify that `name` contains no path separators. | **Path Traversal Vulnerability**: Arbitrary file deletion if `name` contains directory traversal sequences (`../`). | Critical Gap |
| `std_fs_in_async` | `crates/op-services/src/manager/service_manager.rs:146` | Uses `tokio::fs::remove_file` to asynchronously clean up the dinit service file. | Perform non-blocking async file operations to avoid blocking the runtime executor threads. | None. | Compliant |
| `format_json_manual` | `crates/op-services/src/store/mod.rs:15` | Constructs SQLite URL using dynamic interpolation `format!("sqlite:{}?mode=rwc", path)`. | Use structured configuration objects or properly escaped URL builders. | Ad-hoc connection string interpolation; lacks escaping for special path characters. | Minor Gap |

---

### Actionable Recommendations for Major and Critical Gaps

#### 1. Fix Critical Path Traversal Vulnerability in Service Deletion (`crates/op-services/src/manager/service_manager.rs:145`)
* **Vulnerability Analysis**: The code constructs `/etc/dinit.d/{name}` and deletes it using `tokio::fs::remove_file` without checking if `name` contains path traversal characters (`..` or `/`). If `name` is supplied via an external API (such as the gRPC server), an attacker could supply a payload like `../../etc/target_file`, enabling arbitrary file deletion under the user context running the manager.
* **Resolution**: Enforce strict validation on `name` before dynamic file operation path construction. 
```rust
// In crates/op-services/src/manager/service_manager.rs
let path_buf = std::path::Path::new(&name);
if path_buf.components().count() != 1 || path_buf.file_name().unwrap_or_default() != name.as_str() {
    return Err(anyhow::anyhow!("Invalid service name provided"));
}
let path = format!("/etc/dinit.d/{}", name);
```

#### 2. Restrict Command Execution Paths & Implement Schema-as-Code Command Catalog (`crates/op-services/src/grpc/server.rs:229`, `crates/op-services/src/manager/process.rs:25`)
* **Vulnerability Analysis**: Spawning processes from arbitrary configuration strings (`exec.start_program`) bypassed via RPC calls allows arbitrary command execution.
* **Resolution**: 
  1. Map the system execution to an explicit Schema-as-Code command definition (using Protobuf enums or OSCAL component catalogs) instead of accepting raw, arbitrary string inputs.
  2. Implement an execution path allowlist that verifies that any binary requested for execution exists within a hardcoded list of approved system targets (e.g., `/usr/bin/dinit`, `/usr/libexec/op/...`):
```rust
const ALLOWED_EXECUTABLES: &[&str] = &["/usr/bin/my-approved-service", "/usr/sbin/safe-binary"];

if !ALLOWED_EXECUTABLES.contains(&service.exec_start.program.to_str().unwrap_or_default()) {
    return Err(anyhow::anyhow!("Unauthorized executable path attempted"));
}
```