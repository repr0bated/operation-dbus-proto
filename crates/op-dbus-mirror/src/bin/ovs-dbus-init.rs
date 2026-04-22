use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::os::fd::FromRawFd;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use zbus::{connection::Builder, interface};

const DEFAULT_OVSDB_SOCKET: &str = "/var/run/openvswitch/db.sock";
const RUN_OVSDB_SOCKET: &str = "/run/openvswitch/db.sock";
const DEFAULT_BUS_NAME: &str = "org.opdbus.bridge";
const LEGACY_BUS_NAME: &str = "org.opdbus";
const PATH_PREFIX: &str = "/org/opdbus/bridge";

#[derive(Debug, Clone)]
struct BridgeRow {
    name: String,
    uuid: String,
    datapath_type: String,
    other_config: HashMap<String, String>,
    external_ids: HashMap<String, String>,
}

struct BridgeObject {
    row: BridgeRow,
}

impl BridgeObject {
    fn new(row: BridgeRow) -> Self {
        Self { row }
    }
}

#[interface(name = "org.opdbus.Bridge")]
impl BridgeObject {
    async fn get_name(&self) -> String {
        self.row.name.clone()
    }

    async fn get_uuid(&self) -> String {
        self.row.uuid.clone()
    }

    async fn get_datapath_type(&self) -> String {
        self.row.datapath_type.clone()
    }

    async fn get_other_config(&self) -> HashMap<String, String> {
        self.row.other_config.clone()
    }

    async fn get_external_ids(&self) -> HashMap<String, String> {
        self.row.external_ids.clone()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG").unwrap_or_else(|_| "ovs_dbus_init=info,info".to_string()),
        )
        .init();

    let ovsdb_socket = ovsdb_socket();
    let bus_name = env::var("OP_DBUS_OVS_BRIDGE_DEST").unwrap_or_else(|_| DEFAULT_BUS_NAME.into());
    if bus_name == LEGACY_BUS_NAME {
        tracing::warn!(
            bus_name,
            "legacy org.opdbus ownership requested; this can conflict with op-dbus"
        );
    }

    tracing::info!(socket = %ovsdb_socket, bus_name, "starting Rust ovs-dbus-init");
    let bridges = fetch_bridges(&ovsdb_socket).await.unwrap_or_else(|error| {
        tracing::warn!(%error, "OVSDB bridge discovery failed; publishing empty bridge set");
        Vec::new()
    });

    let connection = Builder::session()?
        .name(bus_name.as_str())?
        .build()
        .await
        .context("failed to connect to the configured D-Bus session bus")?;

    for bridge in bridges {
        let path = format!("{PATH_PREFIX}/{}", sanitize_path_segment(&bridge.name));
        tracing::info!(name = bridge.name, path, "publishing OVS bridge object");
        connection
            .object_server()
            .at(path.as_str(), BridgeObject::new(bridge))
            .await
            .with_context(|| format!("failed to publish bridge object at {path}"))?;
    }

    signal_dinit_ready();
    tracing::info!("ovs-dbus-init ready");
    std::future::pending::<()>().await;
    Ok(())
}

fn ovsdb_socket() -> String {
    env::var("OVSDB_SOCK").unwrap_or_else(|_| {
        if fs::metadata(RUN_OVSDB_SOCKET).is_ok() {
            RUN_OVSDB_SOCKET.into()
        } else {
            DEFAULT_OVSDB_SOCKET.into()
        }
    })
}

async fn fetch_bridges(socket_path: &str) -> Result<Vec<BridgeRow>> {
    let select = json!({
        "op": "select",
        "table": "Bridge",
        "where": [],
        "columns": ["name", "datapath_type", "other_config", "external_ids", "_uuid"]
    });
    let reply = ovsdb_transact(socket_path, select).await?;
    let rows = reply
        .get("result")
        .and_then(Value::as_array)
        .and_then(|result| result.first())
        .and_then(|table| table.get("rows"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(rows
        .into_iter()
        .map(|row| BridgeRow {
            name: row
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            uuid: ovsdb_uuid(row.get("_uuid")).unwrap_or_default(),
            datapath_type: row
                .get("datapath_type")
                .and_then(Value::as_str)
                .unwrap_or("system")
                .to_string(),
            other_config: ovsdb_map(row.get("other_config")),
            external_ids: ovsdb_map(row.get("external_ids")),
        })
        .collect())
}

async fn ovsdb_transact(socket_path: &str, operation: Value) -> Result<Value> {
    let request = json!({
        "method": "transact",
        "params": ["Open_vSwitch", operation],
        "id": 1
    });
    let raw = serde_json::to_vec(&request)?;
    let mut stream = UnixStream::connect(socket_path)
        .await
        .with_context(|| format!("failed to connect to OVSDB socket {socket_path}"))?;
    stream.write_all(&raw).await?;

    let mut buffer = Vec::new();
    let mut chunk = vec![0u8; 65536];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Ok(value) = serde_json::from_slice::<Value>(&buffer) {
            return Ok(value);
        }
    }

    Err(anyhow!("OVSDB socket closed before a complete JSON reply"))
}

fn ovsdb_uuid(value: Option<&Value>) -> Option<String> {
    let parts = value?.as_array()?;
    if parts.len() == 2 && parts.first()?.as_str()? == "uuid" {
        return parts.get(1)?.as_str().map(ToString::to_string);
    }
    None
}

fn ovsdb_map(value: Option<&Value>) -> HashMap<String, String> {
    let Some(parts) = value.and_then(Value::as_array) else {
        return HashMap::new();
    };
    if parts.len() != 2 || parts.first().and_then(Value::as_str) != Some("map") {
        return HashMap::new();
    }

    parts
        .get(1)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|pair| {
            let pair = pair.as_array()?;
            let key = pair.first()?.as_str()?.to_string();
            let value = pair.get(1)?.as_str().unwrap_or_default().to_string();
            Some((key, value))
        })
        .collect()
}

fn sanitize_path_segment(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "unnamed".into()
    } else {
        sanitized
    }
}

fn signal_dinit_ready() {
    let Ok(fd) = env::var("DINIT_DBUS_READY_FD") else {
        return;
    };
    let Ok(fd) = fd.parse::<i32>() else {
        return;
    };

    // Dinit passes ownership of this pipe fd to the service process.
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let _ = file.write_all(b"\n");
}
