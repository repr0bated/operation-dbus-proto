//! Translates the `openflow` plugin's JSON `FlowEntry`/`FlowAction` schema
//! (see `op-plugins/src/state_plugins/openflow.rs`) into real `rovs_openflow`
//! wire types.
//!
//! This replaces the old `op-openvswitch-daemon::parse_json_flow`, which only
//! understood 3 of the ~15 match keys and 3 of the 8 action variants the
//! schema actually declares (everything else was silently dropped with a
//! `tracing::warn!`, producing wildcard-match flows instead of the intended
//! narrow ones). Unsupported fields here are a hard error instead — callers
//! must not install a flow that is looser than what was requested.

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use rovs_openflow::{nxm, ActionList, Flow, Match, Message, MessageType, OutputPort, Version};
use std::collections::HashMap;
use std::net::Ipv4Addr;

/// Port hardware addresses discovered from the controller's PortDesc reply.
/// Static flow JSON can refer to one as `port_mac:<port-name>`, avoiding a
/// boot-specific MAC address in persistent policy.
pub type PortMacMap = HashMap<String, [u8; 6]>;

/// A translated flow plus the two OpenFlow features that rovs-openflow 0.2
/// does not yet model: OXM_OF_PACKET_TYPE and NXAST_ENCAP.
pub struct TranslatedFlow {
    flow: Flow,
    packet_type: Option<u32>,
    encap_ethernet: bool,
}

impl TranslatedFlow {
    /// Encode a complete FlowMod. `packet_type` requires OpenFlow 1.5, while
    /// OVS exposes `encap(ethernet)` as the Nicira NXAST_ENCAP action.
    pub fn to_message(&self, version: Version, xid: u32) -> Result<Message> {
        if self.packet_type.is_none() && !self.encap_ethernet {
            return Ok(self.flow.to_message(version, xid));
        }
        if self.packet_type.is_some() && version < Version::Of15 {
            bail!("packet_type match requires OpenFlow 1.5");
        }

        let base = self.flow.encode();
        if base.len() < 48 {
            bail!("encoded FlowMod is shorter than fixed fields plus match header");
        }

        // FlowMod fixed header is 40 bytes. The match length excludes its
        // 8-byte alignment padding; instructions begin after the padded match.
        let old_match_len = u16::from_be_bytes([base[42], base[43]]) as usize;
        if old_match_len < 4 || 40 + old_match_len > base.len() {
            bail!("encoded FlowMod contains an invalid match length");
        }
        let old_match_padded = old_match_len.next_multiple_of(8);
        if 40 + old_match_padded > base.len() {
            bail!("encoded FlowMod match padding exceeds message body");
        }

        let mut oxm = base[44..40 + old_match_len].to_vec();
        if let Some(packet_type) = self.packet_type {
            // OXM_OF_PACKET_TYPE(44), OpenFlow basic class, unmasked, 4 bytes.
            let header = (0x8000u32 << 16) | (44u32 << 9) | 4;
            oxm.extend_from_slice(&header.to_be_bytes());
            oxm.extend_from_slice(&packet_type.to_be_bytes());
        }

        let new_match_len = 4 + oxm.len();
        let mut encoded_match = Vec::with_capacity(new_match_len.next_multiple_of(8));
        encoded_match.extend_from_slice(&1u16.to_be_bytes()); // OFPMT_OXM
        encoded_match.extend_from_slice(&(new_match_len as u16).to_be_bytes());
        encoded_match.extend_from_slice(&oxm);
        encoded_match.resize(new_match_len.next_multiple_of(8), 0);

        let mut instructions = base[40 + old_match_padded..].to_vec();
        if self.encap_ethernet {
            if instructions.len() < 8 || u16::from_be_bytes([instructions[0], instructions[1]]) != 4
            {
                bail!("encap action requires an ApplyActions instruction");
            }
            let old_instruction_len =
                u16::from_be_bytes([instructions[2], instructions[3]]) as usize;
            if old_instruction_len < 8 || old_instruction_len > instructions.len() {
                bail!("invalid ApplyActions instruction length");
            }

            // NXAST_ENCAP: experimenter action, Nicira vendor 0x2320,
            // subtype 46, hdr_size=0, PT_ETH=(0,0).
            let encap = [
                0xff, 0xff, 0x00, 0x10, 0x00, 0x00, 0x23, 0x20, 0x00, 0x2e, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ];
            instructions.splice(8..8, encap);
            let new_instruction_len = old_instruction_len + encap.len();
            instructions[2..4].copy_from_slice(&(new_instruction_len as u16).to_be_bytes());
        }

        let mut body = Vec::with_capacity(40 + encoded_match.len() + instructions.len());
        body.extend_from_slice(&base[..40]);
        body.extend_from_slice(&encoded_match);
        body.extend_from_slice(&instructions);
        Ok(Message::new(
            version,
            MessageType::FlowMod,
            xid,
            Bytes::from(body),
        ))
    }
}

#[derive(serde::Deserialize)]
pub struct JsonFlowEntry {
    #[serde(default)]
    pub table: u8,
    pub priority: u16,
    #[serde(default)]
    pub match_fields: HashMap<String, String>,
    #[serde(default)]
    pub actions: Vec<JsonFlowAction>,
    pub cookie: Option<u64>,
    #[serde(default)]
    pub idle_timeout: u16,
    #[serde(default)]
    pub hard_timeout: u16,
}

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JsonFlowAction {
    Encap { header: String },
    Output { port: String },
    LoadRegister { register: u8, value: u64 },
    Resubmit { table: u8 },
    SetField { field: String, value: String },
    Drop,
    Normal,
    Controller { max_len: Option<u16> },
    ArpResponder { mac: String, ip: String },
}

/// Resolve a port token: a bare port number, `LOCAL`, or a symbolic name
/// looked up in `port_map` (as discovered via the controller's own PortDesc
/// multipart request — no separate OVSDB round trip needed).
fn resolve_port(token: &str, port_map: &HashMap<String, u32>) -> Result<u32> {
    if let Ok(n) = token.parse::<u32>() {
        return Ok(n);
    }
    if token.eq_ignore_ascii_case("LOCAL") {
        return Ok(0xffff_fffe); // OFPP_LOCAL
    }
    port_map
        .get(token)
        .copied()
        .with_context(|| format!("unknown port name '{token}' (not in discovered port map)"))
}

fn parse_mac(s: &str) -> Result<[u8; 6]> {
    let mut out = [0u8; 6];
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 6 {
        bail!("invalid MAC address '{}'", s);
    }
    for (i, p) in parts.iter().enumerate() {
        out[i] = u8::from_str_radix(p, 16).with_context(|| format!("invalid MAC octet '{p}'"))?;
    }
    Ok(out)
}

fn resolve_mac(s: &str, port_macs: &PortMacMap) -> Result<[u8; 6]> {
    if let Some(port) = s.strip_prefix("port_mac:") {
        return port_macs
            .get(port)
            .copied()
            .with_context(|| format!("unknown port name '{port}' for symbolic MAC"));
    }
    parse_mac(s)
}

fn parse_u16_auto(s: &str) -> Result<u16> {
    if let Some(hex) = s.strip_prefix("0x") {
        u16::from_str_radix(hex, 16).with_context(|| format!("invalid u16 value '{s}'"))
    } else {
        s.parse::<u16>()
            .with_context(|| format!("invalid u16 value '{s}'"))
    }
}

/// Parse OVS packet_type syntax `(namespace,type)`, for example
/// `(1,0x800)` for a bare IPv4 packet from an L3 WireGuard interface.
fn parse_packet_type(s: &str) -> Result<u32> {
    let inner = s
        .trim()
        .strip_prefix('(')
        .and_then(|v| v.strip_suffix(')'))
        .with_context(|| format!("packet_type must use '(namespace,type)' syntax, got '{s}'"))?;
    let (namespace, packet_type) = inner
        .split_once(',')
        .with_context(|| format!("packet_type must contain namespace and type, got '{s}'"))?;
    Ok(((parse_u16_auto(namespace.trim())? as u32) << 16)
        | parse_u16_auto(packet_type.trim())? as u32)
}

fn mac_to_u64(mac: [u8; 6]) -> u64 {
    mac.iter().fold(0u64, |acc, &b| (acc << 8) | b as u64)
}

fn ip_to_u32(ip: Ipv4Addr) -> u32 {
    u32::from(ip)
}

fn parse_ipv4(s: &str) -> Result<Ipv4Addr> {
    s.parse::<Ipv4Addr>()
        .with_context(|| format!("invalid IPv4 address '{s}'"))
}

/// Parse `a.b.c.d` or `a.b.c.d/n` into (address, prefix_len). Bare addresses
/// are treated as /32 host matches.
fn parse_cidr(s: &str) -> Result<(Ipv4Addr, u8)> {
    match s.split_once('/') {
        Some((addr, prefix)) => {
            let addr = parse_ipv4(addr)?;
            let prefix: u8 = prefix
                .parse()
                .with_context(|| format!("invalid CIDR prefix in '{s}'"))?;
            Ok((addr, prefix))
        }
        None => Ok((parse_ipv4(s)?, 32)),
    }
}

/// Parse OVS `tcp_flags` syntax: either a raw `0xNNN` value or `+flag` tokens
/// (e.g. `+fin+psh+urg`). Only the "must be set" (`+`) form is used anywhere
/// in this codebase's flow generators.
fn parse_tcp_flags(s: &str) -> Result<u16> {
    if let Some(hex) = s.strip_prefix("0x") {
        return u16::from_str_radix(hex, 16).with_context(|| format!("invalid tcp_flags '{s}'"));
    }
    let mut flags = 0u16;
    for tok in s.split('+').filter(|t| !t.is_empty()) {
        flags |= match tok {
            "fin" => 0x001,
            "syn" => 0x002,
            "rst" => 0x004,
            "psh" => 0x008,
            "ack" => 0x010,
            "urg" => 0x020,
            "ece" => 0x040,
            "cwr" => 0x080,
            "ns" => 0x100,
            other => bail!("unknown tcp_flags token '+{}' in '{}'", other, s),
        };
    }
    Ok(flags)
}

/// Parse OVS `ct_state` syntax (e.g. `+est+trk`, `+inv+trk`) into (value, mask).
/// Only "must be set" flags are supported (matches this codebase's usage).
fn parse_ct_state(s: &str) -> Result<(u32, u32)> {
    let mut bits = 0u32;
    for tok in s.split('+').filter(|t| !t.is_empty()) {
        bits |= match tok {
            "new" => 0x01,
            "est" => 0x02,
            "rel" => 0x04,
            "rpl" => 0x08,
            "inv" => 0x10,
            "trk" => 0x20,
            "snat" => 0x40,
            "dnat" => 0x80,
            other => bail!("unknown ct_state token '+{}' in '{}'", other, s),
        };
    }
    Ok((bits, bits))
}

/// Translate `match_fields` into a `rovs_openflow::Match`.
///
/// Two passes: first detect protocol markers (`tcp`/`udp`/`icmp`/`icmp6`/
/// `arp`/`ip`) to establish `eth_type`/`ip_proto` context, since `HashMap`
/// iteration order can't be relied on to see e.g. `udp` before `tp_dst`.
fn parse_match(
    fields: &HashMap<String, String>,
    port_map: &HashMap<String, u32>,
) -> Result<(Match, Option<u32>)> {
    let mut m = Match::new();
    let mut packet_type = None;
    let mut ip_proto: Option<u8> = None;
    let mut eth_type: Option<u16> = None;

    if fields.contains_key("arp") {
        eth_type = Some(0x0806);
    }
    if fields.contains_key("icmp6") {
        eth_type = Some(0x86dd);
        ip_proto = Some(58);
    }
    if fields.contains_key("ip") {
        eth_type = eth_type.or(Some(0x0800));
    }
    if fields.contains_key("tcp") {
        eth_type = eth_type.or(Some(0x0800));
        ip_proto = ip_proto.or(Some(6));
    }
    if fields.contains_key("udp") {
        eth_type = eth_type.or(Some(0x0800));
        ip_proto = ip_proto.or(Some(17));
    }
    if fields.contains_key("icmp") {
        eth_type = eth_type.or(Some(0x0800));
        ip_proto = ip_proto.or(Some(1));
    }
    if let Some(et) = eth_type {
        m = m.eth_type(et);
    }
    if let Some(p) = ip_proto {
        m = m.ip_proto(p);
    }

    for (k, v) in fields {
        m = match k.as_str() {
            "arp" | "ip" | "tcp" | "udp" | "icmp" | "icmp6" => m, // handled above
            "in_port" => m.in_port(resolve_port(v, port_map)?),
            "packet_type" => {
                packet_type = Some(parse_packet_type(v)?);
                m
            }
            "dl_type" => {
                let et = if let Some(hex) = v.strip_prefix("0x") {
                    u16::from_str_radix(hex, 16)
                        .with_context(|| format!("invalid dl_type '{v}'"))?
                } else {
                    v.parse()
                        .with_context(|| format!("invalid dl_type '{v}'"))?
                };
                m.eth_type(et)
            }
            "dl_vlan" => m.vlan_vid(
                v.parse()
                    .with_context(|| format!("invalid dl_vlan '{v}'"))?,
            ),
            "dl_src" => m.eth_src(parse_mac(v)?),
            "tcp_flags" => {
                let mut m2 = m;
                m2.tcp_flags = Some(parse_tcp_flags(v)?);
                m2
            }
            "tp_src" => {
                let port: u16 = v.parse().with_context(|| format!("invalid tp_src '{v}'"))?;
                match ip_proto {
                    Some(6) => m.tcp_src(port),
                    Some(17) => m.udp_src(port),
                    _ => bail!("tp_src requires a 'tcp' or 'udp' marker in match_fields"),
                }
            }
            "tp_dst" => {
                let port: u16 = v.parse().with_context(|| format!("invalid tp_dst '{v}'"))?;
                match ip_proto {
                    Some(6) => m.tcp_dst(port),
                    Some(17) => m.udp_dst(port),
                    _ => bail!("tp_dst requires a 'tcp' or 'udp' marker in match_fields"),
                }
            }
            "nw_src" => {
                let (addr, prefix) = parse_cidr(v)?;
                m.ipv4_src(addr, prefix)
            }
            "nw_dst" => {
                let (addr, prefix) = parse_cidr(v)?;
                m.ipv4_dst(addr, prefix)
            }
            "ct_state" => {
                let (state, mask) = parse_ct_state(v)?;
                m.ct_state_masked(state, mask)
            }
            other => bail!(
                "unsupported OpenFlow match field '{}'={:?} — no translation available \
                 (rovs-openflow's Match type has no equivalent field)",
                other,
                v
            ),
        };
    }
    Ok((m, packet_type))
}

/// Translate `actions` into a `rovs_openflow::ActionList`.
///
/// `ip_proto` (established while parsing match_fields) disambiguates
/// protocol-relative `SetField` targets like `tp_dst`.
fn build_actions(
    actions: &[JsonFlowAction],
    port_map: &HashMap<String, u32>,
    port_macs: &PortMacMap,
    ip_proto: Option<u8>,
) -> Result<(ActionList, bool)> {
    let mut list = ActionList::new();
    let mut encap_ethernet = false;
    for (index, a) in actions.iter().enumerate() {
        list = match a {
            JsonFlowAction::Encap { header } => {
                if index != 0 || encap_ethernet {
                    bail!("encap must be the first and only encap action");
                }
                if !header.eq_ignore_ascii_case("ethernet") {
                    bail!("unsupported encap header '{header}' (only 'ethernet' is supported)");
                }
                encap_ethernet = true;
                list
            }
            JsonFlowAction::Output { port } => {
                if port.eq_ignore_ascii_case("LOCAL") {
                    list.output(OutputPort::Local)
                } else if port.eq_ignore_ascii_case("NORMAL") {
                    list.output(OutputPort::Normal)
                } else if port.eq_ignore_ascii_case("FLOOD") {
                    list.output(OutputPort::Flood)
                } else if port.eq_ignore_ascii_case("ALL") {
                    list.output(OutputPort::All)
                } else if port.eq_ignore_ascii_case("IN_PORT") {
                    list.output(OutputPort::InPort)
                } else if port.eq_ignore_ascii_case("CONTROLLER") {
                    list.output(OutputPort::Controller)
                } else {
                    list.output(OutputPort::Port(resolve_port(port, port_map)?))
                }
            }
            JsonFlowAction::Drop => list.drop(),
            JsonFlowAction::Normal => list.normal(),
            JsonFlowAction::Controller { max_len } => list.controller(max_len.unwrap_or(128)),
            JsonFlowAction::Resubmit { table } => list.resubmit_table(*table),
            JsonFlowAction::LoadRegister { register, value } => {
                let field = match register {
                    0 => nxm::REG0,
                    1 => nxm::REG1,
                    2 => nxm::REG2,
                    other => bail!(
                        "unsupported register {} — rovs-openflow only exposes NXM_NX_REG0..REG2",
                        other
                    ),
                };
                list.load_field(field, 0, 32, *value)
            }
            JsonFlowAction::SetField { field, value } => match field.as_str() {
                "dl_dst" | "eth_dst" => list.set_eth_dst(resolve_mac(value, port_macs)?),
                "dl_src" | "eth_src" => list.set_eth_src(resolve_mac(value, port_macs)?),
                "dl_vlan" | "vlan_vid" => {
                    let vid: u16 = value
                        .parse()
                        .with_context(|| format!("invalid vlan_vid '{value}'"))?;
                    list.set_vlan_vid(vid)
                }
                "tun_id" | "tunnel_id" => {
                    let id: u64 = value
                        .parse()
                        .with_context(|| format!("invalid tunnel_id '{value}'"))?;
                    list.set_tunnel_id(id)
                }
                "tp_dst" => {
                    let port: u16 = value
                        .parse()
                        .with_context(|| format!("invalid tp_dst '{value}'"))?;
                    match ip_proto {
                        Some(6) => list.load_field(nxm::TCP_DST, 0, 16, port as u64),
                        Some(17) => list.load_field(nxm::UDP_DST, 0, 16, port as u64),
                        _ => bail!(
                            "SetField tp_dst requires a 'tcp' or 'udp' marker in match_fields"
                        ),
                    }
                }
                "tp_src" => {
                    let port: u16 = value
                        .parse()
                        .with_context(|| format!("invalid tp_src '{value}'"))?;
                    match ip_proto {
                        Some(6) => list.load_field(nxm::TCP_SRC, 0, 16, port as u64),
                        Some(17) => list.load_field(nxm::UDP_SRC, 0, 16, port as u64),
                        _ => bail!(
                            "SetField tp_src requires a 'tcp' or 'udp' marker in match_fields"
                        ),
                    }
                }
                other => bail!(
                    "unsupported SetField target '{}'={:?} — no NXM field available in rovs-openflow",
                    other,
                    value
                ),
            },
            JsonFlowAction::ArpResponder { mac, ip } => {
                let mac_bytes = parse_mac(mac)?;
                let ip_addr = parse_ipv4(ip)?;
                list
                    // dst mac := original src mac; src mac := our mac
                    .move_field(nxm::ETH_SRC, nxm::ETH_DST, 48, 0, 0)
                    .set_eth_src(mac_bytes)
                    // ARP reply
                    .set_arp_op(2)
                    // tha := original sha; sha := our mac
                    .move_field(nxm::ARP_SHA, nxm::ARP_THA, 48, 0, 0)
                    .set_arp_sha(mac_to_u64(mac_bytes))
                    // tpa := original spa; spa := our ip
                    .move_field(nxm::ARP_SPA, nxm::ARP_TPA, 32, 0, 0)
                    .set_arp_spa(ip_to_u32(ip_addr))
                    .in_port()
            }
        };
    }
    Ok((list, encap_ethernet))
}

/// Translate a JSON-encoded `FlowEntry` (the openflow plugin's schema shape)
/// into a `rovs_openflow::Flow` ADD, resolving symbolic port names via
/// `port_map` (as discovered from the switch's own PortDesc reply).
pub fn json_flow_to_add(
    flow_json: &str,
    port_map: &HashMap<String, u32>,
    port_macs: &PortMacMap,
) -> Result<TranslatedFlow> {
    let entry: JsonFlowEntry = serde_json::from_str(flow_json).context("invalid FlowEntry JSON")?;
    let (m, packet_type) = parse_match(&entry.match_fields, port_map)?;
    let ip_proto = m.ip_proto;
    let (actions, encap_ethernet) = build_actions(&entry.actions, port_map, port_macs, ip_proto)?;

    let mut flow = Flow::add()
        .priority(entry.priority)
        .match_fields(m)
        .actions(actions)
        .idle_timeout(entry.idle_timeout)
        .hard_timeout(entry.hard_timeout);
    if let Some(c) = entry.cookie {
        flow = flow.cookie(c);
    }
    Ok(TranslatedFlow {
        flow,
        packet_type,
        encap_ethernet,
    })
}

/// Translate a JSON-encoded `FlowEntry` into a `rovs_openflow::Flow` DELETE
/// (matches on the same fields, no actions needed).
pub fn json_flow_to_delete(
    flow_json: &str,
    port_map: &HashMap<String, u32>,
) -> Result<TranslatedFlow> {
    let entry: JsonFlowEntry = serde_json::from_str(flow_json).context("invalid FlowEntry JSON")?;
    let (m, packet_type) = parse_match(&entry.match_fields, port_map)?;
    Ok(TranslatedFlow {
        flow: Flow::delete().match_fields(m),
        packet_type,
        encap_ethernet: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l3_flow_json() -> String {
        serde_json::json!({
            "table": 0,
            "priority": 200,
            "match_fields": {
                "in_port": "netmaker",
                "packet_type": "(1,0x800)"
            },
            "actions": [
                {"type": "encap", "header": "ethernet"},
                {"type": "set_field", "field": "eth_src", "value": "02:00:64:45:00:01"},
                {"type": "set_field", "field": "eth_dst", "value": "port_mac:ovsbr0"},
                {"type": "output", "port": "LOCAL"}
            ],
            "cookie": 3694151570867355650u64,
            "idle_timeout": 0,
            "hard_timeout": 0
        })
        .to_string()
    }

    #[test]
    fn l3_ingress_encodes_openflow15_packet_type_and_encap() {
        let ports = HashMap::from([("netmaker".to_string(), 328)]);
        let macs = HashMap::from([("ovsbr0".to_string(), [0x9e, 0x17, 0xad, 0x27, 0x94, 0x47])]);
        let translated = json_flow_to_add(&l3_flow_json(), &ports, &macs).unwrap();
        let bytes = translated.to_message(Version::Of15, 7).unwrap().encode();

        assert_eq!(bytes[0], 0x06); // OpenFlow 1.5
        assert_eq!(bytes[1], 14); // FlowMod
        assert_eq!(
            u16::from_be_bytes([bytes[2], bytes[3]]) as usize,
            bytes.len()
        );
        assert!(bytes
            .windows(8)
            .any(|w| w == [0x80, 0x00, 0x58, 0x04, 0x00, 0x01, 0x08, 0x00]));
        assert!(bytes.windows(16).any(|w| w
            == [
                0xff, 0xff, 0x00, 0x10, 0x00, 0x00, 0x23, 0x20, 0x00, 0x2e, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ]));
        assert!(bytes
            .windows(6)
            .any(|w| w == [0x9e, 0x17, 0xad, 0x27, 0x94, 0x47]));
    }

    #[test]
    fn packet_type_is_rejected_before_openflow15() {
        let ports = HashMap::from([("netmaker".to_string(), 328)]);
        let macs = HashMap::from([("ovsbr0".to_string(), [0; 6])]);
        let translated = json_flow_to_add(&l3_flow_json(), &ports, &macs).unwrap();
        assert!(translated.to_message(Version::Of13, 7).is_err());
    }
}
