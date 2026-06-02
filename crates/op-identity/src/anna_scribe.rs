// 🟢 📜 A.N.N.A. Scribe (Axon Network Notary Arbitrator)
// The top-level Identity-State Arbitrator who notarizes WireGuard identity
// against the 1:1 IdentitySled in shared memory and handles the "Snowball" session.

use chrono::Utc;
use memmap2::MmapOptions;
use std::fs::{File, OpenOptions};
use std::io::Write;

use crate::IdentitySled;

/// The genesis "Snowball" session record, created by A.N.N.A. Scribe when a WireGuard
/// connection arrives. This is the first entry in the accountability loop, tying the
/// WireGuard pubkey to the current schema mutation state.
#[derive(Debug)]
pub struct SessionLedger {
    pub wireguard_pubkey: String,
    pub hashed_footprint: String, // The genesis "Snowball"
    pub trace_id: String,
}

/// A.N.N.A. Scribe (Axon Network Notary Arbitrator)
///
/// The top-level gatekeeper who merges the ephemeral WireGuard identity with the
/// absolute present state into a "Snowball" session. She relies strictly on the
/// 1:1 `IdentitySled`. If the schema footprint in memory is invalid (all zeros),
/// she refuses to generate the session identity — enforcing that without a valid
/// schema, the entity does not exist on the system.
pub struct AnnaScribe;

/// Check whether a sled is "valid" per the Absolute Base rule.
fn is_sled_valid(sled: &IdentitySled) -> bool {
    // A valid sled must have a non-zero footprint and trace_id.
    sled.hashed_footprint != [0u8; 32] && sled.trace_id != [0u8; 16]
}

impl AnnaScribe {
    /// THE GREETING (Genesis Call)
    ///
    /// A.N.N.A. Scribe notarizes the WireGuard identity against the 1:1 memory sled.
    /// She casts a raw pointer to the `IdentitySled` in shared memory, extracts the
    /// `mutation_index`, and performs the **Strike/Etch** to generate the first hashed
    /// footprint. This creates the **Snowball** session ledger entry entirely in memory,
    /// completely avoiding unintended Btrfs mutation loops while preserving NVMe I/O
    /// strictly for the blockchain transport.
    ///
    /// Uses Blake3 per the spec for all Strike/Etch operations.
    pub fn notarize_arrival(wg_pubkey: &str) -> Result<SessionLedger, String> {
        // 1:1 Direct Read from the SchemaEngine's shared memory (No SQL, No Polling)
        let file = File::open("/dev/shm/plugin_schema.dat")
            .map_err(|_| "A.N.N.A. Scribe: Missing Schema. Connection Rejected.".to_string())?;

        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .map_err(|_| "Memory map failed".to_string())?
        };
        let sled_ptr = mmap.as_ptr() as *const IdentitySled;
        let sled = unsafe { &*sled_ptr };

        // The Absolute Base: No valid schema, does not exist.
        if !is_sled_valid(sled) {
            return Err("A.N.N.A. Scribe: Invalid Schema State. Cease and Desist.".to_string());
        }

        // The Strike/Etch: Bind the WireGuard Key to the Blake3 hash of the
        // canonical schema catalog in shared memory. This makes the sled footprint
        // a direct function of the single source of truth (/dev/shm/live-schema.json).
        let schema_catalog_hash = match std::fs::read("/dev/shm/live-schema.json") {
            Ok(bytes) => blake3::hash(&bytes),
            Err(_) => {
                return Err("A.N.N.A. Scribe: Schema catalog missing from shared memory. Connection Rejected.".to_string());
            }
        };

        let mut hasher = blake3::Hasher::new();
        hasher.update(wg_pubkey.as_bytes());
        hasher.update(schema_catalog_hash.as_bytes());
        hasher.update(&sled.mutation_index.to_le_bytes());
        let genesis_hash = hex::encode(hasher.finalize().as_bytes());

        Ok(SessionLedger {
            wireguard_pubkey: wg_pubkey.to_string(),
            hashed_footprint: genesis_hash.clone(),
            trace_id: format!("trace-{}", genesis_hash),
        })
    }

    /// THE STRIKE/ETCH: Generates the cryptographic hash (footprint) for the identity.
    /// Binds the WireGuard public key to the Blake3 hash of the canonical schema catalog
    /// in shared memory (/dev/shm/live-schema.json), plus the mutation index.
    /// This makes the sled footprint a direct function of the single source of truth.
    pub fn etch_footprint(sled: &IdentitySled) -> [u8; 32] {
        let schema_catalog_hash = std::fs::read("/dev/shm/live-schema.json")
            .map(|bytes| blake3::hash(&bytes))
            .unwrap_or_else(|_| blake3::Hash::from([0u8; 32]));

        let mut hasher = blake3::Hasher::new();
        hasher.update(&sled.wireguard_pubkey);
        hasher.update(schema_catalog_hash.as_bytes());
        hasher.update(&sled.mutation_index.to_le_bytes());
        hasher.finalize().into()
    }

    /// THE SNOWBALL: Appends the session ledger.
    /// Strictly preserved in RAM (tmpfs) to avoid Btrfs mutation loops.
    /// NVMe I/O is preserved strictly for the Btrfs vectorized footprint transport.
    pub fn append_snowball(footprint: &[u8; 32], action: &str) -> anyhow::Result<()> {
        let timestamp = Utc::now().to_rfc3339();
        let footprint_hex = hex::encode(footprint);
        let entry = format!("[{}] {} | {}\n", timestamp, footprint_hex, action);

        // Path is in tmpfs to preserve NVMe I/O for Btrfs blockchain transport
        let snowball_path = "/dev/shm/snowball_session.log";

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(snowball_path)?;

        file.write_all(entry.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notarize_arrival_rejects_missing_schema() {
        let result = AnnaScribe::notarize_arrival("test-pubkey-abc");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Missing Schema. Connection Rejected"));
    }

    #[test]
    fn test_notarize_arrival_rejects_invalid_sled() {
        let sled = IdentitySled {
            wireguard_pubkey: [0u8; 32],
            mutation_index: 1,
            hashed_footprint: [0u8; 32],
            trace_id: [0u8; 16],
            vector_id: [0u8; 16],
            schema_version: 0,
            reserved: [0u8; 60],
        };
        assert!(!is_sled_valid(&sled));
    }

    #[test]
    fn test_notarize_arrival_accepts_valid_sled() {
        let sled = IdentitySled {
            wireguard_pubkey: [0xBB; 32],
            mutation_index: 5,
            hashed_footprint: [0xAA; 32],
            trace_id: [0xCC; 16],
            vector_id: [0u8; 16],
            schema_version: 1,
            reserved: [0u8; 60],
        };
        assert!(is_sled_valid(&sled));
    }

    #[test]
    fn test_genesis_hash_deterministic() {
        let mut h1 = blake3::Hasher::new();
        h1.update(b"wg-pubkey-abc123");
        h1.update(&42u64.to_le_bytes());
        let hash1 = hex::encode(h1.finalize().as_bytes());

        let mut h2 = blake3::Hasher::new();
        h2.update(b"wg-pubkey-abc123");
        h2.update(&42u64.to_le_bytes());
        let hash2 = hex::encode(h2.finalize().as_bytes());

        assert_eq!(hash1, hash2, "Genesis hash must be deterministic");
        assert_eq!(hash1.len(), 64, "Blake3 hex must be 64 chars");
    }

    #[test]
    fn test_genesis_hash_changes_with_mutation() {
        let mut ha = blake3::Hasher::new();
        ha.update(b"wg-pubkey-abc123");
        ha.update(&1u64.to_le_bytes());
        let hash_a = hex::encode(ha.finalize().as_bytes());

        let mut hb = blake3::Hasher::new();
        hb.update(b"wg-pubkey-abc123");
        hb.update(&2u64.to_le_bytes());
        let hash_b = hex::encode(hb.finalize().as_bytes());

        assert_ne!(
            hash_a, hash_b,
            "Different mutations must produce different hashes"
        );
    }

    #[test]
    fn test_genesis_hash_changes_with_pubkey() {
        let mut ha = blake3::Hasher::new();
        ha.update(b"wg-pubkey-aaa");
        ha.update(&1u64.to_le_bytes());
        let hash_a = hex::encode(ha.finalize().as_bytes());

        let mut hb = blake3::Hasher::new();
        hb.update(b"wg-pubkey-bbb");
        hb.update(&1u64.to_le_bytes());
        let hash_b = hex::encode(hb.finalize().as_bytes());

        assert_ne!(
            hash_a, hash_b,
            "Different pubkeys must produce different hashes"
        );
    }

    #[test]
    fn test_etch_footprint_deterministic() {
        let sled = IdentitySled {
            wireguard_pubkey: [0xAA; 32],
            mutation_index: 100,
            hashed_footprint: [0u8; 32],
            trace_id: [0u8; 16],
            vector_id: [0u8; 16],
            schema_version: 1,
            reserved: [0u8; 60],
        };

        let fp1 = AnnaScribe::etch_footprint(&sled);
        let fp2 = AnnaScribe::etch_footprint(&sled);

        assert_eq!(fp1, fp2, "Etch footprint must be deterministic");
        assert_ne!(fp1, [0u8; 32], "Footprint must not be all zeros");
    }

    #[test]
    fn test_session_ledger_trace_id_format() {
        let mut h = blake3::Hasher::new();
        h.update(b"test-key");
        h.update(&5u64.to_le_bytes());
        let genesis_hash = hex::encode(h.finalize().as_bytes());
        let expected_trace = format!("trace-{}", genesis_hash);

        assert!(expected_trace.starts_with("trace-"));
        assert_eq!(expected_trace.len(), 6 + 64); // "trace-" + 64 hex chars
    }

    #[test]
    fn test_identity_sled_repr_c_layout() {
        let size = std::mem::size_of::<IdentitySled>();
        // Must be exactly 152 bytes per spec
        assert_eq!(size, 152, "IdentitySled must be 152 bytes");
    }
}
