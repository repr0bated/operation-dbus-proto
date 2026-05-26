### Standard Environment Variable Reads
No `std::env::var` or `std::env::var_os` reads are found in the provided codebase.

---

### Environment Variables with No Defaults / No Error Handling
No environment variable reads were identified in the source files, meaning there are no instances of missing defaults or unhandled environment errors.

---

### Cargo Features and Additivity

#### Workspace-Level Features (`Cargo.toml`)
*   `default = ["grpc"]`
*   `grpc = []`

#### Crate-Level Features (`crates/op-projection/Cargo.toml`)
The `op-projection` crate does not declare any custom local features. It inherits the workspace packaged settings and manages dependencies transitively.

#### Additivity Analysis
Cargo features are **additive**. If any crate in the workspace dependency graph enables a feature (such as `grpc`), it is enabled globally for all packages sharing that dependency compilation. Because `default = ["grpc"]` is defined in the workspace root, `grpc` will be compiled in by default unless explicitly excluded via `--no-default-features`.

---

### Hardcoded Paths, Ports, and Addresses

#### 1. Hardcoded State Store Directory Path
*   **File/Line**: `crates/op-projection/src/plugin_reader.rs:24`
*   **Code**: `const STATE_STORE_PATH: &str = "/var/lib/op-dbus/state.db";`
*   **Risk**: Bypasses system configuration profiles. If the service runs in a restricted/containerized environment without write access to `/var/lib/op-dbus/`, state management falls back to an in-memory database, losing persistence across restarts.

#### 2. Hardcoded SQLite Memory Path
*   **File/Line**: `crates/op-projection/src/plugin_reader.rs:72`
*   **Code**: `SqliteStore::new(":memory:")`
*   **Risk**: Quietly swallows initialization errors on the primary state path and falls back to a non-persistent in-memory store.

#### 3. Hardcoded System procfs Paths
*   **File/Line**: `crates/op-projection/src/procfs_reader.rs:81`, `104`, `108`, `125`, `151`, `178`, `195`
*   **Code**:
    *   Line 81: `std::path::Path::new("/proc").exists()`
    *   Line 104: `fs::read_dir("/proc")`
    *   Line 108: `format!("/proc/{}/comm", pid)`
    *   Line 125: `fs::read_to_string("/proc/meminfo")`
    *   Line 151: `fs::read_to_string("/proc/cpuinfo")`
    *   Line 178: `fs::read_to_string("/proc/filesystems")`
    *   Line 195: `fs::read_to_string("/proc/net/dev")`
*   **Risk**: Assumes a standard Linux directory hierarchy. Fails on non-Linux platforms (e.g., macOS, Windows) or in containerized/chrooted environments where `/proc` is not mounted or has restricted permissions.

#### 4. Hardcoded SSE Server Binding Address
*   **File/Line**: `crates/op-projection/src/json_stream.rs:126`
*   **Code**: `let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));`
*   **Risk**: Binds the Server-Sent Events (SSE) server to all network interfaces (`0.0.0.0`). This exposes real-time state projections to the external network by default, instead of restricting access to `127.0.0.1` unless configured otherwise.

#### 5. Hardcoded Server Port
*   **File/Line**: `crates/op-projection/src/bin/projection_server.rs:192`
*   **Code**: `stream_server.start(8082)?;`
*   **Risk**: Prevents running multiple server instances on the same host and can cause port collisions if port `8082` is already in use by another system service.

---

### Schema-as-Code & Quality Audit (Data Contracts)

The codebase implements an *ad-hoc schema* approach rather than a strict unified Schema-as-Code model (such as compiled Protocol Buffers or machine-readable OSCAL profiles):

1.  **Ad-Hoc Rust Schemas**:
    *   `crates/op-projection/src/data_models.rs:16` (`PluginSchema`) and `crates/op-projection/src/data_models.rs:41` (`FieldSchema`) define schema components as manually serialized Rust structs rather than importing a single, version-controlled schema format like Protocol Buffers.
2.  **Unstructured Payload Storage**:
    *   `crates/op-projection/src/data_models.rs:151` (`Projection` struct): The core state representation payload `pub data: Value` uses `simd_json::OwnedValue` (an unstructured JSON type).
    *   `crates/op-projection/src/interfaces.rs:104` (`RawEntity` struct): Uses `pub data: Value`.
    *   *Impact*: Because payloads are parsed into generic JSON values, the system relies on runtime-evaluated validation engines rather than compile-time type-safety guarantees offered by native code generation.
3.  **Ad-Hoc Authorization Policies**:
    *   `crates/op-projection/src/data_models.rs:470` (`AccessPolicy`): Authorization controls are defined as native, custom Rust structs rather than versioned OSCAL-compliant security profiles or OAuth/OIDC metadata schemas.

---

### Security and Quality Findings

#### CRITICAL: Complete Bypass of Sensitive Data Redaction (Data Leakage)
*   **File/Line**: `crates/op-projection/src/access_control.rs:107-114`
*   **Code**:
    ```rust
    fn redact_sensitive(
        &self,
        data: &simd_json::OwnedValue,
        _requester: &Requester,
    ) -> simd_json::OwnedValue {
        // In production, use JSON paths from schema to redact
        data.clone()
    }
    ```
*   **Exploitability**: **Directly Exploitable**. The access controller uses this method to redact PII and secret fields (as specified by `secret_paths` and `pii_paths` in `data_models.rs:28-30`) when an access policy dictates that redaction is required (`access_control.rs:54`). Because this function is a placeholder that simply returns a clone of the unmodified data, **sensitive PII and cryptographic credentials are leaked in plaintext to unauthorized requesters** even when the policy explicitly flags the data for redaction.

#### HIGH: On-the-Fly Regex Compilation (ReDoS and Denial of Service)
*   **File/Line**: `crates/op-projection/src/access_control.rs:52`, `71`
*   **Code**:
    *   Line 52: `let re = Regex::new(&policy.resource_pattern)?;`
    *   Line 71: `let re = Regex::new(&policy.resource_pattern)?;`
*   **Exploitability**: **High**. The `enforce_policy` and `validate_permissions` functions compile regular expressions from access control policies *on every single request evaluation*.
    *   If an attacker can inject or modify policy resource patterns (e.g., via administrative actions or configuration manipulation), they can input a complex backtracking pattern (Regular Expression Denial of Service - ReDoS), locking up the CPU during request processing.
    *   If an invalid regex string is loaded, the access evaluation fails entirely, throwing a runtime error and causing service degradation. Patterns must be compiled once upon policy registration/insertion and stored as a pre-compiled `Regex` object.

---
## ⚠ Citation Warnings
- `crates/op-projection/src/data_models.rs:470`: file has 451 lines
