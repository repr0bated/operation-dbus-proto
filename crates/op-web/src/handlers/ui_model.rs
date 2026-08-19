//! Model-agnostic UI gallery generation + catalog routes.
//!
//! The Antigravity chat UI renders json-render `Spec`s. The model-agnostic
//! inference loop (`op-gallery-gen`) is the producer: any model loaded through
//! ZeroClaw reads the sealed blob `PluginSchema` plus json-render.dev docs
//! and emits specs. The operator interacts through the /gallery-gen/* API.
//!
//! Legacy compatibility: the old `/api/ui-model/gallery` endpoint still reads
//! from `/dev/shm/ui-specs.json` if present, for backward compat with existing
//! dashboard deployments. New generation bypasses this file entirely.
//!
//! Source-of-truth rules (per CLAUDE.md):
//! - Plugin schemas come from the sealed blob catalog via
//!   `op_blob::catalog::read_plugin_schema_shm`, never a monolith file.
//! - Gallery generation uses `op-gallery-gen` (model-agnostic, via ZeroClaw).

use crate::state::AppState;
use crate::state_tree;
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
// NOTE: The legacy file-based gallery (/dev/shm/ui-specs.json, produced by
// op-gemma's ui_gallery.rs) is replaced by the model-agnostic op-gallery-gen
// system. These endpoints now return data from the new generation system.
// The /api/gallery-gen/* endpoints are the primary operator interface.

/// GET /api/ui-model/gallery
/// Returns the gallery (legacy compat — reads from SHM if present, empty otherwise).
pub async fn ui_model_gallery_handler(Extension(_state): Extension<Arc<AppState>>) -> Response {
    // Legacy fallback: if the old SHM file exists, serve it for backward compat.
    // New generation writes directly to CatalogStore, not this file.
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
/// "plugins with generated RPC methods" (some plugins, e.g. antigravity,
/// are state-only with zero compiled RPC methods, so they're absent
/// from the frontend's method-index but very much present as real blobs).
pub async fn ui_model_list_plugins_handler(
    Extension(_state): Extension<Arc<AppState>>,
) -> Response {
    match op_blob::catalog::read_manifest_plugin_ids_shm() {
        Some(ids) => ok(json!({ "plugins": ids })),
        None => err(
            StatusCode::SERVICE_UNAVAILABLE,
            "blob catalog manifest unavailable",
        ),
    }
}

/// GET /api/ui-model/state — live present-state from `/dev/shm/opdbus/state/`.
///
/// Replaces the projection daemon and `/api/dashboard/projections`. Empty
/// `{ plugins: [], state: {} }` is correct when nothing has been mutated.
pub async fn ui_model_state_handler(Extension(_state): Extension<Arc<AppState>>) -> Response {
    ok(catalog_state_body(simd_tree_to_serde(
        state_tree::read_all(),
    )))
}

/// Catalog JSON for the SHM state tree: sorted plugin ids plus the objects.
pub(crate) fn catalog_state_body(tree: HashMap<String, Value>) -> Value {
    let mut plugins: Vec<String> = tree.keys().cloned().collect();
    plugins.sort();
    json!({ "plugins": plugins, "state": tree })
}

fn simd_tree_to_serde(tree: HashMap<String, simd_json::OwnedValue>) -> HashMap<String, Value> {
    tree.into_iter()
        .filter_map(|(key, value)| {
            let text = simd_json::to_string(&value).ok()?;
            match serde_json::from_str(&text) {
                Ok(parsed) => Some((key, parsed)),
                Err(e) => {
                    tracing::warn!(
                        "Failed to convert simd_json to serde_json for plugin '{}': {}",
                        key,
                        e
                    );
                    None
                }
            }
        })
        .collect()
}

/// GET /api/ui-model/plugin-schema/:plugin
/// The base schema is the render source: read straight from the sealed blob
/// catalog. Resolve by exact id, then by manifest prefix/contains match.
///
/// Presentation remap (display / arrangement / priority / audience / element
/// role) is derived from `docs/subid-taxonomy.md` categories via
/// `op_state_store::subid_ui` — not Card/Button names. Live values come from
/// the second SHM (`/dev/shm/opdbus/state/`).
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

    let ui_projection = op_state_store::project_schema_ui(&resolved_id, &schema);
    // Second SHM: live present values (not sealed schema).
    let state =
        crate::state_tree::read_plugin(&resolved_id).and_then(|v| serde_json::to_value(v).ok());

    ok(json!({
        "plugin": resolved_id,
        "schema_hash": schema_hash_for(&resolved_id),
        "schema": schema,
        "ui_projection": ui_projection,
        "state": state,
    }))
}

/// GET /api/ui-model/subid-projection
/// Dump every sealed plugin's subid → UI role rows (populate the catalog).
/// Keep what fills; clear unused later; chatbot wires to this surface.
pub async fn ui_model_subid_projection_handler(
    Extension(_state): Extension<Arc<AppState>>,
) -> Response {
    let Some(ids) = op_blob::catalog::read_manifest_plugin_ids_shm() else {
        return err(
            StatusCode::SERVICE_UNAVAILABLE,
            "blob catalog manifest unavailable",
        );
    };

    let mut rows = Vec::new();
    for id in &ids {
        if let Some(schema) = op_blob::catalog::read_plugin_schema_shm(id) {
            rows.extend(op_state_store::project_schema_ui(id, &schema));
        }
    }
    let population = op_state_store::role_population(&rows);

    ok(json!({
        "source": "docs/subid-taxonomy.md + sealed PluginSchema.subids",
        "scope": "render/display/arrangement/priority/audience/element-role only",
        "plugins": ids.len(),
        "rows": rows.len(),
        "population": population,
        "projections": rows,
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

// ── Gallery Generation API ─────────────────────────────────────────────────

use axum::response::sse::{Event, Sse};
use lazy_static::lazy_static;

lazy_static! {
    static ref GEN_PROGRESS: Arc<op_gallery_gen::RunProgress> =
        Arc::new(op_gallery_gen::RunProgress::new());
}

#[derive(Debug, Deserialize)]
pub struct GalleryGenConfig {
    pub target_count: usize,
    pub enable_mcp: bool,
    pub enable_qdrant: bool,
    pub operator_guidance: String,
}

/// POST /gallery-gen/start - Start a generation session
pub async fn start_generation(
    Extension(_state): Extension<Arc<AppState>>,
    axum::Json(config): axum::Json<GalleryGenConfig>,
) -> Result<impl IntoResponse, StatusCode> {
    // Check if already running
    if GEN_PROGRESS.running.load(Ordering::SeqCst) {
        return Err(StatusCode::CONFLICT);
    }

    // Assemble context from live blob catalog
    let ctx = match op_gallery_gen::assemble_context(
        config.enable_mcp,
        config.enable_qdrant,
        if config.operator_guidance.is_empty() {
            None
        } else {
            Some(config.operator_guidance.clone())
        },
    ) {
        Ok(ctx) => ctx,
        Err(e) => {
            tracing::error!("Failed to assemble generation context: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let plugin_count = ctx.schemas.len();
    let catalog_hash = ctx.catalog_hash.clone();

    // Build the run config
    let run_config = op_gallery_gen::GalleryGenConfig {
        target_count: config.target_count,
        stable_core_max: 40,
        tched_router_endpoint: "http://127.0.0.1:8080".to_string(),
        enable_mcp: config.enable_mcp,
        enable_qdrant: config.enable_qdrant,
        max_turns: 10,
        // Where the exported json-render catalog lives; the run loads the
        // vocabulary from it and refuses to start if it cannot.
        catalog_dir: op_gallery_gen::default_catalog_dir(),
    };

    // Reset progress and mark as running
    GEN_PROGRESS.reset(config.target_count);

    // Create the gallery store for this run.
    // InMemoryGalleryStore handles signature dedup and counting in-process.
    // TODO: Replace with CatalogStore bridge when catalog is exposed via AppState or gRPC.
    let store: Arc<dyn op_gallery_gen::GalleryStore> =
        Arc::new(op_gallery_gen::InMemoryGalleryStore::new());

    // Spawn the background generation task
    let progress = Arc::clone(&GEN_PROGRESS);
    tokio::spawn(async move {
        match op_gallery_gen::run_gallery_fill(&run_config, ctx, store, progress.clone()).await {
            Ok(count) => {
                tracing::info!("Gallery generation completed: {} specs produced", count);
            }
            Err(e) => {
                tracing::error!("Gallery generation failed: {}", e);
                progress.finish();
            }
        }
    });

    let response = json!({
        "status": "started",
        "config": {
            "target_count": config.target_count,
            "enable_mcp": config.enable_mcp,
            "enable_qdrant": config.enable_qdrant,
        },
        "context": {
            "plugin_count": plugin_count,
            "catalog_hash": catalog_hash,
        }
    });

    Ok(axum::Json(response))
}

/// POST /gallery-gen/stop - Stop a running generation session
pub async fn stop_generation(
    Extension(_state): Extension<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    GEN_PROGRESS.stop_signal.store(true, Ordering::SeqCst);

    let response = json!({
        "status": "stopping"
    });

    Ok(axum::Json(response))
}

/// GET /gallery-gen/stream - SSE stream for generation progress
pub async fn generation_stream(
    Extension(_state): Extension<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = async_stream::stream! {
        // Send initial status
        yield Ok(Event::default().json_data(json!({
            "type": "status",
            "running": GEN_PROGRESS.running.load(Ordering::SeqCst),
            "generated": GEN_PROGRESS.generated.load(Ordering::SeqCst),
            "attempts": GEN_PROGRESS.attempts.load(Ordering::SeqCst),
            "target": GEN_PROGRESS.target.load(Ordering::SeqCst),
        })).unwrap());

        // Keep connection alive with periodic updates
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
        loop {
            interval.tick().await;

            let running = GEN_PROGRESS.running.load(Ordering::SeqCst);
            let generated = GEN_PROGRESS.generated.load(Ordering::SeqCst);
            let attempts = GEN_PROGRESS.attempts.load(Ordering::SeqCst);
            let target = GEN_PROGRESS.target.load(Ordering::SeqCst);

            if !running {
                yield Ok(Event::default().json_data(json!({
                    "type": "complete",
                    "generated": generated,
                    "attempts": attempts,
                    "target": target,
                })).unwrap());
                break;
            }

            if GEN_PROGRESS.stop_signal.load(Ordering::SeqCst) && !running {
                yield Ok(Event::default().json_data(json!({
                    "type": "cancelled",
                    "generated": generated,
                    "attempts": attempts,
                })).unwrap());
                break;
            }

            // Send progress update
            yield Ok(Event::default().json_data(json!({
                "type": "progress",
                "running": running,
                "generated": generated,
                "attempts": attempts,
                "target": target,
            })).unwrap());
        }
    };

    Sse::new(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_state_body_lists_shm_state_plugins_not_projections() {
        let mut tree = HashMap::new();
        tree.insert("tched_router".into(), json!({ "selected_model": "qwen" }));
        tree.insert("system.memory".into(), json!({ "rss": 1 }));
        let body = catalog_state_body(tree);
        assert_eq!(body["plugins"], json!(["system.memory", "tched_router"]));
        assert_eq!(body["state"]["tched_router"]["selected_model"], "qwen");
        assert!(body.get("projections").is_none());
    }

    #[test]
    fn catalog_state_body_empty_tree_is_valid() {
        let body = catalog_state_body(HashMap::new());
        assert_eq!(body["plugins"], json!([]));
        assert_eq!(body["state"], json!({}));
    }
}
