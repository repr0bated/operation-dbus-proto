# Production Quality and Security Audit: op-snowball

## Part 1: Test Suite Evaluation

### 1. Test Locations and Structure
Unit tests are implemented as inline module tests marked with `#[cfg(test)]` within individual source files. No dedicated integration tests in `tests/` were provided in the analyzed file set.

### 2. Test Function Count
There are exactly **8** test functions across the provided files:
*   `crates/op-snowball/src/footprint.rs`: 2 test functions
*   `crates/op-snowball/src/plugin_footprint.rs`: 2 test functions
*   `crates/op-snowball/src/retention.rs`: 2 test functions
*   `crates/op-snowball/src/snapshot.rs`: 2 test functions

### 3. Representative Tests
*   **Test 1**: `crates/op-snowball/src/footprint.rs:146` (`test_block_event_creation`) - Verifies basic block event hash computation and metadata structure validity.
*   **Test 2**: `crates/op-snowball/src/plugin_footprint.rs:393` (`test_footprint_generation`) - Asserts correct heuristic feature generation and field consistency for plugin operations.
*   **Test 3**: `crates/op-snowball/src/retention.rs:130` (`test_default_policy`) - Tests the default retention window numbers for rolling snapshots.

### 4. Property-Based Testing and Fuzzing
*   **Status**: No property-based tests (e.g., `proptest`, `quickcheck`) or fuzzing targets were found in the provided codebase. Testing is limited to deterministic unit assertions.

---

## Part 2: Schema-as-Code Compliance

The codebase deviates from a strict schema-as-code discipline (Protocol Buffers/OSCAL) by representing data contracts as ad-hoc, weakly typed structures utilizing raw JSON values.

### Finding 1: Unstructured Payload Mapping in Core Events
*   **File/Line**: `crates/op-snowball/src/footprint.rs:9-16` and `crates/op-snowball/src/footprint.rs:44-52`
*   **Vulnerability/Quality Smell**: The `BlockEvent` and `PluginFootprint` structs define the core system events using `simd_json::OwnedValue` and `HashMap<String, simd_json::OwnedValue>`. Bypassing versioned schemas in favor of arbitrary JSON values makes the data contracts fragile and highly susceptible to drift, complicating downstream consumption.

### Finding 2: Duplicate Struct Definitions for Data Contracts
*   **File/Line**: `crates/op-snowball/src/streaming_snowball.rs:20-29`
*   **Vulnerability/Quality Smell**: `BlockEvent` is defined as an ad-hoc struct again inside `streaming_snowball.rs`, duplicating the structure found in `footprint.rs` and maintaining unstructured JSON definitions for its core payloads.

### Finding 3: Ad-Hoc Inline JSON Serialization Mappings
*   **File/Line**: `crates/op-snowball/src/btrfs_numa_integration.rs:104-114`
*   **Vulnerability/Quality Smell**: Block data is serialized to JSON on-the-fly using the `simd_json::json!` macro. Rather than relying on a versioned schema that ensures serialization invariance, the code constructs an ad-hoc JSON structure with manual key-value definitions.

---

## Part 3: Production Security and Quality Findings

### Finding 4 (CRITICAL): Remote Command Injection in `stream_to_remote`
*   **File/Line**: `crates/op-snowball/src/snowball.rs:271-280`
*   **Impact**: Directly Exploitable.
*   **Description**: The function formats shell commands with `snapshot_path` and `remote_path` before passing them directly to `sh -c`. If `snapshot_name` or `remote_path` is derived from an untrusted user request (e.g. over a DBus or JSON-RPC interface), an attacker can supply shell metacharacters (such as `;`, `&`, `|`, or backticks) to execute arbitrary commands on the system.
*   **Remediation**: Use `std::process::Command` to invoke `btrfs` and `ssh` directly with vector arguments, entirely avoiding shell spawning via `sh -c`.

```rust
// Vulnerable Code:
let output = Command::new("sh")
    .arg("-c")
    .arg(format!(
        "btrfs send {} | ssh {} 'btrfs receive {}'",
        snapshot_path.display(),
        remote_path,
        remote_path
    ))
```

### Finding 5 (CRITICAL): Remote Command Injection in `stream_vectors` and `stream_to_replicas`
*   **File/Line**: `crates/op-snowball/src/streaming_snowball.rs:434-448` and `crates/op-snowball/src/streaming_snowball.rs:460-484`
*   **Impact**: Directly Exploitable.
*   **Description**: Similar to Finding 4, both functions compile raw shell command strings with unsanitized inputs (`block_hash` and `remote`/`replicas`) and pipe them directly into a shell interpreter (`bash -c`). An attacker manipulating these strings can execute arbitrary shell commands under the privilege level of the running daemon.
*   **Remediation**: Refactor the logic to invoke the processes (`btrfs`, `tee`, `ssh`) directly as distinct OS processes with structured arguments and pipe their standard input/output descriptors programmatically.

### Finding 6 (HIGH): Path Traversal via Unsanitized Parameters
*   **File/Line**: `crates/op-snowball/src/snowball.rs:252-258` and `crates/op-snowball/src/btrfs_numa_integration.rs:133-143`
*   **Impact**: High.
*   **Description**: `rollback` and `get_cached_block` take a user-supplied string (`snapshot_name` and `block_hash`) and append it directly using `.join()`. Because there is no check for directory traversal sequences (e.g., `../`), an attacker can craft a payload that resolves outside the snapshots or blocks directories, leading to arbitrary file system interactions.
*   **Remediation**: Sanitize inputs to ensure they contain only valid alphanumeric characters and do not contain path separators (`/`, `\`) or traversal elements (`..`).

### Finding 7 (MEDIUM): Unnecessary Use of `unsafe` Deserialization Blocks
*   **File/Line**: `crates/op-snowball/src/btrfs_numa_integration.rs:144` and `crates/op-snowball/src/snowball.rs:222`
*   **Impact**: Quality / Maintainability.
*   **Description**: Safe JSON parsing invocations are wrapped in `unsafe` blocks:
    ```rust
    let block_data: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut data)? };
    ```
    If these represent unsafe unchecked functions, parsing corrupted files from disk could result in memory corruption or undefined behavior. If they are standard safe functions, using unnecessary `unsafe` blocks violates safe Rust guidelines and dilutes codebase auditability.
*   **Remediation**: Remove `unsafe` wrappers and perform parsing using standard safe APIs. Ensure input mutability requirements are met cleanly.

---
## ⚠ Citation Warnings
- `crates/op-snowball/src/footprint.rs:146`: file has 142 lines
