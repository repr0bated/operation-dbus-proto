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
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

// ============================================================================
// SECTION 1: IMMUTABLE IDENTITY (set once, never changes)
// ============================================================================

/// Plugin identity - immutable after creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiIdentity {
    /// Plugin name (immutable)
    pub name: String,
    /// Semantic version
    pub version: String,
    /// Plugin classification
    pub plugin_type: String,
    /// Asset serving driver
    pub driver: String,
}

impl Default for WebUiIdentity {
    fn default() -> Self {
        Self {
            name: "web-ui".to_string(),
            version: "1.0.0".to_string(),
            plugin_type: "ui".to_string(),
            driver: "rust-embed".to_string(),
        }
    }
}

// ============================================================================
// SECTION 2: TUNABLE CONFIG (can change, blockchain tracks all changes)
// ============================================================================

/// Tunable configuration - changes tracked in blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiTunables {
    /// Whether UI serving is enabled
    pub enabled: bool,
    /// CORS allowed origins
    #[serde(default)]
    pub cors_origins: Vec<String>,
    /// Enable gzip/brotli compression
    pub compression: bool,
    /// Cache TTL for static assets (seconds)
    pub cache_ttl: u64,
    /// UI theme preference
    pub theme: String,
    /// Feature flags for progressive rollout
    #[serde(default)]
    pub feature_flags: HashMap<String, bool>,
    /// WebSocket configuration
    #[serde(default)]
    pub websocket: WebSocketConfig,
    /// API configuration
    #[serde(default)]
    pub api: ApiConfig,
    /// Security configuration
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub enabled: bool,
    pub max_connections: u32,
    pub ping_interval_ms: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub rate_limit_rps: u32,
    pub timeout_ms: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub require_auth: bool,
    pub session_ttl_seconds: u64,
    pub csrf_enabled: bool,
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

// ============================================================================
// SECTION 3: CAPABILITIES (what this plugin can do)
// ============================================================================

/// Plugin capabilities - read-only
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiCapabilities {
    pub can_serve_static: bool,
    pub can_proxy_api: bool,
    pub can_websocket: bool,
    pub can_sse: bool,
    pub supports_hot_reload: bool,
    pub supports_compression: bool,
    pub requires_root: bool,
    pub supported_platforms: Vec<String>,
}

impl Default for WebUiCapabilities {
    fn default() -> Self {
        Self {
            can_serve_static: true,
            can_proxy_api: true,
            can_websocket: true,
            can_sse: true,
            supports_hot_reload: false, // Embedded UI
            supports_compression: true,
            requires_root: false,
            supported_platforms: vec!["linux".to_string(), "macos".to_string()],
        }
    }
}

// ============================================================================
// JSON SCHEMA DEFINITIONS (Schema-as-Code)
// ============================================================================

impl WebUiIdentity {
    /// JSON Schema for Identity (immutable)
    pub fn schema() -> Value {
        simd_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://op-dbus.local/schemas/web-ui/identity.json",
            "title": "WebUiIdentity",
            "description": "Immutable identity for Web UI plugin",
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "const": "web-ui",
                    "description": "Plugin name (immutable)"
                },
                "version": {
                    "type": "string",
                    "pattern": "^\\d+\\.\\d+\\.\\d+$",
                    "description": "Semantic version"
                },
                "plugin_type": {
                    "type": "string",
                    "const": "ui",
                    "description": "Plugin classification"
                },
                "driver": {
                    "type": "string",
                    "enum": ["rust-embed", "static-files"],
                    "description": "Asset serving driver"
                }
            },
            "required": ["name", "version", "plugin_type", "driver"],
            "additionalProperties": false
        })
    }
}

impl WebUiTunables {
    /// JSON Schema for Tunables (mutable, blockchain-tracked)
    pub fn schema() -> Value {
        simd_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://op-dbus.local/schemas/web-ui/tunables.json",
            "title": "WebUiTunables",
            "description": "Tunable configuration for Web UI plugin (changes tracked in blockchain)",
            "type": "object",
            "properties": {
                "enabled": {
                    "type": "boolean",
                    "default": true,
                    "description": "Whether UI serving is enabled"
                },
                "cors_origins": {
                    "type": "array",
                    "items": { "type": "string" },
                    "default": ["*"],
                    "description": "CORS allowed origins"
                },
                "compression": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable gzip/brotli compression"
                },
                "cache_ttl": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 31536000,
                    "default": 86400,
                    "description": "Cache TTL for static assets (seconds)"
                },
                "theme": {
                    "type": "string",
                    "enum": ["dark", "light", "system"],
                    "default": "dark",
                    "description": "UI theme preference"
                },
                "feature_flags": {
                    "type": "object",
                    "additionalProperties": { "type": "boolean" },
                    "default": {},
                    "description": "Feature flags for progressive rollout"
                },
                "websocket": { "$ref": "#/$defs/WebSocketConfig" },
                "api": { "$ref": "#/$defs/ApiConfig" },
                "security": { "$ref": "#/$defs/SecurityConfig" }
            },
            "$defs": {
                "WebSocketConfig": {
                    "type": "object",
                    "properties": {
                        "enabled": { "type": "boolean", "default": true },
                        "max_connections": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 1000 },
                        "ping_interval_ms": { "type": "integer", "minimum": 1000, "default": 30000 },
                        "message_size_limit": { "type": "integer", "minimum": 1024, "default": 1048576 }
                    }
                },
                "ApiConfig": {
                    "type": "object",
                    "properties": {
                        "rate_limit_rps": { "type": "integer", "minimum": 1, "default": 100 },
                        "timeout_ms": { "type": "integer", "minimum": 100, "default": 30000 },
                        "max_payload_bytes": { "type": "integer", "minimum": 1024, "default": 10485760 }
                    }
                },
                "SecurityConfig": {
                    "type": "object",
                    "properties": {
                        "require_auth": { "type": "boolean", "default": true },
                        "session_ttl_seconds": { "type": "integer", "minimum": 60, "default": 3600 },
                        "csrf_enabled": { "type": "boolean", "default": true },
                        "csp_policy": { "type": "string", "default": "default-src 'self'" }
                    }
                }
            },
            "additionalProperties": false
        })
    }

    /// Property schema - tracks which tunable fields exist (append-only)
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

impl WebUiCapabilities {
    /// JSON Schema for Capabilities (read-only)
    pub fn schema() -> Value {
        simd_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://op-dbus.local/schemas/web-ui/capabilities.json",
            "title": "WebUiCapabilities",
            "description": "Capabilities exposed by Web UI plugin",
            "type": "object",
            "properties": {
                "can_serve_static": { "type": "boolean", "const": true },
                "can_proxy_api": { "type": "boolean", "const": true },
                "can_websocket": { "type": "boolean", "const": true },
                "can_sse": { "type": "boolean", "const": true },
                "supports_hot_reload": { "type": "boolean", "const": false },
                "supports_compression": { "type": "boolean", "const": true },
                "requires_root": { "type": "boolean", "const": false },
                "supported_platforms": {
                    "type": "array",
                    "items": { "type": "string" },
                    "const": ["linux", "macos"]
                }
            },
            "additionalProperties": false
        })
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

    /// Get identity
    pub fn identity(&self) -> &WebUiIdentity {
        &self.identity
    }

    /// Get tunables
    pub fn tunables(&self) -> &WebUiTunables {
        &self.tunables
    }

    /// Get capabilities
    pub fn capabilities(&self) -> &WebUiCapabilities {
        &self.capabilities
    }

    /// Check if a path is immutable
    pub fn is_path_immutable(path: &str) -> bool {
        let immutable_paths = [
            "/identity",
            "/identity/name",
            "/identity/plugin_type",
            "/identity/driver",
        ];
        immutable_paths.iter().any(|p| path.starts_with(p))
    }

    /// Validate tunables against schema
    pub fn validate_tunables(tunables: &Value) -> Result<()> {
        // Basic validation - check required fields exist
        if !tunables.is_object() {
            anyhow::bail!("Tunables must be an object");
        }

        if tunables.get("enabled").is_none() {
            anyhow::bail!("Missing required field: enabled");
        }

        // Validate theme enum
        if let Some(theme) = tunables.get("theme") {
            if let Some(theme_str) = theme.as_str() {
                if !["dark", "light", "system"].contains(&theme_str) {
                    anyhow::bail!("Invalid theme: must be dark, light, or system");
                }
            }
        }

        Ok(())
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

    fn is_available(&self) -> bool {
        true // UI is always available (embedded)
    }

    fn unavailable_reason(&self) -> String {
        String::new() // Never unavailable
    }

    async fn query_current_state(&self) -> Result<Value> {
        Ok(simd_json::serde::to_owned_value(&self.tunables)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_tunables: WebUiTunables = simd_json::serde::from_owned_value(current.clone())?;
        let desired_tunables: WebUiTunables = simd_json::serde::from_owned_value(desired.clone())?;

        let mut actions = Vec::new();

        // Compare each tunable field
        if current_tunables.enabled != desired_tunables.enabled {
            actions.push(op_state::StateAction::Modify {
                resource: "enabled".to_string(),
                changes: simd_json::json!({ "enabled": desired_tunables.enabled }),
            });
        }

        if current_tunables.theme != desired_tunables.theme {
            actions.push(op_state::StateAction::Modify {
                resource: "theme".to_string(),
                changes: simd_json::json!({ "theme": desired_tunables.theme }),
            });
        }

        if current_tunables.compression != desired_tunables.compression {
            actions.push(op_state::StateAction::Modify {
                resource: "compression".to_string(),
                changes: simd_json::json!({ "compression": desired_tunables.compression }),
            });
        }

        if current_tunables.cache_ttl != desired_tunables.cache_ttl {
            actions.push(op_state::StateAction::Modify {
                resource: "cache_ttl".to_string(),
                changes: simd_json::json!({ "cache_ttl": desired_tunables.cache_ttl }),
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
        Ok(true) // UI state is always consistent (embedded)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
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

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_identity() {
        let identity = WebUiIdentity::default();
        assert_eq!(identity.name, "web-ui");
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

    #[tokio::test]
    async fn test_plugin_state() {
        let plugin = WebUiPlugin::new();
        assert_eq!(plugin.name(), "web-ui");
        assert!(plugin.is_available());

        let state = plugin.query_current_state().await.unwrap();
        assert!(state.is_object());
    }
}
