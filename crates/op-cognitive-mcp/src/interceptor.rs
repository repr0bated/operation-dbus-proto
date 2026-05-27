use op_identity::read_sled;
use tonic::{Request, Status};

pub fn ghostbridge_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    let footprint_value = req.metadata().get("x-ghostbridge-footprint").cloned();
    let trace_value = req.metadata().get("x-ghostbridge-trace-id").cloned();

    if footprint_value.is_none() || trace_value.is_none() {
        return Err(Status::unauthenticated(
            "Missing Ghostbridge Identity Sled.",
        ));
    }

    // Zero-copy read from /dev/shm/plugin_schema.dat via mmap.
    // `_mmap` keeps the mapping alive for the duration of this function.
    let (sled_ptr, _mmap) =
        read_sled().map_err(|_| Status::internal("SchemaEngine Memory Unreachable"))?;

    // SAFETY: read_sled() uses MmapOptions::len(IdentitySled::SIZE) so the
    // mapping is at least SIZE bytes, and write_sled() uses atomic rename so
    // readers never see a partial write.
    let is_valid = unsafe { (*sled_ptr).is_valid };
    let current_footprint = unsafe { (*sled_ptr).hashed_footprint };
    let control_source_bytes = unsafe { (*sled_ptr).control_source };

    if !is_valid {
        return Err(Status::failed_precondition("Invalid Schema State."));
    }

    let request_footprint = footprint_value
        .as_ref()
        .unwrap()
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid footprint encoding"))?;
    let expected_footprint = hex::encode(current_footprint);

    if request_footprint != expected_footprint {
        return Err(Status::permission_denied("Temporal Hash Mismatch."));
    }

    // Log the OSCAL control-source for audit / debug.
    let end = control_source_bytes
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(32);
    let oscal_header = std::str::from_utf8(&control_source_bytes[..end]).unwrap_or("");
    tracing::debug!(
        "Validated request with OSCAL control source: {}",
        oscal_header
    );

    if let Some(trace_val) = trace_value {
        req.extensions_mut().insert(trace_val);
    }

    Ok(req)
}
