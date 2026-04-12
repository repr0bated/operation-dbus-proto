use crate::error::Result;
use crate::execution_job::ExecutionJob;
use crate::{CanonicalDbExport, StoredObject};
use async_trait::async_trait;
use uuid::Uuid;

/// Tool record from database
#[derive(Debug, Clone)]
pub struct ToolRecord {
    pub tool_name: String,
    pub definition_json: String, // Serialized ToolDefinition
    pub category: String,
    pub namespace: String,
    pub schema_version: String, // JSON Schema version
    pub source: String,         // "builtin", "dbus-session.v1", "dbus-system.v1", "mcp", "agent"
    pub created_at: String,
    pub updated_at: String,
}

#[async_trait]
pub trait StateStore: Send + Sync {
    async fn save_job(&self, job: &ExecutionJob) -> Result<()>;
    async fn get_job(&self, id: Uuid) -> Result<Option<ExecutionJob>>;
    async fn update_job(&self, job: &ExecutionJob) -> Result<()>;

    async fn get_object(&self, id: &str) -> Result<Option<StoredObject>>;
    async fn upsert_object(
        &self,
        id: &str,
        object_type: &str,
        namespace: &str,
        data: &simd_json::OwnedValue,
    ) -> Result<()>;
    async fn export_canonical(&self) -> Result<CanonicalDbExport>;

    // Tool persistence (READ on startup, WRITE only on onboarding/upgrade/migration)
    async fn save_tools(&self, tools: Vec<ToolRecord>) -> Result<()>;
    async fn load_tools(&self) -> Result<Vec<ToolRecord>>;
    async fn is_tools_empty(&self) -> Result<bool>;
    async fn clear_tools(&self) -> Result<()>;
}

/// In-memory implementation of StateStore.
///
/// Used when SQL is disabled or for testing.
pub struct MemoryStore {
    jobs: tokio::sync::RwLock<std::collections::HashMap<Uuid, ExecutionJob>>,
    objects: tokio::sync::RwLock<std::collections::HashMap<String, StoredObject>>,
    tools: tokio::sync::RwLock<Vec<ToolRecord>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            jobs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            objects: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            tools: tokio::sync::RwLock::new(Vec::new()),
        }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateStore for MemoryStore {
    async fn save_job(&self, job: &ExecutionJob) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        jobs.insert(job.id, job.clone());
        Ok(())
    }

    async fn get_job(&self, id: Uuid) -> Result<Option<ExecutionJob>> {
        let jobs = self.jobs.read().await;
        Ok(jobs.get(&id).cloned())
    }

    async fn update_job(&self, job: &ExecutionJob) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        jobs.insert(job.id, job.clone());
        Ok(())
    }

    async fn get_object(&self, id: &str) -> Result<Option<StoredObject>> {
        let objects = self.objects.read().await;
        Ok(objects.get(id).cloned())
    }

    async fn upsert_object(
        &self,
        id: &str,
        object_type: &str,
        namespace: &str,
        data: &simd_json::OwnedValue,
    ) -> Result<()> {
        let mut objects = self.objects.write().await;
        objects.insert(
            id.to_string(),
            StoredObject {
                id: id.to_string(),
                object_type: object_type.to_string(),
                namespace: namespace.to_string(),
                data: data.clone(),
            },
        );
        Ok(())
    }

    async fn export_canonical(&self) -> Result<CanonicalDbExport> {
        let objects = self.objects.read().await;
        let jobs = self.jobs.read().await;

        Ok(CanonicalDbExport {
            objects: objects.values().cloned().collect(),
            executions: jobs
                .values()
                .map(|j| simd_json::json!({ "id": j.id, "status": format!("{:?}", j.status) }))
                .collect(),
            blockchain: Vec::new(),
        })
    }

    async fn save_tools(&self, tools: Vec<ToolRecord>) -> Result<()> {
        let mut t = self.tools.write().await;
        *t = tools;
        Ok(())
    }

    async fn load_tools(&self) -> Result<Vec<ToolRecord>> {
        let t = self.tools.read().await;
        Ok(t.clone())
    }

    async fn is_tools_empty(&self) -> Result<bool> {
        let t = self.tools.read().await;
        Ok(t.is_empty())
    }

    async fn clear_tools(&self) -> Result<()> {
        let mut t = self.tools.write().await;
        t.clear();
        Ok(())
    }
}
