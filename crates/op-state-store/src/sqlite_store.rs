//! SQLite-based persistent state store
//!
//! Provides durable storage for execution jobs, plugin state snapshots,
//! and audit trail. Uses SQLx for async database operations.

use crate::error::{Result, StateStoreError};
use crate::execution_job::{ExecutionJob, ExecutionStatus};
use crate::state_store::{StateStore, ToolRecord};
use crate::{CanonicalDbExport, StoredObject};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// SQLite-backed state store for execution jobs and plugin state
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    /// Create a new SQLite store with the given database URL
    ///
    /// URL format: `sqlite:///path/to/db.sqlite` or `sqlite::memory:`
    pub async fn new(url: &str) -> Result<Self> {
        info!("Initializing SQLite state store: {}", url);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await?;

        let store = Self { pool };
        store.initialize_schema().await?;

        info!("SQLite state store initialized successfully");
        Ok(store)
    }

    /// Create an in-memory store for testing
    pub async fn in_memory() -> Result<Self> {
        Self::new("sqlite::memory:").await
    }

    /// Get the underlying SQLx pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Initialize database schema
    async fn initialize_schema(&self) -> Result<()> {
        debug!("Initializing database schema");

        // Create execution_jobs table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS execution_jobs (
                id TEXT PRIMARY KEY,
                tool_name TEXT NOT NULL,
                arguments TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                result TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create plugin_state table for caching plugin state snapshots
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS plugin_state (
                plugin_name TEXT PRIMARY KEY,
                state_json TEXT NOT NULL,
                state_hash TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create checkpoints table for rollback support
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                plugin_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                state_snapshot TEXT NOT NULL,
                backend_checkpoint TEXT,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create audit_log table for tracking all state changes
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                plugin_name TEXT NOT NULL,
                operation TEXT NOT NULL,
                data TEXT NOT NULL,
                footprint_hash TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indices for common queries
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_jobs_status ON execution_jobs(status)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_jobs_created ON execution_jobs(created_at)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_checkpoints_plugin ON checkpoints(plugin_name)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_plugin ON audit_log(plugin_name)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp)")
            .execute(&self.pool)
            .await?;

        // Execute namespace schema (org.opdbus.* enterprise tables)
        debug!("Initializing enterprise namespace schema...");
        let namespace_schema = include_str!("namespace_schema.sql");

        // Split by semicolon but keep multi-line statements together
        let mut current_statement = String::new();
        for line in namespace_schema.lines() {
            let trimmed = line.trim();

            // Skip comment-only lines
            if trimmed.starts_with("--") || trimmed.is_empty() {
                continue;
            }

            current_statement.push_str(line);
            current_statement.push('\n');

            // Execute when we hit a semicolon at the end of a line
            if trimmed.ends_with(';') {
                let stmt = current_statement.trim();
                if !stmt.is_empty() {
                    if let Err(e) = sqlx::query(stmt).execute(&self.pool).await {
                        warn!(
                            "Failed to execute namespace schema statement: {} - Error: {}",
                            stmt.chars().take(100).collect::<String>(),
                            e
                        );
                        // Continue on error for idempotency (IF NOT EXISTS)
                    }
                }
                current_statement.clear();
            }
        }

        info!("✅ Enterprise namespace schema initialized (org.opdbus.*)");

        // Create tools table for tool registry persistence (read-only on startup)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tools (
                tool_name TEXT PRIMARY KEY,
                definition_json TEXT NOT NULL,
                category TEXT NOT NULL,
                namespace TEXT NOT NULL,
                schema_version TEXT NOT NULL DEFAULT 'https://json-schema.org/draft/next/schema',
                source TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create objects table for general object storage
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS objects (
                id TEXT PRIMARY KEY,
                object_type TEXT NOT NULL,
                namespace TEXT NOT NULL,
                data TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Execute Full Active Directory schema
        debug!("Initializing Full Active Directory schema...");
        let ad_schema = include_str!("ad_full_schema.sql");
        let mut current_statement = String::new();
        for line in ad_schema.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("--") || trimmed.is_empty() {
                continue;
            }
            current_statement.push_str(line);
            current_statement.push('\n');
            if trimmed.ends_with(';') {
                let stmt = current_statement.trim();
                if !stmt.is_empty() {
                    if let Err(e) = sqlx::query(stmt).execute(&self.pool).await {
                        warn!("Failed to execute AD schema statement: {}", e);
                    }
                }
                current_statement.clear();
            }
        }

        // Execute Drupal CMS schema
        debug!("Initializing Drupal CMS schema...");
        let drupal_schema = include_str!("cms_drupal_schema.sql");
        let mut current_statement = String::new();
        for line in drupal_schema.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("--") || trimmed.is_empty() {
                continue;
            }
            current_statement.push_str(line);
            current_statement.push('\n');
            if trimmed.ends_with(';') {
                let stmt = current_statement.trim();
                if !stmt.is_empty() {
                    if let Err(e) = sqlx::query(stmt).execute(&self.pool).await {
                        warn!("Failed to execute Drupal schema statement: {}", e);
                    }
                }
                current_statement.clear();
            }
        }

        // Execute WordPress CMS schema
        debug!("Initializing WordPress CMS schema...");
        let wordpress_schema = include_str!("cms_wordpress_schema.sql");
        let mut current_statement = String::new();
        for line in wordpress_schema.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("--") || trimmed.is_empty() {
                continue;
            }
            current_statement.push_str(line);
            current_statement.push('\n');
            if trimmed.ends_with(';') {
                let stmt = current_statement.trim();
                if !stmt.is_empty() {
                    if let Err(e) = sqlx::query(stmt).execute(&self.pool).await {
                        warn!("Failed to execute WordPress schema statement: {}", e);
                    }
                }
                current_statement.clear();
            }
        }

        info!("✅ Extended enterprise schemas loaded: Full AD + Drupal + WordPress");
        debug!("Database schema initialized");
        Ok(())
    }

    /// Save plugin state snapshot
    pub async fn save_plugin_state(
        &self,
        plugin_name: &str,
        state: &simd_json::OwnedValue,
    ) -> Result<()> {
        let state_json = simd_json::to_string(state)?;
        let state_hash = format!("{:x}", md5::compute(&state_json));
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO plugin_state (plugin_name, state_json, state_hash, updated_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(plugin_name) DO UPDATE SET
                state_json = excluded.state_json,
                state_hash = excluded.state_hash,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(plugin_name)
        .bind(&state_json)
        .bind(&state_hash)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        debug!("Saved plugin state for {}", plugin_name);
        Ok(())
    }

    /// Get plugin state snapshot
    pub async fn get_plugin_state(
        &self,
        plugin_name: &str,
    ) -> Result<Option<simd_json::OwnedValue>> {
        let row = sqlx::query("SELECT state_json FROM plugin_state WHERE plugin_name = ?")
            .bind(plugin_name)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let mut state_json: String = row.get("state_json");
                let state: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut state_json)? };
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    /// Save a checkpoint for rollback
    pub async fn save_checkpoint(
        &self,
        id: &str,
        plugin_name: &str,
        timestamp: i64,
        state_snapshot: &simd_json::OwnedValue,
        backend_checkpoint: Option<&simd_json::OwnedValue>,
    ) -> Result<()> {
        let state_json = simd_json::to_string(state_snapshot)?;
        let backend_json = backend_checkpoint
            .map(|v| simd_json::to_string(v))
            .transpose()?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO checkpoints (id, plugin_name, timestamp, state_snapshot, backend_checkpoint, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id)
        .bind(plugin_name)
        .bind(timestamp)
        .bind(&state_json)
        .bind(&backend_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        debug!("Saved checkpoint {} for {}", id, plugin_name);
        Ok(())
    }

    /// Get a checkpoint by ID
    pub async fn get_checkpoint(&self, id: &str) -> Result<Option<CheckpointRecord>> {
        let row = sqlx::query(
            "SELECT id, plugin_name, timestamp, state_snapshot, backend_checkpoint, created_at FROM checkpoints WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let mut state_json: String = row.get("state_snapshot");
                let mut backend_json: Option<String> = row.get("backend_checkpoint");

                Ok(Some(CheckpointRecord {
                    id: row.get("id"),
                    plugin_name: row.get("plugin_name"),
                    timestamp: row.get("timestamp"),
                    state_snapshot: unsafe { simd_json::from_str(&mut state_json)? },
                    backend_checkpoint: backend_json
                        .as_mut()
                        .map(|s| unsafe { simd_json::from_str(s) })
                        .transpose()?,
                    created_at: row.get("created_at"),
                }))
            }
            None => Ok(None),
        }
    }

    /// Get latest checkpoint for a plugin
    pub async fn get_latest_checkpoint(
        &self,
        plugin_name: &str,
    ) -> Result<Option<CheckpointRecord>> {
        let row = sqlx::query(
            "SELECT id, plugin_name, timestamp, state_snapshot, backend_checkpoint, created_at FROM checkpoints WHERE plugin_name = ? ORDER BY timestamp DESC LIMIT 1",
        )
        .bind(plugin_name)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let mut state_json: String = row.get("state_snapshot");
                let mut backend_json: Option<String> = row.get("backend_checkpoint");

                Ok(Some(CheckpointRecord {
                    id: row.get("id"),
                    plugin_name: row.get("plugin_name"),
                    timestamp: row.get("timestamp"),
                    state_snapshot: unsafe { simd_json::from_str(&mut state_json)? },
                    backend_checkpoint: backend_json
                        .as_mut()
                        .map(|s| unsafe { simd_json::from_str(s) })
                        .transpose()?,
                    created_at: row.get("created_at"),
                }))
            }
            None => Ok(None),
        }
    }

    /// Log an audit entry
    pub async fn log_audit(
        &self,
        plugin_name: &str,
        operation: &str,
        data: &simd_json::OwnedValue,
        footprint_hash: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let data_json = simd_json::to_string(data)?;

        sqlx::query(
            r#"
            INSERT INTO audit_log (timestamp, plugin_name, operation, data, footprint_hash)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&now)
        .bind(plugin_name)
        .bind(operation)
        .bind(&data_json)
        .bind(footprint_hash)
        .execute(&self.pool)
        .await?;

        debug!("Logged audit entry for {} - {}", plugin_name, operation);
        Ok(())
    }

    /// Get audit log entries for a plugin
    pub async fn get_audit_log(
        &self,
        plugin_name: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AuditEntry>> {
        let rows = if let Some(name) = plugin_name {
            sqlx::query(
                "SELECT id, timestamp, plugin_name, operation, data, footprint_hash FROM audit_log WHERE plugin_name = ? ORDER BY id DESC LIMIT ?",
            )
            .bind(name)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, timestamp, plugin_name, operation, data, footprint_hash FROM audit_log ORDER BY id DESC LIMIT ?",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        let mut entries = Vec::new();
        for row in rows {
            let mut data_json: String = row.get("data");
            entries.push(AuditEntry {
                id: row.get("id"),
                timestamp: row.get("timestamp"),
                plugin_name: row.get("plugin_name"),
                operation: row.get("operation"),
                data: unsafe { simd_json::from_str(&mut data_json)? },
                footprint_hash: row.get("footprint_hash"),
            });
        }

        Ok(entries)
    }

    /// List all jobs with optional status filter
    pub async fn list_jobs(
        &self,
        status: Option<ExecutionStatus>,
        limit: i64,
    ) -> Result<Vec<ExecutionJob>> {
        let rows = if let Some(status) = status {
            let status_str = status_to_string(&status);
            sqlx::query(
                "SELECT id, tool_name, arguments, status, created_at, updated_at, result FROM execution_jobs WHERE status = ? ORDER BY created_at DESC LIMIT ?",
            )
            .bind(status_str)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, tool_name, arguments, status, created_at, updated_at, result FROM execution_jobs ORDER BY created_at DESC LIMIT ?",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row_to_job(&row)?);
        }

        Ok(jobs)
    }

    /// Count jobs by status
    pub async fn count_jobs_by_status(&self) -> Result<JobCounts> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total,
                SUM(CASE WHEN status = 'Pending' THEN 1 ELSE 0 END) as pending,
                SUM(CASE WHEN status = 'Running' THEN 1 ELSE 0 END) as running,
                SUM(CASE WHEN status = 'Completed' THEN 1 ELSE 0 END) as completed,
                SUM(CASE WHEN status = 'Failed' THEN 1 ELSE 0 END) as failed
            FROM execution_jobs
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(JobCounts {
            total: row.get::<i64, _>("total") as u64,
            pending: row.get::<i64, _>("pending") as u64,
            running: row.get::<i64, _>("running") as u64,
            completed: row.get::<i64, _>("completed") as u64,
            failed: row.get::<i64, _>("failed") as u64,
        })
    }

    /// Delete old jobs (cleanup)
    pub async fn delete_old_jobs(&self, before: DateTime<Utc>) -> Result<u64> {
        let before_str = before.to_rfc3339();

        let result = sqlx::query(
            "DELETE FROM execution_jobs WHERE created_at < ? AND status IN ('Completed', 'Failed')",
        )
        .bind(&before_str)
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected();
        info!("Deleted {} old jobs from before {}", deleted, before_str);
        Ok(deleted)
    }

    /// Delete old checkpoints (keep only latest N per plugin)
    pub async fn cleanup_checkpoints(&self, keep_per_plugin: i64) -> Result<u64> {
        // This is a bit complex - we need to delete all but the latest N checkpoints per plugin
        let result = sqlx::query(
            r#"
            DELETE FROM checkpoints WHERE id IN (
                SELECT id FROM (
                    SELECT id, ROW_NUMBER() OVER (PARTITION BY plugin_name ORDER BY timestamp DESC) as rn
                    FROM checkpoints
                ) WHERE rn > ?
            )
            "#,
        )
        .bind(keep_per_plugin)
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected();
        info!("Deleted {} old checkpoints", deleted);
        Ok(deleted)
    }

    /// Get database statistics
    pub async fn get_stats(&self) -> Result<StoreStats> {
        let job_counts = self.count_jobs_by_status().await?;

        let checkpoint_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM checkpoints")
            .fetch_one(&self.pool)
            .await?;

        let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
            .fetch_one(&self.pool)
            .await?;

        let plugin_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugin_state")
            .fetch_one(&self.pool)
            .await?;

        Ok(StoreStats {
            jobs: job_counts,
            checkpoints: checkpoint_count as u64,
            audit_entries: audit_count as u64,
            plugin_states: plugin_count as u64,
        })
    }
}

#[async_trait]
impl StateStore for SqliteStore {
    async fn save_job(&self, job: &ExecutionJob) -> Result<()> {
        let arguments_json = simd_json::to_string(&job.arguments)?;
        let result_json = job
            .result
            .as_ref()
            .map(|r| simd_json::to_string(r))
            .transpose()?;
        let status_str = status_to_string(&job.status);

        sqlx::query(
            r#"
            INSERT INTO execution_jobs (id, tool_name, arguments, status, created_at, updated_at, result)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(job.id.to_string())
        .bind(&job.tool_name)
        .bind(&arguments_json)
        .bind(status_str)
        .bind(job.created_at.to_rfc3339())
        .bind(job.updated_at.to_rfc3339())
        .bind(&result_json)
        .execute(&self.pool)
        .await?;

        debug!("Saved job {} ({})", job.id, job.tool_name);
        Ok(())
    }

    async fn get_job(&self, id: Uuid) -> Result<Option<ExecutionJob>> {
        let row = sqlx::query(
            "SELECT id, tool_name, arguments, status, created_at, updated_at, result FROM execution_jobs WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(row_to_job(&row)?)),
            None => Ok(None),
        }
    }

    async fn update_job(&self, job: &ExecutionJob) -> Result<()> {
        let arguments_json = simd_json::to_string(&job.arguments)?;
        let result_json = job
            .result
            .as_ref()
            .map(|r| simd_json::to_string(r))
            .transpose()?;
        let status_str = status_to_string(&job.status);

        let result = sqlx::query(
            r#"
            UPDATE execution_jobs
            SET tool_name = ?, arguments = ?, status = ?, updated_at = ?, result = ?
            WHERE id = ?
            "#,
        )
        .bind(&job.tool_name)
        .bind(&arguments_json)
        .bind(status_str)
        .bind(job.updated_at.to_rfc3339())
        .bind(&result_json)
        .bind(job.id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            warn!("Job {} not found for update", job.id);
            return Err(StateStoreError::NotFound(job.id.to_string()));
        }

        debug!("Updated job {} to status {:?}", job.id, job.status);
        Ok(())
    }

    async fn get_object(&self, id: &str) -> Result<Option<StoredObject>> {
        let row = sqlx::query("SELECT id, object_type, namespace, data FROM objects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => {
                let mut data_json: String = r.get("data");
                Ok(Some(StoredObject {
                    id: r.get("id"),
                    object_type: r.get("object_type"),
                    namespace: r.get("namespace"),
                    data: unsafe { simd_json::from_str(&mut data_json)? },
                }))
            }
            None => Ok(None),
        }
    }

    async fn upsert_object(
        &self,
        id: &str,
        object_type: &str,
        namespace: &str,
        data: &simd_json::OwnedValue,
    ) -> Result<()> {
        let data_json = simd_json::to_string(data)?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO objects (id, object_type, namespace, data, updated_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                data = excluded.data,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(id)
        .bind(object_type)
        .bind(namespace)
        .bind(&data_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn export_canonical(&self) -> Result<CanonicalDbExport> {
        // Get all objects
        let rows = sqlx::query("SELECT id, object_type, namespace, data FROM objects")
            .fetch_all(&self.pool)
            .await?;

        let objects: Vec<StoredObject> = rows
            .iter()
            .map(|r| {
                let mut data_json: String = r.get("data");
                StoredObject {
                    id: r.get("id"),
                    object_type: r.get("object_type"),
                    namespace: r.get("namespace"),
                    data: unsafe { simd_json::from_str(&mut data_json).unwrap_or_default() },
                }
            })
            .collect();

        Ok(CanonicalDbExport {
            objects,
            executions: vec![], // To be populated if needed
            blockchain: vec![], // To be populated if needed
        })
    }

    /// Save tools to database (WRITE: onboarding/upgrade/migration/chatbot changes)
    async fn save_tools(&self, tools: Vec<ToolRecord>) -> Result<()> {
        let tool_count = tools.len();
        let mut tx = self.pool.begin().await?;

        for tool in tools {
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO tools (tool_name, definition_json, category, namespace, schema_version, source, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&tool.tool_name)
            .bind(&tool.definition_json)
            .bind(&tool.category)
            .bind(&tool.namespace)
            .bind(&tool.schema_version)
            .bind(&tool.source)
            .bind(&tool.created_at)
            .bind(&tool.updated_at)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        info!("Saved {} tools to database", tool_count);
        Ok(())
    }

    /// Load tools from database (READ: normal startup)
    async fn load_tools(&self) -> Result<Vec<ToolRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT tool_name, definition_json, category, namespace, schema_version, source, created_at, updated_at
            FROM tools
            ORDER BY tool_name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let tools: Vec<ToolRecord> = rows
            .iter()
            .map(|row| ToolRecord {
                tool_name: row.get("tool_name"),
                definition_json: row.get("definition_json"),
                category: row.get("category"),
                namespace: row.get("namespace"),
                schema_version: row.get("schema_version"),
                source: row.get("source"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        info!("Loaded {} tools from database", tools.len());
        Ok(tools)
    }

    /// Check if tools table is empty (indicates first run/onboarding)
    async fn is_tools_empty(&self) -> Result<bool> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tools")
            .fetch_one(&self.pool)
            .await?;
        Ok(count == 0)
    }

    /// Clear all tools (for migration/upgrade)
    async fn clear_tools(&self) -> Result<()> {
        sqlx::query("DELETE FROM tools").execute(&self.pool).await?;
        info!("Cleared all tools from database");
        Ok(())
    }
}

/// Helper function to convert status enum to string
fn status_to_string(status: &ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Pending => "Pending",
        ExecutionStatus::Running => "Running",
        ExecutionStatus::Completed => "Completed",
        ExecutionStatus::Failed => "Failed",
    }
}

/// Helper function to convert string to status enum
fn string_to_status(s: &str) -> ExecutionStatus {
    match s {
        "Pending" => ExecutionStatus::Pending,
        "Running" => ExecutionStatus::Running,
        "Completed" => ExecutionStatus::Completed,
        "Failed" => ExecutionStatus::Failed,
        _ => ExecutionStatus::Pending, // Default fallback
    }
}

/// Helper function to convert database row to ExecutionJob
fn row_to_job(row: &sqlx::sqlite::SqliteRow) -> Result<ExecutionJob> {
    let id_str: String = row.get("id");
    let mut arguments_json: String = row.get("arguments");
    let status_str: String = row.get("status");
    let created_at_str: String = row.get("created_at");
    let updated_at_str: String = row.get("updated_at");
    let mut result_json: Option<String> = row.get("result");

    Ok(ExecutionJob {
        id: Uuid::parse_str(&id_str).map_err(|e| StateStoreError::NotFound(e.to_string()))?,
        tool_name: row.get("tool_name"),
        arguments: unsafe { simd_json::from_str(&mut arguments_json)? },
        status: string_to_status(&status_str),
        created_at: DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| StateStoreError::NotFound(e.to_string()))?
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| StateStoreError::NotFound(e.to_string()))?
            .with_timezone(&Utc),
        result: result_json
            .as_mut()
            .map(|s| unsafe { simd_json::from_str(s) })
            .transpose()?,
    })
}

/// Checkpoint record from database
#[derive(Debug, Clone)]
pub struct CheckpointRecord {
    pub id: String,
    pub plugin_name: String,
    pub timestamp: i64,
    pub state_snapshot: simd_json::OwnedValue,
    pub backend_checkpoint: Option<simd_json::OwnedValue>,
    pub created_at: String,
}

/// Audit log entry
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub id: i64,
    pub timestamp: String,
    pub plugin_name: String,
    pub operation: String,
    pub data: simd_json::OwnedValue,
    pub footprint_hash: Option<String>,
}

/// Job counts by status
#[derive(Debug, Clone, Default)]
pub struct JobCounts {
    pub total: u64,
    pub pending: u64,
    pub running: u64,
    pub completed: u64,
    pub failed: u64,
}

/// Store statistics
#[derive(Debug, Clone)]
pub struct StoreStats {
    pub jobs: JobCounts,
    pub checkpoints: u64,
    pub audit_entries: u64,
    pub plugin_states: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_job::ExecutionResult;

    #[tokio::test]
    async fn test_sqlite_store_job_lifecycle() {
        let store = SqliteStore::in_memory().await.unwrap();

        // Create a job
        let job = ExecutionJob {
            id: Uuid::new_v4(),
            tool_name: "test_tool".to_string(),
            arguments: simd_json::json!({"key": "value"}),
            status: ExecutionStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            result: None,
        };

        // Save
        store.save_job(&job).await.unwrap();

        // Get
        let retrieved = store.get_job(job.id).await.unwrap().unwrap();
        assert_eq!(retrieved.id, job.id);
        assert_eq!(retrieved.tool_name, "test_tool");
        assert_eq!(retrieved.status, ExecutionStatus::Pending);

        // Update
        let mut updated_job = job.clone();
        updated_job.status = ExecutionStatus::Completed;
        updated_job.result = Some(ExecutionResult {
            success: true,
            output: Some(simd_json::json!({"result": "ok"})),
            error: None,
        });
        store.update_job(&updated_job).await.unwrap();

        // Verify update
        let retrieved = store.get_job(job.id).await.unwrap().unwrap();
        assert_eq!(retrieved.status, ExecutionStatus::Completed);
        assert!(retrieved.result.is_some());
    }

    #[tokio::test]
    async fn test_sqlite_store_plugin_state() {
        let store = SqliteStore::in_memory().await.unwrap();

        let state = simd_json::json!({
            "containers": [
                {"id": "100", "status": "running"}
            ]
        });

        // Save
        store.save_plugin_state("lxc", &state).await.unwrap();

        // Get
        let retrieved = store.get_plugin_state("lxc").await.unwrap().unwrap();
        assert_eq!(retrieved, state);

        // Update
        let new_state = simd_json::json!({
            "containers": [
                {"id": "100", "status": "stopped"},
                {"id": "101", "status": "running"}
            ]
        });
        store.save_plugin_state("lxc", &new_state).await.unwrap();

        let retrieved = store.get_plugin_state("lxc").await.unwrap().unwrap();
        assert_eq!(retrieved, new_state);
    }

    #[tokio::test]
    async fn test_sqlite_store_checkpoints() {
        let store = SqliteStore::in_memory().await.unwrap();

        let state = simd_json::json!({"key": "value"});

        // Save checkpoint
        store
            .save_checkpoint("cp-1", "test_plugin", 1000, &state, None)
            .await
            .unwrap();

        // Get checkpoint
        let cp = store.get_checkpoint("cp-1").await.unwrap().unwrap();
        assert_eq!(cp.plugin_name, "test_plugin");
        assert_eq!(cp.timestamp, 1000);

        // Get latest
        store
            .save_checkpoint("cp-2", "test_plugin", 2000, &state, None)
            .await
            .unwrap();

        let latest = store
            .get_latest_checkpoint("test_plugin")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(latest.id, "cp-2");
    }

    #[tokio::test]
    async fn test_sqlite_store_audit_log() {
        let store = SqliteStore::in_memory().await.unwrap();

        // Log entries
        store
            .log_audit("plugin1", "create", &simd_json::json!({"id": "1"}), None)
            .await
            .unwrap();
        store
            .log_audit(
                "plugin1",
                "update",
                &simd_json::json!({"id": "1"}),
                Some("abc123"),
            )
            .await
            .unwrap();
        store
            .log_audit("plugin2", "delete", &simd_json::json!({"id": "2"}), None)
            .await
            .unwrap();

        // Get all
        let entries = store.get_audit_log(None, 10).await.unwrap();
        assert_eq!(entries.len(), 3);

        // Get for plugin1
        let entries = store.get_audit_log(Some("plugin1"), 10).await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_sqlite_store_stats() {
        let store = SqliteStore::in_memory().await.unwrap();

        // Create some jobs
        for i in 0..5 {
            let job = ExecutionJob {
                id: Uuid::new_v4(),
                tool_name: format!("tool_{}", i),
                arguments: simd_json::json!({}),
                status: if i % 2 == 0 {
                    ExecutionStatus::Completed
                } else {
                    ExecutionStatus::Failed
                },
                created_at: Utc::now(),
                updated_at: Utc::now(),
                result: None,
            };
            store.save_job(&job).await.unwrap();
        }

        let stats = store.get_stats().await.unwrap();
        assert_eq!(stats.jobs.total, 5);
        assert_eq!(stats.jobs.completed, 3);
        assert_eq!(stats.jobs.failed, 2);
    }
}
