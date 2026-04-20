//! OpenFlow 1.3 controller server
//!
//! Listens for OVS to connect (passive mode), performs the OF1.3 handshake,
//! discovers port numbers by name, and installs flow rules.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ── OF1.3 constants ───────────────────────────────────────────────────────────

const OF13_VERSION: u8 = 0x04;

const OFPT_HELLO: u8 = 0;
const OFPT_ECHO_REQUEST: u8 = 2;
const OFPT_ECHO_REPLY: u8 = 3;
const OFPT_FEATURES_REQUEST: u8 = 5;
const OFPT_FEATURES_REPLY: u8 = 6;
const OFPT_FLOW_MOD: u8 = 14;
const OFPT_MULTIPART_REQUEST: u8 = 18;
const OFPT_MULTIPART_REPLY: u8 = 19;

const OFPMP_PORT_DESC: u16 = 13;
const OFPFC_ADD: u8 = 0;
const OFPFC_DELETE: u8 = 3;

const OFPP_ANY: u32 = 0xFFFF_FFFF;
const OFPG_ANY: u32 = 0xFFFF_FFFF;
const OFP_NO_BUFFER: u32 = 0xFFFF_FFFF;
const OFPCML_NO_BUFFER: u16 = 0xFFFF;

// ── Wire-encoding helpers ─────────────────────────────────────────────────────

fn build_header(msg_type: u8, body_len: usize, xid: u32) -> [u8; 8] {
    let total = (8 + body_len) as u16;
    let mut h = [0u8; 8];
    h[0] = OF13_VERSION;
    h[1] = msg_type;
    h[2..4].copy_from_slice(&total.to_be_bytes());
    h[4..8].copy_from_slice(&xid.to_be_bytes());
    h
}

fn build_hello(xid: u32) -> Vec<u8> {
    build_header(OFPT_HELLO, 0, xid).to_vec()
}

fn build_features_request(xid: u32) -> Vec<u8> {
    build_header(OFPT_FEATURES_REQUEST, 0, xid).to_vec()
}

fn build_port_desc_request(xid: u32) -> Vec<u8> {
    // body: type(2) + flags(2) + pad(4) = 8 bytes
    let mut body = [0u8; 8];
    body[0..2].copy_from_slice(&OFPMP_PORT_DESC.to_be_bytes());
    let mut msg = build_header(OFPT_MULTIPART_REQUEST, 8, xid).to_vec();
    msg.extend_from_slice(&body);
    msg
}

fn build_echo_reply(xid: u32, payload: &[u8]) -> Vec<u8> {
    let mut msg = build_header(OFPT_ECHO_REPLY, payload.len(), xid).to_vec();
    msg.extend_from_slice(payload);
    msg
}

fn build_flow_mod_delete_all(xid: u32) -> Vec<u8> {
    // Empty OXM match (wildcard all)
    let match_bytes = encode_wildcard_match();
    let body_len = 40 + match_bytes.len();
    let mut msg = build_header(OFPT_FLOW_MOD, body_len, xid).to_vec();

    msg.extend_from_slice(&0u64.to_be_bytes()); // cookie
    msg.extend_from_slice(&0u64.to_be_bytes()); // cookie_mask
    msg.push(0xFF); // table_id = OFPTT_ALL
    msg.push(OFPFC_DELETE);
    msg.extend_from_slice(&0u16.to_be_bytes()); // idle_timeout
    msg.extend_from_slice(&0u16.to_be_bytes()); // hard_timeout
    msg.extend_from_slice(&0u16.to_be_bytes()); // priority
    msg.extend_from_slice(&OFP_NO_BUFFER.to_be_bytes());
    msg.extend_from_slice(&OFPP_ANY.to_be_bytes());
    msg.extend_from_slice(&OFPG_ANY.to_be_bytes());
    msg.extend_from_slice(&0u16.to_be_bytes()); // flags
    msg.extend_from_slice(&0u16.to_be_bytes()); // importance
    msg.extend_from_slice(&match_bytes);
    msg
}

/// Build an OF1.3 FlowMod ADD: match in_port, output to out_port.
pub fn build_flow_mod_add(in_port: u32, out_port: u32, priority: u16, xid: u32) -> Vec<u8> {
    let match_bytes = encode_match_in_port(in_port);
    let output_action = encode_output_action(out_port);
    let instruction = encode_apply_actions(&output_action);

    let body_len = 40 + match_bytes.len() + instruction.len();
    let mut msg = build_header(OFPT_FLOW_MOD, body_len, xid).to_vec();

    msg.extend_from_slice(&0u64.to_be_bytes()); // cookie
    msg.extend_from_slice(&0u64.to_be_bytes()); // cookie_mask
    msg.push(0); // table_id
    msg.push(OFPFC_ADD);
    msg.extend_from_slice(&0u16.to_be_bytes()); // idle_timeout = permanent
    msg.extend_from_slice(&0u16.to_be_bytes()); // hard_timeout = permanent
    msg.extend_from_slice(&priority.to_be_bytes());
    msg.extend_from_slice(&OFP_NO_BUFFER.to_be_bytes());
    msg.extend_from_slice(&OFPP_ANY.to_be_bytes());
    msg.extend_from_slice(&OFPG_ANY.to_be_bytes());
    msg.extend_from_slice(&0u16.to_be_bytes()); // flags
    msg.extend_from_slice(&0u16.to_be_bytes()); // importance
    msg.extend_from_slice(&match_bytes);
    msg.extend_from_slice(&instruction);
    msg
}

fn encode_wildcard_match() -> Vec<u8> {
    // OFPMT_OXM with no OXM fields = match all
    // length = 4 (header only), padded to 8 bytes
    let mut m = Vec::with_capacity(8);
    m.extend_from_slice(&1u16.to_be_bytes()); // OFPMT_OXM
    m.extend_from_slice(&4u16.to_be_bytes()); // length = 4 (no fields)
    m.extend_from_slice(&[0u8; 4]); // pad to 8
    m
}

fn encode_match_in_port(port: u32) -> Vec<u8> {
    // OXM TLV for OXM_OF_IN_PORT:
    //   class=0x8000, field=0, hasmask=0, len=4  → header bytes: 0x80 0x00 0x00 0x04
    let oxm: [u8; 8] = [
        0x80, 0x00, 0x00, 0x04,
        (port >> 24) as u8, (port >> 16) as u8, (port >> 8) as u8, port as u8,
    ];
    let length = (4u16 + oxm.len() as u16).to_be_bytes(); // 4 (match header) + 8 (OXM)
    let mut m = Vec::with_capacity(16);
    m.extend_from_slice(&1u16.to_be_bytes()); // OFPMT_OXM
    m.extend_from_slice(&length);             // length = 12
    m.extend_from_slice(&oxm);                // OXM TLV
    // pad to 8-byte boundary (12 bytes → 4 bytes padding → 16 total)
    let raw_len = 2 + 2 + oxm.len(); // 12
    let pad = (8 - (raw_len % 8)) % 8;
    m.extend(std::iter::repeat(0u8).take(pad));
    m
}

fn encode_output_action(port: u32) -> Vec<u8> {
    // OFPAT_OUTPUT: type(2)+len(2)+port(4)+max_len(2)+pad(6) = 16 bytes
    let mut a = [0u8; 16];
    // type = 0 (already 0)
    a[2..4].copy_from_slice(&16u16.to_be_bytes()); // len
    a[4..8].copy_from_slice(&port.to_be_bytes());
    a[8..10].copy_from_slice(&OFPCML_NO_BUFFER.to_be_bytes());
    // remaining 6 bytes = pad (already 0)
    a.to_vec()
}

fn encode_apply_actions(actions: &[u8]) -> Vec<u8> {
    // OFPIT_APPLY_ACTIONS: type(2)+len(2)+pad(4)+actions
    let len = (8 + actions.len()) as u16;
    let mut i = Vec::with_capacity(8 + actions.len());
    i.extend_from_slice(&4u16.to_be_bytes()); // OFPIT_APPLY_ACTIONS = 4
    i.extend_from_slice(&len.to_be_bytes());
    i.extend_from_slice(&[0u8; 4]); // pad
    i.extend_from_slice(actions);
    i
}

// ── Message receive ───────────────────────────────────────────────────────────

struct RawMsg {
    msg_type: u8,
    xid: u32,
    payload: Vec<u8>,
}

async fn recv_msg(stream: &mut TcpStream) -> Result<RawMsg> {
    let mut hdr = [0u8; 8];
    stream.read_exact(&mut hdr).await.context("reading OF header")?;
    let msg_type = hdr[1];
    let length = u16::from_be_bytes([hdr[2], hdr[3]]) as usize;
    let xid = u32::from_be_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]);
    let payload_len = length.saturating_sub(8);
    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        stream.read_exact(&mut payload).await.context("reading OF payload")?;
    }
    Ok(RawMsg { msg_type, xid, payload })
}

async fn send_msg(stream: &mut TcpStream, bytes: &[u8]) -> Result<()> {
    stream.write_all(bytes).await.context("sending OF message")
}

// ── Port discovery ─────────────────────────────────────────────────────────

/// Parse OF1.3 port descriptor replies → {port_name: ofport_no}
async fn discover_ports(stream: &mut TcpStream, xid: u32) -> Result<HashMap<String, u32>> {
    send_msg(stream, &build_port_desc_request(xid)).await?;

    let mut ports: HashMap<String, u32> = HashMap::new();

    loop {
        let msg = recv_msg(stream).await?;
        match msg.msg_type {
            OFPT_ECHO_REQUEST => {
                send_msg(stream, &build_echo_reply(msg.xid, &msg.payload)).await?;
            }
            OFPT_MULTIPART_REPLY => {
                if msg.payload.len() < 8 {
                    break;
                }
                let reply_type = u16::from_be_bytes([msg.payload[0], msg.payload[1]]);
                let flags = u16::from_be_bytes([msg.payload[2], msg.payload[3]]);

                if reply_type == OFPMP_PORT_DESC {
                    // OF1.3 ofp_port = 64 bytes each
                    // layout: port_no(4) pad(4) hw_addr(6) pad(2) name(16) config(4) state(4)
                    //         curr(4) advertised(4) supported(4) peer(4) curr_speed(4) max_speed(4)
                    let body = &msg.payload[8..];
                    for chunk in body.chunks_exact(64) {
                        let port_no = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        let name = String::from_utf8_lossy(&chunk[16..32])
                            .trim_end_matches('\0')
                            .to_string();
                        if !name.is_empty() && port_no < OFPP_ANY {
                            ports.insert(name, port_no);
                        }
                    }
                    // flags bit 0 = OFPMPF_REPLY_MORE
                    if flags & 1 == 0 {
                        break;
                    }
                } else {
                    break;
                }
            }
            _ => break,
        }
    }

    Ok(ports)
}

// ── Switch connection handler ─────────────────────────────────────────────────

async fn handle_connection(
    mut stream: TcpStream,
    flows: Arc<Vec<(String, String, u16)>>,
) -> Result<()> {
    let mut xid: u32 = 1;

    // 1. Receive Hello from OVS
    let hello = recv_msg(&mut stream).await?;
    if hello.msg_type != OFPT_HELLO {
        anyhow::bail!("expected Hello, got msg_type={}", hello.msg_type);
    }

    // 2. Send Hello
    send_msg(&mut stream, &build_hello(xid)).await?;
    xid += 1;

    // 3. Send FeaturesRequest and wait for FeaturesReply
    send_msg(&mut stream, &build_features_request(xid)).await?;
    xid += 1;

    loop {
        let msg = recv_msg(&mut stream).await?;
        match msg.msg_type {
            OFPT_ECHO_REQUEST => {
                send_msg(&mut stream, &build_echo_reply(msg.xid, &msg.payload)).await?;
            }
            OFPT_FEATURES_REPLY => break,
            _ => {}
        }
    }

    // 4. Discover ports
    let port_map = discover_ports(&mut stream, xid).await?;
    xid += 1;

    log::info!(
        "OF controller: discovered {} ports: {:?}",
        port_map.len(),
        port_map.keys().collect::<Vec<_>>()
    );

    // 5. Clear existing flows
    send_msg(&mut stream, &build_flow_mod_delete_all(xid)).await?;
    xid += 1;

    // 6. Install flows
    let mut installed = 0u32;
    for (in_name, out_name, priority) in flows.iter() {
        match (port_map.get(in_name.as_str()), port_map.get(out_name.as_str())) {
            (Some(&in_port), Some(&out_port)) => {
                send_msg(
                    &mut stream,
                    &build_flow_mod_add(in_port, out_port, *priority, xid),
                )
                .await?;
                xid += 1;
                installed += 1;
                log::info!(
                    "OF controller: installed flow {} (port {}) → {} (port {}), priority={}",
                    in_name, in_port, out_name, out_port, priority
                );
            }
            _ => {
                log::warn!(
                    "OF controller: port not found for flow {} → {} (known: {:?})",
                    in_name,
                    out_name,
                    port_map.keys().collect::<Vec<_>>()
                );
            }
        }
    }

    log::info!("OF controller: {} flows installed; entering keepalive loop", installed);

    // 7. Keepalive loop
    loop {
        let msg = recv_msg(&mut stream).await?;
        if msg.msg_type == OFPT_ECHO_REQUEST {
            send_msg(&mut stream, &build_echo_reply(msg.xid, &msg.payload)).await?;
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// OpenFlow 1.3 controller — accepts connections from OVS and installs flows.
///
/// OVS connects *to* the controller (not the other way around).
/// Configure OVS with: `ovs-vsctl set-controller ovsbr0 tcp:<listen_addr>`
pub struct OpenFlowController {
    listen_addr: SocketAddr,
    flows: Vec<(String, String, u16)>,
}

impl OpenFlowController {
    pub fn new(listen_addr: SocketAddr) -> Self {
        Self {
            listen_addr,
            flows: Vec::new(),
        }
    }

    /// Add a bidirectional forwarding pair (installs two flows).
    pub fn add_port_pair(mut self, port_a: &str, port_b: &str, priority: u16) -> Self {
        self.flows.push((port_a.to_string(), port_b.to_string(), priority));
        self.flows.push((port_b.to_string(), port_a.to_string(), priority));
        self
    }

    /// Add a single directed flow.
    pub fn add_flow(mut self, in_port: &str, out_port: &str, priority: u16) -> Self {
        self.flows.push((in_port.to_string(), out_port.to_string(), priority));
        self
    }

    /// Run the controller — listens for connections and re-programs flows on each reconnect.
    pub async fn run(self) -> Result<()> {
        let listener = TcpListener::bind(self.listen_addr)
            .await
            .with_context(|| format!("binding OpenFlow controller on {}", self.listen_addr))?;

        log::info!("OpenFlow controller listening on {}", self.listen_addr);

        let flows = Arc::new(self.flows);

        loop {
            let (stream, peer) = listener.accept().await?;
            let flows = flows.clone();
            log::info!("OpenFlow controller: OVS connected from {}", peer);

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, flows).await {
                    log::warn!("OF controller: connection from {} ended: {:#}", peer, e);
                }
            });
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_mod_add_length() {
        let msg = build_flow_mod_add(6, 7, 100, 1);
        let declared_len = u16::from_be_bytes([msg[2], msg[3]]) as usize;
        assert_eq!(declared_len, msg.len(), "declared length must match actual");
        assert_eq!(msg[0], OF13_VERSION);
        assert_eq!(msg[1], OFPT_FLOW_MOD);
    }

    #[test]
    fn test_match_in_port_aligned() {
        let m = encode_match_in_port(6);
        assert_eq!(m.len() % 8, 0, "match must be 8-byte aligned");
        // length field should be 12 (4 header + 8 OXM)
        let length = u16::from_be_bytes([m[2], m[3]]);
        assert_eq!(length, 12);
    }

    #[test]
    fn test_wildcard_match_length() {
        let m = encode_wildcard_match();
        assert_eq!(m.len(), 8);
        let length = u16::from_be_bytes([m[2], m[3]]);
        assert_eq!(length, 4);
    }
}
