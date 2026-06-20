//! D-Bus ↔ gRPC Bidirectional Bridge
//!
//! Provides live synchronization between D-Bus objects and gRPC services:
//! - D-Bus property changes → gRPC streaming updates
//! - gRPC mutations → D-Bus method calls / property sets
//! - D-Bus signals → gRPC server-streaming
//! - All changes flow through the event chain for audit/compliance
//!
//! Architecture:
//! ```text
//!                     ┌─────────────────┐
//!                     │   Event Chain   │ ← Source of truth
//!                     │  (audit + hash) │
//!                     └────────┬────────┘
//!                              │
//!               ┌──────────────┴──────────────┐
//!               ▼                              ▼
//!     ┌─────────────────┐            ┌─────────────────┐
//!     │     D-Bus       │◄──────────►│      gRPC       │
//!     │  (local IPC)    │            │  (remote RPC)   │
//!     └─────────────────┘            └─────────────────┘
//! ```

pub mod dbus_object;
pub mod grpc_client;
pub mod grpc_server;
pub mod interceptor;
pub mod proto_gen;
pub mod schema_engine;
pub mod schema_loader;
pub mod server;
pub mod tracing;

// Re-export main types
pub use grpc_client::{GrpcClientPool, RemoteEndpoint, RemoteOperationClient};
pub use grpc_server::{run_grpc_server, OperationGrpcServer, PluginSchemaProvider};
pub use interceptor::ghostbridge_interceptor;
pub use proto_gen::{ProtoGenConfig, ProtoGenerator};
pub use schema_engine::{ChangeSource, ChangeType, SchemaEngine, StateChange};
pub use server::{run_zeroclaw_server, ServerConfig};

/// Generated protobuf types — one sub-module per domain proto.
/// All are compiled into the combined operation_descriptor.bin for reflection.
pub mod proto {
    // Core: StateSync, PluginService, EventChainService, OvsdbMirror, RuntimeMirror
    tonic::include_proto!("operation.v1");

    pub mod mail {
        tonic::include_proto!("operation.mail.v1");
    }
    pub mod privacy {
        tonic::include_proto!("operation.privacy.v1");
    }
    pub mod registration {
        tonic::include_proto!("operation.registration.v1");
    }
    pub mod registry {
        tonic::include_proto!("operation.registry.v1");
    }

    /// Zeroclaw plugin schema gRPC service (GetSchema / WatchSchema).
    pub mod zeroclaw {
        tonic::include_proto!("zeroclaw");
    }

    /// Combined FileDescriptorSet covering all domain protos.
    /// Served by tonic-reflection so clients can discover every service.
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("operation_descriptor");
}
