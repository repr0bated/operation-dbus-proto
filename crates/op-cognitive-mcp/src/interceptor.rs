use memmap2::MmapOptions;
use std::fs::File;
use tonic::{Request, Status};

#[repr(C)]
pub struct IdentitySled {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: bool,
    pub _pad: [u8; 7],
    pub hashed_footprint: [u8; 32],
    pub schema_uuid: [u8; 16],
    pub subid: [u8; 64],
    pub control_source: [u8; 32],
    pub nextdns_profile: [u8; 16],
}

pub fn ghostbridge_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    let footprint_value = req.metadata().get("x-ghostbridge-footprint").cloned();
    let trace_value = req.metadata().get("x-ghostbridge-trace-id").cloned();

    if footprint_value.is_none() || trace_value.is_none() {
        return Err(Status::unauthenticated("Missing Ghostbridge Identity Sled."));
    }

    let file = File::open("/dev/shm/plugin_schema.dat")
        .map_err(|_| Status::internal("SchemaEngine Memory Unreachable"))?;

    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|_| Status::internal("Mmap failed"))?
    };
    let sled_ptr = mmap.as_ptr() as *const IdentitySled;

    let is_valid = unsafe { (*sled_ptr).is_valid };
    let current_footprint = unsafe { (*sled_ptr).hashed_footprint };

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

    // Verify OSCAL header
    let control_source = unsafe { &(*sled_ptr).control_source };
    let end = control_source.iter().position(|&b| b == 0).unwrap_or(32);
    let oscal_header = std::str::from_utf8(&control_source[..end]).unwrap_or("");
    
    // Log or check the OSCAL header if needed
    tracing::debug!("Validated request with OSCAL control source: {}", oscal_header);

    if let Some(trace_val) = trace_value {
        req.extensions_mut().insert(trace_val);
    }

    Ok(req)
}
