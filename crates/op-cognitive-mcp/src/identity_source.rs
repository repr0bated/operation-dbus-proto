use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use op_plugins::state_plugins::identity_sled::{ContainerIdentitySled, IdentitySledState};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct ContainerTraceContext {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub hashed_footprint: [u8; 32],
    pub trace_id: String,
}

pub fn read_identity_sled_state() -> Option<IdentitySledState> {
    let bytes = op_core::projection_shm::read_projection_bytes("identity_sled")?;
    serde_json::from_slice::<IdentitySledState>(&bytes).ok()
}

pub fn resolve_container_identity(
    trace_id: Option<&str>,
    wireguard_pubkey: Option<&str>,
    footprint: Option<&str>,
) -> Option<ContainerIdentitySled> {
    let state = read_identity_sled_state()?;
    state
        .sleds
        .into_iter()
        .filter(|sled| sled.active)
        .find(|sled| {
            matches_nonempty(trace_id, &sled.session_id)
                || matches_nonempty(trace_id, &sled.trace_id)
                || matches_nonempty(wireguard_pubkey, &sled.wireguard_pubkey)
                || matches_nonempty(footprint, &sled.hashed_footprint)
        })
}

pub fn active_container_trace_context() -> Option<ContainerTraceContext> {
    let state = read_identity_sled_state()?;
    let sled = state
        .sleds
        .into_iter()
        .filter(|sled| sled.active)
        .max_by_key(|sled| sled.last_seen_at)?;
    Some(trace_context_from_sled(&sled))
}

pub fn trace_context_from_sled(sled: &ContainerIdentitySled) -> ContainerTraceContext {
    let hashed_footprint = parse_hex_32(&sled.hashed_footprint)
        .unwrap_or_else(|| stable_hash_32(sled.session_id.as_bytes()));
    ContainerTraceContext {
        wireguard_pubkey: decode_wg_pubkey(&sled.wireguard_pubkey),
        mutation_index: sled.mutation_index,
        hashed_footprint,
        trace_id: if sled.trace_id.trim().is_empty() {
            format!("session-{}", sled.session_id)
        } else {
            sled.trace_id.trim().to_string()
        },
    }
}

pub fn populated_footprint(sled: &ContainerIdentitySled) -> Option<&str> {
    let footprint = sled.hashed_footprint.trim();
    (!footprint.is_empty()).then_some(footprint)
}

fn matches_nonempty(left: Option<&str>, right: &str) -> bool {
    let Some(left) = left.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    let right = right.trim();
    !right.is_empty() && left == right
}

fn decode_wg_pubkey(pubkey: &str) -> [u8; 32] {
    BASE64
        .decode(pubkey.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .unwrap_or([0u8; 32])
}

fn parse_hex_32(value: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(value.trim()).ok()?;
    bytes.try_into().ok()
}

fn stable_hash_32(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
