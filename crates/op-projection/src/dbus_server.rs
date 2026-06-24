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

use anyhow::Result;
use std::collections::HashSet;
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

    /// Signal emitted when this object's data changes
    #[zbus(signal)]
    async fn updated(emitter: &SignalEmitter<'_>, data_json: &str) -> zbus::Result<()>;
}

/// Read projected data for a plugin/path from the shm layer (1:1, zero held cache).
///
/// For the plugin root (empty `path_segments`), returns the raw file contents.
/// For nested objects, parses the JSON, navigates to the path, and re-serializes.
fn read_projected_data(plugin_id: &str, path_segments: &[String]) -> String {
    let bytes = match op_core::projection_shm::read_projection_bytes(plugin_id) {
        Some(b) => b,
        None => return "{}".to_string(),
    };
    if path_segments.is_empty() {
        String::from_utf8(bytes).unwrap_or_else(|_| "{}".to_string())
    } else {
        let value: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => return "{}".to_string(),
        };
        match navigate_json(&value, path_segments) {
            Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string()),
            None => "{}".to_string(),
        }
    }
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
    let mut path = op_plugins::canonical::plugin_path(plugin_id);
    for seg in path_segments {
        path.push('/');
        path.push_str(&sanitize_path_segment(seg));
    }
    path
}

/// Manages the set of D-Bus objects served for all projections.
///
/// Each object reads its data 1:1 from the shm projection layer on every
/// property access. The server only tracks which paths are mounted (for
/// mount/unmount lifecycle); it holds no data copies.
pub struct ProjectionDbusServer {
    conn: Connection,
    /// Mounted object paths (for mount/unmount tracking only)
    objects: HashSet<String>,
}

impl ProjectionDbusServer {
    pub async fn new() -> Result<Self> {
        let conn = match std::env::var("OP_DBUS_PROJECTION_BUS")
            .unwrap_or_else(|_| "system".to_string())
            .as_str()
        {
            "session" => {
                Builder::session()?
                    .name(op_plugins::canonical::BASE_SERVICE_NAME)?
                    .build()
                    .await?
            }
            _ => {
                Builder::system()?
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
            objects: HashSet::new(),
        })
    }

    pub async fn new_session() -> Result<Self> {
        let conn = Builder::session()?
            .name(op_plugins::canonical::BASE_SERVICE_NAME)?
            .build()
            .await?;

        info!(
            service = op_plugins::canonical::BASE_SERVICE_NAME,
            "D-Bus session plugin projection connection established"
        );

        Ok(Self {
            conn,
            objects: HashSet::new(),
        })
    }

    /// Mount a projection as a D-Bus object (or emit `updated` if already mounted).
    ///
    /// The object's `data` property reads 1:1 from shm on every access, so
    /// there is no data to update in place — only the mount/unmount lifecycle
    /// is tracked here, plus the `updated` signal for push consumers.
    pub async fn upsert(&mut self, plugin_id: &str, path_segments: &[String]) -> Result<()> {
        let path = build_projected_path(plugin_id, path_segments);

        if !self.objects.contains(&path) {
            let obj = ProjectedObject {
                plugin_id: plugin_id.to_string(),
                path_segments: path_segments.to_vec(),
            };
            self.conn.object_server().at(path.as_str(), obj).await?;
            self.objects.insert(path.clone());
            debug!(path, "Mounted D-Bus projection object");
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
        if self.objects.remove(&path) {
            self.conn
                .object_server()
                .remove::<ProjectedObject, _>(path.as_str())
                .await?;
            info!(path, "Removed D-Bus projection object");
        }
        Ok(())
    }

    /// Sync a plugin's full set of D-Bus objects to match the derived paths.
    ///
    /// Mounts new paths, unmounts paths that no longer exist, and emits
    /// `updated` on all current paths. Called on startup (initial mount from
    /// shm) and on every StateChange (mutation-driven push).
    pub async fn sync_plugin(&mut self, plugin_id: &str, paths: &[Vec<String>]) -> Result<()> {
        let new_paths: HashSet<String> = paths
            .iter()
            .map(|segs| build_projected_path(plugin_id, segs))
            .collect();

        let plugin_root = op_plugins::canonical::plugin_path(plugin_id);
        let plugin_prefix = format!("{}/", plugin_root);

        // Unmount paths that no longer exist in the derived set.
        let to_remove: Vec<String> = self
            .objects
            .iter()
            .filter(|p| {
                (**p == plugin_root || p.starts_with(&plugin_prefix))
                    && !new_paths.contains(*p)
            })
            .cloned()
            .collect();

        for path in to_remove {
            if self.objects.remove(&path) {
                self.conn
                    .object_server()
                    .remove::<ProjectedObject, _>(path.as_str())
                    .await?;
                debug!(path, "Unmounted stale D-Bus projection object");
            }
        }

        // Mount/upsert all current paths.
        for segs in paths {
            self.upsert(plugin_id, segs).await?;
        }

        Ok(())
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
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
