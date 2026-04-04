//! SQLite storage

use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::Path;
use tracing::info;

use crate::schema::{ServiceDef, ServiceName};

pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub async fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let url = format!("sqlite:{}?mode=rwc", path.as_ref().display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await?;

        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS services (
                name TEXT PRIMARY KEY,
                definition TEXT NOT NULL,
                enabled INTEGER DEFAULT 0,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_name TEXT,
                action TEXT NOT NULL,
                details TEXT,
                timestamp TEXT DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&self.pool)
        .await?;

        info!("Database migrated");
        Ok(())
    }

    pub async fn get_service(&self, name: &ServiceName) -> anyhow::Result<Option<ServiceDef>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT definition FROM services WHERE name = ?")
                .bind(name.as_str())
                .fetch_optional(&self.pool)
                .await?;

        match row {
            Some((json,)) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    pub async fn save_service(&self, service: &ServiceDef) -> anyhow::Result<()> {
        let json = serde_json::to_string(service)?;
        sqlx::query(
            "INSERT OR REPLACE INTO services (name, definition, enabled, updated_at) VALUES (?, ?, ?, CURRENT_TIMESTAMP)"
        )
        .bind(service.name.as_str())
        .bind(&json)
        .bind(service.enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_service(&self, name: &ServiceName) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM services WHERE name = ?")
            .bind(name.as_str())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_services(&self) -> anyhow::Result<Vec<ServiceDef>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT definition FROM services")
            .fetch_all(&self.pool)
            .await?;

        rows.into_iter()
            .map(|(json,)| serde_json::from_str(&json).map_err(Into::into))
            .collect()
    }

    pub async fn audit(
        &self,
        service: Option<&str>,
        action: &str,
        details: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO audit_log (service_name, action, details) VALUES (?, ?, ?)")
            .bind(service)
            .bind(action)
            .bind(details)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
