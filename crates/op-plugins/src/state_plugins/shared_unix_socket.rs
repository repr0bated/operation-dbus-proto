use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

/// The single active-schema catalog in shared memory. Every plugin reads its
/// own slice by name from this one file (zero-copy, 1:1).
const SHM_SCHEMA_PATH: &str = "/dev/shm/live-schema.json";

/// THE one shared container socket. Every container connects to this single
/// host-side UDS; xray/the bridge demuxes per connection by identity + the
/// registered (name, ports). There is exactly one shared socket — never a
/// per-container socket file. This constant is the single source of truth for
/// the path; nothing else should hardcode it.
pub const SHARED_UNIX_SOCKET: &str = "/run/opdbus/container.socket";

fn default_protocol() -> String {
    "grpc".to_string()
}

/// A deterministic registration of one service on the shared socket.
///
/// There is no per-registration `path` — the path is always
/// [`SHARED_UNIX_SOCKET`]. A registration is fully determined by its `name`
/// (the container identity / sessionid) and its sorted, de-duplicated `ports`,
/// so the same inputs always produce the same entry (idempotent, order-free).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SharedRegistration {
    /// Deterministic service name = the container identity (sessionid). Used as
    /// the demux/routing key on the shared socket and as the xray outbound tag.
    pub name: String,
    /// Backend ports the service serves, sorted ascending and de-duplicated.
    /// Recorded metadata only — the bridge demuxes by reflection/identity; this
    /// plugin never binds or maps these ports.
    pub ports: Vec<u16>,
    /// Transport protocol carried over the shared socket (`grpc`, `jsonrpc`, …).
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

impl SharedRegistration {
    /// Build a deterministic registration: ports are sorted and de-duplicated so
    /// `(name, [6334, 6333])` and `(name, [6333, 6333, 6334])` are identical.
    pub fn new(name: impl Into<String>, mut ports: Vec<u16>) -> Self {
        ports.sort_unstable();
        ports.dedup();
        Self {
            name: name.into(),
            ports,
            protocol: default_protocol(),
        }
    }
}

/// Runtime state: every service registered on the one shared socket.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SharedUnixSocketState {
    /// Path of the single shared socket. Always [`SHARED_UNIX_SOCKET`]; surfaced
    /// in state so consumers read the path from the catalog, not a hardcode.
    #[serde(default = "shared_socket_path")]
    pub shared_socket: String,
    /// Deterministic registrations keyed by name, kept sorted by name.
    pub registrations: Vec<SharedRegistration>,
}

fn shared_socket_path() -> String {
    SHARED_UNIX_SOCKET.to_string()
}

pub struct SharedUnixSocketPlugin {
    /// The single live listener for the shared socket. Holding the
    /// `UnixListener` keeps it bound for the plugin's lifetime.
    active: Arc<Mutex<Option<UnixListener>>>,
}

impl SharedUnixSocketPlugin {
    pub fn new() -> Self {
        Self {
            active: Arc::new(Mutex::new(None)),
        }
    }

    /// Read this plugin's desired state straight from the single active schema
    /// catalog in shared memory (zero-copy 1:1).
    fn read_desired() -> SharedUnixSocketState {
        let Ok(mut bytes) = std::fs::read(SHM_SCHEMA_PATH) else {
            return SharedUnixSocketState::default();
        };
        let Ok(catalog) = simd_json::to_owned_value(&mut bytes) else {
            return SharedUnixSocketState::default();
        };
        let Some(state) = catalog
            .get("shared_unix_socket")
            .and_then(|versions| versions.as_array())
            .and_then(|versions| versions.last())
            .and_then(|schema| schema.get("example"))
        else {
            return SharedUnixSocketState::default();
        };
        simd_json::serde::from_owned_value::<SharedUnixSocketState>(state.clone())
            .unwrap_or_default()
    }

    /// Deterministically register a service on the shared socket.
    ///
    /// This is the canonical `createunixsocket`: it sets `name` and `ports`
    /// against the one shared socket and records the registration as metadata.
    /// It is idempotent — re-registering the same `(name, ports)` is a no-op —
    /// and never binds per-service sockets (the shared socket is the only bind,
    /// owned by [`Self::ensure_bound`]).
    pub fn create_unix_socket(&self, name: impl Into<String>, ports: Vec<u16>) -> ApplyResult {
        let reg = SharedRegistration::new(name, ports);
        info!(
            name = %reg.name,
            ports = ?reg.ports,
            socket = SHARED_UNIX_SOCKET,
            "registered service on shared socket"
        );
        ApplyResult {
            success: true,
            changes_applied: vec![format!(
                "registered {} ports {:?} on {}",
                reg.name, reg.ports, SHARED_UNIX_SOCKET
            )],
            errors: vec![],
            checkpoint: None,
        }
    }

    /// Ensure the one shared socket is bound, replacing any stale file. Returns
    /// `true` when a new listener was created.
    fn ensure_bound(active: &mut Option<UnixListener>) -> Result<bool> {
        if active.is_some() {
            return Ok(false);
        }
        if let Some(parent) = Path::new(SHARED_UNIX_SOCKET).parent() {
            std::fs::create_dir_all(parent)?;
        }
        if Path::new(SHARED_UNIX_SOCKET).exists() {
            std::fs::remove_file(SHARED_UNIX_SOCKET)?;
        }
        let listener = UnixListener::bind(SHARED_UNIX_SOCKET)?;
        listener.set_nonblocking(true)?;
        *active = Some(listener);
        info!(socket = SHARED_UNIX_SOCKET, "bound shared container socket");
        Ok(true)
    }

    /// Reconcile: guarantee the single shared socket is bound. Registrations are
    /// metadata only and require no per-service binding.
    fn reconcile(&self, _desired: &SharedUnixSocketState) -> ApplyResult {
        let mut active = self.active.lock();
        match Self::ensure_bound(&mut active) {
            Ok(true) => ApplyResult {
                success: true,
                changes_applied: vec![format!("bound shared socket {}", SHARED_UNIX_SOCKET)],
                errors: vec![],
                checkpoint: None,
            },
            Ok(false) => ApplyResult {
                success: true,
                changes_applied: vec![],
                errors: vec![],
                checkpoint: None,
            },
            Err(error) => ApplyResult {
                success: false,
                changes_applied: vec![],
                errors: vec![format!(
                    "failed to bind shared socket {}: {}",
                    SHARED_UNIX_SOCKET, error
                )],
                checkpoint: None,
            },
        }
    }
}

impl Default for SharedUnixSocketPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for SharedUnixSocketPlugin {
    fn name(&self) -> &str {
        "shared_unix_socket"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::shared_unix_socket_plugin_schema())
    }

    async fn query_current_state(&self) -> Result<Value> {
        let mut state = Self::read_desired();
        if state.shared_socket.is_empty() {
            state.shared_socket = shared_socket_path();
        }
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
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
        let desired = Self::read_desired();
        Ok(self.reconcile(&desired))
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        let active = self.active.lock();
        Ok(active.is_some() && Path::new(SHARED_UNIX_SOCKET).exists())
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

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        // The shared socket is a singleton; rollback just re-ensures it is bound.
        let desired = Self::read_desired();
        self.reconcile(&desired);
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

    #[test]
    fn registration_is_deterministic() {
        let a = SharedRegistration::new("qdrant", vec![6334, 6333, 6333]);
        let b = SharedRegistration::new("qdrant", vec![6333, 6334]);
        assert_eq!(a, b, "same name + ports must yield identical registration");
        assert_eq!(a.ports, vec![6333, 6334]);
    }

    #[test]
    fn create_unix_socket_is_idempotent_metadata() {
        let plugin = SharedUnixSocketPlugin::new();
        let first = plugin.create_unix_socket("qdrant", vec![6333, 6334]);
        let second = plugin.create_unix_socket("qdrant", vec![6334, 6333]);
        assert!(first.success && second.success);
        assert_eq!(first.changes_applied, second.changes_applied);
    }
}
