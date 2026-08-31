//! Session-scoped authentication for the waypipe gRPC tunnel.
//!
//! Every connection names a session through its genesis plus trace id or
//! WireGuard key. The values are checked against the authoritative identity
//! projection; no process-wide identity is inferred.

use tonic::{metadata::MetadataMap, Request, Status};
use tracing::info;

pub const FOOTPRINT_HEADER: &str = "x-ghostbridge-footprint";
pub const GENESIS_HEADER: &str = "x-ghostbridge-genesis";
pub const TRACE_HEADER: &str = "x-ghostbridge-trace-id";
pub const WIREGUARD_PUBKEY_HEADER: &str = "x-wireguard-pubkey";

#[derive(Debug, Clone)]
pub struct TunnelIdentity {
    pub footprint: String,
    pub session_id: String,
    pub pubkey_hex: String,
}

#[allow(clippy::result_large_err)]
fn optional_header<'a>(metadata: &'a MetadataMap, key: &str) -> Result<Option<&'a str>, Status> {
    match metadata.get(key) {
        None => Ok(None),
        Some(raw) => Ok(Some(raw.to_str().map_err(|_| {
            Status::invalid_argument(format!("invalid {key} encoding"))
        })?)),
    }
}

/// Authorize one named session against the per-session projection.
#[allow(clippy::result_large_err)]
pub fn authorize_on_connection(metadata: &MetadataMap) -> Result<TunnelIdentity, Status> {
    let genesis = optional_header(metadata, GENESIS_HEADER)?
        .or(optional_header(metadata, FOOTPRINT_HEADER)?)
        .ok_or_else(|| Status::unauthenticated("session genesis header is required"))?;
    let trace = optional_header(metadata, TRACE_HEADER)?;
    let pubkey = optional_header(metadata, WIREGUARD_PUBKEY_HEADER)?;
    let selector = trace.or(pubkey).unwrap_or(genesis);
    let identity = op_identity::resolve_identity_session(Some(selector))
        .map_err(|error| Status::permission_denied(error.to_string()))?;
    if identity.genesis.as_deref() != Some(genesis) {
        return Err(Status::permission_denied(
            "presented genesis does not match the selected session",
        ));
    }

    info!(
        session_id = %identity.session_id,
        "waypipe session identity verified"
    );

    Ok(TunnelIdentity {
        footprint: genesis.to_string(),
        session_id: identity.session_id,
        pubkey_hex: identity.wireguard_pubkey,
    })
}

/// Tonic interceptor: resolve the request's session on every RPC.
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
    use tonic::metadata::MetadataValue;
    use tonic::service::Interceptor;

    fn with_test_session<F: FnOnce()>(f: F) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("identity_sled.json");
        std::fs::write(
            path,
            serde_json::to_vec(&serde_json::json!({
                "sleds": [{
                    "session_id": "session-a",
                    "wireguard_pubkey": "pubkey-a",
                    "mutation_index": 1,
                    "genesis": "ab".repeat(32),
                    "trace_id": "11".repeat(16),
                    "schema_version": 3,
                    "active": true,
                    "arrival_timestamp": 1,
                    "chain_head_at_arrival": "22".repeat(32)
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        unsafe {
            std::env::set_var("OP_SHM_STATE_DIR", dir.path());
        }
        f();
        unsafe {
            std::env::remove_var("OP_SHM_STATE_DIR");
        }
    }

    #[test]
    fn accepts_connection_for_named_session() {
        with_test_session(|| {
            let mut req = Request::new(());
            req.metadata_mut().insert(
                GENESIS_HEADER,
                MetadataValue::try_from("ab".repeat(32)).unwrap(),
            );
            req.metadata_mut().insert(
                TRACE_HEADER,
                MetadataValue::try_from("11".repeat(16)).unwrap(),
            );
            let mut interceptor = IdentityInterceptor;
            let out = interceptor.call(req).unwrap();
            let id = out.extensions().get::<TunnelIdentity>().unwrap();
            assert_eq!(id.footprint, "ab".repeat(32));
            assert_eq!(id.session_id, "session-a");
        });
    }

    #[test]
    fn rejects_mismatched_presented_footprint() {
        with_test_session(|| {
            let mut req = Request::new(());
            let bad = "00".repeat(32);
            req.metadata_mut().insert(
                GENESIS_HEADER,
                MetadataValue::try_from(bad.as_str()).unwrap(),
            );
            req.metadata_mut().insert(
                TRACE_HEADER,
                MetadataValue::try_from("11".repeat(16)).unwrap(),
            );
            let mut interceptor = IdentityInterceptor;
            let err = interceptor.call(req).unwrap_err();
            assert_eq!(err.code(), tonic::Code::PermissionDenied);
        });
    }

    #[test]
    fn rejects_when_identity_headers_are_missing() {
        let req = Request::new(());
        let mut interceptor = IdentityInterceptor;
        let err = interceptor.call(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unauthenticated);
    }
}
