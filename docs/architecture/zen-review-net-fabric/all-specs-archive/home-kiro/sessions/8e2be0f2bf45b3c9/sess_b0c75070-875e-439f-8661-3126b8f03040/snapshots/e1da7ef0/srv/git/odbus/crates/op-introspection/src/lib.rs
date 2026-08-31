//! op-introspection: D-Bus introspection service (stubbed — pending port)

use anyhow::Result;
use std::sync::Arc;
pub use op_core::types::{BusType, InterfaceInfo, ObjectInfo, ServiceInfo};

/// Introspection cache (stub)
#[derive(Clone)]
pub struct IntrospectionCache;

/// Service scanner (stub)
#[derive(Clone)]
pub struct ServiceScanner;

impl ServiceScanner {
    pub fn new() -> Self { Self }
}

impl Default for ServiceScanner {
    fn default() -> Self { Self::new() }
}

/// Main introspection service
#[derive(Clone)]
pub struct IntrospectionService;

impl IntrospectionService {
    pub fn new() -> Self { Self }

    pub async fn list_services(&self, _bus_type: BusType) -> Result<Vec<ServiceInfo>> {
        Ok(Vec::new())
    }

    pub async fn list_services_json(&self, _bus_type: BusType) -> Result<serde_json::Value> {
        Ok(serde_json::json!([]))
    }

    pub async fn introspect(
        &self,
        _bus_type: BusType,
        _service: &str,
        _path: &str,
    ) -> Result<ObjectInfo> {
        Ok(ObjectInfo {
            path: String::new(),
            interfaces: Vec::new(),
            children: Vec::new(),
        })
    }

    pub async fn introspect_json(
        &self,
        _bus_type: BusType,
        _service: &str,
        _path: &str,
    ) -> Result<serde_json::Value> {
        Ok(serde_json::json!({}))
    }

    pub fn cache(&self) -> Arc<IntrospectionCache> {
        Arc::new(IntrospectionCache)
    }
}

impl Default for IntrospectionService {
    fn default() -> Self { Self::new() }
}
