//! Ghostbridge identity interceptor — validates X-Ghostbridge-Footprint and
//! X-Ghostbridge-Trace-ID on every inbound gRPC call, identical to op-grpc-bridge.

use tonic::{Request, Status};

pub fn ghostbridge_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    let footprint = req.metadata().get("x-ghostbridge-footprint");
    let trace_id = req.metadata().get("x-ghostbridge-trace-id");

    if footprint.is_none() || trace_id.is_none() {
        return Err(Status::unauthenticated(
            "Missing Ghostbridge identity headers",
        ));
    }

    // Zero-copy sled read — validate footprint matches live schema state.
    if let Ok((ptr, _mmap)) = op_identity::read_sled() {
        let sled = unsafe { &*ptr };
        let expected = hex::encode(sled.hashed_footprint);
        let received = footprint.unwrap().to_str().unwrap_or("");
        if received != expected {
            return Err(Status::permission_denied(
                "Ghostbridge footprint mismatch — session out of sync",
            ));
        }
    }

    Ok(req)
}
