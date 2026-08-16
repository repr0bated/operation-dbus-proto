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
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::process::Command;

pub const SHM_SLED_PATH: &str = "/dev/shm/plugin_schema.dat";

/// Sled layout/derivation version.
///
/// Bumped to 2 when the footprint's fourth term stopped being a scraped
/// WireGuard peer port (which was structurally zero on this host) and became a
/// caller-supplied transport port, and when writes became read-modify-write
/// with an advance-only index. Records written under either rule are
/// self-describing through this field; nothing re-derives stored footprints, so
/// existing chain records keep their values rather than being reinterpreted.
pub const SLED_SCHEMA_VERSION: u32 = 2;
pub const SHM_XRAY_CONFIG: &str = "/dev/shm/xray-ghostbridge.json";
pub const SCHEMA_BLOB_MAGIC: [u8; 8] = *b"OPBLOB01";
pub const SCHEMA_BLOB_VERSION: u32 = 1;

const SCHEMA_BLOB_HEADER_SIZE: usize = SCHEMA_BLOB_MAGIC.len() + 4 + 8;

/// The tonic-web gRPC bridge (op-grpc-adapters) — the single endpoint xray
/// routes to.  The bridge uses gRPC reflection to demux by service/method and
/// dials backend unix sockets natively (Rust `UnixStream`).  xray never dials
/// sockets directly; it just does TLS + SNI routing to this address.
const GRPC_BRIDGE_HOST: &str = "127.0.0.1";
const GRPC_BRIDGE_PORT: u16 = 50051;

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
/// Extends `.kiro/specs/3tched-schema-shuttle-xray-pipeline` (spec's
/// `reserved [u8; 60]` is split into `vector_id [u8; 16]` + `reserved [u8; 44]`;
/// total size unchanged).
/// Layout (152 bytes total):
///   wireguard_pubkey    [u8; 32]   offset 0
///   mutation_index      u64        offset 32
///   hashed_footprint    [u8; 32]   offset 40   (Blake3)
///   trace_id            [u8; 16]   offset 72   (UUID v4, network order)
///   schema_version      u32        offset 88
///   vector_id           [u8; 16]   offset 92   (Qdrant episode UUID)
///   reserved            [u8; 44]   offset 108
#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize)]
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

/// Group granted read access to the sled, overridable with `OP_SLED_GROUP`.
///
/// The sled is written by root but read by services that drop privileges, so a
/// shared group is what bridges them without exposing it to every local user.
const DEFAULT_SLED_GROUP: &str = "secrets";

fn sled_group() -> String {
    env::var("OP_SLED_GROUP").unwrap_or_else(|_| DEFAULT_SLED_GROUP.to_string())
}

/// Resolve a group name to its gid, or `None` if the group does not exist.
fn gid_for_group(name: &str) -> Option<u32> {
    let c_name = std::ffi::CString::new(name).ok()?;
    // SAFETY: getgrnam returns a pointer into a static buffer, or null when the
    // group is absent. gr_gid is copied out before any further libc call can
    // overwrite that buffer.
    unsafe {
        let gr = libc::getgrnam(c_name.as_ptr());
        if gr.is_null() {
            None
        } else {
            Some((*gr).gr_gid)
        }
    }
}

/// Atomically write the active sled into `/dev/shm`.
///
/// Uses a tmp-file + rename so readers never see a partial write.
/// Aborts if the target path is not on tmpfs (Zero-Btrfs Overhead rule).
pub fn write_sled(sled: &IdentitySled) -> std::io::Result<()> {
    let schema_blob = read_schema_blob().ok();
    write_sled_with_schema_blob(sled, schema_blob.as_deref())
}

fn write_sled_with_schema_blob(
    sled: &IdentitySled,
    schema_blob: Option<&[u8]>,
) -> std::io::Result<()> {
    assert_tmpfs_or_abort(SHM_SLED_PATH)?;

    let tmp = format!("{}.tmp", SHM_SLED_PATH);
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(sled as *const IdentitySled as *const u8, IdentitySled::SIZE)
    };
    let mut f = File::create(&tmp)?;
    // Start owner-only, then widen to the group only if it actually resolves and
    // the chown succeeds. Consumers that drop privileges (notebook-sources-sync
    // runs as `admin`) cannot read a root-owned 0600 sled, but a missing group
    // must fail closed rather than leave identity material world-readable.
    f.set_permissions(fs::Permissions::from_mode(0o600))?;
    if let Some(gid) = gid_for_group(&sled_group()) {
        // SAFETY: `f` is an open fd for the duration of the call. `-1` as uid_t
        // leaves the owner untouched, per chown(2).
        let rc = unsafe { libc::fchown(f.as_raw_fd(), u32::MAX, gid) };
        if rc == 0 {
            // Set the mode after chown: chown(2) clears set-user/group-ID bits.
            f.set_permissions(fs::Permissions::from_mode(0o640))?;
        }
    }
    f.write_all(bytes)?;
    if let Some(schema_blob) = schema_blob {
        write_schema_blob_tail(&mut f, schema_blob)?;
    }
    f.sync_data()?;
    fs::rename(&tmp, SHM_SLED_PATH)?;
    Ok(())
}

fn write_schema_blob_tail(file: &mut File, schema_blob: &[u8]) -> std::io::Result<()> {
    file.write_all(&SCHEMA_BLOB_MAGIC)?;
    file.write_all(&SCHEMA_BLOB_VERSION.to_le_bytes())?;
    file.write_all(&(schema_blob.len() as u64).to_le_bytes())?;
    file.write_all(schema_blob)?;
    Ok(())
}

/// Embed the canonical schema catalog after the fixed `IdentitySled` prefix.
///
/// The first 152 bytes remain the stable sled ABI for zero-copy readers. The
/// appended tail carries the schema blob so consumers can use one SHM artifact.
pub fn write_schema_blob(schema_blob: &[u8]) -> std::io::Result<()> {
    assert_tmpfs_or_abort(SHM_SLED_PATH)?;

    let sled = match fs::read(SHM_SLED_PATH) {
        Ok(bytes) if bytes.len() >= IdentitySled::SIZE => read_sled_prefix(&bytes)?,
        Ok(_) | Err(_) => IdentitySled {
            wireguard_pubkey: [0u8; 32],
            mutation_index: 0,
            hashed_footprint: [0u8; 32],
            trace_id: [0u8; 16],
            schema_version: SCHEMA_BLOB_VERSION,
            vector_id: [0u8; 16],
            reserved: [0u8; 44],
        },
    };

    write_sled_with_schema_blob(&sled, Some(schema_blob))
}

fn read_sled_prefix(bytes: &[u8]) -> std::io::Result<IdentitySled> {
    if bytes.len() < IdentitySled::SIZE {
        return Err(std::io::Error::new(
            ErrorKind::UnexpectedEof,
            "sled prefix is shorter than IdentitySled",
        ));
    }

    let mut sled = std::mem::MaybeUninit::<IdentitySled>::uninit();
    unsafe {
        std::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            sled.as_mut_ptr() as *mut u8,
            IdentitySled::SIZE,
        );
        Ok(sled.assume_init())
    }
}

/// Read the schema catalog embedded in `/dev/shm/plugin_schema.dat`.
pub fn read_schema_blob() -> std::io::Result<Vec<u8>> {
    let bytes = fs::read(SHM_SLED_PATH)?;
    let tail_offset = IdentitySled::SIZE;
    let header_end = tail_offset + SCHEMA_BLOB_HEADER_SIZE;
    if bytes.len() < header_end {
        return Err(std::io::Error::new(
            ErrorKind::NotFound,
            "schema blob tail missing",
        ));
    }

    if bytes[tail_offset..tail_offset + SCHEMA_BLOB_MAGIC.len()] != SCHEMA_BLOB_MAGIC {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "schema blob magic mismatch",
        ));
    }

    let version_offset = tail_offset + SCHEMA_BLOB_MAGIC.len();
    let version = u32::from_le_bytes(
        bytes[version_offset..version_offset + 4]
            .try_into()
            .expect("schema blob version range is fixed"),
    );
    if version != SCHEMA_BLOB_VERSION {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("unsupported schema blob version: {version}"),
        ));
    }

    let len_offset = version_offset + 4;
    let schema_len = u64::from_le_bytes(
        bytes[len_offset..len_offset + 8]
            .try_into()
            .expect("schema blob length range is fixed"),
    ) as usize;
    let schema_start = len_offset + 8;
    let schema_end = schema_start.checked_add(schema_len).ok_or_else(|| {
        std::io::Error::new(ErrorKind::InvalidData, "schema blob length overflow")
    })?;

    if schema_end > bytes.len() {
        return Err(std::io::Error::new(
            ErrorKind::UnexpectedEof,
            "schema blob tail is truncated",
        ));
    }

    Ok(bytes[schema_start..schema_end].to_vec())
}

// ── Reader side (the Shuttle) ────────────────────────────────────────────────

/// Read the sled from shm — zero copy, no allocation.
///
/// # Safety
/// The caller must ensure no concurrent writer is modifying the file
/// mid-read.  `write_sled` uses rename so this is safe in practice.
///
/// The sled path is overridable via the `OP_SLED_PATH` environment variable.
/// This allows tests to point at an isolated/empty memory region so the
/// "SchemaEngine unreachable" branch is deterministic regardless of host
/// SHM state. When the env var is unset, the canonical [`SHM_SLED_PATH`] is
/// used.
pub fn read_sled() -> std::io::Result<(*const IdentitySled, memmap2::Mmap)> {
    let path = std::env::var("OP_SLED_PATH").unwrap_or_else(|_| SHM_SLED_PATH.to_string());
    read_sled_at(&path)
}

/// Read the sled from an explicit path — zero copy, no allocation.
///
/// This is the testable core of [`read_sled`]. Production code should call
/// [`read_sled`] which resolves the path from the environment.
///
/// # Safety
/// The caller must ensure no concurrent writer is modifying the file
/// mid-read.  `write_sled` uses rename so this is safe in practice.
pub fn read_sled_at(path: &str) -> std::io::Result<(*const IdentitySled, memmap2::Mmap)> {
    let file = File::open(path)?;
    let mmap = unsafe { MmapOptions::new().len(IdentitySled::SIZE).map(&file)? };
    let ptr = mmap.as_ptr() as *const IdentitySled;
    Ok((ptr, mmap))
}

/// Why a footprint failed to verify against the live sled — kept transport-
/// agnostic (no `tonic::Status` here) so both gRPC gatekeepers (op-grpc-bridge
/// on the WG uplink, op-cognitive-mcp on the external MCP gateway) can map it
/// to their own error type instead of this crate depending on tonic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintVerifyError {
    /// The sled file couldn't be read (missing, wrong size, no mutation yet).
    SledUnreachable,
    /// The sled exists but is zero-initialized — no valid mutation landed yet.
    InvalidSled,
    /// The request's footprint doesn't match the sled's current hashed footprint.
    Mismatch,
}

/// Verify a request-supplied hex-encoded footprint against the live
/// IdentitySled in shared memory. This is the ONE place the Ghostbridge
/// "Absolute Base" check lives — every gRPC ingress must call this rather
/// than re-implementing the sled read + compare, so the two checks can never
/// drift apart again (see SIGNALS.md: op-cognitive-mcp's interceptor had
/// silently regressed to a presence-only check while op-grpc-bridge's stayed
/// correct).
pub fn verify_ghostbridge_footprint(
    request_footprint_hex: &str,
) -> Result<(), FootprintVerifyError> {
    let (sled_ptr, _mmap) = read_sled().map_err(|_| FootprintVerifyError::SledUnreachable)?;
    let sled = unsafe { &*sled_ptr };

    if sled.hashed_footprint == [0u8; 32] || sled.trace_id == [0u8; 16] {
        return Err(FootprintVerifyError::InvalidSled);
    }

    let expected_footprint = hex::encode(sled.hashed_footprint);
    if request_footprint_hex != expected_footprint {
        return Err(FootprintVerifyError::Mismatch);
    }

    Ok(())
}

#[cfg(test)]
mod ghostbridge_footprint_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn identity_sled_repr_c_layout_is_152_bytes() {
        let size = IdentitySled::SIZE;
        assert_eq!(
            size, 152,
            "IdentitySled must be exactly 152 bytes per spec, got {size} bytes"
        );
    }

    /// Write a raw `IdentitySled` to `path`, point `OP_SLED_PATH` at it, and
    /// exercise `verify_ghostbridge_footprint` — the one shared check both
    /// gRPC gatekeepers call. Single test function (not several) so the
    /// process-global `OP_SLED_PATH` env var isn't racing across parallel
    /// test threads.
    #[test]
    fn verify_ghostbridge_footprint_covers_all_branches() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("op-identity-test-sled-{}.dat", std::process::id()));
        let path_str = path.to_str().unwrap().to_string();
        // SAFETY: this test owns the env var for its whole body and runs as
        // a single test function, so there's no cross-thread interleaving.
        unsafe { std::env::set_var("OP_SLED_PATH", &path_str) };

        // 1. Sled file missing entirely -> SledUnreachable.
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            verify_ghostbridge_footprint("anything"),
            Err(FootprintVerifyError::SledUnreachable)
        );

        // 2. Sled present but zero-initialized -> InvalidSled.
        let zeroed = IdentitySled::default();
        write_raw_sled_for_test(&path, &zeroed);
        assert_eq!(
            verify_ghostbridge_footprint("anything"),
            Err(FootprintVerifyError::InvalidSled)
        );

        // 3. Valid sled, wrong footprint -> Mismatch.
        let valid = IdentitySled {
            hashed_footprint: [0xBB; 32],
            trace_id: [0xEE; 16],
            ..IdentitySled::default()
        };
        write_raw_sled_for_test(&path, &valid);
        assert_eq!(
            verify_ghostbridge_footprint(&hex::encode([0xAA; 32])),
            Err(FootprintVerifyError::Mismatch)
        );

        // 4. Valid sled, matching footprint -> Ok.
        assert_eq!(
            verify_ghostbridge_footprint(&hex::encode([0xBB; 32])),
            Ok(())
        );

        let _ = std::fs::remove_file(&path);
        unsafe { std::env::remove_var("OP_SLED_PATH") };
    }

    fn write_raw_sled_for_test(path: &std::path::Path, sled: &IdentitySled) {
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(sled as *const IdentitySled as *const u8, IdentitySled::SIZE)
        };
        let mut f = std::fs::File::create(path).expect("create test sled file");
        f.write_all(bytes).expect("write test sled bytes");
    }
}

// ── Unix socket endpoint ──────────────────────────────────────────────────────

/// A nicless container endpoint: a subdomain routed straight into a unix socket.
///
/// Declared via `UNIX_SOCKET_ENDPOINTS=label:path:subdomain[,…]`, e.g.:
///   `qdrant:/run/qdrant.sock:qdrant.ghostbridge.tech`
///
/// Traffic enters through the shared REALITY/TLS ingress; xray sniffs the SNI
/// and a domain routing rule sends it to the `to-<label>` gRPC outbound, which
/// redirects to the tonic-web bridge.  The bridge uses gRPC reflection to
/// demux by service/method and dials the container's unix socket natively.
/// No per-socket TCP inbound, no NIC, no xray domainsocket.
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
            (!domain.is_empty()).then_some(SocketEntry {
                label,
                path,
                domain,
            })
        })
        .collect()
}

// ── Gemma xray routes ───────────────────────────────────────────────────────

const SHM_XRAY_ROUTES: &str = "/dev/shm/xray-routes.json";

#[derive(Debug, Clone, Deserialize)]
struct XrayRoute {
    tag: String,
    subdomains: Vec<String>,
    backend: GemmaBackend,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum GemmaBackend {
    #[serde(rename = "ingress")]
    Ingress,
    #[serde(rename = "tcp")]
    Tcp { host: String, port: u16 },
    #[serde(rename = "grpc")]
    Grpc {
        host: String,
        port: u16,
        #[serde(rename = "service_name")]
        service_name: String,
    },
    #[serde(rename = "unix")]
    Unix {
        /// Socket path is retained for deserialization compatibility but no
        /// longer used by xray — the tonic-web bridge dials the socket
        /// natively via gRPC reflection.
        #[allow(dead_code)]
        path: String,
    },
    #[serde(rename = "dns")]
    Dns { host: String, port: u16 },
    /// Internal metadata entry — not xray-routable, produces no outbound.
    #[serde(rename = "file")]
    File {
        #[allow(dead_code)]
        path: String,
    },
    /// Internal service — not xray-routable, produces no outbound.
    #[serde(rename = "internal")]
    Internal,
}

/// Load the Gemma routing map from `/dev/shm/xray-routes.json` if present.
/// Falls back to an empty vector so legacy env-only operation still works.
fn load_xray_routes() -> Vec<XrayRoute> {
    let Ok(raw) = fs::read_to_string(SHM_XRAY_ROUTES) else {
        return vec![];
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return vec![];
    };
    value
        .get("xray_routes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
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
    let routes = load_xray_routes();
    let config = build_xray_config(
        footprint,
        trace_id,
        nextdns_profile,
        uuid,
        private_key,
        short_id,
        sockets,
        &routes,
    );
    let tmp = format!("{}.tmp", SHM_XRAY_CONFIG);
    let mut f = File::create(&tmp)?;
    f.write_all(config.as_bytes())?;
    f.sync_data()?;
    fs::rename(&tmp, SHM_XRAY_CONFIG)?;
    Ok(())
}

/// Build the Xray config JSON as a string (pure — no I/O).
///
/// Routes are sourced from Gemma (`/dev/shm/xray-routes.json`) and merged with
/// legacy `UNIX_SOCKET_ENDPOINTS` entries.  Gemma routes take precedence; legacy
/// sockets are only added when no Gemma route has the same tag.  The final
/// catch-all rule sends unmatched ingress traffic to the grpc-bridge, matching
/// the historical fallback behavior.
#[allow(clippy::too_many_arguments)]
fn build_xray_config(
    footprint: &str,
    trace_id: &str,
    nextdns_profile: &str,
    uuid: &str,
    private_key: &str,
    short_id: &str,
    sockets: &[SocketEntry],
    routes: &[XrayRoute],
) -> String {
    // Gemma-supplied routes; empty when Gemma has not yet run.
    let mut routes = routes.to_vec();
    let mut seen_tags: HashSet<String> = routes.iter().map(|r| r.tag.clone()).collect();

    // Merge legacy UNIX_SOCKET_ENDPOINTS entries for backward compatibility.
    for socket in sockets {
        let tag = socket.label.clone();
        if seen_tags.contains(&tag) {
            continue;
        }
        seen_tags.insert(tag.clone());
        routes.push(XrayRoute {
            tag,
            subdomains: vec![socket.domain.clone()],
            backend: GemmaBackend::Unix {
                path: socket.path.clone(),
            },
        });
    }

    // Outbound for each route that has a backend (ingress tags have none).
    let route_outbounds: String = routes
        .iter()
        .filter_map(|r| route_to_outbound(footprint, trace_id, r))
        .collect();

    // Subdomain routing rules for each route with subdomains and a real backend.
    // Ingress-only entries (e.g. xray-tls/xray-reality) describe the listener,
    // not a destination, so no routing rule is emitted for them.
    let route_rules: String = routes
        .iter()
        .filter_map(|r| {
            if r.subdomains.is_empty() || matches!(r.backend, GemmaBackend::Ingress) {
                return None;
            }
            let domains = r
                .subdomains
                .iter()
                .map(|d| format!("\"full:{d}\""))
                .collect::<Vec<_>>()
                .join(", ");
            Some(format!(
                r#",
      {{ "type": "field", "inboundTag": ["op-tls", "ghostbridge-reality"], "domain": [{domains}], "outboundTag": "to-{tag}" }}"#,
                tag = r.tag,
                domains = domains,
            ))
        })
        .collect();

    // Fallback bridge outbound if Gemma did not provide it.
    let fallback_outbound = if seen_tags.contains("grpc-bridge") {
        String::new()
    } else {
        format!(
            r#",
    {{
      "tag": "to-grpc-bridge",
      "protocol": "freedom",
      "settings": {{ "redirect": "{host}:{port}" }},
      "streamSettings": {{
        "network": "xhttp",
        "sockopt": {{ "tcpNoDelay": true }},
        "xhttpSettings": {{
          "host": "{host}",
          "path": "/Ghostbridge.StateSync",
          "mode": "auto"
        }}
      }}
    }}"#,
            host = GRPC_BRIDGE_HOST,
            port = GRPC_BRIDGE_PORT,
        )
    };

    // Final catch-all fallback to the bridge.
    let fallback_rule = if seen_tags.contains("grpc-bridge") {
        String::from(
            r#",
      {
        "type": "field",
        "inboundTag": ["op-tls", "ghostbridge-reality"],
        "outboundTag": "to-grpc-bridge"
      }"#,
        )
    } else {
        String::new()
    };

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
        "fallbacks": [{{ "dest": {bridge_port} }}]
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
      "tag": "direct",
      "protocol": "freedom"
    }},
    {{ "tag": "dns-out", "protocol": "dns" }}{route_outbounds}{fallback_outbound}
  ],
  "routing": {{
    "domainStrategy": "AsIs",
    "rules": [
      {{ "type": "field", "port": 53, "outboundTag": "dns-out" }}{route_rules}{fallback_rule}
    ]
  }}
}}"#,
        profile = nextdns_profile,
        uuid = uuid,
        private_key = private_key,
        short_id = short_id,
        route_outbounds = route_outbounds,
        fallback_outbound = fallback_outbound,
        route_rules = route_rules,
        fallback_rule = fallback_rule,
        tls_certs = tls_certs,
        bridge_port = GRPC_BRIDGE_PORT,
    );

    config
}

/// Translate a Gemma route into an xray outbound JSON fragment.
fn route_to_outbound(_footprint: &str, _trace_id: &str, route: &XrayRoute) -> Option<String> {
    let tag = &route.tag;
    match &route.backend {
        GemmaBackend::Ingress => None,
        GemmaBackend::Tcp { host, port } => Some(format!(
            r#",
    {{
      "tag": "to-{tag}",
      "protocol": "freedom",
      "settings": {{ "redirect": "{host}:{port}" }}
    }}"#
        )),
        GemmaBackend::Grpc {
            host,
            port,
            service_name,
        } => Some(format!(
            r#",
    {{
      "tag": "to-{tag}",
      "protocol": "freedom",
      "settings": {{ "redirect": "{host}:{port}" }},
      "streamSettings": {{
        "network": "xhttp",
        "sockopt": {{ "tcpNoDelay": true }},
        "xhttpSettings": {{
          "host": "{host}",
          "path": "/{service_name}",
          "mode": "auto"
        }}
      }}
    }}"#,
            tag = tag,
            host = host,
            port = port,
            service_name = service_name,
        )),
        GemmaBackend::Unix { path: _path } => Some(format!(
            r#",
    {{
      "tag": "to-{tag}",
      "protocol": "freedom",
      "settings": {{ "redirect": "{host}:{port}" }},
      "streamSettings": {{
        "network": "xhttp",
        "sockopt": {{ "tcpNoDelay": true }},
        "xhttpSettings": {{
          "host": "{host}",
          "path": "/{tag}",
          "mode": "auto"
        }}
      }}
    }}"#,
            tag = tag,
            host = GRPC_BRIDGE_HOST,
            port = GRPC_BRIDGE_PORT,
        )),
        GemmaBackend::Dns { host, port } => Some(format!(
            r#",
    {{
      "tag": "to-{tag}",
      "protocol": "freedom",
      "settings": {{ "redirect": "{host}:{port}" }}
    }}"#,
            tag = tag,
            host = host,
            port = port,
        )),
        GemmaBackend::File { .. } | GemmaBackend::Internal => None,
    }
}

// ── WireGuard-driven sled writer ─────────────────────────────────────────────

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
/// Blake3( wg_pubkey || schema_catalog_hash(blob-catalog manifest) || mutation_index || source_port ).
/// source_port is the per-session WireGuard-observed source port (0 when none). It binds the
/// footprint to the session network context for the accountability loop -- it is NOT an auth
/// factor (WireGuard is the authenticator; see op-grpc-bridge GhostbridgeInterceptor).
/// Manifest published by the blob sealer — the ONE place the canonical
/// `catalog_hash` lives (blake3 leaf-fold over the sealed per-plugin blob
/// hashes; the blob catalog IS the plugin set).
const SHM_BLOB_MANIFEST_PATH: &str = "/dev/shm/opdbus/plugin-blobs/.manifest.json";
/// Legacy manifest published by op-projection's SchemaEngine; transitional
/// fallback until every host is re-sealed with the blob-catalog manifest.
const SHM_LEGACY_MANIFEST_PATH: &str = "/dev/shm/opdbus/.manifest.json";
/// Legacy derived monolith; last-resort fallback (deploy ordering).
const SHM_LIVE_SCHEMA_PATH: &str = "/dev/shm/live-schema.json";

/// The single canonical schema-catalog hash (32 bytes), computed ONCE by the
/// blob sealer and read here — never re-hashed per call site. Reads the blob
/// catalog manifest's `catalog_hash`; falls back to the legacy SchemaEngine
/// manifest, then the monolith, on hosts not yet re-sealed. `None` if no
/// artifact exists.
pub fn schema_catalog_hash() -> Option<[u8; 32]> {
    for manifest in [SHM_BLOB_MANIFEST_PATH, SHM_LEGACY_MANIFEST_PATH] {
        if let Ok(bytes) = std::fs::read(manifest) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(hex_str) = v.get("catalog_hash").and_then(|h| h.as_str()) {
                    if let Ok(raw) = hex::decode(hex_str) {
                        if let Ok(arr) = <[u8; 32]>::try_from(raw.as_slice()) {
                            return Some(arr);
                        }
                    }
                }
            }
        }
    }
    std::fs::read(SHM_LIVE_SCHEMA_PATH)
        .ok()
        .map(|bytes| *blake3::hash(&bytes).as_bytes())
}

pub fn etch_footprint(
    wireguard_pubkey: &[u8; 32],
    mutation_index: u64,
    source_port: u16,
) -> [u8; 32] {
    // A missing catalog hash used to fall back to zeros silently, which makes a
    // footprint that binds *no contract* indistinguishable from one that binds a
    // real one. The fallback stays — refusing to stamp would break the mutation
    // path — but it is now audible, because it means the proof is weaker than it
    // looks and that is worth knowing at the time it happens.
    let schema_catalog_hash = match schema_catalog_hash() {
        Some(hash) => hash,
        None => {
            tracing::warn!(
                mutation_index,
                "no published catalog hash; footprint binds zeros and cannot \
                 attest which contract this session operated against"
            );
            [0u8; 32]
        }
    };

    let mut hasher = blake3::Hasher::new();
    hasher.update(wireguard_pubkey);
    hasher.update(&schema_catalog_hash);
    hasher.update(&mutation_index.to_le_bytes());
    hasher.update(&source_port.to_le_bytes());
    hasher.finalize().into()
}

/// Per-session WireGuard source port observed for peer_pubkey, parsed from
/// `wg show <iface> dump` (`iface` from `WG_INTERFACE`, default `wg0`).
///
/// Returns 0 when the peer has no current endpoint (not connected, or a local self-write),
/// so the footprint degrades gracefully to the port-less base.
/// Transport port bound into a session's footprint.
///
/// `0` means the session did not arrive over a routable transport — which is
/// the normal case for a provisioned session container, whose only device is
/// the shared `ghostbridge` Unix socket and which therefore has no peer
/// endpoint at all. A non-zero value is supplied by a caller that genuinely
/// knows the port the session arrived on.
///
/// This used to shell out to `wg show <iface> dump` with `WG_INTERFACE`
/// defaulting to `wg0`. That was wrong twice over: subprocesses are forbidden
/// on this path, and no interface named `wg0` exists on this host (they are
/// `netmaker`, `wgcf-egress`, `wgcf-uiStream`), so every footprint ever
/// written bound a silent zero while appearing to bind network context.
/// Reading a real peer endpoint requires the WireGuard genetlink protocol; a
/// caller that has it can pass the port in rather than have this guess.
pub const TRANSPORT_PORT_NONE: u16 = 0;

/// The position currently recorded in the sled, or `0` when no record is
/// readable.
///
/// A caller that is refreshing identity rather than advancing the account
/// passes this, so its write holds position instead of regressing it.
pub fn current_mutation_index() -> u64 {
    match read_sled() {
        // SAFETY: `read_sled` hands back a pointer into a live mapping; the
        // field is copied out before `_mmap` drops.
        Ok((ptr, _mmap)) => unsafe { (*ptr).mutation_index },
        Err(_) => 0,
    }
}

/// Refresh the sled's WireGuard identity, holding the session's position.
///
/// Reads `GB_TRACE_ID` from the environment to propagate an existing trace.
/// All extra metadata (subid, compliance, routing) lives in environment
/// variables — the sled itself is the spec layout and nothing more.
///
/// This is not a mutation, so it does not advance `mutation_index`. It used to
/// stamp a **process-local** counter starting at zero, which meant every
/// process that calls it — op-mcp, op-cognitive-mcp, the bridge's sled
/// dispatch — overwrote the pipeline's real position with its own first write.
/// That is precisely the regression the advance-only guard in
/// [`write_sled_advance`] exists to reject, so the counter is gone rather than
/// merely blocked.
pub fn write_sled_from_wg(peer_pubkey: &str) -> std::io::Result<()> {
    let trace_id_hex = env::var("GB_TRACE_ID").unwrap_or_default();
    write_sled_advance(
        peer_pubkey,
        current_mutation_index(),
        trace_id_hex.trim(),
        TRANSPORT_PORT_NONE,
    )
}

/// Write the sled for one mutation — called from SchemaEngine on mutation.
///
/// `trace_id_hex` is hex; if empty the session's established trace is kept.
///
/// Update the sled for one mutation, preserving everything the caller did not
/// set.
///
/// This is a read-modify-write, not a reconstruction. The previous version
/// rebuilt all seven fields on every call, which meant `vector_id` — the link
/// binding vectorized reasoning episodes to this session — was zeroed by every
/// single mutation, and any writer could silently move `mutation_index`
/// backwards.
///
/// `mutation_index` advances only. The sled's role is to deliver a complete
/// account of a session: given a container, which slice of the chain is its
/// session and which contract it ran against. A counter that can regress
/// cannot do that. `hashed_footprint` is minted once and then kept: it is the
/// session wristband. Re-etching it from the index on every write made every
/// in-flight client fail the interceptor.
///
/// `source_port` is [`TRANSPORT_PORT_NONE`] unless the caller knows the port
/// the session actually arrived on.
pub fn write_sled_full(
    peer_pubkey: &str,
    mutation_index: u64,
    trace_id_hex: &str,
) -> std::io::Result<()> {
    write_sled_advance(peer_pubkey, mutation_index, trace_id_hex, TRANSPORT_PORT_NONE)
}

/// [`write_sled_full`] with an explicit transport port.
pub fn write_sled_advance(
    peer_pubkey: &str,
    mutation_index: u64,
    trace_id_hex: &str,
    source_port: u16,
) -> std::io::Result<()> {
    // Read what is already recorded so this write supplements it rather than
    // replacing it. A missing or unreadable sled starts from defaults.
    let previous: Option<IdentitySled> = match read_sled() {
        // SAFETY: `read_sled` hands back a pointer into a live, page-aligned
        // mapping of exactly `IdentitySled::SIZE`; the record is copied out
        // before `_mmap` drops, so nothing below borrows the mapping.
        Ok((ptr, _mmap)) => Some(unsafe { *ptr }),
        Err(_) => None,
    };

    let wireguard_pubkey = if peer_pubkey.is_empty() {
        previous.map(|p| p.wireguard_pubkey).unwrap_or([0u8; 32])
    } else {
        decode_wg_pubkey(peer_pubkey)
    };

    // Advance-only: a stale writer must never roll the account backwards.
    let previous_index = previous.map(|p| p.mutation_index).unwrap_or(0);
    if mutation_index < previous_index {
        tracing::debug!(
            attempted = mutation_index,
            current = previous_index,
            "ignoring out-of-order sled write"
        );
        return Ok(());
    }
    let mutation_index = mutation_index.max(previous_index);

    let hashed_footprint = match previous {
        Some(p) if p.hashed_footprint != [0u8; 32] => p.hashed_footprint,
        _ => etch_footprint(&wireguard_pubkey, mutation_index, source_port),
    };

    let trace_id: [u8; 16] = if trace_id_hex.is_empty() {
        // Preserve an established trace rather than minting a new one; a fresh
        // uuid per mutation would sever the session's own continuity.
        previous
            .map(|p| p.trace_id)
            .filter(|t| t != &[0u8; 16])
            .unwrap_or_else(|| uuid::Uuid::new_v4().into_bytes())
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
        // Owned by the vector path, never by a mutation write.
        vector_id: previous.map(|p| p.vector_id).unwrap_or([0u8; 16]),
        schema_version: SLED_SCHEMA_VERSION,
        reserved: [0u8; 44],
    };
    write_sled(&sled)
}

/// Watch for new WireGuard peers using `ip monitor` — fires instantly on handshake,
/// no polling delay. Re-writes the sled and xray config on each new peer.
/// Runs forever; call from a thread.
pub fn watch_wireguard_handshakes(iface: &str) {
    let iface = iface.to_string();

    // `ip monitor route` on the host fires the instant a WireGuard peer route
    // appears — no polling delay. WireGuard (wg0) now runs on the host, so we
    // invoke `ip` directly instead of `incus exec wg-xray -- ip ...` (the
    // wg-xray container is deprecated and stopped).
    let mut monitor = loop {
        match Command::new("ip")
            .args(["monitor", "route"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => break child,
            Err(e) => {
                tracing::warn!("ip monitor route spawn failed: {} — retrying in 5s", e);
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

        // Read current peers from the host WireGuard interface immediately
        // (wg0 now runs on the host; the deprecated wg-xray container is gone).
        let Ok(out) = Command::new("wg")
            .args(["show", &iface, "latest-handshakes"])
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
                let Ok(profile) = env::var("NEXTDNS_PROFILE_ID") else {
                    continue;
                };
                let Ok(uuid) = env::var("XRAY_UUID") else {
                    continue;
                };
                let Ok(privkey) = env::var("XRAY_PRIVATE_KEY") else {
                    continue;
                };
                let Ok(short) = env::var("XRAY_SHORT_ID") else {
                    continue;
                };
                if let Err(e) =
                    write_xray_config(&footprint_hex, &trace_id, &profile, &uuid, &privkey, &short)
                {
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

    // 5. Reload Xray so it re-reads the freshly written /dev/shm config.
    // Host-native control (SIGHUP). Deliberate start/stop lifecycle belongs to the
    // `xray` state plugin projected at /org/opdbus/v1/plugins/xray — NOT an
    // out-of-tree `opdbus.v1.Xray` daemon name. Xray is supervised by s6 (gbr-xray)
    // running `xray run -config /dev/shm/xray-ghostbridge.json`.
    reload_xray()?;

    // 6. Watch for new WireGuard handshakes and keep the sled current
    let iface = env::var("WG_INTERFACE").unwrap_or_else(|_| "wg0".to_string());
    std::thread::spawn(move || watch_wireguard_handshakes(&iface));

    Ok(())
}

/// Reload the running Xray so it re-reads the freshly generated /dev/shm config.
///
/// Host-native control via SIGHUP (Xray re-reads its config without dropping
/// connections). Deliberate start/stop lifecycle lives on the `xray` state plugin
/// at `/org/opdbus/v1/plugins/xray` — the only projectable tree — never an
/// out-of-tree `opdbus.v1.Xray` name.
fn reload_xray() -> Result<(), Box<dyn std::error::Error>> {
    let status = std::process::Command::new("pkill")
        .args(["-HUP", "-x", "xray"])
        .status()?;
    if status.success() {
        tracing::info!("Xray reloaded (SIGHUP) — re-read {}", SHM_XRAY_CONFIG);
    } else {
        // pkill exits non-zero when nothing matched: Xray isn't running yet.
        // s6 (gbr-xray) is responsible for starting it with the shm config.
        tracing::warn!(
            "No running xray to SIGHUP; expecting s6 (gbr-xray) to start it with {}",
            SHM_XRAY_CONFIG
        );
    }
    Ok(())
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
        let cfg = build_xray_config(
            "foot",
            "trace",
            "abc123",
            "uuid",
            "pk",
            "sid",
            &sockets,
            &[],
        );

        // Must be valid JSON.
        let v: serde_json::Value = serde_json::from_str(&cfg).expect("valid json");

        // No per-socket TCP inbounds — only the two shared ingress listeners.
        let inbounds = v["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 2, "no dokodemo socket inbounds expected");

        // Each socket routes through XHTTP — xray does not dial unix sockets
        // directly in this branch.
        let outbounds = v["outbounds"].as_array().unwrap();
        let qdrant = outbounds
            .iter()
            .find(|o| o["tag"] == "to-qdrant")
            .expect("to-qdrant outbound");
        assert_eq!(qdrant["streamSettings"]["network"], "xhttp");
        assert_eq!(qdrant["settings"]["redirect"], "127.0.0.1:50051");
        assert_eq!(
            qdrant["streamSettings"]["xhttpSettings"]["host"],
            "127.0.0.1"
        );
        assert_eq!(qdrant["streamSettings"]["xhttpSettings"]["mode"], "auto");

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
    fn gemma_routes_generate_outbounds_and_rules() {
        let routes = vec![
            XrayRoute {
                tag: "cognitive-mcp".into(),
                subdomains: vec!["mcp.internal".into()],
                backend: GemmaBackend::Grpc {
                    host: "127.0.0.1".into(),
                    port: GRPC_BRIDGE_PORT,
                    service_name: "operation.cognitive.v1.CognitiveToolService".into(),
                },
            },
            XrayRoute {
                tag: "qdrant".into(),
                subdomains: vec!["qdrant.ghostbridge.tech".into()],
                backend: GemmaBackend::Unix {
                    path: "/run/qdrant.sock".into(),
                },
            },
        ];
        let cfg = build_xray_config("foot", "trace", "abc123", "uuid", "pk", "sid", &[], &routes);
        let v: serde_json::Value = serde_json::from_str(&cfg).expect("valid json");

        let outbounds = v["outbounds"].as_array().unwrap();
        assert!(outbounds.iter().any(|o| o["tag"] == "to-cognitive-mcp"));
        assert!(outbounds.iter().any(|o| o["tag"] == "to-qdrant"));
        let qdrant = outbounds
            .iter()
            .find(|o| o["tag"] == "to-qdrant")
            .expect("qdrant outbound");
        assert_eq!(qdrant["streamSettings"]["network"], "xhttp");
        assert_eq!(qdrant["streamSettings"]["xhttpSettings"]["path"], "/qdrant");

        let rules = v["routing"]["rules"].as_array().unwrap();
        assert!(rules.iter().any(|r| {
            r["outboundTag"] == "to-cognitive-mcp"
                && r["domain"]
                    .as_array()
                    .map(|d| d.contains(&serde_json::json!("full:mcp.internal")))
                    .unwrap_or(false)
        }));
        assert!(rules.iter().any(|r| {
            r["outboundTag"] == "to-qdrant"
                && r["domain"]
                    .as_array()
                    .map(|d| d.contains(&serde_json::json!("full:qdrant.ghostbridge.tech")))
                    .unwrap_or(false)
        }));
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
