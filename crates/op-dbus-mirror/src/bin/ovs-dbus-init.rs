use anyhow::{Context, Result};
use op_core::types::BusType;
use op_dbus_mirror::DbusMirror;
use op_network::rovs_proxy::OvsdbDbusClient;
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG").unwrap_or_else(|_| "ovs_dbus_init=info,info".to_string()),
        )
        .init();

    let bus_type = match env::var("OP_DBUS_MIRROR_BUS")
        .unwrap_or_else(|_| "system".to_string())
        .as_str()
    {
        "session" => BusType::Session,
        _ => BusType::System,
    };

    tracing::info!(bus = %bus_type, "starting op-dbus-mirror (event-driven)");

    let ovsdb = Arc::new(OvsdbDbusClient::new());

    let mirror = DbusMirror::new(bus_type, ovsdb, None)
        .await
        .context("failed to create DbusMirror")?;

    Arc::new(mirror)
        .start()
        .await
        .context("DbusMirror event loop exited")?;

    Ok(())
}
