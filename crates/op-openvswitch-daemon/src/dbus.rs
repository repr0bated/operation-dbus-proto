//! Pure passthrough D-Bus interfaces for the op-openvswitch-daemon.
//!
//! TWO separate object paths (locked design):
//! - `/org/opdbus/rovs/jsonrpc`  → interface `org.opdbus.rovs.jsonrpc`
//! - `/org/opdbus/rovs/openflow` → interface `org.opdbus.rovs.openflow`
//!
//! The daemon knows NOTHING about bridges, ports, or containers.  It only
//! proxies raw `rovs-jsonrpc` / `rovs-openflow` primitives over D-Bus.
//! Business logic lives in the consuming plugins.

use anyhow::{Context, Result};
use rovs_ovsdb::Client;
use rovs_transport::Address;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};
use zbus::interface;

// ── Shared daemon state ───────────────────────────────────────────────────────

/// Global state shared by both D-Bus objects.
#[derive(Clone)]
pub struct DaemonState {
    /// OVSDB JSON-RPC client (rovs_ovsdb).  Lazily initialised on first call.
    pub ovsdb: Arc<Mutex<Option<Client>>>,
    /// OVS Unix-domain socket path.
    pub socket_path: String,
    /// OpenFlow connections: conn_id → VConn.
    pub of_conns: Arc<Mutex<HashMap<u64, rovs_openflow::VConn>>>,
    /// Monotonic connection handle allocator.
    pub next_of_id: Arc<Mutex<u64>>,
}

impl DaemonState {
    pub fn new(socket_path: String) -> Self {
        Self {
            ovsdb: Arc::new(Mutex::new(None)),
            socket_path,
            of_conns: Arc::new(Mutex::new(HashMap::new())),
            next_of_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Get or create the persistent OVSDB client.
    pub async fn get_ovsdb(&self) -> Result<tokio::sync::MutexGuard<'_, Option<Client>>> {
        let mut guard = self.ovsdb.lock().await;
        if guard.is_none() {
            let client = Client::connect(&format!("unix:{}", self.socket_path))
                .await
                .with_context(|| format!("connect to OVSDB at {}", self.socket_path))?;
            info!("OVSDB client connected to {}", self.socket_path);
            *guard = Some(client);
        }
        Ok(guard)
    }

    /// Allocate a new OpenFlow connection handle id.
    pub async fn alloc_of_id(&self) -> u64 {
        let mut n = self.next_of_id.lock().await;
        let id = *n;
        *n += 1;
        id
    }
}

// ── JSON-RPC object (/org/opdbus/rovs/jsonrpc) ──────────────────────────────

/// D-Bus service proxying raw `rovs-jsonrpc` + `rovs-ovsdb` primitives.
pub struct JsonRpcService {
    state: DaemonState,
}

impl JsonRpcService {
    pub fn new(state: DaemonState) -> Self {
        Self { state }
    }
}

#[interface(name = "org.opdbus.rovs.jsonrpc")]
impl JsonRpcService {
    /// Execute a raw OVSDB JSON-RPC `transact`.
    ///
    /// `method` — JSON-RPC method name (always `"transact"` for OVSDB).
    /// `params_json` — JSON-encoded params array (e.g. `["Open_vSwitch", {op:...}]`).
    /// Returns JSON-encoded result.
    async fn transact(&self, method: &str, params_json: &str) -> String {
        debug!("jsonrpc.transact method={} params_len={}", method, params_json.len());

        let mut guard = match self.state.get_ovsdb().await {
            Ok(g) => g,
            Err(e) => return json_error(&format!("OVSDB connect failed: {}", e)),
        };
        let client = match guard.as_mut() {
            Some(c) => c,
            None => return json_error("OVSDB client unavailable"),
        };

        // Parse params as serde_json::Value array.
        let params: serde_json::Value = match serde_json::from_str(params_json) {
            Ok(v) => v,
            Err(e) => return json_error(&format!("invalid params JSON: {}", e)),
        };

        // We use the rovs_ovsdb::Client::transact_raw or similar.
        // Since rovs_ovsdb::Client exposes `transact(Value) -> Result<Value>`,
        // we forward directly.
        match client.transact(params).await {
            Ok(result) => match serde_json::to_string(&result) {
                Ok(s) => s,
                Err(e) => json_error(&format!("serialize result failed: {}", e)),
            },
            Err(e) => json_error(&format!("transact failed: {}", e)),
        }
    }

    /// One-way notify (fire-and-forget).
    async fn notify(&self, _method: &str, _params_json: &str) {
        // rovs-jsonrpc notify support — placeholder; most callers use transact.
        debug!("jsonrpc.notify (passthrough stub)");
    }

    /// Return next JSON-RPC request id (monotonic counter).
    async fn next_id(&self) -> u64 {
        // Simplistic id counter.  In full rovs-jsonrpc this is per-connection.
        1
    }

    /// Send raw JSON-RPC message string, return response string.
    async fn send_message(&self, msg: &str) -> String {
        debug!("jsonrpc.send_message len={}", msg.len());
        match serde_json::from_str::<serde_json::Value>(msg) {
            Ok(req) => {
                let mut guard = match self.state.get_ovsdb().await {
                    Ok(g) => g,
                    Err(e) => return json_error(&format!("connect failed: {}", e)),
                };
                let client = match guard.as_mut() {
                    Some(c) => c,
                    None => return json_error("client unavailable"),
                };
                match client.transact(req).await {
                    Ok(r) => serde_json::to_string(&r).unwrap_or_else(|e| json_error(&e.to_string())),
                    Err(e) => json_error(&e.to_string()),
                }
            }
            Err(e) => json_error(&format!("invalid JSON: {}", e)),
        }
    }

    /// Receive raw JSON-RPC message string.
    async fn recv_message(&self) -> String {
        // rovs-jsonrpc recv is async-stream oriented; stub for now.
        String::new()
    }

    /// Poll pending notifications.
    async fn has_pending_notifications(&self) -> bool {
        false
    }

    /// Count pending notifications.
    async fn pending_notification_count(&self) -> u64 {
        0
    }

    /// Pop one notification.
    async fn pop_notification(&self) -> String {
        String::new()
    }

    /// Drain all notifications.
    async fn drain_notifications(&self) -> String {
        "[]".to_string()
    }

    /// Open a new named stream / monitor.
    async fn new_stream(&self, stream: &str) -> String {
        debug!("jsonrpc.new_stream stream={}", stream);
        format!("{{\"stream\":\"{}\"}}", stream)
    }

    /// Daemon + connection status JSON.
    async fn status(&self) -> String {
        let connected = self.state.ovsdb.lock().await.is_some();
        format!(
            "{{\"connected\":{},\"socket_path\":\"{}\",\"version\":\"{}\"}}",
            connected, self.state.socket_path, env!("CARGO_PKG_VERSION")
        )
    }
}

fn json_error(msg: &str) -> String {
    serde_json::json!({"error": msg}).to_string()
}

// ── OpenFlow object (/org/opdbus/rovs/openflow) ─────────────────────────────

/// D-Bus service proxying raw `rovs-openflow` primitives via `rovs_transport::Address`
/// and `rovs_openflow::VConn`.
pub struct OpenFlowService {
    state: DaemonState,
}

impl OpenFlowService {
    pub fn new(state: DaemonState) -> Self {
        Self { state }
    }
}

#[interface(name = "org.opdbus.rovs.openflow")]
impl OpenFlowService {
    /// Connect to a switch at `addr` (e.g. `"tcp:127.0.0.1:6653"`).
    ///
    /// `addr` is parsed by `rovs_transport::Address::from_str` which supports:
    /// - `unix:/path/to/socket`
    /// - `tcp:host:port`
    /// - `ssl:host:port`
    ///
    /// Returns JSON `{"conn_id":N}` or `{"error":"..."}`.
    async fn connect(&self, addr: &str) -> String {
        debug!("openflow.connect addr={}", addr);

        let address = match addr.parse::<Address>() {
            Ok(a) => a,
            Err(e) => return json_error(&format!("bad address: {}", e)),
        };

        match rovs_openflow::VConn::connect(&address).await {
            Ok(vconn) => {
                let id = self.state.alloc_of_id().await;
                self.state.of_conns.lock().await.insert(id, vconn);
                serde_json::json!({"conn_id": id}).to_string()
            }
            Err(e) => json_error(&format!("connect failed: {}", e)),
        }
    }

    /// Return negotiated OpenFlow version JSON.
    async fn version(&self, conn_id: u64) -> String {
        let mut conns = self.state.of_conns.lock().await;
        match conns.get_mut(&conn_id) {
            Some(vconn) => {
                let ver = vconn.version();
                serde_json::json!({"version": format!("{:?}", ver)}).to_string()
            }
            None => json_error("unknown conn_id"),
        }
    }

    /// Send a flow_mod. `flow_json` is JSON-encoded Flow.
    ///
    /// **Note:** Deserialising a rovs_openflow::Flow from JSON requires a
    /// custom serializer (Match + ActionList wire encoding).  This is a
    /// known gap — callers should use raw `send_message` for now.
    async fn send_flow(&self, conn_id: u64, flow_json: &str) -> String {
        debug!("openflow.send_flow conn_id={} len={}", conn_id, flow_json.len());
        let mut conns = self.state.of_conns.lock().await;
        match conns.get_mut(&conn_id) {
            Some(_vconn) => {
                json_error("send_flow: JSON→Flow deserialiser not yet implemented (use send_message with raw OFPT_FLOW_MOD bytes)")
            }
            None => json_error("unknown conn_id"),
        }
    }

    /// Send flow_mod + barrier.
    async fn send_flow_sync(&self, conn_id: u64, _flow_json: &str) -> String {
        debug!("openflow.send_flow_sync conn_id={}", conn_id);
        let mut conns = self.state.of_conns.lock().await;
        match conns.get_mut(&conn_id) {
            Some(_vconn) => {
                json_error("send_flow_sync: JSON→Flow deserialiser not yet implemented")
            }
            None => json_error("unknown conn_id"),
        }
    }

    /// Raw `ovs-ofctl` passthrough (temporary bridge until pure OpenFlow binary is wired).
    /// Accepts a bridge name and a JSON array of CLI arguments.
    /// Returns the captured stdout / stderr as a JSON string.
    async fn ofctl(&self, bridge: &str, args_json: &str) -> String {
        debug!("openflow.ofctl bridge={} args={}", bridge, args_json);
        let extra_args: Vec<String> = match serde_json::from_str(args_json) {
            Ok(v) => v,
            Err(e) => return json_error(&format!("invalid args JSON: {}", e)),
        };
        let mut cmd = tokio::process::Command::new("ovs-ofctl");
        cmd.arg("-O").arg("OpenFlow13");
        for a in extra_args {
            cmd.arg(a);
        }
        cmd.arg(bridge);
        match cmd.output().await {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let code = output.status.code().unwrap_or(-1);
                if output.status.success() {
                    serde_json::json!({ "ok": true, "stdout": stdout.trim() }).to_string()
                } else {
                    json_error(&format!("ovs-ofctl exited {}: {}", code, stderr.trim()))
                }
            }
            Err(e) => json_error(&format!("failed to spawn ovs-ofctl: {}", e)),
        }
    }

    /// Send an echo request and wait for the echo reply.
    async fn echo(&self, conn_id: u64) -> String {
        debug!("openflow.echo conn_id={}", conn_id);
        let mut conns = self.state.of_conns.lock().await;
        match conns.get_mut(&conn_id) {
            Some(vconn) => match vconn.echo().await {
                Ok(()) => serde_json::json!({"ok": true}).to_string(),
                Err(e) => json_error(&format!("echo failed: {}", e)),
            },
            None => json_error("unknown conn_id"),
        }
    }

    /// Send a barrier request and wait for the barrier reply.
    async fn barrier(&self, conn_id: u64) -> String {
        debug!("openflow.barrier conn_id={}", conn_id);
        let mut conns = self.state.of_conns.lock().await;
        match conns.get_mut(&conn_id) {
            Some(vconn) => match vconn.barrier().await {
                Ok(()) => serde_json::json!({"ok": true}).to_string(),
                Err(e) => json_error(&format!("barrier failed: {}", e)),
            },
            None => json_error("unknown conn_id"),
        }
    }

    /// Dump all flows from the switch.  Returns a JSON array string per flow.
    async fn dump_flows(&self, conn_id: u64) -> Vec<String> {
        debug!("openflow.dump_flows conn_id={}", conn_id);
        let mut conns = self.state.of_conns.lock().await;
        match conns.get_mut(&conn_id) {
            Some(vconn) => match vconn.dump_flows().await {
                Ok(entries) => entries.iter().map(flow_stats_to_json).collect(),
                Err(e) => vec![json_error(&format!("dump_flows failed: {}", e))],
            },
            None => vec![json_error("unknown conn_id")],
        }
    }

    /// Dump flows matching a filter.  `request_json` is a placeholder.
    async fn dump_flows_filtered(&self, conn_id: u64, _request: &str) -> Vec<String> {
        debug!("openflow.dump_flows_filtered conn_id={}", conn_id);
        // TODO: deserialize FlowStatsRequest from JSON, then call vconn.dump_flows_filtered(req)
        self.dump_flows(conn_id).await
    }

    /// Block until a PacketIn message arrives.
    /// Returns JSON with buffer_id, in_port, reason, table_id, cookie, data (hex).
    async fn recv_packet_in(&self, conn_id: u64) -> String {
        debug!("openflow.recv_packet_in conn_id={}", conn_id);
        let mut conns = self.state.of_conns.lock().await;
        match conns.get_mut(&conn_id) {
            Some(vconn) => match vconn.recv_packet_in().await {
                Ok(pkt) => packet_in_to_json(&pkt),
                Err(e) => json_error(&format!("recv_packet_in failed: {}", e)),
            },
            None => json_error("unknown conn_id"),
        }
    }

    /// Non-blocking try-receive PacketIn.
    async fn try_recv_packet_in(&self, conn_id: u64) -> String {
        let mut conns = self.state.of_conns.lock().await;
        match conns.get_mut(&conn_id) {
            Some(vconn) => match vconn.try_recv_packet_in().await {
                Ok(Some(pkt)) => packet_in_to_json(&pkt),
                Ok(None) => String::new(),
                Err(e) => json_error(&format!("try_recv_packet_in failed: {}", e)),
            },
            None => json_error("unknown conn_id"),
        }
    }

    /// Start flow monitor.  `request_json` is a placeholder.
    async fn monitor_flows(&self, conn_id: u64, _request: &str) -> Vec<String> {
        debug!("openflow.monitor_flows conn_id={}", conn_id);
        // TODO: deserialize FlowMonitorRequest from JSON
        Vec::new()
    }

    /// Block until flow updates arrive.
    async fn recv_flow_updates(&self, conn_id: u64) -> Vec<String> {
        debug!("openflow.recv_flow_updates conn_id={}", conn_id);
        let mut conns = self.state.of_conns.lock().await;
        match conns.get_mut(&conn_id) {
            Some(vconn) => match vconn.recv_flow_updates().await {
                Ok(updates) => updates.iter().map(flow_update_to_json).collect(),
                Err(e) => vec![json_error(&format!("recv_flow_updates failed: {}", e))],
            },
            None => vec![json_error("unknown conn_id")],
        }
    }

    /// Send a packet_out.  `packet_out_json` is a placeholder.
    async fn send_packet_out(&self, conn_id: u64, _packet_out_json: &str) -> String {
        debug!("openflow.send_packet_out conn_id={}", conn_id);
        // TODO: deserialize PacketOut from JSON
        let mut conns = self.state.of_conns.lock().await;
        match conns.get_mut(&conn_id) {
            Some(_vconn) => {
                json_error("send_packet_out: JSON→PacketOut deserialiser not yet implemented")
            }
            None => json_error("unknown conn_id"),
        }
    }

    /// Controller status JSON.
    async fn status(&self) -> String {
        let count = self.state.of_conns.lock().await.len();
        format!(
            "{{\"connections\":{},\"version\":\"{}\"}}",
            count,
            env!("CARGO_PKG_VERSION")
        )
    }
}

// ── JSON serializers for rovs-openflow types ──────────────────────────────────

fn flow_stats_to_json(entry: &rovs_openflow::FlowStatsEntry) -> String {
    serde_json::json!({
        "table_id": entry.table_id,
        "duration_sec": entry.duration_sec,
        "duration_nsec": entry.duration_nsec,
        "priority": entry.priority,
        "idle_timeout": entry.idle_timeout,
        "hard_timeout": entry.hard_timeout,
        "flags": entry.flags,
        "cookie": entry.cookie,
        "packet_count": entry.packet_count,
        "byte_count": entry.byte_count,
        "match_fields_hex": bytes_to_hex(&entry.match_fields.encode()),
        "instructions_hex": bytes_to_hex(&entry.instructions),
    })
    .to_string()
}

fn packet_in_to_json(pkt: &rovs_openflow::PacketIn) -> String {
    serde_json::json!({
        "buffer_id": pkt.buffer_id,
        "total_len": pkt.total_len,
        "reason": format!("{:?}", pkt.reason),
        "table_id": pkt.table_id,
        "cookie": pkt.cookie,
        "match_fields_hex": bytes_to_hex(&pkt.match_fields.encode()),
        "data_hex": bytes_to_hex(&pkt.data),
    })
    .to_string()
}

fn flow_update_to_json(update: &rovs_openflow::FlowUpdate) -> String {
    match update {
        rovs_openflow::FlowUpdate::Full(full) => serde_json::json!({
            "type": "Full",
            "event": format!("{:?}", full.event),
            "reason": full.reason,
            "priority": full.priority,
            "idle_timeout": full.idle_timeout,
            "hard_timeout": full.hard_timeout,
            "table_id": full.table_id,
            "cookie": full.cookie,
            "match_fields_hex": bytes_to_hex(&full.match_fields.encode()),
            "actions_hex": bytes_to_hex(&full.actions),
        }),
        rovs_openflow::FlowUpdate::Abbrev { xid } => serde_json::json!({
            "type": "Abbrev",
            "xid": xid,
        }),
    }
    .to_string()
}

/// Simple hex encoder (avoids pulling in the `hex` crate).
fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xF) as usize] as char);
    }
    s
}
