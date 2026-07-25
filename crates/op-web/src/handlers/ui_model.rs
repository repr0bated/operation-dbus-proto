//! Gemma / agent-GPU UI-spec gallery + catalog routes.
//!
//! The LOVABLE SPA (`operation-dashboard-ui-07`) renders json-render `Spec`s.
//! The inference agent (local Ollama historically, now the SaladCloud GPU
//! agent — "just another model") is the producer: it reads the sealed blob
//! `PluginSchema` (the base contract) plus the live SIGNALS/event-chain, notes
//! an interesting *slice*, and emits a json-render `Spec` rendering just that
//! part. The agent writes specs to `/dev/shm/gemma-ui-specs.json`; this module
//! is the consumer surface that serves them over HTTP and provides the
//! promote-to-catalog path that seals a winning lens into a durable SHM file.
//!
//! Source-of-truth rules (per CLAUDE.md):
//! - Plugin schemas come from the sealed blob catalog via
//!   `op_blob::catalog::read_plugin_schema_shm`, never a monolith file.
//! - Spec files are tmpfs (`/dev/shm`); promotion rewrites them atomically.

use crate::state::AppState;
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

// Deliberately renamed off the old "gemma"-prefixed paths (were independently
// hardcoded in THREE places with no shared constant: op-gemma/ui_gallery.rs
// the producer, op-plugins/ui_model_brain.rs a third undiscovered reader, and
// here). Not synced to those other two on purpose — whatever silently keeps
// assuming the old path was a hidden violator of "the blob/SHM path is the
// single source of truth," and this will surface it instead of masking it.
const GEMMA_SPECS_PATH: &str = "/dev/shm/ui-specs.json";
const GEMMA_CATALOG_PATH: &str = "/dev/shm/ui-catalog.json";
const BLOB_MANIFEST_PATH: &str = "/dev/shm/opdbus/plugin-blobs/.manifest.json";

// ── Wire types — mirror op-gemma's `GemmaSpecEntry`/`GemmaSpecGallery` ───────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpecEntry {
    id: String,
    spec: Value,
    prompt: String,
    tags: Vec<String>,
    created_at: u64,
    signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpecGallery {
    version: u32,
    specs: Vec<SpecEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogEntry {
    id: String,
    spec: Value,
    prompt: String,
    tags: Vec<String>,
    created_at: u64,
    signature: String,
    promoted_at: u64,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Catalog {
    version: u32,
    entries: Vec<CatalogEntry>,
}

// ── SHM read/write helpers ──────────────────────────────────────────────────

fn read_json<T: for<'de> Deserialize<'de>>(path: &str) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Atomically rewrite a tmpfs JSON file via a temp file + rename.
fn write_json_atomic(path: &str, value: &Value) -> std::io::Result<()> {
    let tmp = format!("{path}.tmp");
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn empty_gallery() -> SpecGallery {
    SpecGallery {
        version: 1,
        specs: vec![],
    }
}

fn empty_catalog() -> Catalog {
    Catalog {
        version: 1,
        entries: vec![],
    }
}

fn ok(value: Value) -> Response {
    axum::Json(value).into_response()
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, axum::Json(json!({ "error": msg }))).into_response()
}

// ── Gallery ─────────────────────────────────────────────────────────────────

/// GET /api/ui-model/gallery
pub async fn ui_model_gallery_handler(Extension(_state): Extension<Arc<AppState>>) -> Response {
    let gallery: SpecGallery = read_json(GEMMA_SPECS_PATH).unwrap_or_else(empty_gallery);
    ok(json!({ "version": gallery.version, "specs": gallery.specs }))
}

/// DELETE /api/ui-model/gallery/:id
pub async fn ui_model_gallery_delete_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let mut gallery: SpecGallery = read_json(GEMMA_SPECS_PATH).unwrap_or_else(empty_gallery);
    gallery.specs.retain(|s| s.id != id);
    match write_json_atomic(
        GEMMA_SPECS_PATH,
        &json!({ "version": gallery.version, "specs": gallery.specs }),
    ) {
        Ok(_) => ok(json!({ "ok": true })),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("write failed: {e}"),
        ),
    }
}

// ── Catalog (promoted) ──────────────────────────────────────────────────────

/// GET /api/ui-model/catalog
pub async fn ui_model_catalog_handler(Extension(_state): Extension<Arc<AppState>>) -> Response {
    let catalog: Catalog = read_json(GEMMA_CATALOG_PATH).unwrap_or_else(empty_catalog);
    ok(json!({ "version": catalog.version, "entries": catalog.entries }))
}

/// POST /api/ui-model/catalog/promote/:id
/// Moves a spec from the gallery into the promoted catalog (seal the lens).
pub async fn ui_model_catalog_promote_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let mut gallery: SpecGallery = match read_json(GEMMA_SPECS_PATH) {
        Some(g) => g,
        None => return err(StatusCode::NOT_FOUND, "gallery not available"),
    };
    let idx = match gallery.specs.iter().position(|s| s.id == id) {
        Some(i) => i,
        None => return err(StatusCode::NOT_FOUND, "spec not in gallery"),
    };
    let entry = gallery.specs.remove(idx);

    let mut catalog: Catalog = read_json(GEMMA_CATALOG_PATH).unwrap_or_else(empty_catalog);
    let promoted = CatalogEntry {
        id: entry.id.clone(),
        spec: entry.spec,
        prompt: entry.prompt,
        tags: entry.tags,
        created_at: entry.created_at,
        signature: entry.signature,
        promoted_at: now_secs(),
        label: None,
    };
    catalog.entries.push(promoted);

    if let Err(e) = write_json_atomic(
        GEMMA_SPECS_PATH,
        &json!({ "version": gallery.version, "specs": gallery.specs }),
    ) {
        return err(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("gallery write failed: {e}"),
        );
    }
    match write_json_atomic(
        GEMMA_CATALOG_PATH,
        &json!({ "version": catalog.version, "entries": catalog.entries }),
    ) {
        Ok(_) => ok(json!({ "ok": true, "id": entry.id })),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("catalog write failed: {e}"),
        ),
    }
}

/// DELETE /api/ui-model/catalog/:id
pub async fn ui_model_catalog_delete_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let mut catalog: Catalog = read_json(GEMMA_CATALOG_PATH).unwrap_or_else(empty_catalog);
    catalog.entries.retain(|e| e.id != id);
    match write_json_atomic(
        GEMMA_CATALOG_PATH,
        &json!({ "version": catalog.version, "entries": catalog.entries }),
    ) {
        Ok(_) => ok(json!({ "ok": true })),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("write failed: {e}"),
        ),
    }
}

// ── Plugin schema (sealed blob catalog) ─────────────────────────────────────

/// GET /api/ui-model/plugins — every plugin actually in the sealed blob catalog.
/// A plugin exists iff its blob is sealed here; this is NOT the same list as
/// "plugins with generated RPC methods" (some plugins, e.g. antigravity/
/// antigravity_chat, are state-only with zero methods, so they're absent
/// from the frontend's method-index but very much present as real blobs).
pub async fn ui_model_list_plugins_handler(Extension(_state): Extension<Arc<AppState>>) -> Response {
    match op_blob::catalog::read_manifest_plugin_ids_shm() {
        Some(ids) => ok(json!({ "plugins": ids })),
        None => err(
            StatusCode::SERVICE_UNAVAILABLE,
            "blob catalog manifest unavailable",
        ),
    }
}

/// GET /api/ui-model/plugin-schema/:plugin
/// The base schema is the render source: read straight from the sealed blob
/// catalog. Resolve by exact id, then by manifest prefix/contains match.
pub async fn ui_model_plugin_schema_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Path(plugin): Path<String>,
) -> Response {
    let (resolved_id, schema) = match op_blob::catalog::read_plugin_schema_shm(&plugin) {
        Some(s) => (plugin.clone(), s),
        None => match resolve_plugin_by_prefix(&plugin) {
            Some((id, s)) => (id, s),
            None => {
                return err(
                    StatusCode::NOT_FOUND,
                    &format!("plugin schema not found: {plugin}"),
                )
            }
        },
    };

    ok(json!({
        "plugin": resolved_id,
        "schema_hash": schema_hash_for(&resolved_id),
        "schema": schema,
    }))
}

/// List manifest plugin ids and try a prefix/contains match for the request.
fn resolve_plugin_by_prefix(request: &str) -> Option<(String, op_state_store::PluginSchema)> {
    let ids = op_blob::catalog::read_manifest_plugin_ids_shm()?;
    let hit = ids
        .iter()
        .find(|id| id == &request || id.contains(request) || request.contains(id.as_str()))?;
    op_blob::catalog::read_plugin_schema_shm(hit).map(|s| (hit.clone(), s))
}

/// Per-plugin schema_hash from the catalog manifest (single source of truth).
fn schema_hash_for(plugin: &str) -> Option<String> {
    let bytes = std::fs::read(BLOB_MANIFEST_PATH).ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    let plugins = v.get("plugins")?.as_object()?;
    plugins
        .get(plugin)
        .and_then(|h| h.as_str())
        .map(|s| s.to_string())
}
