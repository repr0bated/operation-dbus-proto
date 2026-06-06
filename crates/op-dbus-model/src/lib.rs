pub mod models;

use models::PluginCatalogDocument;
use sqlx::{Row, SqlitePool};
use tracing::{debug, error, info, warn};

pub use models::{Plugin, PluginCatalogDocument as CatalogDocument, Schema};

/// Strongly-typed error for database / serialization operations.
///
/// Keeps `anyhow` out of the public API surface.
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[tracing::instrument(skip(pool))]
pub async fn create_schema(pool: &SqlitePool) -> std::result::Result<(), ModelError> {
    info!("Creating plugin catalog schema tables");

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
    .await
    .map_err(|e| {
        error!("Failed to create plugins table: {}", e);
        e
    })?;

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
    .await
    .map_err(|e| {
        error!("Failed to create schemas table: {}", e);
        e
    })?;

    info!("Plugin catalog schema tables created successfully");
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
        debug!("Creating SqlitePluginCatalog");
        Self { pool }
    }

    #[tracing::instrument(skip(self, document))]
    pub async fn upsert_document(
        &self,
        document: &PluginCatalogDocument,
    ) -> std::result::Result<(), ModelError> {
        let plugin_name = document.schema.name.as_str();
        info!(%plugin_name, "Upserting plugin catalog document");

        let encoded = serde_json::to_string(document).map_err(|e| {
            error!(%plugin_name, "Failed to serialize plugin catalog document: {}", e);
            e
        })?;
        debug!(%plugin_name, encoded_len = encoded.len(), "Serialized plugin catalog document");

        sqlx::query(
            r#"
            INSERT INTO plugins (name, service_name, base_object)
            VALUES (?, ?, ?)
            ON CONFLICT(name) DO UPDATE SET
                service_name = excluded.service_name,
                base_object = excluded.base_object
            "#,
        )
        .bind(plugin_name)
        .bind(document.service_name.as_str())
        .bind(encoded)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!(%plugin_name, "Database error upserting plugin catalog document: {}", e);
            e
        })?;

        info!(%plugin_name, "Plugin catalog document upserted successfully");
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_document(
        &self,
        name: &str,
    ) -> std::result::Result<Option<PluginCatalogDocument>, ModelError> {
        info!(plugin_name = %name, "Fetching plugin catalog document");

        let row = sqlx::query("SELECT base_object FROM plugins WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!(plugin_name = %name, "Database error fetching plugin catalog document: {}", e);
                e
            })?;

        let Some(row) = row else {
            debug!(plugin_name = %name, "Plugin catalog document not found");
            return Ok(None);
        };

        let encoded: String = row.try_get("base_object").map_err(|e| {
            error!(plugin_name = %name, "Failed to read base_object column: {}", e);
            e
        })?;
        let document = serde_json::from_str(&encoded).map_err(|e| {
            error!(plugin_name = %name, "Failed to deserialize plugin catalog document: {}", e);
            e
        })?;

        info!(plugin_name = %name, "Plugin catalog document fetched successfully");
        Ok(Some(document))
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_documents(
        &self,
    ) -> std::result::Result<Vec<PluginCatalogDocument>, ModelError> {
        info!("Listing all plugin catalog documents");

        let rows = sqlx::query("SELECT name, base_object FROM plugins ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error listing plugin catalog documents: {}", e);
                e
            })?;

        let mut documents = Vec::new();
        for row in rows {
            let name: String = row.try_get("name").map_err(|e| {
                error!("Failed to read name column: {}", e);
                e
            })?;
            let encoded: String = row.try_get("base_object").map_err(|e| {
                error!(plugin_name = %name, "Failed to read base_object column: {}", e);
                e
            })?;
            match serde_json::from_str(&encoded) {
                Ok(document) => documents.push(document),
                Err(error) => {
                    warn!(
                        plugin_name = %name,
                        "Skipping stale plugin catalog document: {}",
                        error
                    );
                }
            }
        }

        info!(count = documents.len(), "Listed plugin catalog documents");
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
