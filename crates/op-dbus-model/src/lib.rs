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
