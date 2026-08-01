# Production Security and Quality Audit: `op-deployment`

## 1. Dependencies & Feature Inventory

A complete analysis of the direct and workspace dependencies of the `op-deployment` crate has been performed based on `crates/op-deployment/Cargo.toml` and the workspace `Cargo.toml`.

### Direct Dependencies of `op-deployment`

| Dependency | Version | Features Enabled (Explicit vs Default) | Security/Quality Flags & Notes |
| :--- | :--- | :--- | :--- |
| `tokio` | Workspace (`1`) | `full` (via workspace default) | Includes multi-threaded runtime, process, signal, and fs APIs. |
| `serde` | Workspace (`1`) | `derive` (via workspace default) | Safe serialization framework. |
| `simd-json` | Workspace (`0.13`) | `serde`, `serde_impl` (via workspace default) | High-performance JSON parser. **Uses native unsafe optimizations.** |
| `anyhow` | Workspace (`1`) | Default features | Used for unstructured error handling. |
| `thiserror` | Workspace (`1`) | Default features | Structured error derivation. |
| `tracing` | Workspace (`0.1`) | Default features | Structured application logging. |
| `reqwest` | Workspace (`0.11`) | `json`, `stream` (via workspace default) | HTTP client. Note: Dual-version hazard in workspace (some workspace crates use `0.12`). |
| `sha2` | Workspace (`0.10`) | Default features | Used for SHA256 checksumming. |
| `chrono` | Workspace (`0.4`) | `serde` (via workspace default) | Time management with serialization. |
| `log` | Workspace (`0.4`) | Default features | Standard logging facade. |
| `uuid` | Workspace (`1.6`) | `v4`, `serde` (via workspace default) | Unique identifiers. |
| `tar` | Workspace (`0.4`) | Default features | Tar archive creation and extraction. |
| `flate2` | Workspace (`1`) | Default features | Gzip compression. |
| `tempfile` *(dev)* | Workspace (`3`) | Default features | Temporary directory utilities for testing. |

### Feature Gates
The `op-deployment` crate does not define any custom feature flags in its `[features]` section (none are declared in `crates/op-deployment/Cargo.toml`).

---

## 2. Storage Backend Check

A comprehensive scan was conducted across the source code of `op-deployment` to locate storage engines (such as SQLite, CozoDB, Sled, Redis, etc.). 

While the parent workspace defines several database engines in the workspace `Cargo.toml` (such as `sqlx`, `rusqlite`, `cozo`, and `redis`), **none** of these database drivers are directly used or imported within the audited source files of `op-deployment`. Instead, state is persisted strictly as files and BTRFS subvolumes on the filesystem.

### Workspace Storage Engines

| Backend | Found at | Role | Audited Crate Presence |
| :--- | :--- | :--- | :--- |
| `sqlx` (SQLite) | `Cargo.toml` | Relational Storage | **Absent** in `op-deployment` source |
| `rusqlite` | `Cargo.toml` | Embedded Relational | **Absent** in `op-deployment` source |
| `redis` | `Cargo.toml` | Cache / KV Store | **Absent** in `op-deployment` source |
| `cozo` / `sled` | `Cargo.toml` | Datalog Relational Graph | **Absent** in `op-deployment` source |

---

## 3. Schema-As-Code Violations

The workspace `Cargo.toml` includes schema-defining utilities such as `prost` and `tonic` (Protocol Buffers) and `jsonschema` (for validating JSON contracts). However, the `op-deployment` crate violates the schema-as-code discipline by expressing critical system metadata as ad-hoc Rust structures and persisting them to unversioned raw JSON files.

### Specific Violations

*   **Ad-hoc Struct definitions without Schema Enforcements**
    *   **Citation**: `crates/op-deployment/src/image_manager.rs:16-25`
    *   **Citation**: `crates/op-deployment/src/image_manager.rs:27-34`
    *   **Vulnerability**: The metadata format of a deployed container/VM snapshot is declared as an ad-hoc Rust struct (`ImageMetadata` and `FileEntry`) that derives `Serialize` and `Deserialize`. No versioning, Protocol Buffer, or OpenAPI schema is generated.
    *   **Risk**: Changes to the fields of `ImageMetadata` or `FileEntry` will lead to silent deserialization failures when reading older metadata files from disk (`.image-metadata.json`). This violates the workspace's schema-as-code discipline and risks breaking backward compatibility during control plane upgrades.

---

## 4. Audited Findings (Security & Quality)

### Finding 1: Arbitrary Host File Leakage / Read via Unrestricted Symlink Following [CRITICAL]
*   **File**: `crates/op-deployment/src/image_manager.rs`
*   **Lines**: 204-239 (`find_file_in_previous_images`), 120-141 (`create_image`)
*   **Exploitability**: Directly Exploitable. If a malicious or manipulated deployment image contains a symlink pointing to a sensitive host file (e.g. `/etc/shadow`), the image manager will follow it and duplicate/copy the host file.

#### Technical Analysis
In `find_file_in_previous_images` (lines 204-239), the code follows symlinks in existing images to find the original files for deduplication:
```rust
let symlink_meta = async_fs::symlink_metadata(&file_path).await?;
if symlink_meta.is_symlink() {
    // Follow the symlink to find the original file
    let target = async_fs::read_link(&file_path).await?;
    let resolved = if target.is_absolute() {
        target
    } else {
        file_path.parent().unwrap().join(&target)
    };

    // Check if the resolved path exists and is a real file
    if async_fs::metadata(&resolved).await.is_ok() {
        let resolved_meta = async_fs::symlink_metadata(&resolved).await?;
        if !resolved_meta.is_symlink() {
            return Ok(Some(resolved));
        }
    }
}
```
No validation is performed to ensure that the `resolved` path resides within `self.images_dir`. If `target` is `/etc/shadow` (or any relative path traversing out of the images directory), the code returns `Some(PathBuf::from("/etc/shadow"))` as the `previous_file` match.

In `create_image` (lines 120-141), this path is handled as follows:
*   On **Unix** platforms, a new relative symlink is calculated using `calculate_relative_path` and created inside the new deployment directory pointing to the target host path. If the resulting deployment image is streamed or exposed to untrusted environments, the symlink exposes the host file.
*   On **non-Unix** platforms, the system directly copies the host file into the destination folder:
```rust
#[cfg(not(unix)]
{
    // On non-Unix, just copy the file
    async_fs::copy(&previous_file, &dest_path)
        .await
        .context(format!("Failed to copy file: {}", dest_path.display()))?;
}
```
This copies the sensitive host file (`/etc/shadow`) into the user-accessible deployment image, bypassing all sandbox boundaries.

#### Remediation
Before returning the resolved target, verify that its canonicalized path resides within `self.images_dir`. Reject any path resolving outside of the sandboxed base directory.
```rust
let canonical_resolved = tokio::fs::canonicalize(&resolved).await?;
if !canonical_resolved.starts_with(tokio::fs::canonicalize(&self.images_dir).await?) {
    anyhow::bail!("Security violation: Symlink target attempts to escape images sandbox.");
}
```

---

### Finding 2: Destructive Directory Traversal and Arbitrary Base Directory Deletion [CRITICAL]
*   **File**: `crates/op-deployment/src/image_manager.rs`
*   **Lines**: 351-395 (`delete_image`)
*   **Exploitability**: Directly Exploitable. If `delete_image` is invoked with `image_name = ".."`, it will recursively delete the entire deployment structure.

#### Technical Analysis
In `delete_image` (line 351), the `image_path` is calculated by directly joining the untrusted `image_name` string:
```rust
let image_path = self.images_dir.join(image_name);
```
If `image_name` is `..`, the path resolves to `self.images_dir.join("..")` which is `self.base_path` (the root directory for all images, snapshots, and metadata). 

If the base path is not on a BTRFS filesystem, the code executes:
```rust
} else {
    async_fs::remove_dir_all(&image_path).await?;
}
```
This deletes the *entire* base directory containing all system data. 

Additionally, on lines 358-372, snapshots are matched against `image_name` using `starts_with`:
```rust
if name.starts_with(image_name) {
```
If `image_name` is empty (`""`), it will match and recursively delete *every* snapshot in the snapshots folder.

#### Remediation
Sanitize the `image_name` input to ensure it contains no directory traversal elements (`..`), is not empty, and stays strictly within the intended base path:
```rust
if image_name.trim().is_empty() || image_name.contains('/') || image_name.contains('\\') || image_name == ".." {
    anyhow::bail!("Invalid image name supplied.");
}
```

---

### Finding 3: Directory Traversal in Image Creation [HIGH]
*   **File**: `crates/op-deployment/src/image_manager.rs`
*   **Lines**: 97-101 (`create_image`)
*   **Exploitability**: High. Path traversal strings passed as the `image_name` during image creation can write files outside the expected container directory.

#### Technical Analysis
In `create_image` (line 97), the path for the new image is constructed as:
```rust
let image_path = self.images_dir.join(image_name);
async_fs::create_dir_all(&image_path).await?;
```
If `image_name` contains path traversal sequences like `../../some_dir`, directory creation is executed outside of `images_dir`. Metadata files and copy targets will subsequently be written to arbitrary locations on the host system.

#### Remediation
Enforce validation checks on the `image_name` input inside `create_image` as well as inside `get_image` and `delete_image`.

---

### Finding 4: Denial of Service (OOM) via Unbuffered Whole File Reads [MEDIUM]
*   **File**: `crates/op-deployment/src/image_manager.rs`
*   **Lines**: 251-258 (`calculate_file_hash`)
*   **Exploitability**: High (if deploying large container/VM images).

#### Technical Analysis
To calculate a file's SHA256 checksum, the code reads the entire file into memory:
```rust
async fn calculate_file_hash(&self, file_path: &Path) -> Result<String> {
    let contents = async_fs::read(file_path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&contents);
    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}
```
If a deployment image contains very large files (e.g. multi-gigabyte virtual disk images, system archives, or raw ISO files), reading the entire file at once will exhaust host memory, triggering the Linux Out-Of-Memory (OOM) killer and crashing the control plane.

#### Remediation
Compute the hash incrementally using a buffered reader and stream chunks into the hasher:
```rust
use tokio::io::AsyncReadExt;

async fn calculate_file_hash(&self, file_path: &Path) -> Result<String> {
    let mut file = async_fs::File::open(file_path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer).await?;
        if n == 0 { break; }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
```

---

### Finding 5: Inflexible Initialization Failure on Missing `findmnt` Binary [LOW]
*   **File**: `crates/op-deployment/src/image_manager.rs`
*   **Lines**: 57-61 (`init`), 68-83 (`is_btrfs`)
*   **Exploitability**: None (Operational / Quality issue).

#### Technical Analysis
During `init` execution, the manager attempts to verify the filesystem type using `findmnt`:
```rust
let output = Command::new("findmnt")
    .args(["-n", "-o", "FSTYPE", "-T"])
    .arg(path)
    .output()
    .await
    .context("Failed to check filesystem type")?;
```
In minimal environments (such as Docker containers or lightweight Alpine/scratch system environments), the `findmnt` binary is often absent from the system's `PATH`. If execution of the `findmnt` command fails, `.context(...)` returns an `Err` which propagates through the `?` operator on line 57:
```rust
if self.is_btrfs(&self.base_path).await? {
```
This causes the entire initialization sequence to fail, preventing the deployment system from starting even when running on standard filesystems where snapshots are disabled anyway.

#### Remediation
Handle the command execution error gracefully inside `is_btrfs`. If the command fails to launch due to a missing binary, log a warning and return `Ok(false)` instead of failing:
```rust
match Command::new("findmnt").args(["-n", "-o", "FSTYPE", "-T"]).arg(path).output().await {
    Ok(output) if output.status.success() => {
        let fstype = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(fstype == "btrfs")
    }
    _ => {
        log::warn!("'findmnt' tool not available or returned error; disabling snapshots");
        Ok(false)
    }
}
```

---

### Finding 6: Reliability Issue in Snapshot Streaming on Platforms Lacking Birth Time [LOW]
*   **File**: `crates/op-deployment/src/image_manager.rs`
*   **Lines**: 334-345 (`get_streamable_snapshot`)
*   **Exploitability**: None (Operational / Platform Compatibility issue).

#### Technical Analysis
The `get_streamable_snapshot` method identifies the latest snapshot using filesystem metadata creation times:
```rust
if let Ok(metadata) = entry.metadata().await {
    if let Ok(created) = metadata.created() {
        let timestamp = created
            .duration_since(std::time::UNIX_EPOCH)
            ...
```
However, creation time (birth time) is not supported or exposed by all filesystems (e.g. ext4 mounted with specific options, or older Linux kernels). In these systems, `metadata.created()` returns an `Err`, causing the loop to skip the snapshot. As a result, the code will fail to locate streamable snapshots even if they exist.

#### Remediation
Since snapshots are explicitly named using a lexicographically sortable timestamp format (`format!("{}-{}", image_name, timestamp)` on line 267), parse the timestamp directly from the directory name instead of querying the filesystem metadata.

---

### Finding 7: Redundant `unsafe` Blocks and Undocumented Invariants [LOW]
*   **File**: `crates/op-deployment/src/image_manager.rs`
*   **Lines**: 301-303 (`list_images`), 319 (`get_image`)
*   **Exploitability**: None (Quality issue).

#### Technical Analysis
The codebase wraps standard `simd_json::from_str` calls in `unsafe` blocks:
```rust
if let Ok(metadata) =
    unsafe { simd_json::from_str::<ImageMetadata>(&mut content) }
```
Using `unsafe` blocks for operations that are safe or should be safe in the library API is a code smell. If an unsafe variant is being intentionally targeted, it completely lacks the required `// SAFETY:` comments explaining the safety invariants being maintained, which violates the core guidelines of safe Rust development.

#### Remediation
Remove unnecessary `unsafe` blocks if using safe parsers. If utilizing low-level optimized parsers requiring unsafe access, document the invariants precisely.