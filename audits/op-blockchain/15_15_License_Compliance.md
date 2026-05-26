# License Compliance Audit

## 1. Extracted License Field
The workspace package license is declared in the root `Cargo.toml` as:
* **License**: `Apache-2.0`
* **Inheritance**: The audited crate `op-blockchain` inherits this license in `crates/op-blockchain/Cargo.toml` via `license.workspace = true`.

## 2. GPL/AGPL/SSPL Crates and Incompatibility Flag
Scanning `Cargo.lock` and workspace dependencies:
* **Incompatible Crate**: `cozo` (version `0.7.6`)
* **License**: **AGPL-3.0** (GNU Affero General Public License v3.0)
* **Risk & Impact**: The `cozo` database is licensed under the AGPL-3.0. Linking against or embedding an AGPL-3.0 crate in the control plane (`op-dbus` / `op-blockchain`) subjects the combined work to viral copyleft provisions. If this software is distributed or exposed over a network, the entire source code of the calling application must be made available under the AGPL-3.0. This directly conflicts with the permissive `Apache-2.0` license declared at the workspace root.

## 3. Crates with No License Field
* `Cargo.lock` files do not natively store package licensing fields.
* Within the workspace members specified in the root `Cargo.toml`, we can only verify the license fields for `op-blockchain` and `op-dbus` as their `Cargo.toml` files were provided. Both properly declare their license.
* The license fields for other workspace members (e.g., `op-services`, `op-gateway`, `op-core`, `op-tools`, `op-introspection`, `op-chat`, `op-http`, `op-web`, `op-cache`, `op-state`, `op-state-store`, `op-jsonrpc`, `op-llm`, `op-network`, `op-inspector`, `op-agents`, `op-plugins`, `op-workflows`, `op-ml`, `op-deployment`, `op-mcp`, `op-mcp-aggregator`, `op-mcp-proxy`, `op-identity`, `op-execution-tracker`, `op-dynamic-loader`, `op-cognitive-mcp`, `op-cozo-store`, `op-dbus-model`, `op-grpc-bridge`, `op-dbus-mirror`, `op-compliance`, `op-projection`) **cannot be verified** because their `Cargo.toml` manifests were excluded from the provided files.

---

# Schema-As-Code Discipline Audit

The codebase violates the strict **schema-as-code** discipline by expressing primary data contracts as ad-hoc Rust structures and dynamically typed JSON objects rather than compiled Protocol Buffers or versioned OSCAL schemas.

### Violations:
1. **Ad-hoc `BlockEvent` Struct**:
   * `crates/op-blockchain/src/footprint.rs:10-17` and `crates/op-blockchain/src/streaming_blockchain.rs:25-32` define an ad-hoc Rust struct representing immutable blockchain block events.
   * The `data` field uses a dynamically typed `simd_json::OwnedValue` instead of a strongly typed, versioned schema contract.
2. **Ad-hoc `PluginFootprint` Struct**:
   * `crates/op-blockchain/src/footprint.rs:41-49` and `crates/op-blockchain/src/plugin_footprint.rs:10-18` declare a duplicate footprint structure.
   * The `metadata` field is defined as an ad-hoc `HashMap<String, simd_json::OwnedValue>` that bypasses structured validation.
3. **Ad-hoc State Storage Contracts**:
   * `crates/op-blockchain/src/blockchain.rs:194-208` reads and writes state keys to disk as arbitrary, unvalidated JSON documents via `simd_json::OwnedValue`. There is no schema validation or contract enforcement against an OSCAL profile or Protocol Buffer.

---

# Security & Quality Findings

## [Critical] OS Command Injection in BTRFS/SSH Replication Pipelines

### File Context:
* `crates/op-blockchain/src/blockchain.rs:241-268`
* `crates/op-blockchain/src/streaming_blockchain.rs:480-505`
* `crates/op-blockchain/src/streaming_blockchain.rs:509-548`

### Vulnerability Analysis:
The system invokes local shell interpreters (`sh` and `bash`) to execute commands for BTRFS subvolume streaming over SSH. These invocations format commands using raw string interpolation:

`crates/op-blockchain/src/blockchain.rs:252-257`:
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

`crates/op-blockchain/src/streaming_blockchain.rs:487-492`:
```rust
let output = Command::new("bash")
    .arg("-c")
    .arg(format!(
        "btrfs send {} | ssh {} 'btrfs receive /var/lib/blockchain/vectors/'",
        vector_snapshot.display(),
        remote
    ))
```

`crates/op-blockchain/src/streaming_blockchain.rs:524-528`:
```rust
let cmd = format!(
    "btrfs send {} | tee {} > /dev/null",
    vector_snapshot.display(),
    tee_args.join(" ")
);
```

### Exploit Scenario:
If an attacker is able to influence the `remote_path`, `remote`, or any entry in the `replicas` string slice (e.g., through a manipulated plugin metadata payload, dynamic configuration register, or compromised DBus interface), they can inject shell metacharacters. 
For instance, setting `remote` to:
`"localhost; rm -rf / #"`
will result in the local shell executing `rm -rf /` with the permissions of the running blockchain daemon.

### Remediation:
1. Avoid executing shell command strings via `sh -c` or `bash -c`.
2. Spawn child processes directly using `tokio::process::Command`, passing arguments as a safe list of arguments (`.arg()`), completely bypassing the shell parser.
3. For pipelines, use Rust standard library `Stdio::piped()` to connect the stdout of the `btrfs` child process to the stdin of the `ssh` process programmatically.

---

## [Medium] Path Traversal via Unsanitized State and Plugin Keys

### File Context:
* `crates/op-blockchain/src/blockchain.rs:194-208`
* `crates/op-blockchain/src/streaming_blockchain.rs:310-323`

### Vulnerability Analysis:
The `write_state`, `read_state`, and `update_plugin_state` routines construct target file paths by joining a BTRFS subvolume base path with a key parameter directly.

`crates/op-blockchain/src/blockchain.rs:195`:
```rust
let state_file = self.state_subvol.join(format!("{}.json", key));
```

`crates/op-blockchain/src/streaming_blockchain.rs:314-315`:
```rust
let plugin_file = plugins_dir.join(format!("{}.json", plugin_name));
let temp_file = plugins_dir.join(format!(".{}.json.tmp", plugin_name));
```

There is no sanitization to ensure that `key` or `plugin_name` do not contain directory traversal sequences (such as `../`).

### Exploit Scenario:
A compromised or untrusted plugin calling `update_plugin_state` with a `plugin_name` of `../../../../etc/shadow` could write arbitrary payload data to highly sensitive local files, resulting in local privilege escalation or complete system compromise.

### Remediation:
Implement strict path canonicalization or assert that the computed path is a direct descendant of the targeted base directory:
```rust
let safe_path = self.state_subvol.join(key);
if !safe_path.starts_with(&self.state_subvol) {
    anyhow::bail!("Path traversal attempt detected!");
}
```

---

## [Medium] Undefined Behavior / Out-Of-Bounds Read in `simd_json::from_str`

### File Context:
* `crates/op-blockchain/src/btrfs_numa_integration.rs:133`
* `crates/op-blockchain/src/blockchain.rs:207`
* `crates/op-blockchain/src/streaming_blockchain.rs:326`

### Vulnerability Analysis:
The code parses deserialized JSON files using `simd_json::from_str` wrapped in `unsafe`:
```rust
let mut data = tokio::fs::read_to_string(&block_file).await?;
let block_data: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut data)? };
```

According to `simd-json` safety requirements, the mutable string buffer passed to the SIMD parser *must* be padded with `simd_json::PADDING` bytes at the end. Standard `String` buffers allocated by `tokio::fs::read_to_string` are not guaranteed to contain this trailing padding. The SIMD processing unit will read beyond the allocated length of the buffer into uninitialized memory, triggering undefined behavior, memory access violations, or information disclosure.

### Remediation:
Convert the parsed input into a padded byte vector using `simd_json::to_vec` or use `simd_json::from_slice` on a `Vec<u8>` container that has been explicitly allocated with extra capacity to safely accommodate SIMD block reads.

---

## [Low] Silent Degradation of Security Invariants via FS Fallbacks

### File Context:
* `crates/op-blockchain/src/blockchain.rs:105-116`
* `crates/op-blockchain/src/blockchain.rs:165-174`

### Quality Analysis:
When BTRFS commands are unavailable (e.g. on non-BTRFS partitions), the streaming blockchain falls back to standard directory structures and recursive copies:
```rust
if stderr.contains("command not found") || stderr.contains("not a btrfs filesystem") {
    warn!(
        "BTRFS not available, creating regular directory: {:?}",
        path
    );
    tokio::fs::create_dir_all(path).await?;
}
```

This silent degradation undermines a core security guarantee of this architecture: **immutable audit logs** backed by read-only subvolume snapshots (`btrfs subvolume snapshot -r`). Under the standard directory fallback, any local process with user permissions can manipulate or overwrite the log history without detection.

### Remediation:
If BTRFS snapshot enforcement is an absolute architectural invariant for tamper-evidence, fail-fast and abort initialization if a non-BTRFS filesystem is detected, rather than continuing silently with standard mutable directories.