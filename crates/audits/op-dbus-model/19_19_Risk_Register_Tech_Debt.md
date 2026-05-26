# Production Security and Quality Audit: op-dbus-model

## Prioritised Risk Register

| Severity | Issue | Evidence | Recommendation |
| :--- | :--- | :--- | :--- |
| **High** | Ad-hoc Struct Data Contracts (Lack of Versioned Schemas) | `crates/op-dbus-model/src/models.rs:5`<br>`crates/op-dbus-model/src/models.rs:13`<br>`crates/op-dbus-model/src/models.rs:35` | Define versioned data schemas using Protocol Buffers or OSCAL JSON schemas. Generate the Rust data structures from these schemas to prevent drift. |
| **Medium** | Unvalidated String Fields in Plugin Catalog Metadata | `crates/op-dbus-model/src/models.rs:39`<br>`crates/op-dbus-model/src/models.rs:41`<br>`crates/op-dbus-model/src/models.rs:43` | Validate path and identifier strings during deserialization. Use strongly typed path/D-Bus name wrappers instead of raw `String` objects. |
| **Medium** | Database Schema Defined via Ad-hoc Inline SQL Strings | `crates/op-dbus-model/src/lib.rs:8` | Move schema generation to structured, versioned database migration files using SQLx's built-in migration utility. |
| **Low** | Bypass of Structured Logging in Library via `eprintln!` | `crates/op-dbus-model/src/lib.rs:84` | Replace direct stderr printing with structured `tracing::warn!` or `tracing::error!` logs for seamless system integration. |
| **Low** | Confusing Field Mapping Naming Drift Between Rust Model and DB | `crates/op-dbus-model/src/lib.rs:49`<br>`crates/op-dbus-model/src/lib.rs:58` | Rename database column from `base_object` to `catalog_document` to clearly indicate it stores serialized `PluginCatalogDocument`. |

---

## Detailed Findings & Recommendations

### 1. Ad-hoc Struct Data Contracts (Lack of Versioned Schemas)
* **Severity**: High (Schema-as-Code Violation)
* **Evidence**:
  * `crates/op-dbus-model/src/models.rs:5` (`pub struct Plugin`)
  * `crates/op-dbus-model/src/models.rs:13` (`pub struct Schema`)
  * `crates/op-dbus-model/src/models.rs:35` (`pub struct PluginCatalogDocument`)
* **Impact**: 
  The data contracts representing critical control-plane entities are defined using ad-hoc Rust structs with generic dynamic JSON fields (`simd_json::OwnedValue`). This directly violates the project's schema-as-code and OSCAL compliance disciplines. Without versioned schemas (e.g., Protocol Buffers, JSON Schema, or OSCAL profiles), downstream projection layers (D-Bus, gRPC, and rendering engines) are highly susceptible to silent breaking changes, serialization mismatches, and protocol desynchronization.
* **Recommendation**: 
  Define standard schemas inside a single source of truth (such as a versioned Protobuf file or an OSCAL schema). Generate the Rust structs automatically using a build script to guarantee contract stability.

---

### 2. Unvalidated String Fields in Plugin Catalog Metadata
* **Severity**: Medium
* **Evidence**: 
  * `crates/op-dbus-model/src/models.rs:39` (`pub dbus_path: String`)
  * `crates/op-dbus-model/src/models.rs:41` (`pub service_name: String`)
  * `crates/op-dbus-model/src/models.rs:43` (`pub storage_path: String`)
* **Impact**: 
  Metadata strings like `storage_path` and `dbus_path` are ingested as raw, unvalidated `String` values. Malicious or malformed inputs (such as directory traversal patterns `../../` in `storage_path` or invalid characters/format strings in `dbus_path`) can lead to file system disclosure, path injection, or D-Bus controller crashes when processed by down-stream system modules.
* **Recommendation**: 
  Introduce validation logic using the `newtype` pattern or apply structural parsing. Validate path correctness using `std::path::Path` limits and match `dbus_path` against `zbus::names` specifications during deserialization.

---

### 3. Database Schema Defined via Ad-hoc Inline SQL Strings
* **Severity**: Medium
* **Evidence**: 
  * `crates/op-dbus-model/src/lib.rs:8` (`pub async fn create_schema`)
* **Impact**: 
  The SQLite database schema is defined as inline, hardcoded raw SQL strings inside the `create_schema` function. This circumvents normal database migration practices. Tracking structural alterations, performing safe rollbacks, and running automated integration tests against precise schema states becomes difficult as the control plane schema scales.
* **Recommendation**: 
  Migrate schema creation to external, versioned `.sql` files and manage them using SQLx’s structured database migration framework.

---

### 4. Bypass of Structured Logging in Library via `eprintln!`
* **Severity**: Low
* **Evidence**: 
  * `crates/op-dbus-model/src/lib.rs:84` (`eprintln!("Skipping stale plugin catalog document...")`)
* **Impact**: 
  When a stale document cannot be parsed during list operations, the error is written directly to standard error using `eprintln!`. Library crates executing in multi-threaded environments must avoid raw standard stream writes, as they bypass active logging frameworks (such as `tracing` or `log`), fail to output in structured formats (e.g., JSON) required by production log shippers, and introduce standard error synchronization overhead.
* **Recommendation**: 
  Leverage the workspace-wide logging standard by replacing `eprintln!` with the structured `tracing::warn!` macro.

---

### 5. Confusing Field Mapping Naming Drift Between Rust Model and DB
* **Severity**: Low
* **Evidence**: 
  * `crates/op-dbus-model/src/lib.rs:49` (`pub async fn upsert_document`)
  * `crates/op-dbus-model/src/lib.rs:58` (`.bind(encoded)`)
* **Impact**: 
  In `upsert_document`, the fully serialized `PluginCatalogDocument` is bound directly into the database column named `base_object`. However, the domain model `Plugin` (`crates/op-dbus-model/src/models.rs:5`) lists `base_object` as a single `simd_json::OwnedValue` representational block, while `PluginCatalogDocument` contains supplementary structures like `schema`, `dbus_path`, and `storage_path`. This semantic drift and naming overlap create technical debt, making subsequent query refactorings and code maintenance error-prone.
* **Recommendation**: 
  Align naming conventions by renaming the database column from `base_object` to `catalog_document` to clearly indicate it persists a serialized `PluginCatalogDocument`.