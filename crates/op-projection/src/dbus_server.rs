//! D-Bus object server for projections.
//!
//! Serves every Projection through org.opdbus.v1.plugins at
//! /org/opdbus/v1/plugins/<plugin>. Nothing mounts outside the plugins root:
//! no plugin means no schema means no object.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use zbus::{connection::Builder, interface, object_server::SignalEmitter, Connection};

use crate::data_models::Projection;

/// A single projected object on the D-Bus object server.
pub struct ProjectedObject {
    pub entity_type: String,
    pub entity_id: String,
    /// JSON-serialized projection data
    pub data_json: Arc<RwLock<String>>,
    pub state: Arc<RwLock<String>>,
}

#[interface(name = "org.opdbus.v1.plugins.ProjectedObject")]
impl ProjectedObject {
    /// The schema/entity type for this object (e.g. "system.memory")
    #[zbus(property)]
    async fn entity_type(&self) -> &str {
        &self.entity_type
    }

    /// The unique entity ID within its type
    #[zbus(property)]
    async fn entity_id(&self) -> &str {
        &self.entity_id
    }

    /// Current projection state: Valid, Quarantined, Degraded, etc.
    #[zbus(property)]
    async fn state(&self) -> String {
        self.state.read().await.clone()
    }

    /// Full projection data as a JSON string
    #[zbus(property)]
    async fn data(&self) -> String {
        self.data_json.read().await.clone()
    }

    /// Signal emitted when this object's data changes
    #[zbus(signal)]
    async fn updated(emitter: &SignalEmitter<'_>, data_json: &str) -> zbus::Result<()>;
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

/// Object state handles for updating in place
type ObjectHandles = (Arc<RwLock<String>>, Arc<RwLock<String>>);

/// Manages the set of D-Bus objects served for all projections.
pub struct ProjectionDbusServer {
    conn: Connection,
    /// path → data/state handles so we can update in place
    objects: HashMap<String, ObjectHandles>,
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
            objects: HashMap::new(),
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
            objects: HashMap::new(),
        })
    }

    /// Register a projection as a D-Bus object (or update it if already registered).
    pub async fn upsert(&mut self, projection: &Projection) -> Result<()> {
        let path = projection_path(&projection.entity_type, &projection.entity_id);
        let data_json = simd_json::to_string(&projection.data).unwrap_or_else(|_| "{}".to_string());
        let state_str = format!("{:?}", projection.state);

        if let Some((data_handle, state_handle)) = self.objects.get(&path) {
            // Update existing object in place
            *data_handle.write().await = data_json.clone();
            *state_handle.write().await = state_str;

            // Emit the updated signal
            let iface_ref = self
                .conn
                .object_server()
                .interface::<_, ProjectedObject>(path.as_str())
                .await?;
            ProjectedObject::updated(iface_ref.signal_emitter(), &data_json).await?;

            debug!(path, "updated D-Bus projection object");
        } else {
            // Register new object
            let data_arc = Arc::new(RwLock::new(data_json));
            let state_arc = Arc::new(RwLock::new(state_str));

            let obj = ProjectedObject {
                entity_type: projection.entity_type.clone(),
                entity_id: projection.entity_id.clone(),
                data_json: data_arc.clone(),
                state: state_arc.clone(),
            };

            self.conn.object_server().at(path.as_str(), obj).await?;

            self.objects.insert(path.clone(), (data_arc, state_arc));
            debug!(path, entity_type = %projection.entity_type, "registered D-Bus projection object");
        }

        Ok(())
    }

    /// Remove a projection's D-Bus object.
    pub async fn remove(&mut self, entity_type: &str, entity_id: &str) -> Result<()> {
        let path = projection_path(entity_type, entity_id);
        if self.objects.remove(&path).is_some() {
            self.conn
                .object_server()
                .remove::<ProjectedObject, _>(path.as_str())
                .await?;
            info!(path, "removed D-Bus projection object");
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
