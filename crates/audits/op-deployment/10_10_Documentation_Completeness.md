# Production Security & Quality Audit

## 1. Executive Summary

This production-grade security and quality audit reviews the `op-deployment` crate, focusing on API documentation, memory safety, schema discipline, and path manipulation invariants. 

Several critical issues have been discovered, including an **Arbitrary File Copy / Information Disclosure** vulnerability via unsanitized symlink traversal, potential **Undefined Behavior / Out-of-Bounds Reads** from unpadded SIMD JSON parsing, and **Path Traversal** vectors due to unvalidated deployment image names.

---

## 2. Security & Vulnerability Findings

### Finding 1: CRITICAL - Host Information Disclosure via Malicious Symlink Traversal
*   **File & Line Citation**: `crates/op-deployment/src/image_manager.rs:231-255`
*   **Vulnerability Type**: Arbitrary File Read / Symlink Follow Bypass
*   **Impact**: Direct read and leakage of sensitive host-level files (e.g., `/etc/shadow`, `/etc/passwd`, private keys).
*   **Description**:
    In `find_file_in_previous_images`, the image manager searches older deployment images for existing files to implement deduplication. If a file is a symlink, it resolves the link destination:
    ```rust
    let target = async_fs::read_link(&file_path).await?;
    let resolved = if target.is_absolute() {
        target
    } else {
        file_path.parent().unwrap().join(&target)
    };
    ```
    This resolved path is returned as a valid file reference without verifying if it escapes the `images` or `snapshots` base directory bounds. 
    
    During `create_image` on lines 101–129:
    *   On non-Unix platforms (line 120), the manager calls `async_fs::copy(&previous_file, &dest_path)`. This copies the host file targeted by the malicious symlink *directly* into the new, public deployment image.
    *   On Unix platforms, `calculate_relative_path` (line 109) computes a relative path to the target (e.g., `../../../../etc/shadow`) and creates a symlink. When this image is packaged or streamed to clients, the target files on the host filesystem are read or exposed.
*   **Remediation**:
    Enforce path sanitization. Before resolving a target file, canonicalize the path and assert that it remains strictly within the authorized sandbox directory (`self.images_dir`). Do not follow absolute links or relative links that contain parent directories escaping the sandbox.

---

### Finding 2: HIGH - Process DoS / Heap Out-of-Bounds Read via Unpadded SIMD JSON Parsing
*   **File & Line Citation**: `crates/op-deployment/src/image_manager.rs:315`, `crates/op-deployment/src/image_manager.rs:338`
*   **Vulnerability Type**: Undefined Behavior / Out-of-Bounds Read
*   **Impact**: Memory corruption, process crashes (Segmentation Faults), or arbitrary heap memory disclosure.
*   **Description**:
    The code reads metadata files directly into a standard `String` and parses them using the `unsafe` API of `simd-json`:
    ```rust
    let mut content = async_fs::read_to_string(&metadata_path).await?;
    if let Ok(metadata) = unsafe { simd_json::from_str::<ImageMetadata>(&mut content) }
    ```
    `simd_json::from_str` requires its input buffer to have `simd_json::SIMDJSON_PADDING` bytes of trailing allocation. Standard `String`s allocated via `async_fs::read_to_string` do not guarantee this padding. When the SIMD processing instructions execute vector loads, they can read past the allocated capacity of the string, leading to undefined behavior or segmentation faults if the allocation is located near a memory page boundary.
*   **Remediation**:
    Either:
    1. Read the file into a padded vector using `simd_json::to_padded_bin` and then parse via `simd_json::from_slice`.
    2. Fall back to `serde_json::from_str` which does not require raw vector padding and is fully safe.

---

### Finding 3: HIGH - Path Traversal via Unvalidated `image_name`
*   **File & Line Citation**: `crates/op-deployment/src/image_manager.rs:92`, `crates/op-deployment/src/image_manager.rs:334`, `crates/op-deployment/src/image_manager.rs:381`
*   **Vulnerability Type**: Path Traversal
*   **Impact**: Unauthorized creation, deletion, or reading of directories outside the designated `images` directory on the host system.
*   **Description**:
    The system directly joins raw, unvalidated `image_name` parameters with internal directories:
    ```rust
    let image_path = self.images_dir.join(image_name);
    ```
    If an attacker controls or influences the `image_name` value (e.g., providing `../../etc` or similar payload through control-plane endpoints), the manager can traverse outside the base path, allowing arbitrary recursive deletions inside `delete_image` (line 415), or creation of directories inside `create_image` (line 93).
*   **Remediation**:
    Reject any `image_name` that contains path separators (`/`, `\`), null bytes, or parent directory markers (`..`).

---

## 3. Schema-as-Code & Quality Analysis

### Finding 4: LOW - Ad-Hoc Data Contracts and Ad-Hoc JSON Serialization
*   **File & Line Citation**: `crates/op-deployment/src/image_manager.rs:15`, `crates/op-deployment/src/image_manager.rs:27`
*   **Violation Type**: Ad-hoc Struct Data Contracts
*   **Description**: 
    The metadata of deployment images (`ImageMetadata`) and file entries (`FileEntry`) are defined as ad-hoc Rust structs and stored directly as JSON files (`.image-metadata.json`). This violates the schema-as-code discipline, as these structures are not backed by versioned, language-agnostic schemas (such as Protocol Buffers or JSON Schema/OSCAL structures). This risks backward-compatibility breakages when structural changes are introduced.
*   **Remediation**:
    Define data contracts using Protocol Buffers or versioned OpenAPI/JSON Schema models. Generate the corresponding Rust deserialization structs automatically as part of the workspace build pipeline.

---

## 4. Documentation & Compliance Audit

### Crate-Level & Module-Level Docs
*   **Status**: **Compliant**
*   **Details**: 
    *   Crate-level documentation is present in `crates/op-deployment/src/lib.rs` (lines 1–5).
    *   Module-level documentation is present in `crates/op-deployment/src/image_manager.rs` (lines 1–7).

---

### README.md Presence
*   **Status**: **Non-Compliant**
*   **Details**: 
    There is no `README.md` file present in the `crates/op-deployment` crate directory. 
*   **Remediation**: 
    Provide a `README.md` explaining the purpose, architecture, setup instructions, and physical filesystem requirements (such as BTRFS prerequisites) for the deployment image manager.

---

### Sample of 10 Public Items: Rustdoc Quality Check
Below is a sampling of 10 public symbols within the crate to check for standard `///` rustdoc comments:

| # | Public Item | File & Line Citation | Rustdoc Status |
|---|-------------|----------------------|----------------|
| 1 | `ImageMetadata` | `crates/op-deployment/src/image_manager.rs:15` | **Compliant** (Has `/// Deployment image...`) |
| 2 | `ImageMetadata::name` | `crates/op-deployment/src/image_manager.rs:16` | **Non-Compliant** (Missing field-level doc) |
| 3 | `FileEntry` | `crates/op-deployment/src/image_manager.rs:27` | **Compliant** (Has `/// File entry...`) |
| 4 | `FileEntry::is_symlink` | `crates/op-deployment/src/image_manager.rs:29` | **Non-Compliant** (Missing field-level doc) |
| 5 | `ImageManager` | `crates/op-deployment/src/image_manager.rs:36` | **Compliant** (Has `/// Image manager...`) |
| 6 | `ImageManager::new` | `crates/op-deployment/src/image_manager.rs:43` | **Compliant** (Has `/// Create new...`) |
| 7 | `ImageManager::init` | `crates/op-deployment/src/image_manager.rs:53` | **Compliant** (Has `/// Initialize the...`) |
| 8 | `ImageManager::create_image` | `crates/op-deployment/src/image_manager.rs:82` | **Compliant** (Has `/// Create a new...`) |
| 9 | `ImageManager::list_images` | `crates/op-deployment/src/image_manager.rs:305` | **Compliant** (Has `/// List all...`) |
| 10| `prelude` module | `crates/op-deployment/src/lib.rs:12` | **Compliant** (Has `/// Prelude for...`) |

*   **Audit Note on Re-exports**: `pub use image_manager::ImageManager` on `crates/op-deployment/src/lib.rs:9` lacks a `#[doc(inline)]` attribute or documenting comment.
*   **Remediation**: Add descriptive, triple-slash (`///`) doc comments to all public fields, modules, and inline re-exports to satisfy full API documentation criteria.

---

### Unsafe Functions & Invariant Documentation
*   **Status**: **Compliant (No Public Unsafe Functions)**
*   **Details**: 
    There are **no public unsafe functions** declared in this crate. 
*   **Audit Note on Private Unsafe Blocks**: 
    While there are no public unsafe functions, the crate makes internal use of `unsafe` blocks to parse JSON files using `simd_json` (lines 315, 338). These blocks are **non-compliant with quality standards** as they completely lack `// SAFETY:` comments explaining why the operations are sound and what invariants are guaranteed by the caller.
*   **Remediation**:
    Always document unsafe blocks with a `// SAFETY:` comment detail explaining how memory layout, pointer offsets, or padding guarantees are satisfied.