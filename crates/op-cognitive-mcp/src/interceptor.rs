//! Ghostbridge gRPC interceptor — same Absolute Base gate as op-grpc-bridge.
//!
//! Presence of Xray-injected headers is not enough: the request must resolve
//! to an active container identity sled. The legacy raw mmap sled remains as a
//! compatibility fallback for hosts not yet publishing `identity_sled`.

use crate::identity_source::{populated_footprint, resolve_container_identity};
use op_identity::IdentitySled;
use tonic::{Request, Status};

fn is_sled_valid(sled: &IdentitySled) -> bool {
    sled.hashed_footprint != [0u8; 32] && sled.trace_id != [0u8; 16]
}

#[allow(clippy::result_large_err)]
pub fn ghostbridge_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    let footprint_value = req.metadata().get("x-ghostbridge-footprint").cloned();
    let trace_value = req.metadata().get("x-ghostbridge-trace-id").cloned();
    let wireguard_value = req.metadata().get("x-wireguard-pubkey").cloned();

    if footprint_value.is_none() || (trace_value.is_none() && wireguard_value.is_none()) {
        return Err(Status::unauthenticated(
            "A.N.N.A. Scribe: Missing Ghostbridge Identity Sled. Connection Dropped.",
        ));
    }

    let request_footprint =
        metadata_str(footprint_value.as_ref(), "footprint")?.expect("footprint checked above");
    let request_trace = metadata_str(trace_value.as_ref(), "trace")?;
    let request_pubkey = metadata_str(wireguard_value.as_ref(), "wireguard pubkey")?;

    if let Some(sled) =
        resolve_container_identity(request_trace, request_pubkey, Some(request_footprint))
    {
        if let Some(expected_footprint) = populated_footprint(&sled) {
            if request_footprint != expected_footprint {
                return Err(Status::permission_denied(
                    "A.N.N.A. Scribe: Temporal Hash Mismatch. \
                     Session footprint is out of sync with current Btrfs mutation.",
                ));
            }
        }

        req.extensions_mut().insert(sled.session_id);
        return Ok(req);
    }

    let (sled_ptr, _mmap) = op_identity::read_sled()
        .map_err(|_| Status::internal("MutationEngine Memory Unreachable"))?;
    let sled = unsafe { &*sled_ptr };

    if !is_sled_valid(sled) {
        return Err(Status::failed_precondition(
            "A.N.N.A. Scribe: Invalid Schema State. Cease and Desist.",
        ));
    }

    let expected_footprint = hex::encode(sled.hashed_footprint);

    if request_footprint != expected_footprint {
        return Err(Status::permission_denied(
            "A.N.N.A. Scribe: Temporal Hash Mismatch. \
             Session footprint is out of sync with current Btrfs mutation.",
        ));
    }

    let session_id = request_trace
        .or(request_pubkey)
        .expect("trace or pubkey checked above")
        .to_string();
    req.extensions_mut().insert(session_id);

    Ok(req)
}

fn metadata_str<'a>(
    value: Option<&'a tonic::metadata::MetadataValue<tonic::metadata::Ascii>>,
    name: &str,
) -> Result<Option<&'a str>, Status> {
    value
        .map(|value| {
            value
                .to_str()
                .map_err(|_| Status::invalid_argument(format!("Invalid {name} header encoding")))
        })
        .transpose()
}
