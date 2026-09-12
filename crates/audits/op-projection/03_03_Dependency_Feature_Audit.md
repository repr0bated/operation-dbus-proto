# 1. Executive Summary

This production security and quality audit evaluates the `op-projection` crate and its parent workspace configuration. The `op-projection` system functions as a schema-validated state transformation engine processing live control-plane data from sources such as D-Bus, gRPC, procfs, and shared-memory sleds.

While the system is architected for high-performance, real-time data streaming (targeting a 50ms processing latency guarantee), several critical security and design flaws were identified in the implementation:
1. **Critical Bypass of Security Access Controls**: The core data redaction and security validation functions are stubbed as inactive NO-OPs, exposing all sensitive data (including PII and secrets defined in schemas) directly to any requester.
2. **High-Risk Regex Denial of Service (ReDoS)**: Compiling un-cached regular expressions on every single permission check allows an attacker who defines custom schemas or policies to trigger CPU exhaustion.
3. **Data Race Undefined Behavior**: The zero-copy memory-mapped shared sled reader lacks memory synchronization primitives, leading to potential data races when accessing non-atomic structures.
4. **Ad-hoc Serialization and Schema Violations**: Despite claiming a "Schema-as-Code Authority" model, multiple data contracts are written as ad-hoc Rust structures without standardized, versioned serialization schemas (such as Protocol Buffers or JSON Schema specifications).

---

# 2. Dependencies & Feature Inventory

## 2.1 Direct Dependencies (from `crates/op-projection/Cargo.toml`)

| Crate Name | Workspace Version / Explicit Version | Explicitly Enabled Features | Default Features / Workspace Context | Risk / Maintenance Warnings |
| :--- | :--- | :--- | :--- | :--- |
| `op-core` | Path: `../op-core` | N/A | Local internal dependency | None |
| `op-state` | Path: `../op-state` | N/A | Local internal dependency | None |
| `op-state-store`| Path: `../op-state-store` | N/A | Local internal dependency | Uses SQLite backend under the hood |
| `op-plugins` | Path: `../op-plugins` | N/A | Local internal dependency | None |
| `op-dbus-mirror`| Path: `../op-dbus-mirror` | N/A | Local internal dependency | None |
| `op-grpc-bridge`| Path: `../op-grpc-bridge` | N/A | Local internal dependency | None |
| `op-snowball` | Path: `../op-snowball` | N/A | Local internal dependency | None |
| `op-identity` | Path: `../op-identity` | N/A | Local internal dependency | Interacts with raw pointer shared memory |
| `tokio` | Workspace | `"full"`, `"sync"` | Pulled via workspace `version = "1"` | **Unpinned** workspace version. Broad feature footprint. |
| `tokio-stream` | Workspace | None | Pulled via workspace `version = "0.1"` | **Unpinned** |
| `futures` | Workspace | None | Pulled via workspace `version = "0.3"` | **Unpinned** |
| `axum` | Workspace | `"ws"`, `"macros"`, `"tokio"` | Pulled via workspace `version = "0.7"` | **Unpinned** |
| `tower` | Workspace | None | Pulled via workspace `version = "0.4"` | **Unpinned** |
| `tower-http` | Workspace | `"cors"`, `"fs"`, `"trace"` | Pulled via workspace `version = "0.5"` | **Unpinned** |
| `serde` | Workspace | None | Pulled via workspace `version = "1"` | **Unpinned** |
| `serde_json` | Workspace | None | Pulled via workspace `version = "1"` | **Unpinned** |
| `simd-json` | Workspace | None | Pulled via workspace `version = "0.13"`, with `"serde"`, `"serde_impl"` | **Unpinned** |
| `regex` | Workspace | None | Pulled via workspace `version = "1"` | **Unpinned** |
| `hex` | Workspace | None | Pulled via workspace `version = "0.4"` | **Unpinned** |
| `tonic` | Workspace | None | Pulled via workspace `version = "0.12"`, with `"tls"`, `"tls-roots"` | **Unpinned** |
| `prost` | Workspace | None | Pulled via workspace `version = "0.13"` | **Unpinned** |
| `zbus` | Workspace | None | Pulled via workspace `version = "4.0"`, with `"tokio"` | **Unpinned** |
| `chrono` | Workspace | `"serde"` | Pulled via workspace `version = "0.4"` | **Unpinned** |
| `anyhow` | Workspace | None | Pulled via workspace `version = "1"` | **Unpinned** |
| `thiserror` | Workspace | None | Pulled via workspace `version = "1"` | **Unpinned** |
| `tracing` | Workspace | None | Pulled via workspace `version = "0.1"` | **Unpinned** |
| `tracing-subscriber`| Workspace | None | Pulled via workspace `version = "0.3"`, with `"env-filter"`, `"json"` | **Unpinned** |
| `dashmap` | `"5.0"` | None | Pure-Rust concurrent map | **Unpinned** minor version constraint (`5.0` allows up to `<6.0.0`) |
| `parking_lot` | `"0.12"` | None | Spinlock/Mutex implementations | **Unpinned** |
| `sha2` | `"0.10"` | None | Cryptographic SHA-256 | **Unpinned** |

## 2.2 Features Section of `crates/op-projection/Cargo.toml`
* **None defined** inside `crates/op-projection/Cargo.toml`. 
* Workspace `Cargo.toml` defines:
  ```toml
  [features]
  default = ["grpc"]
  grpc = []
  ```

## 2.3 Schema-as-Code Critical Gaps
* Although `prost` and `tonic` are imported as dependencies in `crates/op-projection/Cargo.toml`, they are completely unused inside this crate's codebase.
* No `prost-build` or `tonic-build` scripts exist in `crates/op-projection`.
* No `schemars`, `jsonschema`, `openapiv3`, or `oscal-rs` schemas are integrated.
* Instead, `crates/op-projection` utilizes an **ad-hoc, manually written validation parser** inside `crates/op-projection/src/schema_engine.rs` to validate `PluginSchema` instances. This represents a distinct schema-as-code discipline gap: security schemas and field schemas are represented as structural Rust structs that require custom code to match types and constraints manually, rather than deriving validation logic deterministically from compiled schemas.

---

# 3. Storage Backend Inventory

The following table documents all storage engines discovered within the in-scope codebase:

| Backend | Found at File:Line | Role (KV/Graph/Cache/Queue) | Notes / Architectural Alignment |
| :--- | :--- | :--- | :--- |
| **SqliteStore** | `crates/op-projection/src/plugin_reader.rs:87` | KV / State Cache | Fallback database storing serialized plug-in states when bootstrapping. Placed at `/var/lib/op-dbus/state.db`. |
| **DashMap (In-Memory)** | `crates/op-projection/src/projection_store.rs:21` | In-Memory KV Store | The authoritative storage engine representing the current state and historical timeline of all projections. |
| **IdentitySled (Shared Memory)** | `crates/op-projection/src/sled_reader.rs:68` | Mmapped Shared State (Zero-Copy) | Directly memory-maps standard platform configurations from `/dev/shm`. |

### Architectural Deviations:
* **Knowledge & Graph Storage Violation**: The architecture defines complex semantic associations (`AIContextProjection` inside `crates/op-projection/src/data_models.rs:232`) and topological hierarchies (`TopologyProjection` inside `crates/op-projection/src/data_models.rs:189`). However, instead of using `op-cozo-store` (CozoDB) or a unified graph database, these relationships are flattened and stored purely in the flat in-memory tables of `DashMap` inside `crates/op-projection/src/projection_store.rs:21`. This violates the standard architecture requiring relational-graph relationships to be processed via Datalog/CozoDB query engines.

---

# 4. Production Security & Quality Audit Findings

## 4.1 Critical Severity

### 1. Inactive Stubbed Access Control Redaction (Bypasses Data Security Entirely)
* **File Reference**: `crates/op-projection/src/access_control.rs:97-104` and `crates/op-projection/src/access_control.rs:117-120`
* **Directly Exploitable**: Yes.
* **Description**: 
  The methods `redact_sensitive` and `is_accessible` on `ProjectionAccessController` are hardcoded to skip any actual security evaluation or redaction logic:
  ```rust
  fn redact_sensitive(
      &self,
      data: &simd_json::OwnedValue,
      _requester: &Requester,
  ) -> simd_json::OwnedValue {
      // In production, use JSON paths from schema to redact
      data.clone()
  }

  fn is_accessible(&self, _data: &simd_json::OwnedValue, _requester: &Requester) -> bool {
      // Simplified check
      true
  }
  ```
  Even when a policy specifically mandates redaction (e.g., `policy.redact_sensitive` is `true` in `enforce_policy` at line 46), the engine falls back to `self.redact_sensitive(...)`, which immediately returns the unredacted payload. This leaks all PII paths and secrets defined in the `PluginSchema` definitions (`pii_paths` and `secret_paths` in `crates/op-projection/src/data_models.rs:27-28`) directly to any client.
* **Remediation**:
  Implement proper JSON pointer/path parsing using the paths stored in the active `PluginSchema` and recursively redact keys matching those paths prior to returning the data.

---

## 4.2 High Severity

### 2. High-Risk ReDoS & CPU Exhaustion via Regular Expression Re-compilation
* **File Reference**: `crates/op-projection/src/access_control.rs:44` and `crates/op-projection/src/access_control.rs:68`
* **Directly Exploitable**: Yes (under high volume / custom schema injection).
* **Description**:
  The permission engine compiles a new regular expression on *every single evaluation loop* for both policy enforcement and permission checks:
  ```rust
  let re = Regex::new(&policy.resource_pattern)?;
  if re.is_match(&projection.id) && policy.redact_sensitive {
  ```
  Compiling regular expressions is an expensive operation. If an attacker gains the ability to register customized policies or schemas containing highly complex pattern structures, they can trigger regular expression denial-of-service (ReDoS). Even under normal traffic patterns, compiling multiple patterns per request on a high-throughput event materializer loop completely invalidates the 50ms performance guarantee.
* **Remediation**:
  Compile `resource_pattern` once when the `AccessPolicy` is registered or updated, and store the compiled `Regex` directly within `AccessPolicy` or utilize a thread-safe `Lazy` cache.

### 3. Concurrent Memory Data Race on Shared-Memory Raw Pointer
* **File Reference**: `crates/op-projection/src/sled_reader.rs:68-75`
* **Directly Exploitable**: Yes (leads to undefined behavior or segmentation fault).
* **Description**:
  In `IdentitySledReader::read_sled_entity`, the code dereferences a raw memory pointer received from `/dev/shm` without any concurrency protection or memory fences:
  ```rust
  let (ptr, _mmap) =
      read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
  let sled = unsafe { &*ptr };

  let footprint = hex::encode(sled.hashed_footprint);
  let pubkey = hex::encode(sled.wireguard_pubkey);
  ```
  The values `hashed_footprint` and `wireguard_pubkey` are raw byte arrays. If the external control process mutations occur in the shared-memory segment concurrently while this reader is copying them out, a data race occurs on non-atomic types, leading to structural memory corruption, undefined behavior, or memory safety violations (SIGSEGV).
* **Remediation**:
  Use atomic operations or cross-process synchronization primitives (like POSIX semaphores or mutexes embedded in the shared memory) to guarantee exclusive access to the memory-mapped struct.

---

## 4.3 Medium Severity

### 4. Non-Cryptographic and Easily Forgivable Audit Footprints
* **File Reference**: `crates/op-projection/src/schema_engine.rs:92-100` and `crates/op-projection/src/schema_engine.rs:111`
* **Directly Exploitable**: Yes.
* **Description**:
  The system attempts to write immutable accountability audit trails (named "The Strike/Etch" in the documentation), but calculates them using a predictable concatenation:
  ```rust
  fn generate_footprint(&self, schema_name: &str, version: SchemaVersion) -> String {
      let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
      let data = format!("{}:{}:{}", schema_name, version, timestamp);
      format!("0x{:x}", sha2::Sha256::digest(data.as_bytes()))
  }
  ```
  Furthermore, the calling method hardcodes `version` to `0` for all schema audits:
  ```rust
  let footprint = self.generate_footprint(schema_name, 0);
  ```
  Because the footprint generation depends entirely on the public `schema_name`, a constant `0`, and a predictable `timestamp`, any user can forge a valid footprint hash. Additionally, the lack of any cryptographic signature or salt allows any actor who can alter the state database to rewrite audit history without detection.
* **Remediation**:
  Sign audit footprints cryptographically using a private key held by the secure environment, or lock the historical log entries into a trusted-platform TPM-backed audit store.

### 5. Ad-Hoc XML Parsing for Introspection Scans
* **File Reference**: `crates/op-projection/src/dbus_reader.rs:44-59`
* **Directly Exploitable**: No.
* **Description**:
  The `SystemDbusReader` parses XML schemas returned from D-Bus introspection by slicing lines of strings manually:
  ```rust
  // Very basic XML parsing for children
  // In production, use a proper XML parser
  let mut children = Vec::new();
  for line in xml.lines() {
      if line.contains("<node name=\"") {
          if let Some(name) = line
              .split("name=\"")
              .nth(1)
              .and_then(|s| s.split('\"').next())
  ```
  If a D-Bus service returns introspection XML containing nested tags inside comments, unconventional spacing, or complex XML namespace structures, this ad-hoc logic will parse nodes incorrectly.
* **Remediation**:
  Replace this ad-hoc parsing block with a formal, robust parsing crate like `quick-xml` (which is already present in the workspace dependencies).

---

## 4.4 Low Severity

### 6. Blocking Sync-over-Async in Plugin Reader
* **File Reference**: `crates/op-projection/src/plugin_reader.rs:427-441`
* **Directly Exploitable**: No.
* **Description**:
  The plugin reader enforces a synchronous `read_all` interface but relies on asynchronous tasks underneath, resulting in an unsafe sync-over-async block:
  ```rust
  fn block_on<F, T>(&self, future: F) -> Result<T>
  where
      F: Future<Output = Result<T>>,
  {
      match tokio::runtime::Handle::try_current() {
          Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
          Err(_) => {
              let runtime = tokio::runtime::Builder::new_current_thread()
                  .enable_all()
                  .build()
                  ...
  ```
  If this engine runs inside a single-threaded Tokio executor, `block_in_place` will immediately panic at runtime. If executed inside a pool with limited resources, it can cause thread starvation.
* **Remediation**:
  Convert `SourceReader` trait to be inherently `async`, letting async calls propagate natively through the projection system's async architecture.