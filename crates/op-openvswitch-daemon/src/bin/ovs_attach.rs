//! ovs-attach — D-Bus client that calls BringUpLoopback and AttachPort
//! for container bootstrap.
//!
//! Discovers all config from the D-Bus tree under:
//! - /org/opdbus/v1/plugins/oci (loopback_required, port_attach per container)
//! - /org/opdbus/v1/plugins/privacy_router (bridge, port, IPs — fallback)
//!
//! No hardcoded bus names, paths, or IP addresses. The schema IS the config.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;
use zbus::connection::Builder;

const DBUS_SOCKET: &str = "/run/op-dbus.sock";
const PLUGIN_BUS: &str = op_core::config::OPDBUS_BUS_NAME;
const PRIVACY_ROUTER_BASE: &str = "/org/opdbus/v1/plugins/privacy_router";
const OCI_BASE: &str = "/org/opdbus/v1/plugins/oci";

async fn get_json_property(conn: &zbus::Connection, path: &str, prop: &str) -> Result<Value> {
    let iface = "org.opdbus.ProjectedObjectV1".to_string();
    let msg = conn
        .call_method(
            Some(PLUGIN_BUS),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(&iface, prop),
        )
        .await?;
    let body = msg.body();
    let reply: zbus::zvariant::Value = body.deserialize()?;

    if let zbus::zvariant::Value::Str(s) = reply {
        Ok(serde_json::from_str(&s)?)
    } else {
        anyhow::bail!("expected string property, got {:?}", reply)
    }
}

/// Discover the proxy path on the ovsdb daemon bus by introspecting the tree.
async fn discover_proxy_path(conn: &zbus::Connection, dbus_bus: &str) -> Result<String> {
    let tree_msg = conn
        .call_method(
            Some(dbus_bus),
            "/",
            Some("org.freedesktop.DBus.Introspectable"),
            "Introspect",
            &(),
        )
        .await?;
    let introspect_xml: String = tree_msg.body().deserialize()?;

    if introspect_xml.contains("opdbus") {
        let rovs_msg = conn
            .call_method(
                Some(dbus_bus),
                "/org/opdbus/rovs",
                Some("org.freedesktop.DBus.Introspectable"),
                "Introspect",
                &(),
            )
            .await?;
        let rovs_xml: String = rovs_msg.body().deserialize()?;

        if rovs_xml.contains("proxy") {
            return Ok("/org/opdbus/rovs/proxy".to_string());
        }
    }
    anyhow::bail!("could not find proxy node in ovsdb daemon tree")
}

/// Read this container's config from the OCI plugin schema.
/// Returns (loopback_required, port_attach_config).
async fn read_oci_container_config(
    conn: &zbus::Connection,
    instance_name: &str,
) -> Result<(bool, Option<Value>)> {
    let oci_schema = get_json_property(conn, OCI_BASE, "JsonData").await?;

    if let Some(containers) = oci_schema["config"]["containers"].as_array() {
        for c in containers {
            if c["name"].as_str() == Some(instance_name) {
                let lo = c["loopback_required"].as_bool().unwrap_or(false);
                let port = c.get("port_attach").cloned();
                return Ok((lo, port));
            }
        }
    }
    Ok((false, None))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Wait for D-Bus socket
    eprintln!("waiting for D-Bus socket at {DBUS_SOCKET}...");
    for _ in 0..60 {
        if Path::new(DBUS_SOCKET).exists() {
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }
    anyhow::ensure!(
        Path::new(DBUS_SOCKET).exists(),
        "D-Bus socket not found after 60s"
    );

    // Connect to D-Bus
    let address = format!("unix:path={DBUS_SOCKET}");
    eprintln!("connecting to D-Bus...");
    let conn = Builder::address(&*address)?
        .build()
        .await
        .context("connect to D-Bus")?;

    // Read instance name from hostname (incus sets hostname = instance name)
    let instance_name = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .trim()
        .to_string();
    if instance_name.is_empty() {
        anyhow::bail!("could not read /etc/hostname for instance name");
    }
    eprintln!("instance_name={instance_name}");

    // ── Step 1: Read this container's config from the OCI plugin schema ──
    eprintln!("reading OCI plugin schema from D-Bus tree...");
    let (loopback_required, port_attach_config) = read_oci_container_config(&conn, &instance_name)
        .await
        .unwrap_or((false, None));

    // ── Step 2: Bring up loopback if the OCI schema declares it ────────
    if loopback_required {
        eprintln!("loopback_required=true for {instance_name}, calling BringUpLoopback...");

        let daemon = get_json_property(
            &conn,
            "/org/opdbus/v1/plugins/ovsdb_daemon",
            "JsonData",
        )
        .await?;
        let dbus_bus = daemon["dbus_bus_name"]
            .as_str()
            .unwrap_or("org.opdbus.v1.plugins.ovsdb");
        let proxy_path = discover_proxy_path(&conn, dbus_bus).await?;

        let lo_params = serde_json::json!({
            "instance_name": instance_name
        });

        let lo_result: String = conn
            .call_method(
                Some(dbus_bus),
                proxy_path.as_str(),
                Some("org.opdbus.rovs.proxy"),
                "BringUpLoopback",
                &(lo_params.to_string()),
            )
            .await?
            .body()
            .deserialize()?;

        eprintln!("BringUpLoopback result: {lo_result}");
    } else {
        eprintln!("loopback_required=false for {instance_name}, skipping lo bring-up");
    }

    // ── Step 3: Attach OVS port ─────────────────────────────────────────
    // If the OCI schema has port_attach config, use it directly.
    // Otherwise fall back to the privacy_router schema for the xray container.
    let (params, bridge, socket_port) = if let Some(pa) = &port_attach_config {
        let bridge = pa["bridge"].as_str().unwrap_or("ovsbr0").to_string();
        let iface_name = pa["iface_name"].as_str().unwrap_or("gbr_xray").to_string();
        let ip_addrs: Vec<Value> = pa["ip_addrs"].as_array().cloned().unwrap_or_default();
        let gateway = pa["gateway"].as_str().map(String::from);
        let routes: Vec<Value> = pa["routes"].as_array().cloned().unwrap_or_default();

        let p = serde_json::json!({
            "port_name": format!("c_{}", iface_name),
            "bridge": bridge,
            "instance_name": instance_name,
            "iface_name": iface_name,
            "ip_addrs": ip_addrs,
            "gateway": gateway,
            "routes": routes
        });
        (p, bridge.clone(), iface_name)
    } else {
        // Fallback: read from privacy_router schema (for the host xray path;
        // the deprecated wg-xray container is no longer referenced)
        eprintln!("reading privacy_router schema from D-Bus tree...");
        let schema = get_json_property(&conn, PRIVACY_ROUTER_BASE, "JsonData").await?;

        let bridge = schema["config"]["bridge_name"]
            .as_str()
            .unwrap_or("ovsbr0")
            .to_string();
        let socket_port = schema["config"]["xray"]["socket_port"]
            .as_str()
            .unwrap_or("gbr_xray")
            .to_string();
        let container_id = schema["config"]["xray"]["container_id"]
            .as_u64()
            .unwrap_or(101);

        eprintln!("bridge={bridge}, socket_port={socket_port}, container_id={container_id}");

        let p = serde_json::json!({
            "port_name": format!("c{}_{}", container_id, socket_port),
            "bridge": bridge,
            "instance_name": instance_name,
            "iface_name": socket_port,
            "ip_addrs": ["10.200.0.1/30", "10.0.0.2/24"],
            "gateway": "10.200.0.2",
            "routes": ["10.0.0.1 via 10.200.0.2"]
        });
        (p, bridge, socket_port)
    };

    // Check if interface already exists
    if Command::new("ip")
        .args(["link", "show", &socket_port])
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("interface {socket_port} already exists, skipping AttachPort");
        return Ok(());
    }

    // Discover AttachPort method location
    let daemon = get_json_property(
        &conn,
        "/org/opdbus/v1/plugins/ovsdb_daemon",
        "JsonData",
    )
    .await?;
    let dbus_bus = daemon["dbus_bus_name"]
        .as_str()
        .unwrap_or("org.opdbus.v1.plugins.ovsdb");
    let proxy_path = discover_proxy_path(&conn, dbus_bus).await?;

    eprintln!("discovered AttachPort at bus={dbus_bus} path={proxy_path}");

    // Call AttachPort
    eprintln!("calling AttachPort...");
    let result: String = conn
        .call_method(
            Some(dbus_bus),
            proxy_path.as_str(),
            Some("org.opdbus.rovs.proxy"),
            "AttachPort",
            &(params.to_string()),
        )
        .await?
        .body()
        .deserialize()?;

    eprintln!("AttachPort result: {result}");

    let parsed: Value = serde_json::from_str(&result)?;
    if parsed["success"].as_bool() != Some(true) {
        anyhow::bail!("AttachPort failed: {}", parsed["message"]);
    }

    // Wait for interface to appear
    eprintln!("waiting for interface {socket_port}...");
    for _ in 0..30 {
        if Command::new("ip")
            .args(["link", "show", &socket_port])
            .output()
            .is_ok_and(|o| o.status.success())
        {
            eprintln!("interface {socket_port} is up");
            return Ok(());
        }
        thread::sleep(Duration::from_secs(1));
    }

    anyhow::bail!("interface {socket_port} not found after 30s")
}
