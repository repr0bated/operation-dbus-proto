| Severity | Issue | Evidence (file:line) | Recommendation |
| :--- | :--- | :--- | :--- |
| **High** | Undefined Behavior: Memory safety violation via unsafe `simd-json` parsing of unpadded strings | `crates/op-deployment/src/image_manager.rs:318`<br>`crates/op-deployment/src/image_manager.rs:335` | Allocate a padded buffer, use `simd_json::to_padded_bin`, or migrate to safe `serde_json` for local JSON parsing. |
| **High** | Denial of Service: Process memory exhaustion via reading whole files into RAM during hashing | `crates/op-deployment/src/image_manager.rs:241` | Stream file bytes incrementally to the `Sha256` hasher using a buffered reader. |
| **High** | Arbitrary File Copy & Host Information Disclosure via unsafe symlink resolution | `crates/op-deployment/src/image_manager.rs:224` | Resolve and canonicalize the symlink target, verifying it resides strictly within the authorized images directory before copying or symlinking. |
| **High** | Schema-as-Code Violation: Ad-hoc metadata contracts defined via unversioned Rust structs | `crates/op-deployment/src/image_manager.rs:18`<br>`crates/op-deployment/src/image_manager.rs:32` | Refactor deployment image metadata to utilize versioned Protocol Buffers or compliance-aligned OSCAL schemas. |
| **Medium** | Thread Pool Starvation: Synchronous blocking symlink creation within an async Tokio task | `crates/op-deployment/src/image_manager.rs:147` | Replace the synchronous `std::os::unix::fs::symlink` call with its non-blocking async counterpart, `tokio::fs::symlink`. |
| **Medium** | System Command Hijacking via reliance on relative PATH resolution | `crates/op-deployment/src/image_manager.rs:89`<br>`crates/op-deployment/src/image_manager.rs:285`<br>`crates/op-deployment/src/image_manager.rs:390` | Utilize absolute filesystem paths for system binaries (`/usr/bin/findmnt`, `/usr/bin/btrfs`) or enforce strict path sandboxing. |

---

### Detailed Findings & Technical Remediation

#### 1. Memory Safety Violation via Unsafe `simd-json` Parsing of Unpadded Strings
*   **Vulnerability Type**: Undefined Behavior / Out-of-bounds Read
*   **File Context**: `crates/op-deployment/src/image_manager.rs:318` and `crates/op-deployment/src/image_manager.rs:335`
*   **Description**: 
    The `ImageManager::list_images` and `ImageManager::get_image` functions load metadata files using `async_fs::read_to_string` and parse them via `simd_json::from_str`. The invocation is wrapped in an `unsafe` block:
    ```rust
    let mut content = async_fs::read_to_string(&metadata_path).await?;
    if let Ok(metadata) = unsafe { simd_json::from_str::<ImageMetadata>(&mut content) }
    ```
    The `simd-json` library relies on vector instructions (AVX2/SSE) which read chunks of data (typically 32 or 64 bytes) at a time. To prevent reading past the end of the allocated memory, `simd-json` strictly requires that input buffers possess `simd_json::SIMD_JSON_PADDING` bytes of extra padding.
    
    Standard `String` structures returned by `async_fs::read_to_string` are not padded with these trailing sentinel bytes. Calling `simd_json::from_str` directly on standard unpadded strings leads to out-of-bounds memory reads, causing unpredictable application crashes (segmentation faults) or sensitive memory disclosure under certain allocations.
*   **Remediation**:
    For metadata parsing where extreme throughput is not critical, replace `simd_json` with standard, safe `serde_json::from_str`. If `simd_json` is required, read the file into a `Vec<u8>` and ensure padding is added, or use the padding utilities provided by the crate:
    ```rust
    let mut bytes = async_fs::read(&metadata_path).await?;
    // Ensure padding is allocated
    bytes.reserve(simd_json::SIMD_JSON_PADDING);
    let metadata: ImageMetadata = simd_json::from_slice(&mut bytes)?;
    ```

#### 2. Process Memory Exhaustion via Whole-File Reads During Hashing
*   **Vulnerability Type**: Resource Exhaustion / Denial of Service (DoS)
*   **File Context**: `crates/op-deployment/src/image_manager.rs:241`
*   **Description**:
    When creating a deployment image, `calculate_file_hash` calculates the SHA256 signature of files added to the image. It reads the entire file into a heap-allocated buffer at once:
    ```rust
    let contents = async_fs::read(file_path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&contents);
    ```
    Since this crate manages deployment files (e.g., system images, virtual machine disks, or container layers), file sizes can range from hundreds of megabytes to tens of gigabytes. Reading these files in their entirety into memory will rapidly exhaust the host's RAM, causing the operating system's OOM-killer to terminate the application process.
*   **Remediation**:
    Refactor the hash calculation to process the files in chunks:
    ```rust
    use tokio::io::AsyncReadExt;
    
    async fn calculate_file_hash(&self, file_path: &Path) -> Result<String> {
        let mut file = async_fs::File::open(file_path).await?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 16384]; // 16KB buffer
        
        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        
        let hash = hasher.finalize();
        Ok(format!("{:x}", hash))
    }
    ```

#### 3. Arbitrary File Copy & Host Information Disclosure via Unsafe Symlink Resolution
*   **Vulnerability Type**: Path Traversal / Unauthorized File Access
*   **File Context**: `crates/op-deployment/src/image_manager.rs:224`
*   **Description**:
    The deduplication engine implements a method `find_file_in_previous_images` to find and reference existing files. If a matched target file is a symlink, the code resolves it manually:
    ```rust
    let target = async_fs::read_link(&file_path).await?;
    let resolved = if target.is_absolute() {
        target
    } else {
        file_path.parent().unwrap().join(&target)
    };
    ```
    The resolved path is returned directly if it exists, without verifying that the final target path is contained within the `images_dir` sandbox. If a previous deployment contained a symbolic link pointing to a sensitive file on the host system (e.g., `/etc/passwd` or `/root/.ssh/id_rsa`), subsequent images created with the same file name will automatically reference and potentially expose the contents of these host files to the deployment image folder.
*   **Remediation**:
    Verify that the canonicalized target path of any symbolic link is a descendant of the configured base directory:
    ```rust
    let resolved = if target.is_absolute() {
        target
    } else {
        file_path.parent().unwrap().join(&target)
    };
    
    // Canonicalize path to resolve relative segments and symlinks
    if let Ok(canonical_resolved) = tokio::fs::canonicalize(&resolved).await {
        let canonical_base = tokio::fs::canonicalize(&self.images_dir).await?;
        if !canonical_resolved.starts_with(&canonical_base) {
            anyhow::bail!("Directory traversal detected via symlink target");
        }
        return Ok(Some(canonical_resolved));
    }
    ```

#### 4. Schema-as-Code Violation: Ad-hoc Metadata Contracts
*   **Vulnerability Type**: Compliance / Schema-as-Code Disciplinary Gap
*   **File Context**: `crates/op-deployment/src/image_manager.rs:18` and `crates/op-deployment/src/image_manager.rs:32`
*   **Description**:
    The system defines its key domain models (`ImageMetadata` and `FileEntry`) as ad-hoc Rust structs with generic JSON serialization markers. This violates the repository's strict architecture policy of defining structural state and exchange contracts via versioned schemas (such as Protocol Buffers or OSCAL-compliant schemas). 
    
    Exposing raw JSON documents for deployment metadata leads to integration fragility and compliance gaps. Because this is a deterministic control plane, metadata should follow versioned, backward-compatible definitions to ensure safety and auditability during upgrades.
*   **Remediation**:
    Define `ImageMetadata` and `FileEntry` as a Protocol Buffer message inside a shared `.proto` file (compiled using `prost`):
    ```protobuf
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

#### 5. Synchronous Blocking Symlink Creation Within Async Tokio Executor Threads
*   **Vulnerability Type**: Thread Starvation / Performance Degradation
*   **File Context**: `crates/op-deployment/src/image_manager.rs:147`
*   **Description**:
    During the file addition stage of image creation, the program creates symlinks for deduplicated assets:
    ```rust
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&relative_target, &dest_path)
            .context(format!("Failed to create symlink: {}", dest_path.display()))?;
    }
    ```
    `std::os::unix::fs::symlink` is a synchronous blocking system call. When executed directly inside an asynchronous Tokio thread, it halts the execution of any other tasks multiplexed onto that specific runtime thread. Under high deployment workloads with many files, this blocks the Tokio worker pool, causing application-wide latency spikes and potential network timeouts.
*   **Remediation**:
    Replace the standard blocking library call with Tokio's non-blocking filesystem counterpart:
    ```rust
    #[cfg(unix)]
    {
        tokio::fs::symlink(&relative_target, &dest_path)
            .await
            .context(format!("Failed to create symlink: {}", dest_path.display()))?;
    }
    ```

#### 6. System Command Hijacking via Reliance on Relative PATH Resolution
*   **Vulnerability Type**: Local Privilege Escalation / Path Manipulation
*   **File Context**: `crates/op-deployment/src/image_manager.rs:89`, `crates/op-deployment/src/image_manager.rs:285`, and `crates/op-deployment/src/image_manager.rs:390`
*   **Description**:
    The image manager spawns external system utilities (`findmnt` and `btrfs`) to perform system-level tasks:
    ```rust
    let output = Command::new("findmnt")
        .args(["-n", "-o", "FSTYPE", "-T"])
    ```
    Because the system commands are referenced by relative names, the OS resolves their locations using the runtime environment's `PATH` variable. If the application runs as root or with high-privilege capabilities, an attacker who can modify the environment's `PATH` variable can divert the execution to a malicious executable named `findmnt` or `btrfs`, leading to arbitrary command execution at elevated privileges.
*   **Remediation**:
    Define absolute path constants for all critical system commands instead of relying on environment-defined paths:
    ```rust
    const PATH_FINDMNT: &str = "/bin/findmnt";
    const PATH_BTRFS: &str = "/sbin/btrfs";
    
    // Usage
    let output = Command::new(PATH_FINDMNT)
        .args(["-n", "-o", "FSTYPE", "-T"])
    ```