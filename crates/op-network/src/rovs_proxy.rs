//! D-Bus proxy for the OVSDB JSON-RPC passthrough interface.
//!
//! This zbus proxy type allows any crate in the workspace to call an
//! OVSDB JSON-RPC service through D-Bus instead of directly linking rovs_ovsdb
//! or shelling out to ovs-vsctl.
//!
//! Locked design (AGENTS.md §4):
//! - Path: `/org/opdbus/rovs/jsonrpc`
//! - Interface: `org.opdbus.rovs.jsonrpc`
//! - The service is a pure passthrough; business logic stays in the plugins.
//!
//! NOTE: the OpenFlow passthrough half of this module (`RovsOpenFlow`,
//! `openflow_proxy`, `ensure_proxies`) was removed along with
//! `op-openvswitch-daemon` (deprecated, purged — see CLAUDE.md). OpenFlow
//! control now runs entirely through `op-network::openflow::OpenFlowClient`
//! (direct TCP, no D-Bus hop) and the passive `op-of-controller` service in
//! `crates/op-network/src/controller.rs`.

use anyhow::{Context, Result};
use std::sync::Arc;
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
            .context("connect to OVSDB JSON-RPC D-Bus service")
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
            if let Some(row) = result
                .get("rows")
                .and_then(|r| r.as_array())
                .and_then(|a| a.first())
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
                let uuid = rows[0]
                    .get("_uuid")
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
        log::info!(
            "Port {} added to bridge {} via D-Bus daemon",
            port_name,
            bridge_name
        );
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
        let ops = vec![serde_json::json!({
            "op": "update",
            "table": "Interface",
            "where": [["name", "==", iface_name]],
            "row": { "type": iface_type }
        })];
        let result = self.transact_many(ops).await?;
        Self::check_errors(&result)?;
        log::info!(
            "Interface {} type set to {} via D-Bus daemon",
            iface_name,
            iface_type
        );
        Ok(())
    }

    /// Set a bridge property (e.g., "datapath_type", "fail_mode").
    pub async fn set_bridge_property(
        &self,
        bridge_name: &str,
        property: &str,
        value: &str,
    ) -> Result<()> {
        let ops = vec![serde_json::json!({
            "op": "update",
            "table": "Bridge",
            "where": [["name", "==", bridge_name]],
            "row": { property: value }
        })];
        let result = self.transact_many(ops).await?;
        Self::check_errors(&result)?;
        log::info!(
            "Bridge {} property {} set to {} via D-Bus daemon",
            bridge_name,
            property,
            value
        );
        Ok(())
    }

    /// Delete a port from a bridge and remove associated Interface.
    pub async fn delete_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        // First get the port UUID and its interface UUID
        let port_result = self
            .transact_one(serde_json::json!({
                "op": "select",
                "table": "Port",
                "where": [["name", "==", port_name]],
                "columns": ["_uuid", "interfaces"]
            }))
            .await?;

        let port_uuid = port_result
            .get("rows")
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .and_then(|row| row.get("_uuid"))
            .and_then(|u| u.as_array())
            .and_then(|a| a.get(1))
            .and_then(|u| u.as_str())
            .map(String::from)
            .ok_or_else(|| anyhow::anyhow!("Port '{}' not found", port_name))?;

        // Get interface UUIDs from port
        let mut iface_uuids: Vec<String> = Vec::new();
        if let Some(rows) = port_result.get("rows").and_then(|r| r.as_array()) {
            if let Some(row) = rows.first() {
                if let Some(ifaces) = row.get("interfaces") {
                    Self::collect_uuids(ifaces, &mut iface_uuids);
                }
            }
        }

        // Build delete operations: remove from bridge, delete port, delete interfaces
        let mut ops = vec![
            serde_json::json!({
                "op": "mutate",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "mutations": [["ports", "delete", ["uuid", port_uuid.clone()]]]
            }),
            serde_json::json!({
                "op": "delete",
                "table": "Port",
                "where": [["_uuid", "==", ["uuid", port_uuid]]]
            }),
        ];

        for iface_uuid in iface_uuids {
            ops.push(serde_json::json!({
                "op": "delete",
                "table": "Interface",
                "where": [["_uuid", "==", ["uuid", iface_uuid]]]
            }));
        }

        let result = self.transact_many(ops).await?;
        Self::check_errors(&result)?;
        log::info!(
            "Port {} deleted from bridge {} via D-Bus daemon",
            port_name,
            bridge_name
        );
        Ok(())
    }

    /// Add a port with specific type to a bridge.
    pub async fn add_port_with_type(
        &self,
        bridge_name: &str,
        port_name: &str,
        port_type: Option<&str>,
    ) -> Result<()> {
        let ops = match port_type {
            Some(ptype) => vec![
                serde_json::json!({
                    "op": "insert",
                    "table": "Interface",
                    "row": { "name": port_name, "type": ptype },
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
            ],
            None => vec![
                serde_json::json!({
                    "op": "insert",
                    "table": "Interface",
                    "row": { "name": port_name },
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
            ],
        };

        let result = self.transact_many(ops).await?;
        Self::check_errors(&result)?;
        log::info!(
            "Port {} added to bridge {} via D-Bus daemon",
            port_name,
            bridge_name
        );
        Ok(())
    }

    /// Dump the contents of an OVSDB database as JSON.
    /// Returns a JSON object with database contents.
    pub async fn dump_db(&self, database: &str) -> Result<serde_json::Value> {
        // Query all tables in the database
        let _tables_result = self
            .transact_one(serde_json::json!({
                "op": "select",
                "table": "Open_vSwitch",
                "where": [],
                "columns": []
            }))
            .await?;

        // Build a dump structure with all tables
        let mut dump = serde_json::json!({
            "database": database,
            "tables": {}
        });

        // Get list of tables from the database schema
        let schema_result = self
            .transact_one(serde_json::json!({
                "op": "get_schema",
                "id": "dump"
            }))
            .await?;

        if let Some(tables) = schema_result.get("tables").and_then(|t| t.as_object()) {
            for table_name in tables.keys() {
                let table_data = self
                    .transact_one(serde_json::json!({
                        "op": "select",
                        "table": table_name,
                        "where": [],
                        "columns": []
                    }))
                    .await?;

                if let Some(rows) = table_data.get("rows") {
                    dump["tables"][table_name] = rows.clone();
                }
            }
        }

        Ok(dump)
    }

    /// Monitor OVSDB for changes to a database.
    /// Returns a broadcast receiver that will receive JSON updates.
    /// NOTE: This is a compatibility shim built on polling; it is not a
    /// production-grade change feed.
    pub async fn monitor_db(
        &self,
        database: &str,
    ) -> Result<tokio::sync::broadcast::Receiver<serde_json::Value>> {
        let proxy = self.get_proxy().await?;
        // Create a stream/monitor subscription via D-Bus
        let _stream_id = proxy
            .new_stream(database)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create OVSDB monitor stream: {}", e))?;

        // Create a broadcast channel for updates
        // In a full implementation, this would be connected to the daemon's notification stream
        let (tx, rx) = tokio::sync::broadcast::channel(128);

        // Spawn a task that polls for notifications from the daemon
        let proxy_clone = self.proxy.clone();
        tokio::spawn(async move {
            loop {
                match proxy_clone.get() {
                    Some(proxy) => {
                        // Check if there are pending notifications
                        match proxy.has_pending_notifications().await {
                            Ok(true) => {
                                let drain_result: zbus::Result<String> =
                                    proxy.drain_notifications().await;
                                match drain_result {
                                    Ok(json_str) => {
                                        if let Ok(json) =
                                            serde_json::from_str::<serde_json::Value>(&json_str)
                                        {
                                            let _ = tx.send(json);
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to drain notifications: {}", e);
                                    }
                                }
                            }
                            Ok(false) => {
                                // No notifications pending, wait a bit
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            }
                            Err(e) => {
                                log::warn!("Failed to check for pending notifications: {}", e);
                                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            }
                        }
                    }
                    None => {
                        log::warn!("Proxy not available for monitoring");
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Compatibility shim for plugins that pass `simd_json::OwnedValue`.
    /// Converts to `serde_json::Value` and routes through the D-Bus daemon.
    pub async fn transact_simd(
        &self,
        operations: simd_json::OwnedValue,
    ) -> Result<serde_json::Value> {
        let text = simd_json::to_string(&operations)
            .context("failed to serialize simd_json operations to JSON text")?;
        let converted: serde_json::Value = serde_json::from_str(&text)
            .context("failed to deserialize simd_json operations as serde_json::Value")?;
        self.transact_many(converted.as_array().cloned().unwrap_or_default())
            .await
    }
}
