# Production Quality and Security Audit: `op-deployment`

## 1. Observability Profile

### Macro Usage Count
The crate `op-deployment` relies entirely on the standard `log` crate rather than the `tracing` crate, despite `tracing` being declared in its dependencies.

* **`tracing::` macros**: 0
* **`println!` macros**: 0
* **`log::` macros**: 10
  * `log::info!`: 6 (Lines 73, 104, 198, 294, 309, 417)
  * `log::warn!`: 2 (Lines 75, 399)
  * `log::debug!`: 2 (Lines 134, 166)
  * `log::error!`: 0

### Swallowed Errors
Errors are silently ignored or swallowed without logging or propagation in the following locations:
* **`crates/op-deployment/src/image_manager.rs:323-328`**: During `list_images`, if `simd_json::from_str` fails to deserialize `.image-metadata.json`, the image is silently omitted from the returned list. There is no `log::warn!` or `log::error!` diagnostic alerting the operator to metadata corruption.
* **`crates/op-deployment/src/image_manager.rs:356-358`**: In `get_streamable_snapshot`, if `entry.metadata().await` fails or the retrieved metadata's creation timestamp cannot be read (`metadata.created()`), the failure is silently bypassed in the nested `if let Ok(...)` clauses. Operators will have no visibility into why valid snapshots are not identified as streamable.

### Leakage of PII or Secrets in Logs
* **`crates/op-deployment/src/image_manager.rs:134`**: Logs the exact symlink target path: `log::debug!("Symlinking {} from previous image", file_name);`.
* **`crates/op-deployment/src/image_manager.rs:166`**: Logs the exact copied filename: `log::debug!("Copying new file: {}", file_name);`.
* **Risk**: If the deployment system packages assets containing credential/key material (e.g. `id_rsa`, `prod_db_secret.json`) or user-specific datasets containing PII, these sensitive names and metadata are exposed in the system logs.

### Metrics Instrumentation
No metrics instrumentation exists within the `op-deployment` crate. Despite `prometheus` being present in the workspace cargo dependencies, the `ImageManager` lacks counters or gauges tracking:
* Snapshot/image generation duration.
* Unique vs. symlinked data size metrics.
* Streaming throughput rates.

---

## 2. Schema-As-Code Compliance
* **`crates/op-deployment/src/image_manager.rs:17-40`**: The structs `ImageMetadata` and `FileEntry` define the structure of the serialized metadata saved to disk as `.image-metadata.json`. This is an **ad-hoc schema** written directly as Rust structs. It violates the schema-as-code discipline as it is not derived from a declarative, versioned format such as Protocol Buffers or OSCAL. Changes to these structs risk silent breaking changes to the state on disk when upgrading orchestrator versions.

---

## 3. Security Findings

### CRITICAL: Unsafe `simd-json` Deserialization of Unvalidated JSON from Disk
* **Reference**: `crates/op-deployment/src/image_manager.rs:324-325` and `crates/op-deployment/src/image_manager.rs:342-343`.
* **Impact**: Memory Corruption / Arbitrary Code Execution.
* **Exploitation / Threat Model**: 
  The `ImageManager` loads `.image-metadata.json` files from disk and passes the mutable content buffer to `simd_json::from_str` within an `unsafe` block:
  ```rust
  let mut content = async_fs::read_to_string(&metadata_path).await?;
  if let Ok(metadata) =
      unsafe { simd_json::from_str::<ImageMetadata>(&mut content) }
  ```
  `simd-json`'s unsafe parsing functions require strict input string padding and alignment constraints to prevent memory out-of-bounds reads/writes during SIMD processing. If a local attacker, a compromised container with volume access, or a malicious deployment package manipulates the `.image-metadata.json` file on disk, they can feed malformed payloads to the unsafe parser. This directly bypasses safe Rust protections and triggers segmentation faults or arbitrary code execution with the permissions of the orchestrator process.
* **Remediation**:
  Use `simd_json::serde::from_str` (or the safe wrapper API) or switch to standard `serde_json::from_str` for files sourced from unvalidated disk operations. Avoid using raw `unsafe simd_json` methods on un-padded strings read directly via `read_to_string`.