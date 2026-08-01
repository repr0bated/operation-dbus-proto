### Workspace Crates Depending on `op-dbus-model`

Based on the workspace configuration and dependency resolutions recorded in `Cargo.lock`, the following crates depend on `op-dbus-model`:
* **`op-dbus`** (root/control plane service)
* **`op-plugins`** (`crates/op-plugins`)

---

### Registered D-Bus Service Names and Object Paths

No static D-Bus service names or object paths are hardcoded or registered inside `op-dbus-model`. Instead, they are dynamically persisted and modeled:
* **Service Names**: Persisted dynamically via the `service_name` column in the `plugins` database table (`crates/op-dbus-model/src/lib.rs:11`) and modeled inside `PluginCatalogDocument::service_name` (`crates/op-dbus-model/src/models.rs:44`).
* **Object Paths**: Persisted and projected dynamically using the `dbus_path` string field inside the catalog document (`crates/op-dbus-model/src/models.rs:42`).

---

### Exposed HTTP/gRPC Endpoints

The `op-dbus-model` crate acts strictly as a library modeling data structures and SQLite-backed storage persistence (`crates/op-dbus-model/src/lib.rs:40-44`). 
* **Endpoints**: No HTTP or gRPC endpoints are exposed or defined within this crate.

---

### Cross-Crate Circular Dependency Risk

* **Current State**: There is no direct circular dependency. 
  * `op-dbus-model` depends on `op-core` and `op-state-store` (`crates/op-dbus-model/Cargo.toml:14-15`).
  * Neither `op-core` nor `op-state-store` lists `op-dbus-model` as a dependency in `Cargo.lock`.
* **Identified Risk**:
  * `PluginCatalogDocument` (`crates/op-dbus-model/src/models.rs:36`) binds directly to `PluginSchema` imported from `op-state-store` (`crates/op-dbus-model/src/models.rs:39`).
  * If `op-state-store` or `op-core` ever needs to query, mirror, or introspect using `SqlitePluginCatalog` or `PluginCatalogDocument`, a circular dependency cycle (`op-state-store` $\rightarrow$ `op-dbus-model` $\rightarrow$ `op-state-store`) will be introduced.
  * **Mitigation**: Ensure database-backed catalog structures (`SqlitePluginCatalog`) remain decoupled from core state representations. High-level runtime systems must map low-level schemas to catalog documents outside the state storage engine.

---

### Schema-As-Code & Quality Findings

#### 1. Ad-Hoc SQL Schema Representation Instead of Versioned Schemas
* **File & Line**: `crates/op-dbus-model/src/lib.rs:7-35`
* **Severity**: Medium
* **Description**: Under the schema-as-code discipline, database contracts must be defined through versioned, declarative migration schemas. Defining database structures via ad-hoc SQL strings (`CREATE TABLE IF NOT EXISTS...`) inside an inline Rust helper function limits system upgrade tracking, migration rollbacks, and schema evolution.

#### 2. Severe Semantic Mismatch in the Persistence Contract
* **File & Line**: `crates/op-dbus-model/src/lib.rs:57-70` vs `crates/op-dbus-model/src/models.rs:5-11`
* **Severity**: High (Architectural Drift)
* **Description**: 
  * In `upsert_document` (`crates/op-dbus-model/src/lib.rs:57`), the entire `PluginCatalogDocument` is serialized to a JSON string and bound to the `base_object` database column.
  * However, `models.rs` defines a separate struct `Plugin` (`crates/op-dbus-model/src/models.rs:5-11`) which represents the actual database row where the `base_object` field is supposed to store a `simd_json::OwnedValue` (representing only the raw, inner payload of the plugin), not the entire schema-wrapped metadata document.
  * During retrieval in `get_document` (`crates/op-dbus-model/src/lib.rs:73`), the code deserializes the `base_object` column directly into `PluginCatalogDocument`.
  * **Impact**: This structural overloading of the `base_object` column breaks consistency. If another service queries the database expecting the `base_object` column to represent the `Plugin` struct's payload, it will suffer deserialization failures or parse incorrect schema wrappers. Use a distinct column name (e.g., `catalog_payload`) or split the table representations.