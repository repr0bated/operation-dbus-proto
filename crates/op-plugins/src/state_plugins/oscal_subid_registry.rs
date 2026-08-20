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
// The `.vocabulary` facet distinguishes the enum (the closed vocabulary) from
// the entry's `category` field, which reuses the unfaceted subid. Reusing one
// subid for both made the artifact ambiguous.
#[schemars(extend("x-oscal-subid" = "sch.standard.oscal-subid-registry.category.vocabulary@v1"))]
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
#[schemars(extend(
    "x-oscal-subid" = "sch.standard.oscal-subid-registry.component-type.vocabulary@v1"
))]
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

    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    struct OscalInspectorSchema {
        #[serde(default)]
        #[schemars(extend(
            "x-oscal-subid" = "sch.standard.plugin.oscal-subid-registry.inspector-fields@v1"
        ))]
        inspector_fields: inspector_gadget_generated::InspectorGadgetFields,
    }
    let inspector_root = serde_json::to_value(schemars::schema_for!(OscalInspectorSchema))
        .expect("OSCAL Inspector schema serializes to JSON");
    let mut inspector = super::schemars_adapter::plugin_schema_from_json(
        "oscal_subid_registry_inspector",
        "1.0.0",
        "OSCAL upstream Inspector fields",
        &inspector_root,
    );
    if let Some(field) = inspector.fields.remove("inspector_fields") {
        schema.fields.insert("inspector_fields".to_string(), field);
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_preserves_registry_fields_and_methods_and_promotes_inspector_fields() {
        let schema = oscal_subid_registry_schema();
        for field in [
            "uuid",
            "subid",
            "category",
            "component_type",
            "inspector_fields",
        ] {
            assert!(schema.fields.contains_key(field), "missing {field}");
        }
        for method in [
            "register",
            "lookup",
            "list_by_category",
            "resolve",
            "export",
        ] {
            assert!(schema.methods.contains_key(method), "missing {method}");
        }
    }
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
    #[schemars(extend("x-oscal-subid" = "sch.software.oscal-subid-registry.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.activity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.activity@v1"))]
        pub activity: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.assessment-assets`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.assessment-assets@v1"))]
        pub assessment_assets: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.assessment-method`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.assessment-method@v1"))]
        pub assessment_method: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.assessment-part`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.assessment-part@v1"))]
        pub assessment_part: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.assessment-platform`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.assessment-platform@v1"))]
        pub assessment_platform: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.assessment-subject`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.assessment-subject@v1"))]
        pub assessment_subject: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.assessment-subject-placeholder`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.assessment-subject-placeholder@v1"))]
        pub assessment_subject_placeholder: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.associated-activity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.associated-activity@v1"))]
        pub associated_activity: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.associated-risk`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.associated-risk@v1"))]
        pub associated_risk: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.at-frequency`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.at-frequency@v1"))]
        pub at_frequency: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.characterization`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.characterization@v1"))]
        pub characterization: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.control-objective-selection`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.control-objective-selection@v1"))]
        pub control_objective_selection: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.control-selection`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.control-selection@v1"))]
        pub control_selection: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.dependency`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.dependency@v1"))]
        pub dependency: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.entry`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.entry@v1"))]
        pub entry: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.finding`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.finding@v1"))]
        pub finding: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.finding-target`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.finding-target@v1"))]
        pub finding_target: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.identified-subject`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.identified-subject@v1"))]
        pub identified_subject: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.import-ssp`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.import-ssp@v1"))]
        pub import_ssp: Option<u64>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.local-objective`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.local-objective@v1"))]
        pub local_objective: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.logged-by`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.logged-by@v1"))]
        pub logged_by: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.mitigating-factor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.mitigating-factor@v1"))]
        pub mitigating_factor: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.observation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.observation@v1"))]
        pub observation: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.on-date`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.on-date@v1"))]
        pub on_date: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.origin`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.origin@v1"))]
        pub origin: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.origin-actor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.origin-actor@v1"))]
        pub origin_actor: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.related-observation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.related-observation@v1"))]
        pub related_observation: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.related-response`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.related-response@v1"))]
        pub related_response: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.related-task`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.related-task@v1"))]
        pub related_task: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.relevant-evidence`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.relevant-evidence@v1"))]
        pub relevant_evidence: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.required-asset`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.required-asset@v1"))]
        pub required_asset: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.response`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.response@v1"))]
        pub response: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.reviewed-controls`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.reviewed-controls@v1"))]
        pub reviewed_controls: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.risk`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.risk@v1"))]
        pub risk: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.risk-log`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.risk-log@v1"))]
        pub risk_log: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.select-control-by-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.select-control-by-id@v1"))]
        pub select_control_by_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.select-objective-by-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.select-objective-by-id@v1"))]
        pub select_objective_by_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.select-subject-by-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.select-subject-by-id@v1"))]
        pub select_subject_by_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.step`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.step@v1"))]
        pub step: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.subject-reference`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.subject-reference@v1"))]
        pub subject_reference: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.task`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.task@v1"))]
        pub task: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.timing`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.timing@v1"))]
        pub timing: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.uses-component`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.uses-component@v1"))]
        pub uses_component: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.assembly.within-date-range`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.within-date-range@v1"))]
        pub within_date_range: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.activity-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.activity-uuid@v1"))]
        pub activity_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.actor-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.actor-uuid@v1"))]
        pub actor_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.class`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.class@v1"))]
        pub class: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.collected`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.collected@v1"))]
        pub collected: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.component-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.component-uuid@v1"))]
        pub component_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.date`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.date@v1"))]
        pub date: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.deadline`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.deadline@v1"))]
        pub deadline: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.end`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.end@v1"))]
        pub end: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.expires`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.expires@v1"))]
        pub expires: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.href`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.href@v1"))]
        pub href: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.implementation-statement-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.implementation-statement-uuid@v1"))]
        pub implementation_statement_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.implementation-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.implementation-uuid@v1"))]
        pub implementation_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.lifecycle`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.lifecycle@v1"))]
        pub lifecycle: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.method`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.method@v1"))]
        pub method: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.ns`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.ns@v1"))]
        pub ns: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.objective-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.objective-id@v1"))]
        pub objective_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.observation-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.observation-uuid@v1"))]
        pub observation_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.party-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.party-uuid@v1"))]
        pub party_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.period`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.period@v1"))]
        pub period: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.prose`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.prose@v1"))]
        pub prose: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.reason`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.reason@v1"))]
        pub reason: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.response-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.response-uuid@v1"))]
        pub response_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.risk-status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.risk-status@v1"))]
        pub risk_status: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.risk-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.risk-uuid@v1"))]
        pub risk_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.role-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.role-id@v1"))]
        pub role_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.start`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.start@v1"))]
        pub start: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.state`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.state@v1"))]
        pub state: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.statement`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.statement@v1"))]
        pub statement: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.statement-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.statement-id@v1"))]
        pub statement_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.subject-placeholder-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.subject-placeholder-uuid@v1"))]
        pub subject_placeholder_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.subject-type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.subject-type@v1"))]
        pub subject_type: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.subject-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.subject-uuid@v1"))]
        pub subject_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.system`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.system@v1"))]
        pub system: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.target-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.target-id@v1"))]
        pub target_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.task-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.task-uuid@v1"))]
        pub task_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.threat-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.threat-id@v1"))]
        pub threat_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.title`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.title@v1"))]
        pub title: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.type-field@v1"))]
        pub type_field: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.unit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.unit@v1"))]
        pub unit: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-common_metaschema.field.value`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.value@v1"))]
        pub value: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-plan_metaschema.assembly.assessment-plan`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.assessment-plan@v1"))]
        pub assessment_plan: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-plan_metaschema.assembly.local-definitions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.local-definitions@v1"))]
        pub local_definitions: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-plan_metaschema.assembly.terms-and-conditions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.terms-and-conditions@v1"))]
        pub terms_and_conditions: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-results_metaschema.assembly.assessment-log`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.assessment-log@v1"))]
        pub assessment_log: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-results_metaschema.assembly.assessment-results`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.assessment-results@v1"))]
        pub assessment_results: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-results_metaschema.assembly.attestation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.attestation@v1"))]
        pub attestation: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-results_metaschema.assembly.import-ap`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.import-ap@v1"))]
        pub import_ap: Option<u64>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_assessment-results_metaschema.assembly.result`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.result@v1"))]
        pub result: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_catalog_metaschema.assembly.catalog`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.catalog@v1"))]
        pub catalog: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_catalog_metaschema.assembly.control`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.control@v1"))]
        pub control: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_catalog_metaschema.assembly.external-mapping`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.external-mapping@v1"))]
        pub external_mapping: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_catalog_metaschema.assembly.group`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.group@v1"))]
        pub group: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_catalog_metaschema.assembly.local-mapping`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.local-mapping@v1"))]
        pub local_mapping: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_catalog_metaschema.field.map-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.map-uuid@v1"))]
        pub map_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_catalog_metaschema.field.mapping-collection-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.mapping-collection-uuid@v1"))]
        pub mapping_collection_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_component_metaschema.assembly.capability`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.capability@v1"))]
        pub capability: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_component_metaschema.assembly.component-definition`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.component-definition@v1"))]
        pub component_definition: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_component_metaschema.assembly.control-implementation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.control-implementation@v1"))]
        pub control_implementation: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_component_metaschema.assembly.defined-component`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.defined-component@v1"))]
        pub defined_component: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_component_metaschema.assembly.implemented-requirement`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.implemented-requirement@v1"))]
        pub implemented_requirement: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_component_metaschema.assembly.import-component-definition`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.import-component-definition@v1"))]
        pub import_component_definition: Option<u64>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_component_metaschema.assembly.incorporates-component`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.incorporates-component@v1"))]
        pub incorporates_component: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_component_metaschema.field.defined-component-type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.defined-component-type@v1"))]
        pub defined_component_type: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_component_metaschema.field.purpose`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.purpose@v1"))]
        pub purpose: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.assembly.include-all`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.include-all@v1"))]
        pub include_all: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.assembly.matching`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.matching@v1"))]
        pub matching: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.assembly.parameter`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.parameter@v1"))]
        pub parameter: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.assembly.parameter-constraint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.parameter-constraint@v1"))]
        pub parameter_constraint: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.assembly.parameter-guideline`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.parameter-guideline@v1"))]
        pub parameter_guideline: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.assembly.parameter-selection`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.parameter-selection@v1"))]
        pub parameter_selection: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.assembly.part`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.part@v1"))]
        pub part: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.assembly.test`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.test@v1"))]
        pub test: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.field.choice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.choice@v1"))]
        pub choice: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.field.control-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.control-id@v1"))]
        pub control_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.field.depends-on`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.depends-on@v1"))]
        pub depends_on: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.field.expression`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.expression@v1"))]
        pub expression: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.field.how-many`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.how-many@v1"))]
        pub how_many: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.field.label`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.label@v1"))]
        pub label: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.field.parameter-value`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.parameter-value@v1"))]
        pub parameter_value: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.field.pattern`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.pattern@v1"))]
        pub pattern: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.field.usage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.usage@v1"))]
        pub usage: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.field.with-child-controls`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.with-child-controls@v1"))]
        pub with_child_controls: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_control-common_metaschema.field.with-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.with-id@v1"))]
        pub with_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.assembly.authorized-privilege`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.authorized-privilege@v1"))]
        pub authorized_privilege: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.assembly.implementation-status`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.implementation-status@v1"))]
        pub implementation_status: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.assembly.implemented-component`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.implemented-component@v1"))]
        pub implemented_component: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.assembly.inventory-item`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.inventory-item@v1"))]
        pub inventory_item: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.assembly.only-statement`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.only-statement@v1"))]
        pub only_statement: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.assembly.port-range`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.port-range@v1"))]
        pub port_range: Option<u64>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.assembly.protocol`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.protocol@v1"))]
        pub protocol: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.assembly.set-parameter`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.set-parameter@v1"))]
        pub set_parameter: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.assembly.system-component`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.system-component@v1"))]
        pub system_component: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.assembly.system-user`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.system-user@v1"))]
        pub system_user: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.field.asset-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.asset-id@v1"))]
        pub asset_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.field.function-performed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.function-performed@v1"))]
        pub function_performed: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.field.identifier-type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.identifier-type@v1"))]
        pub identifier_type: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.field.param-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.param-id@v1"))]
        pub param_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.field.short-name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.short-name@v1"))]
        pub short_name: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.field.system-component-type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.system-component-type@v1"))]
        pub system_component_type: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.field.system-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.system-id@v1"))]
        pub system_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_implementation-common_metaschema.field.use`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.use-field@v1"))]
        pub use_field: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.assembly.confidence-score`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.confidence-score@v1"))]
        pub confidence_score: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.assembly.gap-summary`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.gap-summary@v1"))]
        pub gap_summary: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.assembly.map`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.map@v1"))]
        pub map: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.assembly.mapping`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.mapping@v1"))]
        pub mapping: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.assembly.mapping-item`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.mapping-item@v1"))]
        pub mapping_item: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.assembly.mapping-provenance`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.mapping-provenance@v1"))]
        pub mapping_provenance: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.assembly.mapping-resource-reference`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.mapping-resource-reference@v1"))]
        pub mapping_resource_reference: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.assembly.qualifier-item`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.qualifier-item@v1"))]
        pub qualifier_item: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.field.coverage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.coverage@v1"))]
        pub coverage: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.field.generation-method`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.generation-method@v1"))]
        pub generation_method: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.field.id-ref`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.id-ref@v1"))]
        pub id_ref: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.field.mapping-description`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.mapping-description@v1"))]
        pub mapping_description: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.field.match-pattern`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.match-pattern@v1"))]
        pub match_pattern: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.field.match-with-child-controls`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.match-with-child-controls@v1"))]
        pub match_with_child_controls: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.field.matching-rationale`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.matching-rationale@v1"))]
        pub matching_rationale: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.field.percentage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.percentage@v1"))]
        pub percentage: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.field.predicate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.predicate@v1"))]
        pub predicate: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping-common_metaschema.field.relationship`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.relationship@v1"))]
        pub relationship: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_mapping_metaschema.assembly.mapping-collection`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.mapping-collection@v1"))]
        pub mapping_collection: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.action`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.action@v1"))]
        pub action: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.address`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.address@v1"))]
        pub address: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.back-matter`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.back-matter@v1"))]
        pub back_matter: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.citation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.citation@v1"))]
        pub citation: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.link`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.link@v1"))]
        pub link: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.location`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.location@v1"))]
        pub location: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.metadata`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.metadata@v1"))]
        pub metadata: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.party`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.party@v1"))]
        pub party: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.property`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.property@v1"))]
        pub property: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.resource`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.resource@v1"))]
        pub resource: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.responsible-party`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.responsible-party@v1"))]
        pub responsible_party: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.responsible-role`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.responsible-role@v1"))]
        pub responsible_role: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.revision`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.revision@v1"))]
        pub revision: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.assembly.rlink`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.rlink@v1"))]
        pub rlink: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.addr-line`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.addr-line@v1"))]
        pub addr_line: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.algorithm`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.algorithm@v1"))]
        pub algorithm: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.base64`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.base64@v1"))]
        pub base64: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.city`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.city@v1"))]
        pub city: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.country`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.country@v1"))]
        pub country: Option<u64>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.document-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.document-id@v1"))]
        pub document_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.email-address`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.email-address@v1"))]
        pub email_address: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.external-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.external-id@v1"))]
        pub external_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.filename`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.filename@v1"))]
        pub filename: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.hash`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.hash@v1"))]
        pub hash: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.last-modified`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.last-modified@v1"))]
        pub last_modified: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.location-type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.location-type@v1"))]
        pub location_type: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.location-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.location-uuid@v1"))]
        pub location_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.media-type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.media-type@v1"))]
        pub media_type: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.member-of-organization`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.member-of-organization@v1"))]
        pub member_of_organization: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.oscal-version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.oscal-version@v1"))]
        pub oscal_version: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.postal-code`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.postal-code@v1"))]
        pub postal_code: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.published`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.published@v1"))]
        pub published: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.rel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.rel@v1"))]
        pub rel: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.remarks`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.remarks@v1"))]
        pub remarks: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.resource-fragment`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.resource-fragment@v1"))]
        pub resource_fragment: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.scheme`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.scheme@v1"))]
        pub scheme: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.telephone-number`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.telephone-number@v1"))]
        pub telephone_number: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.text`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.text@v1"))]
        pub text: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_metadata_metaschema.field.url`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.url@v1"))]
        pub url: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_poam_metaschema.assembly.plan-of-action-and-milestones`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.plan-of-action-and-milestones@v1"))]
        pub plan_of_action_and_milestones: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_poam_metaschema.assembly.poam-item`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.poam-item@v1"))]
        pub poam_item: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_poam_metaschema.assembly.related-finding`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.related-finding@v1"))]
        pub related_finding: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_poam_metaschema.field.finding-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.finding-uuid@v1"))]
        pub finding_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.assembly.add`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.add@v1"))]
        pub add: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.assembly.alter`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.alter@v1"))]
        pub alter: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.assembly.combine`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.combine@v1"))]
        pub combine: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.assembly.custom`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.custom@v1"))]
        pub custom: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.assembly.flat`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.flat@v1"))]
        pub flat: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.assembly.import`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.import@v1"))]
        pub import: Option<u64>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.assembly.insert-controls`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.insert-controls@v1"))]
        pub insert_controls: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.assembly.merge`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.merge@v1"))]
        pub merge: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.assembly.modify`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.modify@v1"))]
        pub modify: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.assembly.profile`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.profile@v1"))]
        pub profile: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.assembly.remove`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.remove@v1"))]
        pub remove: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.field.as-is`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.as-is@v1"))]
        pub as_is: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.field.by-class`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.by-class@v1"))]
        pub by_class: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.field.by-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.by-id@v1"))]
        pub by_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.field.by-item-name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.by-item-name@v1"))]
        pub by_item_name: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.field.by-name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.by-name@v1"))]
        pub by_name: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.field.by-ns`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.by-ns@v1"))]
        pub by_ns: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.field.order`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.order@v1"))]
        pub order: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_profile_metaschema.field.position`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.position@v1"))]
        pub position: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.authorization-boundary`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.authorization-boundary@v1"))]
        pub authorization_boundary: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.by-component`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.by-component@v1"))]
        pub by_component: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.categorization`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.categorization@v1"))]
        pub categorization: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.data-flow`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.data-flow@v1"))]
        pub data_flow: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.diagram`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.diagram@v1"))]
        pub diagram: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.export`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.export@v1"))]
        pub export: Option<u64>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.impact`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.impact@v1"))]
        pub impact: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.import-profile`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.import-profile@v1"))]
        pub import_profile: Option<u64>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.information-type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.information-type@v1"))]
        pub information_type: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.inherited`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.inherited@v1"))]
        pub inherited: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.leveraged-authorization`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.leveraged-authorization@v1"))]
        pub leveraged_authorization: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.network-architecture`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.network-architecture@v1"))]
        pub network_architecture: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.provided`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.provided@v1"))]
        pub provided: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.responsibility`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.responsibility@v1"))]
        pub responsibility: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.satisfied`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.satisfied@v1"))]
        pub satisfied: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.security-impact-level`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.security-impact-level@v1"))]
        pub security_impact_level: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.system-characteristics`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.system-characteristics@v1"))]
        pub system_characteristics: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.system-implementation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.system-implementation@v1"))]
        pub system_implementation: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.system-information`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.system-information@v1"))]
        pub system_information: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.assembly.system-security-plan`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.system-security-plan@v1"))]
        pub system_security_plan: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.adjustment-justification`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.adjustment-justification@v1"))]
        pub adjustment_justification: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.base`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.base@v1"))]
        pub base: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.caption`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.caption@v1"))]
        pub caption: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.date-authorized`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.date-authorized@v1"))]
        pub date_authorized: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.information-type-id`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.information-type-id@v1"))]
        pub information_type_id: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.provided-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.provided-uuid@v1"))]
        pub provided_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.responsibility-uuid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.responsibility-uuid@v1"))]
        pub responsibility_uuid: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.security-objective-availability`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.security-objective-availability@v1"))]
        pub security_objective_availability: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.security-objective-confidentiality`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.security-objective-confidentiality@v1"))]
        pub security_objective_confidentiality: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.security-objective-integrity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.security-objective-integrity@v1"))]
        pub security_objective_integrity: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.security-sensitivity-level`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.security-sensitivity-level@v1"))]
        pub security_sensitivity_level: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.selected`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.selected@v1"))]
        pub selected: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.system-name`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.system-name@v1"))]
        pub system_name: Option<String>,

        /// Discovered from Repomix path `xml.src.metaschema.oscal_ssp_metaschema.field.system-name-short`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.oscal-subid-registry.system-name-short@v1"))]
        pub system_name_short: Option<String>,
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
