//! D-Bus object server for projections.
//!
//! Serves every Projection as a D-Bus object under org.opdbus.projection at
//! /org/opdbus/<category>/<id>, e.g. /org/opdbus/system/memory or
//! /org/opdbus/system/process/1234.

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

#[interface(name = "org.opdbus.projection.v1.Object")]
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
/// entity_type "system.memory"    → /org/opdbus/system/memory
/// entity_type "system.process"   → /org/opdbus/system/process/<entity_id>
/// entity_type "identity.sled"    → /org/opdbus/identity/sled
/// entity_type "ovsdb_bridge"     → /org/opdbus/ovsdb/bridge/<entity_id>
pub fn projection_path(entity_type: &str, entity_id: &str) -> String {
    // Replace dots and underscores in type with slashes for the path prefix
    let type_path = entity_type.replace(['.', '_'], "/").to_lowercase();

    // For singleton objects (memory, cpu, filesystems, network) the entity_id
    // is typically the same as the type — omit it to avoid redundancy.
    let is_singleton = entity_id == entity_type
        || entity_id.is_empty()
        || entity_id == "memory"
        || entity_id == "cpu"
        || entity_id == "filesystems"
        || entity_id == "network"
        || entity_id == "sled";

    if is_singleton {
        format!("/org/opdbus/{}", type_path)
    } else {
        // Sanitize entity_id for use in a path segment
        let safe_id: String = entity_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        format!("/org/opdbus/{}/{}", type_path, safe_id)
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
                    .name("org.opdbus.projection")?
                    .build()
                    .await?
            }
            _ => {
                Builder::system()?
                    .name("org.opdbus.projection")?
                    .build()
                    .await?
            }
        };

        info!("D-Bus connection established for org.opdbus.projection");

        Ok(Self {
            conn,
            objects: HashMap::new(),
        })
    }

    pub async fn new_session() -> Result<Self> {
        let conn = Builder::session()?
            .name("org.opdbus.projection")?
            .build()
            .await?;

        info!("D-Bus session bus connection established for org.opdbus.projection");

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
            info!(path, entity_type = %projection.entity_type, "registered D-Bus projection object");
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
