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
use rovs_openflow::{nxm, ActionList, Flow, FlowCommand, Match, OutputPort};
use std::collections::HashMap;
use std::net::Ipv4Addr;

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
fn parse_match(fields: &HashMap<String, String>, port_map: &HashMap<String, u32>) -> Result<Match> {
    let mut m = Match::new();
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
    Ok(m)
}

/// Translate `actions` into a `rovs_openflow::ActionList`.
///
/// `ip_proto` (established while parsing match_fields) disambiguates
/// protocol-relative `SetField` targets like `tp_dst`.
fn build_actions(
    actions: &[JsonFlowAction],
    port_map: &HashMap<String, u32>,
    ip_proto: Option<u8>,
) -> Result<ActionList> {
    let mut list = ActionList::new();
    for a in actions {
        list = match a {
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
                "dl_dst" | "eth_dst" => list.set_eth_dst(parse_mac(value)?),
                "dl_src" | "eth_src" => list.set_eth_src(parse_mac(value)?),
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
    Ok(list)
}

/// Translate a JSON-encoded `FlowEntry` (the openflow plugin's schema shape)
/// into a `rovs_openflow::Flow` ADD, resolving symbolic port names via
/// `port_map` (as discovered from the switch's own PortDesc reply).
pub fn json_flow_to_add(flow_json: &str, port_map: &HashMap<String, u32>) -> Result<Flow> {
    let entry: JsonFlowEntry = serde_json::from_str(flow_json).context("invalid FlowEntry JSON")?;
    let m = parse_match(&entry.match_fields, port_map)?;
    let ip_proto = m.ip_proto;
    let actions = build_actions(&entry.actions, port_map, ip_proto)?;

    let mut flow = Flow::add()
        .table(entry.table)
        .priority(entry.priority)
        .match_fields(m)
        .actions(actions)
        .idle_timeout(entry.idle_timeout)
        .hard_timeout(entry.hard_timeout);
    if let Some(c) = entry.cookie {
        flow = flow.cookie(c);
    }
    Ok(flow)
}

/// Translate a JSON-encoded `FlowEntry` into a `rovs_openflow::Flow` DELETE
/// (matches on the same fields, no actions needed).
pub fn json_flow_to_delete(flow_json: &str, port_map: &HashMap<String, u32>) -> Result<Flow> {
    let entry: JsonFlowEntry = serde_json::from_str(flow_json).context("invalid FlowEntry JSON")?;
    let m = parse_match(&entry.match_fields, port_map)?;
    let mut flow = Flow::delete()
        .table(entry.table)
        .priority(entry.priority)
        .cookie(entry.cookie.unwrap_or_default())
        .match_fields(m);
    flow.command = FlowCommand::DeleteStrict;
    flow.cookie_mask = u64::MAX;
    Ok(flow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow_json(cookie: serde_json::Value) -> String {
        serde_json::json!({
            "table": 7,
            "priority": 321,
            "match_fields": {
                "in_port": "uplink",
                "ip": "",
                "nw_dst": "10.20.0.0/16"
            },
            "actions": [{"type": "normal"}],
            "cookie": cookie,
            "idle_timeout": 0,
            "hard_timeout": 0
        })
        .to_string()
    }

    fn ports() -> HashMap<String, u32> {
        HashMap::from([("uplink".to_string(), 9)])
    }

    #[test]
    fn add_uses_the_declared_table() {
        let flow = json_flow_to_add(&flow_json(serde_json::json!(42)), &ports()).unwrap();

        assert_eq!(flow.table_id, 7);
    }

    #[test]
    fn delete_is_strict_and_scoped_to_the_exact_flow() {
        let flow = json_flow_to_delete(&flow_json(serde_json::json!(42)), &ports()).unwrap();

        assert_eq!(flow.command, FlowCommand::DeleteStrict);
        assert_eq!(flow.table_id, 7);
        assert_eq!(flow.priority, 321);
        assert_eq!(flow.cookie, 42);
        assert_eq!(flow.cookie_mask, u64::MAX);
    }

    #[test]
    fn delete_scopes_an_unspecified_cookie_to_zero() {
        let flow = json_flow_to_delete(&flow_json(serde_json::Value::Null), &ports()).unwrap();

        assert_eq!(flow.cookie, 0);
        assert_eq!(flow.cookie_mask, u64::MAX);
    }
}
