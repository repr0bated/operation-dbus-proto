//! Code Assist client – uses cloudcode-pa.googleapis.com with
//! project/auth settings aligned to the VSCode extension flow.

use anyhow::Context;
use reqwest::{header, Client};
use serde_json::Value as JsonValue;
use simd_json::prelude::*;
use simd_json::OwnedValue;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

const CODE_ASSIST_ENDPOINT: &str = "https://cloudcode-pa.googleapis.com";
const CODE_ASSIST_DAILY_ENDPOINT: &str = "https://daily-cloudcode-pa.googleapis.com";
const CODE_ASSIST_API_VERSION: &str = "v1internal";
const DEFAULT_MODEL: &str = "gemini-2.5-flash";
const DEFAULT_USER_AGENT: &str =
    "google-cloud-code-vscode/1.22.0 (GPN:Cloud Code for VS Code) vscode/1.85.0 (linux; x64)";
const DEFAULT_X_GOOG_API_CLIENT: &str = "gl-rust/1.76.0 gax/2.12.0 gapic/1.0.0";
const DEFAULT_ORIGIN: &str = "vscode://googlecloudtools.cloudcode";
const DEFAULT_REFERER: &str = "vscode://googlecloudtools.cloudcode";
const DEFAULT_X_CLIENT_DATA: &str =
    "eyJpc0lkZSI6dHJ1ZSwiaWRlVHlwZSI6InZzY29kZSIsImlkZVZlcnNpb24iOiIxLjg1LjAiLCJwbHVnaW5WZXJzaW9uIjoiMS4yMi4wIn0=";

#[derive(Debug, Clone)]
struct IdeEmulationHeaders {
    user_agent: String,
    x_goog_api_client: String,
    origin: String,
    referer: String,
    x_client_data: String,
}

pub struct CloudAICompanion {
    cli: Client,
    project: String,
    quota_project: String,
    headers: IdeEmulationHeaders,
    send_user_project_header: bool,
    resolved_project: Mutex<Option<String>>,
}

impl CloudAICompanion {
    pub fn new() -> Self {
        let antigravity_project = read_antigravity_project();
        let extension_quota_project = read_extension_adc_quota_project();
        let gcloud_adc_quota_project = read_gcloud_adc_quota_project();

        // Only MCP_PROXY_* vars are treated as explicit hard overrides.
        let explicit_quota_project = std::env::var("MCP_PROXY_QUOTA_PROJECT")
            .ok()
            .filter(|v| !v.trim().is_empty());
        let ambient_quota_project = std::env::var("GOOGLE_CLOUD_QUOTA_PROJECT")
            .or_else(|_| std::env::var("QUOTA_PROJECT"))
            .ok()
            .filter(|v| !v.trim().is_empty());
        let quota_project = explicit_quota_project
            .or(extension_quota_project.clone())
            .or(gcloud_adc_quota_project.clone())
            .or(antigravity_project.clone())
            .or(ambient_quota_project)
            .unwrap_or_default();

        let explicit_project = std::env::var("MCP_PROXY_GCLOUD_PROJECT")
            .ok()
            .filter(|v| !v.trim().is_empty());
        let ambient_project = std::env::var("GCLOUD_PROJECT")
            .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT"))
            .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT_ID"))
            .ok()
            .filter(|v| !v.trim().is_empty());
        let project = explicit_project
            .or(antigravity_project)
            .or(ambient_project)
            .or_else(|| {
                if quota_project.is_empty() {
                    None
                } else {
                    Some(quota_project.clone())
                }
            })
            .unwrap_or_default();

        let headers = IdeEmulationHeaders {
            user_agent: std::env::var("MCP_PROXY_USER_AGENT")
                .or_else(|_| std::env::var("USER_AGENT"))
                .unwrap_or_else(|_| DEFAULT_USER_AGENT.to_string()),
            x_goog_api_client: std::env::var("MCP_PROXY_X_GOOG_API_CLIENT")
                .or_else(|_| std::env::var("X_GOOG_API_CLIENT"))
                .unwrap_or_else(|_| DEFAULT_X_GOOG_API_CLIENT.to_string()),
            origin: std::env::var("MCP_PROXY_ORIGIN")
                .unwrap_or_else(|_| DEFAULT_ORIGIN.to_string()),
            referer: std::env::var("MCP_PROXY_REFERER")
                .unwrap_or_else(|_| DEFAULT_REFERER.to_string()),
            x_client_data: std::env::var("MCP_PROXY_X_CLIENT_DATA")
                .unwrap_or_else(|_| DEFAULT_X_CLIENT_DATA.to_string()),
        };
        // Extension requests do not include x-goog-user-project by default.
        // Sending it can force SERVICE_DISABLED checks on cloudcode-pa.
        let send_user_project_header = env_flag("MCP_PROXY_SEND_X_GOOG_USER_PROJECT", false);

        if project.is_empty() {
            warn!(
                "No Code Assist project configured; set GOOGLE_CLOUD_PROJECT or geminicodeassist.project in Antigravity settings"
            );
        }
        info!("Code Assist project: {}", project);
        if quota_project.is_empty() {
            info!("Code Assist quota project: <unset>");
        } else {
            info!("Code Assist quota project: {}", quota_project);
        }
        info!(
            "MCP bridge IDE emulation enabled (user-agent: {})",
            headers.user_agent
        );

        Self {
            cli: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("http client"),
            project,
            quota_project,
            headers,
            send_user_project_header,
            resolved_project: Mutex::new(None),
        }
    }

    /// Generate text using the Code Assist endpoint (cloudcode-pa.googleapis.com).
    pub async fn generate(
        &self,
        prompt: &str,
        token: &str,
        model: Option<&str>,
    ) -> anyhow::Result<String> {
        let env_model = std::env::var("MODEL_ID").ok();
        let model = model.or(env_model.as_deref()).unwrap_or(DEFAULT_MODEL);
        // First pass: normal prompt.
        let first = self.send_generate_request(token, model, prompt).await?;
        let first_inner = first
            .get("response")
            .context("missing 'response' in code-assist reply")?;
        if let Some(reason) = first_inner
            .get("promptFeedback")
            .and_then(|pf| pf.get("blockReason"))
            .and_then(|r| r.as_str())
        {
            anyhow::bail!("content blocked: {}", reason);
        }

        let first_text = extract_candidate_text(first_inner);
        if !first_text.is_empty() {
            return Ok(first_text);
        }

        let finish_reason = first_inner
            .get("candidates")
            .and_then(|c| c.get_idx(0))
            .and_then(|c| c.get("finishReason"))
            .and_then(|r| r.as_str())
            .unwrap_or("unknown");

        // Some preview responses return MALFORMED_FUNCTION_CALL with empty text
        // despite functionCalling mode=NONE. Retry once with a stricter prompt.
        if finish_reason == "MALFORMED_FUNCTION_CALL" || finish_reason == "UNEXPECTED_TOOL_CALL" {
            let strict_prompt = format!(
                "Return plain text only. Do not call any functions or tools.\n\n{}",
                prompt
            );
            let second = self
                .send_generate_request(token, model, &strict_prompt)
                .await?;
            let second_inner = second
                .get("response")
                .context("missing 'response' in code-assist retry reply")?;
            let second_text = extract_candidate_text(second_inner);
            if !second_text.is_empty() {
                warn!("Recovered from MALFORMED_FUNCTION_CALL via strict plain-text retry");
                return Ok(second_text);
            }

            let second_finish_reason = second_inner
                .get("candidates")
                .and_then(|c| c.get_idx(0))
                .and_then(|c| c.get("finishReason"))
                .and_then(|r| r.as_str())
                .unwrap_or("unknown");

            anyhow::bail!(
                "empty response text from code-assist after retry (finish_reason={})",
                second_finish_reason
            );
        }

        anyhow::bail!(
            "empty response text from code-assist (finish_reason={})",
            finish_reason
        );
    }
}

fn extract_candidate_text(inner: &OwnedValue) -> String {
    inner
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|candidates| {
            candidates.iter().find_map(|candidate| {
                candidate
                    .get("content")
                    .and_then(|content| content.get("parts"))
                    .and_then(|parts| parts.as_array())
                    .map(|parts| {
                        parts
                            .iter()
                            .filter_map(|part| part.get("text").and_then(|t| t.as_str()))
                            .collect::<String>()
                    })
                    .filter(|txt| !txt.is_empty())
            })
        })
        .unwrap_or_default()
}

impl CloudAICompanion {
    async fn send_generate_request(
        &self,
        token: &str,
        model: &str,
        prompt: &str,
    ) -> anyhow::Result<OwnedValue> {
        let endpoint = self.code_assist_endpoint();
        let url = format!("{}/{}:generateContent", endpoint, CODE_ASSIST_API_VERSION);
        let request_project = self.resolve_request_project(token).await?;

        let body = serde_json::json!({
            "model": model,
            "project": request_project,
            "user_prompt_id": uuid::Uuid::new_v4().to_string(),
            "request": {
                "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
                "generationConfig": {
                    "temperature": 0.7,
                    "maxOutputTokens": 8192,
                    "topP": 0.95,
                    "topK": 40,
                    "responseMimeType": "text/plain"
                },
                "toolConfig": {
                    "functionCallingConfig": {
                        "mode": "NONE"
                    }
                },
                "session_id": ""
            }
        });

        debug!("POST {} model={}", url, model);

        let mut request = self
            .cli
            .post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::USER_AGENT, &self.headers.user_agent)
            .header("x-goog-api-client", &self.headers.x_goog_api_client)
            .header("x-client-data", &self.headers.x_client_data)
            .header(header::ORIGIN, &self.headers.origin)
            .header(header::REFERER, &self.headers.referer)
            .body(body.to_string());

        if self.send_user_project_header && !request_project.is_empty() {
            request = request.header("x-goog-user-project", &request_project);
        }

        let resp = request.send().await?;
        if !resp.status().is_success() {
            anyhow::bail!(
                "code-assist error {}: {}",
                resp.status(),
                resp.text().await?
            );
        }

        let mut resp_bytes = resp.bytes().await?.to_vec();
        let json: OwnedValue = simd_json::from_slice(&mut resp_bytes)
            .context("failed to parse code-assist response")?;
        Ok(json)
    }
}

impl CloudAICompanion {
    fn code_assist_endpoint(&self) -> String {
        std::env::var("CODE_ASSIST_ENDPOINT")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| {
                if env_flag("MCP_PROXY_USE_DAILY_ENDPOINT", true) {
                    CODE_ASSIST_DAILY_ENDPOINT.to_string()
                } else {
                    CODE_ASSIST_ENDPOINT.to_string()
                }
            })
    }

    fn base_request_project(&self) -> Option<String> {
        let project = if self.quota_project.trim().is_empty() {
            self.project.trim()
        } else {
            self.quota_project.trim()
        };
        if project.is_empty() {
            None
        } else {
            Some(project.to_string())
        }
    }

    async fn resolve_request_project(&self, token: &str) -> anyhow::Result<String> {
        if let Some(project) = self.resolved_project.lock().await.clone() {
            return Ok(project);
        }

        let base_project = self.base_request_project();
        if !env_flag("MCP_PROXY_EXTENSION_ROUTING", true) {
            return base_project.context(
                "missing project for Code Assist; set MCP_PROXY_GCLOUD_PROJECT/GOOGLE_CLOUD_PROJECT or geminicodeassist.project in Antigravity",
            );
        }

        let resolved = self
            .resolve_project_via_extension_flow(token, base_project.as_deref())
            .await
            .or_else(|e| {
                // Keep bridge usable even if extension bootstrap fails.
                if let Some(project) = base_project.clone() {
                    warn!(
                        "Extension bootstrap failed ({}); falling back to configured project {}",
                        e, project
                    );
                    Ok(project)
                } else {
                    Err(e)
                }
            })?;

        *self.resolved_project.lock().await = Some(resolved.clone());
        Ok(resolved)
    }

    async fn resolve_project_via_extension_flow(
        &self,
        token: &str,
        configured_project: Option<&str>,
    ) -> anyhow::Result<String> {
        let metadata = self.build_metadata(configured_project);
        let mut load_req = serde_json::json!({ "metadata": metadata });
        if let Some(project) = configured_project.filter(|p| !p.trim().is_empty()) {
            load_req["cloudaicompanionProject"] = serde_json::json!(project);
        }

        let load = self
            .request_setup_post(token, "loadCodeAssist", &load_req)
            .await
            .context("loadCodeAssist request failed")?;

        if let Some(project) =
            extract_project_id(load.get("cloudaicompanionProject").or_else(|| {
                load.get("response")
                    .and_then(|v| v.get("cloudaicompanionProject"))
            }))
        {
            if let Some(tier_id) = load
                .get("currentTier")
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str())
            {
                info!(
                    "Code Assist bootstrap resolved tier={} project={}",
                    tier_id, project
                );
            }
            return Ok(project.to_string());
        }

        if load.get("currentTier").is_some() {
            if let Some(project) = configured_project.filter(|p| !p.trim().is_empty()) {
                return Ok(project.to_string());
            }
            anyhow::bail!(
                "This account requires setting GOOGLE_CLOUD_PROJECT/GOOGLE_CLOUD_PROJECT_ID (workspace-gca)"
            );
        }

        let tier_id = load
            .get("allowedTiers")
            .and_then(|v| v.as_array())
            .and_then(|tiers| {
                tiers.iter().find_map(|tier| {
                    if tier
                        .get("isDefault")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        tier.get("id")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| "legacy-tier".to_string());

        let mut onboard_req = serde_json::json!({
            "tierId": tier_id,
            "metadata": self.build_metadata(configured_project),
        });
        if let Some(project) = configured_project.filter(|p| !p.trim().is_empty()) {
            onboard_req["cloudaicompanionProject"] = serde_json::json!(project);
        }

        let mut op = self
            .request_setup_post(token, "onboardUser", &onboard_req)
            .await
            .context("onboardUser request failed")?;

        let mut polls = 0usize;
        while !op.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
            let name = op
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .context("onboardUser returned incomplete operation without name")?;
            tokio::time::sleep(Duration::from_millis(1200)).await;
            op = self
                .request_setup_get(token, name)
                .await
                .with_context(|| format!("failed polling operation {}", name))?;
            polls += 1;
            if polls > 20 {
                anyhow::bail!("onboardUser operation polling timed out");
            }
        }

        if let Some(project) = extract_project_id(
            op.get("response")
                .and_then(|v| v.get("cloudaicompanionProject")),
        ) {
            return Ok(project.to_string());
        }

        if let Some(project) = configured_project.filter(|p| !p.trim().is_empty()) {
            return Ok(project.to_string());
        }

        anyhow::bail!(
            "onboardUser did not return cloudaicompanionProject and no configured project is available"
        )
    }

    fn build_metadata(&self, project: Option<&str>) -> JsonValue {
        let mut metadata = serde_json::json!({
            "ideType": "IDE_UNSPECIFIED",
            "platform": "PLATFORM_UNSPECIFIED",
            "pluginType": "GEMINI",
        });
        if let Some(p) = project.filter(|p| !p.trim().is_empty()) {
            metadata["duetProject"] = serde_json::json!(p);
        }
        metadata
    }

    async fn request_setup_post(
        &self,
        token: &str,
        method: &str,
        body: &JsonValue,
    ) -> anyhow::Result<JsonValue> {
        let url = format!(
            "{}/{}:{}",
            self.code_assist_endpoint(),
            CODE_ASSIST_API_VERSION,
            method
        );
        let resp = self
            .cli
            .post(url)
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::USER_AGENT, &self.headers.user_agent)
            .header("x-goog-api-client", &self.headers.x_goog_api_client)
            .header("x-client-data", &self.headers.x_client_data)
            .header(header::ORIGIN, &self.headers.origin)
            .header(header::REFERER, &self.headers.referer)
            .json(body)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            anyhow::bail!("code-assist {} error {}: {}", method, status, text);
        }

        let payload: JsonValue =
            serde_json::from_str(&text).context("failed to parse setup JSON response")?;
        if payload.get("error").is_some() {
            anyhow::bail!("code-assist {} API error: {}", method, payload);
        }
        Ok(payload)
    }

    async fn request_setup_get(
        &self,
        token: &str,
        operation_name: &str,
    ) -> anyhow::Result<JsonValue> {
        let op = operation_name.trim_start_matches('/');
        let url = format!(
            "{}/{}/{}",
            self.code_assist_endpoint(),
            CODE_ASSIST_API_VERSION,
            op
        );
        let resp = self
            .cli
            .get(url)
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::USER_AGENT, &self.headers.user_agent)
            .header("x-goog-api-client", &self.headers.x_goog_api_client)
            .header("x-client-data", &self.headers.x_client_data)
            .header(header::ORIGIN, &self.headers.origin)
            .header(header::REFERER, &self.headers.referer)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            anyhow::bail!("code-assist operation get error {}: {}", status, text);
        }
        let payload: JsonValue =
            serde_json::from_str(&text).context("failed to parse operation JSON response")?;
        if payload.get("error").is_some() {
            anyhow::bail!("code-assist operation API error: {}", payload);
        }
        Ok(payload)
    }
}

fn extract_project_id(value: Option<&JsonValue>) -> Option<String> {
    let raw = value?;
    if let Some(project) = raw.as_str().map(str::trim).filter(|v| !v.is_empty()) {
        return Some(project.to_string());
    }
    raw.get("id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// Read the Gemini CLI access token from ~/.gemini/oauth_creds.json.
/// Returns (access_token, expiry_epoch_ms).
pub fn read_gemini_cli_token() -> anyhow::Result<(String, i64)> {
    let path = gemini_creds_path().context("cannot locate ~/.gemini/oauth_creds.json")?;
    let mut text = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    let creds: OwnedValue = unsafe { simd_json::from_str(&mut text) }
        .with_context(|| format!("cannot parse {}", path.display()))?;

    let token = creds
        .get("access_token")
        .and_then(|v| v.as_str())
        .context("missing access_token in gemini oauth_creds")?
        .to_string();
    let expiry = creds
        .get("expiry_date")
        .and_then(|v| v.as_f64())
        .map(|v| v as i64)
        .unwrap_or(0);

    Ok((token, expiry))
}

/// Refresh the Gemini CLI token using its refresh_token and client credentials.
pub async fn refresh_gemini_cli_token() -> anyhow::Result<String> {
    let path = gemini_creds_path().context("cannot locate ~/.gemini/oauth_creds.json")?;
    let mut text = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    let creds: OwnedValue = unsafe { simd_json::from_str(&mut text) }
        .with_context(|| format!("cannot parse {}", path.display()))?;

    let refresh_token = creds
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .context("missing refresh_token")?;

    let (client_id, client_secret) = read_env_oauth_client()
        .or_else(read_adc_oauth_client)
        .context(
            "missing OAuth client credentials; set GEMINI_OAUTH_CLIENT_ID and \
             GEMINI_OAUTH_CLIENT_SECRET or configure local OAuth creds in ~/.config/gcloud/application_default_credentials.json",
        )?;

    let cli = Client::new();
    let resp = cli
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "token refresh failed {}: {}",
            resp.status(),
            resp.text().await?
        );
    }

    let mut resp_bytes = resp.bytes().await?.to_vec();
    let body: OwnedValue =
        simd_json::from_slice(&mut resp_bytes).context("cannot parse token refresh response")?;

    let new_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .context("missing access_token in refresh response")?
        .to_string();
    let expires_in = body
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .unwrap_or(3600);

    // Update the cached credentials file
    let new_expiry = chrono::Utc::now().timestamp_millis() + (expires_in as i64 * 1000);
    let updated = serde_json::json!({
        "access_token": new_token,
        "scope": creds.get("scope").and_then(|v| v.as_str()).unwrap_or(""),
        "token_type": "Bearer",
        "expiry_date": new_expiry,
        "refresh_token": refresh_token,
    });
    if let Err(e) = std::fs::write(&path, serde_json::to_string_pretty(&updated)?) {
        warn!("Could not update gemini oauth_creds.json: {}", e);
    } else {
        info!("Refreshed gemini CLI token, expires in {}s", expires_in);
    }

    Ok(new_token)
}

fn gemini_creds_path() -> Option<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".gemini").join("oauth_creds.json"))
        .filter(|p| p.exists())
}

fn read_gcloud_adc_quota_project() -> Option<String> {
    let path = dirs::config_dir()?
        .join("gcloud")
        .join("application_default_credentials.json");
    let mut text = std::fs::read_to_string(path).ok()?;
    let val: OwnedValue = unsafe { simd_json::from_str(&mut text) }.ok()?;
    val.get("quota_project_id")
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn read_extension_adc_quota_project() -> Option<String> {
    let path = dirs::home_dir()?
        .join(".cache")
        .join("google-vscode-extension")
        .join("auth")
        .join("application_default_credentials.json");
    let text = std::fs::read_to_string(path).ok()?;
    let val: JsonValue = serde_json::from_str(&text).ok()?;
    val.get("quota_project_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(String::from)
}

fn read_antigravity_project() -> Option<String> {
    let path = dirs::config_dir()?
        .join("Antigravity")
        .join("User")
        .join("settings.json");
    let text = std::fs::read_to_string(path).ok()?;
    let val: JsonValue = serde_json::from_str(&text).ok()?;
    let project = val
        .get("geminicodeassist.project")
        .and_then(|v| v.as_str())
        .or_else(|| val.get("cloudcode.project").and_then(|v| v.as_str()))
        .or_else(|| val.get("cloudcode.duetAI.project").and_then(|v| v.as_str()))?;
    let project = project.trim();
    if project.is_empty() {
        None
    } else {
        Some(project.to_string())
    }
}

fn read_adc_oauth_client() -> Option<(String, String)> {
    let path = dirs::config_dir()?
        .join("gcloud")
        .join("application_default_credentials.json");
    let mut text = std::fs::read_to_string(path).ok()?;
    let val: OwnedValue = unsafe { simd_json::from_str(&mut text) }.ok()?;
    let client_id = val.get("client_id").and_then(|v| v.as_str())?.to_string();
    let client_secret = val
        .get("client_secret")
        .and_then(|v| v.as_str())?
        .to_string();
    Some((client_id, client_secret))
}

fn read_env_oauth_client() -> Option<(String, String)> {
    let client_id = std::env::var("GEMINI_OAUTH_CLIENT_ID").ok()?;
    let client_secret = std::env::var("GEMINI_OAUTH_CLIENT_SECRET").ok()?;
    Some((client_id, client_secret))
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}
