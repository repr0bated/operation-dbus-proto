pub mod models;

use anyhow::Result;
use sqlx::SqlitePool;

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
