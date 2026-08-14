//! Safe OVS datapath attach helpers (plugin contract).
//!
//! Source of truth for assumptions:
//! <https://docs.openvswitch.org/en/latest/topics/design/>
//!
//! # Why attach without fallback black-holes the host
//!
//! Host L3 (SSH/pub0) must **never** depend on `OFPR_NO_MATCH` → PACKET_IN →
//! controller flow install. Keep cookied `priority=0,actions=NORMAL` present.
//!
//! # No CLI shell-outs
//!
//! This module talks to OVS only via:
//! - OVSDB JSON-RPC (`rovs_ovsdb` over `unix:.../db.sock`) for Bridge/Controller
//! - OpenFlow over the bridge management socket (`unix:.../<bridge>.mgmt`) for
//!   FlowMod / flow-stats (replaces forbidden `ovs-ofctl` / `ovs-vsctl`)
//!
//! The live `op-of-controller` also re-installs NORMAL on every OVS reconnect.
//!
use anyhow::{bail, Context, Result};
use rovs_openflow::{ActionList, Flow, Match, VConn};
use rovs_transport::Address;
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

/// Cookie for priority=0 NORMAL host-safety fallback (never delete-all this).
pub const FALLBACK_COOKIE: u64 = 0x3344_4348_0000_0001; // "3DCH"+1
/// Cookie for controller-installed pair / schema flows.
pub const MANAGED_COOKIE: u64 = 0x3344_4348_0000_0002;
const FALLBACK_PRIORITY: u16 = 0;
/// Datapath cache revalidation window (OVS design: usually within ~1s).
const DATAPATH_SETTLE: Duration = Duration::from_millis(1200);

const DEFAULT_OVSDB_ADDR: &str = "unix:/var/run/openvswitch/db.sock";

fn ovsdb_addr() -> String {
    std::env::var("OVSDB_SOCKET_ADDR").unwrap_or_else(|_| DEFAULT_OVSDB_ADDR.to_string())
}

fn bridge_mgmt_addr(bridge: &str) -> Address {
    let path = std::env::var("OVS_BRIDGE_MGMT_SOCK")
        .unwrap_or_else(|_| format!("/var/run/openvswitch/{bridge}.mgmt"));
    Address::Unix(PathBuf::from(path))
}

async fn ovsdb_connect() -> Result<rovs_ovsdb::Client> {
    let addr = ovsdb_addr();
    rovs_ovsdb::Client::connect(&addr)
        .await
        .with_context(|| format!("OVSDB connect {addr}"))
}

async fn ovsdb_transact(
    client: &mut rovs_ovsdb::Client,
    ops: Vec<serde_json::Value>,
) -> Result<serde_json::Value> {
    let result = client
        .transact(serde_json::Value::Array(ops))
        .await
        .context("OVSDB transact failed")?;
    if let Some(arr) = result.as_array() {
        for (i, entry) in arr.iter().enumerate() {
            if let Some(err) = entry.get("error") {
                bail!(
                    "OVSDB op[{i}] error: {err} detail={}",
                    entry.get("details").unwrap_or(&serde_json::Value::Null)
                );
            }
        }
    }
    Ok(result)
}

fn first_rows(result: &serde_json::Value) -> &[serde_json::Value] {
    result
        .as_array()
        .and_then(|a| a.first())
        .and_then(|e| e.get("rows"))
        .and_then(|r| r.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

fn json_str_field(row: &serde_json::Value, key: &str) -> Option<String> {
    let v = row.get(key)?;
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        // OVSDB sometimes wraps enums as ["set",[]] or bare strings
        serde_json::Value::Array(a) if a.len() == 2 && a[0] == "set" => None,
        other => Some(other.to_string().trim_matches('"').to_string()),
    }
}

fn uuid_from_cell(cell: &serde_json::Value) -> Option<String> {
    cell.as_array()
        .filter(|a| a.len() == 2 && a[0] == "uuid")
        .and_then(|a| a[1].as_str())
        .map(str::to_string)
}

fn collect_uuids(cell: &serde_json::Value, out: &mut Vec<String>) {
    if let Some(u) = uuid_from_cell(cell) {
        out.push(u);
        return;
    }
    if let Some(arr) = cell.as_array() {
        if arr.len() == 2 && arr[0] == "set" {
            if let Some(set) = arr[1].as_array() {
                for item in set {
                    collect_uuids(item, out);
                }
            }
        } else {
            for item in arr {
                collect_uuids(item, out);
            }
        }
    }
}

fn fallback_rovs_flow() -> Flow {
    let mut flow = Flow::add()
        .priority(FALLBACK_PRIORITY)
        .cookie(FALLBACK_COOKIE)
        .match_fields(Match::new())
        .actions(ActionList::new().normal());
    flow.flags.no_pkt_counts = true;
    flow.flags.no_byte_counts = true;
    flow
}

async fn with_bridge_of<T, F>(bridge: &str, f: F) -> Result<T>
where
    F: for<'a> FnOnce(
        &'a mut VConn,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<T>> + Send + 'a>,
    >,
{
    let addr = bridge_mgmt_addr(bridge);
    let mut vconn = VConn::connect(&addr)
        .await
        .with_context(|| format!("OpenFlow connect to {addr}"))?;
    f(&mut vconn).await
}

/// Idempotently install cookied priority=0 NORMAL via the bridge .mgmt OF socket.
pub async fn ensure_fallback_normal(bridge: &str) -> Result<()> {
    with_bridge_of(bridge, |vconn| {
        Box::pin(async move {
            vconn
                .send_flow_sync(&fallback_rovs_flow())
                .await
                .context("FlowMod ADD NORMAL via bridge.mgmt")?;
            Ok(())
        })
    })
    .await?;

    if !fallback_present(bridge).await? {
        bail!("EnsureFallbackNormal: NORMAL fallback not present on {bridge} after FlowMod");
    }
    log::info!(
        "EnsureFallbackNormal: cookie={FALLBACK_COOKIE:#x},priority={FALLBACK_PRIORITY},actions=NORMAL present on {bridge}"
    );
    Ok(())
}

pub async fn fallback_present(bridge: &str) -> Result<bool> {
    with_bridge_of(bridge, |vconn| {
        Box::pin(async move {
            let flows = vconn
                .dump_flows()
                .await
                .context("OF dump_flows via bridge.mgmt")?;
            Ok(flows
                .iter()
                .any(|e| e.cookie == FALLBACK_COOKIE && e.priority == FALLBACK_PRIORITY))
        })
    })
    .await
}

pub async fn set_fail_mode(bridge: &str, mode: &str) -> Result<()> {
    match mode {
        "standalone" | "secure" => {}
        other => bail!("fail_mode must be standalone|secure, got {other}"),
    }
    let mut client = ovsdb_connect().await?;
    ovsdb_transact(
        &mut client,
        vec![serde_json::json!({
            "op": "update",
            "table": "Bridge",
            "where": [["name", "==", bridge]],
            "row": { "fail_mode": mode }
        })],
    )
    .await?;
    log::info!("SetFailMode: {bridge} fail_mode={mode}");
    Ok(())
}

/// Keep in-band for bridge-local OF endpoints (design: hidden remotes).
pub async fn ensure_controller_in_band(bridge: &str) -> Result<()> {
    let mut client = ovsdb_connect().await?;

    // Clear disable-in-band if present.
    let br = ovsdb_transact(
        &mut client,
        vec![serde_json::json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge]],
            "columns": ["other_config", "controller"]
        })],
    )
    .await?;
    let rows = first_rows(&br);
    let Some(row) = rows.first() else {
        bail!("Bridge '{bridge}' not found");
    };

    if let Some(other) = row.get("other_config") {
        // other_config is ["map", [[k,v], ...]]
        if let Some(arr) = other.as_array() {
            if arr.len() == 2 && arr[0] == "map" {
                if let Some(pairs) = arr[1].as_array() {
                    let has_disable = pairs.iter().any(|p| {
                        p.as_array()
                            .and_then(|a| a.first())
                            .and_then(|k| k.as_str())
                            == Some("disable-in-band")
                    });
                    if has_disable {
                        let _ = ovsdb_transact(
                            &mut client,
                            vec![serde_json::json!({
                                "op": "mutate",
                                "table": "Bridge",
                                "where": [["name", "==", bridge]],
                                "mutations": [[
                                    "other_config",
                                    "delete",
                                    ["map", [["disable-in-band", "true"]]]
                                ]]
                            })],
                        )
                        .await;
                        let _ = ovsdb_transact(
                            &mut client,
                            vec![serde_json::json!({
                                "op": "mutate",
                                "table": "Bridge",
                                "where": [["name", "==", bridge]],
                                "mutations": [[
                                    "other_config",
                                    "delete",
                                    ["map", [["disable-in-band", "false"]]]
                                ]]
                            })],
                        )
                        .await;
                    }
                }
            }
        }
    }

    let mut ctrl_uuids = Vec::new();
    if let Some(ctrl) = row.get("controller") {
        collect_uuids(ctrl, &mut ctrl_uuids);
    }
    for uuid in ctrl_uuids {
        let _ = ovsdb_transact(
            &mut client,
            vec![serde_json::json!({
                "op": "update",
                "table": "Controller",
                "where": [["_uuid", "==", ["uuid", uuid]]],
                "row": { "connection_mode": "in-band" }
            })],
        )
        .await;
    }
    Ok(())
}

pub async fn del_controller(bridge: &str) -> Result<()> {
    let mut client = ovsdb_connect().await?;
    // Clear Bridge.controller set (orphaned Controller rows are GC'd by ovsdb).
    ovsdb_transact(
        &mut client,
        vec![serde_json::json!({
            "op": "update",
            "table": "Bridge",
            "where": [["name", "==", bridge]],
            "row": { "controller": ["set", []] }
        })],
    )
    .await?;
    log::info!("DelController: {bridge}");
    Ok(())
}

pub async fn set_controller(bridge: &str, endpoint: &str) -> Result<()> {
    ensure_fallback_normal(bridge).await?;
    let ep = if endpoint.starts_with("tcp:") || endpoint.starts_with("unix:") {
        endpoint.to_string()
    } else {
        format!("tcp:{endpoint}")
    };

    let mut client = ovsdb_connect().await?;

    // Insert Controller row, then point Bridge.controller at it.
    let result = ovsdb_transact(
        &mut client,
        vec![
            serde_json::json!({
                "op": "insert",
                "table": "Controller",
                "row": {
                    "target": ep,
                    "connection_mode": "in-band"
                },
                "uuid-name": "new_ctrl"
            }),
            serde_json::json!({
                "op": "update",
                "table": "Bridge",
                "where": [["name", "==", bridge]],
                "row": {
                    "controller": ["set", [["named-uuid", "new_ctrl"]]]
                }
            }),
        ],
    )
    .await?;
    let _ = result;

    ensure_controller_in_band(bridge).await?;
    log::info!("SetController: {bridge} -> {ep} (connection_mode=in-band)");
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct DatapathHealth {
    pub bridge: String,
    pub fail_mode: String,
    pub controllers: Vec<String>,
    pub fallback_normal: bool,
    pub fallback_priority: u16,
    pub fallback_cookie: String,
    pub connection_mode: String,
    pub disable_in_band: bool,
}

pub async fn get_datapath_health(bridge: &str) -> Result<DatapathHealth> {
    let mut client = ovsdb_connect().await?;
    let br = ovsdb_transact(
        &mut client,
        vec![serde_json::json!({
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge]],
            "columns": ["fail_mode", "controller", "other_config"]
        })],
    )
    .await?;
    let rows = first_rows(&br);
    let Some(row) = rows.first() else {
        bail!("Bridge '{bridge}' not found");
    };

    let fail_mode = json_str_field(row, "fail_mode").unwrap_or_default();

    let mut ctrl_uuids = Vec::new();
    if let Some(ctrl) = row.get("controller") {
        collect_uuids(ctrl, &mut ctrl_uuids);
    }

    let mut controllers = Vec::new();
    let mut connection_mode = "n/a".to_string();
    for uuid in &ctrl_uuids {
        let c = ovsdb_transact(
            &mut client,
            vec![serde_json::json!({
                "op": "select",
                "table": "Controller",
                "where": [["_uuid", "==", ["uuid", uuid]]],
                "columns": ["target", "connection_mode"]
            })],
        )
        .await?;
        if let Some(crow) = first_rows(&c).first() {
            if let Some(t) = json_str_field(crow, "target") {
                controllers.push(t);
            }
            if let Some(m) = json_str_field(crow, "connection_mode") {
                connection_mode = m;
            }
        }
    }

    let mut disable_in_band = false;
    if let Some(other) = row.get("other_config").and_then(|v| v.as_array()) {
        if other.len() == 2 && other[0] == "map" {
            if let Some(pairs) = other[1].as_array() {
                disable_in_band = pairs.iter().any(|p| {
                    let a = p.as_array();
                    matches!(a, Some(a) if a.len() == 2
                        && a[0].as_str() == Some("disable-in-band")
                        && a[1].as_str() == Some("true"))
                });
            }
        }
    }

    let fallback_normal = fallback_present(bridge).await?;

    Ok(DatapathHealth {
        bridge: bridge.to_string(),
        fail_mode,
        controllers,
        fallback_normal,
        fallback_priority: FALLBACK_PRIORITY,
        fallback_cookie: format!("{FALLBACK_COOKIE:#x}"),
        connection_mode,
        disable_in_band,
    })
}

pub async fn attach_controller_safe(bridge: &str, endpoint: &str) -> Result<DatapathHealth> {
    set_fail_mode(bridge, "standalone").await?;
    ensure_fallback_normal(bridge).await?;

    if let Err(e) = set_controller(bridge, endpoint).await {
        let _ = del_controller(bridge).await;
        let _ = ensure_fallback_normal(bridge).await;
        return Err(e).context("AttachControllerSafe: set-controller failed; rolled back");
    }

    // Wait for datapath cache revalidation + controller connect wipe.
    tokio::time::sleep(DATAPATH_SETTLE).await;
    if let Err(e) = ensure_fallback_normal(bridge).await {
        let _ = del_controller(bridge).await;
        let _ = ensure_fallback_normal(bridge).await;
        return Err(e).context("AttachControllerSafe: fallback missing after settle; rolled back");
    }

    // Second check after another short window (cache lag).
    tokio::time::sleep(Duration::from_millis(400)).await;
    let health = get_datapath_health(bridge).await?;
    if !health.fallback_normal || health.fail_mode != "standalone" || health.disable_in_band {
        let _ = del_controller(bridge).await;
        let _ = ensure_fallback_normal(bridge).await;
        bail!(
            "AttachControllerSafe: health failed fallback={} fail_mode={} disable_in_band={}; rolled back",
            health.fallback_normal,
            health.fail_mode,
            health.disable_in_band
        );
    }
    log::info!(
        "AttachControllerSafe: ok bridge={} controllers={:?}",
        bridge,
        health.controllers
    );
    Ok(health)
}
