//! Loopback adapter for the real ZeroClaw daemon.
//!
//! ZeroClaw owns provider/model discovery, selection, persistence, reload, and
//! chat execution. The bridge remains the capability and audit boundary; it
//! does not implement another model provider.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{anyhow, Context};
use op_plugins::state_plugins::common::llm_projection::{ModelRoute, Provider};
use op_plugins::state_plugins::tched_router::{ChatInput, ChatOutput, TchedRouterState};
use reqwest::{RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8082";
const DEFAULT_AGENT_ALIAS: &str = "dashboard";

#[derive(Clone, Debug)]
pub(crate) struct ZeroclawRuntimeClient {
    http: reqwest::Client,
    endpoint: String,
    agent_alias: String,
    token: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConfiguredProvider {
    family: String,
    reference: String,
    selected_model: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeSelection {
    pub(crate) provider: String,
    pub(crate) model: String,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
}

#[derive(Debug, Deserialize)]
struct PropResponse {
    value: Value,
}

#[derive(Debug, Deserialize)]
struct ConfigListResponse {
    entries: Vec<ConfigListEntry>,
}

#[derive(Debug, Deserialize)]
struct ConfigListEntry {
    path: String,
    #[serde(default)]
    value: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ModelCatalogResponse {
    #[serde(default)]
    models: Vec<String>,
    #[serde(default)]
    live: bool,
}

impl ZeroclawRuntimeClient {
    pub(crate) fn from_env() -> Self {
        let endpoint = std::env::var("ZEROCLAW_RUNTIME_ENDPOINT")
            .unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string());
        let agent_alias = std::env::var("ZEROCLAW_RUNTIME_AGENT")
            .unwrap_or_else(|_| DEFAULT_AGENT_ALIAS.to_string());
        let token = std::env::var("ZEROCLAW_RUNTIME_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());
        Self::new(endpoint, agent_alias, token)
    }

    pub(crate) fn new(endpoint: String, agent_alias: String, token: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(600))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            http,
            endpoint: endpoint.trim_end_matches('/').to_string(),
            agent_alias,
            token,
        }
    }

    fn request(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
        operation: &str,
    ) -> anyhow::Result<T> {
        let response = self
            .request(request)
            .send()
            .await
            .with_context(|| format!("ZeroClaw {operation} request failed"))?;
        let status = response.status();
        let body = response
            .bytes()
            .await
            .with_context(|| format!("ZeroClaw {operation} response body failed"))?;
        if !status.is_success() {
            let detail = String::from_utf8_lossy(&body);
            return Err(anyhow!(
                "ZeroClaw {operation} returned {status}: {}",
                truncate_error(&detail)
            ));
        }
        serde_json::from_slice(&body)
            .with_context(|| format!("ZeroClaw {operation} returned invalid JSON"))
    }

    async fn health(&self) -> anyhow::Result<()> {
        let health: HealthResponse = self
            .send_json(self.http.get(format!("{}/health", self.endpoint)), "health")
            .await?;
        if health.status != "ok" {
            return Err(anyhow!(
                "ZeroClaw health status is '{}', expected 'ok'",
                health.status
            ));
        }
        Ok(())
    }

    async fn get_prop(&self, path: &str) -> anyhow::Result<String> {
        let response: PropResponse = self
            .send_json(
                self.http
                    .get(format!("{}/api/config/prop", self.endpoint))
                    .query(&[("path", path)]),
                &format!("read config property {path}"),
            )
            .await?;
        response
            .value
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow!("ZeroClaw config property '{path}' is not a string"))
    }

    async fn put_prop(&self, path: &str, value: &str) -> anyhow::Result<()> {
        let _: PropResponse = self
            .send_json(
                self.http
                    .put(format!("{}/api/config/prop", self.endpoint))
                    .json(&json!({ "path": path, "value": value })),
                &format!("write config property {path}"),
            )
            .await?;
        Ok(())
    }

    async fn reload(&self) -> anyhow::Result<()> {
        let response = self
            .request(self.http.post(format!("{}/admin/reload", self.endpoint)))
            .send()
            .await
            .context("ZeroClaw reload request failed")?;
        if response.status() != StatusCode::OK {
            return Err(anyhow!("ZeroClaw reload returned {}", response.status()));
        }
        Ok(())
    }

    async fn configured_providers(&self) -> anyhow::Result<Vec<ConfiguredProvider>> {
        let response: ConfigListResponse = self
            .send_json(
                self.http
                    .get(format!("{}/api/config/list", self.endpoint))
                    .query(&[("prefix", "providers.models")]),
                "list configured providers",
            )
            .await?;
        Ok(parse_configured_providers(response.entries))
    }

    async fn model_catalog(&self, family: &str) -> anyhow::Result<ModelCatalogResponse> {
        self.send_json(
            self.http
                .get(format!("{}/api/config/catalog/models", self.endpoint))
                .query(&[("model_provider", family)]),
            &format!("list {family} models"),
        )
        .await
    }

    async fn current_provider(&self) -> anyhow::Result<ConfiguredProvider> {
        let provider_ref = self
            .get_prop(&format!("agents.{}.model_provider", self.agent_alias))
            .await?;
        self.configured_providers()
            .await?
            .into_iter()
            .find(|provider| provider.reference == provider_ref)
            .ok_or_else(|| {
                anyhow!(
                    "ZeroClaw agent '{}' selects unconfigured provider '{}'",
                    self.agent_alias,
                    provider_ref
                )
            })
    }

    pub(crate) async fn project_state(
        &self,
        mut state: TchedRouterState,
    ) -> anyhow::Result<TchedRouterState> {
        self.health().await?;
        let configured = self.configured_providers().await?;
        let selected_ref = self
            .get_prop(&format!("agents.{}.model_provider", self.agent_alias))
            .await?;
        let selected = configured
            .iter()
            .find(|provider| provider.reference == selected_ref)
            .ok_or_else(|| anyhow!("ZeroClaw selects unknown provider '{selected_ref}'"))?;

        let mut providers = Vec::with_capacity(configured.len());
        let mut routes = Vec::new();
        for provider in &configured {
            let catalog = self.model_catalog(&provider.family).await?;
            providers.push(Provider {
                id: provider.family.clone(),
                route: provider.reference.clone(),
                kind: "zeroclaw-runtime".to_string(),
                aliases: vec![provider.reference.clone()],
                sdk: "zeroclaw".to_string(),
                description: format!(
                    "Configured ZeroClaw provider alias '{}'.",
                    provider.reference
                ),
                ..Default::default()
            });
            for model in catalog.models {
                routes.push(ModelRoute {
                    hint: "runtime".to_string(),
                    provider: provider.family.clone(),
                    upstream_provider: provider.family.clone(),
                    transport: "zeroclaw-loopback".to_string(),
                    model,
                    kind: "chat".to_string(),
                    status: if catalog.live {
                        "available".to_string()
                    } else {
                        "catalogued".to_string()
                    },
                    available: catalog.live,
                    status_reason: if catalog.live {
                        "Discovered through the live ZeroClaw model catalog.".to_string()
                    } else {
                        "Returned by ZeroClaw's cached model catalog.".to_string()
                    },
                    api_key: Some(Value::Null),
                    ..Default::default()
                });
            }
        }

        state.status = "active".to_string();
        state.selected_provider = selected.family.clone();
        state.selected_model = selected.selected_model.clone();
        state.transport.grpc_target = self.endpoint.clone();
        state.catalog.providers = providers;
        state.catalog.model_routes = routes;
        state.catalog.router.provider = selected.family.clone();
        state.catalog.router.model = selected.selected_model.clone();
        state.catalog.router.endpoint = self.endpoint.clone();
        state.catalog.router.role = "zeroclaw-runtime-authority".to_string();
        Ok(state)
    }

    pub(crate) async fn set_provider(&self, provider_id: &str) -> anyhow::Result<RuntimeSelection> {
        self.health().await?;
        let provider = self
            .configured_providers()
            .await?
            .into_iter()
            .find(|provider| {
                provider.family.eq_ignore_ascii_case(provider_id)
                    || provider.reference.eq_ignore_ascii_case(provider_id)
            })
            .ok_or_else(|| anyhow!("ZeroClaw provider '{provider_id}' is not configured"))?;
        self.put_prop(
            &format!("agents.{}.model_provider", self.agent_alias),
            &provider.reference,
        )
        .await?;
        self.reload().await?;
        Ok(RuntimeSelection {
            provider: provider.family,
            model: provider.selected_model,
        })
    }

    pub(crate) async fn set_model(&self, model_id: &str) -> anyhow::Result<String> {
        self.health().await?;
        let provider = self.current_provider().await?;
        let catalog = self.model_catalog(&provider.family).await?;
        if !catalog.models.iter().any(|model| model == model_id) {
            return Err(anyhow!(
                "ZeroClaw model '{model_id}' is not present in the live '{}' catalog",
                provider.family
            ));
        }
        self.put_prop(
            &format!("providers.models.{}.model", provider.reference),
            model_id,
        )
        .await?;
        self.reload().await?;
        Ok(model_id.to_string())
    }

    pub(crate) async fn chat(
        &self,
        state: &TchedRouterState,
        input: ChatInput,
    ) -> anyhow::Result<ChatOutput> {
        self.health().await?;
        if !input.provider.trim().is_empty()
            && !input
                .provider
                .eq_ignore_ascii_case(&state.selected_provider)
        {
            return Err(anyhow!(
                "requested provider '{}' differs from ZeroClaw's selected provider '{}'",
                input.provider,
                state.selected_provider
            ));
        }
        if !input.model.trim().is_empty()
            && !input.model.eq_ignore_ascii_case(&state.selected_model)
        {
            return Err(anyhow!(
                "requested model '{}' differs from ZeroClaw's selected model '{}'",
                input.model,
                state.selected_model
            ));
        }

        let prompt = conversation_prompt(&input)?;
        let request_id = uuid::Uuid::new_v4().to_string();
        let response: Value = self
            .send_json(
                self.http
                    .post(format!("{}/a2a/{}", self.endpoint, self.agent_alias))
                    .json(&json!({
                        "jsonrpc": "2.0",
                        "id": request_id,
                        "method": "message/send",
                        "params": {
                            "message": {
                                "role": "user",
                                "parts": [{ "kind": "text", "text": prompt }]
                            }
                        }
                    })),
                "chat",
            )
            .await?;
        if let Some(error) = response.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown ZeroClaw A2A error");
            return Err(anyhow!("ZeroClaw chat failed: {}", truncate_error(message)));
        }
        let content = extract_chat_text(&response)?;
        Ok(ChatOutput {
            content,
            provider: state.selected_provider.clone(),
            model: state.selected_model.clone(),
            finish_reason: "stop".to_string(),
            usage: Value::Null,
        })
    }
}

fn parse_configured_providers(entries: Vec<ConfigListEntry>) -> Vec<ConfiguredProvider> {
    let mut providers = BTreeMap::new();
    for entry in entries {
        let Some(path) = entry.path.strip_prefix("providers.models.") else {
            continue;
        };
        let Some(reference) = path.strip_suffix(".model") else {
            continue;
        };
        let Some((family, _alias)) = reference.split_once('.') else {
            continue;
        };
        let selected_model = entry
            .value
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default();
        providers.insert(
            reference.to_string(),
            ConfiguredProvider {
                family: family.to_string(),
                reference: reference.to_string(),
                selected_model,
            },
        );
    }
    providers.into_values().collect()
}

fn conversation_prompt(input: &ChatInput) -> anyhow::Result<String> {
    if input.messages.is_empty() {
        if input.message.trim().is_empty() {
            return Err(anyhow!("zeroclaw.Chat requires message or messages"));
        }
        return Ok(input.message.clone());
    }
    let mut prompt = String::from(
        "Continue the following ordered conversation. Respond only as the assistant.\n\n",
    );
    for message in &input.messages {
        prompt.push('[');
        prompt.push_str(message.role.trim());
        prompt.push_str("]\n");
        prompt.push_str(&message.content);
        prompt.push_str("\n\n");
    }
    Ok(prompt)
}

fn extract_chat_text(response: &Value) -> anyhow::Result<String> {
    response
        .pointer("/result/artifacts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|artifact| {
            artifact
                .get("parts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .find_map(|part| {
            (part.get("kind").and_then(Value::as_str) == Some("text"))
                .then(|| part.get("text").and_then(Value::as_str))
                .flatten()
        })
        .map(str::to_string)
        .ok_or_else(|| anyhow!("ZeroClaw chat response contains no text artifact"))
}

fn truncate_error(detail: &str) -> String {
    const LIMIT: usize = 512;
    let mut end = detail.len().min(LIMIT);
    while end > 0 && !detail.is_char_boundary(end) {
        end -= 1;
    }
    let suffix = if end < detail.len() { "…" } else { "" };
    format!("{}{}", &detail[..end], suffix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_plugins::state_plugins::tched_router::TchedChatMessage;

    #[test]
    fn configured_provider_catalog_is_derived_from_runtime_paths() {
        let providers = parse_configured_providers(vec![
            ConfigListEntry {
                path: "providers.models.opencode.go.model".to_string(),
                value: Some(json!("deepseek-v4-flash-free")),
            },
            ConfigListEntry {
                path: "providers.models.opencode.go.api_key".to_string(),
                value: None,
            },
        ]);
        assert_eq!(
            providers,
            vec![ConfiguredProvider {
                family: "opencode".to_string(),
                reference: "opencode.go".to_string(),
                selected_model: "deepseek-v4-flash-free".to_string(),
            }]
        );
    }

    #[test]
    fn conversation_history_is_forwarded_in_order() {
        let input = ChatInput {
            messages: vec![
                TchedChatMessage {
                    role: "user".to_string(),
                    content: "first".to_string(),
                },
                TchedChatMessage {
                    role: "assistant".to_string(),
                    content: "second".to_string(),
                },
            ],
            ..Default::default()
        };
        let prompt = conversation_prompt(&input).unwrap();
        assert!(
            prompt.find("[user]\nfirst").unwrap() < prompt.find("[assistant]\nsecond").unwrap()
        );
    }

    #[test]
    fn a2a_text_artifact_is_extracted() {
        let response = json!({
            "result": { "artifacts": [{ "parts": [{ "kind": "text", "text": "PONG" }] }] }
        });
        assert_eq!(extract_chat_text(&response).unwrap(), "PONG");
    }
}
