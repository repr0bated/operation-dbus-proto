use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::{HashMap, HashSet};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

use super::plugin_scaffold_helpers::{
    method_decl_from_schemars, method_decl_from_schemars_with_output,
};

/// The single active-schema catalog in shared memory. Every plugin reads its
/// own slice by name from this one file — there is no per-plugin shm file and
/// no diff loop. The schema's declared state IS the desired state.
fn default_protocol() -> String {
    "grpc".to_string()
}

/// A configured unix-domain socket endpoint.
///
/// In the single-socket model, one shared socket (e.g.
/// `/run/ghostbridge/container.sock`) serves all services.  xray routes by
/// domain/SNI to the tonic-web gRPC bridge, which listens on this socket and
/// demuxes by gRPC service/method (via reflection).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SocketEndpoint {
    /// Container/service identity (the `createunixsocket --name`). The tonic-web
    /// gRPC bridge demuxes by gRPC service/method via server reflection — this
    /// name is the routing/identity tag gemma emits into the xray config; it is
    /// NOT a network namespace (filesystem UDS are not netns-scoped).
    #[serde(default)]
    #[schemars(
        description = "Container/service identity tag the bridge routes by (createunixsocket --name)",
        example = &"netmaker",
        extend("x-oscal-subid" = "mut.service.unix-socket.bind-name@v1")
    )]
    pub name: String,
    /// Filesystem path to the shared socket.
    #[schemars(
        description = "Filesystem path of the shared unix domain socket",
        example = &"/run/ghostbridge/container.sock",
        extend("x-oscal-subid" = "mut.service.unix-socket.bind-path@v1")
    )]
    pub path: String,
    /// Backend TCP ports this container serves (the `createunixsocket --port`
    /// list, e.g. `25,465,587` for mail). Recorded metadata only — tonic-web
    /// distinguishes single vs multi protocol and demuxes by reflection, so the
    /// plugin neither opens these ports nor maps them; they describe the backend.
    #[serde(default)]
    #[schemars(
        description = "Backend TCP ports the container serves (metadata; tonic-web demuxes by reflection)",
        example = &[25u16, 465, 587],
        extend("x-oscal-subid" = "mut.service.unix-socket.bind-port@v1")
    )]
    pub ports: Vec<u16>,
    /// Transport protocol carried over the socket.
    #[serde(default = "default_protocol")]
    #[schemars(
        description = "Transport protocol carried over the socket (grpc)",
        extend("x-oscal-subid" = "mut.service.unix-socket.bind-protocol@v1")
    )]
    pub protocol: String,
    /// Human-readable label for the shared socket.
    #[serde(default)]
    #[schemars(
        description = "Human-readable label for the shared socket",
        example = &"ghostbridge",
        extend("x-oscal-subid" = "mut.service.unix-socket.bind-label@v1")
    )]
    pub label: String,
}

/// Canonical path for the single shared bidirectional socket every container
/// uses to reach the tonic-web gRPC bridge.
pub const SHARED_CONTAINER_SOCKET: &str = "/run/ghostbridge/container.sock";

/// Runtime state: the single shared socket endpoint for the gRPC bridge.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.unix-socket.schema@v1"))]
#[schemars(extend("x-oscal-category" = "service"))]
pub struct UnixSocketState {
    /// The shared socket endpoint (one socket, all services route through it).
    #[schemars(
        description = "The shared socket endpoint (one socket, all services route through it by domain)",
        extend("x-oscal-subid" = "mut.service.unix-socket.bind@v1")
    )]
    pub sockets: Vec<SocketEndpoint>,
}

/// Input struct for Bind method - bind a socket to a path.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BindInput {
    /// Socket name/identifier
    pub name: String,
    /// Filesystem path for the socket
    pub path: String,
    /// Backend ports
    pub ports: Vec<u16>,
    /// Protocol (grpc)
    #[serde(default)]
    pub protocol: String,
}

/// Input struct for Listen method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListenInput {
    /// Socket path to listen on
    pub path: String,
    /// Backlog size
    #[serde(default = "default_backlog")]
    pub backlog: u32,
}

/// Input struct for Accept method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AcceptInput {
    /// Socket path to accept from
    pub path: String,
    /// Timeout in seconds
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Input struct for Close method.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CloseInput {
    /// Socket path to close
    pub path: String,
    /// Force close even if connections active
    #[serde(default)]
    pub force: bool,
}

fn default_backlog() -> u32 {
    128
}

/// Canonical `unix_socket` schema, **derived** from the structs via schemars
/// (see [`super::schemars_adapter`]). `port` bounds, field descriptions (from
/// doc comments) and `required` flags all come from the type — the struct is
/// the single source of truth.
pub fn unix_socket_schema_derived() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(UnixSocketState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "unix_socket",
        "1.0.0",
        "Single shared unix-domain socket for the tonic-web gRPC bridge — all services route through it, demuxed by domain/SNI at xray and by gRPC service/method at the bridge",
        &root,
    );
    super::common::oscal::ensure_category_metadata_fields(&mut schema);
    // The createunixsocket action (mut.service.unix-socket.create-socket@v1):
    // register a container/service on the shared container.sock. D-Bus/gRPC
    // method signatures derive from the schema; this subid is the OSCAL handle.
    schema.subids.insert(
        "createunixsocket".to_string(),
        "mut.service.unix-socket.create-socket@v1".to_string(),
    );

    // Bind method - bind a socket to a path
    schema.methods.insert(
        "bind".to_string(),
        method_decl_from_schemars_with_output::<BindInput, super::plugin_scaffold_helpers::AckOutput>(
            "Bind",
            op_state_store::SideEffect::Mutation,
            false,
            "network.write",
            "mut.service.unix-socket.bind@v1",
        ),
    );

    // Listen method
    schema.methods.insert(
        "listen".to_string(),
        method_decl_from_schemars_with_output::<
            ListenInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "Listen",
            op_state_store::SideEffect::Mutation,
            false,
            "network.write",
            "mut.service.unix-socket.listen@v1",
        ),
    );

    // Accept method
    schema.methods.insert(
        "accept".to_string(),
        method_decl_from_schemars_with_output::<
            AcceptInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "Accept",
            op_state_store::SideEffect::Read,
            true,
            "network.read",
            "obs.service.unix-socket.accept@v1",
        ),
    );

    // Close method
    schema.methods.insert(
        "close".to_string(),
        method_decl_from_schemars_with_output::<
            CloseInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "Close",
            op_state_store::SideEffect::Mutation,
            false,
            "network.write",
            "mut.service.unix-socket.close@v1",
        ),
    );

    schema
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
        // The sealed blob IS this plugin: desired state is the `example`
        // carried by its own blob in the SHM catalog.
        let Some(schema) = op_blob::catalog::read_plugin_schema_shm("unix_socket") else {
            return UnixSocketState::default();
        };
        let Some(state) = schema.example else {
            return UnixSocketState::default();
        };
        simd_json::serde::from_owned_value::<UnixSocketState>(state).unwrap_or_default()
    }

    /// Bind a single endpoint when no shared transport exists. If the shared
    /// socket is already present, it is owned by the gRPC bridge and this plugin
    /// only registers the endpoint metadata.
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
            info!(
                socket = %endpoint.path,
                name = %endpoint.name,
                ports = ?endpoint.ports,
                protocol = %endpoint.protocol,
                "registered unix socket endpoint on existing shared transport"
            );
            return Ok(false);
        }
        let listener = UnixListener::bind(&endpoint.path)?;
        listener.set_nonblocking(true)?;
        active.insert(endpoint.path.clone(), listener);
        info!(
            socket = %endpoint.path,
            name = %endpoint.name,
            ports = ?endpoint.ports,
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
                    "created socket {} (name {}, ports {:?}, {})",
                    endpoint.path, endpoint.name, endpoint.ports, endpoint.protocol
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

    /// `createunixsocket --name <name> --port <csv>` — register a container/service
    /// against the single shared `container.sock`.
    ///
    /// Pure transport, no OVS: the shared socket is bound once; `name` is the
    /// identity/demux tag (gemma emits it into the xray config) and `ports` are
    /// recorded backend metadata only. tonic-web distinguishes single vs multi
    /// protocol and demuxes by gRPC server reflection, so the plugin neither
    /// opens these ports nor maps them — OSCAL rule-enforcement routing lives in
    /// the separate OpenFlow/policy layer, not in the socket.
    pub fn create_unix_socket(&self, name: impl Into<String>, ports: Vec<u16>) -> ApplyResult {
        let endpoint = SocketEndpoint {
            name: name.into(),
            path: SHARED_CONTAINER_SOCKET.to_string(),
            ports,
            protocol: default_protocol(),
            label: String::new(),
        };
        let mut active = self.active.lock();
        match Self::ensure_bound(&mut active, &endpoint) {
            Ok(_) => ApplyResult {
                success: true,
                changes_applied: vec![format!(
                    "registered '{}' on shared socket {} (ports {:?})",
                    endpoint.name, endpoint.path, endpoint.ports
                )],
                errors: vec![],
                checkpoint: None,
            },
            Err(error) => ApplyResult {
                success: false,
                changes_applied: vec![],
                errors: vec![format!(
                    "createunixsocket '{}' failed: {}",
                    endpoint.name, error
                )],
                checkpoint: None,
            },
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
        let mut schema = unix_socket_schema_derived();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
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
            name: name.to_string(),
            path: dir.join(name).to_string_lossy().into_owned(),
            ports: vec![port],
            protocol: "grpc".to_string(),
            label: name.to_string(),
        }
    }

    #[test]
    fn schema_carries_oscal_subids() {
        let raw = serde_json::to_value(schemars::schema_for!(UnixSocketState)).unwrap();
        assert_eq!(
            raw.get("x-oscal-subid").and_then(|v| v.as_str()),
            Some("sch.software.plugin.unix-socket.schema@v1")
        );

        let sockets = raw
            .get("properties")
            .and_then(|p| p.get("sockets"))
            .expect("sockets property");
        assert_eq!(
            sockets.get("x-oscal-subid").and_then(|v| v.as_str()),
            Some("mut.service.unix-socket.bind@v1")
        );

        let endpoint_def = raw
            .get("$defs")
            .or_else(|| raw.get("definitions"))
            .and_then(|d| d.get("SocketEndpoint"))
            .expect("SocketEndpoint $def");
        let props = endpoint_def
            .get("properties")
            .expect("SocketEndpoint properties");

        let expected = [
            ("name", "mut.service.unix-socket.bind-name@v1"),
            ("path", "mut.service.unix-socket.bind-path@v1"),
            ("ports", "mut.service.unix-socket.bind-port@v1"),
            ("protocol", "mut.service.unix-socket.bind-protocol@v1"),
            ("label", "mut.service.unix-socket.bind-label@v1"),
        ];
        for (field, subid) in expected {
            let actual = props
                .get(field)
                .and_then(|f| f.get("x-oscal-subid"))
                .and_then(|v| v.as_str());
            assert_eq!(actual, Some(subid), "subid mismatch for {field}");
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

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("unix_socket", |_ctx| std::sync::Arc::new(UnixSocketPlugin::new()))
}
