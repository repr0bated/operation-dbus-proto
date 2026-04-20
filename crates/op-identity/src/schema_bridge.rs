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

/// THE SLED — 1:1 zero-copy shared memory layout.
///
/// Written by SchemaEngine (see `write_sled`), read by the Shuttle via
/// a raw pointer cast.  Never touches disk; lives entirely in tmpfs.
#[repr(C)]
pub struct IdentitySled {
    /// WireGuard public key of the active peer (32 bytes, raw Curve25519).
    pub wireguard_pubkey: [u8; 32],
    /// Monotonic mutation index — incremented on every schema change.
    pub mutation_index: u64,
    /// Blake3 / SHA-256 hashed footprint — the vectorized "Thought".
    pub hashed_footprint: [u8; 32],
}

impl IdentitySled {
    pub const SIZE: usize = std::mem::size_of::<Self>();
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

// ── Xray config generator ─────────────────────────────────────────────────────

fn write_xray_config(
    footprint: &str,
    trace_id: &str,
    nextdns_profile: &str,
    uuid: &str,
    private_key: &str,
    short_id: &str,
) -> std::io::Result<()> {
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
      "listen": "10.88.88.1",
      "protocol": "socks",
      "settings": {{ "auth": "noauth", "udp": true }}
    }},
    {{
      "tag": "ovs-tproxy-in",
      "port": 12345,
      "listen": "10.88.88.1",
      "protocol": "dokodemo-door",
      "settings": {{ "network": "tcp,udp", "followRedirect": true }},
      "streamSettings": {{ "sockopt": {{ "tproxy": "tproxy" }} }}
    }}
  ],
  "outbounds": [
    {{
      "tag": "to-grpc-bridge",
      "protocol": "freedom",
      "sendThrough": "10.88.88.1",
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
      "tag": "direct",
      "protocol": "freedom"
    }},
    {{ "tag": "dns-out", "protocol": "dns" }}
  ],
  "routing": {{
    "domainStrategy": "IPIfNonMatch",
    "rules": [
      {{ "type": "field", "port": 53, "outboundTag": "dns-out" }},
      {{
        "type": "field",
        "inboundTag": ["ovs-socks-in", "ovs-tproxy-in"],
        "domain": ["full:dashboard.3tched.com", "full:grpc.internal"],
        "outboundTag": "to-grpc-bridge"
      }},
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
/// `peer_pubkey` is the base64-encoded Curve25519 public key of the
/// authenticated peer whose handshake just completed.
pub fn write_sled_from_wg(peer_pubkey: &str) -> std::io::Result<()> {
    use sha2::{Digest, Sha256};

    let wireguard_pubkey = decode_wg_pubkey(peer_pubkey);
    let mut hasher = Sha256::new();
    hasher.update(&wireguard_pubkey);
    let hashed_footprint: [u8; 32] = hasher.finalize().into();
    let mutation_index = MUTATION_INDEX.fetch_add(1, Ordering::Relaxed);

    let sled = IdentitySled { wireguard_pubkey, mutation_index, hashed_footprint };
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

        let Ok(out) = Command::new("wg")
            .args(["show", &iface, "latest-handshakes"])
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

            // Re-bake xray config with new footprint
            if let Ok((ptr, _mmap)) = read_sled() {
                let sled = unsafe { &*ptr };
                let footprint_hex = hex::encode(sled.hashed_footprint);
                let trace_id = format!("{}-{}", hex::encode(&sled.wireguard_pubkey[..4]), sled.mutation_index);
                let profile = env::var("NEXTDNS_PROFILE_ID").unwrap_or_else(|_| "689ec7".to_string());
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

    // 4. Write stateless Xray config into shm
    let nextdns_profile = env::var("NEXTDNS_PROFILE_ID")
        .unwrap_or_else(|_| "689ec7".to_string());
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
