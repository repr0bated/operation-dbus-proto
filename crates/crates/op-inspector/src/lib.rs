//! op-inspector: Inspector Gadget - Universal Object Inspector
//!
//! Features:
//! - Inspect ANY data structure (JSON, XML, binary, Docker, DBus, Proxmox)
//! - AI-powered gap filling for incomplete introspections
//! - Schema generation and validation
//! - Knowledge base integration
//! - Proxmox LXC template introspection (4500+ editable elements)
//! - GCloud CLI introspection (100+ command groups, all flags/args)

pub mod gcloud;
mod introspective_gadget;

// Re-export main types
pub use gcloud::{
    introspect_gcloud, GCloudArg, GCloudCommand, GCloudFlag, GCloudParser, GCloudSchema,
    GCloudStats,
};
pub use introspective_gadget::*;

use op_introspection::IntrospectionService;
use std::sync::Arc;

/// Simplified Inspector Gadget wrapper
pub struct InspectorGadget {
    introspection: Arc<IntrospectionService>,
}

impl InspectorGadget {
    pub fn new(introspection: Arc<IntrospectionService>) -> Self {
        Self { introspection }
    }

    pub fn introspection(&self) -> Arc<IntrospectionService> {
        Arc::clone(&self.introspection)
    }
}
