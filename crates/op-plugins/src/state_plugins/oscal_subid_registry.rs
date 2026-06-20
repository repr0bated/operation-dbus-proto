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
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

// ── Defaults for serde ───────────────────────────────────────────────────────

fn default_version() -> u32 {
    1
}

fn default_authority_rank() -> i64 {
    100
}

// ── Examples for schemars ────────────────────────────────────────────────────

fn example_uuid() -> String {
    "a1b2c3d4-e5f6-7890-abcd-ef0123456789".to_string()
}

fn example_subid() -> String {
    "mut.service.state-sync.apply-patch@v1".to_string()
}

fn example_category() -> OscalSubidCategory {
    OscalSubidCategory::Mut
}

fn example_component_type() -> OscalComponentType {
    OscalComponentType::Service
}

fn example_subject() -> String {
    "state-sync".to_string()
}

fn example_verb() -> String {
    "apply-patch".to_string()
}

fn example_facet() -> Option<String> {
    Some("rollback".to_string())
}

fn example_version() -> u32 {
    1
}

fn example_control_source() -> Option<String> {
    Some("https://csrc.nist.gov/projects/oscal".to_string())
}

fn example_control_refs() -> Vec<String> {
    vec!["AC-1".to_string(), "CM-3".to_string()]
}

fn example_statement_refs() -> Vec<String> {
    vec!["AC-1_smt.a".to_string()]
}

fn example_actor_id() -> Option<String> {
    Some("user:jeremy".to_string())
}

fn example_capability_id() -> Option<String> {
    Some("cap:state-write".to_string())
}

fn example_source_system() -> Option<String> {
    Some("ovsdb".to_string())
}

fn example_source_locator() -> Option<String> {
    Some("unix:/var/run/openvswitch/db.sock".to_string())
}

fn example_authority_rank() -> i64 {
    1
}

fn example_dbus_path() -> Option<String> {
    Some("/opdbus/v1/plugins/wireguard".to_string())
}

fn example_service_name() -> Option<String> {
    Some("org.opdbus.v1".to_string())
}

fn example_source_subid() -> Option<String> {
    Some("src.network.ovsdb.monitor@v1".to_string())
}

fn example_schema_id() -> Option<String> {
    Some("wireguard".to_string())
}

fn example_consumer_surface() -> Option<OscalConsumerSurface> {
    Some(OscalConsumerSurface::McpTool)
}

fn example_tool_name() -> Option<String> {
    Some("cognitive_memory".to_string())
}

fn example_query_scope() -> Option<String> {
    Some("/opdbus/v1/plugins/*".to_string())
}

// ── Enums ────────────────────────────────────────────────────────────────────

/// Operational category. Determines which additional fields are required.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[schemars(extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.category@v1"))]
pub enum OscalSubidCategory {
    Src,
    Prj,
    Sch,
    Mut,
    Obs,
    Evt,
    Exp,
}

/// OSCAL component-type vocabulary. Reuse OSCAL nouns — do not invent new types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[schemars(extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.component-type@v1"))]
pub enum OscalComponentType {
    ThisSystem,
    System,
    Interconnection,
    Software,
    Hardware,
    Service,
    Policy,
    Physical,
    ProcessProcedure,
    Plan,
    Guidance,
    Standard,
    Validation,
    Network,
}

/// Consumer-facing surface this artifact is rendered on.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[schemars(extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.consumer-surface@v1"))]
pub enum OscalConsumerSurface {
    McpTool,
    DbusMethod,
    GrpcMethod,
    UiField,
    UiPage,
    ApiEndpoint,
}

// ── Registry entry struct ────────────────────────────────────────────────────

/// Canonical shape of one OSCAL subid registry entry.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend(
    "x-oscal-subid" = "sch.standard.plugin.oscal-subid-registry.schema@v1"
))]
pub struct OscalSubidRegistryEntry {
    /// Machine identity UUID (RFC 4122). Never replaced by subid.
    #[schemars(
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.standard.oscal-subid-registry.uuid@v1"),
        example = example_uuid()
    )]
    pub uuid: String,

    /// Human-readable operational taxonomy key. Format: <category>.<component-type>.<subject>.<verb>[.<facet>][@vN]. Immutable per subject.
    #[schemars(
        pattern(r"^(src|prj|sch|mut|obs|evt|exp)\.(this-system|system|interconnection|software|hardware|service|policy|physical|process-procedure|plan|guidance|standard|validation|network)\.[a-z0-9]+(?:-[a-z0-9]+)*\.[a-z0-9]+(?:-[a-z0-9]+)*(?:\.[a-z0-9]+(?:-[a-z0-9]+)*){0,2}(?:@v[1-9][0-9]*)?$"),
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.subid@v1"),
        example = example_subid()
    )]
    pub subid: String,

    /// Operational category. Determines which additional fields are required.
    #[schemars(
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.category@v1"),
        example = example_category()
    )]
    pub category: OscalSubidCategory,

    /// OSCAL component-type vocabulary. Reuse OSCAL nouns — do not invent new types.
    #[schemars(
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.component-type@v1"),
        example = example_component_type()
    )]
    pub component_type: OscalComponentType,

    /// Stable noun identifying the artifact (e.g. state-sync, plugin-schema). Lowercase hyphenated.
    #[schemars(
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.subject@v1"),
        example = example_subject()
    )]
    pub subject: String,

    /// Action performed on the subject (e.g. apply-patch, resolve, monitor). Lowercase hyphenated.
    #[schemars(
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.verb@v1"),
        example = example_verb()
    )]
    pub verb: String,

    /// Optional additional qualifier (up to two segments). Lowercase hyphenated.
    #[schemars(
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.facet@v1"),
        example = example_facet()
    )]
    pub facet: Option<String>,

    /// Schema version of this subid (the @vN suffix). Increment only when subject meaning changes materially.
    #[serde(default = "default_version")]
    #[schemars(
        range(min = 1),
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.version@v1"),
        example = example_version()
    )]
    pub version: u32,

    /// URI of the OSCAL catalog or profile that provides the control baseline.
    #[schemars(
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.control-source@v1"),
        example = example_control_source()
    )]
    pub control_source: Option<String>,

    /// OSCAL control IDs satisfied by this artifact. Compliance detail belongs here, not in the subid string.
    #[serde(default)]
    #[schemars(
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.control-refs@v1"),
        example = example_control_refs()
    )]
    pub control_refs: Vec<String>,

    /// Optional fine-grained OSCAL statement-level references within the controls.
    #[serde(default)]
    #[schemars(
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.statement-refs@v1"),
        example = example_statement_refs()
    )]
    pub statement_refs: Vec<String>,

    /// Required for mut.* entries. Identity of the actor that performed the mutation.
    #[schemars(
        extend("x-oscal-subid" = "mut.standard.oscal-subid-registry.actor-id@v1"),
        example = example_actor_id()
    )]
    pub actor_id: Option<String>,

    /// Required for mut.* entries. Capability that authorized the mutation.
    #[schemars(
        extend("x-oscal-subid" = "mut.standard.oscal-subid-registry.capability-id@v1"),
        example = example_capability_id()
    )]
    pub capability_id: Option<String>,

    /// Required for mut.* entries. Deduplication key for the mutation operation.
    #[schemars(
        extend("x-oscal-subid" = "mut.standard.oscal-subid-registry.idempotency-key@v1")
    )]
    pub idempotency_key: Option<String>,

    /// Required for evt.* entries. Unique identifier for this event in the audit chain.
    #[schemars(
        extend("readOnly" = true),
        extend("x-oscal-subid" = "evt.standard.oscal-subid-registry.event-id@v1")
    )]
    pub event_id: Option<String>,

    /// Required for evt.* entries. Content hash of the event for chain verification.
    #[schemars(
        extend("readOnly" = true),
        extend("x-oscal-subid" = "evt.standard.oscal-subid-registry.event-hash@v1")
    )]
    pub event_hash: Option<String>,

    /// Required for evt.* entries. Tags whose immutability is affected by this event.
    #[serde(default)]
    #[schemars(
        extend("readOnly" = true),
        extend("x-oscal-subid" = "evt.standard.oscal-subid-registry.tags-touched@v1")
    )]
    pub tags_touched: Vec<String>,

    /// Optional for evt.* entries. Merkle proof root for chain verification.
    #[schemars(
        extend("readOnly" = true),
        extend("x-oscal-subid" = "evt.standard.oscal-subid-registry.proof-root@v1")
    )]
    pub proof_root: Option<String>,

    /// Required for src.* entries. Name of the authoritative source system.
    #[schemars(
        extend("x-oscal-subid" = "src.standard.oscal-subid-registry.source-system@v1"),
        example = example_source_system()
    )]
    pub source_system: Option<String>,

    /// Required for src.* entries. Socket path, URL, or address of the source.
    #[schemars(
        extend("x-oscal-subid" = "src.standard.oscal-subid-registry.source-locator@v1"),
        example = example_source_locator()
    )]
    pub source_locator: Option<String>,

    /// Optional for src.* entries. Precedence when multiple sources provide the same subject (lower = higher authority).
    #[serde(default = "default_authority_rank")]
    #[schemars(
        extend("x-oscal-subid" = "src.standard.oscal-subid-registry.authority-rank@v1"),
        example = example_authority_rank()
    )]
    pub authority_rank: i64,

    /// Required for prj.* entries. D-Bus object path of the projected artifact.
    #[schemars(
        extend("x-oscal-subid" = "prj.standard.oscal-subid-registry.dbus-path@v1"),
        example = example_dbus_path()
    )]
    pub dbus_path: Option<String>,

    /// Required for prj.* entries. D-Bus service name hosting the object.
    #[schemars(
        extend("x-oscal-subid" = "prj.standard.oscal-subid-registry.service-name@v1"),
        example = example_service_name()
    )]
    pub service_name: Option<String>,

    /// Optional for prj.* entries. Subid of the src.* record this projection was derived from.
    #[schemars(
        extend("x-oscal-subid" = "prj.standard.oscal-subid-registry.source-subid@v1"),
        example = example_source_subid()
    )]
    pub source_subid: Option<String>,

    /// Required for sch.* entries. Canonical name of the schema.
    #[schemars(
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.schema-id@v1"),
        example = example_schema_id()
    )]
    pub schema_id: Option<String>,

    /// Required for sch.* entries. Content hash of the schema at this version.
    #[schemars(
        extend("readOnly" = true),
        extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.schema-hash@v1")
    )]
    pub schema_hash: Option<String>,

    /// Required for exp.* entries. The consumer-facing surface this artifact is rendered on.
    #[schemars(
        extend("x-oscal-subid" = "exp.standard.oscal-subid-registry.consumer-surface@v1"),
        example = example_consumer_surface()
    )]
    pub consumer_surface: Option<OscalConsumerSurface>,

    /// Required for exp.mcp-tool entries. The MCP tool name as registered.
    #[schemars(
        extend("x-oscal-subid" = "exp.standard.oscal-subid-registry.tool-name@v1"),
        example = example_tool_name()
    )]
    pub tool_name: Option<String>,

    /// Required for obs.* entries. D-Bus path pattern or scope expression for this observation.
    #[schemars(
        extend("x-oscal-subid" = "obs.standard.oscal-subid-registry.query-scope@v1"),
        example = example_query_scope()
    )]
    pub query_scope: Option<String>,
}

/// Derived `oscal_subid_registry` schema from the typed struct.
pub(crate) fn oscal_subid_registry_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(OscalSubidRegistryEntry))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        "oscal_subid_registry",
        "1.0.0",
        "OSCAL subid registry — dual-identifier model for every system artifact. uuid = machine identity, subid = operational taxonomy key.",
        &root,
    )
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

#[cfg(test)]
use op_state_store::{Constraint, FieldSchema, FieldType};
#[cfg(test)]
use simd_json::json;

#[cfg(test)]
pub(crate) fn oscal_subid_registry_schema_golden() -> PluginSchema {
    PluginSchema::builder("oscal_subid_registry")
        .version("1.0.0")
        .description("OSCAL subid registry — dual-identifier model for every system artifact. uuid = machine identity, subid = operational taxonomy key.")
        .category("compliance")
        .subid("__schema__", "sch.standard.plugin.oscal-subid-registry.schema@v1")
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
        .subid("uuid", "obs.standard.oscal-subid-registry.uuid@v1")
        .field("subid", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Human-readable operational taxonomy key. Format: <category>.<component-type>.<subject>.<verb>[.<facet>][@vN]. Immutable per subject.".to_string(),
            default: None,
            example: Some(json!("mut.service.state-sync.apply-patch@v1")),
            constraints: vec![
                Constraint::Pattern {
                    regex: super::common::oscal::SUBID_REGEX.to_string()
                },
            ],
            read_only: false,
            read_only_when: None,
        })
        .subid("subid", "sch.standard.oscal-subid-registry.subid@v1")
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
        .subid("category", "sch.standard.oscal-subid-registry.category@v1")
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
        .subid("component_type", "sch.standard.oscal-subid-registry.component-type@v1")
        .field("subject", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Stable noun identifying the artifact (e.g. state-sync, plugin-schema). Lowercase hyphenated.".to_string(),
            default: None,
            example: Some(json!("state-sync")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .subid("subject", "sch.standard.oscal-subid-registry.subject@v1")
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
        .subid("verb", "sch.standard.oscal-subid-registry.verb@v1")
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
        .subid("facet", "sch.standard.oscal-subid-registry.facet@v1")
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
        .subid("version", "sch.standard.oscal-subid-registry.version@v1")
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
        .subid("control_source", "sch.standard.oscal-subid-registry.control-source@v1")
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
        .subid("control_refs", "sch.standard.oscal-subid-registry.control-refs@v1")
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
        .subid("statement_refs", "sch.standard.oscal-subid-registry.statement-refs@v1")
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
        .subid("actor_id", "mut.standard.oscal-subid-registry.actor-id@v1")
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
        .subid("capability_id", "mut.standard.oscal-subid-registry.capability-id@v1")
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
        .subid("idempotency_key", "mut.standard.oscal-subid-registry.idempotency-key@v1")
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
        .subid("event_id", "evt.standard.oscal-subid-registry.event-id@v1")
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
        .subid("event_hash", "evt.standard.oscal-subid-registry.event-hash@v1")
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
        .subid("tags_touched", "evt.standard.oscal-subid-registry.tags-touched@v1")
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
        .subid("proof_root", "evt.standard.oscal-subid-registry.proof-root@v1")
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
        .subid("source_system", "src.standard.oscal-subid-registry.source-system@v1")
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
        .subid("source_locator", "src.standard.oscal-subid-registry.source-locator@v1")
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
        .subid("authority_rank", "src.standard.oscal-subid-registry.authority-rank@v1")
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
        .subid("dbus_path", "prj.standard.oscal-subid-registry.dbus-path@v1")
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
        .subid("service_name", "prj.standard.oscal-subid-registry.service-name@v1")
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
        .subid("source_subid", "prj.standard.oscal-subid-registry.source-subid@v1")
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
        .subid("schema_id", "sch.standard.oscal-subid-registry.schema-id@v1")
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
        .subid("schema_hash", "sch.standard.oscal-subid-registry.schema-hash@v1")
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
        .subid("consumer_surface", "exp.standard.oscal-subid-registry.consumer-surface@v1")
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
        .subid("tool_name", "exp.standard.oscal-subid-registry.tool-name@v1")
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
        .subid("query_scope", "obs.standard.oscal-subid-registry.query-scope@v1")
        .build()
}

#[cfg(test)]
mod tests {

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let golden = super::oscal_subid_registry_schema_golden();
        let derived = super::oscal_subid_registry_schema();
        let diffs = super::super::schemars_adapter::schema_diffs(&golden, &derived);
        assert!(diffs.is_empty(), "schema drift: {:#?}", diffs);
    }
}
