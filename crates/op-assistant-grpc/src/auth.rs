//! Ghostbridge identity extraction and authentication middleware.
//!
//! Requires `x-ghostbridge-footprint` plus either `x-ghostbridge-trace-id` or
//! `x-wireguard-pubkey`, then verifies the footprint against the live
//! IdentitySled via `op_identity::verify_ghostbridge_footprint` — the same
//! gate as `op-cognitive-mcp`.

use op_identity::FootprintVerifyError;
use tonic::{Request, Status};

pub const WIREGUARD_PUBKEY_HEADER: &str = "x-wireguard-pubkey";

/// Identity extracted by the Ghostbridge interceptor and attached to each
/// accepted request for downstream authorization / trace linking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireGuardIdentity {
    pub pubkey: String,
    pub footprint: String,
    pub session_id: String,
}

/// Tonic interceptor: Ghostbridge footprint gate matching op-cognitive-mcp.
/// Returns `Unauthenticated` when identity headers are missing.
#[allow(clippy::result_large_err)]
pub fn wireguard_auth_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    let footprint_value = req.metadata().get("x-ghostbridge-footprint").cloned();
    let trace_value = req
        .metadata()
        .get("x-ghostbridge-trace-id")
        .or_else(|| req.metadata().get("x-wireguard-pubkey"))
        .cloned();

    if footprint_value.is_none() || trace_value.is_none() {
        return Err(Status::unauthenticated(
            "Missing Ghostbridge Identity Sled.",
        ));
    }

    let request_footprint = footprint_value
        .as_ref()
        .unwrap()
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid footprint header encoding"))?;

    op_identity::verify_ghostbridge_footprint(request_footprint).map_err(|error| match error {
        FootprintVerifyError::SledUnreachable => {
            Status::internal("MutationEngine Memory Unreachable")
        }
        FootprintVerifyError::InvalidSled => {
            Status::failed_precondition("Invalid Schema State. Cease and Desist.")
        }
        FootprintVerifyError::Mismatch => Status::permission_denied(
            "Temporal Hash Mismatch. Session footprint is out of sync with current mutation.",
        ),
    })?;

    let session_id = trace_value
        .as_ref()
        .expect("trace checked above")
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid trace header encoding"))?
        .to_string();
    let pubkey = req
        .metadata()
        .get(WIREGUARD_PUBKEY_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    req.extensions_mut().insert(WireGuardIdentity {
        pubkey,
        footprint: request_footprint.to_string(),
        session_id,
    });

    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_identity::IdentitySled;
    use std::io::Write;
    use tonic::metadata::MetadataValue;

    const FOOTPRINT_HEADER: &str = "x-ghostbridge-footprint";

    fn write_raw_sled(path: &std::path::Path, sled: &IdentitySled) {
        let bytes = unsafe {
            std::slice::from_raw_parts(sled as *const IdentitySled as *const u8, IdentitySled::SIZE)
        };
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(bytes).unwrap();
    }

    #[test]
    fn should_reject_missing_identity() {
        let req = Request::new(());
        let result = wireguard_auth_interceptor(req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn should_reject_pubkey_without_footprint() {
        let mut req = Request::new(());
        req.metadata_mut().insert(
            WIREGUARD_PUBKEY_HEADER,
            MetadataValue::from_static("abcd1234"),
        );
        let result = wireguard_auth_interceptor(req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn should_reject_empty_identity() {
        let mut req = Request::new(());
        req.metadata_mut()
            .insert(WIREGUARD_PUBKEY_HEADER, MetadataValue::from_static(""));
        let result = wireguard_auth_interceptor(req);
        assert!(result.is_err());
    }

    /// Owns `OP_SLED_PATH` for the whole body so parallel test threads cannot
    /// race the process-global env var (same pattern as op-identity).
    #[test]
    fn should_verify_footprint_against_identity_sled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sled.dat");
        let path_str = path.to_str().unwrap().to_string();
        unsafe {
            std::env::set_var("OP_SLED_PATH", &path_str);
        }

        let mut sled = IdentitySled::default();
        sled.hashed_footprint = [0xAB; 32];
        sled.trace_id = [0x11; 16];
        sled.wireguard_pubkey = [0x22; 32];
        write_raw_sled(&path, &sled);
        let matching = hex::encode(sled.hashed_footprint);
        let mismatch = hex::encode([0x00u8; 32]);

        let mut bad = Request::new(());
        bad.metadata_mut().insert(
            FOOTPRINT_HEADER,
            MetadataValue::try_from(mismatch.as_str()).unwrap(),
        );
        bad.metadata_mut().insert(
            WIREGUARD_PUBKEY_HEADER,
            MetadataValue::from_static("abcd1234"),
        );
        let err = wireguard_auth_interceptor(bad).unwrap_err();
        assert_eq!(err.code(), tonic::Code::PermissionDenied);

        let mut good = Request::new(());
        good.metadata_mut().insert(
            FOOTPRINT_HEADER,
            MetadataValue::try_from(matching.as_str()).unwrap(),
        );
        good.metadata_mut().insert(
            WIREGUARD_PUBKEY_HEADER,
            MetadataValue::from_static("abcd1234"),
        );
        let out = wireguard_auth_interceptor(good).unwrap();
        let id = out.extensions().get::<WireGuardIdentity>().unwrap();
        assert_eq!(id.pubkey, "abcd1234");

        unsafe {
            std::env::remove_var("OP_SLED_PATH");
        }
    }
}
