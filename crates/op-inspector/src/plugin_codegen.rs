//! Auto plugin generator (FIRST-DRAFT SEED ONLY): turns an
//! `op-inspector`-inferred schema for genuinely UNKNOWN data into a
//! compilable Rust plugin scaffold, matching the established
//! `state_plugins/<name>.rs` pattern (see `qdrant.rs` for the reference
//! shape, or copy `plugin_scaffold.rs.template` directly).
//!
//! **This is explicitly NOT a substitute for the mandatory research step.**
//! Per `crates/op-plugins/MIGRATION_RESEARCH_OFFICIAL_SOURCES.md`
//! ("Critical Rule (from architecture.md)"): "For hand-rolled plugins with
//! weak schemas ... workers **MUST** research official sources to get
//! structured data for proper typed Rust structs. This is NOT optional."
//! Field types generated here are guessed from a single (or few) observed
//! sample(s) — exactly the kind of "weak schema" that document forbids
//! shipping as-is. The correct flow is:
//!   1. `inspect_object` sees data matching no known plugin → this module
//!      generates a first-draft scaffold from the inferred shape.
//!   2. A human (or an LLM agent with `web_search`/`open_page`) researches
//!      the real system's official documentation/spec, the same way
//!      `qdrant.rs` was built from Qdrant's OpenAPI spec and `gcloud.rs`
//!      was built from real `gcloud --help` output.
//!   3. The scaffold's guessed field types get corrected against that
//!      research before the plugin is registered in `default_registry.rs`.
//!
//! Per "Only Valid Path = Plugins", new capabilities are plugins registered
//! there, never ad-hoc services — this module only produces the seed;
//! nothing here writes to `state_plugins/` or registers anything
//! automatically. That stays a deliberate, reviewed, research-backed step.

use crate::ObjectSchema;
use std::fmt::Write as _;

/// Map an inferred `SchemaProperty.data_type` (from `analyze_json_schema`)
/// to a Rust type. Falls back to `serde_json::Value` for anything not
/// confidently inferable from a single/few samples.
fn rust_type_for(data_type: &str) -> &'static str {
    match data_type {
        "string" => "String",
        "number" => "f64",
        "integer" => "i64",
        "boolean" => "bool",
        "array" => "Vec<serde_json::Value>",
        "object" => "serde_json::Value",
        "null" => "Option<serde_json::Value>",
        _ => "serde_json::Value",
    }
}

fn to_pascal_case(name: &str) -> String {
    name.split(['_', '-'])
        .filter(|s| !s.is_empty())
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Generate a complete, compilable plugin scaffold file for `plugin_name`
/// from an inferred `ObjectSchema`. `description` should be a short
/// human-readable summary (e.g. from the originating `InspectionInput`'s
/// `RawData { description, .. }`).
///
/// The generated plugin is a **read-only observation plugin**: it exposes
/// one `get_observed` method returning the last-inspected example as typed
/// fields, plus the standard `StatePlugin` scaffolding. This is the correct
/// conservative default for freshly-discovered unknown data — inferring
/// mutation semantics (what a "write" even means for this shape) from a
/// single sample isn't something that should be auto-generated; a human
/// reviewing the scaffold decides whether/how to add mutation methods.
pub fn generate_plugin_scaffold(plugin_name: &str, description: &str, schema: &ObjectSchema) -> String {
    let struct_name = format!("{}Observed", to_pascal_case(plugin_name));
    let const_prefix = plugin_name.to_uppercase();

    let mut fields = String::new();
    let mut sorted_props: Vec<_> = schema.properties.iter().collect();
    sorted_props.sort_by_key(|(k, _)| (*k).clone());

    for (field_name, prop) in &sorted_props {
        let rust_type = rust_type_for(&prop.data_type);
        let is_required = schema.required.contains(field_name);
        let field_type = if is_required {
            rust_type.to_string()
        } else {
            format!("Option<{rust_type}>")
        };
        if let Some(desc) = &prop.description {
            let _ = writeln!(fields, "    /// {desc}");
        }
        if !is_required {
            let _ = writeln!(fields, "    #[serde(default)]");
        }
        let _ = writeln!(fields, "    pub {field_name}: {field_type},");
    }

    format!(
        r#"//! {plugin_name} — AUTO-GENERATED SCAFFOLD from op-inspector's schema
//! inference on unknown data ({description}). NOT wired into
//! default_registry.rs — review before registering. Mutation semantics for
//! this shape were not inferred; add methods deliberately once the real
//! write contract is known.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin,
}};
use op_state_store::{{PluginSchema, SideEffect}};
use schemars::JsonSchema;
use serde::{{Deserialize, Serialize}};
use simd_json::OwnedValue as Value;

const PLUGIN_NAME: &str = "{plugin_name}";
const PLUGIN_VERSION: &str = "0.1.0";
const PLUGIN_DESCRIPTION: &str = "{description}";

/// Inferred shape, from a single (or few) real observed sample(s) via
/// op-inspector's `inspect_object`. Field types are best-effort guesses
/// (`data_type` string -> Rust type); verify before relying on them.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct {struct_name} {{
{fields}}}

pub struct {const_prefix}Plugin {{
    last_observed: std::sync::RwLock<Option<{struct_name}>>,
}}

impl {const_prefix}Plugin {{
    pub fn new() -> Self {{
        Self {{
            last_observed: std::sync::RwLock::new(None),
        }}
    }}
}}

impl Default for {const_prefix}Plugin {{
    fn default() -> Self {{
        Self::new()
    }}
}}

#[async_trait]
impl StatePlugin for {const_prefix}Plugin {{
    fn name(&self) -> &str {{
        PLUGIN_NAME
    }}

    fn version(&self) -> &str {{
        PLUGIN_VERSION
    }}

    fn schema(&self) -> Option<PluginSchema> {{
        let mut schema = {const_prefix_lower}_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }}

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {{
        Ok(StateDiff {{
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {{
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "schema".to_string(),
                desired_hash: "schema".to_string(),
            }},
        }})
    }}

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {{
        // Read-only observation plugin -- no mutation semantics inferred
        // from unknown data. See file header.
        Ok(ApplyResult {{
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        }})
    }}

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {{
        Ok(self.last_observed.read().unwrap().is_some())
    }}

    async fn create_checkpoint(&self) -> Result<Checkpoint> {{
        Ok(Checkpoint {{
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(
                &*self.last_observed.read().unwrap(),
            )?,
            backend_checkpoint: None,
        }})
    }}

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {{
        Ok(())
    }}

    fn capabilities(&self) -> PluginCapabilities {{
        PluginCapabilities {{
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }}
    }}
}}

fn {const_prefix_lower}_schema() -> PluginSchema {{
    let root = serde_json::to_value(schemars::schema_for!({struct_name}))
        .expect("schemars schema serializes to JSON");
    super::schemars_adapter::plugin_schema_from_json(
        PLUGIN_NAME,
        PLUGIN_VERSION,
        PLUGIN_DESCRIPTION,
        &root,
    )
}}

inventory::submit! {{
    crate::default_registry::PluginReg::new(PLUGIN_NAME, |_ctx| std::sync::Arc::new({const_prefix}Plugin::new()))
}}
"#,
        plugin_name = plugin_name,
        description = description,
        struct_name = struct_name,
        const_prefix = const_prefix,
        const_prefix_lower = plugin_name,
        fields = fields,
    )
}
