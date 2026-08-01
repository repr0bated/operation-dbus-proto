# Production Security and Quality Audit: op-deployment

## 1. Architecture & Module Map

### Overview
The `op-deployment` crate manages container and image deployments. It is a library crate designed to orchestrate image replication, deduplication, and streaming using a BTRFS filesystem backend with a fallback copy mechanism on non-BTRFS systems.

### Module Tree
```
op-deployment (lib.rs) [crates/op-deployment/src/lib.rs:1]
└── image_manager (image_manager.rs) [crates/op-deployment/src/image_manager.rs:1]
```

### Entry Points
- **Library Entry Point**: `crates/op-deployment/src/lib.rs:1` — Exports the `ImageManager` struct and a convenient prelude module.

### Notes
- There are no binary targets (e.g., `main.rs` or `bin/*`) defined inside `crates/op-deployment`.
- The architecture relies on external command invocations (`btrfs`, `findmnt`) to interface with filesystem features.

---

## 2. Security & Quality Findings

### [CRITICAL] Arbitrary Directory Deletion & Complete Data Destruction via Empty or Traversal `image_name`
- **File**: `crates/op-deployment/src/image_manager.rs`
- **Lines**: 377-434
- **Physically Exploitable**: Yes. If `delete_image` is invoked with an untrusted or empty string input, it can trigger complete loss of all deployment images and snapshots, or delete arbitrary host directories.

#### Technical Description
In `delete_image`, the directory path is constructed by joining `image_name` to `self.images_dir`:
```rust
let image_path = self.images_dir.join(image_name);
```
Under Rust's `PathBuf` implementation, joining an empty string (`""`) to a path returns the path unchanged (resolving to `self.images_dir` itself). 

Later, the snapshot deletion loop checks:
```rust
if name.starts_with(image_name)
```
Since any string slice `starts_with("")` evaluates to `true`, **every snapshot** in `self.snapshots_dir` is matched and deleted.

Finally, the code attempts to remove the image directory:
```rust
} else {
    async_fs::remove_dir_all(&image_path).await?;
}
```
If `image_name` is empty, this translates to `async_fs::remove_dir_all(&self.images_dir)`, recursively deleting all deployed images. Furthermore, if `image_name` contains path traversal sequences like `../../`, a malicious actor can point `image_path` to system critical paths, deleting them recursively if the process runs with elevated privileges (required anyway for `btrfs` commands).

#### Remediation
Validate `image_name` to ensure it is not empty, does not contain path separators (`/`), and does not contain directory traversal sequences (`..`).
```rust
let image_name = image_name.trim();
if image_name.is_empty() || image_name.contains('/') || image_name.contains("..") {
    anyhow::bail!("Invalid image name");
}
```

---

### [HIGH] Denial of Service (OOM Crash) via Unbounded Memory Allocation in File Hashing
- **File**: `crates/op-deployment/src/image_manager.rs`
- **Lines**: 263-269
- **Physically Exploitable**: Yes. If an image contains large files (e.g., VM images, ISOs, or large database dumps), this method will crash the control plane due to RAM exhaustion.

#### Technical Description
The file hash calculation reads the entire file directly into a heap-allocated buffer:
```rust
async fn calculate_file_hash(&self, file_path: &Path) -> Result<String> {
    let contents = async_fs::read(file_path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&contents);
    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}
```
If a file size exceeds available memory, `async_fs::read` fails or triggers the OS Out-Of-Memory (OOM) killer to terminate the service.

#### Remediation
Stream the file contents in chunks through a buffered reader instead of pulling the entire file into memory:
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
    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}
```

---

### [MEDIUM] Denial of Service (Panic) via Unwrapped `duration_since` on System Clock Skew
- **File**: `crates/op-deployment/src/image_manager.rs`
- **Lines**: 363-366
- **Physically Exploitable**: Yes. If the system clock experiences a backwards jump (e.g., via NTP synchronization) or a snapshot metadata creation timestamp is set in the future relative to the system time.

#### Technical Description
The streaming snapshot selection parses timestamps as follows:
```rust
let timestamp = created
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs() as i64;
```
If the file's creation time is prior to `UNIX_EPOCH`, or if clock drift causes `duration_since` to return an `Err`, the unwrap will panic, crashing the active task/worker.

#### Remediation
Avoid unwrapping `SystemTimeError`. Handle clock anomalies gracefully:
```rust
let timestamp = match created.duration_since(std::time::UNIX_EPOCH) {
    Ok(duration) => duration.as_secs() as i64,
    Err(_) => 0, // Fallback safely to a default epoch timestamp
};
```

---

### [MEDIUM] Unnecessary and Fragile `unsafe` blocks for Deserialization
- **File**: `crates/op-deployment/src/image_manager.rs`
- **Lines**: 330-331, 347
- **Physically Exploitable**: No.

#### Technical Description
The parser utilizes `unsafe` deserialization blocks with `simd_json`:
```rust
if let Ok(metadata) =
    unsafe { simd_json::from_str::<ImageMetadata>(&mut content) }
```
`simd-json`'s `from_str` can be destructive/in-place, and standard practice requires carefully managing slice lifetimes. While there are no references in `ImageMetadata` that borrow from the lifetime of `content`, using raw `unsafe` here is a code smell that bypasses compiler invariants.

#### Remediation
Use the safe, standard `simd_json::serde::from_slice` or `simd_json::from_reader` APIs:
```rust
let mut content_bytes = async_fs::read(&metadata_path).await?;
let metadata: ImageMetadata = simd_json::serde::from_slice(&mut content_bytes)?;
```

---

### [LOW] Incomplete Symlink Resolution in Deduplication Search
- **File**: `crates/op-deployment/src/image_manager.rs`
- **Lines**: 239-257
- **Physically Exploitable**: No.

#### Technical Description
The function `find_file_in_previous_images` only resolves one level of symlinks:
```rust
let resolved_meta = async_fs::symlink_metadata(&resolved).await?;
if !resolved_meta.is_symlink() {
    return Ok(Some(resolved));
}
```
If a file has a multi-tiered/nested symlink relationship (e.g., `A -> B -> C`), the method fails to follow the chain to the true underlying inode and falsely decides that no reusable file is present, causing duplicate file copies and wasting disk space.

#### Remediation
Implement a recursive or loop-based canonicalization step (up to a maximum depth limit) to resolve multi-level symlinks completely.

---

## 3. Schema-as-Code Compliance & Protocol Violations

### Ad-hoc JSON Schema Violation
- **File**: `crates/op-deployment/src/image_manager.rs`
- **Lines**: 19-38

#### Technical Description
The contract metadata formats `ImageMetadata` and `FileEntry` are written directly to disk as unversioned, ad-hoc JSON structures (`.image-metadata.json`). 

In a strict schema-as-code discipline, all data contracts, state representations, and persisted metadata configurations must use unified versioned definitions (such as Protocol Buffers or OSCAL component schemas). Defining these directly as ad-hoc Rust structs with generic serialization rules bypasses centralized compliance registries, impedes automated policy checks, and makes schemas prone to silent serialization breaks during future crate updates.

#### Remediation
1. Define the deployment metadata schema inside a shared Protocol Buffer definition file (e.g., `deployment.proto`).
2. Generate the Rust data structures automatically using `prost` (as done elsewhere in the workspace, e.g., in `op-cache` and `op-chat`).
3. Embed an explicit schema version field to handle backward compatibility gracefully during subsequent architectural enhancements.