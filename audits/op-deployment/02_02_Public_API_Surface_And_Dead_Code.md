# Production Security and Quality Audit

## Section 1: Security Audit & Critical Vulnerabilities

### [CRITICAL] Memory Corruption & Undefined Behavior via Unsafe `simd_json::from_str` on Unpadded Rust `String`
* **Citation**: `crates/op-deployment/src/image_manager.rs:319` and `crates/op-deployment/src/image_manager.rs:340`
* **Vulnerability Analysis**: 
  The metadata parsing logic uses the unsafe `simd_json::from_str` function on a standard Rust `String` returned by `tokio::fs::read_to_string`. `simd-json` utilizes advanced vector instructions (AVX2/SSE) for high-performance parsing and strictly requires that the input string buffer be padded with a minimum of 32 or 64 bytes (`simd_json::PADDING`) at the end of the allocation. 
  
  Passing a standard unpadded `String` allocation to `simd_json::from_str` forces SIMD instructions to perform out-of-bounds memory reads and write operations beyond the allocated boundaries of the heap buffer. This directly results in undefined behavior (UB), heap corruption, or immediate segmentation faults.
  
  This vulnerability is directly exploitable because the deployment manager loads these `.image-metadata.json` files from disk when listing or retrieving images. An attacker who can write a malformed metadata JSON file can trigger memory corruption, potentially leading to arbitrary code execution within the orchestrator daemon.
* **Remediation**:
  Avoid using `simd_json::from_str` directly on unpadded standard Strings. Instead, read the file into a mutable vector of bytes and use the padding utility, or swap `simd-json` with the safer standard `serde_json` for file-based configuration loading:
  ```rust
  let content = async_fs::read(&metadata_path).await?;
  let metadata: ImageMetadata = serde_json::from_slice(&content)?;
  ```

---

### [HIGH] Path Traversal and Arbitrary File Destruction via Unsanitized `image_name`
* **Citation**: `crates/op-deployment/src/image_manager.rs:96`, `crates/op-deployment/src/image_manager.rs:336`, and `crates/op-deployment/src/image_manager.rs:384`
* **Vulnerability Analysis**: 
  The methods `create_image`, `get_image`, and `delete_image` accept a string slice parameter `image_name` and directly resolve filesystem paths by joining it onto `self.images_dir`:
  ```rust
  let image_path = self.images_dir.join(image_name);
  ```
  The parameter `image_name` is never sanitized. If an attacker inputs directory traversal sequences (such as `../../../../etc/cron.d` or `/etc`), the path resolves outside of the secure deployment folder boundaries.
  
  During `delete_image` (line 383), this path traversal can be abused to trigger arbitrary directory removal of host filesystems via `async_fs::remove_dir_all(&image_path)` or arbitrary BTRFS subvolume deletion if executing as `root`.
* **Remediation**:
  Sanitize the input to ensure that `image_name` does not contain directory path separators (`/`, `\`), or explicitly canonicalize the resolved path and verify that it starts with the canonical path of `self.images_dir`:
  ```rust
  let target_path = self.images_dir.join(image_name).canonicalize()?;
  if !target_path.starts_with(&self.images_dir.canonicalize()?) {
      anyhow::bail!("Path traversal attempt detected!");
  }
  ```

---

### [MEDIUM] System Privilege Hijacking via Unqualified Command Resolution
* **Citation**: `crates/op-deployment/src/image_manager.rs:70`, `crates/op-deployment/src/image_manager.rs:289`, and `crates/op-deployment/src/image_manager.rs:411`
* **Vulnerability Analysis**: 
  The application uses `tokio::process::Command` to invoke the `findmnt` and `btrfs` system utilities using unqualified relative binary names:
  ```rust
  let output = Command::new("findmnt") // Line 70
  let output = Command::new("btrfs")   // Line 289
  ```
  This forces the operating system to search for these binaries using the ambient system `PATH` environment variable. If the host environment has a modified or compromised `PATH`, an attacker could drop a malicious binary named `btrfs` or `findmnt` into a prioritized directory, achieving local privilege escalation when the orchestrator is run with elevated privileges (which BTRFS actions require).
* **Remediation**:
  Replace unqualified binary references with fully qualified absolute path targets:
  ```rust
  let output = Command::new("/bin/findmnt")...
  let output = Command::new("/sbin/btrfs")...
  ```

---

### [LOW] Out-Of-Memory (OOM) Denial of Service via Unbounded File Reads
* **Citation**: `crates/op-deployment/src/image_manager.rs:272`
* **Vulnerability Analysis**: 
  The `calculate_file_hash` helper reads the entire file contents into system memory at once to calculate the SHA256 checksum:
  ```rust
  let contents = async_fs::read(file_path).await?;
  ```
  Since deployment images and stages typically contain very large binaries, compressed tarballs, or disk images, loading several gigabytes of data into RAM at once will starve host memory resources, prompting the Linux kernel OOM-killer to terminate the deployment process.
* **Remediation**:
  Implement block-by-block file streaming into the SHA256 hasher:
  ```rust
  use tokio::io::AsyncReadExt;
  let mut file = async_fs::File::open(file_path).await?;
  let mut hasher = Sha256::new();
  let mut buffer = [0u8; 8192];
  while let Ok(n) = file.read(&mut buffer).await {
      if n == 0 { break; }
      hasher.update(&buffer[..n]);
  }
  ```

---

## Section 2: Schema-as-Code Compliance

* **Citation**: `crates/op-deployment/src/image_manager.rs:15-25` (`ImageMetadata`) and `crates/op-deployment/src/image_manager.rs:27-34` (`FileEntry`)
* **Non-Compliance Analysis**:
  This codebase implements a schema-as-code discipline using versioned Protocol Buffers and compliance standards (e.g. OSCAL component definitions as visible in workspace dependencies like `op-compliance`). 
  
  However, the metadata structures `ImageMetadata` and `FileEntry` represent data schemas that are expressed as ad-hoc, unversioned Rust structs serialized directly to custom JSON files (`.image-metadata.json`). These custom structures lack schema definitions, API contracts, or semantic versioning mechanisms, making the system prone to breaking on future structural revisions and making it impossible to perform automated compliance audits via OSCAL.
* **Remediation**:
  Define deployment image metadata inside versioned Protocol Buffer schemas (e.g., `image_metadata.proto` in `crates/op-compliance`), generating the Rust structs via `prost`, or structure the snapshot manifest to compile to standard OSCAL Component Definitions to align with the rest of the workspace's schema-as-code model.

---

## Section 3: Public API Surface

### Public Surface Summary
* **Modules**: 2
* **Structs**: 3
* **Public Struct Fields**: 12
* **Public Methods**: 7
* **Re-exports / Preludes**: 2
* **Total Public Items**: 26

### Top 10 Most Impactful Public Items
| Item | Type | Citation | Impact |
| :--- | :--- | :--- | :--- |
| `ImageManager` | `struct` | `crates/op-deployment/src/image_manager.rs:37` | Core driver managing deployment images, BTRFS subvolumes, and staging. |
| `ImageManager::new` | `fn` | `crates/op-deployment/src/image_manager.rs:45` | Factory method to instantiate the deployment manager engine. |
| `ImageManager::init` | `fn` | `crates/op-deployment/src/image_manager.rs:55` | Validates environment capabilities (BTRFS filesystem validation). |
| `ImageManager::create_image` | `fn` | `crates/op-deployment/src/image_manager.rs:89` | Builds fresh snapshot directories, resolves symlinks, and captures checksums. |
| `ImageManager::list_images` | `fn` | `crates/op-deployment/src/image_manager.rs:307` | Retrieves and parses all active image metadata entries from the database path. |
| `ImageManager::get_image` | `fn` | `crates/op-deployment/src/image_manager.rs:335` | Retrieves individual deployment image metadata. |
| `ImageManager::get_streamable_snapshot` | `fn` | `crates/op-deployment/src/image_manager.rs:346` | Locates the most recent BTRFS snapshot stream target for remote nodes. |
| `ImageManager::delete_image` | `fn` | `crates/op-deployment/src/image_manager.rs:383` | Performs recursive host deletion of deployment directories and BTRFS subvolumes. |
| `ImageMetadata` | `struct` | `crates/op-deployment/src/image_manager.rs:15` | Public data transfer schema representing snapshot state metrics. |
| `FileEntry` | `struct` | `crates/op-deployment/src/image_manager.rs:27` | Public schema representing individual file mapping deduplication entries. |

### Struct Fields Exposure Violation
Both `ImageMetadata` and `FileEntry` expose all of their internal fields as `pub` (e.g., `pub total_size: u64`, `pub path: PathBuf`). This violates encapsulation principles. External consumers can modify these values arbitrarily, introducing corrupted metadata states into the persistent metadata files.
* **Remediation**: Remove `pub` from the fields, and provide read-only public getter methods to expose these values safely.

### Glob Re-exports Check
No glob re-exports (`pub use *`) are present in `crates/op-deployment/src/lib.rs`. Re-exports are explicitly defined, preventing namespace pollution.

---

## Section 4: Dead Code Analysis

### Suppression Check
No `#[allow(dead_code)]` attributes were found within the provided files.

### Unused Dependency Analysis
Several dependencies are defined in `crates/op-deployment/Cargo.toml` but are never imported or used inside the source tree:
* `thiserror`: Unused throughout `lib.rs` and `image_manager.rs`.
* `tracing`: Unused (the codebase relies strictly on `log`).
* `reqwest`: Unused.
* `uuid`: Unused.
* `tar`: Unused.
* `flate2`: Unused.

### Dead Code Table
| Item | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `thiserror` | Crate Dependency | `crates/op-deployment/Cargo.toml` | Remove dependency from workspace configuration to optimize compilation times. |
| `tracing` | Crate Dependency | `crates/op-deployment/Cargo.toml` | Remove dependency; clean logging relies on the `log` crate. |
| `reqwest` | Crate Dependency | `crates/op-deployment/Cargo.toml` | Remove dependency to reduce compiled binary size. |
| `uuid` | Crate Dependency | `crates/op-deployment/Cargo.toml` | Remove dependency as identifier generation is handled externally. |
| `tar` | Crate Dependency | `crates/op-deployment/Cargo.toml` | Remove dependency from Cargo manifest; archiving is not yet implemented. |
| `flate2` | Crate Dependency | `crates/op-deployment/Cargo.toml` | Remove dependency from Cargo manifest; compression is not yet implemented. |
| `prelude` | Module | `crates/op-deployment/src/lib.rs:12` | Expose to integration tests or remove if external modules do not import it. |