# Production Security & Quality Audit: op-deployment

## Data Structures Audit

### Concurrency & Smart Pointer Diagnostics
Below is the precise count of concurrency and reference-counting primitives per source file:

| File | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell` | `.clone()` Calls |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `crates/op-deployment/src/image_manager.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 3 |
| `crates/op-deployment/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |

### Large Structs (> 5 Public Fields)
* **`ImageMetadata`** (`crates/op-deployment/src/image_manager.rs:20`): Has 7 public fields (`name`, `path`, `created`, `files`, `total_size`, `unique_size`, `symlinked_size`). Structs exceeding 5 public fields should be evaluated for refactoring into logically grouped sub-components (e.g., separating disk sizing telemetry from identification metadata).

### Globally Mutable State
No occurrences of globally mutable state (such as `static mut` or `lazy_static` with interior mutability) were found in the analyzed codebase.

---

## Schema-as-Code Analysis

### Ad-hoc JSON Serialization Contracts
* **File & Line**: `crates/op-deployment/src/image_manager.rs:20` and `crates/op-deployment/src/image_manager.rs:32`
* **Defect**: The system's deployment state data contracts (`ImageMetadata` and `FileEntry`) are expressed as ad-hoc Rust structs serialized directly to on-disk JSON files (`.image-metadata.json`). 
* **Impact**: This breaks the schema-as-code discipline. The lack of versioned schemas (such as Protocol Buffers or OSCAL component definition structures) makes schema evolution risky, lacks backward/forward compatibility guarantees, and prevents standard, external schema-validation tools from cryptographically verifying deployment package state manifests.

---

## Security & Quality Vulnerability Audit

### Critical Vulnerabilities

#### 1. Out-of-Bounds Memory Access / Undefined Behavior via Unpadded `simd-json` Deserialization
* **File & Line**: `crates/op-deployment/src/image_manager.rs:285` and `crates/op-deployment/src/image_manager.rs:303`
* **Vulnerability Type**: Memory Corruption / Buffer Overread
* **Description**: The functions `list_images` and `get_image` parse `.image-metadata.json` files using the `unsafe { simd_json::from_str(...) }` API. `simd-json` is an in-place, highly optimized parser that mutates the string buffer and **strictly requires** the input buffer to be padded with `simd_json::PADDING` (usually 32 or 64 bytes) beyond the end of the payload to avoid running off the end of the allocated memory during vector register (SIMD) execution. 
The input buffer here is populated via `async_fs::read_to_string`, which allocates a standard, unpadded Rust `String`.
* **Exploitability**: If a deployment metadata file on disk is malformed or maliciously modified (e.g., truncated to cause alignment issues), parsing it via the unsafe interface can result in an out-of-bounds read, segfaulting the process or potentially leaking adjacent heap memory.
* **Remediation**: Avoid using the `unsafe` block with standard strings. Instead, read the file into a mutable vector allocated with padding (`simd_json::to_vec`), or use standard `serde_json` for processing configuration/metadata files where SIMD parsing speeds do not justify undefined behavior risks.

#### 2. Path Traversal resulting in Arbitrary Directory Deletion
* **File & Line**: `crates/op-deployment/src/image_manager.rs:342` (also impacts `create_image` at line 74)
* **Vulnerability Type**: Path Traversal / Arbitrary File Deletion
* **Description**: The public API `delete_image` accepts an unsanitized `image_name: &str` parameter and joins it directly to the base directory:
  ```rust
  let image_path = self.images_dir.join(image_name);
  ```
  No validation is performed to ensure that `image_path` is contained within `self.images_dir`. If `image_name` contains path traversal sequences (e.g., `../../etc`), the resolved path will point outside the intended deployment sandbox.
* **Exploitability**: An authenticated attacker or a compromised workflow agent capable of initiating an image deletion could supply a payload like `../../var/log` or other directories. The manager will then invoke:
  ```rust
  async_fs::remove_dir_all(&image_path).await?;
  ```
  or shell out to `btrfs subvolume delete` (line 354), resulting in arbitrary recursive deletion of host filesystem directories.
* **Remediation**: Sanitize the incoming `image_name`. Strip path traversal sequences, ensure that any path components containing `..` or `.` are rejected, and verify that the canonicalized target path starts with the canonicalized `images_dir` prefix.

---

### High & Medium Vulnerabilities

#### 3. Denial of Service (DoS) via Full Memory File Ingestion
* **File & Line**: `crates/op-deployment/src/image_manager.rs:241`
* **Vulnerability Type**: Resource Exhaustion
* **Description**: The `calculate_file_hash` helper reads the entire content of target deployment files into memory at once:
  ```rust
  let contents = async_fs::read(file_path).await?;
  ```
* **Impact**: Deployment images frequently contain massive binary components, virtual disk files, or container filesystems that span gigabytes. Attempting to ingest these files entirely into memory to calculate a SHA256 checksum will cause extreme memory spikes or trigger an Out-Of-Memory (OOM) panic, crashing the control plane.
* **Remediation**: Use a streaming hasher. Open the file as a `tokio::fs::File`, read it sequentially into a fixed-size buffer (e.g., 64KB), and update the `Sha256` digest iteratively.

#### 4. TOCTOU (Time-of-Check to Time-of-Use) Race Condition in Symlink Verification
* **File & Line**: `crates/op-deployment/src/image_manager.rs:211`
* **Vulnerability Type**: Race Condition (TOCTOU)
* **Description**: `find_file_in_previous_images` attempts to safely traverse previous images to find deduplication targets. However, it performs multiple sequential, non-atomic metadata operations:
  1. `async_fs::metadata(&file_path).await.is_ok()` (Line 211)
  2. `async_fs::symlink_metadata(&file_path).await` (Line 213)
  3. `async_fs::read_link(&file_path).await` (Line 216)
* **Impact**: If a directory or symlink is modified concurrently on the host filesystem during these checks, the status of the path may change between validation and use. This could cause the deployment manager to create invalid symlinks or resolve path targets incorrectly.
* **Remediation**: Perform only a single `symlink_metadata` call to inspect the file attributes. If it is a symlink, call `read_link` directly and handle errors, avoiding redundant probing with `metadata()`.