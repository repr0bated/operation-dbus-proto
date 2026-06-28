//! Shared contract/orchestration types for the Zeroclaw LLM surface.
//!
//! Layer 1 (contract) lives in `llm_projection` + `errors`; Layer 2
//! (orchestration) lives in `selector`. None of these modules carry provider
//! HTTP/runtime behavior — that is the absorbed Provider Adapter Layer
//! (spec §5).

pub mod errors;
pub mod llm_projection;
pub mod selector;
