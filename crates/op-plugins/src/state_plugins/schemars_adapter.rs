//! Derive plugin schemas from `#[derive(schemars::JsonSchema)]` structs instead
//! of hand-building `FieldSchema` maps.
//!
//! The adapter walks the *JSON* of the derived schema (via `serde_json::to_value(schemars::schema_for!(T))`)
//! so it stays decoupled from schemars internals.
//!
//! Primary entry point: `plugin_schema_from_json`.
//!
//! Also provides:
//! - `apply_state_defaults` — patches defaults from a live serialized state snapshot.
//! - `schema_diffs` (#[cfg(test)]) — high-fidelity diff for golden tests.

use op_state_store::{Constraint, FieldSchema, FieldType, PluginSchema};
use serde_json::{Map, Value as JVal};
use simd_json::prelude::*;
use simd_json::OwnedValue;
use std::collections::{HashMap, HashSet};

/// Build a `PluginSchema` from the JSON of a schemars-derived schema.
///
/// The root type's top-level properties become the plugin's `fields`.
/// $ref into $defs/definitions are resolved.
/// x-oscal-subid (root + per-field) and x-immutable-paths are honored.
#[allow(dead_code)]
pub fn plugin_schema_from_json(
    name: &str,
    version: &str,
    description: &str,
    root: &JVal,
) -> PluginSchema {
    let defs = root
        .get("$defs")
        .or_else(|| root.get("definitions"))
        .and_then(JVal::as_object);

    let top = resolve(root, defs);
    let required = required_set(top);

    let mut fields = HashMap::new();
    let mut subids = HashMap::new();

    if let Some(props) = top.get("properties").and_then(JVal::as_object) {
        for (fname, fnode) in props {
            let req = required.contains(fname.as_str());
            fields.insert(fname.clone(), field_schema(fnode, defs, req));

            if let Some(subid) = fnode.get("x-oscal-subid").and_then(JVal::as_str) {
                subids.insert(fname.clone(), subid.to_string());
            }
        }
    }

    // Root-level x-oscal-subid
    if let Some(subid) = root.get("x-oscal-subid").and_then(JVal::as_str) {
        subids.insert("__schema__".to_string(), subid.to_string());
    }

    let mut schema = PluginSchema::builder(name)
        .version(version)
        .description(description)
        .build();

    schema.fields = fields;
    schema.subids = subids;

    if let Some(paths) = root.get("x-immutable-paths").and_then(JVal::as_array) {
        schema.immutable_paths = paths
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
    }

    schema
}

/// Resolve a single $ref recursively.
fn resolve<'a>(node: &'a JVal, defs: Option<&'a Map<String, JVal>>) -> &'a JVal {
    if let (Some(r), Some(d)) = (node.get("$ref").and_then(JVal::as_str), defs) {
        if let Some(key) = r.rsplit('/').next() {
            if let Some(target) = d.get(key) {
                return resolve(target, defs);
            }
        }
    }
    node
}

fn required_set(node: &JVal) -> HashSet<String> {
    node.get("required")
        .and_then(JVal::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn field_type(node: &JVal, defs: Option<&Map<String, JVal>>) -> FieldType {
    let node = resolve(node, defs);

    // oneOf / anyOf (including Option and tagged enums)
    if let Some(alternatives) = node
        .get("anyOf")
        .or_else(|| node.get("oneOf"))
        .and_then(JVal::as_array)
    {
        let non_null: Vec<&JVal> = alternatives
            .iter()
            .filter(|a| type_str(a) != Some("null"))
            .collect();

        if non_null.is_empty() {
            // purely null (unlikely for required fields)
        } else if non_null.len() == 1 {
            return field_type(non_null[0], defs);
        } else {
            // Union / oneOf / anyOf (e.g. Option<T>, or tagged polymorphic like IncusDevice).
            // Current FieldType does not have a dedicated OneOf variant.
            // Compromise: return Any for true unions. Callers that need structure
            // (e.g. incus named devices) should continue using special casing or Object maps.
            // TODO: if richer discriminated-union FieldType ever lands, upgrade here.
            return FieldType::Any;
        }
    }

    if let Some(en) = node.get("enum").and_then(JVal::as_array) {
        return FieldType::Enum(
            en.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        );
    }

    match type_str(node) {
        Some("string") => FieldType::String,
        Some("integer") => FieldType::Integer,
        Some("number") => FieldType::Float,
        Some("boolean") => FieldType::Boolean,
        Some("array") => {
            let item = node
                .get("items")
                .map(|i| field_type(i, defs))
                .unwrap_or(FieldType::Any);
            FieldType::Array(Box::new(item))
        }
        Some("object") => {
            let req = required_set(node);
            let mut m = HashMap::new();
            if let Some(props) = node.get("properties").and_then(JVal::as_object) {
                for (k, v) in props {
                    m.insert(k.clone(), field_schema(v, defs, req.contains(k.as_str())));
                }
            }
            FieldType::Object(m)
        }
        _ => FieldType::Any,
    }
}

fn type_str(node: &JVal) -> Option<&str> {
    match node.get("type") {
        Some(JVal::String(s)) => Some(s.as_str()),
        Some(JVal::Array(a)) => a.iter().filter_map(JVal::as_str).find(|t| *t != "null"),
        _ => None,
    }
}

fn field_schema(node: &JVal, defs: Option<&Map<String, JVal>>, required: bool) -> FieldSchema {
    let meta = node;
    FieldSchema {
        field_type: field_type(node, defs),
        required,
        description: meta
            .get("description")
            .and_then(JVal::as_str)
            .unwrap_or_default()
            .to_string(),
        default: meta.get("default").map(to_simd),
        example: example_of(meta).map(to_simd),
        constraints: constraints(meta),
        read_only: meta
            .get("readOnly")
            .and_then(JVal::as_bool)
            .unwrap_or(false),
        read_only_when: None,
    }
}

fn example_of(node: &JVal) -> Option<&JVal> {
    if let Some(first) = node
        .get("examples")
        .and_then(JVal::as_array)
        .and_then(|a| a.first())
    {
        return Some(first);
    }
    node.get("example")
}

fn constraints(node: &JVal) -> Vec<Constraint> {
    let mut out = Vec::new();
    if let Some(m) = node.get("minimum").and_then(JVal::as_f64) {
        out.push(Constraint::Min { value: m });
    }
    if let Some(m) = node.get("maximum").and_then(JVal::as_f64) {
        out.push(Constraint::Max { value: m });
    }
    if let Some(p) = node.get("pattern").and_then(JVal::as_str) {
        out.push(Constraint::Pattern {
            regex: p.to_string(),
        });
    }
    out
}

/// serde_json::Value → simd_json::OwnedValue
fn to_simd(v: &JVal) -> OwnedValue {
    let mut bytes = serde_json::to_vec(v).unwrap_or_default();
    simd_json::to_owned_value(&mut bytes).unwrap_or_else(|_| simd_json::json!(null))
}

/// Patch a schema's defaults from an actual live state snapshot.
/// Useful for state that has runtime-computed or custom Default behavior
/// that the derive can't see.
pub(crate) fn apply_state_defaults(schema: &mut PluginSchema, state: &OwnedValue) {
    fn set_field_defaults(field: &mut FieldSchema, value: &OwnedValue) {
        field.default = Some(value.clone());

        if let FieldType::Object(ref mut inner) = field.field_type {
            if let Some(obj) = value.as_object() {
                for (k, v) in obj.iter() {
                    if let Some(f) = inner.get_mut(k.as_str()) {
                        set_field_defaults(f, v);
                    }
                }
            }
        }
    }

    if let Some(obj) = state.as_object() {
        for (k, v) in obj.iter() {
            if let Some(f) = schema.fields.get_mut(k.as_str()) {
                set_field_defaults(f, v);
            }
        }
    }
}

/// High-fidelity structural diff between a hand-rolled reference and one
/// derived via the adapter. Empty vec = exact match (within the rules).
#[cfg(test)]
pub(crate) fn schema_diffs(reference: &PluginSchema, derived: &PluginSchema) -> Vec<String> {
    let mut d = Vec::new();

    if reference.name != derived.name {
        d.push(format!("name: {:?} -> {:?}", reference.name, derived.name));
    }
    if reference.version != derived.version {
        d.push(format!(
            "version: {:?} -> {:?}",
            reference.version, derived.version
        ));
    }
    if reference.description != derived.description {
        d.push(format!(
            "description: {:?} -> {:?}",
            reference.description, derived.description
        ));
    }

    let (mut ir, mut id) = (
        reference.immutable_paths.clone(),
        derived.immutable_paths.clone(),
    );
    ir.sort();
    id.sort();
    if ir != id {
        d.push(format!("immutable_paths: {ir:?} -> {id:?}"));
    }
    if reference.subids != derived.subids {
        d.push(format!(
            "subids: {:?} -> {:?}",
            reference.subids, derived.subids
        ));
    }

    let keys: std::collections::BTreeSet<&String> = reference
        .fields
        .keys()
        .chain(derived.fields.keys())
        .collect();

    for k in keys {
        match (reference.fields.get(k), derived.fields.get(k)) {
            (Some(r), Some(v)) => field_diffs(&mut d, k, r, v),
            (Some(_), None) => d.push(format!("{k}: DROPPED (in reference, not derived)")),
            (None, Some(_)) => d.push(format!("{k}: ADDED (in derived, not reference)")),
            (None, None) => {}
        }
    }
    d
}

#[cfg(test)]
fn field_diffs(d: &mut Vec<String>, prefix: &str, r: &FieldSchema, v: &FieldSchema) {
    if type_tag(&r.field_type) != type_tag(&v.field_type) {
        d.push(format!(
            "{prefix}.type: {} -> {}",
            type_tag(&r.field_type),
            type_tag(&v.field_type)
        ));
    }
    if r.required != v.required {
        d.push(format!(
            "{prefix}.required: {} -> {}",
            r.required, v.required
        ));
    }
    if r.description != v.description {
        d.push(format!(
            "{prefix}.description: {:?} -> {:?}",
            r.description, v.description
        ));
    }
    if val_str(&r.default) != val_str(&v.default) {
        d.push(format!(
            "{prefix}.default: {} -> {}",
            val_str(&r.default),
            val_str(&v.default)
        ));
    }
    if val_str(&r.example) != val_str(&v.example) {
        d.push(format!(
            "{prefix}.example: {} -> {}",
            val_str(&r.example),
            val_str(&v.example)
        ));
    }
    if constraint_tags(&r.constraints) != constraint_tags(&v.constraints) {
        d.push(format!(
            "{prefix}.constraints: {:?} -> {:?}",
            constraint_tags(&r.constraints),
            constraint_tags(&v.constraints)
        ));
    }
    if r.read_only != v.read_only {
        d.push(format!(
            "{prefix}.read_only: {} -> {}",
            r.read_only, v.read_only
        ));
    }

    field_type_diffs(d, prefix, &r.field_type, &v.field_type);
}

#[cfg(test)]
fn field_type_diffs(d: &mut Vec<String>, prefix: &str, r: &FieldType, v: &FieldType) {
    match (r, v) {
        (FieldType::Array(r_inner), FieldType::Array(v_inner)) => {
            field_type_diffs(d, &format!("{prefix}[]"), r_inner, v_inner);
        }
        (FieldType::Object(r_map), FieldType::Object(v_map)) => {
            let keys: std::collections::BTreeSet<&String> =
                r_map.keys().chain(v_map.keys()).collect();
            for k in keys {
                match (r_map.get(k), v_map.get(k)) {
                    (Some(rf), Some(vf)) => field_diffs(d, &format!("{prefix}.{k}"), rf, vf),
                    (Some(_), None) => d.push(format!("{prefix}.{k}: DROPPED")),
                    (None, Some(_)) => d.push(format!("{prefix}.{k}: ADDED")),
                    _ => {}
                }
            }
        }
        // oneOf / polymorphic unions are represented as Any in current FieldType.
        // Any deeper diff for unions can be added if/when FieldType gains OneOf.
        _ => {}
    }
}

#[cfg(test)]
fn val_str(v: &Option<OwnedValue>) -> String {
    match v {
        Some(x) => format!("{x:?}"),
        None => "∅".into(),
    }
}

#[cfg(test)]
fn type_tag(t: &FieldType) -> String {
    match t {
        FieldType::String => "string".into(),
        FieldType::Integer => "integer".into(),
        FieldType::Float => "float".into(),
        FieldType::Boolean => "boolean".into(),
        FieldType::Any => "any".into(),
        FieldType::Enum(_) => "enum".into(),
        FieldType::Array(i) => format!("array<{}>", type_tag(i)),
        FieldType::Object(m) => {
            let mut entries: Vec<String> = m.iter()
                .map(|(k, v)| format!("{k}:{}", type_tag(&v.field_type)))
                .collect();
            entries.sort();
            format!("object{{{}}}", entries.join(","))
        }
    }
}

#[cfg(test)]
fn constraint_tags(cs: &[Constraint]) -> Vec<String> {
    let mut v: Vec<String> = cs.iter().map(|c| match c {
        Constraint::Min { value } => format!("min:{value}"),
        Constraint::Max { value } => format!("max:{value}"),
        Constraint::Pattern { regex } => format!("pattern:{regex}"),
        _ => format!("{:?}", c),
    }).collect();
    v.sort();
    v
}
