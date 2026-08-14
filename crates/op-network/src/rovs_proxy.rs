//! OVSDB access for the workspace.
//!
//! `OvsdbDbusClient` (name kept for call-site compatibility) talks to
//! `ovsdb-server` natively via the vendor `rovs_ovsdb::Client` — no D-Bus hop.
//! The dead raw JSON-RPC D-Bus proxy was removed; compatibility callers enter
//! through the schema-declared `ovsdb_bridge` object on the canonical plugin
//! tree, while the audited bridge backend uses this native client.
//!
//! NOTE: the OpenFlow passthrough half of this module (`RovsOpenFlow`,
//! `openflow_proxy`, `ensure_proxies`) was removed along with
//! `op-openvswitch-daemon` (deprecated, purged — see CLAUDE.md). OpenFlow
//! control now runs entirely through `op-network::openflow::OpenFlowClient`
//! (direct TCP, no D-Bus hop) and the passive `op-of-controller` service in
//! `crates/op-network/src/controller.rs` — the same pattern OVSDB now follows.

use anyhow::{Context, Result};
use std::sync::Arc;

// ── OvsdbDbusClient ─────────────────────────────────────────────────────────

/// Default OVSDB socket address, in `rovs_transport::Address` syntax, matching
/// `ovs_capabilities.rs`'s `ovsdb_socket_path`.
const DEFAULT_OVSDB_ADDR: &str = "unix:/var/run/openvswitch/db.sock";

/// High-level OVSDB client — wraps the real vendor `rovs_ovsdb::Client`
/// (crates.io `rovs-ovsdb`, already a workspace dependency and already used
/// natively — no D-Bus hop — by `op-network::openflow` via `rovs_openflow`/
/// `rovs_transport`). No separate D-Bus passthrough daemon exists for OVSDB;
/// one here would (and did, before this fix) target a service that was never
/// built. `rovs_ovsdb::Client` needs `&mut self`, so the connection is held
/// behind a mutex and shared/reconnected lazily.
#[derive(Clone, Default)]
pub struct OvsdbDbusClient {
    client: Arc<tokio::sync::Mutex<Option<rovs_ovsdb::Client>>>,
}

impl OvsdbDbusClient {
    /// Kept sync to preserve the existing construction call sites; the vendor
    /// client connects lazily on first use.
    pub fn new() -> Self {
        Self {
            client: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Run `f` against a connected client, reconnecting once on failure (the
    /// socket may have been idle-closed by `ovsdb-server` between calls).
    async fn with_client<T, F>(&self, f: F) -> Result<T>
    where
        F: for<'a> Fn(
            &'a mut rovs_ovsdb::Client,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T>> + Send + 'a>,
        >,
    {
        let addr =
            std::env::var("OVSDB_SOCKET_ADDR").unwrap_or_else(|_| DEFAULT_OVSDB_ADDR.to_string());
        let mut guard = self.client.lock().await;
        if guard.is_none() {
            let client = rovs_ovsdb::Client::connect(&addr)
                .await
                .with_context(|| format!("failed to connect to OVSDB at {addr}"))?;
            *guard = Some(client);
        }
        match f(guard.as_mut().expect("just ensured Some")).await {
            Ok(val) => Ok(val),
            Err(_first_err) => {
                // Reconnect once and retry — covers idle-closed sockets.
                let client = rovs_ovsdb::Client::connect(&addr)
                    .await
                    .with_context(|| format!("failed to reconnect to OVSDB at {addr}"))?;
                *guard = Some(client);
                f(guard.as_mut().expect("just ensured Some")).await
            }
        }
    }

    // ── Internal: build & send a transact ───────────────────────────────

    async fn transact_one(&self, op: serde_json::Value) -> Result<serde_json::Value> {
        // rovs_ovsdb::Client::transact() prepends the configured database name
        // itself, so operations here are passed WITHOUT it (unlike the raw
        // JSON-RPC ["Open_vSwitch", op, ...] wire shape).
        let result = self
            .with_client(move |client| {
                let op = op.clone();
                Box::pin(async move {
                    client
                        .transact(serde_json::Value::Array(vec![op]))
                        .await
                        .context("OVSDB transact failed")
                })
            })
            .await?;
        // RFC 7047: `result` is an array of one entry per input operation, in
        // order. Callers of transact_one all expect the single op's result
        // object directly (e.g. `result.get("rows")`), so unwrap here once
        // rather than in every caller.
        Self::check_errors(&result)?;
        Ok(result
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }

    async fn transact_many(&self, ops: Vec<serde_json::Value>) -> Result<serde_json::Value> {
        self.with_client(move |client| {
            let ops = ops.clone();
            Box::pin(async move {
                client
                    .transact(serde_json::Value::Array(ops))
                    .await
                    .context("OVSDB transact failed")
            })
        })
        .await
    }

    // ── Read helpers ──────────────────────────────────────────────────────

    /// Return `true` if the daemon (and OVSDB) is reachable.
    pub async fn list_dbs(&self) -> Result<Vec<String>> {
        let val = self
            .with_client(|client| {
                Box::pin(async move { client.list_dbs().await.context("OVSDB list_dbs failed") })
            })
            .await?;
        Ok(val)
    }

    /// Return the schema cached by the connected vendor client.
    pub async fn get_schema(&self) -> Result<serde_json::Value> {
        self.with_client(|client| {
            Box::pin(async move {
                let schema = client
                    .schema()
                    .ok_or_else(|| anyhow::anyhow!("OVSDB schema is not loaded"))?;
                serde_json::to_value(schema).context("serialize OVSDB schema")
            })
        })
        .await
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
    ///
    /// Spawns a dedicated native `rovs_ovsdb::Client` (separate from the
    /// short-lived transact mutex connection) that:
    /// 1. Connects with `ClientConfig` for `database` (monitor V1 / full IDL)
    /// 2. Immediately broadcasts a full IDL snapshot
    /// 3. Blocks on `Client::wait()` and re-broadcasts a snapshot on each update
    /// 4. Reconnects with exponential backoff (capped at 30s) on disconnect
    ///
    /// Snapshot shape (IDL table map — what `MutationEngine::start` consumes):
    /// ```json
    /// { "Bridge": [ { "_uuid": ["uuid", "..."], "name": "br0", ... }, ... ], ... }
    /// ```
    ///
    /// Returns a broadcast receiver immediately; connection happens in the
    /// background so callers can subscribe before OVS is up.
    pub async fn monitor_db(
        &self,
        database: &str,
    ) -> Result<tokio::sync::broadcast::Receiver<serde_json::Value>> {
        let (tx, rx) = tokio::sync::broadcast::channel(100);
        let addr =
            std::env::var("OVSDB_SOCKET_ADDR").unwrap_or_else(|_| DEFAULT_OVSDB_ADDR.to_string());
        let db = database.to_string();

        tokio::spawn(async move {
            use rovs_ovsdb::{Client, ClientConfig};
            use rovs_transport::Reconnect;
            use std::time::Duration;

            let mut reconnect = Reconnect::new();
            reconnect.set_max_backoff(Duration::from_secs(30));

            loop {
                if !reconnect.should_connect() {
                    let backoff = reconnect.current_backoff();
                    tracing::debug!(
                        database = %db,
                        ?backoff,
                        "monitor_db: backing off before reconnect"
                    );
                    tokio::time::sleep(backoff).await;
                }

                reconnect.connecting();
                tracing::debug!(database = %db, %addr, "monitor_db: connecting");

                let config = ClientConfig::default().database(&db);
                let mut client = match Client::connect_with_config(&addr, config).await {
                    Ok(c) => {
                        reconnect.connected();
                        tracing::info!(
                            database = %db,
                            seqno = c.idl().change_seqno(),
                            "monitor_db: connected and monitoring"
                        );
                        c
                    }
                    Err(e) => {
                        tracing::warn!(database = %db, error = %e, "monitor_db: connect failed");
                        reconnect.disconnected();
                        reconnect.increase_backoff();
                        continue;
                    }
                };

                // Initial snapshot — IDL already holds current DB state from initialize().
                let snapshot = idl_snapshot(client.idl());
                if tx.send(snapshot).is_err() {
                    tracing::debug!(database = %db, "monitor_db: no receivers; stopping");
                    return;
                }
                reconnect.activity();

                loop {
                    match client.wait().await {
                        Ok(()) => {
                            reconnect.activity();
                            let snapshot = idl_snapshot(client.idl());
                            if tx.send(snapshot).is_err() {
                                tracing::debug!(
                                    database = %db,
                                    "monitor_db: receivers dropped; stopping"
                                );
                                return;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                database = %db,
                                error = %e,
                                "monitor_db: connection error; will reconnect"
                            );
                            reconnect.disconnected();
                            reconnect.increase_backoff();
                            break;
                        }
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

    // ── Datapath-safe controller attach (OVS async flow contract) ─────────────

    /// See [`crate::datapath_safe::ensure_fallback_normal`].
    pub async fn ensure_fallback_normal(&self, bridge: &str) -> Result<()> {
        crate::datapath_safe::ensure_fallback_normal(bridge).await
    }

    /// See [`crate::datapath_safe::set_fail_mode`].
    pub async fn set_fail_mode(&self, bridge: &str, mode: &str) -> Result<()> {
        crate::datapath_safe::set_fail_mode(bridge, mode).await
    }

    /// See [`crate::datapath_safe::del_controller`].
    pub async fn del_controller(&self, bridge: &str) -> Result<()> {
        crate::datapath_safe::del_controller(bridge).await
    }

    /// See [`crate::datapath_safe::set_controller`].
    pub async fn set_controller(&self, bridge: &str, endpoint: &str) -> Result<()> {
        crate::datapath_safe::set_controller(bridge, endpoint).await
    }

    /// See [`crate::datapath_safe::get_datapath_health`].
    pub async fn get_datapath_health(
        &self,
        bridge: &str,
    ) -> Result<crate::datapath_safe::DatapathHealth> {
        crate::datapath_safe::get_datapath_health(bridge).await
    }

    /// See [`crate::datapath_safe::attach_controller_safe`].
    pub async fn attach_controller_safe(
        &self,
        bridge: &str,
        endpoint: &str,
    ) -> Result<crate::datapath_safe::DatapathHealth> {
        crate::datapath_safe::attach_controller_safe(bridge, endpoint).await
    }
}

/// Build a full snapshot of an [`rovs_ovsdb::Idl`] as a plain JSON value.
///
/// Shape:
/// ```json
/// {
///   "TableName": [
///     { "_uuid": ["uuid", "<uuid-str>"], "col1": val, … },
///     …
///   ],
///   …
/// }
/// ```
fn idl_snapshot(idl: &rovs_ovsdb::Idl) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    if let Some(schema) = idl.schema() {
        for table_name in schema.tables.keys() {
            let rows: Vec<serde_json::Value> = idl
                .rows(table_name)
                .map(|row| {
                    let mut obj = serde_json::Map::new();
                    obj.insert(
                        "_uuid".to_string(),
                        serde_json::json!(["uuid", row.uuid.to_string()]),
                    );
                    for (col, val) in &row.columns {
                        obj.insert(col.clone(), val.clone());
                    }
                    serde_json::Value::Object(obj)
                })
                .collect();
            out.insert(table_name.clone(), serde_json::Value::Array(rows));
        }
    }
    serde_json::Value::Object(out)
}

#[cfg(test)]
mod monitor_db_tests {
    use super::*;
    use serde_json::json;

    /// Pure helper mirroring MutationEngine table extraction for IDL snapshots
    /// and RFC 7047 update notifications — kept here so monitor + consumer stay
    /// aligned without a live ovsdb-server.
    fn tables_from_monitor_update(
        update: &serde_json::Value,
    ) -> Option<&serde_json::Map<String, serde_json::Value>> {
        if let Some(params) = update.get("params").and_then(|p| p.as_array()) {
            // RFC 7047: params = [monitor-id, table-updates]
            if let Some(obj) = params.get(1).and_then(|v| v.as_object()) {
                return Some(obj);
            }
            // Legacy mistaken 3-param shape used by an earlier start() draft
            if let Some(obj) = params.get(2).and_then(|v| v.as_object()) {
                return Some(obj);
            }
            return None;
        }
        update.as_object()
    }

    #[test]
    fn idl_snapshot_empty_idl_is_empty_object() {
        let idl = rovs_ovsdb::Idl::new();
        let snap = idl_snapshot(&idl);
        assert_eq!(snap, json!({}));
    }

    #[test]
    fn tables_from_idl_snapshot_shape() {
        let update = json!({
            "Bridge": [{"_uuid": ["uuid", "aaa"], "name": "br0"}],
            "Port": []
        });
        let tables = tables_from_monitor_update(&update).expect("tables");
        assert!(tables.contains_key("Bridge"));
        assert_eq!(tables["Bridge"][0]["name"], "br0");
    }

    #[test]
    fn tables_from_rfc7047_update_params() {
        let update = json!({
            "method": "update",
            "params": [
                "mon-id",
                {"Bridge": {"row-uuid": {"new": {"name": "br1"}}}}
            ]
        });
        let tables = tables_from_monitor_update(&update).expect("tables");
        assert!(tables.contains_key("Bridge"));
    }

    #[tokio::test]
    async fn monitor_db_returns_receiver_without_blocking() {
        // Does not require a live ovsdb-server: connect runs in a background task.
        let client = OvsdbDbusClient::new();
        let rx = client
            .monitor_db("Open_vSwitch")
            .await
            .expect("monitor_db should return a receiver immediately");
        // Dropping rx should eventually stop the task (send fails → return).
        drop(rx);
        // Give the task a tick; no panic is success.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}
