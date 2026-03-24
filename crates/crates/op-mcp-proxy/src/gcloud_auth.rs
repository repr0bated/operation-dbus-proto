//! Google Cloud authentication for cloudcode-pa.googleapis.com.
//!
//! Supports multiple token sources:
//! 1. Cached token file (WG/MCP-proxy session context)
//! 2. VSCode/Antigravity extension auth cache
//! 3. gcloud CLI
//! 4. Application Default Credentials

use std::path::PathBuf;
use std::process::Command;

use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use tracing::{debug, info, warn};

const OAUTH_SCOPES_PREFERRED: &[&str] = &[
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

#[derive(Clone)]
pub struct GCloudAuth {
    /// Path to cached token file from local session context
    token_file_path: Option<PathBuf>,
}

#[derive(Debug)]
struct ExtensionAuthPaths {
    credentials: PathBuf,
    adc: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ExtensionCredentialsNested {
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExtensionCredentials {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "accessTokenExpirySecond")]
    access_token_expiry: Option<i64>,
    #[serde(rename = "refreshToken")]
    refresh_token: Option<String>,
    credentials: Option<ExtensionCredentialsNested>,
}

#[derive(Debug, Deserialize)]
struct ExtensionAdc {
    client_id: Option<String>,
    client_secret: Option<String>,
    refresh_token: Option<String>,
}

impl GCloudAuth {
    pub fn new() -> Self {
        // 1) Explicit file path override
        let explicit = std::env::var("MCP_PROXY_TOKEN_FILE")
            .ok()
            .map(PathBuf::from)
            .filter(|p| p.exists());

        // 2) Preferred local token locations
        let discovered = dirs::home_dir().and_then(|home| {
            let candidates = [
                home.join(".config").join("op-mcp-proxy"),
                home.join(".op-mcp-proxy"),
                home.join(".antigravity-server"), // backward-compat
            ];
            candidates.into_iter().find_map(find_token_file_in_dir)
        });

        let token_file_path = explicit.or(discovered);

        if let Some(ref path) = token_file_path {
            debug!("Found cached token file at: {:?}", path);
        }

        Self { token_file_path }
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

        // 2. Cached token file
        if let Some(token) = self.try_cached_token_file().await {
            info!("Using token from cached token file");
            // These tokens are typically valid for 1 hour
            return Ok((token, Utc::now() + Duration::minutes(55)));
        }

        // 3. VSCode/Antigravity extension auth cache
        if let Some((token, expires)) = self.try_extension_auth_cache().await {
            info!("Using token from VSCode extension auth cache");
            return Ok((token, expires));
        }

        // 4. gcloud CLI
        if let Some((token, expires)) = self.try_gcloud_cli().await {
            info!("Using token from gcloud CLI");
            return Ok((token, expires));
        }

        // 5. Application Default Credentials via gcloud (opt-in).
        if adc_fallback_enabled() {
            if let Some((token, expires)) = self.try_adc().await {
                info!("Using Application Default Credentials");
                return Ok((token, expires));
            }
        } else {
            debug!("ADC fallback disabled (set OP_ENABLE_ADC_FALLBACK=1 to enable)");
        }

        anyhow::bail!(
            "Could not obtain OAuth token from GCLOUD_TOKEN, cached token file, extension cache, or gcloud CLI credentials"
        )
    }

    async fn try_cached_token_file(&self) -> Option<String> {
        let path = self.token_file_path.as_ref()?;

        let content = std::fs::read_to_string(path).ok()?;
        let token = content.trim().to_string();

        if token.is_empty() {
            return None;
        }

        // Basic validation - OAuth tokens start with "ya29."
        if token.starts_with("ya29.") {
            Some(token)
        } else {
            warn!("Cached token does not look like an OAuth token");
            None
        }
    }

    async fn try_gcloud_cli(&self) -> Option<(String, DateTime<Utc>)> {
        if let Some(token) =
            run_gcloud_access_token(&["auth", "print-access-token"], OAUTH_SCOPES_PREFERRED)
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
            OAUTH_SCOPES_PREFERRED,
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
    #[allow(dead_code)]
    pub async fn refresh_token(&self) -> anyhow::Result<(String, DateTime<Utc>)> {
        if let Some(token) =
            run_gcloud_access_token(&["auth", "print-access-token"], OAUTH_SCOPES_PREFERRED)
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
}

impl Default for GCloudAuth {
    fn default() -> Self {
        Self::new()
    }
}

fn find_token_file_in_dir(dir: PathBuf) -> Option<PathBuf> {
    std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|ext| ext == "token").unwrap_or(false))
}

impl GCloudAuth {
    async fn try_extension_auth_cache(&self) -> Option<(String, DateTime<Utc>)> {
        match self.try_extension_auth_cache_inner().await {
            Ok(result) => Some(result),
            Err(e) => {
                warn!("Extension auth cache unusable: {}", e);
                None
            }
        }
    }

    async fn try_extension_auth_cache_inner(&self) -> anyhow::Result<(String, DateTime<Utc>)> {
        let paths = extension_auth_paths()
            .ok_or_else(|| anyhow::anyhow!("extension auth directory not found"))?;
        let credentials_text = std::fs::read_to_string(&paths.credentials)
            .map_err(|e| anyhow::anyhow!("cannot read credentials.json: {}", e))?;
        let credentials: ExtensionCredentials = serde_json::from_str(&credentials_text)
            .map_err(|e| anyhow::anyhow!("cannot parse credentials.json: {}", e))?;

        // Prefer live access token if it is still valid.
        if let (Some(token), Some(raw_expiry)) = (
            credentials.access_token.clone(),
            credentials.access_token_expiry,
        ) {
            if let Some(expiry) = parse_expiry_epoch(raw_expiry) {
                if expiry > Utc::now() + Duration::minutes(5) && token.starts_with("ya29.") {
                    return Ok((token, expiry));
                }
                debug!("Extension cached token expired or expiring soon");
            }
        }

        // Otherwise refresh from the extension's authorized_user cache.
        self.refresh_extension_token_from_paths(&paths, &credentials)
            .await
    }

    /// Refresh the extension OAuth token using cached credentials.
    /// Public so DirectLLM can call it for background auto-refresh.
    pub async fn refresh_extension_token(&self) -> anyhow::Result<(String, DateTime<Utc>)> {
        let paths = extension_auth_paths()
            .ok_or_else(|| anyhow::anyhow!("extension auth directory not found"))?;
        let credentials_text = std::fs::read_to_string(&paths.credentials)
            .map_err(|e| anyhow::anyhow!("cannot read credentials.json: {}", e))?;
        let credentials: ExtensionCredentials = serde_json::from_str(&credentials_text)
            .map_err(|e| anyhow::anyhow!("cannot parse credentials.json: {}", e))?;
        self.refresh_extension_token_from_paths(&paths, &credentials)
            .await
    }

    async fn refresh_extension_token_from_paths(
        &self,
        paths: &ExtensionAuthPaths,
        credentials: &ExtensionCredentials,
    ) -> anyhow::Result<(String, DateTime<Utc>)> {
        let adc_text = std::fs::read_to_string(&paths.adc)
            .map_err(|e| anyhow::anyhow!("cannot read extension ADC: {}", e))?;
        let adc: ExtensionAdc = serde_json::from_str(&adc_text)
            .map_err(|e| anyhow::anyhow!("cannot parse extension ADC: {}", e))?;

        let refresh_token = credentials
            .refresh_token
            .clone()
            .or_else(|| {
                credentials
                    .credentials
                    .as_ref()
                    .and_then(|nested| nested.refresh_token.clone())
            })
            .or(adc.refresh_token)
            .ok_or_else(|| anyhow::anyhow!("no refresh_token in extension credentials or ADC"))?;
        let client_id = adc
            .client_id
            .ok_or_else(|| anyhow::anyhow!("missing client_id in extension ADC"))?;
        let client_secret = adc
            .client_secret
            .ok_or_else(|| anyhow::anyhow!("missing client_secret in extension ADC"))?;

        refresh_extension_access_token(&refresh_token, &client_id, &client_secret).await
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

fn extension_auth_paths() -> Option<ExtensionAuthPaths> {
    let auth_dir = if let Ok(dir) = std::env::var("MCP_PROXY_VSCODE_AUTH_DIR") {
        PathBuf::from(dir)
    } else {
        dirs::home_dir()?
            .join(".cache")
            .join("google-vscode-extension")
            .join("auth")
    };

    let credentials = auth_dir.join("credentials.json");
    let adc = auth_dir.join("application_default_credentials.json");
    if credentials.exists() && adc.exists() {
        Some(ExtensionAuthPaths { credentials, adc })
    } else {
        None
    }
}

fn parse_expiry_epoch(raw: i64) -> Option<DateTime<Utc>> {
    let seconds = if raw > 10_000_000_000 {
        raw / 1000
    } else {
        raw
    };
    DateTime::<Utc>::from_timestamp(seconds, 0)
}

async fn refresh_extension_access_token(
    refresh_token: &str,
    client_id: &str,
    client_secret: &str,
) -> anyhow::Result<(String, DateTime<Utc>)> {
    let resp = reqwest::Client::new()
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("extension token refresh request failed: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "extension token refresh failed with status {}: {}",
            status,
            body
        );
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("cannot parse token refresh response: {}", e))?;
    let access_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing access_token in refresh response"))?
        .to_string();
    let expires_in = body
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);
    let expiry = Utc::now() + Duration::seconds(expires_in.max(60) - 30);
    info!(
        "Extension token refreshed successfully, expires in {}s",
        expires_in
    );
    Ok((access_token, expiry))
}
