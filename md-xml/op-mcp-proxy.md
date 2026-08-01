This file is a merged representation of the entire codebase, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of the entire repository's contents.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
proto/
  vertex_ai.proto
src/
  cloudaicompanion.rs
  codex.rs
  direct_llm.rs
  gcloud_auth.rs
  http_server.rs
  main.rs
  session.rs
  sled.rs
  vertex_grpc.rs
build.rs
Cargo.toml
compare-op-mcp-proxy.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="proto/vertex_ai.proto">
syntax = "proto3";

// Minimal Vertex AI Prediction Service proto — covers only GenerateContent.
// Package + service name must match exactly so tonic routes to the right gRPC path:
//   /google.cloud.aiplatform.v1.PredictionService/GenerateContent
//   /google.cloud.aiplatform.v1.PredictionService/StreamGenerateContent
package google.cloud.aiplatform.v1;

service PredictionService {
  rpc GenerateContent(GenerateContentRequest) returns (GenerateContentResponse);
  rpc StreamGenerateContent(GenerateContentRequest) returns (stream GenerateContentResponse);
}

// Field numbers match the canonical googleapis proto.
message GenerateContentRequest {
  repeated Content contents = 2;
  GenerationConfig generation_config = 7;
  // Full resource name: projects/{p}/locations/{l}/publishers/google/models/{m}
  string model = 5;
  // System-level instruction — must not appear in contents
  Content system_instruction = 8;
}

message Content {
  string role = 1;
  repeated Part parts = 2;
}

message Part {
  oneof data {
    string text = 1;
  }
}

message GenerationConfig {
  float temperature = 1;
  int32 max_output_tokens = 5;
}

message GenerateContentResponse {
  // field 1 is PromptFeedback (skipped) — candidates is at field 2
  repeated Candidate candidates = 2;
  UsageMetadata usage_metadata = 4;
}

message Candidate {
  int32 index = 1;
  Content content = 2;
  int32 finish_reason = 3;
}

message UsageMetadata {
  int32 prompt_token_count = 1;
  int32 candidates_token_count = 2;
  int32 total_token_count = 3;
}
</file>

<file path="src/cloudaicompanion.rs">
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
        Self::new_with_proxy(None)
    }

    pub fn new_with_proxy(socks_proxy: Option<&str>) -> Self {
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

        let mut client_builder = Client::builder().timeout(Duration::from_secs(120));
        if let Some(proxy_url) = socks_proxy {
            match reqwest::Proxy::all(proxy_url) {
                Ok(proxy) => {
                    client_builder = client_builder.proxy(proxy);
                    info!(proxy = %proxy_url, "LLM HTTP calls routed through Xray SOCKS5");
                }
                Err(e) => warn!("Invalid SOCKS proxy URL {}: {}", proxy_url, e),
            }
        }

        Self {
            cli: client_builder.build().expect("http client"),
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
</file>

<file path="src/codex.rs">
//! Codex CLI backend for the local OpenAI-compatible proxy.
//!
//! This keeps ChatGPT/Codex OAuth in `~/.codex/auth.json`. Factory Droid only sees
//! the loopback proxy and a dummy BYOK token.

use anyhow::{anyhow, Context};
use serde_json::Value;
use std::fmt::Write;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

const DEFAULT_PROXY_MODEL: &str = "codex-gpt-5.5";
const DEFAULT_CODEX_MODEL: &str = "gpt-5.5";

pub fn enabled() -> bool {
    env_flag("CODEX_PROXY_ENABLE", false) || std::env::var("CODEX_PROXY_MODEL").is_ok()
}

pub fn advertised_model() -> String {
    std::env::var("CODEX_PROXY_ADVERTISED_MODEL")
        .or_else(|_| std::env::var("CODEX_PROXY_MODEL"))
        .unwrap_or_else(|_| DEFAULT_PROXY_MODEL.to_string())
}

pub fn is_codex_model(model: &str) -> bool {
    let norm = model.trim().to_ascii_lowercase();
    enabled()
        && (norm == advertised_model().to_ascii_lowercase()
            || norm.starts_with("codex:")
            || norm.starts_with("codex-"))
}

pub async fn generate(model: &str, messages: &[Value]) -> anyhow::Result<String> {
    let prompt = render_messages(messages);
    let codex_model = resolve_codex_model(model);
    let codex_bin = std::env::var("CODEX_PROXY_BIN").unwrap_or_else(|_| "codex".to_string());
    let cwd = std::env::var("CODEX_PROXY_CWD").unwrap_or_else(|_| "/tmp".to_string());
    let timeout_secs = std::env::var("CODEX_PROXY_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v >= 5)
        .unwrap_or(180);

    let mut child = Command::new(codex_bin)
        .arg("exec")
        .arg("--ephemeral")
        .arg("--skip-git-repo-check")
        .arg("--ignore-rules")
        .arg("--sandbox")
        .arg("read-only")
        .arg("--cd")
        .arg(cwd)
        .arg("-m")
        .arg(codex_model)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("failed to run codex exec")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .await
            .context("failed to write prompt to codex stdin")?;
    } else {
        return Err(anyhow!("failed to open codex stdin"));
    }

    let output = match timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await {
        Ok(result) => result.context("failed to wait for codex exec")?,
        Err(_) => return Err(anyhow!("codex exec timed out after {timeout_secs}s")),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow!(
            "codex exec failed with status {}: {}{}{}",
            output.status,
            stderr.trim(),
            if stderr.trim().is_empty() || stdout.trim().is_empty() {
                ""
            } else {
                " | "
            },
            stdout.trim()
        ));
    }

    let text = String::from_utf8(output.stdout)
        .context("codex exec returned non-utf8 stdout")?
        .trim()
        .to_string();

    if text.is_empty() {
        Err(anyhow!("codex exec returned an empty response"))
    } else {
        Ok(text)
    }
}

fn resolve_codex_model(model: &str) -> String {
    if let Ok(explicit) = std::env::var("CODEX_PROXY_CODEX_MODEL") {
        return explicit;
    }

    let norm = model.trim();
    if let Some(rest) = norm.strip_prefix("codex:") {
        return rest.to_string();
    }

    if norm == DEFAULT_PROXY_MODEL {
        return DEFAULT_CODEX_MODEL.to_string();
    }

    norm.strip_prefix("codex-")
        .map(|rest| rest.to_string())
        .unwrap_or_else(|| DEFAULT_CODEX_MODEL.to_string())
}

fn render_messages(messages: &[Value]) -> String {
    let turns = messages
        .iter()
        .rev()
        .filter_map(|m| {
            let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            if role != "user" && role != "assistant" {
                return None;
            }
            let content = render_content(m.get("content")?);
            if content.trim().is_empty() {
                None
            } else {
                Some((role, content))
            }
        })
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let mut prompt = String::from("Reply directly and concisely.\n\n");

    for (role, content) in turns {
        let trimmed = truncate_chars(content.trim(), 12_000);
        let _ = writeln!(prompt, "{role}: {trimmed}\n");
    }

    prompt
}

fn render_content(content: &Value) -> String {
    if let Some(s) = content.as_str() {
        return user_visible_text(s).unwrap_or_default();
    }

    if let Some(items) = content.as_array() {
        return items
            .iter()
            .filter_map(|item| {
                let text = item
                    .get("text")
                    .and_then(|v| v.as_str())
                    .or_else(|| item.get("content").and_then(|v| v.as_str()))?;
                user_visible_text(text)
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    let text = content.to_string();
    user_visible_text(&text).unwrap_or_default()
}

fn user_visible_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.starts_with("<system-reminder>") {
        return None;
    }

    Some(trimmed.to_string())
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }

    let mut out = s.chars().take(max_chars).collect::<String>();
    out.push_str("\n[truncated by local proxy]");
    out
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
</file>

<file path="src/direct_llm.rs">
//! Direct-mode handler for LLM MCP methods.
//! Prefers VSCode/Cloud Code OAuth cache, with Gemini CLI OAuth fallback.
//! Includes background auto-refresh so tokens never expire mid-session.

use crate::cloudaicompanion::{self, CloudAICompanion};
use crate::gcloud_auth::GCloudAuth;
use chrono::{DateTime, Utc};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};

struct CachedToken {
    token: String,
    expiry: DateTime<Utc>,
}

pub struct DirectLLM {
    companion: CloudAICompanion,
    cached_token: Mutex<Option<CachedToken>>,
    gcloud_auth: GCloudAuth,
}

impl DirectLLM {
    pub async fn new() -> anyhow::Result<Self> {
        Self::new_with_proxy(None).await
    }

    pub async fn new_with_proxy(socks_proxy: Option<&str>) -> anyhow::Result<Self> {
        Ok(Self {
            companion: CloudAICompanion::new_with_proxy(socks_proxy),
            cached_token: Mutex::new(None),
            gcloud_auth: GCloudAuth::new(),
        })
    }

    /// Start background auto-refresh task. Call once after wrapping in Arc.
    pub fn start_auto_refresh(self: &Arc<Self>) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            // Initial delay — let the first real request prime the token.
            sleep(Duration::from_secs(60)).await;
            loop {
                let should_refresh = {
                    let guard = this.cached_token.lock().await;
                    match guard.as_ref() {
                        Some(ct) => ct.expiry < Utc::now() + chrono::Duration::minutes(10),
                        None => true,
                    }
                };

                if should_refresh {
                    debug!("Auto-refresh: token expiring soon or missing, refreshing");
                    match this.fetch_fresh_token().await {
                        Ok((token, expiry)) => {
                            info!(
                                "Auto-refresh: token refreshed, valid until {}",
                                expiry.format("%H:%M:%S UTC")
                            );
                            *this.cached_token.lock().await = Some(CachedToken { token, expiry });
                        }
                        Err(e) => {
                            warn!("Auto-refresh: token refresh failed: {}", e);
                        }
                    }
                }

                // Check every 5 minutes.
                sleep(Duration::from_secs(300)).await;
            }
        });
    }

    /// Fetch a fresh token from the best available source.
    async fn fetch_fresh_token(&self) -> anyhow::Result<(String, DateTime<Utc>)> {
        // 1. Try extension token refresh directly (has cloud-ide scope).
        match self.gcloud_auth.refresh_extension_token().await {
            Ok((token, expiry)) => return Ok((token, expiry)),
            Err(e) => {
                debug!("Extension token refresh failed: {}", e);
            }
        }

        // 2. Full auth chain (extension cache → gcloud CLI → etc).
        if env_flag("MCP_PROXY_PREFER_VSCODE_AUTH", true) {
            match self.gcloud_auth.get_token().await {
                Ok(pair) => return Ok(pair),
                Err(e) => {
                    warn!(
                        "Preferred VSCode/Cloud Code auth path failed, trying Gemini OAuth fallback: {}",
                        e
                    );
                }
            }
        }

        // 3. Gemini CLI OAuth.
        if !env_flag("MCP_PROXY_DISABLE_GEMINI_OAUTH", false)
            && !env_flag("OP_MCP_PROXY_DISABLE_GEMINI_OAUTH", false)
        {
            match cloudaicompanion::read_gemini_cli_token() {
                Ok((token, expiry_ms)) => {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    if expiry_ms > now_ms + 300_000 {
                        let expiry = DateTime::from_timestamp_millis(expiry_ms)
                            .unwrap_or_else(|| Utc::now() + chrono::Duration::minutes(55));
                        return Ok((token, expiry));
                    }
                    info!("Gemini CLI token expired or expiring soon, refreshing...");
                }
                Err(e) => {
                    debug!("Cannot read gemini CLI token: {}", e);
                }
            }

            match cloudaicompanion::refresh_gemini_cli_token().await {
                Ok(token) => {
                    return Ok((token, Utc::now() + chrono::Duration::minutes(55)));
                }
                Err(e) => {
                    warn!("Gemini CLI token refresh failed: {}", e);
                }
            }
        }

        // 4. Final fallback to full auth chain.
        self.gcloud_auth.get_token().await
    }

    /// Get a valid token, using cache when possible.
    async fn get_token(&self) -> anyhow::Result<String> {
        // Check cached token first.
        {
            let guard = self.cached_token.lock().await;
            if let Some(ref ct) = *guard {
                if ct.expiry > Utc::now() + chrono::Duration::minutes(2) {
                    return Ok(ct.token.clone());
                }
                debug!("Cached token expiring in < 2 min, fetching fresh token");
            }
        }

        let (token, expiry) = self.fetch_fresh_token().await?;
        *self.cached_token.lock().await = Some(CachedToken {
            token: token.clone(),
            expiry,
        });
        Ok(token)
    }

    /// Handle any MCP LLM-style request and return a JSON-RPC result.
    pub async fn handle(&self, req: &Value) -> Value {
        let id = req.get("id").cloned().unwrap_or_else(Value::null);
        let params = req.get("params").cloned().unwrap_or_else(Value::null);
        let prompt = match Self::extract_prompt(&params) {
            Ok(p) => p,
            Err(e) => return error(&id, -32700, e.to_string()),
        };
        let model = params
            .get("model")
            .and_then(|m| m.as_str())
            .filter(|m| !m.trim().is_empty());

        let token = match self.get_token().await {
            Ok(t) => t,
            Err(e) => return error(&id, -32603, format!("token: {e}")),
        };

        let max_attempts = std::env::var("MCP_PROXY_GENERATE_MAX_ATTEMPTS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v >= 1)
            .unwrap_or(3);

        let mut last_error: Option<String> = None;
        for attempt in 1..=max_attempts {
            match self.companion.generate(&prompt, &token, model).await {
                Ok(text) => {
                    return simd_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "completion": text,
                            "model": model.unwrap_or("gemini-2.5-flash"),
                            "stopReason": "stop"
                        }
                    });
                }
                Err(e) => {
                    let msg = e.to_string();
                    let retryable = is_retryable_generate_error(&msg);
                    last_error = Some(msg.clone());

                    if retryable && attempt < max_attempts {
                        let backoff_ms = 500u64.saturating_mul(attempt as u64);
                        warn!(
                            "Code Assist transient failure (attempt {}/{}): {}; retrying in {}ms",
                            attempt, max_attempts, msg, backoff_ms
                        );
                        sleep(Duration::from_millis(backoff_ms)).await;
                        continue;
                    }

                    return error(&id, -32603, format!("generate: {msg}"));
                }
            }
        }

        error(
            &id,
            -32603,
            format!(
                "generate: {}",
                last_error.unwrap_or_else(|| "unknown generate error".to_string())
            ),
        )
    }

    fn extract_prompt(params: &Value) -> anyhow::Result<String> {
        if let Some(msg_array) = params.get("messages").and_then(|v| v.as_array()) {
            return Ok(msg_array
                .iter()
                .filter_map(|m| {
                    let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                    let content = m.get("content")?;
                    let txt = content
                        .get("text")
                        .and_then(|v| v.as_str())
                        .or_else(|| content.as_str())?;
                    Some(format!("{role}: {txt}"))
                })
                .collect::<Vec<_>>()
                .join("\n"));
        }

        if let Some(txt) = params.get("prompt").and_then(|v| v.as_str()) {
            return Ok(txt.to_string());
        }

        if let Some(txt) = params
            .get("ref")
            .and_then(|r| r.get("text"))
            .and_then(|v| v.as_str())
        {
            return Ok(txt.to_string());
        }

        anyhow::bail!("no prompt found")
    }
}

fn is_retryable_generate_error(msg: &str) -> bool {
    msg.contains("429 Too Many Requests")
        || msg.contains("RESOURCE_EXHAUSTED")
        || msg.contains("RATE_LIMIT_EXCEEDED")
        || msg.contains("MODEL_CAPACITY_EXHAUSTED")
        || msg.contains("empty response text from code-assist")
        || msg.contains("finish_reason=MALFORMED_FUNCTION_CALL")
        || msg.contains("UNEXPECTED_TOOL_CALL")
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

fn error(id: &Value, code: i32, msg: impl Into<String>) -> Value {
    simd_json::json!({
        "jsonrpc": "2.0",
        "id": id.clone(),
        "error": {
            "code": code,
            "message": msg.into()
        }
    })
}
</file>

<file path="src/gcloud_auth.rs">
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
</file>

<file path="src/http_server.rs">
//! OpenAI-compatible HTTP server mode.
//!
//! POST /v1/chat/completions → Vertex AI via gRPC (StreamGenerateContent / GenerateContent)
//! GET  /v1/models            → list of Gemini models
//!
//! Activated by setting HTTP_SERVER_ADDR (e.g. "127.0.0.1:11435").
//! Set VERTEX_PROJECT=<gcp-project> to route to Vertex AI.

use crate::codex;
use crate::direct_llm::DirectLLM;
use crate::vertex_grpc::VertexGrpcClient;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{sse::Sse, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Token-bucket rate limiter — refills `capacity` tokens per minute.
struct TokenBucket {
    tokens: f64,
    capacity: f64, // = rpm limit
    last_refill: Instant,
}

impl TokenBucket {
    fn new(rpm: u32) -> Self {
        let cap = rpm as f64;
        Self {
            tokens: cap,
            capacity: cap,
            last_refill: Instant::now(),
        }
    }

    /// Try to consume one token. Returns how long to wait if empty.
    fn try_consume(&mut self) -> Result<(), std::time::Duration> {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.capacity / 60.0).min(self.capacity);
        self.last_refill = Instant::now();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            let wait_secs = (1.0 - self.tokens) * 60.0 / self.capacity;
            Err(std::time::Duration::from_secs_f64(wait_secs))
        }
    }
}

pub struct AppState {
    pub llm: Option<Arc<DirectLLM>>,
    pub vertex: Option<Arc<VertexGrpcClient>>,
    pub rate_limiter: Arc<Mutex<TokenBucket>>,
    pub chat_manager: Arc<op_llm::chat::ChatManager>,
}

// ── OpenAI request/response types ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Serialize)]
struct ChatCompletionResponse {
    id: String,
    object: &'static str,
    created: i64,
    model: String,
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Serialize)]
struct Choice {
    index: u32,
    message: ChatMessage,
    finish_reason: &'static str,
}

#[derive(Serialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Serialize)]
struct ModelObject {
    id: String,
    object: &'static str,
    created: i64,
    owned_by: &'static str,
}

#[derive(Serialize)]
struct ModelList {
    object: &'static str,
    data: Vec<ModelObject>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn list_models(State(state): State<Arc<AppState>>) -> Json<ModelList> {
    let mut models = Vec::new();

    // Query active models from all available providers in ChatManager
    let providers = state.chat_manager.available_providers();
    for provider_type in providers {
        if provider_type == op_llm::provider::ProviderType::McpProxy {
            // Avoid listing McpProxy provider within the proxy itself
            continue;
        }
        if let Ok(provider_models) = state.chat_manager.list_models_for_provider(&provider_type).await {
            for m in provider_models {
                let owned_by = match provider_type {
                    op_llm::provider::ProviderType::Anthropic => "anthropic",
                    op_llm::provider::ProviderType::Antigravity => "google",
                    op_llm::provider::ProviderType::Gemini => "google",
                    op_llm::provider::ProviderType::GeminiCli => "google",
                    op_llm::provider::ProviderType::OpenClaw => "openclaw",
                    op_llm::provider::ProviderType::Assistant => "assistant",
                    op_llm::provider::ProviderType::OpenAI => "openai",
                    _ => "custom",
                };
                models.push(model_object(&m.id, owned_by));
            }
        }
    }

    if codex::enabled() {
        models.push(model_object(&codex::advertised_model(), "openai"));
    }

    // Fallback: If no providers are loaded yet or list is empty, return defaults
    if models.is_empty() {
        models = vec![
            model_object("gemini-3.5-flash", "google"),
            model_object("gemini-2.5-pro", "google"),
            model_object("gemini-2.5-flash", "google"),
            model_object("gemini-2.5-flash-lite", "google"),
            model_object("gemini-2.0-flash-001", "google"),
            model_object("gemini-2.0-flash-lite", "google"),
        ];
    }

    Json(ModelList {
        object: "list",
        data: models,
    })
}

fn model_object(id: &str, owned_by: &'static str) -> ModelObject {
    ModelObject {
        id: id.to_string(),
        object: "model",
        created: 1700000000,
        owned_by,
    }
}

async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    // Rate limit before touching Vertex AI.
    {
        let wait = state.rate_limiter.lock().await.try_consume().err();
        if let Some(delay) = wait {
            if delay.as_secs() > 5 {
                // Backlog too deep — reject rather than queue indefinitely.
                warn!(wait_ms = delay.as_millis(), "rate limit: rejecting request");
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({ "error": { "message": "rate limit exceeded", "type": "rate_limit_error" } })),
                ).into_response();
            }
            warn!(
                wait_ms = delay.as_millis(),
                "rate limit: throttling request"
            );
            tokio::time::sleep(delay).await;
        }
    }

    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = chrono::Utc::now().timestamp();
    let messages: Vec<serde_json::Value> = req
        .messages
        .iter()
        .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
        .collect();

    if codex::is_codex_model(&req.model) {
        info!(model = %req.model, msgs = req.messages.len(), stream = req.stream, "codex chat request");
        match codex::generate(&req.model, &messages).await {
            Ok(text) if req.stream => return single_chunk_sse(text, req.model, id, created),
            Ok(text) => return ok_response(text, req.model, id, created),
            Err(e) => {
                warn!("Codex proxy error: {}", e);
                let body = serde_json::json!({ "error": { "message": e.to_string() } });
                return (StatusCode::BAD_GATEWAY, Json(body)).into_response();
            }
        }
    }

    // Route request through ChatManager if an active provider supports the model
    if let Some(provider_type) = state.chat_manager.find_provider_for_model(&req.model).await {
        info!(
            model = %req.model,
            provider = ?provider_type,
            msgs = req.messages.len(),
            stream = req.stream,
            "routing request to ChatManager"
        );
        let op_llm_messages = map_openai_messages(&req.messages);

        if req.stream {
            use tokio_stream::StreamExt as _;
            match state.chat_manager.chat_stream_with(&provider_type, &req.model, op_llm_messages).await {
                Ok(rx) => {
                    let model_str = req.model.clone();
                    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
                    let sse_stream = stream
                        .map(move |result| {
                            let id = id.clone();
                            let model_str = model_str.clone();
                            match result {
                                Ok(text) => {
                                    let chunk = serde_json::json!({
                                        "id": id,
                                        "object": "chat.completion.chunk",
                                        "created": created,
                                        "model": model_str,
                                        "choices": [{
                                            "index": 0,
                                            "delta": { "content": text },
                                            "finish_reason": null
                                        }]
                                    });
                                    Ok::<axum::response::sse::Event, std::convert::Infallible>(
                                        axum::response::sse::Event::default().data(chunk.to_string())
                                    )
                                }
                                Err(e) => {
                                    warn!("op-llm stream error: {}", e);
                                    let chunk = serde_json::json!({
                                        "error": { "message": e.to_string() }
                                    });
                                    Ok::<axum::response::sse::Event, std::convert::Infallible>(
                                        axum::response::sse::Event::default().data(chunk.to_string())
                                    )
                                }
                            }
                        })
                        .chain(tokio_stream::once(Ok::<axum::response::sse::Event, std::convert::Infallible>(
                            axum::response::sse::Event::default().data("[DONE]")
                        )));

                    return Sse::new(sse_stream)
                        .keep_alive(axum::response::sse::KeepAlive::default())
                        .into_response();
                }
                Err(e) => {
                    warn!("op-llm stream error: {}", e);
                    let body = serde_json::json!({ "error": { "message": e.to_string() } });
                    return (StatusCode::BAD_GATEWAY, Json(body)).into_response();
                }
            }
        } else {
            match state.chat_manager.chat_with(&provider_type, &req.model, op_llm_messages).await {
                Ok(chat_resp) => {
                    return ok_response(chat_resp.message.content, req.model, id, created);
                }
                Err(e) => {
                    warn!("op-llm chat error: {}", e);
                    let body = serde_json::json!({ "error": { "message": e.to_string() } });
                    return (StatusCode::BAD_GATEWAY, Json(body)).into_response();
                }
            }
        }
    }

    // Vertex AI gRPC path.
    if let Some(ref vertex) = state.vertex {
        info!(model = %req.model, msgs = req.messages.len(), stream = req.stream, "chat request");
        if req.stream {
            match vertex
                .stream_generate(&req.model, &messages, req.max_tokens, id, created)
                .await
            {
                Ok(sse_stream) => {
                    return Sse::new(sse_stream)
                        .keep_alive(axum::response::sse::KeepAlive::default())
                        .into_response();
                }
                Err(e) => {
                    warn!("Vertex AI stream error: {}", e);
                    let body = serde_json::json!({ "error": { "message": e.to_string() } });
                    return (StatusCode::BAD_GATEWAY, Json(body)).into_response();
                }
            }
        } else {
            match vertex.generate(&req.model, &messages, req.max_tokens).await {
                Ok(text) => return ok_response(text, req.model, id, created),
                Err(e) => {
                    warn!("Vertex AI error: {}", e);
                    let body = serde_json::json!({ "error": { "message": e.to_string() } });
                    return (StatusCode::BAD_GATEWAY, Json(body)).into_response();
                }
            }
        }
    }

    // CloudAI companion fallback.
    if let Some(ref llm) = state.llm {
        let mcp_req = simd_json::json!({
            "jsonrpc": "2.0",
            "id": "http-1",
            "method": "sampling/createMessage",
            "params": {
                "model": req.model,
                "messages": messages_to_simd(&messages),
            }
        });

        let llm_resp = llm.handle(&mcp_req).await;

        if let Some(err) = llm_resp.get("error") {
            let msg = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("llm error");
            let code = err.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
            let body = serde_json::json!({ "error": { "message": msg, "code": code } });
            return (StatusCode::BAD_GATEWAY, Json(body)).into_response();
        }

        let text = llm_resp["result"]["completion"]
            .as_str()
            .unwrap_or("")
            .to_string();

        if req.stream {
            return single_chunk_sse(text, req.model, id, created);
        }
        return ok_response(text, req.model, id, created);
    }

    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({ "error": "no LLM backend configured" })),
    )
        .into_response()
}

fn ok_response(text: String, model: String, id: String, created: i64) -> Response {
    let word_count = text.split_whitespace().count() as u32;
    let response = ChatCompletionResponse {
        id,
        object: "chat.completion",
        created,
        model,
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: serde_json::Value::String(text),
            },
            finish_reason: "stop",
        }],
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: word_count,
            total_tokens: word_count,
        },
    };
    Json(response).into_response()
}

// Fake SSE (single chunk) for the CloudAI fallback path which isn't natively streaming.
fn single_chunk_sse(text: String, model: String, id: String, created: i64) -> Response {
    let chunk = serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{ "index": 0, "delta": { "role": "assistant", "content": text }, "finish_reason": "stop" }]
    });
    let body = format!(
        "data: {}\n\ndata: [DONE]\n\n",
        serde_json::to_string(&chunk).unwrap_or_default()
    );
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn messages_to_simd(messages: &[serde_json::Value]) -> OwnedValue {
    let mut arr = Vec::new();
    for m in messages {
        let role = m["role"].as_str().unwrap_or("user").to_string();
        let content_owned;
        let content = if let Some(s) = m["content"].as_str() {
            s
        } else {
            content_owned = m["content"].to_string();
            content_owned.trim_matches('"')
        };
        arr.push(simd_json::json!({
            "role": role,
            "content": content,
        }));
    }
    OwnedValue::Array(arr)
}

// ── Request logging middleware ─────────────────────────────────────────────────

async fn log_request(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let resp = next.run(req).await;
    let status = resp.status();
    if status.is_success() {
        info!(method = %method, path = %uri.path(), status = %status.as_u16(), "http");
    } else {
        warn!(method = %method, path = %uri.path(), status = %status.as_u16(), "http");
    }
    resp
}

// ── Server entry point ────────────────────────────────────────────────────────

fn map_openai_messages(openai_msgs: &[ChatMessage]) -> Vec<op_llm::provider::ChatMessage> {
    openai_msgs
        .iter()
        .map(|m| {
            let content_str = match &m.content {
                serde_json::Value::String(s) => s.clone(),
                other => {
                    if let Some(arr) = other.as_array() {
                        let mut text_parts = Vec::new();
                        for part in arr {
                            if let Some(txt) = part.get("text").and_then(|v| v.as_str()) {
                                text_parts.push(txt.to_string());
                            }
                        }
                        text_parts.join("\n")
                    } else {
                        other.to_string().trim_matches('"').to_string()
                    }
                }
            };
            op_llm::provider::ChatMessage {
                role: m.role.clone(),
                content: content_str,
                tool_calls: None,
                tool_call_id: None,
            }
        })
        .collect()
}

pub async fn run(
    llm: Option<Arc<DirectLLM>>,
    chat_manager: Arc<op_llm::chat::ChatManager>,
    addr: &str,
) -> anyhow::Result<()> {
    let vertex = if let Ok(project) = std::env::var("VERTEX_PROJECT")
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or(())
    {
        let region = std::env::var("VERTEX_REGION").unwrap_or_else(|_| "us-central1".to_string());
        info!(project = %project, region = %region, "Using Vertex AI gRPC backend");
        match VertexGrpcClient::new(project, region).await {
            Ok(c) => Some(c),
            Err(e) => {
                warn!("Vertex AI gRPC init failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    if vertex.is_none() && llm.is_some() {
        info!("Using CloudAI companion backend");
    } else if vertex.is_none() {
        warn!("No LLM backend configured");
    }

    let rpm: u32 = std::env::var("VERTEX_RATE_LIMIT_RPM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);
    info!(rpm, "Vertex AI rate limit");

    let state = Arc::new(AppState {
        llm,
        vertex,
        rate_limiter: Arc::new(Mutex::new(TokenBucket::new(rpm))),
        chat_manager,
    });
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/models", get(list_models))
        .with_state(state)
        .layer(middleware::from_fn(log_request));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("HTTP server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
</file>

<file path="src/main.rs">
//! MCP Proxy – thin shim with optional direct-to-subscription mode.
//!
//! Routing:
//!   - gRPC calls → op-dbus at 10.200.0.2:50051 with Ghostbridge metadata headers
//!     (x-ghostbridge-footprint, x-ghostbridge-trace-id) sourced from the identity sled.
//!   - LLM HTTP calls → Xray SOCKS5 at 10.200.0.1:1080 when sled is valid,
//!     so they pass through NextDNS + the privacy stack.

use op_cache::proto::{mcp_service_client::McpServiceClient, McpRequest};
use simd_json::prelude::*;
use simd_json::OwnedValue;
use std::io::{BufRead, Write};
use std::sync::Arc;
use tonic::transport::Channel;
use tracing::{info, warn};

mod cloudaicompanion;
mod codex;
mod direct_llm;
mod gcloud_auth;
mod http_server;
mod session;
mod sled;
mod vertex_grpc;

use direct_llm::DirectLLM;
use sled::SledSnapshot;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    // Read identity sled — footprint + trace-id for Ghostbridge header injection.
    let snapshot = SledSnapshot::read();
    if let Some(ref s) = snapshot {
        if s.is_valid {
            info!(
                footprint = %s.footprint_hex,
                trace_id  = %s.trace_id,
                nextdns   = %s.nextdns_profile,
                "Identity sled loaded"
            );
        } else {
            warn!("Identity sled present but is_valid=false — headers will be omitted");
        }
    } else {
        warn!(
            "Identity sled not found at {} — Ghostbridge headers disabled",
            sled::SLED_PATH
        );
    }

    // Xray SOCKS5 proxy — only used when XRAY_SOCKS_ADDR is explicitly set to a non-empty value.
    let xray_socks_env = std::env::var("XRAY_SOCKS_ADDR").unwrap_or_default();
    let xray_socks = xray_socks_env.as_str();
    let use_xray = !xray_socks.is_empty() && snapshot.as_ref().map(|s| s.is_valid).unwrap_or(false);

    // Initialize ChatManager to discover active providers/models.
    let chat_manager = Arc::new(op_llm::chat::ChatManager::new());

    // If DIRECT_MODE is set we handle LLM requests ourselves.
    let direct_mode = std::env::var("DIRECT_MODE").is_ok();
    let direct_llm = if direct_mode {
        info!(
            via_xray = use_xray,
            "Running in DIRECT_MODE – LLM calls go to cloudcode-pa.googleapis.com"
        );
        let llm = Arc::new(
            DirectLLM::new_with_proxy(if use_xray { Some(xray_socks) } else { None }).await?,
        );
        llm.start_auto_refresh();

        // Spawn OpenAI-compatible HTTP server in background only when not in HTTP_ONLY mode
        // (HTTP_ONLY runs the server in the main thread instead).
        if let Ok(http_addr) = std::env::var("HTTP_SERVER_ADDR") {
            if std::env::var("HTTP_ONLY").is_err() {
                let llm_clone = Arc::clone(&llm);
                let cm_clone = Arc::clone(&chat_manager);
                tokio::spawn(async move {
                    if let Err(e) = http_server::run(Some(llm_clone), cm_clone, &http_addr).await {
                        tracing::error!("HTTP server error: {}", e);
                    }
                });
            }
        }

        Some(llm)
    } else {
        None
    };

    // gRPC client for op-dbus — always connect; DIRECT_MODE only changes LLM routing.
    let daemon_addr =
        std::env::var("OP_DBUS_ADDR").unwrap_or_else(|_| "http://10.200.0.2:50051".to_string());
    info!(addr = %daemon_addr, direct_mode, "Connecting to op-dbus gRPC");
    let mut client: Option<McpServiceClient<Channel>> =
        match Channel::from_shared(daemon_addr.clone()) {
            Ok(builder) => Some(McpServiceClient::new(builder.connect_lazy())),
            Err(e) => {
                warn!("Invalid op-dbus address {}: {}", daemon_addr, e);
                None
            }
        };

    // HTTP-only mode: spawn the HTTP server (Vertex AI or CloudAI) and wait for signal.
    if std::env::var("HTTP_ONLY").is_ok() {
        if let Ok(http_addr) = std::env::var("HTTP_SERVER_ADDR") {
            if let Err(e) = http_server::run(direct_llm.map(|l| Arc::clone(&l)), Arc::clone(&chat_manager), &http_addr).await {
                tracing::error!("HTTP server error: {}", e);
            }
        } else {
            tokio::signal::ctrl_c().await?;
        }
        return Ok(());
    }

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let mut line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut line) }?;
        let method = req["method"].as_str().unwrap_or("");

        // Direct mode: intercept Gemini LLM methods only; everything else falls through to op-dbus.
        if let Some(ref llm) = direct_llm {
            let is_gemini = req
                .get("params")
                .and_then(|p| p.get("model"))
                .and_then(|m| m.as_str())
                .map(|m| m.to_ascii_lowercase().starts_with("gemini"))
                .unwrap_or(false);

            let direct_resp = match method {
                "completion/complete" | "sampling/createMessage" | "generate" if is_gemini => {
                    Some(llm.handle(&req).await)
                }
                "tools/call" => {
                    let tool_name = req
                        .get("params")
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    if tool_name == "generate" {
                        Some(handle_tools_call(llm, &req).await)
                    } else {
                        None // forward op-dbus tools to gRPC
                    }
                }
                _ => None, // forward everything else (initialize, tools/list, op-dbus calls) to op-dbus
            };

            if let Some(resp) = direct_resp {
                writeln!(stdout, "{}", simd_json::to_string(&resp)?)?;
                stdout.flush()?;
                continue;
            }
        }

        // Forward to op-dbus via gRPC with Ghostbridge identity headers.
        let json_resp = if let Some(client) = client.as_mut() {
            let grpc_req = McpRequest {
                jsonrpc: "2.0".to_string(),
                method: req["method"].as_str().unwrap_or("").to_string(),
                id: req["id"].as_str().unwrap_or("null").to_string(),
                params: simd_json::to_vec(&req["params"]).unwrap_or_default(),
            };

            // Wrap in tonic::Request and inject Ghostbridge headers from sled.
            let mut tonic_req = tonic::Request::new(grpc_req);
            if let Some(ref s) = snapshot {
                if s.is_valid {
                    if let (Ok(fp), Ok(tr)) = (
                        s.footprint_hex.parse::<tonic::metadata::MetadataValue<_>>(),
                        s.trace_id.parse::<tonic::metadata::MetadataValue<_>>(),
                    ) {
                        tonic_req
                            .metadata_mut()
                            .insert("x-ghostbridge-footprint", fp);
                        tonic_req
                            .metadata_mut()
                            .insert("x-ghostbridge-trace-id", tr);
                    }
                }
            }

            match client.handle_request(tonic_req).await {
                Ok(resp) => {
                    let grpc_resp = resp.into_inner();
                    if let Some(err) = grpc_resp.error {
                        simd_json::json!({
                            "jsonrpc": "2.0",
                            "id": grpc_resp.id,
                            "error": { "code": err.code, "message": err.message }
                        })
                    } else {
                        let mut result_bytes = grpc_resp.result;
                        let result = simd_json::to_owned_value(&mut result_bytes)
                            .unwrap_or_else(|_| simd_json::OwnedValue::null());
                        simd_json::json!({
                            "jsonrpc": "2.0",
                            "id": grpc_resp.id,
                            "result": result
                        })
                    }
                }
                Err(e) => simd_json::json!({
                    "jsonrpc": "2.0",
                    "id": req["id"].clone(),
                    "error": { "code": -32603, "message": format!("gRPC error: {}", e) }
                }),
            }
        } else {
            simd_json::json!({
                "jsonrpc": "2.0",
                "id": req["id"].clone(),
                "error": { "code": -32601, "message": format!("Method not available in DIRECT_MODE: {}", method) }
            })
        };

        writeln!(stdout, "{}", simd_json::to_string(&json_resp)?)?;
        stdout.flush()?;
    }
    Ok(())
}

async fn handle_tools_call(llm: &Arc<DirectLLM>, req: &OwnedValue) -> OwnedValue {
    let tool_name = req["params"]["name"].as_str().unwrap_or("");
    if tool_name != "generate" {
        return simd_json::json!({
            "jsonrpc": "2.0",
            "id": req["id"].clone(),
            "error": { "code": -32601, "message": format!("Unknown tool: {}", tool_name) }
        });
    }

    let prompt = match req["params"]["arguments"]["prompt"].as_str() {
        Some(p) if !p.trim().is_empty() => p.to_string(),
        _ => {
            return simd_json::json!({
                "jsonrpc": "2.0",
                "id": req["id"].clone(),
                "error": { "code": -32602, "message": "tools/call generate requires arguments.prompt" }
            });
        }
    };

    let generated_req = simd_json::json!({
        "jsonrpc": "2.0",
        "id": req["id"].clone(),
        "method": "generate",
        "params": {
            "prompt": prompt,
            "model": req["params"]["arguments"]["model"].clone()
        }
    });

    let llm_resp = llm.handle(&generated_req).await;
    if llm_resp.get("error").is_some() {
        return llm_resp;
    }

    let text = llm_resp["result"]["completion"]
        .as_str()
        .unwrap_or("")
        .to_string();
    simd_json::json!({
        "jsonrpc": "2.0",
        "id": req["id"].clone(),
        "result": {
            "content": [{ "type": "text", "text": text }]
        }
    })
}
</file>

<file path="src/session.rs">
//! Session management using WireGuard pubkey as identity.
//!
//! Sessions are created when a WireGuard peer connects and
//! destroyed on disconnect or timeout.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::gcloud_auth::GCloudAuth;

const SESSION_TIMEOUT_SECS: i64 = 3600; // 1 hour

#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: String,
    pub pubkey: String,
    pub user_email: Option<String>,
    pub oauth_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct SessionManager {
    db: Arc<Mutex<Connection>>,
    gcloud_auth: GCloudAuth,
    current_session_id: Arc<Mutex<Option<String>>>,
}

impl SessionManager {
    pub fn new() -> anyhow::Result<Self> {
        let db_path = Self::db_path()?;

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;

        // Initialize schema
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                pubkey TEXT NOT NULL,
                user_email TEXT,
                oauth_token TEXT,
                token_expires_at INTEGER,
                created_at INTEGER NOT NULL,
                last_seen_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_pubkey ON sessions(pubkey);

            CREATE TABLE IF NOT EXISTS wireguard_users (
                pubkey TEXT PRIMARY KEY,
                user_email TEXT NOT NULL,
                allowed_ip TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
        ",
        )?;

        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            gcloud_auth: GCloudAuth::new(),
            current_session_id: Arc::new(Mutex::new(None)),
        })
    }

    fn db_path() -> anyhow::Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("mcp-proxy");
        Ok(data_dir.join("sessions.db"))
    }

    /// Get the local WireGuard public key (this machine's identity)
    fn get_local_wireguard_pubkey() -> anyhow::Result<String> {
        // Try to get from environment first
        if let Ok(pubkey) = std::env::var("WG_PUBKEY") {
            return Ok(pubkey);
        }

        // Try to read from wg interface
        let output = Command::new("wg")
            .args(["show", "wg0", "public-key"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
            }
            _ => {
                // Fallback: generate a deterministic ID from hostname
                let hostname = hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "unknown".to_string());

                warn!("Could not get WireGuard pubkey, using hostname-based ID");
                Ok(format!("local:{}", hostname))
            }
        }
    }

    /// Get peer's pubkey from their IP address via WireGuard
    #[allow(dead_code)]
    fn get_pubkey_for_ip(peer_ip: &str) -> anyhow::Result<Option<String>> {
        let output = Command::new("wg")
            .args(["show", "wg0", "allowed-ips"])
            .output()?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Format: pubkey\tallowed_ip1, allowed_ip2
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let pubkey = parts[0];
                let ips = parts[1];

                if ips.contains(peer_ip) {
                    return Ok(Some(pubkey.to_string()));
                }
            }
        }

        Ok(None)
    }

    /// Create or retrieve session based on WireGuard identity
    pub async fn get_or_create_session_from_wireguard(&self) -> anyhow::Result<Session> {
        let pubkey = Self::get_local_wireguard_pubkey()?;
        self.get_or_create_session(&pubkey).await
    }

    /// Get or create a session for a given pubkey
    pub async fn get_or_create_session(&self, pubkey: &str) -> anyhow::Result<Session> {
        let db = self.db.lock().await;
        let now = Utc::now().timestamp();

        // Check for existing valid session
        let existing: Option<Session> = db
            .query_row(
                "SELECT session_id, pubkey, user_email, oauth_token, token_expires_at, 
                    created_at, last_seen_at
             FROM sessions 
             WHERE pubkey = ? AND last_seen_at > ?",
                params![pubkey, now - SESSION_TIMEOUT_SECS],
                |row| {
                    Ok(Session {
                        session_id: row.get(0)?,
                        pubkey: row.get(1)?,
                        user_email: row.get(2)?,
                        oauth_token: row.get(3)?,
                        token_expires_at: row
                            .get::<_, Option<i64>>(4)?
                            .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_default()),
                        created_at: DateTime::from_timestamp(row.get::<_, i64>(5)?, 0)
                            .unwrap_or_default(),
                        last_seen_at: DateTime::from_timestamp(row.get::<_, i64>(6)?, 0)
                            .unwrap_or_default(),
                    })
                },
            )
            .ok();

        if let Some(mut session) = existing {
            debug!("Found existing session: {}", session.session_id);

            // Update last_seen
            db.execute(
                "UPDATE sessions SET last_seen_at = ? WHERE session_id = ?",
                params![now, session.session_id],
            )?;
            session.last_seen_at = Utc::now();

            // Store current session ID
            *self.current_session_id.lock().await = Some(session.session_id.clone());

            return Ok(session);
        }

        // Create new session
        let session_id = Uuid::new_v4().to_string();
        info!(
            "Creating new session: {} for pubkey: {}",
            session_id, pubkey
        );

        // Try to get user email from WireGuard user mapping
        let user_email: Option<String> = db
            .query_row(
                "SELECT user_email FROM wireguard_users WHERE pubkey = ?",
                params![pubkey],
                |row| row.get(0),
            )
            .ok();

        // Try to get OAuth token
        let (oauth_token, token_expires_at) = match self.gcloud_auth.get_token().await {
            Ok((token, expires)) => (Some(token), Some(expires)),
            Err(e) => {
                warn!("Could not get OAuth token: {}", e);
                (None, None)
            }
        };

        db.execute(
            "INSERT INTO sessions (session_id, pubkey, user_email, oauth_token, 
                                   token_expires_at, created_at, last_seen_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                session_id,
                pubkey,
                user_email,
                oauth_token,
                token_expires_at.map(|t| t.timestamp()),
                now,
                now
            ],
        )?;

        let session = Session {
            session_id: session_id.clone(),
            pubkey: pubkey.to_string(),
            user_email,
            oauth_token,
            token_expires_at,
            created_at: Utc::now(),
            last_seen_at: Utc::now(),
        };

        // Store current session ID
        *self.current_session_id.lock().await = Some(session_id);

        Ok(session)
    }

    /// Update last_seen timestamp for current session
    pub async fn touch_session(&self) -> anyhow::Result<()> {
        let session_id = self.current_session_id.lock().await.clone();

        if let Some(id) = session_id {
            let db = self.db.lock().await;
            let now = Utc::now().timestamp();

            db.execute(
                "UPDATE sessions SET last_seen_at = ? WHERE session_id = ?",
                params![now, id],
            )?;
        }

        Ok(())
    }

    /// Get a valid OAuth token, refreshing if necessary
    pub async fn get_valid_token(&self) -> anyhow::Result<String> {
        let session_id = self.current_session_id.lock().await.clone();

        if let Some(id) = session_id {
            let db = self.db.lock().await;
            let now = Utc::now().timestamp();

            // Check if we have a valid cached token
            let cached: Option<(String, i64)> = db
                .query_row(
                    "SELECT oauth_token, token_expires_at FROM sessions 
                 WHERE session_id = ? AND oauth_token IS NOT NULL",
                    params![id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();

            if let Some((token, expires_at)) = cached {
                // Token valid for at least 5 more minutes
                if expires_at > now + 300 {
                    return Ok(token);
                }
            }

            drop(db); // Release lock before async call
        }

        // Refresh token
        let (token, expires_at) = self.gcloud_auth.get_token().await?;

        // Update in database
        if let Some(id) = self.current_session_id.lock().await.clone() {
            let db = self.db.lock().await;
            db.execute(
                "UPDATE sessions SET oauth_token = ?, token_expires_at = ? WHERE session_id = ?",
                params![token, expires_at.timestamp(), id],
            )?;
        }

        Ok(token)
    }

    /// Register a WireGuard user mapping
    #[allow(dead_code)]
    pub async fn register_wireguard_user(
        &self,
        pubkey: &str,
        user_email: &str,
        allowed_ip: &str,
    ) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        let now = Utc::now().timestamp();

        db.execute(
            "INSERT OR REPLACE INTO wireguard_users (pubkey, user_email, allowed_ip, created_at)
             VALUES (?, ?, ?, ?)",
            params![pubkey, user_email, allowed_ip, now],
        )?;

        info!("Registered WireGuard user: {} -> {}", pubkey, user_email);
        Ok(())
    }

    /// Clean up expired sessions
    #[allow(dead_code)]
    pub async fn cleanup_expired_sessions(&self) -> anyhow::Result<usize> {
        let db = self.db.lock().await;
        let cutoff = Utc::now().timestamp() - SESSION_TIMEOUT_SECS;

        let deleted = db.execute(
            "DELETE FROM sessions WHERE last_seen_at < ?",
            params![cutoff],
        )?;

        if deleted > 0 {
            info!("Cleaned up {} expired sessions", deleted);
        }

        Ok(deleted)
    }
}
</file>

<file path="src/sled.rs">
//! Zero-copy reader for the IdentitySled in /dev/shm/plugin_schema.dat.
//!
//! Mirrors the #[repr(C)] layout from op-identity::schema_bridge — must be
//! kept in sync if the sled struct changes.
//!
//! Layout (208 bytes total):
//!   [  0.. 32]  wireguard_pubkey   [u8; 32]
//!   [ 32.. 40]  mutation_index     u64 LE
//!   [ 40.. 41]  is_valid           bool
//!   [ 41.. 48]  _pad               [u8; 7]
//!   [ 48.. 80]  hashed_footprint   [u8; 32]
//!   [ 80.. 96]  schema_uuid        [u8; 16]
//!   [ 96..160]  subid              [u8; 64]
//!   [160..192]  control_source     [u8; 32]
//!   [192..208]  nextdns_profile    [u8; 16]

use memmap2::MmapOptions;
use std::fs::File;

pub const SLED_SIZE: usize = 208;
pub const SLED_PATH: &str = "/dev/shm/plugin_schema.dat";

pub struct SledSnapshot {
    pub is_valid: bool,
    pub mutation_index: u64,
    pub footprint_hex: String,
    pub trace_id: String,
    pub nextdns_profile: String,
    pub subid: String,
    pub control_source: String,
}

impl SledSnapshot {
    /// Read a snapshot from the sled. Returns None if file missing or invalid.
    pub fn read() -> Option<Self> {
        let file = File::open(SLED_PATH).ok()?;
        let mmap = unsafe { MmapOptions::new().len(SLED_SIZE).map(&file).ok()? };
        if mmap.len() < SLED_SIZE {
            return None;
        }

        let bytes = &mmap[..SLED_SIZE];

        let wg_pubkey = &bytes[0..32];
        let mutation_index = u64::from_le_bytes(bytes[32..40].try_into().ok()?);
        let is_valid = bytes[40] != 0;
        let footprint = &bytes[48..80];
        // subid at [96..160], control_source at [160..192], nextdns at [192..208]
        let nextdns_profile = fixed_str(&bytes[192..208]);
        let subid = fixed_str(&bytes[96..160]);
        let control_source = fixed_str(&bytes[160..192]);

        let footprint_hex = hex::encode(footprint);
        let trace_id = format!("{}-{}", hex::encode(&wg_pubkey[..4]), mutation_index);

        Some(SledSnapshot {
            is_valid,
            mutation_index,
            footprint_hex,
            trace_id,
            nextdns_profile,
            subid,
            control_source,
        })
    }
}

fn fixed_str(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}
</file>

<file path="src/vertex_grpc.rs">
//! Vertex AI gRPC client with cached OAuth token and real server-side streaming.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tonic::{
    metadata::MetadataValue,
    transport::{Channel, ClientTlsConfig},
    Request,
};
use tracing::{debug, info, warn};

use crate::gcloud_auth::GCloudAuth;

pub mod proto {
    tonic::include_proto!("google.cloud.aiplatform.v1");
}

use proto::{
    prediction_service_client::PredictionServiceClient, Content, GenerateContentRequest,
    GenerationConfig, Part,
};

struct CachedToken {
    token: String,
    expiry: DateTime<Utc>,
}

pub struct VertexGrpcClient {
    project: String,
    region: String,
    inner: PredictionServiceClient<Channel>,
    cached_token: Mutex<Option<CachedToken>>,
    gcloud_auth: GCloudAuth,
}

impl VertexGrpcClient {
    pub async fn new(project: String, region: String) -> anyhow::Result<Arc<Self>> {
        let endpoint = if region == "global" {
            "https://aiplatform.googleapis.com".to_string()
        } else {
            format!("https://{region}-aiplatform.googleapis.com")
        };
        let tls = ClientTlsConfig::new().with_webpki_roots();
        let channel = Channel::from_shared(endpoint)?
            .tls_config(tls)?
            .connect_lazy();

        let inner =
            PredictionServiceClient::new(channel).max_decoding_message_size(64 * 1024 * 1024); // 64 MiB for large responses

        let client = Arc::new(Self {
            project,
            region,
            inner,
            cached_token: Mutex::new(None),
            gcloud_auth: GCloudAuth::new(),
        });

        // Pre-warm token in background so first request is fast.
        let c = Arc::clone(&client);
        tokio::spawn(async move {
            match c.refresh_token().await {
                Ok(_) => info!("Vertex AI: initial token cached"),
                Err(e) => warn!("Vertex AI: initial token prefetch failed: {}", e),
            }
        });

        Ok(client)
    }

    async fn get_token(&self) -> anyhow::Result<String> {
        let guard = self.cached_token.lock().await;
        if let Some(ref ct) = *guard {
            if ct.expiry > Utc::now() + chrono::Duration::minutes(5) {
                debug!("Vertex AI: using cached token");
                return Ok(ct.token.clone());
            }
        }
        drop(guard);
        self.refresh_token().await
    }

    async fn refresh_token(&self) -> anyhow::Result<String> {
        let (token, expiry) = self.gcloud_auth.get_token().await?;
        info!(
            "Vertex AI: token refreshed, valid until {}",
            expiry.format("%H:%M:%S UTC")
        );
        let mut guard = self.cached_token.lock().await;
        *guard = Some(CachedToken {
            token: token.clone(),
            expiry,
        });
        Ok(token)
    }

    fn model_resource(&self, model: &str) -> String {
        format!(
            "projects/{}/locations/{}/publishers/google/models/{}",
            self.project, self.region, model
        )
    }

    fn extract_text(content: &JsonValue) -> String {
        // Handle both plain string and OpenAI-style array: [{"type":"text","text":"..."}]
        if let Some(s) = content.as_str() {
            return s.to_string();
        }
        if let Some(arr) = content.as_array() {
            return arr
                .iter()
                .filter(|part| part["type"].as_str() == Some("text"))
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join("");
        }
        content.to_string()
    }

    fn build_contents(messages: &[JsonValue]) -> (Vec<Content>, Option<Content>) {
        let mut system_parts: Vec<String> = Vec::new();
        let mut contents: Vec<Content> = Vec::new();

        for m in messages {
            let role = m["role"].as_str().unwrap_or("user");
            let text = Self::extract_text(&m["content"]);

            if role == "system" {
                system_parts.push(text);
                continue;
            }

            let vertex_role = if role == "assistant" { "model" } else { "user" }.to_string();

            // Merge consecutive messages with the same role (Vertex requires alternating).
            if let Some(last) = contents.last_mut() {
                if last.role == vertex_role {
                    if let Some(part) = last.parts.first_mut() {
                        if let Some(proto::part::Data::Text(ref mut t)) = part.data {
                            t.push('\n');
                            t.push_str(&text);
                            continue;
                        }
                    }
                }
            }

            contents.push(Content {
                role: vertex_role,
                parts: vec![Part {
                    data: Some(proto::part::Data::Text(text)),
                }],
            });
        }

        let system_instruction = if system_parts.is_empty() {
            None
        } else {
            Some(Content {
                role: String::new(), // system_instruction must have no role
                parts: vec![Part {
                    data: Some(proto::part::Data::Text(system_parts.join("\n"))),
                }],
            })
        };

        info!(
            n_contents = contents.len(),
            roles = %contents.iter().map(|c| c.role.as_str()).collect::<Vec<_>>().join(","),
            has_system = system_instruction.is_some(),
            "built vertex request"
        );

        (contents, system_instruction)
    }

    fn make_request(
        &self,
        model: &str,
        messages: &[JsonValue],
        max_tokens: Option<u32>,
    ) -> GenerateContentRequest {
        let (contents, system_instruction) = Self::build_contents(messages);
        GenerateContentRequest {
            model: self.model_resource(model),
            contents,
            system_instruction,
            generation_config: max_tokens.map(|t| GenerationConfig {
                max_output_tokens: t as i32,
                temperature: 0.0, // 0.0 = proto3 default, not sent on wire
            }),
        }
    }

    fn inject_headers(
        req: &mut Request<impl Sized>,
        token: &str,
        model_resource: &str,
    ) -> anyhow::Result<()> {
        req.metadata_mut().insert(
            "authorization",
            format!("Bearer {}", token).parse::<MetadataValue<_>>()?,
        );
        // Required by Google Cloud gRPC routing infrastructure — tells the
        // load balancer which model/project/location to route to.
        let routing = format!("model={}", model_resource.replace('/', "%2F"));
        req.metadata_mut().insert(
            "x-goog-request-params",
            routing.parse::<MetadataValue<_>>()?,
        );
        Ok(())
    }

    /// Unary call — returns full text once complete.
    pub async fn generate(
        &self,
        model: &str,
        messages: &[JsonValue],
        max_tokens: Option<u32>,
    ) -> anyhow::Result<String> {
        let token = self.get_token().await?;
        let model_resource = self.model_resource(model);
        let grpc_req = self.make_request(model, messages, max_tokens);
        let mut req = Request::new(grpc_req);
        Self::inject_headers(&mut req, &token, &model_resource)?;

        let resp = self.inner.clone().generate_content(req).await?;
        let text = resp
            .into_inner()
            .candidates
            .into_iter()
            .next()
            .and_then(|c| c.content)
            .and_then(|c| c.parts.into_iter().next())
            .and_then(|p| {
                if let Some(proto::part::Data::Text(t)) = p.data {
                    Some(t)
                } else {
                    None
                }
            })
            .unwrap_or_default();

        Ok(text)
    }

    /// Streaming call — returns a tokio stream of text chunks as SSE data lines.
    pub async fn stream_generate(
        &self,
        model: &str,
        messages: &[JsonValue],
        max_tokens: Option<u32>,
        id: String,
        created: i64,
    ) -> anyhow::Result<
        impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
    > {
        use tokio_stream::StreamExt as _;

        let token = self.get_token().await?;
        let model_resource = self.model_resource(model);
        let grpc_req = self.make_request(model, messages, max_tokens);
        let mut req = Request::new(grpc_req);
        Self::inject_headers(&mut req, &token, &model_resource)?;

        let model_str = model.to_string();
        let stream = self
            .inner
            .clone()
            .stream_generate_content(req)
            .await?
            .into_inner();

        let sse_stream = stream
            .map(move |result| {
                let id = id.clone();
                let model_str = model_str.clone();
                match result {
                    Ok(resp) => {
                        let text = resp
                            .candidates
                            .into_iter()
                            .next()
                            .and_then(|c| c.content)
                            .and_then(|c| c.parts.into_iter().next())
                            .and_then(|p| {
                                if let Some(proto::part::Data::Text(t)) = p.data {
                                    Some(t)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_default();

                        let chunk = serde_json::json!({
                            "id": id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": model_str,
                            "choices": [{
                                "index": 0,
                                "delta": { "content": text },
                                "finish_reason": null
                            }]
                        });
                        Ok(axum::response::sse::Event::default().data(chunk.to_string()))
                    }
                    Err(e) => {
                        warn!("Vertex AI stream error: {}", e);
                        let chunk = serde_json::json!({
                            "error": { "message": e.to_string() }
                        });
                        Ok(axum::response::sse::Event::default().data(chunk.to_string()))
                    }
                }
            })
            .chain(tokio_stream::once(Ok(axum::response::sse::Event::default(
            )
            .data("[DONE]"))));

        Ok(sse_stream)
    }
}
</file>

<file path="build.rs">
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(&["proto/vertex_ai.proto"], &["proto"])?;
    Ok(())
}
</file>

<file path="Cargo.toml">
[package]
name = "op-mcp-proxy"
version = "0.1.0"
edition = "2021"

[dependencies]
op-cache = { path = "../op-cache" }
op-identity = { path = "../op-identity" }
op-llm = { path = "../op-llm" }
tokio     = { version = "1", features = ["full"] }
tonic     = { workspace = true }
prost     = { workspace = true }
tokio-stream = { workspace = true }
futures   = { workspace = true }
serde     = { version = "1", features = ["derive"] }
simd-json = { workspace = true }
reqwest   = { version = "0.12", features = ["json", "rustls-tls", "socks"], default-features = false }
tracing   = "0.1"
tracing-subscriber = "0.3"
serde_json = "1"
anyhow    = "1"
dirs      = "5"
hostname  = "0.4"
rusqlite  = { workspace = true, features = ["bundled"] }
chrono    = { version = "0.4", features = ["serde"] }
uuid      = { version = "1.6", features = ["v4", "serde"] }
memmap2   = { workspace = true }
hex       = { workspace = true }
axum      = { workspace = true }

[build-dependencies]
tonic-build = { workspace = true }
</file>

<file path="compare-op-mcp-proxy.md">
# compare-op-mcp-proxy

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 5 |
| Proto files | 0 |
| Binary targets | 1 |
| UI files | 0 |
| Root-declared modules | 4 |
| Partial artifacts | 0 |
| Spec-listed source files | 5 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- Internal crate integrations: op-cache, op-identity.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/session.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/session.rs |
| `src/gcloud_auth.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/gcloud_auth.rs |
| `src/main.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/main.rs |
| `src/cloudaicompanion.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/cloudaicompanion.rs |
| `src/direct_llm.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/direct_llm.rs |
| `root` | ✅ Present | root source group | src/cloudaicompanion.rs, src/direct_llm.rs, src/gcloud_auth.rs, src/main.rs, src/session.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| session | ✅ Implemented | src/session.rs | SPEC main module |
| gcloud_auth | ✅ Implemented | src/gcloud_auth.rs | SPEC main module |
| cloudaicompanion | ✅ Implemented | src/cloudaicompanion.rs | SPEC main module |
| direct_llm | ✅ Implemented | src/direct_llm.rs | SPEC main module |
| Primary binary entrypoint | ✅ Implemented | src/main.rs | runtime |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-cache` - documented in SPEC
- `op-identity` - documented in SPEC

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `tonic` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `reqwest` - documented in SPEC
- `tracing` - documented in SPEC
- `tracing-subscriber` - documented in SPEC
- `serde_json` - documented in SPEC
- `anyhow` - documented in SPEC
- `dirs` - documented in SPEC
- `hostname` - documented in SPEC
- `rusqlite` - documented in SPEC
- `chrono` - documented in SPEC
- `uuid` - documented in SPEC

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: cloudaicompanion, direct_llm, gcloud_auth, session.
</file>

<file path="SPEC.md">
# op-mcp-proxy - Specification

## Overview
**Crate**: `op-mcp-proxy`  
**Location**: `crates/op-mcp-proxy`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-mcp-proxy"
version = "0.1.0"
edition = "2021"

[dependencies]
```

### Source Structure
```
op-mcp-proxy/src/session.rs
op-mcp-proxy/src/gcloud_auth.rs
op-mcp-proxy/src/main.rs
op-mcp-proxy/src/cloudaicompanion.rs
op-mcp-proxy/src/direct_llm.rs
```

### Key Dependencies
```toml
op-cache = { path = "../op-cache" }
op-identity = { path = "../op-identity" }
tokio     = { version = "1", features = ["full"] }
tonic     = "0.11"
serde     = { version = "1", features = ["derive"] }
simd-json = { workspace = true }
reqwest   = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tracing   = "0.1"
tracing-subscriber = "0.3"
serde_json = "1"
anyhow    = "1"
dirs      = "5"
hostname  = "0.4"
rusqlite  = { workspace = true, features = ["bundled"] }
chrono    = { version = "0.4", features = ["serde"] }
uuid      = { version = "1.6", features = ["v4", "serde"] }
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
       5 Rust source files

### Main Modules
session
gcloud_auth
cloudaicompanion
direct_llm

## Purpose


## Build Information
- **Edition**: 2021
- **Version**: 0.1.0
- **License**: 

## Related Crates
Internal dependencies:
- op-cache
- op-identity

---
*Generated from crate analysis*
</file>

</files>
