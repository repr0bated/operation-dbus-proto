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
use std::sync::Arc;
use zbus::{proxy, Connection};

// ── RovsJsonRpcProxy ──────────────────────────────────────────────────────────

/// Proxy for the OVSDB JSON-RPC passthrough interface.
///
/// D-Bus destination: `org.opdbus.v1`
/// Object path: `/org/opdbus/rovs/jsonrpc`
/// Interface: `org.opdbus.rovs.jsonrpc`
#[proxy(
    default_service = "org.opdbus.v1",
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
/// D-Bus destination: `org.opdbus.v1`
/// Object path: `/org/opdbus/rovs/openflow`
/// Interface: `org.opdbus.rovs.openflow`
#[proxy(
    default_service = "org.opdbus.v1",
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

// ── OvsdbDbusClient ─────────────────────────────────────────────────────────

/// High-level OVSDB client that routes through the D-Bus daemon.
///
/// Mirrors the `OvsdbClient` API so plugin migrations are mechanical.
///
/// Construction is **synchronous** (like the original `OvsdbClient`) — the
/// underlying D-Bus connection is established lazily on the first async call.
/// This lets `StatePlugin::new()` stay sync.
#[derive(Clone)]
pub struct OvsdbDbusClient {
    proxy: Arc<tokio::sync::OnceCell<RovsJsonRpcProxy<'static>>>,
}

impl Default for OvsdbDbusClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OvsdbDbusClient {
    /// Synchronous constructor — D-Bus connection is deferred until first use.
    pub fn new() -> Self {
        Self {
            proxy: Arc::new(tokio::sync::OnceCell::new()),
        }
    }

    /// Internal: get or create the proxy.
    async fn get_proxy(&self) -> Result<&RovsJsonRpcProxy<'static>> {
        self.proxy
            .get_or_try_init(|| async { jsonrpc_proxy().await })
            .await
            .context("connect to op-openvswitch-daemon via D-Bus")
    }

    // ── Internal: build & send a transact ───────────────────────────────

    async fn transact_one(&self, op: serde_json::Value) -> Result<serde_json::Value> {
        let proxy = self.get_proxy().await?;
        let params = serde_json::json!(["Open_vSwitch", op]);
        let raw = proxy
            .transact("transact", &params.to_string())
            .await
            .context("D-Bus transact call failed")?;
        let val: serde_json::Value = serde_json::from_str(&raw)
            .with_context(|| format!("daemon returned invalid JSON: {}", raw))?;
        Ok(val)
    }

    async fn transact_many(&self, ops: Vec<serde_json::Value>) -> Result<serde_json::Value> {
        let proxy = self.get_proxy().await?;
        let params = serde_json::json!(["Open_vSwitch", ops]);
        let raw = proxy
            .transact("transact", &params.to_string())
            .await
            .context("D-Bus transact call failed")?;
        let val: serde_json::Value = serde_json::from_str(&raw)
            .with_context(|| format!("daemon returned invalid JSON: {}", raw))?;
        Ok(val)
    }

    // ── Read helpers ──────────────────────────────────────────────────────

    /// Return `true` if the daemon (and OVSDB) is reachable.
    pub async fn list_dbs(&self) -> Result<Vec<String>> {
        let proxy = self.get_proxy().await?;
        let raw = proxy
            .transact("list_dbs", "[]")
            .await
            .context("list_dbs D-Bus call failed")?;
        let val: serde_json::Value = serde_json::from_str(&raw)?;
        Ok(val
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect())
    }

    /// Return `true` if a bridge with the given name exists.
    pub async fn bridge_exists(&self, bridge_name: &str) -> Result<bool> {
        let result = self
            .transact_one(serde_json::json!({
                "op": "select",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "columns": ["_uuid"]
            }))
            .await?;
        Ok(result
            .get("rows")
            .and_then(|r| r.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false))
    }

    /// Return the names of all bridges.
    pub async fn list_bridges(&self) -> Result<Vec<String>> {
        let result = self
            .transact_one(serde_json::json!({
                "op": "select",
                "table": "Bridge",
                "where": [],
                "columns": ["name"]
            }))
            .await?;
        Ok(result
            .get("rows")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|r| r.get("name").and_then(|v| v.as_str()).map(String::from))
            .collect())
    }

    /// Return the names of all ports on a bridge.
    pub async fn list_bridge_ports(&self, bridge_name: &str) -> Result<Vec<String>> {
        let bridge_result = self
            .transact_one(serde_json::json!({
                "op": "select",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "columns": ["ports"]
            }))
            .await?;
        let bridge_rows = bridge_result.get("rows").and_then(|r| r.as_array());
        let port_uuids: Vec<String> = match bridge_rows {
            Some(rows) if !rows.is_empty() => {
                let mut uuids = Vec::new();
                if let Some(ports) = rows[0].get("ports") {
                    Self::collect_uuids(ports, &mut uuids);
                }
                uuids
            }
            _ => return Ok(Vec::new()),
        };

        let mut names = Vec::new();
        for uuid in port_uuids {
            let result = self
                .transact_one(serde_json::json!({
                    "op": "select",
                    "table": "Port",
                    "where": [["_uuid", "==", ["uuid", uuid]]],
                    "columns": ["name"]
                }))
                .await?;
            if let Some(row) = result.get("rows").and_then(|r| r.as_array()).and_then(|a| a.first())
            {
                if let Some(n) = row.get("name").and_then(|v| v.as_str()) {
                    names.push(n.to_string());
                }
            }
        }
        Ok(names)
    }

    /// Return the raw JSON row for a bridge.
    pub async fn get_bridge_info(&self, bridge_name: &str) -> Result<String> {
        let result = self
            .transact_one(serde_json::json!({
                "op": "select",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "columns": []
            }))
            .await?;
        Ok(serde_json::to_string_pretty(&result)?)
    }

    // ── Mutation helpers ──────────────────────────────────────────────────

    /// Create a bridge if it does not exist.
    pub async fn create_bridge(&self, bridge_name: &str) -> Result<()> {
        if self.bridge_exists(bridge_name).await? {
            log::info!("Bridge {} already exists, skipping creation", bridge_name);
            return Ok(());
        }
        let ops = vec![
            serde_json::json!({
                "op": "insert",
                "table": "Bridge",
                "row": { "name": bridge_name, "stp_enable": false },
                "uuid-name": "new_bridge"
            }),
            serde_json::json!({
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [],
                "mutations": [["bridges", "insert", ["named-uuid", "new_bridge"]]]
            }),
        ];
        let result = self.transact_many(ops).await?;
        Self::check_errors(&result)?;
        log::info!("Bridge {} created via D-Bus daemon", bridge_name);
        Ok(())
    }

    /// Delete a bridge and its ports/interfaces.
    pub async fn delete_bridge(&self, bridge_name: &str) -> Result<()> {
        let bridge_result = self
            .transact_one(serde_json::json!({
                "op": "select",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "columns": ["_uuid", "ports"]
            }))
            .await?;
        let bridge_rows = bridge_result.get("rows").and_then(|r| r.as_array());
        let (bridge_uuid, bridge_row) = match bridge_rows {
            Some(rows) if !rows.is_empty() => {
                let uuid = rows[0].get("_uuid")
                    .and_then(|u| u.as_array())
                    .and_then(|a| a.get(1))
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .ok_or_else(|| anyhow::anyhow!("bridge UUID not found"))?;
                (uuid, &rows[0])
            }
            _ => return Err(anyhow::anyhow!("Bridge '{}' not found", bridge_name)),
        };

        let mut port_uuids: Vec<String> = Vec::new();
        if let Some(ports) = bridge_row.get("ports") {
            Self::collect_uuids(ports, &mut port_uuids);
        }

        let mut iface_uuids: Vec<String> = Vec::new();
        for port_uuid in &port_uuids {
            let port_result = self
                .transact_one(serde_json::json!({
                    "op": "select",
                    "table": "Port",
                    "where": [["_uuid", "==", ["uuid", port_uuid.clone()]]],
                    "columns": ["interfaces"]
                }))
                .await?;
            if let Some(rows) = port_result.get("rows").and_then(|r| r.as_array()) {
                if let Some(row) = rows.first() {
                    if let Some(ifaces) = row.get("interfaces") {
                        Self::collect_uuids(ifaces, &mut iface_uuids);
                    }
                }
            }
        }

        let mut ops = vec![
            serde_json::json!({
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [],
                "mutations": [["bridges", "delete", ["uuid", bridge_uuid.clone()]]]
            }),
            serde_json::json!({
                "op": "delete",
                "table": "Bridge",
                "where": [["_uuid", "==", ["uuid", bridge_uuid]]]
            }),
        ];
        for port_uuid in &port_uuids {
            ops.push(serde_json::json!({
                "op": "delete",
                "table": "Port",
                "where": [["_uuid", "==", ["uuid", port_uuid]]]
            }));
        }
        for iface_uuid in &iface_uuids {
            ops.push(serde_json::json!({
                "op": "delete",
                "table": "Interface",
                "where": [["_uuid", "==", ["uuid", iface_uuid]]]
            }));
        }

        let result = self.transact_many(ops).await?;
        Self::check_errors(&result)?;
        log::info!("Bridge {} deleted via D-Bus daemon", bridge_name);
        Ok(())
    }

    /// Add a system port to a bridge.
    pub async fn add_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        let ops = vec![
            serde_json::json!({
                "op": "insert",
                "table": "Interface",
                "row": { "name": port_name, "type": "system" },
                "uuid-name": "new_iface"
            }),
            serde_json::json!({
                "op": "insert",
                "table": "Port",
                "row": {
                    "name": port_name,
                    "interfaces": ["set", [["named-uuid", "new_iface"]]]
                },
                "uuid-name": "new_port"
            }),
            serde_json::json!({
                "op": "mutate",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "mutations": [["ports", "insert", ["named-uuid", "new_port"]]]
            }),
        ];
        let result = self.transact_many(ops).await?;
        Self::check_errors(&result)?;
        log::info!("Port {} added to bridge {} via D-Bus daemon", port_name, bridge_name);
        Ok(())
    }

    // ── Private helpers ───────────────────────────────────────────────────

    /// Recursively collect UUID strings from an OVSDB set value.
    fn collect_uuids(value: &serde_json::Value, out: &mut Vec<String>) {
        if let Some(arr) = value.as_array() {
            if arr.len() == 2 {
                if arr[0] == "uuid" {
                    if let Some(s) = arr[1].as_str() {
                        out.push(s.to_string());
                    }
                } else if arr[0] == "set" {
                    if let Some(items) = arr[1].as_array() {
                        for item in items {
                            Self::collect_uuids(item, out);
                        }
                    }
                }
            }
        }
    }

    /// Check a transact result for per-operation error objects.
    fn check_errors(result: &serde_json::Value) -> Result<()> {
        if let Some(results) = result.as_array() {
            for (i, op_result) in results.iter().enumerate() {
                if let Some(error) = op_result.get("error") {
                    if !error.is_null() {
                        let details = op_result
                            .get("details")
                            .and_then(|d| d.as_str())
                            .unwrap_or("no details");
                        return Err(anyhow::anyhow!(
                            "OVSDB operation {} failed: {} ({})",
                            i,
                            error,
                            details
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Set the `type` column on an Interface row (e.g. "internal", "system").
    pub async fn set_interface_type(&self, iface_name: &str, iface_type: &str) -> Result<()> {
        let ops = vec![
            serde_json::json!({
                "op": "update",
                "table": "Interface",
                "where": [["name", "==", iface_name]],
                "row": { "type": iface_type }
            }),
        ];
        let result = self.transact_many(ops).await?;
        Self::check_errors(&result)?;
        log::info!("Interface {} type set to {} via D-Bus daemon", iface_name, iface_type);
        Ok(())
    }

    /// Compatibility shim for plugins that pass `simd_json::OwnedValue`.
    /// Converts to `serde_json::Value` and routes through the D-Bus daemon.
    pub async fn transact_simd(&self, operations: simd_json::OwnedValue) -> Result<serde_json::Value> {
        let text = simd_json::to_string(&operations)
            .context("failed to serialize simd_json operations to JSON text")?;
        let converted: serde_json::Value = serde_json::from_str(&text)
            .context("failed to deserialize simd_json operations as serde_json::Value")?;
        self.transact_many(converted.as_array().cloned().unwrap_or_default()).await
    }
}
