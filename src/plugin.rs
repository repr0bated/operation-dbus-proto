//! Plugin system for OP-DBUS
//!
//! Provides plugin trait, registry, and state management.
//! Ported from op-dbus-p1/crates/op-plugins.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use sha2::{Digest, Sha256};
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::Result;

// ============================================================================
// MIRROR STATE
// ============================================================================

/// Mirror state configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorState {
    /// The target state configuration
    pub state: Value,
    /// When this mirror state was set
    pub timestamp: DateTime<Utc>,
    /// Hash of the state for verification
    pub hash: String,
    /// Optional description of the change
    pub description: Option<String>,
    /// Source of the mirror state (user, auto, import, etc.)
    pub source: StateSource,
}

impl MirrorState {
    /// Create a new mirror state
    pub fn new(state: Value) -> Self {
        let hash = Self::compute_hash(&state);
        Self {
            state,
            timestamp: Utc::now(),
            hash,
            description: None,
            source: StateSource::User,
        }
    }

    /// Create with description
    pub fn with_description(state: Value, description: impl Into<String>) -> Self {
        let mut ms = Self::new(state);
        ms.description = Some(description.into());
        ms
    }

    /// Compute hash of the state
    pub fn compute_hash(state: &Value) -> String {
        let mut hasher = Sha256::new();
        hasher.update(simd_json::to_string(state).unwrap_or_default().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Verify the hash matches
    pub fn verify(&self) -> bool {
        Self::compute_hash(&self.state) == self.hash
    }
}

impl Default for MirrorState {
    fn default() -> Self {
        Self::new(Value::Object(Box::new(simd_json::value::owned::Object::new())))
    }
}

/// Source of the projected state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StateSource {
    User,
    AutoDiscovered,
    Import(String),
    Plugin(String),
    Default,
}

// ============================================================================
// STATE CHANGE
// ============================================================================

/// Represents a change to be applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub operation: ChangeOperation,
    pub path: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub description: String,
    pub hash: String,
    pub timestamp: DateTime<Utc>,
}

impl StateChange {
    pub fn new(
        operation: ChangeOperation,
        path: impl Into<String>,
        old_value: Option<Value>,
        new_value: Option<Value>,
        description: impl Into<String>,
    ) -> Self {
        let path = path.into();
        let description = description.into();

        let mut hasher = Sha256::new();
        hasher.update(format!("{:?}", operation).as_bytes());
        hasher.update(path.as_bytes());
        hasher.update(simd_json::to_string(&old_value).unwrap_or_default().as_bytes());
        hasher.update(simd_json::to_string(&new_value).unwrap_or_default().as_bytes());
        let hash = hex::encode(hasher.finalize());

        Self {
            operation,
            path,
            old_value,
            new_value,
            description,
            hash,
            timestamp: Utc::now(),
        }
    }

    pub fn create(path: impl Into<String>, value: Value, description: impl Into<String>) -> Self {
        Self::new(ChangeOperation::Create, path, None, Some(value), description)
    }

    pub fn update(path: impl Into<String>, old: Value, new: Value, description: impl Into<String>) -> Self {
        Self::new(ChangeOperation::Update, path, Some(old), Some(new), description)
    }

    pub fn delete(path: impl Into<String>, old: Value, description: impl Into<String>) -> Self {
        Self::new(ChangeOperation::Delete, path, Some(old), None, description)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOperation {
    Create,
    Update,
    Delete,
    NoOp,
}

// ============================================================================
// VALIDATION
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
            suggestions: vec![],
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            valid: false,
            errors: vec![ValidationError {
                path: String::new(),
                message: error.into(),
                code: "validation_failed".to_string(),
            }],
            warnings: vec![],
            suggestions: vec![],
        }
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    pub fn with_error(mut self, path: impl Into<String>, message: impl Into<String>) -> Self {
        self.valid = false;
        self.errors.push(ValidationError {
            path: path.into(),
            message: message.into(),
            code: "validation_error".to_string(),
        });
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
    pub code: String,
}

// ============================================================================
// PLUGIN CONTEXT
// ============================================================================

#[derive(Debug, Clone)]
pub struct PluginContext {
    pub storage_path: PathBuf,
    pub numa_node: Option<u32>,
    pub config: Value,
}

impl Default for PluginContext {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from("/var/lib/op-dbus/plugins/default"),
            numa_node: None,
            config: simd_json::json!(null),
        }
    }
}

// ============================================================================
// OBJECT SCHEMA REFERENCE (for D-Bus schema linking)
// ============================================================================

/// Reference to a D-Bus object schema stored in StateStore
/// 
/// Used to link plugins to their discovered D-Bus interfaces.
/// These schemas are persisted and restored during disaster recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSchemaRef {
    /// Object type (e.g., "dbus_interface", "dbus_service")
    pub object_type: String,
    /// Namespace - typically D-Bus service name (e.g., "org.freedesktop.NetworkManager")
    pub namespace: String,
    /// D-Bus object path (e.g., "/org/freedesktop/NetworkManager")
    pub path: String,
    /// Hash of the interface schema for integrity verification
    pub schema_hash: String,
}

impl ObjectSchemaRef {
    pub fn new(object_type: impl Into<String>, namespace: impl Into<String>, path: impl Into<String>, schema_hash: impl Into<String>) -> Self {
        Self {
            object_type: object_type.into(),
            namespace: namespace.into(),
            path: path.into(),
            schema_hash: schema_hash.into(),
        }
    }
}


// ============================================================================
// FEATURE SCHEMAS (Extensible Capabilities)
// ============================================================================

/// Schema for an optional feature capability implemented by the plugin
/// e.g., "compliance/v1", "gpu_inference/v1", "iso_builder/v1"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSchema {
    /// Feature type identifier (e.g., "compliance")
    pub feature_type: String,
    /// Feature version (e.g., "v1")
    pub version: String,
    /// Schema-specific configuration or definition
    pub config: Value,
    /// Capability tags (e.g. "immutable", "core", "optional")
    #[serde(default)]
    pub tags: Vec<String>,
    /// Specific JSON configuration paths that are immutable (e.g. ["/metadata/id"])
    #[serde(default)]
    pub immutable_paths: Vec<String>,
}

impl FeatureSchema {
    pub fn new(feature_type: impl Into<String>, version: impl Into<String>, config: Value) -> Self {
        Self {
            feature_type: feature_type.into(),
            version: version.into(),
            config,
            tags: Vec::new(),
            immutable_paths: Vec::new(),
        }
    }

    /// Check if this feature is fully immutable
    pub fn is_fully_immutable(&self) -> bool {
        self.tags.iter().any(|t| t == "immutable")
    }

    /// Check if a specific config path is immutable
    pub fn is_path_immutable(&self, path: &str) -> bool {
        self.is_fully_immutable() || self.immutable_paths.iter().any(|p| p == path)
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
    
    /// Mark a path as immutable
    pub fn with_immutable_path(mut self, path: impl Into<String>) -> Self {
        self.immutable_paths.push(path.into());
        self
    }
}

// ============================================================================
// PLUGIN CORE (Compatibility with existing codebase)
// ============================================================================

/// Core plugin definition - immutable after registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCore {
    pub name: String,
    pub version: String,
    pub description: String,
    pub tool_list: Vec<String>,
    pub dependencies: Vec<String>,
    /// Linked D-Bus object schemas (for disaster recovery)
    #[serde(default)]
    pub object_schemas: Vec<ObjectSchemaRef>,
    /// Keys in BTRFS state_subvol that should be backed up for disaster recovery
    /// These are state keys managed by this plugin that can't be re-discovered from D-Bus
    #[serde(default)]
    pub restorable_state_keys: Vec<String>,
    /// Extensible feature schemas implemented by this plugin
    #[serde(default)]
    pub feature_schemas: Vec<FeatureSchema>,
    pub author: Option<String>,
    pub license: Option<String>,
}

impl Default for PluginCore {
    fn default() -> Self {
        Self {
            name: "unknown".to_string(),
            version: "0.0.0".to_string(),
            description: "No description".to_string(),
            tool_list: Vec::new(),
            dependencies: Vec::new(),
            object_schemas: Vec::new(),
            restorable_state_keys: Vec::new(),
            feature_schemas: Vec::new(),
            author: None,
            license: None,
        }
    }
}

impl PluginCore {
    /// Compute hash of core plugin definition
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.name.as_bytes());
        hasher.update(self.version.as_bytes());
        for tool in &self.tool_list {
            hasher.update(tool.as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

// ============================================================================
// PLUGIN TUNABLES
// ============================================================================

/// Plugin tunables - can be modified without changing core identity
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginTunables {
    pub enabled: bool,
    pub priority: i32,
    pub settings: HashMap<String, Value>,
    pub rate_limit: Option<u32>,
    pub timeout_ms: Option<u64>,
}

impl PluginTunables {
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(simd_json::to_string(&self.settings).unwrap_or_default().as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Scope for tunables
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TunableScope {
    Global,
    Session(u64),
    User(u64),
}

// ============================================================================
// EFFECTIVE PLUGIN
// ============================================================================

/// Resolved plugin = core + tunables for a specific scope
#[derive(Debug, Clone)]
pub struct EffectivePlugin {
    pub core: Arc<PluginCore>,
    pub tunables: PluginTunables,
    pub core_hash: String,
    pub tunable_hash: String,
}

impl EffectivePlugin {
    pub fn new(core: Arc<PluginCore>, tunables: PluginTunables) -> Self {
        let core_hash = core.compute_hash();
        let tunable_hash = tunables.compute_hash();
        Self {
            core,
            tunables,
            core_hash,
            tunable_hash,
        }
    }
}

impl Default for EffectivePlugin {
    fn default() -> Self {
        let core = Arc::new(PluginCore::default());
        let tunables = PluginTunables::default();
        Self::new(core, tunables)
    }
}

// ============================================================================
// PLUGIN REGISTRY
// ============================================================================

/// Legacy in-process plugin registry.
///
/// This type predates the plugin-catalog refactor and should not be treated as
/// the authoritative runtime source of plugin/schema truth. It is kept only
/// for older root-crate code paths that still compose `PluginCore` plus
/// tunables locally. New runtime authority lives in `op_plugins::PluginCatalog`
/// backed by canonical persisted plugin documents.
pub struct PluginRegistry {
    cores: DashMap<String, Arc<PluginCore>>,
    tunables: DashMap<(String, TunableScope), PluginTunables>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            cores: DashMap::new(),
            tunables: DashMap::new(),
        }
    }

    /// Register a plugin core
    pub fn register_core(&self, plugin: PluginCore) {
        self.cores.insert(plugin.name.clone(), Arc::new(plugin));
    }

    /// Register tunables for a plugin
    pub fn register_tunables(&self, name: &str, scope: TunableScope, tunables: PluginTunables) {
        self.tunables.insert((name.to_string(), scope), tunables);
    }

    /// Get core plugin by name
    pub fn get_core(&self, name: &str) -> Option<Arc<PluginCore>> {
        self.cores.get(name).map(|p| Arc::clone(&p))
    }

    /// Get tunables for a plugin/scope
    pub fn get_tunables(&self, name: &str, scope: TunableScope) -> Option<PluginTunables> {
        self.tunables
            .get(&(name.to_string(), scope))
            .map(|t| t.clone())
    }

    /// Resolve effective plugin for a scope
    pub fn resolve(&self, name: &str, scope: TunableScope) -> Option<EffectivePlugin> {
        let core = self.get_core(name)?;
        let tunables = self
            .get_tunables(name, scope)
            .or_else(|| self.get_tunables(name, TunableScope::Global))
            .unwrap_or_default();
        Some(EffectivePlugin::new(core, tunables))
    }

    /// List all registered plugins
    pub fn list_plugins(&self) -> Vec<String> {
        self.cores.iter().map(|e| e.key().clone()).collect()
    }

    /// Get plugin count
    pub fn len(&self) -> usize {
        self.cores.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cores.is_empty()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Preferred descriptive alias for the legacy root-crate registry.
///
/// Keeping this alias makes the file's role explicit during the crate-by-crate
/// cleanup: this is a local compatibility catalog, not the authoritative
/// plugin/schema/footprint store.
pub type LegacyPluginCatalog = PluginRegistry;

// ============================================================================
// PLUGIN TRAIT
// ============================================================================

/// Core plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn version(&self) -> &str;

    async fn get_state(&self) -> Result<Value>;
    async fn get_projected_state(&self) -> Result<MirrorState>;
    async fn set_projected_state(&self, projected: MirrorState) -> Result<()>;
    async fn apply_state(&self) -> Result<Vec<StateChange>>;
    async fn reconcile_plan(&self) -> Result<Vec<StateChange>>;
    async fn validate(&self, config: &Value) -> Result<ValidationResult>;

    async fn initialize(&mut self, _context: PluginContext) -> Result<()> {
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }

    fn state_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.name().as_bytes());
        hasher.update(self.version().as_bytes());
        hex::encode(hasher.finalize())
    }

    fn as_any(&self) -> &dyn Any;
}

pub type BoxedPlugin = Box<dyn Plugin>;

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registry() {
        let registry = PluginRegistry::new();
        let plugin = PluginCore {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            ..Default::default()
        };
        registry.register_core(plugin);
        
        assert!(registry.get_core("test").is_some());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_mirror_state_hash() {
        let state = simd_json::json!({"key": "value"});
        let ms = MirrorState::new(state);
        assert!(ms.verify());
    }

    #[test]
    fn test_state_change() {
        let change = StateChange::create("/test", simd_json::json!(42), "test change");
        assert!(!change.hash.is_empty());
    }
}
