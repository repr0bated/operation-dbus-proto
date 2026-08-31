//! Ghostbridge identity interceptor — validates a session genesis against the
//! authoritative per-session identity projection.

use tonic::{Request, Status};

#[allow(clippy::result_large_err)]
pub fn ghostbridge_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    let footprint = req
        .metadata()
        .get("x-ghostbridge-genesis")
        .or_else(|| req.metadata().get("x-ghostbridge-footprint"));
    let trace_id = req.metadata().get("x-ghostbridge-trace-id");
    let pubkey = req.metadata().get("x-wireguard-pubkey");

    if footprint.is_none() || trace_id.is_none() {
        return Err(Status::unauthenticated(
            "Missing Ghostbridge identity headers",
        ));
    }

    let received = footprint
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| Status::invalid_argument("Invalid genesis header encoding"))?;
    let trace_id = trace_id.and_then(|value| value.to_str().ok());
    let pubkey = pubkey.and_then(|value| value.to_str().ok());
    op_identity::verify_session_genesis(received, trace_id, pubkey).map_err(|_| {
        Status::permission_denied("Ghostbridge genesis does not match an anchored session")
    })?;

    Ok(req)
}
