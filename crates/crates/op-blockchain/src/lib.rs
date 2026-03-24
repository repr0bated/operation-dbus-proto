//! op-blockchain: Streaming blockchain with BTRFS subvolumes
//!
//! This crate provides:
//! - Streaming blockchain for audit trails
//! - Plugin footprints for change tracking
//! - Dual BTRFS subvolumes (timing/vectors/state)
//! - Automatic snapshots with configurable intervals
//! - Rolling retention policies
//! - btrfs send/receive for replication

pub mod blockchain;
pub mod btrfs_numa_integration;
pub mod footprint;
pub mod plugin_footprint;
pub mod retention;
pub mod snapshot;
pub mod streaming_blockchain;

// Re-export main types
pub use blockchain::StreamingBlockchain;
pub use footprint::{BlockEvent, PluginFootprint};
pub use retention::RetentionPolicy;
pub use snapshot::SnapshotInterval;

// Also export from plugin_footprint for compatibility
pub use plugin_footprint::PluginFootprint as LegacyPluginFootprint;

/// Prelude for convenient imports
pub mod prelude {
    pub use super::blockchain::StreamingBlockchain;
    pub use super::footprint::{BlockEvent, PluginFootprint};
    pub use super::retention::RetentionPolicy;
    pub use super::snapshot::SnapshotInterval;
}
