# Product Security and Quality Audit: `op-cache`

## 1. Executive Summary

This production security and quality audit evaluates the `op-cache` crate, which provides BTRFS-backed caching with NUMA optimization and agent orchestration. The audit focuses on identifying exploitable security vulnerabilities, concurrency performance hazards, schema-as-code violations, and memory map details.

### Crucial Findings
1. **Critical Shell Command Injection**: Parameter formatting directly into a `bash -c` shell invocation without sanitization allows remote command execution.
2. **Undefined Behavior in `simd_json`**: Direct use of `simd_json` parsing functions on standard unpadded strings and slices violates strict memory alignment and padding requirements, risking segmentation faults or memory corruption.
3. **Async Concurrency Hazards**: Recursive directory traversal and synchronous file reads are executed on active Tokio worker threads, leading to potential executor thread starvation.
4. **Schema-as-Code Deficiencies**: Data contracts for orchestration, workflows, and Model Context Protocol (MCP) interactions are expressed as ad-hoc Rust structs rather than versioned, centralized Protobuf schemas.

---

## 2. Critical Security Findings

### 2.1. Critical: Shell Command Injection in BTRFS Remote Sync
* **Location**: `crates/op-cache/src/btrfs_cache.rs:610-621` and `crates/op-cache/src/btrfs_cache.rs:645-654`

#### Mechanism
The cache synchronization methods `stream_to_remote` and `receive_from_remote` execute shell commands using `tokio::process::Command::new("bash")` with `-c`. The arguments—including `remote_host`, `remote_path`, and `remote_snapshot`—are formatted as raw strings directly into the shell script:

```rust
// crates/op-cache/src/btrfs_cache.rs:610-615
let cmd = format!(
    "btrfs send {} | ssh {} 'btrfs receive {}'",
    snapshot_path.display(),
    remote_host,
    remote_path
);
```

#### Exploitability & Impact
Because there is no validation or shell-escaping on `remote_host` or `remote_path`, any control path that passes untrusted user input to these methods will allow an attacker to inject shell metacharacters (e.g., `;`, `&&`, `|`, backticks, or `$()`). 
For instance, a value of `remote_host = "target-node; rm -rf / #"` will result in the immediate execution of `rm -rf /` on the host system. This is a highly critical, directly exploitable Remote Code Execution (RCE) vulnerability.

#### Remediation
Avoid invoking shell processes (`bash -c`) entirely. Execute commands directly with `Command::new("ssh")` and pass parameters as safe, separate elements in an argument vector:
```rust
let output = tokio::process::Command::new("ssh")
    .args([remote_host, "btrfs", "receive", remote_path])
    // ...
```

---

### 2.2. High: Undefined Behavior via Unpadded `simd_json` Buffer Deserialization
* **Location**: 
  - `crates/op-cache/src/grpc/mcp_service.rs:52`
  - `crates/op-cache/src/grpc/mcp_service.rs:147`
  - `crates/op-cache/src/pattern_tracker.rs:258`
  - `crates/op-cache/src/workflow_tracker.rs:290`
  - `crates/op-cache/src/workflow_tracker.rs:337`
  - `crates/op-cache/src/workflow_tracker.rs:364`

#### Mechanism
`simd-json` utilizes advanced vector processing instructions (AVX2/NEON) to parse JSON in-place. Because of this, it has strict buffer requirements: the input slice must be mutable and possess trailing padding bytes equivalent to `simd_json::PADDING` (typically 32 bytes) beyond the payload length to prevent reading out-of-bounds.

The codebase violates this contract in multiple places by performing in-place parsing on standard, unpadded buffers:
1. **gRPC MCP JSON-RPC**:
   ```rust
   // crates/op-cache/src/grpc/mcp_service.rs:51-52
   let mut params_buf = params.to_vec();
   let parsed: Result<ToolCallParams, _> = simd_json::from_slice(&mut params_buf);
   ```
   `params.to_vec()` creates a standard `Vec<u8>` without trailing padding.
2. **Database String Deserialization**:
   ```rust
   // crates/op-cache/src/pattern_tracker.rs:258
   let mut agent_sequence_json: String = row.get(1)?;
   let agent_sequence: Vec<String> =
       unsafe { simd_json::from_str(&mut agent_sequence_json) }
           .unwrap_or_default();
   ```
   Retrieving a `String` directly from `rusqlite` does not provide trailing padding. Passing it to `simd_json::from_str` yields undefined behavior.

#### Impact
Under stress or with malformed payloads, `simd-json` will perform out-of-bounds reads on these unpadded allocations. This leads to sporadic segmentation faults, memory corruption, or information disclosure (leaking adjacent heap memory into parsed fields).

#### Remediation
Ensure that buffers are padded prior to parsing, or use `simd_json::to_padded_bin` to allocate the safe variant. Alternatively, fallback to standard `serde_json` for fields originating from SQLite or unpadded network slices.

---

## 3. Schema-as-Code Violations

The codebase maintains a hybrid approach that violates the "schema-as-code" discipline. While some services use versioned protobuf definitions, several critical system-wide data contracts are written as ad-hoc Rust structures with serialization traits, rendering them difficult to version, govern, or integrate with OSCAL compliance frameworks.

| Ad-hoc Data Contract | File & Location | Issue Description |
| :--- | :--- | :--- |
| `AgentCapability` | `crates/op-cache/src/agent_registry.rs:18` | Defines capability types as an ad-hoc Rust enum. Parsing from strings is performed manually via a match statement. Should be codified in a versioned protobuf enum. |
| `AgentDefinition` | `crates/op-cache/src/agent_registry.rs:82` | Core agent definition metadata metadata is duplicated as an ad-hoc struct, conflicting with the gRPC versioned protobuf schema `super::proto::Agent`. |
| `Mcp JSON-RPC Structures` | `crates/op-cache/src/grpc/mcp_service.rs:201-255` | Internal structures (`ToolCallParams`, `McpContentResponse`, `McpToolsListResult`, `McpInitializeResult`, etc.) represent the JSON-RPC protocol as ad-hoc types. Changes can silently break external clients. |
| `OrchestrationResult` / `StepResult` | `crates/op-cache/src/orchestrator.rs:45-64` | Pipeline execution metadata and intermediate step states are defined locally. Breaks standardization across services. |
| `WorkflowPattern` / `PromotionSuggestion` | `crates/op-cache/src/workflow_tracker.rs:51-75` | Pattern tracking structure definitions are ad-hoc. Restricts sharing trace logs with external analysis tools. |
| `PromotedWorkflow` | `crates/op-cache/src/workflow_tracker.rs:484-493` | Workflows promoted dynamically represent a high-value state, but lack a formal, versioned schema definition. |

---

## 4. Performance & Allocation Analysis

### 4.1. Vector and String Allocations in Loops (No Pre-allocation)
1. **Capabilities Iteration**:
   * **Location**: `crates/op-cache/src/grpc/orchestrator_service.rs:92`
   * **Issue**: Within `resolve_capabilities`, a `vec![cap]` is allocated on each iteration of the loop over required capabilities. This generates redundant heap allocations in the hot request resolution path.
2. **Parallel Group Instantiation**:
   * **Location**: `crates/op-cache/src/capability_resolver.rs:301`
   * **Issue**: The loop in `build_parallel_groups` continuously initializes a fresh vector via `current_group = Vec::new()` without utilizing capacity pre-estimation, triggering progressive array reallocations as elements are appended.
3. **Agent ID Clones**:
   * **Location**: `crates/op-cache/src/agent_registry.rs:386`
   * **Issue**: `seen.insert(agent.id.clone())` allocates a new `String` for every discovered agent capability match within nested loops.

### 4.2. Unnecessary `format!` in Hot Paths
* **Workstack Hash Generation**:
  * **Location**: `crates/op-cache/src/grpc/orchestrator_service.rs:143` (calling `Self::hash_bytes`)
  * **Issue**: In the step loop of `execute_workstack`, `hash_bytes` is called on each iteration. This internally executes `format!("{:x}", hasher.finalize())`, allocating a new string.
* **Sequence Formatting**:
  * **Location**: `crates/op-cache/src/orchestrator.rs:395`
  * **Issue**: `hash_sequence` allocates multiple strings per request path via `agents.join("→")` and `format!("{:x}", ...)`.
* **Path Strings**:
  * **Location**: `crates/op-cache/src/capability_resolver.rs:161`
  * **Issue**: `resolution_path.push(format!("select:{}->{}", cap.name(), agent.id))` executes string formatting inside a hot greed-matching loop.

### 4.3. Concurrency Blocking I/O on Tokio Threads
Tokio's cooperative scheduler relies on tasks yielding rapidly. Executing synchronous file-system blocking calls on the Tokio worker thread starves the executor, reducing throughput and leading to high tail latency.

1. **Recursive Directory Sizing**:
   * **Location**: `crates/op-cache/src/btrfs_cache.rs:440` (called by `stats()` at `btrfs_cache.rs:396`)
   * **Issue**: `dir_size()` is a recursive folder scanner that uses synchronous, blocking `std::fs::read_dir`. When statistics are fetched via gRPC, this blocking call executes directly on the Tokio thread.
2. **Synchronous Cache Reads**:
   * **Location**: `crates/op-cache/src/workflow_cache.rs:147` (called by `WorkflowExecutor::execute_workflow` at `workflow_executor.rs:224`)
   * **Issue**: The asynchronous executor retrieves workflow cached steps by executing `std::fs::read` inside `WorkflowCache::get` synchronously, halting the executor thread.
3. **Synchronous Workstack Cache Reads**:
   * **Location**: `crates/op-cache/src/workstack_cache.rs:125` (called by `Orchestrator` at `orchestrator.rs:334`)
   * **Issue**: Standard synchronous `std::fs::read` is invoked directly from the async task context in the orchestrator pipeline.

---

## 5. Memory Map Table

The codebase does not directly initialize `memmap2` or standard `mmap` references within the provided crate files. However, the workspace leverages `cozo` with the `storage-sled` backend enabled (`Cargo.toml`). `sled` inherently memory-maps its database files. 

### Sled Concurrency & Mount Considerations
Because `sled` relies on internal `mmap` calls for index and log segments, opening a database located on a `tmpfs` or `noexec` partition introduces critical runtime operational risks:
1. **`tmpfs` Double-Caching**: Running a memory-mapped database on a RAM-backed filesystem (`tmpfs`) results in double-caching (data resides in the filesystem RAM allocation and again in the OS page cache/program memory map), significantly wasting physical RAM and increasing OOM crash likelihood.
2. **`noexec` Failures**: Certain hardened Linux environments block execution of pages marked writable-and-executable or restrict memory mapping altogether on `noexec` mounts. If `sled` tries to map writable memory segments under strict LSM rule sets on a `noexec` path, database initialization will fail immediately.

### Memory Map & Allocation Registry

| Site | file:line | Type | Risk |
| :--- | :--- | :--- | :--- |
| `cozo` Database / `sled` | `Cargo.toml` (Workspace dependency) | `sled` (Internal mmap) | **Medium**: Potential memory bloat on `tmpfs` mounts; initialization failures on hardened `noexec` mount points. |
| Cache Sizing | `crates/op-cache/src/btrfs_cache.rs:440` | Heap Allocation (Dynamic) | **Low**: Reads full contents of directory entries synchronously into memory vectors. |
| Embedding Vector Load | `crates/op-cache/src/btrfs_cache.rs:316` | Heap Allocation (Dynamic) | **Low**: `std::fs::read` fully allocates vector space matching vector database file size. |
| Workflow Cache Load | `crates/op-cache/src/workflow_cache.rs:147` | Heap Allocation (Dynamic) | **Low**: Dynamic buffer allocation on read. Potential heap fragmentation if cached steps are exceptionally large. |
| Workstack Cache Load | `crates/op-cache/src/workstack_cache.rs:125` | Heap Allocation (Dynamic) | **Low**: Dynamic buffer allocation proportional to step response size. |

---

## 6. Detailed Code Quality Findings

### 6.1. Permissive Snapshot Directory Permissions
* **Location**: `crates/op-cache/src/snapshot_manager.rs:47`

#### Issue
When initializing the snapshot path, directory structures are created with standard permissive creations:
```rust
tokio::fs::create_dir_all(&self.config.snapshot_dir).await?;
```
Because BTRFS cache snapshots contain highly sensitive security audits, proprietary source code analysis, and query traces, these directories must not be world-readable.

#### Remediation
Explicitly enforce restricted directory permissions (`0700` / read-write-execute only by owner) using `std::os::unix::fs::DirBuilderExt` or `libc::umask` on Unix systems.

### 6.2. Lock Contention on SQLite DB Connections
* **Location**: `crates/op-cache/src/pattern_tracker.rs:125` & `crates/op-cache/src/workflow_tracker.rs:136`

#### Issue
The SQLite database connection is protected via `Mutex<rusqlite::Connection>`. Under high concurrent requests through the gRPC server, multiple worker threads attempting to log agent execution paths will encounter serialized database locks. This forms a bottleneck at the SQLite connection mutex, negating the throughput benefits of the async scheduler.

#### Remediation
Utilize a connection pool (e.g., `r2d2` or `sqlx` with sqlite) or leverage SQLite in WAL mode (`Write-Ahead Logging`) with thread-safe connection caching to permit concurrent reads and non-blocking writes.

### 6.3. Lack of Compression-Bomb Protections
* **Location**: `crates/op-cache/src/workflow_cache.rs:373` & `crates/op-cache/src/workstack_cache.rs:376`

#### Issue
Zstd decompression is performed directly on cached buffers using:
```rust
zstd::decode_all(std::io::Cursor::new(data))
```
If an attacker manages to write a malformed payload or a highly-repetitive cache pattern to disk, decompression can expand a tiny file into gigabytes of memory, causing a Denial of Service (DoS) crash via Out-Of-Memory (OOM).

#### Remediation
Use a streaming decoder wrapper that tracks and limits the maximum decompressed size (e.g., stopping decompression if output exceeds a safety factor of 5x original size or a hard limit of 10MB).