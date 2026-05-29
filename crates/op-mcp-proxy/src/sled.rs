//! Zero-copy reader for the IdentitySled in /dev/shm/plugin_schema.dat.
//!
//! Mirrors the #[repr(C)] layout from op-identity::schema_bridge — must be
//! kept in sync if the sled struct changes.
//!
//! Layout (208 bytes total):
//!   [  0.. 32]  wireguard_pubkey   [u8; 32]
//!   [ 32.. 40]  mutation_index     u64 LE
//!   [ 40.. 41]  is_valid           bool
//!   [ 41.. 48]  _pad               [u8; 7]
//!   [ 48.. 80]  hashed_footprint   [u8; 32]
//!   [ 80.. 96]  schema_uuid        [u8; 16]
//!   [ 96..160]  subid              [u8; 64]
//!   [160..192]  control_source     [u8; 32]
//!   [192..208]  nextdns_profile    [u8; 16]

use memmap2::MmapOptions;
use std::fs::File;

pub const SLED_SIZE: usize = 208;
pub const SLED_PATH: &str = "/dev/shm/plugin_schema.dat";

pub struct SledSnapshot {
    pub is_valid: bool,
    pub mutation_index: u64,
    pub footprint_hex: String,
    pub trace_id: String,
    pub nextdns_profile: String,
    pub subid: String,
    pub control_source: String,
}

impl SledSnapshot {
    /// Read a snapshot from the sled. Returns None if file missing or invalid.
    pub fn read() -> Option<Self> {
        let file = File::open(SLED_PATH).ok()?;
        let mmap = unsafe { MmapOptions::new().len(SLED_SIZE).map(&file).ok()? };
        if mmap.len() < SLED_SIZE {
            return None;
        }

        let bytes = &mmap[..SLED_SIZE];

        let wg_pubkey = &bytes[0..32];
        let mutation_index = u64::from_le_bytes(bytes[32..40].try_into().ok()?);
        let is_valid = bytes[40] != 0;
        let footprint = &bytes[48..80];
        // subid at [96..160], control_source at [160..192], nextdns at [192..208]
        let nextdns_profile = fixed_str(&bytes[192..208]);
        let subid = fixed_str(&bytes[96..160]);
        let control_source = fixed_str(&bytes[160..192]);

        let footprint_hex = hex::encode(footprint);
        let trace_id = format!("{}-{}", hex::encode(&wg_pubkey[..4]), mutation_index);

        Some(SledSnapshot {
            is_valid,
            mutation_index,
            footprint_hex,
            trace_id,
            nextdns_profile,
            subid,
            control_source,
        })
    }
}

fn fixed_str(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}
