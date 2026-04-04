//! Cognitive Memory Store
//!
//! Namespace-based shared memory backend for the op-dbus chatbot and openclaw.
//! Replaces openclaw's file-based memory with a SQLite-backed namespace model.
//!
//! Architecture:
//! - **Namespace** = a named context (project, session, database, workflow, cron job, agent, etc.)
//!   Maps directly to what openclaw calls a "memory file".
//! - **Entry** = a key/value pair within a namespace, stored as JSON.
//! - Both op-dbus chatbot and openclaw read/write through the cognitive MCP endpoint,
//!   so they share the same memory without file sync or race conditions.
//!
//! Memory is scoped to the control plane / chatbot — not per end-user sessions.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NamespaceKind {
    Project,
    Session,
    Database,
    Workflow,
    Agent,
    Cron,
    Custom,
}

impl std::fmt::Display for NamespaceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Project => "project",
            Self::Session => "session",
            Self::Database => "database",
            Self::Workflow => "workflow",
            Self::Agent => "agent",
            Self::Cron => "cron",
            Self::Custom => "custom",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for NamespaceKind {
    type Err = ();
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "project" => Self::Project,
            "session" => Self::Session,
            "database" => Self::Database,
            "workflow" => Self::Workflow,
            "agent" => Self::Agent,
            "cron" => Self::Cron,
            _ => Self::Custom,
        })
    }
}

/// A named memory context. Equivalent to an openclaw memory file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryNamespace {
    pub id: String,
    /// Canonical name: "project:op-dbus", "cron:backup", "db:ovsdb", etc.
    pub name: String,
    pub kind: NamespaceKind,
    pub description: Option<String>,
    /// Linked Zenflow task or workflow ID.
    pub linked_task_id: Option<String>,
    /// Cron expression if this namespace drives a scheduled job.
    pub linked_cron: Option<String>,
    /// Arbitrary metadata as JSON.
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A key/value entry within a namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub namespace_id: String,
    pub key: String,
    pub value: serde_json::Value,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub access_count: i64,
    pub last_accessed: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EntryQuery {
    pub namespace_id: Option<String>,
    pub key_pattern: Option<String>,
    pub tags: Option<Vec<String>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct MemoryStats {
    pub total_namespaces: i64,
    pub total_entries: i64,
    pub entries_by_kind: Vec<(String, i64)>,
}

pub struct CognitiveMemoryStore {
    pool: SqlitePool,
}

impl CognitiveMemoryStore {
    pub async fn new(pool: SqlitePool) -> Result<Self> {
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS memory_namespaces (
                id           TEXT PRIMARY KEY,
                name         TEXT NOT NULL UNIQUE,
                kind         TEXT NOT NULL,
                description  TEXT,
                linked_task_id TEXT,
                linked_cron  TEXT,
                metadata     TEXT NOT NULL DEFAULT '{}',
                created_at   TEXT NOT NULL,
                updated_at   TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS memory_entries (
                id           TEXT PRIMARY KEY,
                namespace_id TEXT NOT NULL REFERENCES memory_namespaces(id) ON DELETE CASCADE,
                key          TEXT NOT NULL,
                value        TEXT NOT NULL,
                tags         TEXT NOT NULL DEFAULT '[]',
                created_at   TEXT NOT NULL,
                updated_at   TEXT NOT NULL,
                expires_at   TEXT,
                access_count INTEGER NOT NULL DEFAULT 0,
                last_accessed TEXT NOT NULL,
                UNIQUE(namespace_id, key)
            );

            CREATE INDEX IF NOT EXISTS idx_entries_namespace ON memory_entries(namespace_id);
            CREATE INDEX IF NOT EXISTS idx_entries_key ON memory_entries(key);
            CREATE INDEX IF NOT EXISTS idx_namespaces_kind ON memory_namespaces(kind);
            "#,
        )
        .execute(&self.pool)
        .await
        .context("memory schema migration failed")?;
        Ok(())
    }

    pub async fn upsert_namespace(
        &self,
        name: &str,
        kind: NamespaceKind,
        description: Option<&str>,
        linked_task_id: Option<&str>,
        linked_cron: Option<&str>,
        metadata: serde_json::Value,
    ) -> Result<MemoryNamespace> {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        let kind_str = kind.to_string();
        let meta_str = serde_json::to_string(&metadata)?;

        sqlx::query(
            r#"
            INSERT INTO memory_namespaces (id, name, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
            ON CONFLICT(name) DO UPDATE SET
                kind = excluded.kind,
                description = excluded.description,
                linked_task_id = excluded.linked_task_id,
                linked_cron = excluded.linked_cron,
                metadata = excluded.metadata,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&id)
        .bind(name)
        .bind(&kind_str)
        .bind(description)
        .bind(linked_task_id)
        .bind(linked_cron)
        .bind(&meta_str)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await
        .context("upsert namespace")?;

        self.get_namespace_by_name(name)
            .await?
            .context("namespace not found after upsert")
    }

    pub async fn get_namespace_by_name(&self, name: &str) -> Result<Option<MemoryNamespace>> {
        let row = sqlx::query(
            "SELECT id, name, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at FROM memory_namespaces WHERE name = ?1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("get namespace by name")?;

        Ok(row.map(|r| self.row_to_namespace(&r)))
    }

    pub async fn list_namespaces(
        &self,
        kind: Option<NamespaceKind>,
    ) -> Result<Vec<MemoryNamespace>> {
        let rows = if let Some(k) = kind {
            sqlx::query(
                "SELECT id, name, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at FROM memory_namespaces WHERE kind = ?1 ORDER BY name",
            )
            .bind(k.to_string())
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, name, kind, description, linked_task_id, linked_cron, metadata, created_at, updated_at FROM memory_namespaces ORDER BY name",
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows.iter().map(|r| self.row_to_namespace(r)).collect())
    }

    pub async fn delete_namespace(&self, name: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM memory_namespaces WHERE name = ?1")
            .bind(name)
            .execute(&self.pool)
            .await
            .context("delete namespace")?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn store_entry(
        &self,
        namespace_name: &str,
        key: &str,
        value: serde_json::Value,
        tags: Vec<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<MemoryEntry> {
        let ns = self
            .get_namespace_by_name(namespace_name)
            .await?
            .context(format!("namespace '{}' not found", namespace_name))?;

        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        let value_str = serde_json::to_string(&value)?;
        let tags_str = serde_json::to_string(&tags)?;

        sqlx::query(
            r#"
            INSERT INTO memory_entries (id, namespace_id, key, value, tags, created_at, updated_at, expires_at, access_count, last_accessed)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, 0, ?6)
            ON CONFLICT(namespace_id, key) DO UPDATE SET
                value = excluded.value,
                tags = excluded.tags,
                updated_at = excluded.updated_at,
                expires_at = excluded.expires_at
            "#,
        )
        .bind(&id)
        .bind(&ns.id)
        .bind(key)
        .bind(&value_str)
        .bind(&tags_str)
        .bind(now.to_rfc3339())
        .bind(expires_at.map(|t| t.to_rfc3339()))
        .execute(&self.pool)
        .await
        .context("store entry")?;

        self.retrieve_entry(namespace_name, key)
            .await?
            .context("entry not found after store")
    }

    pub async fn retrieve_entry(
        &self,
        namespace_name: &str,
        key: &str,
    ) -> Result<Option<MemoryEntry>> {
        let ns = self.get_namespace_by_name(namespace_name).await?;
        let Some(ns) = ns else { return Ok(None) };

        let row = sqlx::query(
            "SELECT id, namespace_id, key, value, tags, created_at, updated_at, expires_at, access_count, last_accessed FROM memory_entries WHERE namespace_id = ?1 AND key = ?2",
        )
        .bind(&ns.id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .context("retrieve entry")?;

        if let Some(ref r) = row {
            let entry_id: String = r.get("id");
            sqlx::query(
                "UPDATE memory_entries SET access_count = access_count + 1, last_accessed = ?1 WHERE id = ?2",
            )
            .bind(Utc::now().to_rfc3339())
            .bind(&entry_id)
            .execute(&self.pool)
            .await?;
        }

        Ok(row.map(|r| self.row_to_entry(&r)))
    }

    pub async fn query_entries(&self, q: EntryQuery) -> Result<Vec<MemoryEntry>> {
        let namespace_id = if let Some(ns_name) = &q.namespace_id {
            self.get_namespace_by_name(ns_name).await?.map(|ns| ns.id)
        } else {
            None
        };

        let limit = q.limit.unwrap_or(100);
        let offset = q.offset.unwrap_or(0);

        let rows = sqlx::query(
            r#"
            SELECT id, namespace_id, key, value, tags, created_at, updated_at, expires_at, access_count, last_accessed
            FROM memory_entries
            WHERE (?1 IS NULL OR namespace_id = ?1)
              AND (?2 IS NULL OR key LIKE '%' || ?2 || '%')
              AND (expires_at IS NULL OR expires_at > ?3)
            ORDER BY updated_at DESC
            LIMIT ?4 OFFSET ?5
            "#,
        )
        .bind(namespace_id)
        .bind(q.key_pattern)
        .bind(Utc::now().to_rfc3339())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .context("query entries")?;

        let mut entries: Vec<MemoryEntry> = rows.iter().map(|r| self.row_to_entry(r)).collect();

        if let Some(tags) = &q.tags {
            entries.retain(|e| tags.iter().all(|t| e.tags.contains(t)));
        }

        Ok(entries)
    }

    pub async fn delete_entry(&self, namespace_name: &str, key: &str) -> Result<bool> {
        let ns = self.get_namespace_by_name(namespace_name).await?;
        let Some(ns) = ns else { return Ok(false) };

        let result = sqlx::query("DELETE FROM memory_entries WHERE namespace_id = ?1 AND key = ?2")
            .bind(&ns.id)
            .bind(key)
            .execute(&self.pool)
            .await
            .context("delete entry")?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn cleanup_expired(&self) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM memory_entries WHERE expires_at IS NOT NULL AND expires_at < ?1",
        )
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await
        .context("cleanup expired")?;
        Ok(result.rows_affected())
    }

    pub async fn get_stats(&self) -> Result<MemoryStats> {
        let total_namespaces: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_namespaces")
            .fetch_one(&self.pool)
            .await?;

        let total_entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_entries")
            .fetch_one(&self.pool)
            .await?;

        let rows = sqlx::query(
            "SELECT n.kind, COUNT(e.id) as cnt FROM memory_namespaces n LEFT JOIN memory_entries e ON e.namespace_id = n.id GROUP BY n.kind",
        )
        .fetch_all(&self.pool)
        .await?;

        let entries_by_kind = rows
            .iter()
            .map(|r| (r.get::<String, _>("kind"), r.get::<i64, _>("cnt")))
            .collect();

        Ok(MemoryStats {
            total_namespaces,
            total_entries,
            entries_by_kind,
        })
    }

    fn row_to_namespace(&self, r: &sqlx::sqlite::SqliteRow) -> MemoryNamespace {
        let kind_str: String = r.get("kind");
        let meta_str: String = r.get("metadata");
        let created: String = r.get("created_at");
        let updated: String = r.get("updated_at");

        MemoryNamespace {
            id: r.get("id"),
            name: r.get("name"),
            kind: kind_str.parse().unwrap_or(NamespaceKind::Custom),
            description: r.get("description"),
            linked_task_id: r.get("linked_task_id"),
            linked_cron: r.get("linked_cron"),
            metadata: serde_json::from_str(&meta_str).unwrap_or(serde_json::Value::Null),
            created_at: DateTime::parse_from_rfc3339(&created)
                .map(|t| t.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated)
                .map(|t| t.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        }
    }

    fn row_to_entry(&self, r: &sqlx::sqlite::SqliteRow) -> MemoryEntry {
        let value_str: String = r.get("value");
        let tags_str: String = r.get("tags");
        let created: String = r.get("created_at");
        let updated: String = r.get("updated_at");
        let last_accessed: String = r.get("last_accessed");
        let expires_str: Option<String> = r.get("expires_at");

        MemoryEntry {
            id: r.get("id"),
            namespace_id: r.get("namespace_id"),
            key: r.get("key"),
            value: serde_json::from_str(&value_str).unwrap_or(serde_json::Value::Null),
            tags: serde_json::from_str(&tags_str).unwrap_or_default(),
            created_at: DateTime::parse_from_rfc3339(&created)
                .map(|t| t.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated)
                .map(|t| t.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            expires_at: expires_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|t| t.with_timezone(&Utc))
                    .ok()
            }),
            access_count: r.get("access_count"),
            last_accessed: DateTime::parse_from_rfc3339(&last_accessed)
                .map(|t| t.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        }
    }
}
