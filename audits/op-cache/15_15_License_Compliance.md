# Security and License Quality Audit Report

## 1. License Analysis

### License Extraction
* **op-cache License**: `MIT` (extracted from `crates/op-cache/Cargo.toml:6`).
* **op-dbus (Workspace Package) License**: `Apache-2.0` (inherited via `Cargo.toml:1423` from `Cargo.toml:47`).

### Copyleft Scan (Cargo.lock)
A rigorous scan of the provided `Cargo.lock` was performed to identify any incompatible copyleft licenses (such as GPL, AGPL, or SSPL):
* No GPL, AGPL, or SSPL licensed crates were detected.
* **Note on Cozo**: The workspace depends on `cozo` (version `0.7.6`), which is licensed under MPL-2.0 (Mozilla Public License 2.0). MPL-2.0 is a weak copyleft license and is compatible with Apache-2.0 and MIT licensed code when linked or distributed, provided that the MPL-2.0 source files (if modified) remain open under MPL-2.0.

### Crates with No License Field
* Because we only have access to the `Cargo.toml` files for `op-cache` and the workspace root `op-dbus`, we cannot audit the individual `Cargo.toml` files of the other 33 workspace members listed under `Cargo.toml:3-38`. We can confirm that `op-cache` and `op-dbus` both correctly define their license fields.

---

## 2. Security Findings

### Critical Vulnerability: OS Command Injection via Unsanitized Shell Execution
* **Location**: `crates/op-cache/src/btrfs_cache.rs:504-522` and `crates/op-cache/src/btrfs_cache.rs:544-562`
* **Impact**: **Critical (Remote Code Execution)**
* **Description**:
  The `BtrfsCache` implementation contains two methods, `stream_to_remote` and `receive_from_remote`, that dynamically format shell commands containing unescaped parameters (`remote_host`, `remote_path`, `remote_snapshot`, `local_path`) and pass them directly to `bash -c`.

  In `stream_to_remote`:
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
  And similarly in `receive_from_remote`:
  ```rust
  let cmd = format!(
      "ssh {} 'btrfs send {}' | btrfs receive {}",
      remote_host,
      remote_snapshot,
      local_path
  );

  let output = tokio::process::Command::new("bash")
      .arg("-c")
      .arg(&cmd)
  ```
  If any of these arguments are supplied by an external gRPC client or untrusted RPC interface, an attacker can append shell metacharacters (e.g., `; rm -rf /` or backticks) to execute arbitrary shell commands with the privileges of the application process.
* **Remediation**:
  Avoid executing shell pipelines via `bash -c`. Instead, instantiate `tokio::process::Command` directly with the executable binary (`ssh` or `btrfs`) and pass the arguments as a safe `Vec` of distinct parameters. For the pipeline logic, spawn both processes in Rust and wire their stdout/stdin together using asynchronous pipes (`std::process::Stdio::piped()`).

---

## 3. Schema-As-Code Violations

The codebase mandates a schema-as-code discipline utilizing Protocol Buffers and OSCAL. Ad-hoc serialization structures and dynamic string schema generations are identified and flagged below:

### Finding 1: Ad-hoc JSON-RPC Structs for Model Context Protocol (MCP)
* **Location**: `crates/op-cache/src/grpc/mcp_service.rs:351-407`
* **Description**: The MCP implementation defines a series of ad-hoc serialization structures inside the service code rather than generating them from versioned schemas:
  * `ToolCallParams` (lines 351-355)
  * `McpContentResponse` (lines 357-360)
  * `McpContent` (lines 362-366)
  * `McpToolsListResult` (lines 368-371)
  * `McpToolJson` (lines 373-379)
  * `McpInitializeResult` (lines 381-389)
  * `McpServerCapabilities` (lines 391-395)
  * `McpToolCapability` (lines 397-401)
  * `McpServerInfo` (lines 403-407)
* **Remediation**: Re-define the Model Context Protocol structures as versioned Protocol Buffer schemas or OSCAL-compliant declarative models.

### Finding 2: Dynamic Construction of JSON Schema String
* **Location**: `crates/op-cache/src/grpc/mcp_service.rs:334-345`
* **Description**:
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
  This dynamically builds a JSON Schema utilizing `serde_json::json!` and parses it using `simd_json` instead of utilizing a statically typed, version-controlled schema definition.
* **Remediation**: Define formal versioned JSON schema files or compile-time checked structures for agent inputs.

### Finding 3: SQLite Ad-hoc Persistence Models
* **Location**: `crates/op-cache/src/pattern_tracker.rs:32-52` and `crates/op-cache/src/workflow_tracker.rs:53-81`
* **Description**: 
  The structs `TrackedPattern` and `PromotionSuggestion` (in both `pattern_tracker.rs` and `workflow_tracker.rs`), along with `AgentCall` (`workflow_tracker.rs:88-99`), represent data contracts that are persisted into SQLite and serialized to JSON. These contracts are defined as plain-old Rust structs instead of deriving from shared schemas.
* **Remediation**: Standardize these workflow and pattern storage schemas under unified schema-as-code definitions.