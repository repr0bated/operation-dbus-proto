use anyhow::{Context, Result};
use op_core::types::BusType;
use op_dbus_mirror::DbusMirror;
use op_jsonrpc::nonnet::NonNetDb;
use op_network::ovsdb::OvsdbClient;
use op_plugins::default_registry::DefaultPluginRegistry;
use op_state::manager::StateManager;
use op_state_store::SqliteStore;
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

    let ovsdb = Arc::new(OvsdbClient::new());
    let nonnet = Arc::new(NonNetDb::new());
    let state_manager = Arc::new(StateManager::new());

    let state_store = Arc::new(
        SqliteStore::in_memory()
            .await
            .context("failed to create op-dbus-mirror in-memory state store")?,
    );
    let plugin_registry = DefaultPluginRegistry::new(state_store);
    let plugins = plugin_registry
        .load_default_plugins()
        .await
        .context("failed to load default PluginSchema-backed plugins")?;

    for plugin in plugins {
        state_manager.register_plugin(plugin.name().to_string(), plugin);
    }

    let plugin_state = state_manager
        .query_current_state()
        .await
        .context("failed to query PluginSchema-backed state for NonNet")?;
    nonnet.load_from_plugins(&plugin_state).await;

    let mirror = DbusMirror::new(bus_type, ovsdb, nonnet, None)
        .await
        .context("failed to create DbusMirror")?
        .with_state_manager(state_manager);

    Arc::new(mirror)
        .start()
        .await
        .context("DbusMirror event loop exited")?;

    Ok(())
}
