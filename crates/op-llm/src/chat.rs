//! Chat Manager - Manages provider switching and chat sessions
//!
//! ## Authentication Priority
//!
//! 1. **MCP Proxy** (VS Code extension emulation through `op-mcp-proxy`)
//! 2. **GCloud ADC** (direct OAuth via gcloud)
//! 3. **Gemini** (API key fallback)
//! 4. **Anthropic** (API key)
//!
//! ## Environment Variables
//!
//! ```bash
//! # Preferred: MCP proxy bridge
//! ENABLE_MCP_PROXY_PROVIDER=true
//! OP_MCP_PROXY_BIN=op-mcp-proxy
//!
//! # Provider selection
//! LLM_PROVIDER=mcp-proxy  # or openclaw, gemini, gemini-cli, anthropic
//! LLM_MODEL=gemini-2.5-flash
//!
//! # Optional API key fallbacks
//! GEMINI_API_KEY=xxx
//! ANTHROPIC_API_KEY=xxx
//! ```

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::anthropic::AnthropicClient;
use crate::gcloud_adc::GCloudADCProvider;
use crate::gemini::GeminiClient;
use crate::gemini_cli::create_gemini_cli_provider;
use crate::mcp_proxy::McpProxyProvider;
use crate::openclaw::OpenClawProvider;
use crate::provider::{
    BoxedProvider, ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType,
};
use async_trait::async_trait;

/// Chat manager - handles multiple providers and model selection
pub struct ChatManager {
    providers: HashMap<ProviderType, BoxedProvider>,
    current_provider: Arc<RwLock<ProviderType>>,
    current_model: Arc<RwLock<String>>,
    model_cache: Arc<RwLock<HashMap<ProviderType, Vec<ModelInfo>>>>,
}

impl ChatManager {
    /// Create a new chat manager
    ///
    /// Initialization order:
    /// 1. Check LLM_PROVIDER environment variable
    /// 2. Try MCP Proxy (VS Code extension emulation)
    /// 3. Try GCloud ADC
    /// 4. Try Gemini (API key)
    /// 5. Try Anthropic (API key)
    pub fn new() -> Self {
        let mut providers: HashMap<ProviderType, BoxedProvider> = HashMap::new();
        let mut default_provider = None;
        let mut default_model = std::env::var("OPENCLAW_DEFAULT_MODEL")
            .unwrap_or_else(|_| "gemini-2.0-flash".to_string());

        // Check environment variables
        let env_provider = std::env::var("LLM_PROVIDER").ok();
        let env_model = std::env::var("LLM_MODEL").ok();

        if let Some(ref provider_name) = env_provider {
            info!("📋 LLM_PROVIDER={}", provider_name);
        }
        if let Some(ref model_name) = env_model {
            info!("📋 LLM_MODEL={}", model_name);
            default_model = model_name.clone();
        }

        // =====================================================
        // MCP Proxy - Gemini through op-mcp-proxy DIRECT_MODE
        // =====================================================
        if std::env::var("ENABLE_MCP_PROXY_PROVIDER")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true)
        {
            match McpProxyProvider::from_env() {
                Ok(proxy) => {
                    info!("✅ MCP Proxy provider initialized");
                    providers.insert(ProviderType::McpProxy, Box::new(proxy));
                    if default_provider.is_none() {
                        default_provider = Some(ProviderType::McpProxy);
                    }
                }
                Err(e) => {
                    debug!("MCP Proxy provider failed: {}", e);
                }
            }
        }

        // =====================================================
        // Gemini CLI provider (optional)
        // =====================================================
        let wants_gemini_cli = std::env::var("ENABLE_GEMINI_CLI_PROVIDER")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
            || env_provider
                .as_deref()
                .map(|v| {
                    v.eq_ignore_ascii_case("gemini-cli")
                        || v.eq_ignore_ascii_case("gemini_cli")
                        || v.eq_ignore_ascii_case("geminicli")
                })
                .unwrap_or(false);

        if wants_gemini_cli {
            let gemini_cli = create_gemini_cli_provider();
            info!("✅ Gemini CLI provider initialized");
            providers.insert(ProviderType::GeminiCli, Box::new(gemini_cli));
            if default_provider.is_none() {
                default_provider = Some(ProviderType::GeminiCli);
            }
        }

        // =====================================================
        // GCloud ADC - Directly from gcloud CLI / application-default credentials
        // Kept under ProviderType::Antigravity for backward compatibility of provider ids.
        // =====================================================
        let gcloud = GCloudADCProvider::new();
        info!("✅ GCloud ADC provider initialized");
        providers.insert(ProviderType::Antigravity, Box::new(gcloud));
        if default_provider.is_none() {
            default_provider = Some(ProviderType::Antigravity);
        }

        // =====================================================
        // Gemini - API key fallback
        // =====================================================
        if std::env::var("GEMINI_API_KEY").is_ok() {
            match GeminiClient::from_env() {
                Ok(gemini) => {
                    info!("✅ Gemini provider initialized (API key)");
                    providers.insert(ProviderType::Gemini, Box::new(gemini));
                    if default_provider.is_none() {
                        default_provider = Some(ProviderType::Gemini);
                    }
                }
                Err(e) => {
                    debug!("Gemini provider failed: {}", e);
                }
            }
        }

        // =====================================================
        // OpenClaw - Bearer token
        // =====================================================
        if std::env::var("OPENCLAW_TOKEN").is_ok() {
            match OpenClawProvider::from_env() {
                Ok(openclaw) => {
                    info!("✅ OpenClaw provider initialized");
                    providers.insert(ProviderType::OpenClaw, Box::new(openclaw));
                    if default_provider.is_none() {
                        default_provider = Some(ProviderType::OpenClaw);
                    }
                }
                Err(e) => {
                    debug!("OpenClaw provider failed: {}", e);
                }
            }
        }

        // =====================================================
        // Anthropic - API key
        // =====================================================
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            match AnthropicClient::from_env() {
                Ok(anthropic) => {
                    info!("✅ Anthropic provider initialized");
                    providers.insert(ProviderType::Anthropic, Box::new(anthropic));
                    if default_provider.is_none() {
                        default_provider = Some(ProviderType::Anthropic);
                    }
                }
                Err(e) => {
                    debug!("Anthropic provider failed: {}", e);
                }
            }
        }

        // Use environment provider if specified and available
        let final_provider = if let Some(ref provider_name) = env_provider {
            if let Ok(pt) = provider_name.parse::<ProviderType>() {
                if providers.contains_key(&pt) {
                    info!("✅ Using LLM_PROVIDER: {:?}", pt);
                    pt
                } else {
                    warn!("⚠️  LLM_PROVIDER '{}' not available", provider_name);
                    default_provider.unwrap_or(ProviderType::McpProxy)
                }
            } else {
                warn!("⚠️  Invalid LLM_PROVIDER '{}'", provider_name);
                default_provider.unwrap_or(ProviderType::McpProxy)
            }
        } else {
            default_provider.unwrap_or(ProviderType::McpProxy)
        };

        if providers.is_empty() {
            warn!("⚠️  No LLM providers available!");
            warn!("   Configure authentication:");
            warn!("   1. Install/build op-mcp-proxy and set OP_MCP_PROXY_BIN");
            warn!("   2. Authenticate: gcloud auth login");
            warn!("   3. Or set OPENCLAW_TOKEN and LLM_PROVIDER=openclaw");
            warn!("   4. Or set GEMINI_API_KEY environment variable");
        } else {
            info!("\n📊 Default provider: {:?}", final_provider);
            info!("📊 Default model: {}", default_model);
            info!("📊 Available providers: {}\n", providers.len());
        }

        Self {
            providers,
            current_provider: Arc::new(RwLock::new(final_provider)),
            current_model: Arc::new(RwLock::new(default_model)),
            model_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a provider
    pub fn add_provider(&mut self, provider: BoxedProvider) {
        let provider_type = provider.provider_type();
        self.providers.insert(provider_type, provider);
    }

    /// Get current provider type
    pub async fn current_provider(&self) -> ProviderType {
        self.current_provider.read().await.clone()
    }

    /// Get current model
    pub async fn current_model(&self) -> String {
        self.current_model.read().await.clone()
    }

    /// Switch provider
    pub async fn switch_provider(&self, provider_type: ProviderType) -> Result<()> {
        if !self.providers.contains_key(&provider_type) {
            return Err(anyhow::anyhow!(
                "Provider {:?} not available. Available: {:?}",
                provider_type,
                self.available_providers()
            ));
        }

        *self.current_provider.write().await = provider_type.clone();
        info!("Switched to provider: {:?}", provider_type);

        // Get first available model for this provider
        let models = self.list_models().await?;
        if let Some(first) = models.first() {
            *self.current_model.write().await = first.id.clone();
            info!("Default model set to: {}", first.id);
        }

        Ok(())
    }

    /// Switch model
    pub async fn switch_model(&self, model_id: impl Into<String>) -> Result<()> {
        let model_id = model_id.into();
        *self.current_model.write().await = model_id.clone();
        info!("Switched to model: {}", model_id);
        Ok(())
    }

    /// List available providers
    pub fn available_providers(&self) -> Vec<ProviderType> {
        self.providers.keys().cloned().collect()
    }

    /// Check if a provider is available
    pub fn has_provider(&self, provider_type: &ProviderType) -> bool {
        self.providers.contains_key(provider_type)
    }

    async fn resolve_provider(&self) -> Result<ProviderType> {
        let current = self.current_provider.read().await.clone();
        if self.providers.contains_key(&current) {
            return Ok(current);
        }

        if let Some(fallback) = self.providers.keys().next().cloned() {
            warn!(
                "Provider {:?} not available, falling back to {:?}",
                current, fallback
            );
            *self.current_provider.write().await = fallback.clone();
            return Ok(fallback);
        }

        Err(anyhow!(
            "No LLM providers configured.\n\n\
            To authenticate:\n\
            1. Build/install op-mcp-proxy and set OP_MCP_PROXY_BIN\n\
            2. Run: gcloud auth login\n\
            3. Optional: set LLM_PROVIDER=mcp-proxy\n\
            4. Or set OPENCLAW_TOKEN and LLM_PROVIDER=openclaw\n\n\
            Or set GEMINI_API_KEY environment variable."
        ))
    }

    /// List models from current provider
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let provider_type = self.current_provider.read().await.clone();

        // Check cache first
        {
            let cache = self.model_cache.read().await;
            if let Some(models) = cache.get(&provider_type) {
                return Ok(models.clone());
            }
        }

        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider not available"))?;

        let models = provider.list_models().await?;

        // Cache
        {
            let mut cache = self.model_cache.write().await;
            cache.insert(provider_type, models.clone());
        }

        Ok(models)
    }

    /// List models for a specific provider
    pub async fn list_models_for_provider(
        &self,
        provider_type: &ProviderType,
    ) -> Result<Vec<ModelInfo>> {
        let provider = self
            .providers
            .get(provider_type)
            .ok_or_else(|| anyhow!("Provider {:?} not available", provider_type))?;

        provider.list_models().await
    }

    /// Search models
    pub async fn search_models(&self, query: &str) -> Result<Vec<ModelInfo>> {
        let provider_type = self.current_provider.read().await.clone();
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider not available"))?;

        provider.search_models(query, 20).await
    }

    /// Refresh models (clear cache)
    pub async fn refresh_models(&self) -> Result<Vec<ModelInfo>> {
        let provider_type = self.current_provider.read().await.clone();
        {
            let mut cache = self.model_cache.write().await;
            cache.remove(&provider_type);
        }
        self.list_models().await
    }

    /// Get model info
    pub async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        let provider_type = self.current_provider.read().await.clone();
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider not available"))?;

        provider.get_model(model_id).await
    }

    /// Check if model is available
    pub async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        let provider_type = self.current_provider.read().await.clone();
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider not available"))?;

        provider.is_model_available(model_id).await
    }

    /// Send chat message
    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let provider_type = self.resolve_provider().await?;
        let model = self.current_model.read().await.clone();

        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider not available"))?;

        provider.chat(&model, messages).await
    }

    /// Send chat with specific provider and model
    pub async fn chat_with(
        &self,
        provider_type: &ProviderType,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<ChatResponse> {
        let provider = self
            .providers
            .get(provider_type)
            .ok_or_else(|| anyhow!("Provider {:?} not available", provider_type))?;

        provider.chat(model, messages).await
    }

    /// Get status
    pub async fn get_status(&self) -> simd_json::OwnedValue {
        let provider = self.current_provider.read().await.clone();
        let model = self.current_model.read().await.clone();
        let providers: Vec<String> = self
            .available_providers()
            .iter()
            .map(|p| p.to_string())
            .collect();

        simd_json::json!({
            "provider": provider.to_string(),
            "model": model,
            "available_providers": providers,
        })
    }

    /// Get detailed status
    pub async fn get_detailed_status(&self) -> simd_json::OwnedValue {
        let current_provider = self.current_provider.read().await.clone();
        let current_model = self.current_model.read().await.clone();

        let mut provider_status = simd_json::value::owned::Object::new();

        for ptype in self.providers.keys() {
            let models = self.list_models_for_provider(ptype).await.ok();
            let (auth_type, features) = match ptype {
                ProviderType::McpProxy => (
                    "OAuth via op-mcp-proxy (VS Code extension emulation)",
                    vec![
                        "Cloud Code-compatible headers",
                        "Gemini models",
                        "Headless-friendly",
                    ],
                ),
                ProviderType::Antigravity => (
                    "OAuth (gcloud ADC, legacy provider id)",
                    vec!["Gemini models", "Application-default credentials"],
                ),
                ProviderType::Gemini => (
                    "API key (GEMINI_API_KEY)",
                    vec!["Gemini models", "Multimodal", "Long context"],
                ),
                ProviderType::GeminiCli => (
                    "Local Gemini CLI bridge",
                    vec![
                        "Gemini CLI binary",
                        "ADC/service account auth",
                        "Headless-friendly",
                    ],
                ),
                ProviderType::Anthropic => (
                    "API key (ANTHROPIC_API_KEY)",
                    vec!["Claude models", "Best reasoning", "Tool use"],
                ),
                ProviderType::OpenClaw => (
                    "Bearer token (OPENCLAW_TOKEN)",
                    vec!["OpenAI-compatible API", "Agent platform", "Tool use"],
                ),
                _ => ("API key", vec![]),
            };

            provider_status.insert(
                ptype.to_string(),
                simd_json::json!({
                    "available": true,
                    "model_count": models.as_ref().map(|m| m.len()).unwrap_or(0),
                    "auth_type": auth_type,
                    "features": features,
                }),
            );
        }

        simd_json::json!({
            "current_provider": current_provider.to_string(),
            "current_model": current_model,
            "providers": provider_status,
        })
    }
}

#[async_trait]
impl LlmProvider for ChatManager {
    fn provider_type(&self) -> ProviderType {
        self.current_provider.blocking_read().clone()
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        ChatManager::list_models(self).await
    }

    async fn search_models(&self, query: &str, limit: usize) -> Result<Vec<ModelInfo>> {
        let _limit = limit;
        ChatManager::search_models(self, query).await
    }

    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        ChatManager::get_model(self, model_id).await
    }

    async fn is_model_available(&self, model_id: &str) -> Result<bool> {
        ChatManager::is_model_available(self, model_id).await
    }

    async fn chat(&self, model: &str, messages: Vec<ChatMessage>) -> Result<ChatResponse> {
        let provider_type = self.resolve_provider().await?;
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider {:?} not available", provider_type))?;

        provider.chat(model, messages).await
    }

    async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
        let provider_type = self.resolve_provider().await?;
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider {:?} not available", provider_type))?;

        provider.chat_with_request(model, request).await
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String>>> {
        let provider_type = self.resolve_provider().await?;
        let provider = self
            .providers
            .get(&provider_type)
            .ok_or_else(|| anyhow!("Provider {:?} not available", provider_type))?;

        provider.chat_stream(model, messages).await
    }
}

impl Default for ChatManager {
    fn default() -> Self {
        Self::new()
    }
}
