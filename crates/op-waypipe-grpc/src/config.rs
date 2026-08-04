use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    /// gRPC listen address for `serve` (host:port).
    pub listen: String,
    /// Basename allow-list for remote commands (first argv element).
    pub allowed_commands: Vec<String>,
    /// Path to the waypipe binary.
    pub waypipe_bin: String,
    /// Directory for per-session Unix sockets.
    pub socket_dir: PathBuf,
    /// Default `--compress` value when the client omits one.
    pub default_compress: String,
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:50052".into(),
            allowed_commands: vec![
                "Hyprland".into(),
                "hyprland".into(),
                "kitty".into(),
                "weston-terminal".into(),
                "foot".into(),
                "alacritty".into(),
            ],
            waypipe_bin: "waypipe".into(),
            socket_dir: PathBuf::from("/tmp/op-waypipe-grpc"),
            default_compress: "lz4".into(),
        }
    }
}

/// Embedded packaged defaults (works when the binary is copied without the repo).
const EMBEDDED_DEFAULT_JSON: &str = include_str!("../config/default.json");

impl TunnelConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let raw =
            fs::read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
        Self::from_json(&raw).with_context(|| format!("parse JSON config {}", path.display()))
    }

    pub fn from_json(raw: &str) -> Result<Self> {
        let cfg: Self = serde_json::from_str(raw).context("parse TunnelConfig JSON")?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn embedded_default() -> Result<Self> {
        Self::from_json(EMBEDDED_DEFAULT_JSON)
    }

    /// Resolve config from an optional CLI path, then common install locations,
    /// then the embedded default JSON.
    pub fn load_resolved(cli_path: Option<&Path>) -> Result<(Self, String)> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(p) = cli_path {
            if !p.as_os_str().is_empty() {
                candidates.push(p.to_path_buf());
            }
        }
        if let Ok(p) = std::env::var("OP_WAYPIPE_GRPC_CONFIG") {
            candidates.push(PathBuf::from(p));
        }
        if let Some(home) = std::env::var_os("HOME") {
            candidates.push(PathBuf::from(home).join(".config/op-waypipe-grpc/config.json"));
        }
        candidates.push(PathBuf::from("/etc/op-waypipe-grpc/config.json"));

        for path in &candidates {
            if path.is_file() {
                let cfg = Self::load(path)?;
                return Ok((cfg, path.display().to_string()));
            }
        }

        let cfg = Self::embedded_default()?;
        Ok((cfg, "embedded:default.json".into()))
    }

    pub fn validate(&self) -> Result<()> {
        if self.listen.trim().is_empty() {
            bail!("listen must be non-empty host:port");
        }
        if self.allowed_commands.is_empty() {
            bail!("allowed_commands must not be empty");
        }
        if self.waypipe_bin.trim().is_empty() {
            bail!("waypipe_bin must be set");
        }
        Ok(())
    }

    pub fn command_allowed(&self, command: &[String]) -> Result<()> {
        let Some(bin) = command.first() else {
            bail!("command argv is empty");
        };
        let base = Path::new(bin)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(bin);
        if self.allowed_commands.iter().any(|a| a == base) {
            Ok(())
        } else {
            bail!("command `{base}` is not in allowed_commands");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_disallowed_command() {
        let cfg = TunnelConfig::default();
        assert!(cfg.command_allowed(&["Hyprland".into()]).is_ok());
        assert!(cfg.command_allowed(&["/usr/bin/rm".into()]).is_err());
    }

    #[test]
    fn loads_default_json() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config/default.json");
        let cfg = TunnelConfig::load(&path).expect("default.json");
        assert_eq!(cfg.listen, "0.0.0.0:50052");
        assert!(cfg.allowed_commands.iter().any(|c| c == "Hyprland"));
    }

    #[test]
    fn embedded_default_matches_file() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config/default.json");
        let from_file = TunnelConfig::load(&path).unwrap();
        let embedded = TunnelConfig::embedded_default().unwrap();
        assert_eq!(embedded.listen, from_file.listen);
        assert_eq!(embedded.allowed_commands, from_file.allowed_commands);
    }
}
