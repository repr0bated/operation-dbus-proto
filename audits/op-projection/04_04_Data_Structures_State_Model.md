# Production Security and Quality Audit: `op-projection`

## 1. Data Structures & Memory Management Analysis

### 1.1 Synchronization and Smart Pointer Counts

The following table tracks the occurrences of concurrency primitives and smart pointers (`Arc`, `Rc`, `RefCell`, `RwLock`, `Mutex`, `OnceCell`) as well as `.clone()` allocations across all files provided in the `FILES` section.

| File | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell` | `.clone()` Calls |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `access_control.rs` | 2 | 0 | 0 | 2 | 0 | 0 | 4 |
| `data_models.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `dbus_reader.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 3 |
| `event_materializer.rs` | 1 | 0 | 0 | 0 | 1 | 0 | 4 |
| `grpc_reader.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `interfaces.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `json_stream.rs` | 5 | 0 | 0 | 0 | 0 | 0 | 8 |
| `lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `ovsdb_mirror.rs` | 1 | 0 | 0 | 0 | 1 | 0 | 0 |
| `plugin_reader.rs` | 2 | 0 | 0 | 0 | 0 | 0 | 14 |
| `procfs_reader.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 5 |
| `projection_engine.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 9 |
| `projection_store.rs` | 2 | 0 | 0 | 0 | 0 | 0 | 8 |
| `schema_engine.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 15 |
| `schema_validator.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `sled_reader.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `bin/projection_server.rs` | 1 | 0 | 0 | 0 | 1 | 0 | 2 |
| **TOTAL** | **13** | **0** | **0** | **2** | **3** | **0** | **74** |

### 1.2 Allocation Highlights & `.clone()` Limits
No single file exceeds the threshold of 20 `.clone()` calls. The highest number of clone operations occurs in `schema_engine.rs` (15 clones) and `plugin_reader.rs` (14 clones), which is acceptable given their role in performing deep-copy schema conversion and validating complex JSON payloads.

---

### 1.3 Large Structs (> 5 Public Fields)

The following public structs exceed the limit of 5 public fields. Having very large structs with many public fields increases coupling, degrades API boundaries, and can lead to inefficient stack-to-heap copies.

*   **`PluginSchema`** (`data_models.rs:19`)
    *   **7 public fields**: `name`, `version`, `fields`, `category`, `examples`, `secret_paths`, `pii_paths`
*   **`FieldSchema`** (`data_models.rs:39`)
    *   **7 public fields**: `name`, `field_type`, `required`, `description`, `constraints`, `example`, `read_only`
*   **`Projection`** (`data_models.rs:145`)
    *   **12 public fields**: `id`, `entity_type`, `entity_id`, `state`, `schema_version`, `data`, `validation_errors`, `quarantine_reason`, `degradation_reason`, `affected_dependencies`, `created_at`, `updated_at`
*   **`Link`** (`data_models.rs:321`)
    *   **6 public fields**: `source`, `target`, `relationship`, `latency_ms`, `bandwidth_mbps`, `reliability`
*   **`ProjectionEvent`** (`data_models.rs:389`)
    *   **7 public fields**: `id`, `event_type`, `entity_type`, `entity_id`, `timestamp`, `data`, `source`
*   **`AccessControlAudit`** (`data_models.rs:463`)
    *   **6 public fields**: `timestamp`, `requester_id`, `action`, `resource`, `allowed`, `reason`
*   **`SchemaAuditEntry`** (`schema_engine.rs:53`)
    *   **7 public fields**: `timestamp`, `actor`, `schema_name`, `change_type`, `reason`, `footprint`, `trace_id`

---

### 1.4 Globally Mutable State
No global mutable state (such as `static mut` or `lazy_static` blocks containing interior mutability) was identified within the provided source files. Concurrency is managed appropriately through the use of `parking_lot` locks and `DashMap` instances wrapper-wrapped inside `Arc`.

---

## 2. Schema-As-Code Discipline Violations

This codebase mandates a **Schema-as-Code** discipline, implying all data contracts should utilize strictly versioned Protocol Buffer schemas or formalized OSCAL schemas rather than ad-hoc JSON or raw string-mapped structs.

*   **Ad-hoc JSON Payload Construction**:
    *   `dbus_reader.rs:68-71` initializes a `RawEntity` with an ad-hoc JSON payload:
        ```rust
        data: json!({
            "service": service,
            "path": child_path,
        }).into()
        ```
    *   `dbus_reader.rs:100` uses a generic empty JSON structure:
        ```rust
        data: json!({ "properties": {} }).into()
        ```
    *   `grpc_reader.rs:44` defines empty JSON arrays:
        ```rust
        data: json!({ "methods": [] }).into()
        ```
    *   `procfs_reader.rs:141` directly converts system stats to ad-hoc JSON:
        ```rust
        data: json!({ "name": comm }).into()
        ```
    *   `procfs_reader.rs:172` and `procfs_reader.rs:194` define raw structures for memory and CPU output using string mapping rather than Protobuf definitions.
    *   `sled_reader.rs:49-53` outputs unstructured mutation indices and public keys directly using `simd_json::json!`.

*   **Impact**: These ad-hoc JSON blocks bypass formal schema-as-code compilation. If schema validation fails silently or downstream components expect specific Protobuf fields, it can result in deserialization panics, data corruption, or validation bypasses.

---

## 3. Production Security & Quality Findings

### CRITICAL: Security Redaction is a No-Op
*   **Location**: `access_control.rs:112-118`
*   **Impact**: Direct exposure of PII (Personally Identifiable Information) and system secrets.
*   **Description**:
    The system defines metadata properties like `secret_paths` and `pii_paths` within its `PluginSchema` (`data_models.rs:19`) to flag sensitive variables. However, the `redact_sensitive` implementation in the access controller is a complete mock:
    ```rust
    fn redact_sensitive(
        &self,
        data: &simd_json::OwnedValue,
        _requester: &Requester,
    ) -> simd_json::OwnedValue {
        // In production, use JSON paths from schema to redact
        data.clone()
    }
    ```
    When policies are evaluated at `access_control.rs:44-45` and `policy.redact_sensitive` is `true`, the unredacted payload is returned directly to the requester. This completely invalidates the privacy and security boundary, directly leaking credentials, cryptographic keys, or PII.

---

### CRITICAL: Memory Corruption via Unchecked Shared Memory Pointer Dereference
*   **Location**: `sled_reader.rs:58-60`
*   **Impact**: Segmentation faults, local denial-of-service, or potential local privilege escalation (LPE).
*   **Description**:
    The system reads the WG public key and footprint directly from the shared memory pointer `/dev/shm/op-identity` (mmap). However, there is zero validation of the pointer's size, structure bounds, magic headers, or alignment before casting and dereferencing:
    ```rust
    let (ptr, _mmap) =
        read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
    let sled = unsafe { &*ptr };
    ```
    If another unprivileged local process on the system corrupts, truncates, or modifies `/dev/shm/op-identity`, dereferencing this pointer will cause the projection server to crash instantly (segmentation fault) or allow malicious data structures to hijack control flow.

---

### HIGH: Unbounded Regex Compilation in Access Control Hot Paths
*   **Location**: `access_control.rs:42` and `access_control.rs:58`
*   **Impact**: Severe CPU exhaustion and Denial of Service (DoS).
*   **Description**:
    Within the main permission checking loops (`enforce_policy` and `validate_permissions`), the regex pattern representing the resource rule is compiled on *every single request* inside a loop over all policies:
    ```rust
    for policy in policies.iter() {
        let re = Regex::new(&policy.resource_pattern)?;
    ```
    Compiling a regex pattern is an expensive operation. Performing this compilation sequentially during policy enforcement completely compromises real-time processing guarantees. If an attacker can inject custom resource pattern strings via dynamic policy updates, they can trigger catastrophically slow regex compilation or backtracking to freeze the host control loop.

---

### HIGH: Potential Deadlock and Panic via Nested Sync-Over-Async Runtime Engine
*   **Location**: `plugin_reader.rs:294-308`
*   **Impact**: Thread starvation, system deadlocks, or runner panics.
*   **Description**:
    The plugin reader defines a helper `block_on` that uses `block_in_place` or spins up a nested current-thread runtime if a handle is not available:
    ```rust
    fn block_on<F, T>(&self, future: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
            Err(_) => {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    ...
    ```
    If `read_all()` is executed on a single-threaded Tokio runtime executor, calling `block_in_place` will cause an immediate runtime panic. Additionally, blocking the execution thread to wait for async database or plugin queries halts the executor thread, causing deadlocks if other async tasks depend on the same thread's progress.

---

### MEDIUM: Naive Ad-Hoc XML Parsing of D-Bus Introspection Data
*   **Location**: `dbus_reader.rs:40-52`
*   **Impact**: Spoofing of projected system nodes, validation bypass.
*   **Description**:
    Instead of using a secure, compliant XML parser, the D-Bus reader parses the introspection XML returned from remote services using naive line-by-line string matching:
    ```rust
    for line in xml.lines() {
        if line.contains("<node name=\"") {
            if let Some(name) = line
                .split("name=\"")
                .nth(1)
                .and_then(|s| s.split('\"').next())
    ```
    An attacker who can influence the introspection XML payload of a D-Bus endpoint can inject malicious formatting (such as nested nodes inside comments, multiline XML attributes, or unexpected whitespace) to inject mock nodes or hide existing ones from the system topology, bypassing downstream integrity controls.

---

### MEDIUM: Missing Heartbeat Stream Leak Protection
*   **Location**: `json_stream.rs:212-213`
*   **Impact**: Resource exhaustion, file descriptor leaks on connection termination.
*   **Description**:
    The SSE (Server-Sent Events) endpoint in `sse_handler` combines the live broadcast stream with an infinite keepalive repeating stream:
    ```rust
    let keepalive = stream::repeat_with(|| Ok(Event::default().comment("keepalive")))
        .throttle(std::time::Duration::from_secs(30));

    let combined = stream::select(initial.chain(live), keepalive);
    ```
    If a client disconnects unexpectedly, the nested `select` and `repeat_with` streams may fail to register the drop immediately if the underlying TCP socket remains half-open (due to missing keepalive write failures), slowly leaking memory and streaming handles over long operational periods.