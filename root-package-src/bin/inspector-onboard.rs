use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use op_dbus::inspector_gadget::{InspectorConfig, InspectorGadget};
use op_plugins::PluginCatalog;
use op_state_store::{SqliteStore, StateStore};
use op_tools::ToolRegistry;

#[tokio::main]
async fn main() -> Result<()> {
    let database_url = std::env::var("OP_DBUS_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:///var/lib/op-dbus/state.db".to_string());
    let cache_dir =
        std::env::var("OP_DBUS_CACHE_DIR").unwrap_or_else(|_| "/var/lib/op-dbus/cache".to_string());

    if let Some(db_path) = database_url.strip_prefix("sqlite://") {
        if db_path != ":memory:" {
            if let Some(parent) = std::path::Path::new(db_path).parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
    }
    tokio::fs::create_dir_all(&cache_dir).await?;

    let sqlite_store = SqliteStore::new(&database_url).await?;
    let state_store: Arc<dyn StateStore> = Arc::new(sqlite_store);

    let plugin_dir = PathBuf::from(&cache_dir).join("plugins");
    tokio::fs::create_dir_all(&plugin_dir).await?;
    let plugin_catalog = Arc::new(PluginCatalog::new(&plugin_dir));

    let tool_registry = Arc::new(ToolRegistry::new());

    let inspector = InspectorGadget::new(
        InspectorConfig::default(),
        state_store.clone(),
        tool_registry.clone(),
    );


    let discovery = inspector.discover().await?;
    println!("discovery_id={}", discovery.discovery_id);
    println!("objects_total={}", discovery.stats.total_objects);
    println!("new_schemas={}", discovery.stats.new_schemas);
    println!("warnings={}", discovery.warnings.len());

    for (object_type, objects) in &discovery.objects {
        println!("  {}={}", object_type, objects.len());
    }

    let import_result = inspector.import(&discovery).await?;
    println!("imported={}", import_result.imported);
    println!("skipped={}", import_result.skipped);
    println!("import_errors={}", import_result.errors.len());

    if !import_result.errors.is_empty() {
        for err in import_result.errors.iter().take(20) {
            println!("  error: {}", err);
        }
    }

    Ok(())
}
