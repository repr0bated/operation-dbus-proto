//! OVSDB client wrapper around `rovs_ovsdb`.
//!
//! Provides a backward-compatible `OvsdbClient` API backed by a **persistent**
//! `rovs_ovsdb::Client` connection with a live IDL replica.
//!
//! # Connection lifecycle
//!
//! `OvsdbClient::new()` is a cheap, synchronous constructor that records the
//! socket path.  The underlying `rovs_ovsdb::Client` is created lazily on the
//! first method call.  Once connected the same `Client` is reused and a
//! background Tokio task drives the IDL monitor pump (`Client::run()`) so that
//! `idl()` is always up to date.
//!
//! # Thread safety
//!
//! The inner `Client` is guarded by a `tokio::sync::Mutex`.  The background
//! pump acquires the lock, calls the non-blocking `run()`, then immediately
//! yields (`tokio::time::sleep`) so that concurrent method calls can acquire
//! the lock between pump iterations.
//!
//! `OvsdbClient` is cheap to clone — all clones share the same underlying
//! `Arc<Mutex<Option<Client>>>`.

use anyhow::{Context, Result};
use rovs_ovsdb::{Client, ClientConfig, RowRef, Transaction};
use rovs_types::{Atom, Datum};
use uuid::Uuid;
// simd_json used only by the transact_simd compatibility shim.
use rovs_transport::Reconnect;
use serde_json::{json, Value};
use simd_json;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};

// ── Socket path resolution ────────────────────────────────────────────────────

/// Returns the first OVS socket path that exists on this host, falling back to
/// the canonical `/var/run/openvswitch/db.sock`.
fn find_socket_path() -> String {
    let candidates = ["/run/openvswitch/db.sock", "/var/run/openvswitch/db.sock"];
    candidates
        .iter()
        .find(|p| Path::new(p).exists())
        .unwrap_or(&"/var/run/openvswitch/db.sock")
        .to_string()
}

/// Format a socket path as a rovs transport address string.
fn socket_addr(path: &str) -> String {
    format!("unix:{}", path)
}

// ── OVSDB wire-format helpers ─────────────────────────────────────────────────

/// Encode a UUID reference as the OVSDB two-element wire form: `["uuid", uuid]`.
///
/// Parses `uuid` into a [`Uuid`] value and delegates to
/// [`RowRef::Uuid::to_json`] so the encoding is always produced by the
/// `rovs_ovsdb` library rather than a hand-rolled `json!` literal.  Use this
/// wherever a column value or mutation operand must refer to an existing row by
/// its UUID string.
///
/// # Panics
///
/// Panics if `uuid` is not a valid UUID string.  Callers should only pass
/// values obtained directly from OVSDB responses (which are always valid).
fn uuid_ref(uuid: &str) -> Value {
    let parsed: Uuid = uuid
        .parse()
        .unwrap_or_else(|e| panic!("uuid_ref: invalid UUID {:?}: {}", uuid, e));
    RowRef::Uuid(parsed).to_json()
}

/// Encode a named-uuid reference: `["named-uuid", name]`.
///
/// Use this to refer to a row that was given a `uuid-name` in the same
/// transaction (insert-to-insert dependencies).  Delegates to
/// [`RowRef::Named::to_json`] so the encoding is always consistent with
/// the `rovs_ovsdb` library.
#[allow(dead_code)]
fn named_uuid_ref(name: &str) -> Value {
    RowRef::Named(name.to_owned()).to_json()
}

/// Encode an OVSDB set: `["set", [item, ...]]`.
///
/// An empty `items` vec produces `["set", []]`, which is the canonical
/// OVSDB wire representation of the empty set.
///
/// When the items are typed atomic values, prefer constructing them via
/// [`Atom`] (e.g. `atom_value(Atom::String(...))`) before passing them into
/// this function so that the type information is explicit at the call site
/// rather than embedded in a raw `json!` literal.
#[allow(dead_code)]
fn ovsdb_set(items: Vec<Value>) -> Value {
    json!(["set", items])
}

/// Encode an OVSDB map: `["map", [[key, value], ...]]`.
///
/// An empty `pairs` vec produces `["map", []]`, which is the canonical
/// OVSDB wire representation of the empty map.
///
/// For maps whose keys and values are typed atomic scalars, consider building
/// each pair as `(atom_value(Atom::String(k)), atom_value(Atom::String(v)))`.
/// This makes the column type explicit and routes all encoding through the
/// `rovs_types` library rather than raw JSON literals.
#[allow(dead_code)]
fn ovsdb_map(pairs: Vec<(Value, Value)>) -> Value {
    let entries: Vec<Value> = pairs.into_iter().map(|(k, v)| json!([k, v])).collect();
    json!(["map", entries])
}

/// Convert a typed [`Atom`] to a [`serde_json::Value`].
///
/// Convenience wrapper around `serde_json::to_value` for `Atom` values.
/// Use this to build row column values from strongly-typed atoms instead of
/// embedding string or integer literals directly in `json!` macros.
///
/// # Example
///
/// ```ignore
/// let row = json!({
///     "name": atom_value(Atom::String("br0".to_owned())),
///     "tag":  atom_value(Atom::Integer(100)),
/// });
/// ```
#[allow(dead_code)]
fn atom_value(atom: Atom) -> Value {
    serde_json::to_value(atom).expect("Atom serialization is infallible")
}

/// Convert a typed [`Datum`] to a [`serde_json::Value`].
///
/// `Datum` uses `#[serde(untagged)]` and serializes as plain JSON, making it
/// suitable for simple scalar column values in OVSDB `row` objects.  For set or
/// map *containers* in mutations or typed collection columns, use [`ovsdb_set`]
/// / [`ovsdb_map`] instead; those produce the `["set", [...]]` / `["map", [...]]`
/// OVSDB wire form.
#[allow(dead_code)]
fn datum_value(datum: Datum) -> Value {
    serde_json::to_value(datum).expect("Datum serialization is infallible")
}

// ── Internal shared state ─────────────────────────────────────────────────────

/// Mutex-guarded optional inner client.  `None` until first use.
type SharedClient = Arc<Mutex<Option<Client>>>;

// ── OvsdbClient ───────────────────────────────────────────────────────────────

/// High-level OVSDB client backed by a persistent `rovs_ovsdb::Client`.
///
/// The client connects lazily on first use and maintains an in-memory IDL
/// replica via a background monitor pump.  Read methods query the IDL
/// directly; mutating methods send `Transaction`s over the live connection.
///
/// `OvsdbClient` is cheap to clone: all clones share the same underlying
/// connection and IDL state.
#[derive(Clone)]
pub struct OvsdbClient {
    /// Path to the OVS Unix-domain socket.
    socket_path: String,
    /// Lazily-initialized shared client.
    inner: SharedClient,
}

impl OvsdbClient {
    /// Create a client pointing at the first available OVS socket path.
    ///
    /// This is synchronous and does **not** open a network connection.
    pub fn new() -> Self {
        Self {
            socket_path: find_socket_path(),
            inner: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a client for a specific socket path (useful for testing or
    /// non-standard OVS installations).
    pub fn with_socket(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            inner: Arc::new(Mutex::new(None)),
        }
    }

    // ── Connection helpers ─────────────────────────────────────────────────

    /// Open a fresh `rovs_ovsdb::Client` for the `Open_vSwitch` database.
    async fn do_connect(socket_path: &str) -> Result<Client> {
        Client::connect(&socket_addr(socket_path))
            .await
            .with_context(|| format!("connect to OVSDB socket {}", socket_path))
    }

    /// Open a fresh `rovs_ovsdb::Client` for an arbitrary database name.
    async fn do_connect_db(socket_path: &str, database: &str) -> Result<Client> {
        let config = ClientConfig::default().database(database);
        Client::connect_with_config(&socket_addr(socket_path), config)
            .await
            .with_context(|| format!("connect to OVSDB socket {} (db: {})", socket_path, database))
    }

    /// Ensure the inner `Client` is connected, creating one and spawning the
    /// background IDL pump if needed.
    ///
    /// Returns the mutex guard so the caller can call methods on the `Client`
    /// without releasing and re-acquiring the lock.
    async fn get_client(&self) -> Result<tokio::sync::MutexGuard<'_, Option<Client>>> {
        let mut guard = self.inner.lock().await;
        if guard.is_none() {
            let client = Self::do_connect(&self.socket_path).await?;
            *guard = Some(client);

            // Spawn the IDL pump.  It calls the non-blocking `run()` to drain
            // buffered update notifications, then yields for a short interval
            // so that concurrent method calls can acquire the lock between
            // pump iterations.
            let shared = Arc::clone(&self.inner);
            tokio::spawn(async move {
                loop {
                    // Yield to let other tasks (method calls) run.
                    tokio::time::sleep(Duration::from_millis(50)).await;

                    let mut g = shared.lock().await;
                    match g.as_mut() {
                        Some(c) => match c.run().await {
                            Ok(_) => {}
                            Err(e) => {
                                log::warn!("OvsdbClient IDL pump error: {e}");
                                // Clear the client; the next method call will
                                // reconnect and restart the pump.
                                *g = None;
                                break;
                            }
                        },
                        // Client was cleared (e.g. failed transact) — stop.
                        None => break,
                    }
                }
            });
        }
        Ok(guard)
    }

    // ── Initialisation ─────────────────────────────────────────────────────

    /// List all databases available on the OVSDB server.
    pub async fn list_dbs(&self) -> Result<Vec<String>> {
        let mut guard = self.get_client().await?;
        let client = guard.as_mut().expect("get_client ensures Some");
        client.list_dbs().await.context("OVSDB list_dbs failed")
    }

    /// Verify that OVSDB is reachable and the `Open_vSwitch` database is
    /// accessible.  Returns an error if the socket cannot be reached or the
    /// schema cannot be loaded.
    pub async fn ensure_initialized(&self) -> Result<()> {
        let mut guard = self.get_client().await?;
        let client = guard.as_mut().expect("get_client ensures Some");

        let dbs = client.list_dbs().await.context("OVSDB list_dbs failed")?;
        log::debug!("OVSDB databases: {:?}", dbs);

        // A connected client has already fetched and validated the schema.
        let schema = client.schema().map(|s| s.name.clone());
        log::debug!("OVSDB schema loaded: {:?}", schema);

        Ok(())
    }

    // ── Raw transaction helpers ─────────────────────────────────────────────

    /// Execute operations (as a JSON array) against `Open_vSwitch`.
    ///
    /// The `operations` value **must** be a `serde_json::Value::Array` of OVSDB
    /// operation objects.  The database name is prepended automatically.
    pub async fn transact(&self, operations: Value) -> Result<Value> {
        let mut guard = self.get_client().await?;
        let client = guard.as_mut().expect("get_client ensures Some");
        let result = client
            .transact(operations)
            .await
            .context("OVSDB transact failed")?;
        check_op_errors(&result)?;
        Ok(result)
    }

    /// Execute operations against an arbitrary named database.
    ///
    /// Uses a one-shot connection because the persistent client is bound to
    /// `Open_vSwitch`.
    pub async fn transact_db(&self, db: &str, operations: Value) -> Result<Value> {
        let mut client = Self::do_connect_db(&self.socket_path, db).await?;
        let result = client
            .transact(operations)
            .await
            .context("OVSDB transact_db failed")?;
        check_op_errors(&result)?;
        Ok(result)
    }

    /// Execute operations built with `simd_json::json!()` against `Open_vSwitch`.
    ///
    /// Compatibility shim for callers that use `simd_json::json!()` to build
    /// their operation arrays.  Serializes the `simd_json::OwnedValue` to a JSON
    /// string and re-parses it as `serde_json::Value` before forwarding to
    /// [`Self::transact`].
    pub async fn transact_simd(&self, operations: simd_json::OwnedValue) -> Result<Value> {
        let text = simd_json::to_string(&operations)
            .context("failed to serialize simd_json operations to JSON text")?;
        let converted: Value = serde_json::from_str(&text)
            .context("failed to deserialize simd_json operations as serde_json::Value")?;
        self.transact(converted).await
    }

    // ── Commit a `Transaction` via the persistent client ───────────────────

    /// Commit a `rovs_ovsdb::Transaction` via the persistent connection.
    ///
    /// Returns `true` if the server accepted the transaction.
    pub async fn commit_txn(&self, txn: &mut Transaction) -> Result<bool> {
        let mut guard = self.get_client().await?;
        let client = guard.as_mut().expect("get_client ensures Some");
        client.commit(txn).await.context("OVSDB commit failed")
    }

    // ── Bridge management ──────────────────────────────────────────────────

    /// Return `true` if a bridge with the given name already exists.
    ///
    /// Reads from the live IDL replica — no round-trip required.
    pub async fn bridge_exists(&self, bridge_name: &str) -> Result<bool> {
        let guard = self.get_client().await?;
        let client = guard.as_ref().expect("get_client ensures Some");
        let exists = client
            .idl()
            .rows("Bridge")
            .any(|r| r.get_string("name") == Some(bridge_name));
        Ok(exists)
    }

    /// Create a bridge if it does not already exist.
    ///
    /// Uses a `Transaction` to atomically create the bridge row, a local
    /// internal port, and the corresponding interface row, registering them
    /// in `Open_vSwitch.bridges`.
    pub async fn create_bridge(&self, bridge_name: &str) -> Result<()> {
        if self.bridge_exists(bridge_name).await? {
            log::info!("Bridge {} already exists, skipping creation", bridge_name);
            return Ok(());
        }

        let db_name = self.db_name().await;
        let mut txn = Transaction::new(&db_name);
        txn.create_bridge(bridge_name);

        let ok = self.commit_txn(&mut txn).await?;
        if !ok {
            return Err(anyhow::anyhow!(
                "Bridge {} creation transaction failed",
                bridge_name
            ));
        }

        log::info!("Bridge {} created successfully", bridge_name);
        Ok(())
    }

    /// Delete a bridge by name, removing it from `Open_vSwitch.bridges` and
    /// deleting all associated port and interface rows.
    ///
    /// Looks up the bridge, its ports, and their interfaces from the live IDL,
    /// then commits a `delete_bridge_uuid` transaction.  Returns an error if
    /// the bridge does not exist.
    pub async fn delete_bridge(&self, bridge_name: &str) -> Result<()> {
        // Collect all required UUIDs from the IDL (fast, no RPC).
        let (bridge_uuid, port_uuids, iface_uuids) = {
            let guard = self.get_client().await?;
            let client = guard.as_ref().expect("get_client ensures Some");
            let idl = client.idl();

            let bridge_row = idl
                .rows("Bridge")
                .find(|r| r.get_string("name") == Some(bridge_name))
                .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", bridge_name))?;
            let bridge_uuid = bridge_row.uuid;

            // Collect port UUIDs from the bridge's `ports` column.
            let mut port_uuids: Vec<uuid::Uuid> = Vec::new();
            if let Some(ports_val) = bridge_row.get("ports") {
                collect_uuid_set(ports_val, &mut port_uuids);
            }

            // Collect interface UUIDs for each port.
            let mut iface_uuids: Vec<uuid::Uuid> = Vec::new();
            for port_uuid in &port_uuids {
                if let Some(port_row) = idl.row("Port", port_uuid) {
                    if let Some(ifaces_val) = port_row.get("interfaces") {
                        collect_uuid_set(ifaces_val, &mut iface_uuids);
                    }
                }
            }

            (bridge_uuid, port_uuids, iface_uuids)
        };

        let db_name = self.db_name().await;
        let mut txn = Transaction::new(&db_name);
        txn.delete_bridge_uuid(bridge_uuid, &port_uuids, &iface_uuids);

        let ok = self.commit_txn(&mut txn).await?;
        if !ok {
            return Err(anyhow::anyhow!(
                "delete_bridge transaction failed for bridge {}",
                bridge_name
            ));
        }

        log::info!("Bridge {} deleted successfully", bridge_name);
        Ok(())
    }

    /// Return the names of all bridges in the database.
    ///
    /// Reads from the live IDL replica — no round-trip required.
    pub async fn list_bridges(&self) -> Result<Vec<String>> {
        let guard = self.get_client().await?;
        let client = guard.as_ref().expect("get_client ensures Some");
        let bridges: Vec<String> = client
            .idl()
            .rows("Bridge")
            .filter_map(|r| r.get_string("name").map(str::to_string))
            .collect();
        Ok(bridges)
    }

    /// Add a system port (physical/virtual interface) to a bridge.
    pub async fn add_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        self.add_port_with_type(bridge_name, port_name, None).await
    }

    /// Add a port to a bridge with an optional interface type.
    ///
    /// If `port_type` is `None` the interface is created with type `"system"`
    /// (the OVS default for kernel interfaces).
    pub async fn add_port_with_type(
        &self,
        bridge_name: &str,
        port_name: &str,
        port_type: Option<&str>,
    ) -> Result<()> {
        let iface_type = port_type.unwrap_or("system");
        let db_name = self.db_name().await;

        let mut txn = Transaction::new(&db_name);
        txn.add_port(bridge_name, port_name, iface_type);

        let ok = self.commit_txn(&mut txn).await?;
        if !ok {
            return Err(anyhow::anyhow!(
                "add_port transaction failed for port {} on bridge {}",
                port_name,
                bridge_name
            ));
        }

        log::info!(
            "Port {} (type: {:?}) added to bridge {}",
            port_name,
            port_type,
            bridge_name
        );
        Ok(())
    }

    /// Remove a port (and its interface) from a bridge.
    ///
    /// Looks up the bridge, port, and interface UUIDs from the live IDL, then
    /// commits a `delete_port_uuid` transaction.
    pub async fn delete_port(&self, bridge_name: &str, port_name: &str) -> Result<()> {
        // Collect the three UUIDs we need from the IDL replica (fast, no RPC).
        let (bridge_uuid, port_uuid, iface_uuid) = {
            let guard = self.get_client().await?;
            let client = guard.as_ref().expect("get_client ensures Some");
            let idl = client.idl();

            let bridge_uuid = idl
                .rows("Bridge")
                .find(|r| r.get_string("name") == Some(bridge_name))
                .map(|r| r.uuid)
                .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", bridge_name))?;

            let port_uuid = idl
                .rows("Port")
                .find(|r| r.get_string("name") == Some(port_name))
                .map(|r| r.uuid)
                .ok_or_else(|| anyhow::anyhow!("Port '{}' not found", port_name))?;

            // OVS convention: the interface row has the same name as the port.
            let iface_uuid = idl
                .rows("Interface")
                .find(|r| r.get_string("name") == Some(port_name))
                .map(|r| r.uuid)
                .ok_or_else(|| anyhow::anyhow!("Interface '{}' not found", port_name))?;

            (bridge_uuid, port_uuid, iface_uuid)
        };

        let db_name = self.db_name().await;
        let mut txn = Transaction::new(&db_name);
        txn.delete_port_uuid(bridge_uuid, port_uuid, iface_uuid);

        let ok = self.commit_txn(&mut txn).await?;
        if !ok {
            return Err(anyhow::anyhow!(
                "delete_port transaction failed for port {} on bridge {}",
                port_name,
                bridge_name
            ));
        }
        Ok(())
    }

    /// Return the names of all ports attached to a bridge.
    ///
    /// Reads from the live IDL replica — no round-trip required.
    pub async fn list_bridge_ports(&self, bridge_name: &str) -> Result<Vec<String>> {
        let guard = self.get_client().await?;
        let client = guard.as_ref().expect("get_client ensures Some");
        let idl = client.idl();

        // Find the bridge row.
        let bridge_row = idl
            .rows("Bridge")
            .find(|r| r.get_string("name") == Some(bridge_name))
            .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", bridge_name))?;

        // The `ports` column is an OVSDB UUID-set value; collect all port UUIDs.
        let mut port_uuids: Vec<Uuid> = Vec::new();
        if let Some(ports_val) = bridge_row.get("ports") {
            collect_uuid_set(ports_val, &mut port_uuids);
        }

        // Resolve each UUID to a port name via the IDL Port table.
        let names: Vec<String> = port_uuids
            .iter()
            .filter_map(|u| idl.row("Port", u))
            .filter_map(|r| r.get_string("name").map(str::to_string))
            .collect();

        Ok(names)
    }

    /// Return the raw JSON row object for a bridge as a pretty-printed string.
    pub async fn get_bridge_info(&self, bridge_name: &str) -> Result<String> {
        let bridge_uuid = self.find_bridge_uuid(bridge_name).await?;
        let result = self
            .transact(json!([{
                "op": "select",
                "table": "Bridge",
                "where": [["_uuid", "==", uuid_ref(&bridge_uuid)]],
                "columns": []
            }]))
            .await?;
        Ok(serde_json::to_string_pretty(&result[0]["rows"][0])?)
    }

    /// Set a bridge-level property (e.g. `datapath_type`, `fail_mode`).
    pub async fn set_bridge_property(
        &self,
        bridge_name: &str,
        property: &str,
        value: &str,
    ) -> Result<()> {
        let row = match property {
            "datapath_type" => json!({ "datapath_type": value }),
            "fail_mode" => json!({ "fail_mode": value }),
            "stp_enable" => json!({ "stp_enable": value == "true" }),
            "mcast_snooping_enable" => json!({ "mcast_snooping_enable": value == "true" }),
            _ => return Err(anyhow::anyhow!("Unknown bridge property: {}", property)),
        };

        let result = self
            .transact(json!([{
                "op": "update",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "row": row
            }]))
            .await?;

        if let Some(errors) = result.as_array() {
            for error in errors {
                if error.get("error").is_some() {
                    return Err(anyhow::anyhow!(
                        "OVSDB set_bridge_property failed: {:?}",
                        error
                    ));
                }
            }
        }
        Ok(())
    }

    /// Set the `type` column on an `Interface` row.
    pub async fn set_interface_type(
        &self,
        interface_name: &str,
        interface_type: &str,
    ) -> Result<()> {
        let result = self
            .transact(json!([{
                "op": "update",
                "table": "Interface",
                "where": [["name", "==", interface_name]],
                "row": { "type": interface_type }
            }]))
            .await?;

        if let Some(errors) = result.as_array() {
            for error in errors {
                if error.get("error").is_some() {
                    return Err(anyhow::anyhow!(
                        "OVSDB set_interface_type failed: {:?}",
                        error
                    ));
                }
            }
        }
        Ok(())
    }

    // ── Database dump ──────────────────────────────────────────────────────

    /// Dump all rows in all tables of the given database as a JSON object.
    ///
    /// The returned value is `{ "TableName": { "rows": [...] }, ... }`.
    ///
    /// For `Open_vSwitch` this reuses the shared persistent client connection —
    /// no new monitoring connection is created and no "Loaded schema" / "Monitoring
    /// started" spam is emitted.  For other databases a one-shot connection is still
    /// used (those databases are not monitored by the shared client).
    pub async fn dump_db(&self, db: &str) -> Result<Value> {
        if db == "Open_vSwitch" {
            return self.dump_open_vswitch().await;
        }

        // Other databases: one-shot connection (they are not monitored by the shared client).
        let mut client = Self::do_connect_db(&self.socket_path, db).await?;
        let table_names: Vec<String> = client
            .schema()
            .map(|s| s.tables.keys().cloned().collect())
            .unwrap_or_default();
        Self::select_all_tables_raw(&mut client, &table_names).await
    }

    /// Dump Open_vSwitch via the shared persistent client (no new monitoring connection).
    async fn dump_open_vswitch(&self) -> Result<Value> {
        // Phase 1 — read table names from the already-loaded schema.  Hold the
        // lock only as long as needed to avoid blocking the IDL pump.
        let table_names: Vec<String> = {
            let guard = self.get_client().await?;
            let client = guard.as_ref().expect("get_client ensures Some");
            client
                .schema()
                .map(|s| s.tables.keys().cloned().collect())
                .unwrap_or_default()
        };

        if table_names.is_empty() {
            return Ok(Value::Object(serde_json::Map::new()));
        }

        // Phase 2 — run SELECT transacts on the shared connection.
        let ops: Vec<Value> = table_names
            .iter()
            .map(|t| json!({ "op": "select", "table": t, "where": [] }))
            .collect();

        let result = {
            let mut guard = self.get_client().await?;
            let client = guard.as_mut().expect("get_client ensures Some");
            client
                .transact(json!(ops))
                .await
                .context("OVSDB dump_db transact")?
        };

        // Assemble { TableName: { rows: [...] } } — same wire format as before.
        let mut out = serde_json::Map::new();
        if let Some(results) = result.as_array() {
            for (i, table_name) in table_names.iter().enumerate() {
                let rows = results
                    .get(i)
                    .and_then(|r| r.get("rows"))
                    .cloned()
                    .unwrap_or_else(|| json!([]));
                out.insert(table_name.clone(), json!({ "rows": rows }));
            }
        }
        Ok(Value::Object(out))
    }

    /// Run `SELECT *` on a list of tables and assemble `{ TableName: { rows: [...] } }`.
    async fn select_all_tables_raw(client: &mut Client, table_names: &[String]) -> Result<Value> {
        let ops: Vec<Value> = table_names
            .iter()
            .map(|t| json!({ "op": "select", "table": t, "where": [] }))
            .collect();

        let result = client
            .transact(json!(ops))
            .await
            .context("OVSDB select_all_tables transact")?;

        let mut out = serde_json::Map::new();
        if let Some(results) = result.as_array() {
            for (i, table_name) in table_names.iter().enumerate() {
                let rows = results
                    .get(i)
                    .and_then(|r| r.get("rows"))
                    .cloned()
                    .unwrap_or_else(|| json!([]));
                out.insert(table_name.clone(), json!({ "rows": rows }));
            }
        }
        Ok(Value::Object(out))
    }

    // ── Monitoring ─────────────────────────────────────────────────────────

    /// Subscribe to OVSDB update notifications for the given database.
    ///
    /// Returns an `mpsc::Receiver` that receives a full IDL snapshot as a
    /// `serde_json::Value` on every database change.  The snapshot format is:
    ///
    /// ```json
    /// { "TableName": [ { "_uuid": ["uuid", "..."], "col": val, ... }, ... ], ... }
    /// ```
    ///
    /// The first message is sent immediately after the initial connection (it
    /// carries the current database state at connect time).  Subsequent messages
    /// are sent each time `wait()` signals an update.
    ///
    /// The background task uses `rovs_ovsdb::Client::wait()` (which handles OVS
    /// echo keepalives) and reconnects with exponential backoff capped at 30 s.
    /// It stops when the receiver is dropped.
    pub async fn monitor_db(&self, db: &str) -> Result<mpsc::Receiver<Value>> {
        let (tx, rx) = mpsc::channel(100);
        let socket_path = self.socket_path.clone();
        let db = db.to_string();

        tokio::spawn(async move {
            // Reconnection state machine — caps backoff at 30 s.
            let mut reconnect = Reconnect::new();
            reconnect.set_max_backoff(Duration::from_secs(30));

            loop {
                // Wait out any current backoff before attempting the connection.
                if !reconnect.should_connect() {
                    let backoff = reconnect.current_backoff();
                    tracing::debug!("monitor_db({}): backing off for {:?}", db, backoff);
                    tokio::time::sleep(backoff).await;
                }

                reconnect.connecting();
                tracing::debug!("monitor_db({}): connecting to {}", db, socket_path);

                let config = ClientConfig::default().database(&db);
                let addr = socket_addr(&socket_path);

                let mut client = match Client::connect_with_config(&addr, config).await {
                    Ok(c) => {
                        reconnect.connected();
                        tracing::info!(
                            "monitor_db({}): connected, seqno={}",
                            db,
                            c.idl().change_seqno()
                        );
                        c
                    }
                    Err(e) => {
                        tracing::warn!("monitor_db({}): failed to connect: {}", db, e);
                        reconnect.disconnected();
                        reconnect.increase_backoff();
                        continue;
                    }
                };

                // Send the initial snapshot — IDL was populated by initialize() inside
                // Client::connect_with_config(), so it already holds the current DB state.
                let snapshot = idl_snapshot(client.idl());
                if tx.send(snapshot).await.is_err() {
                    return; // receiver dropped
                }
                reconnect.activity();

                // Pump update notifications until the connection drops.
                loop {
                    match client.wait().await {
                        Err(e) => {
                            tracing::warn!("monitor_db({}): connection error: {}", db, e);
                            reconnect.disconnected();
                            reconnect.increase_backoff();
                            break;
                        }
                        Ok(()) => {
                            reconnect.activity();
                            // Snapshot the IDL — wait() already merged the update into it.
                            let snapshot = idl_snapshot(client.idl());
                            if tx.send(snapshot).await.is_err() {
                                return; // receiver dropped
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    /// Return the database name from the connected client's schema, defaulting
    /// to `"Open_vSwitch"` if not yet available.
    async fn db_name(&self) -> String {
        if let Ok(guard) = self.inner.try_lock() {
            if let Some(c) = guard.as_ref() {
                if let Some(s) = c.schema() {
                    return s.name.clone();
                }
            }
        }
        "Open_vSwitch".to_string()
    }

    /// Look up the UUID string for a bridge by name.
    ///
    /// Tries the IDL first (fast, no round-trip); falls back to a `SELECT`
    /// transact if the IDL does not yet have the row.
    async fn find_bridge_uuid(&self, bridge_name: &str) -> Result<String> {
        // Fast path: read from the IDL replica.
        {
            let guard = self.get_client().await?;
            let client = guard.as_ref().expect("get_client ensures Some");
            // Collect the UUID string before dropping the guard so we don't
            // return from inside a borrow of the mutex-guarded client.
            let maybe_uuid: Option<String> = client
                .idl()
                .rows("Bridge")
                .find(|r| r.get_string("name") == Some(bridge_name))
                .map(|r| r.uuid.to_string());
            if let Some(uuid_str) = maybe_uuid {
                return Ok(uuid_str);
            }
        }

        // Slow path: IDL miss — fall back to a direct SELECT.
        let result = self
            .transact(json!([{
                "op": "select",
                "table": "Bridge",
                "where": [["name", "==", bridge_name]],
                "columns": ["_uuid"]
            }]))
            .await?;

        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(row) = rows.first() {
                if let Some(uuid_array) = row["_uuid"].as_array() {
                    if uuid_array.len() == 2 && uuid_array[0] == "uuid" {
                        if let Some(uuid_str) = uuid_array[1].as_str() {
                            return Ok(uuid_str.to_string());
                        }
                    }
                }
            }
        }
        Err(anyhow::anyhow!("Bridge '{}' not found", bridge_name))
    }
}

impl Default for OvsdbClient {
    fn default() -> Self {
        Self::new()
    }
}

// ── IDL snapshot helper ───────────────────────────────────────────────────────

/// Build a full snapshot of an [`rovs_ovsdb::Idl`] as a plain JSON value.
///
/// The returned shape is:
///
/// ```json
/// {
///   "TableName": [
///     { "_uuid": ["uuid", "<uuid-str>"], "col1": val, "col2": val, … },
///     …
///   ],
///   …
/// }
/// ```
///
/// This is a zero-RPC operation — it reads directly from the in-memory replica
/// that `wait()` / `run()` keep up to date.
fn idl_snapshot(idl: &rovs_ovsdb::Idl) -> Value {
    let mut out = serde_json::Map::new();
    if let Some(schema) = idl.schema() {
        for table_name in schema.tables.keys() {
            let rows: Vec<Value> = idl
                .rows(table_name)
                .map(|row| {
                    let mut obj = serde_json::Map::new();
                    // Expose the UUID in the canonical OVSDB wire form so that
                    // callers can use extract_uuid() / similar helpers.
                    obj.insert("_uuid".to_string(), json!(["uuid", row.uuid.to_string()]));
                    for (col, val) in &row.columns {
                        obj.insert(col.clone(), val.clone());
                    }
                    Value::Object(obj)
                })
                .collect();
            out.insert(table_name.clone(), Value::Array(rows));
        }
    }
    Value::Object(out)
}

// ── UUID-set parsing ───────────────────────────────────────────────────────────

/// Parse an OVSDB UUID-set column value and append the contained UUIDs to
/// `out`.
///
/// Handles the forms OVSDB uses:
/// - `["uuid", "<uuid-str>"]` — single UUID
/// - `["set", [["uuid", "..."], ...]]` — set of UUIDs
/// - `null` / other — ignored
fn collect_uuid_set(val: &Value, out: &mut Vec<Uuid>) {
    if let Some(arr) = val.as_array() {
        if arr.len() == 2 {
            if arr[0] == "uuid" {
                // Single UUID.
                if let Some(s) = arr[1].as_str() {
                    if let Ok(u) = s.parse::<Uuid>() {
                        out.push(u);
                    }
                }
            } else if arr[0] == "set" {
                // Set of values — recurse into each element.
                if let Some(items) = arr[1].as_array() {
                    for item in items {
                        collect_uuid_set(item, out);
                    }
                }
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Check an OVSDB transact result array for per-operation error objects and
/// return the first error encountered, if any.
fn check_op_errors(result: &Value) -> Result<()> {
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
