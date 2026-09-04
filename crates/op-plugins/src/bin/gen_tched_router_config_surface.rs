//! Generator: emit `tched_router_config_surface.rs` from the OFFICIAL zeroclaw
//! binary's own config schema.
//!
//! The **binary is the single source of truth** — not a linked crate. The
//! canonical document is captured at `schemas/zeroclaw/config.schema.json` by
//! running the shipped binary:
//!
//! ```text
//! /usr/bin/zeroclaw config schema > schemas/zeroclaw/config.schema.json
//! /usr/bin/zeroclaw --version | awk '{print $2}' > schemas/zeroclaw/VERSION
//! cargo run -p op-plugins --bin gen_tched_router_config_surface
//! ```
//!
//! This mirrors how `emqx.rs` works: the plugin declares its own schema and
//! drives the official binary, rather than linking an upstream crate whose
//! version may not match what is deployed. It also satisfies the invariant the
//! `upstream_schema_tests` drift alarm in `tched_router.rs` already asserted —
//! that our surface must equal what `zeroclaw config schema` prints — by making
//! that document the input instead of something to compare against.
//!
//! Every config section becomes a `Get<Section>Config` method whose output is
//! typed by the section's real JSON Schema (self-contained, with just the
//! `$defs` it transitively needs), plus `GetConfig` (full, secret-masked) and
//! `PatchConfig` (validated mutation). Local 3tched Router configuration
//! structs are emitted as plain config sections too ("configurable options are
//! just configurations") and keep their real Rust types.

use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// One config section discovered from the official schema document.
struct Section {
    /// serde key on the zeroclaw config (e.g. "gateway", "channels").
    key: String,
    /// Self-contained JSON Schema for this section (property + needed `$defs`).
    schema: Value,
}

/// Local (3tched Router-owned) configuration sections — plain configurations,
/// not a separate "options" projection. Types live in `super::tched_router`.
const LOCAL_SECTIONS: &[(&str, &str)] = &[
    ("model_assignments", "super::tched_router::ModelAssignments"),
    (
        "memory_namespaces",
        "Vec<super::tched_router::MemoryNamespaceOption>",
    ),
    (
        "registration_service",
        "super::tched_router::RegistrationServiceSchema",
    ),
    (
        "user_container",
        "super::tched_router::UserContainerOptions",
    ),
    ("identity_chain", "super::tched_router::IdentityOptions"),
    ("privacy_policy", "super::tched_router::PrivacyOptions"),
];

fn pascal(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn kebab(s: &str) -> String {
    s.replace('_', "-")
}

/// `Get` + Pascal(section) + `Config`, without doubling a trailing `Config`.
fn method_name(section_key: &str) -> String {
    let p = pascal(section_key);
    if p.ends_with("Config") {
        format!("Get{p}")
    } else {
        format!("Get{p}Config")
    }
}

/// Collect every `#/$defs/<Name>` referenced anywhere under `node`.
fn refs_in(node: &Value, out: &mut BTreeSet<String>) {
    match node {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref") {
                if let Some(name) = r.strip_prefix("#/$defs/") {
                    out.insert(name.to_string());
                }
            }
            for value in map.values() {
                refs_in(value, out);
            }
        }
        Value::Array(items) => {
            for value in items {
                refs_in(value, out);
            }
        }
        _ => {}
    }
}

/// Transitive closure of `$defs` names reachable from `start`.
fn closure(start: &Value, defs: &Map<String, Value>) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut frontier: BTreeSet<String> = BTreeSet::new();
    refs_in(start, &mut frontier);
    while let Some(name) = frontier.pop_first() {
        if seen.contains(&name) {
            continue;
        }
        let Some(def) = defs.get(&name) else { continue };
        seen.insert(name);
        let mut next = BTreeSet::new();
        refs_in(def, &mut next);
        for candidate in next {
            if !seen.contains(&candidate) {
                frontier.insert(candidate);
            }
        }
    }
    seen
}

/// Build a self-contained schema: the section's own property schema plus only
/// the `$defs` it transitively needs, so each section stands alone.
fn self_contained(prop: &Value, defs: &Map<String, Value>) -> Value {
    let needed = closure(prop, defs);
    let mut out = match prop {
        Value::Object(map) => map.clone(),
        other => {
            let mut map = Map::new();
            map.insert("allOf".into(), Value::Array(vec![other.clone()]));
            map
        }
    };
    if !needed.is_empty() {
        let mut sub = Map::new();
        for name in needed {
            if let Some(def) = defs.get(&name) {
                sub.insert(name, def.clone());
            }
        }
        out.insert("$defs".into(), Value::Object(sub));
    }
    Value::Object(out)
}

/// Extract sections from the official config schema document.
fn discover_sections(schema: &Value, defs: &Map<String, Value>) -> Vec<Section> {
    let props = schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("config schema has properties");
    let mut out = Vec::new();
    for (key, prop) in props {
        let kind = prop.get("type").and_then(Value::as_str);
        let has_ref = prop.get("$ref").is_some();
        // Scalars stay reachable through `GetConfig`; they get no method of
        // their own, matching the previous surface.
        if !has_ref && !matches!(kind, Some("array") | Some("object")) {
            continue;
        }
        out.push(Section {
            key: key.clone(),
            schema: self_contained(prop, defs),
        });
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

/// Rust fn name carrying a section's embedded schema.
fn schema_fn(key: &str) -> String {
    format!("{}_section_schema", key.replace('-', "_"))
}

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest
        .parent()
        .and_then(Path::parent)
        .expect("crates/op-plugins has a repo root")
        .to_path_buf();
    let schema_path = repo_root.join("schemas/zeroclaw/config.schema.json");
    let sections_dir = repo_root.join("schemas/zeroclaw/sections");

    let out_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("src/state_plugins/tched_router_config_surface.rs"));

    let version = std::fs::read_to_string(repo_root.join("schemas/zeroclaw/VERSION"))
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let raw = std::fs::read_to_string(&schema_path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\n\
             Capture it from the OFFICIAL binary first:\n  \
             zeroclaw config schema > schemas/zeroclaw/config.schema.json",
            schema_path.display()
        )
    });
    let document: Value = serde_json::from_str(&raw).expect("config schema is valid JSON");

    let defs = document
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let prop_count = document
        .get("properties")
        .and_then(Value::as_object)
        .map(|m| m.len())
        .unwrap_or(0);

    // ── Stub guard ──────────────────────────────────────────────────────────
    // Refuse to emit from an empty document. A stubbed-out source silently
    // deletes the entire config surface, which is exactly how this file lost
    // its meaning before: `vendor/zeroclawlabs` declared 54 empty structs and
    // every generated method carried a payload with no fields.
    assert!(
        prop_count >= 32 && defs.len() >= 64,
        "REFUSING TO GENERATE: {} has {prop_count} properties and {} $defs, which is far \
         too small to be a real zeroclaw config schema. Regenerating from a stub or a \
         truncated dump would silently delete the entire config surface. Re-capture it \
         from the official binary:\n  zeroclaw config schema > schemas/zeroclaw/config.schema.json",
        schema_path.display(),
        defs.len(),
    );

    let upstream = discover_sections(&document, &defs);
    assert!(
        !upstream.is_empty(),
        "REFUSING TO GENERATE: no config sections discovered in {}",
        schema_path.display()
    );

    // ── Emit per-section schema documents ───────────────────────────────────
    std::fs::create_dir_all(&sections_dir)
        .unwrap_or_else(|e| panic!("create {}: {e}", sections_dir.display()));
    for existing in std::fs::read_dir(&sections_dir)
        .into_iter()
        .flatten()
        .flatten()
    {
        if existing.path().extension().is_some_and(|e| e == "json") {
            let _ = std::fs::remove_file(existing.path());
        }
    }
    for sec in &upstream {
        let path = sections_dir.join(format!("{}.json", sec.key));
        let body = serde_json::to_string_pretty(&sec.schema).expect("section schema serializes");
        std::fs::write(&path, body).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    }
    let full_path = sections_dir.join("_full.json");
    std::fs::write(
        &full_path,
        serde_json::to_string_pretty(&document).expect("document serializes"),
    )
    .unwrap_or_else(|e| panic!("write {}: {e}", full_path.display()));

    // ── Emit the Rust surface ───────────────────────────────────────────────
    let mut s = String::new();
    let _ = write!(
        s,
        "//! GENERATED by `cargo run -p op-plugins --bin gen_tched_router_config_surface`.\n\
         //! DO NOT EDIT — regenerate after upgrading the zeroclaw binary.\n\
         //!\n\
         //! Source of truth: `zeroclaw config schema` from the OFFICIAL binary\n\
         //! (v{version}), captured at `schemas/zeroclaw/config.schema.json`. There is no\n\
         //! `zeroclaw` crate dependency — the plugin declares its own schema and drives\n\
         //! the shipped binary, exactly like `emqx.rs`.\n\
         //!\n\
         //! Complete 3tched Router config surface: every config section as a\n\
         //! `Get<Section>Config` method typed by that section's real JSON Schema, plus\n\
         //! `GetConfig` (full, secret-masked) and `PatchConfig` (validated mutation).\n\
         //! Local 3tched Router configuration structs are plain config sections here —\n\
         //! there is no separate \"configurable options\" projection. No cache, no\n\
         //! projection: reads parse the current config; mutations land in state.\n\n"
    );
    s.push_str("use super::common::errors::TchedRouterError;\n");
    s.push_str("use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;\n");
    s.push_str("use super::tched_router::{DispatchOutcome, TchedRouterState};\n");
    s.push_str("use op_state_store::{CapabilityDecl, PluginSchema, SideEffect};\n");
    s.push_str("use serde::{Deserialize, Serialize};\n");
    s.push_str("use serde_json::Value as JsonValue;\n\n");

    let _ = write!(
        s,
        "/// Version of the official zeroclaw binary this surface was generated from.\n\
         pub const ZEROCLAW_SCHEMA_VERSION: &str = \"{version}\";\n\n"
    );

    s.push_str(
        "/// Parse an embedded section schema document into a schemars `Schema`.\n\
         fn embedded(raw: &str) -> schemars::Schema {\n\
         \x20   let value: JsonValue = serde_json::from_str(raw)\n\
         \x20       .expect(\"embedded zeroclaw section schema is valid JSON\");\n\
         \x20   schemars::Schema::try_from(value)\n\
         \x20       .expect(\"embedded zeroclaw section schema is a valid JSON Schema\")\n\
         }\n\n",
    );

    // Shared input/output structs.
    s.push_str(
        "/// Empty input for config read methods.\n\
         #[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]\n\
         pub struct ConfigMethodInput {}\n\n",
    );
    s.push_str(
        "/// Full configuration schema, as printed by `zeroclaw config schema`.\n\
         pub fn full_config_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {\n\
         \x20   embedded(include_str!(concat!(\n\
         \x20       env!(\"CARGO_MANIFEST_DIR\"),\n\
         \x20       \"/../../schemas/zeroclaw/sections/_full.json\"\n\
         \x20   )))\n\
         }\n\n",
    );
    s.push_str(
        "/// Output for `GetConfig` — the full configuration, typed by the official\n\
         /// binary's own schema. Secrets are masked by dispatch.\n\
         #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]\n\
         #[schemars(extend(\"x-oscal-subid\" = \"exp.service.3tched-router.config.result@v1\"))]\n\
         pub struct GetConfigOutput {\n\
         \x20   /// Full configuration.\n\
         \x20   #[schemars(schema_with = \"full_config_schema\")]\n\
         \x20   pub config: JsonValue,\n\
         }\n\n",
    );
    s.push_str(
        "/// Input for `PatchConfig`: replace one config section.\n\
         #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]\n\
         pub struct PatchConfigInput {\n\
         \x20   /// Section key (e.g. `gateway`, `memory`, `channels`).\n\
         \x20   pub section: String,\n\
         \x20   /// New section value; validated against the official section schema.\n\
         \x20   pub value: JsonValue,\n\
         }\n\n\
         /// Output for `PatchConfig` — the validated value that was applied.\n\
         #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]\n\
         pub struct PatchConfigOutput {\n\
         \x20   /// Section that was patched.\n\
         \x20   pub section: String,\n\
         \x20   /// Validated value now in effect.\n\
         \x20   pub value: JsonValue,\n\
         }\n\n",
    );

    // Per-section schema accessors.
    s.push_str("// ── Section schemas (from the official binary) ──────────────────────────\n\n");
    for sec in &upstream {
        let f = schema_fn(&sec.key);
        let _ = write!(
            s,
            "/// JSON Schema for the `{}` config section.\n\
             pub fn {f}(_: &mut schemars::SchemaGenerator) -> schemars::Schema {{\n\
             \x20   embedded(include_str!(concat!(\n\
             \x20       env!(\"CARGO_MANIFEST_DIR\"),\n\
             \x20       \"/../../schemas/zeroclaw/sections/{}.json\"\n\
             \x20   )))\n\
             }}\n\n",
            sec.key, sec.key
        );
    }

    // Per-section output structs.
    let mut all: Vec<(String, Option<String>)> = Vec::new(); // (key, local rust type)
    for sec in &upstream {
        all.push((sec.key.clone(), None));
    }
    for (key, ty) in LOCAL_SECTIONS {
        all.push(((*key).to_string(), Some((*ty).to_string())));
    }

    s.push_str("// ── Per-section outputs ────────────────────────────────────────────────\n\n");
    for (key, local) in &all {
        let m = method_name(key);
        match local {
            Some(ty) => {
                let _ = write!(
                    s,
                    "/// Output for `{m}`.\n\
                     #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]\n\
                     #[schemars(extend(\"x-oscal-subid\" = \"exp.service.3tched-router.{}-config.result@v1\"))]\n\
                     pub struct {m}Output {{\n\
                     \x20   /// Current `{key}` configuration.\n\
                     \x20   pub config: {ty},\n\
                     }}\n\n",
                    kebab(key)
                );
            }
            None => {
                let f = schema_fn(key);
                let _ = write!(
                    s,
                    "/// Output for `{m}`.\n\
                     #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]\n\
                     #[schemars(extend(\"x-oscal-subid\" = \"exp.service.3tched-router.{}-config.result@v1\"))]\n\
                     pub struct {m}Output {{\n\
                     \x20   /// Current `{key}` configuration.\n\
                     \x20   #[schemars(schema_with = \"{f}\")]\n\
                     \x20   pub config: JsonValue,\n\
                     }}\n\n",
                    kebab(key)
                );
            }
        }
    }

    // Method table + registration.
    s.push_str("/// (method, section key, source) for every config read.\n");
    s.push_str("pub const SECTIONS: &[(&str, &str, SectionSource)] = &[\n");
    for (key, local) in &all {
        let src = if local.is_some() { "Local" } else { "Upstream" };
        let _ = writeln!(
            s,
            "    (\"{}\", \"{key}\", SectionSource::{src}),",
            method_name(key)
        );
    }
    s.push_str("];\n\n");
    s.push_str("/// Where a config section is read from.\n");
    s.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    s.push_str("pub enum SectionSource {\n    /// The zeroclaw config file, as described by the official binary's schema.\n    Upstream,\n    /// Local 3tched Router state (a plain configuration).\n    Local,\n}\n\n");

    s.push_str("/// Every method owned by this surface (for dispatch membership checks).\n");
    s.push_str("pub const CONFIG_METHODS: &[&str] = &[\n    \"GetConfig\",\n    \"PatchConfig\",\n");
    for (key, _local) in &all {
        let _ = writeln!(s, "    \"{}\",", method_name(key));
    }
    s.push_str("];\n\n");

    s.push_str(
        "/// Register every config method and its declared capability on the schema.\n\
         ///\n\
         /// The plugin is the single declaration point: each method's\n\
         /// `required_capability` has a matching `schema.capabilities` entry\n\
         /// (closure-clean under `validate_capability_closure`).\n\
         pub fn register_config_methods(schema: &mut PluginSchema) {\n",
    );
    s.push_str(
        "    schema.methods.insert(\n\
         \x20       \"GetConfig\".to_string(),\n\
         \x20       method_decl_from_schemars_with_output::<ConfigMethodInput, GetConfigOutput>(\n\
         \x20           \"GetConfig\",\n\
         \x20           SideEffect::Read,\n\
         \x20           true,\n\
         \x20           \"cap.software.3tched-router.config.read@v1\",\n\
         \x20           \"obs.service.3tched-router.config.get@v1\",\n\
         \x20       ),\n\
         \x20   );\n\
         \x20   schema.capabilities.insert(\n\
         \x20       \"cap.software.3tched-router.config.read@v1\".to_string(),\n\
         \x20       CapabilityDecl {\n\
         \x20           id: \"cap.software.3tched-router.config.read@v1\".to_string(),\n\
         \x20           description: \"Read the full 3tched Router configuration (secrets masked).\".to_string(),\n\
         \x20       },\n\
         \x20   );\n\
         \x20   schema.methods.insert(\n\
         \x20       \"PatchConfig\".to_string(),\n\
         \x20       method_decl_from_schemars_with_output::<PatchConfigInput, PatchConfigOutput>(\n\
         \x20           \"PatchConfig\",\n\
         \x20           SideEffect::Mutation,\n\
         \x20           false,\n\
         \x20           \"cap.software.3tched-router.config.write@v1\",\n\
         \x20           \"mut.service.3tched-router.config.patch@v1\",\n\
         \x20       ),\n\
         \x20   );\n\
         \x20   schema.capabilities.insert(\n\
         \x20       \"cap.software.3tched-router.config.write@v1\".to_string(),\n\
         \x20       CapabilityDecl {\n\
         \x20           id: \"cap.software.3tched-router.config.write@v1\".to_string(),\n\
         \x20           description: \"Patch 3tched Router configuration sections (validated).\".to_string(),\n\
         \x20       },\n\
         \x20   );\n",
    );
    for (key, _local) in &all {
        let m = method_name(key);
        let k = kebab(key);
        let _ = writeln!(
            s,
            "    schema.methods.insert(\n\
             \x20       \"{m}\".to_string(),\n\
             \x20       method_decl_from_schemars_with_output::<ConfigMethodInput, {m}Output>(\n\
             \x20           \"{m}\",\n\
             \x20           SideEffect::Read,\n\
             \x20           true,\n\
             \x20           \"cap.software.3tched-router.{k}-config.read@v1\",\n\
             \x20           \"obs.service.3tched-router.{k}-config.get@v1\",\n\
             \x20       ),\n\
             \x20   );\n\
             \x20   schema.capabilities.insert(\n\
             \x20       \"cap.software.3tched-router.{k}-config.read@v1\".to_string(),\n\
             \x20       CapabilityDecl {{\n\
             \x20           id: \"cap.software.3tched-router.{k}-config.read@v1\".to_string(),\n\
             \x20           description: \"Read the 3tched Router `{key}` configuration.\".to_string(),\n\
             \x20       }},\n\
             \x20   );"
        );
    }
    s.push_str("}\n\n");

    s.push_str(include_str!("tched_router_config_surface.dispatch.inc"));

    std::fs::write(&out_path, s).unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
    eprintln!(
        "wrote {} from zeroclaw {version} ({} upstream sections, {} local, {} config methods; \
         {} section schemas in {})",
        out_path.display(),
        upstream.len(),
        LOCAL_SECTIONS.len(),
        all.len() + 2,
        upstream.len() + 1,
        sections_dir.display(),
    );
}
