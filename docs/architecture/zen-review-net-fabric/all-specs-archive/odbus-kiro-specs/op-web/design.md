# Operation D-Bus UI Design Document

**Version**: 1.0.0
**Date**: 2026-01-30
**Status**: DRAFT

---

## Architecture Overview

The UI is **embedded directly into the Rust binary** using `rust-embed`. No external static files - the entire frontend is compiled into the `op-web` binary for single-binary deployment.

### Embedded Architecture

```
crates/op-web/
├── src/
│   ├── lib.rs              # Module exports
│   ├── embedded_ui.rs      # NEW: rust-embed + axum handlers
│   ├── handlers/           # API handlers (existing)
│   ├── websocket.rs        # WebSocket handler (existing)
│   └── ...
├── ui/                     # NEW: React source (built at compile time)
│   ├── src/
│   │   ├── components/
│   │   ├── pages/
│   │   ├── hooks/
│   │   ├── stores/
│   │   ├── api/
│   │   ├── App.tsx
│   │   └── main.tsx
│   ├── dist/               # Build output (embedded via rust-embed)
│   ├── package.json
│   ├── vite.config.ts
│   └── index.html
├── build.rs                # Build script to compile UI before Rust
└── Cargo.toml              # rust-embed dependency
```

### Embedding Strategy

```rust
// crates/op-web/src/embedded_ui.rs
use rust_embed::RustEmbed;
use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};

#[derive(RustEmbed)]
#[folder = "ui/dist"]
#[prefix = ""]
struct UiAssets;

pub async fn serve_embedded_ui(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    
    // Try exact path first
    if let Some(content) = UiAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content.data.into_owned()))
            .unwrap();
    }
    
    // SPA fallback: serve index.html for client-side routing
    if let Some(content) = UiAssets::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(content.data.into_owned()))
            .unwrap();
    }
    
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}
```

### Build Process

```rust
// crates/op-web/build.rs
use std::process::Command;

fn main() {
    // Only rebuild UI if sources changed
    println!("cargo:rerun-if-changed=ui/src");
    println!("cargo:rerun-if-changed=ui/package.json");
    println!("cargo:rerun-if-changed=ui/vite.config.ts");
    
    // Build the UI
    let status = Command::new("npm")
        .args(["run", "build"])
        .current_dir("ui")
        .status()
        .expect("Failed to build UI");
    
    if !status.success() {
        panic!("UI build failed");
    }
}
```

### Router Integration

```rust
// crates/op-web/src/router.rs
use axum::{Router, routing::get};
use crate::embedded_ui::serve_embedded_ui;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // API routes (existing)
        .nest("/api", api_routes())
        .route("/ws", get(websocket_handler))
        .route("/mcp", post(mcp_handler))
        
        // Embedded UI - catch-all for SPA
        .fallback(serve_embedded_ui)
        
        .with_state(state)
}
```

### Technology Stack

| Layer | Technology | Rationale |
|-------|------------|-----------|
| Embedding | rust-embed | Compile-time asset embedding |
| Framework | React 18 + TypeScript | Industry standard, strong typing |
| Build | Vite | Fast builds, tree-shaking |
| State | Zustand + React Query | Lightweight, server-state focused |
| Styling | Tailwind CSS | Utility-first, small bundle |
| Charts | Recharts | React-native, performant |
| Virtualization | @tanstack/virtual | Handle 16k+ items |
| WebSocket | Native | Connect to /ws endpoint |
| gRPC | grpc-web + protobuf-ts | Native gRPC for internal comms |

### Benefits of Embedded UI

1. **Single Binary**: One executable, no external dependencies
2. **Atomic Deployment**: UI and backend always in sync
3. **No File System**: Works in containers, read-only systems
4. **Compression**: rust-embed can compress assets
5. **Cache Headers**: Easy to set immutable cache for hashed assets

---

## Plugin Architecture (Core of the System)

The UI is implemented as a **proper op-dbus plugin** following the schema-as-code pattern. This is critical - everything in op-dbus is a plugin, and the UI is no exception.

### Plugin 3-Section Pattern

All plugins follow this structure (see `net.rs`, `systemd.rs` for reference):

```rust
// crates/op-plugins/src/state_plugins/web_ui.rs

/// Web UI Plugin - serves embedded React SPA
/// 
/// SECTION 1: IMMUTABLE IDENTITY (set once, never changes)
/// - name: "web-ui"
/// - version: "1.0.0"  
/// - plugin_type: "ui"
/// - driver: "rust-embed"
/// 
/// SECTION 2: TUNABLE CONFIG (can change, snowball tracks all changes)
/// - enabled: bool
/// - port: u16
/// - cors_origins: Vec<String>
/// - compression: bool
/// - cache_ttl: u64
/// - theme: "dark" | "light"
/// - feature_flags: HashMap<String, bool>
/// 
/// SECTION 3: CAPABILITIES (what this plugin can do)
/// - can_serve_static: true
/// - can_proxy_api: true
/// - can_websocket: true
/// - supports_hot_reload: false (embedded)
/// - requires_root: false

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, PluginCapabilities, StateDiff, StatePlugin};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

/// SECTION 1: Immutable Identity Schema
/// Uses op-identity crate for WireGuard-based identity + OAuth token management
/// 
/// The WebUiPlugin integrates with op-identity::SessionManager for:
/// - WireGuard pubkey as user identity (zero passwords)
/// - OAuth token caching via org.freedesktop.secrets
/// - Session lifecycle management
use op_identity::{SessionManager, Session, WireGuardIdentity, GCloudAuth};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiPluginIdentity {
    pub name: &'static str,
    pub version: &'static str,
    pub plugin_type: &'static str,
    pub driver: &'static str,
}

impl Default for WebUiPluginIdentity {
    fn default() -> Self {
        Self {
            name: "web-ui",
            version: "1.0.0",
            plugin_type: "ui",
            driver: "rust-embed",
        }
    }
}

/// SECTION 2: Tunable Configuration Schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiTunables {
    pub enabled: bool,
    #[serde(default)]
    pub cors_origins: Vec<String>,
    pub compression: bool,
    pub cache_ttl: u64,
    pub theme: String,
    #[serde(default)]
    pub feature_flags: std::collections::HashMap<String, bool>,
    #[serde(default)]
    pub websocket: WebSocketConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebSocketConfig {
    pub enabled: bool,
    pub max_connections: u32,
    pub ping_interval_ms: u64,
    pub message_size_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiConfig {
    pub rate_limit_rps: u32,
    pub timeout_ms: u64,
    pub max_payload_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecurityConfig {
    pub require_auth: bool,
    pub session_ttl_seconds: u64,
    pub csrf_enabled: bool,
    pub csp_policy: String,
}

impl Default for WebUiTunables {
    fn default() -> Self {
        Self {
            enabled: true,
            cors_origins: vec!["*".to_string()],
            compression: true,
            cache_ttl: 86400,
            theme: "dark".to_string(),
            feature_flags: std::collections::HashMap::new(),
            websocket: WebSocketConfig {
                enabled: true,
                max_connections: 1000,
                ping_interval_ms: 30000,
                message_size_limit: 1024 * 1024,
            },
            api: ApiConfig {
                rate_limit_rps: 100,
                timeout_ms: 30000,
                max_payload_bytes: 10 * 1024 * 1024,
            },
            security: SecurityConfig {
                require_auth: true,
                session_ttl_seconds: 3600,
                csrf_enabled: true,
                csp_policy: "default-src 'self'".to_string(),
            },
        }
    }
}

/// SECTION 3: Capabilities
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
            supports_hot_reload: false,
            supports_compression: true,
            requires_root: false,
            supported_platforms: vec!["linux".to_string(), "macos".to_string()],
        }
    }
}

/// Embedded UI assets
#[derive(RustEmbed)]
#[folder = "ui/dist"]
#[prefix = ""]
pub struct UiAssets;

/// Web UI State Plugin
/// 
/// Integrates with op-identity for authentication:
/// - SessionManager handles WireGuard-based sessions
/// - GCloudAuth provides OAuth tokens for API calls
/// - Sessions are created on WireGuard peer connect, destroyed on disconnect
pub struct WebUiPlugin {
    identity: WebUiPluginIdentity,
    tunables: WebUiTunables,
    capabilities: WebUiCapabilities,
    /// Session manager from op-identity crate
    session_manager: Option<SessionManager>,
}

impl WebUiPlugin {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            identity: WebUiPluginIdentity::default(),
            tunables: WebUiTunables::default(),
            capabilities: WebUiCapabilities::default(),
            session_manager: SessionManager::new().ok(),
        })
    }

    /// Get current session (creates one if needed based on WireGuard identity)
    pub async fn get_session(&self) -> anyhow::Result<Session> {
        let sm = self.session_manager.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Session manager not initialized"))?;
        sm.get_or_create_session_from_wireguard().await
    }

    /// Get valid OAuth token for API calls
    pub async fn get_oauth_token(&self) -> anyhow::Result<String> {
        let sm = self.session_manager.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Session manager not initialized"))?;
        sm.get_valid_token().await
    }
}
```

### JSON Schema Definitions (Schema-as-Code)

Every plugin defines its schema in code. The UI plugin exposes these schemas for validation and introspection:

```rust
impl WebUiPlugin {
    /// JSON Schema for Identity (immutable)
    pub fn identity_schema() -> Value {
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

    /// JSON Schema for Tunables (mutable, snowball-tracked)
    pub fn tunables_schema() -> Value {
        simd_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://op-dbus.local/schemas/web-ui/tunables.json",
            "title": "WebUiTunables",
            "description": "Tunable configuration for Web UI plugin (changes tracked in snowball)",
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

    /// JSON Schema for Capabilities (read-only)
    pub fn capabilities_schema() -> Value {
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
```

### Plugin Metadata with Schemas

```rust
impl WebUiPlugin {
    fn metadata(&self) -> PluginMetadata {
        let mut object_schemas = std::collections::HashMap::new();
        object_schemas.insert("identity".to_string(), Self::identity_schema());
        object_schemas.insert("tunables".to_string(), Self::tunables_schema());
        object_schemas.insert("capabilities".to_string(), Self::capabilities_schema());

        PluginMetadata {
            name: self.identity.name.to_string(),
            version: self.identity.version.to_string(),
            description: "Embedded React SPA UI for op-dbus system management".to_string(),
            author: Some("op-dbus team".to_string()),
            license: Some("MIT".to_string()),
            dependencies: vec!["op-web".to_string(), "op-state".to_string()],
            dbus_services: vec![],
            object_schemas,
            feature_schemas: vec![
                FeatureSchema {
                    feature_type: "embedded-ui".to_string(),
                    version: "1.0.0".to_string(),
                    config: simd_json::json!({
                        "framework": "react",
                        "bundler": "vite",
                        "embedding": "rust-embed"
                    }),
                    tags: vec!["core".to_string(), "ui".to_string()],
                    immutable_paths: vec![
                        "/identity/name".to_string(),
                        "/identity/plugin_type".to_string(),
                    ],
                },
            ],
        }
    }
}
```

### StatePlugin Implementation

```rust
#[async_trait]
impl StatePlugin for WebUiPlugin {
    fn name(&self) -> &str {
        self.identity.name
    }

    fn version(&self) -> &str {
        self.identity.version
    }

    fn is_available(&self) -> bool {
        true // UI is always available (embedded)
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
        // ... more field comparisons

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
```

### D-Bus Interface for UI Plugin

The UI plugin exposes its state via D-Bus for consistency with other plugins:

```rust
// D-Bus interface: org.opdbus.plugins.WebUi
// Object path: /org/opdbus/plugins/web_ui

#[dbus_interface(name = "org.opdbus.plugins.WebUi")]
impl WebUiPlugin {
    /// Get current tunables as JSON
    async fn get_tunables(&self) -> String {
        simd_json::to_string(&self.tunables).unwrap_or_default()
    }

    /// Set tunables (validates against schema, tracks in snowball)
    async fn set_tunables(&mut self, tunables_json: &str) -> Result<(), zbus::fdo::Error> {
        let mut json = tunables_json.to_string();
        let tunables: WebUiTunables = unsafe {
            simd_json::from_str(&mut json)
                .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?
        };
        
        // Validate against schema before applying
        let value = simd_json::serde::to_owned_value(&tunables)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        self.validate_tunables(&value)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        
        self.tunables = tunables;
        Ok(())
    }

    /// Get JSON Schema for tunables
    fn get_tunables_schema(&self) -> String {
        simd_json::to_string(&Self::tunables_schema()).unwrap_or_default()
    }

    /// Get capabilities
    fn get_capabilities(&self) -> String {
        simd_json::to_string(&self.capabilities).unwrap_or_default()
    }
}
```

### Schema Validation

```rust
use jsonschema::JSONSchema;

impl WebUiPlugin {
    /// Validate tunables against schema before applying
    pub fn validate_tunables(&self, tunables: &Value) -> Result<()> {
        let schema = Self::tunables_schema();
        let schema_json: serde_json::Value = serde_json::from_str(
            &simd_json::to_string(&schema)?
        )?;
        
        let compiled = JSONSchema::compile(&schema_json)
            .map_err(|e| anyhow::anyhow!("Schema compilation failed: {}", e))?;
        
        let tunables_json: serde_json::Value = serde_json::from_str(
            &simd_json::to_string(tunables)?
        )?;
        
        compiled.validate(&tunables_json)
            .map_err(|errors| {
                let msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
                anyhow::anyhow!("Validation failed: {}", msgs.join(", "))
            })?;
        
        Ok(())
    }

    /// Check if a path is immutable
    pub fn is_path_immutable(&self, path: &str) -> bool {
        let immutable_paths = vec![
            "/identity",
            "/identity/name",
            "/identity/plugin_type",
            "/identity/driver",
        ];
        immutable_paths.iter().any(|p| path.starts_with(p))
    }
}
```

### Plugin Registration

```rust
// In op-plugins/src/default_registry.rs
pub fn register_default_plugins(registry: &mut PluginRegistry) {
    registry.register(Box::new(SystemdStatePlugin::new()));
    registry.register(Box::new(NetStatePlugin::new()));
    registry.register(Box::new(WebUiPlugin::new())); // NEW
}
```

### Why Plugin Architecture Matters

1. **Schema-as-Code**: JSON Schemas defined in Rust code, not external files
2. **Snowball Tracking**: All tunable changes tracked in audit ledger
3. **Consistent Interface**: Same `StatePlugin` trait as systemd, net, etc.
4. **Runtime Reconfiguration**: Tunables can be changed without restart
5. **Capability Discovery**: Other plugins can query what UI can do
6. **Validation**: All config changes validated against schema before apply
7. **Immutability Enforcement**: Identity fields cannot be changed after creation
8. **D-Bus First**: Plugin state accessible via D-Bus like all other plugins

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     operation-dbus-ui (React SPA)                   │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐ │
│  │   Pages     │  │ Components  │  │   Hooks     │  │   Stores   │ │
│  ├─────────────┤  ├─────────────┤  ├─────────────┤  ├────────────┤ │
│  │ Dashboard   │  │ VirtualList │  │ useApi      │  │ authStore  │ │
│  │ Agents      │  │ VirtualTree │  │ useWebSocket│  │ quotaStore │ │
│  │ Tools       │  │ ChatPanel   │  │ useQuota    │  │ uiStore    │ │
│  │ Workflows   │  │ DAGCanvas   │  │ useGrpc     │  │            │ │
│  │ DBusBrowser │  │ MetricChart │  │             │  │            │ │
│  │ MCPs        │  │ RBACGate    │  │             │  │            │ │
│  │ Snowball  │  │ QuotaMeter  │  │             │  │            │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘ │
├─────────────────────────────────────────────────────────────────────┤
│                        API Layer                                     │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
│  │   gRPC-Web       │  │   WebSocket      │  │   REST (compat)  │  │
│  │   (primary)      │  │   (streaming)    │  │   (fallback)     │  │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     op-http (Backend)                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐ │
│  │ /api/grpc/* │  │ /ws/live    │  │ /api/v1/*   │  │ Static     │ │
│  │ gRPC-Web    │  │ WebSocket   │  │ REST        │  │ Files      │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Component Architecture

### Page Components

```
src/pages/
├── Dashboard.tsx          # System overview with chat
├── Agents/
│   ├── AgentCatalog.tsx   # Browse 70+ agents
│   ├── AgentDetail.tsx    # Agent info and execution
│   └── AgentStatus.tsx    # Running agent instances
├── Tools/
│   ├── ToolCatalog.tsx    # Browse 16k+ tools
│   ├── ToolDetail.tsx     # Tool schema and execution
│   └── ToolExecution.tsx  # Execute with form
├── DBus/
│   ├── ServiceBrowser.tsx # D-Bus service tree
│   ├── ObjectDetail.tsx   # Object inspector
│   └── MethodInvoke.tsx   # Method execution
├── Workflows/
│   ├── WorkflowList.tsx   # Saved workflows
│   ├── WorkflowBuilder.tsx# DAG editor
│   └── WorkflowRun.tsx    # Execution monitor
├── MCPs/
│   ├── McpList.tsx        # MCP server list
│   ├── McpDetail.tsx      # MCP dashboard
│   └── McpPolicies.tsx    # Policy editor
├── Snowball/
│   ├── AuditTrail.tsx     # Event browser
│   └── EventChain.tsx     # Chain visualization
├── State/
│   ├── StateDiff.tsx      # Diff viewer
│   └── PluginState.tsx    # Plugin state browser
├── Network/
│   ├── Topology.tsx       # Network diagram
│   └── OpenFlow.tsx       # Flow rules
├── Execution/
│   ├── Timeline.tsx       # Execution timeline
│   └── Metrics.tsx        # Performance metrics
└── Settings/
    ├── Profile.tsx        # User profile
    └── Quotas.tsx         # Quota management
```

### Core Components

```
src/components/
├── layout/
│   ├── AppShell.tsx       # Main layout with sidebar
│   ├── Sidebar.tsx        # Navigation sidebar
│   ├── Header.tsx         # Top bar with search, quota
│   └── Breadcrumb.tsx     # Navigation breadcrumb
├── data/
│   ├── VirtualList.tsx    # Virtualized list (16k items)
│   ├── VirtualTree.tsx    # Virtualized tree
│   ├── DataTable.tsx      # Sortable, filterable table
│   └── Pagination.tsx     # Cursor-based pagination
├── chat/
│   ├── ChatPanel.tsx      # Chat interface
│   ├── ChatMessage.tsx    # Message bubble
│   ├── ChatInput.tsx      # Input with suggestions
│   └── ChatActions.tsx    # Suggested action buttons
├── workflow/
│   ├── DAGCanvas.tsx      # Workflow canvas
│   ├── NodePalette.tsx    # Draggable nodes
│   ├── NodeEditor.tsx     # Node configuration
│   └── ConnectionLine.tsx # Edge rendering
├── visualization/
│   ├── MetricChart.tsx    # Time series chart
│   ├── NetworkGraph.tsx   # Network topology
│   ├── Timeline.tsx       # Gantt-style timeline
│   └── TreeMap.tsx        # Hierarchical data
├── forms/
│   ├── DynamicForm.tsx    # JSON Schema form
│   ├── FilterBar.tsx      # Search and filters
│   ├── TimeRangeSelector.tsx
│   └── SearchInput.tsx    # Global search
├── feedback/
│   ├── LoadingSpinner.tsx
│   ├── ErrorBoundary.tsx
│   ├── Toast.tsx          # Notifications
│   └── ConfirmModal.tsx   # Action confirmation
├── security/
│   ├── RBACGate.tsx       # Role-based visibility
│   ├── QuotaMeter.tsx     # Quota usage display
│   ├── QuotaCostBadge.tsx # Action cost indicator
│   └── SnowballComment.tsx # Audit comment input
└── payload/
    ├── PayloadViewer.tsx  # JSON/binary viewer
    ├── PayloadModal.tsx   # Full payload with unmask
    └── WasmDecoder.tsx    # WASM decoder wrapper
```

---

## State Management

### Zustand Stores

```typescript
// src/stores/authStore.ts
interface AuthState {
  user: User | null;
  token: string | null;
  roles: string[];
  isAuthenticated: boolean;
  login: (credentials: Credentials) => Promise<void>;
  logout: () => void;
  hasRole: (role: string) => boolean;
}

// src/stores/quotaStore.ts
interface QuotaState {
  quotas: QuotaInfo;
  usage: QuotaUsage;
  warnings: QuotaWarning[];
  fetchQuotas: () => Promise<void>;
  checkCost: (action: string) => number;
}

// src/stores/uiStore.ts
interface UIState {
  sidebarCollapsed: boolean;
  theme: 'dark' | 'light';
  recentSearches: string[];
  toggleSidebar: () => void;
  addRecentSearch: (query: string) => void;
}
```

### React Query for Server State

```typescript
// src/hooks/useApi.ts
export function useAgents(options?: QueryOptions) {
  return useQuery({
    queryKey: ['agents'],
    queryFn: () => grpcClient.listAgents(),
    staleTime: 30_000,
  });
}

export function useTools(query: string, cursor?: string) {
  return useInfiniteQuery({
    queryKey: ['tools', query],
    queryFn: ({ pageParam }) => 
      grpcClient.searchTools({ query, cursor: pageParam, limit: 25 }),
    getNextPageParam: (lastPage) => lastPage.nextCursor,
  });
}

export function useDBusObjects(service: string, path: string) {
  return useQuery({
    queryKey: ['dbus', service, path],
    queryFn: () => grpcClient.introspect({ service, path }),
  });
}
```

---

## API Integration

### gRPC-Web Client (Primary)

```typescript
// src/api/grpc/client.ts
import { GrpcWebFetchTransport } from '@protobuf-ts/grpc-web-transport';
import { AgentsServiceClient } from './generated/agents.client';
import { ToolsServiceClient } from './generated/tools.client';
import { IntrospectionServiceClient } from './generated/introspection.client';

const transport = new GrpcWebFetchTransport({
  baseUrl: import.meta.env.VITE_GRPC_URL || '/api/grpc',
});

export const agentsClient = new AgentsServiceClient(transport);
export const toolsClient = new ToolsServiceClient(transport);
export const introspectionClient = new IntrospectionServiceClient(transport);
```

### WebSocket Client (Streaming)

```typescript
// src/api/websocket/client.ts
interface Subscription {
  id: string;
  channel: string;
  filters?: Record<string, unknown>;
  sampleRate?: number;
}

class WebSocketClient {
  private ws: WebSocket | null = null;
  private subscriptions = new Map<string, Subscription>();
  private handlers = new Map<string, (event: unknown) => void>();
  private reconnectAttempts = 0;

  connect(url: string, token: string): void {
    this.ws = new WebSocket(`${url}?token=${token}`);
    this.ws.onmessage = this.handleMessage.bind(this);
    this.ws.onclose = this.handleClose.bind(this);
  }

  subscribe(channel: string, options: SubscribeOptions): string {
    const id = crypto.randomUUID();
    this.ws?.send(JSON.stringify({
      type: 'subscribe',
      channel,
      subscriptionId: id,
      sample: { rate: options.sampleRate || 0.001 },
      filters: options.filters,
    }));
    return id;
  }

  unsubscribe(subscriptionId: string): void {
    this.ws?.send(JSON.stringify({
      type: 'unsubscribe',
      subscriptionId,
    }));
    this.subscriptions.delete(subscriptionId);
  }
}
```

### REST Client (Fallback)

```typescript
// src/api/rest/client.ts
const api = {
  async get<T>(path: string, params?: Record<string, string>): Promise<T> {
    const url = new URL(path, import.meta.env.VITE_API_URL);
    if (params) {
      Object.entries(params).forEach(([k, v]) => url.searchParams.set(k, v));
    }
    const res = await fetch(url, {
      headers: { Authorization: `Bearer ${getToken()}` },
    });
    if (!res.ok) throw new ApiError(res);
    return res.json();
  },

  async post<T>(path: string, body: unknown): Promise<T> {
    const res = await fetch(`${import.meta.env.VITE_API_URL}${path}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${getToken()}`,
      },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new ApiError(res);
    return res.json();
  },
};
```

---

## Key Component Designs

### VirtualList (16k+ items)

```typescript
// src/components/data/VirtualList.tsx
interface VirtualListProps<T> {
  items: T[];
  itemHeight: number;
  renderItem: (item: T, index: number) => ReactNode;
  onLoadMore?: () => void;
  hasMore?: boolean;
}

function VirtualList<T>({ items, itemHeight, renderItem, onLoadMore, hasMore }: VirtualListProps<T>) {
  const parentRef = useRef<HTMLDivElement>(null);
  
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => itemHeight,
    overscan: 10,
  });

  // Infinite scroll trigger
  useEffect(() => {
    const lastItem = virtualizer.getVirtualItems().at(-1);
    if (lastItem && lastItem.index >= items.length - 5 && hasMore && onLoadMore) {
      onLoadMore();
    }
  }, [virtualizer.getVirtualItems()]);

  return (
    <div ref={parentRef} className="h-full overflow-auto">
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
        {virtualizer.getVirtualItems().map((virtualRow) => (
          <div
            key={virtualRow.key}
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              width: '100%',
              height: virtualRow.size,
              transform: `translateY(${virtualRow.start}px)`,
            }}
          >
            {renderItem(items[virtualRow.index], virtualRow.index)}
          </div>
        ))}
      </div>
    </div>
  );
}
```

### DAGCanvas (Workflow Builder)

```typescript
// src/components/workflow/DAGCanvas.tsx
interface DAGCanvasProps {
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  onNodeAdd: (node: WorkflowNode) => void;
  onNodeRemove: (nodeId: string) => void;
  onEdgeAdd: (edge: WorkflowEdge) => void;
  onNodeSelect: (nodeId: string) => void;
  executionState?: ExecutionState;
}

function DAGCanvas({ nodes, edges, onNodeAdd, executionState }: DAGCanvasProps) {
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const canvasRef = useRef<HTMLDivElement>(null);

  // Layout nodes using dagre
  const layout = useMemo(() => {
    const g = new dagre.graphlib.Graph();
    g.setGraph({ rankdir: 'LR', nodesep: 50, ranksep: 100 });
    g.setDefaultEdgeLabel(() => ({}));
    
    nodes.forEach(n => g.setNode(n.id, { width: 200, height: 80 }));
    edges.forEach(e => g.setEdge(e.source, e.target));
    
    dagre.layout(g);
    return g;
  }, [nodes, edges]);

  return (
    <div ref={canvasRef} className="relative w-full h-full bg-zinc-900 overflow-hidden">
      <div
        style={{
          transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
          transformOrigin: '0 0',
        }}
      >
        {/* Render edges */}
        <svg className="absolute inset-0 pointer-events-none">
          {edges.map(edge => (
            <ConnectionLine
              key={`${edge.source}-${edge.target}`}
              from={layout.node(edge.source)}
              to={layout.node(edge.target)}
            />
          ))}
        </svg>
        
        {/* Render nodes */}
        {nodes.map(node => {
          const pos = layout.node(node.id);
          const status = executionState?.nodeStatus[node.id];
          return (
            <WorkflowNodeCard
              key={node.id}
              node={node}
              position={pos}
              status={status}
              onSelect={() => onNodeSelect(node.id)}
            />
          );
        })}
      </div>
      
      {/* Zoom controls */}
      <div className="absolute bottom-4 right-4 flex gap-2">
        <button onClick={() => setZoom(z => Math.min(z * 1.2, 2))}>+</button>
        <button onClick={() => setZoom(z => Math.max(z / 1.2, 0.5))}>-</button>
        <button onClick={() => { setZoom(1); setPan({ x: 0, y: 0 }); }}>Reset</button>
      </div>
    </div>
  );
}
```

### RBACGate (Security)

```typescript
// src/components/security/RBACGate.tsx
interface RBACGateProps {
  requiredRoles: string[];
  children: ReactNode;
  fallback?: ReactNode;
  requireAll?: boolean;
}

function RBACGate({ requiredRoles, children, fallback, requireAll = false }: RBACGateProps) {
  const { roles, isAuthenticated } = useAuthStore();
  
  if (!isAuthenticated) {
    return fallback ?? null;
  }
  
  const hasAccess = requireAll
    ? requiredRoles.every(r => roles.includes(r))
    : requiredRoles.some(r => roles.includes(r));
  
  if (!hasAccess) {
    return fallback ?? null;
  }
  
  return <>{children}</>;
}

// Usage
<RBACGate requiredRoles={['admin', 'operator']}>
  <DangerousButton onClick={handleDelete} />
</RBACGate>
```

### PayloadViewer with WASM Decoder

```typescript
// src/components/payload/PayloadViewer.tsx
interface PayloadViewerProps {
  payload: string; // base64 encoded
  masked?: boolean;
  onUnmask?: () => void;
}

function PayloadViewer({ payload, masked, onUnmask }: PayloadViewerProps) {
  const [decoded, setDecoded] = useState<string | null>(null);
  const [decodeError, setDecodeError] = useState<string | null>(null);
  
  useEffect(() => {
    if (masked) return;
    
    (async () => {
      try {
        // Try WASM decoder first
        const wasm = await import('@/wasm/decoder');
        const result = wasm.decodePayload(payload);
        setDecoded(result);
      } catch {
        // Fallback to pure TS decoder
        try {
          const bytes = atob(payload);
          const text = new TextDecoder().decode(
            Uint8Array.from(bytes, c => c.charCodeAt(0))
          );
          setDecoded(JSON.stringify(JSON.parse(text), null, 2));
        } catch (e) {
          setDecodeError(`Decode failed: ${e}`);
        }
      }
    })();
  }, [payload, masked]);

  if (masked) {
    return (
      <div className="bg-zinc-800 p-4 rounded">
        <p className="text-zinc-400 mb-2">
          Payload redacted. Request unmask (admins only). 
          This action will be snowballed.
        </p>
        <RBACGate requiredRoles={['admin']}>
          <button onClick={onUnmask} className="btn-primary">
            Unmask Payload
          </button>
        </RBACGate>
      </div>
    );
  }

  if (decodeError) {
    return <div className="text-red-400">{decodeError}</div>;
  }

  return (
    <pre className="bg-zinc-900 p-4 rounded overflow-auto text-sm">
      <code>{decoded}</code>
    </pre>
  );
}
```

---

## WASM Decoder Module

```rust
// wasm/decoder/src/lib.rs
use wasm_bindgen::prelude::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[wasm_bindgen]
pub fn decode_payload(base64_input: &str) -> Result<String, JsValue> {
    // Decode base64
    let bytes = STANDARD.decode(base64_input)
        .map_err(|e| JsValue::from_str(&format!("Base64 decode error: {}", e)))?;
    
    // Convert to UTF-8 string
    let text = String::from_utf8(bytes)
        .map_err(|e| JsValue::from_str(&format!("UTF-8 decode error: {}", e)))?;
    
    // Parse and pretty-print JSON
    let value: simd_json::OwnedValue = unsafe {
        simd_json::from_str(&mut text.clone())
            .map_err(|e| JsValue::from_str(&format!("JSON parse error: {}", e)))?
    };
    
    Ok(simd_json::to_string_pretty(&value)
        .map_err(|e| JsValue::from_str(&format!("JSON serialize error: {}", e)))?)
}
```

---

## Routing Structure

```typescript
// src/routes.tsx
const routes = [
  { path: '/', element: <Dashboard /> },
  { path: '/agents', element: <AgentCatalog /> },
  { path: '/agents/:agentType', element: <AgentDetail /> },
  { path: '/tools', element: <ToolCatalog /> },
  { path: '/tools/:toolName', element: <ToolDetail /> },
  { path: '/dbus', element: <ServiceBrowser /> },
  { path: '/dbus/:service/*', element: <ObjectDetail /> },
  { path: '/workflows', element: <WorkflowList /> },
  { path: '/workflows/new', element: <WorkflowBuilder /> },
  { path: '/workflows/:id', element: <WorkflowRun /> },
  { path: '/mcps', element: <McpList /> },
  { path: '/mcps/:mcpId', element: <McpDetail /> },
  { path: '/snowball', element: <AuditTrail /> },
  { path: '/state', element: <StateDiff /> },
  { path: '/network', element: <Topology /> },
  { path: '/execution', element: <Timeline /> },
  { path: '/settings', element: <Settings /> },
];
```

---

## Correctness Properties

### P1: Virtual List Rendering
**Property**: For any list of N items where N > 1000, only visible items + overscan are rendered in DOM.
**Test**: Generate 16,000 items, verify DOM node count < 100.

### P2: WebSocket Reconnection
**Property**: WebSocket client reconnects with exponential backoff on disconnect.
**Test**: Simulate disconnect, verify reconnection attempts with increasing delays.

### P3: RBAC Enforcement
**Property**: UI elements gated by RBACGate are not rendered for unauthorized users.
**Test**: For each protected element, verify visibility matches user roles.

### P4: Quota Enforcement
**Property**: Actions exceeding quota are blocked with appropriate error message.
**Test**: Set quota to 0, attempt action, verify block and error display.

### P5: Snowball Logging
**Property**: All audited actions create snowball entries before execution.
**Test**: Execute audited action, verify snowball entry exists with correct data.

### P6: Cursor Pagination
**Property**: Pagination maintains consistency across page loads.
**Test**: Load page 1, load page 2, verify no duplicates or gaps.

---

## Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Initial Load | < 2s | Lighthouse FCP |
| Tool Search | < 200ms | Time to first result |
| Virtual List Scroll | 60fps | Chrome DevTools |
| WebSocket Latency | < 50ms | Round-trip time |
| Memory (16k items) | < 100MB | Chrome Task Manager |

---

## Security Considerations

1. **Token Storage**: Use httpOnly cookies for refresh tokens, memory for access tokens
2. **XSS Prevention**: React's default escaping + CSP headers
3. **CSRF Protection**: SameSite cookies + custom headers
4. **Input Validation**: JSON Schema validation before submission
5. **Audit Trail**: All mutations logged to snowball with user context

---

## File Structure

```
crates/op-web/
├── src/                          # Rust backend
│   ├── lib.rs
│   ├── main.rs
│   ├── router.rs                 # Axum router with embedded UI fallback
│   ├── embedded_ui.rs            # NEW: rust-embed handler
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── agents.rs
│   │   ├── chat.rs
│   │   ├── tools.rs
│   │   ├── health.rs
│   │   └── ...
│   ├── websocket.rs
│   ├── state.rs
│   └── ...
├── ui/                           # NEW: React frontend (embedded at compile)
│   ├── src/
│   │   ├── api/
│   │   │   ├── grpc/
│   │   │   │   ├── client.ts
│   │   │   │   └── generated/    # protobuf-ts output
│   │   │   ├── websocket.ts
│   │   │   └── rest.ts
│   │   ├── components/
│   │   │   ├── layout/
│   │   │   │   ├── AppShell.tsx
│   │   │   │   ├── Sidebar.tsx
│   │   │   │   └── Header.tsx
│   │   │   ├── data/
│   │   │   │   ├── VirtualList.tsx
│   │   │   │   ├── VirtualTree.tsx
│   │   │   │   └── DataTable.tsx
│   │   │   ├── chat/
│   │   │   │   ├── ChatPanel.tsx
│   │   │   │   ├── ChatMessage.tsx
│   │   │   │   └── ChatInput.tsx
│   │   │   ├── workflow/
│   │   │   │   ├── DAGCanvas.tsx
│   │   │   │   ├── NodePalette.tsx
│   │   │   │   └── NodeEditor.tsx
│   │   │   ├── visualization/
│   │   │   │   ├── MetricChart.tsx
│   │   │   │   └── Timeline.tsx
│   │   │   ├── forms/
│   │   │   │   ├── DynamicForm.tsx
│   │   │   │   └── FilterBar.tsx
│   │   │   ├── security/
│   │   │   │   ├── RBACGate.tsx
│   │   │   │   ├── QuotaMeter.tsx
│   │   │   │   └── QuotaCostBadge.tsx
│   │   │   └── payload/
│   │   │       ├── PayloadViewer.tsx
│   │   │       └── WasmDecoder.tsx
│   │   ├── hooks/
│   │   │   ├── useApi.ts
│   │   │   ├── useWebSocket.ts
│   │   │   ├── useQuota.ts
│   │   │   └── useGrpc.ts
│   │   ├── pages/
│   │   │   ├── Dashboard.tsx
│   │   │   ├── Agents/
│   │   │   ├── Tools/
│   │   │   ├── DBus/
│   │   │   ├── Workflows/
│   │   │   ├── MCPs/
│   │   │   ├── Snowball/
│   │   │   ├── State/
│   │   │   ├── Network/
│   │   │   ├── Execution/
│   │   │   └── Settings/
│   │   ├── stores/
│   │   │   ├── authStore.ts
│   │   │   ├── quotaStore.ts
│   │   │   └── uiStore.ts
│   │   ├── types/
│   │   │   └── index.ts
│   │   ├── App.tsx
│   │   ├── routes.tsx
│   │   └── main.tsx
│   ├── dist/                     # Build output (embedded by rust-embed)
│   ├── wasm/
│   │   └── decoder/
│   │       ├── Cargo.toml
│   │       └── src/lib.rs
│   ├── index.html
│   ├── package.json
│   ├── tsconfig.json
│   ├── vite.config.ts
│   └── tailwind.config.js
├── build.rs                      # NEW: Build UI before Rust compilation
├── Cargo.toml                    # Add rust-embed dependency
└── README.md
```

### Cargo.toml Additions

```toml
[dependencies]
rust-embed = { version = "8", features = ["compression"] }
mime_guess = "2"

[build-dependencies]
# None needed - build.rs uses std::process::Command
```

### Vite Config for Embedded Build

```typescript
// crates/op-web/ui/vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  base: '/',  // Served from root
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    // Generate hashed filenames for cache busting
    rollupOptions: {
      output: {
        entryFileNames: 'assets/[name]-[hash].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash].[ext]',
      },
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    // Dev server proxies to Rust backend
    proxy: {
      '/api': 'http://localhost:8080',
      '/ws': { target: 'ws://localhost:8080', ws: true },
      '/mcp': 'http://localhost:8080',
    },
  },
});
```