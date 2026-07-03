use anyhow::{bail, Context, Result};
use op_identity::schema_bridge::{
    read_schema_blob, write_sled_from_wg, IdentitySled, SHM_SLED_PATH,
};
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
    pubkey: Option<String>,
    trace_id: Option<String>,
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
    schema_blob_bytes: usize,
    trace_id: String,
    schema_version: u32,
}

fn main() -> Result<()> {
    let args = parse_args()?;

    if args.refresh {
        let pubkey = match args.pubkey.as_deref() {
            Some(pubkey) => pubkey.to_string(),
            None => read_wg_pubkey(&args.iface)?,
        };
        if let Some(trace_id) = args.trace_id.as_deref() {
            env::set_var("GB_TRACE_ID", normalize_trace_id(trace_id)?);
        }
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
        pubkey: env::var("WG_PUBKEY").ok(),
        trace_id: env::var("GB_TRACE_ID").ok(),
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
            "--pubkey" => {
                parsed.pubkey = Some(
                    args.next()
                        .context("--pubkey requires a WireGuard public key")?,
                );
            }
            "--trace-id" => {
                parsed.trace_id = Some(
                    args.next()
                        .context("--trace-id requires a hex or UUID value")?,
                );
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
Usage:\n  op-identity-sled [--path FILE] [--iface IFACE] [--pubkey KEY] [--trace-id ID] [--refresh] [--pretty]\n\n\
Options:\n  --path FILE     Read sled from FILE instead of /dev/shm/plugin_schema.dat\n  -i, --iface     WireGuard interface used with --refresh, default wg0\n  --pubkey KEY    Detached WireGuard public key used with --refresh instead of wg show\n  --trace-id ID   Detached random wristband trace ID, as UUID or 32 hex chars\n  --refresh       Rewrite the sled from --pubkey, WG_PUBKEY, or wg show <iface> public-key\n  --pretty        Print a compact human-readable view instead of JSON\n\n\
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

fn normalize_trace_id(trace_id: &str) -> Result<String> {
    let compact = trace_id.trim().replace('-', "");
    if compact.len() != 32 || !compact.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("--trace-id must be a UUID or 32 hex characters");
    }
    Ok(compact)
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
        let (schema_catalog_hash, schema_blob_bytes) = schema_blob_summary();

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
            schema_blob_bytes,
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
        let (schema_catalog_hash, schema_blob_bytes) = schema_blob_summary();

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
            schema_blob_bytes,
            trace_id: trace_id(&wg, mutation_index),
            schema_version: 0,
        }
    }
}

fn schema_blob_summary() -> (String, usize) {
    match read_schema_blob().or_else(|_| std::fs::read("/dev/shm/live-schema.json")) {
        Ok(bytes) => (hex::encode(blake3::hash(&bytes).as_bytes()), bytes.len()),
        Err(_) => ("(missing)".to_string(), 0),
    }
}

fn trace_id(wireguard_pubkey: &[u8; 32], mutation_index: u64) -> String {
    format!("{}-{}", hex::encode(&wireguard_pubkey[..4]), mutation_index)
}

fn encode_b64(bytes: &[u8; 32]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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
