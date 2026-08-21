//! Loopback adapter for the real ZeroClaw daemon.
//!
//! ZeroClaw owns provider/model discovery, chat execution, and per-agent
//! specialization (skill/knowledge/MCP bundles, risk profile, workspace access,
//! identity). The bridge remains the capability and audit boundary; it does not
//! implement another model provider.
//!
//! Agent resolution is deliberately internal. Callers choose a provider/model
//! pair; they never name an agent. The pair resolves to the task-specialized
//! agent for that provider and executes through ZeroClaw's one-shot agent
//! surface with explicit provider/model overrides. Selection never rewrites
//! shared daemon config, and catalog models cannot silently fall back to the
//! model pinned in an A2A agent entry.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, Context};
use op_plugins::state_plugins::common::llm_projection::{ModelRoute, Provider};
use op_plugins::state_plugins::tched_router::{ChatInput, ChatOutput, TchedRouterState};
use reqwest::RequestBuilder;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8084";
const DEFAULT_ZEROCLAW_BIN: &str = "/usr/bin/zeroclaw";
const DEFAULT_ZEROCLAW_CONFIG_DIR: &str = "/var/lib/tched-router";
const SALAD_PROVIDER_REF: &str = "custom.salad";

/// Agent used only when a caller specifies neither model nor provider. It is a
/// last resort, not "the chat agent": any configured agent is reachable by
/// naming a model it serves.
const FALLBACK_AGENT_ALIAS: &str = "dashboard";

#[derive(Clone, Debug)]
pub(crate) struct TchedRouterRuntimeClient {
    http: reqwest::Client,
    endpoint: String,
    fallback_agent: String,
    token: Option<String>,
    zeroclaw_bin: PathBuf,
    zeroclaw_config_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConfiguredProvider {
    family: String,
    reference: String,
    selected_model: String,
    uri: Option<String>,
}

/// One task-oriented agent as the daemon reports it. `skill_bundles` is the
/// specialization: two agents with different bundles are different tasks even
/// when they share a provider.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ConfiguredAgent {
    alias: String,
    provider_ref: String,
    family: String,
    model: String,
    enabled: bool,
    skill_bundles: Vec<String>,
}

fn provider_selection_id(family: &str, reference: &str) -> String {
    if family == "custom" {
        reference
            .strip_prefix("custom.")
            .unwrap_or(reference)
            .to_string()
    } else {
        family.to_string()
    }
}

fn configured_provider_id(provider: &ConfiguredProvider) -> String {
    provider_selection_id(&provider.family, &provider.reference)
}

fn configured_agent_provider_id(agent: &ConfiguredAgent) -> String {
    provider_selection_id(&agent.family, &agent.provider_ref)
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
struct ConfigListResponse {
    entries: Vec<ConfigListEntry>,
}

#[derive(Debug, Deserialize)]
struct ConfigListEntry {
    path: String,
    #[serde(default)]
    value: Option<Value>,
}

/// Structured error body from the daemon's config surface. `code` is a stable
/// identifier (`path_not_found`, `validation_failed`, `config_changed_externally`,
/// …) meant for programmatic matching.
#[derive(Debug, Deserialize)]
struct ConfigApiError {
    code: String,
    message: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    op_index: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
struct ModelCatalogResponse {
    #[serde(default)]
    models: Vec<String>,
    #[serde(default)]
    live: bool,
}

impl TchedRouterRuntimeClient {
    pub(crate) fn from_env() -> Self {
        let endpoint = std::env::var("ZEROCLAW_RUNTIME_ENDPOINT")
            .unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string());
        let fallback_agent = std::env::var("ZEROCLAW_RUNTIME_AGENT")
            .unwrap_or_else(|_| FALLBACK_AGENT_ALIAS.to_string());
        let token = std::env::var("ZEROCLAW_RUNTIME_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let mut client = Self::new(endpoint, fallback_agent, token);
        client.zeroclaw_bin = std::env::var_os("ZEROCLAW_RUNTIME_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_ZEROCLAW_BIN));
        client.zeroclaw_config_dir = std::env::var_os("ZEROCLAW_RUNTIME_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_ZEROCLAW_CONFIG_DIR));
        client
    }

    pub(crate) fn new(endpoint: String, fallback_agent: String, token: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(600))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            http,
            endpoint: endpoint.trim_end_matches('/').to_string(),
            fallback_agent,
            token,
            zeroclaw_bin: PathBuf::from(DEFAULT_ZEROCLAW_BIN),
            zeroclaw_config_dir: PathBuf::from(DEFAULT_ZEROCLAW_CONFIG_DIR),
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
            // The config surface answers with a structured `ConfigApiError`
            // carrying a stable `code`. Its own schema says never to invent
            // codes at call sites, so propagate the daemon's code instead of
            // re-deriving one from message text: this is the single error
            // vocabulary for anything the daemon rejects.
            if let Ok(api) = serde_json::from_slice::<ConfigApiError>(&body) {
                let mut message = format!(
                    "ZeroClaw {operation} failed [{}]: {}",
                    api.code,
                    truncate_error(&api.message)
                );
                if let Some(path) = api.path.as_deref().filter(|path| !path.is_empty()) {
                    message.push_str(&format!(" (path '{path}')"));
                }
                if let Some(index) = api.op_index {
                    message.push_str(&format!(" (patch op {index})"));
                }
                return Err(anyhow!(message));
            }
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

    async fn config_entries(&self, prefix: &str) -> anyhow::Result<Vec<ConfigListEntry>> {
        let response: ConfigListResponse = self
            .send_json(
                self.http
                    .get(format!("{}/api/config/list", self.endpoint))
                    .query(&[("prefix", prefix)]),
                &format!("list config under {prefix}"),
            )
            .await?;
        Ok(response.entries)
    }

    async fn configured_providers(&self) -> anyhow::Result<Vec<ConfiguredProvider>> {
        Ok(parse_configured_providers(
            self.config_entries("providers.models").await?,
        ))
    }

    /// Enumerate the agents the daemon actually has, joined against the provider
    /// each one pins. This is the routing table; it is read from the daemon
    /// rather than declared in the bridge so adding an agent to zeroclaw's
    /// config makes it reachable with no code change.
    async fn configured_agents(&self) -> anyhow::Result<Vec<ConfiguredAgent>> {
        let providers = self.configured_providers().await?;
        Ok(parse_configured_agents(
            self.config_entries("agents").await?,
            &providers,
        ))
    }

    /// Cumulative cost/token counters. Used to report real `usage` instead of
    /// a null placeholder.
    async fn cost_snapshot(&self) -> Option<Value> {
        let value: Value = self
            .send_json(self.http.get(format!("{}/api/cost", self.endpoint)), "cost")
            .await
            .ok()?;
        value.get("cost").cloned()
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

    async fn provider_catalog(
        &self,
        provider: &ConfiguredProvider,
    ) -> anyhow::Result<ModelCatalogResponse> {
        let mut catalog = if provider.reference == SALAD_PROVIDER_REF {
            self.salad_model_catalog(provider).await.unwrap_or_default()
        } else {
            self.model_catalog(&provider.family)
                .await
                .unwrap_or_default()
        };
        if catalog.models.is_empty() && !provider.selected_model.is_empty() {
            catalog.models.push(provider.selected_model.clone());
        }
        Ok(catalog)
    }

    async fn salad_model_catalog(
        &self,
        provider: &ConfiguredProvider,
    ) -> anyhow::Result<ModelCatalogResponse> {
        let endpoint = provider
            .uri
            .as_deref()
            .context("custom.salad has no configured URI")?
            .trim_end_matches('/');
        let key = std::env::var("SALAD_API_KEY").context("SALAD_API_KEY is not configured")?;
        let response: Value = self
            .send_json(
                self.http.get(format!("{endpoint}/models")).bearer_auth(key),
                "list custom.salad models",
            )
            .await?;
        let mut models = response
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|model| model.get("id").and_then(Value::as_str))
            .map(str::to_string)
            .collect::<Vec<_>>();
        models.sort();
        models.dedup();
        Ok(ModelCatalogResponse { models, live: true })
    }

    /// Retry a Salad completion without Qwen's separate reasoning channel.
    ///
    /// Salad names that channel `reasoning`, while the currently installed
    /// ZeroClaw compatible-provider parser recognizes `reasoning_content`.
    /// Some prompts therefore complete successfully with empty stdout. Keep
    /// ZeroClaw as the primary agent surface, but make that provider-specific
    /// response-shape mismatch recoverable without exposing chain-of-thought.
    async fn salad_visible_completion(
        &self,
        provider: &ConfiguredProvider,
        model: &str,
        input: &ChatInput,
    ) -> anyhow::Result<String> {
        let endpoint = provider
            .uri
            .as_deref()
            .context("custom.salad has no configured URI")?
            .trim_end_matches('/');
        let key = std::env::var("SALAD_API_KEY").context("SALAD_API_KEY is not configured")?;
        let messages = if input.messages.is_empty() {
            json!([{ "role": "user", "content": input.message }])
        } else {
            serde_json::to_value(&input.messages).context("serialize Salad chat history")?
        };
        let response = self
            .http
            .post(format!("{endpoint}/chat/completions"))
            .bearer_auth(key)
            .json(&json!({
                "model": model,
                "messages": messages,
                "chat_template_kwargs": { "enable_thinking": false }
            }))
            .send()
            .await
            .context("Salad visible-completion retry failed")?;
        let status = response.status();
        let body = response
            .bytes()
            .await
            .context("Salad visible-completion response body failed")?;
        if !status.is_success() {
            return Err(anyhow!(
                "Salad visible-completion retry returned {status}: {}",
                truncate_error(&String::from_utf8_lossy(&body))
            ));
        }
        let response: Value = serde_json::from_slice(&body)
            .context("Salad visible-completion retry returned invalid JSON")?;
        response
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|content| !content.is_empty())
            .map(str::to_string)
            .context("Salad visible-completion retry returned no visible content")
    }

    /// Resolve the agent that serves a requested model/provider. Callers never
    /// pass an agent; this is the whole point of the indirection.
    fn resolve_agent(
        agents: &[ConfiguredAgent],
        fallback_agent: &str,
        provider_hint: &str,
        model_hint: &str,
    ) -> anyhow::Result<ConfiguredAgent> {
        let enabled = || agents.iter().filter(|agent| agent.enabled);

        let model_hint = model_hint.trim();
        if !model_hint.is_empty() {
            if let Some(agent) =
                enabled().find(|agent| agent.model.eq_ignore_ascii_case(model_hint))
            {
                return Ok(agent.clone());
            }
        }

        let provider_hint = provider_hint.trim();
        if !provider_hint.is_empty() {
            if let Some(agent) = enabled().find(|agent| {
                agent.family.eq_ignore_ascii_case(provider_hint)
                    || agent.provider_ref.eq_ignore_ascii_case(provider_hint)
                    || configured_agent_provider_id(agent).eq_ignore_ascii_case(provider_hint)
                    || agent.alias.eq_ignore_ascii_case(provider_hint)
            }) {
                return Ok(agent.clone());
            }
        }

        if model_hint.is_empty() && provider_hint.is_empty() {
            if let Some(agent) = enabled().find(|agent| agent.alias == fallback_agent) {
                return Ok(agent.clone());
            }
            if let Some(agent) = enabled().next() {
                return Ok(agent.clone());
            }
            return Err(anyhow!("ZeroClaw reports no enabled agent"));
        }

        // Surface what *is* reachable rather than only what failed: the old
        // error told callers their choice "differs from the selected model"
        // without saying what they could have asked for.
        let mut available: Vec<String> = enabled()
            .map(|agent| format!("{} ({})", agent.model, agent.family))
            .collect();
        available.sort();
        available.dedup();
        Err(anyhow!(
            "no enabled ZeroClaw agent serves provider '{}' model '{}'; reachable models: {}",
            provider_hint,
            model_hint,
            if available.is_empty() {
                "none".to_string()
            } else {
                available.join(", ")
            }
        ))
    }

    pub(crate) async fn project_state(
        &self,
        mut state: TchedRouterState,
    ) -> anyhow::Result<TchedRouterState> {
        self.health().await?;
        let agents = self.configured_agents().await?;
        if agents.is_empty() {
            return Err(anyhow!("ZeroClaw reports no configured agents"));
        }
        let configured = self.configured_providers().await?;

        // Retain the dashboard's last validated selection when refreshing the
        // projection. Resolving unconditionally with empty hints chooses the
        // configured fallback agent and makes every GetRouter call undo a
        // successful SetProvider/SetModel/SetSelection mutation.
        let mut selected = Self::resolve_agent(
            &agents,
            &self.fallback_agent,
            &state.selected_provider,
            &state.selected_model,
        )
        .or_else(|_| Self::resolve_agent(&agents, &self.fallback_agent, "", ""))?;

        let mut providers = Vec::with_capacity(configured.len());
        for provider in &configured {
            providers.push(Provider {
                id: configured_provider_id(provider),
                route: provider.reference.clone(),
                kind: "tched_router-runtime".to_string(),
                aliases: vec![provider.family.clone()],
                sdk: "tched_router".to_string(),
                description: format!(
                    "Configured ZeroClaw provider alias '{}'.",
                    provider.reference
                ),
                ..Default::default()
            });
        }

        // Fetch each provider family's catalog once. Agents commonly share a
        // family, and the previous code re-fetched per agent.
        let mut catalogs: std::collections::BTreeMap<String, ModelCatalogResponse> =
            std::collections::BTreeMap::new();
        for provider in &configured {
            if agents
                .iter()
                .any(|agent| agent.enabled && agent.provider_ref == provider.reference)
            {
                if let Ok(catalog) = self.provider_catalog(provider).await {
                    catalogs.insert(configured_provider_id(provider), catalog);
                }
            }
        }

        // Preserve a catalog model selected for the current provider.
        if agents.iter().any(|agent| {
            agent.enabled && configured_agent_provider_id(agent) == state.selected_provider
        }) && catalogs
            .get(&state.selected_provider)
            .is_some_and(|catalog| {
                catalog
                    .models
                    .iter()
                    .any(|model| model == &state.selected_model)
            })
        {
            selected.model = state.selected_model.clone();
        }

        // One route per enabled agent: the agent is the thing that can actually
        // serve the model, so the route set and the reachable set are the same
        // set by construction.
        let mut routes = Vec::new();
        for agent in agents.iter().filter(|agent| agent.enabled) {
            // `catalogs` is keyed by the dashboard-facing provider id (for
            // example `salad`), not ZeroClaw's internal provider reference
            // (`custom.salad`). Looking up the latter marked pinned models
            // unavailable and then suppressed their live catalog duplicate.
            let provider_id = configured_agent_provider_id(agent);
            let catalog = catalogs.get(&provider_id);
            let listed = catalog
                .map(|catalog| catalog.models.iter().any(|model| model == &agent.model))
                .unwrap_or(false);
            let live = catalog.map(|catalog| catalog.live).unwrap_or(false);
            routes.push(ModelRoute {
                hint: if agent.skill_bundles.is_empty() {
                    "runtime".to_string()
                } else {
                    agent.skill_bundles.join("+")
                },
                provider: provider_id,
                upstream_provider: agent.family.clone(),
                transport: "tched_router-agent".to_string(),
                model: agent.model.clone(),
                kind: "chat".to_string(),
                status: if listed {
                    "available".to_string()
                } else {
                    "configured".to_string()
                },
                available: listed && live,
                status_reason: format!(
                    "Served by ZeroClaw agent '{}' (provider {}); {}.",
                    agent.alias,
                    agent.provider_ref,
                    match (listed, live) {
                        (true, true) => "present in the live model catalog",
                        (true, false) => "present in the cached catalog only",
                        (false, _) => "configured but not reported by the provider catalog",
                    }
                ),
                api_key: None,
                ..Default::default()
            });
        }

        // Every model the provider catalog reports is selectable, not just the
        // one each agent pins. Without this the catalog was fetched and thrown
        // away except for the `listed`/`live` booleans above, so a provider
        // offering 90+ models surfaced only the handful named by agents.
        // Agent-served models already have a richer route and are not duplicated.
        for (provider_id, catalog) in &catalogs {
            let provider = configured
                .iter()
                .find(|provider| configured_provider_id(provider) == *provider_id);
            let family = provider
                .map(|provider| provider.family.as_str())
                .unwrap_or(provider_id);
            for model in &catalog.models {
                if routes
                    .iter()
                    .any(|route| &route.provider == provider_id && &route.model == model)
                {
                    continue;
                }
                routes.push(ModelRoute {
                    hint: "catalog".to_string(),
                    provider: provider_id.clone(),
                    upstream_provider: family.to_string(),
                    transport: "tched_router-catalog".to_string(),
                    model: model.clone(),
                    kind: "chat".to_string(),
                    status: if catalog.live {
                        "available".to_string()
                    } else {
                        "configured".to_string()
                    },
                    available: catalog.live,
                    status_reason: format!(
                        "Reported by the '{}' provider catalog; {}.",
                        provider_id,
                        if catalog.live {
                            "live"
                        } else {
                            "cached catalog only"
                        }
                    ),
                    api_key: None,
                    ..Default::default()
                });
            }
        }

        state.status = "active".to_string();
        state.selected_provider = configured_agent_provider_id(&selected);
        state.selected_model = selected.model.clone();
        state.transport.grpc_target = self.endpoint.clone();
        state.projection.providers = providers;
        state.projection.model_routes = routes;
        state.projection.router.provider = configured_agent_provider_id(&selected);
        state.projection.router.model = selected.model.clone();
        state.projection.router.endpoint = self.endpoint.clone();
        state.projection.router.role = "tched_router-runtime-authority".to_string();
        Ok(state)
    }

    /// Change the *default* provider for callers who name neither model nor
    /// provider. This no longer rewrites `agents.<alias>.model_provider` and no
    /// longer calls `/admin/reload`: rewriting shared daemon config to satisfy
    /// one caller is what made concurrent callers with different models fight
    /// and let a reload land on an in-flight request.
    pub(crate) async fn set_provider(&self, provider_id: &str) -> anyhow::Result<RuntimeSelection> {
        self.health().await?;
        let agents = self.configured_agents().await?;
        let agent = Self::resolve_agent(&agents, &self.fallback_agent, provider_id, "")?;
        Ok(RuntimeSelection {
            provider: configured_agent_provider_id(&agent),
            model: agent.model,
        })
    }

    /// Change the *default* model. Validated against what an enabled agent can
    /// actually serve; no daemon mutation, for the same reason as
    /// [`set_provider`](Self::set_provider).
    pub(crate) async fn set_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> anyhow::Result<RuntimeSelection> {
        self.health().await?;
        let agents = self.configured_agents().await?;
        let provider = Self::resolve_agent(&agents, &self.fallback_agent, provider_id, "")?;
        let configured = self.configured_providers().await?;
        let provider = configured
            .iter()
            .find(|candidate| candidate.reference == provider.provider_ref)
            .context("resolved agent references an unconfigured provider")?;
        let catalog = self.provider_catalog(provider).await?;
        if !catalog.models.iter().any(|model| model == model_id) {
            return Err(anyhow!(
                "model {} is not in the {} provider catalog",
                model_id,
                provider.reference
            ));
        }
        Ok(RuntimeSelection {
            provider: configured_provider_id(provider),
            model: model_id.to_string(),
        })
    }

    /// Validate and return a provider/model pair as one indivisible selection.
    pub(crate) async fn set_selection(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> anyhow::Result<RuntimeSelection> {
        self.health().await?;
        let providers = self.configured_providers().await?;
        let provider = providers
            .iter()
            .find(|provider| {
                provider.family.eq_ignore_ascii_case(provider_id)
                    || provider.reference.eq_ignore_ascii_case(provider_id)
                    || configured_provider_id(provider).eq_ignore_ascii_case(provider_id)
            })
            .with_context(|| format!("provider '{provider_id}' is not configured"))?;
        let catalog = self.provider_catalog(provider).await?;
        if !catalog.models.iter().any(|model| model == model_id) {
            return Err(anyhow!(
                "model '{model_id}' is not in the '{}' provider catalog",
                provider.reference
            ));
        }
        Ok(RuntimeSelection {
            provider: configured_provider_id(provider),
            model: model_id.to_string(),
        })
    }

    pub(crate) async fn chat(
        &self,
        state: &TchedRouterState,
        input: ChatInput,
    ) -> anyhow::Result<ChatOutput> {
        self.health().await?;
        let agents = self.configured_agents().await?;

        // Fall back to the projected default only when the caller named
        // nothing. A caller naming a model no longer has to match whatever the
        // daemon most recently had selected.
        let provider_hint = if input.provider.trim().is_empty() && input.model.trim().is_empty() {
            state.selected_provider.as_str()
        } else {
            input.provider.as_str()
        };
        let agent =
            Self::resolve_agent(&agents, &self.fallback_agent, provider_hint, &input.model)?;

        let prompt = conversation_prompt(&input)?;
        let requested_model = if input.model.trim().is_empty() {
            agent.model.as_str()
        } else {
            input.model.trim()
        };

        // A2A dispatch contains no provider/model override. Sending a catalog
        // selection to `/a2a/{alias}` therefore executes the model pinned in
        // the agent config and can truthfully return a different vendor. The
        // official one-shot agent surface accepts both overrides and uses the
        // same sealed config/auth profiles, so every request executes exactly
        // the pair the capability boundary already validated.
        // SAFETY: `geteuid` has no preconditions and does not dereference memory.
        let running_as_root = unsafe { libc::geteuid() } == 0;
        let mut command = if running_as_root {
            let mut command = tokio::process::Command::new("/usr/bin/chpst");
            command.args(["-u", "jeremy:jeremy"]);
            command.arg(&self.zeroclaw_bin);
            command
        } else {
            tokio::process::Command::new(&self.zeroclaw_bin)
        };
        command
            .env("HOME", self.zeroclaw_config_dir.join("home"))
            .env("USER", "jeremy")
            .env("ZEROCLAW_CONFIG_DIR", &self.zeroclaw_config_dir)
            .arg("agent")
            .arg("--config-dir")
            .arg(&self.zeroclaw_config_dir)
            .arg("--agent")
            .arg(&agent.alias)
            .arg("--model-provider")
            .arg(&agent.provider_ref)
            .arg("--model")
            .arg(requested_model)
            .arg("--message")
            .arg(prompt)
            .arg("--log-level")
            .arg("error")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if agent.provider_ref == SALAD_PROVIDER_REF {
            if let Ok(key) = std::env::var("SALAD_API_KEY") {
                command.env("API_KEY", key);
            }
        }
        let output = tokio::time::timeout(Duration::from_secs(600), command.output())
            .await
            .context("ZeroClaw exact-model chat timed out")?
            .context("failed to start ZeroClaw exact-model chat")?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "ZeroClaw exact-model chat failed: {}",
                truncate_error(&detail)
            ));
        }
        let mut content = String::from_utf8(output.stdout)
            .context("ZeroClaw exact-model chat returned non-UTF-8 output")?
            .trim()
            .to_string();
        if content.is_empty() && agent.provider_ref == SALAD_PROVIDER_REF {
            let providers = self.configured_providers().await?;
            let provider = providers
                .iter()
                .find(|provider| provider.reference == agent.provider_ref)
                .context("ZeroClaw Salad agent references an unconfigured provider")?;
            content = self
                .salad_visible_completion(provider, requested_model, &input)
                .await?;
        } else if content.is_empty() {
            return Err(anyhow!(
                "ZeroClaw exact-model chat returned an empty response"
            ));
        }

        Ok(ChatOutput {
            content,
            provider: configured_agent_provider_id(&agent),
            model: requested_model.to_string(),
            finish_reason: "stop".to_string(),
            usage: self.usage_for(&agent).await,
        })
    }

    /// Real counters from `/api/cost`, explicitly labelled cumulative. A
    /// per-request delta would be wrong under concurrency, so the scope is
    /// stated rather than implied.
    async fn usage_for(&self, agent: &ConfiguredAgent) -> Value {
        let Some(cost) = self.cost_snapshot().await else {
            return json!({
                "scope": "unavailable",
                "reason": "ZeroClaw /api/cost did not answer",
            });
        };
        json!({
            "scope": "cumulative",
            "agent": agent.alias,
            "model": agent.model,
            "total_tokens": cost.get("total_tokens").cloned().unwrap_or(Value::Null),
            "request_count": cost.get("request_count").cloned().unwrap_or(Value::Null),
            "by_agent": cost.pointer(&format!("/by_agent/{}", agent.alias)).cloned(),
            "by_model": cost.pointer(&format!("/by_model/{}", agent.model)).cloned(),
        })
    }

    // ── Config surface ──────────────────────────────────────────────────────

    async fn get_prop(&self, path: &str) -> anyhow::Result<Value> {
        self.send_json(
            self.http
                .get(format!("{}/api/config/prop", self.endpoint))
                .query(&[("path", path)]),
            &format!("read config property '{path}'"),
        )
        .await
    }

    async fn put_prop(&self, path: &str, value: Value) -> anyhow::Result<Value> {
        self.send_json(
            self.http
                .put(format!("{}/api/config/prop", self.endpoint))
                .json(&json!({ "path": path, "value": value })),
            &format!("set config property '{path}'"),
        )
        .await
    }

    /// Apply an RFC 6902 patch document atomically.
    ///
    /// Every multi-property change routes through here. Creating an agent means
    /// writing several properties, and doing that as separate `PUT`s can leave
    /// the daemon holding an agent with no provider if one call fails. Note the
    /// daemon's `PatchOp` has no `from` member, so `move` is unavailable and a
    /// rename is an `add` plus a `remove` inside one atomic document.
    async fn patch_config(&self, ops: Vec<Value>) -> anyhow::Result<Value> {
        self.send_json(
            self.http
                .patch(format!("{}/api/config", self.endpoint))
                .json(&ops),
            "patch config",
        )
        .await
    }

    async fn get_json(&self, path: &str, operation: &str) -> anyhow::Result<Value> {
        self.send_json(
            self.http.get(format!("{}{}", self.endpoint, path)),
            operation,
        )
        .await
    }

    /// Execute the declared methods that the daemon actually backs.
    ///
    /// These are declared in the plugin schema but executed here, for the same
    /// reason `Chat` is: the plugin is a synchronous D-Bus surface that must not
    /// perform I/O, while this crate owns the daemon session, the capability
    /// check, and the audit chain.
    ///
    /// Every arm maps to a route verified against the running daemon's own
    /// OpenAPI document. Methods the daemon exposes no route for are not
    /// declared, so there is no arm that can only fail.
    pub(crate) async fn dispatch_runtime_method(
        &self,
        method: &str,
        json_args: &str,
    ) -> anyhow::Result<Value> {
        let args: Value = if json_args.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(json_args)
                .with_context(|| format!("{method} arguments are not valid JSON"))?
        };
        let required_str = |key: &str| -> anyhow::Result<String> {
            args.get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow!("{method} requires a non-empty string '{key}'"))
        };
        let optional_str = |key: &str| args.get(key).and_then(Value::as_str).map(str::to_string);

        self.health().await?;
        match method {
            "config_get" => self.get_prop(&required_str("path")?).await,
            "config_set" => {
                let path = required_str("path")?;
                let value = args
                    .get("value")
                    .cloned()
                    .ok_or_else(|| anyhow!("config_set requires 'value'"))?;
                self.put_prop(&path, value).await
            }
            "config_list" => {
                let prefix = optional_str("prefix").unwrap_or_default();
                let entries = self.config_entries(&prefix).await?;
                Ok(json!({
                    "prefix": prefix,
                    "entries": entries
                        .into_iter()
                        .map(|entry| json!({ "path": entry.path, "value": entry.value }))
                        .collect::<Vec<_>>(),
                }))
            }
            "config_patch" => {
                let ops = args
                    .get("operations")
                    .and_then(Value::as_array)
                    .cloned()
                    .ok_or_else(|| {
                        anyhow!("config_patch requires an 'operations' array of RFC 6902 ops")
                    })?;
                self.patch_config(ops).await
            }
            "config_init" => {
                self.send_json(
                    self.http
                        .post(format!("{}/api/config/init", self.endpoint))
                        .json(&args),
                    "initialize config",
                )
                .await
            }
            "config_migrate" => {
                self.send_json(
                    self.http
                        .post(format!("{}/api/config/migrate", self.endpoint))
                        .json(&args),
                    "migrate config",
                )
                .await
            }

            "agents_list" => {
                let agents = self.configured_agents().await?;
                Ok(json!({
                    "agents": agents
                        .iter()
                        .map(|agent| json!({
                            "alias": agent.alias,
                            "model_provider": agent.provider_ref,
                            "provider": agent.family,
                            "model": agent.model,
                            "enabled": agent.enabled,
                            "skill_bundles": agent.skill_bundles,
                        }))
                        .collect::<Vec<_>>(),
                }))
            }
            "agents_create" => {
                let alias = required_str("alias")?;
                let provider_ref = required_str("model_provider")?;
                // Reject a dangling provider reference before writing rather
                // than letting the agent exist un-servable.
                let providers = self.configured_providers().await?;
                if !providers
                    .iter()
                    .any(|provider| provider.reference == provider_ref)
                {
                    return Err(anyhow!(
                        "model_provider '{provider_ref}' is not configured; configured: {}",
                        providers
                            .iter()
                            .map(|provider| provider.reference.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                let mut agent = json!({
                    "enabled": args.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                    "model_provider": provider_ref,
                });
                // Specialization knobs, all native to the daemon's agent config.
                for key in ["skill_bundles", "knowledge_bundles", "mcp_bundles"] {
                    if let Some(list) = args.get(key).filter(|value| value.is_array()) {
                        agent[key] = list.clone();
                    }
                }
                for key in ["risk_profile", "runtime_profile"] {
                    if let Some(value) = optional_str(key) {
                        agent[key] = json!(value);
                    }
                }
                if let Some(published) = args.get("a2a_published").and_then(Value::as_bool) {
                    agent["a2a"] = json!({ "published": published });
                }
                self.patch_config(vec![json!({
                    "op": "add",
                    "path": format!("/agents/{alias}"),
                    "value": agent,
                })])
                .await
            }
            "agents_delete" => {
                let alias = required_str("alias")?;
                self.patch_config(vec![json!({
                    "op": "remove",
                    "path": format!("/agents/{alias}"),
                })])
                .await
            }
            "agents_rename" => {
                let from = required_str("from")?;
                let to = required_str("to")?;
                let existing = self.get_prop(&format!("agents.{from}")).await?;
                self.patch_config(vec![
                    json!({ "op": "add", "path": format!("/agents/{to}"), "value": existing }),
                    json!({ "op": "remove", "path": format!("/agents/{from}") }),
                ])
                .await
            }

            "providers_create" => {
                let reference = required_str("reference")?;
                let (family, alias) = reference.split_once('.').ok_or_else(|| {
                    anyhow!("provider reference must be '<family>.<alias>', got '{reference}'")
                })?;
                let mut provider = json!({});
                for key in ["model", "uri", "api_key_env"] {
                    if let Some(value) = optional_str(key) {
                        provider[key] = json!(value);
                    }
                }
                self.patch_config(vec![json!({
                    "op": "add",
                    "path": format!("/providers/models/{family}/{alias}"),
                    "value": provider,
                })])
                .await
            }
            "providers_delete" => {
                let reference = required_str("reference")?;
                let (family, alias) = reference.split_once('.').ok_or_else(|| {
                    anyhow!("provider reference must be '<family>.<alias>', got '{reference}'")
                })?;
                self.patch_config(vec![json!({
                    "op": "remove",
                    "path": format!("/providers/models/{family}/{alias}"),
                })])
                .await
            }
            "providers_rename" => {
                let from = required_str("from")?;
                let to = required_str("to")?;
                let (from_family, from_alias) = from.split_once('.').ok_or_else(|| {
                    anyhow!("provider reference must be '<family>.<alias>', got '{from}'")
                })?;
                let (to_family, to_alias) = to.split_once('.').ok_or_else(|| {
                    anyhow!("provider reference must be '<family>.<alias>', got '{to}'")
                })?;
                let existing = self
                    .get_prop(&format!("providers.models.{from_family}.{from_alias}"))
                    .await?;
                self.patch_config(vec![
                    json!({
                        "op": "add",
                        "path": format!("/providers/models/{to_family}/{to_alias}"),
                        "value": existing,
                    }),
                    json!({
                        "op": "remove",
                        "path": format!("/providers/models/{from_family}/{from_alias}"),
                    }),
                ])
                .await
            }

            "model_list" => {
                let family = match optional_str("provider") {
                    Some(provider) => provider,
                    None => {
                        Self::resolve_agent(
                            &self.configured_agents().await?,
                            &self.fallback_agent,
                            "",
                            "",
                        )?
                        .family
                    }
                };
                let catalog = self.model_catalog(&family).await?;
                Ok(json!({
                    "provider": family,
                    "models": catalog.models,
                    "live": catalog.live,
                }))
            }

            "cron_list" => self.get_json("/api/cron", "list cron jobs").await,
            "memory_list" => self.get_json("/api/memory", "list memory entries").await,
            "channel_list" | "channels_list" => {
                self.get_json("/api/channels", "list channels").await
            }

            other => Err(anyhow!(
                "'{other}' is not backed by a ZeroClaw route; it is not a declared method"
            )),
        }
    }
}

fn parse_configured_providers(entries: Vec<ConfigListEntry>) -> Vec<ConfiguredProvider> {
    #[derive(Default)]
    struct Partial {
        model: Option<String>,
        uri: Option<String>,
    }

    let mut partials: BTreeMap<String, Partial> = BTreeMap::new();
    for entry in entries {
        let Some(path) = entry.path.strip_prefix("providers.models.") else {
            continue;
        };
        let Some((reference, field)) = path.rsplit_once('.') else {
            continue;
        };
        if reference.split_once('.').is_none() {
            continue;
        }
        let value = entry
            .value
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default();
        let partial = partials.entry(reference.to_string()).or_default();
        match field {
            "model" => partial.model = Some(value),
            "uri" => partial.uri = Some(value),
            _ => {}
        }
    }
    partials
        .into_iter()
        .filter_map(|(reference, partial)| {
            let (family, _alias) = reference.split_once('.')?;
            Some(ConfiguredProvider {
                family: family.to_string(),
                reference,
                selected_model: partial.model.unwrap_or_default(),
                uri: partial.uri.filter(|uri| !uri.is_empty()),
            })
        })
        .collect()
}

/// Build the agent routing table from `agents.<alias>.*` entries.
///
/// The daemon reports unset optional values as the literal string `<unset>`,
/// and reports list values as a JSON-encoded string, so both are normalized
/// here rather than at every use site.
fn parse_configured_agents(
    entries: Vec<ConfigListEntry>,
    providers: &[ConfiguredProvider],
) -> Vec<ConfiguredAgent> {
    #[derive(Default)]
    struct Partial {
        provider_ref: Option<String>,
        enabled: Option<bool>,
        skill_bundles: Vec<String>,
    }

    let mut partials: BTreeMap<String, Partial> = BTreeMap::new();
    for entry in entries {
        let Some(rest) = entry.path.strip_prefix("agents.") else {
            continue;
        };
        let Some((alias, field)) = rest.split_once('.') else {
            continue;
        };
        let raw = entry.value.as_ref().and_then(|value| value.as_str());
        let raw = match raw {
            Some("<unset>") | None => None,
            Some(value) => Some(value),
        };
        let slot = partials.entry(alias.to_string()).or_default();
        match field {
            "model_provider" => slot.provider_ref = raw.map(str::to_string),
            "enabled" => slot.enabled = raw.map(|value| value.eq_ignore_ascii_case("true")),
            "skill_bundles" => {
                if let Some(value) = raw {
                    slot.skill_bundles = serde_json::from_str::<Vec<String>>(value)
                        .unwrap_or_else(|_| vec![value.to_string()]);
                }
            }
            _ => {}
        }
    }

    partials
        .into_iter()
        .filter_map(|(alias, partial)| {
            let provider_ref = partial.provider_ref?;
            let provider = providers
                .iter()
                .find(|provider| provider.reference == provider_ref)?;
            Some(ConfiguredAgent {
                alias,
                provider_ref: provider_ref.clone(),
                family: provider.family.clone(),
                model: provider.selected_model.clone(),
                enabled: partial.enabled.unwrap_or(true),
                skill_bundles: partial.skill_bundles,
            })
        })
        .collect()
}

fn conversation_prompt(input: &ChatInput) -> anyhow::Result<String> {
    if input.messages.is_empty() {
        if input.message.trim().is_empty() {
            return Err(anyhow!("tched_router.Chat requires message or messages"));
        }
        return Ok(input.message.clone());
    }

    // The one-shot agent surface carries one prompt, so a multi-turn
    // conversation is serialized into text. System turns are hoisted
    // ahead of the transcript so they read as instructions rather than as
    // another line of dialogue.
    let (system, dialogue): (Vec<_>, Vec<_>) = input
        .messages
        .iter()
        .partition(|message| message.role.trim().eq_ignore_ascii_case("system"));

    let mut prompt = String::new();
    for message in &system {
        prompt.push_str(message.content.trim());
        prompt.push_str("\n\n");
    }
    prompt.push_str(
        "Continue the following ordered conversation. Respond only as the assistant.\n\n",
    );
    for message in &dialogue {
        prompt.push('[');
        prompt.push_str(message.role.trim());
        prompt.push_str("]\n");
        prompt.push_str(&message.content);
        prompt.push_str("\n\n");
    }
    Ok(prompt)
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
    use op_plugins::state_plugins::tched_router::TchedRouterChatMessage;

    fn entry(path: &str, value: Option<&str>) -> ConfigListEntry {
        ConfigListEntry {
            path: path.to_string(),
            value: value.map(|value| json!(value)),
        }
    }

    fn sample_providers() -> Vec<ConfiguredProvider> {
        vec![
            ConfiguredProvider {
                family: "opencode".to_string(),
                reference: "opencode.go".to_string(),
                selected_model: "deepseek-v4-flash-free".to_string(),
                uri: None,
            },
            ConfiguredProvider {
                family: "custom".to_string(),
                reference: "custom.salad".to_string(),
                selected_model: "qwen3.6-35b-a3b".to_string(),
                uri: Some("https://ai.salad.cloud/v1".to_string()),
            },
        ]
    }

    fn sample_agents() -> Vec<ConfiguredAgent> {
        parse_configured_agents(
            vec![
                entry("agents.dashboard.model_provider", Some("opencode.go")),
                entry("agents.dashboard.enabled", Some("true")),
                entry("agents.dashboard.skill_bundles", Some("[\"default\"]")),
                entry("agents.salad_chat.model_provider", Some("custom.salad")),
                entry("agents.salad_chat.enabled", Some("true")),
                entry("agents.salad_chat.skill_bundles", Some("[\"default\"]")),
                entry("agents.retired.model_provider", Some("custom.salad")),
                entry("agents.retired.enabled", Some("false")),
            ],
            &sample_providers(),
        )
    }

    #[test]
    fn configured_provider_catalog_is_derived_from_runtime_paths() {
        let providers = parse_configured_providers(vec![
            entry(
                "providers.models.opencode.go.model",
                Some("deepseek-v4-flash-free"),
            ),
            entry("providers.models.opencode.go.api_key", None),
        ]);
        assert_eq!(
            providers,
            vec![ConfiguredProvider {
                family: "opencode".to_string(),
                reference: "opencode.go".to_string(),
                selected_model: "deepseek-v4-flash-free".to_string(),
                uri: None,
            }]
        );
    }

    #[test]
    fn agents_join_against_the_provider_they_pin() {
        let agents = sample_agents();
        let dashboard = agents
            .iter()
            .find(|agent| agent.alias == "dashboard")
            .unwrap();
        assert_eq!(dashboard.family, "opencode");
        assert_eq!(dashboard.model, "deepseek-v4-flash-free");
        assert_eq!(dashboard.skill_bundles, vec!["default".to_string()]);
    }

    #[test]
    fn unset_sentinel_does_not_become_a_provider_reference() {
        let agents = parse_configured_agents(
            vec![entry("agents.ghost.model_provider", Some("<unset>"))],
            &sample_providers(),
        );
        assert!(
            agents.is_empty(),
            "an unset provider is not a routable agent"
        );
    }

    #[test]
    fn a_named_model_selects_its_own_agent_not_the_default() {
        let agents = sample_agents();
        let resolved =
            TchedRouterRuntimeClient::resolve_agent(&agents, "dashboard", "", "qwen3.6-35b-a3b")
                .unwrap();
        assert_eq!(resolved.alias, "salad_chat");
    }

    #[test]
    fn empty_hints_fall_back_to_the_configured_default_agent() {
        let agents = sample_agents();
        let resolved =
            TchedRouterRuntimeClient::resolve_agent(&agents, "dashboard", "", "").unwrap();
        assert_eq!(resolved.alias, "dashboard");
    }

    #[test]
    fn disabled_agents_are_never_resolved() {
        let agents = sample_agents();
        // `retired` also serves custom.salad but is disabled, so the enabled
        // salad agent must win rather than the disabled one.
        let resolved =
            TchedRouterRuntimeClient::resolve_agent(&agents, "dashboard", "custom.salad", "")
                .unwrap();
        assert_eq!(resolved.alias, "salad_chat");
    }

    #[test]
    fn unreachable_model_error_lists_what_is_reachable() {
        let agents = sample_agents();
        let error =
            TchedRouterRuntimeClient::resolve_agent(&agents, "dashboard", "", "gpt-9-imaginary")
                .unwrap_err()
                .to_string();
        assert!(error.contains("reachable models"), "got: {error}");
        assert!(error.contains("qwen3.6-35b-a3b"), "got: {error}");
    }

    #[test]
    fn conversation_history_is_forwarded_in_order() {
        let input = ChatInput {
            messages: vec![
                TchedRouterChatMessage {
                    role: "user".to_string(),
                    content: "first".to_string(),
                },
                TchedRouterChatMessage {
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
    fn system_turns_are_hoisted_ahead_of_the_transcript() {
        let input = ChatInput {
            messages: vec![
                TchedRouterChatMessage {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                },
                TchedRouterChatMessage {
                    role: "system".to_string(),
                    content: "BE-TERSE".to_string(),
                },
            ],
            ..Default::default()
        };
        let prompt = conversation_prompt(&input).unwrap();
        assert!(
            prompt.find("BE-TERSE").unwrap() < prompt.find("[user]").unwrap(),
            "system content must precede the transcript: {prompt}"
        );
    }
}
