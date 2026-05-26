# Security & Quality Audit Report

## 1. Unsafe Code Audit

This codebase contains three (3) `unsafe` blocks. All three instances bypass safe bounds-checking or validation constraints and completely lack the required `// SAFETY:` comments detailing safety invariants.

### Finding 1: Unvalidated JSON Parsing via Raw Mut String
* **Location:** `crates/op-cache/src/pattern_tracker.rs:333`
* **Context:**
  ```rust
  let agent_sequence: Vec<String> =
      unsafe { simd_json::from_str(&mut agent_sequence_json) }
          .unwrap_or_default();
  ```
* **Analysis:** This block uses `simd_json::from_str` with a mutable reference to a deserialized SQLite string database field. No safety contract is documented explaining why `agent_sequence_json` is guaranteed to be well-formed UTF-8 and safe for SIMD-accelerated mutable parsing without causing undefined behavior.
* **Flag:** Missing `// SAFETY:` explanation.

### Finding 2: Unsafe DB Deserialization of Call Sequences (First Instance)
* **Location:** `crates/op-cache/src/workflow_tracker.rs:432`
* **Context:**
  ```rust
  let agent_sequence: Vec<String> =
      unsafe { simd_json::from_str(&mut agent_sequence_json) }
          .unwrap_or_default();
  ```
* **Analysis:** Deserializes raw string values retrieved from the SQLite table `workflow_patterns` directly into arbitrary vectors of `String`. There is no check to guarantee that database corruption or manual schema edits won't violate memory alignment or size invariants.
* **Flag:** Missing `// SAFETY:` explanation.

### Finding 3: Unsafe DB Deserialization of Call Sequences (Second Instance)
* **Location:** `crates/op-cache/src/workflow_tracker.rs:482`
* **Context:**
  ```rust
  let agent_sequence: Vec<String> =
      unsafe { simd_json::from_str(&mut agent_sequence_json) }
          .unwrap_or_default();
  ```
* **Analysis:** Bypasses safety checking when querying the `promoted_workflows` table to reconstruct execution pipelines.
* **Flag:** Missing `// SAFETY:` explanation.

---

## 2. Command Execution Audit & Forbidden Commands

There are exactly **seven (7) distinct locations** where commands are spawned via `std::process::Command` or `tokio::process::Command`. Two (2) of these invoke forbidden shell execution structures, representing a **High** risk of command injection.

### Command Spawn Registry

| File | Line | Command Invoked | Argument Type | Severity |
| :--- | :--- | :--- | :--- | :--- |
| `crates/op-cache/src/btrfs_cache.rs` | 92 | `btrfs` | Controlled slice | Informational |
| `crates/op-cache/src/btrfs_cache.rs` | 550 | `bash` | **Forbidden Shell** / Dynamic String | **High** |
| `crates/op-cache/src/btrfs_cache.rs` | 592 | `bash` | **Forbidden Shell** / Dynamic String | **High** |
| `crates/op-cache/src/btrfs_cache.rs` | 699 | `taskset` | Programmatic array | Informational |
| `crates/op-cache/src/snapshot_manager.rs` | 59 | `btrfs` | Programmatic array | Informational |
| `crates/op-cache/src/snapshot_manager.rs` | 207 | `btrfs` | Programmatic array | Informational |
| `crates/op-cache/src/workflow_executor.rs` | 442 | `taskset` | Programmatic array | Informational |

---

### Vulnerability Analysis of Forbidden Command Sites

#### Finding 4: Forbidden Shell Spawn & Command Injection in Subvolume Send
* **Location:** `crates/op-cache/src/btrfs_cache.rs:550`
* **Command String:**
  ```rust
  let cmd = format!(
      "btrfs send {} | ssh {} 'btrfs receive {}'",
      snapshot_path.display(),
      remote_host,
      remote_path
  );
  ```
* **Analysis:** The orchestrator invokes `bash -c` and passes the formatted string containing raw inputs (`remote_host`, `remote_path`). This directly violates standard security practices. Any user-controlled input containing shell metacharacters (such as `;`, `&&`, or backticks) inside `remote_host` or `remote_path` will result in arbitrary code execution on the local control plane host. No argument validation or sanitization is performed on these strings.
* **Severity:** **High**

#### Finding 5: Forbidden Shell Spawn & Command Injection in Subvolume Receive
* **Location:** `crates/op-cache/src/btrfs_cache.rs:592`
* **Command String:**
  ```rust
  let cmd = format!(
      "ssh {} 'btrfs send {}' | btrfs receive {}",
      remote_host, remote_snapshot, local_path
  );
  ```
* **Analysis:** Similar to the previous finding, `bash -c` is used to launch a nested SSH tunnel to a remote host. The `remote_host`, `remote_snapshot`, and `local_path` are formatted raw. If an attacker can taint any of these values, they can escape the SSH wrapper and execute commands locally or on the target system.
* **Severity:** **High**

---

## 3. Hardcoded Secrets, IPs, and Cryptographic Assets

### Finding 6: Default Hardcoded Loopback Socket Bind
* **Location:** `crates/op-cache/src/grpc/server.rs:32`
* **Configuration:**
  ```rust
  listen_addr: "[::1]:50051".parse().unwrap(),
  ```
* **Analysis:** Although limited to the loopback interface (`[::1]`), standard deployments should ingest socket configurations through highly structured environment parameters or key-value engines to prevent port conflicts or accidental exposure in production containers.

### Finding 7: Static Subvolume Storage Paths
* **Location:** `crates/op-cache/src/snapshot_manager.rs:22`
* **Configuration:**
  ```rust
  snapshot_dir: PathBuf::from("/var/lib/op-dbus/@cache-snapshots"),
  ```
* **Analysis:** The directory for active control plane backups is statically set to `/var/lib/op-dbus/@cache-snapshots`. Placing system-wide, critical snapshots under a default directory without runtime isolation checks leaves snapshots vulnerable to access-control misalignment if standard group policies are not properly configured on `/var/lib/op-dbus`.

---

## 4. D-Bus Method Exposure Analysis

A complete audit of all provided files shows **zero (0) D-Bus methods, interfaces, or signals** registered or exported to the system bus. 

* `zbus` is listed as a workspace dependency in `Cargo.toml`, but no zbus-derived attributes (`#[dbus_interface]`, `#[dbus_proxy]`) are declared inside any source files of the `op-cache` crate. 
* There are no active IPC peer-exposure entrypoints via D-Bus in the audited scope.

---

## 5. Schema-as-Code Compliance & Architecture Quality

The project asserts a schema-as-code discipline utilizing Protocol Buffers and OSCAL standard contracts. However, the architectural implementation deviates significantly by embedding ad-hoc, unversioned structures for internal processing and interface translation.

### Finding 8: Ad-hoc MCP JSON-RPC Payload Structures
* **Location:** `crates/op-cache/src/grpc/mcp_service.rs:360-406`
* **Ad-hoc Contracts:**
  * `ToolCallParams` (deserializes arbitrary `serde_json::Value` arguments)
  * `McpContentResponse` (encodes untyped lists of `McpContent`)
  * `McpContent` (manual text wrap)
  * `McpToolsListResult` (ad-hoc tool array mapping)
  * `McpToolJson` (unversioned struct expressing tool properties)
  * `McpInitializeResult`, `McpServerCapabilities`, `McpToolCapability`, `McpServerInfo`
* **Analysis:** Rather than defining these JSON-RPC payloads in versioned Protocol Buffer contracts or schema specifications, they are declared as ad-hoc, manual-serialization structures. Any changes in MCP protocol definitions are not compile-time guarded or schema-validated.

### Finding 9: Dynamic JSON-Schema Assembly
* **Location:** `crates/op-cache/src/grpc/mcp_service.rs:339-351`
* **Dynamic Contract:**
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
* **Analysis:** Building schemas via the `json!` macro on the fly bypasses compile-time schema-as-code principles. Changes to input formats must be managed as versioned schemas rather than dynamically-allocated strings.

### Finding 10: Non-protobuf Serialized Core Contracts
* **Location:** `crates/op-cache/src/agent_registry.rs:142-181`
* **Struct:** `AgentDefinition`
* **Analysis:** The `AgentDefinition` struct represents the core operational contract of registered execution modules. This data contract is handled as an ad-hoc Serde struct rather than a Protocol Buffer message format, leading to dual serialization formats (Ad-hoc JSON/YAML via Serde and binary Protobuf via gRPC wrappers). This creates a synchronization burden and can lead to runtime decoding failures.