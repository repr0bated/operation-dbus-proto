### Memory Mapping & Large Allocations

#### Memory Map Table
| Site | file:line | Type (ro/rw/sled) | Risk |
| :--- | :--- | :--- | :--- |
| N/A | N/A | N/A | No explicit memory mapping (`memmap2`, `mmap`, `MmapMut`, or `MmapOptions`) was found in the provided Rust source files. |

> **Note on sled**: While `sled` is included in the workspace dependencies (via `cozo` in `Cargo.toml`), no embedded database using `sled` is instantiated or mounted within the audited `op-deployment` crate. Consequently, there is no direct risk of running sled on a `tmpfs` or `noexec` mount within this crate's scope.

#### Large Heap Allocations
* **`crates/op-deployment/src/image_manager.rs:241`**
  ```rust
  let contents = async_fs::read(file_path).await?;
  ```
  **Risk**: High. When calculating the SHA256 hash of files added to a deployment image, the entire file content is loaded into memory at once via `async_fs::read`. For large deployment payloads (such as virtual machine disk images, large binary packages, or container layers exceeding 1MB to multiple gigabytes), this results in massive, contiguous heap allocations. This can easily trigger Out-Of-Memory (OOM) panics, leading to Denial of Service (DoS) of the native control plane.
  
  **Remediation**: Stream the file contents in chunks (e.g., 64KB buffers) using a buffered reader and feed them incrementally into the `Sha256` hasher:
  ```rust
  use tokio::io::AsyncReadExt;
  let mut file = async_fs::File::open(file_path).await?;
  let mut hasher = Sha256::new();
  let mut buffer = [0u8; 65536];
  loop {
      let n = file.read(&mut buffer).await?;
      if n == 0 { break; }
      hasher.update(&buffer[..n]);
  }
  ```

---

### Performance & Allocation Audit

#### 1. Unpadded Buffer Usage with `simd-json` (Undefined Behavior / Crash Risk)
* **`crates/op-deployment/src/image_manager.rs:283`**
  ```rust
  unsafe { simd_json::from_str::<ImageMetadata>(&mut content) }
  ```
* **`crates/op-deployment/src/image_manager.rs:299`**
  ```rust
  let metadata: ImageMetadata = unsafe { simd_json::from_str(&mut content)? };
  ```

**Risk**: High / Undefined Behavior. `content` is loaded via `async_fs::read_to_string` (lines 281 and 298), which returns a standard `std::string::String`. `simd-json` explicitly requires that the input buffer have trailing padding (typically `simd_json::PADDING_SIZE` bytes, which is 32 or 64 bytes) to safely perform vectorized SIMD reads. Vectorized instructions read chunks of memory at once; if the JSON payload ends right at the allocation boundary (especially near a page boundary), `simd-json` will perform an out-of-bounds read. This can result in immediate segmentation faults or unpredictable memory leakages.

**Remediation**: Ensure the buffer is converted to a padded container before calling unsafe parsing functions:
```rust
use simd_json::to_padded_container;
let mut padded = to_padded_container(content.as_bytes());
let metadata: ImageMetadata = unsafe { simd_json::from_slice(&mut padded)? };
```

#### 2. `format!()` in Hot Paths / Loops
* **`crates/op-deployment/src/image_manager.rs:121`**
  ```rust
  .context(format!("Failed to create symlink: {}", dest_path.display()))
  ```
* **`crates/op-deployment/src/image_manager.rs:128`**
  ```rust
  .context(format!("Failed to copy file: {}", dest_path.display()))
  ```
* **`crates/op-deployment/src/image_manager.rs:144`**
  ```rust
  .context(format!("Failed to copy file: {}", file_path.display()))
  ```
* **`crates/op-deployment/src/image_manager.rs:164`**
  ```rust
  Ok(format!("{:x}", hash))
  ```

**Risk**: Moderate. These `format!` calls run inside the file processing loop (`for file_path in files`, line 92). In deployment scenarios with thousands of files, this causes intensive, unnecessary heap allocations on the happy path. Specifically, `anyhow::Context::context(format!(...))` eagerly allocates and formats the error string even when the operation succeeds.

**Remediation**: Use `with_context` to lazily evaluate error formatting only when an error actually occurs:
```rust
.with_context(|| format!("Failed to copy file: {}", dest_path.display()))
```

#### 3. Vector/String Reallocation in Loops
* **`crates/op-deployment/src/image_manager.rs:191`**
  ```rust
  let mut relative = PathBuf::new();
  ```
  **Risk**: Low. `calculate_relative_path` is called inside the loop (line 115) for deduplicated files. It allocates a new `PathBuf` and incrementally pushes path components.
  
  **Remediation**: Although path components are generally small, pre-allocating the `PathBuf` capacity if the base component count is known would prevent dynamic resizing of the underlying vector.

---

### Schema-As-Code Discipline

#### Ad-hoc Serialization of Critical Deployment Records
* **`crates/op-deployment/src/image_manager.rs:19-28`**
  ```rust
  pub struct ImageMetadata {
      pub name: String,
      pub path: PathBuf,
      pub created: i64,
      pub files: Vec<FileEntry>,
      pub total_size: u64,
      pub unique_size: u64,
      pub symlinked_size: u64,
  }
  ```
* **`crates/op-deployment/src/image_manager.rs:31-37`**
  ```rust
  pub struct FileEntry {
      pub path: PathBuf,
      pub is_symlink: bool,
      pub symlink_target: Option<PathBuf>,
      pub size: u64,
      pub hash: Option<String>,
  }
  ```

**Risk**: High / Structural. The deployment state is written to disk at `crates/op-deployment/src/image_manager.rs:153` as a serialized JSON file (`.image-metadata.json`). These structures are entirely ad-hoc Rust structs and are not versioned or validated against a canonical schema (such as OSCAL or Protocol Buffers). 

If the fields of `ImageMetadata` or `FileEntry` change in future versions of the code, existing `.image-metadata.json` files on active btrfs systems will fail to parse during startup or list operations (line 283), causing the deployment agent to crash.

**Remediation**: Formalize the data contracts. Define `ImageMetadata` and `FileEntry` as Protocol Buffer messages (utilizing the `prost` dependency already defined in the workspace):
```proto
syntax = "proto3";
package op.deployment.v1;

message FileEntry {
  string path = 1;
  bool is_symlink = 2;
  optional string symlink_target = 3;
  uint64 size = 4;
  optional string hash = 5;
}

message ImageMetadata {
  string name = 1;
  string path = 2;
  int64 created = 3;
  repeated FileEntry files = 4;
  uint64 total_size = 5;
  uint64 unique_size = 6;
  uint64 symlinked_size = 7;
}
```
Serialize deployment metadata using Protobuf to enforce backward/forward compatibility, or at minimum include an explicit `$schema` or `version` integer field in the JSON payload to allow managed migrations.

---

### Critical Security Vulnerabilities

#### 1. Critical: Unvalidated Path Traversal & Arbitrary Directory Write via `image_name`
* **Citations**: 
  * `crates/op-deployment/src/image_manager.rs:100`
  * `crates/op-deployment/src/image_manager.rs:251`
  * `crates/op-deployment/src/image_manager.rs:328`

**Vulnerability Description**:
The `ImageManager` API accepts `image_name: &str` without validation or sanitization. At line 100, the application resolves the directory path using:
```rust
let image_path = self.images_dir.join(image_name);
```
Under Rust's `std::path::Path::join` rules, if `image_name` is an absolute path (e.g., `/etc` or `/var/lib`), the base path (`self.images_dir`) is entirely discarded, and `image_path` resolves directly to the absolute path. 

This leads to the following directly exploitable attack vectors:
1. **Arbitrary Directory Creation**: An attacker providing `image_name` as `/etc/malicious` will trigger `async_fs::create_dir_all(&image_path)` on that directory (line 101).
2. **File Injection**: During file processing, files are copied directly into `image_path` (line 141):
   ```rust
   let dest_path = image_path.join(file_name);
   ```
   If `image_name` was resolved to `/usr/bin`, an attacker can write arbitrary executable files directly into system binary folders.
3. **Arbitrary Subvolume/Directory Deletion**: In `delete_image` (line 328), the path is computed similarly. If an attacker requests the deletion of `image_name = "../../var"`, it will trigger recursive directory deletions or `btrfs subvolume delete` on target system folders.

**Exploit Scenario**:
A remote attacker triggers an image creation with `image_name` set to `/etc/cron.d`. They include a file named `backdoor`. The system writes the file directly to `/etc/cron.d/backdoor`, achieving immediate remote root code execution.

**Remediation**:
Strictly sanitize the `image_name` parameter. Ensure it is purely alphanumeric and does not contain absolute indicators or path traversal components before joining:
```rust
let safe_name = Path::new(image_name)
    .file_name()
    .context("Invalid image name structure")?;
let image_path = self.images_dir.join(safe_name);
```

---

#### 2. Critical: Arbitrary Symlink Target Verification Bypass (TOCTOU & Information Disclosure)
* **Citations**:
  * `crates/op-deployment/src/image_manager.rs:214-227`
  * `crates/op-deployment/src/image_manager.rs:115-119`

**Vulnerability Description**:
When creating a new deployment image, `find_file_in_previous_images` attempts to find duplicate files to symlink instead of copying them. However, when it encounters an existing symlink (line 218), it follows it recursively:
```rust
let target = async_fs::read_link(&file_path).await?;
let resolved = if target.is_absolute() {
    target
} else {
    file_path.parent().unwrap().join(&target)
};
```
If the resolved path exists, it returns it as the reference file (line 224). The caller then calculates a relative path from the *new* destination folder back to this target and runs:
```rust
std::os::unix::fs::symlink(&relative_target, &dest_path)
```
There is no validation ensuring that `resolved` points within `self.images_dir`. If a malicious actor can place a symlink pointing to sensitive host system files (e.g., `/etc/shadow` or private SSH keys) inside a previous image metadata folder or database directory, the agent will happily generate relative symlinks in the new deployment pointing directly to these host-sensitive targets. 

When the deployment snapshot is streamed or packaged, the private keys/shadow file of the control host will be read and transmitted.

**Exploit Scenario**:
An attacker inserts a file entry with `is_symlink: true` pointing to `/etc/shadow` in a previous image directory. During the next run of `create_image`, the agent resolves the duplicate file, computes a relative path to the host's `/etc/shadow`, and creates a symlink in the public deployment folder. When streamed, the attacker receives the host system's password hashes.

**Remediation**:
Before resolving or linking any found reference file, strictly assert that its canonicalized path resides entirely within `self.images_dir`:
```rust
let canonical_resolved = resolved.canonicalize()?;
if !canonical_resolved.starts_with(&self.images_dir) {
    anyhow::bail!("Path traversal attempt via symlink target detected");
}
```