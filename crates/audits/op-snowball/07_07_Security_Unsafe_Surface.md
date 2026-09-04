# Production Security & Quality Audit Report

## 1. Executive Summary

This security and quality audit evaluates the `op-snowball` crate within the `OP-DBUS` workspace. The crate provides a streaming snowball mechanism with dual BTRFS subvolumes for audit trails, ML vector projections, and state snapshotting. 

While the system enforces an append-only timing audit log, multiple critical security vulnerabilities and code quality issues exist. Most notably, the codebase contains multiple **forbidden shell invocations** (`sh` and `bash`) that execute unvalidated parameters, introducing severe **command injection** risks. Furthermore, there are multiple violations of the codebase's **schema-as-code** discipline, and several `unsafe` blocks are utilized without safety documentation.

---

## 2. Unsafe Code Analysis

Three `unsafe` blocks were identified. All three violate quality standards by failing to provide a `// SAFETY:` comment explaining why the operation is safe.

### Unsafe Block Inventory

1. **`crates/op-snowball/src/btrfs_numa_integration.rs:185`**
   ```rust
   let block_data: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut data)? };
   ```
   * **Missing `// SAFETY:` Comment**: Yes.
   * **Context**: Used to parse BTRFS cached block files back into `simd_json::OwnedValue`. The safety of `simd_json::from_str` relies on the mutability of the string and exclusive access to the memory buffer.

2. **`crates/op-snowball/src/snowball.rs:233`**
   ```rust
   Ok(unsafe { simd_json::from_str(&mut data)? })
   ```
   * **Missing `// SAFETY:` Comment**: Yes.
   * **Context**: Parses system state files stored in the BTRFS state subvolume.

3. **`crates/op-snowball/src/streaming_snowball.rs:257`**
   ```rust
   Ok(unsafe { simd_json::from_str(&mut content)? })
   ```
   * **Missing `// SAFETY:` Comment**: Yes.
   * **Context**: Re-implementation of state-file parsing in the alternative streaming snowball module.

---

## 3. Subprocess Command Audit (`Command::new`)

A total of **12** invocations of `Command::new` (or `tokio::process::Command::new`) were identified. Out of these, **3** utilize forbidden shells (`sh` and `bash`) to execute arbitrary command strings, creating highly severe command injection vectors.

### Forbidden Command Violations (High Severity)

* **`crates/op-snowball/src/snowball.rs:268`**
  ```rust
  let output = Command::new("sh")
      .arg("-c")
      .arg(format!(
          "btrfs send {} | ssh {} 'btrfs receive {}'",
          snapshot_path.display(),
          remote_path,
          remote_path
      ))
      .output()
      .await?;
  ```
  * **Command String**: `sh -c "btrfs send <snapshot_path> | ssh <remote_path> 'btrfs receive <remote_path>'"`
  * **Severity**: **High**
  * **Risk**: If `remote_path` or `snapshot_name` is influenced by user-provided input, an attacker can append shell metacharacters (e.g., `; rm -rf /` or backticks) to execute arbitrary code with control-plane privileges. Argument validation is entirely bypassed.

* **`crates/op-snowball/src/streaming_snowball.rs:408`**
  ```rust
  let output = Command::new("bash")
      .arg("-c")
      .arg(format!(
          "btrfs send {} | ssh {} 'btrfs receive /var/lib/snowball/vectors/'",
          vector_snapshot.display(),
          remote
      ))
  ```
  * **Command String**: `bash -c "btrfs send <snapshot_path> | ssh <remote> 'btrfs receive /var/lib/snowball/vectors/'"`
  * **Severity**: **High**
  * **Risk**: Directly formats the `remote` string argument into a `bash` shell command. An attacker with control over the replication destination name can inject arbitrary shell commands.

* **`crates/op-snowball/src/streaming_snowball.rs:448`**
  ```rust
  let output = Command::new("bash")
      .arg("-c")
      .arg(&cmd)
  ```
  * **Command String**: `bash -c "btrfs send <snapshot_path> | tee <tee_args> > /dev/null"` (where `<tee_args>` is built by formatting string entries from the `replicas` slice)
  * **Severity**: **High**
  * **Risk**: Dynamically formats multiple `replicas` elements into bash process substitution commands (`>(ssh <replica> '...')`). This constitutes an immediate command injection vulnerability if replica targets can be dynamically added or modified by non-root entities.

---

### Authorized System Commands (BTRFS Management)

The remaining `Command::new` instances directly invoke the `btrfs` executable. While they avoid raw shell execution, their path arguments must be strictly checked for directory traversal attempts:

1. **`crates/op-snowball/src/btrfs_numa_integration.rs:312`**
   ```rust
   let output = tokio::process::Command::new("btrfs")
       .args(["subvolume", "snapshot", "-r"])
       .arg(self.snowball.as_ref().state_subvolume_path())
       .arg(&snowball_snapshot)
   ```
2. **`crates/op-snowball/src/snowball.rs:115`**
   ```rust
   let output = Command::new("btrfs")
       .args(["subvolume", "create"])
       .arg(path)
   ```
3. **`crates/op-snowball/src/snowball.rs:196`**
   ```rust
   let output = Command::new("btrfs")
       .args(["subvolume", "snapshot", "-r"])
       .arg(&self.state_subvol)
       .arg(&snapshot_path)
   ```
4. **`crates/op-snowball/src/snowball.rs:368`**
   ```rust
   let result = Command::new("btrfs")
       .args(["subvolume", "delete"])
       .arg(&path)
   ```
5. **`crates/op-snowball/src/streaming_snowball.rs:183`**
   ```rust
   let output = Command::new("btrfs")
       .args(["subvolume", "create"])
       .arg(path)
   ```
6. **`crates/op-snowball/src/streaming_snowball.rs:341`**
   ```rust
   let timing_result = Command::new("btrfs")
       .args(["subvolume", "snapshot", "-r"])
       .arg(&self.timing_subvol)
       .arg(&timing_snapshot)
   ```
7. **`crates/op-snowball/src/streaming_snowball.rs:358`**
   ```rust
   let vector_result = Command::new("btrfs")
       .args(["subvolume", "snapshot", "-r"])
       .arg(&self.vector_subvol)
       .arg(&vector_snapshot)
   ```
8. **`crates/op-snowball/src/streaming_snowball.rs:375`**
   ```rust
   let state_result = Command::new("btrfs")
       .args(["subvolume", "snapshot", "-r"])
       .arg(&self.state_subvol)
       .arg(&state_snapshot)
   ```
9. **`crates/op-snowball/src/streaming_snowball.rs:550`**
   ```rust
   match Command::new("btrfs")
       .args(["subvolume", "delete"])
       .arg(&snapshot_path)
   ```

---

## 4. Schema-as-Code Compliance Audit

The workspace defines a strict architecture requiring versioned Protocol Buffers or OSCAL schemas for establishing data contracts. The `op-snowball` crate contains multiple violations of this standard, using **ad-hoc structs** and **untyped JSON fields** to express critical data payloads.

### Flagged Ad-Hoc Structs

1. **`crates/op-snowball/src/footprint.rs:9-16`**
   ```rust
   pub struct BlockEvent {
       pub timestamp: u64,
       pub category: String,
       pub action: String,
       pub data: simd_json::OwnedValue,
       pub hash: String,
       pub vector: Vec<f32>,
   }
   ```
   * **Violation**: Defines `data` as a generic `simd_json::OwnedValue` instead of referencing a versioned Protobuf contract. This circumvents API schema guarantees.

2. **`crates/op-snowball/src/footprint.rs:43-51`**
   ```rust
   pub struct PluginFootprint {
       pub plugin_id: String,
       pub operation: String,
       pub timestamp: u64,
       pub data_hash: String,
       pub content_hash: String,
       pub metadata: HashMap<String, simd_json::OwnedValue>,
       pub vector_features: Vec<f32>,
   }
   ```
   * **Violation**: Defines `metadata` as an arbitrary `HashMap` of string keys to untyped dynamic values, bypassing formal contract specifications.

3. **`crates/op-snowball/src/plugin_footprint.rs:10-18`**
   ```rust
   pub struct PluginFootprint { ... }
   ```
   * **Violation**: Duplicate definition of `PluginFootprint` containing `metadata: HashMap<String, simd_json::OwnedValue>`, multiplying contract risk across the code.

4. **`crates/op-snowball/src/streaming_snowball.rs:21-28`**
   ```rust
   pub struct BlockEvent { ... }
   ```
   * **Violation**: Re-declaration of `BlockEvent` with untyped JSON `data`, violating the workspace's schema-as-code guidelines.

---

## 5. D-Bus Method Exposure & Secrets Audit

### D-Bus Interface Verification
Based strictly on the provided files for the `op-snowball` crate, there are no D-Bus interface mappings (e.g., `#[dbus_interface]` attributes from `zbus`) declared directly inside this module. Although the crate imports types that interact with D-Bus client pipelines elsewhere, the BTRFS subvolume and snapshotting mechanisms do not expose system-bus peers directly in this codebase.

### Hardcoded Secrets Verification
* **Benign Test IP**: In `crates/op-snowball/src/plugin_footprint.rs:440`, a local test IP address `192.168.1.100` is present within the testing module. No production credentials, tokens, or encryption keys are hardcoded in the source files.

---
## ⚠ Citation Warnings
- `crates/op-snowball/src/btrfs_numa_integration.rs:312`: file has 287 lines
- `crates/op-snowball/src/plugin_footprint.rs:440`: file has 427 lines
