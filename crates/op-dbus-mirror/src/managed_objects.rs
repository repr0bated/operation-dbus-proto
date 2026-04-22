//! org.freedesktop.DBus.ObjectManager implementation
//!
//! Provides `GetManagedObjects` so any D-Bus client can enumerate every object
//! created by a plugin in a single round-trip call.
//!
//! The interface is registered at `/org/opdbus/v1/plugins`.  Every plugin
//! object published under that path is reflected in the registry; the
//! `InterfacesAdded` / `InterfacesRemoved` signals are emitted as objects
//! come and go.
//!
//! D-Bus signature of GetManagedObjects: `a{oa{sa{sv}}}`
//!   ObjectPath  →  interface-name  →  property-name  →  variant

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use zbus::interface;
use zbus::zvariant::OwnedObjectPath;

/// Properties for a single interface: `a{ss}` — property name → JSON-encoded value.
/// Using String instead of OwnedValue keeps the type Clone + simple.
pub type PropertyMap = HashMap<String, String>;

/// All interfaces (with their properties) for one object: `a{sa{ss}}`
pub type InterfaceMap = HashMap<String, PropertyMap>;

/// Registry: ObjectPath → InterfaceMap.  Shared between DbusMirror and the
/// ObjectManagerInterface so writes are visible immediately to readers.
pub type ManagedObjectRegistry = Arc<DashMap<OwnedObjectPath, InterfaceMap>>;

/// D-Bus path where the ObjectManager is registered.
pub const OBJECT_MANAGER_PATH: &str = "/org/opdbus/v1";

/// Interface name exposed on every projected plugin object.
pub const PROJECTED_IFACE: &str = "org.opdbus.ProjectedObjectV1";

// ── Interface ──────────────────────────────────────────────────────────────

pub struct ObjectManagerInterface {
    registry: ManagedObjectRegistry,
}

impl ObjectManagerInterface {
    pub fn new(registry: ManagedObjectRegistry) -> Self {
        Self { registry }
    }
}

#[interface(name = "org.freedesktop.DBus.ObjectManager")]
impl ObjectManagerInterface {
    /// Return every managed object with all their interface properties.
    fn get_managed_objects(&self) -> HashMap<OwnedObjectPath, InterfaceMap> {
        self.registry
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect()
    }

    /// Emitted when a new object (or new interfaces on an existing object)
    /// appears under this manager.
    #[zbus(signal)]
    pub async fn interfaces_added(
        ctxt: &zbus::SignalContext<'_>,
        object_path: OwnedObjectPath,
        interfaces_and_properties: InterfaceMap,
    ) -> zbus::Result<()>;

    /// Emitted when an object (or some of its interfaces) is removed.
    #[zbus(signal)]
    pub async fn interfaces_removed(
        ctxt: &zbus::SignalContext<'_>,
        object_path: OwnedObjectPath,
        interfaces: Vec<String>,
    ) -> zbus::Result<()>;
}

// ── Helper ─────────────────────────────────────────────────────────────────

/// Build the `InterfaceMap` for a plugin object whose state is a raw JSON blob.
///
/// The single interface `org.opdbus.ProjectedObjectV1` is exposed with a
/// `JsonData` property that carries the serialised JSON.
pub fn build_interface_map(json_str: &str) -> InterfaceMap {
    let mut props = PropertyMap::new();
    props.insert("JsonData".to_string(), json_str.to_string());
    let mut iface_map = InterfaceMap::new();
    iface_map.insert(PROJECTED_IFACE.to_string(), props);
    iface_map
}
