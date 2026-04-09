use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::URL_SAFE;
use base64::Engine as _;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

const DEFAULT_TENANT: &str = "consumers";
const DEFAULT_SCOPES: &[&str] = &["offline_access", "openid", "profile", "User.Read"];
const GRAPH_ME_URL: &str =
    "https://graph.microsoft.com/v1.0/me?$select=id,displayName,mail,userPrincipalName";

#[derive(Debug, Clone)]
struct Config {
    command: Command,
}

#[derive(Debug, Clone)]
enum Command {
    Login(LoginArgs),
    Refresh(TokenArgs),
    WhoAmI(TokenArgs),
    PrintEnv(TokenArgs),
}

#[derive(Debug, Clone)]
struct LoginArgs {
    client_id: String,
    tenant: String,
    scopes: Vec<String>,
    label: String,
    token_file: Option<PathBuf>,
    expected_email: Option<String>,
}

#[derive(Debug, Clone)]
struct TokenArgs {
    label: Option<String>,
    token_file: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    #[serde(default)]
    interval: Option<u64>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthErrorResponse {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphMeResponse {
    id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    mail: Option<String>,
    #[serde(rename = "userPrincipalName")]
    user_principal_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AccountMetadata {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    user_principal_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedToken {
    provider: String,
    authority: String,
    tenant: String,
    client_id: String,
    label: String,
    scopes: Vec<String>,
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    expires_at: Option<f64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    saved_at: Option<f64>,
    #[serde(default)]
    account: Option<AccountMetadata>,
}

impl SavedToken {
    fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => now_epoch() >= expires_at - 300.0,
            None => false,
        }
    }

    fn display_identity(&self) -> String {
        if let Some(account) = &self.account {
            if let Some(email) = account
                .email
                .as_deref()
                .or(account.user_principal_name.as_deref())
            {
                if let Some(name) = account.display_name.as_deref() {
                    return format!("{name} <{email}>");
                }
                return email.to_string();
            }
        }
        "unknown account".to_string()
    }
}

struct MicrosoftAuthClient {
    http: Client,
    tenant: String,
    client_id: String,
    scopes: Vec<String>,
}

impl MicrosoftAuthClient {
    fn new(
        tenant: impl Into<String>,
        client_id: impl Into<String>,
        scopes: Vec<String>,
    ) -> Result<Self> {
        let http = Client::builder()
            .user_agent("op-dbus-microsoft-device-login/1.0")
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self {
            http,
            tenant: tenant.into(),
            client_id: client_id.into(),
            scopes,
        })
    }

    fn authority(&self) -> String {
        format!("https://login.microsoftonline.com/{}", self.tenant)
    }

    fn device_code_url(&self) -> String {
        format!("{}/oauth2/v2.0/devicecode", self.authority())
    }

    fn token_url(&self) -> String {
        format!("{}/oauth2/v2.0/token", self.authority())
    }

    async fn start_device_code(&self) -> Result<DeviceCodeResponse> {
        let scope_string = self.scopes.join(" ");
        let response = self
            .http
            .post(self.device_code_url())
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("scope", scope_string.as_str()),
            ])
            .send()
            .await
            .context("device code request failed")?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("device code request failed {}: {}", status, body);
        }

        serde_json::from_str(&body).context("invalid device code response")
    }

    async fn poll_for_token(&self, device_code: &str, interval_secs: u64) -> Result<TokenResponse> {
        let mut wait_secs = interval_secs.max(5);

        loop {
            sleep(Duration::from_secs(wait_secs)).await;

            let response = self
                .http
                .post(self.token_url())
                .form(&[
                    ("client_id", self.client_id.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("device_code", device_code),
                ])
                .send()
                .await
                .context("token polling request failed")?;

            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            if status.is_success() {
                return serde_json::from_str(&body).context("invalid token response");
            }

            let parsed =
                serde_json::from_str::<OAuthErrorResponse>(&body).unwrap_or(OAuthErrorResponse {
                    error: "unknown_error".to_string(),
                    error_description: Some(body.clone()),
                });

            match parsed.error.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    wait_secs += 5;
                    continue;
                }
                "authorization_declined" => bail!("sign-in was declined in the browser"),
                "bad_verification_code" => bail!("device code became invalid; start over"),
                "expired_token" => bail!("device code expired before sign-in completed"),
                _ => bail!(
                    "token request failed: {}",
                    parsed
                        .error_description
                        .unwrap_or_else(|| parsed.error.clone())
                ),
            }
        }
    }

    async fn refresh(&self, token: &SavedToken) -> Result<SavedToken> {
        let refresh_token = token
            .refresh_token
            .as_deref()
            .ok_or_else(|| anyhow!("token file does not contain a refresh token"))?;
        let scope_string = self.scopes.join(" ");

        let response = self
            .http
            .post(self.token_url())
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("scope", scope_string.as_str()),
            ])
            .send()
            .await
            .context("refresh request failed")?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("refresh request failed {}: {}", status, body);
        }

        let refreshed: TokenResponse =
            serde_json::from_str(&body).context("invalid refresh response")?;
        let mut saved = saved_from_token_response(
            &self.tenant,
            &self.client_id,
            &token.label,
            self.scopes.clone(),
            refreshed,
        );

        if saved.refresh_token.is_none() {
            saved.refresh_token = token.refresh_token.clone();
        }

        saved.account = match self.fetch_account(&saved).await {
            Ok(account) => Some(account),
            Err(_) => token
                .account
                .clone()
                .or_else(|| account_from_id_token(&saved.id_token)),
        };

        Ok(saved)
    }

    async fn fetch_account(&self, token: &SavedToken) -> Result<AccountMetadata> {
        let response = self
            .http
            .get(GRAPH_ME_URL)
            .bearer_auth(&token.access_token)
            .send()
            .await
            .context("graph profile request failed")?;

        if !response.status().is_success() {
            bail!("graph profile request failed with {}", response.status());
        }

        let profile: GraphMeResponse = response.json().await.context("invalid graph profile")?;

        Ok(AccountMetadata {
            id: profile.id,
            display_name: profile.display_name,
            email: profile.mail,
            user_principal_name: profile.user_principal_name,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = parse_args(env::args().skip(1).collect())?;

    match config.command {
        Command::Login(args) => login(args).await,
        Command::Refresh(args) => refresh(args).await,
        Command::WhoAmI(args) => whoami(args).await,
        Command::PrintEnv(args) => print_env(args),
    }
}

async fn login(args: LoginArgs) -> Result<()> {
    let scopes = dedupe_scopes(args.scopes);
    let client =
        MicrosoftAuthClient::new(args.tenant.clone(), args.client_id.clone(), scopes.clone())?;
    let device = client.start_device_code().await?;

    if let Some(message) = &device.message {
        println!("{message}");
    } else {
        println!(
            "Open {} and enter code {}",
            device.verification_uri, device.user_code
        );
    }
    if let Some(url) = &device.verification_uri_complete {
        println!("Direct verification URL: {url}");
    }
    println!("Code expires in {} seconds.", device.expires_in);

    let token = client
        .poll_for_token(&device.device_code, device.interval.unwrap_or(5))
        .await?;

    let mut saved =
        saved_from_token_response(&args.tenant, &args.client_id, &args.label, scopes, token);

    saved.account = match client.fetch_account(&saved).await {
        Ok(account) => Some(account),
        Err(_) => account_from_id_token(&saved.id_token),
    };

    if let Some(expected_email) = args.expected_email.as_deref() {
        if !account_matches_expected(saved.account.as_ref(), expected_email) {
            bail!(
                "signed-in account does not match expected email {}\nresolved identity: {}",
                expected_email,
                saved.display_identity()
            );
        }
    }

    let token_path = resolve_token_path(args.token_file.as_deref(), Some(&args.label))?;
    write_saved_token(&token_path, &saved).await?;

    println!("Saved token to {}", token_path.display());
    println!("Resolved identity: {}", saved.display_identity());
    println!(
        "Export for agents: export MICROSOFT_AUTH_TOKEN_FILE={}",
        shell_escape_path(&token_path)
    );

    Ok(())
}

async fn refresh(args: TokenArgs) -> Result<()> {
    let token_path = resolve_token_path(args.token_file.as_deref(), args.label.as_deref())?;
    let token = read_saved_token(&token_path).await?;
    let client = MicrosoftAuthClient::new(
        token.tenant.clone(),
        token.client_id.clone(),
        token.scopes.clone(),
    )?;
    let refreshed = client.refresh(&token).await?;
    write_saved_token(&token_path, &refreshed).await?;

    println!("Refreshed {}", token_path.display());
    println!("Resolved identity: {}", refreshed.display_identity());
    Ok(())
}

async fn whoami(args: TokenArgs) -> Result<()> {
    let token_path = resolve_token_path(args.token_file.as_deref(), args.label.as_deref())?;
    let mut token = read_saved_token(&token_path).await?;
    if token.is_expired() && token.refresh_token.is_some() {
        let client = MicrosoftAuthClient::new(
            token.tenant.clone(),
            token.client_id.clone(),
            token.scopes.clone(),
        )?;
        token = client.refresh(&token).await?;
        write_saved_token(&token_path, &token).await?;
    }

    println!("Token file: {}", token_path.display());
    println!("Identity: {}", token.display_identity());
    println!("Tenant: {}", token.tenant);
    println!("Scopes: {}", token.scopes.join(" "));
    if let Some(expires_at) = token.expires_at {
        println!("Expires at: {}", expires_at);
    }

    Ok(())
}

fn print_env(args: TokenArgs) -> Result<()> {
    let token_path = resolve_token_path(args.token_file.as_deref(), args.label.as_deref())?;
    println!(
        "export MICROSOFT_AUTH_TOKEN_FILE={}",
        shell_escape_path(&token_path)
    );
    Ok(())
}

fn saved_from_token_response(
    tenant: &str,
    client_id: &str,
    label: &str,
    scopes: Vec<String>,
    token: TokenResponse,
) -> SavedToken {
    let saved_at = now_epoch();
    let expires_at = token
        .expires_in
        .map(|expires_in| saved_at + expires_in as f64);

    SavedToken {
        provider: "microsoft".to_string(),
        authority: format!("https://login.microsoftonline.com/{tenant}"),
        tenant: tenant.to_string(),
        client_id: client_id.to_string(),
        label: label.to_string(),
        scopes,
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        token_type: token.token_type,
        expires_in: token.expires_in,
        expires_at,
        scope: token.scope,
        id_token: token.id_token,
        saved_at: Some(saved_at),
        account: None,
    }
}

fn account_from_id_token(id_token: &Option<String>) -> Option<AccountMetadata> {
    let token = id_token.as_ref()?;
    let payload = decode_jwt_payload(token).ok()?;

    Some(AccountMetadata {
        id: payload
            .get("oid")
            .and_then(Value::as_str)
            .map(str::to_string),
        display_name: payload
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string),
        email: payload
            .get("email")
            .and_then(Value::as_str)
            .map(str::to_string),
        user_principal_name: payload
            .get("preferred_username")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn decode_jwt_payload(token: &str) -> Result<Value> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| anyhow!("JWT payload missing"))?;
    let decoded = decode_base64url(payload)?;
    serde_json::from_slice(&decoded).context("invalid JWT payload JSON")
}

fn decode_base64url(input: &str) -> Result<Vec<u8>> {
    let mut padded = input.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }

    URL_SAFE
        .decode(padded.as_bytes())
        .context("invalid base64url payload")
}

fn account_matches_expected(account: Option<&AccountMetadata>, expected_email: &str) -> bool {
    let expected = expected_email.trim().to_ascii_lowercase();
    account
        .map(|account| {
            [
                account.email.as_deref(),
                account.user_principal_name.as_deref(),
            ]
            .into_iter()
            .flatten()
            .any(|candidate| candidate.trim().eq_ignore_ascii_case(&expected))
        })
        .unwrap_or(false)
}

async fn read_saved_token(path: &Path) -> Result<SavedToken> {
    let contents = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("invalid token file {}", path.display()))
}

async fn write_saved_token(path: &Path, token: &SavedToken) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let contents = serde_json::to_string_pretty(token).context("failed to serialize token")?;
    tokio::fs::write(path, contents)
        .await
        .with_context(|| format!("failed to write {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to secure {}", path.display()))?;
    }

    Ok(())
}

fn resolve_token_path(explicit: Option<&Path>, label: Option<&str>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }

    let label = label.unwrap_or("default");
    let sanitized = sanitize_label(label);
    let config_home = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| anyhow!("HOME or XDG_CONFIG_HOME must be set"))?;

    Ok(config_home
        .join("op-dbus")
        .join("microsoft")
        .join(format!("{sanitized}.json")))
}

fn sanitize_label(label: &str) -> String {
    let cleaned: String = label
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect();

    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "default".to_string()
    } else {
        trimmed.to_string()
    }
}

fn shell_escape_path(path: &Path) -> String {
    let text = path.display().to_string();
    if !text.chars().any(|ch| matches!(ch, ' ' | '\t' | '\'' | '"')) {
        return text;
    }

    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

fn now_epoch() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn dedupe_scopes(scopes: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    for scope in scopes {
        if !result.iter().any(|existing| existing == &scope) {
            result.push(scope);
        }
    }
    result
}

fn parse_args(args: Vec<String>) -> Result<Config> {
    if args.is_empty() {
        print_usage();
        bail!("missing command");
    }

    let command = args[0].as_str();
    let rest = &args[1..];

    let parsed = match command {
        "login" => Command::Login(parse_login_args(rest)?),
        "refresh" => Command::Refresh(parse_token_args(rest)?),
        "whoami" => Command::WhoAmI(parse_token_args(rest)?),
        "print-env" => Command::PrintEnv(parse_token_args(rest)?),
        "--help" | "-h" | "help" => {
            print_usage();
            std::process::exit(0);
        }
        other => bail!("unknown command: {other}"),
    };

    Ok(Config { command: parsed })
}

fn parse_login_args(args: &[String]) -> Result<LoginArgs> {
    let mut client_id = env::var("MICROSOFT_CLIENT_ID").ok();
    let mut tenant = env::var("MICROSOFT_TENANT").unwrap_or_else(|_| DEFAULT_TENANT.to_string());
    let mut scopes: Vec<String> = DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect();
    let mut label = "default".to_string();
    let mut token_file = None;
    let mut expected_email = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--client-id" => {
                i += 1;
                client_id = Some(next_arg(args, i, "--client-id")?.to_string());
            }
            "--tenant" => {
                i += 1;
                tenant = next_arg(args, i, "--tenant")?.to_string();
            }
            "--scope" => {
                i += 1;
                let value = next_arg(args, i, "--scope")?;
                if scopes
                    == DEFAULT_SCOPES
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                {
                    scopes.clear();
                }
                scopes.push(value.to_string());
            }
            "--label" => {
                i += 1;
                label = next_arg(args, i, "--label")?.to_string();
            }
            "--token-file" => {
                i += 1;
                token_file = Some(PathBuf::from(next_arg(args, i, "--token-file")?));
            }
            "--expected-email" => {
                i += 1;
                expected_email = Some(next_arg(args, i, "--expected-email")?.to_string());
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => bail!("unknown login argument: {other}"),
        }
        i += 1;
    }

    let client_id = client_id.ok_or_else(|| {
        anyhow!(
            "MICROSOFT_CLIENT_ID is required. Pass --client-id or set the environment variable."
        )
    })?;

    Ok(LoginArgs {
        client_id,
        tenant,
        scopes: dedupe_scopes(scopes),
        label,
        token_file,
        expected_email,
    })
}

fn parse_token_args(args: &[String]) -> Result<TokenArgs> {
    let mut label = None;
    let mut token_file = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--label" => {
                i += 1;
                label = Some(next_arg(args, i, "--label")?.to_string());
            }
            "--token-file" => {
                i += 1;
                token_file = Some(PathBuf::from(next_arg(args, i, "--token-file")?));
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other}"),
        }
        i += 1;
    }

    Ok(TokenArgs { label, token_file })
}

fn next_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str> {
    args.get(index)
        .map(|value| value.as_str())
        .ok_or_else(|| anyhow!("{flag} requires a value"))
}

fn print_usage() {
    eprintln!(
        "Usage:
  microsoft-device-login login --client-id <APP_ID> [--tenant consumers] [--label NAME] [--expected-email EMAIL] [--scope SCOPE ...]
  microsoft-device-login refresh (--label NAME | --token-file PATH)
  microsoft-device-login whoami (--label NAME | --token-file PATH)
  microsoft-device-login print-env (--label NAME | --token-file PATH)

Notes:
  - Use a public client app registration that supports personal Microsoft accounts.
  - Default scopes: offline_access openid profile User.Read
  - Token files default to ~/.config/op-dbus/microsoft/<label>.json"
    );
}
