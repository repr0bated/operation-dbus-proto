//! D-Bus proxies for the op-openvswitch-daemon
//!
//! These zbus proxy types allow any crate in the workspace to call the
//! hypervisor daemon through D-Bus instead of directly linking rovs_ovsdb
//! or shelling out to ovs-vsctl / ovs-ofctl.
//!
//! Locked design (AGENTS.md §4):
//! - Daemon paths: `/org/opdbus/rovs/jsonrpc` and `/org/opdbus/rovs/openflow`
//! - Interfaces: `org.opdbus.rovs.jsonrpc` and `org.opdbus.rovs.openflow`
//! - The daemon is a pure passthrough; business logic stays in the plugins.

use anyhow::{Context, Result};
use zbus::{proxy, Connection};

// ── RovsJsonRpcProxy ──────────────────────────────────────────────────────────

/// Proxy for the OVSDB JSON-RPC passthrough interface.
///
/// D-Bus destination: `org.opdbus.v1.plugins.ovsdb`
/// Object path: `/org/opdbus/rovs/jsonrpc`
/// Interface: `org.opdbus.rovs.jsonrpc`
#[proxy(
    default_service = "org.opdbus.v1.plugins.ovsdb",
    default_path = "/org/opdbus/rovs/jsonrpc",
    interface = "org.opdbus.rovs.jsonrpc"
)]
pub trait RovsJsonRpc {
    /// Execute a raw OVSDB JSON-RPC `transact`.
    ///
    /// `method` is the JSON-RPC method name (e.g. `"transact"`).
    /// `params_json` is the JSON-encoded parameter array.
    /// Returns the JSON-encoded result.
    async fn transact(&self, method: &str, params_json: &str) -> zbus::Result<String>;

    /// Execute a one-way OVSDB JSON-RPC `notify`.
    async fn notify(&self, method: &str, params_json: &str) -> zbus::Result<()>;

    /// Return the next JSON-RPC request id.
    async fn next_id(&self) -> zbus::Result<u64>;

    /// Send a raw JSON-RPC message string and return the response string.
    async fn send_message(&self, msg: &str) -> zbus::Result<String>;

    /// Receive a raw JSON-RPC message string.
    async fn recv_message(&self) -> zbus::Result<String>;

    /// Poll whether notification queue has pending items.
    async fn has_pending_notifications(&self) -> zbus::Result<bool>;

    /// Return the count of pending notifications.
    async fn pending_notification_count(&self) -> zbus::Result<u64>;

    /// Pop one notification from the queue as a JSON string.
    async fn pop_notification(&self) -> zbus::Result<String>;

    /// Drain all pending notifications as a JSON array string.
    async fn drain_notifications(&self) -> zbus::Result<String>;

    /// Open a new named stream / monitor.
    async fn new_stream(&self, stream: &str) -> zbus::Result<String>;

    /// Daemon status JSON.
    async fn status(&self) -> zbus::Result<String>;
}

/// Convenience constructor: build a `RovsJsonRpcProxy` on the system bus.
pub async fn jsonrpc_proxy() -> Result<RovsJsonRpcProxy<'static>> {
    let conn = Connection::system()
        .await
        .context("connect to system D-Bus for RovsJsonRpcProxy")?;
    Ok(RovsJsonRpcProxy::new(&conn).await?)
}

// ── RovsOpenFlowProxy ─────────────────────────────────────────────────────────

/// Proxy for the OpenFlow passthrough interface.
///
/// D-Bus destination: `org.opdbus.of_controller`
/// Object path: `/org/opdbus/rovs/openflow`
/// Interface: `org.opdbus.rovs.openflow`
#[proxy(
    default_service = "org.opdbus.of_controller",
    default_path = "/org/opdbus/rovs/openflow",
    interface = "org.opdbus.rovs.openflow"
)]
pub trait RovsOpenFlow {
    /// Connect to a switch at `addr` (e.g. `"tcp:127.0.0.1:6653"`).
    /// Returns connection handle id or error JSON.
    async fn connect(&self, addr: &str) -> zbus::Result<String>;

    /// Return negotiated OpenFlow version JSON.
    async fn version(&self) -> zbus::Result<String>;

    /// Send a flow_mod. `flow_json` is a JSON-encoded Flow struct.
    async fn send_flow(&self, flow_json: &str) -> zbus::Result<String>;

    /// Send a flow_mod and wait for barrier reply.
    async fn send_flow_sync(&self, flow_json: &str) -> zbus::Result<String>;

    /// Raw `ovs-ofctl` passthrough (temporary until pure OpenFlow binary is wired).
    /// `bridge` is the bridge name, `args_json` is a JSON array of extra CLI args.
    async fn ofctl(&self, bridge: &str, args_json: &str) -> zbus::Result<String>;

    /// Send an echo request, return echo reply JSON.
    async fn echo(&self) -> zbus::Result<String>;

    /// Send a barrier request, return barrier reply JSON.
    async fn barrier(&self) -> zbus::Result<String>;

    /// Dump all flows. Returns JSON array of FlowStatsEntry.
    async fn dump_flows(&self) -> zbus::Result<Vec<String>>;

    /// Dump flows matching a filter request JSON.
    async fn dump_flows_filtered(&self, request: &str) -> zbus::Result<Vec<String>>;

    /// Block until a PacketIn message arrives. Returns JSON PacketIn.
    async fn recv_packet_in(&self) -> zbus::Result<String>;

    /// Non-blocking try-receive PacketIn. Returns JSON or empty string.
    async fn try_recv_packet_in(&self) -> zbus::Result<String>;

    /// Start flow monitor with request JSON. Returns initial updates.
    async fn monitor_flows(&self, request: &str) -> zbus::Result<Vec<String>>;

    /// Block until flow updates arrive. Returns JSON array of FlowUpdate.
    async fn recv_flow_updates(&self) -> zbus::Result<Vec<String>>;

    /// Send a packet_out. `packet_out_json` is JSON-encoded PacketOut.
    async fn send_packet_out(&self, packet_out_json: &str) -> zbus::Result<String>;

    /// Controller status JSON.
    async fn status(&self) -> zbus::Result<String>;
}

/// Convenience constructor: build a `RovsOpenFlowProxy` on the system bus.
pub async fn openflow_proxy() -> Result<RovsOpenFlowProxy<'static>> {
    let conn = Connection::system()
        .await
        .context("connect to system D-Bus for RovsOpenFlowProxy")?;
    Ok(RovsOpenFlowProxy::new(&conn).await?)
}

// ── Unified helper ────────────────────────────────────────────────────────────

/// Ensure the op-openvswitch-daemon is reachable on D-Bus before proceeding.
///
/// This is the preferred entry-point for plugins: call this, then use the
/// returned proxies instead of `OvsdbClient` or `Command::new("ovs-vsctl")`.
pub async fn ensure_proxies() -> Result<(RovsJsonRpcProxy<'static>, RovsOpenFlowProxy<'static>)> {
    let json = jsonrpc_proxy().await?;
    let of = openflow_proxy().await?;
    Ok((json, of))
}

// ── OvsdbDbusClient (Compatibility Wrapper) ──────────────────────────────────

use crate::ovsdb::OvsdbClient;
use serde_json::Value;
use simd_json::OwnedValue as SimdValue;

/// Legacy compatibility wrapper around `OvsdbClient`.
/// Routes requests through the D-Bus daemon.
#[derive(Clone)]
pub struct OvsdbDbusClient {
    inner: OvsdbClient,
}

impl Default for OvsdbDbusClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OvsdbDbusClient {
    /// Synchronous constructor.
    pub fn new() -> Self {
        Self {
            inner: OvsdbClient::new(),
        }
    }

    /// Return the list of databases.
    pub async fn list_dbs(&self) -> Result<Vec<String>> {
        self.inner.list_dbs().await
    }

    /// Check if a bridge exists.
    pub async fn bridge_exists(&self, bridge_name: &str) -> Result<bool> {
        self.inner.bridge_exists(bridge_name).await
    }

    /// List all bridges.
    pub async fn list_bridges(&self) -> Result<Vec<String>> {
        self.inner.list_bridges().await
    }

    /// List all ports attached to a bridge.
    pub async fn list_bridge_ports(&self, bridge_name: &str) -> Result<Vec<String>> {
        self.inner.list_bridge_ports(bridge_name).await
    }

    /// Get bridge details as a JSON-encoded String.
    pub async fn get_bridge_info(&self, bridge_name: &str) -> Result<String> {
        let val = self.inner.get_bridge_info(bridge_name).await?;
        Ok(serde_json::to_string_pretty(&val)?)
    }

    /// Create a new OVS bridge.
    pub async fn create_bridge(&self, bridge_name: &str) -> Result<()> {
        self.inner.create_bridge(bridge_name).await
    }

    /// Delete an OVS bridge.
    pub async fn delete_bridge(&self, bridge_name: &str) -> Result<()> {
        self.inner.delete_bridge(bridge_name).await
    }

    /// Add a port to an OVS bridge.
    pub async fn add_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        self.inner.add_port(bridge_name, port_name).await
    }

    /// Set interface type.
    pub async fn set_interface_type(&self, iface_name: &str, iface_type: &str) -> Result<()> {
        self.inner.set_interface_type(iface_name, iface_type).await
    }

    /// Set a property on a bridge.
    pub async fn set_bridge_property(
        &self,
        bridge_name: &str,
        property: &str,
        value: &str,
    ) -> Result<()> {
        self.inner
            .set_bridge_property(bridge_name, property, value)
            .await
    }

    /// Set an option on an interface.
    pub async fn set_interface_option(
        &self,
        bridge_name: &str,
        iface_name: &str,
        option_name: &str,
        option_value: &str,
    ) -> Result<()> {
        self.inner
            .set_interface_option(bridge_name, iface_name, option_name, option_value)
            .await
    }

    /// Delete a port from a bridge.
    pub async fn delete_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        self.inner.delete_port(bridge_name, port_name).await
    }

    /// Add a port with specific type to a bridge.
    pub async fn add_port_with_type(
        &self,
        bridge_name: &str,
        port_name: &str,
        port_type: Option<&str>,
    ) -> Result<()> {
        self.inner
            .add_port_with_type(bridge_name, port_name, port_type)
            .await
    }

    /// Dump database contents as JSON Value.
    pub async fn dump_db(&self, database: &str) -> Result<Value> {
        self.inner.dump_db(database).await
    }

    /// Monitor updates to a database.
    pub async fn monitor_db(
        &self,
        database: &str,
    ) -> Result<tokio::sync::broadcast::Receiver<Value>> {
        let mut mpsc_rx = self.inner.monitor_db(database).await?;
        let (tx, rx) = tokio::sync::broadcast::channel(128);
        tokio::spawn(async move {
            while let Some(val) = mpsc_rx.recv().await {
                if tx.send(val).is_err() {
                    break;
                }
            }
        });
        Ok(rx)
    }

    /// Execute transaction with simd_json Value (compatibility layer).
    pub async fn transact_simd(&self, operations: SimdValue) -> Result<Value> {
        self.inner.transact_simd("Open_vSwitch", operations).await
    }

    /// Execute transaction.
    pub async fn transact(&self, operations: Vec<Value>) -> Result<Value> {
        self.inner
            .transact("Open_vSwitch", Value::Array(operations))
            .await
    }
}
