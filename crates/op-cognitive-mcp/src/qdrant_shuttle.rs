use std::fs::File;
use std::mem::size_of;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use memmap2::MmapOptions;
use op_state_store::{FieldType, PluginSchema};
use qdrant_client::qdrant::{
    Condition, Filter, QueryPointsBuilder, RetrievedPoint, ScoredPoint, ScrollPointsBuilder,
};
use qdrant_client::Qdrant;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;

const DEFAULT_QDRANT_URL: &str = "http://127.0.0.1:6334";
const DEFAULT_COLLECTION_NAME: &str = "ctl_plane_reasoning_episodes";
const DEFAULT_SCHEMA_SLED_PATH: &str = "/dev/shm/plugin_schema.dat";
const DEFAULT_TRACE_LIMIT: u32 = 5;
const DEFAULT_VOYAGE_API_URL: &str = "https://api.voyageai.com/v1/embeddings";
const DEFAULT_VOYAGE_QUERY_MODEL: &str = "voyage-4";
const DEFAULT_VOYAGE_OUTPUT_DIMENSION: u32 = 1024;

/// THE SLED: 1:1 Zero-copy shared memory layout mapping directly to the SchemaEngine.
/// Keep this ABI aligned with the gRPC Ghostbridge interceptor until the workspace
/// converges on a single canonical sled definition.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentitySled {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: bool,
    pub hashed_footprint: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionTraceContext {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub hashed_footprint: [u8; 32],
    pub trace_id: String,
}

pub struct QdrantSemanticShuttle {
    client: Qdrant,
    collection_name: String,
    sled_path: PathBuf,
    voyage_client: VoyageClient,
}

impl QdrantSemanticShuttle {
    /// Initializes the Qdrant gRPC client used by the Accountability Loop.
    pub async fn new() -> Result<Self> {
        let qdrant_url =
            std::env::var("COGNITIVE_MCP_QDRANT_URL").unwrap_or_else(|_| DEFAULT_QDRANT_URL.into());
        let collection_name = std::env::var("COGNITIVE_MCP_QDRANT_COLLECTION")
            .unwrap_or_else(|_| DEFAULT_COLLECTION_NAME.into());
        let sled_path = std::env::var("COGNITIVE_MCP_SCHEMA_SLED_PATH")
            .unwrap_or_else(|_| DEFAULT_SCHEMA_SLED_PATH.into());
        let voyage_client = VoyageClient::from_env()?;

        Self::new_with_clients(&qdrant_url, collection_name, sled_path, voyage_client).await
    }

    async fn new_with_clients(
        qdrant_url: &str,
        collection_name: impl Into<String>,
        sled_path: impl Into<PathBuf>,
        voyage_client: VoyageClient,
    ) -> Result<Self> {
        let collection_name = collection_name.into();
        let sled_path = sled_path.into();
        let client = Qdrant::from_url(qdrant_url)
            .build()
            .with_context(|| format!("failed to build Qdrant client for {qdrant_url}"))?;

        client.health_check().await.with_context(|| {
            format!("failed to reach Qdrant gRPC health endpoint at {qdrant_url}")
        })?;

        tracing::info!(
            qdrant_url,
            collection = %collection_name,
            sled_path = %sled_path.display(),
            "Qdrant Semantic Shuttle linked to the gRPC interface"
        );

        Ok(Self {
            client,
            collection_name,
            sled_path,
            voyage_client,
        })
    }

    /// Reads the active identity sled directly from shared memory.
    pub fn current_trace_context(&self) -> Result<SessionTraceContext> {
        let sled = read_identity_sled(&self.sled_path)?;
        ensure!(
            sled.is_valid,
            "A.N.N.A. Scribe: Invalid Schema State. No active trace available."
        );

        Ok(SessionTraceContext {
            wireguard_pubkey: sled.wireguard_pubkey,
            mutation_index: sled.mutation_index,
            hashed_footprint: sled.hashed_footprint,
            trace_id: format_trace_id(sled.hashed_footprint),
        })
    }

    /// Renders the active appended PluginSchema into deterministic retrieval text.
    pub fn current_schema_embedding_text(&self) -> Result<String> {
        let schema = read_plugin_schema(&self.sled_path)?;
        Ok(render_schema_embedding_text(&schema))
    }

    /// Fetches the exact session episodes currently associated with the active trace.
    pub async fn stream_semantic_trace(&self) -> Result<Vec<RetrievedPoint>> {
        self.fetch_trace_episodes(DEFAULT_TRACE_LIMIT).await
    }

    pub async fn fetch_trace_episodes(&self, limit: u32) -> Result<Vec<RetrievedPoint>> {
        let trace = self.current_trace_context()?;
        let response = self
            .client
            .scroll(
                ScrollPointsBuilder::new(self.collection_name.clone())
                    .filter(Filter::must([Condition::matches(
                        "trace_id",
                        trace.trace_id.clone(),
                    )]))
                    .limit(limit)
                    .with_payload(true)
                    .with_vectors(true),
            )
            .await
            .with_context(|| {
                format!(
                    "failed to query Qdrant collection {} for trace {}",
                    self.collection_name, trace.trace_id
                )
            })?;

        tracing::info!(
            trace_id = %trace.trace_id,
            mutation_index = trace.mutation_index,
            matches = response.result.len(),
            "Accountability Loop fetched semantic trace episodes"
        );

        Ok(response.result)
    }

    /// Performs semantic retrieval within the active trace using a schema-derived Voyage query.
    pub async fn search_semantic_trace(&self, limit: u64) -> Result<Vec<ScoredPoint>> {
        let (trace, schema_query_text) = self.active_schema_query_text()?;
        let limit = if limit == 0 {
            u64::from(DEFAULT_TRACE_LIMIT)
        } else {
            limit
        };

        let embedding = self
            .voyage_client
            .embed_query(&schema_query_text)
            .await
            .context("failed to embed active shared-memory schema with Voyage")?;

        let response = self
            .client
            .query(
                QueryPointsBuilder::new(self.collection_name.clone())
                    .query(embedding)
                    .filter(Filter::must([Condition::matches(
                        "trace_id",
                        trace.trace_id.clone(),
                    )]))
                    .limit(limit)
                    .with_payload(true),
            )
            .await
            .with_context(|| {
                format!(
                    "failed semantic query against Qdrant collection {} for trace {}",
                    self.collection_name, trace.trace_id
                )
            })?;

        tracing::info!(
            trace_id = %trace.trace_id,
            mutation_index = trace.mutation_index,
            schema_name = %extract_schema_title(&schema_query_text),
            matches = response.result.len(),
            model = %self.voyage_client.model,
            "Accountability Loop fetched semantic matches from the shared-memory schema projection"
        );

        Ok(response.result)
    }

    fn active_schema_query_text(&self) -> Result<(SessionTraceContext, String)> {
        let (sled, schema) = read_identity_sled_and_schema(&self.sled_path)?;
        ensure!(
            sled.is_valid,
            "A.N.N.A. Scribe: Invalid Schema State. No active trace available."
        );

        Ok((
            SessionTraceContext {
                wireguard_pubkey: sled.wireguard_pubkey,
                mutation_index: sled.mutation_index,
                hashed_footprint: sled.hashed_footprint,
                trace_id: format_trace_id(sled.hashed_footprint),
            },
            render_schema_embedding_text(&schema),
        ))
    }
}

struct VoyageClient {
    client: Client,
    api_url: String,
    api_key: String,
    model: String,
    output_dimension: u32,
}

impl VoyageClient {
    fn from_env() -> Result<Self> {
        let api_key = std::env::var("COGNITIVE_MCP_VOYAGE_API_KEY")
            .or_else(|_| std::env::var("VOYAGE_API_KEY"))
            .context(
                "missing Voyage API key: set COGNITIVE_MCP_VOYAGE_API_KEY or VOYAGE_API_KEY",
            )?;
        let api_url = std::env::var("COGNITIVE_MCP_VOYAGE_API_URL")
            .unwrap_or_else(|_| DEFAULT_VOYAGE_API_URL.into());
        let model = std::env::var("COGNITIVE_MCP_VOYAGE_QUERY_MODEL")
            .unwrap_or_else(|_| DEFAULT_VOYAGE_QUERY_MODEL.into());
        let output_dimension = std::env::var("COGNITIVE_MCP_VOYAGE_OUTPUT_DIMENSION")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(DEFAULT_VOYAGE_OUTPUT_DIMENSION);

        Ok(Self {
            client: Client::new(),
            api_url,
            api_key,
            model,
            output_dimension,
        })
    }

    async fn embed_query(&self, input: &str) -> Result<Vec<f32>> {
        let body = VoyageEmbeddingRequest {
            input,
            model: &self.model,
            input_type: "query",
            truncation: true,
            output_dimension: self.output_dimension,
            output_dtype: "float",
        };

        let response = self
            .client
            .post(&self.api_url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("failed to call Voyage embeddings API at {}", self.api_url))?
            .error_for_status()
            .context("Voyage embeddings API returned an error status")?;

        let response_json = response
            .json::<Value>()
            .await
            .context("failed to decode Voyage embeddings response")?;

        extract_embedding(&response_json)
    }
}

#[derive(Serialize)]
struct VoyageEmbeddingRequest<'a> {
    input: &'a str,
    model: &'a str,
    input_type: &'a str,
    truncation: bool,
    output_dimension: u32,
    output_dtype: &'a str,
}

fn read_identity_sled(path: &Path) -> Result<IdentitySled> {
    let (sled, _) = read_shared_mapping(path)?;
    Ok(sled)
}

fn read_plugin_schema(path: &Path) -> Result<PluginSchema> {
    let (_, schema_bytes) = read_shared_mapping(path)?;
    parse_plugin_schema(schema_bytes, path)
}

fn read_identity_sled_and_schema(path: &Path) -> Result<(IdentitySled, PluginSchema)> {
    let (sled, schema_bytes) = read_shared_mapping(path)?;
    let schema = parse_plugin_schema(schema_bytes, path)?;
    Ok((sled, schema))
}

fn read_shared_mapping(path: &Path) -> Result<(IdentitySled, Vec<u8>)> {
    let file = File::open(path)
        .with_context(|| format!("failed to open SchemaEngine sled at {}", path.display()))?;
    let mmap = unsafe { MmapOptions::new().map(&file) }
        .with_context(|| format!("failed to mmap SchemaEngine sled at {}", path.display()))?;

    ensure!(
        mmap.len() >= size_of::<IdentitySled>(),
        "SchemaEngine sled at {} is smaller than IdentitySled ABI ({})",
        path.display(),
        size_of::<IdentitySled>()
    );

    let sled_ptr = mmap.as_ptr().cast::<IdentitySled>();
    let sled = unsafe { std::ptr::read_unaligned(sled_ptr) };
    let schema_bytes = mmap[size_of::<IdentitySled>()..]
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .collect::<Vec<_>>();

    Ok((sled, schema_bytes))
}

fn parse_plugin_schema(schema_bytes: Vec<u8>, path: &Path) -> Result<PluginSchema> {
    ensure!(
        !schema_bytes.is_empty(),
        "SchemaEngine sled at {} did not contain appended PluginSchema bytes",
        path.display()
    );

    serde_json::from_slice(&schema_bytes).with_context(|| {
        format!(
            "failed to parse appended PluginSchema from shared memory at {}",
            path.display()
        )
    })
}

fn format_trace_id(hashed_footprint: [u8; 32]) -> String {
    format!("trace-{}", hex::encode(hashed_footprint))
}

fn render_schema_embedding_text(schema: &PluginSchema) -> String {
    let mut lines = vec![
        format!("schema_name: {}", schema.name),
        format!("schema_category: {}", schema.category),
        format!("schema_version: {}", schema.version),
        format!("schema_description: {}", schema.description.trim()),
    ];

    let mut tags = schema.tags.clone();
    tags.sort();
    if !tags.is_empty() {
        lines.push(format!("schema_tags: {}", tags.join(", ")));
    }

    let mut immutable_paths = schema.immutable_paths.clone();
    immutable_paths.sort();
    if !immutable_paths.is_empty() {
        lines.push(format!("immutable_paths: {}", immutable_paths.join(", ")));
    }

    let mut dependencies = schema.dependencies.clone();
    dependencies.sort();
    if !dependencies.is_empty() {
        lines.push(format!("dependencies: {}", dependencies.join(", ")));
    }

    let mut field_names = schema.fields.keys().cloned().collect::<Vec<_>>();
    field_names.sort();

    for field_name in field_names {
        let Some(field_schema) = schema.fields.get(&field_name) else {
            continue;
        };

        lines.push(format!(
            "field {}: type={}, required={}, read_only={}, description={}",
            field_name,
            render_field_type(&field_schema.field_type),
            field_schema.required,
            field_schema.read_only,
            field_schema.description.trim()
        ));

        if let Some(condition) = &field_schema.read_only_when {
            lines.push(format!(
                "field {} read_only_when: {}={}",
                field_name, condition.property, condition.value
            ));
        }

        if !field_schema.constraints.is_empty() {
            let constraints = field_schema
                .constraints
                .iter()
                .map(render_constraint)
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("field {} constraints: {}", field_name, constraints));
        }
    }

    lines.join("\n")
}

fn render_field_type(field_type: &FieldType) -> String {
    match field_type {
        FieldType::String => "string".to_string(),
        FieldType::Integer => "integer".to_string(),
        FieldType::Float => "float".to_string(),
        FieldType::Boolean => "boolean".to_string(),
        FieldType::Array(inner) => format!("array<{}>", render_field_type(inner)),
        FieldType::Object(fields) => {
            let mut names = fields.keys().cloned().collect::<Vec<_>>();
            names.sort();
            format!("object<{}>", names.join("|"))
        }
        FieldType::Enum(values) => format!("enum<{}>", values.join("|")),
        FieldType::Any => "any".to_string(),
    }
}

fn render_constraint(constraint: &op_state_store::Constraint) -> String {
    match constraint {
        op_state_store::Constraint::Min { value } => format!("min={value}"),
        op_state_store::Constraint::Max { value } => format!("max={value}"),
        op_state_store::Constraint::Pattern { regex } => format!("pattern={regex}"),
        op_state_store::Constraint::OneOf { values } => {
            format!(
                "one_of={}",
                serde_json::to_string(values).unwrap_or_default()
            )
        }
        op_state_store::Constraint::RequiresField { field } => format!("requires_field={field}"),
        op_state_store::Constraint::Custom { validator } => format!("custom={validator}"),
    }
}

fn extract_schema_title(schema_query_text: &str) -> &str {
    schema_query_text
        .lines()
        .find_map(|line| line.strip_prefix("schema_name: "))
        .unwrap_or("unknown")
}

fn extract_embedding(response_json: &Value) -> Result<Vec<f32>> {
    let Some(embedding_values) = response_json
        .get("data")
        .and_then(Value::as_array)
        .and_then(|entries| entries.first())
        .and_then(|entry| entry.get("embedding"))
        .and_then(Value::as_array)
        .or_else(|| {
            response_json
                .get("embeddings")
                .and_then(Value::as_array)
                .and_then(|entries| entries.first())
                .and_then(Value::as_array)
        })
    else {
        return Err(anyhow::anyhow!(
            "Voyage embeddings response did not contain a usable embedding"
        ));
    };

    let mut embedding = Vec::with_capacity(embedding_values.len());
    for value in embedding_values {
        let number = value
            .as_f64()
            .context("Voyage embedding contained a non-numeric value")?;
        embedding.push(number as f32);
    }

    ensure!(!embedding.is_empty(), "Voyage embedding response was empty");
    Ok(embedding)
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::{Constraint, FieldSchema, ReadOnlyCondition};
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn should_preserve_identity_sled_abi_shape() {
        assert!(
            size_of::<IdentitySled>() >= 32 + 8 + 1 + 32,
            "IdentitySled ABI unexpectedly shrank"
        );
    }

    #[test]
    fn should_format_trace_id_from_hashed_footprint() {
        let trace_id = format_trace_id([0xAB; 32]);
        assert_eq!(trace_id, format!("trace-{}", "ab".repeat(32)));
    }

    #[test]
    fn should_extract_embedding_from_openai_style_data_payload() {
        let embedding = extract_embedding(&json!({
            "data": [{
                "embedding": [0.25, -0.5, 1.5]
            }]
        }))
        .unwrap();

        assert_eq!(embedding, vec![0.25, -0.5, 1.5]);
    }

    #[test]
    fn should_extract_embedding_from_embeddings_array_payload() {
        let embedding = extract_embedding(&json!({
            "embeddings": [[0.1, 0.2, 0.3]]
        }))
        .unwrap();

        assert_eq!(embedding, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn should_render_schema_embedding_text_deterministically() {
        let mut nested_fields = HashMap::new();
        nested_fields.insert(
            "beta".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: String::new(),
                default: None,
                example: None,
                constraints: vec![],
                read_only: false,
                read_only_when: None,
            },
        );
        nested_fields.insert(
            "alpha".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: String::new(),
                default: None,
                example: None,
                constraints: vec![],
                read_only: false,
                read_only_when: None,
            },
        );

        let mut fields = HashMap::new();
        fields.insert(
            "outcome_class".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["deny".into(), "allow".into()]),
                required: true,
                description: "Outcome bucket".into(),
                default: None,
                example: None,
                constraints: vec![Constraint::OneOf {
                    values: vec![simd_json::json!("allow"), simd_json::json!("deny")],
                }],
                read_only: false,
                read_only_when: Some(ReadOnlyCondition {
                    property: "sealed".into(),
                    value: "true".into(),
                }),
            },
        );
        fields.insert(
            "tools_consulted".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object(nested_fields))),
                required: false,
                description: "Tools touched by the episode".into(),
                default: None,
                example: None,
                constraints: vec![Constraint::Min { value: 1.0 }],
                read_only: true,
                read_only_when: None,
            },
        );

        let schema = PluginSchema {
            name: "ctl-plane-chatbot".into(),
            category: "accountability".into(),
            version: "1.0.0".into(),
            description: "Human reviewable reasoning episodes".into(),
            fields,
            dependencies: vec!["op-grpc-bridge".into(), "op-state-store".into()],
            example: None,
            immutable_paths: vec!["/episode_id".into()],
            tags: vec!["audit".into(), "pii".into()],
            dialect: op_state_store::DEFAULT_SCHEMA_DIALECT.into(),
            mutation_index: Some(7),
        };

        let rendered = render_schema_embedding_text(&schema);

        assert!(rendered.contains("schema_name: ctl-plane-chatbot"));
        assert!(rendered.contains("schema_category: accountability"));
        assert!(rendered.contains("schema_tags: audit, pii"));
        assert!(rendered.contains("immutable_paths: /episode_id"));
        assert!(rendered.contains("dependencies: op-grpc-bridge, op-state-store"));
        assert!(rendered.contains(
            "field outcome_class: type=enum<deny|allow>, required=true, read_only=false, description=Outcome bucket"
        ));
        assert!(rendered.contains("field outcome_class read_only_when: sealed=true"));
        assert!(rendered.contains("field tools_consulted: type=array<object<alpha|beta>>"));
    }
}
