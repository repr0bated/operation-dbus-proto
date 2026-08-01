# Monorepo Production Quality & Security Audit: `op-cache`

### Summary Table of Findings

| ID | File & Line Citation | Severity | Title |
|----|----------------------|----------|-------|
| 1 | `crates/op-cache/src/btrfs_cache.rs:550` | **Critical** | Shell Command Injection in Remote BTRFS Sender Pipeline |
| 2 | `crates/op-cache/src/btrfs_cache.rs:588` | **Critical** | Shell Command Injection in Remote BTRFS Receiver Pipeline |
| 3 | `crates/op-cache/src/btrfs_cache.rs:420` | **Medium** | Time-of-Check to Time-of-Use (TOCTOU) Stale Cache Race Condition |
| 4 | `crates/op-cache/src/workflow_cache.rs:280` | **Medium** | Out-of-Sync Cache Metrics on Step Invalidation |
| 5 | `crates/op-cache/src/grpc/orchestrator_service.rs:46` | **Medium** | Unbounded Memory Leak in gRPC Pattern Tracking Map |
| 6 | `crates/op-cache/src/agent_registry.rs:119` | **Medium** | Schema-as-Code Violation: Decoupled Domain Models and Dual Resolvers |
| 7 | `crates/op-cache/src/workflow_tracker.rs:356` | **Low** | Unnecessary Unsafe Block on Safe String Deserialization |

---

### Detailed Findings Report

#### Finding 1: Shell Command Injection in Remote BTRFS Sender Pipeline
- **Severity**: **Critical**
- **File**: `crates/op-cache/src/btrfs_cache.rs`
- **Lines**: 550–574
- **Description**: 
The `stream_to_remote` method takes string slice arguments (`remote_host`, `remote_path`) from the caller, formats them directly into a shell command, and executes the payload through an intermediate shell shell interpreter (`bash -c`):
```rust
        let cmd = format!(
            "btrfs send {} | ssh {} 'btrfs receive {}'",
            snapshot_path.display(),
            remote_host,
            remote_path
        );

        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&cmd)
```
If an attacker is able to influence the `remote_host` or `remote_path` variables (via gRPC request payload parameters or state mirror configurations), they can append arbitrary shell metacharacters (e.g., `; rm -rf / ;` or backticks) to execute arbitrary commands with the permissions of the host process.
- **Remediation**:
Avoid invoking intermediate shells (`bash -c`) with string interpolation. Execute the binaries directly and programmatically link their stdout/stdin pipes:
```rust
let mut send_child = tokio::process::Command::new("btrfs")
    .args(["send", &snapshot_path.to_string_lossy()])
    .stdout(std::process::Stdio::piped())
    .spawn()?;

let output = tokio::process::Command::new("ssh")
    .arg(remote_host)
    .arg(format!("btrfs receive {}", remote_path)) // remote command execution argument
    .stdin(send_child.stdout.take().unwrap())
    .output()
    .await?;
```

#### Finding 2: Shell Command Injection in Remote BTRFS Receiver Pipeline
- **Severity**: **Critical**
- **File**: `crates/op-cache/src/btrfs_cache.rs`
- **Lines**: 588–608
- **Description**:
Similar to Finding 1, the `receive_from_remote` method uses string formatting to construct a pipeline executed via `bash -c`:
```rust
        let cmd = format!(
            "ssh {} 'btrfs send {}' | btrfs receive {}",
            remote_host, remote_snapshot, local_path
        );

        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&cmd)
```
This leaves the system highly vulnerable to shell injection if `remote_host`, `remote_snapshot`, or `local_path` are derived from unsanitized network transactions.
- **Remediation**:
Refactor process execution to use safe argument arrays without shell expansion:
```rust
let mut ssh_child = tokio::process::Command::new("ssh")
    .args([remote_host, &format!("btrfs send {}", remote_snapshot)])
    .stdout(std::process::Stdio::piped())
    .spawn()?;

let output = tokio::process::Command::new("btrfs")
    .args(["receive", local_path])
    .stdin(ssh_child.stdout.take().unwrap())
    .output()
    .await?;
```

#### Finding 3: Time-of-Check to Time-of-Use (TOCTOU) Stale Cache Race Condition
- **Severity**: **Medium**
- **File**: `crates/op-cache/src/btrfs_cache.rs`
- **Lines**: 420–445
- **Description**:
The `cleanup_old` method uses unsynchronized transactions on SQLite index metadata and local storage cache files. The database connection lock (`index.lock()`) is released (at line 436) after identifying stale vectors, then the files are deleted on disk, and finally the connection is re-locked to remove entries from SQLite:
```rust
        drop(stmt); // Release statement
        drop(index); // Release lock before file I/O

        // Delete files
        for (_hash, file) in &old_entries {
            let path = self.cache_dir.join("embeddings/vectors").join(file);
            let _ = std::fs::remove_file(path); // Ignore errors
        }

        // Delete from index
        let index = self.index.lock().unwrap();
        index.execute("DELETE FROM embeddings WHERE accessed_at < ?1", [cutoff])?;
```
If a concurrent process queries `load_embedding` between these two phases, it will successfully read the metadata from the database index, attempt to fetch the vector file from disk, fail because the file has been deleted, and bubble up a hard `std::io::Error` to the caller, interrupting the workflow runtime rather than cleanly falling back to a cache-miss workflow.
- **Remediation**:
Modify `load_embedding` to intercept `std::io::ErrorKind::NotFound` and handle it as a standard cache-miss (`Ok(None)`) while lazily scrubbing the stale SQLite row:
```rust
            let data = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(None);
                }
                Err(e) => return Err(e).context("Failed to read cached embedding"),
            };
```

#### Finding 4: Out-of-Sync Cache Metrics on Step Invalidation
- **Severity**: **Medium**
- **File**: `crates/op-cache/src/workflow_cache.rs`
- **Lines**: 280–306
- **Description**:
The `invalidate` function removes an output cache entry and deletes its associated file from disk. However, it fails to update the aggregated `workflow_cache_meta` table. The same omission occurs inside `invalidate_workflow` (line 309), `invalidate_step` (line 339), `cleanup_expired` (line 370), and `evict_to_size` (line 406).
As a result, any invalidation, expiration, or eviction leaves the aggregated metrics stale, rendering metrics like `total_entries` and `total_size_bytes` highly inaccurate for client dashboards and state management libraries.
- **Remediation**:
Invoke `update_workflow_meta` inside all invalidation, deletion, and cleanup helper functions immediately before releasing the database write lock.

#### Finding 5: Unbounded Memory Leak in gRPC Pattern Tracking Map
- **Severity**: **Medium**
- **File**: `crates/op-cache/src/grpc/orchestrator_service.rs`
- **Lines**: 46–49, 290–332
- **Description**:
The `OrchestratorServiceImpl` uses an in-memory `HashMap` to record executed agent sequence patterns:
```rust
pub struct OrchestratorServiceImpl {
    agent_service: Arc<AgentServiceImpl>,
    cache_service: Arc<CacheServiceImpl>,
    patterns: Arc<RwLock<HashMap<String, TrackedPattern>>>,
...
```
Unlike the SQLite-backed `PatternTracker` in `pattern_tracker.rs` which supports explicit eviction and pruning limits, the gRPC orchestrator service inserts keys into this memory map indefinitely. Under prolonged server uptime with dynamically generated sequences, this collection will grow without bound, resulting in eventual process termination via Out-Of-Memory (OOM).
- **Remediation**:
Refactor `OrchestratorServiceImpl` to utilize the SQLite-backed `PatternTracker` from `pattern_tracker.rs` directly, or wrap the `patterns` map in a bounded Least Recently Used (LRU) cache.

#### Finding 6: Schema-as-Code Violation: Decoupled Domain Models and Dual Resolvers
- **Severity**: **Medium**
- **File**: `crates/op-cache/src/agent_registry.rs`, `crates/op-cache/src/grpc/orchestrator_service.rs`
- **Lines**: `agent_registry.rs:119`, `orchestrator_service.rs:73`
- **Description**:
The project aims to enforce a schema-as-code discipline using Protocol Buffers (`op_cache` package). However:
1. `AgentDefinition` in `agent_registry.rs` is defined as a native, ad-hoc Rust struct with serde definitions instead of mapping directly to the protobuf definition.
2. `AgentCapability` is defined as an ad-hoc Rust enum requiring string-based manual parsing (line 68) instead of relying on the versioned Protobuf enum.
3. This dissociation forces the implementation of two distinct, non-integrated capability resolution systems: `CapabilityResolver` (in `capability_resolver.rs`) and `resolve_capabilities` (in `grpc/orchestrator_service.rs`). This architectural drift results in dual maintenance overhead and risks semantic variations in how agent sequences are scored and generated.
- **Remediation**:
Replace ad-hoc models with Rust types derived from versioned `.proto` contracts. Standardize capability resolution by feeding gRPC requests through `CapabilityResolver` via `From`/`Into` conversions, making protobuf schemas the single source of truth.

#### Finding 7: Unnecessary Unsafe Block on Safe String Deserialization
- **Severity**: **Low**
- **File**: `crates/op-cache/src/workflow_tracker.rs`
- **Lines**: 356, 403, 436
- **Description**:
The code parses a JSON sequence using `simd_json::from_str` within an `unsafe` block:
```rust
                    let mut agent_sequence_json: String = row.get(1)?;
                    let agent_sequence: Vec<String> =
                        unsafe { simd_json::from_str(&mut agent_sequence_json) }
                            .unwrap_or_default();
```
In `simd-json` version `0.13`, `from_str` is a safe function taking `&mut str`. Wrapping a safe function inside an `unsafe` block violates safety design guidelines, masks compilation warnings, and increases the difficulty of performing code audits.
- **Remediation**:
Remove the `unsafe` block and invoke the safe `simd_json::from_str` directly.

---

### Crate and Workspace Build Audit

#### Cargo.toml Metadata
- **Crate Name**: `op-cache`
- **Edition**: `2021` (compliant with workspace settings).
- **Rust Version**: Not configured inside the monorepo workspace package.
- **Targets**: No custom binaries or examples are present.

#### Workspace Integration & Inherited Dependencies
- **Inherited Dependencies**: uses `{ workspace = true }` for core asynchronous, protobuf, database, and serializing packages: `futures`, `prost`, `prost-types`, `rusqlite` (retaining workspace `bundled` features), `serde_json`, `simd-json`, `tonic`, and `tonic-build`.
- **Local Overrides**: Key platform dependencies are overriden locally rather than via workspace version selection: `anyhow = "1.0"`, `bincode = "1.3"`, `chrono = "0.4"`, `serde = "1.0"`, `tokio = "1.0"`, `uuid = "1.0"`, and `zstd = "0.13"`. This creates monorepo inconsistencies and compilation overhead.

#### Codegen & Build Verification
- **Protocol Buffer Compilation**: Crate `op-cache` depends on `tonic-build` under `[build-dependencies]` (workspace version `0.12`).
- **Source of Truth Check**: Protobuf schemas are resolved at build-time (verified by `tonic::include_proto!("op_cache")` in `lib.rs`). No compiled Rust files are checked in under `src/` which conforms to clean workspace patterns.
- **Runtime Compilation**: Verification confirmed that proto compilation is handled strictly at build-time. No dynamic runtime compilers are present.