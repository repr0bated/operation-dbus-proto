use base64::Engine;
use op_identity::schema_bridge::{etch_footprint, peer_source_port, IdentitySled};

fn main() {
    let pubkey_b64 = std::env::args()
        .nth(1)
        .expect("usage: write_sled <base64_pubkey>");
    let pubkey = base64::engine::general_purpose::STANDARD
        .decode(&pubkey_b64)
        .expect("invalid base64 pubkey");
    let mut wg_key = [0u8; 32];
    wg_key.copy_from_slice(&pubkey);

    let source_port = peer_source_port(&pubkey_b64);
    let footprint = etch_footprint(&wg_key, 0, source_port);
    let trace_id = uuid::Uuid::new_v4().into_bytes();

    let sled = IdentitySled {
        wireguard_pubkey: wg_key,
        mutation_index: 0,
        hashed_footprint: footprint,
        trace_id,
        vector_id: [0u8; 16],
        schema_version: 1,
        reserved: [0u8; 44],
    };

    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            &sled as *const IdentitySled as *const u8,
            IdentitySled::SIZE,
        )
    };
    use std::io::Write;
    std::io::stdout().write_all(bytes).expect("stdout write");
    eprintln!(
        "Sled: {} bytes, footprint={}, trace_id={}",
        bytes.len(),
        hex::encode(footprint),
        hex::encode(trace_id)
    );
}
