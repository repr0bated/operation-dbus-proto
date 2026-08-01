# Production Security and Quality Audit: op-deployment

## 1. Audit Summary

This document presents a production security and quality audit of the `op-deployment` crate. The codebase manages BTRFS-based deployment images using symlink deduplication and BTRFS snapshots. 

During the audit, two **Critical** security vulnerabilities and several high-to-medium risk flaws were identified. These vulnerabilities allow for host-level arbitrary file access (sandbox escape) and memory corruption leading to potential privilege escalation.

---

## 2. Test Review (ROLE: Tests)

### Total Test Functions Count
* **Total test functions found**: `1`

### Representative Test
* **`test_image_manager_init`** — `crates/op-deployment/src/image_manager.rs:373`
  * *Description*: Verifies that structural subdirectories (`images` and `snapshots`) are correctly initialized within a temporary directory.

### Property Tests & Fuzzing
* **No property tests** (e.g., using `proptest` or `quickcheck`) or fuzzing harness setups are present anywhere in the provided files.

### Testing Risk Assessment: HIGH RISK
The test coverage in this crate is extremely poor. A single, superficial test function exists, verifying only the creation of directories during initialization. The core business logic—including symlink relative path calculation, previous image lookup, metadata serialization, file hashing, and BTRFS command execution—is completely untested. In system-level software running with elevated root privileges (to execute BTRFS snapshot operations), this lack of testing poses an extreme risk of regression, state corruption, and security bypasses.

---

## 3. Schema-as-Code Violations

The crate defines its core deployment metadata contracts as ad-hoc Rust serialization structs rather than formal, versioned schemas:

* **Ad-hoc Metadata Definitions**:
  * `ImageMetadata` — `crates/op-deployment/src/image_manager.rs:16`
  * `FileEntry` — `crates/op-deployment/src/image_manager.rs:31`
* **Ad-hoc Serialization**:
  * Metadata is serialized to JSON and stored directly on the file system under `.image-metadata.json` (`crates/op-deployment/src/image_manager.rs:154-157`).

### Risk Analysis
Storing system deployment contracts as serialized ad-hoc JSON structs lacks validation, version negotiation, and strict cross-language boundaries. Schema definitions should be expressed via Protocol Buffers (or structured OSCAL-compliant formats) to ensure backwards-compatibility, schema evolution safety, and tamper-resistant validation.

---

## 4. Detailed Findings

### CRITICAL: Host Arbitrary Symlink Injection & Sandbox Escape
* **Location**: `crates/op-deployment/src/image_manager.rs:201-243` (`find_file_in_previous_images`) and `crates/op-deployment/src/image_manager.rs:165-198` (`calculate_relative_path`)
* **Impact**: Sandbox escape and arbitrary host file read.
* **Description**: 
  The file deduplication mechanism manually resolves symlinks from previous deployment images. If a previous image contains a symlink pointing to an absolute host path (e.g., `/etc/shadow`), `find_file_in_previous_images` resolves it and returns the target absolute path:
  ```rust
  let resolved = if target.is_absolute() {
      target
  } else {
      file_path.parent().unwrap().join(&target)
  };
  ```
  If `resolved` is a valid file, it is returned as `previous_file`. 
  Inside `create_image` (line 128), `calculate_relative_path` is called with this target absolute path. Because there is no check ensuring the resolved target remains within the base image directory boundary, `calculate_relative_path` generates a relative path escaping the deployment tree:
  ```rust
  // If base is /deploy/images/img2 and target is /etc/shadow,
  // relative resolves to ../../../../etc/shadow
  ```
  A symlink is then created at `dest_path` pointing to this relative escape target. Any low-privileged attacker capable of placing a symlink into an image directory (e.g., via standard tarball extractions) can force the deployment manager to generate valid symlinks pointing to arbitrary host files in subsequent images, escaping the deployment container.

---

### CRITICAL: Memory Corruption via Unsafe Deserialization of Untrusted Metadata
* **Location**: `crates/op-deployment/src/image_manager.rs:303` and `317` (`list_images` and `get_image`)
* **Impact**: Process crash or potential arbitrary code execution with elevated privileges.
* **Description**:
  The image manager parses metadata using the unsafe in-place parser `simd_json::from_str`:
  ```rust
  if let Ok(metadata) =
      unsafe { simd_json::from_str::<ImageMetadata>(&mut content) }
  ```
  `simd-json`'s in-place parsing requires strict memory alignment and mutates the buffer in-place. Because the JSON string is read directly from disk (`.image-metadata.json`), any local unprivileged process or external actor with write access to the deployment metadata files can modify the JSON. Passing untrusted, mutable file input directly into an `unsafe` parser bypasses Rust's safety invariants. Any memory misalignment, malformed surrogate pair, or payload designed to exploit the SIMD execution path will trigger undefined behavior, memory corruption, or direct privilege escalation (since this binary executes privileged `btrfs` commands as `root`).

---

### HIGH: Memory Exhaustion via Whole-File Loading for Hashing
* **Location**: `crates/op-deployment/src/image_manager.rs:247` (`calculate_file_hash`)
* **Impact**: Denial of Service (OOM Crash).
* **Description**:
  The hashing mechanism reads the entire contents of a deployment file into memory to compute its SHA-256 digest:
  ```rust
  let contents = async_fs::read(file_path).await?;
  ```
  In deployment systems (where images routinely contain large binaries, VM disks, or database dumps), reading entire multi-gigabyte files into heap memory instantly exhausts host RAM, leading to out-of-memory crashes of the control plane daemon.
* **Remediation**: Re-implement hashing using buffered chunk streams (e.g., utilizing `tokio::io::AsyncReadExt` to read and update the hash state incrementally).

---

### MEDIUM: Privilege Abuse / Command Flag Injection
* **Location**: `crates/op-deployment/src/image_manager.rs:271`, `341`, and `360` (`create_image_snapshot` and `delete_image`)
* **Impact**: Manipulation of system command executions.
* **Description**:
  The crate invokes system binaries (`btrfs`) using `tokio::process::Command`. While it does not spawn a raw shell (avoiding direct shell command injection), it uses unvalidated string variables—such as `image_name` and `snapshot_name`—directly as arguments to `btrfs subvolume snapshot` or `btrfs subvolume delete`. If `image_name` is supplied via user input and starts with hyphens (e.g., `--some-flag`), it can alter the execution behavior of the underlying command, causing unauthorized subvolume mutations or deletion of unexpected paths.
* **Remediation**: Validate `image_name` using strict alphanumeric regex patterns to prevent directory traversals and command flag manipulation before passing arguments to system processes.

---

### MEDIUM: State Inconsistencies due to Lack of Directory Locking
* **Location**: `crates/op-deployment/src/image_manager.rs:101-163` (`create_image`)
* **Impact**: Local race conditions, corrupted metadata, and inconsistent deployments.
* **Description**:
  The image manager performs highly destructive filesystem mutations—such as symlink creation, file copies, and metadata serialization—without acquiring any directory-level or global lock. If two concurrent operations call `create_image` or if a deletion occurs concurrently via `delete_image`, the state of the `.image-metadata.json` or BTRFS subvolumes will result in partial writes, broken relative symlinks, or corrupted JSON files.
* **Remediation**: Implement a file-locking mechanism (e.g., using `fs2` for advisory locking) on the base directory during all deployment image state changes.