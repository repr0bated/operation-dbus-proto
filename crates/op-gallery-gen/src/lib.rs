//! Model-agnostic generative UI gallery system.
//!
//! This crate implements a model-agnostic inference loop that reads sealed plugin blobs,
//! accepts operator guidance via Antigravity chat UI, and generates json-render.dev specs
//! rendered by the existing DSL interpreter.
//!
//! # Architecture
//!
//! 1. **Context Assembler** — Gathers baseline context (blobs, docs, catalog)
//! 2. **Inference Loop** — Calls ZeroClaw `/v1/chat/completions` with tool support
//! 3. **Spec Validator** — Validates generated specs against grammar
//! 4. **Gallery Admission** — Admits validated specs to catalog store
//!
//! # Model Agnosticism
//!
//! No model-specific code. Any model exposed through ZeroClaw can be used. Model selection
//! is ZeroClaw's responsibility via the tched_router plugin's `selected_provider` and
//! `selected_model` fields.

pub mod admission;
pub mod catalog_guard;
pub mod context;
pub mod inference;
pub mod qdrant;
pub mod run;
pub mod session;
pub mod spec_stream;
pub mod tools;
pub mod validator;

pub use admission::{try_admit, AdmissionResult, GalleryStats, GalleryStore, InMemoryGalleryStore};
pub use catalog_guard::{
    default_catalog_dir, CatalogGuard, CATALOG_MANIFEST_FILE, CATALOG_SCHEMA_FILE,
    JSON_RENDER_DIR_ENV,
};
pub use context::{
    load_from_shm, read_catalog_hash_from_shm, CatalogLoadResult, GenerationContext, SchemaPayload,
};
pub use inference::InferenceLoop;
pub use run::{run_gallery_fill, RunProgress};
pub use validator::SpecValidator;

use anyhow::{Context, Result};

/// Assemble a complete generation context from the live SHM blob catalog.
///
/// This is the primary entry point for the HTTP handler: it reads the sealed
/// catalog, builds the context with static docs, and applies the operator's
/// configuration.
pub fn assemble_context(
    enable_mcp: bool,
    enable_qdrant: bool,
    operator_guidance: Option<String>,
) -> Result<GenerationContext> {
    let catalog = context::load_from_shm()?;
    // The component vocabulary is loaded from the same artifact the admission
    // gate compiles, so what the model is told and what it is held to are one
    // thing. Failing here is better than generating against a guess.
    let component_catalog = GalleryGenConfig::default().load_catalog()?;

    let mut ctx = GenerationContext::new(
        catalog.schemas,
        catalog.catalog_hash,
        component_catalog.prompt().to_string(),
    );
    ctx.mcp_enabled = enable_mcp;
    ctx.qdrant_enabled = enable_qdrant;
    ctx.operator_guidance = operator_guidance;

    Ok(ctx)
}

/// Gallery generation configuration
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GalleryGenConfig {
    /// Target number of specs to generate (default: 200)
    pub target_count: usize,

    /// Maximum stable-core elements to protect (default: 40)
    pub stable_core_max: usize,

    /// OpenAI-compatible chat endpoint (op-web owns this on :8080;
    /// ZeroClaw's daemon :8082 serves A2A, not `/v1/chat/completions`).
    pub tched_router_endpoint: String,

    /// Enable MCP tool layer for cross-blob discovery
    pub enable_mcp: bool,

    /// Enable Qdrant semantic search
    pub enable_qdrant: bool,

    /// Maximum inference turns per spec (default: 10)
    pub max_turns: usize,

    /// Directory holding the exported json-render catalog
    /// (`catalog.schema.json` + `catalog.manifest.json`), from
    /// `scripts/export-catalog-schema.mts` in the UI repo.
    ///
    /// Defaults to `schemas/json-render`, or `OPDBUS_JSON_RENDER_DIR` when set.
    #[serde(default = "default_catalog_dir")]
    pub catalog_dir: std::path::PathBuf,
}

impl Default for GalleryGenConfig {
    fn default() -> Self {
        Self {
            target_count: 200,
            stable_core_max: 40,
            tched_router_endpoint: "http://127.0.0.1:8080".to_string(),
            enable_mcp: false,
            enable_qdrant: false,
            max_turns: 10,
            catalog_dir: default_catalog_dir(),
        }
    }
}

impl GalleryGenConfig {
    /// Load the catalog vocabulary this run will admit against.
    ///
    /// Fails rather than degrading to grammar-only checks: a run that cannot
    /// read the catalog would fill the gallery with specs no renderer accepts,
    /// and it would do it silently.
    pub fn load_catalog(&self) -> Result<CatalogGuard> {
        CatalogGuard::load(&self.catalog_dir).with_context(|| {
            format!(
                "loading the json-render catalog from {} (set {} to point elsewhere; regenerate \
                 with scripts/export-catalog-schema.mts)",
                self.catalog_dir.display(),
                JSON_RENDER_DIR_ENV
            )
        })
    }
}
