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
    Some(op_core::config::OPDBUS_BUS_NAME.to_string())
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
#[schemars(extend("x-oscal-category" = "standard"))]
pub struct OscalSubidRegistryEntry {
    /// Machine identity UUID (RFC 4122). Never replaced by subid.
    #[schemars(
        extend("readOnly" = true),
        extend("x-oscal-subid" = "obs.standard.oscal-subid-registry.uuid@v1"),
        example = example_uuid()
    )]
    pub uuid: String,

    /// Human-readable operational taxonomy key. Format: <category>.<component-type>.<subject>.<verb>\[.<facet>\]\[@vN\]. Immutable per subject.
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
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "oscal_subid_registry",
        "1.0.0",
        "OSCAL subid registry — dual-identifier model for every system artifact. uuid = machine identity, subid = operational taxonomy key.",
        &root,
    );

    // Output structs
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct LookupOutput {
        pub entry: Option<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListByCategoryOutput {
        pub entries: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ResolveOutput {
        pub resolved: Option<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ExportOutput {
        pub registry: serde_json::Value,
    }

    // Add methods
    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "register".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "register",
            SideEffect::Mutation,
            false,
            "oscal.invoke",
            "mut.standard.oscal.subid.register@v1",
        ),
    );
    schema.methods.insert(
        "lookup".to_string(),
        method_decl_from_schemars_with_output::<(), LookupOutput>(
            "lookup",
            SideEffect::Read,
            true,
            "oscal.read",
            "obs.standard.oscal.subid.lookup@v1",
        ),
    );
    schema.methods.insert(
        "list_by_category".to_string(),
        method_decl_from_schemars_with_output::<(), ListByCategoryOutput>(
            "list_by_category",
            SideEffect::Read,
            true,
            "oscal.read",
            "obs.standard.oscal.category.list@v1",
        ),
    );
    schema.methods.insert(
        "resolve".to_string(),
        method_decl_from_schemars_with_output::<(), ResolveOutput>(
            "resolve",
            SideEffect::Read,
            true,
            "oscal.read",
            "obs.standard.oscal.subid.resolve@v1",
        ),
    );
    schema.methods.insert(
        "export".to_string(),
        method_decl_from_schemars_with_output::<(), ExportOutput>(
            "export",
            SideEffect::Read,
            true,
            "oscal.read",
            "obs.standard.oscal.export@v1",
        ),
    );

    schema
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
        let state = simd_json::json!(null);
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

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("oscal_subid_registry", |_ctx| std::sync::Arc::new(OscalSubidRegistryPlugin::new()))
}
