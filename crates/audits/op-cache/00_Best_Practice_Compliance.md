| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `format_json_manual` | `crates/op-cache/src/agent_registry.rs:434` | Eagerly evaluates string formatting with `context(format!(...))`. | Use lazy evaluation `with_context(|| format!(...))` to avoid allocation overhead when the error path is not taken. | Eager allocation of error string. | Minor Gap |
| `unwrap_expect` | `crates/op-cache/src/agent_registry.rs:478` | Raw `.unwrap()` on an async database call. | Propagate errors with `?` or use `.expect("descriptive message")` for diagnostic safety. | Lack of context on assertion failures. | Minor Gap |
| `unwrap_expect` | `crates/op-cache/src/agent_registry.rs:482` | Checked unwrap via `assert!(retrieved.is_some())` followed by `.unwrap()`. | Destructure with `if let` / pattern matching, or utilize `.expect()`. | Redundant check followed by panic-prone unwrap. | Minor Gap |
| `unwrap_expect` | `crates/op-cache/src/agent_registry.rs:499` | Raw `.unwrap()` on an async call. | Map errors cleanly or use `expect()` with a descriptive reason. | Potential unhandled panic in async execution context. | Minor Gap |
| `unwrap_expect` | `crates/op-cache/src/agent_registry.rs:503` | Raw `.unwrap()` on an async call. | Map errors cleanly or use `expect()` with a descriptive reason. | Unhandled panic vector. | Minor Gap |
| `command_new` | `crates/op-cache/src/btrfs_cache.rs:71` | Calls `tokio::process::Command::new("btrfs")` with raw string name. | Invoke external binaries using absolute paths and verify system configuration. | Use of unvalidated relative system PATH binary reference. | Minor Gap |
| `command_new` | `crates/op-cache/src/btrfs_cache.rs:652` | Invokes `Command::new("bash").arg("-c").arg(&cmd)` with dynamically formatted command string. | Avoid shell invocation (`bash -c`) entirely; pass structured parameters directly via `args` to prevent command injection. | **High Risk**: Dynamic command execution via shell interpreter makes it vulnerable to argument injection. | Major Gap |
| `command_new` | `crates/op-cache/src/btrfs_cache.rs:688` | Invokes `Command::new("bash").arg("-c").arg(&cmd)` with dynamically formatted command string. | Avoid shell invocation (`bash -c`); use direct argument arrays with explicit binaries. | **High Risk**: Vulnerable to command injection if parameters contain dynamic filesystem paths or metadata. | Major Gap |
| `command_new` | `crates/op-cache/src/btrfs_cache.rs:786` | Executes `taskset` via relative path binary name with raw string formatting. | Standardize external binary execution using direct argument structures and absolute path checking. | Potential path-hijacking risk on untrusted runtime environments. | Minor Gap |
| `format_json_manual` | `crates/op-cache/src/btrfs_cache.rs:380` | Formats hash values manually using `format!("{:x}", ...)`. | Idiomatic standard library hex formatting. | Matches standard patterns. | Compliant |
| `format_json_manual` | `crates/op-cache/src/btrfs_cache.rs:402` | Eagerly evaluates formatting in `.context(format!(...))`. | Use lazy closure `with_context(|| format!(...))` to defer string formatting until error generation is guaranteed. | Eager allocation of error diagnostic strings on success path. | Minor Gap |
| `format_json_manual` | `crates/op-cache/src/btrfs_cache.rs:417` | Generates a vector filename string using `format!`. | Typical dynamic string generation. | Matches standard path construction patterns. | Compliant |
| `format_json_manual` | `crates/op-cache/src/btrfs_cache.rs:638` | Maps errors using eager `map_err(|e| format!(...))`. | Use structured custom error enums or lazy formatted string templates. | Allocation of dynamic error string regardless of whether it is consumed. | Minor Gap |
| `unwrap_expect` | `crates/op-cache/src/btrfs_cache.rs:384` | Raw `.unwrap()` on standard library Mutex lock. | Handle lock poisoning gracefully or map to an application error context. | Thread panic in case of poisoned mutex state. | Minor Gap |
| `std_fs_in_async` | `crates/op-cache/src/btrfs_cache.rs:85` | Uses `tokio::fs::create_dir_all`. | Avoid blocking OS filesystem calls in async tokio runtime. | Correctly uses async tokio fs API. | Compliant |
| `std_fs_in_async` | `crates/op-cache/src/btrfs_cache.rs:100` | Uses `tokio::fs::create_dir_all`. | Use async alternatives to prevent blocking worker threads. | Correctly uses async tokio fs API. | Compliant |
| `std_fs_in_async` | `crates/op-cache/src/btrfs_cache.rs:111` | Uses `tokio::fs::create_dir_all`. | Prevent blocking file system IO in tokio async context. | Correctly uses async tokio fs API. | Compliant |
| `std_fs_in_async` | `crates/op-cache/src/btrfs_cache.rs:112` | Uses `tokio::fs::create_dir_all`. | Prevent blocking file system IO in tokio async context. | Correctly uses async tokio fs API. | Compliant |
| `std_fs_in_async` | `crates/op-cache/src/btrfs_cache.rs:113` | Uses `tokio::fs::create_dir_all`. | Prevent blocking file system IO in tokio async context. | Correctly uses async tokio fs API. | Compliant |
| `unsafe_block` | `crates/op-cache/src/pattern_tracker.rs:246` | Uses `unsafe { simd_json::from_str(...) }` to parse JSON sequences from dynamic text variables. | Document unsafe invariants via `// SAFETY:` comments. Validate in-place buffer mutability/alignment. | Missing safety documentation for parsing mutably-shared string buffers. | Minor Gap |
| `simd_json_from_str` | `crates/op-cache/src/pattern_tracker.rs:246` | Parses data contract as ad-hoc JSON array string (`agent_sequence_json`) using raw `simd_json`. | **Schema-as-Code**: Data structures must be defined as versioned schemas (e.g., Protobuf/OSCAL) rather than raw JSON strings. | Violates schema-as-code discipline by parsing ad-hoc JSON strings inside database columns. | Major Gap |
| `command_new` | `crates/op-cache/src/snapshot_manager.rs:57` | Runs `Command::new("btrfs")` with relative executable name. | Use absolute path configurations and handle execution errors strictly. | Use of relative command execution PATH context. | Minor Gap |
| `unsafe_block` | `crates/op-cache/src/workflow_tracker.rs:403` | Uses `unsafe { simd_json::from_str(...) }` without safety proofs. | Guarantee mutability and padding requirements for `simd_json` parsing in safety documentation. | Missing required `// SAFETY:` commentary. | Minor Gap |
| `simd_json_from_str` | `crates/op-cache/src/workflow_tracker.rs:403` | Deserializes `agent_sequence_json` as an ad-hoc JSON string directly in business logic. | **Schema-as-Code**: Store and exchange contracts as structured Protobuf schemas. | Uses unstructured ad-hoc JSON data contract representation. | Major Gap |
| `unsafe_block` | `crates/op-cache/src/workflow_tracker.rs:447` | Uses `unsafe { simd_json::from_str(...) }` for dynamic parsing. | Explicit safety comments verifying allocation constraints. | Missing validation documentation. | Minor Gap |
| `simd_json_from_str` | `crates/op-cache/src/workflow_tracker.rs:447` | Parses JSON representation of sequence list directly from database record. | **Schema-as-Code**: Ad-hoc strings must be migrated to structured, versioned schemas. | Ad-hoc serialization instead of structured Protobuf or OSCAL schemas. | Major Gap |
| `unsafe_block` | `crates/op-cache/src/workflow_tracker.rs:477` | Uses `unsafe { simd_json::from_str(...) }`. | Guarantee internal layout constraints are satisfied. | Missing safety verification comments. | Minor Gap |
| `simd_json_from_str` | `crates/op-cache/src/workflow_tracker.rs:477` | Parses ad-hoc sequence strings directly without strict schemas. | **Schema-as-Code**: Strict enforcement of versioned schemas for persistence. | Violates Schema-as-Code requirement using unstructured JSON strings. | Major Gap |

---

### Actionable Recommendations for Major / Critical Gaps

#### 1. Replace Dangerous Shell Execution (`bash -c`) with Direct Execution
* **Citations**: `crates/op-cache/src/btrfs_cache.rs:652`, `crates/op-cache/src/btrfs_cache.rs:688`
* **Vulnerability Analysis**: Invoking `bash -c` with string-formatted commands executes the system's command shell interpreter. If filenames, paths, or subvolume names contain space characters, quotes, or semi-colons, it can alter the command structure or execute arbitrary commands.
* **Remediation**: Execute `btrfs` or the desired binary directly by passing parameters as structured arguments to `Command::args`. Avoid dynamic string concatenation.
```rust
// Instead of tokio::process::Command::new("bash").arg("-c").arg(&cmd)...
let output = tokio::process::Command::new("/usr/sbin/btrfs")
    .args(["subvolume", "snapshot", "-r", &source_path, &destination_path])
    .output()
    .await?;
```

#### 2. Align Data Serialization with Schema-as-Code Discipline (Replace Ad-hoc JSON with Protobuf/OSCAL)
* **Citations**: 
  * `crates/op-cache/src/pattern_tracker.rs:246`
  * `crates/op-cache/src/workflow_tracker.rs:403`
  * `crates/op-cache/src/workflow_tracker.rs:447`
  * `crates/op-cache/src/workflow_tracker.rs:477`
* **Deficiency**: Storing, retrieving, and parsing unstructured JSON arrays representing list sequences (`agent_sequence`) bypasses the project’s schema-as-code discipline. This makes forward/backward compatibility difficult and relies on dynamic parsing (e.g. `simd-json`) in unsafe blocks.
* **Remediation**:
  1. Define a versioned Protocol Buffer schema (e.g., `agent_sequence.proto`) or structured OSCAL data model representing these sequence execution logs.
  2. Store binary-serialized Protobuf blocks or structured schemas in the database.
  3. Deserialise the payloads using generated safe Protobuf bindings, removing unsafe memory operations from the business logic.