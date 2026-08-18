// The Ghostbridge Gatekeeper for op-cognitive-mcp — the universal external MCP
// gateway. Calls op_identity::verify_ghostbridge_footprint, the single shared
// implementation of the Absolute Base check, so this and op-grpc-bridge's
// interceptor can never silently drift apart again (see SIGNALS.md: this
// interceptor had previously regressed to a presence-only check while
// op-grpc-bridge's stayed correct).

use op_identity::FootprintVerifyError;
use tonic::{Request, Status};

/// Identity extracted by the Ghostbridge interceptor and attached to each
/// accepted request for downstream authorization / trace linking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhostbridgeIdentity {
    pub footprint: String,
    pub session_id: String,
}

#[allow(clippy::result_large_err)]
pub fn ghostbridge_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
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
    req.extensions_mut().insert(GhostbridgeIdentity {
        footprint: request_footprint.to_string(),
        session_id,
    });

    Ok(req)
}
