# Production Quality & Security Audit: op-projection

---

### Memory Map Table

| Site | file:line | Type | Risk |
| :--- | :--- | :--- | :--- |
| `IdentitySledReader::read_sled_entity` | `crates/op-projection/src/sled_reader.rs:53` | Read-Only | **Data Race & TOCTOU**: Directly mapping and dereferencing raw pointers (`unsafe { &*ptr }`) from `/dev/shm` without memory barriers, atomic operations, or synchronization locks. Concurrently mutating the sled data in another process causes undefined behavior and raw data races. |

---

### Security Findings

#### Finding 1: Critical Data Exposure — Missing Redaction Engine Stub (CRITICAL)
* **File:Line**: `crates/op-projection/src/access_control.rs:113-119`
* **Impact**: Access control policies defining `redact_sensitive = true` (to clean secrets and PII) are completely bypassed. The redaction logic is a pass-through stub that cloned and returned the original raw unredacted data:
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
  This results in silent leakage of credentials, cryptographic keys, and PII to unauthorized requesters who are granted base read permission.

#### Finding 2: Unsynchronized Shared Memory Dereference & Concurrency Hazard (HIGH)
* **File:Line**: `crates/op-projection/src/sled_reader.rs:53-56`
* **Impact**: The Sled reader loads a raw memory-mapped pointer and immediately casts it to an unsynchronized Rust reference:
  ```rust
  let (ptr, _mmap) = read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
  let sled = unsafe { &*ptr };
  ```
  Because the backing memory map is located in `/dev/shm` (shared memory) and is subject to external process mutations (e.g. the identity control plane updating cryptographic keys), dereferencing it without volatile reads, atomic wrappers, or synchronized read-locks violates the Rust aliasing model and triggers compiler-optimization-induced data races and undefined behavior.

#### Finding 3: Dynamic Regular Expression Compilation inside Critical Hot Paths (HIGH)
* **File:Line**: `crates/op-projection/src/access_control.rs:53`, `crates/op-projection/src/access_control.rs:72`, `crates/op-projection/src/schema_engine.rs:410`
* **Impact**: Regular expressions are dynamically compiled from strings on *every* single projection access check and field validation:
  ```rust
  let re = Regex::new(&policy.resource_pattern)?; // access_control.rs
  let regex = Regex::new(pattern)?;               // schema_engine.rs
  ```
  Under high-frequency queries or event validation loops, recompiling identical patterns repeatedly degrades performance. Additionally, if an untrusted source can register a schema or policy, supplying a maliciously constructed regex (e.g., nesting quantifiers like `(a+)+$`) leads to **Catastrophic Backtracking / Regular Expression Denial of Service (ReDoS)**, locking up the entire projection engine threat pool.

#### Finding 4: SSE Client Count Telemetry Memory Leak (MEDIUM)
* **File:Line**: `crates/op-projection/src/json_stream.rs:198-204`
* **Impact**: The SSE endpoint increments `client_count` and `total_clients` during connection establishment, but lacks clean-up/decrement logic upon client disconnection. Over time, `client_count` will continuously climb and diverge from actual active connections, leaking telemetry state and corrupting server monitoring.

---

### Performance & Allocation Audit

#### Finding 5: Dynamic Vector Re-Allocations in Loop Structures
* **File:Line**: 
  * `crates/op-projection/src/dbus_reader.rs:53`
  * `crates/op-projection/src/procfs_reader.rs:70`
  * `crates/op-projection/src/procfs_reader.rs:172`
  * `crates/op-projection/src/procfs_reader.rs:190`
  * `crates/op-projection/src/plugin_reader.rs:238`
  * `crates/op-projection/src/schema_engine.rs:225`
* **Impact**: Vectors (`children`, `processes`, `fs_types`, `interfaces`, `entities`, `errors`) are created via `Vec::new()` or grown recursively without capacity pre-allocation inside loops or recursive JSON traversals. This triggers repeated allocation resize sequences, causing severe heap fragmentation under high state density.
* **Remediation**: Use `Vec::with_capacity` or estimate sizing before entering dynamic loops.

#### Finding 6: Intensive String Formatting on Critical Paths
* **File:Line**:
  * `crates/op-projection/src/projection_engine.rs:38`
  * `crates/op-projection/src/plugin_reader.rs:265`
  * `crates/op-projection/src/plugin_reader.rs:298`
  * `crates/op-projection/src/plugin_reader.rs:313`
  * `crates/op-projection/src/procfs_reader.rs:85`
  * `crates/op-projection/src/schema_engine.rs:231`
  * `crates/op-projection/src/schema_engine.rs:249`
* **Impact**: Heavy usage of `format!()` in hot operations (e.g. projecting IDs, assembling recursive JSON pointer paths, scanning the filesystem per PID, or building validation error paths) results in continuous allocation of short-lived strings.
* **Remediation**: Replace with statically allocated string segments, reuse existing buffers, or construct identifier formats lazily.

#### Finding 7: Heavy Deep-Cloning of `simd_json::OwnedValue` Payloads
* **File:Line**:
  * `crates/op-projection/src/access_control.rs:47`
  * `crates/op-projection/src/access_control.rs:118`
  * `crates/op-projection/src/plugin_reader.rs:208`
  * `crates/op-projection/src/plugin_reader.rs:241`
  * `crates/op-projection/src/plugin_reader.rs:279`
  * `crates/op-projection/src/projection_store.rs:37`
  * `crates/op-projection/src/projection_store.rs:53`, `61`, `70`, `77`, `90`
* **Impact**: Instead of utilizing zero-copy references or copy-on-write pointers (`Cow`), the engine frequently calls `.clone()` on projections and raw JSON values (`simd_json::OwnedValue`). This deep-clones entire JSON syntax trees inside stores, loops, and security controllers, negating the zero-copy design targets of the control plane.
* **Remediation**: Transition store reads to return reference guards (e.g., `Ref` or `Arc<Projection>`) instead of copying the underlying data tree.

---

### Schema-as-Code & Compliance Audit

#### Finding 8: Ad-Hoc Data Contracts and Inline JSON Formats (Compliance Deviation)
* **File:Line**:
  * `crates/op-projection/src/data_models.rs:434`
  * `crates/op-projection/src/schema_engine.rs:46`
  * `crates/op-projection/src/dbus_reader.rs:63-66`
  * `crates/op-projection/src/dbus_reader.rs:92`
  * `crates/op-projection/src/grpc_reader.rs:41`
  * `crates/op-projection/src/procfs_reader.rs:89`
  * `crates/op-projection/src/procfs_reader.rs:132`
  * `crates/op-projection/src/procfs_reader.rs:160`
  * `crates/op-projection/src/procfs_reader.rs:177`
  * `crates/op-projection/src/procfs_reader.rs:196`
  * `crates/op-projection/src/sled_reader.rs:60`
* **Details**: Data contracts and schemas are expressed as ad-hoc strings, inline structs, and `json!` macros instead of versioned Protocol Buffers or standardized OSCAL compliance formats. Specifically:
  * Audit logs (`AccessControlAudit`, `SchemaAuditEntry`) are structured as custom, unversioned Rust structs rather than standard, schema-compliant OSCAL Assessment Results/Log formats.
  * Reader outputs directly construct JSON payloads (e.g., `"total_kb"`, `"mutation_index"`) using unversioned inline macros, bypassing the central `PluginSchema` authority and allowing structural drift between source telemetry and the central projection engine.
* **Remediation**: Define all readers' output schema models in Protocol Buffers or versioned JSON schemas, and ensure auditing logs map directly to OSCAL schema structures.