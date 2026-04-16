//! D-Bus interface for the publication service.

use crate::DbusMirror;
use std::sync::Arc;
use zbus::interface;

pub struct DbusMirrorInterface {
    mirror: Arc<DbusMirror>,
}

impl DbusMirrorInterface {
    pub fn new(mirror: Arc<DbusMirror>) -> Self {
        Self { mirror }
    }
}

#[interface(name = "org.opdbus.MirrorV1")]
impl DbusMirrorInterface {
    /// Publish a fresh snapshot from authoritative stores.
    async fn publish_snapshot(&self) -> zbus::fdo::Result<()> {
        self.mirror
            .publish_snapshot()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Compatibility alias for older callers that still use the old term.
    async fn reconcile(&self) -> zbus::fdo::Result<()> {
        self.publish_snapshot().await
    }

    /// Get current publication statistics.
    async fn get_stats(&self) -> zbus::fdo::Result<String> {
        let stats = simd_json::json!({
            "published_objects": self.mirror.published_count(),
            "projected_objects": self.mirror.projected_count(),
        });
        Ok(simd_json::to_string(&stats).unwrap_or_default())
    }

    /// Get list of all published object paths.
    async fn list_paths(&self) -> zbus::fdo::Result<Vec<String>> {
        Ok(self.mirror.list_published_paths())
    }
}
