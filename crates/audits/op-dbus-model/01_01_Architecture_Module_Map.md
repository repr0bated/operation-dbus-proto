## Role: Architecture & Module Map

### Overview
The `op-dbus-model` crate acts as a persistence definition and access layer for the control plane's plugin catalog. It manages an SQLite-backed storage model for registered system plugins and their associated metadata schemas. This store is used as a mirror/projection layer to ensure that downstream D-Bus, gRPC, and rendering interfaces resolve a consistent shape of the underlying plugins.

### Module Tree
```
op-dbus-model (crates/op-dbus-model)
 ├── lib.rs (Crate root, defines SqlitePluginCatalog/SqliteSchemaCatalog and SQL DDL)
 └── models.rs (Defines ad-hoc Rust models: Plugin, Schema, and PluginCatalogDocument)
```

### Entry Points
* **Library Entry Point**: `crates/op-dbus-model/src/lib.rs` — exposes the database schema creation functions and the primary database engine wrapper `SqlitePluginCatalog`.

### Notes
* The crate implements an alias `SqliteSchemaCatalog` to maintain compatibility with legacy workspace crates.
* Data serialization uses a mix of `simd-json` (Workspace dependency) and standard `serde_json` / `serde` frameworks.

---

## Security & Quality Findings

### [High] Schema-as-Code Violation: Ad-hoc Struct Definitions for Data Contracts
* **Reference**: `crates/op-dbus-model/src/models.rs:5-46`
* **Vulnerability Type**: Schema Drift / Architecture Violation
* **Description**:
  The codebase violates the defined schema-as-code discipline. The data contracts for `Plugin`, `Schema`, and `PluginCatalogDocument` are declared as ad-hoc, manual Rust structs rather than being generated from a versioned, single-source-of-truth contract format (such as Protocol Buffers or OSCAL-compliant JSON/YAML component schemas). E.g.:
  ```rust
  pub struct Plugin {
      pub name: String,
      pub service_name: String,
      pub base_object: simd_json::OwnedValue,
      pub created_at: DateTime<Utc>,
  }
  ```
  Specifically, both `Plugin::base_object` (line 8) and `Schema::definition` (line 17) use untyped `simd_json::OwnedValue` fields, representing unstructured data boundaries. This completely bypasses schema validation, leaving downstream consumers vulnerable to breaking changes and parsing errors if external schemas shift.

* **Remediation**:
  Define these models using Protocol Buffers (`.proto`) and use `prost` to generate the Rust structures. If they must represent dynamic JSON objects, enforce a versioned JSON Schema validation mechanism using the workspace's `jsonschema` library before writing to or reading from the database.

---

### [High] Architectural Type & Database Column Misalignment
* **Reference**: `crates/op-dbus-model/src/lib.rs:52-69`, `crates/op-dbus-model/src/models.rs:5-11`, `crates/op-dbus-model/src/models.rs:32-46`
* **Vulnerability Type**: Code Quality / Logic Design Flaw
* **Description**:
  There is a severe structural mismatch between the database schema, the `Plugin` model, and how `SqlitePluginCatalog` operates on the database.
  1. The database table `plugins` (defined at `crates/op-dbus-model/src/lib.rs:8-15`) contains a column named `base_object TEXT NOT NULL`.
  2. The logical model `Plugin` in `crates/op-dbus-model/src/models.rs:5-11` represents a database row, where `base_object` is a `simd_json::OwnedValue` (ostensibly the parsed base object representation of the plugin itself).
  3. However, in `SqlitePluginCatalog::upsert_document` (at `crates/op-dbus-model/src/lib.rs:52-69`), the catalog takes a `PluginCatalogDocument` (which is a metadata wrapper containing `schema`, `dbus_path`, `service_name`, etc.), serializes the *entire wrapper document* to a JSON string, and stores it in the `base_object` column:
     ```rust
     let encoded = serde_json::to_string(document)?;
     sqlx::query(
         r#"
         INSERT INTO plugins (name, service_name, base_object)
         VALUES (?, ?, ?)
         ...
         "#,
     )
     .bind(document.schema.name.as_str())
     .bind(document.service_name.as_str())
     .bind(encoded) // Binds serialized PluginCatalogDocument to base_object!
     ```
  
  This violates the structural integrity of the schema. If another component queries the database directly expecting the `plugins` table to store a standard plugin representation matching `models::Plugin`, it will fail or receive corrupt/mismatched data. Conversely, reading a standard `Plugin` row and attempting to deserialize `base_object` as a pure `simd_json::OwnedValue` will succeed but return a completely different type structure than documented.

* **Remediation**:
  Refactor the database schema to match the logical structures. The `plugins` table should have explicit columns for all fields of the `PluginCatalogDocument` (such as `dbus_path`, `storage_path`, and `source`) rather than packing the entire structural envelope into an unrelated `base_object` text field.

---

### [Medium] Schema-as-Code Violation: Ad-hoc Raw SQL Schema Creation
* **Reference**: `crates/op-dbus-model/src/lib.rs:8-35`
* **Vulnerability Type**: Ad-hoc Schema Enforcement
* **Description**:
  The embedded database schema is created via inline, raw SQL DDL strings in `create_schema`. This bypasses versioned database migration files or declarative schema definition frameworks. Changes to the database tables require manual, error-prone edits to this raw string, increasing the risk of deployment schema drift and migration failures across production instances.
* **Remediation**:
  Migrate database schema definitions to SQLx migration files (e.g., using `sqlx::migrate!`), or derive the database schema programmatically from unified Protobuf definitions.

---

### [Medium] Potential Denial of Service via Unvalidated DB Deserialization
* **Reference**: `crates/op-dbus-model/src/lib.rs:77-83`
* **Vulnerability Type**: Unvalidated Input Deserialization
* **Description**:
  In `get_document`, the function reads `base_object` from the database and immediately deserializes it:
  ```rust
  let encoded: String = row.try_get("base_object")?;
  let document = serde_json::from_str(&encoded)?;
  Ok(Some(document))
  ```
  If the database file is tampered with, corrupted, or contains stale structures from an incomplete migration, any call to `get_document` will bubble up a `serde_json::Error` (converted to `anyhow::Error`). Because this error propagates up to core orchestrator layers, a single corrupted entry in the database can halt startup or block administrative queries, resulting in a denial-of-service state for the control plane.
* **Remediation**:
  Safely handle deserialization failures. Rather than allowing the entire query to fail and panic/abort, log the structural corruption event, isolate the failing entry, and optionally fall back to a safe default or return an explicit validation error type.