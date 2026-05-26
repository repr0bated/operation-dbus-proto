# Workspace Integration & Security Audit Report

## Workspace Integration

### Crates Depending on `op-cache`
Based on the workspace configuration provided in the root `Cargo.toml`:
* **`op-dbus`** (root package crate) depends directly on `op-cache` via `op-cache.workspace = true` (`Cargo.toml:380`).

### D-Bus Registrations
No D-Bus service names or object paths are directly registered within the `op-cache` source files provided. However, the workspace declares a dependency on `zbus = "4.0"` (`Cargo.toml:112`) and `zbus_xml = "4.0"` (`Cargo.toml:113`), indicating D-Bus support is utilized at the workspace level (e.g., in `op-dbus`).

### Exposed HTTP/gRPC Endpoints
The gRPC server configured in `crates/op-cache/src/grpc/server.rs` binds to a default listen address of `[::1]:50051` (`crates/op-cache/src/grpc/server.rs:31`) and exposes the following services and endpoints:

#### 1. `AgentService` (`crates/op-cache/src/grpc/agent_service.rs`)
* `register`: Registers a new agent (`crates/op-cache/src/grpc/agent_service.rs:114`)
* `unregister`: Unregisters an agent (`crates/op-cache/src/grpc/agent_service.rs:159`)
* `execute`: Executes an agent synchronously (`crates/op-cache/src/grpc/agent_service.rs:194`)
* `execute_stream`: Executes an agent and streams back chunks of output (`crates/op-cache/src/grpc/agent_service.rs:222`)
* `get_agent`: Retrieves agent definitions (`crates/op-cache/src/grpc/agent_service.rs:271`)
* `list_agents`: Lists all registered agents (`crates/op-cache/src/grpc/agent_service.rs:284`)
* `find_by_capability`: Finds agents matching requested capabilities (`crates/op-cache/src/grpc/agent_service.rs:300`)
* `list_capabilities`: Lists all unique registered capabilities (`crates/op-cache/src/grpc/agent_service.rs:356`)
* `health_check`: Inspects a specific agent's health/uptime (`crates/op-cache/src/grpc/agent_service.rs:372`)

#### 2. `CacheService` (`crates/op-cache/src/grpc/cache_service.rs`)
* `get_step`: Retrieves a cached result of a workstack step (`crates/op-cache/src/grpc/cache_service.rs:172`)
* `put_step`: Caches the result of a workstack step (`crates/op-cache/src/grpc/cache_service.rs:207`)
* `invalidate_workstack`: Clears cache entries for an entire workstack (`crates/op-cache/src/grpc/cache_service.rs:249`)
* `invalidate_step`: Clears cache entries for a specific step index (`crates/op-cache/src/grpc/cache_service.rs:269`)
* `cleanup`: Evaluates and purges expired cache files (`crates/op-cache/src/grpc/cache_service.rs:284`)
* `get_stats`: Retrieves cache service statistics (`crates/op-cache/src/grpc/cache_service.rs:315`)
* `get_workstack_stats`: Retrieves performance metrics for a specific workstack (`crates/op-cache/src/grpc/cache_service.rs:319`)

#### 3. `OrchestratorService` (`crates/op-cache/src/grpc/orchestrator_service.rs`)
* `execute`: Resolves capabilities to agents and executes the chain (`crates/op-cache/src/grpc/orchestrator_service.rs:358`)
* `execute_stream`: Streams individual workstack step results (`crates/op-cache/src/grpc/orchestrator_service.rs:452`)
* `execute_agents`: Bypasses resolver to execute an explicit agent sequence (`crates/op-cache/src/grpc/orchestrator_service.rs:563`)
* `resolve`: Resolves capabilities to a sequence without executing (`crates/op-cache/src/grpc/orchestrator_service.rs:619`)
* `get_patterns`: Retrieves detected multi-agent invocation patterns (`crates/op-cache/src/grpc/orchestrator_service.rs:643`)
* `promote_pattern`: Promotes a highly-used pattern to a named workstack (`crates/op-cache/src/grpc/orchestrator_service.rs:682`)
* `get_stats`: Retrieves statistics from the orchestrator and sub-services (`crates/op-cache/src/grpc/orchestrator_service.rs:712`)

#### 4. `McpService` (Model Context Protocol JSON-RPC Bridge, `crates/op-cache/src/grpc/mcp_service.rs`)
* `handle_request`: Translates incoming MCP JSON-RPC requests (`crates/op-cache/src/grpc/mcp_service.rs:326`), routing standard methods:
  * `initialize` (`crates/op-cache/src/grpc/mcp_service.rs:188`)
  * `ping` (`crates/op-cache/src/grpc/mcp_service.rs:211`)
  * `tools/list` (`crates/op-cache/src/grpc/mcp_service.rs:159`)
  * `tools/call` (`crates/op-cache/src/grpc/mcp_service.rs:45`)
* `list_tools`: Directly lists all registered agents exposed as MCP tools (`crates/op-cache/src/grpc/mcp_service.rs:371`)

### Cross-Crate Circular Dependency Risk
There is **no circular dependency risk** introduced by the `op-cache` crate. 
* Under `crates/op-cache/Cargo.toml`, `op-cache` does not import any other local workspace packages (such as `op-core`, `op-state`, or `op-tools`). 
* It is architected as a leaf utility library. Higher-level controller crates like `op-dbus` import `op-cache`, but `op-cache` remains decoupled, avoiding compile-time circular dependency chains.

---

## Critical Security Findings

### [CRITICAL] Shell Command Injection in BTRFS Cache Remote Streaming
* **File & Line**: `crates/op-cache/src/btrfs_cache.rs:475`, `crates/op-cache/src/btrfs_cache.rs:511`

#### Description
The `BtrfsCache` implements snapshot synchronization functions `stream_to_remote` and `receive_from_remote` which invoke `bash -c` with unescaped string formatting:

```rust
// crates/op-cache/src/btrfs_cache.rs:469-474
let cmd = format!(
    "btrfs send {} | ssh {} 'btrfs receive {}'",
    snapshot_path.display(),
    remote_host,
    remote_path
);
```

```rust
// crates/op-cache/src/btrfs_cache.rs:505-509
let cmd = format!(
    "ssh {} 'btrfs send {}' | btrfs receive {}",
    remote_host, remote_snapshot, local_path
);
```

These formatted strings are evaluated directly by spawning a `bash` shell process:
```rust
// crates/op-cache/src/btrfs_cache.rs:475-479
let output = tokio::process::Command::new("bash")
    .arg("-c")
    .arg(&cmd)
    ...
```

If `remote_host`, `remote_path`, or `remote_snapshot` are retrieved from user-controlled inputs, an attacker can append arbitrary shell meta-characters (such as `;`, `&&`, or `|`) to execute arbitrary operating system commands with the privileges of the running application.

#### Remediation
Avoid invoking shell interpreters (`bash -c`) entirely. Instead, execute the underlying processes (`ssh` and `btrfs`) directly using argument arrays via `tokio::process::Command::new`, piping stdout/stderr safely in Rust code:

```rust
let mut btrfs_send = tokio::process::Command::new("btrfs")
    .args(["send", snapshot_path.to_str().unwrap()])
    .stdout(std::process::Stdio::piped())
    .spawn()?;

let btrfs_send_stdout = btrfs_send.stdout.take().unwrap();

let mut ssh = tokio::process::Command::new("ssh")
    .args([remote_host, "btrfs", "receive", remote_path])
    .stdin(btrfs_send_stdout)
    .spawn()?;
```

---

## Architectural & Quality Findings

### 1. Data Contract Violations (Schema-as-Code Discipline)
* **File & Line**: `crates/op-cache/src/grpc/mcp_service.rs:250-262`, `crates/op-cache/src/grpc/mcp_service.rs:265-316`, `crates/op-cache/src/agent_registry.rs:109`

#### Description
The system defines critical interfaces and payload boundaries as ad-hoc, unstructured Rust structs or dynamic JSON structures rather than using versioned schemas (such as Protocol Buffers or OSCAL-compliant formats):
* **Model Context Protocol (MCP) Structs**: Payload metadata such as `ToolCallParams` and `McpContentResponse` (`crates/op-cache/src/grpc/mcp_service.rs:265-316`) are expressed using ad-hoc `serde` representations.
* **Ad-hoc Input Schema Generation**: The `build_agent_input_schema` function (`crates/op-cache/src/grpc/mcp_service.rs:250-262`) creates raw, dynamically formatted JSON values on the fly to represent tool schemas, risking runtime serialization issues:
  ```rust
  let schema = serde_json::json!({
      "type": "object",
      "properties": {
          "input": { ... }
      }
  });
  ```
* **Agent Metadata**: `AgentDefinition` (`crates/op-cache/src/agent_registry.rs:109`) is designed as a raw Rust struct, and its interactions map to unstructured strings and lists rather than versioned schema instances.

#### Remediation
Define all message boundaries, agent schemas, and tools using versioned Protocol Buffers or JSON Schemas derived directly from unified contract specifications. Keep metadata synchronized with the gRPC `.proto` contracts.

---

### 2. Thread Starvation Risks (Blocking I/O in Async Contexts)
* **File & Line**: `crates/op-cache/src/grpc/agent_service.rs:241-248`, `crates/op-cache/src/btrfs_cache.rs:320-377`

#### Description
* **Synchronous File Operations**: The `load_embedding` and `save_embedding` methods of `BtrfsCache` (`crates/op-cache/src/btrfs_cache.rs:320-377`) perform synchronous disk operations (`std::fs::read` and `std::fs::write`). If called in a high-concurrency async context, these block the thread of execution.
* **Synchronous Executor Closures**: The gRPC agent service `execute_stream` endpoint uses a synchronous `AgentExecutor` signature (`crates/op-cache/src/grpc/agent_service.rs:23`):
  ```rust
  pub type AgentExecutor = Arc<dyn Fn(&[u8]) -> Result<Vec<u8>, String> + Send + Sync>;
  ```
  This is executed inside a `tokio::spawn` worker thread (`crates/op-cache/src/grpc/agent_service.rs:241-248`). Because the closure is blocking rather than async, long-running agent tasks will starve the tokio worker thread pool, degrading overall control plane throughput and responsiveness.

#### Remediation
1. Use `tokio::task::spawn_blocking` around all synchronous database/filesystem transactions inside `BtrfsCache`.
2. Redefine the gRPC `AgentExecutor` to match the asynchronous `BoxFuture` based signature used by the core agent registry (`crates/op-cache/src/agent_registry.rs:188`).

---

### 3. Silent Snapshot Failures on Fallback Filesystems
* **File & Line**: `crates/op-cache/src/btrfs_cache.rs:78`, `crates/op-cache/src/snapshot_manager.rs:49`

#### Description
If the host system does not support BTRFS (or BTRFS commands fail), the cache silently falls back to establishing standard directories (`crates/op-cache/src/btrfs_cache.rs:78`):
```rust
warn!("BTRFS not available, creating regular directory: {:?}", path);
tokio::fs::create_dir_all(path)...
```
While this allows initialization to succeed, any subsequent attempt to run `create_snapshot` or snapshot rotation (`crates/op-cache/src/snapshot_manager.rs:49`) will unconditionally fail with a shell command error when trying to run `btrfs subvolume snapshot -r` on non-subvolume directories.

#### Remediation
Track the filesystem fallback status inside `BtrfsCache`. If initialization has degraded to standard directories, disable snapshot functionality gracefully and return structured, descriptive errors (e.g., `UnsupportedFilesystem`) instead of attempting to run failing shell processes.

---
## ⚠ Citation Warnings
- `crates/op-cache/src/grpc/mcp_service.rs:371`: file has 368 lines
