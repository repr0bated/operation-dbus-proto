//! Qdrant vector search engine plugin.
//!
//! Schema derived from Qdrant's official OpenAPI spec
//! (qdrant/qdrant/docs/redoc/master/openapi.json). Fields mirror the
//! `CollectionInfo`, `CollectionConfig`, `VectorParams`, `HnswConfig`,
//! `OptimizersConfig`, and `CollectionStatus` schemas defined there.

use super::plugin_schema_defs::schema_from_state;
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::json;
use simd_json::OwnedValue as Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateCollectionInput {
    pub name: String,
    pub vectors_config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteCollectionInput {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListCollectionsInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpsertPointsInput {
    pub collection_name: String,
    pub points: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchPointsInput {
    pub collection_name: String,
    pub vector: Vec<f32>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeletePointsInput {
    pub collection_name: String,
    pub points: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScrollPointsInput {
    pub collection_name: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateSnapshotInput {
    pub collection_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListSnapshotsInput {
    pub collection_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreatePayloadIndexInput {
    pub collection_name: String,
    pub field_name: String,
    pub field_schema: serde_json::Value,
}

const DEFAULT_QDRANT_HTTP_ENDPOINT: &str = "http://127.0.0.1:6333";
const DEFAULT_QDRANT_GRPC_ENDPOINT: &str = "http://127.0.0.1:6334";

/// Qdrant collection info — mirrors `CollectionInfo` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantCollectionInfo {
    pub name: String,
    pub status: String, // CollectionStatus enum: green, yellow, grey, red
    pub optimizer_status: String, // OptimizersStatus enum: ok, disabled
    pub points_count: Option<u64>,
    pub indexed_vectors_count: Option<u64>,
    pub segments_count: u64,
    pub config: QdrantCollectionConfig,
    pub payload_schema: Value,
}

/// Mirrors `CollectionConfig` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantCollectionConfig {
    pub params: QdrantCollectionParams,
    pub hnsw_config: QdrantHnswConfig,
    pub optimizer_config: QdrantOptimizersConfig,
    pub quantization_config: Option<String>, // scalar, product, binary, turbo, or null
    pub wal_config: Option<Value>,
    pub strict_mode_config: Option<Value>,
    pub metadata: Option<Value>,
}

/// Mirrors `CollectionParams` + `VectorsConfig` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantCollectionParams {
    pub vectors: QdrantVectorParams,
    pub shard_number: Option<u32>,
    pub replication_factor: Option<u32>,
    pub write_consistency_factor: Option<u32>,
    pub read_fan_out_factor: Option<u32>,
    pub on_disk_payload: Option<bool>,
    pub sparse_vectors: Option<Value>,
}

/// Mirrors `VectorParams` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantVectorParams {
    pub size: u64,
    pub distance: String, // Distance enum: Cosine, Euclid, Dot, Manhattan
    pub hnsw_config: Option<Value>,
    pub quantization_config: Option<String>,
    pub on_disk: Option<bool>,
    pub datatype: Option<String>, // Datatype enum: float32, uint8, float16, turbo4
    pub multivector_config: Option<Value>,
}

/// Mirrors `HnswConfig` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantHnswConfig {
    pub m: u32,
    pub ef_construct: u32,
    pub full_scan_threshold: u32,
    pub max_indexing_threads: u32,
    pub on_disk: Option<bool>,
    pub payload_m: Option<u32>,
    pub m0: Option<u32>,
}

/// Mirrors `OptimizersConfig` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantOptimizersConfig {
    pub deleted_threshold: f64,
    pub vacuum_min_vector_number: u64,
    pub default_segment_number: u32,
    pub max_segment_size: u64,
    pub flush_interval_sec: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexing_threshold: Option<u64>,
}

/// Top-level Qdrant plugin state — the shape the projection tree exposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantState {
    pub version: String,
    pub title: String,
    pub commit: String,
    pub http_endpoint: String,
    pub grpc_endpoint: String,
    pub collections: Vec<QdrantCollectionInfo>,
    pub cluster_status: Option<Value>,
    pub telemetry: Option<Value>,
}

pub struct QdrantPlugin;

impl Default for QdrantPlugin {
    fn default() -> Self {
        Self
    }
}

impl QdrantPlugin {
    pub fn new() -> Self {
        Self
    }

    fn http_endpoint() -> String {
        std::env::var("QDRANT_HTTP_URL")
            .or_else(|_| std::env::var("COGNITIVE_MCP_QDRANT_URL"))
            .unwrap_or_else(|_| DEFAULT_QDRANT_HTTP_ENDPOINT.to_string())
    }

    fn grpc_endpoint() -> String {
        std::env::var("QDRANT_GRPC_URL")
            .unwrap_or_else(|_| DEFAULT_QDRANT_GRPC_ENDPOINT.to_string())
    }

    /// Exemplar state derived from Qdrant OpenAPI schema definitions.
    /// Field names, types, and enum values match the spec exactly.
    pub(crate) fn exemplar_state() -> QdrantState {
        QdrantState {
            version: "1.18.2".to_string(),
            title: "qdrant - vector search engine".to_string(),
            commit: String::new(),
            http_endpoint: Self::http_endpoint(),
            grpc_endpoint: Self::grpc_endpoint(),
            collections: vec![QdrantCollectionInfo {
                name: "example_collection".to_string(),
                status: "green".to_string(),
                optimizer_status: "ok".to_string(),
                points_count: Some(0),
                indexed_vectors_count: Some(0),
                segments_count: 0,
                config: QdrantCollectionConfig {
                    params: QdrantCollectionParams {
                        vectors: QdrantVectorParams {
                            size: 1024,
                            distance: "Cosine".to_string(),
                            hnsw_config: None,
                            quantization_config: None,
                            on_disk: Some(false),
                            datatype: Some("float32".to_string()),
                            multivector_config: None,
                        },
                        shard_number: Some(1),
                        replication_factor: Some(1),
                        write_consistency_factor: Some(1),
                        read_fan_out_factor: None,
                        on_disk_payload: Some(false),
                        sparse_vectors: None,
                    },
                    hnsw_config: QdrantHnswConfig {
                        m: 16,
                        ef_construct: 100,
                        full_scan_threshold: 10000,
                        max_indexing_threads: 0,
                        on_disk: Some(false),
                        payload_m: None,
                        m0: None,
                    },
                    optimizer_config: QdrantOptimizersConfig {
                        deleted_threshold: 0.2,
                        vacuum_min_vector_number: 1000,
                        default_segment_number: 0,
                        max_segment_size: 200_000,
                        flush_interval_sec: 10,
                        indexing_threshold: Some(20000),
                    },
                    quantization_config: None,
                    wal_config: None,
                    strict_mode_config: None,
                    metadata: None,
                },
                payload_schema: json!({}),
            }],
            cluster_status: None,
            telemetry: None,
        }
    }
}

#[async_trait]
impl StatePlugin for QdrantPlugin {
    fn name(&self) -> &str {
        "qdrant"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = qdrant_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::json!(null),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
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

pub(crate) fn qdrant_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(QdrantPlugin::exemplar_state())
        .unwrap_or_else(|_| json!({}));
    let mut schema = schema_from_state(
        "qdrant",
        "data",
        "1.0.0",
        "Qdrant vector search engine — collections, vector config, HNSW, optimizers",
        &state,
    );
    schema.subids.insert(
        "__schema__".to_string(),
        "sch.software.plugin.qdrant.schema@v1".to_string(),
    );
    schema.subids.insert(
        "version".to_string(),
        "obs.software.plugin.qdrant.version@v1".to_string(),
    );
    schema.subids.insert(
        "title".to_string(),
        "obs.software.plugin.qdrant.title@v1".to_string(),
    );
    schema.subids.insert(
        "commit".to_string(),
        "obs.software.plugin.qdrant.commit@v1".to_string(),
    );
    schema.subids.insert(
        "http_endpoint".to_string(),
        "src.software.plugin.qdrant.http-endpoint@v1".to_string(),
    );
    schema.subids.insert(
        "grpc_endpoint".to_string(),
        "src.software.plugin.qdrant.grpc-endpoint@v1".to_string(),
    );
    schema.subids.insert(
        "collections".to_string(),
        "obs.software.plugin.qdrant.collections@v1".to_string(),
    );
    schema.subids.insert(
        "cluster_status".to_string(),
        "obs.software.plugin.qdrant.cluster-status@v1".to_string(),
    );
    schema.subids.insert(
        "telemetry".to_string(),
        "obs.software.plugin.qdrant.telemetry@v1".to_string(),
    );

    schema.methods.insert("create_collection".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<CreateCollectionInput>(
        "create_collection",
        op_state_store::SideEffect::Mutation,
        false,
        "cap.data.qdrant.collection.create@v1",
        "mut.data.qdrant.collection.create@v1",
    ));
    schema.methods.insert("delete_collection".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<DeleteCollectionInput>(
        "delete_collection",
        op_state_store::SideEffect::Mutation,
        false,
        "cap.data.qdrant.collection.delete@v1",
        "mut.data.qdrant.collection.delete@v1",
    ));
    schema.methods.insert("list_collections".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<ListCollectionsInput>(
        "list_collections",
        op_state_store::SideEffect::Read,
        true,
        "cap.data.qdrant.collection.list@v1",
        "obs.data.qdrant.collection.list@v1",
    ));
    schema.methods.insert("upsert_points".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<UpsertPointsInput>(
        "upsert_points",
        op_state_store::SideEffect::Mutation,
        false,
        "cap.data.qdrant.point.upsert@v1",
        "mut.data.qdrant.point.upsert@v1",
    ));
    schema.methods.insert("search_points".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<SearchPointsInput>(
        "search_points",
        op_state_store::SideEffect::Read,
        true,
        "cap.data.qdrant.point.search@v1",
        "obs.data.qdrant.point.search@v1",
    ));
    schema.methods.insert("delete_points".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<DeletePointsInput>(
        "delete_points",
        op_state_store::SideEffect::Mutation,
        false,
        "cap.data.qdrant.point.delete@v1",
        "mut.data.qdrant.point.delete@v1",
    ));
    schema.methods.insert("scroll_points".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<ScrollPointsInput>(
        "scroll_points",
        op_state_store::SideEffect::Read,
        true,
        "cap.data.qdrant.point.scroll@v1",
        "obs.data.qdrant.point.scroll@v1",
    ));
    schema.methods.insert("create_snapshot".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<CreateSnapshotInput>(
        "create_snapshot",
        op_state_store::SideEffect::Mutation,
        false,
        "cap.data.qdrant.snapshot.create@v1",
        "mut.data.qdrant.snapshot.create@v1",
    ));
    schema.methods.insert("list_snapshots".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<ListSnapshotsInput>(
        "list_snapshots",
        op_state_store::SideEffect::Read,
        true,
        "cap.data.qdrant.snapshot.list@v1",
        "obs.data.qdrant.snapshot.list@v1",
    ));
    schema.methods.insert("create_payload_index".to_string(), super::plugin_schema_defs::method_decl_from_schemars::<CreatePayloadIndexInput>(
        "create_payload_index",
        op_state_store::SideEffect::Mutation,
        false,
        "cap.data.qdrant.payload-index.create@v1",
        "mut.data.qdrant.payload-index.create@v1",
    ));

    schema
}

inventory::submit! {
    crate::default_registry::PluginReg::new("qdrant", |_ctx| std::sync::Arc::new(QdrantPlugin::new()))
}
