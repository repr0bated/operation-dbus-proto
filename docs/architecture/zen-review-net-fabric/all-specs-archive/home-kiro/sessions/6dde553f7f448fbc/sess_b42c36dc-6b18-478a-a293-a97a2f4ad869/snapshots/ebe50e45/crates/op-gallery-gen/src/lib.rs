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
//! is ZeroClaw's responsibility via the zeroclaw plugin's `selected_provider` and
//! `selected_model` fields.

pub mod admission;
pub mod context;
pub mod inference;
pub mod tools;
pub mod validator;

pub use admission::GalleryAdmission;
pub use context::{GenerationContext, SchemaPayload};
pub use inference::InferenceLoop;
pub use validator::SpecValidator;

/// Gallery generation configuration
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GalleryGenConfig {
    /// Target number of specs to generate (default: 200)
    pub target_count: usize,

    /// Maximum stable-core elements to protect (default: 40)
    pub stable_core_max: usize,

    /// ZeroClaw HTTP endpoint (default: "http://localhost:8082")
    pub zeroclaw_endpoint: String,

    /// Enable MCP tool layer for cross-blob discovery
    pub enable_mcp: bool,

    /// Enable Qdrant semantic search
    pub enable_qdrant: bool,

    /// Maximum inference turns per spec (default: 10)
    pub max_turns: usize,
}

impl Default for GalleryGenConfig {
    fn default() -> Self {
        Self {
            target_count: 200,
            stable_core_max: 40,
            zeroclaw_endpoint: "http://localhost:8082".to_string(),
            enable_mcp: false,
            enable_qdrant: false,
            max_turns: 10,
        }
    }
}
