This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-dbus-model/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-dbus-model/
              src/
                lib.rs
                models.rs
              Cargo.toml
              compare-op-dbus-model.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dbus-model/src/lib.rs">
pub mod models;

use anyhow::Result;
use models::PluginCatalogDocument;
use sqlx::{Row, SqlitePool};

pub use models::{Plugin, PluginCatalogDocument as CatalogDocument, Schema};

pub async fn create_schema(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS plugins (
            name TEXT PRIMARY KEY,
            service_name TEXT NOT NULL,
            base_object TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS schemas (
            id TEXT PRIMARY KEY,
            plugin_name TEXT NOT NULL,
            definition TEXT NOT NULL,
            discovered_from TEXT,
            discovered_at TIMESTAMP,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (plugin_name) REFERENCES plugins(name)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// SQLite-backed catalog for canonical plugin documents.
///
/// This is a persistence backend, not the architectural source of truth.
/// The source of truth originates in plugin code, which emits one canonical
/// plugin document. The catalog stores that document so D-Bus/gRPC/rendering
/// layers can mirror the same persisted shape.
#[derive(Clone)]
pub struct SqlitePluginCatalog {
    pool: SqlitePool,
}

impl SqlitePluginCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_document(&self, document: &PluginCatalogDocument) -> Result<()> {
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
        .bind(encoded)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_document(&self, name: &str) -> Result<Option<PluginCatalogDocument>> {
        let row = sqlx::query("SELECT base_object FROM plugins WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let encoded: String = row.try_get("base_object")?;
        let document = serde_json::from_str(&encoded)?;
        Ok(Some(document))
    }

    pub async fn list_documents(&self) -> Result<Vec<PluginCatalogDocument>> {
        let rows = sqlx::query("SELECT name, base_object FROM plugins ORDER BY name")
            .fetch_all(&self.pool)
            .await?;

        let mut documents = Vec::new();
        for row in rows {
            let name: String = row.try_get("name")?;
            let encoded: String = row.try_get("base_object")?;
            match serde_json::from_str(&encoded) {
                Ok(document) => documents.push(document),
                Err(error) => {
                    eprintln!(
                        "Skipping stale plugin catalog document '{}': {}",
                        name, error
                    );
                }
            }
        }

        Ok(documents)
    }
}

/// Compatibility alias while the rest of the workspace still says "schema
/// catalog" in some places.
///
/// Architecturally the primary name is `SqlitePluginCatalog` because each
/// entry is a canonical plugin document whose schema, footprint, and render
/// contract are one and the same.
pub type SqliteSchemaCatalog = SqlitePluginCatalog;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dbus-model/src/models.rs">
use chrono::{DateTime, Utc};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub service_name: String,
    pub base_object: simd_json::OwnedValue,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub id: String,
    pub plugin_name: String,
    pub definition: simd_json::OwnedValue,
    pub discovered_from: Option<String>,
    pub discovered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Canonical persisted plugin document.
///
/// Architectural rule:
/// - The plugin defines the schema.
/// - That same schema is the footprint and JSON render contract.
/// - This document is the persisted authority that projection layers mirror.
///
/// The document stays intentionally small. We do not create separate runtime
/// "schema", "footprint", or "render" authorities here because that would
/// reintroduce the drift this refactor is removing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCatalogDocument {
    /// Canonical plugin-owned schema. This is the thing every downstream
    /// consumer ultimately resolves.
    pub schema: PluginSchema,
    /// Stable D-Bus projection path for the plugin.
    pub dbus_path: String,
    /// Service identity used by external projections and compatibility layers.
    pub service_name: String,
    /// Durable storage path allocated to the plugin instance.
    pub storage_path: String,
    /// Origin marker for diagnostics; runtime plugin registration should use
    /// `"plugin"` rather than inventing a second authority.
    pub source: String,
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dbus-model/Cargo.toml">
[package]
name = "op-dbus-model"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
simd-json = { workspace = true }
serde_json = { workspace = true }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "json"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.6", features = ["v4", "serde"] }
thiserror = "1.0"
anyhow = "1.0"
op-core = { path = "../op-core" }
op-state-store = { path = "../op-state-store" }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dbus-model/compare-op-dbus-model.md">
# compare-op-dbus-model

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 2 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 1 |
| Partial artifacts | 0 |
| Spec-listed source files | 0 |
| Spec-listed but missing | 0 |
| Extra implementation files | 2 |

## Current Implementation Overview

- Internal crate integrations: op-core, op-state-store.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `root` | ✅ Present | root source group | src/lib.rs, src/models.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| Architecture | ❌ Missing | no clear source match for SPEC.md | SPEC.md |
| Key Components | ✅ Implemented | src/lib.rs | SPEC.md |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - not listed in SPEC dependency block
- `op-state-store` - not listed in SPEC dependency block

### External Runtime Dependencies
- `serde` - not listed in SPEC dependency block
- `simd-json` - not listed in SPEC dependency block
- `serde_json` - not listed in SPEC dependency block
- `sqlx` - not listed in SPEC dependency block
- `chrono` - not listed in SPEC dependency block
- `uuid` - not listed in SPEC dependency block
- `thiserror` - not listed in SPEC dependency block
- `anyhow` - not listed in SPEC dependency block

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Current implementation contains 2 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: models.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-dbus-model/SPEC.md">
# op-dbus-model - Specification

## Overview
**Crate**: `op-dbus-model`  
**Location**: `crates/op-dbus-model`  
**Version**: 0.1.0  
**Edition**: 2021

## Purpose

The `op-dbus-model` crate provides the core data models and database schema management for the operation-dbus system. It defines the persistence layer for plugin metadata and D-Bus interface schemas discovered through introspection.

This crate serves as the foundational data layer that enables:
- Plugin registration and lifecycle tracking
- Schema discovery and versioning
- Metadata persistence for D-Bus services
- Historical tracking of interface definitions

## Architecture

### Database Layer
- **Backend**: SQLite via sqlx
- **Runtime**: Tokio async runtime
- **Schema Management**: Automated table creation and migration

### Data Models

#### Plugin Model
Represents a registered D-Bus plugin/service in the system.

```rust
pub struct Plugin {
    pub name: String,              // Unique plugin identifier
    pub service_name: String,      // D-Bus service name
    pub base_object: OwnedValue,   // Base object path (JSON)
    pub created_at: DateTime<Utc>, // Registration timestamp
}
```

**Database Schema**:
```sql
CREATE TABLE plugins (
    name TEXT PRIMARY KEY,
    service_name TEXT NOT NULL,
    base_object TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
)
```

#### Schema Model
Represents a discovered D-Bus interface schema.

```rust
pub struct Schema {
    pub id: String,                      // Unique schema identifier
    pub plugin_name: String,             // Foreign key to plugin
    pub definition: OwnedValue,          // Interface definition (JSON)
    pub discovered_from: Option<String>, // Discovery source
    pub discovered_at: Option<DateTime<Utc>>, // Discovery timestamp
    pub created_at: DateTime<Utc>,       // Record creation time
}
```

**Database Schema**:
```sql
CREATE TABLE schemas (
    id TEXT PRIMARY KEY,
    plugin_name TEXT NOT NULL,
    definition TEXT NOT NULL,
    discovered_from TEXT,
    discovered_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (plugin_name) REFERENCES plugins(name)
)
```

## Key Components

### Schema Creation
```rust
pub async fn create_schema(pool: &SqlitePool) -> Result<()>
```

Initializes the database schema with all required tables. This function is idempotent and safe to call multiple times.

**Features**:
- Creates `plugins` table for service registration
- Creates `schemas` table for interface definitions
- Establishes foreign key relationships
- Automatic timestamp management

## Dependencies

### Core Dependencies
- **sqlx** (0.8): Async SQL toolkit with SQLite support
  - Features: `runtime-tokio`, `sqlite`, `json`
- **serde** (1.0): Serialization framework
  - Features: `derive`
- **simd-json**: High-performance JSON handling
- **chrono** (0.4): Date and time handling
  - Features: `serde`
- **uuid** (1.6): Unique identifier generation
  - Features: `v4`, `serde`

### Error Handling
- **thiserror** (1.0): Derive macros for error types
- **anyhow** (1.0): Flexible error handling

### Internal Dependencies
- **op-core**: Core types and utilities

## Usage

### Initialization

```rust
use op_dbus_model::create_schema;
use sqlx::SqlitePool;

// Connect to database
let pool = SqlitePool::connect("sqlite:operation-dbus.db").await?;

// Initialize schema
create_schema(&pool).await?;
```

### Working with Models

```rust
use op_dbus_model::models::{Plugin, Schema};
use chrono::Utc;

// Create a plugin record
let plugin = Plugin {
    name: "my-service".to_string(),
    service_name: "org.example.MyService".to_string(),
    base_object: simd_json::json!("/org/example"),
    created_at: Utc::now(),
};

// Create a schema record
let schema = Schema {
    id: uuid::Uuid::new_v4().to_string(),
    plugin_name: "my-service".to_string(),
    definition: simd_json::json!({
        "interface": "org.example.MyInterface",
        "methods": [...]
    }),
    discovered_from: Some("introspection".to_string()),
    discovered_at: Some(Utc::now()),
    created_at: Utc::now(),
};
```

## Integration Points

### Plugin Registration Flow
1. Plugin discovered via D-Bus introspection
2. Plugin metadata stored in `plugins` table
3. Interface schemas extracted and stored in `schemas` table
4. Foreign key maintains relationship between plugin and schemas

### Schema Discovery Flow
1. D-Bus interface introspected
2. Schema definition serialized to JSON
3. Schema record created with discovery metadata
4. Linked to parent plugin via `plugin_name`

## Data Integrity

### Referential Integrity
- Foreign key constraint ensures schemas reference valid plugins
- Cascade behavior can be configured for plugin deletion

### Timestamp Tracking
- `created_at`: Automatic timestamp on record creation
- `discovered_at`: Manual timestamp for schema discovery events

### JSON Storage
- `base_object`: Flexible storage for object path configurations
- `definition`: Complete interface definition with methods, signals, properties

## Performance Considerations

- **SQLite**: Suitable for single-node deployments
- **Connection Pooling**: Managed by sqlx for concurrent access
- **JSON Indexing**: Consider adding indexes on JSON fields for large datasets
- **Async Operations**: Non-blocking database access via tokio

## Future Enhancements

- Schema versioning and migration tracking
- Plugin dependency management
- Schema validation and compatibility checking
- Query builders for common access patterns
- Migration to PostgreSQL for distributed deployments

---
*Core data models and persistence layer for operation-dbus*
</file>

</files>
