//! Schema-as-Code Agent
//!
//! Designs and validates machine-readable schema contracts: JSON Schema, OpenAPI,
//! AsyncAPI, Protocol Buffers, OSCAL extensions, OpenControl YAML.

use crate::agents::base::{AgentTask, AgentTrait, TaskResult};
use crate::security::SecurityProfile;
use async_trait::async_trait;
use simd_json::json;

pub struct SchemaAsCodeAgent {
    agent_id: String,
    profile: SecurityProfile,
}

impl SchemaAsCodeAgent {
    pub fn new(agent_id: String) -> Self {
        Self {
            profile: SecurityProfile::content_generation("schema-as-code"),
            agent_id,
        }
    }

    fn analyze(&self, op: &str, args: Option<&str>) -> Result<String, String> {
        let input = args.unwrap_or("").to_lowercase();

        let result = match op {
            "design_schema" => {
                let format = if input.contains("openapi") || input.contains("rest") {
                    "OpenAPI 3.1"
                } else if input.contains("proto") || input.contains("grpc") {
                    "Protocol Buffers 3"
                } else if input.contains("asyncapi")
                    || input.contains("event")
                    || input.contains("kafka")
                {
                    "AsyncAPI 3.0"
                } else if input.contains("oscal") {
                    "OSCAL JSON Schema"
                } else {
                    "JSON Schema Draft 2020-12"
                };
                json!({
                    "operation": "design_schema",
                    "recommended_format": format,
                    "best_practices": [
                        "Use $id and $schema for self-describing documents",
                        "Prefer explicit required over nullable for mandatory fields",
                        "Use $defs for reusable types",
                        "Version your schema with semver in the URI",
                        "Provide examples for every complex type"
                    ],
                    "compliance_extensions": {
                        "oscal": "x-oscal-control-id for mapping fields to controls",
                        "gdpr": "x-pii: true to flag personal data fields",
                        "fedramp": "x-fedramp-impact for sensitivity classification"
                    },
                    "input": args.unwrap_or("")
                })
            }
            "validate" => json!({
                "operation": "validate",
                "tools": {
                    "json_schema": "ajv validate --spec=draft2020 -s schema.json -d data.json",
                    "openapi": "spectral lint openapi.yaml",
                    "protobuf": "buf lint && buf breaking --against .git#branch=main",
                    "oscal": "oscal-cli validate -f document.json"
                },
                "ci_integration": "Run validators in pre-commit hooks and CI pipelines",
                "breaking_change_detection": "buf breaking, openapi-diff, json-schema-diff"
            }),
            "opencontrol" => json!({
                "operation": "opencontrol",
                "description": "OpenControl YAML schema for compliance-as-code (alternative to OSCAL)",
                "schema_elements": {
                    "component": "system component declaration",
                    "satisfies": "list of control satisfactions with narrative",
                    "documentation_complete": "boolean flag per control"
                },
                "example": {
                    "schema_version": "3.1.0",
                    "name": "My Component",
                    "key": "MC",
                    "satisfies": [{
                        "standard_key": "NIST-800-53",
                        "control_key": "AC-1",
                        "narrative": [{"text": "The system implements..."}]
                    }]
                },
                "tools": ["compliance-masonry", "opencontrol/schemas", "fedramp-gocomply"],
                "migration_to_oscal": "compliance-io converts OpenControl → OSCAL component-definition"
            }),
            "generate_schema" => json!({
                "operation": "generate_schema",
                "from_rust": "schemars crate: #[derive(JsonSchema)] generates JSON Schema",
                "from_typescript": "zod.toJsonSchema() or ts-json-schema-generator",
                "from_proto": "buf generate --template buf.gen.yaml (openapi/jsonschema plugins)",
                "from_examples": "quicktype infers schema from JSON examples",
                "input": args.unwrap_or("")
            }),
            _ => json!({
                "agent": "schema-as-code",
                "description": "Schema-as-code design and validation expert",
                "operations": ["design_schema", "validate", "opencontrol", "generate_schema"],
                "formats": ["JSON Schema", "OpenAPI", "AsyncAPI", "Protobuf", "OSCAL", "OpenControl"]
            }),
        };
        Ok(simd_json::to_string_pretty(&result).unwrap_or_default())
    }
}

#[async_trait]
impl AgentTrait for SchemaAsCodeAgent {
    fn agent_type(&self) -> &str {
        "schema-as-code"
    }
    fn name(&self) -> &str {
        "Schema-as-Code Architect"
    }
    fn description(&self) -> &str {
        "Schema-as-code design expert: JSON Schema, OpenAPI, AsyncAPI, Protobuf, OSCAL extensions, OpenControl YAML. Designs compliance-aware schemas with PII/control annotations, validates with linters, detects breaking changes."
    }
    fn operations(&self) -> Vec<String> {
        vec![
            "design_schema".to_string(),
            "validate".to_string(),
            "opencontrol".to_string(),
            "generate_schema".to_string(),
            "advise".to_string(),
        ]
    }
    fn security_profile(&self) -> &SecurityProfile {
        &self.profile
    }
    fn get_status(&self) -> String {
        format!("Schema-as-Code agent {} is running", self.agent_id)
    }
    async fn execute(&self, task: AgentTask) -> Result<TaskResult, String> {
        if task.task_type != "schema-as-code" {
            return Err(format!("Invalid task type: {}", task.task_type));
        }
        match self.analyze(&task.operation, task.args.as_deref()) {
            Ok(data) => Ok(TaskResult::success(&task.operation, data)),
            Err(e) => Ok(TaskResult::failure(&task.operation, e)),
        }
    }
}
