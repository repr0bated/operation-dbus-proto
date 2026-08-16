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

use std::{collections::HashMap, path::PathBuf, sync::OnceLock};

use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::server::UdsConnectInfo;
use tracing::info;

/// Default path for the shared container socket.
pub const DEFAULT_SOCKET_PATH: &str = "/run/ghostbridge/container.sock";

/// Comma-separated `uid=session_id` entries provisioned alongside Incus
/// subuid mappings. The map is parsed once, so request metadata cannot alter
/// the transport-to-session binding.
const UID_SESSION_MAP_ENV: &str = "GHOSTBRIDGE_UID_SESSION_MAP";

static UID_SESSION_MAP: OnceLock<HashMap<u32, String>> = OnceLock::new();

fn parse_uid_session_map(value: &str) -> HashMap<u32, String> {
    value
        .split(',')
        .filter_map(|entry| {
            let (uid, session_id) = entry.trim().split_once('=')?;
            let uid = uid.trim().parse::<u32>().ok()?;
            let session_id = session_id.trim();
            (!session_id.is_empty()).then(|| (uid, session_id.to_owned()))
        })
        .collect()
}

fn uid_session_map() -> &'static HashMap<u32, String> {
    UID_SESSION_MAP.get_or_init(|| {
        std::env::var(UID_SESSION_MAP_ENV)
            .map(|value| parse_uid_session_map(&value))
            .unwrap_or_default()
    })
}

fn session_id_for_uid(uid: u32, bridge_uid: u32) -> Option<String> {
    if uid == 0 || uid == bridge_uid {
        crate::identity_sled_dispatch::host_session_id()
    } else {
        uid_session_map().get(&uid).cloned()
    }
}

#[cfg(unix)]
fn bridge_uid() -> std::io::Result<u32> {
    use std::os::unix::fs::MetadataExt;
    Ok(std::fs::metadata("/proc/self")?.uid())
}

/// Resolved canonical identity from peer credentials.
///
/// This is NOT minted from cgroup names or caller-supplied metadata. It is read
/// from the host's own session record (the authoritative identity source)
/// after the transport is proven to be a Unix socket — never the global
/// 152-byte sled projection, which no longer carries an identity verdict.
#[derive(Debug, Clone)]
pub struct CanonicalPeerIdentity {
    /// The hex-encoded session genesis for the host ("container zero").
    pub genesis_hex: String,
    /// The hex-encoded trace_id from the host's session record.
    pub trace_id_hex: String,
    /// Whether the identity is valid (record exists and is non-zero).
    pub is_valid: bool,
}

impl CanonicalPeerIdentity {
    fn invalid() -> Self {
        Self {
            genesis_hex: String::new(),
            trace_id_hex: String::new(),
            is_valid: false,
        }
    }

    /// Resolve the canonical identity of the host ("container zero") from its
    /// own session record in the authoritative state cache.
    ///
    /// For UDS connections, the peer credential is the acceptable transport
    /// anchor, but the identity itself is the session record the
    /// GhostbridgeInterceptor validates against — the same source, never the
    /// shared global sled. Returns an invalid identity when no engine has been
    /// registered or the host record has not been provisioned yet.
    pub fn from_session(session_id: &str) -> Self {
        let engine = match crate::interceptor::engine_handle() {
            Some(engine) => engine,
            None => return Self::invalid(),
        };
        let record = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(
                crate::identity_sled_dispatch::session_record_for_actor(
                    engine.as_ref(),
                    session_id,
                ),
            )
        });
        match record {
            Some(sled) => {
                let genesis = sled.genesis.unwrap_or_default();
                let is_valid = !genesis.is_empty() && !sled.trace_id.is_empty();
                Self {
                    genesis_hex: genesis,
                    trace_id_hex: sled.trace_id,
                    is_valid,
                }
            }
            None => Self::invalid(),
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

    let peer_uid = extract_peer_cred(&req)
        .map(|cred| cred.uid())
        .ok_or_else(|| {
            tonic::Status::unauthenticated("UDS peer credentials unavailable — connection rejected")
        })?;
    let bridge_uid = bridge_uid()
        .map_err(|_| tonic::Status::internal("Unable to resolve bridge process uid"))?;
    let session_id = session_id_for_uid(peer_uid, bridge_uid)
        .ok_or_else(|| tonic::Status::unauthenticated("UDS peer uid is not mapped to a session"))?;
    let identity = CanonicalPeerIdentity::from_session(&session_id);

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

    // Inject the canonical session genesis so the GhostbridgeInterceptor passes.
    let genesis = identity
        .genesis_hex
        .parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>()
        .map_err(|_| tonic::Status::internal("Failed to encode genesis header"))?;
    let tr = identity
        .trace_id_hex
        .parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>()
        .map_err(|_| tonic::Status::internal("Failed to encode trace_id header"))?;

    req.metadata_mut().insert("x-ghostbridge-genesis", genesis);
    req.metadata_mut().insert("x-ghostbridge-trace-id", tr);

    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Code;

    fn valid_identity() -> CanonicalPeerIdentity {
        CanonicalPeerIdentity {
            genesis_hex: "11".repeat(32),
            trace_id_hex: "22".repeat(16),
            is_valid: true,
        }
    }

    #[test]
    fn canonical_identity_overwrites_caller_metadata() {
        let mut request = tonic::Request::new(());
        request.metadata_mut().insert(
            "x-ghostbridge-genesis",
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
                .get("x-ghostbridge-genesis")
                .expect("genesis")
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
                genesis_hex: String::new(),
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

    #[test]
    fn uid_to_session_mapping() {
        let map = parse_uid_session_map("100000=session-one, 200000 = session-two, malformed, 7=");

        assert_eq!(map.get(&100000).map(String::as_str), Some("session-one"));
        assert_eq!(map.get(&200000).map(String::as_str), Some("session-two"));
        assert!(!map.contains_key(&7));
    }
}
