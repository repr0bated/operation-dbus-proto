# Integration Audit & Production Security Report: `op-deployment`

## 1. Workspace Integration Analysis

### Crates Depending on `op-deployment`
* **Audited File:** `Cargo.toml` (Workspace Root), `Cargo.lock`
* **Findings:** 
  An analysis of the workspace-level `Cargo.toml` and the compiled dependency graph in `Cargo.lock` reveals that **no other crates in the workspace currently depend on `op-deployment`**. While `crates/op-deployment` is defined as a workspace member, it is currently an unintegrated, orphaned library module with no inbound workspace dependencies.

### Registered D-Bus Service Names and Object Paths
* **Audited Files:** `crates/op-deployment/src/lib.rs`, `crates/op-deployment/src/image_manager.rs`
* **Findings:** 
  There are **no D-Bus service names or object paths registered** within the `op-deployment` crate. It operates purely as a local filesystem utility and does not contain any IPC interface bindings.

### Exposed HTTP/gRPC Endpoints
* **Audited Files:** `crates/op-deployment/src/lib.rs`, `crates/op-deployment/src/image_manager.rs`
* **Findings:** 
  There are **no HTTP or gRPC endpoints exposed** by this crate. The `ImageManager` structure performs only localized disk and subvolume management operations.

### Cross-Crate Circular Dependency Risks
* **Audited Files:** `crates/op-deployment/Cargo.toml`, `Cargo.toml`
* **Findings:** 
  There is **zero circular dependency risk** at present. `op-deployment` does not depend on any internal workspace crates (such as `op-core` or `op-state`) and is not depended upon by others. 

---

## 2. Schema-As-Code Violations

* **Ad-hoc Data Contracts:** `crates/op-deployment/src/image_manager.rs:15` (`ImageMetadata`), `crates/op-deployment/src/image_manager.rs:28` (`FileEntry`)
* **Ad-hoc Serialization/Deserialization:** `crates/op-deployment/src/image_manager.rs:170`, `crates/op-deployment/src/image_manager.rs:301`, `crates/op-deployment/src/image_manager.rs:319`

### Description
The codebase strictly mandates a *schema-as-code* discipline utilizing Protocol Buffers or OSCAL for versioned data contracts. However, the `ImageMetadata` and `FileEntry` data contracts are declared as ad-hoc, unversioned Rust structs serialized directly to/from unstructured JSON files (`.image-metadata.json`) via `simd_json`. 

### Remediation
Define the deployment image metadata model using a versioned Protocol Buffer schema (e.g., `image_metadata.v1.proto`). Generate the Rust structures via `prost` or `tonic` to guarantee strict schema evolution safety and compliance with system-wide serialization standards.

---

## 3. Production Security & Quality Findings

### [CRITICAL] Arbitrary Directory Deletion and Path Traversal via Unsanitized `image_name`
* **Vulnerability Type:** Path Traversal / Arbitrary Directory Deletion
* **Location:** 
  * `crates/op-deployment/src/image_manager.rs:315` (in `get_image`)
  * `crates/op-deployment/src/image_manager.rs:324` (in `get_streamable_snapshot`)
  * `crates/op-deployment/src/image_manager.rs:340` (in `delete_image`)

#### Analysis
In `delete_image`, the unsanitized string argument `image_name` is directly joined with `self.images_dir`:
```rust
let image_path = self.images_dir.join(image_name);
```
If an attacker passes an `image_name` containing directory traversal components (e.g., `../../../../etc` or `../../../../usr`), the path escapes the sandboxed directory. Under BTRFS filesystems, this triggers subvolume deletion on arbitrary directories; on standard filesystems, it falls back to:
```rust
async_fs::remove_dir_all(&path).await?;
```
Because BTRFS subvolume operations require execution with root privileges (`CAP_SYS_ADMIN`), this vulnerability allows an attacker to recursively delete any directory on the host filesystem, resulting in complete host denial of service (DoS).

#### Remediation
Sanitize the `image_name` argument before joining. Ensure it does not contain directory separators or parent directory references (`..`), and validate that the resolved path is strictly a child of `self.images_dir`:
```rust
let canonical_images_dir = self.images_dir.canonicalize()?;
let target_path = self.images_dir.join(image_name);
let canonical_target = target_path.canonicalize()?;

if !canonical_target.starts_with(&canonical_images_dir) {
    anyhow::bail!("Path traversal attempt detected!");
}
```

---

### [CRITICAL] Arbitrary Host File Disclosure via Symlink Resolution
* **Vulnerability Type:** Arbitrary File Read / Symlink Follow Leak
* **Location:** `crates/op-deployment/src/image_manager.rs:224-234` (in `find_file_in_previous_images`)

#### Analysis
The image deduplication logic searches previous images to find files to symlink. If a file in a previous image is a symlink, the system follows it to locate the source:
```rust
let target = async_fs::read_link(&file_path).await?;
let resolved = if target.is_absolute() {
    target
} else {
    file_path.parent().unwrap().join(&target)
};
```
There is no verification that `resolved` remains bounded inside the snapshot directory. If a malicious deployment image contains a symlink pointing to an absolute host path (such as `/etc/shadow` or `/root/.ssh/id_rsa`), `find_file_in_previous_images` resolves it and returns the absolute host path. 

In `create_image`, a relative symlink is then computed and created pointing to this host path:
```rust
let relative_target = self.calculate_relative_path(dest_path.parent().unwrap(), &previous_file)?;
std::os::unix::fs::symlink(&relative_target, &dest_path)...
```
When this newly generated deployment image is streamed or mounted by clients, the host's private system files are leaked to the deployment target.

#### Remediation
Do not resolve absolute symlinks, and validate that any resolved relative symlink remains strictly within the bounds of the specific parent image path:
```rust
if target.is_absolute() {
    anyhow::bail!("Absolute symlinks are forbidden");
}
let resolved = file_path.parent().unwrap().join(&target);
if !resolved.canonicalize()?.starts_with(&self.images_dir.canonicalize()?) {
    anyhow::bail!("Symlink escapes image sandbox");
}
```

---

### [WARNING] Unnecessary `unsafe` Block and Memory Safety Risk in `simd_json` Deserialization
* **Vulnerability Type:** Memory Unsafety Risk
* **Location:** 
  * `crates/op-deployment/src/image_manager.rs:301`
  * `crates/op-deployment/src/image_manager.rs:319`

#### Analysis
The code invokes `simd_json` using raw `unsafe` blocks:
```rust
if let Ok(metadata) = unsafe { simd_json::from_str::<ImageMetadata>(&mut content) }
```
`simd_json::from_str` is destructive (it mutates the input string slice in-place). The `unsafe` variant bypasses Rust's standard lifetime and alignment guarantees. Because `content` is an owned `String` loaded from disk, parsing it using `unsafe` is highly prone to undefined behavior or segmentation faults if the metadata files on disk are corrupted, concurrently modified, or contain invalid UTF-8.

#### Remediation
Replace the `unsafe` invocation with `simd_json`'s safe parsing APIs (such as `simd_json::serde::from_slice` or `simd_json::from_str` with proper lifetime controls), or fall back to standard `serde_json` for file deserialization where ultra-high-throughput SIMD parsing is not bottlenecking the system.