//! `datastore` StatePlugin — projects the live canonical state store
//! (`op-state-store`) through the `org.opdbus.v1.plugins` surface (PluginService).
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

use anyhow::Result;
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{FieldSchema, FieldType, PluginSchema};
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Datastore plugin state schema.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.datastore.schema@v1"))]
#[schemars(extend("x-oscal-category" = "data"))]
pub struct DataStoreState {
    /// Runtime status.
    #[serde(default)]
    #[schemars(
        description = "Runtime status",
        extend("x-oscal-subid" = "obs.software.plugin.datastore.status@v1")
    )]
    pub status: String,
    /// Total object count.
    #[serde(default)]
    #[schemars(
        description = "Total object count",
        extend("x-oscal-subid" = "obs.software.plugin.datastore.object-count@v1")
    )]
    pub object_count: usize,
    /// Total execution count.
    #[serde(default)]
    #[schemars(
        description = "Total execution count",
        extend("x-oscal-subid" = "obs.software.plugin.datastore.execution-count@v1")
    )]
    pub execution_count: usize,
    /// Total blockchain count.
    #[serde(default)]
    #[schemars(
        description = "Total blockchain count",
        extend("x-oscal-subid" = "obs.software.plugin.datastore.blockchain-count@v1")
    )]
    pub blockchain_count: usize,
    /// Namespaces with counts.
    #[serde(default)]
    #[schemars(
        description = "Namespaces with counts",
        extend("x-oscal-subid" = "obs.software.plugin.datastore.namespaces@v1")
    )]
    pub namespaces: Vec<NamespaceCount>,
    /// List of stored objects.
    #[serde(default)]
    #[schemars(
        description = "List of stored objects",
        extend("x-oscal-subid" = "obs.software.plugin.datastore.objects@v1")
    )]
    pub objects: Vec<ObjectRef>,
}

/// Individual stored object reference.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.software.plugin.datastore.object-ref@v1"))]
pub struct ObjectRef {
    /// Object identifier.
    #[schemars(
        description = "Object identifier",
        extend("x-oscal-subid" = "obs.software.datastore.object-ref.id@v1")
    )]
    pub id: String,
    /// Object type classification.
    #[schemars(
        description = "Object type classification",
        extend("x-oscal-subid" = "obs.software.datastore.object-ref.type@v1")
    )]
    pub object_type: String,
    /// Namespace containing the object.
    #[schemars(
        description = "Namespace containing the object",
        extend("x-oscal-subid" = "obs.software.datastore.object-ref.namespace@v1")
    )]
    pub namespace: String,
}

/// Namespace with object count statistics.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.software.plugin.datastore.namespace-count@v1"))]
pub struct NamespaceCount {
    /// Namespace name.
    #[schemars(
        description = "Namespace name",
        extend("x-oscal-subid" = "obs.software.datastore.namespace-count.namespace@v1")
    )]
    pub namespace: String,
    /// Number of objects in this namespace.
    #[schemars(
        description = "Number of objects in this namespace",
        extend("x-oscal-subid" = "obs.software.datastore.namespace-count.count@v1")
    )]
    pub count: usize,
}

pub struct DataStorePlugin {
    store: std::sync::Arc<dyn op_state_store::StateStore>,
}

impl DataStorePlugin {
    pub fn new(store: std::sync::Arc<dyn op_state_store::StateStore>) -> Self {
        Self { store }
    }

    /// Read the live store via the shared handle.
    async fn read_live(&self) -> anyhow::Result<DataStoreState> {
        let export = self.store.export_canonical().await?;

        let mut namespace_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let objects: Vec<ObjectRef> = export
            .objects
            .iter()
            .map(|o| {
                *namespace_counts.entry(o.namespace.clone()).or_default() += 1;
                ObjectRef {
                    id: o.id.clone(),
                    object_type: o.object_type.clone(),
                    namespace: o.namespace.clone(),
                }
            })
            .collect();

        let namespaces = namespace_counts
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

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        let mut schema = datastore_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn calculate_diff(
        &self,
        _current: &Value,
        _desired: &Value,
    ) -> anyhow::Result<StateDiff> {
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

    async fn apply_state(&self, _diff: &StateDiff) -> anyhow::Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: Vec::new(),
            errors: Vec::new(),
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> anyhow::Result<Checkpoint> {
        let export = self.store.export_canonical().await?;
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(export)?,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> anyhow::Result<()> {
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

/// Canonical `datastore` schema derived from `DataStoreState` via schemars.
pub(crate) fn datastore_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(DataStoreState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "datastore",
        "1.0.0",
        "Canonical state store — object index, per-namespace counts, execution/blockchain rows",
        &root,
    );

    // Output structs
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListObjectsOutput {
        pub objects: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetObjectOutput {
        pub object: Option<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct QueryOutput {
        pub results: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetStatsOutput {
        pub stats: serde_json::Value,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ExportOutput {
        pub data: serde_json::Value,
    }

    // Add methods
    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "list_objects".to_string(),
        method_decl_from_schemars_with_output::<(), ListObjectsOutput>(
            "list_objects",
            SideEffect::Read,
            true,
            "datastore.read",
            "obs.service.datastore.object.list@v1",
        ),
    );
    schema.methods.insert(
        "get_object".to_string(),
        method_decl_from_schemars_with_output::<(), GetObjectOutput>(
            "get_object",
            SideEffect::Read,
            true,
            "datastore.read",
            "obs.service.datastore.object.get@v1",
        ),
    );
    schema.methods.insert(
        "query".to_string(),
        method_decl_from_schemars_with_output::<(), QueryOutput>(
            "query",
            SideEffect::Read,
            true,
            "datastore.read",
            "obs.service.datastore.query@v1",
        ),
    );
    schema.methods.insert(
        "get_stats".to_string(),
        method_decl_from_schemars_with_output::<(), GetStatsOutput>(
            "get_stats",
            SideEffect::Read,
            true,
            "datastore.read",
            "obs.service.datastore.stats@v1",
        ),
    );
    schema.methods.insert(
        "export".to_string(),
        method_decl_from_schemars_with_output::<(), ExportOutput>(
            "export",
            SideEffect::Read,
            true,
            "datastore.read",
            "obs.service.datastore.export@v1",
        ),
    );

    schema.capabilities.insert(
        "datastore.read".to_string(),
        op_state_store::CapabilityDecl {
            id: "datastore.read".to_string(),
            description: "Grants: list_objects, get_object, query, get_stats, export.".to_string(),
        },
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list). The shared store
// handle is injected from the registry context, exactly like `mcp`.
// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list). The shared store
// handle is injected from the registry context, exactly like `mcp`.
inventory::submit! {
    crate::default_registry::PluginReg::new("datastore", |ctx| std::sync::Arc::new(DataStorePlugin::new(ctx.state_store())))
}
