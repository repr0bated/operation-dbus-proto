# Production Security and Quality Audit: op-deployment

## 1. Schema-as-Code Audit

The following table identifies ad-hoc data contracts that violate the schema-as-code discipline. All persisted state, IPC interfaces, and metadata schemas must be declared in versioned Protocol Buffer schemas or OSCAL component definitions rather than hand-rolled Rust structs with custom JSON serialization.

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `ImageMetadata` | Rust Struct | `crates/op-deployment/src/image_manager.rs:19` | No | Persisted on disk as `.image-metadata.json` and parsed dynamically using unsafe `simd_json::from_str`. Lacks a versioned Protocol Buffer schema definition, bypassing evolution safety and type validation. |
| `FileEntry` | Rust Struct | `crates/op-deployment/src/image_manager.rs:31` | No | Nested structural contract inside `ImageMetadata` representing physical deployment files. No formal schema limits evolutionary compatibility, structural verification, or cross-language portability. |
| `.image-metadata.json` | JSON File | `crates/op-deployment/src/image_manager.rs:189-191` | No | Ad-hoc JSON serialization output written directly to disk. This bypasses structured schema validators (e.g., JSON Schema or Protobuf schemas) and increases the risk of corruption or deserialization issues. |

---

## 2. OSCAL Coverage Audit

The system implements high-impact operations (e.g., executing system commands, creating snapshots, and deleting local directories) but lacks traceability to system security requirements or machine-readable authorization policies.

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **Audit Logging (NIST 800-53 AU-12 / AU-2)** | `crates/op-deployment/src/image_manager.rs:342-383` | None | Destructive snapshot and directory deletion actions are performed without writing structural audit records mapped to OSCAL control expectations. Standard system output (`log::info!`) is volatile and does not comply with secure audit trail standards. |
| **Access Control & Command Execution (NIST 800-53 AC-3 / AC-6)** | `crates/op-deployment/src/image_manager.rs:76`, `crates/op-deployment/src/image_manager.rs:265`, `crates/op-deployment/src/image_manager.rs:352` | None | Spawning elevated shell sub-processes (`findmnt`, `btrfs`) lacks an documented, machine-readable OSCAL component policy enforcing authorization constraints on actors who initiate image operations. |
| **System and Information Integrity (NIST 800-53 SI-7)** | `crates/op-deployment/src/image_manager.rs:175-182` | None | The implementation calculates file-level hashes for deduplication but does not cryptographically sign or verify deployment metadata. This lacks mapping to NIST 800-53 SI-7 (Software, Firmware, and Information Integrity) or OSCAL policy controls. |

---

## 3. Recommendations & Vulnerability Remediation

### [CRITICAL] Arbitrary Directory Deletion via Path Traversal
* **Location**: `crates/op-deployment/src/image_manager.rs:342-343`
* **Vulnerability Description**: The `delete_image` function takes an unvalidated `image_name: &str` parameter and joins it directly to the system's `images_dir` path:
  ```rust
  let image_path = self.images_dir.join(image_name);
  ```
  If `image_name` contains path traversal sequences (such as `../../var/lib`), `image_path` resolves to a directory outside of the designated deployment scope. Since the manager subsequently runs either `btrfs subvolume delete` or `async_fs::remove_dir_all(&image_path)`, an attacker who can influence the `image_name` can delete arbitrary directories on the host system.
* **Remediation**:
  1. Restrict `image_name` to alphanumeric characters and safe punctuation (e.g., dashes, underscores). Reject any strings containing directory separators (`/`, `\`) or traversal elements (`..`).
  2. Implement canonicalization and strict path boundary checks:
     ```rust
     let target_path = self.images_dir.join(image_name);
     let canonical_images_dir = tokio::fs::canonicalize(&self.images_dir).await?;
     let canonical_target = tokio::fs::canonicalize(&target_path).await?;
     if !canonical_target.starts_with(&canonical_images_dir) {
         anyhow::bail!("Access Denied: Path is outside boundary");
     }
     ```

### [HIGH] Arbitrary Host File Exposure via Unsanitized Symlink Resolution
* **Location**: `crates/op-deployment/src/image_manager.rs:234-238` and `crates/op-deployment/src/image_manager.rs:147`
* **Vulnerability Description**: When scanning previous images, `find_file_in_previous_images` resolves absolute and relative symlinks located inside previously deployed images:
  ```rust
  let target = async_fs::read_link(&file_path).await?;
  let resolved = if target.is_absolute() {
      target
  } else {
      file_path.parent().unwrap().join(&target)
  };
  ```
  If a deployed image contains a malicious symlink pointing to a sensitive host file (e.g., `/etc/passwd` or `/root/.ssh/id_rsa`), the manager resolves it to that exact host path and returns it. During `create_image` processing, the manager subsequently creates a new symlink pointing relative to that resolved location:
  ```rust
  std::os::unix::fs::symlink(&relative_target, &dest_path)
  ```
  This duplicates the reference inside the new image folder, resulting in host information exposure during subsequent streaming or execution steps.
* **Remediation**:
  Ensure that all resolved symlink targets reside within the configured storage root boundary before passing them back as candidate files. Any link target resolving to a path outside the image manager base path must be filtered out.

### [HIGH] Violation of Schema-as-Code Discipline
* **Location**: `crates/op-deployment/src/image_manager.rs:19-37` and `crates/op-deployment/src/image_manager.rs:189-191`
* **Vulnerability Description**: The deployment metadata contract is written as an ad-hoc, hand-rolled JSON structure. Storing critical deployment metadata directly to disk as unversioned JSON limits system security auditing and interoperability across components. Furthermore, parsing this file using unsafe `simd_json::from_str` can result in undefined behavior if the schema drifts or is manipulated by malicious local users.
* **Remediation**:
  Define a versioned Protocol Buffer schema (e.g., `deployment_metadata.proto`) that represents the state of the deployment images. Use `prost` or `tonic-build` to generate compile-time verified serialization structures, and store the output as standardized Protobuf payloads on disk.

### [MEDIUM] Synchronous Blocking I/O inside Async Executor
* **Location**: `crates/op-deployment/src/image_manager.rs:147`
* **Vulnerability Description**: The implementation uses `std::os::unix::fs::symlink` to create symlinks:
  ```rust
  std::os::unix::fs::symlink(&relative_target, &dest_path)
  ```
  Calling synchronous, blocking filesystem operations inside an asynchronous context blocks the Tokio worker thread, causing execution delays and potential thread starvation.
* **Remediation**:
  Use the asynchronous `tokio::fs::symlink` (aliased as `async_fs::symlink` in this module) to execute the operation asynchronously:
  ```rust
  async_fs::symlink(&relative_target, &dest_path).await?;
  ```