//! Identity integration — reads identity from the state-tree JSON
//! at /dev/shm/opdbus/state/identity_sled.json and provides
//! the GhostBridge footprint + trace_id for authenticated gRPC calls.

use serde::Deserialize;
use tracing::{debug, warn};

/// Canonical location written by `write_projection()` (post projection-removal).
const STATE_PATH: &str = "/dev/shm/opdbus/state/identity_sled.json";
/// Pre-removal location under `projections/`. Read fallback only, one deploy cycle.
const LEGACY_PROJECTION_PATH: &str = "/dev/shm/opdbus/projections/identity_sled.json";

/// A single sled entry from the identity projection
#[derive(Debug, Clone, Deserialize)]
pub struct SledEntry {
    pub active: bool,
    pub hashed_footprint: String,
    pub trace_id: String,
    pub session_id: String,
    pub wireguard_pubkey: String,
    pub mutation_index: u64,
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub last_seen_at: i64,
    #[serde(default)]
    pub session_started_at: i64,
}

/// The full identity projection structure
#[derive(Debug, Deserialize)]
struct IdentityProjection {
    sleds: Vec<SledEntry>,
}

/// Identity context for authenticated calls.
#[derive(Debug, Clone)]
pub struct CliIdentity {
    /// Hex-encoded Blake3 hashed footprint
    pub footprint: String,
    /// Hex-encoded trace ID
    pub trace_id: String,
    /// Session ID (container name / derived identity)
    pub session_id: String,
    /// WireGuard public key (base64)
    pub wireguard_pubkey: String,
    /// Schema mutation index
    pub mutation_index: u64,
}

impl CliIdentity {
    /// Read the active identity from the state-tree JSON.
    /// Returns the first active sled with a non-empty footprint.
    pub fn read() -> Option<Self> {
        let data = match std::fs::read_to_string(STATE_PATH)
            .or_else(|_| std::fs::read_to_string(LEGACY_PROJECTION_PATH))
        {
            Ok(d) => d,
            Err(e) => {
                debug!("Could not read identity state: {}", e);
                return None;
            }
        };

        let projection: IdentityProjection = match serde_json::from_str(&data) {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to parse identity projection: {}", e);
                return None;
            }
        };

        // Find the first active sled with a valid footprint
        projection
            .sleds
            .iter()
            .find(|s| s.active && !s.hashed_footprint.is_empty() && !s.trace_id.is_empty())
            .map(|s| {
                debug!(
                    "Using identity: session={}, footprint={}…",
                    s.session_id,
                    &s.hashed_footprint[..16.min(s.hashed_footprint.len())]
                );
                CliIdentity {
                    footprint: s.hashed_footprint.clone(),
                    trace_id: s.trace_id.clone(),
                    session_id: s.session_id.clone(),
                    wireguard_pubkey: s.wireguard_pubkey.clone(),
                    mutation_index: s.mutation_index,
                }
            })
    }

    /// Print identity info for `dbus-plugin-cli identity` command.
    pub fn display(&self) {
        println!("Identity (from {}):", STATE_PATH);
        println!("  Session ID:      {}", self.session_id);
        println!("  Footprint:       {}", self.footprint);
        println!("  Trace ID:        {}", self.trace_id);
        println!("  WireGuard Key:   {}", self.wireguard_pubkey);
        println!("  Mutation Index:  {}", self.mutation_index);
    }
}
