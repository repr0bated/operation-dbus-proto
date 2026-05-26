### 1. Environment Variable Configuration Audit

#### 1.1 Complete List of `std::env::var` Reads
The following environment variables are queried at runtime within the `op-cache` crate:

*   **`OPDBUS_MAX_CACHE_SNAPSHOTS`**
    *   **Location:** `crates/op-cache/src/btrfs_cache.rs:173`
    *   **Usage:** Configures the retention policy limit for BTRFS snapshots.

*   **`OPDBUS_CACHE_SNAPSHOT_PREFIX`**
    *   **Location:** `crates/op-cache/src/btrfs_cache.rs:177`
    *   **Usage:** Sets the string prefix used when naming newly generated cache snapshots.

*   **`OPDBUS_CACHE_PLACEMENT`**
    *   **Location:** `crates/op-cache/src/btrfs_cache.rs:207`
    *   **Usage:** Directs the NUMA-aware cache placement strategy (`round-robin`, `most-memory`, `local-node`, or `disabled`).

*   **`OPDBUS_CACHE_MEMORY_POLICY`**
    *   **Location:** `crates/op-cache/src/btrfs_cache.rs:240`
    *   **Usage:** Specifies the system memory allocation policy for multi-socket NUMA structures (`bind`, `preferred`, `interleave`, or `default`).

#### 1.2 Defensive Evaluation of Environment Variables
All identified environment variable reads are implemented defensively with robust fallbacks and error handling. No variables fail to declare defaults or risk crashing due to unhandled results:

*   **`OPDBUS_MAX_CACHE_SNAPSHOTS`** (`btrfs_cache.rs:173-176`): Handles parsing errors and missing variables cleanly by utilizing `.ok().and_then(|s| s.parse().ok()).unwrap_or(24)`.
*   **`OPDBUS_CACHE_SNAPSHOT_PREFIX`** (`btrfs_cache.rs:177-178`): Defaults safely to `"SNP-cache"` via `.unwrap_or_else(|_| "SNP-cache".to_string())`.
*   **`OPDBUS_CACHE_PLACEMENT`** (`btrfs_cache.rs:207-209`): Captures missing environment variables by falling back to `default_choice` using `.unwrap_or(default_choice)`.
*   **`OPDBUS_CACHE_MEMORY_POLICY`** (`btrfs_cache.rs:240-278`): Evaluates the `Result` of `std::env::var` inside a matching block. If missing or invalid, it prints warnings and falls back to `MemoryPolicy::Default`.

---

### 2. Cargo Features & Workspace Additivity Analysis

#### 2.1 Crate and Workspace Features
*   **Crate `op-cache` (`crates/op-cache/Cargo.toml`):** Does not define any explicit feature flags under a `[features]` section. It relies on standard external dependencies and workspace-defined crates.
*   **Workspace package `op-dbus` (`Cargo.toml`):** Defines the following features:
    *   `default = ["grpc"]`
    *   `grpc = []`

#### 2.2 Feature Additivity Assessment
Cargo features are designed to be additive. Because the target crate `op-cache` does not leverage feature gates or conditional compilation flags (`#[cfg(feature = ...)]`), its compilation units are static regardless of the parent workspace feature configurations. 

---

### 3. Hardcoded Assets and Injection Risks

#### 3.1 Hardcoded Paths, Ports, and Network Addresses
*   **Hardcoded Default Database/Storage Path Fallbacks:**
    *   **Location:** `crates/op-cache/src/btrfs_cache.rs:171-172`
    *   **Code:** `.unwrap_or(Path::new("/var/lib/op-dbus"))`
    *   **Location:** `crates/op-cache/src/snapshot_manager.rs:24`
    *   **Code:** `snapshot_dir: PathBuf::from("/var/lib/op-dbus/@cache-snapshots"),`
    *   **Risk:** Utilizing `/var/lib/op-dbus` limits portability and assumes highly specific directory permissions on the host system.

*   **Hardcoded Port and Network Socket Binding:**
    *   **Location:** `crates/op-cache/src/grpc/server.rs:31`
    *   **Code:** `listen_addr: "[::1]:50051".parse().unwrap(),`
    *   **Risk:** Defaulting to the standard gRPC development port (`50051`) on the IPv6 loopback interface (`[::1]`) can lead to address conflicts or unauthorized local bindings if not actively overridden by dynamic runtime configurations.

*   **Hardcoded Test Pathing:**
    *   **Location:** `crates/op-cache/src/btrfs_cache.rs:539`
    *   **Code:** `BtrfsCache::new(PathBuf::from("/tmp/test-cache"))`
    *   **Risk:** Exposes test fixtures to shared directory space (`/tmp`), which can be vulnerable to local symlink race conditions if executed on a production-like environment.

#### 3.2 High-Severity Vulnerabilities: OS Command Injection
*   **Shell Interpolation and Injection in Remote Synchronization:**
    *   **Location:** `crates/op-cache/src/btrfs_cache.rs:400-403` (`stream_to_remote`)
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
    *   **Location:** `crates/op-cache/src/btrfs_cache.rs:431-434` (`receive_from_remote`)
        ```rust
        let cmd = format!(
            "ssh {} 'btrfs send {}' | btrfs receive {}",
            remote_host, remote_snapshot, local_path
        );

        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&cmd)
        ```
    *   **Risk (High Severity):** If `remote_host`, `remote_path`, or `remote_snapshot` are populated using user-provided metadata or external API parameters, an attacker could inject shell control metacharacters (such as `;`, `&&`, or backticks) to execute arbitrary commands under the privileges of the running daemon.

#### 3.3 Path Hijacking Vulnerabilities (Unvalidated PATH Executables)
The codebase executes host binaries by targeting naked relative command names rather than qualified absolute paths. If the binary environment contains a polluted `PATH` variable, this can lead to execution hijacking:
*   **`btrfs` Execution:**
    *   `crates/op-cache/src/btrfs_cache.rs:74`: `tokio::process::Command::new("btrfs")`
    *   `crates/op-cache/src/snapshot_manager.rs:52`: `Command::new("btrfs")`
    *   `crates/op-cache/src/snapshot_manager.rs:180`: `Command::new("btrfs")`
*   **`taskset` Execution:**
    *   `crates/op-cache/src/btrfs_cache.rs:440`: `tokio::process::Command::new("taskset")`
    *   `crates/op-cache/src/workflow_executor.rs:364`: `tokio::process::Command::new("taskset")`

---

### 4. Schema-as-Code Compliance Audit

This codebase utilizes a hybrid approach where Protocol Buffers are compiled (via `tonic::include_proto!`), but several microservices and serialization boundaries utilize ad-hoc, loosely-typed struct definitions and raw byte slices instead of schema-governed messages:

*   **Ad-Hoc JSON-RPC Schema Declarations:**
    *   **Location:** `crates/op-cache/src/grpc/mcp_service.rs:353-407`
    *   **Finding:** The Model Context Protocol (MCP) service maps internal JSON-RPC operations to structs defined entirely inline (`ToolCallParams`, `McpContentResponse`, `McpContent`, `McpToolsListResult`, `McpToolJson`, `McpInitializeResult`, `McpServerCapabilities`, `McpToolCapability`, `McpServerInfo`). These types are not defined in shared Protobuf files or versioned schema documents, increasing drift risk.

*   **Dynamically Generated JSON-Schema Strings:**
    *   **Location:** `crates/op-cache/src/grpc/mcp_service.rs:338`
    *   **Finding:** The `build_agent_input_schema` function defines input expectations using a hardcoded, dynamically rendered macro block:
        ```rust
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string",
                    "description": "Input data for the agent"
                }
            }
        });
        ```
        This relies on raw JSON manipulation rather than strong typings compiled from a version-controlled contract model.

*   **Raw Untyped Byte Contracts at Execution Boundaries:**
    *   **Location:** `crates/op-cache/src/agent_registry.rs:163`
    *   **Finding:** The `AgentExecutor` signature is typed as `Arc<dyn Fn(&[u8]) -> BoxFuture<'static, Result<Vec<u8>>> + Send + Sync>`.
    *   **Location:** `crates/op-cache/src/orchestrator.rs:50`
    *   **Finding:** The `OrchestrationResult` structures store raw output data in an unmapped byte field (`pub output: Vec<u8>`).
    *   **Risk:** Bypassing schema serialization at the component interfaces prevents static tracing of message structures, relying on runtime serialization assumptions between agents.