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
