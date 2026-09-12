//! op-introspection: D-Bus introspection service (stubbed — pending StreamingSnowball port)

pub mod projection;

/// Stub introspection service. Real implementation pending snowball API port.
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
