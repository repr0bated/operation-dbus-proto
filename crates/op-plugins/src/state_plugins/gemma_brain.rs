//! Gemma brain — the umbrella cognitive plugin.
//!
//! Gemma does multiple things, so this is a "brain", not a "router": it owns the
//! tag-routing / subid-classification configuration, the four reasoning
//! perspectives, and the UI-spec gallery surface. It is the WHAT-to-decide
//! surface; it **consumes** the generic [`super::large_language_model`] plugin
//! for inference and embeds no model of its own ("gemma" here is a brand for the
//! brain, not a model binding — swap the underlying model via the
//! `large_language_model` plugin).
//!
//! Inference and gallery generation live in the `op-gemma` binary → Ollama. This
//! plugin reports the live observed state (gallery population, op-gemma
//! availability) and the operator-set routing configuration. Mutations
//! (regenerate / delete-spec) and the routing-decision execution are routed by
//! the MutationEngine, not performed in this plugin's `apply_state` (which, like
//! every StatePlugin, is declarative). The schema is published to the SHM
//! folder/manifest by op-projection's enumerator.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

/// Path to the UI-spec gallery produced by the `op-gemma` binary.
const GEMMA_SPECS_PATH: &str = "/dev/shm/gemma-ui-specs.json";

/// Tag-routing / subid-classification configuration owned by the brain.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.gemma-brain.routing.schema@v1"))]
pub struct GemmaRouting {
    /// Whether Gemma performs OSCAL subid classification.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.gemma-brain.subid-classification@v1"))]
    pub subid_classification: bool,
    /// Whether Gemma performs xray/OpenFlow tag routing.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.gemma-brain.tag-routing@v1"))]
    pub tag_routing: bool,
    /// xray config the routing decisions are written into.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.service.gemma-brain.xray-config@v1"))]
    pub xray_config_path: String,
}

/// UI-spec gallery surface (the artifact the lovable SPA renders).
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.gemma-brain.gallery.schema@v1"))]
pub struct GemmaGallery {
    /// Tmpfs path of the gallery JSON produced by `op-gemma`.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.service.gemma-brain.gallery-path@v1"))]
    pub specs_path: String,
    /// Target number of specs the gallery is topped up to.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.gemma-brain.target-count@v1"))]
    pub target_count: u32,
    /// Current number of specs present (observed live).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.gemma-brain.spec-count@v1"))]
    pub spec_count: u32,
}

/// Top-level state for the Gemma brain.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.gemma-brain.schema@v1"))]
#[schemars(extend("x-oscal-category" = "llm"))]
pub struct GemmaBrainState {
    /// Derived operational status (observed): `unavailable` (op-gemma binary
    /// missing), `empty`, `partial`, or `ready`.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.gemma-brain.status@v1"))]
    pub status: String,
    /// The generic LLM plugin this brain consumes for inference. Not a model —
    /// the model is selected inside that plugin.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.gemma-brain.llm-plugin@v1"))]
    pub llm_plugin: String,
    /// Enabled reasoning perspectives (the four axes).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.gemma-brain.perspectives@v1"))]
    pub perspectives: Vec<String>,
    /// Routing / subid-classification configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.service.gemma-brain.routing@v1"))]
    pub routing: GemmaRouting,
    /// UI-spec gallery surface.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.gemma-brain.gallery@v1"))]
    pub gallery: GemmaGallery,
}

pub struct GemmaBrainPlugin;

impl Default for GemmaBrainPlugin {
    fn default() -> Self {
        Self
    }
}

impl GemmaBrainPlugin {
    pub fn new() -> Self {
        Self
    }

    fn env_or(key: &str, fallback: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| fallback.to_string())
    }

    fn env_bool(key: &str, fallback: bool) -> bool {
        std::env::var(key)
            .ok()
            .and_then(|v| match v.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => Some(true),
                "0" | "false" | "no" | "off" => Some(false),
                _ => None,
            })
            .unwrap_or(fallback)
    }

    fn env_u32(key: &str, fallback: u32) -> u32 {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(fallback)
    }

    /// Path of the `op-gemma` generator binary.
    fn gemma_binary() -> String {
        Self::env_or("GEMMA_BINARY_PATH", "/usr/local/bin/op-gemma")
    }

    /// Count the specs currently in the gallery artifact (0 if absent/unreadable).
    fn gallery_spec_count() -> u32 {
        std::fs::read(GEMMA_SPECS_PATH)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|v| {
                v.get("specs")
                    .and_then(serde_json::Value::as_array)
                    .map(|a| a.len() as u32)
            })
            .unwrap_or(0)
    }

    /// Build the live state: observed fields are read from the system, config
    /// fields from the environment (with sensible defaults).
    pub fn current_state() -> GemmaBrainState {
        let target_count = Self::env_u32("GEMMA_TARGET_COUNT", 200);
        let spec_count = Self::gallery_spec_count();
        let binary_present = std::path::Path::new(&Self::gemma_binary()).exists();

        let status = if !binary_present {
            "unavailable"
        } else if spec_count == 0 {
            "empty"
        } else if spec_count >= target_count {
            "ready"
        } else {
            "partial"
        }
        .to_string();

        GemmaBrainState {
            status,
            llm_plugin: Self::env_or("GEMMA_LLM_PLUGIN", "large_language_model"),
            perspectives: vec![
                "data_numeric".to_string(),
                "spatial_layout".to_string(),
                "user_flow".to_string(),
                "context_aesthetic".to_string(),
            ],
            routing: GemmaRouting {
                subid_classification: Self::env_bool("GEMMA_SUBID_CLASSIFICATION", true),
                tag_routing: Self::env_bool("GEMMA_TAG_ROUTING", true),
                xray_config_path: Self::env_or("GEMMA_XRAY_CONFIG", "/etc/xray/xray_config.json"),
            },
            gallery: GemmaGallery {
                specs_path: GEMMA_SPECS_PATH.to_string(),
                target_count,
                spec_count,
            },
        }
    }
}

#[async_trait]
impl StatePlugin for GemmaBrainPlugin {
    fn name(&self) -> &str {
        "gemma_brain"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    /// Available only when the `op-gemma` generator binary is installed.
    fn is_available(&self) -> bool {
        std::path::Path::new(&Self::gemma_binary()).exists()
    }

    fn unavailable_reason(&self) -> String {
        format!(
            "op-gemma generator binary not found at {}",
            Self::gemma_binary()
        )
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = gemma_brain_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "schema-declared".to_string(),
                desired_hash: "schema-declared".to_string(),
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
            state_snapshot: simd_json::serde::to_owned_value(Self::current_state())?,
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

/// Canonical `gemma_brain` schema derived from [`GemmaBrainState`] via schemars.
pub(crate) fn gemma_brain_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(GemmaBrainState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "gemma_brain",
        "1.0.0",
        "Gemma cognitive brain: tag routing, subid classification, reasoning perspectives, and UI-spec gallery; consumes the large_language_model plugin for inference",
        &root,
    );

    // Output structs
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ClassifyOutput {
        pub classification: serde_json::Value,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RouteOutput {
        pub route: serde_json::Value,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListPerspectivesOutput {
        pub perspectives: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetUiSpecOutput {
        pub spec: Option<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListTagsOutput {
        pub tags: Vec<serde_json::Value>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AnalyzeIntentOutput {
        pub intent: serde_json::Value,
    }

    // Add methods
    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "classify".to_string(),
        method_decl_from_schemars_with_output::<(), ClassifyOutput>(
            "classify",
            SideEffect::Mutation,
            false,
            "gemma.invoke",
            "mut.service.gemma.classify@v1",
        ),
    );
    schema.methods.insert(
        "route".to_string(),
        method_decl_from_schemars_with_output::<(), RouteOutput>(
            "route",
            SideEffect::Mutation,
            false,
            "gemma.invoke",
            "mut.service.gemma.route@v1",
        ),
    );
    schema.methods.insert(
        "list_perspectives".to_string(),
        method_decl_from_schemars_with_output::<(), ListPerspectivesOutput>(
            "list_perspectives",
            SideEffect::Read,
            true,
            "gemma.read",
            "obs.service.gemma.perspective.list@v1",
        ),
    );
    schema.methods.insert(
        "get_ui_spec".to_string(),
        method_decl_from_schemars_with_output::<(), GetUiSpecOutput>(
            "get_ui_spec",
            SideEffect::Read,
            true,
            "gemma.read",
            "obs.service.gemma.ui-spec.get@v1",
        ),
    );
    schema.methods.insert(
        "list_tags".to_string(),
        method_decl_from_schemars_with_output::<(), ListTagsOutput>(
            "list_tags",
            SideEffect::Read,
            true,
            "gemma.read",
            "obs.service.gemma.tag.list@v1",
        ),
    );
    schema.methods.insert(
        "register_tag".to_string(),
        method_decl_from_schemars_with_output::<(), AckOutput>(
            "register_tag",
            SideEffect::Mutation,
            false,
            "gemma.invoke",
            "mut.service.gemma.tag.register@v1",
        ),
    );
    schema.methods.insert(
        "analyze_intent".to_string(),
        method_decl_from_schemars_with_output::<(), AnalyzeIntentOutput>(
            "analyze_intent",
            SideEffect::Mutation,
            false,
            "gemma.invoke",
            "mut.service.gemma.intent.analyze@v1",
        ),
    );

    schema.capabilities.insert(
        "gemma.invoke".to_string(),
        op_state_store::CapabilityDecl {
            id: "gemma.invoke".to_string(),
            description: "Grants: classify, route, register_tag, analyze_intent.".to_string(),
        },
    );
    schema.capabilities.insert(
        "gemma.read".to_string(),
        op_state_store::CapabilityDecl {
            id: "gemma.read".to_string(),
            description: "Grants: list_perspectives, get_ui_spec, list_tags.".to_string(),
        },
    );

    schema
}

/// Mutation-path dispatch for UI/control-plane reads. Domain results are shaped
/// for zeroclaw-gui page binds (not a silent echo of empty args).
pub fn dispatch_gemma_brain_method(
    method: &str,
    state: &GemmaBrainState,
) -> Result<serde_json::Value> {
    match method {
        "get_ui_spec" => {
            let last_mint = std::fs::metadata(GEMMA_SPECS_PATH)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let secs = t
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    chrono::DateTime::from_timestamp(secs, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            Ok(serde_json::json!({
                "catalog": {
                    "status": state.status,
                    "elements": state.gallery.spec_count,
                    "last_mint": last_mint,
                },
                "plugins": [
                    { "name": "gemma_brain", "status": state.status },
                    { "name": state.llm_plugin, "status": "ok" },
                    { "name": "json_render", "status": "ok" },
                ],
                "spec": {
                    "perspectives": state.perspectives,
                    "gallery": state.gallery,
                    "routing": state.routing,
                },
            }))
        }
        "list_perspectives" => Ok(serde_json::json!({
            "perspectives": state.perspectives,
        })),
        "list_tags" => Ok(serde_json::json!({
            "tags": [],
        })),
        "classify" | "route" | "register_tag" | "analyze_intent" => Err(anyhow::anyhow!(
            "gemma_brain.{method} requires a live op-gemma runtime; not available via echo"
        )),
        other => Err(anyhow::anyhow!(
            "gemma_brain method is not declared for mutation dispatch: {other}"
        )),
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("gemma_brain", |_ctx| std::sync::Arc::new(GemmaBrainPlugin::new()))
}
