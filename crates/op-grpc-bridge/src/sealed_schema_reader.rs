//! Exact, read-only inspection of manifest-pinned sealed PluginSchema blobs.
//!
//! This is shared by every bridge surface. MCP, generated gRPC, and D-Bus
//! therefore return the same schema bytes and OSCAL routing-tag index instead
//! of reconstructing a schema from source or consulting an unpinned blob.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde_json::{json, Value};

pub(crate) fn blob_catalog_dir_from_env() -> PathBuf {
    std::env::var("OP_BLOB_CATALOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(op_blob::catalog::DEFAULT_SHM_DIR))
}

pub(crate) fn manifest_plugins(dir: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let bytes = std::fs::read(dir.join(op_blob::catalog::MANIFEST_FILENAME))?;
    let value: Value = serde_json::from_slice(&bytes)?;
    serde_json::from_value(
        value
            .get("plugins")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("blob manifest has no plugins map"))?,
    )
    .map_err(Into::into)
}

pub(crate) fn valid_plugin_id(plugin_id: &str) -> bool {
    !plugin_id.is_empty()
        && plugin_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

struct ManifestPinnedSchemaDocument {
    schema_hash: String,
    schema: op_state_store::PluginSchema,
    schema_json: String,
}

pub(crate) fn read_manifest_pinned_plugin_schema(
    dir: &Path,
    plugin_id: &str,
) -> anyhow::Result<op_state_store::PluginSchema> {
    Ok(read_manifest_pinned_schema_document(dir, plugin_id)?.schema)
}

fn read_manifest_pinned_schema_document(
    dir: &Path,
    plugin_id: &str,
) -> anyhow::Result<ManifestPinnedSchemaDocument> {
    if !valid_plugin_id(plugin_id) || op_plugins::default_registry::is_retired_plugin(plugin_id) {
        anyhow::bail!("sealed plugin not found: {plugin_id}");
    }
    let entries = manifest_plugins(dir)?;
    let schema_hash = entries
        .get(plugin_id)
        .ok_or_else(|| anyhow::anyhow!("sealed plugin not found: {plugin_id}"))?;
    read_schema_document_at_hash(dir, plugin_id, schema_hash)
}

fn read_schema_document_at_hash(
    dir: &Path,
    plugin_id: &str,
    schema_hash: &str,
) -> anyhow::Result<ManifestPinnedSchemaDocument> {
    if schema_hash.len() < 16 || !schema_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        anyhow::bail!("manifest hash for {plugin_id} is malformed");
    }

    let path = dir.join(format!("{plugin_id}.{}.blob", &schema_hash[..16]));
    let bytes = std::fs::read(&path)?;
    let blob = op_blob::BlobRef::new(&bytes)
        .map_err(|error| anyhow::anyhow!("invalid sealed blob {}: {error}", path.display()))?;
    if blob.schema_hash_hex() != schema_hash {
        anyhow::bail!("manifest/blob schema hash mismatch for {plugin_id}");
    }
    let blob_manifest = blob
        .manifest()
        .map_err(|error| anyhow::anyhow!("invalid sealed manifest for {plugin_id}: {error}"))?;
    if blob_manifest.plugin_id != plugin_id || blob_manifest.schema_hash != schema_hash {
        anyhow::bail!("sealed manifest identity mismatch for {plugin_id}");
    }

    // Preserve section 1 exactly as sealed for the public schema result. The
    // typed form is parsed independently for method descriptors and identity
    // checks; it is never serialized back over the raw document.
    let schema_json = blob.schema_json().to_string();
    let schema = blob
        .state_store_schema()
        .map_err(|error| anyhow::anyhow!("invalid sealed schema for {plugin_id}: {error}"))?;
    if schema.name != plugin_id {
        anyhow::bail!("sealed schema name mismatch for {plugin_id}");
    }
    Ok(ManifestPinnedSchemaDocument {
        schema_hash: schema_hash.to_string(),
        schema,
        schema_json,
    })
}

pub(crate) fn read_sealed_schema_result(dir: &Path, plugin_id: &str) -> anyhow::Result<Value> {
    let document = read_manifest_pinned_schema_document(dir, plugin_id)?;
    let schema: Value = serde_json::from_str(&document.schema_json)
        .with_context(|| format!("sealed schema JSON is invalid for {plugin_id}"))?;
    let oscal_subids = oscal_subids_in_schema(&schema);
    Ok(json!({
        "plugin_id": plugin_id,
        "uri": format!("blob://{plugin_id}"),
        "schema_hash": document.schema_hash,
        "oscal_subid_count": oscal_subids.len(),
        "oscal_subids": oscal_subids,
        "schema": schema
    }))
}

pub(crate) fn read_oscal_subids_result(
    dir: &Path,
    plugin_filter: Option<&str>,
    prefix: Option<&str>,
) -> anyhow::Result<Value> {
    let manifest = manifest_plugins(dir)?;
    let plugin_ids = match plugin_filter {
        Some(plugin_id) => {
            if !valid_plugin_id(plugin_id)
                || op_plugins::default_registry::is_retired_plugin(plugin_id)
                || !manifest.contains_key(plugin_id)
            {
                anyhow::bail!("sealed plugin not found: {plugin_id}");
            }
            vec![plugin_id.to_string()]
        }
        None => manifest
            .keys()
            .filter(|plugin_id| !op_plugins::default_registry::is_retired_plugin(plugin_id))
            .cloned()
            .collect(),
    };

    let mut by_subid: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for plugin_id in &plugin_ids {
        let schema_hash = manifest
            .get(plugin_id)
            .ok_or_else(|| anyhow::anyhow!("sealed plugin not found: {plugin_id}"))?;
        let document = read_schema_document_at_hash(dir, plugin_id, schema_hash)?;
        let schema: Value = serde_json::from_str(&document.schema_json)
            .with_context(|| format!("sealed schema JSON is invalid for {plugin_id}"))?;
        for subid in oscal_subids_in_schema(&schema) {
            if prefix.is_some_and(|expected| !subid.starts_with(expected)) {
                continue;
            }
            by_subid
                .entry(subid)
                .or_default()
                .insert(plugin_id.clone(), document.schema_hash.clone());
        }
    }

    let subids = by_subid
        .into_iter()
        .map(|(subid, sources)| {
            let category = subid.split('.').next().unwrap_or_default();
            json!({
                "subid": subid,
                "category": category,
                "sources": sources.into_iter().map(|(plugin_id, schema_hash)| json!({
                    "plugin_id": plugin_id,
                    "schema_hash": schema_hash
                })).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "plugin_count": plugin_ids.len(),
        "subid_count": subids.len(),
        "subids": subids
    }))
}

fn oscal_subids_in_schema(schema: &Value) -> Vec<String> {
    let mut subids = BTreeSet::new();
    collect_oscal_subids(schema, &mut subids);
    subids.into_iter().collect()
}

fn collect_oscal_subids(value: &Value, subids: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if matches!(key.as_str(), "subid" | "x-oscal-subid") {
                    if let Some(subid) = nested.as_str().filter(|value| is_oscal_subid(value)) {
                        subids.insert(subid.to_string());
                    }
                } else if key == "subids" {
                    if let Some(entries) = nested.as_object() {
                        for subid in entries
                            .values()
                            .filter_map(Value::as_str)
                            .filter(|value| is_oscal_subid(value))
                        {
                            subids.insert(subid.to_string());
                        }
                    }
                }
                collect_oscal_subids(nested, subids);
            }
        }
        Value::Array(values) => {
            for nested in values {
                collect_oscal_subids(nested, subids);
            }
        }
        _ => {}
    }
}

fn is_oscal_subid(value: &str) -> bool {
    value.split_once('.').is_some_and(|(category, rest)| {
        matches!(
            category,
            "src" | "prj" | "sch" | "mut" | "obs" | "evt" | "exp"
        ) && rest.contains('.')
    })
}
