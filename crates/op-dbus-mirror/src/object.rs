//! Mirror Object D-Bus Interface

use op_state_store::PluginSchema;
use serde_json::Value;
use zbus::interface;

/// A generic D-Bus object representing a database row
pub struct MirrorObject {
    data: Value,
    schema: Option<PluginSchema>,
}

impl MirrorObject {
    pub fn new(data: Value) -> Self {
        Self { data, schema: None }
    }

    pub fn with_schema(data: Value, schema: Option<PluginSchema>) -> Self {
        Self { data, schema }
    }

    pub fn update_data(&mut self, new_data: Value) -> bool {
        if self.data == new_data {
            return false;
        }
        tracing::debug!("Updating MirrorObject data");
        self.data = new_data;
        true
    }

    fn schema_json_value(&self, selector: SchemaSurface) -> String {
        let Some(schema) = &self.schema else {
            return "{}".to_string();
        };

        let value = match selector {
            SchemaSurface::Schema => serde_json::to_value(schema),
            SchemaSurface::Methods => serde_json::to_value(&schema.methods),
            SchemaSurface::Signals => serde_json::to_value(&schema.signals),
            SchemaSurface::Guarantees => serde_json::to_value(&schema.guarantees),
        };

        value
            .ok()
            .and_then(|value| serde_json::to_string(&value).ok())
            .unwrap_or_else(|| "{}".to_string())
    }
}

enum SchemaSurface {
    Schema,
    Methods,
    Signals,
    Guarantees,
}

#[interface(name = "org.opdbus.ProjectedObjectV1")]
impl MirrorObject {
    /// Get the full JSON representation of the row
    #[zbus(property)]
    async fn json_data(&self) -> String {
        serde_json::to_string(&self.data).unwrap_or_default()
    }

    /// Full PluginSchema JSON for schema-backed plugin objects.
    #[zbus(property)]
    async fn schema_json(&self) -> String {
        self.schema_json_value(SchemaSurface::Schema)
    }

    /// Declared method/capability surface for schema-backed plugin objects.
    #[zbus(property)]
    async fn methods(&self) -> String {
        self.schema_json_value(SchemaSurface::Methods)
    }

    /// Declared signal surface for schema-backed plugin objects.
    #[zbus(property)]
    async fn signals(&self) -> String {
        self.schema_json_value(SchemaSurface::Signals)
    }

    /// Declared guarantee surface for schema-backed plugin objects.
    #[zbus(property)]
    async fn guarantees(&self) -> String {
        self.schema_json_value(SchemaSurface::Guarantees)
    }

    /// Get a specific property value by key
    async fn get_property(&self, key: String) -> String {
        self.data
            .get(&key)
            .map(|v| serde_json::to_string(v).unwrap_or_default())
            .unwrap_or_default()
    }

    /// Signal emitted when json_data changes
    #[zbus(signal)]
    pub async fn data_updated(
        &self,
        _ctxt: &zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::Result<()>;
}

pub fn plugin_id_from_path(path: &str) -> Option<String> {
    let prefix = "/org/opdbus/v1/plugins/";
    path.strip_prefix(prefix)
        .and_then(|rest| rest.split('/').next())
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
}

pub fn read_plugin_schema(plugin_id: &str) -> Option<PluginSchema> {
    let bytes = std::fs::read("/dev/shm/live-schema.json").ok()?;
    let root = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    let schema = root
        .as_object()?
        .get(plugin_id)?
        .as_array()
        .and_then(|items| items.first())
        .or_else(|| root.as_object()?.get(plugin_id))?;
    serde_json::from_value::<PluginSchema>(schema.clone()).ok()
}
