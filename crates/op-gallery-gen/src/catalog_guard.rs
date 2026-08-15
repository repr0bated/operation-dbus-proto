//! The component vocabulary a spec may use, loaded from the catalog's own export.
//!
//! # Why this is a wrapper
//!
//! json-render's catalog is authored in TypeScript with Zod, and it is the only
//! place the vocabulary exists: the renderer refuses anything the catalog does
//! not declare. Rust cannot evaluate Zod, so the admission gate here has two
//! options — restate the vocabulary in Rust, or validate against something the
//! catalog itself produced. Restating it means the gate can admit a spec the
//! renderer then refuses, which is the failure the gate exists to prevent, so
//! this reads an artifact exported by `catalog.jsonSchema()`/`z.toJSONSchema`:
//!
//! ```text
//! src/json-render/catalog/catalog.ts        (the one declaration)
//!         │  scripts/export-catalog-schema.mts
//!         ▼
//! schemas/json-render/catalog.schema.json   (derived: per-component prop schemas)
//! schemas/json-render/catalog.manifest.json (its sha256)
//!         │  CatalogGuard::load
//!         ▼
//! admission gate
//! ```
//!
//! There is no vocabulary in this file. Every component name, prop, slot and
//! action comes from the artifact, and a stale artifact is detectable because
//! its digest is carried alongside it and surfaced as [`CatalogGuard::hash`].
//!
//! # Directive-valued props
//!
//! A prop may hold a value or a directive — `{"$state": "/ptr"}`,
//! `{"$cond": …, "$then": …}`, `{"$template": "…"}`. json-render dispatches on
//! `$`-prefixed keys (`findDirective` checks for them), and a directive's
//! resolved type is a property of runtime state, not of the spec, so a
//! directive-valued prop is checked for *shape* and deliberately not for type.
//! Anything else would reject valid specs: 67 of the 74 elements in the live
//! shell spec bind at least one prop.

use anyhow::{anyhow, Context, Result};
use jsonschema::Validator;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::validator::ValidationError;

/// Exported catalog: per-component prop schemas, slots, actions, visibility.
pub const CATALOG_SCHEMA_FILE: &str = "catalog.schema.json";
/// The catalog's own system prompt, from `Catalog.prompt()`.
pub const CATALOG_PROMPT_FILE: &str = "catalog.prompt.md";
/// Digests and names for the two files above.
pub const CATALOG_MANIFEST_FILE: &str = "catalog.manifest.json";
/// Default artifact directory, relative to the repo root like `schemas/plugin`.
pub const JSON_RENDER_SCHEMA_DIR: &str = "schemas/json-render";
/// Environment override, shared with the exporter so both ends agree.
pub const JSON_RENDER_DIR_ENV: &str = "OPDBUS_JSON_RENDER_DIR";

/// Resolve the artifact directory: the environment override, else the default.
pub fn default_catalog_dir() -> std::path::PathBuf {
    std::env::var_os(JSON_RENDER_DIR_ENV)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(JSON_RENDER_SCHEMA_DIR))
}

/// One component's admission rules, as the catalog declared them.
struct ComponentRule {
    /// Compiled schema per declared prop.
    ///
    /// Per-prop rather than one schema for the whole props object, because a
    /// directive-valued prop has to be skipped and a whole-object validation
    /// cannot skip one member without rewriting the instance.
    props: BTreeMap<String, Validator>,
    /// Props the catalog requires. Nullable props are required-and-nullable in
    /// this catalog's convention, so absence is a rejection, not a default.
    required: BTreeSet<String>,
    /// A component with no slots is a leaf; children on it cannot render.
    accepts_children: bool,
}

/// The catalog, compiled and ready to admit or reject elements.
pub struct CatalogGuard {
    hash: String,
    json_render_version: String,
    components: BTreeMap<String, ComponentRule>,
    action_names: BTreeSet<String>,
    visibility: Validator,
    /// The catalog's system prompt, verified against the manifest. Empty only
    /// for guards built from schema bytes alone (validation without generation).
    prompt: String,
}

impl std::fmt::Debug for CatalogGuard {
    /// Compiled validators are not printable, and would be noise if they were;
    /// what identifies a guard is which artifact it came from.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CatalogGuard")
            .field("hash", &self.hash)
            .field("json_render_version", &self.json_render_version)
            .field("components", &self.components.len())
            .field("actions", &self.action_names.len())
            .finish()
    }
}

impl CatalogGuard {
    /// Load and verify the artifact set from a directory.
    ///
    /// The manifest's digests are checked against both files' bytes. A mismatch
    /// means one of them was edited or replaced independently, and the gate
    /// refuses rather than admitting against an unknown vocabulary — or, for the
    /// prompt, teaching a model a vocabulary the gate will not accept.
    pub fn load(dir: &Path) -> Result<Self> {
        let manifest_path = dir.join(CATALOG_MANIFEST_FILE);
        let manifest_bytes = std::fs::read(&manifest_path)
            .with_context(|| format!("reading catalog manifest {}", manifest_path.display()))?;
        let manifest: Value = serde_json::from_slice(&manifest_bytes)
            .with_context(|| format!("parsing {}", manifest_path.display()))?;

        let schema_bytes = read_verified(
            dir,
            CATALOG_SCHEMA_FILE,
            &manifest,
            "schemaSha256",
            &manifest_path,
        )?;
        let prompt_bytes = read_verified(
            dir,
            CATALOG_PROMPT_FILE,
            &manifest,
            "promptSha256",
            &manifest_path,
        )?;

        let mut guard = Self::from_schema_bytes(&schema_bytes)?;
        guard.prompt = String::from_utf8(prompt_bytes)
            .with_context(|| format!("{CATALOG_PROMPT_FILE} is not UTF-8"))?;
        Ok(guard)
    }

    /// Compile a guard from the artifact bytes, without a manifest to check
    /// them against and without the catalog's prompt. Callers that hold the
    /// bytes from a verified source (a sealed blob, a test fixture) and only
    /// need to validate use this; [`Self::load`] is the disk path.
    pub fn from_schema_bytes(bytes: &[u8]) -> Result<Self> {
        let hash = sha256_hex(bytes);
        let artifact: Value =
            serde_json::from_slice(bytes).context("parsing exported catalog schema")?;

        let json_render_version = artifact
            .get("jsonRenderVersion")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();

        let components_json = artifact
            .get("components")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow!("exported catalog has no 'components' object"))?;

        let mut components = BTreeMap::new();
        for (name, def) in components_json {
            components.insert(name.clone(), compile_component(name, def)?);
        }

        let action_names = artifact
            .get("actions")
            .and_then(Value::as_object)
            .map(|actions| actions.keys().cloned().collect())
            .unwrap_or_default();

        let visibility_schema = artifact
            .get("visibility")
            .ok_or_else(|| anyhow!("exported catalog has no 'visibility' schema"))?;
        let visibility = jsonschema::validator_for(visibility_schema)
            .map_err(|e| anyhow!("visibility schema does not compile: {e}"))?;

        Ok(Self {
            hash,
            json_render_version,
            components,
            action_names,
            visibility,
            prompt: String::new(),
        })
    }

    /// sha256 of the artifact this guard was compiled from.
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// The catalog's system prompt: output contract, directives, state model and
    /// every available component, as the catalog describes itself.
    ///
    /// Empty for guards built by [`Self::from_schema_bytes`].
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// json-render version the artifact was exported against.
    pub fn json_render_version(&self) -> &str {
        &self.json_render_version
    }

    /// Number of components the catalog declares.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Number of actions the catalog declares.
    pub fn action_count(&self) -> usize {
        self.action_names.len()
    }

    /// Declared component names, sorted.
    pub fn component_names(&self) -> impl Iterator<Item = &str> {
        self.components.keys().map(String::as_str)
    }

    /// Whether the catalog declares an action by this name.
    pub fn declares_action(&self, name: &str) -> bool {
        self.action_names.contains(name)
    }

    /// Check one element against the catalog, appending every problem found.
    ///
    /// Every check keeps going rather than returning on the first failure: a
    /// generator that gets one report per attempt converges faster than one
    /// that gets the first error repeatedly.
    pub fn check_element(&self, id: &str, element: &Value, errors: &mut Vec<ValidationError>) {
        let Some(type_name) = element.get("type").and_then(Value::as_str) else {
            // Absent or non-string `type` is a structural problem, reported by
            // the structural pass; nothing here can be said about it.
            return;
        };

        let Some(rule) = self.components.get(type_name) else {
            errors.push(ValidationError {
                code: "E_UNKNOWN_COMPONENT".to_string(),
                message: format!(
                    "element '{id}' uses component '{type_name}', which the catalog does not \
                     declare ({} declared)",
                    self.components.len()
                ),
            });
            return;
        };

        let child_count = element
            .get("children")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        if child_count > 0 && !rule.accepts_children {
            errors.push(ValidationError {
                code: "E_CHILDREN_NOT_ALLOWED".to_string(),
                message: format!(
                    "element '{id}' ({type_name}) has {child_count} children, but the catalog \
                     declares no slots for it"
                ),
            });
        }

        match element.get("props") {
            Some(Value::Object(props)) => self.check_props(id, type_name, rule, props, errors),
            Some(other) => errors.push(ValidationError {
                code: "E_PROPS_NOT_OBJECT".to_string(),
                message: format!(
                    "element '{id}' ({type_name}) has props of type {}, expected an object",
                    value_kind(other)
                ),
            }),
            None => {
                for prop in &rule.required {
                    errors.push(ValidationError {
                        code: "E_PROP_REQUIRED".to_string(),
                        message: format!(
                            "element '{id}' ({type_name}) is missing required prop '{prop}'"
                        ),
                    });
                }
            }
        }

        if let Some(visible) = element.get("visible") {
            if let Err(error) = self.visibility.validate(visible) {
                errors.push(ValidationError {
                    code: "E_VISIBLE_SCHEMA".to_string(),
                    message: format!("element '{id}' has an invalid `visible` condition: {error}"),
                });
            }
        }
    }

    fn check_props(
        &self,
        id: &str,
        type_name: &str,
        rule: &ComponentRule,
        props: &serde_json::Map<String, Value>,
        errors: &mut Vec<ValidationError>,
    ) {
        for prop in &rule.required {
            if !props.contains_key(prop) {
                errors.push(ValidationError {
                    code: "E_PROP_REQUIRED".to_string(),
                    message: format!(
                        "element '{id}' ({type_name}) is missing required prop '{prop}'"
                    ),
                });
            }
        }

        for (name, value) in props {
            let Some(validator) = rule.props.get(name) else {
                errors.push(ValidationError {
                    code: "E_UNKNOWN_PROP".to_string(),
                    message: format!(
                        "element '{id}' ({type_name}) sets prop '{name}', which the catalog does \
                         not declare"
                    ),
                });
                continue;
            };

            if is_directive(value) {
                check_pointers(id, name, value, errors);
                continue;
            }

            if let Err(error) = validator.validate(value) {
                errors.push(ValidationError {
                    code: "E_PROP_SCHEMA".to_string(),
                    message: format!("element '{id}' ({type_name}) prop '{name}': {error}"),
                });
            }
        }
    }
}

/// Compile one component definition from the artifact.
fn compile_component(name: &str, def: &Value) -> Result<ComponentRule> {
    let props_schema = def
        .get("props")
        .ok_or_else(|| anyhow!("component '{name}' has no props schema"))?;

    let required: BTreeSet<String> = props_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    // A prop schema may reference definitions hoisted to the root of the
    // component's schema. Sub-schemas are compiled on their own, so those
    // definitions have to travel with them or the `$ref` dangles.
    let defs = props_schema.get("$defs").cloned();

    let mut props = BTreeMap::new();
    if let Some(properties) = props_schema.get("properties").and_then(Value::as_object) {
        for (prop_name, prop_schema) in properties {
            let mut schema = prop_schema.clone();
            if let (Some(defs), Some(object)) = (defs.clone(), schema.as_object_mut()) {
                object.entry("$defs").or_insert(defs);
            }
            let validator = jsonschema::validator_for(&schema).map_err(|e| {
                anyhow!("component '{name}' prop '{prop_name}' schema does not compile: {e}")
            })?;
            props.insert(prop_name.clone(), validator);
        }
    }

    let accepts_children = def
        .get("slots")
        .and_then(Value::as_array)
        .is_some_and(|slots| !slots.is_empty());

    Ok(ComponentRule {
        props,
        required,
        accepts_children,
    })
}

/// Whether a value is a directive rather than a literal.
///
/// Mirrors json-render's own dispatch: core's `findDirective` treats a value as
/// dynamic when it carries a `$`-prefixed key. Not every key needs the prefix —
/// `{"$computed": "fn", "args": {…}}` is one directive with a plain member.
fn is_directive(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.keys().any(|key| key.starts_with('$')))
}

/// Check that state pointers inside a directive are JSON Pointers.
///
/// `$state` and `$bindState` address the state model by RFC 6901 pointer, which
/// is either empty or begins with `/`. A path written `plugins/x/status`
/// resolves to nothing at runtime and renders as a silently missing value, so
/// it is worth catching at admission.
fn check_pointers(id: &str, prop: &str, value: &Value, errors: &mut Vec<ValidationError>) {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if matches!(key.as_str(), "$state" | "$bindState") {
                    if let Some(pointer) = nested.as_str() {
                        if !pointer.is_empty() && !pointer.starts_with('/') {
                            errors.push(ValidationError {
                                code: "E_BIND_PATH".to_string(),
                                message: format!(
                                    "element '{id}' prop '{prop}' binds '{pointer}', which is not \
                                     a JSON Pointer (must be empty or start with '/')"
                                ),
                            });
                        }
                    }
                }
                check_pointers(id, prop, nested, errors);
            }
        }
        Value::Array(items) => {
            for item in items {
                check_pointers(id, prop, item, errors);
            }
        }
        _ => {}
    }
}

/// Read one artifact file and check it against the digest the manifest declares.
fn read_verified(
    dir: &Path,
    file: &str,
    manifest: &Value,
    digest_field: &str,
    manifest_path: &Path,
) -> Result<Vec<u8>> {
    let path = dir.join(file);
    let bytes = std::fs::read(&path)
        .with_context(|| format!("reading catalog artifact {}", path.display()))?;

    let declared = manifest
        .get(digest_field)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{} has no {digest_field}", manifest_path.display()))?;

    let actual = sha256_hex(&bytes);
    if actual != declared {
        return Err(anyhow!(
            "catalog artifact digest mismatch: {} declares {digest_field} {}, {} hashes to {}",
            manifest_path.display(),
            declared,
            path.display(),
            actual
        ));
    }

    Ok(bytes)
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The artifact this crate is built alongside. Loading the real one keeps
    /// these tests honest: a catalog change that breaks the export breaks here.
    fn artifact_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/json-render")
            .canonicalize()
            .expect("schemas/json-render must exist")
    }

    fn guard() -> CatalogGuard {
        CatalogGuard::load(&artifact_dir()).expect("real catalog artifact must load")
    }

    #[test]
    fn loads_the_real_catalog_and_verifies_its_digest() {
        let guard = guard();
        assert!(
            guard.component_count() > 10,
            "expected a populated catalog, got {}",
            guard.component_count()
        );
        assert_eq!(guard.hash().len(), 64, "hash should be hex sha256");
        assert!(guard.component_names().any(|name| name == "card"));
    }

    #[test]
    fn every_component_the_gate_enforces_appears_in_the_prompt() {
        // The prompt and the prop schemas come from one catalog, so a name the
        // gate knows and the prompt omits (or the reverse) means the export
        // drifted. This is the check that keeps generation and admission from
        // disagreeing about what exists.
        let guard = guard();
        let prompt = guard.prompt();
        assert!(
            prompt.contains("AVAILABLE COMPONENTS"),
            "prompt should carry the component list"
        );
        for name in guard.component_names() {
            assert!(
                prompt.contains(name),
                "component '{name}' is enforced but never taught"
            );
        }
    }

    #[test]
    fn digest_mismatch_refuses_to_load() {
        let dir = tempdir();
        let schema = std::fs::read(artifact_dir().join(CATALOG_SCHEMA_FILE)).unwrap();
        std::fs::write(dir.join(CATALOG_SCHEMA_FILE), &schema).unwrap();
        std::fs::write(
            dir.join(CATALOG_MANIFEST_FILE),
            br#"{"schemaSha256":"0000000000000000000000000000000000000000000000000000000000000000"}"#,
        )
        .unwrap();

        let error = CatalogGuard::load(&dir).expect_err("mismatched digest must fail");
        assert!(
            error.to_string().contains("digest mismatch"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn unknown_component_is_rejected() {
        let mut errors = Vec::new();
        guard().check_element(
            "e1",
            &serde_json::json!({"type": "status_pill", "props": {}, "children": []}),
            &mut errors,
        );
        assert_eq!(
            errors
                .iter()
                .filter(|e| e.code == "E_UNKNOWN_COMPONENT")
                .count(),
            1,
            "expected the retired dialect name to be refused: {errors:?}"
        );
    }

    #[test]
    fn declared_component_with_valid_props_passes() {
        let mut errors = Vec::new();
        guard().check_element(
            "e1",
            &serde_json::json!({
                "type": "statCard",
                "props": {"label": "Uptime", "value": "3d", "sub": null, "variant": "ok"},
                "children": []
            }),
            &mut errors,
        );
        assert!(errors.is_empty(), "expected no errors, got {errors:?}");
    }

    #[test]
    fn wrong_prop_type_is_rejected_but_a_bound_prop_is_not() {
        let g = guard();

        let mut typed = Vec::new();
        g.check_element(
            "e1",
            &serde_json::json!({
                "type": "statCard",
                "props": {"label": 7, "value": "3d", "sub": null, "variant": "ok"},
                "children": []
            }),
            &mut typed,
        );
        assert!(
            typed.iter().any(|e| e.code == "E_PROP_SCHEMA"),
            "a number where the catalog declares a string must fail: {typed:?}"
        );

        let mut bound = Vec::new();
        g.check_element(
            "e1",
            &serde_json::json!({
                "type": "statCard",
                "props": {
                    "label": {"$state": "/plugins/netmaker/status"},
                    "value": {"$template": "${/plugins/netmaker/peers} peers"},
                    "sub": null,
                    "variant": "ok"
                },
                "children": []
            }),
            &mut bound,
        );
        assert!(
            bound.is_empty(),
            "directive-valued props must not be type-checked: {bound:?}"
        );
    }

    #[test]
    fn undeclared_prop_and_missing_required_prop_are_both_reported() {
        let mut errors = Vec::new();
        guard().check_element(
            "e1",
            &serde_json::json!({
                "type": "statCard",
                "props": {"label": "Uptime", "value": "3d", "sub": null, "colour": "red"},
                "children": []
            }),
            &mut errors,
        );
        assert!(
            errors.iter().any(|e| e.code == "E_UNKNOWN_PROP"),
            "expected the undeclared prop to be caught: {errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.code == "E_PROP_REQUIRED"),
            "expected the missing required prop to be caught: {errors:?}"
        );
    }

    #[test]
    fn a_bind_that_is_not_a_pointer_is_rejected() {
        let mut errors = Vec::new();
        guard().check_element(
            "e1",
            &serde_json::json!({
                "type": "statCard",
                "props": {
                    "label": {"$state": "plugins/netmaker/status"},
                    "value": "3d",
                    "sub": null,
                    "variant": null
                },
                "children": []
            }),
            &mut errors,
        );
        assert!(
            errors.iter().any(|e| e.code == "E_BIND_PATH"),
            "expected a non-pointer bind to be caught: {errors:?}"
        );
    }

    #[test]
    fn children_on_a_leaf_component_are_rejected() {
        let g = guard();
        let leaf = g
            .components
            .iter()
            .find(|(_, rule)| !rule.accepts_children)
            .map(|(name, _)| name.clone())
            .expect("catalog should declare at least one leaf component");

        let mut errors = Vec::new();
        g.check_element(
            "e1",
            &serde_json::json!({"type": leaf, "props": {}, "children": ["e2"]}),
            &mut errors,
        );
        assert!(
            errors.iter().any(|e| e.code == "E_CHILDREN_NOT_ALLOWED"),
            "expected children on '{leaf}' to be refused: {errors:?}"
        );
    }

    fn tempdir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "op-gallery-gen-catalog-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }
}
