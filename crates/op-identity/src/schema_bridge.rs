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
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::env;

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
            if seg.is_empty() || !seg.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()) {
                return Err(format!("invalid segment '{seg}': must be lowercase ascii/digits/hyphens"));
            }
        }

        Ok(Self { category, component_type, subject, verb, facet, version })
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
/// ── Identity block ────────────────────────────────────────────────────────────
///   wireguard_pubkey    [u8; 32]    raw Curve25519 peer key
///   mutation_index      u64         monotonic schema mutation counter
///   is_valid            bool        schema notarized and ready
///   _pad                [u8; 7]     alignment
///   hashed_footprint    [u8; 32]    SHA-256(wg_pubkey ∥ mutation_index)
///   schema_uuid         [u8; 16]    OSCAL UUID (UUIDv5 bytes, network order)
///
/// ── Subid taxonomy block ──────────────────────────────────────────────────────
///   subid               [u8; 64]    full subid string  "sch.network.plugin-schema.resolve@v1"
///   subid_category      [u8; 8]     category segment   "sch"
///   subid_component_type[u8; 32]    component-type     "network"
///   subid_subject       [u8; 64]    subject segment    "plugin-schema"
///   subid_verb          [u8; 32]    verb segment       "resolve"
///   subid_facet         [u8; 32]    optional facet     "read-path"
///   subid_version       u8          @vN number (0=unset)
///   _pad2               [u8; 7]     alignment
///
/// ── Compliance block ──────────────────────────────────────────────────────────
///   control_source      [u8; 32]    "NIST_SP_800_53_R5"
///   control_refs        [u8; 128]   space-delimited control IDs  "AC-2 AC-3 CM-2"
///   statement_refs      [u8; 128]   space-delimited statement refs
///
/// ── Routing block ─────────────────────────────────────────────────────────────
///   nextdns_profile     [u8; 16]    NextDNS profile ID  "689ec7"
#[repr(C)]
pub struct IdentitySled {
    // ── Identity ──────────────────────────────────────────────────────────────
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: bool,
    pub _pad: [u8; 7],
    pub hashed_footprint: [u8; 32],
    pub schema_uuid: [u8; 16],

    // ── Subid taxonomy ────────────────────────────────────────────────────────
    /// Full subid string, e.g. `"sch.network.plugin-schema.resolve@v1"`.
    pub subid: [u8; 64],
    /// Category segment: `"src"|"prj"|"sch"|"mut"|"obs"|"evt"|"exp"`.
    pub subid_category: [u8; 8],
    /// OSCAL component-type: `"software"|"service"|"network"|…`.
    pub subid_component_type: [u8; 32],
    /// Subject segment, e.g. `"plugin-schema"`.
    pub subid_subject: [u8; 64],
    /// Verb segment, e.g. `"resolve"`.
    pub subid_verb: [u8; 32],
    /// Optional facet, e.g. `"read-path"` (empty = unset).
    pub subid_facet: [u8; 32],
    /// `@vN` version number (0 = unset).
    pub subid_version: u8,
    pub _pad2: [u8; 7],

    // ── Compliance ────────────────────────────────────────────────────────────
    /// Compliance framework source, e.g. `"NIST_SP_800_53_R5"`.
    pub control_source: [u8; 32],
    /// Space-delimited control IDs, e.g. `"AC-2 AC-3 CM-2"`.
    pub control_refs: [u8; 128],
    /// Space-delimited statement-level refs (optional).
    pub statement_refs: [u8; 128],

    // ── Routing ───────────────────────────────────────────────────────────────
    /// NextDNS profile ID, e.g. `"689ec7"`.
    pub nextdns_profile: [u8; 16],
}

impl IdentitySled {
    pub const SIZE: usize = std::mem::size_of::<Self>();

    fn read_fixed<const N: usize>(buf: &[u8; N]) -> &str {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(N);
        std::str::from_utf8(&buf[..end]).unwrap_or("")
    }

    pub fn subid_str(&self) -> &str { Self::read_fixed(&self.subid) }
    pub fn subid_category_str(&self) -> &str { Self::read_fixed(&self.subid_category) }
    pub fn subid_component_type_str(&self) -> &str { Self::read_fixed(&self.subid_component_type) }
    pub fn subid_subject_str(&self) -> &str { Self::read_fixed(&self.subid_subject) }
    pub fn subid_verb_str(&self) -> &str { Self::read_fixed(&self.subid_verb) }
    pub fn subid_facet_str(&self) -> &str { Self::read_fixed(&self.subid_facet) }
    pub fn control_source_str(&self) -> &str { Self::read_fixed(&self.control_source) }
    pub fn control_refs_str(&self) -> &str { Self::read_fixed(&self.control_refs) }
    pub fn statement_refs_str(&self) -> &str { Self::read_fixed(&self.statement_refs) }
    pub fn nextdns_profile_str(&self) -> &str { Self::read_fixed(&self.nextdns_profile) }

    /// Parse the subid fields back into a `SubidTaxonomy`, if the sled
    /// carries a valid subid.
    pub fn subid_taxonomy(&self) -> Option<SubidTaxonomy> {
        let s = self.subid_str();
        if s.is_empty() { return None; }
        SubidTaxonomy::parse(s).ok()
    }
}

/// Copy at most `N` bytes of `s` into a zeroed `[u8; N]` buffer.
fn str_to_fixed<const N: usize>(s: &str) -> [u8; N] {
    let mut buf = [0u8; N];
    let bytes = s.as_bytes();
    let len = bytes.len().min(N);
    buf[..len].copy_from_slice(&bytes[..len]);
    buf
}

// ── Writer side (called from SchemaEngine) ───────────────────────────────────

/// Atomically write the active sled into `/dev/shm`.
///
/// Uses a tmp-file + rename so readers never see a partial write.
pub fn write_sled(sled: &IdentitySled) -> std::io::Result<()> {
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
    let Ok(raw) = env::var("UNIX_SOCKET_ENDPOINTS") else { return vec![] };
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
    write_xray_config_with_sockets(footprint, trace_id, nextdns_profile, uuid, private_key, short_id, &sockets)
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
        .map(|s| format!(
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
        ))
        .collect();

    // Build socket outbounds: freedom via xray domain-socket transport.
    let socket_outbounds: String = sockets
        .iter()
        .map(|s| format!(
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
        ))
        .collect();

    // Build socket routing rules: inbound tag → outbound tag.
    let socket_rules: String = sockets
        .iter()
        .map(|s| format!(
            r#",
      {{ "type": "field", "inboundTag": ["{label}-in"], "outboundTag": "to-{label}" }}"#,
            label = s.label,
        ))
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
      "sendThrough": "10.200.0.1",
      "settings": {{ "redirect": "10.200.0.2:50051" }},
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

/// Decompose a subid string into taxonomy fixed buffers.
///
/// Parses the string (best-effort; invalid subids are stored as-is in `subid`
/// with the component fields left zeroed).
fn subid_to_fields(subid_str: &str) -> (
    [u8; 64],  // subid
    [u8; 8],   // category
    [u8; 32],  // component_type
    [u8; 64],  // subject
    [u8; 32],  // verb
    [u8; 32],  // facet
    u8,        // version
) {
    let subid = str_to_fixed::<64>(subid_str);
    match SubidTaxonomy::parse(subid_str) {
        Ok(tax) => (
            subid,
            str_to_fixed::<8>(tax.category.as_str()),
            str_to_fixed::<32>(&tax.component_type),
            str_to_fixed::<64>(&tax.subject),
            str_to_fixed::<32>(&tax.verb),
            str_to_fixed::<32>(tax.facet.as_deref().unwrap_or("")),
            tax.version,
        ),
        Err(_) => (
            subid,
            [0u8; 8],
            [0u8; 32],
            [0u8; 64],
            [0u8; 32],
            [0u8; 32],
            0,
        ),
    }
}

/// Build and atomically write the sled from live WireGuard state.
///
/// Fields read from environment variables:
///   SCHEMA_UUID, SCHEMA_SUBID, SCHEMA_CONTROL_SOURCE,
///   SCHEMA_CONTROL_REFS, SCHEMA_STATEMENT_REFS, NEXTDNS_PROFILE_ID
pub fn write_sled_from_wg(peer_pubkey: &str) -> std::io::Result<()> {
    use sha2::{Digest, Sha256};

    let wireguard_pubkey = decode_wg_pubkey(peer_pubkey);
    let mutation_index = MUTATION_INDEX.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(&wireguard_pubkey);
    hasher.update(mutation_index.to_le_bytes());
    let hashed_footprint: [u8; 32] = hasher.finalize().into();

    let subid_str = env::var("SCHEMA_SUBID").unwrap_or_default();
    if !subid_str.is_empty() {
        if let Err(e) = SubidTaxonomy::parse(&subid_str) {
            tracing::warn!(subid = %subid_str, error = %e, "SCHEMA_SUBID failed taxonomy validation — stored as-is");
        }
    }
    let (subid, subid_category, subid_component_type, subid_subject, subid_verb, subid_facet, subid_version) =
        subid_to_fields(&subid_str);

    let sled = IdentitySled {
        wireguard_pubkey,
        mutation_index,
        is_valid: true,
        _pad: [0u8; 7],
        hashed_footprint,
        schema_uuid: parse_uuid_bytes(&env::var("SCHEMA_UUID").unwrap_or_default()),
        subid,
        subid_category,
        subid_component_type,
        subid_subject,
        subid_verb,
        subid_facet,
        subid_version,
        _pad2: [0u8; 7],
        control_source: str_to_fixed::<32>(
            &env::var("SCHEMA_CONTROL_SOURCE").unwrap_or_else(|_| "NIST_SP_800_53_R5".into()),
        ),
        control_refs: str_to_fixed::<128>(&env::var("SCHEMA_CONTROL_REFS").unwrap_or_default()),
        statement_refs: str_to_fixed::<128>(&env::var("SCHEMA_STATEMENT_REFS").unwrap_or_default()),
        nextdns_profile: str_to_fixed::<16>(
            &env::var("NEXTDNS_PROFILE_ID").unwrap_or_else(|_| "689ec7".into()),
        ),
    };
    write_sled(&sled)
}

/// Write the sled with fully explicit fields — called from SchemaEngine on mutation.
pub fn write_sled_full(
    peer_pubkey: &str,
    mutation_index: u64,
    uuid_str: &str,
    subid_str: &str,
    control_source_str: &str,
    control_refs_str: &str,
    statement_refs_str: &str,
    nextdns_profile_str: &str,
) -> std::io::Result<()> {
    use sha2::{Digest, Sha256};

    let wireguard_pubkey = decode_wg_pubkey(peer_pubkey);
    let mut hasher = Sha256::new();
    hasher.update(&wireguard_pubkey);
    hasher.update(mutation_index.to_le_bytes());
    let hashed_footprint: [u8; 32] = hasher.finalize().into();

    let (subid, subid_category, subid_component_type, subid_subject, subid_verb, subid_facet, subid_version) =
        subid_to_fields(subid_str);

    let sled = IdentitySled {
        wireguard_pubkey,
        mutation_index,
        is_valid: true,
        _pad: [0u8; 7],
        hashed_footprint,
        schema_uuid: parse_uuid_bytes(uuid_str),
        subid,
        subid_category,
        subid_component_type,
        subid_subject,
        subid_verb,
        subid_facet,
        subid_version,
        _pad2: [0u8; 7],
        control_source: str_to_fixed::<32>(control_source_str),
        control_refs: str_to_fixed::<128>(control_refs_str),
        statement_refs: str_to_fixed::<128>(statement_refs_str),
        nextdns_profile: str_to_fixed::<16>(nextdns_profile_str),
    };
    write_sled(&sled)
}

/// Parse a hyphenated UUID string into 16 raw bytes. Returns zeros on failure.
fn parse_uuid_bytes(s: &str) -> [u8; 16] {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 { return [0u8; 16]; }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&hex[i*2..i*2+2], 16).unwrap_or(0);
    }
    out
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
            .args(["exec", "wg-xray", "--", "wg", "show", &iface, "latest-handshakes"])
            .output()
        else { continue };

        if !out.status.success() { continue }

        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            let mut parts = line.split('\t');
            let (Some(pubkey), Some(ts_str)) = (parts.next(), parts.next()) else { continue };
            let ts: u64 = ts_str.trim().parse().unwrap_or(0);
            if ts == 0 { continue }

            // Treat any handshake within the last 3 minutes as "new" if not yet seen
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now.saturating_sub(ts) > 180 { continue }

            let key = format!("{}:{}", pubkey, ts);
            if seen.contains(&key) { continue }
            seen.insert(key);

            tracing::info!(peer = %pubkey, "WireGuard handshake → updating identity sled");

            if let Err(e) = write_sled_from_wg(pubkey) {
                tracing::warn!("write_sled_from_wg failed: {}", e);
                continue;
            }

            // Re-bake xray config — pull NextDNS profile from sled, not env.
            if let Ok((ptr, _mmap)) = read_sled() {
                let sled = unsafe { &*ptr };
                let footprint_hex = hex::encode(sled.hashed_footprint);
                let trace_id = format!("{}-{}", hex::encode(&sled.wireguard_pubkey[..4]), sled.mutation_index);
                let profile = {
                    let p = sled.nextdns_profile_str();
                    if p.is_empty() { "689ec7".into() } else { p.to_string() }
                };
                let uuid    = env::var("XRAY_UUID").unwrap_or_else(|_| "40813c05-4a7c-4d5b-b027-33912551287f".to_string());
                let privkey = env::var("XRAY_PRIVATE_KEY").unwrap_or_else(|_| "-MULA7gIbk_58CKa4TNHovpYNt192NUkPlQF7f3caWo".to_string());
                let short   = env::var("XRAY_SHORT_ID").unwrap_or_else(|_| "2a32c53278372687".to_string());
                if let Err(e) = write_xray_config(&footprint_hex, &trace_id, &profile, &uuid, &privkey, &short) {
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
    let trace_id = format!(
        "{}-{}",
        hex::encode(&sled.wireguard_pubkey[..4]),
        sled.mutation_index
    );

    // 3. Stamp into environment — zero Btrfs, zero disk I/O
    env::set_var("GB_FOOTPRINT", &footprint_hex);
    env::set_var("GB_TRACE_ID", &trace_id);

    // 4. Write stateless Xray config — NextDNS profile comes from sled, not env.
    let nextdns_profile = {
        let p = sled.nextdns_profile_str();
        if p.is_empty() {
            env::var("NEXTDNS_PROFILE_ID").unwrap_or_else(|_| "689ec7".to_string())
        } else {
            p.to_string()
        }
    };
    let xray_uuid = env::var("XRAY_UUID")
        .unwrap_or_else(|_| "40813c05-4a7c-4d5b-b027-33912551287f".to_string());
    let xray_privkey = env::var("XRAY_PRIVATE_KEY")
        .unwrap_or_else(|_| "-MULA7gIbk_58CKa4TNHovpYNt192NUkPlQF7f3caWo".to_string());
    let xray_short_id = env::var("XRAY_SHORT_ID")
        .unwrap_or_else(|_| "2a32c53278372687".to_string());
    write_xray_config(&footprint_hex, &trace_id, &nextdns_profile, &xray_uuid, &xray_privkey, &xray_short_id)?;

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
