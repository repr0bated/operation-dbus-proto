//! op-ovsbr0-afxdp — attach/detach the host uplink to ovsbr0 as an AF_XDP port
//! and migrate the management IP to/from the OVS internal bridge interface.
//!
//! Usage: op-ovsbr0-afxdp <up|down|attach-only|detach-only>
//!
//! Environment variables:
//!   BR            OVS bridge name          (default: ovsbr0)
//!   UPLINK        Physical NIC to enslave  (default: eth0)
//!   MGMT_ADDR     Host public IP/prefix    (default: 148.113.204.83/32)
//!   CONTAINER_ADDR Container public IP/pfx (default: 15.235.37.41/32)
//!   GW            IPv4 gateway             (default: 148.113.204.1)
//!   METRIC        Route metric (optional)

use anyhow::{bail, Context, Result};
use op_network::rtnetlink;
use rovs_ovsdb::{Client, Transaction};
use serde_json::{json, Value};
use std::net::Ipv4Addr;
use std::path::Path;
use std::process::Command;
use tracing::info;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

const OVSDB_DB: &str = "Open_vSwitch";

struct Config {
    br: String,
    uplink: String,
    mgmt_addr: String,
    mgmt_prefix: u8,
    gw: Ipv4Addr,
    ovsdb_socket: String,
}

impl Config {
    fn from_env() -> Result<Self> {
        let br = std::env::var("BR").unwrap_or_else(|_| "ovsbr0".into());
        let uplink = std::env::var("UPLINK").unwrap_or_else(|_| "eth0".into());
        let mgmt_addr_str =
            std::env::var("MGMT_ADDR").unwrap_or_else(|_| "148.113.204.83/32".into());
        let gw_str = std::env::var("GW").unwrap_or_else(|_| "148.113.204.1".into());
        let ovsdb_socket = std::env::var("OVSDB_SOCKET").unwrap_or_else(|_| find_socket_path());

        let (mgmt_ip, mgmt_prefix) = parse_cidr(&mgmt_addr_str)?;
        let gw: Ipv4Addr = gw_str.parse().context("invalid GW")?;

        Ok(Config {
            br,
            uplink,
            mgmt_addr: mgmt_ip,
            mgmt_prefix,
            gw,
            ovsdb_socket,
        })
    }
}

fn find_socket_path() -> String {
    let candidates = [
        "/usr/local/var/run/openvswitch/db.sock",
        "/run/openvswitch/db.sock",
        "/var/run/openvswitch/db.sock",
    ];
    candidates
        .iter()
        .find(|p| Path::new(p).exists())
        .unwrap_or(&"/usr/local/var/run/openvswitch/db.sock")
        .to_string()
}

fn parse_cidr(s: &str) -> Result<(String, u8)> {
    let mut parts = s.splitn(2, '/');
    let ip = parts.next().context("empty CIDR")?.to_string();
    let prefix: u8 = parts
        .next()
        .unwrap_or("32")
        .parse()
        .context("invalid prefix")?;
    Ok((ip, prefix))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("op_network=info".parse()?))
        .init();

    let cmd = std::env::args().nth(1).unwrap_or_else(|| "up".into());
    let cfg = Config::from_env()?;

    match cmd.as_str() {
        "up" => up(&cfg).await,
        "down" => down(&cfg).await,
        "attach-only" => attach_only(&cfg).await,
        "detach-only" => detach_only(&cfg).await,
        other => bail!("unknown command: {}", other),
    }
}

async fn up(cfg: &Config) -> Result<()> {
    let mut ovsdb = connect_ovsdb(cfg).await?;

    let gw_str = cfg.gw.to_string();

    // 1. Set management address on bridge internal port BEFORE touching the uplink.
    //    This ensures the kernel has a valid route via ovsbr0 the moment eth0 joins
    //    OVS — no connectivity window even in AF_XDP fail_mode=standalone.
    info!(br = %cfg.br, addr = %cfg.mgmt_addr, "set management address on bridge");
    let _ = rtnetlink::del_ipv4_address(&cfg.br, &cfg.mgmt_addr, cfg.mgmt_prefix).await;
    rtnetlink::add_ipv4_address(&cfg.br, &cfg.mgmt_addr, cfg.mgmt_prefix)
        .await
        .with_context(|| format!("Failed to set {} on {}", cfg.mgmt_addr, cfg.br))?;

    // 2. Gateway host route via bridge (scope link, no `onlink` needed for a
    //    host/32 route — using onlink triggers "PERVASIVE and ONLINK can not be set").
    info!(br = %cfg.br, gw = %gw_str, "gateway route via bridge");
    run_ignore("ip", ["route", "replace", &gw_str, "dev", &cfg.br]);

    // 3. Default route via gateway onlink (OVS internal port requires onlink because
    //    the kernel cannot verify gateway reachability before OVS starts forwarding).
    info!(br = %cfg.br, gw = %gw_str, "default route via bridge onlink");
    rtnetlink::add_default_route_onlink(&cfg.br, &gw_str)
        .await
        .with_context(|| format!("Failed to add default route via {} on {}", gw_str, cfg.br))?;

    // 4. Add UPLINK to bridge as AF_XDP port.  IP/routes are already on ovsbr0 so
    //    OVS standalone MAC-learning takes over with no connectivity gap.
    info!(uplink = %cfg.uplink, bridge = %cfg.br, "add AF_XDP port");
    ensure_afxdp_port(&mut ovsdb, &cfg.br, &cfg.uplink)
        .await
        .with_context(|| format!("Transaction to add port {} to {}", cfg.uplink, cfg.br))?;
    rechain_xdp_steer(&cfg.uplink)?;

    // 5. Flush addresses from UPLINK — kernel stack no longer owns it.
    info!(uplink = %cfg.uplink, "flush addresses from uplink");
    let _ = rtnetlink::flush_addresses(&cfg.uplink).await;

    info!(uplink = %cfg.uplink, bridge = %cfg.br, "AF_XDP up complete");
    Ok(())
}

async fn down(cfg: &Config) -> Result<()> {
    detach_only(cfg).await?;

    // Remove management address from bridge
    let _ = rtnetlink::del_ipv4_address(&cfg.br, &cfg.mgmt_addr, cfg.mgmt_prefix).await;

    // Remove gateway and default routes via bridge
    run_ignore("ip", ["route", "del", "default", "dev", &cfg.br]);
    run_ignore("ip", ["route", "del", &cfg.gw.to_string(), "dev", &cfg.br]);

    // Restore management address on UPLINK so DHCP/kernel can take over
    let _ = rtnetlink::add_ipv4_address(&cfg.uplink, &cfg.mgmt_addr, cfg.mgmt_prefix).await;
    run_ignore(
        "ip",
        [
            "route",
            "replace",
            "default",
            "via",
            &cfg.gw.to_string(),
            "dev",
            &cfg.uplink,
        ],
    );

    info!(uplink = %cfg.uplink, "AF_XDP down complete");
    Ok(())
}

async fn attach_only(cfg: &Config) -> Result<()> {
    let mut ovsdb = connect_ovsdb(cfg).await?;

    info!(uplink = %cfg.uplink, bridge = %cfg.br, "add AF_XDP port");
    ensure_afxdp_port(&mut ovsdb, &cfg.br, &cfg.uplink)
        .await
        .with_context(|| format!("Transaction to add port {} to {}", cfg.uplink, cfg.br))?;
    Ok(())
}

async fn detach_only(cfg: &Config) -> Result<()> {
    let mut ovsdb = connect_ovsdb(cfg).await?;

    info!(uplink = %cfg.uplink, bridge = %cfg.br, "remove AF_XDP port");
    delete_port(&mut ovsdb, &cfg.br, &cfg.uplink).await
}

fn run_ignore(cmd: &str, args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>) {
    let _ = Command::new(cmd).args(args).status();
}

fn rechain_xdp_steer(ifname: &str) -> Result<()> {
    let helper = std::env::var("OP_XDP_WG").unwrap_or_else(|_| {
        if Path::new("/usr/local/sbin/op-xdp-wg").exists() {
            "/usr/local/sbin/op-xdp-wg".into()
        } else {
            "op-xdp-wg".into()
        }
    });

    let status = Command::new(&helper)
        .arg("chain")
        .status()
        .with_context(|| format!("failed to execute {}", helper))?;
    if !status.success() {
        bail!("{} chain failed with {}", helper, status);
    }
    ensure_xdp_dispatcher_ready(ifname)
}

async fn connect_ovsdb(cfg: &Config) -> Result<Client> {
    Client::connect(&format!("unix:{}", cfg.ovsdb_socket))
        .await
        .with_context(|| format!("connect to OVSDB socket {}", cfg.ovsdb_socket))
}

async fn commit(client: &mut Client, txn: &mut Transaction, label: &str) -> Result<()> {
    let ok = client
        .commit(txn)
        .await
        .with_context(|| label.to_string())?;
    if !ok {
        bail!("transaction '{}' failed in OVSDB", label);
    }
    Ok(())
}

fn extract_uuids(val: &Value) -> Vec<Uuid> {
    let mut out = Vec::new();
    if let Some(arr) = val.as_array() {
        match arr.first().and_then(|v| v.as_str()) {
            Some("uuid") => {
                if let Some(s) = arr.get(1).and_then(|v| v.as_str()) {
                    if let Ok(u) = s.parse() {
                        out.push(u);
                    }
                }
            }
            Some("set") => {
                if let Some(items) = arr.get(1).and_then(|v| v.as_array()) {
                    for item in items {
                        out.extend(extract_uuids(item));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn bridge_port_uuids(client: &Client, bridge: &str) -> Result<(Uuid, Vec<Uuid>)> {
    let row = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(bridge))
        .ok_or_else(|| anyhow::anyhow!("Bridge '{}' not found", bridge))?;

    let ports = row.get("ports").map(extract_uuids).unwrap_or_default();
    Ok((row.uuid, ports))
}

fn port_ifaces(client: &Client, port_uuid: Uuid) -> Vec<Uuid> {
    client
        .idl()
        .row("Port", &port_uuid)
        .and_then(|p| p.get("interfaces"))
        .map(extract_uuids)
        .unwrap_or_default()
}

fn bridge_port_by_name(client: &Client, bridge: &str, port_name: &str) -> Result<Option<Uuid>> {
    let (_, port_uuids) = bridge_port_uuids(client, bridge)?;
    Ok(port_uuids.into_iter().find(|uuid| {
        client
            .idl()
            .row("Port", uuid)
            .and_then(|p| p.get_string("name"))
            == Some(port_name)
    }))
}

async fn ensure_afxdp_port(client: &mut Client, bridge: &str, port_name: &str) -> Result<()> {
    let (bridge_uuid, _) = bridge_port_uuids(client, bridge)?;

    if let Some(port_uuid) = bridge_port_by_name(client, bridge, port_name)? {
        let iface_uuids = port_ifaces(client, port_uuid);
        if iface_uuids.is_empty() {
            bail!("Port '{}' exists without Interface rows", port_name);
        }

        let mut txn = Transaction::new(OVSDB_DB);
        for iface_uuid in iface_uuids {
            txn.update(
                "Interface",
                iface_uuid,
                json!({
                    "type": "afxdp",
                    "options": ["map", [["n_rxq", "1"]]]
                }),
            );
        }
        commit(client, &mut txn, "update AF_XDP port n_rxq").await?;
        info!(uplink = %port_name, bridge = %bridge, "AF_XDP port already present; set n_rxq=1");
        return Ok(());
    }

    let mut txn = Transaction::new(OVSDB_DB);
    let iface_ref = txn.insert(
        "Interface",
        json!({
            "name": port_name,
            "type": "afxdp",
            "options": ["map", [["n_rxq", "1"]]]
        }),
    );
    let port_ref = txn.insert(
        "Port",
        json!({
            "name": port_name,
            "interfaces": iface_ref.to_json()
        }),
    );
    txn.mutate(
        "Bridge",
        bridge_uuid,
        vec![json!(["ports", "insert", port_ref.to_json()])],
    );
    commit(client, &mut txn, "add AF_XDP port n_rxq").await?;
    info!(uplink = %port_name, bridge = %bridge, "added AF_XDP port with n_rxq=1");
    Ok(())
}

async fn delete_port(client: &mut Client, bridge: &str, port_name: &str) -> Result<()> {
    let (bridge_uuid, _) = bridge_port_uuids(client, bridge)?;
    let Some(port_uuid) = bridge_port_by_name(client, bridge, port_name)? else {
        info!(uplink = %port_name, bridge = %bridge, "port already absent");
        return Ok(());
    };
    let iface_uuids = port_ifaces(client, port_uuid);

    let mut txn = Transaction::new(OVSDB_DB);
    txn.mutate(
        "Bridge",
        bridge_uuid,
        vec![json!([
            "ports",
            "delete",
            ["set", [["uuid", port_uuid.to_string()]]]
        ])],
    );
    for iface_uuid in iface_uuids {
        txn.delete("Interface", iface_uuid);
    }
    txn.delete("Port", port_uuid);
    commit(client, &mut txn, "delete AF_XDP port").await
}

fn ensure_xdp_dispatcher_ready(ifname: &str) -> Result<()> {
    let output = Command::new("xdp-loader")
        .args(["status", ifname])
        .output()
        .with_context(|| format!("failed to inspect XDP status for {}", ifname))?;

    if !output.status.success() {
        bail!(
            "xdp-loader status {} failed; load xdp_steer through the dispatcher first",
            ifname
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("xdp_dispatcher") {
        bail!(
            "{} is not using the libxdp dispatcher; refusing AF_XDP attach",
            ifname
        );
    }
    if !stdout.contains("xdp_steer") {
        bail!(
            "{} dispatcher does not contain xdp_steer; run op-xdp-wg hostside first",
            ifname
        );
    }

    Ok(())
}
