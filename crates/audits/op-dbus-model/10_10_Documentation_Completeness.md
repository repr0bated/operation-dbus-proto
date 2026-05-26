# Crate-Level Documentation Audit

## Crate-Level Docs Check
* **File**: `crates/op-dbus-model/src/lib.rs`
* **Status**: **Missing**.
* **Details**: There are no crate-level `//!` rustdocs at the top of `crates/op-dbus-model/src/lib.rs` to explain the high-level architecture, database layout, or usage patterns of this crate.

## README.md Presence
* **Status**: **Missing**.
* **Details**: No `README.md` file was found in the crate directory `crates/op-dbus-model/` or workspace root files provided.

## Sample of 10 Public Items & Rustdoc Status

Below is a systematic audit of 10 public items declared in `op-dbus-model`:

| # | Item Signature / Identifier | File & Line Citation | Rustdoc Status |
|---|---|---|---|
| 1 | `pub mod models;` | `crates/op-dbus-model/src/lib.rs:1` | **Missing** |
| 2 | `pub use models::{Plugin, PluginCatalogDocument as CatalogDocument, Schema};` | `crates/op-dbus-model/src/lib.rs:7` | **Missing** |
| 3 | `pub async fn create_schema(pool: &SqlitePool) -> Result<()>` | `crates/op-dbus-model/src/lib.rs:9` | **Missing** |
| 4 | `pub fn new(pool: SqlitePool) -> Self` | `crates/op-dbus-model/src/lib.rs:53` | **Missing** |
| 5 | `pub async fn upsert_document(&self, document: &PluginCatalogDocument) -> Result<()>` | `crates/op-dbus-model/src/lib.rs:57` | **Missing** |
| 6 | `pub async fn get_document(&self, name: &str) -> Result<Option<PluginCatalogDocument>>` | `crates/op-dbus-model/src/lib.rs:77` | **Missing** |
| 7 | `pub async fn list_documents(&self) -> Result<Vec<PluginCatalogDocument>>` | `crates/op-dbus-model/src/lib.rs:91` | **Missing** |
| 8 | `pub struct Plugin` | `crates/op-dbus-model/src/models.rs:5` | **Missing** |
| 9 | `pub struct Schema` | `crates/op-dbus-model/src/models.rs:13` | **Missing** |
| 10 | `pub struct PluginCatalogDocument` | `crates/op-dbus-model/src/models.rs:31` | Present |

*Only 1 of the 10 sampled public items has a doc comment.*

## Public Unsafe Functions
* **Status**: **Pass**.
* **Details**: There are no public `unsafe` functions exposed in `op-dbus-model`, so no safety invariant documents are missing.

---

# Schema-as-Code Compliance

This workspace enforces a strict Schema-as-Code discipline using Protocol Buffers and OSCAL versioned documents. The following structures violate this rule by utilizing ad-hoc string formatting, dynamic JSON types, or unversioned database definitions:

### 1. Ad-hoc JSON values in Plugin Model
* **File**: `crates/op-dbus-model/src/models.rs:5-10`
* **Details**: `pub struct Plugin` represents a primary data model but utilizes `simd_json::OwnedValue` for the `base_object` field. This allows untyped, schema-less JSON payloads to bypass version control, validation, and contract parsing.

### 2. Ad-hoc JSON values in Schema Model
* **File**: `crates/op-dbus-model/src/models.rs:13-20`
* **Details**: `pub struct Schema` maps the `definition` field to `simd_json::OwnedValue`. This is an ad-hoc schema definition structure that should be defined as a structured, versioned Protocol Buffer schema message rather than parsed JSON.

### 3. Unversioned Metadata Fields in Catalog Document
* **File**: `crates/op-dbus-model/src/models.rs:31-48`
* **Details**: `PluginCatalogDocument` defines several string-based operational fields, such as `dbus_path: String`, `service_name: String`, `storage_path: String`, and `source: String`. These ad-hoc fields represent infrastructure and system integration contracts that bypass version-controlled specifications.

### 4. Dynamic/Imperative SQLite Schema Initializer
* **File**: `crates/op-dbus-model/src/lib.rs:9-41`
* **Details**: The database schema is defined as inline raw string SQL blocks inside `create_schema`. These tables (`plugins` and `schemas`) should be generated and synchronized directly from OSCAL metadata profiles or Protobuf descriptors rather than maintained as ad-hoc, imperative string definitions.

### 5. String-based JSON Serialization for Database Boundaries
* **File**: `crates/op-dbus-model/src/lib.rs:57-90`
* **Details**: Serialization mechanisms rely on ad-hoc runtime string transformations (`serde_json::to_string` and `serde_json::from_str`). Data boundaries must be governed by serialized Protobuf buffers or versioned OSCAL documents to ensure forward/backward compatibility across migrations.

---

# Quality and Architectural Issues

### 1. Naming & Structural Mismatch on `base_object`
* **Files**: `crates/op-dbus-model/src/lib.rs:57-90` and `crates/op-dbus-model/src/models.rs:5-10`
* **Details**: In `upsert_document`, the catalog encodes the full `PluginCatalogDocument` as JSON and binds it directly to the `base_object` column of the `plugins` table. However, the `Plugin` model defined in `models.rs` has its own `base_object` field represented as `simd_json::OwnedValue`. This leads to a severe architectural inconsistency: the database column `base_object` contains a completely serialized `PluginCatalogDocument` object (which itself includes fields like `schema`, `dbus_path`, and `storage_path`), whereas other layers of the workspace may parse that column expecting the unstructured runtime value mapped by the `Plugin` model structure. This type confusion can cause runtime deserialization failures.

### 2. Incomplete Catalog Logic (Dead Schema Persistence)
* **Files**: `crates/op-dbus-model/src/lib.rs:21-39` and `crates/op-dbus-model/src/models.rs:13-20`
* **Details**: `create_schema` defines and creates the `schemas` table, and `models.rs` contains the `Schema` struct representing database records. However, `SqlitePluginCatalog` provides absolutely no logic (neither insert, read, list, nor delete) to manage the `schemas` table or the `Schema` structs. The table remains completely unmanaged and represents dead database definitions.

### 3. Use of `eprintln!` in Library Crate
* **File**: `crates/op-dbus-model/src/lib.rs:100-103`
* **Details**: During `list_documents`, any deserialization failures are logged using standard error output via `eprintln!`. This is a library antipattern that pollutes the stdout/stderr of executing binaries. The catalog should instead import and utilize the workspace-available `tracing` crate (`tracing::error!`) to allow application-level configuration of diagnostic logs.