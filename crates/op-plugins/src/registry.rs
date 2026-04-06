//! Plugin catalog and registration path.
//!
//! Architectural intent:
//! - Plugin code is the origin of schema truth.
//! - Registration materializes one canonical plugin document.
//! - That document is persisted to the schema catalog store first.
//! - The in-memory catalog here is only a local index/projection cache.
//!
//! Compatibility note:
//! this file still exports `PluginRegistry` because much of the workspace still
//! uses that name. New code should think of it as a plugin catalog entry point.

use anyhow::{anyhow, Result};
use op_core::state_publisher::StatePublisher;
use op_dbus_model::{CatalogDocument, SqlitePluginCatalog};
use op_state::StatePlugin;
use op_state_store::{
    builtin_plugin_schema, FieldSchema, FieldType, PluginSchema, SchemaCatalog, SchemaRegistry,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{info, warn};
use crate::plugin::PluginMetadata;

/// Live runtime record for a registered plugin instance.
///
/// This is not the persisted source of truth. The canonical persisted form is
/// the catalog document written during registration.
pub struct PluginRecord {
    pub name: String,
    pub plugin: Arc<dyn StatePlugin>,
    pub storage_path: PathBuf,
    pub change_count: u64,
    pub schema: Option<PluginSchema>,
    pub dbus_path: String,
}

/// Compatibility name for the plugin-catalog entry point.
pub struct PluginRegistry {
    plugins: Arc<AsyncRwLock<HashMap<String, PluginRecord>>>,
    schema_catalog: Arc<RwLock<SchemaCatalog>>,
    schema_catalog_store: Option<Arc<SqlitePluginCatalog>>,
    base_path: PathBuf,
    publisher: AsyncRwLock<Option<Arc<dyn StatePublisher>>>,
    dbus_conn: AsyncRwLock<Option<zbus::Connection>>,
}

/// Preferred architectural name for `PluginRegistry`.
pub type PluginCatalog = PluginRegistry;

impl PluginRegistry {
    /// Create a new plugin catalog entry point.
    ///
    /// Authority still originates in the plugin and the persisted canonical
    /// plugin document it writes. This type is the local runtime catalog over
    /// that persisted truth.
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self::with_schema_catalog(base_path, Arc::new(RwLock::new(SchemaCatalog::empty())))
    }

    /// Preferred constructor: create a new plugin catalog backed by a shared
    /// schema catalog.
    pub fn with_schema_catalog(
        base_path: impl AsRef<Path>,
        schema_catalog: Arc<RwLock<SchemaCatalog>>,
    ) -> Self {
        Self::with_schema_registry(base_path, schema_catalog)
    }

    /// Compatibility constructor for older code that still says `registry`.
    pub fn with_schema_registry(
        base_path: impl AsRef<Path>,
        schema_catalog: Arc<RwLock<SchemaCatalog>>,
    ) -> Self {
        Self::with_schema_catalog_and_store(base_path, schema_catalog, None)
    }

    /// Preferred constructor: create a new plugin catalog backed by a shared
    /// schema catalog and an optional persisted plugin-catalog store.
    pub fn with_schema_catalog_and_store(
        base_path: impl AsRef<Path>,
        schema_catalog: Arc<RwLock<SchemaCatalog>>,
        schema_catalog_store: Option<Arc<SqlitePluginCatalog>>,
    ) -> Self {
        Self::with_schema_registry_and_catalog(base_path, schema_catalog, schema_catalog_store)
    }

    /// Compatibility constructor for older code that still says `registry`.
    ///
    /// The persisted catalog store is where the canonical plugin document
    /// lives. The in-memory schema catalog remains an index/projection layer
    /// over that data for fast local resolution.
    pub fn with_schema_registry_and_catalog(
        base_path: impl AsRef<Path>,
        schema_catalog: Arc<RwLock<SchemaCatalog>>,
        schema_catalog_store: Option<Arc<SqlitePluginCatalog>>,
    ) -> Self {
        Self {
            plugins: Arc::new(AsyncRwLock::new(HashMap::new())),
            schema_catalog,
            schema_catalog_store,
            base_path: base_path.as_ref().to_path_buf(),
            publisher: AsyncRwLock::new(None),
            dbus_conn: AsyncRwLock::new(None),
        }
    }

    /// Compatibility accessor. Architecturally this is the in-memory schema
    /// catalog, not an origin-authority registry.
    pub fn schema_registry(&self) -> Arc<RwLock<SchemaRegistry>> {
        self.schema_catalog.clone()
    }

    pub fn schema_catalog(&self) -> Arc<RwLock<SchemaCatalog>> {
        self.schema_catalog.clone()
    }

    /// Set the state publisher for catalog-derived projection updates.
    pub async fn set_publisher(&self, publisher: Arc<dyn StatePublisher>) {
        *self.publisher.write().await = Some(publisher);
        self.project_registered_schemas().await;
    }

    /// Set the D-Bus connection for object exportation
    pub async fn set_dbus_connection(&self, conn: zbus::Connection) {
        *self.dbus_conn.write().await = Some(conn);
    }

    /// Register a plugin instance
    pub async fn register(&self, plugin: Arc<dyn StatePlugin>) -> Result<()> {
        let name = plugin.name().to_string();
        let mut plugins = self.plugins.write().await;

        if plugins.contains_key(&name) {
            return Err(anyhow!("Plugin '{}' already registered", name));
        }

        // Ensure BTRFS subvolume for plugin storage
        let storage_path = self.create_plugin_subvolume(&name).await?;

        let dbus_path_str = format!("/org/opdbus/v1/plugins/{}", name);

        let metadata = plugin.metadata();
        let schema = match build_plugin_schema(&name, plugin.as_ref(), &metadata) {
            Some(schema) => schema,
            None => {
                let msg = format!("Plugin '{}' has no schema", name);
                warn!("{}", msg);
                return Err(anyhow!(msg));
            }
        };

        // Registration order matters:
        // 1. Build the canonical plugin document from plugin-owned schema.
        // 2. Persist that document into the schema catalog store.
        // 3. Update the in-memory schema catalog for fast local resolution.
        // 4. Export projections (D-Bus, gRPC-facing state publisher).
        //
        // This keeps the persisted canonical plugin document ahead of local
        // projection caches and avoids reintroducing a second authority.
        let document = build_catalog_document(plugin.as_ref(), &schema, &dbus_path_str, &storage_path);

        if let Some(catalog_store) = &self.schema_catalog_store {
            catalog_store.upsert_document(&document).await?;
            info!("Plugin {} persisted to schema catalog store", name);
        }

        self.schema_catalog.write().register(schema.clone());
        info!("Plugin {} indexed in schema catalog", name);

        if let Some(conn) = &*self.dbus_conn.read().await {
            let host = op_state::dbus_server::PluginDbusHost {
                plugin: plugin.clone(),
                schema_registry: self.schema_catalog.clone(),
            };
            if let Ok(path) = zbus::zvariant::ObjectPath::try_from(dbus_path_str.as_str()) {
                let _ = conn.object_server().at(path, host).await;
                info!("Plugin {} exported to D-Bus at {}", name, dbus_path_str);
            }
        }

        if let Some(publ) = &*self.publisher.read().await {
            let _ = publ
                .publish_change(
                    name.clone(),
                    format!("schema/{}/{}", schema.category, name),
                    op_core::state_publisher::ChangeType::PropertySet,
                    Some("definition".to_string()),
                    None,
                    schema.to_json_schema(),
                    vec!["schema".to_string(), schema.category.clone()],
                    "catalog".to_string(),
                )
                .await;
            info!("Plugin {} schema projected from catalog", name);
        }

        plugins.insert(
            name.clone(),
            PluginRecord {
                name,
                plugin,
                storage_path,
                change_count: 0,
                schema: Some(schema),
                dbus_path: dbus_path_str,
            },
        );

        Ok(())
    }

    /// Get a plugin by name
    pub async fn get(&self, name: &str) -> Option<Arc<dyn StatePlugin>> {
        let plugins = self.plugins.read().await;
        plugins.get(name).map(|r| r.plugin.clone())
    }

    async fn create_plugin_subvolume(&self, name: &str) -> Result<PathBuf> {
        let path = self.base_path.join("plugins").join(name);

        if path.exists() {
            return Ok(path);
        }

        tokio::fs::create_dir_all(path.parent().unwrap()).await?;

        let output = Command::new("btrfs")
            .args(["subvolume", "create"])
            .arg(&path)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("not a btrfs filesystem") {
                warn!(
                    "BTRFS subvolume creation failed: {}. Falling back to directory.",
                    stderr
                );
            }
            tokio::fs::create_dir_all(&path).await?;
        }

        Ok(path)
    }

    async fn project_registered_schemas(&self) {
        let Some(publisher) = self.publisher.read().await.clone() else {
            return;
        };

        let snapshots = {
            let catalog = self.schema_catalog.read();
            let mut names: Vec<String> = catalog.list().into_iter().map(str::to_string).collect();
            names.sort();
            names
                .into_iter()
                .filter_map(|name| {
                    catalog.get_copies(&name).map(|copies| {
                        (
                            name,
                            copies.plugin.category.clone(),
                            copies.json_schema.clone(),
                        )
                    })
                })
                .collect::<Vec<_>>()
        };

        for (name, category, schema) in snapshots {
            if let Err(error) = publisher
                .publish_change(
                    name.clone(),
                    format!("schema/{category}/{name}"),
                    op_core::state_publisher::ChangeType::PropertySet,
                    Some("definition".to_string()),
                    None,
                    schema,
                    vec!["schema".to_string(), category],
                    "catalog".to_string(),
                )
                .await
            {
                warn!("Failed to project schema for {}: {}", name, error);
            }
        }
    }

    /// Hydrate the in-memory catalog from any persisted plugin documents.
    pub async fn hydrate_catalog_from_store(&self) -> Result<()> {
        let store = match &self.schema_catalog_store {
            Some(store) => store,
            None => return Ok(()),
        };

        let documents = store.list_documents().await?;
        let mut catalog = self.schema_catalog.write();
        for document in documents {
            catalog.register(document.schema);
        }
        Ok(())
    }
}

fn build_catalog_document(
    plugin: &dyn StatePlugin,
    schema: &PluginSchema,
    dbus_path: &str,
    storage_path: &Path,
) -> CatalogDocument {
    let metadata = plugin.metadata();
    CatalogDocument {
        schema: schema.clone(),
        dbus_path: dbus_path.to_string(),
        service_name: default_service_name(schema.name.as_str(), &metadata.dbus_services),
        storage_path: storage_path.to_string_lossy().to_string(),
        source: "plugin".to_string(),
    }
}

fn default_service_name(name: &str, dbus_services: &[String]) -> String {
    dbus_services
        .first()
        .cloned()
        .unwrap_or_else(|| format!("org.opdbus.{}.v1", name.replace('-', "_")))
}

fn default_plugin_category(name: &str) -> &'static str {
    match name {
        "net" | "rtnetlink" | "wireguard" | "openflow" | "ovsdb_bridge" => "network",
        "privacy_router" | "privacy_routes" | "adc" | "gcloud_adc" | "keypair" => "security",
        "incus" | "proxmox" => "compute",
        "users" => "identity",
        "hardware" | "software" => "inventory",
        "mcp" => "automation",
        "web_ui" => "ui",
        "config" | "agent_config" | "dinit" | "endpoint" | "proxy_server" | "sess_decl" => "system",
        _ => "plugin",
    }
}

fn build_plugin_schema(
    name: &str,
    plugin: &dyn StatePlugin,
    metadata: &PluginMetadata,
) -> Option<PluginSchema> {
    let mut schema = plugin.schema().or_else(|| builtin_plugin_schema(name))?;
    apply_plugin_metadata(&mut schema, name, metadata);
    Some(schema)
}

fn apply_plugin_metadata(schema: &mut PluginSchema, name: &str, metadata: &PluginMetadata) {
    schema.name = name.to_string();
    if schema.category.is_empty() {
        schema.category = default_plugin_category(name).to_string();
    }
    if schema.description.is_empty() {
        schema.description = metadata.description.clone();
    }
    if schema.dependencies.is_empty() {
        schema.dependencies = metadata.dependencies.clone();
    }
    if !schema.tags.iter().any(|tag| tag == &schema.category) {
        schema.tags.push(schema.category.clone());
    }
}
