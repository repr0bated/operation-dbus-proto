//! Shared Unix Domain Socket — Full Tonic Data Plane
//!
//! Serves the identical gRPC service stack over a single host-side UDS that
//! thousands of containers connect to. The Unix peer credential proves that
//! the request arrived on the UDS transport; the canonical Ghostbridge
//! footprint still comes from the authoritative shared-memory sled.
//!
//! ## Identity Model
//!
//! ```text
//! Container connects → UDS accept → require tonic::UdsConnectInfo.peer_cred
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
/// This is NOT minted from cgroup names or caller-supplied metadata. It is read
/// from the authoritative IdentitySled in shared memory after the transport is
/// proven to be a Unix socket.
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
        // `create_dir_all` is umask-limited, so it lands 0755 root:root and
        // silently re-breaks the parent every time this process restarts.
        // Unprivileged containers map root to a subuid: at 0755 they can
        // CONNECT to this socket but cannot BIND their own alongside it, which
        // is the only inbound channel a NIC-less container has. That is the
        // regression f9188f96 fixed in opdbus-rundirs-up; keep both in sync.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o777)).await?;
        }
    }

    if let Err(e) = tokio::fs::remove_file(&path).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(e);
        }
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
#[allow(clippy::result_large_err)]
pub fn uds_identity_interceptor(
    req: tonic::Request<()>,
) -> Result<tonic::Request<()>, tonic::Status> {
    if extract_peer_cred(&req).is_none() {
        return Err(tonic::Status::unauthenticated(
            "UDS peer credentials unavailable — connection rejected",
        ));
    }

    let identity = CanonicalPeerIdentity::from_sled();

    inject_canonical_identity(req, identity)
}

#[allow(clippy::result_large_err)]
fn inject_canonical_identity(
    mut req: tonic::Request<()>,
    identity: CanonicalPeerIdentity,
) -> Result<tonic::Request<()>, tonic::Status> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Code;

    fn valid_identity() -> CanonicalPeerIdentity {
        CanonicalPeerIdentity {
            footprint_hex: "11".repeat(32),
            trace_id_hex: "22".repeat(16),
            is_valid: true,
        }
    }

    #[test]
    fn canonical_identity_overwrites_caller_metadata() {
        let mut request = tonic::Request::new(());
        request.metadata_mut().insert(
            "x-ghostbridge-footprint",
            "spoofed".parse().expect("test metadata"),
        );
        request.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            "spoofed".parse().expect("test metadata"),
        );

        let request = inject_canonical_identity(request, valid_identity())
            .expect("valid canonical identity should be injected");

        assert_eq!(
            request
                .metadata()
                .get("x-ghostbridge-footprint")
                .expect("footprint")
                .to_str()
                .expect("ASCII footprint"),
            "11".repeat(32)
        );
        assert_eq!(
            request
                .metadata()
                .get("x-ghostbridge-trace-id")
                .expect("trace id")
                .to_str()
                .expect("ASCII trace id"),
            "22".repeat(16)
        );
    }

    #[test]
    fn invalid_canonical_identity_fails_closed() {
        let error = inject_canonical_identity(
            tonic::Request::new(()),
            CanonicalPeerIdentity {
                footprint_hex: String::new(),
                trace_id_hex: String::new(),
                is_valid: false,
            },
        )
        .expect_err("invalid sled identity must be rejected");

        assert_eq!(error.code(), Code::FailedPrecondition);
    }

    #[test]
    fn interceptor_rejects_request_without_uds_connect_info() {
        let error = uds_identity_interceptor(tonic::Request::new(()))
            .expect_err("non-UDS request must not enter the UDS identity path");

        assert_eq!(error.code(), Code::Unauthenticated);
    }
}
