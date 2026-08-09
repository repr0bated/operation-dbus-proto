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
    GenerationStarted {
        target: usize,
        model: String,
    },
    /// A spec was admitted to the gallery.
    Admitted {
        spec_id: String,
        index: usize,
        total: usize,
    },
    /// A spec was rejected (validation or dedup failure).
    Rejected {
        reason: String,
        index: usize,
    },
    /// Generation completed.
    Complete {
        generated: usize,
        target: usize,
        attempts: usize,
    },
    /// Generation was cancelled.
    Cancelled {
        generated: usize,
    },
    /// An error occurred.
    Error {
        message: String,
    },
}

/// Render the gallery-gen session UI as a json-render spec.
///
/// This spec is rendered by the same DSL interpreter as gallery elements.
/// It provides: tier toggles, guidance input, generate/cancel buttons, and
/// a progress log.
pub fn render_session_spec(session: &GalleryGenSession) -> serde_json::Value {
    serde_json::json!({
        "root": "session-root",
        "elements": {
            "session-root": {
                "type": "stack",
                "props": { "dir": "v", "gap": 12 },
                "children": ["header", "config-panel", "actions", "progress-log"]
            },
            "header": {
                "type": "heading",
                "props": { "text": "Gallery Generation Session", "size": 20 }
            },
            "config-panel": {
                "type": "card",
                "props": { "title": "Configuration" },
                "children": ["tier-baseline", "tier-mcp", "tier-qdrant", "target-input", "guidance-input"]
            },
            "tier-baseline": {
                "type": "kv_pair",
                "props": { "key": "Baseline", "value": "ON (always)" }
            },
            "tier-mcp": {
                "type": "toggle",
                "props": {
                    "label": "MCP Cross-Discovery",
                    "bind": "/tiers/mcp_enabled"
                }
            },
            "tier-qdrant": {
                "type": "toggle",
                "props": {
                    "label": "Qdrant Semantic Search",
                    "bind": "/tiers/qdrant_enabled"
                },
                "visible": "/tiers/mcp_enabled"
            },
            "target-input": {
                "type": "number_input",
                "props": {
                    "label": "Target Specs",
                    "bind": "/target_count",
                    "min": 1,
                    "max": 200,
                    "step": 1
                }
            },
            "guidance-input": {
                "type": "text_input",
                "props": {
                    "label": "Operator Guidance (optional)",
                    "bind": "/operator_guidance",
                    "placeholder": "e.g., focus on network observability, for a compliance auditor"
                }
            },
            "actions": {
                "type": "button_group",
                "props": {
                    "buttons": [
                        { "label": "Generate", "variant": "default" },
                        { "label": "Cancel", "variant": "destructive" }
                    ]
                }
            },
            "progress-log": {
                "type": "card",
                "props": { "title": "Progress" },
                "children": ["log-stream"]
            },
            "log-stream": {
                "type": "log_stream",
                "props": {
                    "bind": "/progress_log",
                    "autoScroll": true,
                    "maxHeight": "400px"
                }
            }
        }
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
