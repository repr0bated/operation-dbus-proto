//! op-identity-shuttle — The Shuttle: run the schema shuttle once.
//!
//! Reads the Identity Sled from `/dev/shm/plugin_schema.dat`, stamps the
//! Ghostbridge footprint/trace/wg-pubkey into the environment, and writes the
//! dynamic xray config to `/dev/shm/xray-ghostbridge.json`. Then it reloads the
//! running xray via SIGHUP so the new config takes effect.
//!
//! Required secrets are loaded from `/etc/ghostbridge/xray.env` by default.
//! The env file can be overridden with `--env-file`.
//!
//! Per AGENTS.md: The Shuttle is the Rust bridge/courier; xray lifecycle is
//! owned by the `xray` state plugin at `/org/opdbus/v1/plugins/xray` only.

use anyhow::{Context, Result};
use op_identity::run_schema_shuttle;
use std::env;
use std::fs;
use std::path::PathBuf;

const DEFAULT_ENV_FILE: &str = "/etc/ghostbridge/xray.env";

fn main() -> Result<()> {
    let args = parse_args()?;
    load_env_file(&args.env_file)?;

    run_schema_shuttle().map_err(|e| anyhow::anyhow!("schema shuttle failed: {e}"))?;
    Ok(())
}

struct Args {
    env_file: PathBuf,
}

fn parse_args() -> Result<Args> {
    let mut env_file = PathBuf::from(DEFAULT_ENV_FILE);
    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--env-file" => {
                let path = iter.next().context("--env-file requires a path")?;
                env_file = PathBuf::from(path);
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }
    Ok(Args { env_file })
}

fn print_help() {
    println!(
        "op-identity-shuttle\n\n\
Usage:\n  op-identity-shuttle [--env-file PATH]\n\n\
Options:\n  --env-file PATH  Load XRAY_* / NEXTDNS_* secrets from PATH\n                   (default: /etc/ghostbridge/xray.env)\n  -h, --help       Show this help\n\n\
Reads the Identity Sled from /dev/shm/plugin_schema.dat and writes the\n\
dynamic xray config to /dev/shm/xray-ghostbridge.json, then SIGHUPs xray.\n"
    );
}

/// Load a simple KEY=VALUE env file, skipping comments and blank lines.
fn load_env_file(path: &PathBuf) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("env file not found: {}", path.display());
    }
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read env file: {}", path.display()))?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        env::set_var(key.trim(), value.trim());
    }
    Ok(())
}
