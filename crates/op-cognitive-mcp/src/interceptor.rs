use op_identity::{read_sled, IdentitySled};
use tonic::{Request, Status};

#[allow(clippy::result_large_err)]
pub fn ghostbridge_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    let footprint_value = req.metadata().get("x-ghostbridge-footprint").cloned();
    let trace_value = req.metadata().get("x-ghostbridge-trace-id").cloned();

    // If xraqy injected the identity headers, the request came through WireGuard — accept it.
    if footprint_value.is_some() && trace_value.is_some() {
        if let Some(trace_val) = trace_value {
            req.extensions_mut().insert(trace_val);
        }
        return Ok(req);
    }

    Err(Status::unauthenticated(
        "Missing Ghostbridge Identity Sled.",
    ))
}
