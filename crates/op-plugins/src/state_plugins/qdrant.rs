//! Qdrant vector search engine plugin — GB.Qdrant.
//!
//! Schema derived from Qdrant's official OpenAPI spec
//! (qdrant/qdrant/docs/redoc/master/openapi.json). Fields mirror the
//! `CollectionInfo`, `CollectionConfig`, `VectorParams`, `HnswConfig`,
//! `OptimizersConfig`, and `CollectionStatus` schemas defined there.

use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, AckOutput};
use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use op_state_store::{PluginSchema, SideEffect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

// =============================================================================
// PLUGIN ENTRY: identity and typed schema seed
// =============================================================================

const PLUGIN_NAME: &str = "qdrant";
const PLUGIN_VERSION: &str = "1.0.0";
const PLUGIN_CATEGORY: &str = "data";
const PLUGIN_DESCRIPTION: &str =
    "Qdrant vector search engine — collections, vector config, HNSW, optimizers";
const PLUGIN_DISPLAY_NAME: &str = "GB.Qdrant";

const DEFAULT_QDRANT_HTTP_ENDPOINT: &str = "http://10.200.0.2:6333";
const DEFAULT_QDRANT_GRPC_ENDPOINT: &str = "http://127.0.0.1:6334";

// ── State structs with schemars::JsonSchema ──────────────────────────────────

/// Qdrant collection info — mirrors `CollectionInfo` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QdrantCollectionInfo {
    pub name: String,
    pub status: String,
    pub optimizer_status: String,
    #[serde(default)]
    pub points_count: Option<u64>,
    #[serde(default)]
    pub indexed_vectors_count: Option<u64>,
    pub segments_count: u64,
    pub config: QdrantCollectionConfig,
    pub payload_schema: serde_json::Value,
}

/// Mirrors `CollectionConfig` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QdrantCollectionConfig {
    pub params: QdrantCollectionParams,
    pub hnsw_config: QdrantHnswConfig,
    pub optimizer_config: QdrantOptimizersConfig,
    #[serde(default)]
    pub quantization_config: Option<String>,
    #[serde(default)]
    pub wal_config: Option<serde_json::Value>,
    #[serde(default)]
    pub strict_mode_config: Option<serde_json::Value>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Mirrors `CollectionParams` + `VectorsConfig` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QdrantCollectionParams {
    pub vectors: QdrantVectorParams,
    #[serde(default)]
    pub shard_number: Option<u32>,
    #[serde(default)]
    pub replication_factor: Option<u32>,
    #[serde(default)]
    pub write_consistency_factor: Option<u32>,
    #[serde(default)]
    pub read_fan_out_factor: Option<u32>,
    #[serde(default)]
    pub on_disk_payload: Option<bool>,
    #[serde(default)]
    pub sparse_vectors: Option<serde_json::Value>,
}

/// Mirrors `VectorParams` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QdrantVectorParams {
    pub size: u64,
    pub distance: String,
    #[serde(default)]
    pub hnsw_config: Option<serde_json::Value>,
    #[serde(default)]
    pub quantization_config: Option<String>,
    #[serde(default)]
    pub on_disk: Option<bool>,
    #[serde(default)]
    pub datatype: Option<String>,
    #[serde(default)]
    pub multivector_config: Option<serde_json::Value>,
}

/// Mirrors `HnswConfig` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QdrantHnswConfig {
    pub m: u32,
    pub ef_construct: u32,
    pub full_scan_threshold: u32,
    pub max_indexing_threads: u32,
    #[serde(default)]
    pub on_disk: Option<bool>,
    #[serde(default)]
    pub payload_m: Option<u32>,
    #[serde(default)]
    pub m0: Option<u32>,
}

/// Mirrors `OptimizersConfig` from the OpenAPI spec.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QdrantOptimizersConfig {
    pub deleted_threshold: f64,
    pub vacuum_min_vector_number: u64,
    pub default_segment_number: u32,
    pub max_segment_size: u64,
    pub flush_interval_sec: u64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub indexing_threshold: Option<u64>,
}

/// Top-level Qdrant plugin state.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("x-oscal-category" = "data"))]
pub struct QdrantState {
    pub version: String,
    pub title: String,
    pub commit: String,
    pub http_endpoint: String,
    pub grpc_endpoint: String,
    pub collections: Vec<QdrantCollectionInfo>,
    #[serde(default)]
    pub cluster_status: Option<serde_json::Value>,
    #[serde(default)]
    pub telemetry: Option<serde_json::Value>,
    /// Uncapped fields discovered from the authoritative Qdrant sources.
    #[serde(default)]
    pub inspector_fields: inspector_gadget_generated::InspectorGadgetFields,
}

// ── Method input types ───────────────────────────────────────────────────────

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

// ── Method output types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListCollectionsOutput {
    pub collections: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchPointsOutput {
    pub results: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScrollPointsOutput {
    pub points: Vec<serde_json::Value>,
    pub next_page_offset: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListSnapshotsOutput {
    pub snapshots: Vec<String>,
}

// =============================================================================
// PLUGIN BODY: D-Bus-backed behavior only
// =============================================================================

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

    pub fn exemplar_state() -> QdrantState {
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
                payload_schema: serde_json::json!({}),
            }],
            cluster_status: None,
            telemetry: None,
            inspector_fields: inspector_gadget_generated::InspectorGadgetFields::default(),
        }
    }

    /// Preferred HTTP base for mutation dispatch (fabric first, then env default).
    pub fn resolved_http_endpoint() -> String {
        let configured = Self::http_endpoint();
        if !configured.is_empty() {
            return configured;
        }
        DEFAULT_QDRANT_HTTP_ENDPOINT.to_string()
    }
}

/// Mutation-path dispatch for qdrant. Never echo — live HTTP or fail closed.
pub async fn dispatch_qdrant_method(
    method: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let base = QdrantPlugin::resolved_http_endpoint()
        .trim_end_matches('/')
        .to_string();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .context("qdrant http client")?;

    match method {
        "list_collections" => {
            let url = format!("{base}/collections");
            let body: serde_json::Value = client
                .get(&url)
                .send()
                .await
                .with_context(|| format!("GET {url}"))?
                .error_for_status()
                .with_context(|| format!("GET {url} status"))?
                .json()
                .await
                .context("decode /collections")?;
            let names: Vec<String> = body
                .pointer("/result/collections")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|c| c.get("name").and_then(|n| n.as_str()).map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            Ok(serde_json::to_value(ListCollectionsOutput {
                collections: names,
            })?)
        }
        "create_collection" => {
            let input: CreateCollectionInput =
                serde_json::from_value(args.clone()).context("invalid create_collection args")?;
            let url = format!("{base}/collections/{}", input.name);
            let resp = client
                .put(&url)
                .json(&serde_json::json!({ "vectors": input.vectors_config }))
                .send()
                .await
                .with_context(|| format!("PUT {url}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("create_collection failed ({status}): {text}");
            }
            Ok(serde_json::to_value(AckOutput { success: true })?)
        }
        "delete_collection" => {
            let input: DeleteCollectionInput =
                serde_json::from_value(args.clone()).context("invalid delete_collection args")?;
            let url = format!("{base}/collections/{}", input.name);
            let resp = client
                .delete(&url)
                .send()
                .await
                .with_context(|| format!("DELETE {url}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("delete_collection failed ({status}): {text}");
            }
            Ok(serde_json::to_value(AckOutput { success: true })?)
        }
        "upsert_points" => {
            let input: UpsertPointsInput =
                serde_json::from_value(args.clone()).context("invalid upsert_points args")?;
            let url = format!(
                "{base}/collections/{}/points?wait=true",
                input.collection_name
            );
            let resp = client
                .put(&url)
                .json(&serde_json::json!({ "points": input.points }))
                .send()
                .await
                .with_context(|| format!("PUT {url}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("upsert_points failed ({status}): {text}");
            }
            Ok(serde_json::to_value(AckOutput { success: true })?)
        }
        "search_points" => {
            let input: SearchPointsInput =
                serde_json::from_value(args.clone()).context("invalid search_points args")?;
            let url = format!("{base}/collections/{}/points/search", input.collection_name);
            let body: serde_json::Value = client
                .post(&url)
                .json(&serde_json::json!({
                    "vector": input.vector,
                    "limit": input.limit,
                    "with_payload": true,
                }))
                .send()
                .await
                .with_context(|| format!("POST {url}"))?
                .error_for_status()
                .with_context(|| format!("POST {url} status"))?
                .json()
                .await
                .context("decode search")?;
            let results = body
                .get("result")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            let results = results.as_array().cloned().unwrap_or_default();
            Ok(serde_json::to_value(SearchPointsOutput { results })?)
        }
        "delete_points" => {
            let input: DeletePointsInput =
                serde_json::from_value(args.clone()).context("invalid delete_points args")?;
            let url = format!(
                "{base}/collections/{}/points/delete?wait=true",
                input.collection_name
            );
            let resp = client
                .post(&url)
                .json(&serde_json::json!({ "points": input.points }))
                .send()
                .await
                .with_context(|| format!("POST {url}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("delete_points failed ({status}): {text}");
            }
            Ok(serde_json::to_value(AckOutput { success: true })?)
        }
        "scroll_points" => {
            let input: ScrollPointsInput =
                serde_json::from_value(args.clone()).context("invalid scroll_points args")?;
            let url = format!("{base}/collections/{}/points/scroll", input.collection_name);
            let body: serde_json::Value = client
                .post(&url)
                .json(&serde_json::json!({
                    "limit": input.limit,
                    "with_payload": true,
                    "with_vector": false,
                }))
                .send()
                .await
                .with_context(|| format!("POST {url}"))?
                .error_for_status()
                .with_context(|| format!("POST {url} status"))?
                .json()
                .await
                .context("decode scroll")?;
            let result = body.get("result").cloned().unwrap_or(serde_json::json!({}));
            Ok(serde_json::to_value(ScrollPointsOutput {
                points: result
                    .get("points")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default(),
                next_page_offset: result.get("next_page_offset").cloned(),
            })?)
        }
        "create_snapshot" => {
            let input: CreateSnapshotInput =
                serde_json::from_value(args.clone()).context("invalid create_snapshot args")?;
            let url = format!("{base}/collections/{}/snapshots", input.collection_name);
            let resp = client
                .post(&url)
                .send()
                .await
                .with_context(|| format!("POST {url}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("create_snapshot failed ({status}): {text}");
            }
            Ok(serde_json::to_value(AckOutput { success: true })?)
        }
        "list_snapshots" => {
            let input: ListSnapshotsInput =
                serde_json::from_value(args.clone()).context("invalid list_snapshots args")?;
            let url = format!("{base}/collections/{}/snapshots", input.collection_name);
            let body: serde_json::Value = client
                .get(&url)
                .send()
                .await
                .with_context(|| format!("GET {url}"))?
                .error_for_status()
                .with_context(|| format!("GET {url} status"))?
                .json()
                .await
                .context("decode snapshots")?;
            let snapshots: Vec<String> = body
                .get("result")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|s| s.get("name").and_then(|n| n.as_str()).map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            Ok(serde_json::to_value(ListSnapshotsOutput { snapshots })?)
        }
        "create_payload_index" => {
            let input: CreatePayloadIndexInput = serde_json::from_value(args.clone())
                .context("invalid create_payload_index args")?;
            let url = format!(
                "{base}/collections/{}/index?wait=true",
                input.collection_name
            );
            let resp = client
                .put(&url)
                .json(&serde_json::json!({
                    "field_name": input.field_name,
                    "field_schema": input.field_schema,
                }))
                .send()
                .await
                .with_context(|| format!("PUT {url}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("create_payload_index failed ({status}): {text}");
            }
            Ok(serde_json::to_value(AckOutput { success: true })?)
        }
        other => Err(anyhow::anyhow!(
            "qdrant method '{other}' has no mutation dispatch arm"
        )),
    }
}

#[async_trait]
impl StatePlugin for QdrantPlugin {
    fn name(&self) -> &str {
        PLUGIN_NAME
    }

    fn version(&self) -> &str {
        PLUGIN_VERSION
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = qdrant_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: PLUGIN_NAME.to_string(),
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
            plugin: PLUGIN_NAME.to_string(),
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

// =============================================================================
// PLUGIN EXIT: publish the single PluginSchema contract
// =============================================================================

pub(crate) fn qdrant_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(QdrantState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        PLUGIN_NAME,
        PLUGIN_VERSION,
        PLUGIN_DESCRIPTION,
        &root,
    );
    schema.category = PLUGIN_CATEGORY.to_string();
    if let Ok(defaults) = simd_json::serde::to_owned_value(QdrantPlugin::exemplar_state()) {
        super::schemars_adapter::apply_state_defaults(&mut schema, &defaults);
    }
    schema.display_name = Some(PLUGIN_DISPLAY_NAME.to_string());
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

    schema.methods.insert(
        "create_collection".to_string(),
        method_decl_from_schemars_with_output::<CreateCollectionInput, AckOutput>(
            "create_collection",
            SideEffect::Mutation,
            false,
            "cap.data.qdrant.collection.create@v1",
            "mut.data.qdrant.collection.create@v1",
        ),
    );
    schema.methods.insert(
        "delete_collection".to_string(),
        method_decl_from_schemars_with_output::<DeleteCollectionInput, AckOutput>(
            "delete_collection",
            SideEffect::Mutation,
            false,
            "cap.data.qdrant.collection.delete@v1",
            "mut.data.qdrant.collection.delete@v1",
        ),
    );
    schema.methods.insert(
        "list_collections".to_string(),
        method_decl_from_schemars_with_output::<ListCollectionsInput, ListCollectionsOutput>(
            "list_collections",
            SideEffect::Read,
            true,
            "cap.data.qdrant.collection.list@v1",
            "obs.data.qdrant.collection.list@v1",
        ),
    );
    schema.methods.insert(
        "upsert_points".to_string(),
        method_decl_from_schemars_with_output::<UpsertPointsInput, AckOutput>(
            "upsert_points",
            SideEffect::Mutation,
            false,
            "cap.data.qdrant.point.upsert@v1",
            "mut.data.qdrant.point.upsert@v1",
        ),
    );
    schema.methods.insert(
        "search_points".to_string(),
        method_decl_from_schemars_with_output::<SearchPointsInput, SearchPointsOutput>(
            "search_points",
            SideEffect::Read,
            true,
            "cap.data.qdrant.point.search@v1",
            "obs.data.qdrant.point.search@v1",
        ),
    );
    schema.methods.insert(
        "delete_points".to_string(),
        method_decl_from_schemars_with_output::<DeletePointsInput, AckOutput>(
            "delete_points",
            SideEffect::Mutation,
            false,
            "cap.data.qdrant.point.delete@v1",
            "mut.data.qdrant.point.delete@v1",
        ),
    );
    schema.methods.insert(
        "scroll_points".to_string(),
        method_decl_from_schemars_with_output::<ScrollPointsInput, ScrollPointsOutput>(
            "scroll_points",
            SideEffect::Read,
            true,
            "cap.data.qdrant.point.scroll@v1",
            "obs.data.qdrant.point.scroll@v1",
        ),
    );
    schema.methods.insert(
        "create_snapshot".to_string(),
        method_decl_from_schemars_with_output::<CreateSnapshotInput, AckOutput>(
            "create_snapshot",
            SideEffect::Mutation,
            false,
            "cap.data.qdrant.snapshot.create@v1",
            "mut.data.qdrant.snapshot.create@v1",
        ),
    );
    schema.methods.insert(
        "list_snapshots".to_string(),
        method_decl_from_schemars_with_output::<ListSnapshotsInput, ListSnapshotsOutput>(
            "list_snapshots",
            SideEffect::Read,
            true,
            "cap.data.qdrant.snapshot.list@v1",
            "obs.data.qdrant.snapshot.list@v1",
        ),
    );
    schema.methods.insert(
        "create_payload_index".to_string(),
        method_decl_from_schemars_with_output::<CreatePayloadIndexInput, AckOutput>(
            "create_payload_index",
            SideEffect::Mutation,
            false,
            "cap.data.qdrant.payload-index.create@v1",
            "mut.data.qdrant.payload-index.create@v1",
        ),
    );

    schema
}

inventory::submit! {
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new(QdrantPlugin::new()))
}

// ── Inspector Gadget + Repomix generated candidates ───────────────────────
// Generated against PLUGIN-RENDER-CONTRACT.md. The original plugin above is
// preserved. Review ownership, concrete types, defaults, side effects, and
// runtime dispatch before flattening these candidates into the live state/schema.
#[allow(dead_code)]
mod inspector_gadget_generated {
    use serde::{Deserialize, Serialize};

    /// Repomix-discovered fields not represented by the input plugin.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    #[schemars(extend("x-oscal-subid" = "sch.software.qdrant.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `struct.rs.CollectionPath.collection_name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.qdrant.collection-name@v1"))]
        pub collection_name: Option<String>,

        /// Discovered from Repomix path `struct.rs.CollectionShardPath.shard`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.qdrant.shard@v1"))]
        pub shard: Option<String>,

        /// Discovered from Repomix path `struct.rs.CollectionShardSnapshotPath.snapshot`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.qdrant.snapshot@v1"))]
        pub snapshot: Option<String>,

        /// Discovered from Repomix path `struct.rs.CollectionSnapshotPath.snapshot_name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.qdrant.snapshot-name@v1"))]
        pub snapshot_name: Option<String>,

        /// Discovered from Repomix path `struct.rs.InternalUpdateResult.op_num`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.qdrant.op-num@v1"))]
        pub op_num: Option<String>,
    }

    /// Metadata needed when promoting a generated typed method into `schema.methods`.
    pub struct MethodCandidate {
        pub name: &'static str,
        pub side_effect: &'static str,
        pub idempotent: bool,
        pub required_capability: &'static str,
        pub subid: &'static str,
        pub repomix_path: &'static str,
        pub command: &'static [&'static str],
    }

    pub const METHOD_CANDIDATES: &[MethodCandidate] = &[];

    /// Promote every generated method into the sealed plugin schema.
    pub(super) fn register_methods(schema: &mut op_state_store::PluginSchema) {
        use super::super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    }
}

// Promotion checklist (Fable contract):
// 1. Move owned fields into the plugin State struct with concrete Rust types.
// 2. Replace method placeholders with dedicated typed Input/Output fields.
// 3. Register with method_decl_from_schemars_with_output and correct SideEffect.
// 4. Register every subid, implement dispatch, and add schema/subid tests.
// 5. Re-run op-plugin-lint; only then replace the original plugin file.
