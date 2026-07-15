//! OpenFlow controller — D-Bus service (org.opdbus.of_controller).
//!
//! Maintains a passive TCP listener that OVS connects to, performs the
//! OpenFlow handshake + port discovery, and exposes the
//! `org.opdbus.rovs.openflow` D-Bus interface so the openflow plugin
//! (in projection_server) can call `send_flow` to install flows.
//!
//! NO flow logic lives here — no DELETE ALL, no port pairs. All flow
//! programming comes via D-Bus from the openflow plugin.
//!
//! Environment variables:
//!   OF_CONTROLLER_LISTEN   listen address (default: 10.200.0.1:6653)

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Shared state between the TCP handler and the D-Bus interface.
struct ControllerState {
    /// Write half of the TCP connection to OVS (None until OVS connects).
    writer: Option<tokio::io::WriteHalf<TcpStream>>,
    /// Port name → ofport number map (populated during handshake).
    port_map: HashMap<String, u32>,
    /// Negotiated OpenFlow version byte.
    version: u8,
}

/// D-Bus interface exposed at /org/opdbus/rovs/openflow.
///
/// This is the backend for the openflow plugin's RovsOpenFlowProxy.
/// The openflow plugin calls send_flow(flow_json) to install flows;
/// this controller translates the JSON to OpenFlow wire protocol and
/// sends it to OVS via the TCP connection.
#[zbus::interface(name = "org.opdbus.rovs.openflow")]
impl OfControllerDbus {
    async fn connect(&self, _addr: &str) -> Result<String, zbus::fdo::Error> {
        Ok(r#"{"status":"passive_mode"}"#.to_string())
    }

    async fn version(&self) -> Result<String, zbus::fdo::Error> {
        let st = self.state.lock().await;
        Ok(format!(r#"{{"version":{}}}"#, st.version))
    }

    async fn send_flow(&self, flow_json: &str) -> Result<String, zbus::fdo::Error> {
        let mut st = self.state.lock().await;
        let port_map = st.port_map.clone();
        let version = st.version;
        let writer = st
            .writer
            .as_mut()
            .ok_or_else(|| zbus::fdo::Error::Failed("OVS not connected".to_string()))?;

        match build_flow_mod(flow_json, &port_map, version) {
            Ok(bytes) => {
                writer
                    .write_all(&bytes)
                    .await
                    .map_err(|e| zbus::fdo::Error::Failed(format!("write failed: {e}")))?;
                Ok(r#"{"success":true}"#.to_string())
            }
            Err(e) => {
                warn!("send_flow failed: {e:#}");
                Err(zbus::fdo::Error::Failed(format!("build_flow_mod: {e}")))
            }
        }
    }

    async fn send_flow_sync(&self, flow_json: &str) -> Result<String, zbus::fdo::Error> {
        self.send_flow(flow_json).await
    }

    async fn ofctl(&self, _bridge: &str, _args_json: &str) -> Result<String, zbus::fdo::Error> {
        Err(zbus::fdo::Error::Failed(
            "ofctl passthrough not supported".to_string(),
        ))
    }

    async fn echo(&self) -> Result<String, zbus::fdo::Error> {
        let mut st = self.state.lock().await;
        let version = st.version;
        if let Some(w) = st.writer.as_mut() {
            let echo = [version, 2, 0, 8, 0, 0, 0, 1];
            w.write_all(&echo)
                .await
                .map_err(|e| zbus::fdo::Error::Failed(format!("echo write: {e}")))?;
        }
        Ok(r#"{"success":true}"#.to_string())
    }

    async fn barrier(&self) -> Result<String, zbus::fdo::Error> {
        let mut st = self.state.lock().await;
        let version = st.version;
        if let Some(w) = st.writer.as_mut() {
            let barrier = [version, 20, 0, 8, 0, 0, 0, 2];
            w.write_all(&barrier)
                .await
                .map_err(|e| zbus::fdo::Error::Failed(format!("barrier write: {e}")))?;
        }
        Ok(r#"{"success":true}"#.to_string())
    }

    async fn dump_flows(&self) -> Result<Vec<String>, zbus::fdo::Error> {
        Ok(vec![])
    }

    async fn dump_flows_filtered(&self, _request: &str) -> Result<Vec<String>, zbus::fdo::Error> {
        Ok(vec![])
    }

    async fn recv_packet_in(&self) -> Result<String, zbus::fdo::Error> {
        Err(zbus::fdo::Error::Failed("not implemented".to_string()))
    }

    async fn try_recv_packet_in(&self) -> Result<String, zbus::fdo::Error> {
        Ok(String::new())
    }

    async fn monitor_flows(&self, _request: &str) -> Result<Vec<String>, zbus::fdo::Error> {
        Ok(vec![])
    }

    async fn recv_flow_updates(&self) -> Result<Vec<String>, zbus::fdo::Error> {
        Ok(vec![])
    }

    async fn send_packet_out(&self, _packet_out_json: &str) -> Result<String, zbus::fdo::Error> {
        Err(zbus::fdo::Error::Failed("not implemented".to_string()))
    }

    async fn status(&self) -> Result<String, zbus::fdo::Error> {
        let st = self.state.lock().await;
        let connected = st.writer.is_some();
        Ok(format!(
            r#"{{"connected":{},"ports":{},"version":{}}}"#,
            connected,
            st.port_map.len(),
            st.version
        ))
    }
}

struct OfControllerDbus {
    state: Arc<Mutex<ControllerState>>,
}

// ── OpenFlow wire encoding ────────────────────────────────────────────────────

/// OF1.3/1.5 message types.
const OFPT_FLOW_MOD: u8 = 14;
const OFPT_ECHO_REQUEST: u8 = 2;
const OFPT_ECHO_REPLY: u8 = 3;
const OFPT_FEATURES_REQUEST: u8 = 5;
const OFPT_MULTIPART_REQUEST: u8 = 18;
const OFPMP_PORT_DESC: u16 = 13;

/// FlowMod command codes.
const OFPFC_ADD: u16 = 0;
const OFPFC_MODIFY: u16 = 1;
const OFPFC_MODIFY_STRICT: u16 = 2;
const OFPFC_DELETE: u16 = 3;
const OFPFC_DELETE_STRICT: u16 = 4;

/// Action types.
const OFPAT_OUTPUT: u16 = 0;
const OFPAT_SET_FIELD: u16 = 25;
const OFPAT_ENCAP: u16 = 26;
const OFPAT_DECAP: u16 = 27;

/// Special output ports.
const OFPP_LOCAL: u32 = 0xFFFF_FFFA;
const OFPP_NORMAL: u32 = 0xFFFF_FFFB;
const OFPP_FLOOD: u32 = 0xFFFF_FFFC;
const OFPP_ALL: u32 = 0xFFFF_FFFD;
const OFPP_IN_PORT: u32 = 0xFFFF_FFFF;
const OFPP_CONTROLLER: u32 = 0xFFFF_FFF3;

/// OXM class for OpenFlow Basic.
const OXM_CLASS_OPENFLOW_BASIC: u16 = 0x8000;

/// OXM field IDs (OpenFlow Basic class 0x8000).
const OXM_FIELD_IN_PORT: u8 = 0;
const OXM_FIELD_ETH_DST: u8 = 3;
const OXM_FIELD_ETH_SRC: u8 = 4;
const OXM_FIELD_ETH_TYPE: u8 = 5;
const OXM_FIELD_IP_PROTO: u8 = 10;
const OXM_FIELD_IPV4_SRC: u8 = 11;
const OXM_FIELD_IPV4_DST: u8 = 12;
const OXM_FIELD_TCP_SRC: u8 = 13;
const OXM_FIELD_TCP_DST: u8 = 14;
const OXM_FIELD_UDP_SRC: u8 = 15;
const OXM_FIELD_UDP_DST: u8 = 16;
const OXM_FIELD_PACKET_TYPE: u8 = 20;

/// Nicira NXM_1 class (OVS registers live here as NXM_NX_REGn).
const OXM_CLASS_NXM_1: u16 = 0x0001;

/// Build a raw OpenFlow message header.
fn of_header(version: u8, msg_type: u8, length: u16, xid: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8);
    buf.push(version);
    buf.push(msg_type);
    buf.extend_from_slice(&length.to_be_bytes());
    buf.extend_from_slice(&xid.to_be_bytes());
    buf
}

/// Parse a FlowEntry JSON and build a FlowMod wire message.
///
/// The FlowEntry JSON has the structure:
///   { "table": 0, "priority": 100, "match_fields": {...}, "actions": [...], ... }
fn build_flow_mod(
    flow_json: &str,
    port_map: &HashMap<String, u32>,
    version: u8,
) -> Result<Vec<u8>> {
    let flow: serde_json::Value = serde_json::from_str(flow_json).context("parse flow JSON")?;

    let table_id = flow.get("table").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    let priority = flow.get("priority").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
    let idle_timeout = flow
        .get("idle_timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u16;
    let hard_timeout = flow
        .get("hard_timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u16;
    let cookie = flow.get("cookie").and_then(|v| v.as_u64()).unwrap_or(0);
    let command = match flow.get("command").and_then(|v| v.as_str()) {
        Some("OFPFC_MODIFY") => OFPFC_MODIFY,
        Some("OFPFC_MODIFY_STRICT") => OFPFC_MODIFY_STRICT,
        Some("OFPFC_DELETE") => OFPFC_DELETE,
        Some("OFPFC_DELETE_STRICT") => OFPFC_DELETE_STRICT,
        _ => OFPFC_ADD,
    };

    // Build OXM match fields.
    let mut match_body = Vec::new();
    if let Some(match_fields) = flow.get("match_fields").and_then(|v| v.as_object()) {
        for (key, val) in match_fields {
            let val_str = val.as_str().unwrap_or("");
            match key.as_str() {
                "in_port" => {
                    let port_no = resolve_port(val_str, port_map);
                    oxm_basic(
                        &mut match_body,
                        OXM_FIELD_IN_PORT,
                        4,
                        &port_no.to_be_bytes(),
                    );
                }
                "eth_dst" => {
                    if let Some(mac) = parse_mac(val_str) {
                        oxm_basic(&mut match_body, OXM_FIELD_ETH_DST, 6, &mac);
                    }
                }
                "eth_src" => {
                    if let Some(mac) = parse_mac(val_str) {
                        oxm_basic(&mut match_body, OXM_FIELD_ETH_SRC, 6, &mac);
                    }
                }
                "eth_type" => {
                    if let Some(etype) = parse_u16(val_str) {
                        oxm_basic(&mut match_body, OXM_FIELD_ETH_TYPE, 2, &etype.to_be_bytes());
                    }
                }
                "ip_proto" => {
                    if let Ok(proto) = val_str.parse::<u8>() {
                        oxm_basic(&mut match_body, OXM_FIELD_IP_PROTO, 1, &[proto]);
                    }
                }
                "nw_src" | "ipv4_src" => {
                    if let Some(ip) = parse_ipv4(val_str) {
                        oxm_basic(&mut match_body, OXM_FIELD_IPV4_SRC, 4, &ip);
                    }
                }
                "nw_dst" | "ipv4_dst" => {
                    if let Some(ip) = parse_ipv4(val_str) {
                        oxm_basic(&mut match_body, OXM_FIELD_IPV4_DST, 4, &ip);
                    }
                }
                "tcp_src" | "tp_src" => {
                    if let Ok(port) = val_str.parse::<u16>() {
                        oxm_basic(&mut match_body, OXM_FIELD_TCP_SRC, 2, &port.to_be_bytes());
                    }
                }
                "tcp_dst" | "tp_dst" => {
                    if let Ok(port) = val_str.parse::<u16>() {
                        oxm_basic(&mut match_body, OXM_FIELD_TCP_DST, 2, &port.to_be_bytes());
                    }
                }
                "udp_src" => {
                    if let Ok(port) = val_str.parse::<u16>() {
                        oxm_basic(&mut match_body, OXM_FIELD_UDP_SRC, 2, &port.to_be_bytes());
                    }
                }
                "udp_dst" => {
                    if let Ok(port) = val_str.parse::<u16>() {
                        oxm_basic(&mut match_body, OXM_FIELD_UDP_DST, 2, &port.to_be_bytes());
                    }
                }
                "packet_type" => {
                    // Format: "1,0x0800" → ns=1, type=0x0800
                    let parts: Vec<&str> = val_str.split(',').collect();
                    if parts.len() == 2 {
                        let ns: u16 = parts[0].trim().parse().unwrap_or(0);
                        let pt: u16 =
                            u16::from_str_radix(parts[1].trim().trim_start_matches("0x"), 16)
                                .unwrap_or(0);
                        let val = ((ns as u32) << 16) | (pt as u32);
                        oxm_basic(
                            &mut match_body,
                            OXM_FIELD_PACKET_TYPE,
                            4,
                            &val.to_be_bytes(),
                        );
                    }
                }
                _ => {} // Skip unknown match fields for now
            }
        }
    }

    // Build actions.
    let mut actions_body = Vec::new();
    if let Some(actions) = flow.get("actions").and_then(|v| v.as_array()) {
        for action in actions {
            let action_type = action.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match action_type {
                "output" => {
                    let port_str = action.get("port").and_then(|v| v.as_str()).unwrap_or("");
                    let port_no = resolve_port(port_str, port_map);
                    encode_output(&mut actions_body, port_no);
                }
                "set_field" => {
                    let field = action.get("field").and_then(|v| v.as_str()).unwrap_or("");
                    let value = action.get("value").and_then(|v| v.as_str()).unwrap_or("");
                    encode_set_field(&mut actions_body, field, value);
                }
                "load_register" => {
                    // FlowAction::LoadRegister serializes as snake_case tag.
                    let register =
                        action.get("register").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                    let value = action.get("value").and_then(|v| v.as_u64()).unwrap_or(0);
                    encode_load_register(&mut actions_body, register, value);
                }
                "encap" => {
                    let ether_type = action
                        .get("ether_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("ethernet");
                    let new_type: u16 = match ether_type {
                        "ethernet" => 1,
                        _ => 1,
                    };
                    encode_encap(&mut actions_body, new_type);
                }
                "decap" => {
                    encode_decap(&mut actions_body);
                }
                "drop" => {} // No action = drop
                "normal" => {
                    encode_output(&mut actions_body, OFPP_NORMAL);
                }
                _ => {} // Skip unknown actions
            }
        }
    }

    // Assemble FlowMod body (56 bytes fixed + match + actions).
    let match_length = 4 + match_body.len() as u16;
    // Pad match to 8-byte boundary
    let match_padded = (match_length + 7) & !7;
    let match_pad = match_padded as usize - match_length as usize;

    let total_len = 56 + match_padded as usize + actions_body.len();

    let mut msg = of_header(version, OFPT_FLOW_MOD, total_len as u16, 1);
    // cookie (8)
    msg.extend_from_slice(&cookie.to_be_bytes());
    // cookie_mask (8)
    msg.extend_from_slice(&[0xFF; 8]);
    // table_id (1)
    msg.push(table_id);
    // command (1)
    msg.push(command as u8);
    // idle_timeout (2)
    msg.extend_from_slice(&idle_timeout.to_be_bytes());
    // hard_timeout (2)
    msg.extend_from_slice(&hard_timeout.to_be_bytes());
    // priority (2)
    msg.extend_from_slice(&priority.to_be_bytes());
    // buffer_id (4) = OFP_NO_BUFFER
    msg.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
    // out_port (4) = OFPP_ANY
    msg.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
    // out_group (4) = OFPG_ANY
    msg.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
    // flags (2)
    msg.extend_from_slice(&0u16.to_be_bytes());
    // pad (2)
    msg.extend_from_slice(&0u16.to_be_bytes());
    // match: type=OFPMT_OXM(1) + length(2) + pad(4) = 8 bytes header... actually:
    // match type (2) = OFPMT_OXM = 1
    msg.extend_from_slice(&1u16.to_be_bytes());
    // match length (2)
    msg.extend_from_slice(&match_length.to_be_bytes());
    // oxm fields
    msg.extend_from_slice(&match_body);
    // match padding
    msg.extend_from_slice(&vec![0u8; match_pad]);
    // actions
    msg.extend_from_slice(&actions_body);

    Ok(msg)
}

/// Encode an OXM field: header(4) + value(variable).
fn oxm_encode(buf: &mut Vec<u8>, class: u16, field: u8, length: u8, value: &[u8]) {
    let oxm_header = (class as u32) << 16 | (field as u32) << 9 | (length as u32);
    buf.extend_from_slice(&oxm_header.to_be_bytes());
    buf.extend_from_slice(value);
}

fn oxm_basic(buf: &mut Vec<u8>, field: u8, length: u8, value: &[u8]) {
    oxm_encode(buf, OXM_CLASS_OPENFLOW_BASIC, field, length, value);
}

/// Encode OFPAT_OUTPUT action (16 bytes).
fn encode_output(buf: &mut Vec<u8>, port: u32) {
    buf.extend_from_slice(&OFPAT_OUTPUT.to_be_bytes());
    buf.extend_from_slice(&16u16.to_be_bytes()); // len
    buf.extend_from_slice(&port.to_be_bytes()); // port
    buf.extend_from_slice(&0xFFFFu16.to_be_bytes()); // max_len = OFPCML_NO_BUFFER
    buf.extend_from_slice(&[0u8; 6]); // pad
}

/// Pad `buf` so its length is a multiple of 8 (OFPAT_SET_FIELD requirement).
fn pad8(buf: &mut Vec<u8>) {
    while buf.len() % 8 != 0 {
        buf.push(0);
    }
}

/// Encode OFPAT_SET_FIELD action for supported fields.
fn encode_set_field(buf: &mut Vec<u8>, field: &str, value: &str) {
    // OVS registers: set_field:VALUE->regN  (NXM_1 class, field = N).
    if let Some(reg_str) = field.strip_prefix("reg") {
        if let Ok(reg) = reg_str.parse::<u8>() {
            if let Some(v) = parse_u32(value) {
                encode_load_register(buf, reg, v as u64);
            }
            return;
        }
    }
    match field {
        "eth_dst" => {
            if let Some(mac) = parse_mac(value) {
                let start = buf.len();
                buf.extend_from_slice(&OFPAT_SET_FIELD.to_be_bytes());
                buf.extend_from_slice(&0u16.to_be_bytes()); // placeholder len
                oxm_basic(buf, OXM_FIELD_ETH_DST, 6, &mac);
                pad8(buf);
                let len = (buf.len() - start) as u16;
                buf[start + 2..start + 4].copy_from_slice(&len.to_be_bytes());
            }
        }
        "nw_src" | "ipv4_src" => {
            if let Some(ip) = parse_ipv4(value) {
                let start = buf.len();
                buf.extend_from_slice(&OFPAT_SET_FIELD.to_be_bytes());
                buf.extend_from_slice(&0u16.to_be_bytes());
                oxm_basic(buf, OXM_FIELD_IPV4_SRC, 4, &ip);
                pad8(buf);
                let len = (buf.len() - start) as u16;
                buf[start + 2..start + 4].copy_from_slice(&len.to_be_bytes());
            }
        }
        "nw_dst" | "ipv4_dst" => {
            if let Some(ip) = parse_ipv4(value) {
                let start = buf.len();
                buf.extend_from_slice(&OFPAT_SET_FIELD.to_be_bytes());
                buf.extend_from_slice(&0u16.to_be_bytes());
                oxm_basic(buf, OXM_FIELD_IPV4_DST, 4, &ip);
                pad8(buf);
                let len = (buf.len() - start) as u16;
                buf[start + 2..start + 4].copy_from_slice(&len.to_be_bytes());
            }
        }
        _ => {} // Skip unsupported set_field types for now
    }
}

/// Encode load into NXM_NX_REGn via OFPAT_SET_FIELD (OVS-compatible).
fn encode_load_register(buf: &mut Vec<u8>, register: u8, value: u64) {
    let start = buf.len();
    buf.extend_from_slice(&OFPAT_SET_FIELD.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes()); // placeholder len
                                                // NXM_NX_REG0..REG15: class NXM_1, field = register index, length 4.
    let v = (value as u32).to_be_bytes();
    oxm_encode(buf, OXM_CLASS_NXM_1, register, 4, &v);
    pad8(buf);
    let len = (buf.len() - start) as u16;
    buf[start + 2..start + 4].copy_from_slice(&len.to_be_bytes());
}

/// Encode OFPAT_ENCAP action (8 bytes).
fn encode_encap(buf: &mut Vec<u8>, new_type: u16) {
    buf.extend_from_slice(&OFPAT_ENCAP.to_be_bytes());
    buf.extend_from_slice(&8u16.to_be_bytes()); // len
    buf.extend_from_slice(&new_type.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes()); // pad
}

/// Encode OFPAT_DECAP action (8 bytes).
fn encode_decap(buf: &mut Vec<u8>) {
    buf.extend_from_slice(&OFPAT_DECAP.to_be_bytes());
    buf.extend_from_slice(&8u16.to_be_bytes()); // len
    buf.extend_from_slice(&0xFFFFu16.to_be_bytes()); // all types
    buf.extend_from_slice(&0u16.to_be_bytes()); // pad
}

/// Resolve a port name string to an ofport number.
fn resolve_port(name: &str, port_map: &HashMap<String, u32>) -> u32 {
    match name {
        "LOCAL" => OFPP_LOCAL,
        "NORMAL" => OFPP_NORMAL,
        "FLOOD" => OFPP_FLOOD,
        "ALL" => OFPP_ALL,
        "IN_PORT" => OFPP_IN_PORT,
        "CONTROLLER" => OFPP_CONTROLLER,
        _ => *port_map.get(name).unwrap_or(&0),
    }
}

/// Parse decimal or 0x-hex u16.
fn parse_u16(s: &str) -> Option<u16> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).ok()
    } else {
        s.parse().ok()
    }
}

/// Parse decimal or 0x-hex u32.
fn parse_u32(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse().ok()
    }
}

/// Parse dotted IPv4 to 4 bytes.
fn parse_ipv4(s: &str) -> Option<[u8; 4]> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let mut ip = [0u8; 4];
    for (i, p) in parts.iter().enumerate() {
        ip[i] = p.parse().ok()?;
    }
    Some(ip)
}

/// Parse a MAC address string (aa:bb:cc:dd:ee:ff) to 6 bytes.
fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 6 {
        return None;
    }
    let mut mac = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        mac[i] = u8::from_str_radix(part, 16).ok()?;
    }
    Some(mac)
}

// ── TCP connection handler ────────────────────────────────────────────────────

/// Handle an inbound OVS connection: handshake, port discovery, then
/// store the write half in shared state and run a keepalive reader.
async fn handle_connection(stream: TcpStream, state: Arc<Mutex<ControllerState>>) -> Result<()> {
    let (mut reader, mut writer) = tokio::io::split(stream);

    // Negotiate version — send OF1.5 Hello, accept whatever OVS offers.
    let hello = [0x06u8, 0, 0, 8, 0, 0, 0, 1]; // OF1.5 Hello
    reader_write_pair(&mut reader, &mut writer, &hello).await?;

    // Read OVS Hello
    let mut hdr = [0u8; 8];
    reader.read_exact(&mut hdr).await.context("read hello")?;
    let ovs_version = hdr[0];
    let version = std::cmp::min(0x06, ovs_version);
    info!("OVS connected, negotiated OF version 0x{:02x}", version);

    // Send FeaturesRequest
    let feat_req = [version, OFPT_FEATURES_REQUEST, 0, 8, 0, 0, 0, 2];
    reader_write_pair(&mut reader, &mut writer, &feat_req).await?;

    // Wait for FeaturesReply (type 6), echo any requests
    loop {
        reader.read_exact(&mut hdr).await?;
        let msg_type = hdr[1];
        let len = u16::from_be_bytes([hdr[2], hdr[3]]) as usize;
        let mut body = vec![0u8; len.saturating_sub(8)];
        if body.len() > 0 {
            reader.read_exact(&mut body).await?;
        }
        match msg_type {
            2 /* EchoRequest */ => {
                let reply = [version, OFPT_ECHO_REPLY, 0, 8, 0, 0, 0, 3];
                writer_write(&mut writer, &reply).await?;
            }
            6 /* FeaturesReply */ => break,
            _ => {}
        }
    }

    // PortDesc discovery
    let port_desc_req = build_multipart_request(version, OFPMP_PORT_DESC);
    writer_write(&mut writer, &port_desc_req).await?;

    let mut port_map = HashMap::new();
    loop {
        reader.read_exact(&mut hdr).await?;
        let msg_type = hdr[1];
        let len = u16::from_be_bytes([hdr[2], hdr[3]]) as usize;
        let mut body = vec![0u8; len.saturating_sub(8)];
        if body.len() > 0 {
            reader.read_exact(&mut body).await?;
        }
        match msg_type {
            2 /* EchoRequest */ => {
                let reply = [version, OFPT_ECHO_REPLY, 0, 8, 0, 0, 0, 4];
                writer_write(&mut writer, &reply).await?;
            }
            19 /* MultipartReply */ => {
                if body.len() >= 8 {
                    let reply_type = u16::from_be_bytes([body[0], body[1]]);
                    let flags = u16::from_be_bytes([body[2], body[3]]);
                    if reply_type == OFPMP_PORT_DESC {
                        for chunk in body[8..].chunks_exact(64) {
                            let port_no = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                            let name = String::from_utf8_lossy(&chunk[16..32])
                                .trim_end_matches('\0').to_string();
                            if !name.is_empty() && port_no < 0xFFFF_FFFF {
                                port_map.insert(name, port_no);
                            }
                        }
                        if flags & 1 == 0 { break; }
                    }
                }
            }
            _ => {}
        }
    }

    info!(
        "Discovered {} ports: {:?}",
        port_map.len(),
        port_map.keys().collect::<Vec<_>>()
    );

    // Store write half in shared state.
    {
        let mut st = state.lock().await;
        st.writer = Some(writer);
        st.port_map = port_map;
        st.version = version;
    }
    info!("Controller ready for D-Bus flow commands");

    // Keepalive reader — handle echo requests from OVS.
    loop {
        reader.read_exact(&mut hdr).await?;
        let msg_type = hdr[1];
        let len = u16::from_be_bytes([hdr[2], hdr[3]]) as usize;
        let mut body = vec![0u8; len.saturating_sub(8)];
        if body.len() > 0 {
            reader.read_exact(&mut body).await?;
        }
        if msg_type == 2
        /* EchoRequest */
        {
            // Reply via the shared writer
            let mut st = state.lock().await;
            if let Some(w) = st.writer.as_mut() {
                let mut reply = vec![version, OFPT_ECHO_REPLY, 0, 8];
                reply.extend_from_slice(&hdr[4..8]); // mirror xid
                w.write_all(&reply).await.ok();
            }
        }
    }
}

async fn reader_write_pair(
    _reader: &mut tokio::io::ReadHalf<TcpStream>,
    writer: &mut tokio::io::WriteHalf<TcpStream>,
    msg: &[u8],
) -> Result<()> {
    writer_write(writer, msg).await
}

async fn writer_write(writer: &mut tokio::io::WriteHalf<TcpStream>, msg: &[u8]) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    writer.write_all(msg).await.context("write to OVS")
}

fn build_multipart_request(version: u8, mp_type: u16) -> Vec<u8> {
    let mut body = [0u8; 8];
    body[0..2].copy_from_slice(&mp_type.to_be_bytes());
    let mut msg = of_header(version, OFPT_MULTIPART_REQUEST, 16, 3);
    msg.extend_from_slice(&body);
    msg
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("op_network=info".parse()?),
        )
        .init();

    let listen: SocketAddr = std::env::var("OF_CONTROLLER_LISTEN")
        .unwrap_or_else(|_| "10.200.0.1:6653".to_string())
        .parse()
        .expect("OF_CONTROLLER_LISTEN must be a valid socket address");

    let state = Arc::new(Mutex::new(ControllerState {
        writer: None,
        port_map: HashMap::new(),
        version: 0x04,
    }));

    // Start D-Bus service.
    let dbus_state = state.clone();
    let dbus_task = tokio::spawn(async move {
        let conn = zbus::Connection::system()
            .await
            .context("connect to system D-Bus")?;
        let interface = OfControllerDbus { state: dbus_state };
        conn.object_server()
            .at("/org/opdbus/rovs/openflow", interface)
            .await
            .context("register D-Bus object")?;
        conn.request_name("org.opdbus.of_controller")
            .await
            .context("request D-Bus name")?;
        info!("D-Bus service org.opdbus.of_controller registered");
        // Keep the connection alive forever.
        std::future::pending::<()>().await;
        anyhow::Ok(())
    });

    // Start TCP listener for OVS connections.
    let listener = TcpListener::bind(listen)
        .await
        .with_context(|| format!("bind {}", listen))?;
    info!("OpenFlow controller listening on {}", listen);

    loop {
        let (stream, peer) = listener.accept().await?;
        info!("OVS connected from {}", peer);
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, state).await {
                warn!("OVS connection ended: {e:#}");
            }
        });
    }
}
