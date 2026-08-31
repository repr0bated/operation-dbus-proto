//! op-waypipe-grpc — tunnel waypipe Unix sockets over gRPC.
//!
//! Auth: server resolves one projected identity session on each
//! connection — clients do not need a local sled.
//!
//! ```text
//! laptop:  waypipe client  ←→  op-waypipe-grpc launch  ←gRPC→  serve  ←→  waypipe server -- Hyprland
//!                                                         ↑ read sled on connect
//! ```

pub mod auth;
pub mod bridge;
pub mod client;
pub mod config;
pub mod server;

pub mod proto {
    tonic::include_proto!("op.waypipe.v1");

    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("waypipe_descriptor");
}

pub use auth::{IdentityInterceptor, TunnelIdentity};
pub use client::{launch, LaunchOpts};
pub use config::TunnelConfig;
pub use server::{build_tunnel_service, serve, ServeOpts, WaypipeTunnelService};
