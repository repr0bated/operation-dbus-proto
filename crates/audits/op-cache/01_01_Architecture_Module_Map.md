# OP-CACHE: SECURITY & QUALITY AUDIT

---

## 1. Architecture & Module Map

### Overview
`op-cache` is an enterprise-grade caching, orchestration, and pattern-tracking engine designed to optimize control planes on multi-socket Linux systems. It features native integration with the BTRFS filesystem for subvolume management and snapshot-based replication, NUMA-aware scheduling for optimized memory and L3 cache access, a flexible capabilities-based agent router, and multiple caching tiers (workstack caching, workflow caching, and embedding caching) backed by SQLite metadata databases.

```
                                  [ gRPC Clients ]
                                         │
                                         ▼
   ┌──────────────────────────────────────────────────────────────────────────┐
   │                            gRPC Entry Points                             │
   │  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────────────┐  │
   │  │  AgentService    │  │   CacheService   │  │  OrchestratorService   │  │
   │  └────────┬─────────┘  └────────┬─────────┘  └───────────┬────────────┘  │
   └───────────┼─────────────────────┼────────────────────────┼───────────────┘
               │                     │                        │
               ▼                     ▼                        ▼
   ┌───────────────────┐     ┌───────────────────┐  ┌─────────────────────────┐
   │   AgentRegistry   │     │  WorkstackCache   │  │      Orchestrator       │
   │                   │     │  WorkflowCache    │  │                         │
   └───────────────────┘     └────────┬──────────┘  └─────────┬───────────────┘
                                      │                       │
                                      ▼                       ▼
                            ┌───────────────────┐  ┌─────────────────────────┐
                            │    BtrfsCache     │  │   CapabilityResolver    │
                            │                   │  │                         │
                            └─────────┬─────────┘  └─────────────────────────┘
                                      │
                                      ▼
                            ┌───────────────────┐
                            │   NumaTopology    │
                            │   (Sysfs / CPU)   │
                            └───────────────────┘
```

### Module Tree
The crate is organized into modular components under `crates/op-cache/src/`:
*   `lib.rs`: The crate root, consolidating re-exports, establishing the prelude, and compiling the generated protobuf definitions.
*   `agent.rs`: Exposes public type aliases for capability definitions and agent configurations.
*   `agent_registry.rs`: Implements thread-safe registration of agents, managing available capabilities, execution priorities, and executor callbacks.
*   `btrfs_cache.rs`: Orchestrates file caching on BTRFS subvolumes, handling raw blocks, embedding vectors, sqlite indices, and remote snapshot replication.
*   `capability_resolver.rs`: Implements greedy resolution of capabilities to determine optimal sequential or parallel execution pathways.
*   `numa.rs`: Parses sysfs configuration (`/sys/devices/system/node/`) to build a comprehensive multi-socket affinity map.
*   `orchestrator.rs`: Serves as the primary coordinator, routing execution requests through single agents or workstacks with automated caching.
*   `pattern_tracker.rs`: Uses SQLite to analyze frequently repeated sequence patterns for promotion suggestions.
*   `snapshot_manager.rs`: Implements rotational backup rules on BTRFS subvolumes.
*   `workflow_cache.rs`: Manages TTL-based, compressed execution state maps for multi-step tasks.
*   `workflow_executor.rs`: A pipeline execution engine applying topological affinity to multi-agent pipelines.
*   `workflow_tracker.rs`: Implements sliding-window analysis of active sessions to capture emerging compound workflows.
*   `workstack_cache.rs`: Caches intermediate outputs of complex workstacks to prevent redundant computation.
*   `grpc/`:
    *   `mod.rs`: Consolidates and re-exports gRPC service definitions.
    *   `agent_service.rs`: Handles gRPC registration and direct/streamed invocation of agents.
    *   `cache_service.rs`: Implements gRPC endpoints for step caching and cleanup.
    *   `mcp_service.rs`: Bridges Model Context Protocol (MCP) JSON-RPC requests onto internal agents.
    *   `orchestrator_service.rs`: Manages high-level request routing and workflow pattern mining.
    *   `server.rs`: Configures and hosts the unified Tonic gRPC server stack.

### Entry Points
*   **Library Entry Point**: `crates/op-cache/src/lib.rs`
*   **gRPC Server Entry Point**: `crates/op-cache/src/grpc/server.rs`
*   **Service Builders**: Instantiated within individual gRPC implementation files under `crates/op-cache/src/grpc/`.

---

## 2. Production Security & Quality Audit

### CRITICAL: Remote Command Injection via Shell Interpolation
*   **File**: `crates/op-cache/src/btrfs_cache.rs`
*   **Lines**: 604-640 (in `stream_to_remote`) and 642-678 (in `receive_from_remote`)

#### Description
The `stream_to_remote` and `receive_from_remote` methods execute external shell commands by formatting user-controlled string inputs (`remote_host`, `remote_path`, and `remote_snapshot`) directly into a command string, which is then passed to `tokio::process::Command::new("bash").arg("-c").arg(&cmd)`.

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
    ...
```

An attacker capable of manipulating these string arguments (e.g., via gRPC APIs, configuration injection, or system metadata updates) can inject arbitrary shell commands (for example, setting `remote_host` to `attacker-host; rm -rf /;`). Because BTRFS commands are typically executed as a privileged user (often `root` due to subvolume privileges), this allows arbitrary command execution with high privilege.

#### Remediation
Avoid invoking a shell runner (`bash -c`) entirely. Instead, use `tokio::process::Command` to invoke the native binary with arguments structured as independent elements, or separate the command pipeline into distinct child processes in Rust, chaining stdout to stdin using `std::process::Stdio::piped()`.

```rust
// Safer implementation using structured process pipelines
let mut send_child = tokio::process::Command::new("btrfs")
    .args(["send"])
    .arg(snapshot_path)
    .stdout(std::process::Stdio::piped())
    .spawn()?;

let mut ssh_child = tokio::process::Command::new("ssh")
    .arg(remote_host) // Remote host validation is still necessary to prevent flag injection
    .arg(format!("btrfs receive {}", remote_path)) // remote_path must be strictly validated
    .stdin(send_child.stdout.take().unwrap())
    .spawn()?;
```

---

### CRITICAL: Out-of-Bounds Memory Corruption via Unsafe `simd-json` Parsing of Unpadded DB Strings
*   **File**: `crates/op-cache/src/pattern_tracker.rs` (Lines 232-234)
*   **File**: `crates/op-cache/src/workflow_tracker.rs` (Lines 369-371, 410-412, and 437-439)

#### Description
Throughout `pattern_tracker.rs` and `workflow_tracker.rs`, serialized JSON strings are retrieved from SQLite using `rusqlite` and subsequently deserialized using the unsafe interface `simd_json::from_str`:

```rust
let mut agent_sequence_json: String = row.get(1)?;
let agent_sequence: Vec<String> =
    unsafe { simd_json::from_str(&mut agent_sequence_json) }
        .unwrap_or_default();
```

`simd-json` requires that input buffers are padded with a minimum of 16 or 32 bytes (depending on the target vector extension width) of scratch memory. Standard Rust `String` allocations retrieved from `rusqlite` row entries do *not* guarantee this padding structure. 

Passing an unpadded mutable `String` buffer directly to `simd_json::from_str` can result in out-of-bounds reads or writes during SIMD vector load and alignment operations. If the parsed string is located near page boundaries, this will trigger a Segmentation Fault (DoS) or corrupt surrounding heap memory.

#### Remediation
If high performance is required, clone the string into a padded buffer structure (e.g., `simd_json::to_padded_bin` or a manually zero-padded `Vec<u8>`) before parsing. Alternatively, use standard, safe `serde_json` deserialization for configuration and DB parsing paths, as these operations are not CPU bottlenecks.

```rust
// Safe parsing implementation
let agent_sequence: Vec<String> = serde_json::from_str(&agent_sequence_json)
    .unwrap_or_default();
```

---

### HIGH: Denial of Service via Long-Lived Read Lock Over Async Yield Points
*   **File**: `crates/op-cache/src/grpc/agent_service.rs`
*   **Lines**: 232-263 (inside `execute_stream`'s spawned task)

#### Description
Within the spawned tokio task for `execute_stream`, a read lock on the agent registry is acquired and held continuously over the entire execution of `executor` and the subsequent asynchronous streaming loop:

```rust
tokio::spawn(async move {
    let agents_guard = agents.read().await; // Lock acquired
    if let Some(agent) = agents_guard.get(&agent_id) {
        if let Some(executor) = &agent.executor {
            match executor(&input) {
                Ok(output) => {
                    let chunk_size = 64 * 1024;
                    for (sequence, chunk) in output.chunks(chunk_size).enumerate() {
                        ...
                        let _ = tx
                            .send(Ok(ExecuteAgentChunk { ... }))
                            .await; // Yield point while lock is held!
                    }
                }
            }
        }
    }
}); // Lock released only when the spawned task terminates
```

Because `tx.send().await` is an async yield point, if the client is slow to consume the stream (inducing TCP backpressure), this spawned task will yield while maintaining the read guard on the registry. 

During this time, any thread requesting a write lock (e.g., to register a new agent or modify active configurations) will block. This completely starves the write queue of the `AgentRegistry`, causing a total control plane deadlock (DoS).

#### Remediation
Minimize the critical section of the lock. Clone the executor callback (wrapped in an `Arc`) inside a tight block, and drop the read guard immediately before executing the callback and starting the async stream transmission loop.

```rust
// Remediation: Drop the lock before executing the callback and yielding
let executor = {
    let agents_guard = agents.read().await;
    agents_guard.get(&agent_id)
        .and_then(|a| a.executor.clone())
};

if let Some(executor) = executor {
    match executor(&input) {
        Ok(output) => {
            let chunk_size = 64 * 1024;
            for (sequence, chunk) in output.chunks(chunk_size).enumerate() {
                let _ = tx.send(Ok(ExecuteAgentChunk { ... })).await; // Safe yield point
            }
        }
        ...
    }
}
```

---

### MEDIUM: Local Privilege Escalation / Information Disclosure via Weak Cache Permissions
*   **File**: `crates/op-cache/src/btrfs_cache.rs` (Lines 91-105)
*   **File**: `crates/op-cache/src/workflow_cache.rs` (Lines 64-70)
*   **File**: `crates/op-cache/src/pattern_tracker.rs` (Lines 54-58)
*   **File**: `crates/op-cache/src/workstack_cache.rs` (Lines 46-50)

#### Description
Directories such as the SQLite database parent paths and file cache volumes are instantiated using `tokio::fs::create_dir_all`. 

On Unix-based operating systems, `create_dir_all` creates folders using default system umask parameters (typically resulting in permissions like `0755` or `0777`). Because these cache folders contain sensitive pipeline artifacts, database indices, and potentially proprietary training embeddings, world-readable or group-readable permissions allow local unprivileged users to read and manipulate sensitive assets, leading to cache poisoning or local data leakage.

#### Remediation
Set explicit permissions (`0700`) during directory creation using Unix-specific filesystem extension traits.

```rust
use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;

let mut builder = DirBuilder::new();
builder.recursive(true);
builder.mode(0o700); // Strict read/write/execute for owner only
builder.create(&data_dir)?;
```

---

### MEDIUM: Thread Starvation via Sync DB Blocking Operations on Async Executors
*   **File**: `crates/op-cache/src/btrfs_cache.rs` (Line 48)
*   **File**: `crates/op-cache/src/workflow_cache.rs` (Line 59)
*   **File**: `crates/op-cache/src/workstack_cache.rs` (Line 43)
*   **File**: `crates/op-cache/src/pattern_tracker.rs` (Line 48)

#### Description
The cache managers and tracker implementations store persistent state in local SQLite databases, protecting the connections using synchronous lock primitives (`std::sync::Mutex<rusqlite::Connection>`). 

Executing synchronous SQLite queries and write transactions inside async tasks locks the mutex and blocks the underlying Tokio worker thread. Since database operations require physical disk I/O, a slow database query or a write transaction under heavy load will block the thread executor, degrading concurrent throughput and risking thread starvation.

#### Remediation
Wrap database accesses in `tokio::task::spawn_blocking` blocks to offload execution to Tokio's dedicated blocking pool, or migrate key databases to async-native pools (such as `sqlx`, which is already declared in the workspace).

```rust
// Example using spawn_blocking to safeguard the executor thread
let db = self.db.clone();
let old_entries = tokio::task::spawn_blocking(move || {
    let index = db.lock().unwrap();
    let mut stmt = index.prepare("SELECT ...")?;
    // Execute and return data
}).await??;
```

---

### MEDIUM: Diagnostic Information Leakage in gRPC Error Responses
*   **File**: `crates/op-cache/src/grpc/agent_service.rs` (Line 147)
*   **File**: `crates/op-cache/src/grpc/orchestrator_service.rs` (Lines 178 and 369)

#### Description
Error messages from underlying callback failures, BTRFS pipeline exceptions, or SQLite error states are formatted directly into Tonic `Status::internal` messages and returned over the wire to remote clients. These messages leak internal details, including internal database structures, exact module pathways, file hierarchies, and execution context.

#### Remediation
Log the exact diagnostic stack trace locally using `tracing::error!` and return a generic error status (coupled with a unique tracking identifier) to the calling gRPC client.

---

### LOW: Weak Temporary Directory Path Resolution in Test Suites
*   **File**: `crates/op-cache/src/btrfs_cache.rs`
*   **Lines**: 779-781

#### Description
The unit test `test_text_hashing` instantiates `BtrfsCache` on a static path `/tmp/test-cache`. This introduces potential test failures on shared development machines when multiple users run tests simultaneously, and it is vulnerable to symlink hijacking attacks (where an attacker links the fixed location to target system files, causing the runner to modify or truncate them).

#### Remediation
Leverage the `tempfile` library (already specified as a dev-dependency) to instantiate a dynamic, isolated workspace for every test execution loop.

```rust
let temp_dir = tempfile::TempDir::new().unwrap();
let cache = BtrfsCache::new(temp_dir.path().to_path_buf()).await.unwrap();
```

---

## 3. Schema-as-Code Compliance Review

The codebase implements a schema-as-code discipline using Protocol Buffers and Tonic to auto-generate gRPC client/server contracts. However, several critical boundaries violate this paradigm:

### 1. Ad-Hoc Model Context Protocol (MCP) JSON Serialization
*   **File**: `crates/op-cache/src/grpc/mcp_service.rs` (Lines 352-411)

#### Description
The MCP interface declares a series of ad-hoc JSON parsing structs (`ToolCallParams`, `McpContentResponse`, `McpContent`, `McpToolsListResult`, `McpToolJson`, `McpInitializeResult`, etc.) directly inside the service code. These structures model external API contracts but bypass versioned schema definitions. Any modifications to these contracts require modifying code internals rather than updating version-controlled models.

#### Remediation
Incorporate the MCP API specifications into version-controlled Protobuf schemas (`.proto`) or a central JSON Schema workspace document, and compile them to native Rust types via automated build pipelines.

---

### 2. Dual Definition of Agent Entities
*   **File**: `crates/op-cache/src/agent_registry.rs` (Lines 101-143)
*   **File**: `crates/op-cache/src/grpc/agent_service.rs` (Generated from `.proto`)

#### Description
There is a duplicate definition pattern: the core engine relies on `AgentDefinition` for configuration parsing, while gRPC boundaries translate these to and from the generated protobuf-defined type `super::proto::Agent`. This duplication results in translation drift risk and maintenance overhead.

#### Remediation
Unify on a single, schema-defined structure. Declare the entity definitions inside the `.proto` schemas and decorate the generated types with custom attribute serialization targets (e.g., using `prost` field annotations) to directly support internal engine execution pathways.