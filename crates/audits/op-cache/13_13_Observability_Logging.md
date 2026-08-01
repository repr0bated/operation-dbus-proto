# OP-CACHE PRODUCTION SECURITY & QUALITY AUDIT

## 1. Observability Metrics & Log Counts

### Macro Counts
A complete analysis of observability macros within the `op-cache` crate reveals a heavy reliance on the modern `tracing` ecosystem, alongside legacy fallbacks to the standard library's `log` crate in isolated modules. No raw `println!` or `eprintln!` statements are utilized in production code.

*   **`println!` / `eprintln!`**: **0**
*   **`log` Crate Macros**: **14**
    *   `log::info!`: 9
    *   `log::warn!`: 3
    *   `log::debug!`: 2
*   **`tracing` Crate Macros**: **86**
    *   `tracing::info!`: 39
    *   `tracing::debug!`: 30
    *   `tracing::warn!`: 17
    *   `tracing::error!`: 0

### Observability Distribution by File
*   `crates/op-cache/src/agent_registry.rs`: `tracing::info!` (2)
*   `crates/op-cache/src/btrfs_cache.rs`: `tracing::warn!` (9), `tracing::debug!` (10), `tracing::info!` (4), `log::info!` (4), `log::warn!` (3)
*   `crates/op-cache/src/orchestrator.rs`: `tracing::info!` (4), `tracing::warn!` (1), `tracing::debug!` (3)
*   `crates/op-cache/src/pattern_tracker.rs`: `tracing::info!` (3)
*   `crates/op-cache/src/snapshot_manager.rs`: `log::info!` (5), `log::debug!` (2)
*   `crates/op-cache/src/workflow_cache.rs`: `tracing::info!` (3), `tracing::debug!` (5)
*   `crates/op-cache/src/workflow_executor.rs`: `tracing::info!` (5), `tracing::debug!` (4), `tracing::warn!` (1)
*   `crates/op-cache/src/workflow_tracker.rs`: `tracing::info!` (3)
*   `crates/op-cache/src/workstack_cache.rs`: `tracing::info!` (3), `tracing::debug!` (3)
*   `crates/op-cache/src/capability_resolver.rs`: `tracing::debug!` (1), `tracing::warn!` (1), `tracing::info!` (1)
*   `crates/op-cache/src/numa.rs`: `tracing::info!` (2), `tracing::debug!` (1), `tracing::warn!` (1)
*   `crates/op-cache/src/grpc/agent_service.rs`: `tracing::info!` (3), `tracing::debug!` (1), `tracing::warn!` (1)
*   `crates/op-cache/src/grpc/cache_service.rs`: `tracing::debug!` (1)
*   `crates/op-cache/src/grpc/mcp_service.rs`: `tracing::debug!` (2), `tracing::warn!` (2), `tracing::info!` (2)
*   `crates/op-cache/src/grpc/orchestrator_service.rs`: `tracing::debug!` (1), `tracing::info!` (3)
*   `crates/op-cache/src/grpc/server.rs`: `tracing::info!` (2)

---

## 2. Swallowed Errors Audit

Several functions silently ignore failures, discarding returned `Result` types without logging diagnostic information or propagating the error.

### 2.1 File System Deletion Failures
When clearing cached raw data, deletion errors are completely swallowed using `let _ =`. If the file system has locked handles, bad sectors, or permission changes, the database index will be cleared, but the physical files will remain on disk. This results in persistent, untracked disk space leakage.
*   `crates/op-cache/src/btrfs_cache.rs:510`: `let _ = std::fs::remove_file(path);`
*   `crates/op-cache/src/workflow_cache.rs:271`: `let _ = std::fs::remove_file(data_path);`
*   `crates/op-cache/src/workflow_cache.rs:311`: `let _ = std::fs::remove_file(data_path);`
*   `crates/op-cache/src/workflow_cache.rs:343`: `let _ = std::fs::remove_file(data_path);`
*   `crates/op-cache/src/workflow_cache.rs:380`: `let _ = std::fs::remove_file(data_path);`
*   `crates/op-cache/src/workflow_cache.rs:434`: `let _ = std::fs::remove_file(data_path);`
*   `crates/op-cache/src/workstack_cache.rs:243`: `let _ = std::fs::remove_file(...);`
*   `crates/op-cache/src/workstack_cache.rs:268`: `let _ = std::fs::remove_file(...);`
*   `crates/op-cache/src/workstack_cache.rs:294`: `let _ = std::fs::remove_file(...);`

### 2.2 Silent JSON Parsing Failure in Database Retrieval
If the SQLite database suffers silent corruption or contains incompatible structural changes in its JSON text columns, parsing errors are silently ignored. The application falls back to an empty sequence without logging the deserialization failure.
*   `crates/op-cache/src/pattern_tracker.rs:295`: `unsafe { simd_json::from_str(&mut agent_sequence_json) }.unwrap_or_default();`
*   `crates/op-cache/src/workflow_tracker.rs:245`: `unsafe { simd_json::from_str(&mut agent_sequence_json) }.unwrap_or_default();`
*   `crates/op-cache/src/workflow_tracker.rs:333`: `unsafe { simd_json::from_str(&mut agent_sequence_json) }.unwrap_or_default();`
*   `crates/op-cache/src/workflow_tracker.rs:361`: `unsafe { simd_json::from_str(&mut agent_sequence_json) }.unwrap_or_default();`

### 2.3 Taskset Execution Discard
*   `crates/op-cache/src/workflow_executor.rs:441`:
    ```rust
    let _ = tokio::process::Command::new("taskset")
        .args(["-cp", &cpu_list, &std::process::id().to_string()])
        .output()
        .await;
    ```
    If `taskset` is unavailable on the target Linux system, or if permission to change CPU affinity is denied, the error is discarded. Performance degradation on NUMA boundaries occurs silently with no indicator in system logs.

---

## 3. PII & Secrets Leakage Analysis

### 3.1 Raw Execution Error Logging
During agent execution, workflow step failures, or JSON-RPC tool calls, error strings are printed directly to the system log at `warn!` level.
*   `crates/op-cache/src/grpc/mcp_service.rs:126`:
    ```rust
    warn!("MCP tools/call failed: agent={} error={}", tool_call.name, result.error);
    ```
*   `crates/op-cache/src/grpc/agent_service.rs:265`:
    ```rust
    warn!("Agent {} execution failed: {}", agent_id, e);
    ```
*   `crates/op-cache/src/workflow_executor.rs:394`:
    ```rust
    warn!("Step {} ({}) failed (attempt {}/{}): {}", step_index, agent_id, attempt + 1, max_attempts, e);
    ```
**Risk**: If an agent fails due to an unauthorized API attempt, database connection failure, or credential rejection, the raw error payload `e` or `result.error` will contain connection strings, credentials, authorization tokens, or internal database schemas.

### 3.2 Hostname and Snapshot Metadata Disclosure
*   `crates/op-cache/src/btrfs_cache.rs:598`: `info!("Streaming cache snapshot to {}:{}", remote_host, remote_path);`
*   `crates/op-cache/src/btrfs_cache.rs:631`: `info!("Receiving cache snapshot from {}:{}", remote_host, remote_snapshot);`
**Risk**: If `remote_host` includes embedded credential syntax (e.g., `ssh://username:password@host`), secrets are printed directly to system logs in cleartext.

---

## 4. Metrics Instrumentation Status

Although `prometheus` is defined as a workspace-level dependency in `Cargo.toml`, there is **no direct metrics instrumentation** (such as standard counters, histograms, or gauges) registered within the `op-cache` crate. 

Instead, `op-cache` relies on static statistics structures containing `AtomicU64` counters or SQLite `SUM` queries:
*   `crates/op-cache/src/grpc/cache_service.rs:56`: Uses `AtomicU64` to maintain `total_hits` and `total_misses` internally.
*   `crates/op-cache/src/btrfs_cache.rs:444`: Computes cache hit/miss and sizes dynamically by querying SQLite table records on demand.

**Recommendation**: Replace passive statistical polling with active, standard prometheus registries. Register a thread-safe static collector for cache hits, misses, compression latency, and NUMA node locality metrics.

---

## 5. Schema-as-Code Compliance

This codebase exhibits a hybrid, non-unified approach to schemas, breaking the *Schema-as-Code* discipline by maintaining dual, parallel representations for identical structures.

### 5.1 Ad-Hoc Serde Structs Duplicating Protobuf Contracts
*   `crates/op-cache/src/agent_registry.rs:71`: `AgentDefinition` is declared as an ad-hoc Serde serialization target:
    ```rust
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AgentDefinition {
        pub id: String,
        pub name: String,
        pub description: String,
        pub capabilities: Vec<AgentCapability>,
        ...
    }
    ```
    This struct mirrors the generated protobuf message contract `super::proto::Agent` defined via gRPC. Maintaining both structures forces manual field-by-field translation mapping across architectural layers, leading to drift.

### 5.2 Ad-Hoc Serialization of Data Structures in Database
*   `crates/op-cache/src/workflow_tracker.rs:133`:
    ```rust
    let agent_sequence_json = simd_json::to_string(agents)?;
    ```
    Rather than storing schema-versioned Protocol Buffers or FlatBuffers arrays in the database, the agent execution path relies on serializing raw string vectors directly into SQL text columns (`agent_sequence TEXT NOT NULL`). Any change to the structure of `AgentDefinition` or the naming convention of `AgentCapability` will render existing records in SQLite unparseable.

### 5.3 Hardcoded Protocol Strings
*   `crates/op-cache/src/grpc/mcp_service.rs:257-303`: Formats such as `McpToolsListResult`, `McpInitializeResult`, and `McpToolJson` are defined inside the source code as ad-hoc, hardcoded JSON maps. These lack a unified versioned schema specification, departing from formal Model Context Protocol standards.

---

## 6. Security Vulnerabilities

### Critical: Arbitrary Command Injection via Shell Interpolation
*   **File**: `crates/op-cache/src/btrfs_cache.rs`
*   **Lines**: 599–604, 635–639
*   **Vulnerability Type**: OS Command Injection (CWE-78)
*   **Severity**: **Critical** (Directly exploitable)

```rust
// btrfs_cache.rs:599-604
let cmd = format!(
    "btrfs send {} | ssh {} 'btrfs receive {}'",
    snapshot_path.display(),
    remote_host,
    remote_path
);

let output = tokio::process::Command::new("bash")
    .arg("-c")
    .arg(&cmd)
    .output()
    .await
    ...
```

```rust
// btrfs_cache.rs:635-639
let cmd = format!(
    "ssh {} 'btrfs send {}' | btrfs receive {}",
    remote_host,
    remote_snapshot,
    local_path
);

let output = tokio::process::Command::new("bash")
    .arg("-c")
    .arg(&cmd)
    .output()
    .await
    ...
```

#### Attack Vector & Impact
The `remote_host`, `remote_path`, `remote_snapshot`, and `local_path` values are accepted as raw, unsanitized string slices. They are directly formatted into a shell string sequence and executed under system privileges using `bash -c`.

An attacker who has access to trigger snapshot streaming (via gRPC configuration interfaces or database configuration manipulation) can pass a malicious payload as the `remote_host` parameter. 

For instance, providing a hostname value such as:
```text
"localhost 'btrfs receive /dev/null'; rm -f /etc/shadow; #"
```
Results in the shell executing the following command sequence:
```bash
btrfs send <path> | ssh localhost 'btrfs receive /dev/null'; rm -f /etc/shadow; # 'btrfs receive <path>'
```
This bypasses ssh authentication boundaries entirely, resulting in arbitrary execution of system-level shell commands on the local machine with the permission set of the active control plane process.

#### Remediation
1.  **Do not execute shell processes (`bash -c`)**.
2.  Invoke binary commands directly via `tokio::process::Command` argument arrays.
3.  Use Rust process redirection to bridge the pipe safely between `btrfs send` and `ssh`:

```rust
use std::process::Stdio;
use tokio::process::Command;

// Safe execution example avoiding shell context
let mut send_child = Command::new("btrfs")
    .args(["send", &snapshot_path.to_string_lossy()])
    .stdout(Stdio::piped())
    .spawn()?;

let send_stdout = send_child.stdout.take().unwrap();

let mut ssh_child = Command::new("ssh")
    .args([remote_host, "btrfs", "receive", remote_path])
    .stdin(send_stdout)
    .spawn()?;

let _ = tokio::try_join!(send_child.wait(), ssh_child.wait())?;
```