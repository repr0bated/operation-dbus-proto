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

pub mod callable_reflection;
pub mod chat_service;
pub mod dbus_object;
pub mod dynamic_reflection;
pub mod emqx_hook_provider;
pub mod grpc_client;
pub mod grpc_server;
pub mod identity_sled_dispatch;
pub mod interceptor;
pub mod mutation_engine;
pub mod per_plugin_reflection;
pub mod plugin_grpc_gen;
pub mod plugin_object_blob;
pub mod proto_gen;
pub mod schema_loader;
pub mod server;
pub mod shared_socket;
pub mod tracing;
pub mod zeroclaw_object_blob;

// Re-export main types
pub use grpc_client::{GrpcClientPool, RemoteEndpoint, RemoteOperationClient};
pub use grpc_server::{run_grpc_server, OperationGrpcServer, PluginSchemaProvider};
pub use interceptor::ghostbridge_interceptor;
pub use mutation_engine::{ChangeSource, ChangeType, MutationEngine, StateChange};
pub use per_plugin_reflection::{
    generate_all_method_protos_for_plugin, generate_reflection_file_descriptor_set,
    to_method_service_name, MethodReflectionMeta, PerMethodReflectionConfig,
    PerMethodReflectionRegistry,
};
pub use plugin_grpc_gen::{
    generate_method_file_descriptor, generate_method_proto, generate_plugin_method_protos,
    MethodServiceLifecycleEvent, MethodServiceRegistry, PerMethodGrpcServices,
};
pub use proto_gen::{ProtoGenConfig, ProtoGenerator};
pub use server::{run_zeroclaw_server, ServerConfig};
// Object blob artifacts (schema-coupled D-Bus + gRPC reflection units),
// backed by the op-blob crate.
pub use plugin_object_blob::{BlobMethod, DbusObjectIdentity, PluginObjectBlob};
pub use zeroclaw_object_blob::ZeroclawObjectBlob;

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

    /// EMQX ExHook v2 broker callback service.
    pub mod emqx_exhook {
        tonic::include_proto!("emqx.exhook.v2");
    }

    /// Zeroclaw plugin schema gRPC service (GetSchema / WatchSchema).
    pub mod zeroclaw {
        tonic::include_proto!("zeroclaw");
    }

    /// ChatService — operator-to-system chat interface (delegator, forced tool calling).
    pub mod chat {
        tonic::include_proto!("op_chat.chat");
    }

    /// Build-time generated PluginSchema method services.
    pub mod plugin_methods {
        tonic::include_proto!("operation.plugin.v1");
    }

    /// Combined FileDescriptorSet covering all domain protos (including chat).
    /// Served by tonic-reflection so clients can discover every service.
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("operation_descriptor");
}
