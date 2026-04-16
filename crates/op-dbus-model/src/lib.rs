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

/// Schema-library catalog for plugin documents.
///
/// This store is only an index of plugin-owned schema documents. It exists so
/// builders, renderers, and compatibility layers can reuse known schema shapes
/// when composing new schemas. Runtime state and platform reality live outside
/// this catalog.
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
        // Legacy column retained for compatibility with existing databases.
        // The schema-library document itself no longer carries service identity.
        .bind(document.schema.name.as_str())
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
        decode_document(&encoded)
    }

    pub async fn list_documents(&self) -> Result<Vec<PluginCatalogDocument>> {
        let rows = sqlx::query("SELECT base_object FROM plugins ORDER BY name")
            .fetch_all(&self.pool)
            .await?;

        let mut documents = Vec::with_capacity(rows.len());
        for row in rows {
            let encoded: String = row.try_get("base_object")?;
            if let Some(document) = decode_document(&encoded)? {
                documents.push(document);
            }
        }
        Ok(documents)
    }
}

fn decode_document(encoded: &str) -> Result<Option<PluginCatalogDocument>> {
    match serde_json::from_str::<PluginCatalogDocument>(encoded) {
        Ok(document) => Ok(Some(document)),
        Err(error) => {
            let parsed = match serde_json::from_str::<serde_json::Value>(encoded) {
                Ok(parsed) => parsed,
                Err(_) => return Err(error.into()),
            };

            if parsed.get("schema").is_none() {
                return Ok(None);
            }

            Err(error.into())
        }
    }
}

/// Compatibility alias while the rest of the workspace still says "schema
/// catalog" in some places.
///
/// Each entry is a plugin document in the schema library.
pub type SqliteSchemaCatalog = SqlitePluginCatalog;

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::PluginSchema;
    use std::collections::HashMap;

    fn sample_document() -> PluginCatalogDocument {
        PluginCatalogDocument {
            schema: PluginSchema {
                name: "sample".to_string(),
                category: "test".to_string(),
                version: "1.0.0".to_string(),
                description: "sample schema".to_string(),
                fields: HashMap::new(),
                dependencies: Vec::new(),
                example: None,
                immutable_paths: Vec::new(),
                tags: Vec::new(),
                dialect: op_state_store::DEFAULT_SCHEMA_DIALECT.to_string(),
            },
            source: "plugin".to_string(),
        }
    }

    #[test]
    fn decode_document_accepts_schema_library_document() {
        let encoded = serde_json::to_string(&sample_document()).expect("schema document");

        let decoded = decode_document(&encoded).expect("decode should succeed");

        assert!(decoded.is_some());
        assert_eq!(decoded.unwrap().schema.name, "sample");
    }

    #[test]
    fn decode_document_accepts_legacy_runtime_hints() {
        let encoded = r#"{
            "schema": {
                "name": "sample",
                "category": "test",
                "version": "1.0.0",
                "description": "sample schema",
                "fields": {},
                "dependencies": [],
                "example": null,
                "immutable_paths": [],
                "tags": [],
                "dialect": "op-state/v1"
            },
            "dbus_path": "/org/opdbus/v1/plugins/sample",
            "service_name": "org.opdbus.sample.v1",
            "storage_path": "/var/lib/op-dbus/plugins/sample",
            "source": "plugin"
        }"#;

        let decoded = decode_document(encoded).expect("decode should succeed");

        assert!(decoded.is_some());
        assert_eq!(decoded.unwrap().schema.name, "sample");
    }

    #[test]
    fn decode_document_skips_legacy_plugin_rows_without_schema() {
        let legacy = r#"{
            "type": "DirectoryEntry",
            "description": "legacy row",
            "object_types": {
                "User": {
                    "interface": "org.opdbus.directory.v1.User"
                }
            }
        }"#;

        let decoded = decode_document(legacy).expect("legacy rows should be tolerated");

        assert!(decoded.is_none());
    }
}
