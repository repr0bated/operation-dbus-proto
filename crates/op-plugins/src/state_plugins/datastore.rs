//! `datastore` StatePlugin — projects the live canonical state store
//! (`op-state-store`) through the `org.dbus.v1.plugins` surface (PluginService).
//!
//! This is the correct home for the capability the Lovable frontend mistakenly
//! called as a standalone `operation.stores.v1.DataStoreService` gRPC package —
//! there is no such proto and there must not be one (see OD-30).
//!
//! The read is **live, not mocked**: the plugin's live state calls
//! `StateStore::export_canonical()` on the *same shared store handle* the rest
//! of the process uses (injected at registration via `PluginCtx::state_store`),
//! so there is no second DB open / lock contention. It reports the real object
//! index (id/type/namespace), per-namespace counts and execution/blockchain row
//! counts.
//!
//! The store is mutable, but writes flow through the MutationEngine and the
//! owning plugins (every mutation is an enforcement point). This projection is
//! therefore read-only: it rejects state-diff mutations with an explanation
//! rather than letting arbitrary objects be upserted around the enforcement path.

use super::plugin_schema_defs::schema_from_state;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{PluginSchema, StateStore};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRef {
    pub id: String,
    pub object_type: String,
    pub namespace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceCount {
    pub namespace: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataStoreState {
    pub status: String,
    pub object_count: usize,
    pub execution_count: usize,
    pub blockchain_count: usize,
    pub namespaces: Vec<NamespaceCount>,
    pub objects: Vec<ObjectRef>,
}

pub struct DataStorePlugin {
    store: Arc<dyn StateStore>,
}

impl DataStorePlugin {
    pub fn new(store: Arc<dyn StateStore>) -> Self {
        Self { store }
    }

    /// Read the live store via the shared handle.
    async fn read_live(&self) -> Result<DataStoreState> {
        let export = self.store.export_canonical().await?;

        let mut ns: BTreeMap<String, usize> = BTreeMap::new();
        let objects: Vec<ObjectRef> = export
            .objects
            .iter()
            .map(|o| {
                *ns.entry(o.namespace.clone()).or_default() += 1;
                ObjectRef {
                    id: o.id.clone(),
                    object_type: o.object_type.clone(),
                    namespace: o.namespace.clone(),
                }
            })
            .collect();

        let namespaces = ns
            .into_iter()
            .map(|(namespace, count)| NamespaceCount { namespace, count })
            .collect();

        Ok(DataStoreState {
            status: "active".to_string(),
            object_count: export.objects.len(),
            execution_count: export.executions.len(),
            blockchain_count: export.blockchain.len(),
            namespaces,
            objects,
        })
    }

    /// Synchronous shape exemplar for the schema (the data path is always live).
    fn schema_exemplar() -> DataStoreState {
        DataStoreState {
            status: "active".to_string(),
            object_count: 0,
            execution_count: 0,
            blockchain_count: 0,
            namespaces: vec![NamespaceCount {
                namespace: "default".to_string(),
                count: 0,
            }],
            objects: vec![ObjectRef {
                id: "example".to_string(),
                object_type: "plugin".to_string(),
                namespace: "default".to_string(),
            }],
        }
    }
}

#[async_trait]
impl StatePlugin for DataStorePlugin {
    fn name(&self) -> &str {
        "datastore"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(datastore_schema())
    }


    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        // Writes flow through the MutationEngine / owning plugins, not through a
        // whole-store diff. Always NoOp here.
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![StateAction::NoOp {
                resource: "objects".to_string(),
            }],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut errors = Vec::new();
        for action in &diff.actions {
            match action {
                StateAction::NoOp { .. } => {}
                other => errors.push(format!(
                    "datastore is written through the MutationEngine and owning \
                     plugins (every mutation is an enforcement point); refusing to \
                     apply {:?} directly",
                    other
                )),
            }
        }
        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied: Vec::new(),
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = simd_json::serde::to_owned_value(self.read_live().await?)?;
        Ok(&current == desired)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        // A real, full canonical export of the store.
        let export = self.store.export_canonical().await?;
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(export)?,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        // Restoring the canonical store is an import operation owned by the
        // disaster-recovery path, not this projection.
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

pub(crate) fn datastore_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(DataStorePlugin::schema_exemplar())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "datastore",
        "storage",
        "1.0.0",
        "Canonical state store — object index, per-namespace counts, execution/blockchain rows",
        &state,
    )
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list). The shared store
// handle is injected from the registry context, exactly like `mcp`.
inventory::submit! {
    crate::default_registry::PluginReg::new("datastore", |ctx| std::sync::Arc::new(DataStorePlugin::new(ctx.state_store())))
}
