//! D-Bus proxies for op-openvswitch-daemon
//!
//! These zbus proxies provide consumer-side access to the `org.opdbus.rovs.jsonrpc`
//! and `org.opdbus.rovs.openflow` interfaces exposed by op-openvswitch-daemon.
//!
//! Per AGENTS.md §4: D-Bus is the ONLY control plane.

use anyhow::{anyhow, Result};
use tracing::debug;

const ROV_JSONRPC_BUS_NAME: &str = "org.opdbus.v1";
const ROV_JSONRPC_OBJECT_PATH: &str = "/opdbus/v1/plugins/rovs_daemon/jsonrpc";
const ROV_JSONRPC_INTERFACE: &str = "org.opdbus.rovs.jsonrpc";

const ROV_OPENFLOW_BUS_NAME: &str = "org.opdbus.v1";
const ROV_OPENFLOW_OBJECT_PATH: &str = "/opdbus/v1/plugins/rovs_daemon/openflow";
const ROV_OPENFLOW_INTERFACE: &str = "org.opdbus.rovs.openflow";

/// RovsJsonRpcProxy - D-Bus proxy for JSON-RPC operations
#[derive(Debug)]
pub struct RovsJsonRpcProxy {
    proxy: zbus::Proxy<'static>,
}

impl RovsJsonRpcProxy {
    /// Create a new proxy on the system bus
    pub async fn new() -> Result<Self> {
        let conn = zbus::Connection::system()
            .await
            .map_err(|e| anyhow!("Failed to connect to system D-Bus: {}", e))?;

        let proxy: zbus::Proxy<'_> = zbus::proxy::Builder::new(&conn)
            .destination(ROV_JSONRPC_BUS_NAME)?
            .path(ROV_JSONRPC_OBJECT_PATH)?
            .interface(ROV_JSONRPC_INTERFACE)?
            .build()
            .await
            .map_err(|e| anyhow!("Failed to build RovsJsonRpcProxy: {}", e))?;

        Ok(Self { proxy })
    }

    /// Execute a JSON-RPC transact
    pub async fn transact(&self, method: &str, params_json: &str) -> Result<String> {
        debug!("jsonrpc.transact method={} params_len={}", method, params_json.len());
        let result: String = self
            .proxy
            .call("Transact", &(method, params_json))
            .await
            .map_err(|e| anyhow!("Transact call failed: {}", e))?;
        Ok(result)
    }

    /// Send a raw message
    pub async fn send_message(&self, msg: &str) -> Result<String> {
        debug!("jsonrpc.send_message len={}", msg.len());
        let result: String = self
            .proxy
            .call("SendMessage", &(msg,))
            .await
            .map_err(|e| anyhow!("SendMessage call failed: {}", e))?;
        Ok(result)
    }

    /// Receive a message
    pub async fn recv_message(&self) -> Result<String> {
        let result: String = self
            .proxy
            .call("RecvMessage", &())
            .await
            .map_err(|e| anyhow!("RecvMessage call failed: {}", e))?;
        Ok(result)
    }

    /// Get next request id
    pub async fn next_id(&self) -> Result<u64> {
        let result: String = self
            .proxy
            .call("NextId", &())
            .await
            .map_err(|e| anyhow!("NextId call failed: {}", e))?;
        result.parse().map_err(|e| anyhow!("Failed to parse NextId result: {}", e))
    }

    /// Check for pending notifications
    pub async fn has_pending_notifications(&self) -> Result<bool> {
        let result: bool = self
            .proxy
            .call("HasPendingNotifications", &())
            .await
            .map_err(|e| anyhow!("HasPendingNotifications call failed: {}", e))?;
        Ok(result)
    }

    /// Get pending notification count
    pub async fn pending_notification_count(&self) -> Result<u64> {
        let result: String = self
            .proxy
            .call("PendingNotificationCount", &())
            .await
            .map_err(|e| anyhow!("PendingNotificationCount call failed: {}", e))?;
        result.parse().map_err(|e| anyhow!("Failed to parse pending count: {}", e))
    }

    /// Pop one notification
    pub async fn pop_notification(&self) -> Result<String> {
        let result: String = self
            .proxy
            .call("PopNotification", &())
            .await
            .map_err(|e| anyhow!("PopNotification call failed: {}", e))?;
        Ok(result)
    }

    /// Drain all notifications
    pub async fn drain_notifications(&self) -> Result<String> {
        let result: String = self
            .proxy
            .call("DrainNotifications", &())
            .await
            .map_err(|e| anyhow!("DrainNotifications call failed: {}", e))?;
        Ok(result)
    }

    /// Open a new stream
    pub async fn new_stream(&self, stream: &str) -> Result<String> {
        debug!("jsonrpc.new_stream stream={}", stream);
        let result: String = self
            .proxy
            .call("NewStream", &(stream,))
            .await
            .map_err(|e| anyhow!("NewStream call failed: {}", e))?;
        Ok(result)
    }
}

/// RovsOpenFlowProxy - D-Bus proxy for OpenFlow operations
#[derive(Debug)]
pub struct RovsOpenFlowProxy {
    proxy: zbus::Proxy<'static>,
}

impl RovsOpenFlowProxy {
    /// Create a new proxy on the system bus
    pub async fn new() -> Result<Self> {
        let conn = zbus::Connection::system()
            .await
            .map_err(|e| anyhow!("Failed to connect to system D-Bus: {}", e))?;

        let proxy: zbus::Proxy<'_> = zbus::proxy::Builder::new(&conn)
            .destination(ROV_OPENFLOW_BUS_NAME)?
            .path(ROV_OPENFLOW_OBJECT_PATH)?
            .interface(ROV_OPENFLOW_INTERFACE)?
            .build()
            .await
            .map_err(|e| anyhow!("Failed to build RovsOpenFlowProxy: {}", e))?;

        Ok(Self { proxy })
    }

    /// Connect to an OpenFlow switch
    pub async fn connect(&self, addr: &str) -> Result<String> {
        debug!("openflow.connect addr={}", addr);
        let result: String = self
            .proxy
            .call("Connect", &(addr,))
            .await
            .map_err(|e| anyhow!("Connect call failed: {}", e))?;
        Ok(result)
    }

    /// Get OpenFlow version
    pub async fn version(&self, conn_id: u64) -> Result<String> {
        debug!("openflow.version conn_id={}", conn_id);
        let result: String = self
            .proxy
            .call("Version", &(conn_id,))
            .await
            .map_err(|e| anyhow!("Version call failed: {}", e))?;
        Ok(result)
    }

    /// Send a flow
    pub async fn send_flow(&self, conn_id: u64, flow_json: &str) -> Result<String> {
        debug!(
            "openflow.send_flow conn_id={} len={}",
            conn_id,
            flow_json.len()
        );
        let result: String = self
            .proxy
            .call("SendFlow", &(conn_id, flow_json))
            .await
            .map_err(|e| anyhow!("SendFlow call failed: {}", e))?;
        Ok(result)
    }

    /// Send a flow synchronously
    pub async fn send_flow_sync(&self, conn_id: u64, flow_json: &str) -> Result<String> {
        debug!("openflow.send_flow_sync conn_id={}", conn_id);
        let result: String = self
            .proxy
            .call("SendFlowSync", &(conn_id, flow_json))
            .await
            .map_err(|e| anyhow!("SendFlowSync call failed: {}", e))?;
        Ok(result)
    }

    /// Send an echo request
    pub async fn echo(&self, conn_id: u64) -> Result<String> {
        debug!("openflow.echo conn_id={}", conn_id);
        let result: String = self
            .proxy
            .call("Echo", &(conn_id,))
            .await
            .map_err(|e| anyhow!("Echo call failed: {}", e))?;
        Ok(result)
    }

    /// Send a barrier request
    pub async fn barrier(&self, conn_id: u64) -> Result<String> {
        debug!("openflow.barrier conn_id={}", conn_id);
        let result: String = self
            .proxy
            .call("Barrier", &(conn_id,))
            .await
            .map_err(|e| anyhow!("Barrier call failed: {}", e))?;
        Ok(result)
    }

    /// Dump all flows
    pub async fn dump_flows(&self, conn_id: u64) -> Result<Vec<String>> {
        debug!("openflow.dump_flows conn_id={}", conn_id);
        let result: Vec<String> = self
            .proxy
            .call("DumpFlows", &(conn_id,))
            .await
            .map_err(|e| anyhow!("DumpFlows call failed: {}", e))?;
        Ok(result)
    }

    /// Dump flows with a filter
    pub async fn dump_flows_filtered(&self, conn_id: u64, request: &str) -> Result<Vec<String>> {
        debug!("openflow.dump_flows_filtered conn_id={}", conn_id);
        let result: Vec<String> = self
            .proxy
            .call("DumpFlowsFiltered", &(conn_id, request))
            .await
            .map_err(|e| anyhow!("DumpFlowsFiltered call failed: {}", e))?;
        Ok(result)
    }

    /// Receive a packet-in message
    pub async fn recv_packet_in(&self, conn_id: u64) -> Result<String> {
        debug!("openflow.recv_packet_in conn_id={}", conn_id);
        let result: String = self
            .proxy
            .call("RecvPacketIn", &(conn_id,))
            .await
            .map_err(|e| anyhow!("RecvPacketIn call failed: {}", e))?;
        Ok(result)
    }

    /// Try to receive a packet-in message (non-blocking)
    pub async fn try_recv_packet_in(&self, conn_id: u64) -> Result<Option<String>> {
        debug!("openflow.try_recv_packet_in conn_id={}", conn_id);
        let result: String = self
            .proxy
            .call("TryRecvPacketIn", &(conn_id,))
            .await
            .map_err(|e| anyhow!("TryRecvPacketIn call failed: {}", e))?;
        
        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    /// Monitor flows
    pub async fn monitor_flows(&self, conn_id: u64, request: &str) -> Result<Vec<String>> {
        debug!("openflow.monitor_flows conn_id={}", conn_id);
        let result: Vec<String> = self
            .proxy
            .call("MonitorFlows", &(conn_id, request))
            .await
            .map_err(|e| anyhow!("MonitorFlows call failed: {}", e))?;
        Ok(result)
    }

    /// Receive flow updates
    pub async fn recv_flow_updates(&self, conn_id: u64) -> Result<Vec<String>> {
        debug!("openflow.recv_flow_updates conn_id={}", conn_id);
        let result: Vec<String> = self
            .proxy
            .call("RecvFlowUpdates", &(conn_id,))
            .await
            .map_err(|e| anyhow!("RecvFlowUpdates call failed: {}", e))?;
        Ok(result)
    }

    /// Send a packet-out
    pub async fn send_packet_out(&self, conn_id: u64, packet_out_json: &str) -> Result<String> {
        debug!("openflow.send_packet_out conn_id={}", conn_id);
        let result: String = self
            .proxy
            .call("SendPacketOut", &(conn_id, packet_out_json))
            .await
            .map_err(|e| anyhow!("SendPacketOut call failed: {}", e))?;
        Ok(result)
    }
}

/// Convenience constructor: build a RovsOpenFlowProxy on the system bus.
pub async fn openflow_proxy() -> Result<RovsOpenFlowProxy> {
    RovsOpenFlowProxy::new().await
}

/// Convenience constructor: build a RovsJsonRpcProxy on the system bus.
pub async fn jsonrpc_proxy() -> Result<RovsJsonRpcProxy> {
    RovsJsonRpcProxy::new().await
}

/// Ensure both proxies are available.
pub async fn ensure_proxies() -> Result<(RovsJsonRpcProxy, RovsOpenFlowProxy)> {
    let jsonrpc = RovsJsonRpcProxy::new().await?;
    let openflow = RovsOpenFlowProxy::new().await?;
    Ok((jsonrpc, openflow))
}

/// Helper to parse flow stats from JSON string
pub fn flow_stats_to_json(entry: &rovs_openflow::FlowStatsEntry) -> String {
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

/// Simple hex encoder
fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xF) as usize] as char);
    }
    s
}