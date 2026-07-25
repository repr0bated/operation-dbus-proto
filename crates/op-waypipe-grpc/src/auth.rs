//! Identity-sled authentication for the waypipe gRPC tunnel.
//!
//! On every connection the server reads `/dev/shm/plugin_schema.dat` (the
//! IdentitySled) and attaches that identity — same model as
//! `op-grpc-bridge::shared_socket::CanonicalPeerIdentity::from_sled`.
//! Clients do not need a local sled; optional Ghostbridge headers, when
//! present, must still match the live sled.

use op_identity::{verify_ghostbridge_footprint, FootprintVerifyError, IdentitySled};
use tonic::{metadata::MetadataMap, Request, Status};
use tracing::info;

pub const FOOTPRINT_HEADER: &str = "x-ghostbridge-footprint";
pub const TRACE_HEADER: &str = "x-ghostbridge-trace-id";
pub const WIREGUARD_PUBKEY_HEADER: &str = "x-wireguard-pubkey";

#[derive(Debug, Clone)]
pub struct TunnelIdentity {
    pub footprint: String,
    pub session_id: String,
    pub pubkey_hex: String,
}

#[derive(Debug, Clone)]
pub struct SledIdentity {
    pub footprint_hex: String,
    pub trace_id_hex: String,
    pub pubkey_hex: String,
}

impl SledIdentity {
    fn from_sled(sled: &IdentitySled) -> Self {
        Self {
            footprint_hex: hex::encode(sled.hashed_footprint),
            trace_id_hex: sled.trace_id_hex(),
            pubkey_hex: hex::encode(sled.wireguard_pubkey),
        }
    }

    fn into_tunnel(self) -> TunnelIdentity {
        TunnelIdentity {
            footprint: self.footprint_hex,
            session_id: self.trace_id_hex,
            pubkey_hex: self.pubkey_hex,
        }
    }
}

/// Fresh read of the live IdentitySled in SHM (called on every connection).
pub fn read_sled_identity() -> Result<SledIdentity, Status> {
    let (ptr, _mmap) = op_identity::read_sled().map_err(|e| {
        Status::unauthenticated(format!("Identity Sled unreadable on connection: {e}"))
    })?;
    let sled = unsafe { &*ptr };
    if !sled.is_sled_valid() {
        return Err(Status::failed_precondition(
            "Identity Sled is invalid on connection (zero footprint/trace)",
        ));
    }
    Ok(SledIdentity::from_sled(sled))
}

#[allow(clippy::result_large_err)]
fn optional_header<'a>(metadata: &'a MetadataMap, key: &str) -> Result<Option<&'a str>, Status> {
    match metadata.get(key) {
        None => Ok(None),
        Some(raw) => Ok(Some(
            raw.to_str()
                .map_err(|_| Status::invalid_argument(format!("invalid {key} encoding")))?,
        )),
    }
}

/// Authorize by reading the SHM sled on connection.
///
/// Headers are optional. If the client sends a footprint/pubkey, they must
/// match the sled that was just read — stale or foreign identity is rejected.
#[allow(clippy::result_large_err)]
pub fn authorize_on_connection(metadata: &MetadataMap) -> Result<TunnelIdentity, Status> {
    let sled = read_sled_identity()?;

    if let Some(footprint) = optional_header(metadata, FOOTPRINT_HEADER)? {
        verify_ghostbridge_footprint(footprint).map_err(|error| match error {
            FootprintVerifyError::SledUnreachable => {
                Status::internal("Identity Sled unreachable in SHM")
            }
            FootprintVerifyError::InvalidSled => Status::failed_precondition(
                "Identity Sled invalid; no valid mutation has landed yet",
            ),
            FootprintVerifyError::Mismatch => Status::permission_denied(
                "Temporal Hash Mismatch. Presented footprint != Identity Sled on connection.",
            ),
        })?;
    }

    if let Some(presented) = optional_header(metadata, WIREGUARD_PUBKEY_HEADER)? {
        let presented = presented.trim();
        if presented.len() == 64 && !presented.eq_ignore_ascii_case(&sled.pubkey_hex) {
            return Err(Status::permission_denied(
                "x-wireguard-pubkey does not match Identity Sled on connection",
            ));
        }
    }

    if let Some(trace) = optional_header(metadata, TRACE_HEADER)? {
        let trace = trace.trim();
        if !trace.is_empty() && !trace.eq_ignore_ascii_case(&sled.trace_id_hex) {
            return Err(Status::permission_denied(
                "x-ghostbridge-trace-id does not match Identity Sled on connection",
            ));
        }
    }

    info!(
        footprint = %sled.footprint_hex,
        trace = %sled.trace_id_hex,
        pubkey = %sled.pubkey_hex,
        "identity sled read on connection"
    );

    Ok(sled.into_tunnel())
}

/// Tonic interceptor: read IdentitySled from SHM on every RPC.
#[derive(Clone, Default)]
pub struct IdentityInterceptor;

impl tonic::service::Interceptor for IdentityInterceptor {
    #[allow(clippy::result_large_err)]
    fn call(&mut self, mut req: Request<()>) -> Result<Request<()>, Status> {
        let identity = authorize_on_connection(req.metadata())?;
        req.extensions_mut().insert(identity);
        Ok(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tonic::metadata::MetadataValue;
    use tonic::service::Interceptor;

    fn write_test_sled(
        path: &std::path::Path,
        footprint: [u8; 32],
        trace: [u8; 16],
        pubkey: [u8; 32],
    ) {
        let mut sled = IdentitySled::default();
        sled.hashed_footprint = footprint;
        sled.trace_id = trace;
        sled.wireguard_pubkey = pubkey;
        sled.mutation_index = 1;
        sled.schema_version = 1;
        let mut f = std::fs::File::create(path).unwrap();
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (&sled as *const IdentitySled) as *const u8,
                IdentitySled::SIZE,
            )
        };
        f.write_all(bytes).unwrap();
    }

    fn with_test_sled<F: FnOnce()>(f: F) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sled.dat");
        write_test_sled(&path, [0xABu8; 32], [0x11u8; 16], [0x22u8; 32]);
        unsafe {
            std::env::set_var("OP_SLED_PATH", path.to_str().unwrap());
        }
        f();
        unsafe {
            std::env::remove_var("OP_SLED_PATH");
        }
    }

    #[test]
    fn accepts_connection_by_reading_sled_without_headers() {
        with_test_sled(|| {
            let req = Request::new(());
            let mut interceptor = IdentityInterceptor;
            let out = interceptor.call(req).unwrap();
            let id = out.extensions().get::<TunnelIdentity>().unwrap();
            assert_eq!(id.footprint, hex::encode([0xABu8; 32]));
            assert_eq!(id.pubkey_hex, hex::encode([0x22u8; 32]));
        });
    }

    #[test]
    fn rejects_mismatched_presented_footprint() {
        with_test_sled(|| {
            let mut req = Request::new(());
            let bad = "00".repeat(32);
            req.metadata_mut().insert(
                FOOTPRINT_HEADER,
                MetadataValue::try_from(bad.as_str()).unwrap(),
            );
            let mut interceptor = IdentityInterceptor;
            let err = interceptor.call(req).unwrap_err();
            assert_eq!(err.code(), tonic::Code::PermissionDenied);
        });
    }

    #[test]
    fn rejects_when_sled_missing() {
        unsafe {
            std::env::set_var("OP_SLED_PATH", "/tmp/op-waypipe-grpc-no-such-sled.dat");
        }
        let req = Request::new(());
        let mut interceptor = IdentityInterceptor;
        let err = interceptor.call(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unauthenticated);
        unsafe {
            std::env::remove_var("OP_SLED_PATH");
        }
    }
}
