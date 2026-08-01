# Production Security and Quality Audit: op-projection

## Executive Summary
This production security and quality audit targets the `op-projection` crate. The assessment identified one directly exploitable **Critical** vulnerability regarding sensitive data leakage, multiple **High** severity vulnerabilities covering memory safety, denial of service, resource exhaustion, and stubbed authorization gates, as well as a structural architecture gap regarding the project's **Schema-as-Code** discipline.

---

## Prioritised Findings

| Severity | Issue | Evidence (file:line) | Recommendation |
| :--- | :--- | :--- | :--- |
| **Critical** | **Stubbed Redaction Bypass (PII & Secret Leakage)** | `crates/op-projection/src/access_control.rs:113-118` | Replace the mock implementation of `redact_sensitive` with an active JSON path filtering engine. Read `secret_paths` and `pii_paths` from the `PluginSchema` and dynamically redact or mask matching JSON pointers within the `simd_json::OwnedValue` structure. |
| **High** | **Unvalidated Shared Memory Unsafe Pointer Dereference** | `crates/op-projection/src/sled_reader.rs:60-63` | Before casting and dereferencing the pointer, validate that the shared memory file descriptor size matches `std::mem::size_of::<IdentitySled>()`. Implement a magic bytes header and checksum verification within `IdentitySled` to guarantee structure alignment and integrity. |
| **High** | **Async Reactor Thread Starvation via Blocking Sync IO** | `crates/op-projection/src/bin/projection_server.rs:305-310` | Wrap all synchronous filesystem scans (`procfs_reader.read_all()`, `sled_reader.read_all()`) within a `tokio::task::spawn_blocking` closure to prevent blocking worker threads on the main tokio event loop. |
| **High** | **Unbounded Memory Leak in Projection State History** | `crates/op-projection/src/projection_store.rs:32-47` | Introduce a maximum historical version limit (e.g., ring-buffer strategy) or a time-to-live (TTL) eviction policy for `HistoricalProjection` elements in `self.history` to prevent infinite memory growth under continuous metrics/process updates. |
| **High** | **Schema-as-Code Violation: Ad-hoc Inline Schema Declarations** | `crates/op-projection/src/data_models.rs:16`<br>`crates/op-projection/src/bin/projection_server.rs:30-192` | Migrate `PluginSchema`, `FieldSchema`, and associated entities to versioned Protocol Buffers or standardized OSCAL models. Generate the Rust structures via compiler build-scripts (`prost`) rather than maintaining them as manually constructed ad-hoc structs inside application binaries. |
| **High** | **Hardcoded Stub in `is_accessible` Authorization Gate** | `crates/op-projection/src/access_control.rs:136-139` | Remove the hardcoded `true` stub. Wire `is_accessible` to the permission validation and policy evaluation loop, matching the requester's active permissions against resources. |
| **Medium** | **Repeated Regex Compilation Bottleneck / ReDoS Vector** | `crates/op-projection/src/access_control.rs:49`<br>`crates/op-projection/src/access_control.rs:69`<br>`crates/op-projection/src/schema_engine.rs:388` | Pre-compile and cache all `Regex` instances inside the `AccessPolicy` and `Constraint` structs during initialization or registration. Avoid re-compiling user-controlled regex strings on every policy validation check. |

---

## Detailed Audit Findings & Remediation

### 1. Stubbed Redaction Bypass (PII & Secret Leakage)
* **Severity**: Critical (Directly exploitable)
* **Vulnerability Description**:
The access controller defines an active policy flow where projections containing PII or secrets must be redacted:
```rust
if re.is_match(&projection.id) && policy.redact_sensitive {
    result.data = self.redact_sensitive(&result.data, requester);
}
```
However, `redact_sensitive` is merely a stub returning the cloned raw payload:
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
Because the implementation is stubbed, any policy designed to redact PII or secrets will silently fail to do so, transmitting completely unredacted payloads to unauthorized requesters.
* **Remediation**:
Implement the redaction logic by traversing `data` and checking if any key paths match the schema's `secret_paths` or `pii_paths` definitions.
```rust
fn redact_sensitive(&self, data: &Value, requester: &Requester) -> Value {
    let mut redacted = data.clone();
    // Assuming schema is available:
    for path in &schema.secret_paths {
        if let Some(target) = redacted.pointer_mut(path) {
            *target = Value::from("[REDACTED]");
        }
    }
    redacted
}
```

### 2. Unvalidated Shared Memory Unsafe Pointer Dereference
* **Severity**: High
* **Vulnerability Description**:
In `sled_reader.rs`, `IdentitySledReader` maps shared memory from `/dev/shm` and directly casts the raw pointer:
```rust
let (ptr, _mmap) =
    read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
let sled = unsafe { &*ptr };
```
No validation is performed on the pointer alignment, size, or structure metadata. If the mapped shared memory region is truncated, malformed, or written to concurrently by an unaligned payload, accessing fields such as `sled.hashed_footprint` will lead to undefined behavior, platform alignment panic, or direct segmentation faults.
* **Remediation**:
Introduce size checks and layout bounds checks prior to accessing the shared memory slice.
```rust
let shm_len = _mmap.len();
if shm_len < std::mem::size_of::<IdentitySled>() {
    return Err(anyhow::anyhow!("Shared memory block is truncated"));
}
// Ensure alignment is correct
if (ptr as usize) % std::mem::align_of::<IdentitySled>() != 0 {
    return Err(anyhow::anyhow!("Unaligned shared memory pointer"));
}
```

### 3. Async Reactor Thread Starvation via Blocking Sync IO
* **Severity**: High
* **Vulnerability Description**:
The main application loop in `projection_server.rs` is decorated with `#[tokio::main]`. Inside this async context, the code periodically invokes blocking, synchronous filesystem scanning operations:
```rust
loop {
    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    ...
    if let Ok(entities) = procfs_reader.read_all() { ... }
```
`procfs_reader.read_all()` calls synchronous directory reads on `/proc` and opens files synchronously (`fs::read_to_string` on each pid directory's `/comm`). Because these blocking operations run directly on the executor's worker thread, they block the cooperative scheduler thread, creating latency spikes, stalling other concurrent async tasks, and potentially triggering thread starvation.
* **Remediation**:
Isolate the blocking IO tasks by using `tokio::task::spawn_blocking`:
```rust
let procfs_entities = tokio::task::spawn_blocking(move || {
    procfs_reader.read_all()
}).await??;
```

### 4. Unbounded Memory Leak in Projection State History
* **Severity**: High
* **Vulnerability Description**:
Every time `ProjectionStore::upsert` updates an existing projection, it appends a clone of the old projection state to an in-memory `DashMap` history vector:
```rust
self.history
    .entry(id.clone())
    .or_insert_with(Vec::new)
    .push(historical);
```
There is no maximum size check, ring-buffer bounding, or TTL-based eviction on the historical array. For high-frequency projections (such as memory or CPU usage metrics updating every minute), the history list will grow indefinitely, resulting in a progressive and irreversible memory leak that will eventually cause the kernel's Out-Of-Memory (OOM) killer to terminate the server.
* **Remediation**:
Enforce a rolling window limit on the maximum size of the history trace:
```rust
let mut history = self.history.entry(id.clone()).or_insert_with(Vec::new);
history.push(historical);
if history.len() > MAX_HISTORY_ENTRIES {
    history.remove(0); // Evict the oldest entry
}
```

### 5. Schema-as-Code Gaps: Ad-hoc Inline Rust Schema definitions
* **Severity**: High
* **Vulnerability Description**:
The codebase does not maintain schemas as versioned, declarative code assets. In `projection_server.rs`, schemas for core resources are declared imperatively using ad-hoc, inline Rust struct builders:
```rust
let memory_schema = PluginSchema {
    name: "system.memory".to_string(),
    version: "1.0.0".to_string(),
    fields: vec![ ... ]
    ...
```
This violates the core **Schema-as-Code Authority** and **OSCAL Compliance** mandates. Since the data schemas are tightly compiled into the server executable, updating, auditing, or versioning schemas requires rewriting code and recompiling the application binary, rather than utilizing versioned Protocol Buffers or standardized OSCAL components.
* **Remediation**:
Migrate all schema declarations out of application code and into versioned `.proto` schemas or standardized OSCAL JSON artifacts. Load and validate these dynamic schemas during application startup:
```rust
// Load schemas from a secure directory of versioned schema files
let schema_json = std::fs::read_to_string("/etc/op-projection/schemas/system.memory.v1.json")?;
let schema: PluginSchema = serde_json::from_str(&schema_json)?;
schema_engine.register_schema(schema)?;
```