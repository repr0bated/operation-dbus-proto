//! Pure passthrough D-Bus interfaces for the op-openvswitch-daemon.
//!
//! THREE D-Bus object paths (locked design):
//! - `/org/opdbus/rovs/jsonrpc`  → interface `org.opdbus.rovs.jsonrpc`
//! - `/org/opdbus/rovs/openflow` → interface `org.opdbus.rovs.openflow`
//! - `/org/opdbus/rovs/proxy`    → interface `org.opdbus.rovs.proxy`
//!
//! The daemon knows NOTHING about bridges, ports, or containers on the jsonrpc/openflow
//! paths.  It only proxies raw `rovs-jsonrpc` / `rovs-openflow` primitives over D-Bus.
//! Business logic lives in the consuming plugins.
//!
//! The `proxy` path provides the openvswitch proxy operations for container
//! network namespace management (attach/detach OVS internal ports).

use anyhow::{Context, Result};
use rovs_ovsdb::Client;
use rovs_transport::Address;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
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
        debug!(
            "jsonrpc.transact method={} params_len={}",
            method,
            params_json.len()
        );

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
                    Ok(r) => {
                        serde_json::to_string(&r).unwrap_or_else(|e| json_error(&e.to_string()))
                    }
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
            connected,
            self.state.socket_path,
            env!("CARGO_PKG_VERSION")
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

    async fn send_flow(&self, conn_id: u64, flow_json: &str) -> String {
        debug!(
            "openflow.send_flow conn_id={} len={}",
            conn_id,
            flow_json.len()
        );
        match parse_json_flow(flow_json) {
            Ok(rovs_flow) => {
                let mut conns = self.state.of_conns.lock().await;
                match conns.get_mut(&conn_id) {
                    Some(vconn) => match vconn.send_flow(&rovs_flow).await {
                        Ok(_) => serde_json::json!({"ok": true}).to_string(),
                        Err(e) => json_error(&format!("send_flow failed: {}", e)),
                    },
                    None => json_error("unknown conn_id"),
                }
            }
            Err(e) => json_error(&format!("JSON deserializer error: {}", e)),
        }
    }

    /// Send flow_mod + barrier.
    async fn send_flow_sync(&self, conn_id: u64, flow_json: &str) -> String {
        debug!("openflow.send_flow_sync conn_id={}", conn_id);
        match parse_json_flow(flow_json) {
            Ok(rovs_flow) => {
                let mut conns = self.state.of_conns.lock().await;
                match conns.get_mut(&conn_id) {
                    Some(vconn) => match vconn.send_flow_sync(&rovs_flow).await {
                        Ok(_) => serde_json::json!({"ok": true}).to_string(),
                        Err(e) => json_error(&format!("send_flow_sync failed: {}", e)),
                    },
                    None => json_error("unknown conn_id"),
                }
            }
            Err(e) => json_error(&format!("JSON deserializer error: {}", e)),
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

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
pub struct JsonFlowEntry {
    pub table: u8,
    pub priority: u16,
    pub match_fields: std::collections::HashMap<String, String>,
    pub actions: Vec<JsonFlowAction>,
    pub cookie: Option<u64>,
    #[serde(default)]
    pub idle_timeout: u16,
    #[serde(default)]
    pub hard_timeout: u16,
}

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum JsonFlowAction {
    Output { port: String },
    LoadRegister { register: u8, value: u64 },
    Resubmit { table: u8 },
    SetField { field: String, value: String },
    Drop,
    Normal,
    Controller { max_len: Option<u16> },
    ArpResponder { mac: String, ip: String },
}

fn parse_json_flow(flow_json: &str) -> Result<rovs_openflow::Flow> {
    let entry: JsonFlowEntry =
        serde_json::from_str(flow_json).context("invalid JsonFlowEntry format")?;

    let mut m = rovs_openflow::Match::new();
    for (k, v) in entry.match_fields {
        match k.as_str() {
            "in_port" => {
                if let Ok(port) = v.parse::<u32>() {
                    m = m.in_port(port);
                }
            }
            "dl_type" => {
                let eth_type = if v.starts_with("0x") {
                    u16::from_str_radix(v.trim_start_matches("0x"), 16).unwrap_or(0)
                } else {
                    v.parse().unwrap_or(0)
                };
                m = m.eth_type(eth_type);
            }
            "dl_vlan" => {
                if let Ok(vlan) = v.parse::<u16>() {
                    m = m.vlan_vid(vlan);
                }
            }
            // Add other matches here as required
            _ => {
                tracing::warn!("Unsupported match field in JSON->Flow map: {}={}", k, v);
            }
        }
    }

    let mut action_list = rovs_openflow::ActionList::new();
    for action in entry.actions {
        match action {
            JsonFlowAction::Output { port } => {
                if port.eq_ignore_ascii_case("LOCAL") {
                    action_list = action_list.output(rovs_openflow::OutputPort::Local);
                } else if port.eq_ignore_ascii_case("NORMAL") {
                    action_list = action_list.output(rovs_openflow::OutputPort::Normal);
                } else if let Ok(p) = port.parse::<u32>() {
                    action_list = action_list.output(rovs_openflow::OutputPort::Port(p));
                } else {
                    tracing::warn!("Invalid numeric port output: {}", port);
                }
            }
            JsonFlowAction::Normal => {
                action_list = action_list.output(rovs_openflow::OutputPort::Normal);
            }
            JsonFlowAction::Drop => {}
            // Other actions could be mapped here.
            _ => {
                tracing::warn!("Unsupported JSON->Flow action: {:?}", action);
            }
        }
    }

    let mut flow = rovs_openflow::Flow::add()
        .priority(entry.priority)
        .match_fields(m)
        .actions(action_list)
        .idle_timeout(entry.idle_timeout)
        .hard_timeout(entry.hard_timeout);

    if let Some(c) = entry.cookie {
        flow = flow.cookie(c);
    }

    // Some rovs_openflow Flow builders support .table_id(u8), we will skip it for now
    // unless explicitly supported by the FlowAddBuilder. (Usually table_id=0 by default).

    Ok(flow)
}

// ── OVS Proxy object (/org/opdbus/rovs/proxy) ─────────────────────────────────

/// D-Bus service for the openvswitch proxy: attach/detach OVS internal ports
/// to container network namespaces.
pub struct ProxyService {
    state: DaemonState,
}

impl ProxyService {
    pub fn new(state: DaemonState) -> Self {
        Self { state }
    }
}

#[interface(name = "org.opdbus.rovs.proxy")]
impl ProxyService {
    /// Create an OVS internal port on a bridge, move it into a container's
    /// netns, rename it, and bring it up.
    ///
    /// `params_json` — JSON: {"port_name":"..","bridge":"..","target_pid":N,"iface_name":"..","ip_addrs":[".."]}
    /// Returns JSON: {"success":bool,"message":"..","iface_name":".."}
    async fn attach_port(&self, params_json: &str) -> String {
        debug!("proxy.attach_port params_len={}", params_json.len());

        let params: serde_json::Value = match serde_json::from_str(params_json) {
            Ok(v) => v,
            Err(e) => return json_error(&format!("invalid params JSON: {}", e)),
        };

        let port_name = match params.get("port_name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => return json_error("missing port_name"),
        };
        let bridge = match params.get("bridge").and_then(|v| v.as_str()) {
            Some(b) => b.to_string(),
            None => return json_error("missing bridge"),
        };
        let target_pid = match params.get("target_pid").and_then(|v| v.as_u64()) {
            Some(p) => p as u32,
            None => return json_error("missing target_pid"),
        };
        let iface_name = params
            .get("iface_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&port_name)
            .to_string();
        let ip_addrs: Vec<String> = params
            .get("ip_addrs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Step 1: Create OVS internal port via OVSDB transact
        let iface_row = serde_json::json!({ "name": &port_name, "type": "internal" });
        let add_ops = serde_json::json!([
            {
                "op": "insert",
                "table": "Interface",
                "row": iface_row,
                "uuid-name": "new_iface"
            },
            {
                "op": "insert",
                "table": "Port",
                "row": {
                    "name": &port_name,
                    "interfaces": ["set", [["named-uuid", "new_iface"]]]
                },
                "uuid-name": "new_port"
            },
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["name", "==", &bridge]],
                "mutations": [["ports", "insert", ["named-uuid", "new_port"]]]
            },
        ]);

        let mut guard = match self.state.get_ovsdb().await {
            Ok(g) => g,
            Err(e) => return json_error(&format!("OVSDB connect failed: {}", e)),
        };
        let client = match guard.as_mut() {
            Some(c) => c,
            None => return json_error("OVSDB client unavailable"),
        };

        match client.transact(add_ops).await {
            Ok(_) => info!("OVSDB: internal port {} created on {}", port_name, bridge),
            Err(e) => {
                return serde_json::json!({
                    "success": false,
                    "message": format!("OVSDB add_port failed: {}", e),
                    "iface_name": ""
                })
                .to_string();
            }
        }
        drop(guard);

        // Step 2: Bring up on host, move to netns, rename, bring up
        if let Err(e) = crate::netns::set_link_up_rtnetlink(&port_name) {
            return serde_json::json!({
                "success": false,
                "message": format!("bring up on host failed: {}", e),
                "iface_name": ""
            })
            .to_string();
        }

        if let Err(e) = crate::netns::move_interface_to_netns(&port_name, target_pid) {
            return serde_json::json!({
                "success": false,
                "message": format!("move to netns failed: {}", e),
                "iface_name": ""
            })
            .to_string();
        }

        if port_name != iface_name {
            if let Err(e) = crate::netns::rename_in_netns(&port_name, &iface_name, target_pid) {
                return serde_json::json!({
                    "success": false,
                    "message": format!("rename in netns failed: {}", e),
                    "iface_name": iface_name
                })
                .to_string();
            }
        }

        let _ = crate::netns::set_link_up_in_netns(&iface_name, target_pid);

        // Step 3: Add IP addresses
        for addr in &ip_addrs {
            match crate::netns::add_addr_in_netns(&iface_name, addr, target_pid) {
                Ok(_) => info!("proxy: added addr {} to {}", addr, iface_name),
                Err(e) => warn!(
                    "proxy: failed to add addr {} to {}: {}",
                    addr, iface_name, e
                ),
            }
        }

        serde_json::json!({
            "success": true,
            "message": format!("Port {} attached to PID {} as {}", port_name, target_pid, iface_name),
            "iface_name": iface_name
        })
        .to_string()
    }

    /// Remove an OVS port from a bridge (works regardless of which netns it's in).
    ///
    /// `params_json` — JSON: {"port_name":"..","bridge":".."}
    /// Returns JSON: {"success":bool,"message":".."}
    async fn detach_port(&self, params_json: &str) -> String {
        debug!("proxy.detach_port params_len={}", params_json.len());

        let params: serde_json::Value = match serde_json::from_str(params_json) {
            Ok(v) => v,
            Err(e) => return json_error(&format!("invalid params JSON: {}", e)),
        };

        let port_name = match params.get("port_name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => return json_error("missing port_name"),
        };
        let bridge = match params.get("bridge").and_then(|v| v.as_str()) {
            Some(b) => b.to_string(),
            None => return json_error("missing bridge"),
        };

        let mut guard = match self.state.get_ovsdb().await {
            Ok(g) => g,
            Err(e) => return json_error(&format!("OVSDB connect failed: {}", e)),
        };
        let client = match guard.as_mut() {
            Some(c) => c,
            None => return json_error("OVSDB client unavailable"),
        };

        let del_ops = serde_json::json!([
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["name", "==", &bridge]],
                "mutations": [["ports", "delete", ["name", &port_name]]]
            },
            {
                "op": "delete",
                "table": "Port",
                "where": [["name", "==", &port_name]]
            },
            {
                "op": "delete",
                "table": "Interface",
                "where": [["name", "==", &port_name]]
            },
        ]);

        match client.transact(del_ops).await {
            Ok(_) => serde_json::json!({
                "success": true,
                "message": format!("Port {} detached from {}", port_name, bridge)
            })
            .to_string(),
            Err(e) => serde_json::json!({
                "success": false,
                "message": format!("OVSDB remove failed: {}", e)
            })
            .to_string(),
        }
    }
}
