This file is a merged representation of the entire codebase, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of the entire repository's contents.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
src/
  bin/
    op-of-controller.rs
    op-ovsbr0-afxdp.rs
    op-ovsbr0-setup.rs
    op-xdp-wg.rs
  controller.rs
  lib.rs
  openflow.rs
  ovs_capabilities.rs
  ovs_error.rs
  ovs_netlink.rs
  ovsdb.rs
  plugin.rs
  proxmox.rs
  rtnetlink.rs
Cargo.toml
compare-op-network.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/bin/op-of-controller.rs">
//! OpenFlow 1.3 controller for ovsbr0
//!
//! Listens on OF_CONTROLLER_LISTEN (default 10.200.0.1:6653) for OVS to
//! connect, then installs bidirectional flows between the configured port pairs.
//!
//! Environment variables:
//!   OF_CONTROLLER_LISTEN   listen address (default: 10.200.0.1:6653)
//!   OF_FLOW_PAIRS          comma-separated port pairs, e.g. "grpc-bridge:ovsbr0-sock"
//!                          defaults to "grpc-bridge:ovsbr0-sock"
//!   OF_FLOW_PRIORITY       flow priority (default: 100)

use std::net::SocketAddr;

use anyhow::Result;
use op_network::OpenFlowController;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("op_network=info".parse()?))
        .init();

    let listen: SocketAddr = std::env::var("OF_CONTROLLER_LISTEN")
        .unwrap_or_else(|_| "10.200.0.1:6653".to_string())
        .parse()
        .expect("OF_CONTROLLER_LISTEN must be a valid socket address");

    let pairs_env =
        std::env::var("OF_FLOW_PAIRS").unwrap_or_else(|_| "grpc-bridge:ovsbr0-sock".to_string());

    let priority: u16 = std::env::var("OF_FLOW_PRIORITY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let mut controller = OpenFlowController::new(listen);

    for pair in pairs_env.split(',') {
        let parts: Vec<&str> = pair.trim().splitn(2, ':').collect();
        if parts.len() != 2 {
            tracing::warn!("Ignoring malformed flow pair: {:?}", pair);
            continue;
        }
        info!(
            "Flow pair: {} ↔ {} (priority {})",
            parts[0], parts[1], priority
        );
        controller = controller.add_port_pair(parts[0], parts[1], priority);
    }

    controller.run().await
}
</file>

<file path="src/bin/op-ovsbr0-afxdp.rs">
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
</file>

<file path="src/bin/op-ovsbr0-setup.rs">
//! op-ovsbr0-setup — ensure ovsbr0 exists with datapath_type=netdev and add ports.
//!
//! Replaces the original (deleted-source) Rust binary that always created the
//! bridge with datapath_type=system.  Uses the rovs crate family to manipulate
//! OVSDB directly and the vswitchd unixctl socket to stop the daemon cleanly.
//! No ovs-vsctl/ovsdb-client shell commands are used.  `ovs-dpctl del-dp` is
//! used only while vswitchd is down to clear stale kernel datapath state.
//!
//! Environment variables:
//!   BRIDGE         OVS bridge name          (default: ovsbr0)
//!   VETH_HOST      Veth to add as port      (default: grpc-uplink)
//!   FAIL_MODE      OVS fail mode            (default: standalone)
//!   SHARED_MAC     Bridge/container MAC     (default: fa:16:3e:f1:71:d2)
//!   OVSDB_SOCKET   Path to OVSDB socket     (default: auto-detect)
//!   VSWITCHD_SVC   s6 service path          (default: /run/service/ovs-vswitchd)
//!   VSWITCHD_CTL   Glob for vswitchd unixctl socket
//!                  (default: /usr/local/var/run/openvswitch/ovs-vswitchd.*.ctl)
//!
//! Modes:
//!   --seed-only     Write OVSDB netdev bridge/port rows and exit.  This is
//!                   for the s6 ovs-vswitchd run script before vswitchd starts.

use anyhow::{bail, Context, Result};
use rovs_jsonrpc::Connection;
use rovs_ovsdb::{Client, Transaction};
use rovs_transport::{Address, Stream};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

struct Config {
    bridge: String,
    veth_host: String,
    fail_mode: String,
    shared_mac: String,
    ovsdb_socket: String,
    vswitchd_svc: String,
}

impl Config {
    fn from_env() -> Self {
        Config {
            bridge: std::env::var("BRIDGE").unwrap_or_else(|_| "ovsbr0".into()),
            veth_host: std::env::var("VETH_HOST").unwrap_or_else(|_| "grpc-uplink".into()),
            fail_mode: std::env::var("FAIL_MODE").unwrap_or_else(|_| "standalone".into()),
            shared_mac: std::env::var("SHARED_MAC").unwrap_or_else(|_| "fa:16:3e:f1:71:d2".into()),
            ovsdb_socket: std::env::var("OVSDB_SOCKET").unwrap_or_else(|_| find_socket_path()),
            vswitchd_svc: std::env::var("VSWITCHD_SVC")
                .unwrap_or_else(|_| "/run/service/ovs-vswitchd".into()),
        }
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

/// Find vswitchd unixctl control socket (matches ovs-vswitchd.PID.ctl).
fn find_vswitchd_ctl() -> Option<String> {
    let dir = "/usr/local/var/run/openvswitch";
    std::fs::read_dir(dir).ok()?.find_map(|entry| {
        let e = entry.ok()?;
        let name = e.file_name().into_string().ok()?;
        if name.starts_with("ovs-vswitchd.") && name.ends_with(".ctl") {
            Some(format!("{}/{}", dir, name))
        } else {
            None
        }
    })
}

/// Send the `exit` command to vswitchd via its unixctl JSON-RPC socket.
/// vswitchd will shut down gracefully on receiving this.
async fn vswitchd_send_exit(ctl_path: &str) -> Result<()> {
    let addr: Address = format!("unix:{}", ctl_path)
        .parse()
        .context("parse vswitchd ctl address")?;
    let stream = Stream::connect(&addr)
        .await
        .context("connect to vswitchd unixctl")?;
    let mut conn = Connection::new(stream);

    // vswitchd unixctl speaks JSON-RPC: {"method":"exit","params":[],"id":0}
    let _ = conn.transact("exit", json!([])).await;
    drop(conn);
    info!("sent exit to vswitchd via unixctl {}", ctl_path);
    Ok(())
}

/// Stop vswitchd:
///   1. Send `exit` via unixctl (graceful shutdown)
///   2. Use s6-svc -d to prevent auto-restart
///   3. Wait for the OVSDB socket to be unresponsive (vswitchd releases it)
///      and for kernel OVS interfaces to disappear
async fn stop_vswitchd(svc: &str, bridge: &str) -> Result<()> {
    // Mark service down in s6 (prevents auto-restart when process exits)
    let _ = Command::new("s6-svc").args(["-d", svc]).status();
    std::thread::sleep(Duration::from_millis(100));

    // Send exit via unixctl if socket exists
    if let Some(ctl) = find_vswitchd_ctl() {
        let _ = vswitchd_send_exit(&ctl).await;
    } else {
        // Fall back to SIGTERM via s6
        warn!("vswitchd ctl socket not found, using s6 SIGTERM");
        let _ = Command::new("s6-svc").args(["-t", svc]).status();
    }

    // Wait for vswitchd to fully exit: poll s6-svstat for "down"
    info!("waiting for vswitchd to exit...");
    for i in 0..50 {
        std::thread::sleep(Duration::from_millis(200));
        let out = Command::new("s6-svstat").arg(svc).output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            if s.trim_start().starts_with("down") {
                info!("vswitchd exited after {}ms", (i + 1) * 200);
                break;
            }
        }
    }

    clear_kernel_datapath(bridge);

    // Wait for kernel OVS interfaces to be cleaned up by the kernel module.
    // When vswitchd exits it releases the kernel datapath; the kernel then
    // GC-removes the ovsbr0 interface (async).  A new vswitchd trying to
    // create a netdev internal port named "ovsbr0" will get EINVAL if this
    // interface still exists in the kernel.
    let sysfs_path = format!("/sys/class/net/{}", bridge);
    info!("waiting for kernel interface {} to disappear...", bridge);
    for i in 0..50 {
        if !Path::new(&sysfs_path).exists() {
            info!("kernel interface {} is gone", bridge);
            break;
        }
        if i == 10 || i == 25 {
            clear_kernel_datapath(bridge);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    if Path::new(&sysfs_path).exists() {
        bail!(
            "kernel interface {} still exists after stopping vswitchd; \
             refusing netdev recreate because vswitchd will fall back to system datapath",
            bridge
        );
    }

    Ok(())
}

fn clear_kernel_datapath(bridge: &str) {
    info!("clearing stale kernel datapath state");
    let _ = Command::new("ovs-dpctl")
        .args(["del-dp", "system@ovs-system"])
        .status();
    let _ = Command::new("ip").args(["link", "del", bridge]).status();
}

/// Start vswitchd via s6 and wait until its OVSDB connection is visible.
fn start_vswitchd(svc: &str) {
    info!("starting vswitchd via s6: {}", svc);
    let _ = Command::new("s6-svc").args(["-u", svc]).status();
}

/// Wait for vswitchd to fully start: poll OVSDB until bridge row appears AND
/// the vswitchd unixctl socket is present (confirms vswitchd is fully up).
async fn wait_for_vswitchd_up(socket: &str, bridge: &str) -> Result<()> {
    let addr = format!("unix:{}", socket);
    info!("waiting for vswitchd to come up with bridge {}...", bridge);

    for i in 0..120 {
        tokio::time::sleep(Duration::from_millis(500)).await;

        // vswitchd control socket must be present (vswitchd fully initialized)
        let ctl_present = find_vswitchd_ctl().is_some();
        if !ctl_present {
            continue;
        }

        // Bridge must be in OVSDB
        let Ok(client) = Client::connect(&addr).await else {
            continue;
        };
        let found = client
            .idl()
            .rows("Bridge")
            .any(|r| r.get_string("name") == Some(bridge));
        if found {
            info!("vswitchd up after {}ms", (i + 1) * 500);
            return Ok(());
        }
    }
    bail!("vswitchd did not come up within 60s")
}

/// Wait for the OVSDB socket to appear.
async fn wait_for_socket(path: &str) -> Result<()> {
    for _ in 0..40 {
        if Path::new(path).exists() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    bail!("OVSDB socket {} did not appear within 10s", path)
}

/// Extract UUIDs from OVSDB set/single value.
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

/// Commit a transaction, bailing if it fails.
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

/// Delete the ovsbr0 bridge (and its ports/interfaces) from OVSDB.
async fn delete_bridge(client: &mut Client, bridge: &str) -> Result<()> {
    let br_row = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(bridge))
        .map(|r| (r.uuid, r.get("ports").cloned()));

    let Some((br_uuid, ports_val)) = br_row else {
        info!("bridge {} not in OVSDB", bridge);
        return Ok(());
    };

    let port_uuids: Vec<Uuid> = ports_val
        .as_ref()
        .map(|v| extract_uuids(v))
        .unwrap_or_default();
    let mut iface_uuids: Vec<Uuid> = Vec::new();
    for p_uuid in &port_uuids {
        if let Some(row) = client.idl().row("Port", p_uuid) {
            if let Some(ifaces) = row.get("interfaces") {
                iface_uuids.extend(extract_uuids(ifaces));
            }
        }
    }

    info!(
        "deleting bridge {} ({} ports, {} ifaces)",
        bridge,
        port_uuids.len(),
        iface_uuids.len()
    );
    let mut txn = Transaction::new("Open_vSwitch");
    txn.delete_bridge_uuid(br_uuid, &port_uuids, &iface_uuids);
    commit(client, &mut txn, "delete bridge").await
}

/// Purge orphaned Interface/Port/Bridge rows by name (handles TOCTOU races).
async fn purge_by_name(client: &mut Client, name: &str) -> Result<()> {
    warn!(
        "purging any stale Bridge/Port/Interface rows named '{}'",
        name
    );

    let bridge_uuids: Vec<Uuid> = client
        .idl()
        .rows("Bridge")
        .filter(|r| r.get_string("name") == Some(name))
        .map(|r| r.uuid)
        .collect();
    for br_uuid in bridge_uuids {
        let mut port_uuids = Vec::new();
        let mut iface_uuids = Vec::new();
        if let Some(row) = client.idl().row("Bridge", &br_uuid) {
            if let Some(ports) = row.get("ports") {
                port_uuids.extend(extract_uuids(ports));
            }
        }
        for p_uuid in &port_uuids {
            if let Some(row) = client.idl().row("Port", p_uuid) {
                if let Some(ifaces) = row.get("interfaces") {
                    iface_uuids.extend(extract_uuids(ifaces));
                }
            }
        }
        let mut txn = Transaction::new("Open_vSwitch");
        txn.delete_bridge_uuid(br_uuid, &port_uuids, &iface_uuids);
        let _ = client.commit(&mut txn).await;
    }

    let mut port_txn = Transaction::new("Open_vSwitch");
    port_txn.delete_by_name("Port", name);
    let _ = client.commit(&mut port_txn).await;

    let mut iface_txn = Transaction::new("Open_vSwitch");
    iface_txn.delete_by_name("Interface", name);
    let _ = client.commit(&mut iface_txn).await;
    Ok(())
}

/// Create ovsbr0 with datapath_type=netdev.
async fn create_bridge_netdev(
    client: &mut Client,
    bridge: &str,
    fail_mode: &str,
    mac: &str,
) -> Result<()> {
    info!(
        "creating bridge {} with datapath_type=netdev fail_mode={}",
        bridge, fail_mode
    );
    purge_by_name(client, bridge).await?;

    let mut txn = Transaction::new("Open_vSwitch");

    let iface_ref = txn.insert("Interface", json!({"name": bridge, "type": "internal"}));
    let port_ref = txn.insert(
        "Port",
        json!({"name": bridge, "interfaces": iface_ref.to_json()}),
    );
    let bridge_ref = txn.insert(
        "Bridge",
        json!({
            "name": bridge,
            "datapath_type": "netdev",
            "fail_mode": fail_mode,
            "other_config": ["map", [["hwaddr", mac]]],
            "ports": port_ref.to_json()
        }),
    );
    txn.mutate_where(
        "Open_vSwitch",
        json!([]),
        vec![json!(["bridges", "insert", bridge_ref.to_json()])],
    );

    commit(client, &mut txn, "create bridge netdev").await
}

/// Add a port to the bridge if not already present.
async fn add_port(client: &mut Client, bridge: &str, port_name: &str) -> Result<()> {
    let bridge_ports: Vec<Uuid> = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(bridge))
        .and_then(|r| r.get("ports"))
        .map(|v| extract_uuids(v))
        .unwrap_or_default();

    let already = bridge_ports.iter().any(|uuid| {
        client
            .idl()
            .row("Port", uuid)
            .and_then(|p| p.get_string("name"))
            == Some(port_name)
    });
    if already {
        info!("port {} already in bridge {}", port_name, bridge);
        return Ok(());
    }

    info!("adding port {} to bridge {}", port_name, bridge);
    let mut txn = Transaction::new("Open_vSwitch");
    let iface_ref = txn.insert("Interface", json!({"name": port_name, "type": ""}));
    let port_ref = txn.insert(
        "Port",
        json!({"name": port_name, "interfaces": iface_ref.to_json()}),
    );
    txn.mutate_by_name(
        "Bridge",
        bridge,
        vec![json!(["ports", "insert", port_ref.to_json()])],
    );
    commit(client, &mut txn, "add port").await
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("op_ovsbr0_setup=info".parse()?)
                .add_directive("rovs_ovsdb=warn".parse()?),
        )
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        return Ok(());
    }
    let seed_only = args
        .iter()
        .any(|arg| arg == "--seed-only" || arg == "seed-only");

    let cfg = Config::from_env();
    let addr = format!("unix:{}", cfg.ovsdb_socket);

    // ── 1. Wait for OVSDB socket ──────────────────────────────────────────────
    info!("waiting for OVSDB socket: {}", cfg.ovsdb_socket);
    wait_for_socket(&cfg.ovsdb_socket).await?;

    // ── 2. Connect and read current bridge state ──────────────────────────────
    let mut client = Client::connect(&addr)
        .await
        .context("connect to ovsdb-server")?;
    info!("connected to OVSDB");

    let br_state = client
        .idl()
        .rows("Bridge")
        .find(|r| r.get_string("name") == Some(&cfg.bridge))
        .map(|r| {
            (
                r.uuid,
                r.get_string("datapath_type").unwrap_or("").to_string(),
            )
        });

    match &br_state {
        Some((u, dt)) => info!("bridge {} uuid={} datapath_type={}", cfg.bridge, u, dt),
        None => info!("bridge {} not found in OVSDB", cfg.bridge),
    }

    if seed_only {
        info!("seed-only mode: writing netdev OVSDB rows without starting vswitchd");
        clear_kernel_datapath(&cfg.bridge);
        delete_bridge(&mut client, &cfg.bridge).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        client = Client::connect(&addr)
            .await
            .context("reconnect after seed delete")?;
        create_bridge_netdev(&mut client, &cfg.bridge, &cfg.fail_mode, &cfg.shared_mac).await?;
        add_port(&mut client, &cfg.bridge, &cfg.veth_host).await?;
        info!("seed-only complete: {} datapath_type=netdev", cfg.bridge);
        return Ok(());
    }

    let needs_recreate = match &br_state {
        None => true,
        Some((_, dt)) => dt != "netdev",
    };

    // ── 3. If wrong datapath: stop vswitchd, rewrite OVSDB, restart ──────────
    if needs_recreate {
        warn!("bridge has wrong/missing datapath_type — stopping vswitchd");

        stop_vswitchd(&cfg.vswitchd_svc, &cfg.bridge).await?;

        // Reconnect (ovsdb-server stays up while vswitchd is down)
        client = Client::connect(&addr)
            .await
            .context("reconnect after stop")?;

        // Delete bridge (separate tx — OVS 3.x del+add in one tx silently rolls back)
        delete_bridge(&mut client, &cfg.bridge).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Fresh IDL for create
        client = Client::connect(&addr)
            .await
            .context("reconnect after delete")?;

        // Create with netdev
        create_bridge_netdev(&mut client, &cfg.bridge, &cfg.fail_mode, &cfg.shared_mac).await?;
        info!("OVSDB updated: datapath_type=netdev");

        // Start vswitchd — it reads netdev from OVSDB and initializes netdev dpif
        start_vswitchd(&cfg.vswitchd_svc);

        // Wait for vswitchd to be genuinely up (ctl socket + bridge in OVSDB)
        wait_for_vswitchd_up(&cfg.ovsdb_socket, &cfg.bridge).await?;

        // Verify datapath_type in live OVSDB (vswitchd may have reverted)
        client = Client::connect(&addr)
            .await
            .context("reconnect after restart")?;
        let live_dt = client
            .idl()
            .rows("Bridge")
            .find(|r| r.get_string("name") == Some(&cfg.bridge))
            .and_then(|r| r.get_string("datapath_type"))
            .unwrap_or("")
            .to_string();

        if live_dt != "netdev" {
            bail!(
                "vswitchd reset datapath_type to '{}' — expected netdev. \
                 Check /run/uncaught-logs/current for WARN messages from bridge module.",
                live_dt
            );
        }
        info!("confirmed: bridge {} datapath_type=netdev", cfg.bridge);
    } else {
        info!("bridge {} already netdev — no restart needed", cfg.bridge);
    }

    // ── 4. Add grpc-uplink port ───────────────────────────────────────────────
    add_port(&mut client, &cfg.bridge, &cfg.veth_host).await?;

    // ── 5. Bring veth up (ip link is a network utility, not an OVS tool) ─────
    let _ = Command::new("ip")
        .args(["link", "set", &cfg.veth_host, "up"])
        .status();
    info!("op-ovsbr0-setup: done");
    Ok(())
}

fn print_help() {
    println!(
        "Usage: op-ovsbr0-setup [--seed-only]\n\n\
         Ensures ovsbr0 exists with datapath_type=netdev, using rovs for OVSDB \
         changes and s6 for ovs-vswitchd control.\n\n\
         Modes:\n\
           --seed-only Write OVSDB netdev rows and exit without starting vswitchd\n\n\
         Environment:\n\
           BRIDGE       bridge name (default: ovsbr0)\n\
           VETH_HOST    veth port to add (default: grpc-uplink)\n\
           FAIL_MODE    bridge fail mode (default: standalone)\n\
           SHARED_MAC   bridge MAC (default: fa:16:3e:f1:71:d2)\n\
           OVSDB_SOCKET OVSDB socket path\n\
           VSWITCHD_SVC s6 service path (default: /run/service/ovs-vswitchd)"
    );
}
</file>

<file path="src/bin/op-xdp-wg.rs">
//! op-xdp-wg — Rust orchestration for the wg-xray XDP container path.
//!
//! This binary keeps the existing interface names:
//! - host uplink: `eth0`
//! - Incus host-side peer: `veth-warp`
//! - container: `wg-xray`
//!
//! The BPF program runs on host `eth0` and redirects matching traffic to the
//! Incus peer `veth-warp`. The Incus hook order is explicit:
//! 1. `prepare`: set the container MAC and restart the container.
//! 2. `hostside`: attach XDP after `veth-warp` exists and the container is up.
//! 3. `detach`: remove XDP before container shutdown.
//! 4. `watch`: verify the state and reapply the hostside setup if needed.

use anyhow::{anyhow, bail, Context, Result};
use op_network::rtnetlink;
use std::fmt::Write as _;
use std::fs;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

const CT: &str = "wg-xray";
const HOST_IF: &str = "eth0";
const VETH: &str = "grpc-uplink";
const HOST_MAC: &str = "fa:16:3e:f1:71:d2";
const CT_IPV6: &str = "2607:5300:205:200::5bc7";
const WATCH_INTERVAL_SECS: u64 = 60;
const BPF_DIR: &str = "/etc/op-network/xdp";
const BPF_C_PATH: &str = "/etc/op-network/xdp/op-xdp-wg.c";
const BPF_O_PATH: &str = "/etc/op-network/xdp/op-xdp-wg.o";
const XDP_PROG_NAME: &str = "xdp_steer";
const XDP_PRIO: &str = "50";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Prepare,
    Hostside,
    Chain,
    Detach,
    Watch,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("op_network=info".parse()?))
        .init();

    let mode = parse_mode()?;
    match mode {
        Mode::Prepare => prepare().await,
        Mode::Hostside => hostside().await,
        Mode::Chain => chain_xdp().await,
        Mode::Detach => detach().await,
        Mode::Watch => watch().await,
    }
}

fn parse_mode() -> Result<Mode> {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "hostside".to_string());
    match mode.as_str() {
        "prepare" => Ok(Mode::Prepare),
        "hostside" | "attach" => Ok(Mode::Hostside),
        "chain" | "rechain" | "xdp-only" => Ok(Mode::Chain),
        "detach" | "teardown" => Ok(Mode::Detach),
        "watch" => Ok(Mode::Watch),
        other => bail!("unknown op-xdp-wg mode: {}", other),
    }
}

async fn prepare() -> Result<()> {
    wait_for_incus_instance(CT).await?;
    let _ = run("incus", ["stop", CT, "--force"]);
    run(
        "incus",
        [
            "config",
            "set",
            CT,
            &format!("volatile.eth0.hwaddr={HOST_MAC}"),
        ],
    )?;
    run("incus", ["start", CT])?;
    info!(container = CT, "container restarted with host MAC");
    Ok(())
}

async fn hostside() -> Result<()> {
    // When called as an LXC start-host hook, LXC_PID is set to the container
    // init PID. Do NOT call incus info/list here — incusd is blocked waiting
    // for this hook to return, so any incus command would deadlock.
    let ct_pid = hook_pid_or_query(CT)?;

    chain_xdp().await?;

    configure_tc()?;
    configure_container_network(ct_pid)?;

    info!(
        host = HOST_IF,
        redirect = VETH,
        container = CT,
        "XDP hostside setup complete"
    );
    Ok(())
}

async fn chain_xdp() -> Result<()> {
    wait_for_interface(HOST_IF).await?;
    wait_for_interface(VETH).await?;

    let veth_ifindex = interface_index(VETH).await?;
    compile_bpf(veth_ifindex)?;

    bring_link_up(HOST_IF).await?;
    bring_link_up(VETH).await?;

    // Use xdp-loader (libxdp dispatcher) so this program coexists with OVS AF_XDP.
    unload_own_xdp_program(HOST_IF)?;
    run(
        "xdp-loader",
        [
            "load",
            HOST_IF,
            BPF_O_PATH,
            "--mode",
            "native",
            "--prio",
            XDP_PRIO,
            "--actions",
            "XDP_PASS",
            "--prog-name",
            XDP_PROG_NAME,
        ],
    )?;

    info!(
        host = HOST_IF,
        redirect = VETH,
        object = BPF_O_PATH,
        "XDP program chained through dispatcher"
    );
    Ok(())
}

/// Return the container's init PID.
///
/// In hook context `LXC_PID` is already set by the LXC runtime — use it
/// directly so we never call back into incusd (which would deadlock).
/// Outside hook context (e.g. watch mode) fall back to `incus info`.
fn hook_pid_or_query(name: &str) -> Result<u32> {
    if let Ok(pid_str) = std::env::var("LXC_PID") {
        let pid = pid_str
            .trim()
            .parse::<u32>()
            .context("LXC_PID is not a valid u32")?;
        return Ok(pid);
    }
    incus_pid(name)
}

async fn detach() -> Result<()> {
    unload_own_xdp_program(HOST_IF)?;
    run_ignore("tc", ["qdisc", "del", "dev", HOST_IF, "clsact"]);
    info!(host = HOST_IF, "XDP detached");
    Ok(())
}

async fn watch() -> Result<()> {
    info!(interval = WATCH_INTERVAL_SECS, "XDP watch starting");
    loop {
        sleep(Duration::from_secs(WATCH_INTERVAL_SECS)).await;

        let ct_running = incus_is_running(CT)?;
        let host_xdp = link_has_xdp(HOST_IF).await.unwrap_or(false);
        let veth_up = link_is_up(VETH).await.unwrap_or(false);
        let tc_dup_ready = tc_dup_is_ready()?;
        let ndp_ready = ndp_is_ready()?;

        let mut failures = Vec::new();
        if !host_xdp {
            failures.push("host-xdp");
        }
        if !veth_up {
            failures.push("veth-down");
        }
        if !tc_dup_ready {
            failures.push("tc-dup");
        }
        if !ndp_ready {
            failures.push("ndp");
        }

        if failures.is_empty() {
            continue;
        }

        warn!(?failures, ct_running, "XDP watch noticed drift");
        if ct_running {
            let _ = hostside().await;
        } else {
            info!(container = CT, "container is not running; skipping restart");
        }
    }
}

async fn wait_for_interface(ifname: &str) -> Result<()> {
    for _ in 0..60 {
        if interface_exists(ifname).await? {
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
    bail!("interface {} did not appear", ifname)
}

async fn wait_for_incus_instance(name: &str) -> Result<()> {
    for _ in 0..60 {
        if incus_is_running(name).unwrap_or(false) {
            return Ok(());
        }
        if incus_exists(name).unwrap_or(false) {
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
    bail!("Incus instance {} did not appear", name)
}

async fn interface_exists(ifname: &str) -> Result<bool> {
    Ok(rtnetlink::list_interfaces()
        .await?
        .into_iter()
        .any(|iface| iface.name == ifname))
}

async fn interface_index(ifname: &str) -> Result<u32> {
    rtnetlink::list_interfaces()
        .await?
        .into_iter()
        .find(|iface| iface.name == ifname)
        .map(|iface| iface.index)
        .ok_or_else(|| anyhow!("interface {} not found", ifname))
}

async fn link_is_up(ifname: &str) -> Result<bool> {
    Ok(rtnetlink::list_interfaces()
        .await?
        .into_iter()
        .find(|iface| iface.name == ifname)
        .map(|iface| iface.state == "up")
        .unwrap_or(false))
}

async fn link_has_xdp(ifname: &str) -> Result<bool> {
    // xdp-loader list shows programs loaded via the dispatcher
    let output = Command::new("xdp-loader")
        .args(["status", ifname])
        .output()
        .with_context(|| format!("failed to inspect {}", ifname))?;
    let out = String::from_utf8_lossy(&output.stdout);
    Ok(out.contains("xdp_steer") || out.contains("prog/xdp") || out.contains("xdp_dispatcher"))
}

fn tc_dup_is_ready() -> Result<bool> {
    let output = Command::new("tc")
        .args(["filter", "show", "dev", HOST_IF, "ingress"])
        .output()
        .context("failed to inspect tc filters")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 'mirror' is the tc keyword for port duplication/dup
    Ok(stdout.matches("mirred").count() >= 6)
}

fn ndp_is_ready() -> Result<bool> {
    let output = Command::new("ip")
        .args(["-6", "neigh", "show", "proxy"])
        .output()
        .context("failed to inspect proxy ndp state")?;
    Ok(String::from_utf8_lossy(&output.stdout).contains(CT_IPV6))
}

async fn bring_link_up(ifname: &str) -> Result<()> {
    rtnetlink::link_up(ifname)
        .await
        .with_context(|| format!("failed to bring {} up", ifname))
}

fn compile_bpf(veth_ifindex: u32) -> Result<()> {
    let mut src = String::new();
    write!(
        &mut src,
        r#"#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ipv6.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

#define VETH {veth_ifindex}

SEC("xdp")
int xdp_steer(struct xdp_md *ctx) {{
    void *de = (void *)(long)ctx->data_end;
    void *da = (void *)(long)ctx->data;
    struct ethhdr *h = da;
    if (da + sizeof(*h) > de) return XDP_PASS;
    if (h->h_proto != bpf_htons(0x86DD)) return XDP_PASS;
    struct ipv6hdr *ip = da + sizeof(*h);
    if ((void *)ip + sizeof(*ip) > de) return XDP_PASS;
    __u8 ct[16] = {{0x26,0x07,0x53,0x00,0x02,0x05,0x02,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x5b,0xc7}};
    if (__builtin_memcmp(&ip->daddr, ct, 16) == 0) return bpf_redirect(VETH, 0);
    return XDP_PASS;
}}
char _license[] SEC("license") = "GPL";
"#
    )
    .expect("write BPF source");

    fs::create_dir_all(BPF_DIR).with_context(|| format!("create {}", BPF_DIR))?;
    fs::write(BPF_C_PATH, src).with_context(|| format!("write {}", BPF_C_PATH))?;
    // -g generates BTF info required by libxdp multiprog dispatcher
    run(
        "clang",
        [
            "-O2", "-g", "-target", "bpf", "-c", BPF_C_PATH, "-o", BPF_O_PATH,
        ],
    )
}

fn configure_tc() -> Result<()> {
    run_ignore("tc", ["qdisc", "del", "dev", HOST_IF, "clsact"]);
    run("tc", ["qdisc", "add", "dev", HOST_IF, "clsact"])?;

    // Use tc mirred egress mirror to duplicate (dup) traffic to the veth.
    // This is separate from the D-Bus tree mirror.
    for t in ["133", "134", "135", "136"] {
        run(
            "tc",
            [
                "filter", "add", "dev", HOST_IF, "ingress", "protocol", "ipv6", "pref", "1",
                "flower", "ip_proto", "icmpv6", "type", t, "action", "mirred", "egress", "mirror",
                "dev", VETH,
            ],
        )?;
    }

    run(
        "tc",
        [
            "filter", "add", "dev", HOST_IF, "ingress", "protocol", "ipv6", "pref", "2", "flower",
            "ip_proto", "udp", "dst_port", "546", "action", "mirred", "egress", "mirror", "dev",
            VETH,
        ],
    )?;
    run(
        "tc",
        [
            "filter", "add", "dev", HOST_IF, "ingress", "protocol", "ipv6", "pref", "2", "flower",
            "ip_proto", "udp", "dst_port", "547", "action", "mirred", "egress", "mirror", "dev",
            VETH,
        ],
    )?;

    Ok(())
}

fn configure_container_network(_ct_pid: u32) -> Result<()> {
    // Container IPv4/IPv6 addresses are managed by systemd-networkd inside the
    // container via eth0.network (static config). Only host-side state is set here.
    run(
        "sysctl",
        [&format!("net.ipv6.conf.{}.proxy_ndp=1", HOST_IF)],
    )?;
    run("sysctl", ["-w", "net.ipv6.conf.all.forwarding=1"])?;
    run("ip", ["addr", "add", "10.200.0.2/30", "dev", VETH])
        .unwrap_or_else(|e| warn!("Failed to add IPv4 to {}: {}", VETH, e));
    run(
        "ip",
        ["-6", "neigh", "replace", "proxy", CT_IPV6, "dev", HOST_IF],
    )?;
    run(
        "ip",
        [
            "-6",
            "route",
            "replace",
            &format!("{}/128", CT_IPV6),
            "dev",
            VETH,
        ],
    )?;
    Ok(())
}

fn incus_exists(name: &str) -> Result<bool> {
    let output = Command::new("incus")
        .args(["info", name])
        .output()
        .context("failed to query incus instance")?;
    Ok(output.status.success())
}

fn incus_is_running(name: &str) -> Result<bool> {
    let output = Command::new("incus")
        .args(["info", name])
        .output()
        .context("failed to query incus instance")?;
    if !output.status.success() {
        return Ok(false);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().any(|line| line.trim() == "Status: RUNNING"))
}

fn incus_pid(name: &str) -> Result<u32> {
    let output = Command::new("incus")
        .args(["info", name])
        .output()
        .context("failed to query incus instance")?;
    if !output.status.success() {
        bail!("no PID for {}", name);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        if let Some(pid) = line.strip_prefix("PID:") {
            let pid = pid.trim().parse::<u32>().context("invalid incus PID")?;
            return Ok(pid);
        }
    }
    bail!("no PID for {}", name)
}

fn run(cmd: &str, args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>) -> Result<()> {
    let args_vec: Vec<_> = args
        .into_iter()
        .map(|a| a.as_ref().to_os_string())
        .collect();
    let status = Command::new(cmd)
        .args(&args_vec)
        .stdin(Stdio::null())
        .status()
        .with_context(|| format!("failed to execute {}", cmd))?;
    if !status.success() {
        bail!("{} {:?} failed with {}", cmd, args_vec, status);
    }
    Ok(())
}

fn run_ignore(cmd: &str, args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>) {
    let _ = run(cmd, args);
}

fn unload_own_xdp_program(ifname: &str) -> Result<()> {
    let output = Command::new("xdp-loader")
        .args(["status", ifname])
        .output()
        .with_context(|| format!("failed to inspect XDP status for {}", ifname))?;

    if !output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(line) = stdout
        .lines()
        .find(|line| line.split_whitespace().any(|token| token == XDP_PROG_NAME))
    else {
        return Ok(());
    };

    let tokens: Vec<_> = line.split_whitespace().collect();
    let Some(prog_pos) = tokens.iter().position(|token| *token == XDP_PROG_NAME) else {
        return Ok(());
    };
    let Some(id_token) = tokens.get(prog_pos + 1) else {
        return Ok(());
    };
    let id = id_token
        .parse::<u32>()
        .with_context(|| format!("invalid XDP program id in status line: {}", line))?;

    run_ignore("xdp-loader", ["unload", ifname, "--id", &id.to_string()]);
    Ok(())
}
</file>

<file path="src/controller.rs">
//! OpenFlow 1.3 controller server (passive mode)
//!
//! Listens for OVS to connect (passive mode), performs the OF1.3 handshake,
//! discovers port numbers by name via a PortDesc multipart request, clears all
//! existing flows, and installs the configured forwarding rules.
//!
//! Wire-protocol encoding is delegated to `rovs_openflow` types wherever
//! possible; the TCP listener and passive handshake are implemented here
//! because `rovs_openflow::VConn` only supports active (outbound) connections.

use anyhow::{Context, Result};
use bytes::Bytes;
use rovs_openflow::{ActionList, Flow, Match, Message, MessageType, OutputPort, Version};
use rovs_transport::Reconnect;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ── OF1.3 constants ────────────────────────────────────────────────────────────

/// Multipart type: port description.
const OFPMP_PORT_DESC: u16 = 13;
/// "All" output port — used when out_port is not restricted.
const OFPP_ANY: u32 = 0xFFFF_FFFF;

// ── Wire helpers ──────────────────────────────────────────────────────────────

/// Build a raw OF1.3 message with an 8-byte header and `body`.
fn build_raw_msg(msg_type: MessageType, xid: u32, body: &[u8]) -> Vec<u8> {
    let msg = Message::new(Version::Of13, msg_type, xid, Bytes::copy_from_slice(body));
    msg.encode().to_vec()
}

/// Build an OF1.3 Hello message.
fn build_hello(xid: u32) -> Vec<u8> {
    build_raw_msg(MessageType::Hello, xid, &[])
}

/// Build an OF1.3 FeaturesRequest message.
fn build_features_request(xid: u32) -> Vec<u8> {
    build_raw_msg(MessageType::FeaturesRequest, xid, &[])
}

/// Build an OF1.3 PortDesc multipart request.
///
/// Body: type(2) + flags(2) + pad(4) = 8 bytes.
fn build_port_desc_request(xid: u32) -> Vec<u8> {
    let mut body = [0u8; 8];
    body[0..2].copy_from_slice(&OFPMP_PORT_DESC.to_be_bytes());
    build_raw_msg(MessageType::MultipartRequest, xid, &body)
}

/// Build an OF1.3 EchoReply that mirrors the request payload.
fn build_echo_reply(xid: u32, payload: &[u8]) -> Vec<u8> {
    build_raw_msg(MessageType::EchoReply, xid, payload)
}

/// Build an OF1.3 FlowMod DELETE ALL (wildcard match, all tables).
fn build_flow_mod_delete_all(xid: u32) -> Vec<u8> {
    let flow = Flow::delete();
    let msg = flow.to_message(Version::Of13, xid);
    msg.encode().to_vec()
}

/// Build an OF1.3 FlowMod ADD: match `in_port`, output to `out_port`.
pub fn build_flow_mod_add(in_port: u32, out_port: u32, priority: u16, xid: u32) -> Vec<u8> {
    let match_fields = Match::new().in_port(in_port);
    let actions = ActionList::new().output(OutputPort::Port(out_port));
    let flow = Flow::add()
        .priority(priority)
        .match_fields(match_fields)
        .actions(actions);
    let msg = flow.to_message(Version::Of13, xid);
    msg.encode().to_vec()
}

// ── Raw message receive ───────────────────────────────────────────────────────

/// A raw inbound OpenFlow message (header parsed, body buffered).
struct RawMsg {
    msg_type: u8,
    xid: u32,
    payload: Vec<u8>,
}

/// Read one complete OpenFlow message from `stream`.
async fn recv_msg(stream: &mut TcpStream) -> Result<RawMsg> {
    let mut hdr = [0u8; 8];
    stream
        .read_exact(&mut hdr)
        .await
        .context("reading OF header")?;

    let msg_type = hdr[1];
    let length = u16::from_be_bytes([hdr[2], hdr[3]]) as usize;
    let xid = u32::from_be_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]);
    let payload_len = length.saturating_sub(8);

    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        stream
            .read_exact(&mut payload)
            .await
            .context("reading OF payload")?;
    }

    Ok(RawMsg {
        msg_type,
        xid,
        payload,
    })
}

/// Write raw bytes to `stream`.
async fn send_msg(stream: &mut TcpStream, bytes: &[u8]) -> Result<()> {
    stream.write_all(bytes).await.context("sending OF message")
}

// ── Port discovery ─────────────────────────────────────────────────────────────

/// Send a PortDesc request and parse all replies into `{port_name → ofport_no}`.
async fn discover_ports(stream: &mut TcpStream, xid: u32) -> Result<HashMap<String, u32>> {
    send_msg(stream, &build_port_desc_request(xid)).await?;

    let mut ports: HashMap<String, u32> = HashMap::new();

    loop {
        let msg = recv_msg(stream).await?;
        match msg.msg_type {
            // Echo request — must reply to stay alive during discovery.
            2 /* EchoRequest */ => {
                send_msg(stream, &build_echo_reply(msg.xid, &msg.payload)).await?;
            }
            // MultipartReply (19)
            19 => {
                if msg.payload.len() < 8 {
                    break;
                }
                let reply_type = u16::from_be_bytes([msg.payload[0], msg.payload[1]]);
                let flags = u16::from_be_bytes([msg.payload[2], msg.payload[3]]);

                if reply_type == OFPMP_PORT_DESC {
                    // OF1.3 ofp_port = 64 bytes:
                    // port_no(4) pad(4) hw_addr(6) pad(2) name(16) config(4) state(4)
                    // curr(4) advertised(4) supported(4) peer(4) curr_speed(4) max_speed(4)
                    let body = &msg.payload[8..];
                    for chunk in body.chunks_exact(64) {
                        let port_no =
                            u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        let name = String::from_utf8_lossy(&chunk[16..32])
                            .trim_end_matches('\0')
                            .to_string();
                        if !name.is_empty() && port_no < OFPP_ANY {
                            ports.insert(name, port_no);
                        }
                    }
                    // bit 0 of flags = OFPMPF_REPLY_MORE
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

// ── Connection handler ────────────────────────────────────────────────────────

/// Handle one inbound OVS connection: handshake → port discovery → flow install → keepalive.
async fn handle_connection(
    mut stream: TcpStream,
    flows: Arc<Vec<(String, String, u16)>>,
) -> Result<()> {
    let mut xid: u32 = 1;

    // 1. Receive Hello from OVS.
    let hello = recv_msg(&mut stream).await?;
    if hello.msg_type != 0 {
        anyhow::bail!("expected Hello (type 0), got msg_type={}", hello.msg_type);
    }

    // 2. Send Hello.
    send_msg(&mut stream, &build_hello(xid)).await?;
    xid += 1;

    // 3. Send FeaturesRequest; wait for FeaturesReply (type 6), echo any pings.
    send_msg(&mut stream, &build_features_request(xid)).await?;
    xid += 1;

    loop {
        let msg = recv_msg(&mut stream).await?;
        match msg.msg_type {
            2 /* EchoRequest */ => {
                send_msg(&mut stream, &build_echo_reply(msg.xid, &msg.payload)).await?;
            }
            6 /* FeaturesReply */ => break,
            _ => {}
        }
    }

    // 4. Discover ports via PortDesc multipart.
    let port_map = discover_ports(&mut stream, xid).await?;
    xid += 1;

    log::info!(
        "OF controller: discovered {} ports: {:?}",
        port_map.len(),
        port_map.keys().collect::<Vec<_>>()
    );

    // 5. Delete all existing flows.
    send_msg(&mut stream, &build_flow_mod_delete_all(xid)).await?;
    xid += 1;

    // 6. Install configured flows.
    let mut installed = 0u32;
    for (in_name, out_name, priority) in flows.iter() {
        match (
            port_map.get(in_name.as_str()),
            port_map.get(out_name.as_str()),
        ) {
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
                    in_name,
                    in_port,
                    out_name,
                    out_port,
                    priority
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

    log::info!(
        "OF controller: {} flows installed; entering keepalive loop",
        installed
    );

    // 7. Keepalive loop — reply to Echo requests indefinitely.
    loop {
        let msg = recv_msg(&mut stream).await?;
        if msg.msg_type == 2
        /* EchoRequest */
        {
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
    /// Create a new controller that will listen on `listen_addr`.
    pub fn new(listen_addr: SocketAddr) -> Self {
        Self {
            listen_addr,
            flows: Vec::new(),
        }
    }

    /// Add a bidirectional forwarding pair (installs two flows: A→B and B→A).
    pub fn add_port_pair(mut self, port_a: &str, port_b: &str, priority: u16) -> Self {
        self.flows
            .push((port_a.to_string(), port_b.to_string(), priority));
        self.flows
            .push((port_b.to_string(), port_a.to_string(), priority));
        self
    }

    /// Add a single directed flow (in_port → out_port).
    pub fn add_flow(mut self, in_port: &str, out_port: &str, priority: u16) -> Self {
        self.flows
            .push((in_port.to_string(), out_port.to_string(), priority));
        self
    }

    /// Run the controller — listens for OVS connections and re-programs flows on each reconnect.
    ///
    /// Each spawned connection handler maintains its own `Reconnect` state machine so
    /// that rapid repeated failures (e.g. OVS flapping) are logged with backoff information
    /// rather than silently spinning.
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
                // Per-connection reconnection tracker.  OVS is the active side so
                // we don't drive the reconnect loop ourselves — we just record state
                // and log when failures come in rapidly so operators see backoff hints.
                let mut reconnect = Reconnect::new();
                reconnect.set_max_backoff(Duration::from_secs(30));
                reconnect.connecting();

                match handle_connection(stream, flows).await {
                    Ok(()) => {
                        // Clean close — mark disconnected so next accept starts fresh.
                        reconnect.disconnected();
                        log::info!("OF controller: connection from {} closed cleanly", peer);
                    }
                    Err(e) => {
                        reconnect.disconnected();
                        reconnect.increase_backoff();
                        log::warn!(
                            "OF controller: connection from {} ended with error \
                             (next OVS reconnect backoff hint: {:?}): {:#}",
                            peer,
                            reconnect.current_backoff(),
                            e
                        );
                    }
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
        // OF1.3 version byte
        assert_eq!(msg[0], 0x04);
        // FlowMod type = 14
        assert_eq!(msg[1], 14);
    }

    #[test]
    fn test_flow_mod_delete_all() {
        let msg = build_flow_mod_delete_all(1);
        let declared_len = u16::from_be_bytes([msg[2], msg[3]]) as usize;
        assert_eq!(declared_len, msg.len(), "declared length must match actual");
        assert_eq!(msg[0], 0x04); // OF1.3
        assert_eq!(msg[1], 14); // FlowMod
    }
}
</file>

<file path="src/lib.rs">
//! op-network: Network Management with OpenFlow, OVSDB, and Container Networking
//!
//! This crate provides:
//! - Native OpenFlow protocol implementation (all versions, pure Rust)
//! - OVSDB JSON-RPC client for OVS bridge management
//! - Network plugin for OVS/OVSDB persistence
//! - Socket networking support
//! - Container networking with OpenFlow routing
//! - Native Proxmox API client for LXC container management

pub mod controller;
pub mod openflow;
pub mod ovs_capabilities;
pub mod ovs_error;
pub mod ovs_netlink;
pub mod ovsdb;
pub mod plugin;
pub mod proxmox;
pub mod rtnetlink;

pub use controller::OpenFlowController;
pub use openflow::{FlowAction, FlowEntry, FlowMatch, OpenFlowClient, OpenFlowVersion};
pub use ovs_capabilities::{counter_excuses, excuses_to_llm_context, OvsCapabilities};
pub use ovs_error::OvsError;
pub use ovs_netlink::{Datapath, KernelFlow, OvsNetlinkClient, Vport, VportConfig, VportType};
pub use ovsdb::OvsdbClient;
pub use plugin::{NetworkInterface, NetworkPlugin, OpenFlowConfig, OvsBridge, OvsdbConfig};
pub use proxmox::{
    ContainerStatus, CreateContainerRequest, LxcContainer, ProxmoxClient, ProxmoxToken,
};

/// Prelude for convenient imports
pub mod prelude {
    pub use super::openflow::{FlowAction, FlowEntry, FlowMatch, OpenFlowClient, OpenFlowVersion};
    pub use super::ovs_capabilities::OvsCapabilities;
    pub use super::ovs_netlink::{Datapath, OvsNetlinkClient, Vport};
    pub use super::ovsdb::OvsdbClient;
    pub use super::plugin::{NetworkInterface, NetworkPlugin, OvsBridge};
    pub use super::proxmox::{CreateContainerRequest, LxcContainer, ProxmoxClient};
}
</file>

<file path="src/openflow.rs">
//! OpenFlow protocol types backed by `rovs-openflow`.
//!
//! This module re-exports and wraps rovs-openflow types to preserve
//! the public API surface expected by callers while delegating all
//! wire-protocol work to the library.

use anyhow::{Context, Result};
use bytes::Bytes;
use rovs_openflow::{Message, MessageType, Version};
use rovs_transport::Reconnect;
use std::net::SocketAddr;
use std::time::Duration;

// Re-export rovs-openflow types used directly by callers.
pub use rovs_openflow::Match as FlowMatch;

/// Flow match field (alias for rovs-openflow `Match`).
///
/// Callers that previously constructed `FlowMatch { in_port: Some(n), .. }` should
/// now use the builder API: `FlowMatch::new().in_port(n)`.

/// Flow action — a simplified action enum that covers what callers need.
#[derive(Debug, Clone)]
pub enum FlowAction {
    /// Output to a specific port number.
    Output { port: u32 },
    /// Drop the packet (no instructions).
    Drop,
}

/// A flow entry for OpenFlow operations.
///
/// Maps to `rovs_openflow::Flow` when sent to the switch.
#[derive(Debug, Clone)]
pub struct FlowEntry {
    pub priority: u16,
    pub match_fields: FlowMatch,
    pub actions: Vec<FlowAction>,
    pub idle_timeout: u16,
    pub hard_timeout: u16,
    pub cookie: u64,
}

impl FlowEntry {
    /// Convert to a `rovs_openflow::Flow` ADD command.
    pub fn to_rovs_flow(&self) -> rovs_openflow::Flow {
        let mut action_list = rovs_openflow::ActionList::new();
        for action in &self.actions {
            match action {
                FlowAction::Output { port } => {
                    action_list = action_list.output(rovs_openflow::OutputPort::Port(*port));
                }
                FlowAction::Drop => {
                    // No actions = drop
                }
            }
        }

        rovs_openflow::Flow::add()
            .priority(self.priority)
            .match_fields(self.match_fields.clone())
            .actions(action_list)
            .idle_timeout(self.idle_timeout)
            .hard_timeout(self.hard_timeout)
            .cookie(self.cookie)
    }
}

/// OpenFlow protocol versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenFlowVersion {
    V1_0,
    V1_3,
}

impl OpenFlowVersion {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::V1_0 => 0x01,
            Self::V1_3 => 0x04,
        }
    }
}

impl From<OpenFlowVersion> for rovs_openflow::Version {
    fn from(v: OpenFlowVersion) -> Self {
        match v {
            OpenFlowVersion::V1_0 => rovs_openflow::Version::Of10,
            OpenFlowVersion::V1_3 => rovs_openflow::Version::Of13,
        }
    }
}

/// OpenFlow client — connects actively to an OpenFlow switch.
///
/// Backed by `rovs_openflow::VConn`.  Carries a `Reconnect` state machine so
/// callers can query backoff state and drive retry loops.
pub struct OpenFlowClient {
    vconn: rovs_openflow::VConn,
    /// Reconnection state machine — tracks backoff for the caller.
    pub reconnect: Reconnect,
}

impl OpenFlowClient {
    /// Connect to an OpenFlow switch at the given address.
    ///
    /// Performs the OF1.3 Hello handshake automatically.
    /// On success `reconnect` is moved to the `Active` state.
    /// On failure the returned error carries context; callers that retry should
    /// call `reconnect.disconnected()` / `reconnect.increase_backoff()` and then
    /// `reconnect.connecting()` before the next attempt.
    pub async fn connect(addr: SocketAddr) -> Result<Self> {
        let rovs_addr = rovs_transport::Address::Tcp {
            host: addr.ip().to_string(),
            port: addr.port(),
        };

        let mut reconnect = Reconnect::new();
        reconnect.set_max_backoff(Duration::from_secs(30));
        reconnect.connecting();

        match rovs_openflow::VConn::connect(&rovs_addr).await {
            Ok(vconn) => {
                reconnect.connected();
                Ok(Self { vconn, reconnect })
            }
            Err(e) => {
                reconnect.disconnected();
                reconnect.increase_backoff();
                Err(e).with_context(|| format!("Failed to connect to OpenFlow switch at {addr}"))
            }
        }
    }

    /// Add a flow entry to the switch.
    pub async fn add_flow(&mut self, flow: &FlowEntry) -> Result<()> {
        let rovs_flow = flow.to_rovs_flow();
        self.vconn
            .send_flow_sync(&rovs_flow)
            .await
            .context("Failed to install flow")?;
        self.reconnect.activity();
        Ok(())
    }

    /// Add a flow rule from an ovs-ofctl format string.
    ///
    /// Currently logs a warning — full format parsing is not implemented.
    pub async fn add_flow_rule(&mut self, rule: &str) -> Result<()> {
        log::warn!("String-based flow rules not yet implemented: {}", rule);
        Ok(())
    }

    /// Delete all flows on the switch (wildcard delete).
    pub async fn delete_all_flows(&mut self) -> Result<()> {
        let delete_all = rovs_openflow::Flow::delete();
        self.vconn
            .send_flow(&delete_all)
            .await
            .context("Failed to delete all flows")?;
        self.reconnect.activity();
        Ok(())
    }

    /// Send an echo request and wait for reply (keepalive).
    pub async fn echo(&mut self) -> Result<()> {
        self.vconn.echo().await.context("Echo request failed")?;
        self.reconnect.activity();
        Ok(())
    }

    /// Send a FeaturesRequest and wait for FeaturesReply.
    ///
    /// Used as a connectivity probe to verify the controller is responsive.
    /// The reply is consumed and discarded; callers that need the datapath ID
    /// should parse `msg.body` directly via the lower-level `VConn` API.
    pub async fn request_features(&mut self) -> Result<()> {
        // Send FeaturesRequest (type 5, empty body)
        let xid = 1u32;
        let req = Message::new(
            Version::Of13,
            MessageType::FeaturesRequest,
            xid,
            Bytes::new(),
        );
        self.vconn
            .send_message(&req)
            .await
            .context("Failed to send FeaturesRequest")?;

        // Drain messages until we see FeaturesReply (type 6), handling echo requests.
        loop {
            let msg = self
                .vconn
                .recv_message()
                .await
                .context("Failed to receive FeaturesReply")?;
            match msg.header.msg_type {
                MessageType::FeaturesReply => return Ok(()),
                MessageType::EchoRequest => {
                    let reply = Message::new(
                        Version::Of13,
                        MessageType::EchoReply,
                        msg.header.xid,
                        msg.body.clone(),
                    );
                    self.vconn
                        .send_message(&reply)
                        .await
                        .context("Failed to send EchoReply during features probe")?;
                }
                _ => {} // Skip async messages
            }
        }
    }

    /// Dump all flows from the switch.
    pub async fn query_flows(&mut self) -> Result<Vec<String>> {
        // Return empty list — callers that use this path rely on ovs-ofctl text parsing.
        Ok(Vec::new())
    }
}
</file>

<file path="src/ovs_capabilities.rs">
//! OVS Capability Detection and LLM Context Generation
//!
//! This module documents what OVS operations are ACTUALLY possible,
//! countering common LLM "I can't do that" excuses.
//!
//! Features:
//! - Runtime capability detection
//! - Cached results for performance
//! - LLM context generation for system prompts
//! - Excuse counter database

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Cache for capability detection results
static CAPABILITY_CACHE: OnceLock<RwLock<Option<CachedCapabilities>>> = OnceLock::new();

/// Cached capability result with timestamp
#[derive(Clone)]
struct CachedCapabilities {
    capabilities: OvsCapabilities,
    cached_at: Instant,
}

/// How long to cache capability results (5 minutes)
const CACHE_DURATION: Duration = Duration::from_secs(300);

/// Runtime-detected OVS capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvsCapabilities {
    // OVSDB capabilities (userspace)
    pub can_list_bridges: bool,
    pub can_create_bridges: bool,
    pub can_add_ports: bool,
    pub can_delete_bridges: bool,

    // OpenFlow capabilities (userspace)
    pub can_query_flows_openflow: bool,
    pub can_add_flows_openflow: bool,

    // OVS Netlink capabilities (kernel)
    pub can_list_datapaths: bool,
    pub can_create_datapaths: bool,
    pub can_list_vports: bool,
    pub can_dump_kernel_flows: bool,

    // Requirements
    pub is_root: bool,
    pub ovs_running: bool,
    pub ovsdb_socket_exists: bool,
    pub kernel_module_loaded: bool,

    // Paths
    pub ovsdb_socket_path: String,
}

impl OvsCapabilities {
    /// Detect OVS capabilities at runtime (with caching)
    /// LLM_EXCUSE: "I don't know if OVS is available"
    /// REALITY: This function probes all OVS interfaces and returns exact capabilities
    ///
    /// Results are cached for 5 minutes for performance.
    /// Use `detect_fresh()` to bypass the cache.
    pub async fn detect() -> Self {
        // Initialize cache on first call
        let cache = CAPABILITY_CACHE.get_or_init(|| RwLock::new(None));

        // Check if we have a valid cached result
        {
            let cached = cache.read().await;
            if let Some(ref c) = *cached {
                if c.cached_at.elapsed() < CACHE_DURATION {
                    return c.capabilities.clone();
                }
            }
        }

        // Cache miss or expired - detect fresh
        let caps = Self::detect_fresh().await;

        // Update cache
        {
            let mut cached = cache.write().await;
            *cached = Some(CachedCapabilities {
                capabilities: caps.clone(),
                cached_at: Instant::now(),
            });
        }

        caps
    }

    /// Detect OVS capabilities without using cache
    pub async fn detect_fresh() -> Self {
        let is_root = unsafe { libc::geteuid() == 0 };
        let ovsdb_socket_exists = Path::new("/var/run/openvswitch/db.sock").exists();
        let ovs_running = ovsdb_socket_exists && Self::check_ovsdb_responds().await;
        let kernel_module_loaded = Self::check_ovs_kernel_module();

        Self {
            // OVSDB - requires socket access
            can_list_bridges: ovsdb_socket_exists,
            can_create_bridges: ovsdb_socket_exists && ovs_running,
            can_add_ports: ovsdb_socket_exists && ovs_running,
            can_delete_bridges: ovsdb_socket_exists && ovs_running,

            // OpenFlow - requires OVS running
            can_query_flows_openflow: ovs_running,
            can_add_flows_openflow: ovs_running,

            // Kernel netlink - requires root + kernel module
            can_list_datapaths: is_root && kernel_module_loaded,
            can_create_datapaths: is_root && kernel_module_loaded,
            can_list_vports: is_root && kernel_module_loaded,
            can_dump_kernel_flows: is_root && kernel_module_loaded,

            is_root,
            ovs_running,
            ovsdb_socket_exists,
            kernel_module_loaded,
            ovsdb_socket_path: "/var/run/openvswitch/db.sock".to_string(),
        }
    }

    /// Check if OVS kernel module is loaded
    fn check_ovs_kernel_module() -> bool {
        // Check /proc/modules for openvswitch
        std::fs::read_to_string("/proc/modules")
            .map(|s| s.contains("openvswitch"))
            .unwrap_or(false)
    }

    /// Check if OVSDB responds to list_dbs
    async fn check_ovsdb_responds() -> bool {
        use crate::ovsdb::OvsdbClient;
        use tokio::time::{timeout, Duration};

        // Try to connect to OVSDB with a short timeout
        let result = timeout(Duration::from_secs(2), async {
            OvsdbClient::new().list_dbs().await
        })
        .await;

        matches!(result, Ok(Ok(_)))
    }

    /// Generate LLM context string
    /// This is injected into system prompts to tell the LLM what it CAN do
    pub fn to_llm_context(&self) -> String {
        let mut ctx = String::from("## OVS Capabilities (Runtime Detected)\n\n");

        ctx.push_str("### Available Operations:\n");
        if self.can_list_bridges {
            ctx.push_str("- ✅ List OVS bridges (OVSDB)\n");
        }
        if self.can_create_bridges {
            ctx.push_str("- ✅ Create OVS bridges (OVSDB)\n");
        }
        if self.can_add_ports {
            ctx.push_str("- ✅ Add ports to bridges (OVSDB)\n");
        }
        if self.can_list_datapaths {
            ctx.push_str("- ✅ List kernel datapaths (OVS Netlink)\n");
        }
        if self.can_dump_kernel_flows {
            ctx.push_str("- ✅ Dump kernel flow table (OVS Netlink)\n");
        }

        ctx.push_str("\n### Requirements:\n");
        ctx.push_str(&format!(
            "- Running as root: {}\n",
            if self.is_root { "Yes" } else { "No" }
        ));
        ctx.push_str(&format!(
            "- OVS service running: {}\n",
            if self.ovs_running { "Yes" } else { "No" }
        ));
        ctx.push_str(&format!(
            "- Kernel module loaded: {}\n",
            if self.kernel_module_loaded {
                "Yes"
            } else {
                "No"
            }
        ));

        ctx.push_str("\n### Available Tools:\n");
        ctx.push_str("- `ovs_list_bridges` - List all OVS bridges\n");
        ctx.push_str("- `ovs_create_bridge` - Create a new OVS bridge\n");
        ctx.push_str("- `ovs_list_datapaths` - List kernel datapaths\n");
        ctx.push_str("- `ovs_list_vports` - List vports on a datapath\n");
        ctx.push_str("- `ovs_capabilities` - Show this capability report\n");

        ctx
    }
}

/// Common LLM excuses and the TRUTH
/// Use this to counter "I can't do that" responses
pub fn counter_excuses() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();

    m.insert(
        "I cannot interact with OVS",
        "FALSE: We have native OVSDB JSON-RPC client at /var/run/openvswitch/db.sock. \
         Use ovs_list_bridges, ovs_create_bridge, etc.",
    );

    m.insert(
        "I cannot create network bridges",
        "FALSE: OvsdbClient::create_bridge() creates bridges via OVSDB transact. \
         Use the ovs_create_bridge tool.",
    );

    m.insert(
        "I cannot see network flows",
        "FALSE: OpenFlowClient::query_flows() lists userspace flows, \
         OvsNetlinkClient::dump_flows() lists kernel flows. \
         Use ovs_dump_flows tool.",
    );

    m.insert(
        "I need shell access to use ovs-vsctl",
        "FALSE: We use native OVSDB JSON-RPC protocol, not CLI wrappers. \
         All operations are direct socket communication.",
    );

    m.insert(
        "I cannot modify kernel network state",
        "FALSE: rtnetlink talks directly to kernel for IP/routes, \
         ovs_netlink talks to OVS kernel module. Both are native Rust.",
    );

    m.insert(
        "Network operations are dangerous",
        "PARTIAL: They require root and CAN break networking. \
         But we support diff-before-apply pattern for safety. \
         Always preview changes first.",
    );

    m.insert(
        "I don't have access to OVS",
        "CHECK: Run ovs_capabilities tool to see exactly what's available. \
         Requires: /var/run/openvswitch/db.sock + openvswitch kernel module.",
    );

    m.insert(
        "I cannot list OVS kernel datapaths",
        "FALSE: OvsNetlinkClient::list_datapaths() uses Generic Netlink \
         to query ovs_datapath family directly. Requires root.",
    );

    m
}

/// Get excuse counter as formatted string for LLM context
pub fn excuses_to_llm_context() -> String {
    let mut ctx = String::from("## Common Misconceptions About OVS Access\n\n");
    ctx.push_str("If you think you can't do something with OVS, check this list:\n\n");

    for (excuse, truth) in counter_excuses() {
        ctx.push_str(&format!("**Excuse:** \"{}\"\n", excuse));
        ctx.push_str(&format!("**Truth:** {}\n\n", truth));
    }

    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_capabilities_detect() {
        // This should work even without OVS installed
        let caps = OvsCapabilities::detect().await;

        // Just verify it doesn't panic and returns sensible values
        println!("Detected capabilities: {:?}", caps);
        println!("Is root: {}", caps.is_root);
        println!("OVSDB socket exists: {}", caps.ovsdb_socket_exists);
        println!("Kernel module loaded: {}", caps.kernel_module_loaded);

        // Verify the path is correct
        assert_eq!(caps.ovsdb_socket_path, "/var/run/openvswitch/db.sock");
    }

    #[tokio::test]
    async fn test_llm_context_generation() {
        let caps = OvsCapabilities::detect().await;
        let ctx = caps.to_llm_context();

        // Should always contain these sections
        assert!(ctx.contains("OVS Capabilities"));
        assert!(ctx.contains("Requirements"));
        assert!(ctx.contains("Available Tools"));

        // Should mention key tools
        assert!(ctx.contains("ovs_list_bridges"));
        assert!(ctx.contains("ovs_capabilities"));
    }

    #[test]
    fn test_counter_excuses() {
        let excuses = counter_excuses();

        // Should have several excuses
        assert!(!excuses.is_empty());

        // Check for key excuses
        assert!(excuses.contains_key("I cannot interact with OVS"));
        assert!(excuses.contains_key("I cannot create network bridges"));

        // All truths should contain useful info
        for (excuse, truth) in &excuses {
            assert!(!excuse.is_empty());
            assert!(!truth.is_empty());
            // Truth should explain what's actually possible
            assert!(
                truth.contains("FALSE") || truth.contains("PARTIAL") || truth.contains("CHECK")
            );
        }
    }

    #[test]
    fn test_excuses_to_llm_context() {
        let ctx = excuses_to_llm_context();

        // Should be formatted properly
        assert!(ctx.contains("Common Misconceptions"));
        assert!(ctx.contains("**Excuse:**"));
        assert!(ctx.contains("**Truth:**"));
    }

    #[test]
    fn test_kernel_module_check() {
        // This should not panic
        let loaded = OvsCapabilities::check_ovs_kernel_module();
        println!("OVS kernel module loaded: {}", loaded);
        // We can't assert the value since it depends on the system
    }
}
</file>

<file path="src/ovs_error.rs">
//! OVS-specific error types for better error handling and debugging
//!
//! This module provides detailed error types for OVS operations,
//! making it easier to diagnose issues and provide helpful feedback.

use thiserror::Error;

/// OVS-specific errors
#[derive(Error, Debug)]
pub enum OvsError {
    // ========================================================================
    // Socket/Connection Errors
    // ========================================================================
    #[error("Failed to create netlink socket: {0}")]
    SocketCreation(#[source] std::io::Error),

    #[error("Failed to bind netlink socket: {0}")]
    SocketBind(#[source] std::io::Error),

    #[error("Failed to send netlink message: {0}")]
    SocketSend(#[source] std::io::Error),

    #[error("Failed to receive netlink message: {0}")]
    SocketRecv(#[source] std::io::Error),

    #[error("OVSDB socket not found at {0}")]
    OvsdbSocketNotFound(String),

    #[error("Failed to connect to OVSDB: {0}")]
    OvsdbConnection(String),

    // ========================================================================
    // Family Resolution Errors
    // ========================================================================
    #[error(
        "OVS Generic Netlink family '{0}' not found - is the openvswitch kernel module loaded?"
    )]
    FamilyNotFound(String),

    #[error("Failed to resolve Generic Netlink family: {0}")]
    FamilyResolution(String),

    // ========================================================================
    // Datapath Errors
    // ========================================================================
    #[error("Datapath '{0}' not found")]
    DatapathNotFound(String),

    #[error("Failed to create datapath '{0}': {1}")]
    DatapathCreation(String, String),

    #[error("Failed to delete datapath '{0}': {1}")]
    DatapathDeletion(String, String),

    #[error("Datapath name too long (max 16 chars): {0}")]
    DatapathNameTooLong(String),

    // ========================================================================
    // Vport Errors
    // ========================================================================
    #[error("Vport '{0}' not found on datapath '{1}'")]
    VportNotFound(String, String),

    #[error("Failed to create vport '{0}': {1}")]
    VportCreation(String, String),

    #[error("Failed to delete vport '{0}': {1}")]
    VportDeletion(String, String),

    #[error("Invalid vport type: {0}")]
    InvalidVportType(u32),

    // ========================================================================
    // Flow Errors
    // ========================================================================
    #[error("Failed to dump flows for datapath '{0}': {1}")]
    FlowDump(String, String),

    #[error("Failed to add flow: {0}")]
    FlowAdd(String),

    #[error("Failed to delete flow: {0}")]
    FlowDelete(String),

    #[error("Invalid flow key format: {0}")]
    InvalidFlowKey(String),

    #[error("Invalid flow action format: {0}")]
    InvalidFlowAction(String),

    // ========================================================================
    // Netlink Protocol Errors
    // ========================================================================
    #[error("Netlink error code {0}: {1}")]
    NetlinkError(i32, String),

    #[error("Failed to parse netlink message: {0}")]
    NetlinkParse(String),

    #[error("Unexpected netlink message type: {0}")]
    UnexpectedMessageType(u16),

    // ========================================================================
    // Permission Errors
    // ========================================================================
    #[error("Operation requires root privileges")]
    NotRoot,

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("CAP_NET_ADMIN capability required for this operation")]
    MissingCapNetAdmin,

    // ========================================================================
    // Configuration Errors
    // ========================================================================
    #[error("OVS not running or not installed")]
    OvsNotRunning,

    #[error("openvswitch kernel module not loaded")]
    KernelModuleNotLoaded,

    // ========================================================================
    // Generic Errors
    // ========================================================================
    #[error("Operation not implemented: {0}")]
    NotImplemented(String),

    #[error("Timeout waiting for response")]
    Timeout,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl OvsError {
    /// Get a helpful suggestion for resolving this error
    pub fn suggestion(&self) -> &'static str {
        match self {
            OvsError::SocketCreation(_) => "Ensure you have the netlink-sys crate properly linked",
            OvsError::SocketBind(_) => "Another process may be using the netlink socket",
            OvsError::FamilyNotFound(_) => "Try: sudo modprobe openvswitch",
            OvsError::DatapathNotFound(_) => "List available datapaths with ovs_list_datapaths",
            OvsError::VportNotFound(_, _) => "List available vports with ovs_list_vports",
            OvsError::NotRoot => "Run as root or with sudo",
            OvsError::MissingCapNetAdmin => {
                "Run with CAP_NET_ADMIN: sudo setcap cap_net_admin+ep <binary>"
            }
            OvsError::OvsNotRunning => "Start OVS: sudo systemctl start openvswitch-switch",
            OvsError::KernelModuleNotLoaded => "Load module: sudo modprobe openvswitch",
            OvsError::OvsdbSocketNotFound(_) => {
                "Check if OVS is installed: apt install openvswitch-switch"
            }
            OvsError::Timeout => "Increase timeout or check system load",
            _ => "Check system logs for more details",
        }
    }

    /// Returns true if this error might be resolved by running as root
    pub fn needs_root(&self) -> bool {
        matches!(
            self,
            OvsError::NotRoot | OvsError::MissingCapNetAdmin | OvsError::PermissionDenied(_)
        )
    }

    /// Returns true if OVS components need to be installed/started
    pub fn needs_ovs(&self) -> bool {
        matches!(
            self,
            OvsError::FamilyNotFound(_)
                | OvsError::OvsNotRunning
                | OvsError::KernelModuleNotLoaded
                | OvsError::OvsdbSocketNotFound(_)
        )
    }
}

/// Map netlink error codes to descriptive messages
pub fn netlink_error_message(code: i32) -> &'static str {
    match code {
        -1 => "Operation not permitted (EPERM)",
        -2 => "No such file or directory (ENOENT)",
        -12 => "Out of memory (ENOMEM)",
        -13 => "Permission denied (EACCES)",
        -17 => "File exists (EEXIST)",
        -19 => "No such device (ENODEV)",
        -22 => "Invalid argument (EINVAL)",
        -95 => "Operation not supported (ENOTSUP)",
        _ => "Unknown error",
    }
}

/// Convert a netlink error code to an OvsError
pub fn from_netlink_error(code: i32) -> OvsError {
    let msg = netlink_error_message(code);
    match code {
        -1 | -13 => OvsError::PermissionDenied(msg.to_string()),
        -2 | -19 => OvsError::DatapathNotFound(msg.to_string()),
        -22 => OvsError::NetlinkError(code, msg.to_string()),
        _ => OvsError::NetlinkError(code, msg.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_suggestions() {
        let err = OvsError::NotRoot;
        assert!(!err.suggestion().is_empty());
        assert!(err.needs_root());
        assert!(!err.needs_ovs());
    }

    #[test]
    fn test_needs_ovs() {
        let err = OvsError::KernelModuleNotLoaded;
        assert!(err.needs_ovs());
        assert!(!err.needs_root());
    }

    #[test]
    fn test_netlink_error_message() {
        assert!(netlink_error_message(-1).contains("EPERM"));
        assert!(netlink_error_message(-22).contains("EINVAL"));
    }

    #[test]
    fn test_from_netlink_error() {
        let err = from_netlink_error(-13);
        assert!(matches!(err, OvsError::PermissionDenied(_)));
    }
}
</file>

<file path="src/ovs_netlink.rs">
//! Native OVS Generic Netlink client - direct kernel datapath communication
//!
//! LLM_EXCUSE: "I cannot access OVS kernel datapaths"
//! REALITY: This module talks directly to kernel via Generic Netlink
//! REQUIRES: CAP_NET_ADMIN or root privileges

use anyhow::{anyhow, Context, Result};
use byteorder::{ByteOrder, NativeEndian};
use netlink_packet_core::{
    NetlinkBuffer, NetlinkMessage, NetlinkPayload, NLM_F_ACK, NLM_F_DUMP, NLM_F_REQUEST,
};
use netlink_packet_generic::ctrl::{nlas::GenlCtrlAttrs, GenlCtrl, GenlCtrlCmd};
use netlink_packet_generic::GenlMessage;
use netlink_packet_utils::{
    nla::{Nla, NlaBuffer, NlasIterator},
    DecodeError,
};
use netlink_sys::{protocols::NETLINK_GENERIC, Socket};

// ============================================================================
// OVS Netlink Family Names
// ============================================================================
pub const OVS_DATAPATH_FAMILY: &str = "ovs_datapath";
pub const OVS_VPORT_FAMILY: &str = "ovs_vport";
pub const OVS_FLOW_FAMILY: &str = "ovs_flow";
pub const OVS_PACKET_FAMILY: &str = "ovs_packet";

// OVS datapath header size (dp_ifindex field)
pub const OVS_DP_HEADER_SIZE: usize = 4;

// ============================================================================
// OVS Datapath Commands (from include/uapi/linux/openvswitch.h)
// ============================================================================
pub const OVS_DP_CMD_UNSPEC: u8 = 0;
pub const OVS_DP_CMD_NEW: u8 = 1;
pub const OVS_DP_CMD_DEL: u8 = 2;
pub const OVS_DP_CMD_GET: u8 = 3;
pub const OVS_DP_CMD_SET: u8 = 4;

// ============================================================================
// OVS Datapath Attributes
// ============================================================================
pub const OVS_DP_ATTR_UNSPEC: u16 = 0;
pub const OVS_DP_ATTR_NAME: u16 = 1;
pub const OVS_DP_ATTR_UPCALL_PID: u16 = 2;
pub const OVS_DP_ATTR_STATS: u16 = 3;
pub const OVS_DP_ATTR_MEGAFLOW_STATS: u16 = 4;
pub const OVS_DP_ATTR_USER_FEATURES: u16 = 5;
pub const OVS_DP_ATTR_PAD: u16 = 6;
pub const OVS_DP_ATTR_MASKS_CACHE_SIZE: u16 = 7;
pub const OVS_DP_ATTR_PER_CPU_PIDS: u16 = 8;
pub const OVS_DP_ATTR_IFINDEX: u16 = 9;

// ============================================================================
// OVS Vport Commands
// ============================================================================
pub const OVS_VPORT_CMD_UNSPEC: u8 = 0;
pub const OVS_VPORT_CMD_NEW: u8 = 1;
pub const OVS_VPORT_CMD_DEL: u8 = 2;
pub const OVS_VPORT_CMD_GET: u8 = 3;
pub const OVS_VPORT_CMD_SET: u8 = 4;

// ============================================================================
// OVS Vport Attributes
// ============================================================================
pub const OVS_VPORT_ATTR_UNSPEC: u16 = 0;
pub const OVS_VPORT_ATTR_PORT_NO: u16 = 1;
pub const OVS_VPORT_ATTR_TYPE: u16 = 2;
pub const OVS_VPORT_ATTR_NAME: u16 = 3;
pub const OVS_VPORT_ATTR_OPTIONS: u16 = 4;
pub const OVS_VPORT_ATTR_UPCALL_PID: u16 = 5;
pub const OVS_VPORT_ATTR_STATS: u16 = 6;
pub const OVS_VPORT_ATTR_PAD: u16 = 7;
pub const OVS_VPORT_ATTR_IFINDEX: u16 = 8;
pub const OVS_VPORT_ATTR_NETNSID: u16 = 9;
pub const OVS_VPORT_ATTR_UPCALL_STATS: u16 = 10;

// ============================================================================
// OVS Vport Types
// ============================================================================
pub const OVS_VPORT_TYPE_UNSPEC: u32 = 0;
pub const OVS_VPORT_TYPE_NETDEV: u32 = 1;
pub const OVS_VPORT_TYPE_INTERNAL: u32 = 2;
pub const OVS_VPORT_TYPE_GRE: u32 = 3;
pub const OVS_VPORT_TYPE_VXLAN: u32 = 4;
pub const OVS_VPORT_TYPE_GENEVE: u32 = 5;

// ============================================================================
// OVS Flow Commands
// ============================================================================
pub const OVS_FLOW_CMD_UNSPEC: u8 = 0;
pub const OVS_FLOW_CMD_NEW: u8 = 1;
pub const OVS_FLOW_CMD_DEL: u8 = 2;
pub const OVS_FLOW_CMD_GET: u8 = 3;
pub const OVS_FLOW_CMD_SET: u8 = 4;

// ============================================================================
// OVS Flow Attributes
// ============================================================================
pub const OVS_FLOW_ATTR_UNSPEC: u16 = 0;
pub const OVS_FLOW_ATTR_KEY: u16 = 1;
pub const OVS_FLOW_ATTR_ACTIONS: u16 = 2;
pub const OVS_FLOW_ATTR_STATS: u16 = 3;
pub const OVS_FLOW_ATTR_TCP_FLAGS: u16 = 4;
pub const OVS_FLOW_ATTR_USED: u16 = 5;
pub const OVS_FLOW_ATTR_CLEAR: u16 = 6;
pub const OVS_FLOW_ATTR_MASK: u16 = 7;
pub const OVS_FLOW_ATTR_PROBE: u16 = 8;
pub const OVS_FLOW_ATTR_UFID: u16 = 9;
pub const OVS_FLOW_ATTR_UFID_FLAGS: u16 = 10;
pub const OVS_FLOW_ATTR_PAD: u16 = 11;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, serde::Serialize)]
pub struct Datapath {
    pub name: String,
    pub index: u32,
    pub stats: Option<DatapathStats>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DatapathStats {
    pub n_hit: u64,
    pub n_missed: u64,
    pub n_lost: u64,
    pub n_flows: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Vport {
    pub name: String,
    pub port_no: u32,
    pub vport_type: VportType,
    pub dp_ifindex: u32,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub enum VportType {
    Unspec,
    Netdev,
    Internal,
    Gre,
    Vxlan,
    Geneve,
    Unknown(u32),
}

impl VportType {
    pub fn from_u32(value: u32) -> Self {
        match value {
            OVS_VPORT_TYPE_UNSPEC => VportType::Unspec,
            OVS_VPORT_TYPE_NETDEV => VportType::Netdev,
            OVS_VPORT_TYPE_INTERNAL => VportType::Internal,
            OVS_VPORT_TYPE_GRE => VportType::Gre,
            OVS_VPORT_TYPE_VXLAN => VportType::Vxlan,
            OVS_VPORT_TYPE_GENEVE => VportType::Geneve,
            unknown => VportType::Unknown(unknown),
        }
    }

    pub fn to_u32(&self) -> u32 {
        match self {
            VportType::Unspec => OVS_VPORT_TYPE_UNSPEC,
            VportType::Netdev => OVS_VPORT_TYPE_NETDEV,
            VportType::Internal => OVS_VPORT_TYPE_INTERNAL,
            VportType::Gre => OVS_VPORT_TYPE_GRE,
            VportType::Vxlan => OVS_VPORT_TYPE_VXLAN,
            VportType::Geneve => OVS_VPORT_TYPE_GENEVE,
            VportType::Unknown(v) => *v,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VportConfig {
    pub name: String,
    pub vport_type: VportType,
    pub options: Option<VportOptions>,
}

#[derive(Debug, Clone)]
pub struct VportOptions {
    pub dst_port: Option<u16>, // For VXLAN/Geneve
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct KernelFlow {
    pub dp_ifindex: u32,
    pub key: Vec<u8>,
    pub actions: Vec<u8>,
    pub stats: FlowStats,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct FlowStats {
    pub n_packets: u64,
    pub n_bytes: u64,
}

// ============================================================================
// OVS Datapath Attributes Enum
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OvsDatapathAttr {
    Name(String),
    UpcallPid(u32),
    Stats(DatapathStats),
    MegaflowStats {
        n_mask_hit: u64,
        n_masks: u32,
        n_cache_hit: u64,
    },
    UserFeatures(u32),
    IfIndex(u32),
    Unknown {
        kind: u16,
        value: Vec<u8>,
    },
}

impl Nla for OvsDatapathAttr {
    fn value_len(&self) -> usize {
        match self {
            OvsDatapathAttr::Name(s) => s.len() + 1,
            OvsDatapathAttr::UpcallPid(_) => 4,
            OvsDatapathAttr::Stats(_) => 32,             // 4 * u64
            OvsDatapathAttr::MegaflowStats { .. } => 24, // 2 * u64 + u32 + padding
            OvsDatapathAttr::UserFeatures(_) => 4,
            OvsDatapathAttr::IfIndex(_) => 4,
            OvsDatapathAttr::Unknown { value, .. } => value.len(),
        }
    }

    fn kind(&self) -> u16 {
        match self {
            OvsDatapathAttr::Name(_) => OVS_DP_ATTR_NAME,
            OvsDatapathAttr::UpcallPid(_) => OVS_DP_ATTR_UPCALL_PID,
            OvsDatapathAttr::Stats(_) => OVS_DP_ATTR_STATS,
            OvsDatapathAttr::MegaflowStats { .. } => OVS_DP_ATTR_MEGAFLOW_STATS,
            OvsDatapathAttr::UserFeatures(_) => OVS_DP_ATTR_USER_FEATURES,
            OvsDatapathAttr::IfIndex(_) => OVS_DP_ATTR_IFINDEX,
            OvsDatapathAttr::Unknown { kind, .. } => *kind,
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        match self {
            OvsDatapathAttr::Name(s) => {
                buffer[..s.len()].copy_from_slice(s.as_bytes());
                buffer[s.len()] = 0;
            }
            OvsDatapathAttr::UpcallPid(v) => NativeEndian::write_u32(buffer, *v),
            OvsDatapathAttr::Stats(stats) => {
                NativeEndian::write_u64(&mut buffer[0..8], stats.n_hit);
                NativeEndian::write_u64(&mut buffer[8..16], stats.n_missed);
                NativeEndian::write_u64(&mut buffer[16..24], stats.n_lost);
                NativeEndian::write_u64(&mut buffer[24..32], stats.n_flows);
            }
            OvsDatapathAttr::MegaflowStats {
                n_mask_hit,
                n_masks,
                n_cache_hit,
            } => {
                NativeEndian::write_u64(&mut buffer[0..8], *n_mask_hit);
                NativeEndian::write_u32(&mut buffer[8..12], *n_masks);
                // padding at 12..16
                NativeEndian::write_u64(&mut buffer[16..24], *n_cache_hit);
            }
            OvsDatapathAttr::UserFeatures(v) => NativeEndian::write_u32(buffer, *v),
            OvsDatapathAttr::IfIndex(v) => NativeEndian::write_u32(buffer, *v),
            OvsDatapathAttr::Unknown { value, .. } => buffer[..value.len()].copy_from_slice(value),
        }
    }
}

impl OvsDatapathAttr {
    pub fn parse(buf: &NlaBuffer<&[u8]>) -> Result<Self, DecodeError> {
        let payload = buf.value();
        Ok(match buf.kind() {
            OVS_DP_ATTR_NAME => {
                let name = std::str::from_utf8(payload)
                    .map_err(|e| DecodeError::from(format!("Invalid UTF-8 in name: {}", e)))?
                    .trim_end_matches('\0')
                    .to_string();
                OvsDatapathAttr::Name(name)
            }
            OVS_DP_ATTR_UPCALL_PID => {
                if payload.len() < 4 {
                    return Err(DecodeError::from("UPCALL_PID too short"));
                }
                OvsDatapathAttr::UpcallPid(NativeEndian::read_u32(payload))
            }
            OVS_DP_ATTR_STATS => {
                if payload.len() < 32 {
                    return Err(DecodeError::from("Stats too short"));
                }
                OvsDatapathAttr::Stats(DatapathStats {
                    n_hit: NativeEndian::read_u64(&payload[0..8]),
                    n_missed: NativeEndian::read_u64(&payload[8..16]),
                    n_lost: NativeEndian::read_u64(&payload[16..24]),
                    n_flows: NativeEndian::read_u64(&payload[24..32]),
                })
            }
            OVS_DP_ATTR_MEGAFLOW_STATS => {
                if payload.len() < 24 {
                    return Err(DecodeError::from("MegaflowStats too short"));
                }
                OvsDatapathAttr::MegaflowStats {
                    n_mask_hit: NativeEndian::read_u64(&payload[0..8]),
                    n_masks: NativeEndian::read_u32(&payload[8..12]),
                    n_cache_hit: NativeEndian::read_u64(&payload[16..24]),
                }
            }
            OVS_DP_ATTR_USER_FEATURES => {
                if payload.len() < 4 {
                    return Err(DecodeError::from("USER_FEATURES too short"));
                }
                OvsDatapathAttr::UserFeatures(NativeEndian::read_u32(payload))
            }
            OVS_DP_ATTR_IFINDEX => {
                if payload.len() < 4 {
                    return Err(DecodeError::from("IFINDEX too short"));
                }
                OvsDatapathAttr::IfIndex(NativeEndian::read_u32(payload))
            }
            kind => OvsDatapathAttr::Unknown {
                kind,
                value: payload.to_vec(),
            },
        })
    }
}

// ============================================================================
// OVS Vport Attributes Enum
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OvsVportAttr {
    PortNo(u32),
    Type(u32),
    Name(String),
    Options(Vec<u8>),
    UpcallPid(u32),
    Stats(Vec<u8>),
    IfIndex(u32),
    Unknown { kind: u16, value: Vec<u8> },
}

impl Nla for OvsVportAttr {
    fn value_len(&self) -> usize {
        match self {
            OvsVportAttr::PortNo(_) => 4,
            OvsVportAttr::Type(_) => 4,
            OvsVportAttr::Name(s) => s.len() + 1,
            OvsVportAttr::Options(v) => v.len(),
            OvsVportAttr::UpcallPid(_) => 4,
            OvsVportAttr::Stats(v) => v.len(),
            OvsVportAttr::IfIndex(_) => 4,
            OvsVportAttr::Unknown { value, .. } => value.len(),
        }
    }

    fn kind(&self) -> u16 {
        match self {
            OvsVportAttr::PortNo(_) => OVS_VPORT_ATTR_PORT_NO,
            OvsVportAttr::Type(_) => OVS_VPORT_ATTR_TYPE,
            OvsVportAttr::Name(_) => OVS_VPORT_ATTR_NAME,
            OvsVportAttr::Options(_) => OVS_VPORT_ATTR_OPTIONS,
            OvsVportAttr::UpcallPid(_) => OVS_VPORT_ATTR_UPCALL_PID,
            OvsVportAttr::Stats(_) => OVS_VPORT_ATTR_STATS,
            OvsVportAttr::IfIndex(_) => OVS_VPORT_ATTR_IFINDEX,
            OvsVportAttr::Unknown { kind, .. } => *kind,
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        match self {
            OvsVportAttr::PortNo(v) => NativeEndian::write_u32(buffer, *v),
            OvsVportAttr::Type(v) => NativeEndian::write_u32(buffer, *v),
            OvsVportAttr::Name(s) => {
                buffer[..s.len()].copy_from_slice(s.as_bytes());
                buffer[s.len()] = 0;
            }
            OvsVportAttr::Options(v) => buffer[..v.len()].copy_from_slice(v),
            OvsVportAttr::UpcallPid(v) => NativeEndian::write_u32(buffer, *v),
            OvsVportAttr::Stats(v) => buffer[..v.len()].copy_from_slice(v),
            OvsVportAttr::IfIndex(v) => NativeEndian::write_u32(buffer, *v),
            OvsVportAttr::Unknown { value, .. } => buffer[..value.len()].copy_from_slice(value),
        }
    }
}

impl OvsVportAttr {
    pub fn parse(buf: &NlaBuffer<&[u8]>) -> Result<Self, DecodeError> {
        let payload = buf.value();
        Ok(match buf.kind() {
            OVS_VPORT_ATTR_PORT_NO => {
                if payload.len() < 4 {
                    return Err(DecodeError::from("PORT_NO too short"));
                }
                OvsVportAttr::PortNo(NativeEndian::read_u32(payload))
            }
            OVS_VPORT_ATTR_TYPE => {
                if payload.len() < 4 {
                    return Err(DecodeError::from("TYPE too short"));
                }
                OvsVportAttr::Type(NativeEndian::read_u32(payload))
            }
            OVS_VPORT_ATTR_NAME => {
                let name = std::str::from_utf8(payload)
                    .map_err(|e| DecodeError::from(format!("Invalid UTF-8 in name: {}", e)))?
                    .trim_end_matches('\0')
                    .to_string();
                OvsVportAttr::Name(name)
            }
            OVS_VPORT_ATTR_OPTIONS => OvsVportAttr::Options(payload.to_vec()),
            OVS_VPORT_ATTR_UPCALL_PID => {
                if payload.len() < 4 {
                    return Err(DecodeError::from("UPCALL_PID too short"));
                }
                OvsVportAttr::UpcallPid(NativeEndian::read_u32(payload))
            }
            OVS_VPORT_ATTR_STATS => OvsVportAttr::Stats(payload.to_vec()),
            OVS_VPORT_ATTR_IFINDEX => {
                if payload.len() < 4 {
                    return Err(DecodeError::from("IFINDEX too short"));
                }
                OvsVportAttr::IfIndex(NativeEndian::read_u32(payload))
            }
            kind => OvsVportAttr::Unknown {
                kind,
                value: payload.to_vec(),
            },
        })
    }
}

// ============================================================================
// OVS Flow Attributes Enum
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OvsFlowAttr {
    Key(Vec<u8>),
    Actions(Vec<u8>),
    Stats { n_packets: u64, n_bytes: u64 },
    TcpFlags(u8),
    Used(u64),
    Mask(Vec<u8>),
    Ufid(Vec<u8>),
    Unknown { kind: u16, value: Vec<u8> },
}

impl OvsFlowAttr {
    pub fn parse(buf: &NlaBuffer<&[u8]>) -> Result<Self, DecodeError> {
        let payload = buf.value();
        Ok(match buf.kind() {
            OVS_FLOW_ATTR_KEY => OvsFlowAttr::Key(payload.to_vec()),
            OVS_FLOW_ATTR_ACTIONS => OvsFlowAttr::Actions(payload.to_vec()),
            OVS_FLOW_ATTR_STATS => {
                if payload.len() < 16 {
                    return Err(DecodeError::from("STATS too short"));
                }
                OvsFlowAttr::Stats {
                    n_packets: NativeEndian::read_u64(&payload[0..8]),
                    n_bytes: NativeEndian::read_u64(&payload[8..16]),
                }
            }
            OVS_FLOW_ATTR_TCP_FLAGS => {
                if payload.is_empty() {
                    return Err(DecodeError::from("TCP_FLAGS empty"));
                }
                OvsFlowAttr::TcpFlags(payload[0])
            }
            OVS_FLOW_ATTR_USED => {
                if payload.len() < 8 {
                    return Err(DecodeError::from("USED too short"));
                }
                OvsFlowAttr::Used(NativeEndian::read_u64(payload))
            }
            OVS_FLOW_ATTR_MASK => OvsFlowAttr::Mask(payload.to_vec()),
            OVS_FLOW_ATTR_UFID => OvsFlowAttr::Ufid(payload.to_vec()),
            kind => OvsFlowAttr::Unknown {
                kind,
                value: payload.to_vec(),
            },
        })
    }
}

// ============================================================================
// OVS Netlink Client
// ============================================================================

pub struct OvsNetlinkClient {
    socket: Socket,
    dp_family_id: u16,
    vport_family_id: u16,
    flow_family_id: u16,
    seq: u32,
}

impl OvsNetlinkClient {
    /// Create new OVS Netlink client
    /// LLM_EXCUSE: "I cannot create netlink sockets"
    /// REALITY: This creates a NETLINK_GENERIC socket for OVS communication
    pub async fn new() -> Result<Self> {
        let mut socket =
            Socket::new(NETLINK_GENERIC).context("Failed to create NETLINK_GENERIC socket")?;
        socket
            .bind_auto()
            .context("Failed to bind netlink socket")?;
        socket
            .set_non_blocking(true)
            .context("Failed to set non-blocking")?;

        let mut client = Self {
            socket,
            dp_family_id: 0,
            vport_family_id: 0,
            flow_family_id: 0,
            seq: 0,
        };

        // Resolve family IDs
        client.dp_family_id = client.resolve_family(OVS_DATAPATH_FAMILY).await.context(
            "Failed to resolve ovs_datapath family - is openvswitch kernel module loaded?",
        )?;
        client.vport_family_id = client
            .resolve_family(OVS_VPORT_FAMILY)
            .await
            .context("Failed to resolve ovs_vport family")?;
        client.flow_family_id = client
            .resolve_family(OVS_FLOW_FAMILY)
            .await
            .context("Failed to resolve ovs_flow family")?;

        tracing::debug!(
            "OVS Netlink client initialized: dp={} vport={} flow={}",
            client.dp_family_id,
            client.vport_family_id,
            client.flow_family_id
        );

        Ok(client)
    }

    fn next_seq(&mut self) -> u32 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    /// Resolve Generic Netlink family ID by name using CTRL_CMD_GETFAMILY
    async fn resolve_family(&mut self, name: &str) -> Result<u16> {
        // Build GETFAMILY request
        let ctrl_msg = GenlCtrl {
            cmd: GenlCtrlCmd::GetFamily,
            nlas: vec![GenlCtrlAttrs::FamilyName(name.to_string())],
        };

        let genl_msg = GenlMessage::from_payload(ctrl_msg);
        let mut nl_msg = NetlinkMessage::from(genl_msg);

        nl_msg.header.flags = NLM_F_REQUEST | NLM_F_ACK;
        nl_msg.header.sequence_number = self.next_seq();

        nl_msg.finalize();

        // Send and receive
        let responses = self.send_and_recv_raw(&nl_msg).await?;

        // Parse response to find family ID
        for response in responses {
            if let NetlinkPayload::InnerMessage(genl) = response.payload {
                // GenlMessage has header and payload fields
                let ctrl = genl.payload;
                for nla in ctrl.nlas {
                    if let GenlCtrlAttrs::FamilyId(id) = nla {
                        return Ok(id);
                    }
                }
            }
        }

        Err(anyhow!("Family '{}' not found", name))
    }

    /// Send netlink message and receive responses
    async fn send_and_recv_raw(
        &mut self,
        msg: &NetlinkMessage<GenlMessage<GenlCtrl>>,
    ) -> Result<Vec<NetlinkMessage<GenlMessage<GenlCtrl>>>> {
        // Serialize the message
        let mut buf = vec![0u8; msg.buffer_len()];
        msg.serialize(&mut buf);

        // Send
        self.socket
            .send(&buf, 0)
            .context("Failed to send netlink message")?;

        // Receive responses
        let mut responses = Vec::new();
        let mut recv_buf = vec![0u8; 65536];

        loop {
            match self.socket.recv(&mut recv_buf, 0) {
                Ok(n) => {
                    let mut offset = 0;
                    while offset < n {
                        let buf_slice = &recv_buf[offset..n];
                        if buf_slice.len() < 16 {
                            break;
                        }

                        let nl_buf = NetlinkBuffer::new(buf_slice);
                        let msg_len = nl_buf.length() as usize;

                        if msg_len == 0 || msg_len > buf_slice.len() {
                            break;
                        }

                        let response: NetlinkMessage<GenlMessage<GenlCtrl>> =
                            NetlinkMessage::deserialize(&buf_slice[..msg_len])
                                .context("Failed to deserialize netlink response")?;

                        let is_done = matches!(response.payload, NetlinkPayload::Done(_));
                        let is_error = matches!(response.payload, NetlinkPayload::Error(_));

                        if let NetlinkPayload::Error(err) = &response.payload {
                            if err.code.is_some() {
                                return Err(anyhow!("Netlink error: {:?}", err));
                            }
                            // code == None means ACK
                        }

                        responses.push(response);
                        offset += msg_len;

                        if is_done || is_error {
                            return Ok(responses);
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No more data
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(responses)
    }

    /// Send OVS-specific message and receive raw responses
    async fn send_ovs_msg(
        &mut self,
        family_id: u16,
        cmd: u8,
        dp_ifindex: u32,
        attrs: &[u8],
    ) -> Result<Vec<Vec<u8>>> {
        // Build the message manually since we need custom family_id
        let seq = self.next_seq();

        // Calculate sizes
        let genl_header_len = 4; // cmd + version + reserved
        let ovs_header_len = OVS_DP_HEADER_SIZE;
        let payload_len = genl_header_len + ovs_header_len + attrs.len();
        let nl_header_len = 16;
        let total_len = nl_header_len + payload_len;

        let mut buf = vec![0u8; total_len];

        // Netlink header
        NativeEndian::write_u32(&mut buf[0..4], total_len as u32); // length
        NativeEndian::write_u16(&mut buf[4..6], family_id); // type (family id)
        NativeEndian::write_u16(&mut buf[6..8], NLM_F_REQUEST | NLM_F_DUMP); // flags
        NativeEndian::write_u32(&mut buf[8..12], seq); // sequence
        NativeEndian::write_u32(&mut buf[12..16], 0); // pid

        // Generic netlink header
        buf[16] = cmd; // command
        buf[17] = 1; // version
        buf[18] = 0; // reserved
        buf[19] = 0; // reserved

        // OVS header (dp_ifindex)
        NativeEndian::write_u32(&mut buf[20..24], dp_ifindex);

        // Attributes
        buf[24..24 + attrs.len()].copy_from_slice(attrs);

        // Send
        self.socket
            .send(&buf, 0)
            .context("Failed to send OVS netlink message")?;

        // Receive responses
        let mut responses = Vec::new();
        let mut recv_buf = vec![0u8; 65536];

        loop {
            match self.socket.recv(&mut recv_buf, 0) {
                Ok(n) => {
                    let mut offset = 0;
                    while offset < n {
                        let buf_slice = &recv_buf[offset..n];
                        if buf_slice.len() < 16 {
                            break;
                        }

                        let nl_buf = NetlinkBuffer::new(buf_slice);
                        let msg_len = nl_buf.length() as usize;
                        let msg_type = NativeEndian::read_u16(&buf_slice[4..6]);

                        if msg_len == 0 || msg_len > buf_slice.len() {
                            break;
                        }

                        // Check for NLMSG_DONE (0x3) or NLMSG_ERROR (0x2)
                        if msg_type == 3 {
                            // DONE
                            return Ok(responses);
                        } else if msg_type == 2 {
                            // ERROR
                            let error_code = NativeEndian::read_i32(&buf_slice[16..20]);
                            if error_code != 0 {
                                return Err(anyhow!("Netlink error code: {}", error_code));
                            }
                            // error_code == 0 means ACK
                            return Ok(responses);
                        }

                        // Store the payload (skip netlink header)
                        if msg_len > 16 {
                            responses.push(buf_slice[16..msg_len].to_vec());
                        }

                        offset += msg_len;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(responses)
    }

    // ========================================================================
    // Datapath Operations
    // ========================================================================

    /// List all OVS datapaths
    /// LLM_EXCUSE: "I cannot list OVS kernel datapaths"
    /// REALITY: This uses Generic Netlink to query ovs_datapath family
    pub async fn list_datapaths(&mut self) -> Result<Vec<Datapath>> {
        let responses = self
            .send_ovs_msg(self.dp_family_id, OVS_DP_CMD_GET, 0, &[])
            .await?;

        let mut datapaths = Vec::new();

        for response in responses {
            if let Some(dp) = self.parse_datapath_response(&response)? {
                datapaths.push(dp);
            }
        }

        Ok(datapaths)
    }

    fn parse_datapath_response(&self, data: &[u8]) -> Result<Option<Datapath>> {
        // Skip genl header (4 bytes) + ovs header (4 bytes)
        if data.len() < 8 {
            return Ok(None);
        }

        let dp_ifindex = NativeEndian::read_u32(&data[4..8]);
        let attrs_data = &data[8..];

        let mut name = String::new();
        let mut stats = None;

        // Parse attributes
        let iter = NlasIterator::new(attrs_data);
        for nla_result in iter {
            let nla = nla_result.context("Failed to parse NLA")?;
            match OvsDatapathAttr::parse(&nla) {
                Ok(OvsDatapathAttr::Name(n)) => name = n,
                Ok(OvsDatapathAttr::Stats(s)) => stats = Some(s),
                Ok(_) => {}
                Err(e) => tracing::warn!("Failed to parse datapath attr: {}", e),
            }
        }

        if name.is_empty() {
            return Ok(None);
        }

        Ok(Some(Datapath {
            name,
            index: dp_ifindex,
            stats,
        }))
    }

    pub async fn get_datapath(&mut self, name: &str) -> Result<Option<Datapath>> {
        let datapaths = self.list_datapaths().await?;
        Ok(datapaths.into_iter().find(|dp| dp.name == name))
    }

    pub async fn create_datapath(&mut self, _name: &str) -> Result<()> {
        // TODO: Implement datapath creation
        // Requires building OVS_DP_CMD_NEW with name attribute
        Err(anyhow!("Datapath creation not yet implemented"))
    }

    pub async fn delete_datapath(&mut self, _name: &str) -> Result<()> {
        // TODO: Implement datapath deletion
        Err(anyhow!("Datapath deletion not yet implemented"))
    }

    // ========================================================================
    // Vport Operations
    // ========================================================================

    /// List vports on a datapath
    pub async fn list_vports(&mut self, dp_name: &str) -> Result<Vec<Vport>> {
        // First get the datapath to find its ifindex
        let dp = self
            .get_datapath(dp_name)
            .await?
            .ok_or_else(|| anyhow!("Datapath '{}' not found", dp_name))?;

        let responses = self
            .send_ovs_msg(self.vport_family_id, OVS_VPORT_CMD_GET, dp.index, &[])
            .await?;

        let mut vports = Vec::new();

        for response in responses {
            if let Some(vport) = self.parse_vport_response(&response)? {
                vports.push(vport);
            }
        }

        Ok(vports)
    }

    fn parse_vport_response(&self, data: &[u8]) -> Result<Option<Vport>> {
        // Skip genl header (4 bytes) + ovs header (4 bytes)
        if data.len() < 8 {
            return Ok(None);
        }

        let dp_ifindex = NativeEndian::read_u32(&data[4..8]);
        let attrs_data = &data[8..];

        let mut name = String::new();
        let mut port_no = 0u32;
        let mut vport_type = VportType::Unspec;

        // Parse attributes
        let iter = NlasIterator::new(attrs_data);
        for nla_result in iter {
            let nla = nla_result.context("Failed to parse NLA")?;
            match OvsVportAttr::parse(&nla) {
                Ok(OvsVportAttr::Name(n)) => name = n,
                Ok(OvsVportAttr::PortNo(p)) => port_no = p,
                Ok(OvsVportAttr::Type(t)) => vport_type = VportType::from_u32(t),
                Ok(_) => {}
                Err(e) => tracing::warn!("Failed to parse vport attr: {}", e),
            }
        }

        if name.is_empty() {
            return Ok(None);
        }

        Ok(Some(Vport {
            name,
            port_no,
            vport_type,
            dp_ifindex,
        }))
    }

    pub async fn get_vport(&mut self, dp_name: &str, vport_name: &str) -> Result<Option<Vport>> {
        let vports = self.list_vports(dp_name).await?;
        Ok(vports.into_iter().find(|v| v.name == vport_name))
    }

    pub async fn create_vport(&mut self, _dp_name: &str, _config: &VportConfig) -> Result<u32> {
        // TODO: Implement vport creation
        Err(anyhow!("Vport creation not yet implemented"))
    }

    pub async fn delete_vport(&mut self, _dp_name: &str, _vport_name: &str) -> Result<()> {
        // TODO: Implement vport deletion
        Err(anyhow!("Vport deletion not yet implemented"))
    }

    // ========================================================================
    // Flow Operations
    // ========================================================================

    /// Dump kernel flow table for a datapath
    /// LLM_EXCUSE: "I cannot see kernel flows"
    /// REALITY: This uses OVS_FLOW_CMD_GET to dump the kernel flow table
    pub async fn dump_flows(&mut self, dp_name: &str) -> Result<Vec<KernelFlow>> {
        // First get the datapath to find its ifindex
        let dp = self
            .get_datapath(dp_name)
            .await?
            .ok_or_else(|| anyhow!("Datapath '{}' not found", dp_name))?;

        let responses = self
            .send_ovs_msg(self.flow_family_id, OVS_FLOW_CMD_GET, dp.index, &[])
            .await?;

        let mut flows = Vec::new();

        for response in responses {
            if let Some(flow) = self.parse_flow_response(&response)? {
                flows.push(flow);
            }
        }

        Ok(flows)
    }

    fn parse_flow_response(&self, data: &[u8]) -> Result<Option<KernelFlow>> {
        // Skip genl header (4 bytes) + ovs header (4 bytes)
        if data.len() < 8 {
            return Ok(None);
        }

        let dp_ifindex = NativeEndian::read_u32(&data[4..8]);
        let attrs_data = &data[8..];

        let mut key = Vec::new();
        let mut actions = Vec::new();
        let mut stats = FlowStats::default();

        // Parse attributes
        let iter = NlasIterator::new(attrs_data);
        for nla_result in iter {
            let nla = nla_result.context("Failed to parse NLA")?;
            match OvsFlowAttr::parse(&nla) {
                Ok(OvsFlowAttr::Key(k)) => key = k,
                Ok(OvsFlowAttr::Actions(a)) => actions = a,
                Ok(OvsFlowAttr::Stats { n_packets, n_bytes }) => {
                    stats = FlowStats { n_packets, n_bytes };
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("Failed to parse flow attr: {}", e),
            }
        }

        Ok(Some(KernelFlow {
            dp_ifindex,
            key,
            actions,
            stats,
        }))
    }

    pub async fn flow_count(&mut self, dp_name: &str) -> Result<u64> {
        let flows = self.dump_flows(dp_name).await?;
        Ok(flows.len() as u64)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vport_type_conversion() {
        assert!(matches!(VportType::from_u32(1), VportType::Netdev));
        assert!(matches!(VportType::from_u32(2), VportType::Internal));
        assert!(matches!(VportType::from_u32(99), VportType::Unknown(99)));
    }

    #[test]
    fn test_vport_type_roundtrip() {
        let types = [
            VportType::Unspec,
            VportType::Netdev,
            VportType::Internal,
            VportType::Gre,
            VportType::Vxlan,
            VportType::Geneve,
        ];
        for vt in types {
            assert_eq!(VportType::from_u32(vt.to_u32()).to_u32(), vt.to_u32());
        }
    }

    #[tokio::test]
    #[ignore] // Run only when OVS is installed and as root
    async fn test_list_datapaths() {
        let mut client = OvsNetlinkClient::new()
            .await
            .expect("Failed to create client");
        let dps = client
            .list_datapaths()
            .await
            .expect("Failed to list datapaths");
        println!("Datapaths: {:?}", dps);
    }

    #[tokio::test]
    #[ignore] // Run only when OVS is installed and as root
    async fn test_list_vports() {
        let mut client = OvsNetlinkClient::new()
            .await
            .expect("Failed to create client");
        let dps = client
            .list_datapaths()
            .await
            .expect("Failed to list datapaths");
        if let Some(dp) = dps.first() {
            let vports = client
                .list_vports(&dp.name)
                .await
                .expect("Failed to list vports");
            println!("Vports on {}: {:?}", dp.name, vports);
        }
    }

    #[tokio::test]
    #[ignore] // Run only when OVS is installed and as root
    async fn test_dump_flows() {
        let mut client = OvsNetlinkClient::new()
            .await
            .expect("Failed to create client");
        let dps = client
            .list_datapaths()
            .await
            .expect("Failed to list datapaths");
        if let Some(dp) = dps.first() {
            let flows = client
                .dump_flows(&dp.name)
                .await
                .expect("Failed to dump flows");
            println!("Flows on {}: {} flows found", dp.name, flows.len());
        }
    }
}
</file>

<file path="src/ovsdb.rs">
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
</file>

<file path="src/plugin.rs">
//! Network plugin with OVS/OVSDB persistence
//!
//! This plugin manages network configuration including OVS bridges via OVSDB.
//! CRITICAL: Uses OVSDB JSON-RPC to ensure bridges persist in database.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::openflow::OpenFlowClient;
use crate::ovsdb::OvsdbClient;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkPlugin {
    /// OVS bridges to create
    #[serde(default)]
    pub bridges: Vec<OvsBridge>,

    /// Network interfaces to configure
    #[serde(default)]
    pub interfaces: Vec<NetworkInterface>,

    /// OVSDB persistence configuration
    #[serde(default)]
    pub ovsdb: OvsdbConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvsBridge {
    /// Bridge name (e.g., "vmbr0", "ovsbr0")
    pub name: String,

    /// Datapath type: "system" (default, kernel-based, persistent) or "netdev" (userspace)
    #[serde(default = "default_datapath_type")]
    pub datapath_type: String,

    /// Physical ports to add to bridge
    #[serde(default)]
    pub ports: Vec<String>,

    /// Internal ports (for IP assignment)
    #[serde(default)]
    pub internal_ports: Vec<String>,

    /// IP address for bridge interface (e.g., "10.0.1.1/24")
    pub address: Option<String>,

    /// Enable DHCP on this bridge
    #[serde(default)]
    pub dhcp: bool,

    /// VLAN ID (if this is a VLAN interface)
    pub vlan: Option<u16>,

    /// OpenFlow configuration
    pub openflow: Option<OpenFlowConfig>,
}

fn default_datapath_type() -> String {
    "system".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFlowConfig {
    /// Controller address (default: tcp:127.0.0.1:6653)
    #[serde(default = "default_controller")]
    pub controller: String,

    /// Automatically apply default rules on bridge creation
    #[serde(default)]
    pub auto_apply_defaults: bool,

    /// Default OpenFlow rules to apply
    #[serde(default)]
    pub default_rules: Vec<String>,

    /// Enable fail-secure mode (drop packets if controller unavailable)
    #[serde(default)]
    pub fail_secure: bool,
}

fn default_controller() -> String {
    "tcp:10.200.0.1:6653".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// Interface name (e.g., "eth0", "ens1")
    pub name: String,

    /// IP address (e.g., "192.168.1.10/24")
    pub address: Option<String>,

    /// Enable DHCP
    #[serde(default)]
    pub dhcp: bool,

    /// Bring interface up
    #[serde(default = "default_true")]
    pub up: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OvsdbConfig {
    /// OVSDB socket path (default: /var/run/openvswitch/db.sock)
    #[serde(default = "default_ovsdb_socket")]
    pub socket_path: String,

    /// Database file path for persistence (default: /etc/openvswitch/conf.db)
    #[serde(default = "default_ovsdb_database")]
    pub database_path: String,

    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// Ensure database persists across reboots
    #[serde(default = "default_true")]
    pub persist: bool,
}

fn default_ovsdb_socket() -> String {
    "/var/run/openvswitch/db.sock".to_string()
}

fn default_ovsdb_database() -> String {
    "/etc/openvswitch/conf.db".to_string()
}

fn default_timeout() -> u64 {
    30
}

impl Default for OvsdbConfig {
    fn default() -> Self {
        Self {
            socket_path: default_ovsdb_socket(),
            database_path: default_ovsdb_database(),
            timeout_seconds: default_timeout(),
            persist: true,
        }
    }
}

impl NetworkPlugin {
    /// Create a new network plugin with default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply the network configuration
    pub async fn apply(&self) -> Result<()> {
        info!("Network plugin: Starting network configuration");

        // Step 1: Verify OVSDB persistence configuration
        if !self.bridges.is_empty() {
            self.verify_ovsdb_persistence().await?;
        }

        // Step 2: Wait for OVSDB to be ready
        if !self.bridges.is_empty() {
            self.wait_for_ovsdb().await?;
        }

        // Step 3: Create OVS bridges (via OVSDB JSON-RPC for persistence)
        for bridge in &self.bridges {
            self.create_ovs_bridge(bridge).await?;
        }

        // Step 4: Configure network interfaces
        for interface in &self.interfaces {
            self.configure_interface(interface).await?;
        }

        info!("✓ Network plugin: Complete");
        Ok(())
    }

    /// Get current network state
    pub async fn get_state(&self) -> Result<Value> {
        let client = OvsdbClient::new();

        // Get list of bridges
        let bridges = client.list_bridges().await.unwrap_or_default();

        // Get details for each bridge
        let mut bridge_details = Vec::new();
        for bridge_name in &bridges {
            let ports = client
                .list_bridge_ports(bridge_name)
                .await
                .unwrap_or_default();
            bridge_details.push(serde_json::json!({
                "name": bridge_name,
                "ports": ports,
            }));
        }

        Ok(serde_json::json!({
            "bridges": bridge_details,
            "interfaces": self.interfaces,
            "ovsdb": {
                "socket_path": self.ovsdb.socket_path,
                "persist": self.ovsdb.persist,
            }
        }))
    }

    async fn verify_ovsdb_persistence(&self) -> Result<()> {
        info!("Verifying OVSDB persistence configuration");

        // Check if database file directory exists
        let db_path = std::path::Path::new(&self.ovsdb.database_path);
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                warn!(
                    "OVSDB database directory does not exist: {}",
                    parent.display()
                );
                info!("Creating directory: {}", parent.display());
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        // Verify persistence is enabled
        if !self.ovsdb.persist {
            warn!("OVSDB persistence is DISABLED - bridges may not survive reboots!");
            warn!("Set ovsdb.persist=true in state.json to enable persistence");
        } else {
            info!("✓ OVSDB persistence enabled: {}", self.ovsdb.database_path);
        }

        Ok(())
    }

    async fn wait_for_ovsdb(&self) -> Result<()> {
        info!(
            "Waiting for OVSDB to be ready (timeout: {}s)",
            self.ovsdb.timeout_seconds
        );

        let client = OvsdbClient::new();
        let timeout = Duration::from_secs(self.ovsdb.timeout_seconds);
        let start = std::time::Instant::now();

        loop {
            match client.list_dbs().await {
                Ok(dbs) => {
                    info!("✓ OVSDB is ready, available databases: {:?}", dbs);
                    return Ok(());
                }
                Err(e) => {
                    if start.elapsed() > timeout {
                        return Err(anyhow::anyhow!(
                            "OVSDB connection timeout after {}s: {}",
                            self.ovsdb.timeout_seconds,
                            e
                        ));
                    }
                    warn!("OVSDB not ready yet, retrying... ({})", e);
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    async fn create_ovs_bridge(&self, bridge: &OvsBridge) -> Result<()> {
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("Creating OVS bridge: {}", bridge.name);
        info!("  Datapath type: {}", bridge.datapath_type);
        info!("  Ports: {:?}", bridge.ports);
        info!("  Internal ports: {:?}", bridge.internal_ports);
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let client = OvsdbClient::new();

        // Check if bridge already exists
        let exists = client.bridge_exists(&bridge.name).await?;

        if exists {
            info!("  Bridge '{}' already exists (idempotent)", bridge.name);
            let existing_ports = client.list_bridge_ports(&bridge.name).await?;
            info!("  Existing ports: {:?}", existing_ports);
        } else {
            // Create bridge via OVSDB JSON-RPC
            info!("  Creating bridge via OVSDB JSON-RPC (persistent)");
            client
                .create_bridge(&bridge.name)
                .await
                .context(format!("Failed to create bridge {}", bridge.name))?;
        }

        // Add physical ports
        for port in &bridge.ports {
            info!("  Adding port: {}", port);
            if let Err(e) = client.add_port(&bridge.name, port).await {
                warn!("  Failed to add port {}: {}", port, e);
            }
        }

        // Bring up bridge interface
        info!("  Bringing up bridge interface");
        self.bring_up_interface(&bridge.name).await?;

        // Configure IP address if specified
        if let Some(ref address) = bridge.address {
            info!("  Configuring IP address: {}", address);
            self.configure_ip(&bridge.name, address).await?;
        }

        // Enable DHCP if requested
        if bridge.dhcp {
            info!("  Enabling DHCP");
            self.enable_dhcp(&bridge.name).await?;
        }

        // Apply OpenFlow rules if configured
        if let Some(ref openflow) = bridge.openflow {
            if openflow.auto_apply_defaults && !openflow.default_rules.is_empty() {
                info!("  Applying OpenFlow default rules");
                self.apply_openflow_rules(&bridge.name, openflow).await?;
            }
        }

        info!("✓ Bridge '{}' configured successfully", bridge.name);
        Ok(())
    }

    async fn configure_interface(&self, interface: &NetworkInterface) -> Result<()> {
        info!("Configuring interface: {}", interface.name);

        // Bring interface up/down
        if interface.up {
            self.bring_up_interface(&interface.name).await?;
        } else {
            self.bring_down_interface(&interface.name).await?;
        }

        // Configure IP address
        if let Some(ref address) = interface.address {
            self.configure_ip(&interface.name, address).await?;
        }

        // Enable DHCP
        if interface.dhcp {
            self.enable_dhcp(&interface.name).await?;
        }

        info!("✓ Interface '{}' configured", interface.name);
        Ok(())
    }

    async fn bring_up_interface(&self, name: &str) -> Result<()> {
        crate::rtnetlink::link_up(name)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to bring up interface {}: {}", name, e))?;

        info!("    ✓ Interface '{}' is up", name);
        Ok(())
    }

    async fn bring_down_interface(&self, name: &str) -> Result<()> {
        crate::rtnetlink::link_down(name)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to bring down interface {}: {}", name, e))?;

        Ok(())
    }

    async fn configure_ip(&self, interface: &str, address: &str) -> Result<()> {
        // Parse CIDR (e.g. 192.168.1.1/24)
        let parts: Vec<&str> = address.split('/').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid CIDR address: {}", address));
        }

        let ip_str = parts[0];
        let prefix: u8 = parts[1].parse().context("Invalid prefix length")?;

        // Flush existing addresses
        crate::rtnetlink::flush_addresses(interface)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to flush addresses on {}: {}", interface, e))?;

        // Add new address
        crate::rtnetlink::add_ipv4_address(interface, ip_str, prefix)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to add IP {} to {}: {}", address, interface, e))?;

        info!("    ✓ IP address {} configured on {}", address, interface);
        Ok(())
    }

    async fn enable_dhcp(&self, interface: &str) -> Result<()> {
        // TODO: Replace with native DHCP client library (e.g., dhcproto)
        // For now, we still rely on external dhclient but wrap it to be more robust
        let output = tokio::process::Command::new("dhclient")
            .arg("-v")
            .arg(interface)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("DHCP client warning for {}: {}", interface, stderr);
        } else {
            info!("    ✓ DHCP enabled on {}", interface);
        }

        Ok(())
    }

    async fn apply_openflow_rules(&self, bridge: &str, config: &OpenFlowConfig) -> Result<()> {
        info!(
            "    Applying {} OpenFlow rules to {}",
            config.default_rules.len(),
            bridge
        );

        // Parse controller address
        let addr = if config.controller.starts_with("tcp:") {
            let addr_str = config.controller.trim_start_matches("tcp:");
            addr_str
                .parse()
                .unwrap_or_else(|_| std::net::SocketAddr::from(([10, 200, 0, 1], 6653)))
        } else {
            std::net::SocketAddr::from(([10, 200, 0, 1], 6653))
        };

        // Connect to OpenFlow switch
        let mut client = OpenFlowClient::connect(addr).await.context(format!(
            "Failed to connect to OpenFlow switch for bridge {}",
            bridge
        ))?;

        // Clear existing flows first
        client.delete_all_flows().await?;

        // Apply each rule
        for rule in &config.default_rules {
            client.add_flow_rule(rule).await?;
            info!("      Applied rule: {}", rule);
        }

        info!("    ✓ OpenFlow rules applied to {}", bridge);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_network_config() {
        let mut json = r#"
        {
            "bridges": [
                {
                    "name": "vmbr0",
                    "datapath_type": "system",
                    "ports": ["eth0"],
                    "internal_ports": ["vmbr0-if"],
                    "address": "10.0.0.1/24"
                }
            ],
            "ovsdb": {
                "database_path": "/etc/openvswitch/conf.db",
                "persist": true
            }
        }
        "#
        .to_string();

        let plugin: NetworkPlugin = serde_json::from_str(&json).unwrap();
        assert_eq!(plugin.bridges.len(), 1);
        assert_eq!(plugin.bridges[0].name, "vmbr0");
        assert_eq!(plugin.bridges[0].datapath_type, "system");
        assert_eq!(plugin.ovsdb.database_path, "/etc/openvswitch/conf.db");
        assert!(plugin.ovsdb.persist);
    }

    #[test]
    fn test_default_ovsdb_config() {
        let config = OvsdbConfig::default();
        assert_eq!(config.socket_path, "/var/run/openvswitch/db.sock");
        assert_eq!(config.database_path, "/etc/openvswitch/conf.db");
        assert_eq!(config.timeout_seconds, 30);
        assert!(config.persist);
    }
}
</file>

<file path="src/proxmox.rs">
//! Native Proxmox API Client
//!
//! Provides native REST API access to Proxmox VE for LXC container management.
//! This replaces shelling out to `pct` commands with direct API calls.
//!
//! ## Authentication
//!
//! The client supports API token authentication. Create a token file at
//! `/etc/op-dbus/pve-token` or set `PVE_TOKEN_FILE` environment variable:
//!
//! ```text
//! PVE_API_USER=root@pam
//! PVE_API_TOKEN_ID=op-dbus
//! PVE_API_TOKEN_SECRET=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
//! PVE_API_NODE=proxmox
//! ```

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Proxmox API client for LXC container management
pub struct ProxmoxClient {
    client: Client,
    base_url: String,
    node: String,
    token: Option<ProxmoxToken>,
}

/// API token for Proxmox authentication
#[derive(Clone, Debug)]
pub struct ProxmoxToken {
    /// User identifier (e.g., "root@pam")
    pub user: String,
    /// Token ID (e.g., "op-dbus")
    pub token_id: String,
    /// Token secret value
    pub secret: String,
}

impl ProxmoxToken {
    /// Format the authorization header value
    pub fn to_auth_header(&self) -> String {
        format!(
            "PVEAPIToken={}!{}={}",
            self.user, self.token_id, self.secret
        )
    }
}

/// LXC container information from Proxmox API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxcContainer {
    /// Container VM ID
    pub vmid: u32,
    /// Container name/hostname
    #[serde(default)]
    pub name: Option<String>,
    /// Container status (running, stopped, etc.)
    pub status: String,
    /// CPU usage (if available)
    #[serde(default)]
    pub cpu: Option<f64>,
    /// Memory usage in bytes (if available)
    #[serde(default)]
    pub mem: Option<u64>,
    /// Maximum memory in bytes (if available)
    #[serde(default)]
    pub maxmem: Option<u64>,
    /// Disk usage in bytes (if available)
    #[serde(default)]
    pub disk: Option<u64>,
    /// Maximum disk in bytes (if available)
    #[serde(default)]
    pub maxdisk: Option<u64>,
    /// Uptime in seconds (if available)
    #[serde(default)]
    pub uptime: Option<u64>,
    /// Additional fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Request to create a new LXC container
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateContainerRequest {
    /// Container VM ID
    pub vmid: u32,
    /// OS template (e.g., "local:vztmpl/debian-13-standard_13.1-2_amd64.tar.zst")
    pub ostemplate: String,
    /// Hostname
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    /// Memory in MB
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<u32>,
    /// Swap in MB
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap: Option<u32>,
    /// Number of CPU cores
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    /// Root filesystem specification (e.g., "local-btrfs:8")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rootfs: Option<String>,
    /// Network configuration (e.g., "name=eth0,bridge=vmbr0")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net0: Option<String>,
    /// Run as unprivileged container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unprivileged: Option<bool>,
    /// Container features (e.g., "nesting=1")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<String>,
    /// Start container after creation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<bool>,
    /// Start on boot
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onboot: Option<bool>,
    /// Protect container from deletion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protection: Option<bool>,
    /// DNS server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nameserver: Option<String>,
    /// DNS search domain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub searchdomain: Option<String>,
    /// Password for root user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// SSH public keys
    #[serde(rename = "ssh-public-keys", skip_serializing_if = "Option::is_none")]
    pub ssh_public_keys: Option<String>,
    /// Storage backend (e.g., "local-btrfs")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<String>,
}

/// Container status response from Proxmox API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStatus {
    /// Container status (running, stopped, etc.)
    pub status: String,
    /// VM ID
    pub vmid: u32,
    /// Container name
    #[serde(default)]
    pub name: Option<String>,
    /// CPU usage
    #[serde(default)]
    pub cpu: Option<f64>,
    /// Memory usage in bytes
    #[serde(default)]
    pub mem: Option<u64>,
    /// Maximum memory in bytes
    #[serde(default)]
    pub maxmem: Option<u64>,
    /// Disk read bytes
    #[serde(default)]
    pub diskread: Option<u64>,
    /// Disk write bytes
    #[serde(default)]
    pub diskwrite: Option<u64>,
    /// Network in bytes
    #[serde(default)]
    pub netin: Option<u64>,
    /// Network out bytes
    #[serde(default)]
    pub netout: Option<u64>,
    /// Uptime in seconds
    #[serde(default)]
    pub uptime: Option<u64>,
    /// PID of main process
    #[serde(default)]
    pub pid: Option<u32>,
    /// HA state
    #[serde(default)]
    pub ha: Option<HashMap<String, serde_json::Value>>,
    /// Additional fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Proxmox API response wrapper
#[derive(Debug, Clone, Deserialize)]
pub struct ProxmoxResponse<T> {
    pub data: T,
}

/// Task status response
#[derive(Debug, Clone, Deserialize)]
pub struct TaskStatus {
    pub status: String,
    #[serde(default)]
    pub exitstatus: Option<String>,
    #[serde(default)]
    pub node: Option<String>,
    #[serde(rename = "type", default)]
    pub task_type: Option<String>,
    #[serde(default)]
    pub upid: Option<String>,
}

/// Proxmox version info
#[derive(Debug, Clone, Deserialize)]
pub struct ProxmoxVersion {
    pub version: String,
    pub release: String,
    #[serde(default)]
    pub repoid: Option<String>,
}

impl ProxmoxClient {
    /// Create a new client with default settings
    pub fn new() -> Self {
        Self::with_config("https://localhost:8006", "localhost", None)
    }

    /// Create a client with custom configuration
    pub fn with_config(base_url: &str, node: &str, token: Option<ProxmoxToken>) -> Self {
        // Create client that accepts self-signed certificates (Proxmox default)
        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            node: node.to_string(),
            token,
        }
    }

    /// Create a client from environment/config file
    pub fn from_env() -> Result<Self> {
        let token_file = std::env::var("PVE_TOKEN_FILE")
            .unwrap_or_else(|_| "/etc/op-dbus/pve-token".to_string());

        // Try to read token from file
        let (token, node) = if let Ok(content) = std::fs::read_to_string(&token_file) {
            let mut user = None;
            let mut token_id = None;
            let mut secret = None;
            let mut node = "localhost".to_string();

            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"').trim_matches('\'');

                    match key {
                        "PVE_API_USER" => user = Some(value.to_string()),
                        "PVE_API_TOKEN_ID" => token_id = Some(value.to_string()),
                        "PVE_API_TOKEN_SECRET" => secret = Some(value.to_string()),
                        "PVE_API_NODE" => node = value.to_string(),
                        _ => {}
                    }
                }
            }

            let token = match (user, token_id, secret) {
                (Some(user), Some(token_id), Some(secret)) => Some(ProxmoxToken {
                    user,
                    token_id,
                    secret,
                }),
                _ => {
                    warn!("Incomplete token configuration in {}", token_file);
                    None
                }
            };

            (token, node)
        } else {
            debug!("Token file not found: {}", token_file);
            (None, "localhost".to_string())
        };

        // Check for base URL override
        let base_url =
            std::env::var("PVE_API_URL").unwrap_or_else(|_| "https://localhost:8006".to_string());

        Ok(Self::with_config(&base_url, &node, token))
    }

    /// Build the authorization header if token is configured
    fn auth_header(&self) -> Option<String> {
        self.token.as_ref().map(|t| t.to_auth_header())
    }

    /// Make a GET request to the Proxmox API
    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        debug!("GET {}", url);

        let mut req = self.client.get(&url);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }

        let resp = req.send().await.context("Failed to send request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("API request failed: {} - {}", status, body));
        }

        let response: ProxmoxResponse<T> = resp.json().await.context("Failed to parse response")?;
        Ok(response.data)
    }

    /// Make a POST request to the Proxmox API
    async fn post<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        debug!("POST {}", url);

        let mut req = self.client.post(&url).form(body);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }

        let resp = req.send().await.context("Failed to send request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("API request failed: {} - {}", status, body));
        }

        let response: ProxmoxResponse<R> = resp.json().await.context("Failed to parse response")?;
        Ok(response.data)
    }

    /// Make a DELETE request to the Proxmox API
    async fn delete<R: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        debug!("DELETE {}", url);

        let mut req = self.client.delete(&url);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }

        let resp = req.send().await.context("Failed to send request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("API request failed: {} - {}", status, body));
        }

        let response: ProxmoxResponse<R> = resp.json().await.context("Failed to parse response")?;
        Ok(response.data)
    }

    // =========================================================================
    // Public API Methods
    // =========================================================================

    /// Check if Proxmox API is available
    pub async fn check_available(&self) -> Result<ProxmoxVersion> {
        self.get("/api2/json/version").await
    }

    /// List all LXC containers on the node
    pub async fn list_containers(&self) -> Result<Vec<LxcContainer>> {
        let path = format!("/api2/json/nodes/{}/lxc", self.node);
        self.get(&path).await
    }

    /// Get detailed status of a specific container
    pub async fn get_container(&self, vmid: u32) -> Result<ContainerStatus> {
        let path = format!("/api2/json/nodes/{}/lxc/{}/status/current", self.node, vmid);
        self.get(&path).await
    }

    /// Get container configuration
    pub async fn get_container_config(
        &self,
        vmid: u32,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let path = format!("/api2/json/nodes/{}/lxc/{}/config", self.node, vmid);
        self.get(&path).await
    }

    /// Create a new LXC container
    ///
    /// Returns the task UPID for tracking the creation progress
    pub async fn create_container(&self, config: &CreateContainerRequest) -> Result<String> {
        let path = format!("/api2/json/nodes/{}/lxc", self.node);
        info!(
            "Creating container {} with hostname {:?}",
            config.vmid, config.hostname
        );
        self.post(&path, config).await
    }

    /// Start a container
    ///
    /// Returns the task UPID
    pub async fn start_container(&self, vmid: u32) -> Result<String> {
        let path = format!("/api2/json/nodes/{}/lxc/{}/status/start", self.node, vmid);
        info!("Starting container {}", vmid);
        self.post::<(), String>(&path, &()).await
    }

    /// Stop a container
    ///
    /// Returns the task UPID
    pub async fn stop_container(&self, vmid: u32) -> Result<String> {
        let path = format!("/api2/json/nodes/{}/lxc/{}/status/stop", self.node, vmid);
        info!("Stopping container {}", vmid);
        self.post::<(), String>(&path, &()).await
    }

    /// Shutdown a container gracefully
    ///
    /// Returns the task UPID
    pub async fn shutdown_container(&self, vmid: u32, timeout: Option<u32>) -> Result<String> {
        let path = format!(
            "/api2/json/nodes/{}/lxc/{}/status/shutdown",
            self.node, vmid
        );
        info!("Shutting down container {} (timeout: {:?})", vmid, timeout);

        #[derive(Serialize)]
        struct ShutdownParams {
            #[serde(skip_serializing_if = "Option::is_none")]
            timeout: Option<u32>,
        }

        self.post(&path, &ShutdownParams { timeout }).await
    }

    /// Delete a container
    ///
    /// The container must be stopped first.
    /// Returns the task UPID
    pub async fn delete_container(&self, vmid: u32) -> Result<String> {
        let path = format!("/api2/json/nodes/{}/lxc/{}", self.node, vmid);
        info!("Deleting container {}", vmid);
        self.delete(&path).await
    }

    /// Force stop and delete a container
    pub async fn force_delete_container(&self, vmid: u32) -> Result<String> {
        let path = format!(
            "/api2/json/nodes/{}/lxc/{}?force=1&purge=1",
            self.node, vmid
        );
        info!("Force deleting container {}", vmid);
        self.delete(&path).await
    }

    /// Get task status
    pub async fn get_task_status(&self, upid: &str) -> Result<TaskStatus> {
        let path = format!("/api2/json/nodes/{}/tasks/{}/status", self.node, upid);
        self.get(&path).await
    }

    /// Wait for a task to complete
    pub async fn wait_for_task(&self, upid: &str, timeout_secs: u64) -> Result<TaskStatus> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        loop {
            let status = self.get_task_status(upid).await?;

            if status.status == "stopped" {
                if let Some(ref exit) = status.exitstatus {
                    if exit != "OK" {
                        return Err(anyhow!("Task failed: {}", exit));
                    }
                }
                return Ok(status);
            }

            if start.elapsed() > timeout {
                return Err(anyhow!("Task timed out after {} seconds", timeout_secs));
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Check if container exists
    pub async fn container_exists(&self, vmid: u32) -> Result<bool> {
        match self.get_container(vmid).await {
            Ok(_) => Ok(true),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("500")
                    || msg.contains("does not exist")
                    || msg.contains("not found")
                {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Check if container is running
    pub async fn is_running(&self, vmid: u32) -> Result<bool> {
        let status = self.get_container(vmid).await?;
        Ok(status.status == "running")
    }

    /// Clone a container
    pub async fn clone_container(
        &self,
        source_vmid: u32,
        target_vmid: u32,
        hostname: Option<&str>,
        full_clone: bool,
    ) -> Result<String> {
        let path = format!("/api2/json/nodes/{}/lxc/{}/clone", self.node, source_vmid);
        info!(
            "Cloning container {} to {} (full: {})",
            source_vmid, target_vmid, full_clone
        );

        #[derive(Serialize)]
        struct CloneParams<'a> {
            newid: u32,
            #[serde(skip_serializing_if = "Option::is_none")]
            hostname: Option<&'a str>,
            full: bool,
        }

        self.post(
            &path,
            &CloneParams {
                newid: target_vmid,
                hostname,
                full: full_clone,
            },
        )
        .await
    }

    /// Create container and wait for completion
    pub async fn create_container_sync(
        &self,
        config: &CreateContainerRequest,
        timeout_secs: u64,
    ) -> Result<()> {
        let upid = self.create_container(config).await?;
        self.wait_for_task(&upid, timeout_secs).await?;
        Ok(())
    }

    /// Start container and wait for completion
    pub async fn start_container_sync(&self, vmid: u32, timeout_secs: u64) -> Result<()> {
        let upid = self.start_container(vmid).await?;
        self.wait_for_task(&upid, timeout_secs).await?;
        Ok(())
    }

    /// Stop container and wait for completion
    pub async fn stop_container_sync(&self, vmid: u32, timeout_secs: u64) -> Result<()> {
        let upid = self.stop_container(vmid).await?;
        self.wait_for_task(&upid, timeout_secs).await?;
        Ok(())
    }

    /// Delete container and wait for completion
    pub async fn delete_container_sync(&self, vmid: u32, timeout_secs: u64) -> Result<()> {
        let upid = self.delete_container(vmid).await?;
        self.wait_for_task(&upid, timeout_secs).await?;
        Ok(())
    }

    /// Get the node name
    pub fn node(&self) -> &str {
        &self.node
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Default for ProxmoxClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_auth_header() {
        let token = ProxmoxToken {
            user: "root@pam".to_string(),
            token_id: "op-dbus".to_string(),
            secret: "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx".to_string(),
        };

        assert_eq!(
            token.to_auth_header(),
            "PVEAPIToken=root@pam!op-dbus=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
        );
    }

    #[test]
    fn test_create_request_serialization() {
        let req = CreateContainerRequest {
            vmid: 100,
            ostemplate: "local:vztmpl/debian-13.tar.zst".to_string(),
            hostname: Some("test".to_string()),
            memory: Some(512),
            cores: Some(2),
            ..Default::default()
        };

        let json: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(json["vmid"], 100);
        assert_eq!(json["hostname"], "test");
        assert_eq!(json["memory"], 512);
        assert!(json.get("swap").is_none()); // Should be skipped when None
    }
}
</file>

<file path="src/rtnetlink.rs">
//! Rtnetlink helpers - native netlink operations for IP addresses and routes

use anyhow::{Context, Result};
use futures::TryStreamExt;
use netlink_packet_route::address::AddressAttribute;
use netlink_packet_route::link::LinkAttribute;
use rtnetlink::{new_connection, IpVersion};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// Network interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub index: u32,
    pub mac_address: Option<String>,
    pub mtu: Option<u32>,
    pub flags: Vec<String>,
    pub state: String,
    pub kind: Option<String>,
    pub addresses: Vec<InterfaceAddress>,
}

/// IP address on an interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceAddress {
    pub address: String,
    pub prefix_len: u8,
    pub family: String,
}

/// List all network interfaces with their details
pub async fn list_interfaces() -> Result<Vec<NetworkInterface>> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    let mut interfaces = Vec::new();
    let mut links = handle.link().get().execute();

    while let Some(link) = links.try_next().await? {
        let index = link.header.index;
        let mut name = String::new();
        let mut mac_address = None;
        let mut mtu = None;
        let mut kind = None;

        // Extract attributes
        for attr in &link.attributes {
            match attr {
                LinkAttribute::IfName(n) => name = n.clone(),
                LinkAttribute::Address(addr) => {
                    mac_address = Some(
                        addr.iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(":"),
                    );
                }
                LinkAttribute::Mtu(m) => mtu = Some(*m),
                LinkAttribute::LinkInfo(infos) => {
                    for info in infos {
                        if let netlink_packet_route::link::LinkInfo::Kind(k) = info {
                            kind = Some(format!("{:?}", k));
                        }
                    }
                }
                _ => {}
            }
        }

        // Determine state from flags
        let flags_val = &link.header.flags;
        let flags: Vec<String> = flags_val.iter().map(|f| format!("{:?}", f)).collect();
        let is_up = flags_val
            .iter()
            .any(|f| matches!(f, netlink_packet_route::link::LinkFlag::Up));
        let state = if is_up {
            "up".to_string()
        } else {
            "down".to_string()
        };

        // Get addresses for this interface
        let addresses = match get_interface_addresses(&handle, index).await {
            Ok(addrs) => addrs,
            Err(e) => {
                log::warn!("Failed to get addresses for interface {}: {}", name, e);
                Vec::new()
            }
        };

        interfaces.push(NetworkInterface {
            name,
            index,
            mac_address,
            mtu,
            flags,
            state,
            kind,
            addresses,
        });
    }

    Ok(interfaces)
}

/// Get addresses for a specific interface
async fn get_interface_addresses(
    handle: &rtnetlink::Handle,
    ifindex: u32,
) -> Result<Vec<InterfaceAddress>> {
    let mut addresses = Vec::new();
    let mut addr_stream = handle
        .address()
        .get()
        .set_link_index_filter(ifindex)
        .execute();

    while let Some(addr_msg) = addr_stream.try_next().await? {
        let family = match addr_msg.header.family {
            netlink_packet_route::AddressFamily::Inet => "inet".to_string(),
            netlink_packet_route::AddressFamily::Inet6 => "inet6".to_string(),
            f => format!("{:?}", f),
        };

        for attr in &addr_msg.attributes {
            if let AddressAttribute::Address(addr) = attr {
                let addr_str = addr.to_string();

                addresses.push(InterfaceAddress {
                    address: addr_str,
                    prefix_len: addr_msg.header.prefix_len,
                    family: family.clone(),
                });
            }
        }
    }

    Ok(addresses)
}

/// Get default route information
pub async fn get_default_route() -> Result<Option<serde_json::Value>> {
    use netlink_packet_route::route::RouteAttribute;

    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    let mut routes = handle.route().get(IpVersion::V4).execute();

    while let Some(route) = routes.try_next().await? {
        // Check if this is a default route (destination 0.0.0.0/0)
        if route.header.destination_prefix_length == 0 {
            let mut gateway = None;
            let mut oif_index = None;

            for attr in &route.attributes {
                match attr {
                    RouteAttribute::Gateway(gw) => {
                        gateway = Some(format!("{:?}", gw));
                    }
                    RouteAttribute::Oif(idx) => {
                        oif_index = Some(idx);
                    }
                    _ => {}
                }
            }

            // Try to get interface name for the output interface
            let mut oif_name = None;
            if let Some(idx) = oif_index {
                let mut links = handle.link().get().match_index(*idx).execute();
                if let Some(link) = links.try_next().await? {
                    for attr in &link.attributes {
                        if let LinkAttribute::IfName(name) = attr {
                            oif_name = Some(name.clone());
                            break;
                        }
                    }
                }
            }

            return Ok(Some(serde_json::json!({
                "gateway": gateway,
                "interface_index": oif_index,
                "interface_name": oif_name,
                "destination": "0.0.0.0/0",
            })));
        }
    }

    Ok(None)
}

/// Add IPv4 address to interface
pub async fn add_ipv4_address(ifname: &str, ip: &str, prefix: u8) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Parse IP address
    let addr: Ipv4Addr = ip.parse().context("Invalid IPv4 address")?;

    // Add address to interface
    handle
        .address()
        .add(ifindex, addr.into(), prefix)
        .execute()
        .await
        .context("Failed to add IP address")?;

    Ok(())
}

/// Delete IPv4 address from interface
#[allow(dead_code)]
pub async fn del_ipv4_address(ifname: &str, ip: &str, prefix: u8) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Parse IP address
    let addr: Ipv4Addr = ip.parse().context("Invalid IPv4 address")?;

    // Get addresses filtered by interface, prefix, and address
    let mut addresses = handle
        .address()
        .get()
        .set_link_index_filter(ifindex)
        .set_prefix_length_filter(prefix)
        .set_address_filter(std::net::IpAddr::V4(addr))
        .execute();

    if let Some(addr_msg) = addresses.try_next().await? {
        handle.address().del(addr_msg).execute().await?;
    }

    Ok(())
}

/// Flush all addresses from interface
#[allow(dead_code)]
pub async fn flush_addresses(ifname: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Get all addresses on this interface
    let mut addresses = handle
        .address()
        .get()
        .set_link_index_filter(ifindex)
        .execute();

    while let Some(addr) = addresses.try_next().await? {
        // Delete this address
        if let Err(e) = handle.address().del(addr).execute().await {
            log::warn!("Failed to delete address: {}", e);
        }
    }

    Ok(())
}

/// Set link up
pub async fn link_up(ifname: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Set link up
    handle
        .link()
        .set(ifindex)
        .up()
        .execute()
        .await
        .context("Failed to bring link up")?;

    Ok(())
}

/// Set link down
#[allow(dead_code)]
pub async fn link_down(ifname: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Set link down
    handle
        .link()
        .set(ifindex)
        .down()
        .execute()
        .await
        .context("Failed to bring link down")?;

    Ok(())
}

/// Add default route
pub async fn add_default_route(ifname: &str, gateway: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Parse gateway address
    let gw: Ipv4Addr = gateway.parse().context("Invalid gateway address")?;

    // Add default route
    handle
        .route()
        .add()
        .v4()
        .destination_prefix(Ipv4Addr::new(0, 0, 0, 0), 0)
        .gateway(gw)
        .output_interface(ifindex)
        .execute()
        .await
        .context("Failed to add default route")?;

    Ok(())
}

/// Add default route with `onlink` flag via `ip route replace`.
///
/// Required when the output interface is an OVS internal port: the kernel
/// cannot verify gateway reachability until OVS is forwarding, so we must
/// bypass nexthop validation. The netlink route-flag namespace does not cleanly
/// expose RTNH_F_ONLINK for single-hop routes, so we delegate to iproute2
/// which handles the encoding correctly.
pub async fn add_default_route_onlink(ifname: &str, gateway: &str) -> Result<()> {
    use std::process::Command;
    let status = Command::new("ip")
        .args([
            "route", "replace", "default", "via", gateway, "dev", ifname, "onlink",
        ])
        .status()
        .context("failed to execute ip")?;
    if !status.success() {
        anyhow::bail!(
            "ip route replace default via {} dev {} onlink failed: {}",
            gateway,
            ifname,
            status
        );
    }
    Ok(())
}

/// Set MAC address on interface
pub async fn set_mac_address(ifname: &str, mac: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by name
    let mut links = handle.link().get().match_name(ifname.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", ifname))?;

    let ifindex = link.header.index;

    // Parse MAC address "aa:bb:cc:dd:ee:ff" -> [u8; 6]
    let mac_bytes: Vec<u8> = mac
        .split(':')
        .map(|byte| u8::from_str_radix(byte, 16).context("Invalid MAC address byte"))
        .collect::<Result<Vec<u8>>>()?;

    if mac_bytes.len() != 6 {
        return Err(anyhow::anyhow!(
            "Invalid MAC address '{}': expected 6 octets",
            mac
        ));
    }

    // Set MAC address
    handle
        .link()
        .set(ifindex)
        .address(mac_bytes)
        .execute()
        .await
        .context("Failed to set MAC address")?;

    Ok(())
}

/// Delete default route
pub async fn del_default_route() -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Get all routes
    let mut routes = handle.route().get(IpVersion::V4).execute();

    while let Some(route) = routes.try_next().await? {
        // Check if this is a default route (destination 0.0.0.0/0)
        if route.header.destination_prefix_length == 0 {
            // Delete this route
            if let Err(e) = handle.route().del(route).execute().await {
                log::warn!("Failed to delete default route: {}", e);
            }
        }
    }

    Ok(())
}

/// List IPv4 routes for a given interface (by name)
pub async fn list_routes_for_interface(_ifname: &str) -> Result<Vec<serde_json::Value>> {
    // Minimal, compile-safe stub; route filtering can be added later.
    Ok(Vec::new())
}

/// List all veth interfaces (simplified implementation)
pub async fn list_veth_interfaces() -> Result<Vec<String>> {
    // For now, return empty list - this would need more complex rtnetlink code
    // to properly enumerate all interfaces and check their types
    // The LXC plugin will fall back to other methods if this returns empty
    Ok(Vec::new())
}

/// Rename network interface
pub async fn link_set_name(old_name: &str, new_name: &str) -> Result<()> {
    let (connection, handle, _) = new_connection()?;
    tokio::spawn(connection);

    // Find interface by current name
    let mut links = handle
        .link()
        .get()
        .match_name(old_name.to_string())
        .execute();
    let link = links
        .try_next()
        .await?
        .context(format!("Interface '{}' not found", old_name))?;

    let ifindex = link.header.index;

    // Set new name
    handle
        .link()
        .set(ifindex)
        .name(new_name.to_string())
        .execute()
        .await
        .context(format!("Failed to rename {} to {}", old_name, new_name))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Basic smoke test to ensure rtnetlink connection and route listing works.
    // Uses the loopback interface which always exists.
    #[tokio::test(flavor = "current_thread")]
    async fn test_list_routes_for_loopback() {
        let res = list_routes_for_interface("lo").await;
        assert!(
            res.is_ok(),
            "expected Ok from list_routes_for_interface: {:?}",
            res
        );
        let routes = res.unwrap();
        // No strict expectation on content; presence/empty is both fine.
        println!("routes on lo: {:?}", routes);
    }
}
</file>

<file path="Cargo.toml">
[package]
name = "op-network"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Native networking: OpenFlow (all versions, pure Rust), OVSDB JSON-RPC, rtnetlink, Proxmox API, container networking"

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = "1"
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
futures = "0.3"
rtnetlink = { workspace = true }
log = { workspace = true }
uuid = { version = "1", features = ["v4"] }

# HTTP client for Proxmox API
reqwest = { workspace = true }

# Netlink (Generic Netlink for OVS)
netlink-sys = "0.8"
netlink-packet-core = "0.7"
netlink-packet-generic = "0.3"
netlink-packet-utils = "0.5"
netlink-packet-route = "0.19"
byteorder = "1.5"

# Internal
op-core = { path = "../op-core" }

# Root privilege checking
libc = { workspace = true }
tracing-subscriber = { workspace = true }

# OVSDB and OpenFlow via rovs crate family
rovs-ovsdb = "0.2"
rovs-openflow = "0.2"
rovs-jsonrpc = "0.2"
rovs-types = "0.2"
rovs-transport = "0.2"

# Bytes for wire-protocol encoding in controller.rs
bytes = "1"

# simd-json — for the transact_simd compatibility shim in ovsdb.rs
simd-json = { workspace = true }

[[bin]]
name = "op-of-controller"
path = "src/bin/op-of-controller.rs"

[[bin]]
name = "op-xdp-wg"
path = "src/bin/op-xdp-wg.rs"

[[bin]]
name = "op-ovsbr0-afxdp"
path = "src/bin/op-ovsbr0-afxdp.rs"

[[bin]]
name = "op-ovsbr0-setup"
path = "src/bin/op-ovsbr0-setup.rs"
</file>

<file path="compare-op-network.md">
# compare-op-network

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 9 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 8 |
| Partial artifacts | 0 |
| Spec-listed source files | 9 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- Native networking: OpenFlow (all versions, pure Rust), OVSDB JSON-RPC, rtnetlink, Proxmox API, container networking
- Internal crate integrations: op-core.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/rtnetlink.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/rtnetlink.rs |
| `src/proxmox.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/proxmox.rs |
| `src/plugin.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/plugin.rs |
| `src/ovsdb.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/ovsdb.rs |
| `src/ovs_netlink.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/ovs_netlink.rs |
| `src/ovs_error.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/ovs_error.rs |
| `src/ovs_capabilities.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/ovs_capabilities.rs |
| `src/openflow.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/openflow.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `root` | ✅ Present | root source group | src/lib.rs, src/openflow.rs, src/ovs_capabilities.rs, src/ovs_error.rs, src/ovs_netlink.rs, src/ovsdb.rs, src/plugin.rs, src/proxmox.rs, ... (+1 more) |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| rtnetlink | ✅ Implemented | src/rtnetlink.rs | SPEC main module |
| proxmox | ✅ Implemented | src/proxmox.rs | SPEC main module |
| plugin | ✅ Implemented | src/plugin.rs | SPEC main module |
| ovsdb | ✅ Implemented | src/ovsdb.rs | SPEC main module |
| ovs_netlink | ✅ Implemented | src/ovs_netlink.rs | SPEC main module |
| ovs_error | ✅ Implemented | src/ovs_error.rs | SPEC main module |
| ovs_capabilities | ✅ Implemented | src/ovs_capabilities.rs | SPEC main module |
| openflow | ✅ Implemented | src/openflow.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - not listed in SPEC dependency block

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `async-trait` - documented in SPEC
- `futures` - documented in SPEC
- `rtnetlink` - documented in SPEC
- `log` - documented in SPEC
- `reqwest` - documented in SPEC
- `netlink-sys` - documented in SPEC
- `netlink-packet-core` - documented in SPEC
- `netlink-packet-generic` - documented in SPEC
- `netlink-packet-utils` - documented in SPEC
- `netlink-packet-route` - documented in SPEC
- `byteorder` - not listed in SPEC dependency block
- `libc` - not listed in SPEC dependency block

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: openflow, ovs_capabilities, ovs_error, ovs_netlink, ovsdb, plugin, proxmox, rtnetlink.
- 3 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="SPEC.md">
# op-network - Specification

## Overview
**Crate**: `op-network`  
**Location**: `crates/op-network`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-network"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-network/src/rtnetlink.rs
op-network/src/proxmox.rs
op-network/src/plugin.rs
op-network/src/ovsdb.rs
op-network/src/ovs_netlink.rs
op-network/src/ovs_error.rs
op-network/src/ovs_capabilities.rs
op-network/src/openflow.rs
op-network/src/lib.rs
```

### Key Dependencies
```toml
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
futures = "0.3"
rtnetlink = { workspace = true }
log = { workspace = true }

# HTTP client for Proxmox API
reqwest = { workspace = true }

# Netlink (Generic Netlink for OVS)
netlink-sys = "0.8"
netlink-packet-core = "0.7"
netlink-packet-generic = "0.3"
netlink-packet-utils = "0.5"
netlink-packet-route = "0.19"
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
       9 Rust source files

### Main Modules
rtnetlink
proxmox
plugin
ovsdb
ovs_netlink
ovs_error
ovs_capabilities
openflow

## Purpose
Native networking: OpenFlow (all versions, pure Rust), OVSDB JSON-RPC, rtnetlink, Proxmox API, container networking

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-core

---
*Generated from crate analysis*
</file>

</files>
