//! D-Bus object server for projections.
//!
//! Serves every Projection through org.opdbus.v1.plugins at
//! /org/opdbus/v1/plugins/<plugin>. Nothing mounts outside the plugins root:
//! no plugin means no schema means no object.
//!
//! Data is read 1:1 from the shared-memory projection layer
//! (`/dev/shm/opdbus/projections/<plugin>.json`) on every property access.
//! No held cache — the projected tree is a pure read-through of the mutation
//! fold written by the MutationEngine (the single write door).
//!
//! Tree structure is derived on-demand: the `ProjectionRoot` interface at
//! `/org/opdbus/v1/plugins` exposes a `refresh()` D-Bus method that re-reads
//! shm and mounts/unmounts objects to match. No gRPC subscription, no polling,
//! no inotify — the tree syncs when asked.

use crate::data_models::{FieldSchema, FieldType, PluginSchema};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};
use zbus::{connection::Builder, interface, object_server::SignalEmitter, Connection};

/// A single projected object on the D-Bus object server.
///
/// Reads its data 1:1 from the shm projection layer on every property access.
/// No held cache — the mutation fold in `/dev/shm/opdbus/projections/` is the
/// single source of truth, written by the MutationEngine on every mutation.
pub struct ProjectedObject {
    /// The plugin that owns this object (e.g. "wireguard").
    pub plugin_id: String,
    /// Path segments below the plugin root (empty for the plugin root itself).
    /// e.g. `["interfaces", "0", "peers"]` for
    /// `/org/opdbus/v1/plugins/wireguard/interfaces/0/peers`
    pub path_segments: Vec<String>,
    /// Canonical schema captured when the D-Bus object is mounted.
    pub schema: Option<PluginSchema>,
}

impl ProjectedObject {
    fn new(plugin_id: &str, path_segments: Vec<String>, schema: Option<PluginSchema>) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            path_segments,
            schema: schema.or_else(|| read_plugin_schema(plugin_id)),
        }
    }

    fn schema_json_value(&self, selector: SchemaSurface) -> String {
        let Some(schema) = &self.schema else {
            return "{}".to_string();
        };

        let value = match selector {
            SchemaSurface::Methods => serde_json::to_value(&schema.methods),
            SchemaSurface::Signals => serde_json::to_value(&schema.signals),
            SchemaSurface::Guarantees => serde_json::to_value(&schema.guarantees),
        };

        value
            .ok()
            .and_then(|value| serde_json::to_string(&value).ok())
            .unwrap_or_else(|| "{}".to_string())
    }
}

enum SchemaSurface {
    Methods,
    Signals,
    Guarantees,
}

#[interface(name = "org.opdbus.v1.plugins.ProjectedObject")]
impl ProjectedObject {
    /// The schema/entity type for this object (plugin id for roots, "plugin.object" for nested)
    #[zbus(property)]
    async fn entity_type(&self) -> String {
        if self.path_segments.is_empty() {
            self.plugin_id.clone()
        } else {
            "plugin.object".to_string()
        }
    }

    /// The unique entity ID within its type
    #[zbus(property)]
    async fn entity_id(&self) -> String {
        if self.path_segments.is_empty() {
            self.plugin_id.clone()
        } else {
            format!("{}:/{}", self.plugin_id, self.path_segments.join("/"))
        }
    }

    /// Current projection state: always Valid (1:1 read of canonical state).
    #[zbus(property)]
    async fn state(&self) -> String {
        "Valid".to_string()
    }

    /// Full projection data as a JSON string, read 1:1 from shm.
    #[zbus(property)]
    async fn data(&self) -> String {
        read_projected_data(&self.plugin_id, &self.path_segments)
    }

    /// Declared D-Bus methods and required capabilities as JSON.
    #[zbus(property)]
    async fn methods(&self) -> String {
        self.schema_json_value(SchemaSurface::Methods)
    }

    /// Declared D-Bus signals as JSON.
    #[zbus(property)]
    async fn signals(&self) -> String {
        self.schema_json_value(SchemaSurface::Signals)
    }

    /// Declared operational guarantees as JSON.
    #[zbus(property)]
    async fn guarantees(&self) -> String {
        self.schema_json_value(SchemaSurface::Guarantees)
    }

    /// Signal emitted when this object's data changes
    #[zbus(signal)]
    async fn updated(emitter: &SignalEmitter<'_>, data_json: &str) -> zbus::Result<()>;
}

/// Read projected data for a plugin/path from the shm layer (1:1, zero held cache).
///
/// For the plugin root (empty `path_segments`), returns the `data` field from
/// the composite shm object (or the raw file if not a composite).
/// For nested objects, parses the JSON, navigates to the path within `data`,
/// and re-serializes.
fn read_projected_data(plugin_id: &str, path_segments: &[String]) -> String {
    let bytes = match op_core::projection_shm::read_projection_bytes(plugin_id) {
        Some(b) => b,
        None => return "{}".to_string(),
    };
    let value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return "{}".to_string(),
    };

    // Navigate from the `data` field if present (composite shm format).
    let root = value.get("data").unwrap_or(&value);

    if path_segments.is_empty() {
        serde_json::to_string(root).unwrap_or_else(|_| "{}".to_string())
    } else {
        match navigate_json(root, path_segments) {
            Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string()),
            None => "{}".to_string(),
        }
    }
}

fn read_plugin_schema(plugin_id: &str) -> Option<PluginSchema> {
    let bytes = std::fs::read("/dev/shm/live-schema.json").ok()?;
    let root = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    let schema = root
        .as_object()?
        .get(plugin_id)?
        .as_array()
        .and_then(|items| items.first())
        .or_else(|| root.as_object()?.get(plugin_id))?;
    serde_json::from_value::<PluginSchema>(schema.clone()).ok()
}

/// Navigate a JSON value by path segments (object keys or array indices).
fn navigate_json<'a>(
    value: &'a serde_json::Value,
    segments: &[String],
) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in segments {
        current = if let Ok(idx) = segment.parse::<usize>() {
            current.get(idx)?
        } else {
            current.get(segment.as_str())?
        };
    }
    Some(current)
}

/// Derives the D-Bus object path from a projection's entity_type and entity_id.
///
/// Every projected object lives under the single plugins root, and nested
/// objects live under the plugin that produced them. No plugin means no schema
/// means no object — nothing is ever mounted outside this path.
///
/// entity_type "mail_server", entity_id "mail_server"
///   → /org/opdbus/v1/plugins/mail_server
/// entity_type "plugin.object", entity_id "wireguard:/interfaces/0"
///   → /org/opdbus/v1/plugins/wireguard/interfaces/0
pub fn projection_path(entity_type: &str, entity_id: &str) -> String {
    if entity_type == "plugin.object" {
        if let Some((plugin_id, object_path)) = entity_id.split_once(':') {
            return plugin_object_path(plugin_id, object_path);
        }
    }

    op_plugins::canonical::plugin_path(if entity_id.is_empty() {
        entity_type
    } else {
        entity_id
    })
}

fn plugin_object_path(plugin_id: &str, object_path: &str) -> String {
    let mut path = op_plugins::canonical::plugin_path(plugin_id);

    for segment in object_path.split('/').filter(|segment| !segment.is_empty()) {
        path.push('/');
        path.push_str(&sanitize_path_segment(segment));
    }

    path
}

fn sanitize_path_segment(segment: &str) -> String {
    let sanitized: String = segment
        .chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                Some(c)
            } else if c == '-' || c == '.' {
                Some('_')
            } else {
                None
            }
        })
        .take(255)
        .collect();

    if sanitized.is_empty() {
        "_".to_string()
    } else {
        sanitized
    }
}

/// Build the D-Bus object path from a plugin id and path segments.
fn build_projected_path(plugin_id: &str, path_segments: &[String]) -> String {
    let mut path = format!(
        "{}/{}",
        op_plugins::canonical::PLUGIN_BASE_PATH,
        sanitize_path_segment(plugin_id)
    );
    for seg in path_segments {
        path.push('/');
        path.push_str(&sanitize_path_segment(seg));
    }
    path
}

/// Root interface at `/org/opdbus/v1/plugins`.
///
/// Always present so D-Bus introspection works even with zero plugins mounted.
/// The `plugins` property reads shm live. The `refresh()` method re-reads shm
/// and syncs the D-Bus tree (mount/unmount objects to match). This is the
/// dynamic-mount mechanism: no gRPC, no polling, no inotify — the tree is
/// derived on demand.
pub struct ProjectionRoot {
    objects: Arc<Mutex<HashSet<String>>>,
    schemas: Arc<HashMap<String, PluginSchema>>,
}

#[interface(name = "org.opdbus.v1.plugins")]
impl ProjectionRoot {
    /// List of plugin IDs currently projected in shm (live read).
    #[zbus(property)]
    async fn plugins(&self) -> Vec<String> {
        op_core::projection_shm::list_projected_plugins()
    }

    /// Number of D-Bus objects currently mounted.
    #[zbus(property)]
    async fn object_count(&self) -> u64 {
        self.objects.lock().await.len() as u64
    }

    /// Re-read shm and sync the D-Bus tree. Mounts new objects, unmounts stale
    /// ones. Returns the total object count after refresh.
    ///
    /// Call this after a mutation has written new state to shm. The
    /// MutationEngine can call this via D-Bus after writing to shm, or any
    /// client can call it to force a tree refresh.
    async fn refresh(&self, #[zbus(connection)] conn: &Connection) -> u64 {
        let mut objects = self.objects.lock().await;
        match sync_all_from_shm(conn, &mut objects, &self.schemas).await {
            Ok(()) => objects.len() as u64,
            Err(e) => {
                tracing::warn!(error = %e, "refresh() failed");
                objects.len() as u64
            }
        }
    }
}

/// Read all plugins from shm, derive paths, and sync the D-Bus tree.
///
/// For each plugin in shm, derives all nested object paths from the JSON
/// structure and mounts/unmounts `ProjectedObject` instances to match.
/// Plugins that are in shm but not yet mounted are added; objects that are
/// mounted but no longer in shm are removed.
async fn sync_all_from_shm(
    conn: &Connection,
    objects: &mut HashSet<String>,
    schemas: &HashMap<String, PluginSchema>,
) -> Result<()> {
    let plugins = op_core::projection_shm::list_projected_plugins();

    // Unmount plugins that are no longer in shm.
    let plugin_root_base = op_plugins::canonical::plugin_path("");
    let _ = plugin_root_base; // path prefix helper

    let to_remove: Vec<String> = objects
        .iter()
        .filter(|p| {
            // Keep only paths whose plugin is still in shm
            let parts: Vec<&str> = p.trim_start_matches('/').split('/').collect();
            if parts.len() >= 5
                && parts[0] == "org"
                && parts[1] == "opdbus"
                && parts[2] == "v1"
                && parts[3] == "plugins"
            {
                let plugin_id = parts[4];
                !plugins.iter().any(|p| p.as_str() == plugin_id)
            } else {
                false
            }
        })
        .cloned()
        .collect();

    for path in to_remove {
        if objects.remove(&path) {
            conn.object_server()
                .remove::<ProjectedObject, _>(path.as_str())
                .await?;
            debug!(
                path,
                "Unmounted stale D-Bus projection object (plugin gone from shm)"
            );
        }
    }

    // Mount/sync all plugins currently in shm.
    for plugin_id in &plugins {
        if let Some(paths) = read_and_derive_paths(plugin_id, schemas.get(plugin_id)) {
            sync_plugin_paths(conn, objects, plugin_id, &paths, schemas).await?;
        }
    }

    Ok(())
}

/// Sync a single plugin's D-Bus objects to match the derived paths.
async fn sync_plugin_paths(
    conn: &Connection,
    objects: &mut HashSet<String>,
    plugin_id: &str,
    paths: &[Vec<String>],
    schemas: &HashMap<String, PluginSchema>,
) -> Result<()> {
    let new_paths: HashSet<String> = paths
        .iter()
        .map(|segs| build_projected_path(plugin_id, segs))
        .collect();

    let plugin_root = op_plugins::canonical::plugin_path(plugin_id);
    let plugin_prefix = format!("{}/", plugin_root);

    // Unmount paths that no longer exist in the derived set.
    let to_remove: Vec<String> = objects
        .iter()
        .filter(|p| {
            (**p == plugin_root || p.starts_with(&plugin_prefix)) && !new_paths.contains(*p)
        })
        .cloned()
        .collect();

    for path in to_remove {
        if objects.remove(&path) {
            conn.object_server()
                .remove::<ProjectedObject, _>(path.as_str())
                .await?;
            debug!(path, "Unmounted stale D-Bus projection object");
        }
    }

    // Mount all current paths.
    for segs in paths {
        let path = build_projected_path(plugin_id, segs);
        if !objects.contains(&path) {
            let obj =
                ProjectedObject::new(plugin_id, segs.to_vec(), schemas.get(plugin_id).cloned());
            conn.object_server().at(path.as_str(), obj).await?;
            objects.insert(path.clone());
            debug!(path, "Mounted D-Bus projection object");
        }
    }

    Ok(())
}

/// Read a plugin's projection from shm and derive schema-backed D-Bus paths.
fn read_and_derive_paths(
    plugin_id: &str,
    schema: Option<&PluginSchema>,
) -> Option<Vec<Vec<String>>> {
    let schema = schema?;
    let bytes = op_core::projection_shm::read_projection_bytes(plugin_id)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    // Derive paths from the `data` field if present (composite shm format).
    let root = value.get("data").unwrap_or(&value);
    Some(derive_paths_from_schema_and_state(schema, root))
}

/// Derive D-Bus paths from a schemars-generated PluginSchema and current state.
///
/// The schema controls which object/array fields can become D-Bus child
/// objects. State only supplies current values and instance identifiers.
fn derive_paths_from_schema_and_state(
    schema: &PluginSchema,
    value: &serde_json::Value,
) -> Vec<Vec<String>> {
    let mut paths = vec![Vec::new()]; // root (plugin root)
    derive_paths_recursive(value, &schema.fields, &mut Vec::new(), &mut paths);
    paths
}

fn derive_paths_recursive(
    value: &serde_json::Value,
    fields: &HashMap<String, FieldSchema>,
    current: &mut Vec<String>,
    paths: &mut Vec<Vec<String>>,
) {
    let Some(map) = value.as_object() else {
        return;
    };

    for (field_name, field_schema) in fields {
        if field_name.starts_with('_') {
            continue;
        }
        let Some(child) = map.get(field_name) else {
            continue;
        };

        match &field_schema.field_type {
            FieldType::Object(child_fields) if child.is_object() => {
                current.push(field_name.clone());
                paths.push(current.clone());
                derive_paths_recursive(child, child_fields, current, paths);
                current.pop();
            }
            FieldType::Array(item_type) => {
                let Some(items) = child.as_array() else {
                    continue;
                };
                let FieldType::Object(item_fields) = item_type.as_ref() else {
                    continue;
                };

                current.push(field_name.clone());
                paths.push(current.clone());
                for (idx, item) in items.iter().enumerate() {
                    if !item.is_object() {
                        continue;
                    }
                    let segment = ["id", "name", "label", "key", "path", "domain", "host"]
                        .iter()
                        .find_map(|&field| item.get(field).and_then(|v| v.as_str()))
                        .map(|s| s.replace(['/', ' ', ':'], "_"))
                        .unwrap_or_else(|| idx.to_string());
                    current.push(segment);
                    paths.push(current.clone());
                    derive_paths_recursive(item, item_fields, current, paths);
                    current.pop();
                }
                current.pop();
            }
            _ => {}
        }
    }
}

/// Manages the set of D-Bus objects served for all projections.
///
/// Each object reads its data 1:1 from the shm projection layer on every
/// property access. The server only tracks which paths are mounted (for
/// mount/unmount lifecycle); it holds no data copies.
pub struct ProjectionDbusServer {
    conn: Connection,
    /// Mounted object paths (shared with ProjectionRoot for refresh())
    objects: Arc<Mutex<HashSet<String>>>,
    /// Canonical schema catalog passed into D-Bus object creation.
    schemas: Arc<HashMap<String, PluginSchema>>,
}

impl ProjectionDbusServer {
    pub async fn new() -> Result<Self> {
        Self::new_with_schemas(HashMap::new()).await
    }

    pub async fn new_with_schemas(schemas: HashMap<String, PluginSchema>) -> Result<Self> {
        let objects = Arc::new(Mutex::new(HashSet::new()));
        let schemas = Arc::new(schemas);
        let root = ProjectionRoot {
            objects: objects.clone(),
            schemas: schemas.clone(),
        };

        let conn = match std::env::var("OP_DBUS_PROJECTION_BUS")
            .unwrap_or_else(|_| "system".to_string())
            .as_str()
        {
            "session" => {
                Builder::session()?
                    .serve_at("/org/opdbus/v1/plugins", root)?
                    .name(op_plugins::canonical::BASE_SERVICE_NAME)?
                    .build()
                    .await?
            }
            _ => {
                Builder::system()?
                    .serve_at("/org/opdbus/v1/plugins", root)?
                    .name(op_plugins::canonical::BASE_SERVICE_NAME)?
                    .build()
                    .await?
            }
        };

        info!(
            service = op_plugins::canonical::BASE_SERVICE_NAME,
            "D-Bus plugin projection connection established"
        );

        Ok(Self {
            conn,
            objects,
            schemas,
        })
    }

    pub async fn new_session() -> Result<Self> {
        Self::new_session_with_schemas(HashMap::new()).await
    }

    pub async fn new_session_with_schemas(schemas: HashMap<String, PluginSchema>) -> Result<Self> {
        let objects = Arc::new(Mutex::new(HashSet::new()));
        let schemas = Arc::new(schemas);
        let root = ProjectionRoot {
            objects: objects.clone(),
            schemas: schemas.clone(),
        };

        let conn = Builder::session()?
            .serve_at("/org/opdbus/v1/plugins", root)?
            .name(op_plugins::canonical::BASE_SERVICE_NAME)?
            .build()
            .await?;

        info!(
            service = op_plugins::canonical::BASE_SERVICE_NAME,
            "D-Bus session plugin projection connection established"
        );

        Ok(Self {
            conn,
            objects,
            schemas,
        })
    }

    /// Sync the D-Bus tree from shm. Mounts new objects, unmounts stale ones.
    /// Called on startup. At runtime, the `refresh()` D-Bus method does the same.
    pub async fn sync_from_shm(&self) -> Result<()> {
        let mut objects = self.objects.lock().await;
        sync_all_from_shm(&self.conn, &mut objects, &self.schemas).await
    }

    /// Ensure all known plugins have at least a root object mounted, even if
    /// no shm data exists yet (no mutations). Plugins with shm data get their
    /// full derived tree; plugins without shm get just the root (data returns `{}`).
    pub async fn seed_plugin_roots(&self, plugin_ids: &[String]) -> Result<()> {
        let mut objects = self.objects.lock().await;
        for plugin_id in plugin_ids {
            let Some(schema) = self.schemas.get(plugin_id) else {
                debug!(
                    plugin_id,
                    "Skipping plugin root seed because schema is missing"
                );
                continue;
            };
            // If shm has data, sync the full derived tree.
            if let Some(paths) = read_and_derive_paths(plugin_id, Some(schema)) {
                sync_plugin_paths(&self.conn, &mut objects, plugin_id, &paths, &self.schemas)
                    .await?;
            } else {
                // No shm data — mount just the root so the plugin appears in the tree.
                let root_path = build_projected_path(plugin_id, &[]);
                if !objects.contains(&root_path) {
                    let obj = ProjectedObject::new(plugin_id, Vec::new(), Some(schema.clone()));
                    self.conn
                        .object_server()
                        .at(root_path.as_str(), obj)
                        .await?;
                    objects.insert(root_path.clone());
                    debug!(path = root_path, "Seeded plugin root (no shm data yet)");
                }
            }
        }
        Ok(())
    }

    /// Sync a single plugin's D-Bus objects to match the derived paths.
    pub async fn sync_plugin(&mut self, plugin_id: &str, paths: &[Vec<String>]) -> Result<()> {
        let mut objects = self.objects.lock().await;
        sync_plugin_paths(&self.conn, &mut objects, plugin_id, paths, &self.schemas).await
    }

    pub async fn object_count(&self) -> usize {
        self.objects.lock().await.len()
    }

    /// Mount a projection as a D-Bus object (or emit `updated` if already mounted).
    pub async fn upsert(&mut self, plugin_id: &str, path_segments: &[String]) -> Result<()> {
        let path = build_projected_path(plugin_id, path_segments);

        {
            let mut objects = self.objects.lock().await;
            if !objects.contains(&path) {
                let obj = ProjectedObject::new(
                    plugin_id,
                    path_segments.to_vec(),
                    self.schemas.get(plugin_id).cloned(),
                );
                self.conn.object_server().at(path.as_str(), obj).await?;
                objects.insert(path.clone());
                debug!(path, "Mounted D-Bus projection object");
            }
        }

        // Emit updated signal with current shm data (push notification).
        let data = read_projected_data(plugin_id, path_segments);
        let iface_ref = self
            .conn
            .object_server()
            .interface::<_, ProjectedObject>(path.as_str())
            .await?;
        ProjectedObject::updated(iface_ref.signal_emitter(), &data).await?;

        debug!(path, "Updated D-Bus projection object");
        Ok(())
    }

    /// Remove a projection's D-Bus object.
    pub async fn remove(&mut self, plugin_id: &str, path_segments: &[String]) -> Result<()> {
        let path = build_projected_path(plugin_id, path_segments);
        let mut objects = self.objects.lock().await;
        if objects.remove(&path) {
            self.conn
                .object_server()
                .remove::<ProjectedObject, _>(path.as_str())
                .await?;
            info!(path, "Removed D-Bus projection object");
        }
        Ok(())
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::projection_path;

    #[test]
    fn projects_plugin_roots_under_canonical_plugin_path() {
        assert_eq!(
            projection_path("mail_server", "mail_server"),
            "/org/opdbus/v1/plugins/mail_server"
        );
        assert_eq!(
            projection_path("ovsdb_bridge", "ovsdb_bridge"),
            "/org/opdbus/v1/plugins/ovsdb_bridge"
        );
    }

    #[test]
    fn projects_nested_objects_under_owning_plugin() {
        assert_eq!(
            projection_path("plugin.object", "wireguard:/interfaces/0/peers"),
            "/org/opdbus/v1/plugins/wireguard/interfaces/0/peers"
        );
    }
}
