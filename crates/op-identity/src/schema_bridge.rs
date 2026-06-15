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
use serde::Serialize;
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use zbus::{Connection, Proxy};

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

    pub fn parse(s: &str) -> Option<Self> {
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
        let category =
            SubidCategory::parse(cat_str).ok_or_else(|| format!("unknown category: {cat_str}"))?;
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
///   schema_version      u32        offset 88
///   reserved            [u8; 60]   offset 92
#[repr(C)]
#[derive(Debug, Clone, Serialize)]
pub struct IdentitySled {
    /// Raw Curve25519 WireGuard peer key.
    pub wireguard_pubkey: [u8; 32],
    /// Monotonic schema mutation counter.
    pub mutation_index: u64,
    /// Blake3 hashed footprint of the canonicalized schema state.
    pub hashed_footprint: [u8; 32],
    /// UUID v4 trace ID (16 raw bytes, network order).
    pub trace_id: [u8; 16],
    /// Schema version for compatibility.
    pub schema_version: u32,
    /// Qdrant vector ID for the last reasoning episode (UUID v4, 16 raw bytes).
    /// Bound to identity: every vectorized episode is traceable to this sled.
    pub vector_id: [u8; 16],
    /// Reserved for future use (zero-initialized).
    #[serde(skip)]
    pub reserved: [u8; 44],
}

impl Default for IdentitySled {
    fn default() -> Self {
        Self {
            wireguard_pubkey: [0u8; 32],
            mutation_index: 0,
            hashed_footprint: [0u8; 32],
            trace_id: [0u8; 16],
            schema_version: 0,
            vector_id: [0u8; 16],
            reserved: [0u8; 44],
        }
    }
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
/// (blockchain); all sled and Xray config writes must live in tmpfs.
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

/// A nicless container endpoint: a subdomain routed straight into a unix socket.
///
/// Declared via `UNIX_SOCKET_ENDPOINTS=label:path:subdomain[,…]`, e.g.:
///   `qdrant:/run/qdrant.sock:qdrant.ghostbridge.tech`
///
/// Traffic enters through the shared REALITY/TLS ingress; xray sniffs the SNI
/// and a domain routing rule sends it to the `to-<label>` freedom/ds outbound,
/// which dials the container's socket. No per-socket TCP inbound, no NIC.
#[derive(Debug, Clone)]
pub struct SocketEntry {
    /// Xray tag suffix (e.g. `"qdrant"`) — becomes the `"to-<label>"` outbound.
    pub label: String,
    /// Filesystem path of the container's unix domain socket.
    pub path: String,
    /// Subdomain that routes to this socket (matched against the sniffed SNI).
    pub domain: String,
}

/// Parse `UNIX_SOCKET_ENDPOINTS` env var into a list of `SocketEntry`.
///
/// Format: `label:/path/to/sock:subdomain[,…]`
/// Example: `qdrant:/run/qdrant.sock:qdrant.ghostbridge.tech`
pub fn socket_entries_from_env() -> Vec<SocketEntry> {
    let Ok(raw) = env::var("UNIX_SOCKET_ENDPOINTS") else {
        return vec![];
    };
    raw.split(',')
        .filter_map(|entry| {
            // Split into exactly 3 parts: label, path, subdomain
            let mut parts = entry.trim().splitn(3, ':');
            let label = parts.next()?.to_string();
            let path = parts.next()?.to_string(); // already has leading '/'
            let domain = parts.next()?.trim().to_string();
            (!domain.is_empty()).then_some(SocketEntry { label, path, domain })
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
    let config = build_xray_config(
        footprint,
        trace_id,
        nextdns_profile,
        uuid,
        private_key,
        short_id,
        sockets,
    );
    let tmp = format!("{}.tmp", SHM_XRAY_CONFIG);
    let mut f = File::create(&tmp)?;
    f.write_all(config.as_bytes())?;
    f.sync_data()?;
    fs::rename(&tmp, SHM_XRAY_CONFIG)?;
    Ok(())
}

/// Build the Xray config JSON as a string (pure — no I/O). Routes each nicless
/// container socket from its subdomain (sniffed SNI) into a freedom/ds outbound.
fn build_xray_config(
    footprint: &str,
    trace_id: &str,
    nextdns_profile: &str,
    uuid: &str,
    private_key: &str,
    short_id: &str,
    sockets: &[SocketEntry],
) -> String {
    // Socket outbounds: freedom over xray's unix-domain-socket transport — dials
    // the container's socket directly, no NIC.
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

    // Subdomain routing: sniffed SNI on the shared ingress → the socket outbound.
    let socket_rules: String = sockets
        .iter()
        .map(|s| {
            format!(
                r#",
      {{ "type": "field", "inboundTag": ["op-tls", "ghostbridge-reality"], "domain": ["full:{domain}"], "outboundTag": "to-{label}" }}"#,
                domain = s.domain,
                label = s.label,
            )
        })
        .collect();

    // TLS certificate paths — auto-generated by xray or provisioned by ACME.
    let tls_certs = match (
        env::var("XRAY_TLS_CERT").ok(),
        env::var("XRAY_TLS_KEY").ok(),
    ) {
        (Some(cert), Some(key)) => format!(
            r#",
            "certificates": [
              {{ "certificateFile": "{cert}", "keyFile": "{key}" }}
            ]"#
        ),
        _ => String::new(), // xray auto-generates when no certs specified
    };

    let config = format!(
        r#"{{
  "log": {{ "loglevel": "warning" }},
  "dns": {{
    "servers": [ "https://dns.nextdns.io/{profile}/Ghostbridge-Incus" ],
    "tag": "nextdns-in"
  }},
  "inbounds": [
    {{
      "tag": "op-tls",
      "port": 443,
      "listen": "0.0.0.0",
      "protocol": "vless",
      "settings": {{
        "clients": [{{ "id": "{uuid}" }}],
        "decryption": "none",
        "fallbacks": [{{ "dest": 18789 }}]
      }},
      "streamSettings": {{
        "network": "tcp",
        "security": "tls",
        "tlsSettings": {{
          "alpn": ["h2", "http/1.1"]{tls_certs}
        }}
      }},
      "sniffing": {{
        "enabled": true,
        "destOverride": ["http", "tls", "quic"],
        "routeOnly": true
      }}
    }},
    {{
      "tag": "ghostbridge-reality",
      "port": 8443,
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
          "dest": "www.microsoft.com:443",
          "serverNames": ["www.microsoft.com"],
          "privateKey": "{private_key}",
          "shortIds": ["{short_id}"]
        }}
      }},
      "sniffing": {{
        "enabled": true,
        "destOverride": ["http", "tls", "quic"],
        "routeOnly": true
      }}
    }}
  ],
  "outbounds": [
    {{
      "tag": "to-grpc-bridge",
      "protocol": "freedom",
      "settings": {{ "redirect": "127.0.0.1:18789" }},
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
      "settings": {{ "redirect": "127.0.0.1:3003" }},
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
    "domainStrategy": "AsIs",
    "rules": [
      {{ "type": "field", "port": 53, "outboundTag": "dns-out" }},
      {{
        "type": "field",
        "inboundTag": ["op-tls", "ghostbridge-reality"],
        "domain": ["full:mcp.internal"],
        "outboundTag": "to-cognitive-mcp"
      }},
      {{
        "type": "field",
        "inboundTag": ["op-tls", "ghostbridge-reality"],
        "outboundTag": "to-grpc-bridge"
      }}{socket_rules}
    ]
  }}
}}"#,
        profile = nextdns_profile,
        footprint = footprint,
        trace_id = trace_id,
        uuid = uuid,
        private_key = private_key,
        short_id = short_id,
        socket_outbounds = socket_outbounds,
        socket_rules = socket_rules,
    );

    config
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
/// THE STRIKE/ETCH -- single source of truth for the identity footprint.
///
/// Blake3( wg_pubkey || schema_catalog_hash(/dev/shm/live-schema.json) || mutation_index || source_port ).
/// source_port is the per-session WireGuard-observed source port (0 when none). It binds the
/// footprint to the session network context for the accountability loop -- it is NOT an auth
/// factor (WireGuard is the authenticator; see op-grpc-bridge GhostbridgeInterceptor).
pub fn etch_footprint(wireguard_pubkey: &[u8; 32], mutation_index: u64, source_port: u16) -> [u8; 32] {
    let schema_catalog_hash = std::fs::read("/dev/shm/live-schema.json")
        .map(|bytes| blake3::hash(&bytes))
        .unwrap_or_else(|_| blake3::Hash::from([0u8; 32]));

    let mut hasher = blake3::Hasher::new();
    hasher.update(wireguard_pubkey);
    hasher.update(schema_catalog_hash.as_bytes());
    hasher.update(&mutation_index.to_le_bytes());
    hasher.update(&source_port.to_le_bytes());
    hasher.finalize().into()
}

/// Per-session WireGuard source port observed for peer_pubkey, parsed from
/// wg show <iface> dump (iface from WG_INTERFACE, default wg0).
///
/// Returns 0 when the peer has no current endpoint (not connected, or a local self-write),
/// so the footprint degrades gracefully to the port-less base.
pub fn peer_source_port(peer_pubkey: &str) -> u16 {
    let iface = env::var("WG_INTERFACE").unwrap_or_else(|_| "wg0".to_string());
    let out = match Command::new("wg").arg("show").arg(&iface).arg("dump").output() {
        Ok(o) if o.status.success() => o,
        _ => return 0,
    };
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        // peer fields: pubkey, psk, endpoint, allowed-ips, last-hs, rx, tx, keepalive
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 3 && fields[0] == peer_pubkey {
            let endpoint = fields[2];
            if endpoint == "(none)" {
                return 0;
            }
            return endpoint
                .rsplit(":")
                .next()
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(0);
        }
    }
    0
}

/// Reads `GB_TRACE_ID` from environment to propagate an existing trace;
/// if absent, mints a fresh UUID v4.  All extra metadata (subid, compliance,
/// routing) lives in environment variables — the sled itself is the spec
/// layout and nothing more.
pub fn write_sled_from_wg(peer_pubkey: &str) -> std::io::Result<()> {
    let wireguard_pubkey = decode_wg_pubkey(peer_pubkey);
    let mutation_index = MUTATION_INDEX.fetch_add(1, Ordering::Relaxed);

    // Per-session WireGuard source port binds the footprint to the session
    // network context (accountability loop); then the canonical Strike/Etch.
    let source_port = peer_source_port(peer_pubkey);
    let hashed_footprint = etch_footprint(&wireguard_pubkey, mutation_index, source_port);

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

    // Per-session WireGuard source port binds the footprint to the session
    // network context (accountability loop); then the canonical Strike/Etch.
    let source_port = peer_source_port(peer_pubkey);
    let hashed_footprint = etch_footprint(&wireguard_pubkey, mutation_index, source_port);

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

/// Watch for new WireGuard peers using `ip monitor` — fires instantly on handshake,
/// no polling delay. Re-writes the sled and xray config on each new peer.
/// Runs forever; call from a thread.
pub fn watch_wireguard_handshakes(iface: &str) {
    let iface = iface.to_string();

    // `ip monitor route` inside wg-xray fires the instant a WireGuard peer
    // route appears — no polling delay. wg0 lives in the container namespace.
    let mut monitor = loop {
        match Command::new("incus")
            .args(["exec", "wg-xray", "--", "ip", "monitor", "route"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => break child,
            Err(e) => {
                tracing::warn!("incus exec ip monitor spawn failed: {} — retrying in 5s", e);
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        }
    };

    let stdout = monitor.stdout.take().expect("piped");
    let reader = std::io::BufReader::new(stdout);

    // Track the last pubkey we wrote the sled for — don't re-write for same peer.
    let mut last_pubkey = String::new();

    use std::io::BufRead;
    for line in reader.lines() {
        let Ok(line) = line else { break };

        // Only act on route additions — deletions fire when a peer drops,
        // which is not a reason to rewrite the sled.
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Deleted") || trimmed.starts_with("del") {
            continue;
        }

        // Read current peers from inside wg-xray immediately.
        let Ok(out) = Command::new("incus")
            .args(["exec", "wg-xray", "--", "wg", "show", &iface, "latest-handshakes"])
            .output()
        else {
            continue;
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            let mut parts = line.split('\t');
            let (Some(pubkey), Some(ts_str)) = (parts.next(), parts.next()) else {
                continue;
            };
            let ts: u64 = ts_str.trim().parse().unwrap_or(0);
            // Only act on handshakes within the last 30 seconds (one keepalive window).
            if ts == 0 || now.saturating_sub(ts) > 30 {
                continue;
            }
            if pubkey == last_pubkey {
                continue;
            }

            tracing::info!(peer = %pubkey, "WireGuard peer → updating identity sled");
            last_pubkey = pubkey.to_string();

            if let Err(e) = write_sled_from_wg(pubkey) {
                tracing::warn!("write_sled_from_wg failed: {}", e);
                continue;
            }

            if let Ok((ptr, _mmap)) = read_sled() {
                let sled = unsafe { &*ptr };
                let footprint_hex = hex::encode(sled.hashed_footprint);
                let trace_id = sled.trace_id_hex();
                let Ok(profile) = env::var("NEXTDNS_PROFILE_ID") else { continue };
                let Ok(uuid) = env::var("XRAY_UUID") else { continue };
                let Ok(privkey) = env::var("XRAY_PRIVATE_KEY") else { continue };
                let Ok(short) = env::var("XRAY_SHORT_ID") else { continue };
                if let Err(e) = write_xray_config(&footprint_hex, &trace_id, &profile, &uuid, &privkey, &short) {
                    tracing::warn!("write_xray_config failed: {}", e);
                }
            }
        }
    }

    // ip monitor exited — respawn the thread.
    tracing::warn!("ip monitor exited — restarting watcher in 2s");
    std::thread::sleep(std::time::Duration::from_secs(2));
    watch_wireguard_handshakes(&iface);
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
    let nextdns_profile =
        env::var("NEXTDNS_PROFILE_ID").map_err(|_| "NEXTDNS_PROFILE_ID not set")?;
    let xray_uuid = env::var("XRAY_UUID").map_err(|_| "XRAY_UUID not set")?;
    let xray_privkey = env::var("XRAY_PRIVATE_KEY").map_err(|_| "XRAY_PRIVATE_KEY not set")?;
    let xray_short_id = env::var("XRAY_SHORT_ID").map_err(|_| "XRAY_SHORT_ID not set")?;
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

    // 5. Start Xray via D-Bus — config lives entirely in /dev/shm
    // Per AGENTS.md §4: D-Bus first. D-Bus always. D-Bus only.
    // Use block_on since run_schema_shuttle is synchronous
    let rt = tokio::runtime::Runtime::new()?;
    match rt.block_on(start_xray_via_dbus(SHM_XRAY_CONFIG)) {
        Ok(()) => tracing::info!("Xray started via D-Bus opdbus.v1"),
        Err(e) => {
            tracing::error!(
                "Failed to start Xray via D-Bus: {}. Ensure op-xray-daemon is running.",
                e
            );
            return Err(e.into());
        }
    }

    // 6. Watch for new WireGuard handshakes and keep the sled current
    let iface = env::var("WG_INTERFACE").unwrap_or_else(|_| "wg0".to_string());
    std::thread::spawn(move || watch_wireguard_handshakes(&iface));

    Ok(())
}

/// Start Xray via D-Bus service (opdbus.v1.Xray)
///
/// Per AGENTS.md §4: All control plane operations must go through D-Bus.
/// The op-xray-daemon manages the xray process lifecycle.
async fn start_xray_via_dbus(config_path: &str) -> anyhow::Result<()> {
    let conn = Connection::system()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to system D-Bus: {}", e))?;

    let proxy = Proxy::new(&conn, "opdbus.v1", "/org/opdbus/v1/plugins/xray", "opdbus.v1.Xray")
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create Xray D-Bus proxy: {}", e))?;

    let (success, message): (bool, String) = proxy
        .call_method("start", &(config_path,))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to call Xray start via D-Bus: {}", e))?
        .body()
        .deserialize()
        .map_err(|e| anyhow::anyhow!("Failed to deserialize Xray response: {}", e))?;

    if success {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Xray D-Bus start failed: {}", message))
    }
}

#[cfg(test)]
mod xray_config_tests {
    use super::*;

    #[test]
    fn subdomain_routes_into_socket_outbound() {
        let sockets = vec![
            SocketEntry {
                label: "qdrant".into(),
                path: "/run/qdrant.sock".into(),
                domain: "qdrant.ghostbridge.tech".into(),
            },
            SocketEntry {
                label: "cozo".into(),
                path: "/run/cozo.sock".into(),
                domain: "cozo.ghostbridge.tech".into(),
            },
        ];
        let cfg = build_xray_config("foot", "trace", "abc123", "uuid", "pk", "sid", &sockets);

        // Must be valid JSON.
        let v: serde_json::Value = serde_json::from_str(&cfg).expect("valid json");

        // No per-socket TCP inbounds — only the two shared ingress listeners.
        let inbounds = v["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 2, "no dokodemo socket inbounds expected");

        // Each socket has a freedom/ds outbound dialing its path.
        let outbounds = v["outbounds"].as_array().unwrap();
        let qdrant = outbounds
            .iter()
            .find(|o| o["tag"] == "to-qdrant")
            .expect("to-qdrant outbound");
        assert_eq!(qdrant["streamSettings"]["network"], "ds");
        assert_eq!(qdrant["streamSettings"]["dsSettings"]["path"], "/run/qdrant.sock");

        // Each subdomain routes to its socket outbound off the shared ingress.
        let rules = v["routing"]["rules"].as_array().unwrap();
        let rule = rules
            .iter()
            .find(|r| r["outboundTag"] == "to-cozo")
            .expect("cozo domain rule");
        assert_eq!(rule["domain"][0], "full:cozo.ghostbridge.tech");
        assert!(rule["inboundTag"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("ghostbridge-reality")));
    }

    #[test]
    fn parses_label_path_subdomain() {
        std::env::set_var(
            "UNIX_SOCKET_ENDPOINTS",
            "qdrant:/run/qdrant.sock:qdrant.ghostbridge.tech,bad-no-domain:/run/x.sock:",
        );
        let entries = socket_entries_from_env();
        std::env::remove_var("UNIX_SOCKET_ENDPOINTS");
        assert_eq!(entries.len(), 1, "entry with empty domain is dropped");
        assert_eq!(entries[0].label, "qdrant");
        assert_eq!(entries[0].domain, "qdrant.ghostbridge.tech");
    }
}
