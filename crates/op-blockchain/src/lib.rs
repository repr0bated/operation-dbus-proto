//! op-blockchain: Streaming blockchain with BTRFS subvolumes
//!
//! This crate provides:
//! - Streaming blockchain for audit trails
//! - Plugin footprints for change tracking
//! - Dual BTRFS subvolumes (timing/vectors/state)
//! - Automatic snapshots with configurable intervals
//! - Rolling retention policies
//! - btrfs send/receive for replication

#![deny(rustdoc::broken_intra_doc_links)]

pub mod blockchain;
pub mod btrfs_delta;
pub mod btrfs_numa_integration;
pub mod footprint;
pub mod plugin_footprint;
pub mod retention;
pub mod snapshot;

// Re-export main types
pub use blockchain::{
    decode_vector, encode_vector, parse_block_number, parse_vector_block_number, ChainBlockRef,
    ReplicationReport, StreamingBlockchain, SNAPSHOT_LABELS,
};
pub use btrfs_delta::{find_new_since, generation, received_uuid, FindNewDelta};
pub use btrfs_numa_integration::OptimizedBlockchain;
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
