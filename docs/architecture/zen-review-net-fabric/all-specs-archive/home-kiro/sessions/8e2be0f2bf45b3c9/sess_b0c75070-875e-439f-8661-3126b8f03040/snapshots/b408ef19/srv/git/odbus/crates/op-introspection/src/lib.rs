//! op-introspection: D-Bus introspection service (stubbed — pending StreamingSnowball port)

/// Stub introspection service.
#[derive(Clone)]
pub struct IntrospectionService;

impl IntrospectionService {
    pub fn new() -> Self { Self }

    pub async fn introspect(&self, _bus: &str, _service: &str, _path: &str) -> anyhow::Result<String> {
        Ok(String::new())
    }

    pub async fn list_services(&self) -> Vec<String> {
        Vec::new()
    }

    pub async fn list_services_json(&self) -> serde_json::Value {
        serde_json::json!([])
    }
}

impl Default for IntrospectionService {
    fn default() -> Self { Self::new() }
}

/// Stub service scanner.
#[derive(Clone)]
pub struct ServiceScanner;

impl ServiceScanner {
    pub fn new() -> Self { Self }
}

impl Default for ServiceScanner {
    fn default() -> Self { Self::new() }
}
