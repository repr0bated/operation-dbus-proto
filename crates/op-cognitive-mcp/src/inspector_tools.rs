//! `inspect_object` — LLM-callable wrapper around `op-inspector`'s
//! `IntrospectiveGadget`.
//!
//! Exposes the introspection/gap-filling capability proven out and fixed
//! this session (multi-sample `required`-intersection merging in
//! `merge_schema_definitions`, and the O(n) `analyze_binary_patterns` fix
//! that makes full-file binary/blob analysis fast enough to call live)
//! through cognitive-mcp, the sanctioned universal external MCP gateway —
//! per architecture, new capabilities are exposed here, not as a new
//! standalone service.
//!
//! One `IntrospectiveGadget` (and its `KnowledgeBase`) is shared across every
//! call so repeated `inspect_object` calls against the same `description`
//! benefit from cross-sample gap-filling automatically.

use anyhow::Result;
use async_trait::async_trait;
use op_inspector::{InspectionInput, InspectionSource, IntrospectiveGadget, KnowledgeBase};
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Register the `inspect_object` tool into the MCP tool registry.
pub async fn register_inspector_tools(registry: &ToolRegistry) -> Result<usize> {
    let knowledge_base = Arc::new(RwLock::new(KnowledgeBase::default()));
    let gadget = Arc::new(IntrospectiveGadget::new(knowledge_base).await?);

    let tools: Vec<BoxedTool> = vec![Arc::new(InspectObjectTool::new(gadget))];

    let count = tools.len();
    for tool in tools {
        registry.register(tool).await?;
    }

    tracing::info!(registered = count, "Registered op-inspector tools");
    Ok(count)
}

struct InspectObjectTool {
    gadget: Arc<IntrospectiveGadget>,
}

impl InspectObjectTool {
    fn new(gadget: Arc<IntrospectiveGadget>) -> Self {
        Self { gadget }
    }
}

#[async_trait]
impl Tool for InspectObjectTool {
    fn name(&self) -> &str {
        "inspect_object"
    }

    fn description(&self) -> &str {
        "Inspect a file, a raw data blob, or a Docker container and infer its schema \
         (JSON/XML/YAML/binary auto-detected). Repeated calls with the same `description` \
         accumulate: `required` fields converge to the true intersection across every sample \
         seen so far, instead of naively marking every field from a single sample as required."
    }

    fn category(&self) -> &str {
        "introspection"
    }

    fn namespace(&self) -> &str {
        "op-inspector"
    }

    fn tags(&self) -> Vec<String> {
        vec![
            "inspector".to_string(),
            "schema".to_string(),
            "introspection".to_string(),
        ]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_type": {
                    "type": "string",
                    "enum": ["file", "raw", "docker_container", "url"],
                    "description": "What kind of thing to inspect"
                },
                "path_or_container_or_url": {
                    "type": "string",
                    "description": "File path / Docker container name / URL (required unless source_type is 'raw')"
                },
                "data": {
                    "type": "string",
                    "description": "Raw data to inspect (required when source_type is 'raw')"
                },
                "format_hint": {
                    "type": "string",
                    "description": "Optional format hint for 'raw' data (e.g. 'json', 'binary')"
                },
                "description": {
                    "type": "string",
                    "description": "Stable name for this logical object -- reused across calls \
                                     to accumulate gap-filling merge (same description = same \
                                     knowledge-base entry, converging `required` over samples)"
                }
            },
            "required": ["source_type", "description"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let source_type = input["source_type"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing source_type"))?;
        let description = input["description"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing description"))?
            .to_string();

        let source = match source_type {
            "file" => InspectionSource::File(
                input["path_or_container_or_url"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing path_or_container_or_url for source_type=file"))?
                    .to_string(),
            ),
            "docker_container" => InspectionSource::DockerContainer(
                input["path_or_container_or_url"]
                    .as_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!("missing path_or_container_or_url for source_type=docker_container")
                    })?
                    .to_string(),
            ),
            "url" => InspectionSource::Url(
                input["path_or_container_or_url"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing path_or_container_or_url for source_type=url"))?
                    .to_string(),
            ),
            "raw" => InspectionSource::RawData {
                format_hint: input["format_hint"].as_str().map(|s| s.to_string()),
                description: description.clone(),
            },
            other => {
                return Err(anyhow::anyhow!(
                    "unknown source_type '{other}': expected file|raw|docker_container|url"
                ))
            }
        };

        let data = input["data"].as_str().map(|s| s.to_string());

        let inspection_input = InspectionInput {
            source,
            data,
            metadata: Default::default(),
        };

        let result = self.gadget.inspect_object(inspection_input).await?;

        Ok(json!({
            "detected_format": result.detected_format,
            "schema_type": result.schema.schema_type,
            "required": result.schema.required,
            "properties": result.schema.properties.keys().collect::<Vec<_>>(),
            "knowledge_base_entry": result.knowledge_base_entry,
            "inspection_time_ms": result.inspection_time_ms as u64,
            "parsing_errors": result.parsing_errors,
        }))
    }
}
