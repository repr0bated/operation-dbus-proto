# Production Security and Quality Audit: `op-dbus-model`

## 1. Public API Surface Analysis

### Public Items Enumeration
The `op-dbus-model` crate exposes the following public items:

*   **Modules**:
    *   `pub mod models;` (`crates/op-dbus-model/src/lib.rs:1`)
*   **Re-exports**:
    *   `pub use models::Plugin;` (`crates/op-dbus-model/src/lib.rs:7`)
    *   `pub use models::PluginCatalogDocument as CatalogDocument;` (`crates/op-dbus-model/src/lib.rs:7`)
    *   `pub use models::Schema;` (`crates/op-dbus-model/src/lib.rs:7`)
*   **Structs**:
    *   `pub struct SqlitePluginCatalog;` (`crates/op-dbus-model/src/lib.rs:42`)
    *   `pub struct Plugin;` (`crates/op-dbus-model/src/models.rs:5`)
    *   `pub struct Schema;` (`crates/op-dbus-model/src/models.rs:13`)
    *   `pub struct PluginCatalogDocument;` (`crates/op-dbus-model/src/models.rs:33`)
*   **Functions & Methods**:
    *   `pub async fn create_schema(pool: &SqlitePool) -> Result<()>;` (`crates/op-dbus-model/src/lib.rs:9`)
    *   `pub fn new(pool: SqlitePool) -> Self;` (`crates/op-dbus-model/src/lib.rs:47`)
    *   `pub async fn upsert_document(&self, document: &PluginCatalogDocument) -> Result<()>;` (`crates/op-dbus-model/src/lib.rs:51`)
    *   `pub async fn get_document(&self, name: &str) -> Result<Option<PluginCatalogDocument>>;` (`crates/op-dbus-model/src/lib.rs:71`)
    *   `pub async fn list_documents(&self) -> Result<Vec<PluginCatalogDocument>>;` (`crates/op-dbus-model/src/lib.rs:86`)
*   **Type Aliases**:
    *   `pub type SqliteSchemaCatalog = SqlitePluginCatalog;` (`crates/op-dbus-model/src/lib.rs:117`)

### Totals
*   **Modules**: 1
*   **Re-exports**: 3
*   **Structs**: 4
*   **Functions / Methods**: 5
*   **Type Aliases**: 1
*   **Total Public Items**: 14

### Top 10 Most Impactful Public Items
1.  `SqlitePluginCatalog` (`crates/op-dbus-model/src/lib.rs:42`): Core storage client for persisting system configuration.
2.  `upsert_document` (`crates/op-dbus-model/src/lib.rs:51`): Main write entry point to persist catalog states.
3.  `get_document` (`crates/op-dbus-model/src/lib.rs:71`): Core read accessor for individual catalogs.
4.  `list_documents` (`crates/op-dbus-model/src/lib.rs:86`): Collects all entries for downstream D-Bus projections.
5.  `create_schema` (`crates/op-dbus-model/src/lib.rs:9`): Initializes schema layout on target SQLite databases.
6.  `PluginCatalogDocument` (`crates/op-dbus-model/src/models.rs:33`): The structural contract mapping dynamic plug-ins to control plane footings.
7.  `Plugin` (`crates/op-dbus-model/src/models.rs:5`): Internal/external representation of system plugins.
8.  `Schema` (`crates/op-dbus-model/src/models.rs:13`): Internal/external metadata structure tracking registered dynamic schemas.
9.  `SqliteSchemaCatalog` (`crates/op-dbus-model/src/lib.rs:117`): Type alias utilized globally to shield workspace refactoring drift.
10.  `new` (`crates/op-dbus-model/src/lib.rs:47`): Constructor for the persistence handle.

### Glob Re-exports Check
No glob re-exports (`pub use x::*`) exist in the provided source files. All exports are explicitly qualified (`crates/op-dbus-model/src/lib.rs:7`).

### Struct Public Fields Audit
All fields on the following domain structures are exposed as `pub`:
*   `Plugin` (`crates/op-dbus-model/src/models.rs:5`): `name`, `service_name`, `base_object`, and `created_at` are all public.
*   `Schema` (`crates/op-dbus-model/src/models.rs:13`): `id`, `plugin_name`, `definition`, `discovered_from`, `discovered_at`, and `created_at` are all public.
*   `PluginCatalogDocument` (`crates/op-dbus-model/src/models.rs:33`): `schema`, `dbus_path`, `service_name`, `storage_path`, and `source` are all public.

**Refactoring Recommendation**: Structs with purely public fields cannot guarantee inner structural invariants. Additionally, exposing backend-specific models such as `simd_json::OwnedValue` directly via public fields (`Plugin::base_object` and `Schema::definition`) binds consumers directly to third-party dependencies. These fields should be made private, using constructor builders and read-only getter methods instead to preserve encapsulation boundaries.

---

## 2. Dead Code & Dead Schema Analysis

*   No instances of `#[allow(dead_code)]` were identified within the provided codebase.
*   No unused imports are present within `crates/op-dbus-model/src/lib.rs` or `crates/op-dbus-model/src/models.rs`.

### Dead Code Table

| Item | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `Plugin` | `struct` | `crates/op-dbus-model/src/models.rs:5` | **Remove/Expose**: This struct is defined and re-exported, but never referenced anywhere within this crate's catalog logic. If it is only used by external crates, move it to an integration/projection model. |
| `Schema` | `struct` | `crates/op-dbus-model/src/models.rs:13` | **Remove/Expose**: This struct is defined and re-exported, but not used by any database operations in this crate. |
| `SqliteSchemaCatalog` | `type alias` | `crates/op-dbus-model/src/lib.rs:117` | **Expose**: Retained purely for workspace-level backward compatibility. Preserve unless a wider refactoring sweeps downstream crates. |

### Dead Database Schemas
The database schema creation routine (`create_schema` in `crates/op-dbus-model/src/lib.rs:9`) initializes the `schemas` table:
```rust
CREATE TABLE IF NOT EXISTS schemas (
    id TEXT PRIMARY KEY,
    plugin_name TEXT NOT NULL,
    definition TEXT NOT NULL,
    discovered_from TEXT,
    discovered_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (plugin_name) REFERENCES plugins(name)
)
```
However, **no code inside `SqlitePluginCatalog` ever reads, writes, deletes, or references the `schemas` table**. The catalog is purely used to query and store in the `plugins` table. This creates a significant "Dead Schema" hazard, inflating database structure footprint without active application code managing its state.

---

## 3. Schema-As-Code Flagging

The codebase claims to employ a strict schema-as-code discipline using Protocol Buffers and OSCAL, yet multiple components violate this structure by utilizing ad-hoc structs and unstructured, raw dynamic JSON.

1.  **Ad-Hoc JSON Models (`crates/op-dbus-model/src/models.rs:7`, `crates/op-dbus-model/src/models.rs:17`)**:
    The `Plugin` struct's `base_object` and `Schema` struct's `definition` fields bypass protocol contract rules by using `simd_json::OwnedValue`. This allows clients to insert completely unvalidated dynamic JSON structures. There are no versioned schemas, definitions, or protobuf-backed constraints protecting these fields.
2.  **Unstructured SQLite Text Columns (`crates/op-dbus-model/src/lib.rs:15`, `crates/op-dbus-model/src/lib.rs:24`)**:
    In the database DDL, `base_object` and `definition` are defined as `TEXT` columns. Raw string serialized payloads are stored here with zero check-constraints or JSON schemas enforced at the database level.
3.  **Ad-Hoc Catalog Struct (`crates/op-dbus-model/src/models.rs:33`)**:
    While `PluginCatalogDocument` includes a structured `PluginSchema` type, other key control metadata fields such as `dbus_path`, `service_name`, `storage_path`, and `source` are represented as arbitrary, unconstrained `String` fields. They should instead be expressed as strongly typed URI formats or bounded schemas conforming to the workspace's OSCAL standards.

---

## 4. Security & Quality Vulnerabilities

### [High] Architectural Serialization Drift & Type Poisoning
*   **Location**: `crates/op-dbus-model/src/lib.rs:51-82`
*   **Description**: In `upsert_document`, the logic serializes a `PluginCatalogDocument` to a JSON string and binds it to the `base_object` column of the `plugins` table:
    ```rust
    let encoded = serde_json::to_string(document)?;
    sqlx::query(
        r#"
        INSERT INTO plugins (name, service_name, base_object)
        VALUES (?, ?, ?)
        ON CONFLICT(name) DO UPDATE SET
            service_name = excluded.service_name,
            base_object = excluded.base_object
        "#,
    )
    .bind(document.schema.name.as_str())
    .bind(document.service_name.as_str())
    .bind(encoded) // Binds serialized PluginCatalogDocument JSON as 'base_object'
    ```
    However, the defined database schema (`crates/op-dbus-model/src/lib.rs:12`) specifies:
    ```rust
    CREATE TABLE IF NOT EXISTS plugins (
        name TEXT PRIMARY KEY,
        service_name TEXT NOT NULL,
        base_object TEXT NOT NULL, ...
    )
    ```
    And the corresponding `Plugin` struct representation in the application (`crates/op-dbus-model/src/models.rs:5`) is:
    ```rust
    pub struct Plugin {
        pub name: String,
        pub service_name: String,
        pub base_object: simd_json::OwnedValue,
        pub created_at: DateTime<Utc>,
    }
    ```
    This constitutes severe design and architectural drift. The `base_object` column in the database does **not** store a plugin's base object configuration; it stores a serialized representation of the *entire* `PluginCatalogDocument`. 
    
    If any downstream workspace queries the `plugins` table and maps it directly to the `Plugin` struct, the deserializer will swallow the parsed `PluginCatalogDocument` JSON directly into the dynamic `simd_json::OwnedValue`. This triggers structural confusion (Type Poisoning) where the domain model's fields fail to align with its underlying runtime values.

### [Medium] Discrepant Parser Configuration (Serde vs. Simd-JSON)
*   **Location**: `crates/op-dbus-model/src/lib.rs:51` vs `crates/op-dbus-model/src/models.rs:7`
*   **Description**: The write path in `SqlitePluginCatalog` leverages standard `serde_json` for serialization/deserialization:
    ```rust
    let encoded = serde_json::to_string(document)?;
    ```
    Yet the models themselves define dynamic fields using `simd_json::OwnedValue`:
    ```rust
    pub base_object: simd_json::OwnedValue,
    ```
    Mixing `serde_json` and `simd_json` in the same processing pipeline introduces parsing discrepancies (e.g., in numeric float precision, duplicate key handling, or UTF-8 validation strictness). These inconsistencies can cause edge-case validation bypasses or sudden deserialization errors on the write/read boundaries.

### [Low] Missing SQLite Indexing and Foreign Key Enforcements
*   **Location**: `crates/op-dbus-model/src/lib.rs:9-38`
*   **Description**: In the DDL migrations, the table schemas lack proper indexing or PRAGMA requirements:
    1.  SQLite does not enforce foreign keys by default unless explicit `PRAGMA foreign_keys = ON;` is called upon pool connection. This is missing from `create_schema`.
    2.  `schemas.plugin_name` has a foreign key constraint referencing `plugins(name)`, but it has no index. This will cause full-table scans on modifications of the parent `plugins` table.