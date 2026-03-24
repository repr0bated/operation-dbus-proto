//! D-Bus server for system bus integration

use crate::manager::StateManager;
use crate::plugin::{StateAction, StateDiff};
use anyhow::Result;
use op_jsonrpc::nonnet::NonNetDb;
use op_jsonrpc::ovsdb::OvsdbClient;
use op_jsonrpc::protocol::JsonRpcRequest;
use quick_xml::events::Event;
use quick_xml::Reader;
use simd_json::prelude::*;
use std::collections::HashMap;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use zbus::{connection::Builder, interface, Connection};

/// D-Bus interface for the state manager
pub struct StateManagerDBus {
    state_manager: Arc<StateManager>,
    nonnet: Arc<NonNetDb>,
}

#[derive(Clone)]
struct ProjectedObject {
    origin_service: String,
    origin_path: String,
}

#[derive(Default)]
struct PublicationRegistry {
    published_paths: HashSet<String>,
    paths_by_service: HashMap<String, HashSet<String>>,
}

impl PublicationRegistry {
    fn insert(&mut self, service: &str, path: String) -> bool {
        if !self.published_paths.insert(path.clone()) {
            return false;
        }

        self.paths_by_service
            .entry(service.to_string())
            .or_default()
            .insert(path);
        true
    }

    fn remove_path(&mut self, service: &str, path: &str) {
        self.published_paths.remove(path);
        if let Some(paths) = self.paths_by_service.get_mut(service) {
            paths.remove(path);
            if paths.is_empty() {
                self.paths_by_service.remove(service);
            }
        }
    }

    fn remove_service(&mut self, service: &str) -> Vec<String> {
        let paths = self.paths_by_service.remove(service).unwrap_or_default();
        for path in &paths {
            self.published_paths.remove(path);
        }
        paths.into_iter().collect()
    }

    fn total_paths(&self) -> usize {
        self.published_paths.len()
    }
}

#[zbus::interface(name = "org.opdbus.ProjectedObjectV1")]
impl ProjectedObject {
    #[zbus(property)]
    async fn origin_service(&self) -> String {
        self.origin_service.clone()
    }

    #[zbus(property)]
    async fn origin_path(&self) -> String {
        self.origin_path.clone()
    }
}

#[zbus::interface(name = "org.opdbus.StateManager")]
impl StateManagerDBus {
    /// Apply state from JSON string
    async fn apply_openflow_state(&self, state_json: String) -> zbus::fdo::Result<String> {
        let mut state_json_mut = state_json;
        match unsafe { simd_json::from_str::<crate::manager::DesiredState>(&mut state_json_mut) } {
            Ok(desired_state) => match self.state_manager.apply_state(desired_state).await {
                Ok(report) => Ok(format!("Applied successfully: {}", report.success)),
                Err(e) => Err(zbus::fdo::Error::Failed(format!("Apply failed: {}", e))),
            },
            Err(e) => Err(zbus::fdo::Error::InvalidArgs(format!(
                "Invalid JSON: {}",
                e
            ))),
        }
    }

    /// Query current state
    async fn query_state(&self) -> zbus::fdo::Result<String> {
        match self.state_manager.query_current_state().await {
            Ok(state) => match simd_json::to_string(&state) {
                Ok(json) => Ok(json),
                Err(e) => Err(zbus::fdo::Error::Failed(format!(
                    "Serialization failed: {}",
                    e
                ))),
            },
            Err(e) => Err(zbus::fdo::Error::Failed(format!("Query failed: {}", e))),
        }
    }

    /// Apply one contract mutation routed from transport adapters.
    /// This is the canonical write ingress for strict flow mode.
    #[zbus(name = "ApplyContractMutation")]
    async fn apply_contract_mutation(&self, mutation_json: String) -> zbus::fdo::Result<String> {
        let mut mutation_json_mut = mutation_json;
        let mutation =
            unsafe { simd_json::from_str::<simd_json::OwnedValue>(&mut mutation_json_mut) }
                .map_err(|e| {
                    zbus::fdo::Error::InvalidArgs(format!("Invalid mutation JSON: {}", e))
                })?;

        let plugin_id = mutation
            .get("plugin_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs("Missing plugin_id".to_string()))?;
        let value = mutation
            .get("value")
            .cloned()
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs("Missing value".to_string()))?;

        let desired_state = crate::manager::DesiredState {
            version: 1,
            plugins: HashMap::from([(plugin_id.to_string(), value)]),
        };

        match self
            .state_manager
            .apply_state_single_plugin(desired_state, plugin_id)
            .await
        {
            Ok(report) if report.success => {
                if let Ok(current) = self.state_manager.query_current_state().await {
                    self.nonnet.load_from_plugins(&current.plugins).await;
                }
                Ok("ok".to_string())
            }
            Ok(_) => Err(zbus::fdo::Error::Failed(
                "apply_state_single_plugin returned success=false".to_string(),
            )),
            Err(e) => Err(zbus::fdo::Error::Failed(format!(
                "apply_state_single_plugin failed: {}",
                e
            ))),
        }
    }

    /// Restore OpenFlow flows from state file (used after OVS restart)
    ///
    /// # Arguments
    /// * `state_file_path` - Optional path to state file (default: /etc/op-dbus/state.json)
    /// * `bridge_name` - Optional bridge filter (empty string = all bridges)
    ///
    /// # Returns
    /// Success message with count of restored flows
    async fn restore_flows(
        &self,
        state_file_path: String,
        bridge_name: String,
    ) -> zbus::fdo::Result<String> {
        use std::path::PathBuf;

        // Handle default state file path
        let state_path = if state_file_path.is_empty() {
            PathBuf::from("/etc/op-dbus/state.json")
        } else {
            PathBuf::from(state_file_path)
        };

        // Check if state file exists
        if !state_path.exists() {
            return Err(zbus::fdo::Error::Failed(format!(
                "State file not found: {}",
                state_path.display()
            )));
        }

        // Load desired state
        let desired_state = match self.state_manager.load_desired_state(&state_path).await {
            Ok(state) => state,
            Err(e) => {
                return Err(zbus::fdo::Error::Failed(format!(
                    "Failed to load state file: {}",
                    e
                )))
            }
        };

        // Check if openflow plugin state exists
        let openflow_state = match desired_state.plugins.get("openflow") {
            Some(state) => state,
            None => {
                return Err(zbus::fdo::Error::Failed(
                    "No 'openflow' plugin configuration in state file".to_string(),
                ))
            }
        };

        // Get the openflow plugin
        let openflow_plugin = match self.state_manager.get_plugin("openflow").await {
            Some(plugin) => plugin,
            None => {
                return Err(zbus::fdo::Error::Failed(
                    "OpenFlow plugin not registered".to_string(),
                ))
            }
        };

        // Query current state
        let current_state = match openflow_plugin.query_current_state().await {
            Ok(state) => state,
            Err(e) => {
                return Err(zbus::fdo::Error::Failed(format!(
                    "Failed to query current state: {}",
                    e
                )))
            }
        };

        // Calculate diff
        let diff = match openflow_plugin
            .calculate_diff(&current_state, openflow_state)
            .await
        {
            Ok(diff) => diff,
            Err(e) => {
                return Err(zbus::fdo::Error::Failed(format!(
                    "Failed to calculate diff: {}",
                    e
                )))
            }
        };

        // Filter for flow-only actions
        let flow_actions: Vec<StateAction> = diff
            .actions
            .iter()
            .filter(|action| match action {
                StateAction::Create { resource, .. } => {
                    resource.contains("flow/") || resource.contains("flows")
                }
                _ => false,
            })
            .cloned()
            .collect();

        if flow_actions.is_empty() {
            return Ok("No flows need to be restored".to_string());
        }

        // Filter by bridge if specified
        let filtered_actions: Vec<StateAction> = if !bridge_name.is_empty() {
            flow_actions
                .into_iter()
                .filter(|action| {
                    if let StateAction::Create { resource, .. } = action {
                        resource.contains(&bridge_name)
                    } else {
                        false
                    }
                })
                .collect()
        } else {
            flow_actions
        };

        if filtered_actions.is_empty() {
            return Ok(format!("No flows to restore for bridge: {}", bridge_name));
        }

        // Create filtered diff
        let flow_count = filtered_actions.len();
        let filtered_diff = StateDiff {
            plugin: diff.plugin.clone(),
            actions: filtered_actions.clone(),
            metadata: diff.metadata.clone(),
        };

        // Apply the restoration
        match openflow_plugin.apply_state(&filtered_diff).await {
            Ok(_) => Ok(format!("Successfully restored {} flows", flow_count)),
            Err(e) => Err(zbus::fdo::Error::Failed(format!(
                "Failed to restore flows: {}",
                e
            ))),
        }
    }
}

/// D-Bus interface for direct OVS bridge/port operations via OvsdbClient
pub struct OvsdbDBus {
    ovsdb: Arc<OvsdbClient>,
}

#[zbus::interface(name = "org.opdbus.OvsdbV1")]
impl OvsdbDBus {
    /// Create an OVS bridge (with internal management port)
    async fn create_bridge(&self, name: String) -> zbus::fdo::Result<String> {
        self.ovsdb
            .create_bridge(&name)
            .await
            .map(|_| format!("Bridge '{}' created", name))
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// Delete an OVS bridge
    async fn delete_bridge(&self, name: String) -> zbus::fdo::Result<String> {
        self.ovsdb
            .delete_bridge(&name)
            .await
            .map(|_| format!("Bridge '{}' deleted", name))
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// Add a port to a bridge
    async fn add_port(&self, bridge: String, port: String) -> zbus::fdo::Result<String> {
        self.ovsdb
            .add_port(&bridge, &port)
            .await
            .map(|_| format!("Port '{}' added to bridge '{}'", port, bridge))
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// List all bridges
    async fn list_bridges(&self) -> zbus::fdo::Result<Vec<String>> {
        self.ovsdb
            .list_bridges()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// List ports on a bridge
    async fn list_ports(&self, bridge: String) -> zbus::fdo::Result<Vec<String>> {
        self.ovsdb
            .list_ports(&bridge)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// Check if a bridge exists
    async fn bridge_exists(&self, name: String) -> zbus::fdo::Result<bool> {
        self.ovsdb
            .bridge_exists(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }
}

/// D-Bus interface for native rtnetlink operations.
pub struct RtnetlinkDBus;

#[zbus::interface(name = "org.opdbus.RtnetlinkV1")]
impl RtnetlinkDBus {
    /// Bring an interface up via rtnetlink.
    async fn link_up(&self, interface: String) -> zbus::fdo::Result<String> {
        op_network::rtnetlink::link_up(&interface)
            .await
            .map(|_| format!("Interface '{}' is up", interface))
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// Bring an interface down via rtnetlink.
    async fn link_down(&self, interface: String) -> zbus::fdo::Result<String> {
        op_network::rtnetlink::link_down(&interface)
            .await
            .map(|_| format!("Interface '{}' is down", interface))
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// Add an IPv4 address to an interface via rtnetlink.
    async fn add_ipv4_address(
        &self,
        interface: String,
        address: String,
        prefix_len: u8,
    ) -> zbus::fdo::Result<String> {
        op_network::rtnetlink::add_ipv4_address(&interface, &address, prefix_len)
            .await
            .map(|_| format!("Added {}/{} to {}", address, prefix_len, interface))
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// Flush all addresses from an interface via rtnetlink.
    async fn flush_addresses(&self, interface: String) -> zbus::fdo::Result<String> {
        op_network::rtnetlink::flush_addresses(&interface)
            .await
            .map(|_| format!("Flushed addresses on {}", interface))
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// Set interface MAC address via rtnetlink.
    async fn set_mac_address(&self, interface: String, mac: String) -> zbus::fdo::Result<String> {
        op_network::rtnetlink::set_mac_address(&interface, &mac)
            .await
            .map(|_| format!("Set MAC {} on {}", mac, interface))
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// Add default route via rtnetlink.
    async fn add_default_route(
        &self,
        interface: String,
        gateway: String,
    ) -> zbus::fdo::Result<String> {
        op_network::rtnetlink::add_default_route(&interface, &gateway)
            .await
            .map(|_| format!("Added default route via {} on {}", gateway, interface))
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// Delete default route(s) via rtnetlink.
    async fn del_default_route(&self) -> zbus::fdo::Result<String> {
        op_network::rtnetlink::del_default_route()
            .await
            .map(|_| "Deleted default route(s)".to_string())
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }

    /// List interfaces discovered by rtnetlink.
    async fn list_interfaces(&self) -> zbus::fdo::Result<String> {
        let interfaces = op_network::rtnetlink::list_interfaces()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))?;
        simd_json::to_string(&interfaces).map_err(|e| zbus::fdo::Error::Failed(format!("{}", e)))
    }
}

/// D-Bus interface for direct NonNet JSON-RPC operations.
pub struct NonNetDBus {
    nonnet: Arc<NonNetDb>,
}

#[zbus::interface(name = "org.opdbus.NonNetV1")]
impl NonNetDBus {
    /// Execute a raw JSON-RPC request against NonNet.
    async fn transact(&self, request: String) -> zbus::fdo::Result<String> {
        let req: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut request.clone()) }
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let json_req: JsonRpcRequest = simd_json::serde::from_owned_value(req)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let response = self.nonnet.handle_request(json_req).await;
        Ok(simd_json::to_string(&response).unwrap_or_default())
    }

    /// Return the NonNet schema payload for OpNonNet.
    async fn get_schema(&self) -> zbus::fdo::Result<String> {
        let request = JsonRpcRequest::new("get_schema", simd_json::json!(["OpNonNet"]));
        let response = self.nonnet.handle_request(request).await;
        Ok(simd_json::to_string(&response.result).unwrap_or_default())
    }

    /// List NonNet logical databases.
    async fn list_dbs(&self) -> zbus::fdo::Result<String> {
        let request = JsonRpcRequest::new("list_dbs", simd_json::json!([]));
        let response = self.nonnet.handle_request(request).await;
        Ok(simd_json::to_string(&response.result).unwrap_or_default())
    }
}

/// Register StateManager + OvsdbV1 interfaces on an existing D-Bus connection.
///
/// Use this when the caller already owns the bus name (e.g. `org.opdbus`).
pub async fn register_on_connection(
    connection: &Connection,
    state_manager: Arc<StateManager>,
    ovsdb: Arc<OvsdbClient>,
) -> Result<()> {
    let nonnet = Arc::new(NonNetDb::new());
    match state_manager.query_current_state().await {
        Ok(current) => nonnet.load_from_plugins(&current.plugins).await,
        Err(e) => log::warn!("Failed to seed NonNet from StateManager: {}", e),
    }

    let state_iface = StateManagerDBus {
        state_manager,
        nonnet: nonnet.clone(),
    };
    let ovsdb_iface = OvsdbDBus { ovsdb };
    let rtnetlink_iface = RtnetlinkDBus;
    let nonnet_iface = NonNetDBus { nonnet };

    connection
        .object_server()
        .at("/org/opdbus/state", state_iface)
        .await?;
    connection
        .object_server()
        .at("/org/opdbus/ovsdb", ovsdb_iface)
        .await?;
    connection
        .object_server()
        .at("/org/opdbus/rtnetlink", rtnetlink_iface)
        .await?;
    connection
        .object_server()
        .at("/org/opdbus/nonnet", nonnet_iface)
        .await?;

    log::info!("Registered StateManager at /org/opdbus/state");
    log::info!("Registered OvsdbV1 at /org/opdbus/ovsdb");
    log::info!("Registered RtnetlinkV1 at /org/opdbus/rtnetlink");
    log::info!("Registered NonNetV1 at /org/opdbus/nonnet");

    Ok(())
}

/// Start a standalone D-Bus service (owns its own bus name).
///
/// Use this when running as a standalone daemon, not embedded in op-web.
pub async fn start_system_bus(
    state_manager: Arc<StateManager>,
    ovsdb: Arc<OvsdbClient>,
) -> Result<()> {
    let nonnet = Arc::new(NonNetDb::new());
    match state_manager.query_current_state().await {
        Ok(current) => nonnet.load_from_plugins(&current.plugins).await,
        Err(e) => log::warn!("Failed to seed NonNet from StateManager: {}", e),
    }

    let state_iface = StateManagerDBus {
        state_manager,
        nonnet: nonnet.clone(),
    };
    let ovsdb_iface = OvsdbDBus { ovsdb };
    let rtnetlink_iface = RtnetlinkDBus;
    let nonnet_iface = NonNetDBus { nonnet };

    let connection = Builder::system()?
        .name("org.opdbus")?
        .serve_at("/org/opdbus/state", state_iface)?
        .serve_at("/org/opdbus/ovsdb", ovsdb_iface)?
        .serve_at("/org/opdbus/rtnetlink", rtnetlink_iface)?
        .serve_at("/org/opdbus/nonnet", nonnet_iface)?
        .build()
        .await?;

    log::info!("D-Bus StateManager + OvsdbV1 + RtnetlinkV1 + NonNetV1 started on org.opdbus");

    spawn_publication_task(connection.clone());

    std::future::pending::<()>().await;
    Ok(())
}

fn spawn_publication_task(connection: Connection) {
    tokio::spawn(async move {
        let registry = Arc::new(RwLock::new(PublicationRegistry::default()));
        if let Err(e) = publish_known_services(&connection, &registry).await {
            log::warn!("Initial D-Bus service publication failed: {}", e);
        }

        let dbus = match zbus::fdo::DBusProxy::new(&connection).await {
            Ok(proxy) => proxy,
            Err(e) => {
                log::warn!("Failed to open D-Bus proxy for publication updates: {}", e);
                return;
            }
        };

        let mut owner_changes = match dbus.receive_name_owner_changed().await {
            Ok(stream) => stream,
            Err(e) => {
                log::warn!("Failed to subscribe to NameOwnerChanged: {}", e);
                return;
            }
        };

        let repair_seconds = std::env::var("OP_DBUS_PUBLICATION_REPAIR_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0);

        if let Some(repair_seconds) = repair_seconds {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(repair_seconds));

            loop {
                tokio::select! {
                    change = owner_changes.next() => {
                        match change {
                            Some(signal) => {
                                if let Err(e) = handle_name_owner_changed(&connection, &registry, signal).await {
                                    log::warn!("D-Bus publication update failed: {}", e);
                                }
                            }
                            None => break,
                        }
                    }
                    _ = interval.tick() => {
                        if let Err(e) = publish_known_services(&connection, &registry).await {
                            log::warn!("D-Bus publication repair failed: {}", e);
                        }
                    }
                }
            }
        } else {
            while let Some(signal) = owner_changes.next().await {
                if let Err(e) = handle_name_owner_changed(&connection, &registry, signal).await {
                    log::warn!("D-Bus publication update failed: {}", e);
                }
            }
        }
    });
}

async fn publish_known_services(
    connection: &Connection,
    registry: &Arc<RwLock<PublicationRegistry>>,
) -> Result<()> {
    let dbus = zbus::fdo::DBusProxy::new(connection).await?;
    let names = dbus.list_names().await?;

    let max_services = std::env::var("OP_DBUS_PROJECTION_MAX_SERVICES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(512);
    let max_total_objects = std::env::var("OP_DBUS_PROJECTION_MAX_OBJECTS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(20_000);
    let max_nodes_per_service = std::env::var("OP_DBUS_PROJECTION_MAX_NODES_PER_SERVICE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4_096);

    let mut service_count = 0usize;
    let mut published_count = 0usize;

    for name in names {
        if service_count >= max_services || published_count >= max_total_objects {
            break;
        }

        let service = name.to_string();
        if service.starts_with(':') || service == "org.opdbus" {
            continue;
        }

        service_count += 1;
        let remaining = max_total_objects.saturating_sub(published_count);
        let newly_published = publish_service_objects(
            connection,
            registry,
            &service,
            max_nodes_per_service,
            remaining,
        )
        .await?;
        published_count += newly_published;
    }

    let total_published = registry.read().await.total_paths();
    if published_count > 0 {
        log::info!(
            "D-Bus publication seed complete: services_scanned={}, new_objects={}, total_published={}",
            service_count,
            published_count,
            total_published
        );
    } else {
        log::debug!(
            "D-Bus publication seed: services_scanned={}, no new objects (total_published={})",
            service_count,
            total_published
        );
    }

    Ok(())
}

async fn handle_name_owner_changed(
    connection: &Connection,
    registry: &Arc<RwLock<PublicationRegistry>>,
    signal: zbus::fdo::NameOwnerChanged,
) -> Result<()> {
    let args = signal.args()?;
    let service = args.name().to_string();

    if service.starts_with(':') || service == "org.opdbus" {
        return Ok(());
    }

    let old_owner_present = args.old_owner().is_some();
    let new_owner_present = args.new_owner().is_some();

    if old_owner_present {
        unpublish_service_objects(connection, registry, &service).await?;
    }

    if new_owner_present {
        let max_nodes_per_service = std::env::var("OP_DBUS_PROJECTION_MAX_NODES_PER_SERVICE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(4_096);
        publish_service_objects(
            connection,
            registry,
            &service,
            max_nodes_per_service,
            usize::MAX,
        )
        .await?;
    }

    Ok(())
}

async fn publish_service_objects(
    connection: &Connection,
    registry: &Arc<RwLock<PublicationRegistry>>,
    service: &str,
    max_nodes_per_service: usize,
    max_new_objects: usize,
) -> Result<usize> {
    if max_new_objects == 0 {
        return Ok(0);
    }

    let paths = discover_service_paths(connection, service, max_nodes_per_service).await;
    let mut published = 0usize;

    for origin_path in paths {
        if published >= max_new_objects {
            break;
        }

        let published_path = map_to_projected_path(service, &origin_path);
        let should_publish = {
            let mut guard = registry.write().await;
            guard.insert(service, published_path.clone())
        };

        if !should_publish {
            continue;
        }

        let projected = ProjectedObject {
            origin_service: service.to_string(),
            origin_path: origin_path.clone(),
        };

        if let Err(e) = connection
            .object_server()
            .at(published_path.as_str(), projected)
            .await
        {
            let mut guard = registry.write().await;
            guard.remove_path(service, &published_path);
            log::debug!(
                "Failed to publish object {} from {}{}: {}",
                published_path,
                service,
                origin_path,
                e
            );
            continue;
        }

        published += 1;
    }

    if published > 0 {
        log::debug!(
            "Published {} D-Bus objects for service {}",
            published,
            service
        );
    }

    Ok(published)
}

async fn unpublish_service_objects(
    connection: &Connection,
    registry: &Arc<RwLock<PublicationRegistry>>,
    service: &str,
) -> Result<()> {
    let paths = {
        let mut guard = registry.write().await;
        guard.remove_service(service)
    };

    for path in &paths {
        if let Err(e) = connection
            .object_server()
            .remove::<ProjectedObject, _>(path.as_str())
            .await
        {
            log::debug!("Failed to unpublish object {} for {}: {}", path, service, e);
        }
    }

    if !paths.is_empty() {
        log::debug!(
            "Unpublished {} D-Bus objects for service {}",
            paths.len(),
            service
        );
    }

    Ok(())
}

async fn discover_service_paths(
    connection: &Connection,
    service: &str,
    max_nodes: usize,
) -> Vec<String> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut discovered = Vec::new();
    let mut object_manager_hits = 0usize;
    let candidate_paths = candidate_object_manager_paths(service);

    for candidate in &candidate_paths {
        if visited.len() >= max_nodes {
            break;
        }
        let proxy = match zbus::Proxy::new(
            connection,
            service,
            candidate.as_str(),
            "org.freedesktop.DBus.ObjectManager",
        )
        .await
        {
            Ok(proxy) => proxy,
            Err(_) => continue,
        };

        type ManagedMap = std::collections::HashMap<
            zbus::zvariant::OwnedObjectPath,
            std::collections::HashMap<
                String,
                std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
            >,
        >;

        if let Ok(objects) = proxy
            .call::<_, _, ManagedMap>("GetManagedObjects", &())
            .await
        {
            for object_path in objects.keys() {
                if visited.len() >= max_nodes {
                    break;
                }
                let p = object_path.as_str().to_string();
                if visited.insert(p.clone()) {
                    discovered.push(p);
                    object_manager_hits += 1;
                }
            }
        }
    }

    // Seed introspection from common root paths for services that do not expose "/".
    for seed in candidate_paths {
        queue.push_back((seed, 0usize));
    }

    while let Some((path, depth)) = queue.pop_front() {
        if visited.len() >= max_nodes || depth > 24 {
            break;
        }
        if !visited.insert(path.clone()) {
            continue;
        }

        discovered.push(path.clone());

        let proxy = match zbus::fdo::IntrospectableProxy::builder(connection)
            .destination(service)
            .and_then(|b| b.path(path.as_str()))
        {
            Ok(builder) => match builder.build().await {
                Ok(p) => p,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        let xml = match proxy.introspect().await {
            Ok(xml) => xml,
            Err(_) => continue,
        };

        for child in parse_child_nodes(&xml, &path) {
            if !visited.contains(&child) {
                queue.push_back((child, depth + 1));
            }
        }
    }

    if object_manager_hits > 0 {
        log::debug!(
            "ObjectManager discovered {} object paths for service {}",
            object_manager_hits,
            service
        );
    }

    discovered
}

fn parse_child_nodes(xml: &str, parent_path: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut children = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                if e.name().as_ref() == b"node" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"name" {
                            let child = String::from_utf8_lossy(&attr.value).to_string();
                            if child.is_empty() {
                                continue;
                            }
                            let full = if child.starts_with('/') {
                                child
                            } else if parent_path == "/" {
                                format!("/{}", child)
                            } else {
                                format!("{}/{}", parent_path, child)
                            };
                            children.push(full);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        buf.clear();
    }

    children
}

fn map_to_projected_path(service: &str, origin_path: &str) -> String {
    let mut sanitized = String::with_capacity(service.len());
    for ch in service.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }

    if origin_path == "/" {
        format!("/org/opdbus/projected/{}/root", sanitized)
    } else {
        format!("/org/opdbus/projected/{}{}", sanitized, origin_path)
    }
}

fn candidate_object_manager_paths(service: &str) -> Vec<String> {
    let mut paths = vec!["/".to_string()];
    let mut current = String::new();

    for part in service.split('.') {
        if part.is_empty() {
            continue;
        }
        current.push('/');
        current.push_str(part);
        if !paths.contains(&current) {
            paths.push(current.clone());
        }
    }

    paths
}
