# Production Security and Quality Audit Report

## 1. License & Dependency Compliance Scan

### 1.1 License Extraction
* **Audited Crate**: `op-deployment`
* **Workspace Package License**: `Apache-2.0` (defined in `Cargo.toml:43`)
* **Inherited License**: The `crates/op-deployment/Cargo.toml` manifest correctly inherits the workspace license setting:
  ```toml
  license.workspace = true
  ```
  Consequently, the `op-deployment` crate is officially licensed under **Apache-2.0**.
* **Other Audited Crates**: `op-dbus` (defined in `Cargo.toml:79-88`) also inherits the workspace license via `license.workspace = true`, resolving to **Apache-2.0**.

### 1.2 Copyleft and Restricted License Scan (GPL/AGPL/SSPL)
A comprehensive scan of `Cargo.lock` was conducted to identify transitive or direct dependencies licensed under restrictive copyleft agreements (such as GPL, AGPL, or SSPL) that could compromise the commercial usability of the Apache-2.0 workspace.

* **Result**: **No GPL, AGPL, or SSPL dependencies were found** in the audited `Cargo.lock`.
* **Notable Weak Copyleft Crates**:
  * `cozo` (version `0.7.6`) is licensed under the **MPL-2.0** (Mozilla Public License 2.0). MPL-2.0 is a weak copyleft license. It is legally compatible with Apache-2.0 workspace compositions, provided that any direct modifications to `cozo` files themselves remain licensed under the MPL-2.0.

### 1.3 Crates with Missing License Fields
All packages defined and managed within the audited workspace files (`Cargo.toml`) contain a valid `license.workspace` or `license` field. There are no crates in the provided workspace source lacking a designated license field.

---

## 2. Critical Security Vulnerabilities

### Finding 1: Path Traversal & Arbitrary Directory Deletion (CRITICAL)
* **Location**: `crates/op-deployment/src/image_manager.rs:410-438`
* **Impact**: Direct, arbitrary directory deletion on the host system.
* **Mechanism**:
  The `delete_image` function takes an unvalidated string slice `image_name` and resolves the targeted path using:
  ```rust
  let image_path = self.images_dir.join(image_name);
  ```
  If `image_name` contains path traversal segments (e.g., `../../../../usr/local/vital_service`), `Path::join` will resolve the path to a location outside the designated sandbox directory `self.images_dir`. 
  
  The function subsequently executes a recursive directory deletion:
  ```rust
  async_fs::remove_dir_all(&image_path).await?;
  ```
  If the application is running on a BTRFS filesystem, it executes the equivalent external command:
  ```rust
  let output = Command::new("btrfs")
      .args(["subvolume", "delete"])
      .arg(&image_path)
      .output()
      .await?;
  ```
* **Remediation**:
  Normalize and sanitize the target path before any operations to ensure it remains strictly within the sandbox boundaries:
  ```rust
  let image_path = self.images_dir.join(image_name);
  let canonical_images_dir = self.images_dir.canonicalize()?;
  let canonical_image_path = image_path.canonicalize()?;
  if !canonical_image_path.starts_with(&canonical_images_dir) {
      anyhow::bail!("Path traversal attempt detected");
  }
  ```

---

### Finding 2: Undefined Behavior & Memory Safety via Misuse of `simd_json` (CRITICAL)
* **Location**: `crates/op-deployment/src/image_manager.rs:363`, `crates/op-deployment/src/379`
* **Impact**: Potential segmentation faults, heap out-of-bounds reads, or memory corruption.
* **Mechanism**:
  The image manager attempts to parse metadata using `simd_json::from_str` within an `unsafe` block:
  ```rust
  let mut content = async_fs::read_to_string(&metadata_path).await?;
  if let Ok(metadata) =
      unsafe { simd_json::from_str::<ImageMetadata>(&mut content) }
  ```
  The `simd-json` parser is designed for high-performance SIMD execution and explicitly requires that the input slice/string is padded with a specific number of padding bytes (`simd_json::SIMDJSON_PADDING`, usually 32 bytes) past the end of the data. 

  Using `async_fs::read_to_string` produces a standard, unpadded Rust `String`. Passing an unpadded string directly to the `unsafe` SIMD-accelerated `from_str` interface violates memory alignment and safety invariant guarantees, resulting in Undefined Behavior (UB) when the SIMD vector reads past the allocated heap boundary.
* **Remediation**:
  Use `simd_json::to_padded_container` or convert the data to a padded vector (`Vec<u8>`) using `simd_json::Padded_Vec` before calling the unsafe parsing functions, or leverage safe deserialization wrappers that handle padding transparently.

---

### Finding 3: Arbitrary Symlink Creation & Arbitrary File Access (HIGH)
* **Location**: `crates/op-deployment/src/image_manager.rs:244-279`
* **Impact**: Uncontrolled symlink resolution allowing arbitrary file reads and cloning of sensitive host files into deployment images.
* **Mechanism**:
  The `find_file_in_previous_images` function searches for matching files in prior deployments. If the file is a symlink, it attempts to follow it:
  ```rust
  let target = async_fs::read_link(&file_path).await?;
  let resolved = if target.is_absolute() {
      target
  } else {
      file_path.parent().unwrap().join(&target)
  };
  ```
  Because the target of the symlink is never validated, a malicious or compromised deployment image can include a symlink pointing to `/etc/passwd` or other sensitive system configurations. 
  
  When a new deployment is built and a file with the same name is requested, `create_image` resolves this path to the absolute system path. It then generates a new symlink to it or copies it directly into the new build on non-Unix environments:
  ```rust
  async_fs::copy(&previous_file, &dest_path)
  ```
* **Remediation**:
  Strictly validate that the resolved target path of any processed symlink resides within the canonical directory of the respective previous image. Do not follow or replicate links that resolve outside of the managed deployment paths.

---

## 3. Quality & Schema Discipline

### Finding 4: Ad-hoc Data Serialization Contracts (SCHEMA-AS-CODE VIOLATION)
* **Location**: `crates/op-deployment/src/image_manager.rs:13-38`
* **Impact**: Loss of data integrity, failure in cross-system integration, and potential schema drift.
* **Mechanism**:
  The `ImageMetadata` and `FileEntry` data contracts are defined as ad-hoc, raw Rust structures serialized directly to JSON via `serde`. 
  
  This violates the project's strict **Schema-as-Code** discipline, which requires data contracts to be declared via standardized, versioned Protocol Buffers or formal OSCAL schemas. Defining structures natively in Rust makes it difficult to enforce schema validation at rest or maintain reliable backward compatibility across system updates.
* **Remediation**:
  Re-declare `ImageMetadata` and `FileEntry` inside a versioned `.proto` definition and generate the corresponding Rust structures using `prost` during compilation.

---

### Finding 5: Unhandled Clock Reversals Leading to Panic (MEDIUM)
* **Location**: `crates/op-deployment/src/image_manager.rs:400`
* **Impact**: Thread panic and local Denial of Service (DoS) during snapshot queries.
* **Mechanism**:
  The snapshot manager queries file creation time and calculates elapsed seconds:
  ```rust
  let timestamp = created
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_secs() as i64;
  ```
  If the host system's hardware clock is incorrectly configured (e.g., set to a date prior to 1970-01-01) or experiences a negative clock step via NTP synchronization, `duration_since` will return an `Err`. Calling `.unwrap()` on this result will instantly trigger a panic, crashing the active runtime task.
* **Remediation**:
  Handle time-travel and clock reversal cases safely:
  ```rust
  let timestamp = created
      .duration_since(std::time::UNIX_EPOCH)
      .map(|d| d.as_secs() as i64)
      .unwrap_or(0);
  ```

---

### Finding 6: Command Argument Injection Risk (LOW)
* **Location**: `crates/op-deployment/src/image_manager.rs:74-77`
* **Impact**: Potential command execution failure or subversion of `findmnt` execution parameters.
* **Mechanism**:
  System validation is executed using `findmnt` without separating system flags from arguments:
  ```rust
  let output = Command::new("findmnt")
      .args(["-n", "-o", "FSTYPE", "-T"])
      .arg(path)
  ```
  If `path` starts with a hyphen (e.g., `-o`), `findmnt` may interpret it as a command option rather than a positional file-path argument.
* **Remediation**:
  Use the standard double-dash separator `--` to terminate option parsing:
  ```rust
  let output = Command::new("findmnt")
      .args(["-n", "-o", "FSTYPE", "-T", "--"])
      .arg(path)
  ```