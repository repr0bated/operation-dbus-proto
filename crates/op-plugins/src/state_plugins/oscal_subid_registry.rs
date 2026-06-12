//! OSCAL Subid Registry plugin — dual-identifier model for every system artifact.
//!
//! Every D-Bus object, plugin, schema, mutation, event, and tool carries two
//! identifiers: a `uuid` (machine identity) and a `subid` (human-readable
//! operational taxonomy key). This plugin defines the canonical shape of one
//! registry entry.
//!
//! Compliance refs live in metadata arrays — never inside the subid string itself.

use anyhow::Result;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{Constraint, FieldSchema, FieldType, PluginSchema};
use simd_json::json;
use simd_json::OwnedValue as Value;

pub(crate) fn oscal_subid_registry_schema() -> PluginSchema {
    PluginSchema::builder("oscal_subid_registry")
        .version("1.0.0")
        .description("OSCAL subid registry — dual-identifier model for every system artifact. uuid = machine identity, subid = operational taxonomy key.")
        .category("compliance")
        .field("uuid", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Machine identity UUID (RFC 4122). Never replaced by subid.".to_string(),
            default: None,
            example: Some(json!("a1b2c3d4-e5f6-7890-abcd-ef0123456789")),
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("subid", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Human-readable operational taxonomy key. Format: <category>.<component-type>.<subject>.<verb>[.<facet>][@vN]. Immutable per subject.".to_string(),
            default: None,
            example: Some(json!("mut.service.state-sync.apply-patch@v1")),
            constraints: vec![
                Constraint::Pattern {
                    regex: "^(src|prj|sch|mut|obs|evt|exp)\\.(this-system|system|interconnection|software|hardware|service|policy|physical|process-procedure|plan|guidance|standard|validation|network)\\.[a-z0-9]+(?:-[a-z0-9]+)*\\.[a-z0-9]+(?:-[a-z0-9]+)*(?:\\.[a-z0-9]+(?:-[a-z0-9]+)*){0,2}(?:@v[1-9][0-9]*)?$".to_string()
                },
            ],
            read_only: false,
            read_only_when: None,
        })
        .field("category", FieldSchema {
            field_type: FieldType::Enum(vec![
                "src".to_string(), "prj".to_string(), "sch".to_string(),
                "mut".to_string(), "obs".to_string(), "evt".to_string(), "exp".to_string(),
            ]),
            required: true,
            description: "Operational category. Determines which additional fields are required.".to_string(),
            default: None,
            example: Some(json!("mut")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("component_type", FieldSchema {
            field_type: FieldType::Enum(vec![
                "software".to_string(), "service".to_string(), "network".to_string(),
                "hardware".to_string(), "process-procedure".to_string(), "standard".to_string(),
                "validation".to_string(), "policy".to_string(), "plan".to_string(),
                "guidance".to_string(), "physical".to_string(), "this-system".to_string(),
                "system".to_string(), "interconnection".to_string(),
            ]),
            required: true,
            description: "OSCAL component-type vocabulary. Reuse OSCAL nouns — do not invent new types.".to_string(),
            default: None,
            example: Some(json!("service")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("subject", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Stable noun identifying the artifact (e.g. state-sync, plugin-schema, event-chain). Lowercase hyphenated.".to_string(),
            default: None,
            example: Some(json!("state-sync")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("verb", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Action performed on the subject (e.g. apply-patch, resolve, monitor). Lowercase hyphenated.".to_string(),
            default: None,
            example: Some(json!("apply-patch")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("facet", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Optional additional qualifier (up to two segments). Lowercase hyphenated.".to_string(),
            default: None,
            example: Some(json!("rollback")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("version", FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Schema version of this subid (the @vN suffix). Increment only when subject meaning changes materially.".to_string(),
            default: Some(json!(1)),
            example: Some(json!(1)),
            constraints: vec![Constraint::Min { value: 1.0 }],
            read_only: false,
            read_only_when: None,
        })
        .field("control_source", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "URI of the OSCAL catalog or profile that provides the control baseline.".to_string(),
            default: None,
            example: Some(json!("https://csrc.nist.gov/projects/oscal")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("control_refs", FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "OSCAL control IDs satisfied by this artifact. Compliance detail belongs here, not in the subid string.".to_string(),
            default: Some(json!([])),
            example: Some(json!(["AC-1", "CM-3"])),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("statement_refs", FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Optional fine-grained OSCAL statement-level references within the controls.".to_string(),
            default: Some(json!([])),
            example: Some(json!(["AC-1_smt.a"])),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("actor_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for mut.* entries. Identity of the actor that performed the mutation.".to_string(),
            default: None,
            example: Some(json!("user:jeremy")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("capability_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for mut.* entries. Capability that authorized the mutation.".to_string(),
            default: None,
            example: Some(json!("cap:state-write")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("idempotency_key", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for mut.* entries. Deduplication key for the mutation operation.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("event_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for evt.* entries. Unique identifier for this event in the audit chain.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("event_hash", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for evt.* entries. Content hash of the event for chain verification.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("tags_touched", FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Required for evt.* entries. Tags whose immutability is affected by this event.".to_string(),
            default: Some(json!([])),
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("proof_root", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Optional for evt.* entries. Merkle proof root for chain verification.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("source_system", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for src.* entries. Name of the authoritative source system.".to_string(),
            default: None,
            example: Some(json!("ovsdb")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("source_locator", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for src.* entries. Socket path, URL, or address of the source.".to_string(),
            default: None,
            example: Some(json!("unix:/var/run/openvswitch/db.sock")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("authority_rank", FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Optional for src.* entries. Precedence when multiple sources provide the same subject (lower = higher authority).".to_string(),
            default: Some(json!(100)),
            example: Some(json!(1)),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("dbus_path", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for prj.* entries. D-Bus object path of the projected artifact.".to_string(),
            default: None,
            example: Some(json!("/opdbus/v1/plugins/wireguard")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("service_name", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for prj.* entries. D-Bus service name hosting the object.".to_string(),
            default: None,
            example: Some(json!("org.opdbus.v1")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("source_subid", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Optional for prj.* entries. Subid of the src.* record this projection was derived from.".to_string(),
            default: None,
            example: Some(json!("src.network.ovsdb.monitor@v1")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("schema_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for sch.* entries. Canonical name of the schema.".to_string(),
            default: None,
            example: Some(json!("wireguard")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("schema_hash", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for sch.* entries. Content hash of the schema at this version.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("consumer_surface", FieldSchema {
            field_type: FieldType::Enum(vec![
                "mcp-tool".to_string(), "dbus-method".to_string(), "grpc-method".to_string(),
                "ui-field".to_string(), "ui-page".to_string(), "api-endpoint".to_string(),
            ]),
            required: false,
            description: "Required for exp.* entries. The consumer-facing surface this artifact is rendered on.".to_string(),
            default: None,
            example: Some(json!("mcp-tool")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("tool_name", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for exp.mcp-tool entries. The MCP tool name as registered.".to_string(),
            default: None,
            example: Some(json!("cognitive_memory")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("query_scope", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for obs.* entries. D-Bus path pattern or scope expression for this observation.".to_string(),
            default: None,
            example: Some(json!("/opdbus/v1/plugins/*")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .build()
}

pub struct OscalSubidRegistryPlugin;

impl Default for OscalSubidRegistryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl OscalSubidRegistryPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl StatePlugin for OscalSubidRegistryPlugin {
    fn name(&self) -> &str {
        "oscal_subid_registry"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(oscal_subid_registry_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::json!({"entries": []}))
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
        let state = self.query_current_state().await?;
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
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
