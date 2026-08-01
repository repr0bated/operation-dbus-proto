| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `unsafe_block` / `simd_json_from_str` | `crates/op-deployment/src/image_manager.rs:335` | Uses `unsafe { simd_json::from_str(...) }` without documenting safety invariants. | Use safe parsing interfaces (`serde_json`) or explicitly document the safety contract when mutating buffers. | Undocumented unsafe block; risks UB if input string is mutated and reused or contains invalid UTF-8. | Minor Gap |
| `unsafe_block` / `simd_json_from_str` | `crates/op-deployment/src/image_manager.rs:355` | Deserializes JSON via raw `unsafe` block parsing on mutable string reference. | Use safe parser APIs or document why `unsafe` optimizations are strictly necessary. | Undocumented unsafe block; string buffer is modified in place, which requires careful boundary handling. | Minor Gap |
| `command_new` | `crates/op-deployment/src/image_manager.rs:73` | Calls external shell command `findmnt` to determine filesystem type. | Use native library bindings or Rust filesystem APIs to query filesystem traits. | Shells out to system utilities; introduces external runtime dependency and platform-specific brittleness. | Minor Gap |
| `command_new` | `crates/op-deployment/src/image_manager.rs:302` | Executes `btrfs subvolume snapshot` via external subprocess execution. | Leverage native OS system calls or specialized crates if available; otherwise wrap commands with robust validation. | Hard dependency on `btrfs` CLI binary presence and system-level permissions. | Minor Gap |
| `command_new` | `crates/op-deployment/src/image_manager.rs:409` | Spawns `btrfs subvolume delete` subprocess in production cleanup code. | Handle command execution failures, parsing errors, and non-zero exit codes explicitly. | Spawns system binaries; risks runtime failure if `btrfs` command-line interface changes. | Minor Gap |
| `command_new` | `crates/op-deployment/src/image_manager.rs:428` | Spawns `btrfs subvolume delete` to clean up base path snapshots. | Leverage structured system automation abstractions rather than raw CLI invocations. | No validation of executable path; hard dependence on system environment environment paths. | Minor Gap |
| `format_json_manual` | `crates/op-deployment/src/image_manager.rs:141` | Allocates error strings eagerly using `format!` inside `.context()`. | Use lazy evaluation or structured error types (e.g., `thiserror`) to avoid heap allocations on success paths. | Eager allocation of debug strings on active execution paths. | Minor Gap |
| `format_json_manual` | `crates/op-deployment/src/image_manager.rs:149` | Constructs context messages eagerly via `format!` during file copying. | Prefer structured error contexts or lazily evaluated error closures. | Unnecessary heap allocation on execution hot path. | Minor Gap |
| `format_json_manual` | `crates/op-deployment/src/image_manager.rs:169` | Eagerly formats file path names into dynamic error messages. | Use structured errors to decouple error representation from formatting. | Eager heap allocations inside loop contexts. | Minor Gap |
| `format_json_manual` | `crates/op-deployment/src/image_manager.rs:290` | Manually formats cryptographic hash output using lower-hex specifier. | Leverage standard serialization or wrapper formatting structs. | Manual ad-hoc formatting of internal states. | Minor Gap |
| `format_json_manual` | `crates/op-deployment/src/image_manager.rs:297` | Generates snapshot names using manually formatted timestamp strings. | Use a structured naming schema or config-driven naming pattern. | Dynamic string generation with potential collision risk. | Minor Gap |
| `unwrap_expect` | `crates/op-deployment/src/image_manager.rs:136` | Invokes `.unwrap()` on `Path::parent()`. | Implement error propagation or use `.ok_or_else()` to handle root paths gracefully. | Panics at runtime if the destination path is root or empty. | Major Gap |
| `unwrap_expect` | `crates/op-deployment/src/image_manager.rs:186` | Invokes `.unwrap()` on `Vec::last()`. | Check if the collection is empty before accessing elements, or return a structured error. | Potential panic if the parsed metadata file list is empty. | Major Gap |
| `unwrap_expect` | `crates/op-deployment/src/image_manager.rs:265` | Invokes `.unwrap()` on `Path::parent()` inside symbolic link target calculations. | Use structured propagation of directory extraction failures. | Potential runtime panic if path resolution reaches the root system boundary. | Major Gap |
| `unwrap_expect` | `crates/op-deployment/src/image_manager.rs:381` | Invokes `.unwrap()` on `SystemTime::duration_since()`. | Gracefully handle clock skew by using `unwrap_or` or wrapping the conversion. | Potential panic if the system clock is set to a time before `UNIX_EPOCH`. | Major Gap |
| `unwrap_expect` | `crates/op-deployment/src/image_manager.rs:455` | Invokes `.unwrap()` on temporary directory initialization in tests. | Use test helper annotations or propagate errors via `Result` in test signatures. | Panics in test suite initialization rather than returning clean test results. | Minor Gap |
| Schema-As-Code | `crates/op-deployment/src/image_manager.rs:335`, `355` | Uses ad-hoc JSON structure (`ImageMetadata`) for disk-backed configuration contracts. | Express data contracts using strongly typed, versioned schemas like Protocol Buffers or OpenAPI specs. | Ad-hoc serialization is prone to breaking changes across deployments. | Major Gap |

---

### Actionable Recommendations for Major & Critical Gaps

#### 1. Eliminate Runtime Panics from Unsafe Path/Collection Operations
* **File:** `crates/op-deployment/src/image_manager.rs:136, 186, 265, 381`
* **Issue:** Direct usage of `.unwrap()` on `dest_path.parent()`, `image_metadata.files.last()`, and `SystemTime::duration_since()` can cause panic conditions if unexpected inputs are processed (e.g., path is a root directory, configuration file list is empty, or the system clock experiences backwards skew).
* **Remediation:** Replace raw unwraps with structured error types or `anyhow::Context` mapping. For example:
  ```rust
  // Line 136
  let parent = dest_path.parent()
      .ok_or_else(|| anyhow::anyhow!("Destination path has no parent directory: {}", dest_path.display()))?;

  // Line 186
  let last_file = image_metadata.files.last()
      .ok_or_else(|| anyhow::anyhow!("Image metadata contains empty file list"))?;
  
  // Line 381
  let timestamp = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_secs() as i64)
      .unwrap_or(0); // Safely handle backward clock skew
  ```

#### 2. Establish Schema-As-Code Discipline for Image Metadata Configuration
* **File:** `crates/op-deployment/src/image_manager.rs:335, 355`
* **Issue:** The deployment manager reads and writes metadata using unstructured, ad-hoc JSON definitions (`ImageMetadata`). Changes to this struct can cause serialization mismatches between different system versions, leading to deployment failures.
* **Remediation:** Migrated the data definitions to a formal, versioned contract (e.g., Protocol Buffers or an OpenAPI schema). Generate the Rust struct from this single source of truth, ensuring that backwards compatibility guarantees and migration tracks are programmatically validated. Add an explicit schema/api version field to the serialized format to prevent parsing incompatibility.