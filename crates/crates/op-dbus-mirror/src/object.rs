//! Mirror Object D-Bus Interface

use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use zbus::interface;

/// A generic D-Bus object representing a database row
pub struct MirrorObject {
    data: Value,
}

impl MirrorObject {
    pub fn new(data: Value) -> Self {
        Self { data }
    }

    pub fn update_data(&mut self, new_data: Value) -> bool {
        if self.data == new_data {
            return false;
        }
        tracing::debug!("Updating MirrorObject data");
        self.data = new_data;
        true
    }
}

#[interface(name = "org.opdbus.ProjectedObjectV1")]
impl MirrorObject {
    /// Get the full JSON representation of the row
    #[zbus(property)]
    async fn json_data(&self) -> String {
        simd_json::to_string(&self.data).unwrap_or_default()
    }

    /// Get a specific property value by key
    async fn get_property(&self, key: String) -> String {
        self.data
            .get(&key)
            .map(|v| simd_json::to_string(v).unwrap_or_default())
            .unwrap_or_default()
    }

    /// Signal emitted when json_data changes
    #[zbus(signal)]
    pub async fn data_updated(&self, ctxt: &zbus::SignalContext<'_>) -> zbus::Result<()>;
}
