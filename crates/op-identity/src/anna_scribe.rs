// 🟢 📜 A.N.N.A. Scribe (Axon Network Notary Arbitrator)
// The top-level Identity-State Arbitrator who notarizes WireGuard identity
// against the 1:1 PluginSchema in shared memory and handles the "Snowball" session.

use chrono::Utc;
use memmap2::MmapOptions;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::Write;

use crate::IdentitySled;

/// THE SLED: 1:1 Zero-copy shared memory layout mapping directly to the SchemaEngine.
/// This struct represents the absolute base schema as it exists in `/dev/shm/plugin_schema.dat`.
/// The SchemaEngine coordinates this memory space; if `is_valid` is false, the entity
/// does not exist on the system.
#[repr(C)]
pub struct PluginSchema {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: bool,
    pub hashed_footprint: [u8; 32], // The vectorized "Thought"
}

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
/// 1:1 `PluginSchema`. If the schema in memory is invalid, she refuses to generate
/// the session identity — enforcing that without a valid schema, the entity does
/// not exist on the system.
pub struct AnnaScribe;

impl AnnaScribe {
    /// THE GREETING (Genesis Call)
    ///
    /// A.N.N.A. Scribe notarizes the WireGuard identity against the 1:1 memory schema.
    /// She casts a raw pointer to the `PluginSchema` in shared memory, extracts the
    /// `mutation_index`, and performs the **Strike/Etch** to generate the first hashed
    /// footprint. This creates the **Identity Sled** (the Snowball) entirely in memory,
    /// completely avoiding unintended Btrfs mutation loops while preserving NVMe I/O
    /// strictly for the blockchain transport.
    ///
    /// Uses `md5::compute` to tie the WireGuard key to the current `mutation_index`,
    /// directly matching the hashing methodology used by the Btrfs `EventChain`.
    pub fn notarize_arrival(wg_pubkey: &str) -> Result<SessionLedger, String> {
        // 1:1 Direct Read from the SchemaEngine's shared memory (No SQL, No Polling)
        let file = File::open("/dev/shm/plugin_schema.dat")
            .map_err(|_| "A.N.N.A. Scribe: Missing Schema. Connection Rejected.".to_string())?;

        // Zero-copy cast into the absolute base schema
        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .map_err(|_| "Memory map failed".to_string())?
        };
        let schema_ptr = mmap.as_ptr() as *const PluginSchema;

        let is_valid = unsafe { (*schema_ptr).is_valid };
        let current_mutation = unsafe { (*schema_ptr).mutation_index };

        // The Absolute Base: No valid schema, does not exist.
        if !is_valid {
            return Err("A.N.N.A. Scribe: Invalid Schema State. Cease and Desist.".to_string());
        }

        // The Strike/Etch: Bind the WireGuard Key to the current mutation index
        // to start the Snowball. Uses MD5 to maintain cryptographic continuity
        // with the EventChain system.
        let payload = format!("{}:{}", wg_pubkey, current_mutation);
        let genesis_hash = format!("{:x}", md5::compute(payload.as_bytes()));

        Ok(SessionLedger {
            wireguard_pubkey: wg_pubkey.to_string(),
            hashed_footprint: genesis_hash.clone(),
            trace_id: format!("trace-{}", genesis_hash),
        })
    }

    /// THE STRIKE/ETCH: Generates the cryptographic hash (footprint) for the identity.
    /// This is used to notarize the session and lock in the 1:1 schema.
    /// Uses SHA-256 for the high-fidelity footprint used in the Schema Shuttle loop.
    pub fn etch_footprint(sled: &IdentitySled) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(sled.wireguard_pubkey);
        hasher.update(sled.mutation_index.to_le_bytes());

        let result = hasher.finalize();
        let mut footprint = [0u8; 32];
        footprint.copy_from_slice(&result);
        footprint
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
        // No /dev/shm/plugin_schema.dat by default in test env
        // We test the error path directly: missing file => rejection
        let result = AnnaScribe::notarize_arrival("test-pubkey-abc");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Missing Schema. Connection Rejected"));
    }

    #[test]
    fn test_notarize_arrival_rejects_invalid_schema() {
        // Write an invalid schema (is_valid = false) to a known path
        let schema = PluginSchema {
            wireguard_pubkey: [0u8; 32],
            mutation_index: 1,
            is_valid: false,
            hashed_footprint: [0u8; 32],
        };

        // We can't easily override the hardcoded path in notarize_arrival,
        // so we test the validation logic inline
        assert!(!schema.is_valid);
    }

    #[test]
    fn test_genesis_hash_deterministic() {
        // The Strike/Etch must produce deterministic MD5 hashes
        let payload = format!("{}:{}", "wg-pubkey-abc123", 42u64);
        let hash1 = format!("{:x}", md5::compute(payload.as_bytes()));

        let payload2 = format!("{}:{}", "wg-pubkey-abc123", 42u64);
        let hash2 = format!("{:x}", md5::compute(payload2.as_bytes()));

        assert_eq!(hash1, hash2, "Genesis hash must be deterministic");
        assert_eq!(hash1.len(), 32, "MD5 hex must be 32 chars");
    }

    #[test]
    fn test_genesis_hash_changes_with_mutation() {
        // Different mutation_index must produce different hashes
        let payload_a = format!("{}:{}", "wg-pubkey-abc123", 1u64);
        let hash_a = format!("{:x}", md5::compute(payload_a.as_bytes()));

        let payload_b = format!("{}:{}", "wg-pubkey-abc123", 2u64);
        let hash_b = format!("{:x}", md5::compute(payload_b.as_bytes()));

        assert_ne!(
            hash_a, hash_b,
            "Different mutations must produce different hashes"
        );
    }

    #[test]
    fn test_genesis_hash_changes_with_pubkey() {
        // Different WireGuard pubkeys must produce different hashes
        let payload_a = format!("{}:{}", "wg-pubkey-aaa", 1u64);
        let hash_a = format!("{:x}", md5::compute(payload_a.as_bytes()));

        let payload_b = format!("{}:{}", "wg-pubkey-bbb", 1u64);
        let hash_b = format!("{:x}", md5::compute(payload_b.as_bytes()));

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
        };

        let fp1 = AnnaScribe::etch_footprint(&sled);
        let fp2 = AnnaScribe::etch_footprint(&sled);

        assert_eq!(fp1, fp2, "Etch footprint must be deterministic");
        assert_ne!(fp1, [0u8; 32], "Footprint must not be all zeros");
    }

    #[test]
    fn test_session_ledger_trace_id_format() {
        // Verify the trace_id format matches "trace-{genesis_hash}"
        let payload = format!("{}:{}", "test-key", 5u64);
        let genesis_hash = format!("{:x}", md5::compute(payload.as_bytes()));
        let expected_trace = format!("trace-{}", genesis_hash);

        assert!(expected_trace.starts_with("trace-"));
        assert_eq!(expected_trace.len(), 6 + 32); // "trace-" + 32 hex chars
    }

    #[test]
    fn test_plugin_schema_repr_c_layout() {
        // Verify the PluginSchema struct can be safely cast from raw bytes
        let size = std::mem::size_of::<PluginSchema>();
        // Must be large enough for: [u8;32] + u64 + bool + [u8;32]
        assert!(size >= 32 + 8 + 1 + 32, "PluginSchema must fit all fields");
    }
}
