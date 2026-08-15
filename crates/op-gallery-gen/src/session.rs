//! Gallery generation session management.
//!
//! Defines the gallery-gen session mode for the Antigravity chat interface.
//! Each session is fully isolated — no state persists between runs (REQ-6).
//!
//! The session UI is rendered as a json-render spec by the same interpreter
//! used for gallery specs. Tier toggles, progress indicators, and generation
//! logs are DSL elements.

use serde::{Deserialize, Serialize};

/// Session mode identifier stored in chat metadata.
pub const SESSION_MODE_KEY: &str = "session_mode";
pub const SESSION_MODE_VALUE: &str = "gallery-gen";

/// Gallery generation session configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryGenSession {
    /// Unique session ID (from chat session).
    pub session_id: String,

    /// Tier configuration.
    pub tiers: TierConfig,

    /// Operator guidance text (appended after universal prompt).
    pub operator_guidance: Option<String>,

    /// Target number of specs to generate.
    pub target_count: usize,

    /// Session state.
    pub state: SessionState,
}

/// Tier toggle configuration for a generation session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    /// Baseline is always on (read-only).
    pub baseline: bool,
    /// MCP cross-blob discovery toggle.
    pub mcp_enabled: bool,
    /// Qdrant semantic search toggle (requires MCP).
    pub qdrant_enabled: bool,
}

impl Default for TierConfig {
    fn default() -> Self {
        Self {
            baseline: true,
            mcp_enabled: false,
            qdrant_enabled: false,
        }
    }
}

/// Session lifecycle state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Awaiting operator configuration and guidance.
    Configuring,
    /// Generation is running.
    Running,
    /// Generation was cancelled by operator.
    Cancelled,
    /// Generation completed (success or partial).
    Completed,
}

/// Progress event emitted during generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgressEvent {
    /// Context assembly started.
    Assembling {
        catalog_hash: String,
        plugin_count: usize,
    },
    /// Generation started.
    GenerationStarted { target: usize, model: String },
    /// A spec was admitted to the gallery.
    Admitted {
        spec_id: String,
        index: usize,
        total: usize,
    },
    /// A spec was rejected (validation or dedup failure).
    Rejected { reason: String, index: usize },
    /// Generation completed.
    Complete {
        generated: usize,
        target: usize,
        attempts: usize,
    },
    /// Generation was cancelled.
    Cancelled { generated: usize },
    /// An error occurred.
    Error { message: String },
}

/// Render the gallery-gen session as a json-render spec.
///
/// Reports the session's configuration and state; it does not offer controls,
/// because the catalog declares no input components (no toggle, no text or
/// number input, no button group). The operator changes a session through the
/// chat/HTTP path, and this spec shows what that produced. Anything else here
/// would be a component the renderer cannot render.
///
/// Progress lines are bound rather than baked in: the log grows while this spec
/// stays fixed, so the host feeds `/progress_log` and the renderer follows it.
pub fn render_session_spec(session: &GalleryGenSession) -> serde_json::Value {
    // `card` and `badge` do not accept the same tones — the catalog gives badge
    // an extra `info` — so the mapping is per component rather than shared.
    let (card_tone, badge_tone) = match session.state {
        SessionState::Configuring => ("default", "default"),
        SessionState::Running => ("default", "info"),
        SessionState::Completed => ("ok", "ok"),
        SessionState::Cancelled => ("warn", "warn"),
    };

    serde_json::json!({
        "root": "session-root",
        "elements": {
            "session-root": {
                "type": "container",
                "props": {},
                "children": ["header", "config-panel", "progress-panel"]
            },
            "header": {
                "type": "heading",
                "props": { "text": "Gallery Generation Session", "level": 2 }
            },
            "config-panel": {
                "type": "card",
                "props": { "title": "Configuration", "tone": "default" },
                "children": [
                    "session-id",
                    "tier-baseline",
                    "tier-mcp",
                    "tier-qdrant",
                    "target-count",
                    "guidance"
                ]
            },
            "session-id": {
                "type": "kv",
                "props": { "label": "Session", "value": session.session_id, "kind": null }
            },
            "tier-baseline": {
                "type": "kv",
                "props": { "label": "Baseline", "value": "on (always)", "kind": null }
            },
            "tier-mcp": {
                "type": "kv",
                "props": {
                    "label": "MCP cross-discovery",
                    "value": if session.tiers.mcp_enabled { "on" } else { "off" },
                    "kind": null
                }
            },
            "tier-qdrant": {
                "type": "kv",
                "props": {
                    "label": "Qdrant semantic search",
                    "value": if session.tiers.qdrant_enabled { "on" } else { "off" },
                    "kind": null
                }
            },
            "target-count": {
                "type": "kv",
                "props": { "label": "Target specs", "value": session.target_count, "kind": null }
            },
            "guidance": {
                "type": "kv",
                "props": {
                    "label": "Operator guidance",
                    "value": session.operator_guidance.clone().unwrap_or_else(|| "(none)".to_string()),
                    "kind": null
                }
            },
            "progress-panel": {
                "type": "card",
                "props": { "title": "Progress", "tone": card_tone },
                "children": ["state-badge", "progress-log"]
            },
            "state-badge": {
                "type": "badge",
                "props": { "text": format!("{:?}", session.state), "tone": badge_tone }
            },
            "progress-log": {
                "type": "code",
                "props": { "content": { "$state": "/progress_log" } }
            }
        },
        "state": { "progress_log": "" }
    })
}

/// Create a new gallery-gen session.
pub fn create_session(session_id: String) -> GalleryGenSession {
    GalleryGenSession {
        session_id,
        tiers: TierConfig::default(),
        operator_guidance: None,
        target_count: 58, // Default: fill empty novelty slots (200 - 40 stable - existing)
        state: SessionState::Configuring,
    }
}

/// Ensure session isolation: clear all state.
/// Called at session end to guarantee REQ-6 (no state persists between runs).
pub fn destroy_session(_session: GalleryGenSession) {
    // The session is dropped here. No external state to clean up because:
    // 1. GenerationContext is ephemeral (assembled fresh each run)
    // 2. No entries written to cognitive-mcp memory
    // 3. No entries written to disk
    // 4. RunProgress atomics are reset at next run start
    // The drop is the cleanup.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CatalogGuard, SpecValidator};

    fn validator() -> SpecValidator {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/json-render");
        SpecValidator::with_catalog(CatalogGuard::load(&dir).expect("catalog artifact"))
    }

    /// This spec ships hand-written, so it is held to the same gate as generated
    /// specs. Without this test it is the one spec in the system that can name a
    /// component the renderer does not have and never be told — which is how it
    /// came to be written entirely in a retired dialect.
    #[test]
    fn every_session_state_renders_an_admissible_spec() {
        let validator = validator();

        for state in [
            SessionState::Configuring,
            SessionState::Running,
            SessionState::Completed,
            SessionState::Cancelled,
        ] {
            let mut session = create_session("gallery-gen-1".to_string());
            session.state = state;
            session.tiers.mcp_enabled = true;
            session.operator_guidance = Some("focus on network observability".to_string());

            let spec = render_session_spec(&session);
            let result = validator.validate(&spec);
            assert!(
                result.valid,
                "session spec for {:?} must be admissible: {:?}",
                session.state, result.errors
            );
        }
    }

    #[test]
    fn the_spec_reports_the_session_it_was_given() {
        let mut session = create_session("gallery-gen-7".to_string());
        session.target_count = 12;
        session.tiers.qdrant_enabled = true;

        let spec = render_session_spec(&session);
        let elements = &spec["elements"];
        assert_eq!(elements["session-id"]["props"]["value"], "gallery-gen-7");
        assert_eq!(elements["target-count"]["props"]["value"], 12);
        assert_eq!(elements["tier-qdrant"]["props"]["value"], "on");
        assert_eq!(elements["tier-mcp"]["props"]["value"], "off");
    }
}
