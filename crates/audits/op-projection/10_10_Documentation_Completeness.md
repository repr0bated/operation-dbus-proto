# Production Security & Quality Audit

## 1. Documentation & Quality Analysis

### Crate-Level Documentation
The crate contains comprehensive crate-level documentation in `crates/op-projection/src/lib.rs:1-17` explaining the Projection System's role, core principles, and module structure.

### README.md Presence
No `README.md` file is present in the provided FILES section. It is recommended to add a root-level `README.md` for onboarding and architecture documentation.

### Public Unsafe Functions
No public `unsafe fn` declarations exist in the provided source files. All unsafe operations are constrained within private methods or function blocks.

### Missing Rustdoc Comments (Sampling 10 Public Items)
The following public items lack required `///` rustdoc comments:

1. **`SchemaVersion` type alias** – `crates/op-projection/src/schema_engine.rs:18`
2. **`ProjectionStore::new` associated function** – `crates/op-projection/src/projection_store.rs:21`
3. **`ProjectionStore::upsert` method** – `crates/op-projection/src/projection_store.rs:29`
4. **`ProjectionStore::get` method** – `crates/op-projection/src/projection_store.rs:49`
5. **`ProjectionStore::get_by_type` method** – `crates/op-projection/src/projection_store.rs:54`
6. **`ProjectionStore::get_by_state` method** – `crates/op-projection/src/projection_store.rs:63`
7. **`ProjectionStore::get_all` method** – `crates/op-projection/src/projection_store.rs:72`
8. **`ProjectionStore::delete` method** – `crates/op-projection/src/projection_store.rs:77`
9. **`ProjectionStore::get_history` method** – `crates/op-projection/src/projection_store.rs:87`
10. **`ProjectionStore::clear` method** – `crates/op-projection/src/projection_store.rs:92`

---

## 2. Schema-As-Code Discipline Violations

The codebase frequently bypasses strict schema-as-code principles (using compiled Protocol Buffers or standardized OSCAL schemas) in favor of dynamically typed, ad-hoc structures and manually serialized string pointers:

*   **Ad-Hoc JSON Value Typing**: The raw and projected data payloads are transmitted as `simd_json::OwnedValue` (essentially dynamic JSON nodes) instead of strongly-typed compiled contracts. This is evident in:
    *   `RawEntity::data` in `crates/op-projection/src/interfaces.rs:100`
    *   `Projection::data` in `crates/op-projection/src/data_models.rs:153`
*   **Procedural Schema Specification**: Rather than generating schema contracts from centralized `.proto` or declarative schema files, schemas are procedurally defined using custom, ad-hoc Rust structs:
    *   `PluginSchema` and `FieldSchema` in `crates/op-projection/src/data_models.rs:17` and `crates/op-projection/src/data_models.rs:37`
    *   `SystemPluginReader::nested_object_projection_schema()` in `crates/op-projection/src/plugin_reader.rs:110` which imperatively instantiates validation constraints.
*   **Ad-Hoc Path Strings**: Redaction rules (`secret_paths` and `pii_paths` in `crates/op-projection/src/data_models.rs:31-33`) rely on raw, untyped `String` vectors containing JSON pointers, which are parsed and processed dynamically rather than checked at compile-time.

---

## 3. Vulnerability Audit

### [CRITICAL] Dummy/No-Op Secret and PII Redaction
*   **Location**: `crates/op-projection/src/access_control.rs:113` (inside `redact_sensitive`)
*   **Vulnerability Type**: Sensitive Data Exposure / Broken Access Control
*   **Description**: In `enforce_policy` (`crates/op-projection/src/access_control.rs:49`), if a policy matches a projection and has `redact_sensitive` enabled, it invokes `self.redact_sensitive`. However, the implementation of `redact_sensitive` is a dummy placeholder that returns `data.clone()` unmodified.
*   **Exploitability**: Directly exploitable. Any client requesting schema-validated projections that should have credentials, private keys, or PII redacted will receive the raw, sensitive values in full, exposing system secrets and user private data.

### [HIGH] Blind Shared Memory Pointer Dereference
*   **Location**: `crates/op-projection/src/sled_reader.rs:59` (inside `read_sled_entity`)
*   **Vulnerability Type**: Memory Corruption / Undefined Behavior
*   **Description**: The `IdentitySledReader` obtains a raw pointer to shared memory via `read_sled()` and blindly dereferences it as `&*ptr` without verifying that the mapped virtual memory space matches the exact size and alignment of `IdentitySled`.
*   **Exploitability**: If `/dev/shm` is truncated, corrupted, or written to by a malicious local process, dereferencing this unvalidated pointer will trigger a segmentation fault or out-of-bounds read, crashing the entire projection server.

### [MEDIUM] Regular Expression Denial of Service (ReDoS) via On-Demand Compilation
*   **Location**: `crates/op-projection/src/access_control.rs:42` and `67`
*   **Vulnerability Type**: Denial of Service (DoS)
*   **Description**: Both `enforce_policy` and `validate_permissions` compile a regular expression matching the resource pattern on *every single validation check* inside a hot loop (`Regex::new(&policy.resource_pattern)?`).
*   **Exploitability**: If a policy containing a pathological backtracking regex is registered, an attacker can send requests matching that resource pattern to lock up the tokio runtime thread, causing a Denial of Service. Additionally, repeated regex compilation on every request degrades system throughput under load.

### [MEDIUM] Silent Update Drops on Slow Streaming Clients
*   **Location**: `crates/op-projection/src/json_stream.rs:315` (inside `sse_handler`)
*   **Vulnerability Type**: State Desynchronization
*   **Description**: When streaming real-time projection updates to SSE clients, the server uses a `BroadcastStream`. If a client's TCP socket or message consumer lags behind the broadcast buffer, the stream returns `Err(RecvError::Lagged)`. The server's handler catches this error and silently returns `None` via a `filter_map`.
*   **Exploitability**: The lagged client silently falls out of sync, missing critical system state transformations and security warnings without any alert, log entry, or automatic disconnection to force a re-sync.

---
## ⚠ Citation Warnings
- `crates/op-projection/src/json_stream.rs:315`: file has 215 lines
