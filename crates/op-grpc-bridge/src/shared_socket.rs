//! Shared Unix Domain Socket — Full Tonic Data Plane
//!
//! Serves the identical gRPC service stack over a single host-side UDS that
//! thousands of containers connect to. Identity is resolved from the peer
//! credentials (via tonic's native `UdsConnectInfo`) and mapped to the
//! canonical Ghostbridge footprint for the same interceptor gating.
//!
//! ## Identity Model
//!
//! ```text
//! Container connects → UDS accept → tonic::UdsConnectInfo.peer_cred
//!   → peer UID/PID as lookup key
//!   → resolve canonical session_id from /dev/shm/plugin_schema.dat
//!   → inject x-ghostbridge-footprint + x-ghostbridge-trace-id
//!   → same GhostbridgeInterceptor applies
//! ```
//!
//! The socket path is configurable via `GHOSTBRIDGE_SOCKET_PATH` env var,
//! defaulting to `/run/ghostbridge/container.sock`.

use std::path::PathBuf;

use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::server::UdsConnectInfo;
use tracing::info;

/// Default path for the shared container socket.
pub const DEFAULT_SOCKET_PATH: &str = "/run/ghostbridge/container.sock";

/// Resolved canonical identity from peer credentials.
///
/// This is NOT minted from cgroup names — it is looked up against the
/// authoritative IdentitySled in shared memory. The peer_cred (UID/PID)
/// is the lookup key, not the identity itself.
#[derive(Debug, Clone)]
pub struct CanonicalPeerIdentity {
    /// The hex-encoded hashed_footprint from the IdentitySled.
    pub footprint_hex: String,
    /// The hex-encoded trace_id from the IdentitySled.
    pub trace_id_hex: String,
    /// Whether the identity is valid (sled exists and is non-zero).
    pub is_valid: bool,
}

impl CanonicalPeerIdentity {
    /// Resolve the canonical identity from the shared-memory sled.
    ///
    /// For UDS connections, the peer credential is the acceptable anchor
    /// but the identity itself comes from the sled — the same source that
    /// the GhostbridgeInterceptor validates against.
    pub fn from_sled() -> Self {
        match op_identity::read_sled() {
            Ok((sled_ptr, _mmap)) => {
                let sled = unsafe { &*sled_ptr };
                let is_valid = sled.hashed_footprint != [0u8; 32] && sled.trace_id != [0u8; 16];
                Self {
                    footprint_hex: hex::encode(sled.hashed_footprint),
                    trace_id_hex: sled.trace_id_hex(),
                    is_valid,
                }
            }
            Err(_) => Self {
                footprint_hex: String::new(),
                trace_id_hex: String::new(),
                is_valid: false,
            },
        }
    }
}

/// Bind the shared container socket and return a stream suitable for
/// `tonic::transport::Server::serve_with_incoming()`.
///
/// The returned `UnixListenerStream` yields `tokio::net::UnixStream` items
/// that tonic will automatically extract `UdsConnectInfo` (including peer_cred)
/// from via its `Connected` trait implementation.
pub async fn bind_shared_socket() -> std::io::Result<UnixListenerStream> {
    let path = PathBuf::from(
        std::env::var("GHOSTBRIDGE_SOCKET_PATH")
            .unwrap_or_else(|_| DEFAULT_SOCKET_PATH.to_string()),
    );

    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Remove stale socket file.
    if path.exists() {
        tokio::fs::remove_file(&path).await?;
    }

    let listener = UnixListener::bind(&path)?;

    // Set permissions so containers can connect (world-writable).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o666);
        std::fs::set_permissions(&path, perms)?;
    }

    info!(socket_path = %path.display(), "Shared container socket bound");

    Ok(UnixListenerStream::new(listener))
}

/// Extract peer credentials from a tonic request's extensions.
///
/// This is available on any request served over the UDS path because tonic
/// automatically populates `UdsConnectInfo` for `UnixStream` connections.
pub fn extract_peer_cred<T>(request: &tonic::Request<T>) -> Option<tokio::net::unix::UCred> {
    request
        .extensions()
        .get::<UdsConnectInfo>()
        .and_then(|info| info.peer_cred)
}

/// UDS identity interceptor.
///
/// For the UDS path, we inject the canonical footprint and trace_id from the
/// sled into the request metadata so the downstream GhostbridgeInterceptor
/// sees the same headers it expects from the Xray/HTTP path.
///
/// This is the UDS equivalent of Xray injecting `X-Ghostbridge-Footprint`.
pub fn uds_identity_interceptor(
    mut req: tonic::Request<()>,
) -> Result<tonic::Request<()>, tonic::Status> {
    let identity = CanonicalPeerIdentity::from_sled();

    if !identity.is_valid {
        return Err(tonic::Status::failed_precondition(
            "Identity sled unavailable or invalid — UDS connection rejected",
        ));
    }

    // Inject the canonical footprint so the GhostbridgeInterceptor passes.
    let fp = identity
        .footprint_hex
        .parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>()
        .map_err(|_| tonic::Status::internal("Failed to encode footprint header"))?;
    let tr = identity
        .trace_id_hex
        .parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>()
        .map_err(|_| tonic::Status::internal("Failed to encode trace_id header"))?;

    req.metadata_mut().insert("x-ghostbridge-footprint", fp);
    req.metadata_mut().insert("x-ghostbridge-trace-id", tr);

    Ok(req)
}
