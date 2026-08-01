# D-Bus & IPC Attack Surface Audit

## 1. D-Bus Interface Inventory
Based on the files provided in the `FILES` section, there are **no native D-Bus interfaces, methods, or signals registered or defined** within the `op-cache` crate. 

Although the parent workspace depends on `zbus` (as seen in `Cargo.toml`), the IPC attack surface of `op-cache` is exposed entirely via **gRPC services** and a **Model Context Protocol (MCP) JSON-RPC bridge over gRPC**.

### gRPC & MCP IPC Surface Inventory

| Service | Method | Path / RPC Name | Mutates State? | Spawns Processes / Commands? | Caller Identity Checked? | Deserialization Performed? |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `AgentService` | `Register` | `/op_cache.AgentService/Register` | **Yes** (Registers agent metadata) | No | **No** | Yes (`prost` / protobuf) |
| `AgentService` | `Unregister` | `/op_cache.AgentService/Unregister` | **Yes** (Removes agent metadata) | No | **No** | Yes (`prost` / protobuf) |
| `AgentService` | `Execute` | `/op_cache.AgentService/Execute` | **Yes** (Runs arbitrary agent code) | **Yes** (Spawns shell/tool execution) | **No** | Yes (`prost` / protobuf) |
| `AgentService` | `ExecuteStream` | `/op_cache.AgentService/ExecuteStream` | **Yes** (Runs arbitrary agent code) | **Yes** (Spawns shell/tool execution) | **No** | Yes (`prost` / protobuf) |
| `CacheService` | `GetStep` | `/op_cache.CacheService/GetStep` | No | No | **No** | Yes (`prost` / protobuf) |
| `CacheService` | `PutStep` | `/op_cache.CacheService/PutStep` | **Yes** (Writes to cache DB/disk) | No | **No** | Yes (`prost` / protobuf) |
| `CacheService` | `InvalidateWorkstack` | `/op_cache.CacheService/InvalidateWorkstack` | **Yes** (Removes cache entries) | No | **No** | Yes (`prost` / protobuf) |
| `CacheService` | `InvalidateStep` | `/op_cache.CacheService/InvalidateStep` | **Yes** (Removes cache entries) | No | **No** | Yes (`prost` / protobuf) |
| `CacheService` | `Cleanup` | `/op_cache.CacheService/Cleanup` | **Yes** (Removes cache entries) | No | **No** | Yes (`prost` / protobuf) |
| `OrchestratorService` | `Execute` | `/op_cache.OrchestratorService/Execute` | **Yes** (Runs orchestrated agent pipeline) | **Yes** (Executes agents) | **No** | Yes (`prost` / protobuf) |
| `OrchestratorService` | `ExecuteStream` | `/op_cache.OrchestratorService/ExecuteStream` | **Yes** (Runs orchestrated agent pipeline) | **Yes** (Executes agents) | **No** | Yes (`prost` / protobuf) |
| `OrchestratorService` | `ExecuteAgents` | `/op_cache.OrchestratorService/ExecuteAgents` | **Yes** (Runs explicit agents) | **Yes** (Executes agents) | **No** | Yes (`prost` / protobuf) |
| `McpService` | `HandleRequest` | `/op_cache.McpService/HandleRequest` | **Yes** (Dispatches MCP JSON-RPC commands) | **Yes** (Dispatches shell/API calls) | **No** | Yes (`prost` + `simd-json`) |

---

## 2. Security Assessment of Spawning & Mutating Surface

### Missing Authorization and Caller Validation
In `crates/op-cache/src/grpc/server.rs:92`, the gRPC server is instantiated and bound to `listen_addr` without configuring mutual TLS (mTLS) or transport credentials:
```rust
Server::builder()
    .add_service(AgentServiceServer::from_arc(self.agent_service))
    .add_service(CacheServiceServer::from_arc(self.cache_service))
    .add_service(OrchestratorServiceServer::from_arc(self.orchestrator_service))
    .add_service(McpServiceServer::from_arc(self.mcp_service))
    .serve(addr)
```
* **No Authentication Interceptors:** There are no gRPC interceptors checking authorization headers, bearer tokens, or client credentials.
* **No Network Restrictions:** By default, loopback interfaces (`[::1]:50051`) are accessible by any local user or unprivileged process on the host. Any local client can connect and issue privileged mutation/execution RPCs.

### Mutation and Process Spawning Vectors
* **Arbitrary Code Execution via Agents:** The `Execute` method on `AgentService` (`crates/op-cache/src/grpc/agent_service.rs:217`) retrieves a registered agent executor and executes it with raw caller-controlled bytes (`req.input`). If agents possess capabilities such as `ShellExecution` or `FileOperation` (defined in `crates/op-cache/src/agent_registry.rs:51-52`), unauthenticated RPC callers can trigger arbitrary command execution as the daemon process.
* **State Tampering:** The `PutStep` (`crates/op-cache/src/grpc/cache_service.rs:178`) and `Cleanup` (`crates/op-cache/src/grpc/cache_service.rs:232`) RPCs allow arbitrary clients to populate the persistent cache directory with binary payloads or purge database tables, opening vectors for denial of service and cache poisoning.

---

## 3. System Bus Policy Comparison
No system bus XML configuration (such as `/usr/share/dbus-1/system.d/` policy files) was provided in the `FILES` section. Thus, comparison with D-Bus permission rules cannot be performed.

---

# Critical & High Security Findings

### [Finding 1] Remote Code Execution via Shell Command Injection in Btrfs Cache Syncing
* **File & Line:** `crates/op-cache/src/btrfs_cache.rs:496` and `crates/op-cache/src/btrfs_cache.rs:532`
* **Severity:** Critical
* **Impact:** Direct command injection as the running user (highly likely `root` given that `btrfs` subvolume manipulation typically requires root/CAP_SYS_ADMIN privileges).
* **Vulnerability Analysis:**
  The `stream_to_remote` and `receive_from_remote` helper functions format untrusted strings directly into a shell command passed to `bash -c`:
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
  If `remote_host` or `remote_path` contains shell metacharacters (e.g., `; rm -rf /;` or backticks), they will be executed verbatim by `bash`. Because these inputs are parsed dynamically, any mechanism supplying remote synchronization paths to this backend cache will expose an immediate, unauthenticated remote code execution vector.
* **Remediation:**
  Do not invoke commands through a shell (`bash -c`). Execute `ssh` and `btrfs` directly using structured vector arguments:
  ```rust
  tokio::process::Command::new("btrfs")
      .args(["send", &snapshot_path.to_string_lossy()])
      ...
  ```

---

### [Finding 2] Undefined Behavior & Out-of-Bounds Memory Read via Unpadded Unsafe `simd_json` Deserialization
* **File & Line:** `crates/op-cache/src/pattern_tracker.rs:258`, `crates/op-cache/src/workflow_tracker.rs:386`, and `crates/op-cache/src/workflow_tracker.rs:419`
* **Severity:** High
* **Impact:** Potential heap corruption, crash (denial of service), or information disclosure via heap out-of-bounds reads.
* **Vulnerability Analysis:**
  The tracking databases store JSON representations of agent sequences. During retrieval, the code reads standard `String` representations from rusqlite and deserializes them using `simd_json` within an `unsafe` block:
  ```rust
  let mut agent_sequence_json: String = row.get(1)?;
  let agent_sequence: Vec<String> =
      unsafe { simd_json::from_str(&mut agent_sequence_json) }
          .unwrap_or_default();
  ```
  **Underlying Safety Violation:** `simd-json` performance guarantees rely heavily on the parsed buffer being mutable and having a trailing padding of at least `simd_json::SIMDJSON_PADDING` (64 bytes) to safely load vector instructions without crossing page boundaries. Standard Rust `String` buffers returned from SQLite rows *do not* guarantee this padding. Running `unsafe { simd_json::from_str(...) }` on standard unpadded strings retrieved from a database row causes the SIMD engine to perform out-of-bounds reads over adjacent heap memory.
* **Remediation:**
  Use the safe `simd_json::from_slice` API after converting the string into a padded byte vector, or allocate a padded container explicitly:
  ```rust
  let mut agent_sequence_json: String = row.get(1)?;
  let mut padded_bytes = agent_sequence_json.into_bytes();
  // Ensure SIMD padding is respected if mutating/parsing in-place
  padded_bytes.reserve(simd_json::SIMDJSON_PADDING);
  let agent_sequence: Vec<String> = simd_json::from_slice(&mut padded_bytes)?;
  ```

---

### [Finding 3] Unauthenticated TCP gRPC IPC Port Bound to Loopback Allowing Local Privilege Escalation
* **File & Line:** `crates/op-cache/src/grpc/server.rs:92`
* **Severity:** High
* **Impact:** Local privilege escalation (LPE) or cross-user payload tampering.
* **Vulnerability Analysis:**
  The gRPC server serves multiple highly sensitive services (such as orchestration, cache modification, and agent execution) over an plaintext loopback TCP socket (`[::1]:50051`). On shared Linux hosts, any local user (even sandboxed or unprivileged ones) can connect to this port and invoke any service. If `op-cache` runs as root (necessary to issue raw local shell execution or privileged `btrfs` commands), an unprivileged local user can gain root execution access by simply submitting an `ExecuteAgentRequest` targeting a `ShellExecution` agent.
* **Remediation:**
  * Implement Mutual TLS (mTLS) with client certificate verification via `tonic::transport::Server::tls_config`.
  * Alternatively, bind the gRPC server exclusively to a UNIX Domain Socket (UDS) with strict filesystem permissions (`0600` or `0660` with restricted group access).

---

# Schema-as-Code Discipline Audit

The codebase follows a schema-as-code paradigm via protobuf (`prost`), but displays several prominent gaps where data contracts are expressed as ad-hoc Rust structs, untyped strings, or dynamic JSON schema strings instead of versioned Proto/OSCAL models.

### [Gap 1] Persistent JSON Storage of Sequences
* **File & Line:** `crates/op-cache/src/pattern_tracker.rs:211`, `crates/op-cache/src/workflow_tracker.rs:155`
* **Violation:**
  Agent sequences are saved to SQLite by serializing Rust vectors into ad-hoc JSON text strings (`agent_sequence TEXT NOT NULL`) rather than structured schema columns or binary-encoded versioned Protocol Buffer structures. 
  ```rust
  let agent_sequence_json = simd_json::to_string(agents)?;
  ```
  If the agent naming convention or metadata format changes in subsequent version schemas, existing SQLite records will fail to deserialize cleanly, with no database migration schema path.

### [Gap 2] Ad-Hoc Serde Structs for Model Context Protocol (MCP) Bridge
* **File & Line:** `crates/op-cache/src/grpc/mcp_service.rs:280-334`
* **Violation:**
  The JSON-RPC endpoints for MCP (`initialize`, `tools/list`, `tools/call`) are backed by custom, ad-hoc serialized Rust structs (`ToolCallParams`, `McpContentResponse`, `McpInitializeResult`, etc.) rather than codified and versioned Protocol Buffers or official schema models.
  ```rust
  #[derive(serde::Deserialize)]
  struct ToolCallParams {
      name: String,
      #[serde(default)]
      arguments: serde_json::Value,
  }
  ```
  This duplicates data models and fails to leverage code-generation for cross-language compatibility.

### [Gap 3] Dynamic JSON Schema Generation
* **File & Line:** `crates/op-cache/src/grpc/mcp_service.rs:265`
* **Violation:**
  The `build_agent_input_schema` constructs an ad-hoc JSON schema string dynamically using raw macro interpolation rather than deriving it from a declarative schema compiler:
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
  Dynamic, untyped generation of JSON Schema documents breaks compile-time safety and bypasses declarative schema validation.

### [Gap 4] Unstructured Cache Payloads on Disk
* **File & Line:** `crates/op-cache/src/workflow_cache.rs:192-230` and `crates/op-cache/src/workstack_cache.rs:165-200`
* **Violation:**
  Intermediate step results are written as unstructured binary slices (`Vec<u8>`) to `{cache_key}.cache` files:
  ```rust
  let output_file = format!("{}.cache", cache_key);
  let data_path = self.cache_dir.join("data").join(&output_file);
  std::fs::write(&data_path, &data)?;
  ```
  These files have no structured envelopes, schema version headers, magic bytes, or checksums, leading to corruption or deserialization failure when schemas are updated. Payloads should be wrapped in versioned Proto envelopes before being flushed to persistent storage.