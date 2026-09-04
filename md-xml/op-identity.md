This file is a merged representation of the entire codebase, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of the entire repository's contents.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
src/
  bin/
    op-identity-sled.rs
  anna_scribe.rs
  gcloud_auth.rs
  lib.rs
  registration.rs
  schema_bridge.rs
  session.rs
  token.rs
  wg.rs
  wireguard.rs
Cargo.toml
compare-op-identity.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/bin/op-identity-sled.rs">
use anyhow::{bail, Context, Result};
use op_identity::schema_bridge::{write_sled_from_wg, IdentitySled, SHM_SLED_PATH};
use serde::Serialize;
use std::env;
use std::fs::File;
use std::io::Read;
use std::mem;
use std::process::Command;

const COMPACT_SLED_SIZE: usize = 80;
const COMPACT_MUTATION_OFFSET: usize = 32;
const COMPACT_VALID_OFFSET: usize = 40;
const COMPACT_FOOTPRINT_OFFSET: usize = 48;

#[derive(Debug)]
struct Args {
    path: String,
    iface: String,
    refresh: bool,
    pretty: bool,
}

#[derive(Debug, Serialize)]
struct SledView {
    path: String,
    layout: &'static str,
    size: usize,
    is_valid: bool,
    wireguard_pubkey_hex: String,
    wireguard_pubkey_b64: String,
    mutation_index: u64,
    hashed_footprint: String,
    schema_catalog_hash: String,
    trace_id: String,
    schema_version: u32,
}

fn main() -> Result<()> {
    let args = parse_args()?;

    if args.refresh {
        let pubkey = read_wg_pubkey(&args.iface)?;
        write_sled_from_wg(&pubkey).context("failed to refresh sled from WireGuard public key")?;
    }

    let view = read_sled_view(&args.path)?;

    if args.pretty {
        print_pretty(&view);
    } else {
        println!("{}", serde_json::to_string_pretty(&view)?);
    }

    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut args = env::args().skip(1);
    let mut parsed = Args {
        path: env::var("OP_IDENTITY_SLED_PATH").unwrap_or_else(|_| SHM_SLED_PATH.to_string()),
        iface: env::var("WG_INTERFACE").unwrap_or_else(|_| "wg0".to_string()),
        refresh: false,
        pretty: false,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--path" => {
                parsed.path = args.next().context("--path requires a file path")?;
            }
            "--iface" | "-i" => {
                parsed.iface = args.next().context("--iface requires an interface name")?;
            }
            "--refresh" => parsed.refresh = true,
            "--pretty" => parsed.pretty = true,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other}"),
        }
    }

    Ok(parsed)
}

fn print_help() {
    println!(
        "op-identity-sled\n\n\
Usage:\n  op-identity-sled [--path FILE] [--iface IFACE] [--refresh] [--pretty]\n\n\
Options:\n  --path FILE     Read sled from FILE instead of /dev/shm/plugin_schema.dat\n  -i, --iface     WireGuard interface used with --refresh, default wg0\n  --refresh       Rewrite the sled from wg show <iface> public-key before reading\n  --pretty        Print a compact human-readable view instead of JSON\n\n\
The reader accepts both the canonical op-identity sled and the legacy 80-byte\n\
bridge sled used by older Ghostbridge components.\n"
    );
}

fn read_wg_pubkey(iface: &str) -> Result<String> {
    let output = Command::new("wg")
        .args(["show", iface, "public-key"])
        .output()
        .with_context(|| format!("failed to run wg show {iface} public-key"))?;

    if !output.status.success() {
        bail!(
            "wg show {iface} public-key failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let pubkey = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if pubkey.is_empty() {
        bail!("wg show {iface} public-key returned an empty key");
    }
    Ok(pubkey)
}

fn read_sled_view(path: &str) -> Result<SledView> {
    let bytes = read_file(path)?;
    if bytes.len() >= IdentitySled::SIZE {
        let sled = read_full_sled(&bytes)?;
        return Ok(SledView::from_full(path.to_string(), &sled));
    }
    if bytes.len() >= COMPACT_SLED_SIZE {
        return Ok(SledView::from_compact(path.to_string(), &bytes));
    }
    bail!(
        "sled too short: {} bytes, expected at least {} for compact layout or {} for canonical layout",
        bytes.len(),
        COMPACT_SLED_SIZE,
        IdentitySled::SIZE
    )
}

fn read_file(path: &str) -> Result<Vec<u8>> {
    let mut file = File::open(path).with_context(|| format!("failed to open {path}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {path}"))?;
    Ok(bytes)
}

fn read_full_sled(bytes: &[u8]) -> Result<IdentitySled> {
    let mut sled = mem::MaybeUninit::<IdentitySled>::uninit();
    unsafe {
        std::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            sled.as_mut_ptr() as *mut u8,
            IdentitySled::SIZE,
        );
        Ok(sled.assume_init())
    }
}

/// Check whether a sled is "valid" per the Absolute Base rule.
fn is_sled_valid(sled: &IdentitySled) -> bool {
    sled.hashed_footprint != [0u8; 32] && sled.trace_id != [0u8; 16]
}

impl SledView {
    fn from_full(path: String, sled: &IdentitySled) -> Self {
        let schema_catalog_hash = std::fs::read("/dev/shm/live-schema.json")
            .map(|bytes| hex::encode(blake3::hash(&bytes).as_bytes()))
            .unwrap_or_else(|_| "(missing)".to_string());

        Self {
            path,
            layout: "canonical",
            size: IdentitySled::SIZE,
            is_valid: is_sled_valid(sled),
            wireguard_pubkey_hex: hex::encode(sled.wireguard_pubkey),
            wireguard_pubkey_b64: encode_b64(&sled.wireguard_pubkey),
            mutation_index: sled.mutation_index,
            hashed_footprint: hex::encode(sled.hashed_footprint),
            schema_catalog_hash,
            trace_id: sled.trace_id_hex(),
            schema_version: sled.schema_version,
        }
    }

    fn from_compact(path: String, bytes: &[u8]) -> Self {
        let mut wg = [0u8; 32];
        wg.copy_from_slice(&bytes[0..32]);
        let mutation_index = u64::from_le_bytes(
            bytes[COMPACT_MUTATION_OFFSET..COMPACT_MUTATION_OFFSET + 8]
                .try_into()
                .expect("compact mutation range is fixed"),
        );
        let is_valid = bytes[COMPACT_VALID_OFFSET] != 0;
        let footprint = &bytes[COMPACT_FOOTPRINT_OFFSET..COMPACT_FOOTPRINT_OFFSET + 32];
        let schema_catalog_hash = std::fs::read("/dev/shm/live-schema.json")
            .map(|bytes| hex::encode(blake3::hash(&bytes).as_bytes()))
            .unwrap_or_else(|_| "(missing)".to_string());

        Self {
            path,
            layout: "compact",
            size: COMPACT_SLED_SIZE,
            is_valid,
            wireguard_pubkey_hex: hex::encode(wg),
            wireguard_pubkey_b64: encode_b64(&wg),
            mutation_index,
            hashed_footprint: hex::encode(footprint),
            schema_catalog_hash,
            trace_id: trace_id(&wg, mutation_index),
            schema_version: 0,
        }
    }
}

fn trace_id(wireguard_pubkey: &[u8; 32], mutation_index: u64) -> String {
    format!("{}-{}", hex::encode(&wireguard_pubkey[..4]), mutation_index)
}

fn encode_b64(bytes: &[u8; 32]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn format_uuid(bytes: &[u8; 16]) -> String {
    if bytes.iter().all(|b| *b == 0) {
        return String::new();
    }
    format!(
        "{}-{}-{}-{}-{}",
        hex::encode(&bytes[0..4]),
        hex::encode(&bytes[4..6]),
        hex::encode(&bytes[6..8]),
        hex::encode(&bytes[8..10]),
        hex::encode(&bytes[10..16])
    )
}

fn split_words(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_string).collect()
}

fn print_pretty(view: &SledView) {
    println!("path: {}", view.path);
    println!("layout: {}", view.layout);
    println!("valid: {}", view.is_valid);
    println!("wg_pubkey: {}", view.wireguard_pubkey_b64);
    println!("mutation_index: {}", view.mutation_index);
    println!("schema_catalog_hash: {}", view.schema_catalog_hash);
    println!("footprint: {}", view.hashed_footprint);
    println!("trace_id: {}", view.trace_id);
    println!("schema_version: {}", view.schema_version);
}
</file>

<file path="src/anna_scribe.rs">
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
    /// strictly for the snowball transport.
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

        // Path is in tmpfs to preserve NVMe I/O for Btrfs snowball transport
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
</file>

<file path="src/gcloud_auth.rs">
//! Google Cloud authentication for cloudaicompanion.googleapis.com
//!
//! Supports multiple token sources:
//! 1. Environment variable (GCLOUD_TOKEN)
//! 2. Cached token from antigravity-server
//! 3. gcloud CLI
//! 4. Application Default Credentials

use std::path::PathBuf;
use std::process::Command;

use chrono::{DateTime, Duration, Utc};
use tracing::{debug, info, warn};

/// OAuth scopes required for Cloud AI Companion
pub const OAUTH_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/cloud-ide",
];
const OAUTH_SCOPES_FALLBACK: &[&str] = &["https://www.googleapis.com/auth/cloud-platform"];

fn adc_fallback_enabled() -> bool {
    std::env::var("OP_ENABLE_ADC_FALLBACK")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// GCloud authentication provider
#[derive(Clone)]
pub struct GCloudAuth {
    /// Path to cached token from antigravity-server
    antigravity_token_path: Option<PathBuf>,
}

impl GCloudAuth {
    pub fn new() -> Self {
        // Look for antigravity token
        let antigravity_token_path = dirs::home_dir()
            .map(|h| h.join(".antigravity-server"))
            .and_then(|dir| {
                // Find any .token file in the directory
                std::fs::read_dir(&dir)
                    .ok()?
                    .filter_map(|e| e.ok())
                    .find(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "token")
                            .unwrap_or(false)
                    })
                    .map(|e| e.path())
            });

        if let Some(ref path) = antigravity_token_path {
            debug!("Found antigravity token at: {:?}", path);
        }

        Self {
            antigravity_token_path,
        }
    }

    /// Get a valid OAuth token and its expiration time
    pub async fn get_token(&self) -> anyhow::Result<(String, DateTime<Utc>)> {
        // Try sources in order of preference

        // 1. Environment variable (for testing)
        if let Ok(token) = std::env::var("GCLOUD_TOKEN") {
            info!("Using token from GCLOUD_TOKEN env var");
            // Assume 1 hour validity
            return Ok((token, Utc::now() + Duration::hours(1)));
        }

        // 2. Antigravity cached token
        if let Some(token) = self.try_antigravity_token().await {
            info!("Using token from antigravity cache");
            // These tokens are typically valid for 1 hour
            return Ok((token, Utc::now() + Duration::minutes(55)));
        }

        // 3. gcloud CLI
        if let Some((token, expires)) = self.try_gcloud_cli().await {
            info!("Using token from gcloud CLI");
            return Ok((token, expires));
        }

        // 4. Application Default Credentials via gcloud (opt-in).
        if adc_fallback_enabled() {
            if let Some((token, expires)) = self.try_adc().await {
                info!("Using Application Default Credentials");
                return Ok((token, expires));
            }
        } else {
            debug!("ADC fallback disabled (set OP_ENABLE_ADC_FALLBACK=1 to enable)");
        }

        anyhow::bail!(
            "Could not obtain OAuth token from GCLOUD_TOKEN, cached token file, or gcloud CLI credentials"
        )
    }

    async fn try_antigravity_token(&self) -> Option<String> {
        let path = self.antigravity_token_path.as_ref()?;

        let content = std::fs::read_to_string(path).ok()?;
        let token = content.trim().to_string();

        if token.is_empty() {
            return None;
        }

        // Basic validation - OAuth tokens start with "ya29."
        if token.starts_with("ya29.") {
            Some(token)
        } else {
            warn!("Antigravity token doesn't look like an OAuth token");
            None
        }
    }

    async fn try_gcloud_cli(&self) -> Option<(String, DateTime<Utc>)> {
        if let Some(token) = run_gcloud_access_token(&["auth", "print-access-token"], OAUTH_SCOPES)
        {
            return Some((token, Utc::now() + Duration::minutes(55)));
        }
        warn!("Preferred scopes failed; retrying gcloud CLI token with cloud-platform only");
        if let Some(token) =
            run_gcloud_access_token(&["auth", "print-access-token"], OAUTH_SCOPES_FALLBACK)
        {
            return Some((token, Utc::now() + Duration::minutes(55)));
        }
        // Final fallback: let gcloud decide default scopes.
        if let Some(token) = run_gcloud_access_token_no_scopes(&["auth", "print-access-token"]) {
            return Some((token, Utc::now() + Duration::minutes(55)));
        }
        None
    }

    async fn try_adc(&self) -> Option<(String, DateTime<Utc>)> {
        if let Some(token) = run_gcloud_access_token(
            &["auth", "application-default", "print-access-token"],
            OAUTH_SCOPES,
        ) {
            return Some((token, Utc::now() + Duration::minutes(55)));
        }
        warn!("Preferred scopes failed; retrying ADC token with cloud-platform only");
        if let Some(token) = run_gcloud_access_token(
            &["auth", "application-default", "print-access-token"],
            OAUTH_SCOPES_FALLBACK,
        ) {
            return Some((token, Utc::now() + Duration::minutes(55)));
        }
        // Final fallback: let gcloud decide default scopes.
        if let Some(token) = run_gcloud_access_token_no_scopes(&[
            "auth",
            "application-default",
            "print-access-token",
        ]) {
            return Some((token, Utc::now() + Duration::minutes(55)));
        }
        None
    }

    /// Force a token refresh via gcloud
    pub async fn refresh_token(&self) -> anyhow::Result<(String, DateTime<Utc>)> {
        if let Some(token) = run_gcloud_access_token(&["auth", "print-access-token"], OAUTH_SCOPES)
        {
            return Ok((token, Utc::now() + Duration::minutes(55)));
        }
        if let Some(token) =
            run_gcloud_access_token(&["auth", "print-access-token"], OAUTH_SCOPES_FALLBACK)
        {
            return Ok((token, Utc::now() + Duration::minutes(55)));
        }
        if let Some(token) = run_gcloud_access_token_no_scopes(&["auth", "print-access-token"]) {
            return Ok((token, Utc::now() + Duration::minutes(55)));
        }

        anyhow::bail!("gcloud auth failed for preferred, fallback, and default scope sets")
    }

    /// Check if gcloud is available and authenticated
    pub fn is_authenticated(&self) -> bool {
        if run_gcloud_access_token_no_scopes(&["auth", "print-access-token"]).is_some() {
            return true;
        }
        if adc_fallback_enabled()
            && run_gcloud_access_token_no_scopes(&[
                "auth",
                "application-default",
                "print-access-token",
            ])
            .is_some()
        {
            return true;
        }
        false
    }
}

impl Default for GCloudAuth {
    fn default() -> Self {
        Self::new()
    }
}

fn run_gcloud_access_token(base_args: &[&str], scopes: &[&str]) -> Option<String> {
    let mut args: Vec<String> = base_args.iter().map(|s| s.to_string()).collect();
    args.push(format!("--scopes={}", scopes.join(",")));

    let output = Command::new("gcloud").args(args).output().ok()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        debug!("gcloud {:?} failed: {}", base_args, stderr);
        return None;
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        return None;
    }
    Some(token)
}

fn run_gcloud_access_token_no_scopes(base_args: &[&str]) -> Option<String> {
    let output = Command::new("gcloud").args(base_args).output().ok()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        debug!("gcloud {:?} without scopes failed: {}", base_args, stderr);
        return None;
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        return None;
    }
    Some(token)
}
</file>

<file path="src/lib.rs">
//! Identity crate – WireGuard pubkey as identity + OAuth token cache via
//! org.freedesktop.secrets. Zero passwords; the WireGuard handshake is the login.

pub mod anna_scribe;
pub mod gcloud_auth;
pub mod registration;
pub mod schema_bridge;
pub mod session;
pub mod token; // Keeping for now if needed internally
pub mod wireguard;

pub use anna_scribe::{AnnaScribe, SessionLedger};
pub use gcloud_auth::GCloudAuth;
pub use registration::{generate_magic_link_token, generate_wireguard_keypair, WireGuardKeyPair};
pub use schema_bridge::{
    read_sled, run_schema_shuttle, socket_entries_from_env, watch_wireguard_handshakes, write_sled,
    write_sled_from_wg, write_sled_full, IdentitySled, SocketEntry, SubidCategory, SubidTaxonomy,
    SHM_SLED_PATH, SHM_XRAY_CONFIG,
};
pub use session::{Session, SessionManager};
pub use token::{CachedToken, TokenManager};
pub use wireguard::{PeerInfo, WireGuardIdentity};
</file>

<file path="src/registration.rs">
//! Registration helpers for signup flows.
//!
//! Centralize WireGuard key generation and magic-link token creation so
//! web/API layers can reuse a single identity implementation.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::distributions::Alphanumeric;
use rand::rngs::OsRng;
use rand::Rng;
use x25519_dalek::{PublicKey, StaticSecret};

/// WireGuard keypair used for user identity and VPN config.
#[derive(Debug, Clone)]
pub struct WireGuardKeyPair {
    pub private_key: String,
    pub public_key: String,
}

/// Generate a new WireGuard keypair.
pub fn generate_wireguard_keypair() -> WireGuardKeyPair {
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);

    WireGuardKeyPair {
        private_key: BASE64.encode(secret.as_bytes()),
        public_key: BASE64.encode(public.as_bytes()),
    }
}

/// Generate a random token suitable for magic-link flows.
pub fn generate_magic_link_token(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_wireguard_keypair() {
        let keypair = generate_wireguard_keypair();
        assert_eq!(keypair.private_key.len(), 44);
        assert_eq!(keypair.public_key.len(), 44);
    }

    #[test]
    fn generates_magic_token_with_requested_length() {
        let token = generate_magic_link_token(32);
        assert_eq!(token.len(), 32);
    }
}
</file>

<file path="src/schema_bridge.rs">
//! The Identity Sled & Shuttle Bridge
//!
//! THE SLED: a `#[repr(C)]` struct written into `/dev/shm/plugin_schema.dat`
//! by the SchemaEngine whenever the active PluginSchema mutates.
//!
//! THE SHUTTLE: reads that mapping via zero-copy mmap, extracts the
//! Strike/Etch (hashed footprint + WireGuard identity), stamps
//! `GB_FOOTPRINT` / `GB_TRACE_ID` into the process environment, then
//! writes a stateless Xray config into `/dev/shm/xray-ghostbridge.json`
//! and spawns Xray — all without touching any Btrfs-backed path.

use memmap2::MmapOptions;
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

pub const SHM_SLED_PATH: &str = "/dev/shm/plugin_schema.dat";
pub const SHM_XRAY_CONFIG: &str = "/dev/shm/xray-ghostbridge.json";

// ── Subid taxonomy ────────────────────────────────────────────────────────────

/// Seven operational categories for the subid taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubidCategory {
    /// Authoritative source / ingress channel.
    Src,
    /// Source-to-object projection / mirror publication.
    Prj,
    /// Schema contract / vocabulary / control-mapping artifact.
    Sch,
    /// Write-path mutation / state change.
    Mut,
    /// Read-path observation / enumeration / discovery.
    Obs,
    /// Emitted signal / audit-chain event / proof.
    Evt,
    /// Consumer-facing rendering / materialized presentation surface.
    Exp,
}

impl SubidCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Src => "src",
            Self::Prj => "prj",
            Self::Sch => "sch",
            Self::Mut => "mut",
            Self::Obs => "obs",
            Self::Evt => "evt",
            Self::Exp => "exp",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "src" => Some(Self::Src),
            "prj" => Some(Self::Prj),
            "sch" => Some(Self::Sch),
            "mut" => Some(Self::Mut),
            "obs" => Some(Self::Obs),
            "evt" => Some(Self::Evt),
            "exp" => Some(Self::Exp),
            _ => None,
        }
    }
}

/// Parsed subid components.
///
/// Pattern: `<category>.<component-type>.<subject>.<verb>[.<facet>][@vN]`
/// Example:  `sch.network.plugin-schema.resolve@v1`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubidTaxonomy {
    pub category: SubidCategory,
    pub component_type: String,
    pub subject: String,
    pub verb: String,
    pub facet: Option<String>,
    pub version: u8,
}

impl SubidTaxonomy {
    /// Parse a subid string into its taxonomy components.
    pub fn parse(s: &str) -> Result<Self, String> {
        // Strip optional @vN suffix
        let (body, version) = if let Some(at) = s.rfind('@') {
            let ver_str = &s[at + 1..];
            let ver: u8 = ver_str
                .strip_prefix('v')
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| format!("invalid version suffix: {ver_str}"))?;
            (&s[..at], ver)
        } else {
            (s, 0)
        };

        let mut parts = body.splitn(5, '.');
        let cat_str = parts.next().ok_or("missing category")?;
        let category = SubidCategory::from_str(cat_str)
            .ok_or_else(|| format!("unknown category: {cat_str}"))?;
        let component_type = parts.next().ok_or("missing component-type")?.to_string();
        let subject = parts.next().ok_or("missing subject")?.to_string();
        let verb = parts.next().ok_or("missing verb")?.to_string();
        let facet = parts.next().map(str::to_string);

        // Basic segment validation: lowercase ASCII + hyphens only
        for seg in [&component_type, &subject, &verb] {
            if seg.is_empty()
                || !seg
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit())
            {
                return Err(format!(
                    "invalid segment '{seg}': must be lowercase ascii/digits/hyphens"
                ));
            }
        }

        Ok(Self {
            category,
            component_type,
            subject,
            verb,
            facet,
            version,
        })
    }

    /// Reconstruct the canonical subid string.
    pub fn canonical(&self) -> String {
        let mut s = format!(
            "{}.{}.{}.{}",
            self.category.as_str(),
            self.component_type,
            self.subject,
            self.verb,
        );
        if let Some(f) = &self.facet {
            s.push('.');
            s.push_str(f);
        }
        if self.version > 0 {
            s.push_str(&format!("@v{}", self.version));
        }
        s
    }
}

impl std::fmt::Display for SubidTaxonomy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.canonical())
    }
}

/// THE SLED — 1:1 zero-copy shared memory layout.
///
/// Written by SchemaEngine (`write_sled`), read by the Shuttle via mmap.
/// Never touches disk; lives entirely in tmpfs (`/dev/shm`).
///
/// Matches `.kiro/specs/3tched-schema-shuttle-xray-pipeline` exactly.
/// Layout (152 bytes total):
///   wireguard_pubkey    [u8; 32]   offset 0
///   mutation_index      u64        offset 32
///   hashed_footprint    [u8; 32]   offset 40   (Blake3)
///   trace_id            [u8; 16]   offset 72   (UUID v4, network order)
        vector_id: [0u8; 16],
///   schema_version      u32        offset 88
///   reserved            [u8; 60]   offset 92
#[repr(C)]
pub struct IdentitySled {
    /// Raw Curve25519 WireGuard peer key.
    pub wireguard_pubkey: [u8; 32],
    /// Monotonic schema mutation counter.
    pub mutation_index: u64,
    /// Blake3 hashed footprint of the canonicalized schema state.
    pub hashed_footprint: [u8; 32],
    /// UUID v4 trace ID (16 raw bytes, network order).
    pub trace_id: [u8; 16],
        vector_id: [0u8; 16],
    /// Schema version for compatibility.
    pub schema_version: u32,
    /// Qdrant vector ID for the last reasoning episode (UUID v4, 16 raw bytes).
    /// Bound to identity: every vectorized episode is traceable to this sled.
    pub vector_id: [u8; 16],
    /// Reserved for future use (zero-initialized).
    pub reserved: [u8; 44],
}

impl IdentitySled {
    pub const SIZE: usize = std::mem::size_of::<Self>();

    /// Hex-encode the trace_id field for header injection.
    pub fn trace_id_hex(&self) -> String {
        hex::encode(self.trace_id)
    }

    /// Hex-encode the vector_id for Qdrant upsert / lookup.
    pub fn vector_id_hex(&self) -> String {
        hex::encode(self.vector_id)
    }

    /// UUID string representation of vector_id.
    pub fn vector_id_uuid(&self) -> String {
        let b = self.vector_id;
        format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7],b[8],b[9],b[10],b[11],b[12],b[13],b[14],b[15]
        )
    }

    /// Absolute Base validity check per the spec:
    /// a sled is valid only if its footprint and trace_id are non-zero.
    pub fn is_sled_valid(&self) -> bool {
        self.hashed_footprint != [0u8; 32] && self.trace_id != [0u8; 16]
    }
}

// ── Zero-Btrfs disk-I/O guard ───────────────────────────────────────────────

/// Linux tmpfs magic number (`TMPFS_MAGIC`).
const TMPFS_MAGIC: libc::c_long = 0x01021994;

/// Abort if the directory backing `path` is not mounted on tmpfs.
///
/// The Shuttle must never trigger unintended Btrfs mutation loops.
/// NVMe I/O is reserved strictly for the vectorized footprint transport
/// (snowball); all sled and Xray config writes must live in tmpfs.
fn assert_tmpfs_or_abort(path: &str) -> std::io::Result<()> {
    let parent = std::path::Path::new(path)
        .parent()
        .unwrap_or(std::path::Path::new("/"));

    let c_path = std::ffi::CString::new(parent.as_os_str().as_encoded_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let mut buf: libc::statfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statfs(c_path.as_ptr(), &mut buf) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }

    if buf.f_type as libc::c_long != TMPFS_MAGIC {
        tracing::error!(
            path = %path,
            parent = %parent.display(),
            f_type = buf.f_type,
            "ABORT: sled write would hit disk (not tmpfs). Zero-Btrfs rule violated."
        );
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "Zero-Btrfs Overhead violation: {} is on filesystem type {}, not tmpfs",
                parent.display(),
                buf.f_type
            ),
        ));
    }

    Ok(())
}

// ── Writer side (called from SchemaEngine) ───────────────────────────────────

/// Atomically write the active sled into `/dev/shm`.
///
/// Uses a tmp-file + rename so readers never see a partial write.
/// Aborts if the target path is not on tmpfs (Zero-Btrfs Overhead rule).
pub fn write_sled(sled: &IdentitySled) -> std::io::Result<()> {
    assert_tmpfs_or_abort(SHM_SLED_PATH)?;

    let tmp = format!("{}.tmp", SHM_SLED_PATH);
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(sled as *const IdentitySled as *const u8, IdentitySled::SIZE)
    };
    let mut f = File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_data()?;
    fs::rename(&tmp, SHM_SLED_PATH)?;
    Ok(())
}

// ── Reader side (the Shuttle) ────────────────────────────────────────────────

/// Read the sled from shm — zero copy, no allocation.
///
/// # Safety
/// The caller must ensure no concurrent writer is modifying the file
/// mid-read.  `write_sled` uses rename so this is safe in practice.
pub fn read_sled() -> std::io::Result<(*const IdentitySled, memmap2::Mmap)> {
    let file = File::open(SHM_SLED_PATH)?;
    let mmap = unsafe { MmapOptions::new().len(IdentitySled::SIZE).map(&file)? };
    let ptr = mmap.as_ptr() as *const IdentitySled;
    Ok((ptr, mmap))
}

// ── Unix socket endpoint ──────────────────────────────────────────────────────

/// A unix socket endpoint proxied into an xray inbound + outbound pair.
///
/// Declared via `UNIX_SOCKET_ENDPOINTS=label:path:port[,…]`, e.g.:
///   `qdrant:/run/qdrant.sock:6334`
#[derive(Debug, Clone)]
pub struct SocketEntry {
    /// Xray tag suffix (e.g. `"qdrant"`) — becomes `"to-<label>"` / `"<label>-in"`.
    pub label: String,
    /// Filesystem path of the unix domain socket.
    pub path: String,
    /// Local TCP port xray should listen on and proxy into the socket.
    pub port: u16,
}

/// Parse `UNIX_SOCKET_ENDPOINTS` env var into a list of `SocketEntry`.
///
/// Format: `label:/path/to/sock:port[,…]`
/// Example: `qdrant:/run/qdrant.sock:6334`
pub fn socket_entries_from_env() -> Vec<SocketEntry> {
    let Ok(raw) = env::var("UNIX_SOCKET_ENDPOINTS") else {
        return vec![];
    };
    raw.split(',')
        .filter_map(|entry| {
            // Split into exactly 3 parts: label, path, port
            let mut parts = entry.trim().splitn(3, ':');
            let label = parts.next()?.to_string();
            let path = parts.next()?.to_string(); // already has leading '/'
            let port: u16 = parts.next()?.trim().parse().ok()?;
            Some(SocketEntry { label, path, port })
        })
        .collect()
}

// ── Xray config generator ─────────────────────────────────────────────────────

fn write_xray_config(
    footprint: &str,
    trace_id: &str,
    nextdns_profile: &str,
    uuid: &str,
    private_key: &str,
    short_id: &str,
) -> std::io::Result<()> {
    let sockets = socket_entries_from_env();
    write_xray_config_with_sockets(
        footprint,
        trace_id,
        nextdns_profile,
        uuid,
        private_key,
        short_id,
        &sockets,
    )
}

fn write_xray_config_with_sockets(
    footprint: &str,
    trace_id: &str,
    nextdns_profile: &str,
    uuid: &str,
    private_key: &str,
    short_id: &str,
    sockets: &[SocketEntry],
) -> std::io::Result<()> {
    // Build socket inbounds: one dokodemo-door per unix socket endpoint.
    let socket_inbounds: String = sockets
        .iter()
        .map(|s| {
            format!(
                r#",
    {{
      "tag": "{label}-in",
      "port": {port},
      "listen": "127.0.0.1",
      "protocol": "dokodemo-door",
      "settings": {{ "network": "tcp", "address": "127.0.0.1", "port": {port} }}
    }}"#,
                label = s.label,
                port = s.port,
            )
        })
        .collect();

    // Build socket outbounds: freedom via xray domain-socket transport.
    let socket_outbounds: String = sockets
        .iter()
        .map(|s| {
            format!(
                r#",
    {{
      "tag": "to-{label}",
      "protocol": "freedom",
      "streamSettings": {{
        "network": "ds",
        "dsSettings": {{ "path": "{path}", "abstract": false, "padding": false }}
      }}
    }}"#,
                label = s.label,
                path = s.path,
            )
        })
        .collect();

    // Build socket routing rules: inbound tag → outbound tag.
    let socket_rules: String = sockets
        .iter()
        .map(|s| {
            format!(
                r#",
      {{ "type": "field", "inboundTag": ["{label}-in"], "outboundTag": "to-{label}" }}"#,
                label = s.label,
            )
        })
        .collect();

    let config = format!(
        r#"{{
  "log": {{ "loglevel": "warning" }},
  "dns": {{
    "servers": [ "https://dns.nextdns.io/{profile}/Ghostbridge-Incus" ],
    "tag": "nextdns-in"
  }},
  "inbounds": [
    {{
      "tag": "reality-in",
      "port": 443,
      "listen": "0.0.0.0",
      "protocol": "vless",
      "settings": {{
        "clients": [{{ "id": "{uuid}", "flow": "xtls-rprx-vision" }}],
        "decryption": "none"
      }},
      "streamSettings": {{
        "network": "tcp",
        "security": "reality",
        "realitySettings": {{
          "show": false,
          "dest": "www.microsoft.com:443",
          "serverNames": ["www.microsoft.com"],
          "privateKey": "{private_key}",
          "shortIds": ["{short_id}"]
        }}
      }}
    }},
    {{
      "tag": "ovs-socks-in",
      "port": 1080,
      "listen": "10.200.0.1",
      "protocol": "socks",
      "settings": {{ "auth": "noauth", "udp": true }}
    }},
    {{
      "tag": "ovs-tproxy-in",
      "port": 12345,
      "listen": "10.200.0.1",
      "protocol": "dokodemo-door",
      "settings": {{ "network": "tcp,udp", "followRedirect": true }},
      "streamSettings": {{ "sockopt": {{ "tproxy": "tproxy" }} }}
    }}{socket_inbounds}
  ],
  "outbounds": [
    {{
      "tag": "to-grpc-bridge",
      "protocol": "freedom",
      "settings": {{ "redirect": "127.0.0.1:8090" }},
      "streamSettings": {{
        "network": "grpc",
        "sockopt": {{ "tcpNoDelay": true, "mark": 255 }},
        "grpcSettings": {{
          "serviceName": "Ghostbridge.StateSync",
          "multiMode": true,
          "metadata": {{
            "X-Ghostbridge-Footprint": "{footprint}",
            "X-Ghostbridge-Trace-ID": "{trace_id}"
          }}
        }}
      }}
    }},
    {{
      "tag": "to-cognitive-mcp",
      "protocol": "freedom",
      "sendThrough": "10.200.0.1",
      "settings": {{ "redirect": "10.200.0.2:50052" }},
      "streamSettings": {{
        "network": "grpc",
        "sockopt": {{ "tcpNoDelay": true, "mark": 255 }},
        "grpcSettings": {{
          "serviceName": "operation.cognitive.v1.CognitiveToolService",
          "multiMode": true,
          "metadata": {{
            "X-Ghostbridge-Footprint": "{footprint}",
            "X-Ghostbridge-Trace-ID": "{trace_id}"
          }}
        }}
      }}
    }},
    {{
      "tag": "direct",
      "protocol": "freedom"
    }},
    {{ "tag": "dns-out", "protocol": "dns" }}{socket_outbounds}
  ],
  "routing": {{
    "domainStrategy": "IPIfNonMatch",
    "rules": [
      {{ "type": "field", "port": 53, "outboundTag": "dns-out" }},
      {{
        "type": "field",
        "inboundTag": ["ovs-socks-in", "ovs-tproxy-in"],
        "domain": ["full:mcp.internal"],
        "outboundTag": "to-cognitive-mcp"
      }},
      {{
        "type": "field",
        "inboundTag": ["ovs-socks-in", "ovs-tproxy-in"],
        "domain": ["full:dashboard.3tched.com", "full:grpc.internal"],
        "outboundTag": "to-grpc-bridge"
      }}{socket_rules},
      {{ "type": "field", "network": "tcp,udp", "outboundTag": "direct" }}
    ]
  }}
}}"#,
        profile = nextdns_profile,
        footprint = footprint,
        trace_id = trace_id,
        uuid = uuid,
        private_key = private_key,
        short_id = short_id,
        socket_inbounds = socket_inbounds,
        socket_outbounds = socket_outbounds,
        socket_rules = socket_rules,
    );

    let tmp = format!("{}.tmp", SHM_XRAY_CONFIG);
    let mut f = File::create(&tmp)?;
    f.write_all(config.as_bytes())?;
    f.sync_data()?;
    fs::rename(&tmp, SHM_XRAY_CONFIG)?;
    Ok(())
}

// ── WireGuard-driven sled writer ─────────────────────────────────────────────

static MUTATION_INDEX: AtomicU64 = AtomicU64::new(0);

/// Decode a base64 WireGuard public key (32-byte Curve25519) into raw bytes.
fn decode_wg_pubkey(b64: &str) -> [u8; 32] {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()
        .and_then(|v| v.try_into().ok())
        .unwrap_or([0u8; 32])
}

/// Build and atomically write the sled from live WireGuard state.
///
/// Reads `GB_TRACE_ID` from environment to propagate an existing trace;
/// if absent, mints a fresh UUID v4.  All extra metadata (subid, compliance,
/// routing) lives in environment variables — the sled itself is the spec
/// layout and nothing more.
pub fn write_sled_from_wg(peer_pubkey: &str) -> std::io::Result<()> {
    let wireguard_pubkey = decode_wg_pubkey(peer_pubkey);
    let mutation_index = MUTATION_INDEX.fetch_add(1, Ordering::Relaxed);

    // Blake3 Strike/Etch: bind pubkey to the canonical schema catalog in shm.
    // The footprint is a direct function of the single source of truth.
    let schema_catalog_hash = std::fs::read("/dev/shm/live-schema.json")
        .map(|bytes| blake3::hash(&bytes))
        .unwrap_or_else(|_| blake3::Hash::from([0u8; 32]));

    let mut hasher = blake3::Hasher::new();
    hasher.update(&wireguard_pubkey);
    hasher.update(schema_catalog_hash.as_bytes());
    hasher.update(&mutation_index.to_le_bytes());
    let hashed_footprint = hasher.finalize().into();

    // Trace propagation: reuse existing UUID if present, else mint v4
    let trace_id: [u8; 16] = if let Ok(existing) = env::var("GB_TRACE_ID") {
        hex::decode(existing.trim())
            .ok()
            .and_then(|v| v.try_into().ok())
            .unwrap_or_else(|| uuid::Uuid::new_v4().into_bytes())
    } else {
        uuid::Uuid::new_v4().into_bytes()
    };

    let sled = IdentitySled {
        wireguard_pubkey,
        mutation_index,
        hashed_footprint,
        trace_id,
        vector_id: [0u8; 16],
        schema_version: 1,
        reserved: [0u8; 44],
    };
    write_sled(&sled)
}

/// Write the sled with fully explicit fields — called from SchemaEngine on mutation.
///
/// `trace_id` is passed as hex; if empty a fresh UUID v4 is minted.
pub fn write_sled_full(
    peer_pubkey: &str,
    mutation_index: u64,
    trace_id_hex: &str,
) -> std::io::Result<()> {
    let wireguard_pubkey = decode_wg_pubkey(peer_pubkey);

    // Blake3 Strike/Etch: bind pubkey to the canonical schema catalog in shm.
    // The footprint is a direct function of the single source of truth.
    let schema_catalog_hash = std::fs::read("/dev/shm/live-schema.json")
        .map(|bytes| blake3::hash(&bytes))
        .unwrap_or_else(|_| blake3::Hash::from([0u8; 32]));

    let mut hasher = blake3::Hasher::new();
    hasher.update(&wireguard_pubkey);
    hasher.update(schema_catalog_hash.as_bytes());
    hasher.update(&mutation_index.to_le_bytes());
    let hashed_footprint = hasher.finalize().into();

    let trace_id: [u8; 16] = if trace_id_hex.is_empty() {
        uuid::Uuid::new_v4().into_bytes()
    } else {
        hex::decode(trace_id_hex.trim())
            .ok()
            .and_then(|v| v.try_into().ok())
            .unwrap_or_else(|| uuid::Uuid::new_v4().into_bytes())
    };

    let sled = IdentitySled {
        wireguard_pubkey,
        mutation_index,
        hashed_footprint,
        trace_id,
        vector_id: [0u8; 16],
        schema_version: 1,
        reserved: [0u8; 44],
    };
    write_sled(&sled)
}

/// Poll `wg show <iface> latest-handshakes` and re-write the sled + xray config
/// whenever a new peer handshake is detected.  Runs forever; call from a thread.
pub fn watch_wireguard_handshakes(iface: &str) {
    let iface = iface.to_string();
    let poll_secs = std::time::Duration::from_secs(15);
    let mut seen: HashSet<String> = HashSet::new();

    loop {
        std::thread::sleep(poll_secs);

        // wg0 lives inside the wg-xray Incus container, not on the host.
        let Ok(out) = Command::new("incus")
            .args([
                "exec",
                "wg-xray",
                "--",
                "wg",
                "show",
                &iface,
                "latest-handshakes",
            ])
            .output()
        else {
            continue;
        };

        if !out.status.success() {
            continue;
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            let mut parts = line.split('\t');
            let (Some(pubkey), Some(ts_str)) = (parts.next(), parts.next()) else {
                continue;
            };
            let ts: u64 = ts_str.trim().parse().unwrap_or(0);
            if ts == 0 {
                continue;
            }

            // Treat any handshake within the last 3 minutes as "new" if not yet seen
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now.saturating_sub(ts) > 180 {
                continue;
            }

            let key = format!("{}:{}", pubkey, ts);
            if seen.contains(&key) {
                continue;
            }
            seen.insert(key);

            tracing::info!(peer = %pubkey, "WireGuard handshake → updating identity sled");

            if let Err(e) = write_sled_from_wg(pubkey) {
                tracing::warn!("write_sled_from_wg failed: {}", e);
                continue;
            }

            // Re-bake xray config — NextDNS profile from env, trace_id from sled.
            if let Ok((ptr, _mmap)) = read_sled() {
                let sled = unsafe { &*ptr };
                let footprint_hex = hex::encode(sled.hashed_footprint);
                let trace_id = sled.trace_id_hex();
                let profile = env::var("NEXTDNS_PROFILE_ID")
                    .unwrap_or_else(|_| "689ec7".to_string());
                let uuid = env::var("XRAY_UUID")
                    .unwrap_or_else(|_| "40813c05-4a7c-4d5b-b027-33912551287f".to_string());
                let privkey = env::var("XRAY_PRIVATE_KEY")
                    .unwrap_or_else(|_| "-MULA7gIbk_58CKa4TNHovpYNt192NUkPlQF7f3caWo".to_string());
                let short =
                    env::var("XRAY_SHORT_ID").unwrap_or_else(|_| "2a32c53278372687".to_string());
                if let Err(e) =
                    write_xray_config(&footprint_hex, &trace_id, &profile, &uuid, &privkey, &short)
                {
                    tracing::warn!("write_xray_config failed: {}", e);
                }
            }
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Run the Shuttle: map the Sled, stamp env vars, write Xray config, spawn Xray.
///
/// This is the only function that should be called from the shuttle binary.
/// It intentionally avoids all SQLite, D-Bus, and Btrfs-backed paths.
pub fn run_schema_shuttle() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Zero-copy read of the Absolute Base Schema
    let (ptr, _mmap) = read_sled()?;
    let sled = unsafe { &*(ptr) };

    // 2. Extract Strike/Etch — hex-encode for env injection
    let footprint_hex = hex::encode(sled.hashed_footprint);
    let trace_id = sled.trace_id_hex();
    let wg_pubkey_hex = hex::encode(sled.wireguard_pubkey);

    // 3. Stamp into environment — zero Btrfs, zero disk I/O
    env::set_var("GB_FOOTPRINT", &footprint_hex);
    env::set_var("GB_TRACE_ID", &trace_id);
    env::set_var("GB_WIREGUARD_PUBKEY", &wg_pubkey_hex);

    // 4. Write stateless Xray config — NextDNS profile from env, not sled.
    let nextdns_profile = env::var("NEXTDNS_PROFILE_ID")
        .unwrap_or_else(|_| "689ec7".to_string());
    let xray_uuid = env::var("XRAY_UUID")
        .unwrap_or_else(|_| "40813c05-4a7c-4d5b-b027-33912551287f".to_string());
    let xray_privkey = env::var("XRAY_PRIVATE_KEY")
        .unwrap_or_else(|_| "-MULA7gIbk_58CKa4TNHovpYNt192NUkPlQF7f3caWo".to_string());
    let xray_short_id =
        env::var("XRAY_SHORT_ID").unwrap_or_else(|_| "2a32c53278372687".to_string());
    write_xray_config(
        &footprint_hex,
        &trace_id,
        &nextdns_profile,
        &xray_uuid,
        &xray_privkey,
        &xray_short_id,
    )?;

    tracing::info!(
        footprint = %footprint_hex,
        trace_id = %trace_id,
        "Shuttle: sled read, xray config written to {}",
        SHM_XRAY_CONFIG
    );

    // 5. Spawn Xray — config lives entirely in /dev/shm
    Command::new("xray")
        .args(["run", "-c", SHM_XRAY_CONFIG])
        .spawn()?;

    // 6. Watch for new WireGuard handshakes and keep the sled current
    let iface = env::var("WG_INTERFACE").unwrap_or_else(|_| "wg0".to_string());
    std::thread::spawn(move || watch_wireguard_handshakes(&iface));

    Ok(())
}
</file>

<file path="src/session.rs">
//! Session management using WireGuard pubkey as identity.
//!
//! Sessions are created when a WireGuard peer connects and
//! destroyed on disconnect or timeout.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::gcloud_auth::GCloudAuth;
use crate::wireguard::WireGuardIdentity;

const SESSION_TIMEOUT_SECS: i64 = 3600; // 1 hour

/// Represents an active session
#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: String,
    pub pubkey: String,
    pub user_email: Option<String>,
    pub oauth_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

/// Mapping of WireGuard pubkey to user information
#[derive(Debug, Clone)]
pub struct UserMapping {
    pub pubkey: String,
    pub user_email: String,
    pub allowed_ip: String,
    pub created_at: DateTime<Utc>,
}

/// Manages sessions and their lifecycle
#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<DashMap<String, Session>>,
    wireguard_users: Arc<DashMap<String, UserMapping>>,
    gcloud_auth: GCloudAuth,
    wireguard: WireGuardIdentity,
    current_session_id: Arc<Mutex<Option<String>>>,
}

impl SessionManager {
    pub fn new() -> anyhow::Result<Self> {
        Self::with_wireguard_interface("wg0")
    }

    pub fn with_wireguard_interface(interface: &str) -> anyhow::Result<Self> {
        // SQL is obsolete in the 3tched architecture.
        // We use in-memory DashMaps to prevent Btrfs mutation loops.
        Ok(Self {
            sessions: Arc::new(DashMap::new()),
            wireguard_users: Arc::new(DashMap::new()),
            gcloud_auth: GCloudAuth::new(),
            wireguard: WireGuardIdentity::with_interface(interface),
            current_session_id: Arc::new(Mutex::new(None)),
        })
    }

    /// Get the GCloud auth provider
    pub fn gcloud_auth(&self) -> &GCloudAuth {
        &self.gcloud_auth
    }

    /// Get the WireGuard identity provider
    pub fn wireguard(&self) -> &WireGuardIdentity {
        &self.wireguard
    }

    /// Create or retrieve session based on WireGuard identity
    pub async fn get_or_create_session_from_wireguard(&self) -> anyhow::Result<Session> {
        let pubkey = self.wireguard.get_local_pubkey()?;
        self.get_or_create_session(&pubkey).await
    }

    /// Get or create a session for a given pubkey
    pub async fn get_or_create_session(&self, pubkey: &str) -> anyhow::Result<Session> {
        let now = Utc::now();

        // Check for existing valid session
        if let Some(mut session_ref) = self.sessions.iter_mut().find(|r| {
            r.pubkey == pubkey && (now - r.last_seen_at).num_seconds() < SESSION_TIMEOUT_SECS
        }) {
            debug!("Found existing session: {}", session_ref.session_id);
            session_ref.last_seen_at = now;
            let session = session_ref.clone();

            *self.current_session_id.lock().await = Some(session.session_id.clone());
            return Ok(session);
        }

        // Create new session
        let session_id = Uuid::new_v4().to_string();
        info!(
            "Creating new session: {} for pubkey: {}",
            session_id, pubkey
        );

        // Try to get user email from WireGuard user mapping
        let user_email = self
            .wireguard_users
            .get(pubkey)
            .map(|u| u.user_email.clone());

        // Try to get OAuth token
        let (oauth_token, token_expires_at) = match self.gcloud_auth.get_token().await {
            Ok((token, expires)) => (Some(token), Some(expires)),
            Err(e) => {
                warn!("Could not get OAuth token: {}", e);
                (None, None)
            }
        };

        let session = Session {
            session_id: session_id.clone(),
            pubkey: pubkey.to_string(),
            user_email,
            oauth_token,
            token_expires_at,
            created_at: now,
            last_seen_at: now,
        };

        self.sessions.insert(session_id.clone(), session.clone());

        // Store current session ID
        *self.current_session_id.lock().await = Some(session_id);

        Ok(session)
    }

    /// Get the current session ID
    pub async fn current_session_id(&self) -> Option<String> {
        self.current_session_id.lock().await.as_ref().cloned()
    }

    /// Update last_seen timestamp for current session
    pub async fn touch_session(&self) -> anyhow::Result<()> {
        let session_id = self.current_session_id.lock().await.as_ref().cloned();

        if let Some(id) = session_id {
            if let Some(mut session) = self.sessions.get_mut(&id) {
                session.last_seen_at = Utc::now();
            }
        }

        Ok(())
    }

    /// Get a valid OAuth token, refreshing if necessary
    pub async fn get_valid_token(&self) -> anyhow::Result<String> {
        let session_id = self.current_session_id.lock().await.as_ref().cloned();

        if let Some(id) = session_id {
            if let Some(session) = self.sessions.get(&id) {
                if let (Some(token), Some(expires_at)) =
                    (&session.oauth_token, session.token_expires_at)
                {
                    // Token valid for at least 5 more minutes
                    if expires_at > Utc::now() + chrono::Duration::minutes(5) {
                        return Ok(token.clone());
                    }
                }
            }
        }

        // Refresh token
        let (token, expires_at) = self.gcloud_auth.get_token().await?;

        // Update in session
        if let Some(id) = self.current_session_id.lock().await.as_ref().cloned() {
            if let Some(mut session) = self.sessions.get_mut(&id) {
                session.oauth_token = Some(token.clone());
                session.token_expires_at = Some(expires_at);
            }
        }

        Ok(token)
    }

    /// Register a WireGuard user mapping
    pub async fn register_wireguard_user(
        &self,
        pubkey: &str,
        user_email: &str,
        allowed_ip: &str,
    ) -> anyhow::Result<()> {
        let mapping = UserMapping {
            pubkey: pubkey.to_string(),
            user_email: user_email.to_string(),
            allowed_ip: allowed_ip.to_string(),
            created_at: Utc::now(),
        };

        self.wireguard_users.insert(pubkey.to_string(), mapping);

        info!("Registered WireGuard user: {} -> {}", pubkey, user_email);
        Ok(())
    }

    /// Get user email for a pubkey
    pub async fn get_user_for_pubkey(&self, pubkey: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .wireguard_users
            .get(pubkey)
            .map(|u| u.user_email.clone()))
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> anyhow::Result<usize> {
        let now = Utc::now();
        let mut deleted = 0;

        self.sessions.retain(|_, session| {
            if (now - session.last_seen_at).num_seconds() >= SESSION_TIMEOUT_SECS {
                deleted += 1;
                false
            } else {
                true
            }
        });

        if deleted > 0 {
            info!("Cleaned up {} expired sessions", deleted);
        }

        Ok(deleted)
    }

    /// Invalidate a specific session
    pub async fn invalidate_session(&self, session_id: &str) -> anyhow::Result<()> {
        self.sessions.remove(session_id);

        // Clear current session if it matches
        let mut current_guard = self.current_session_id.lock().await;
        if current_guard.as_deref() == Some(session_id) {
            *current_guard = None;
        }

        info!("Invalidated session: {}", session_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_creation() {
        let manager = SessionManager::new().unwrap();
        let session = manager.get_or_create_session("test-pubkey").await.unwrap();
        assert_eq!(session.pubkey, "test-pubkey");
        assert!(manager.current_session_id().await.is_some());
    }

    #[tokio::test]
    async fn test_session_touch() {
        let manager = SessionManager::new().unwrap();
        let session = manager.get_or_create_session("test-pubkey").await.unwrap();
        let last_seen = session.last_seen_at;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        manager.touch_session().await.unwrap();

        let updated = manager.get_or_create_session("test-pubkey").await.unwrap();
        assert!(updated.last_seen_at > last_seen);
    }

    #[tokio::test]
    async fn test_wireguard_user_registration() {
        let manager = SessionManager::new().unwrap();
        manager
            .register_wireguard_user("pubkey1", "user@example.com", "10.0.0.1")
            .await
            .unwrap();

        let email = manager.get_user_for_pubkey("pubkey1").await.unwrap();
        assert_eq!(email, Some("user@example.com".to_string()));
    }
}
</file>

<file path="src/token.rs">
//! OAuth token acquisition & org.freedesktop.secrets cache.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::process::Command;

const SCOPES: &str =
    "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/cloud-ide";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedToken {
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
}

/// Thin wrapper around gcloud/ADC that caches the token in the system keyring.
pub struct TokenManager;

impl Default for TokenManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenManager {
    pub fn new() -> Self {
        Self
    }

    /// Return a valid token (cached or fresh).
    pub async fn get_token(&self) -> Result<CachedToken> {
        // 1. Env var override (testing)
        if let Ok(tok) = std::env::var("GCLOUD_TOKEN") {
            return Ok(CachedToken {
                access_token: tok,
                expires_at: Utc::now() + Duration::minutes(55),
            });
        }
        // 2. Try keyring first
        if let Ok(ct) = self.read_from_keyring().await {
            if ct.expires_at > Utc::now() + Duration::minutes(5) {
                return Ok(ct);
            }
        }
        // 3. gcloud CLI
        let ct = self.fetch_via_gcloud().await?;
        // 4. Store it
        let _ = self.write_to_keyring(&ct).await;
        Ok(ct)
    }

    /// Force refresh.
    pub async fn refresh(&self) -> Result<CachedToken> {
        let ct = self.fetch_via_gcloud().await?;
        self.write_to_keyring(&ct).await?;
        Ok(ct)
    }

    // ---------- private ----------

    async fn fetch_via_gcloud(&self) -> Result<CachedToken> {
        let out = Command::new("gcloud")
            .args(["auth", "print-access-token", &format!("--scopes={SCOPES}")])
            .output()
            .context("gcloud not found")?;
        if !out.status.success() {
            anyhow::bail!(
                "gcloud auth failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let tok = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Ok(CachedToken {
            access_token: tok,
            expires_at: Utc::now() + Duration::minutes(55),
        })
    }

    async fn read_from_keyring(&self) -> Result<CachedToken> {
        let entry = keyring::Entry::new("mcp-identity", "gcloud-token")?;
        let mut json = entry.get_password()?;
        Ok(unsafe { simd_json::from_str(&mut json) }?)
    }

    async fn write_to_keyring(&self, ct: &CachedToken) -> Result<()> {
        let entry = keyring::Entry::new("mcp-identity", "gcloud-token")?;
        entry.set_password(&simd_json::to_string(ct)?)?;
        Ok(())
    }
}
</file>

<file path="src/wg.rs">
use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardIdentity {
    pub pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub pubkey: String,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
}

/// Get the WireGuard public key for a given peer IP
pub async fn get_peer_pubkey(peer_ip: &str) -> Result<Option<String>> {
    // Run `wg show wg0 allowed-ips` (assuming wg0, could make configurable)
    // Output format: <public-key>\t<allowed-ips>
    // e.g. "AbC...123\t10.100.0.2/32"
    
    let output = Command::new("wg")
        .arg("show")
        .arg("wg0") // TODO: Make interface configurable
        .arg("allowed-ips")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn 'wg' command. Is WireGuard tools installed?")?
        .wait_with_output()
        .await?;

    if !output.status.success() {
        warn!("wg command failed: {}", String::from_utf8_lossy(&output.stderr));
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        
        let pubkey = parts[0];
        let allowed_ips = &parts[1..];
        
        // simple check: if any allowed IP exactly matches the peer IP (or contains it? usually /32 for peers)
        // For now, we look for exact match of IP/32 or IP
        for ip_cidr in allowed_ips {
            if ip_cidr.starts_with(peer_ip) {
                // strict check: 10.100.0.2 should match 10.100.0.2/32 but not 10.100.0.20
                // simple prefix match is risky.
                // stripping /32
                let clean_ip = ip_cidr.split('/').next().unwrap_or("");
                if clean_ip == peer_ip {
                    debug!("Found pubkey {} for IP {}", pubkey, peer_ip);
                    return Ok(Some(pubkey.to_string()));
                }
            }
        }
    }

    debug!("No WireGuard peer found for IP {}", peer_ip);
    Ok(None)
}

/// Get the local device's public key
pub async fn get_local_pubkey() -> Result<String> {
    let output = Command::new("wg")
        .arg("show")
        .arg("wg0")
        .arg("public-key")
        .output()
        .await?;
        
    if !output.status.success() {
        anyhow::bail!("Failed to get local pubkey");
    }
    
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
</file>

<file path="src/wireguard.rs">
//! WireGuard identity detection and peer management.

use std::process::Command;
use tracing::{debug, warn};

/// WireGuard identity provider
#[derive(Debug, Clone)]
pub struct WireGuardIdentity {
    /// Interface name (default: wg0)
    interface: String,
}

impl WireGuardIdentity {
    pub fn new() -> Self {
        Self::with_interface("wg0")
    }

    pub fn with_interface(interface: &str) -> Self {
        Self {
            interface: interface.to_string(),
        }
    }

    /// Get the local WireGuard public key (this machine's identity)
    pub fn get_local_pubkey(&self) -> anyhow::Result<String> {
        // Try environment variable first
        if let Ok(pubkey) = std::env::var("WG_PUBKEY") {
            debug!("Using WG_PUBKEY from environment");
            return Ok(pubkey);
        }

        // Try to read from wg interface
        let output = Command::new("wg")
            .args(["show", &self.interface, "public-key"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let pubkey = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !pubkey.is_empty() {
                    debug!("Got pubkey from wg interface {}", self.interface);
                    return Ok(pubkey);
                }
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                debug!("wg show failed: {}", stderr);
            }
            Err(e) => {
                debug!("Failed to run wg command: {}", e);
            }
        }

        // Fallback: generate a deterministic ID from hostname
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        warn!("Could not get WireGuard pubkey, using hostname-based ID");
        Ok(format!("local:{}", hostname))
    }

    /// Get peer's pubkey from their IP address
    pub fn get_pubkey_for_ip(&self, peer_ip: &str) -> anyhow::Result<Option<String>> {
        let output = Command::new("wg")
            .args(["show", &self.interface, "allowed-ips"])
            .output()?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Format: pubkey\tallowed_ip1, allowed_ip2
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let pubkey = parts[0];
                let ips = parts[1];

                if ips.contains(peer_ip) {
                    return Ok(Some(pubkey.to_string()));
                }
            }
        }

        Ok(None)
    }

    /// Get all connected peers with their latest handshake times
    pub fn get_connected_peers(&self) -> anyhow::Result<Vec<PeerInfo>> {
        let output = Command::new("wg")
            .args(["show", &self.interface, "latest-handshakes"])
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut peers = Vec::new();

        // Format: pubkey\ttimestamp
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let pubkey = parts[0].to_string();
                let timestamp: u64 = parts[1].parse().unwrap_or(0);

                // Only include peers with recent handshakes (within 3 minutes)
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                if timestamp > 0 && now - timestamp < 180 {
                    peers.push(PeerInfo {
                        pubkey,
                        last_handshake: timestamp,
                        allowed_ips: self.get_allowed_ips_for_peer(parts[0]).unwrap_or_default(),
                    });
                }
            }
        }

        Ok(peers)
    }

    /// Get the primary IPv4 address of the WireGuard interface.
    ///
    /// Uses `ip -4 addr show <iface>` and parses the first `inet` address.
    /// Returns `None` if the interface has no IPv4 address or the command fails.
    pub fn get_local_ip(&self) -> Option<String> {
        let output = Command::new("ip")
            .args(["-4", "addr", "show", &self.interface])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("inet ") {
                // rest is e.g. "100.90.37.254/32 scope global netmaker"
                let ip = rest.split('/').next()?.trim();
                if !ip.is_empty() {
                    return Some(ip.to_string());
                }
            }
        }
        None
    }

    /// Get allowed IPs for a specific peer
    fn get_allowed_ips_for_peer(&self, pubkey: &str) -> anyhow::Result<Vec<String>> {
        let output = Command::new("wg")
            .args(["show", &self.interface, "allowed-ips"])
            .output()?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 && parts[0] == pubkey {
                return Ok(parts[1].split(',').map(|s| s.trim().to_string()).collect());
            }
        }

        Ok(Vec::new())
    }
}

impl Default for WireGuardIdentity {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a WireGuard peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub pubkey: String,
    pub last_handshake: u64,
    pub allowed_ips: Vec<String>,
}
</file>

<file path="Cargo.toml">
[package]
name = "op-identity"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = { workspace = true }
simd-json = { workspace = true }
zbus = { workspace = true }
chrono = { version = "0.4", features = ["serde"] }
sha2 = { workspace = true }
uuid = { version = "1.6", features = ["v4", "serde"] }
tracing = "0.1"
keyring = "2"
op-core = { path = "../op-core" }
op-compliance = { path = "../op-compliance" }
dashmap = { workspace = true }
dirs = "5"
hostname = "0.4"
rand = { workspace = true }
base64 = { workspace = true }
hex = { workspace = true }
memmap2 = { workspace = true }
md5 = { workspace = true }
blake3 = { workspace = true }
libc = { workspace = true }
x25519-dalek = { version = "2", features = ["static_secrets"] }

[dev-dependencies]
tempfile = "3"
</file>

<file path="compare-op-identity.md">
# compare-op-identity

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 7 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 5 |
| Partial artifacts | 0 |
| Spec-listed source files | 7 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- Current implementation inferred from source layout and Cargo metadata.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/session.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/session.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/wg.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/wg.rs |
| `src/token.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/token.rs |
| `src/wireguard.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/wireguard.rs |
| `src/registration.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/registration.rs |
| `src/gcloud_auth.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/gcloud_auth.rs |
| `root` | ✅ Present | root source group | src/gcloud_auth.rs, src/lib.rs, src/registration.rs, src/session.rs, src/token.rs, src/wg.rs, src/wireguard.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| session | ✅ Implemented | src/session.rs | SPEC main module |
| wg | ✅ Implemented | src/wg.rs | SPEC main module |
| token | ✅ Implemented | src/token.rs | SPEC main module |
| wireguard | ✅ Implemented | src/wireguard.rs | SPEC main module |
| registration | ✅ Implemented | src/registration.rs | SPEC main module |
| gcloud_auth | ✅ Implemented | src/gcloud_auth.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- None

### External Runtime Dependencies
- `anyhow` - documented in SPEC
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `zbus` - documented in SPEC
- `chrono` - documented in SPEC
- `uuid` - documented in SPEC
- `tracing` - documented in SPEC
- `keyring` - documented in SPEC
- `rusqlite` - documented in SPEC
- `dirs` - documented in SPEC
- `hostname` - documented in SPEC
- `rand` - documented in SPEC
- `base64` - documented in SPEC
- `x25519-dalek` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: gcloud_auth, registration, session, token, wireguard.
</file>

<file path="SPEC.md">
# op-identity - Specification

## Overview
**Crate**: `op-identity`  
**Location**: `crates/op-identity`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-identity"
version = "0.1.0"
edition = "2021"

[dependencies]
```

### Source Structure
```
op-identity/src/session.rs
op-identity/src/lib.rs
op-identity/src/wg.rs
op-identity/src/token.rs
op-identity/src/wireguard.rs
op-identity/src/registration.rs
op-identity/src/gcloud_auth.rs
```

### Key Dependencies
```toml
anyhow = "1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
simd-json = { workspace = true }
zbus = { version = "5.12", features = ["tokio"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.6", features = ["v4", "serde"] }
tracing = "0.1"
keyring = "2"
rusqlite = { workspace = true }
dirs = "5"
hostname = "0.4"
rand = { workspace = true }
base64 = { workspace = true }
x25519-dalek = { version = "2", features = ["static_secrets"] }
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
       7 Rust source files

### Main Modules
session
wg
token
wireguard
registration
gcloud_auth

## Purpose


## Build Information
- **Edition**: 2021
- **Version**: 0.1.0
- **License**: 

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
</file>

</files>
