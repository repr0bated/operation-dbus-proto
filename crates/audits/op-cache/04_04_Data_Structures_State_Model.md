# Production Security and Quality Audit: op-cache

---

## 1. Data Structures Audit

### 1.1 Synchronization, Interior Mutability, and Cell Counts

The following table tracks the occurrences of concurrency, reference counting, and cell primitives across all audited files.

| File Path | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell` |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| `crates/op-cache/src/agent.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cache/src/agent_registry.rs` | 3 | 0 | 0 | 3 | 0 | 0 |
| `crates/op-cache/src/btrfs_cache.rs` | 0 | 0 | 0 | 0 | 2 | 0 |
| `crates/op-cache/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cache/src/orchestrator.rs` | 4 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cache/src/pattern_tracker.rs` | 0 | 0 | 0 | 0 | 1 | 0 |
| `crates/op-cache/src/snapshot_manager.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cache/src/workflow_cache.rs` | 0 | 0 | 0 | 0 | 1 | 0 |
| `crates/op-cache/src/workflow_executor.rs` | 6 | 0 | 0 | 2 | 0 | 0 |
| `crates/op-cache/src/workflow_tracker.rs` | 0 | 0 | 0 | 0 | 2 | 0 |
| `crates/op-cache/src/workstack_cache.rs` | 0 | 0 | 0 | 0 | 1 | 0 |
| `crates/op-cache/src/capability_resolver.rs` | 2 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cache/src/numa.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cache/src/grpc/agent_service.rs` | 6 | 0 | 0 | 2 | 0 | 0 |
| `crates/op-cache/src/grpc/cache_service.rs` | 3 | 0 | 0 | 2 | 0 | 0 |
| `crates/op-cache/src/grpc/mcp_service.rs` | 3 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cache/src/grpc/mod.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-cache/src/grpc/orchestrator_service.rs` | 4 | 0 | 0 | 1 | 0 | 0 |
| `crates/op-cache/src/grpc/server.rs` | 5 | 0 | 0 | 0 | 0 | 0 |

---

### 1.2 Cloning Evaluation (High-Clone Files)

*   **`crates/op-cache/src/grpc/orchestrator_service.rs`**: **26 `.clone()` calls** (Flagged: Count > 20).
    Many clone calls in this file are used to duplicate generated protobuf and metadata structs (such as `agent_id`, `p.pattern_id`, `p.agent_sequence`, `req.input`, and internal service pointers) during async task spawning and routing. This indicates excessive allocations inside the hot execution path. Consider passing `Arc<str>` or borrowing using lifetime bounds where possible.

---

### 1.3 Large Structs (> 5 Public Fields)

The following public structs exceed the structural threshold of 5 public fields. These should be refactored to use encapsulated builder patterns or nested sub-components.

1.  **`AgentDefinition`** (`crates/op-cache/src/agent_registry.rs:134`):
    Contains **11 public fields** (`id`, `name`, `description`, `capabilities`, `requires`, `priority`, `parallelizable`, `estimated_latency_ms`, `max_input_size`, `version`, `enabled`).
2.  **`CacheStats`** (`crates/op-cache/src/btrfs_cache.rs:514`):
    Contains **6 public fields** (`total_entries`, `hot_entries`, `total_accesses`, `disk_usage_bytes`, `embeddings_size_bytes`, `blocks_size_bytes`).
3.  **`OrchestrationResult`** (`crates/op-cache/src/orchestrator.rs:39`):
    Contains **8 public fields** (`request_id`, `output`, `steps`, `total_latency_ms`, `cache_hits`, `cache_misses`, `used_workstack`, `resolved_agents`).
4.  **`OrchestratorStats`** (`crates/op-cache/src/orchestrator.rs:369`):
    Contains **7 public fields** (`registered_agents`, `enabled_agents`, `available_capabilities`, `tracked_patterns`, `promoted_patterns`, `cache_entries`, `cache_hit_rate`).
5.  **`TrackedPattern`** (`crates/op-cache/src/pattern_tracker.rs:30`):
    Contains **8 public fields** (`pattern_id`, `agent_sequence`, `call_count`, `first_seen`, `last_called`, `avg_latency_ms`, `promoted`, `workstack_id`).
6.  **`CachedStepResult`** (`crates/op-cache/src/workflow_cache.rs:34`):
    Contains **9 public fields** (`workflow_id`, `step_index`, `input_hash`, `output`, `created_at`, `expires_at`, `access_count`, `last_accessed`, `size_bytes`).
7.  **`CacheStats`** (`crates/op-cache/src/workflow_cache.rs:467`):
    Contains **8 public fields** (`total_entries`, `total_size_bytes`, `hot_entries`, `expired_entries`, `total_hits`, `total_misses`, `workflows_cached`, `hit_rate`).
8.  **`WorkflowCacheStats`** (`crates/op-cache/src/workflow_cache.rs:480`):
    Contains **6 public fields** (`workflow_id`, `total_entries`, `total_size_bytes`, `hit_count`, `miss_count`, `hit_rate`).
9.  **`StepResult`** (`crates/op-cache/src/workflow_executor.rs:46`):
    Contains **6 public fields** (`step_index`, `agent_id`, `output`, `latency_ms`, `cached`, `retries`).
10. **`WorkflowResult`** (`crates/op-cache/src/workflow_executor.rs:57`):
    Contains **6 public fields** (`workflow_id`, `steps`, `total_latency_ms`, `cache_hits`, `cache_misses`, `numa_node`).
11. **`ExecutorStats`** (`crates/op-cache/src/workflow_executor.rs:379`):
    Contains **9 public fields** (`registered_agents`, `promoted_workflows`, `pending_promotions`, `total_workflow_executions`, `cache_entries`, `cache_size_bytes`, `cache_hit_rate`, `numa_nodes`, `numa_pinning_enabled`).
12. **`WorkflowPattern`** (`crates/op-cache/src/workflow_tracker.rs:33`):
    Contains **8 public fields** (`pattern_id`, `agent_sequence`, `call_count`, `first_seen`, `last_called`, `avg_latency_ms`, `promoted`, `workflow_id`).
13. **`PromotedWorkflow`** (`crates/op-cache/src/workflow_tracker.rs:424`):
    Contains **7 public fields** (`workflow_id`, `pattern_hash`, `name`, `description`, `agent_sequence`, `created_at`, `execution_count`).
14. **`TrackerStats`** (`crates/op-cache/src/workflow_tracker.rs:436`):
    Contains **6 public fields** (`total_patterns`, `promoted_count`, `pending_promotion`, `total_calls`, `total_workflow_executions`, `promotion_threshold`).
15. **`CacheStats`** (`crates/op-cache/src/workstack_cache.rs:320`):
    Contains **7 public fields** (`total_entries`, `total_size_bytes`, `hot_entries`, `total_hits`, `total_misses`, `workstacks_cached`, `hit_rate`).
16. **`CapabilityRequest`** (`crates/op-cache/src/capability_resolver.rs:12`):
    Contains **6 public fields** (`required_capabilities`, `preferred_agents`, `excluded_agents`, `allow_parallel`, `max_agents`, `input`).
17. **`ResolvedSequence`** (`crates/op-cache/src/capability_resolver.rs:58`):
    Contains **6 public fields** (`agents`, `fulfilled_capabilities`, `missing_capabilities`, `estimated_latency_ms`, `parallel_groups`, `resolution_path`).

---

### 1.4 Globally Mutable State

No globally mutable state (`static mut` or `lazy_static!`) was found in the provided files.

---

## 2. Security Findings & Vulnerabilities

### Finding 1: Critical — gRPC Cache Poisoning and Capability Bypass via Agent-Omitted Workstack IDs
*   **Location**: `crates/op-cache/src/grpc/orchestrator_service.rs:191` and `crates/op-cache/src/grpc/orchestrator_service.rs:391`
*   **Vulnerability Type**: Cache Poisoning / Logic Bypass
*   **Description**:
    When a request is routed through the gRPC `OrchestratorService`, the server constructs a `workstack_id` used for step caching using only the hash of the raw input bytes:
    ```rust
    let workstack_id = format!("ws-{}", &Self::hash_bytes(&req.input)[..12]);
    ```
    This completely omits the sequence of agent IDs assigned to process the step. In multi-tenant environments or systems running diverse workflows, two distinct requests that share the *same input* but require *completely different capability resolutions* (and thus different agent sequences, e.g., `[SecurityAudit, Formatting]` vs. `[ShellExecution]`) will resolve to the exact same `workstack_id`. 
    
    When executing step `0`, the cache lookups will collide:
    ```rust
    let cache_result = cache_service
        .get_step_internal(&workstack_id, step_index as u32, &step_input_hash)
        .await;
    ```
    The cache lookup key is generated from `workstack_id`, `step_index`, and the `input_hash`. Since these match perfectly, the first agent's output from the previous pipeline is served directly, bypassing the execution of the target agent entirely. This enables cache poisoning, sensitive data leakage, and total validation bypass (e.g., bypassing a security scan or format enforcement).
*   **Remediation**:
    Derive the `workstack_id` from both the sorted/resolved agent IDs and the input hash, matching the safe implementation found in the native orchestrator:
    ```rust
    let workstack_id = format!("ws-{}", &Self::hash_sequence(&agent_ids, &req.input)[..12]);
    ```

---

### Finding 2: Critical — Memory Safety Violation/Undefined Behavior via Unpadded `simd-json` Deserialization
*   **Location**: `crates/op-cache/src/pattern_tracker.rs:226` and `crates/op-cache/src/workflow_tracker.rs:348`
*   **Vulnerability Type**: Out-of-Bounds Read / Memory Corruption
*   **Description**:
    The code retrieves serialized agent sequences from SQLite as standard standard-allocated strings and attempts to parse them using the unsafe `simd-json` API:
    ```rust
    let mut agent_sequence_json: String = row.get(1)?;
    let agent_sequence: Vec<String> =
        unsafe { simd_json::from_str(&mut agent_sequence_json) }
            .unwrap_or_default();
    ```
    `simd-json` is optimized to parse JSON using SIMD vector instructions (AVX2/AVX-512). It strictly requires that the input buffer contain trailing padding (`simd_json::SIMDJSON_PADDING`, usually 32 or 64 bytes) to prevent the SIMD load instructions from reading past the end of the allocation. 
    
    Standard strings fetched via `rusqlite`'s `row.get` do *not* guarantee this padding. Deserializing them with `simd_json::from_str` within an `unsafe` block triggers undefined behavior, leading to potential segmentation faults, memory disclosure, or memory corruption.
*   **Remediation**:
    Avoid raw `unsafe` invocations of `simd_json` on unpadded standard types. Either use standard `serde_json::from_str` for database fields or copy the string into a padded buffer before passing it to `simd-json`:
    ```rust
    // Safe alternative using serde_json
    let agent_sequence: Vec<String> = serde_json::from_str(&agent_sequence_json).unwrap_or_default();
    ```

---

### Finding 3: High — Command Injection in BTRFS Remote Send/Receive Syncing
*   **Location**: `crates/op-cache/src/btrfs_cache.rs:397-401` and `crates/op-cache/src/btrfs_cache.rs:424-428`
*   **Vulnerability Type**: OS Command Injection
*   **Description**:
    The `stream_to_remote` and `receive_from_remote` methods build shell command strings directly using string formatting on arguments and execute them through bash:
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
        .output()
        ...
    ```
    If `remote_host`, `remote_path`, or `remote_snapshot` are populated from user input, an attacker can supply malicious command separators (such as `; rm -rf /` or backticks) to execute arbitrary commands on the system. Because BTRFS commands require root or elevated control-plane privileges, any command execution here will immediately result in full host compromise.
*   **Remediation**:
    Do not invoke shell interpreters (`bash -c`) with formatted command strings. Execute the processes (`btrfs`, `ssh`) directly using `tokio::process::Command`, passing arguments as a safe vector slice (`args`), and pipe their input/output streams programmatically.

---

### Finding 4: Medium — Unbounded Cache Growth & Disk Exhaustion (Denial of Service)
*   **Location**: `crates/op-cache/src/workflow_cache.rs:172` and `crates/op-cache/src/workstack_cache.rs:141`
*   **Vulnerability Type**: Resource Exhaustion DoS
*   **Description**:
    The cache implementation defines an eviction algorithm (`evict_to_size`) and accepts a `max_size_bytes` configuration limit. However, the `put` methods in both `WorkflowCache` and `WorkstackCache` perform insertions into SQLite and write files to BTRFS without ever triggering `evict_to_size` or checking current disk consumption. 
    
    An attacker can repeatedly trigger workflow steps with varying inputs to consume unlimited disk space on the host cache volume, leading to physical system crash or control-plane lockout due to disk exhaustion.
*   **Remediation**:
    Call `evict_to_size` or an equivalent checking function inside `put` prior to or immediately after persisting new cached entries to enforce configured storage limits.

---

## 3. Schema-as-Code & Protocol Discipline Violations

The codebase frequently bypasses structured schema-as-code serialization paradigms in favor of raw strings, ad-hoc tables, and unversioned binary serialization formats.

### 3.1 Ad-Hoc SQLite Relational Schemas
Instead of declaring index schemas, states, and metadata using a migration engine (e.g., SQLx migrations) or serializing them using schemas, schemas are manually written as raw, embedded SQL strings directly in Rust files:
*   `crates/op-cache/src/btrfs_cache.rs:102-124`
*   `crates/op-cache/src/pattern_tracker.rs:52-78`
*   `crates/op-cache/src/workflow_cache.rs:78-100`
*   `crates/op-cache/src/workflow_tracker.rs:86-135`
*   `crates/op-cache/src/workstack_cache.rs:51-78`

This creates a high risk of schema drift, data corruption upon upgrades, and hard-to-maintain data migrations.

---

### 3.2 Unversioned Binary / JSON Serialization
Ad-hoc serialization is used to store data, violating versioned data contract standards:
*   **Raw Bincode Vectors**: In `crates/op-cache/src/btrfs_cache.rs:341`, vector embeddings are serialized directly into files using `bincode::serialize`. `bincode` is an unversioned binary protocol; changes in data definitions (e.g., float precision or array dimensions) will fail silently or crash during deserialization.
*   **JSON-in-DB Ad-hoc Contracts**: In `crates/op-cache/src/pattern_tracker.rs:118` and `crates/op-cache/src/workflow_tracker.rs:194`, lists of agents are converted to raw JSON strings using `simd_json::to_string` and stored as text fields in SQLite database rows.
*   **Model Context Protocol (MCP) Structs**: In `crates/op-cache/src/grpc/mcp_service.rs:280-335`, JSON-RPC and tool payload contracts are declared as ad-hoc custom structs rather than using versioned schemas or Protobuf-generated models.