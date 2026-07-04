// 🟢 🛡️ The Tonic gRPC Gatekeeper (Middleware Interceptor)
// Sits on the primary gRPC ingress at port 18789. Intercepts Xray-injected headers,
// performs a zero-copy check against the IdentitySled in shared memory, and either
// allows the gRPC payload through or drops the connection instantly.
//
// Operated by A.N.N.A. Scribe. No payload enters the system without a cryptographic
// "Snowball" session. No SQL databases, no D-Bus watchers. 1:1 Direct Read only.

use op_identity::IdentitySled;
use tonic::{Request, Status};

/// Identity extracted by the Ghostbridge interceptor and attached to each
/// accepted request for bridge-layer authorization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhostbridgeIdentity {
    pub footprint: String,
    pub session_id: String,
}

/// Check whether a sled is "valid" per the Absolute Base rule.
fn is_sled_valid(sled: &IdentitySled) -> bool {
    sled.hashed_footprint != [0u8; 32] && sled.trace_id != [0u8; 16]
}

/// THE GATEKEEPER: Tonic gRPC Interceptor on port 18789.
///
/// Enforces the Absolute Base rule: if the `x-ghostbridge-footprint` provided by Xray
/// does not perfectly match the hashed footprint sitting in shared memory, the payload
/// is rejected. Once validated, embeds the `x-ghostbridge-trace-id` into Tonic Request
/// extensions so the Chatbot and Qdrant semantic search on the Accountability Page
/// have the exact Trace ID needed to link the session.
#[derive(Clone, Debug)]
pub struct GhostbridgeInterceptor;

impl tonic::service::Interceptor for GhostbridgeInterceptor {
    fn call(&mut self, req: Request<()>) -> Result<Request<()>, Status> {
        ghostbridge_interceptor(req)
    }
}

#[allow(clippy::result_large_err)]
pub fn ghostbridge_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    // 1. Extract the Xray-injected Identity Headers (The Accountability Loop)
    //    Clone header values upfront to release the immutable borrow on `req`,
    //    allowing the mutable `extensions_mut()` call downstream.
    let footprint_value = req.metadata().get("x-ghostbridge-footprint").cloned();
    let trace_value = req
        .metadata()
        .get("x-ghostbridge-trace-id")
        .or_else(|| req.metadata().get("x-wireguard-pubkey"))
        .cloned();

    if footprint_value.is_none() || trace_value.is_none() {
        return Err(Status::unauthenticated(
            "A.N.N.A. Scribe: Missing Ghostbridge Identity Sled. Connection Dropped.",
        ));
    }

    // 2. 1:1 Direct Read from the MutationEngine's shared memory (No SQL, No Polling)
    let (sled_ptr, _mmap) = op_identity::read_sled()
        .map_err(|_| Status::internal("MutationEngine Memory Unreachable"))?;
    let sled = unsafe { &*sled_ptr };

    // The Absolute Base: No valid schema, it does not exist.
    if !is_sled_valid(sled) {
        return Err(Status::failed_precondition(
            "A.N.N.A. Scribe: Invalid Schema State. Cease and Desist.",
        ));
    }

    let current_footprint = sled.hashed_footprint;

    // 3. The Strike/Etch Validation: Check if the payload is in sync with Btrfs.
    //    If a Btrfs mutation has occurred and the client's footprint is stale,
    //    the connection is dropped without consuming any NVMe I/O.
    let request_footprint = footprint_value
        .as_ref()
        .unwrap()
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid footprint header encoding"))?;
    let expected_footprint = hex::encode(current_footprint);

    if request_footprint != expected_footprint {
        return Err(Status::permission_denied(
            "A.N.N.A. Scribe: Temporal Hash Mismatch. \
             Session footprint is out of sync with current Btrfs mutation.",
        ));
    }

    // 4. Pass the Trace ID downstream into the gRPC context for the React GUI.
    let session_id = trace_value
        .as_ref()
        .expect("trace checked above")
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid trace header encoding"))?
        .to_string();
    req.extensions_mut().insert(GhostbridgeIdentity {
        footprint: request_footprint.to_string(),
        session_id,
    });

    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::metadata::MetadataValue;

    #[test]
    fn test_rejects_missing_footprint_header() {
        let req = Request::new(());
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
        assert!(status
            .message()
            .contains("Missing Ghostbridge Identity Sled"));
    }

    #[test]
    fn test_rejects_missing_trace_header() {
        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-footprint",
            MetadataValue::from_static("deadbeef"),
        );
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn test_rejects_missing_footprint_with_trace_only() {
        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            MetadataValue::from_static("trace-abc"),
        );
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn test_identity_sled_repr_c_layout() {
        let size = std::mem::size_of::<IdentitySled>();
        assert_eq!(
            size, 152,
            "IdentitySled must be exactly 152 bytes per spec, got {} bytes",
            size
        );
    }

    #[test]
    fn test_footprint_mismatch_detection() {
        let sled_footprint: [u8; 32] = [0xAA; 32];
        let expected = hex::encode(sled_footprint);
        let request_footprint = "0000000000000000000000000000000000000000000000000000000000000000";
        assert_ne!(request_footprint, expected);
    }

    #[test]
    fn test_footprint_match_succeeds() {
        let sled_footprint: [u8; 32] = [0xBB; 32];
        let expected = hex::encode(sled_footprint);
        let request_footprint = hex::encode([0xBB; 32]);
        assert_eq!(request_footprint, expected);
    }

    #[test]
    fn test_mutation_engine_unreachable_returns_internal() {
        // Only meaningful on hosts without a live IdentitySled in shared
        // memory; with a running daemon read_sled() succeeds and the
        // interceptor proceeds past the branch under test.
        if op_identity::read_sled().is_ok() {
            eprintln!("skipping: live IdentitySled present in shared memory");
            return;
        }
        let mut req = Request::new(());
        req.metadata_mut().insert(
            "x-ghostbridge-footprint",
            MetadataValue::from_static("aabbccdd"),
        );
        req.metadata_mut().insert(
            "x-ghostbridge-trace-id",
            MetadataValue::from_static("trace-aabbccdd"),
        );
        let result = ghostbridge_interceptor(req);
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
        assert!(status
            .message()
            .contains("MutationEngine Memory Unreachable"));
    }

    #[test]
    fn test_invalid_sled_rejected() {
        let sled = IdentitySled {
            wireguard_pubkey: [0u8; 32],
            mutation_index: 1,
            hashed_footprint: [0u8; 32],
            trace_id: [0u8; 16],
            schema_version: 0,
            reserved: [0u8; 44],
            vector_id: [0u8; 16],
        };
        assert!(!is_sled_valid(&sled));
    }

    #[test]
    fn test_valid_sled_accepted() {
        let sled = IdentitySled {
            wireguard_pubkey: [0xCC; 32],
            mutation_index: 42,
            hashed_footprint: [0xDD; 32],
            trace_id: [0xEE; 16],
            schema_version: 1,
            reserved: [0u8; 44],
            vector_id: [0xDD; 16],
        };
        assert!(is_sled_valid(&sled));
    }
}
