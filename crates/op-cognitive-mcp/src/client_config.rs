//! Cognitive MCP Client Configuration & Optimization
//!
//! Provides connection pooling, caching, retry logic, and optimized transport
//! patterns for external MCP clients connecting to the cognitive-mcp gateway.
//!
//! ## Architecture
//!
//! - **cognitive-mcp** (`:3003`, Netmaker WG IP `100.90.37.254`):
//!   Universal gateway for ALL external clients: NotebookLM, Droid, Cursor,
//!   Codex, Junie, Gemini CLI. External clients MUST use this endpoint.
//!
//! - **compact-mcp** (`127.0.0.1:11436`):
//!   Loopback/chatbot only. NEVER expose externally.
//!
//! ## Client Optimization Strategies
//!
//! 1. **Connection Pooling**: Reuse HTTP/SSE connections to reduce latency
//! 2. **Capability Caching**: Cache tool/resource discovery to reduce round-trips
//! 3. **Batched Operations**: Parallelize independent tool calls
//! 4. **Circuit Breaker**: Fail-fast on degraded service
//! 5. **Exponential Backoff**: Graceful retry with jitter
//!
//! ## Usage
//!
//! ```rust
//! use op_cognitive_mcp::client_config::{CognitiveMcpClient, ClientConfig};
//!
//! let config = ClientConfig::for_external_client("droid-instance-1");
//! let client = CognitiveMcpClient::connect(config).await?;
//!
//! // Cached tool discovery
//! let tools = client.list_tools_cached().await?;
//!
//! // Batched execution
//! let results = client.call_tools_batch([
//!     ("memory/store", json!({"key": "ctx", "value": "data"})),
//!     ("rag/search", json!({"query": "architecture"})),
//! ]).await?;
//! ```

use crate::quota::{QuotaManager, QuotaTier};
use anyhow::{Context, Result};
use reqwest::{Client as HttpClient, ClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Default cognitive-mcp endpoint for external clients.
/// Per AGENTS.md: cognitive-mcp (`:3003`, Netmaker WG IP `100.90.37.254`)
/// is the universal gateway for ALL external clients.
pub const COGNITIVE_MCP_DEFAULT_ENDPOINT: &str = "http://100.90.37.254:3003";

/// Default compact-mcp endpoint (loopback only, chatbot only).
pub const COMPACT_MCP_DEFAULT_ENDPOINT: &str = "http://127.0.0.1:11436";

/// Connection pool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Maximum idle connections per host.
    pub max_idle: usize,
    /// Connection idle timeout before closing.
    pub idle_timeout_secs: u64,
    /// Maximum connections in the pool.
    pub max_connections: usize,
    /// Enable HTTP/2 if server supports it.
    pub use_http2: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_idle: 10,
            idle_timeout_secs: 90,
            max_connections: 100,
            use_http2: true,
        }
    }
}

/// Retry configuration with exponential backoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts.
    pub max_retries: u32,
    /// Base delay in milliseconds.
    pub base_delay_ms: u64,
    /// Maximum delay in milliseconds.
    pub max_delay_ms: u64,
    /// Backoff multiplier.
    pub backoff_multiplier: f64,
    /// Enable jitter to prevent thundering herd.
    pub use_jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            use_jitter: true,
        }
    }
}

/// Cache configuration for capability discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Cache TTL for tools list in seconds.
    pub tools_cache_ttl_secs: u64,
    /// Cache TTL for resources list in seconds.
    pub resources_cache_ttl_secs: u64,
    /// Maximum cache entries.
    pub max_entries: usize,
    /// Enable stale-while-revalidate pattern.
    pub stale_while_revalidate: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            tools_cache_ttl_secs: 300,    // 5 minutes
            resources_cache_ttl_secs: 60, // 1 minute
            max_entries: 1000,
            stale_while_revalidate: true,
        }
    }
}

/// Circuit breaker configuration for fault tolerance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Failure threshold before opening circuit.
    pub failure_threshold: u32,
    /// Success threshold to close circuit after being half-open.
    pub success_threshold: u32,
    /// Timeout before attempting reset (half-open).
    pub timeout_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout_secs: 30,
        }
    }
}

/// Client type classification for proper endpoint selection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    /// External client (Droid, Cursor, Codex, NotebookLM, Gemini CLI, etc.)
    /// Must use cognitive-mcp `:3003` endpoint.
    External,
    /// Local chatbot/loopback client.
    /// Uses compact-mcp `:11436` endpoint.
    LocalChatbot,
    /// D-Bus native client (same machine, no network).
    DbusNative,
}

/// Complete client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Client identifier (e.g., "droid-instance-1", "notebooklm-session-abc").
    pub client_id: String,
    /// Client type determines endpoint selection.
    pub client_type: ClientType,
    /// Server endpoint URL.
    pub endpoint: String,
    /// Connection pool settings.
    pub pool: PoolConfig,
    /// Retry settings.
    pub retry: RetryConfig,
    /// Cache settings.
    pub cache: CacheConfig,
    /// Circuit breaker settings.
    pub circuit_breaker: CircuitBreakerConfig,
    /// Request timeout.
    pub request_timeout_secs: u64,
    /// WireGuard public key for Ghostbridge auth (optional).
    pub wg_pubkey: Option<String>,
    /// Quota tier for rate limiting awareness.
    pub quota_tier: QuotaTier,
    /// Enable debug logging.
    pub debug: bool,
}

impl ClientConfig {
    /// Create configuration for an external client.
    /// Per AGENTS.md, external clients MUST use cognitive-mcp `:3003`.
    pub fn for_external_client<S: Into<String>>(client_id: S) -> Self {
        Self {
            client_id: client_id.into(),
            client_type: ClientType::External,
            endpoint: COGNITIVE_MCP_DEFAULT_ENDPOINT.to_string(),
            pool: PoolConfig::default(),
            retry: RetryConfig::default(),
            cache: CacheConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            request_timeout_secs: 30,
            wg_pubkey: None,
            quota_tier: QuotaTier::default(),
            debug: false,
        }
    }

    /// Create configuration for a local chatbot client (loopback only).
    pub fn for_local_chatbot<S: Into<String>>(client_id: S) -> Self {
        Self {
            client_id: client_id.into(),
            client_type: ClientType::LocalChatbot,
            endpoint: COMPACT_MCP_DEFAULT_ENDPOINT.to_string(),
            pool: PoolConfig::default(),
            retry: RetryConfig::default(),
            cache: CacheConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            request_timeout_secs: 30,
            wg_pubkey: None,
            quota_tier: QuotaTier::default(),
            debug: false,
        }
    }

    /// Set custom endpoint (useful for testing or custom deployments).
    pub fn with_endpoint<S: Into<String>>(mut self, endpoint: S) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Set WireGuard public key for Ghostbridge authentication.
    pub fn with_wg_pubkey<S: Into<String>>(mut self, pubkey: S) -> Self {
        self.wg_pubkey = Some(pubkey.into());
        self
    }

    /// Set custom quota tier.
    pub fn with_quota_tier(mut self, tier: QuotaTier) -> Self {
        self.quota_tier = tier;
        self
    }

    /// Validate configuration based on client type constraints.
    pub fn validate(&self) -> Result<()> {
        match self.client_type {
            ClientType::External => {
                // External clients must use cognitive-mcp port 3003
                if !self.endpoint.contains(":3003") && !self.endpoint.contains("/mcp") {
                    warn!(
                        client_id = %self.client_id,
                        endpoint = %self.endpoint,
                        "External client may be using wrong endpoint. Should use :3003"
                    );
                }
            }
            ClientType::LocalChatbot => {
                // Local chatbot must use loopback
                if !self.endpoint.starts_with("http://127.0.0.1")
                    && !self.endpoint.starts_with("http://localhost")
                {
                    return Err(anyhow::anyhow!(
                        "LocalChatbot client must use loopback address (127.0.0.1 or localhost). \
                         Got: {}",
                        self.endpoint
                    ));
                }
            }
            ClientType::DbusNative => {
                // D-Bus native clients don't use HTTP
            }
        }
        Ok(())
    }
}

/// Cached capability entry with TTL.
#[derive(Debug, Clone)]
struct CachedCapability<T> {
    data: T,
    expires_at: Instant,
}

impl<T> CachedCapability<T> {
    fn new(data: T, ttl_secs: u64) -> Self {
        Self {
            data,
            expires_at: Instant::now() + Duration::from_secs(ttl_secs),
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

/// Circuit breaker state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing, reject requests
    HalfOpen, // Testing if service recovered
}

/// Thread-safe circuit breaker.
struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    failures: Arc<RwLock<u32>>,
    successes: Arc<RwLock<u32>>,
    last_failure: Arc<RwLock<Option<Instant>>>,
}

impl CircuitBreaker {
    fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failures: Arc::new(RwLock::new(0)),
            successes: Arc::new(RwLock::new(0)),
            last_failure: Arc::new(RwLock::new(None)),
        }
    }

    async fn can_execute(&self) -> bool {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let last = *self.last_failure.read().await;
                if let Some(last_fail) = last {
                    let elapsed = Instant::now().duration_since(last_fail).as_secs();
                    if elapsed >= self.config.timeout_secs {
                        info!("Circuit breaker entering half-open state");
                        *state = CircuitState::HalfOpen;
                        *self.failures.write().await = 0;
                        *self.successes.write().await = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    async fn record_success(&self) {
        let mut state = self.state.write().await;
        *self.last_failure.write().await = None;

        if *state == CircuitState::HalfOpen {
            let mut successes = self.successes.write().await;
            *successes += 1;
            if *successes >= self.config.success_threshold {
                info!("Circuit breaker closing (service recovered)");
                *state = CircuitState::Closed;
                *successes = 0;
                *self.failures.write().await = 0;
            }
        } else {
            *self.failures.write().await = 0;
        }
    }

    async fn record_failure(&self) {
        let mut state = self.state.write().await;
        *self.last_failure.write().await = Some(Instant::now());

        let mut failures = self.failures.write().await;
        *failures += 1;

        if *failures >= self.config.failure_threshold {
            warn!(
                failures = *failures,
                threshold = self.config.failure_threshold,
                "Circuit breaker opening (service degraded)"
            );
            *state = CircuitState::Open;
        }
    }
}

/// Optimized cognitive-mcp client with pooling, caching, and resilience.
pub struct CognitiveMcpClient {
    config: ClientConfig,
    http: HttpClient,
    circuit_breaker: CircuitBreaker,
    quota_manager: Arc<QuotaManager>,
    // Capability caches
    tools_cache: Arc<RwLock<Option<CachedCapability<Vec<Value>>>>>,
    resources_cache: Arc<RwLock<Option<CachedCapability<Vec<Value>>>>>,
    request_count: Arc<RwLock<u64>>,
}

impl CognitiveMcpClient {
    /// Connect to cognitive-mcp with optimized configuration.
    pub async fn connect(config: ClientConfig) -> Result<Self> {
        config.validate().context("Invalid client configuration")?;

        let http = Self::build_http_client(&config)?;
        let circuit_breaker = CircuitBreaker::new(config.circuit_breaker.clone());
        let quota_manager = Arc::new(QuotaManager::new(config.quota_tier.clone()));

        info!(
            client_id = %config.client_id,
            client_type = ?config.client_type,
            endpoint = %config.endpoint,
            "CognitiveMcpClient initialized"
        );

        Ok(Self {
            config,
            http,
            circuit_breaker,
            quota_manager,
            tools_cache: Arc::new(RwLock::new(None)),
            resources_cache: Arc::new(RwLock::new(None)),
            request_count: Arc::new(RwLock::new(0)),
        })
    }

    fn build_http_client(config: &ClientConfig) -> Result<HttpClient> {
        let mut builder = ClientBuilder::new()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .pool_max_idle_per_host(config.pool.max_idle)
            .pool_idle_timeout(Duration::from_secs(config.pool.idle_timeout_secs));

        if config.pool.use_http2 {
            builder = builder.http2_prior_knowledge();
        }

        builder.build().context("Failed to build HTTP client")
    }

    /// List available tools with caching.
    pub async fn list_tools_cached(&self) -> Result<Vec<Value>> {
        // Check cache first
        {
            let cache = self.tools_cache.read().await;
            if let Some(ref cached) = *cache {
                if !cached.is_expired() {
                    debug!("Returning cached tools list");
                    return Ok(cached.data.clone());
                }
            }
        }

        // Fetch fresh
        let tools = self.list_tools().await?;

        // Update cache
        let mut cache = self.tools_cache.write().await;
        *cache = Some(CachedCapability::new(
            tools.clone(),
            self.config.cache.tools_cache_ttl_secs,
        ));

        Ok(tools)
    }

    /// List tools (uncached, direct API call).
    pub async fn list_tools(&self) -> Result<Vec<Value>> {
        self.check_circuit_and_quota().await?;

        let url = format!("{}/tools", self.config.endpoint);
        debug!(url = %url, "Fetching tools list");

        let response = self
            .http
            .get(&url)
            .header("X-Client-ID", &self.config.client_id)
            .header("X-Client-Type", format!("{:?}", self.config.client_type))
            .send()
            .await
            .context("Failed to fetch tools list")?;

        if response.status().is_success() {
            let tools: Vec<Value> = response.json().await?;
            self.circuit_breaker.record_success().await;
            Ok(tools)
        } else {
            let status = response.status();
            self.circuit_breaker.record_failure().await;
            Err(anyhow::anyhow!("Server returned error: {}", status))
        }
    }

    /// List available resources with caching.
    pub async fn list_resources_cached(&self) -> Result<Vec<Value>> {
        {
            let cache = self.resources_cache.read().await;
            if let Some(ref cached) = *cache {
                if !cached.is_expired() {
                    debug!("Returning cached resources list");
                    return Ok(cached.data.clone());
                }
            }
        }

        let resources = self.list_resources().await?;

        let mut cache = self.resources_cache.write().await;
        *cache = Some(CachedCapability::new(
            resources.clone(),
            self.config.cache.resources_cache_ttl_secs,
        ));

        Ok(resources)
    }

    /// List resources (uncached, direct API call).
    pub async fn list_resources(&self) -> Result<Vec<Value>> {
        self.check_circuit_and_quota().await?;

        let url = format!("{}/resources", self.config.endpoint);

        let response = self
            .http
            .get(&url)
            .header("X-Client-ID", &self.config.client_id)
            .send()
            .await
            .context("Failed to fetch resources list")?;

        if response.status().is_success() {
            let resources: Vec<Value> = response.json().await?;
            self.circuit_breaker.record_success().await;
            Ok(resources)
        } else {
            self.circuit_breaker.record_failure().await;
            Err(anyhow::anyhow!(
                "Server returned error: {}",
                response.status()
            ))
        }
    }

    /// Execute a single tool call with automatic retry.
    pub async fn call_tool(&self, tool_name: &str, params: Value) -> Result<Value> {
        self.check_circuit_and_quota().await?;

        let url = format!("{}/tools/{}", self.config.endpoint, tool_name);

        for attempt in 0..=self.config.retry.max_retries {
            debug!(
                tool = tool_name,
                attempt = attempt + 1,
                max_attempts = self.config.retry.max_retries + 1,
                "Calling tool"
            );

            let request = self
                .http
                .post(&url)
                .header("X-Client-ID", &self.config.client_id)
                .header("Content-Type", "application/json");

            // Add WireGuard auth header if available
            let request = if let Some(ref pubkey) = self.config.wg_pubkey {
                request.header("X-Ghostbridge-Footprint", pubkey)
            } else {
                request
            };

            match request.json(&params).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        let result: Value = response.json().await?;
                        self.circuit_breaker.record_success().await;
                        self.increment_request_count().await;
                        return Ok(result);
                    } else {
                        let status = response.status();
                        warn!(tool = tool_name, status = %status, "Tool call returned error");

                        if status.as_u16() >= 500 {
                            self.circuit_breaker.record_failure().await;
                        }

                        if attempt < self.config.retry.max_retries {
                            let delay = self.calculate_backoff(attempt);
                            tokio::time::sleep(delay).await;
                            continue;
                        } else {
                            return Err(anyhow::anyhow!("Tool call failed: {}", status));
                        }
                    }
                }
                Err(e) => {
                    warn!(tool = tool_name, error = %e, "Tool call failed");
                    self.circuit_breaker.record_failure().await;

                    if attempt < self.config.retry.max_retries {
                        let delay = self.calculate_backoff(attempt);
                        tokio::time::sleep(delay).await;
                        continue;
                    } else {
                        return Err(e.into());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Max retries exceeded"))
    }

    /// Execute multiple tool calls in parallel (batched).
    pub async fn call_tools_batch(
        &self,
        calls: Vec<(&str, Value)>,
    ) -> Result<Vec<(String, Result<Value>)>> {
        use tokio::join;

        let futures: Vec<_> = calls
            .into_iter()
            .map(|(name, params)| async move {
                let result = self.call_tool(name, params).await;
                (name.to_string(), result)
            })
            .collect();

        let results: Vec<_> = futures::future::join_all(futures).await;
        Ok(results)
    }

    /// Store a memory entry (convenience wrapper).
    pub async fn store_memory(
        &self,
        namespace: &str,
        key: &str,
        value: Value,
        tags: Vec<String>,
    ) -> Result<Value> {
        self.call_tool(
            "memory/store",
            serde_json::json!({
                "namespace": namespace,
                "key": key,
                "value": value,
                "tags": tags,
            }),
        )
        .await
    }

    /// Retrieve a memory entry (convenience wrapper).
    pub async fn retrieve_memory(&self, namespace: &str, key: &str) -> Result<Option<Value>> {
        let result = self
            .call_tool(
                "memory/retrieve",
                serde_json::json!({
                    "namespace": namespace,
                    "key": key,
                }),
            )
            .await?;

        Ok(result.get("value").cloned())
    }

    /// Perform semantic search via RAG (convenience wrapper).
    pub async fn rag_search(&self, query: &str, limit: Option<usize>) -> Result<Vec<Value>> {
        let result = self
            .call_tool(
                "rag/search",
                serde_json::json!({
                    "query": query,
                    "limit": limit.unwrap_or(10),
                }),
            )
            .await?;

        result
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Invalid RAG response format"))
    }

    /// Get current quota status.
    pub async fn quota_status(&self) -> (u32, u32) {
        self.quota_manager.status().await
    }

    /// Get client statistics.
    pub async fn stats(&self) -> ClientStats {
        let requests = *self.request_count.read().await;
        let (remaining, limit) = self.quota_status().await;

        ClientStats {
            client_id: self.config.client_id.clone(),
            total_requests: requests,
            quota_remaining: remaining,
            quota_limit: limit,
            endpoint: self.config.endpoint.clone(),
        }
    }

    /// Invalidate capability caches.
    pub async fn invalidate_cache(&self) {
        *self.tools_cache.write().await = None;
        *self.resources_cache.write().await = None;
        debug!("Capability caches invalidated");
    }

    // Internal helpers

    async fn check_circuit_and_quota(&self) -> Result<()> {
        // Check circuit breaker
        if !self.circuit_breaker.can_execute().await {
            return Err(anyhow::anyhow!(
                "Circuit breaker is OPEN - service temporarily unavailable"
            ));
        }

        // Check quota
        let (allowed, remaining, limit) = self.quota_manager.check_and_increment().await;
        if !allowed {
            return Err(anyhow::anyhow!(
                "Quota exceeded: {}/{} queries used. Try again tomorrow.",
                limit - remaining,
                limit
            ));
        }

        Ok(())
    }

    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base = self.config.retry.base_delay_ms as f64;
        let multiplier = self.config.retry.backoff_multiplier.powi(attempt as i32);
        let delay_ms = (base * multiplier).min(self.config.retry.max_delay_ms as f64);

        let jitter = if self.config.retry.use_jitter {
            rand::random::<f64>() * 0.3 * delay_ms // 30% jitter
        } else {
            0.0
        };

        Duration::from_millis((delay_ms + jitter) as u64)
    }

    async fn increment_request_count(&self) {
        let mut count = self.request_count.write().await;
        *count += 1;
    }
}

/// Client statistics snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientStats {
    pub client_id: String,
    pub total_requests: u64,
    pub quota_remaining: u32,
    pub quota_limit: u32,
    pub endpoint: String,
}

/// Factory for creating pre-configured clients for specific use cases.
pub struct CognitiveMcpClientFactory;

impl CognitiveMcpClientFactory {
    /// Create a high-throughput client optimized for batch operations.
    pub fn high_throughput<S: Into<String>>(client_id: S) -> ClientConfig {
        ClientConfig::for_external_client(client_id).with_quota_tier(QuotaTier {
            name: "high_throughput".into(),
            daily_limit: 10000,
        })
    }

    /// Create a low-latency client optimized for interactive use.
    pub fn low_latency<S: Into<String>>(client_id: S) -> ClientConfig {
        let mut config = ClientConfig::for_external_client(client_id);
        config.pool.max_idle = 20;
        config.pool.idle_timeout_secs = 300;
        config.request_timeout_secs = 5;
        config.retry.max_retries = 1;
        config.cache.tools_cache_ttl_secs = 60; // Short TTL for frequent updates
        config
    }

    /// Create a resilient client optimized for unreliable networks.
    pub fn resilient<S: Into<String>>(client_id: S) -> ClientConfig {
        let mut config = ClientConfig::for_external_client(client_id);
        config.retry.max_retries = 10;
        config.retry.base_delay_ms = 500;
        config.retry.max_delay_ms = 30000;
        config.circuit_breaker.timeout_secs = 60;
        config.request_timeout_secs = 60;
        config
    }

    /// Create a notebooklm-compatible client.
    pub fn for_notebooklm<S: Into<String>>(session_id: S) -> ClientConfig {
        let mut config =
            ClientConfig::for_external_client(format!("notebooklm-{}", session_id.into()));
        // NotebookLM has specific quota expectations per spec
        config.quota_tier = QuotaTier {
            name: "notebooklm".into(),
            daily_limit: 50,
        };
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_validate_external_endpoint() {
        let config = ClientConfig::for_external_client("test-1");
        assert_eq!(config.client_type, ClientType::External);
        assert!(config.endpoint.contains(":3003"));
    }

    #[test]
    fn should_validate_local_chatbot_endpoint() {
        let config = ClientConfig::for_local_chatbot("test-2");
        assert_eq!(config.client_type, ClientType::LocalChatbot);
        assert!(config.endpoint.contains("127.0.0.1"));
    }

    #[test]
    fn should_reject_invalid_local_endpoint() {
        let config = ClientConfig {
            client_id: "test".into(),
            client_type: ClientType::LocalChatbot,
            endpoint: "http://192.168.1.1:11436".into(),
            pool: PoolConfig::default(),
            retry: RetryConfig::default(),
            cache: CacheConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            request_timeout_secs: 30,
            wg_pubkey: None,
            quota_tier: QuotaTier::default(),
            debug: false,
        };
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn should_track_quota() {
        let config = ClientConfig {
            client_id: "test".into(),
            client_type: ClientType::External,
            endpoint: COGNITIVE_MCP_DEFAULT_ENDPOINT.into(),
            pool: PoolConfig::default(),
            retry: RetryConfig::default(),
            cache: CacheConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            request_timeout_secs: 30,
            wg_pubkey: None,
            quota_tier: QuotaTier {
                name: "test".into(),
                daily_limit: 3,
            },
            debug: false,
        };

        let client = CognitiveMcpClient::connect(config).await.unwrap();
        let (remaining, limit) = client.quota_status().await;
        assert_eq!(remaining, 3);
        assert_eq!(limit, 3);
    }
}
