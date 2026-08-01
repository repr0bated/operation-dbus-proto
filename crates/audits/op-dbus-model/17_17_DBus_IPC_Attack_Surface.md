# D-Bus & IPC Attack Surface Audit

## 1. D-Bus & IPC Attack Surface Analysis

Within the provided files in the `crates/op-dbus-model` crate, there are no active D-Bus interfaces, methods, or signals registered. The crate is designed strictly to handle model and persistence layers representing metadata for downstream D-Bus projections. 

However, structural placeholders for D-Bus projections are defined in the catalog layer:
* **D-Bus Path Model Reference**: `crates/op-dbus-model/src/models.rs:40` (`pub dbus_path: String`)
* **D-Bus Service Name Reference**: `crates/op-dbus-model/src/models.rs:42` (`pub service_name: String`)

### Connection Bus & Identity Verification
Because the actual D-Bus service registration code (using `zbus`) is outside the scope of the provided source files, the following properties cannot be audited or verified:
* Whether the service connects to the system bus or the session bus.
* Whether caller credentials (such as UID, GID, or SELinux context via `zbus::MessageHeader`) are verified before method execution.
* The enforcement of any system bus security policy configuration files.

---

## 2. Schema-as-Code Compliance & Violations

The codebase does not strictly follow a schema-as-code discipline using Protocol Buffers or versioned OSCAL schemas. Instead, data contracts are expressed as ad-hoc Rust structs, database rows, and dynamically typed JSON payloads.

### Violation 1: Ad-hoc Plugin Catalog Contract
* **File & Line**: `crates/op-dbus-model/src/models.rs:35`
* **Detail**: `PluginCatalogDocument` serves as the primary contract for external projections and compatibility layers. However, instead of being generated from a versioned schema definition (such as Protobuf or OSCAL Component Definitions), it is defined as an ad-hoc Rust struct with direct Serde serialization annotations.

### Violation 2: Untyped Schemas using Dynamically-Parsed JSON
* **File & Line**: `crates/op-dbus-model/src/models.rs:8` and `crates/op-dbus-model/src/models.rs:17`
* **Detail**: Both `Plugin::base_object` and `Schema::definition` use `simd_json::OwnedValue` to store data schemas. This relies on arbitrary, dynamically-typed JSON structures rather than explicit, strongly-typed, or versioned schemas. Any consumer must perform run-time structural inspection rather than relying on compile-time or schema-enforced constraints.

### Violation 3: Ad-hoc Embedded Database Schema
* **File & Line**: `crates/op-dbus-model/src/lib.rs:9`
* **Detail**: The SQL database schema is defined as ad-hoc raw strings inside the `create_schema` function. This approach bypasses structured, versioned migration schemas, increasing the risk of uncoordinated database schema drift across deployment environments.

---

## 3. Security & Code Quality Findings

### Finding 1: Unvalidated Deserialization of Database Payloads (Medium)
* **File & Line**: `crates/op-dbus-model/src/lib.rs:76` and `crates/op-dbus-model/src/lib.rs:88`
* **Type**: Resource Exhaustion / Denial of Service
* **Detail**: In `get_document` and `list_documents`, data retrieved from the SQLite database (`base_object` column) is directly deserialized into `PluginCatalogDocument` using `serde_json::from_str`. While parameterized queries are used correctly to prevent SQL injection, there is no structural validation, depth-limiting, or size checking performed on the retrieved JSON string before deserialization. If a malicious or corrupted payload is introduced into the database, parsing highly nested or massive JSON elements could trigger severe resource exhaustion or panics, leading to denial of service.

### Finding 2: Inconsistent Console Logging on Deserialization Failures (Low)
* **File & Line**: `crates/op-dbus-model/src/lib.rs:91`
* **Type**: Observability & Error Handling Defect
* **Detail**: When listing documents, if a database row contains corrupted or drifted JSON structures, the error is written directly to standard error (`eprintln!`) and the entry is silently skipped. In production control planes, skipping registry entries silently can lead to inconsistent state where registered plugins fail to project without raising proper alerts to supervisory systems. Errors of this nature should be surfaced through structured error propagation or formal logging facades.