# Test Suite Analysis

### Test Configurations and Attributes
Unit tests are declared using `#[cfg(test)]` modules containing test functions with `#[tokio::test]` and `#[test]` attributes across various core modules of the `op-cache` crate. 

### Integration Tests
No integration tests in a dedicated `tests/` directory are provided in the audited file list. All reviewed tests are inline unit tests.

### Test Functions Count
There are **40** test functions defined in total across the provided files:
*   `agent_registry.rs`: 4 tests
*   `btrfs_cache.rs`: 1 test
*   `orchestrator.rs`: 3 tests
*   `pattern_tracker.rs`: 3 tests
*   `snapshot_manager.rs`: 1 test
*   `workflow_cache.rs`: 6 tests
*   `workflow_executor.rs`: 5 tests
*   `workflow_tracker.rs`: 4 tests
*   `workstack_cache.rs`: 4 tests
*   `capability_resolver.rs`: 6 tests
*   `numa.rs`: 3 tests

### Representative Test List
The following three tests represent different system layers of the codebase:
1.  **Agent Registration Unit Test**: `crates/op-cache/src/agent_registry.rs:451` (`async fn test_agent_registration()`)
2.  **L3 Latch / Cryptographic Hash Verification**: `crates/op-cache/src/btrfs_cache.rs:540` (`async fn test_text_hashing()`)
3.  **NUMA Hardware CPU Range Parsing**: `crates/op-cache/src/numa.rs:442` (`fn test_parse_cpu_range()`)

### Property Testing and Fuzzing
No property-based tests (e.g., using `proptest` or `quickcheck`) or fuzzing harnesses are implemented or specified as dependencies in the provided `op-cache` cargo manifests.

---

# Security & Quality Audit Findings

### Finding 1: Shell Command Injection in BTRFS Replication Utilities
*   **Severity**: High
*   **Location**: `crates/op-cache/src/btrfs_cache.rs:432` and `crates/op-cache/src/btrfs_cache.rs:467`
*   **Description**: 
    The replication helpers `stream_to_remote` and `receive_from_remote` format dynamic string parameters (`remote_host`, `remote_path`, `remote_snapshot`, and `local_path`) directly into shell command templates:
    ```rust
    let cmd = format!(
        "btrfs send {} | ssh {} 'btrfs receive {}'",
        snapshot_path.display(),
        remote_host,
        remote_path
    );
    ```
    This constructed string is subsequently passed directly to a raw shell process execution:
    ```rust
    let output = tokio::process::Command::new("bash")
        .arg("-c")
        .arg(&cmd)
        ...
    ```
    If an attacker is able to pass inputs containing shell metacharacters (such as `;`, `&&`, or `|`) into any of these fields, they can execute arbitrary system commands with the privileges of the running process. Although these functions are not directly called by the audited gRPC services, they are exposed as public methods of `BtrfsCache`.
*   **Remediation**: 
    Avoid passing raw formatted strings to `bash -c`. Instead, execute `ssh` and `btrfs` as structured sub-processes with explicitly bounded argument arrays via `tokio::process::Command`, and programmatically pipe their stdout/stdin descriptors.

---

### Finding 2: Unsafe SIMD JSON Parsing over Unpadded Database Strings
*   **Severity**: Medium
*   **Location**: `crates/op-cache/src/pattern_tracker.rs:211` and `crates/op-cache/src/workflow_tracker.rs:349`
*   **Description**: 
    The codebase deserializes agent sequences from database results using `simd_json::from_str` within an `unsafe` block:
    ```rust
    let agent_sequence: Vec<String> =
        unsafe { simd_json::from_str(&mut agent_sequence_json) }
            .unwrap_or_default();
    ```
    The `simd-json` parser mutates strings in-place and relies heavily on specific compiler SIMD alignment and trailing memory padding guarantees (specifically `simd_json::SIMDJSON_PADDING`). SQLite-allocated strings (`agent_sequence_json`) lack this specialized allocation padding. Feeding unpadded, arbitrarily sized database strings to an unsafe SIMD deserializer can trigger out-of-bounds reads/writes, memory corruption, or unpredictable segmentation faults.
*   **Remediation**: 
    Convert the SQLite-sourced string into a padded buffer or use `simd_json::from_slice` over a vector constructed with safe padding. Alternatively, use a safe, standard JSON parser (e.g., `serde_json`) for database fields where memory layout cannot be strictly managed.

---

### Finding 3: Ad-Hoc Data Contracts and Schema-as-Code Violations
*   **Severity**: Medium (Architecture Quality)
*   **Location**:
    *   `crates/op-cache/src/grpc/mcp_service.rs:330-388` (MCP JSON-RPC structs: `ToolCallParams`, `McpContentResponse`, `McpContent`, `McpToolsListResult`, `McpToolJson`, `McpInitializeResult`, `McpServerCapabilities`, `McpToolCapability`, `McpServerInfo`)
    *   `crates/op-cache/src/orchestrator.rs:33-58` (`OrchestrationResult` and `StepResult` structs)
    *   `crates/op-cache/src/workflow_executor.rs:47-73` (`StepResult` and `WorkflowResult` structs)
    *   `crates/op-cache/src/workflow_tracker.rs:53-62` (`WorkflowPattern` and `PromotionSuggestion` structs)
*   **Description**: 
    The audited codebase contains several ad-hoc Rust structs that serve as serialization and communication contracts across network boundaries (specifically for JSON-RPC and Model Context Protocol APIs), instead of utilizing versioned schemas generated via Protocol Buffers. This breaks the crate's unified schema-as-code discipline, risking data model drift and compatibility errors as components scale.
*   **Remediation**: 
    Formulate versioned Protocol Buffer messages for MCP payloads and execution step results in the `.proto` files, and compile them to generated structures using `prost`/`tonic` to ensure type and contract consistency.