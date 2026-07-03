//! Host gRPC-bridge endpoint + Xray schema-tag routing.
//!
//! Topology (current — xray + gRPC-bridge run on the host):
//!
//! - The `wg-xray` Incus container is DEPRECATED and STOPPED. Do not
//!   reference it.
//! - Xray runs on the HOST via the `gbr-xray` s6 service
//!   (config: `/dev/shm/xray-ghostbridge.json` or `/etc/xray/config.json`).
//! - The operation.v1 gRPC server (StateSync, etc., port 50051) runs on the
//!   HOST served by `op-dbus` at `10.200.0.2:50051` (the `grpc-uplink` veth
//!   IP). The old `10.200.0.1:50051` lived inside the wg-xray container and
//!   is dead.
//! - Xray applies OpenFlow + PluginSchema tags to route traffic to WireGuard
//!   peers / wgcf egress.
//!
//! Outbound RPC calls from this crate target the on-host operation.v1
//! endpoint directly and carry `x-ghostbridge-footprint` /
//! `x-ghostbridge-trace-id` headers sourced from `/dev/shm/plugin_schema.dat`
//! so Xray's OpenFlow rules can route them.

use crate::error::{AssistantError, Result};
use std::fs::File;
use std::io::Read;

/// Default on-host operation.v1 gRPC endpoint served by `op-dbus` at the
/// `grpc-uplink` veth IP `10.200.0.2:50051`.
///
/// (Renamed from `DEFAULT_WG_XRAY_ENDPOINT`; the wg-xray container is
/// deprecated. The alias is preserved for downstream callers.)
pub const DEFAULT_GRPC_ENDPOINT: &str = "http://10.200.0.2:50051";
/// Backwards-compatible alias for callers that still reference the old
/// `DEFAULT_WG_XRAY_ENDPOINT` name.
pub const DEFAULT_WG_XRAY_ENDPOINT: &str = DEFAULT_GRPC_ENDPOINT;
/// Xray SOCKS/MCP control plane (host-side proxy device).
pub const DEFAULT_XRAY_MCP_ENDPOINT: &str = "tcp://127.0.0.1:1081";

pub const ENV_RPC_ENDPOINT: &str = "OP_ASSISTANT_RPC_ENDPOINT";
pub const ENV_XRAY_MCP: &str = "OP_ASSISTANT_XRAY_MCP";
pub const ENV_SCHEMA_PATH: &str = "OP_ASSISTANT_SCHEMA_PATH";

pub const HEADER_FOOTPRINT: &str = "x-ghostbridge-footprint";
pub const HEADER_TRACE_ID: &str = "x-ghostbridge-trace-id";

const DEFAULT_SCHEMA_PATH: &str = "/dev/shm/plugin_schema.dat";

/// Schema tags pulled from the host's PluginSchema sled. Injected into every
/// outbound RPC so Xray's OpenFlow controller can route the request.
#[derive(Debug, Clone, Default)]
pub struct SchemaTags {
    pub footprint_hex: String,
    pub trace_id: String,
}

impl SchemaTags {
    /// Load tags from `/dev/shm/plugin_schema.dat` (or `OP_ASSISTANT_SCHEMA_PATH`).
    /// Returns an all-zero/empty struct when the sled is missing — the
    /// transport may then choose to fail closed.
    pub fn load() -> Self {
        let path = std::env::var(ENV_SCHEMA_PATH).unwrap_or_else(|_| DEFAULT_SCHEMA_PATH.into());
        match read_schema_sled(&path) {
            Ok(tags) => tags,
            Err(e) => {
                tracing::debug!(error = %e, path = %path, "schema sled not loadable");
                Self::default()
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.footprint_hex.is_empty() && self.footprint_hex.chars().any(|c| c != '0')
    }
}

/// Read the raw `IdentitySled` layout from shared memory. We re-implement the
/// minimal parse here rather than pulling in op-identity to keep this crate
/// dependency-light.
///
/// Layout (matches `op_identity::schema_bridge::IdentitySled`):
///   wg_pubkey:        [u8; 32]
///   mutation_index:   u64 (LE)
///   hashed_footprint: [u8; 32]
///   trace_id:         [u8; 32]
fn read_schema_sled(path: &str) -> Result<SchemaTags> {
    let mut buf = Vec::with_capacity(128);
    File::open(path)
        .and_then(|mut f| f.read_to_end(&mut buf))
        .map_err(|e| AssistantError::Transport(format!("read {path}: {e}")))?;

    const WG_OFF: usize = 0;
    const _WG_LEN: usize = 32;
    const MUT_OFF: usize = 32;
    const _MUT_LEN: usize = 8;
    const FP_OFF: usize = 40;
    const FP_LEN: usize = 32;
    const TRACE_OFF: usize = 72;
    const TRACE_LEN: usize = 32;
    const MIN_LEN: usize = TRACE_OFF + TRACE_LEN;

    if buf.len() < MIN_LEN {
        return Err(AssistantError::Transport(format!(
            "schema sled too short: {} < {}",
            buf.len(),
            MIN_LEN
        )));
    }
    let _ = WG_OFF;
    let _ = MUT_OFF;
    let footprint = &buf[FP_OFF..FP_OFF + FP_LEN];
    let trace = &buf[TRACE_OFF..TRACE_OFF + TRACE_LEN];
    Ok(SchemaTags {
        footprint_hex: hex_encode(footprint),
        trace_id: hex_encode(trace),
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(nibble((b >> 4) & 0xF));
        s.push(nibble(b & 0xF));
    }
    s
}

fn nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + (n - 10)) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_encode_matches_lowercase() {
        assert_eq!(hex_encode(&[0xAB, 0xCD, 0x01]), "abcd01");
        assert_eq!(hex_encode(&[0; 4]), "00000000");
    }

    #[test]
    fn tags_default_invalid() {
        let t = SchemaTags::default();
        assert!(!t.is_valid());
    }

    #[test]
    fn read_short_buffer_fails() {
        let p = std::env::temp_dir().join("op-assistant-grpc-short-sled.dat");
        std::fs::write(&p, b"too short").unwrap();
        assert!(read_schema_sled(p.to_str().unwrap()).is_err());
        let _ = std::fs::remove_file(&p);
    }
}
