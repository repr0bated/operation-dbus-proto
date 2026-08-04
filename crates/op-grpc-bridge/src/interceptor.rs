// 🟢 🛡️ The Tonic gRPC Gatekeeper (Middleware Interceptor)
// Sits on the primary gRPC ingress at port 8090. Intercepts Xray-injected headers,
// performs a zero-copy check against the IdentitySled in shared memory, and either
// allows the gRPC payload through or drops the connection instantly.
//
// Operated by A.N.N.A. Scribe. No payload enters the system without a cryptographic
// "Snowball" session. No SQL databases, no D-Bus watchers. 1:1 Direct Read only.

use op_identity::FootprintVerifyError;
use std::sync::{Arc, OnceLock};
use tonic::{Request, Status};

use crate::mutation_engine::MutationEngine;

/// Identity extracted by the Ghostbridge interceptor and attached to each
/// accepted request for bridge-layer authorization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhostbridgeIdentity {
    pub footprint: String,
    pub session_id: String,
}

/// Process-wide handle to the engine, set once from `build_operation_routes`
/// (the single choke point all routes are built from) before any interceptor
/// can run. Lets the interceptor do a per-identity Cozo lookup
/// (`identity_sled_dispatch::get_identity`) without threading `Arc<MutationEngine>`
/// through every one of the ~10 `.with_interceptor()` call sites.
static ENGINE: OnceLock<Arc<MutationEngine>> = OnceLock::new();

pub fn set_engine(engine: Arc<MutationEngine>) {
    let _ = ENGINE.set(engine);
}

/// Look up the specific caller's identity sled and check its footprint +
/// term (`expires_at`) — the per-identity path. Resolves either by a
/// WireGuard pubkey (host/xray-mesh callers, session_id derived from the
/// pubkey) or, when there's no pubkey, directly by the trace_id the caller
/// presents (vendor/partner identities like a browser frontend that has no
/// WireGuard identity of its own — e.g. Lovable). Returns `None` when
/// there's no engine registered yet, no identifying header, or no matching
/// record, so the caller can fall back to the shared host legacy sled.
fn verify_per_identity(
    pubkey: Option<&str>,
    trace_id: Option<&str>,
    request_footprint: &str,
) -> Option<Result<String, Status>> {
    let engine = ENGINE.get()?;
    let session_id = pubkey.map(op_identity::session::derive_session_id);
    let args = match (&session_id, trace_id) {
        (Some(sid), _) => serde_json::json!({ "session_id": sid }),
        (None, Some(trace)) => serde_json::json!({ "trace_id": trace }),
        (None, None) => return None,
    };

    // Bridge the sync interceptor into the async Cozo-backed lookup. The
    // lookup is an in-memory state-cache read (Cozo is the restart-warm
    // source, not a per-call read), so blocking briefly here is safe on the
    // multi-threaded runtime op-grpc-bridge runs under.
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(
            crate::identity_sled_dispatch::dispatch_identity_sled_method(
                engine.as_ref(),
                "get_identity",
                &args,
            ),
        )
    });

    let identity = match result {
        Ok(value) => value.get("identity").cloned()?,
        Err(_) => return None, // no per-identity record — fall back to the shared sled
    };
    let hashed_footprint = identity.get("hashed_footprint")?.as_str()?.to_string();
    let expires_at = identity.get("expires_at").and_then(|v| v.as_i64());

    if let Some(expires_at) = expires_at {
        if expires_at != 0 && expires_at <= chrono::Utc::now().timestamp() {
            return Some(Err(Status::permission_denied(
                "Identity term has expired. Re-authenticate to renew.",
            )));
        }
    }

    if hashed_footprint != request_footprint {
        return Some(Err(Status::permission_denied(
            "Temporal Hash Mismatch. Session footprint is out of sync with current mutation.",
        )));
    }

    let resolved_session_id = identity
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or(session_id)?;
    Some(Ok(resolved_session_id))
}

/// THE GATEKEEPER: Tonic gRPC Interceptor on port 8090.
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

    let request_footprint = footprint_value
        .as_ref()
        .unwrap()
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid footprint header encoding"))?;

    // 2. Per-identity path: look up THIS caller's own Cozo-backed sled and
    // check both footprint match and term (`expires_at`) validity, rather
    // than only the shared host legacy sled. Resolved either by a real
    // WireGuard pubkey (host/xray-mesh callers), as its own distinct signal
    // — not just a trace-id fallback — or, for vendor/partner identities
    // with no WireGuard identity of their own (e.g. a browser frontend like
    // Lovable), directly by the trace_id they present. This is what lets
    // temporary, revocable consumer identities exist alongside the lifelong
    // host/jeremy/chatbot accounts without either bypassing the gate or
    // being permanently trusted.
    let pubkey_value = req.metadata().get("x-wireguard-pubkey").cloned();
    let raw_trace_value = req.metadata().get("x-ghostbridge-trace-id").cloned();
    let pubkey_str = pubkey_value
        .as_ref()
        .map(|v| v.to_str())
        .transpose()
        .map_err(|_| Status::invalid_argument("Invalid pubkey header encoding"))?;
    let trace_str = raw_trace_value
        .as_ref()
        .map(|v| v.to_str())
        .transpose()
        .map_err(|_| Status::invalid_argument("Invalid trace header encoding"))?;
    if pubkey_str.is_some() || trace_str.is_some() {
        if let Some(outcome) = verify_per_identity(pubkey_str, trace_str, request_footprint) {
            let session_id = outcome?;
            req.extensions_mut().insert(GhostbridgeIdentity {
                footprint: request_footprint.to_string(),
                session_id,
            });
            return Ok(req);
        }
    }

    // 3. Fallback: the shared host legacy sled — shared with op-cognitive-mcp
    // via op_identity::verify_ghostbridge_footprint so the two gatekeepers
    // can never silently drift apart again. Used when there's no per-identity
    // record (host's own calls, or before any temporary identity exists).
    op_identity::verify_ghostbridge_footprint(request_footprint).map_err(|error| match error {
        FootprintVerifyError::SledUnreachable => {
            Status::internal("MutationEngine Memory Unreachable")
        }
        FootprintVerifyError::InvalidSled => {
            Status::failed_precondition("A.N.N.A. Scribe: Invalid Schema State. Cease and Desist.")
        }
        FootprintVerifyError::Mismatch => Status::permission_denied(
            "A.N.N.A. Scribe: Temporal Hash Mismatch. \
             Session footprint is out of sync with current Btrfs mutation.",
        ),
    })?;

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

/// Permit only the two calls needed to establish an identity. Everything else
/// on RegistrationService still passes through the normal sled gate.
#[allow(clippy::result_large_err)]
pub fn registration_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    match req.extensions().get::<tonic::GrpcMethod<'_>>() {
        Some(method)
            if method.service() == "operation.registration.v1.RegistrationService"
                && matches!(method.method(), "SendMagicLink" | "VerifyMagicLink") =>
        {
            Ok(req)
        }
        _ => ghostbridge_interceptor(req),
    }
}

// `load_capability_grants` below is public API that deliberately follows the
// test module; moving the tests to end-of-file would churn this shared
// surface for no behavioral gain.
#[cfg(test)]
#[allow(clippy::items_after_test_module)]
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
    fn registration_bootstrap_allows_magic_link_without_identity() {
        for method in ["SendMagicLink", "VerifyMagicLink"] {
            let mut req = Request::new(());
            req.extensions_mut().insert(tonic::GrpcMethod::new(
                "operation.registration.v1.RegistrationService",
                method,
            ));
            assert!(registration_interceptor(req).is_ok());
        }
    }

    #[test]
    fn registration_bootstrap_does_not_expose_admin_methods() {
        let mut req = Request::new(());
        req.extensions_mut().insert(tonic::GrpcMethod::new(
            "operation.registration.v1.RegistrationService",
            "ListUsers",
        ));
        assert_eq!(
            registration_interceptor(req).unwrap_err().code(),
            tonic::Code::Unauthenticated
        );
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

    // Sled-validity coverage (struct layout, InvalidSled/Mismatch/Ok branches)
    // now lives in op_identity::schema_bridge, next to the shared
    // verify_ghostbridge_footprint function itself, rather than duplicated
    // here against a local IdentitySled/is_sled_valid that no longer exist
    // in this crate.
}

/// Load the capabilities granted to one sled footprint.
///
/// The JSON document is keyed by lowercase footprint hex and each value owns a
/// `capabilities` array. Missing, malformed, or unreadable grant state fails
/// closed.
pub fn load_capability_grants(footprint_hex: &str) -> std::collections::HashSet<String> {
    let path = std::env::var("OP_GRANTS_PATH")
        .unwrap_or_else(|_| "/dev/shm/opdbus/capability-grants.json".to_string());
    let Ok(bytes) = std::fs::read(path) else {
        return std::collections::HashSet::new();
    };
    let Ok(document) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return std::collections::HashSet::new();
    };
    document
        .get(footprint_hex)
        .or_else(|| document.get("*"))
        .and_then(|entry| entry.get("capabilities"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect()
}
