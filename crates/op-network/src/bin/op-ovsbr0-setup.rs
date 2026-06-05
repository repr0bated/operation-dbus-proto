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
        .map(extract_uuids)
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
        .map(extract_uuids)
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
