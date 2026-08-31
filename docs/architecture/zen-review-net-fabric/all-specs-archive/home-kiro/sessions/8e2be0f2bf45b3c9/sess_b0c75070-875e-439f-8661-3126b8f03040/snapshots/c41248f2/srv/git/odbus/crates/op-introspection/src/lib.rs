//! op-introspection: D-Bus introspection service (stubbed — pending StreamingBlockchain port)

pub mod projection;

/// Stub introspection service. Real implementation pending blockchain API port.
#[derive(Clone)]
pub struct IntrospectionService;

impl IntrospectionService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for IntrospectionService {
    fn default() -> Self {
        Self::new()
    }
}
