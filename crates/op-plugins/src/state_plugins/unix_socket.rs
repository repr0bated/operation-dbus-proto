use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{Constraint, FieldSchema, FieldType, PluginSchema};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::{HashMap, HashSet};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

/// The single active-schema catalog in shared memory. Every plugin reads its
/// own slice by name from this one file — there is no per-plugin shm file and
/// no diff loop. The schema's declared state IS the desired state.
const SHM_SCHEMA_PATH: &str = "/dev/shm/live-schema.json";

fn default_protocol() -> String {
    "grpc".to_string()
}

/// A configured unix-domain socket endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketEndpoint {
    /// Filesystem path to the socket (e.g. `/run/qdrant.sock`).
    pub path: String,
    /// Local TCP port xray listens on and proxies into this socket.
    pub port: u16,
    /// Transport protocol carried over the socket (`"grpc"`, `"jsonrpc"`, …).
    #[serde(default = "default_protocol")]
    pub protocol: String,
    /// Human-readable service label used as the xray outbound tag.
    #[serde(default)]
    pub label: String,
}

/// Runtime state: all declared socket endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnixSocketState {
    /// Declared unix socket endpoints visible to internal services.
    pub sockets: Vec<SocketEndpoint>,
}

pub struct UnixSocketPlugin {
    /// Live listeners keyed by socket path. Holding the `UnixListener` keeps the
    /// socket bound for the lifetime of the plugin; dropping it tears it down.
    active: Arc<Mutex<HashMap<String, UnixListener>>>,
}

impl UnixSocketPlugin {
    pub fn new() -> Self {
        Self {
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Read this plugin's desired socket set straight from the single active
    /// schema catalog in shared memory (zero-copy 1:1). The catalog is keyed by
    /// plugin name and each entry carries the schema-declared state.
    fn read_desired() -> UnixSocketState {
        let Ok(mut bytes) = std::fs::read(SHM_SCHEMA_PATH) else {
            return UnixSocketState::default();
        };
        let Ok(catalog) = simd_json::to_owned_value(&mut bytes) else {
            return UnixSocketState::default();
        };
        let Some(state) = catalog
            .get("unix_socket")
            .and_then(|versions| versions.as_array())
            .and_then(|versions| versions.last())
            .and_then(|schema| schema.get("example"))
        else {
            return UnixSocketState::default();
        };
        simd_json::serde::from_owned_value::<UnixSocketState>(state.clone()).unwrap_or_default()
    }

    /// Bind a single endpoint, replacing any stale socket file. Returns `true`
    /// when a new listener was created.
    fn ensure_bound(
        active: &mut HashMap<String, UnixListener>,
        endpoint: &SocketEndpoint,
    ) -> Result<bool> {
        if active.contains_key(&endpoint.path) {
            return Ok(false);
        }
        if let Some(parent) = Path::new(&endpoint.path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        if Path::new(&endpoint.path).exists() {
            std::fs::remove_file(&endpoint.path)?;
        }
        let listener = UnixListener::bind(&endpoint.path)?;
        listener.set_nonblocking(true)?;
        active.insert(endpoint.path.clone(), listener);
        info!(
            socket = %endpoint.path,
            port = endpoint.port,
            protocol = %endpoint.protocol,
            "bound unix socket endpoint"
        );
        Ok(true)
    }

    /// Drop a listener and unlink its socket file. Returns `true` when removed.
    fn unbind(active: &mut HashMap<String, UnixListener>, path: &str) -> bool {
        if active.remove(path).is_some() {
            if let Err(error) = std::fs::remove_file(path) {
                warn!(socket = %path, %error, "failed to unlink unix socket file");
            }
            true
        } else {
            false
        }
    }

    /// Reconcile the live listeners to exactly the supplied desired set.
    fn reconcile(&self, desired: &UnixSocketState) -> ApplyResult {
        let mut active = self.active.lock();
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        let wanted: HashSet<String> = desired.sockets.iter().map(|s| s.path.clone()).collect();

        for endpoint in &desired.sockets {
            match Self::ensure_bound(&mut active, endpoint) {
                Ok(true) => changes_applied.push(format!(
                    "created socket {} (port {}, {})",
                    endpoint.path, endpoint.port, endpoint.protocol
                )),
                Ok(false) => {}
                Err(error) => errors.push(format!(
                    "failed to create socket {}: {}",
                    endpoint.path, error
                )),
            }
        }

        let stale: Vec<String> = active
            .keys()
            .filter(|path| !wanted.contains(*path))
            .cloned()
            .collect();
        for path in stale {
            if Self::unbind(&mut active, &path) {
                changes_applied.push(format!("removed socket {}", path));
            }
        }

        ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        }
    }
}

impl Default for UnixSocketPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for UnixSocketPlugin {
    fn name(&self) -> &str {
        "unix_socket"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(unix_socket_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        // The schema-declared desired state in the active-schema catalog is the
        // current state (1:1, zero-copy). Fall back to whatever is live-bound.
        let mut state = Self::read_desired();
        if state.sockets.is_empty() {
            let active = self.active.lock();
            state.sockets = active
                .keys()
                .map(|path| SocketEndpoint {
                    path: path.clone(),
                    port: 0,
                    protocol: default_protocol(),
                    label: String::new(),
                })
                .collect();
        }
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        // No diff loop: the schema-declared state is authoritative and applied
        // wholesale by `apply_state`.
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "schema".to_string(),
                desired_hash: "schema".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        // Reconcile the live unix sockets to exactly the schema-declared set.
        let desired = Self::read_desired();
        Ok(self.reconcile(&desired))
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        let desired = Self::read_desired();
        let active = self.active.lock();
        let satisfied = desired.sockets.iter().all(|endpoint| {
            active.contains_key(&endpoint.path) && Path::new(&endpoint.path).exists()
        });
        Ok(satisfied)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(Self::read_desired())?,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let snapshot = simd_json::serde::from_owned_value::<UnixSocketState>(
            checkpoint.state_snapshot.clone(),
        )
        .unwrap_or_default();
        self.reconcile(&snapshot);
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(dir: &std::path::Path, name: &str, port: u16) -> SocketEndpoint {
        SocketEndpoint {
            path: dir.join(name).to_string_lossy().into_owned(),
            port,
            protocol: "grpc".to_string(),
            label: name.to_string(),
        }
    }

    #[test]
    fn should_bind_and_unbind_declared_sockets() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin = UnixSocketPlugin::new();

        let ep = endpoint(dir.path(), "qdrant.sock", 6334);
        let desired = UnixSocketState {
            sockets: vec![ep.clone()],
        };

        let result = plugin.reconcile(&desired);
        assert!(result.success, "errors: {:?}", result.errors);
        assert!(std::path::Path::new(&ep.path).exists());
        assert!(plugin.active.lock().contains_key(&ep.path));

        // Reconciling to an empty set tears the socket down.
        let cleared = plugin.reconcile(&UnixSocketState::default());
        assert!(cleared.success);
        assert!(!std::path::Path::new(&ep.path).exists());
        assert!(plugin.active.lock().is_empty());
    }

    #[test]
    fn should_be_idempotent_for_already_bound_sockets() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin = UnixSocketPlugin::new();
        let desired = UnixSocketState {
            sockets: vec![
                endpoint(dir.path(), "a.sock", 1),
                endpoint(dir.path(), "b.sock", 2),
            ],
        };

        let first = plugin.reconcile(&desired);
        assert_eq!(first.changes_applied.len(), 2);

        // Second reconcile against the same desired set creates nothing new.
        let second = plugin.reconcile(&desired);
        assert!(second.changes_applied.is_empty());
        assert_eq!(plugin.active.lock().len(), 2);
    }
}

pub(crate) fn unix_socket_schema() -> PluginSchema {
    let mut socket_fields = HashMap::new();
    socket_fields.insert(
        "path".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Filesystem path of the unix domain socket".to_string(),
            default: None,
            example: Some(json!("/run/qdrant.sock")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    socket_fields.insert(
        "port".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Local TCP port xray listens on and proxies into this socket".to_string(),
            default: None,
            example: Some(json!(6334)),
            constraints: vec![
                Constraint::Min { value: 1.0 },
                Constraint::Max { value: 65535.0 },
            ],
            read_only: false,
            read_only_when: None,
        },
    );
    socket_fields.insert(
        "protocol".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Transport protocol carried over the socket (grpc, jsonrpc, …)"
                .to_string(),
            default: Some(json!("grpc")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    socket_fields.insert(
        "label".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Human-readable service label used as the xray outbound tag".to_string(),
            default: None,
            example: Some(json!("qdrant-grpc")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    PluginSchema::builder("unix_socket")
        .version("1.0.0")
        .description("Unix domain socket endpoints proxied into xray outbounds")
        .array_field(
            "sockets",
            FieldType::Object(socket_fields),
            true,
            "Declared unix socket endpoints",
        )
        .example(json!({
            "sockets": [
                {
                    "path": "/run/qdrant.sock",
                    "port": 6334,
                    "protocol": "grpc",
                    "label": "qdrant-grpc"
                },
                {
                    "path": "/run/netmaker/api.sock",
                    "port": 8081,
                    "protocol": "http",
                    "label": "netmaker-api"
                },
                {
                    "path": "/run/netmaker/mq.sock",
                    "port": 1883,
                    "protocol": "mqtt",
                    "label": "netmaker-mqtt"
                },
                {
                    "path": "/run/netmaker/mqtts.sock",
                    "port": 8883,
                    "protocol": "mqtt",
                    "label": "netmaker-mqtts"
                },
                {
                    "path": "/run/netmaker/ui.sock",
                    "port": 80,
                    "protocol": "http",
                    "label": "netmaker-ui"
                }
            ]
        }))
        .build()
}
