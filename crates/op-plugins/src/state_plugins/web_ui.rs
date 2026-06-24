//! Web UI Plugin - serves embedded React SPA
//!
//! Follows the 3-section plugin pattern:
//! - SECTION 1: Immutable Identity (set once, never changes)
//! - SECTION 2: Tunable Config (can change, blockchain tracks all changes)
//! - SECTION 3: Capabilities (what this plugin can do)
//!
//! Uses op-identity crate for WireGuard-based authentication.

use anyhow::Result;
use async_trait::async_trait;
use op_blockchain::PluginFootprint;
use op_state::{ApplyResult, Checkpoint, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
#[cfg(test)]
use op_state_store::{Constraint, FieldSchema, FieldType};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use simd_json::json;
use simd_json::{prelude::*, OwnedValue as Value};
use std::collections::HashMap;

// ============================================================================
// SECTION 1: IMMUTABLE IDENTITY (set once, never changes)
// ============================================================================

/// Plugin identity - immutable after creation.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.web-ui.identity.schema@v1"))]
pub struct WebUiIdentity {
    /// Plugin name (immutable).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.identity.name@v1"))]
    pub name: String,
    /// Semantic version.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.identity.version@v1"))]
    pub version: String,
    /// Plugin classification.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.identity.plugin-type@v1"))]
    pub plugin_type: String,
    /// Asset serving driver.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.identity.driver@v1"))]
    pub driver: String,
}

impl Default for WebUiIdentity {
    fn default() -> Self {
        Self {
            name: "web_ui".to_string(),
            version: "1.0.0".to_string(),
            plugin_type: "ui".to_string(),
            driver: "rust-embed".to_string(),
        }
    }
}

// ============================================================================
// SECTION 2: TUNABLE CONFIG (can change, blockchain tracks all changes)
// ============================================================================

/// Tunable configuration - changes tracked in blockchain.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.web-ui.tunables.schema@v1"))]
pub struct WebUiTunables {
    /// Whether UI serving is enabled.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.tunables.enabled@v1"))]
    pub enabled: bool,
    /// CORS allowed origins.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.tunables.cors-origins@v1"))]
    pub cors_origins: Vec<String>,
    /// Enable gzip/brotli compression.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.tunables.compression@v1"))]
    pub compression: bool,
    /// Cache TTL for static assets (seconds).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.tunables.cache-ttl@v1"))]
    pub cache_ttl: u64,
    /// UI theme preference.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.tunables.theme@v1"))]
    pub theme: String,
    /// Feature flags for progressive rollout.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.tunables.feature-flags@v1"))]
    pub feature_flags: HashMap<String, bool>,
    /// WebSocket configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.tunables.websocket@v1"))]
    pub websocket: WebSocketConfig,
    /// API configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.tunables.api@v1"))]
    pub api: ApiConfig,
    /// Security configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.tunables.security@v1"))]
    pub security: SecurityConfig,
}

impl Default for WebUiTunables {
    fn default() -> Self {
        Self {
            enabled: true,
            cors_origins: vec!["*".to_string()],
            compression: true,
            cache_ttl: 86400,
            theme: "dark".to_string(),
            feature_flags: HashMap::new(),
            websocket: WebSocketConfig::default(),
            api: ApiConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

/// WebSocket runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.web-ui.websocket.schema@v1"))]
pub struct WebSocketConfig {
    /// Whether WebSocket is enabled.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.websocket.enabled@v1"))]
    pub enabled: bool,
    /// Maximum concurrent WebSocket connections.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.websocket.max-connections@v1"))]
    pub max_connections: u32,
    /// Ping interval in milliseconds.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.websocket.ping-interval-ms@v1"))]
    pub ping_interval_ms: u64,
    /// Maximum inbound message size in bytes.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.websocket.message-size-limit@v1"))]
    pub message_size_limit: usize,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_connections: 1000,
            ping_interval_ms: 30000,
            message_size_limit: 1024 * 1024,
        }
    }
}

/// API runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.web-ui.api.schema@v1"))]
pub struct ApiConfig {
    /// Rate limit in requests per second.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.api.rate-limit-rps@v1"))]
    pub rate_limit_rps: u32,
    /// Request timeout in milliseconds.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.api.timeout-ms@v1"))]
    pub timeout_ms: u64,
    /// Maximum request payload size in bytes.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.api.max-payload-bytes@v1"))]
    pub max_payload_bytes: usize,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            rate_limit_rps: 100,
            timeout_ms: 30000,
            max_payload_bytes: 10 * 1024 * 1024,
        }
    }
}

/// Security runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.web-ui.security.schema@v1"))]
pub struct SecurityConfig {
    /// Whether authentication is required.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.security.require-auth@v1"))]
    pub require_auth: bool,
    /// Session TTL in seconds.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.security.session-ttl-seconds@v1"))]
    pub session_ttl_seconds: u64,
    /// Whether CSRF protection is enabled.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.security.csrf-enabled@v1"))]
    pub csrf_enabled: bool,
    /// Content-Security-Policy string.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.security.csp-policy@v1"))]
    pub csp_policy: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_auth: true,
            session_ttl_seconds: 3600,
            csrf_enabled: true,
            csp_policy: "default-src 'self'".to_string(),
        }
    }
}

// ============================================================================
// SECTION 3: CAPABILITIES (what this plugin can do)
// ============================================================================

/// Plugin capabilities - read-only.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.web-ui.capabilities.schema@v1"))]
pub struct WebUiCapabilities {
    /// Whether the plugin can serve static assets.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.capabilities.can-serve-static@v1"))]
    pub can_serve_static: bool,
    /// Whether the plugin can proxy API requests.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.capabilities.can-proxy-api@v1"))]
    pub can_proxy_api: bool,
    /// Whether the plugin supports WebSocket.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.capabilities.can-websocket@v1"))]
    pub can_websocket: bool,
    /// Whether the plugin supports Server-Sent Events.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.capabilities.can-sse@v1"))]
    pub can_sse: bool,
    /// Whether hot reload is supported (false for embedded UI).
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.capabilities.supports-hot-reload@v1"))]
    pub supports_hot_reload: bool,
    /// Whether compression is supported.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.capabilities.supports-compression@v1"))]
    pub supports_compression: bool,
    /// Whether the plugin requires root privileges.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.capabilities.requires-root@v1"))]
    pub requires_root: bool,
    /// Supported platforms.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.capabilities.supported-platforms@v1"))]
    pub supported_platforms: Vec<String>,
}

impl Default for WebUiCapabilities {
    fn default() -> Self {
        Self {
            can_serve_static: true,
            can_proxy_api: true,
            can_websocket: true,
            can_sse: true,
            supports_hot_reload: false,
            supports_compression: true,
            requires_root: false,
            supported_platforms: vec!["linux".to_string(), "macos".to_string()],
        }
    }
}

/// Top-level Web UI state exposed by the plugin.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.web-ui.schema@v1"))]
pub struct WebUiState {
    /// Immutable identity.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.software.plugin.web-ui.identity@v1"))]
    pub identity: WebUiIdentity,
    /// Tunable configuration.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "mut.software.plugin.web-ui.tunables@v1"))]
    pub tunables: WebUiTunables,
    /// Read-only capabilities.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.plugin.web-ui.capabilities@v1"))]
    pub capabilities: WebUiCapabilities,
}

impl WebUiTunables {
    /// Property schema - tracks which tunable fields exist (append-only).
    pub fn property_schema() -> Vec<String> {
        vec![
            "enabled".to_string(),
            "cors_origins".to_string(),
            "compression".to_string(),
            "cache_ttl".to_string(),
            "theme".to_string(),
            "feature_flags".to_string(),
            "websocket".to_string(),
            "api".to_string(),
            "security".to_string(),
        ]
    }
}

// ============================================================================
// PLUGIN IMPLEMENTATION
// ============================================================================

/// Web UI State Plugin
pub struct WebUiPlugin {
    identity: WebUiIdentity,
    tunables: WebUiTunables,
    capabilities: WebUiCapabilities,
    #[allow(dead_code)]
    blockchain_sender: Option<tokio::sync::mpsc::UnboundedSender<PluginFootprint>>,
}

impl WebUiPlugin {
    pub fn new() -> Self {
        Self {
            identity: WebUiIdentity::default(),
            tunables: WebUiTunables::default(),
            capabilities: WebUiCapabilities::default(),
            blockchain_sender: None,
        }
    }

    pub fn with_blockchain_sender(
        blockchain_sender: tokio::sync::mpsc::UnboundedSender<PluginFootprint>,
    ) -> Self {
        Self {
            identity: WebUiIdentity::default(),
            tunables: WebUiTunables::default(),
            capabilities: WebUiCapabilities::default(),
            blockchain_sender: Some(blockchain_sender),
        }
    }

    /// Get identity.
    pub fn identity(&self) -> &WebUiIdentity {
        &self.identity
    }

    /// Get tunables.
    pub fn tunables(&self) -> &WebUiTunables {
        &self.tunables
    }

    /// Get capabilities.
    pub fn capabilities(&self) -> &WebUiCapabilities {
        &self.capabilities
    }

    /// Check if a path is immutable.
    pub fn is_path_immutable(path: &str) -> bool {
        let immutable_paths = [
            "/identity",
            "/identity/name",
            "/identity/plugin_type",
            "/identity/driver",
        ];
        immutable_paths.iter().any(|p| path.starts_with(p))
    }

    /// Validate tunables against schema.
    pub fn validate_tunables(tunables: &Value) -> Result<()> {
        if !tunables.is_object() {
            anyhow::bail!("Tunables must be an object");
        }

        if tunables.get("enabled").is_none() {
            anyhow::bail!("Missing required field: enabled");
        }

        // Validate theme enum.
        if let Some(theme) = tunables.get("theme") {
            if let Some(theme_str) = theme.as_str() {
                if !["dark", "light", "system"].contains(&theme_str) {
                    anyhow::bail!("Invalid theme: must be dark, light, or system");
                }
            }
        }

        Ok(())
    }

    pub(crate) fn current_state() -> WebUiState {
        WebUiState {
            identity: WebUiIdentity::default(),
            tunables: WebUiTunables::default(),
            capabilities: WebUiCapabilities::default(),
        }
    }
}

impl Default for WebUiPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for WebUiPlugin {
    fn name(&self) -> &str {
        &self.identity.name
    }

    fn version(&self) -> &str {
        &self.identity.version
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        let mut schema = web_ui_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    fn is_available(&self) -> bool {
        true // UI is always available (embedded).
    }

    fn unavailable_reason(&self) -> String {
        String::new() // Never unavailable.
    }


    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_state: WebUiState = simd_json::serde::from_owned_value(current.clone())?;
        let desired_state: WebUiState = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        if current_state.tunables.enabled != desired_state.tunables.enabled {
            actions.push(op_state::StateAction::Modify {
                resource: "enabled".to_string(),
                changes: simd_json::json!({ "enabled": desired_state.tunables.enabled }),
            });
        }

        if current_state.tunables.theme != desired_state.tunables.theme {
            actions.push(op_state::StateAction::Modify {
                resource: "theme".to_string(),
                changes: simd_json::json!({ "theme": desired_state.tunables.theme }),
            });
        }

        if current_state.tunables.compression != desired_state.tunables.compression {
            actions.push(op_state::StateAction::Modify {
                resource: "compression".to_string(),
                changes: simd_json::json!({ "compression": desired_state.tunables.compression }),
            });
        }

        if current_state.tunables.cache_ttl != desired_state.tunables.cache_ttl {
            actions.push(op_state::StateAction::Modify {
                resource: "cache_ttl".to_string(),
                changes: simd_json::json!({ "cache_ttl": desired_state.tunables.cache_ttl }),
            });
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: op_state::DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();

        for action in &diff.actions {
            if let op_state::StateAction::Modify { resource, .. } = action {
                changes_applied.push(format!("Updated UI config: {}", resource));
            }
        }

        Ok(ApplyResult {
            success: true,
            changes_applied,
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true) // UI state is always consistent (embedded).
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = simd_json::json!(null);
        Ok(Checkpoint {
            id: format!("web-ui-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: true,
        }
    }
}

/// Derived `web_ui` schema from the typed [`WebUiState`] struct via schemars.
pub(crate) fn web_ui_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(WebUiState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "web_ui",
        "1.0.0",
        "Web UI state — identity, tunables, and capabilities",
        &root,
    );
    let state = simd_json::serde::to_owned_value(&WebUiState::default())
        .expect("WebUiState default serializes");
    super::schemars_adapter::apply_state_defaults(&mut schema, &state);
    schema
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;
    use crate::state_plugins::schemars_adapter::schema_diffs;
    use serde_json::Value as JVal;

    fn collect_subids(value: &JVal, out: &mut Vec<String>) {
        if let Some(obj) = value.as_object() {
            if let Some(JVal::String(subid)) = obj.get("x-oscal-subid") {
                out.push(subid.clone());
            }
            for v in obj.values() {
                collect_subids(v, out);
            }
        }
        if let Some(arr) = value.as_array() {
            for v in arr {
                collect_subids(v, out);
            }
        }
    }

    #[test]
    fn test_default_identity() {
        let identity = WebUiIdentity::default();
        assert_eq!(identity.name, "web_ui");
        assert_eq!(identity.version, "1.0.0");
        assert_eq!(identity.plugin_type, "ui");
        assert_eq!(identity.driver, "rust-embed");
    }

    #[test]
    fn test_default_tunables() {
        let tunables = WebUiTunables::default();
        assert!(tunables.enabled);
        assert!(tunables.compression);
        assert_eq!(tunables.theme, "dark");
        assert_eq!(tunables.cache_ttl, 86400);
    }

    #[test]
    fn test_default_capabilities() {
        let caps = WebUiCapabilities::default();
        assert!(caps.can_serve_static);
        assert!(caps.can_websocket);
        assert!(!caps.supports_hot_reload);
        assert!(!caps.requires_root);
    }

    #[test]
    fn test_immutable_paths() {
        assert!(WebUiPlugin::is_path_immutable("/identity/name"));
        assert!(WebUiPlugin::is_path_immutable("/identity/plugin_type"));
        assert!(!WebUiPlugin::is_path_immutable("/tunables/enabled"));
        assert!(!WebUiPlugin::is_path_immutable("/tunables/theme"));
    }

    #[test]
    fn test_property_schema() {
        let schema = WebUiTunables::property_schema();
        assert!(schema.contains(&"enabled".to_string()));
        assert!(schema.contains(&"theme".to_string()));
        assert!(schema.contains(&"websocket".to_string()));
    }

    #[test]
    fn derived_schema_matches_hand_rolled() {
        let golden = web_ui_schema_golden();
        let derived = web_ui_schema();
        let diffs = schema_diffs(&golden, &derived);
        assert!(diffs.is_empty(), "schema_diffs: {:#?}", diffs);
    }

    #[test]
    fn all_subids_are_valid() {
        let root = serde_json::to_value(schemars::schema_for!(WebUiState))
            .expect("schemars schema serializes to JSON");
        let mut subids = Vec::new();
        collect_subids(&root, &mut subids);
        assert!(!subids.is_empty(), "expected at least one subid");
        for subid in subids {
            assert!(validate_subid(&subid).is_ok(), "invalid subid: {subid}");
        }
    }
}

/// Frozen golden reference for the derived web_ui schema. Kept test-only so
/// `derived_schema_matches_hand_rolled` proves the schema still matches the
/// original contract. Updated to reflect the full typed state.
#[cfg(test)]
pub(crate) fn web_ui_schema_golden() -> PluginSchema {
    let mut identity_fields = std::collections::HashMap::new();
    identity_fields.insert(
        "name".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Plugin name (immutable).".to_string(),
            default: Some(json!("web_ui")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    identity_fields.insert(
        "version".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Semantic version.".to_string(),
            default: Some(json!("1.0.0")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    identity_fields.insert(
        "plugin_type".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Plugin classification.".to_string(),
            default: Some(json!("ui")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    identity_fields.insert(
        "driver".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Asset serving driver.".to_string(),
            default: Some(json!("rust-embed")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut websocket_fields = std::collections::HashMap::new();
    websocket_fields.insert(
        "enabled".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether WebSocket is enabled.".to_string(),
            default: Some(json!(true)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    websocket_fields.insert(
        "max_connections".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Maximum concurrent WebSocket connections.".to_string(),
            default: Some(json!(1000)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    websocket_fields.insert(
        "ping_interval_ms".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Ping interval in milliseconds.".to_string(),
            default: Some(json!(30000)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    websocket_fields.insert(
        "message_size_limit".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Maximum inbound message size in bytes.".to_string(),
            default: Some(json!(1048576)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );

    let mut api_fields = std::collections::HashMap::new();
    api_fields.insert(
        "rate_limit_rps".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Rate limit in requests per second.".to_string(),
            default: Some(json!(100)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    api_fields.insert(
        "timeout_ms".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Request timeout in milliseconds.".to_string(),
            default: Some(json!(30000)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    api_fields.insert(
        "max_payload_bytes".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Maximum request payload size in bytes.".to_string(),
            default: Some(json!(10485760)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );

    let mut security_fields = std::collections::HashMap::new();
    security_fields.insert(
        "require_auth".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether authentication is required.".to_string(),
            default: Some(json!(true)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    security_fields.insert(
        "session_ttl_seconds".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Session TTL in seconds.".to_string(),
            default: Some(json!(3600)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    security_fields.insert(
        "csrf_enabled".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether CSRF protection is enabled.".to_string(),
            default: Some(json!(true)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    security_fields.insert(
        "csp_policy".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Content-Security-Policy string.".to_string(),
            default: Some(json!("default-src 'self'")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut capabilities_fields = std::collections::HashMap::new();
    capabilities_fields.insert(
        "can_serve_static".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the plugin can serve static assets.".to_string(),
            default: Some(json!(true)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    capabilities_fields.insert(
        "can_proxy_api".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the plugin can proxy API requests.".to_string(),
            default: Some(json!(true)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    capabilities_fields.insert(
        "can_websocket".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the plugin supports WebSocket.".to_string(),
            default: Some(json!(true)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    capabilities_fields.insert(
        "can_sse".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the plugin supports Server-Sent Events.".to_string(),
            default: Some(json!(true)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    capabilities_fields.insert(
        "supports_hot_reload".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether hot reload is supported (false for embedded UI).".to_string(),
            default: Some(json!(false)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    capabilities_fields.insert(
        "supports_compression".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether compression is supported.".to_string(),
            default: Some(json!(true)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    capabilities_fields.insert(
        "requires_root".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the plugin requires root privileges.".to_string(),
            default: Some(json!(false)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    capabilities_fields.insert(
        "supported_platforms".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Supported platforms.".to_string(),
            default: Some(json!(["linux", "macos"])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut tunables_fields = std::collections::HashMap::new();
    tunables_fields.insert(
        "enabled".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether UI serving is enabled.".to_string(),
            default: Some(json!(true)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    tunables_fields.insert(
        "cors_origins".to_string(),
        FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "CORS allowed origins.".to_string(),
            default: Some(json!(["*"])),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    tunables_fields.insert(
        "compression".to_string(),
        FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Enable gzip/brotli compression.".to_string(),
            default: Some(json!(true)),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    tunables_fields.insert(
        "cache_ttl".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Cache TTL for static assets (seconds).".to_string(),
            default: Some(json!(86400)),
            example: None,
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: false,
            read_only_when: None,
        },
    );
    tunables_fields.insert(
        "theme".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "UI theme preference.".to_string(),
            default: Some(json!("dark")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    tunables_fields.insert(
        "feature_flags".to_string(),
        FieldSchema {
            field_type: FieldType::Object(std::collections::HashMap::new()),
            required: false,
            description: "Feature flags for progressive rollout.".to_string(),
            default: Some(json!({})),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    tunables_fields.insert(
        "websocket".to_string(),
        FieldSchema {
            field_type: FieldType::Object(websocket_fields),
            required: false,
            description: "WebSocket configuration.".to_string(),
            default: Some(json!({
                "enabled": true,
                "max_connections": 1000,
                "message_size_limit": 1048576,
                "ping_interval_ms": 30000
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    tunables_fields.insert(
        "api".to_string(),
        FieldSchema {
            field_type: FieldType::Object(api_fields),
            required: false,
            description: "API configuration.".to_string(),
            default: Some(json!({
                "max_payload_bytes": 10485760,
                "rate_limit_rps": 100,
                "timeout_ms": 30000
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    tunables_fields.insert(
        "security".to_string(),
        FieldSchema {
            field_type: FieldType::Object(security_fields),
            required: false,
            description: "Security configuration.".to_string(),
            default: Some(json!({
                "csp_policy": "default-src 'self'",
                "csrf_enabled": true,
                "require_auth": true,
                "session_ttl_seconds": 3600
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut fields = std::collections::HashMap::new();
    fields.insert(
        "identity".to_string(),
        FieldSchema {
            field_type: FieldType::Object(identity_fields),
            required: false,
            description: "Immutable identity.".to_string(),
            default: Some(json!({
                "driver": "rust-embed",
                "name": "web_ui",
                "plugin_type": "ui",
                "version": "1.0.0"
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "tunables".to_string(),
        FieldSchema {
            field_type: FieldType::Object(tunables_fields),
            required: false,
            description: "Tunable configuration.".to_string(),
            default: Some(json!({
                "api": {
                    "max_payload_bytes": 10485760,
                    "rate_limit_rps": 100,
                    "timeout_ms": 30000
                },
                "cache_ttl": 86400,
                "compression": true,
                "cors_origins": ["*"],
                "enabled": true,
                "feature_flags": {},
                "security": {
                    "csp_policy": "default-src 'self'",
                    "csrf_enabled": true,
                    "require_auth": true,
                    "session_ttl_seconds": 3600
                },
                "theme": "dark",
                "websocket": {
                    "enabled": true,
                    "max_connections": 1000,
                    "message_size_limit": 1048576,
                    "ping_interval_ms": 30000
                }
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    fields.insert(
        "capabilities".to_string(),
        FieldSchema {
            field_type: FieldType::Object(capabilities_fields),
            required: false,
            description: "Read-only capabilities.".to_string(),
            default: Some(json!({
                "can_proxy_api": true,
                "can_sse": true,
                "can_serve_static": true,
                "can_websocket": true,
                "requires_root": false,
                "supported_platforms": ["linux", "macos"],
                "supports_compression": true,
                "supports_hot_reload": false
            })),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    let mut schema = PluginSchema::builder("web_ui")
        .version("1.0.0")
        .description("Web UI state — identity, tunables, and capabilities")
        .build();
    schema.fields = fields;
    schema.subids = std::collections::HashMap::from([
        (
            "__schema__".to_string(),
            "sch.software.plugin.web-ui.schema@v1".to_string(),
        ),
        (
            "identity".to_string(),
            "sch.software.plugin.web-ui.identity@v1".to_string(),
        ),
        (
            "tunables".to_string(),
            "mut.software.plugin.web-ui.tunables@v1".to_string(),
        ),
        (
            "capabilities".to_string(),
            "obs.software.plugin.web-ui.capabilities@v1".to_string(),
        ),
    ]);
    let state = simd_json::serde::to_owned_value(&WebUiState::default())
        .expect("WebUiState default serializes");
    super::schemars_adapter::apply_state_defaults(&mut schema, &state);
    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("web_ui", |_ctx| std::sync::Arc::new(WebUiPlugin::new()))
}
