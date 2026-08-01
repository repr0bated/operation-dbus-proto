# Schema-as-Code & OSCAL Compliance Security Audit

## 1. Schema-as-Code Table

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `Plugin` | Struct | `crates/op-dbus-model/src/models.rs:6` | No | Defined as an ad-hoc Rust struct with no corresponding versioned Protocol Buffer schema. |
| `base_object` | Field (`simd_json::OwnedValue`) | `crates/op-dbus-model/src/models.rs:9` | No | Use of untyped JSON (`simd_json::OwnedValue`) violates the schema-as-code discipline. |
| `Schema` | Struct | `crates/op-dbus-model/src/models.rs:13` | No | Defined as an ad-hoc Rust struct with no corresponding versioned Protocol Buffer schema. |
| `definition` | Field (`simd_json::OwnedValue`) | `crates/op-dbus-model/src/models.rs:17` | No | Use of untyped JSON (`simd_json::OwnedValue`) for schema definitions lacks typed schema-as-code validations. |
| `PluginCatalogDocument` | Struct | `crates/op-dbus-model/src/models.rs:29` | No | Ad-hoc serialization target for catalog persistence, bypassing schema-defined contracts. |
| `plugins` Database Table | SQL DDL String | `crates/op-dbus-model/src/lib.rs:8` | No | Hand-rolled, inline SQL DDL representation of schema contracts instead of versioned migration schemas. |
| `schemas` Database Table | SQL DDL String | `crates/op-dbus-model/src/lib.rs:18` | No | Inline database schema creation bypasses structured, versioned schema management. |

---

## 2. OSCAL Coverage Table

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **System and Communications Protection** (NIST SP 800-53 SC-28: Protection of Information at Rest) | `crates/op-dbus-model/src/lib.rs:47` | None | Storing plain JSON `PluginCatalogDocument` serialized files containing sensitive system footings (`storage_path`, `dbus_path`) without encryption or cryptographic integrity checking, lacking OSCAL mapping. |
| **Configuration Management** (NIST SP 800-53 CM-8: Information System Component Inventory) | `crates/op-dbus-model/src/models.rs:29` | None | Storing component inventory data dynamically (`PluginCatalogDocument`) without alignment, sync, or export capabilities matching an OSCAL Component Definition. |

---

## 3. Recommendations

### Recommendation 1: Transition Data Contracts to Versioned Protobufs
* **File/Line:** `crates/op-dbus-model/src/models.rs:6-43`
* **Details:** Define `Plugin`, `Schema`, and `PluginCatalogDocument` structures within a `.proto` schema (e.g., `op/dbus/v1/model.proto`). Use `prost` or `tonic` to generate Rust structures instead of defining ad-hoc structs.
* **Remediation:** Replace untyped JSON fields (`simd_json::OwnedValue`) with strongly-typed Protobuf structures or `google.protobuf.Struct` messages if dynamic properties are structurally required, and utilize validation frameworks such as `protovalidate` for security assertions.

### Recommendation 2: Externalize Database Schema Operations to Versioned Migrations
* **File/Line:** `crates/op-dbus-model/src/lib.rs:7-31`
* **Details:** Remove inline SQL DDL query strings inside the Rust codebase. 
* **Remediation:** Establish a dedicated SQL migrations folder utilizing versioned SQL migration scripts (e.g., `migrations/0001_initial.sql`). Use the `sqlx::migrate!` macro during program initialization or deployment steps to apply database updates.

### Recommendation 3: Implement Cryptographic Security Policies for Inventory at Rest
* **File/Line:** `crates/op-dbus-model/src/lib.rs:46-64`
* **Details:** Store serialized catalog documents using cryptographically sound methods if they contain sensitive path specifications or system footprint mappings.
* **Remediation:** Incorporate authenticated encryption (e.g., AES-GCM) or verify cryptographic signatures of plugin catalog documents before inserting them into SQLite, aligning the code controls with OSCAL SC-28. Ensure an OSCAL Component Definition is generated to document this security control mapping for FedRAMP/NIST compliance.