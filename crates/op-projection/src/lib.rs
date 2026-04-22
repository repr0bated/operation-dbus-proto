//! Projection System: Schema-validated state transformation engine
//!
//! The Projection System transforms live authoritative state from diverse sources
//! (D-Bus, gRPC, procfs, plugin-defined objects) into schema-validated derived
//! projections for dashboards, orchestration context, audit/compliance, topology,
//! AI context, and historical replay.
//!
//! # Core Principles
//!
//! - **Schema-as-Code Authority**: `PluginSchema` is the single source of truth
//! - **1:1 Direct Read (Zero-Copy)**: Reads from The Sled (`/dev/shm`)
//! - **Zero-Btrfs Overhead**: Identity extraction uses in-memory environment variables
//! - **The Accountability Loop**: `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID`
//!
//! # Modules
//!
//! - `data_models`: Core data structures for projections
//! - `interfaces`: Trait interfaces for projection components
//! - `schema_engine`: Schema registry and validation
//! - `schema_validator`: Entity validation services
//! - `projection_engine`: State transformation engine
//! - `event_materializer`: Event-driven projection updates
//! - `json_stream`: Real-time UI delivery
//! - `access_control`: Access control and redaction

pub mod access_control;
pub mod data_models;
pub mod dbus_reader;
pub mod event_materializer;
pub mod grpc_reader;
pub mod interfaces;
pub mod json_stream;
pub mod ovsdb_mirror;
pub mod plugin_reader;
pub mod procfs_reader;
pub mod projection_engine;
pub mod projection_store;
pub mod schema_engine;
pub mod schema_validator;

// Re-export core types
pub mod sled_reader;

pub use access_control::ProjectionAccessController;
pub use data_models::*;
pub use dbus_reader::SystemDbusReader;
pub use event_materializer::ProjectionMaterializer;
pub use grpc_reader::SystemGrpcReader;
pub use interfaces::*;
pub use json_stream::ProjectionStreamServer;
pub use ovsdb_mirror::OvsdbMirrorProjectionImpl;
pub use plugin_reader::SystemPluginReader;
pub use procfs_reader::SystemProcfsReader;
pub use projection_engine::ProjectionSystemEngine;
pub use projection_store::ProjectionStore;
pub use schema_engine::SchemaEngine;
pub use schema_validator::SchemaValidator;
pub use sled_reader::IdentitySledReader;
