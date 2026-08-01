### 1. Environment Variable Audits (`std::env::var` reads)

The following table lists all reads of `std::env::var` in the `op-blockchain` crate:

| File | Line | Environment Variable | Default Value | Fallback / Error Handling Method |
| :--- | :--- | :--- | :--- | :--- |
| `crates/op-blockchain/src/retention.rs` | 63 | `OPDBUS_RETAIN_HOURLY` | `5` | `.ok().and_then(...).unwrap_or(5)` |
| `crates/op-blockchain/src/retention.rs` | 67 | `OPDBUS_RETAIN_DAILY` | `5` | `.ok().and_then(...).unwrap_or(5)` |
| `crates/op-blockchain/src/retention.rs` | 71 | `OPDBUS_RETAIN_WEEKLY` | `5` | `.ok().and_then(...).unwrap_or(5)` |
| `crates/op-blockchain/src/retention.rs` | 75 | `OPDBUS_RETAIN_QUARTERLY` | `5` | `.ok().and_then(...).unwrap_or(5)` |
| `crates/op-blockchain/src/snapshot.rs` | 32 | `OPDBUS_SNAPSHOT_INTERVAL` | `"every-15-minutes"` | `.unwrap_or_else(|_| "every-15-minutes".to_string())` |
| `crates/op-blockchain/src/blockchain.rs` | 444 | `OPDBUS_STATE_SNAPSHOT_PREFIX` | `"SNP-state"` | `.unwrap_or_else(|_| "SNP-state".to_string())` |
| `crates/op-blockchain/src/streaming_blockchain.rs` | 74 | `OPDBUS_RETAIN_HOURLY` | `5` | `.ok().and_then(...).unwrap_or(5)` |
| `crates/op-blockchain/src/streaming_blockchain.rs` | 78 | `OPDBUS_RETAIN_DAILY` | `5` | `.ok().and_then(...).unwrap_or(5)` |
| `crates/op-blockchain/src/streaming_blockchain.rs` | 82 | `OPDBUS_RETAIN_WEEKLY` | `5` | `.ok().and_then(...).unwrap_or(5)` |
| `crates/op-blockchain/src/streaming_blockchain.rs` | 86 | `OPDBUS_RETAIN_QUARTERLY` | `5` | `.ok().and_then(...).unwrap_or(5)` |
| `crates/op-blockchain/src/streaming_blockchain.rs` | 104 | `OPDBUS_SNAPSHOT_INTERVAL` | `"every-15-minutes"` | `.unwrap_or_else(|_| "every-15-minutes".to_string())` |
| `crates/op-blockchain/src/streaming_blockchain.rs` | 538 | `OPDBUS_STATE_SNAPSHOT_PREFIX` | `"SNP-state"` | `.unwrap_or_else(|_| "SNP-state".to_string())` |

#### Flagged Environment Variables (No Default / No Error Handling)
*   **None.** All environment variables processed by `op-blockchain` safely invoke `unwrap_or`, `unwrap_or_else`, or mapping operations that handle `Result::Err` and provide valid production defaults.

---

### 2. Cargo Features & Additivity

From `crates/op-blockchain/Cargo.toml`:

```toml
[features]
default = []
ml = []
```

#### Additivity Analysis
Yes, the Cargo features are fully additive. The `default` features array is empty, ensuring that unless explicitly specified, compilation remains minimal. 
*   The `ml` feature gates model vectorization integration in `crates/op-blockchain/src/plugin_footprint.rs`. When not enabled, the system falls back gracefully to a localized heuristic-based vector generation, satisfying Cargo's additive feature model without causing build breakages.

---

### 3. Hardcoded Paths, Ports, and Addresses

The following hardcoded paths and remote locations were identified:

*   **`crates/op-blockchain/src/streaming_blockchain.rs:318`**: The remote path `'/var/lib/blockchain/vectors/'` is hardcoded inside the remote `ssh` call to `btrfs receive`.
*   **`crates/op-blockchain/src/streaming_blockchain.rs:342`**: The remote path `'/var/lib/blockchain/vectors/'` is hardcoded as the target destination for replica streaming.
*   **`crates/op-blockchain/src/btrfs_numa_integration.rs:111` & `136`**: The directory structures `"blocks"` and `"by-hash"` are hardcoded cache directory paths relative to the cache root.
*   **`crates/op-blockchain/src/blockchain.rs:43` & `44` & `45`**: Relative subvolume directory names `"timing"`, `"vectors"`, and `"state"` are hardcoded.

No hardcoded ports or IP addresses were identified in the source files.

---

### 4. Schema-as-Code Violations

The codebase has a declared schema-as-code discipline using Protocol Buffers and OSCAL. However, the audit revealed extensive use of ad-hoc JSON structures, manual deserialization, and untyped key-value maps:

*   **`crates/op-blockchain/src/footprint.rs:14`**: `BlockEvent` uses `simd_json::OwnedValue` for its primary payload (`pub data`). This allows arbitrary, non-validated JSON payloads to bypass versioned schemas.
*   **`crates/op-blockchain/src/footprint.rs:47`** and **`crates/op-blockchain/src/plugin_footprint.rs:11`**: `PluginFootprint` represents its metadata using `HashMap<String, simd_json::OwnedValue>`. This represents an unversioned ad-hoc data contract.
*   **`crates/op-blockchain/src/btrfs_numa_integration.rs:98-106`**: Defines an ad-hoc anonymous serialization schema using the `simd_json::json!` macro.
*   **`crates/op-blockchain/src/btrfs_numa_integration.rs:142-162`**: Performs manual structural validation and extraction on untyped `simd_json::OwnedValue` elements (e.g., matching string values like `plugin_id` and map fields manually), which is prone to contract drift.
*   **`crates/op-blockchain/src/streaming_blockchain.rs:207-214`**: Constructs an ad-hoc Timing schema with JSON primitives instead of a defined, structured protobuf event structure.
*   **`crates/op-blockchain/src/streaming_blockchain.rs:218-228`**: Manually packages vector data with metadata fields in an ad-hoc JSON representation.

---

### 5. Critical Vulnerabilities (Directly Exploitable)

#### CRITICAL: Shell Command Injection via Raw String Interpolation
*   **Location**: `crates/op-blockchain/src/blockchain.rs:230`
*   **Location**: `crates/op-blockchain/src/streaming_blockchain.rs:315-321`
*   **Location**: `crates/op-blockchain/src/streaming_blockchain.rs:337-347`

##### Description
The streaming blockchain implementation uses raw shell execution via `sh -c` or `bash -c` with direct string interpolation (`format!`) of user-controlled parameters (`remote_path`, `remote`, and `replicas`).

Example from `crates/op-blockchain/src/streaming_blockchain.rs`:
```rust
let output = Command::new("bash")
    .arg("-c")
    .arg(format!(
        "btrfs send {} | ssh {} 'btrfs receive /var/lib/blockchain/vectors/'",
        vector_snapshot.display(),
        remote
    ))
```

If an attacker controls the `remote` host parameter or the elements inside the `replicas` string slice, they can inject shell metacharacters (such as `;`, `&&`, or backticks) to execute arbitrary shell commands with the privileges of the running application.

Example attack string for `remote` / `replicas`:
`"localhost; rm -rf /; #"`

This allows immediate remote code execution (RCE) on the control plane.

##### Remediation
Do not use `sh -c` or `bash -c` with string formatting. Instead, execute system binaries directly by passing arguments as independent vectors in `Command::args`. Pipe output streams using Rust's `Stdio::piped()` rather than delegating piping behavior to the system shell:

```rust
// Safe direct execution pattern:
let mut send_child = Command::new("btrfs")
    .args(["send"])
    .arg(&vector_snapshot)
    .stdout(Stdio::piped())
    .spawn()?;

let send_stdout = send_child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdout"))?;

let mut ssh_child = Command::new("ssh")
    .arg(remote) // Passed safely as a single argument
    .arg("btrfs receive /var/lib/blockchain/vectors/")
    .stdin(send_stdout)
    .spawn()?;
```