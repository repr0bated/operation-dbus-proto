## 1. D-Bus & IPC Attack Surface Audit

### Registered D-Bus Interfaces, Methods, and Signals
After a complete review of the provided files:
* `crates/op-deployment/src/image_manager.rs`
* `crates/op-deployment/src/lib.rs`
* `crates/op-deployment/Cargo.toml`
* `Cargo.toml`

**Result:** No D-Bus interfaces, methods, signals, system bus, or session bus registrations exist within the audited files of the `op-deployment` crate. While the workspace manifest (`Cargo.toml`) defines a workspace dependency on `zbus` (version 5.12), the active source files for this crate do not expose or register any IPC endpoints over D-Bus.

---

## 2. Security & Quality Findings

### [CRITICAL] Arbitrary Directory Deletion via Path Traversal in Image Deletion Fallback
* **File:** `crates/op-deployment/src/image_manager.rs`
* **Line Reference:** Line 440 (within `delete_image` at lines 397–446)

#### Description
The `delete_image` function takes an unvalidated `image_name` string slice from the caller and constructs the image directory path using `self.images_dir.join(image_name)`. 

If the workspace path traversal resolution is not executing on a BTRFS filesystem, the code falls back to using `async_fs::remove_dir_all(&image_path).await?` to clean up the directory:

```rust
if image_path.exists() {
    if self.is_btrfs(&self.base_path).await? {
        // ... BTRFS subvolume deletion
    } else {
        async_fs::remove_dir_all(&image_path).await?;
    }
}
```

Because there is no sanitization or validation of `image_name` (e.g. checking for `..` components or validating characters), an attacker who can invoke this image management method with a payload such as `../../../../etc` or `../../../../var` will cause the privileged daemon (which must run with elevated permissions to perform volume-level mounting/snapshots) to recursively delete arbitrary directories on the host filesystem.

#### Remediation
Strictly sanitize and canoncalize the `image_name` input prior to path construction. Implement a validator that restricts the name to safe, alphanumeric characters or checks that the resolved absolute path remains strictly within the bounds of `self.images_dir`:

```rust
let image_path = self.images_dir.join(image_name);
let canonical_path = image_path.canonicalize()?;
if !canonical_path.starts_with(&self.images_dir) {
    anyhow::bail!("Access denied: Path traversal detected");
}
```

---

### [HIGH] Host Arbitrary File Symlink Injection via Untrusted Metadata Manipulation
* **File:** `crates/op-deployment/src/image_manager.rs`
* **Line Reference:** Lines 128–146, 267–302, and 337–360

#### Description
During the `create_image` operation, the manager attempts to deduplicate files by finding if a file exists in previous images using `find_file_in_previous_images` (lines 128-132). 

1. `list_images` reads `.image-metadata.json` directly from subdirectories within `self.images_dir` and trusts the serialized `image.path` (PathBuf) parsed from JSON (lines 337–360).
2. If an attacker can write or manipulate the contents of any `.image-metadata.json` file on disk, they can point `image.path` to an arbitrary host directory (e.g., `/etc`).
3. `find_file_in_previous_images` resolves `file_path = image.path.join(file_name)` and, if the file exists on the host, follows any symlink using `async_fs::read_link` (lines 280–288).
4. `create_image` then calculates a relative path from the newly created image destination to the resolved file using `calculate_relative_path` (line 134) and creates a host-level symlink pointing directly to it (lines 137–141):

```rust
let relative_target =
    self.calculate_relative_path(dest_path.parent().unwrap(), &previous_file)?;

#[cfg(unix)]
{
    std::os::unix::fs::symlink(&relative_target, &dest_path)
        .context(format!("Failed to create symlink: {}", dest_path.display()))?;
}
```

This sequence allows an attacker with write access to the metadata storage path to trick the snapshot engine into constructing relative symlinks to arbitrary, sensitive host configuration files (such as `/etc/passwd` or `/etc/shadow`) inside the streaming image payload.

#### Remediation
1. Never trust the `path` field stored inside serialized `.image-metadata.json` files. Re-derive the image path dynamically using the validated image name concatenated with the trusted base directory: `self.images_dir.join(metadata.name)`.
2. Ensure that any calculated `relative_target` path does not escape the boundary of the image manager's isolated snapshot directories.

---

### [HIGH] Unsafe In-Place JSON Deserialization of Untrusted Disk Files
* **File:** `crates/op-deployment/src/image_manager.rs`
* **Line Reference:** Line 349 and Line 373

#### Description
The code loads metadata content from `.image-metadata.json` files and uses `simd_json::from_str` within an `unsafe` block:

```rust
let mut content = async_fs::read_to_string(&metadata_path).await?;
if let Ok(metadata) =
    unsafe { simd_json::from_str::<ImageMetadata>(&mut content) }
```

Calling `unsafe` deserialization using `simd-json` on mutable string buffers parsed from persistent storage is a major memory safety risk. Because `simd_json::from_str` modifies the underlying string in-place (null-terminating and rewriting escapes) and relies heavily on structured alignment constraints and SIMD register layout assumptions, any malformed, corrupted, or malicious mutation of `.image-metadata.json` can cause out-of-bounds reads/writes, alignment panics, or memory corruption.

#### Remediation
Replace the unsafe, mutable in-place deserializer with a safe parsing variant (such as standard `serde_json` or the safe API wrappers of `simd-json` that guarantee memory safety in the presence of unvalidated payloads):

```rust
let content = async_fs::read_to_string(&metadata_path).await?;
let metadata: ImageMetadata = serde_json::from_str(&content)?;
```

---

## 3. Schema-as-Code Compliance Audit

### ad-hoc Struct Serialization Violations
* **File:** `crates/op-deployment/src/image_manager.rs`
* **Line Reference:** Lines 16–25 (`ImageMetadata`) and Lines 28–35 (`FileEntry`)

#### Description
The data contracts representing deployment image states (`ImageMetadata` and `FileEntry`) are expressed as ad-hoc, raw Rust structs decorated with standard `#[derive(Serialize, Deserialize)]`. They are directly serialized to and deserialized from disk as raw, unversioned JSON structures.

This direct persistence violates the project's strict "schema-as-code" discipline. Because the structures are not bound to versioned, formal Protocol Buffers or OSCAL schemas, any changes to the field layouts of `ImageMetadata` or `FileEntry` across updates will lead to parsing breaks, unrecognized keys, silent failures, or potential undefined behavior when loading legacy images from disk.

#### Remediation
Define these objects using versioned Protocol Buffers (`.proto` files) and generate the corresponding Rust structures using `prost`. Save the data to disk using either Protobuf binary format or the standardized versioned JSON schema generated from the Protobuf model. Ensure compliance metrics are tracked according to OSCAL system security profiles.