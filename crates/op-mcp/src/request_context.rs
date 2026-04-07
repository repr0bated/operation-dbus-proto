//! Request Context - Per-Request Tool Loading
//!
//! Tools are loaded when a request starts and unloaded when it completes.
//! This ensures:
//! - All tools available during request (no eviction)
//! - Memory freed between requests
//! - Clean isolation per request
//! - max_turns enforced per request (not session)

use anyhow::Result;
use simd_json::OwnedValue;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::compact::ToolDefinition;
use crate::tool_registry::{BoxedTool, Tool};

/// Configuration for request handling
#[derive(Debug, Clone)]
pub struct RequestConfig {
    /// Maximum tool calls per REQUEST (not session)
    pub max_turns: u32,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Whether to preload all tools at request start
    pub preload_all: bool,
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self {
            max_turns: 75,
            timeout_secs: 300, // 5 minutes per request
            preload_all: true,
        }
    }
}

/// Per-request context that holds loaded tools
/// 
/// Created at request start, dropped at request end.
/// All tools are loaded into this context and remain available
/// for the entire duration of the request.
pub struct RequestContext {
    /// Request ID for tracking
    pub request_id: String,
    /// Session ID for auth continuity
    pub session_id: Option<String>,
    /// Is this a controller (you/chatbot) with full access
    pub is_controller: bool,
    /// WireGuard peer public key (if authenticated)
    pub peer_pubkey: Option<String>,
    /// When request started
    pub started_at: Instant,
    /// Configuration
    pub config: RequestConfig,
    /// Loaded tools (owned for this request)
    tools: HashMap<String, BoxedTool>,
    /// Tool definitions (for list/search)
    definitions: HashMap<String, ToolDefinition>,
    /// Turn counter for this request
    turn_count: AtomicU32,
    /// Request-scoped variables
    variables: RwLock<HashMap<String, Value>>,
}

impl RequestContext {
    /// Create a new request context
    pub fn new(request_id: String, config: RequestConfig) -> Self {
        info!(request_id = %request_id, "Creating request context");
        Self {
            request_id,
            session_id: None,
            is_controller: false,
            peer_pubkey: None,
            started_at: Instant::now(),
            config,
            tools: HashMap::new(),
            definitions: HashMap::new(),
            turn_count: AtomicU32::new(0),
            variables: RwLock::new(HashMap::new()),
        }
    }

    /// Create with session info (from gateway auth)
    pub fn with_session(
        request_id: String,
        config: RequestConfig,
        session_id: String,
        is_controller: bool,
        peer_pubkey: Option<String>,
    ) -> Self {
        info!(
            request_id = %request_id,
            session_id = %session_id,
            is_controller = %is_controller,
            "Creating authenticated request context"
        );
        Self {
            request_id,
            session_id: Some(session_id),
            is_controller,
            peer_pubkey,
            started_at: Instant::now(),
            config,
            tools: HashMap::new(),
            definitions: HashMap::new(),
            turn_count: AtomicU32::new(0),
            variables: RwLock::new(HashMap::new()),
        }
    }

    /// Check if caller can access controller-only tools
    pub fn can_access_controller_tools(&self) -> bool {
        self.is_controller
    }

    /// Check if caller has any valid session
    pub fn is_authenticated(&self) -> bool {
        self.session_id.is_some()
    }

    /// Load a tool into this request context
    pub fn load_tool(&mut self, tool: BoxedTool) {
        let name = tool.name().to_string();
        let definition = ToolDefinition {
            name: name.clone(),
            description: tool.description().to_string(),
            input_schema: tool.input_schema(),
            category: tool.category().to_string(),
            tags: tool.tags(),
        };
        
        self.tools.insert(name.clone(), tool);
        self.definitions.insert(name.clone(), definition);
        debug!("Loaded tool into request context: {}", name);
    }

    /// Load all tools from a factory function
    pub async fn load_all_tools<F, Fut>(&mut self, factory: F) -> Result<usize>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<BoxedTool>>>,
    {
        let tools = factory().await?;
        let count = tools.len();
        
        for tool in tools {
            self.load_tool(tool);
        }
        
        info!(
            request_id = %self.request_id,
            tool_count = count,
            "Loaded all tools for request"
        );
        
        Ok(count)
    }

    /// Get current turn count
    pub fn turn_count(&self) -> u32 {
        self.turn_count.load(Ordering::Relaxed)
    }

    /// Increment turn count and check limit
    /// Returns Err if max_turns exceeded
    pub fn increment_turn(&self) -> Result<u32, TurnLimitError> {
        let current = self.turn_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        if current > self.config.max_turns {
            warn!(
                request_id = %self.request_id,
                current = current,
                max = self.config.max_turns,
                "Turn limit exceeded"
            );
            return Err(TurnLimitError {
                current,
                max: self.config.max_turns,
            });
        }
        
        debug!(
            request_id = %self.request_id,
            turn = current,
            remaining = self.config.max_turns - current,
            "Turn {} of {}",
            current,
            self.config.max_turns
        );
        
        Ok(current)
    }

    /// Check if request has timed out
    pub fn is_timed_out(&self) -> bool {
        self.started_at.elapsed().as_secs() > self.config.timeout_secs
    }

    /// Get remaining turns
    pub fn remaining_turns(&self) -> u32 {
        self.config.max_turns.saturating_sub(self.turn_count())
    }

    /// Get a tool by name
    pub fn get_tool(&self, name: &str) -> Option<&BoxedTool> {
        self.tools.get(name)
    }

    /// Get tool definition
    pub fn get_definition(&self, name: &str) -> Option<&ToolDefinition> {
        self.definitions.get(name)
    }

    /// Execute a tool
    pub async fn execute_tool(&self, name: &str, input: Value) -> Result<Value> {
        // Check turn limit
        self.increment_turn()?;
        
        // Check timeout
        if self.is_timed_out() {
            anyhow::bail!("Request timed out after {} seconds", self.config.timeout_secs);
        }
        
        // Get and execute tool
        let tool = self.tools.get(name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", name))?;
        
        tool.execute(input).await
    }

    /// List all tools (paginated)
    pub fn list_tools(&self, offset: usize, limit: usize, category: Option<&str>) -> Vec<&ToolDefinition> {
        self.definitions.values()
            .filter(|d| category.map_or(true, |c| d.category == c))
            .skip(offset)
            .take(limit)
            .collect()
    }

    /// Search tools
    pub fn search_tools(&self, query: &str) -> Vec<&ToolDefinition> {
        let query_lower = query.to_lowercase();
        
        self.definitions.values()
            .filter(|d| {
                d.name.to_lowercase().contains(&query_lower) ||
                d.description.to_lowercase().contains(&query_lower) ||
                d.category.to_lowercase().contains(&query_lower)
            })
            .take(50)
            .collect()
    }

    /// Total tool count
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Set a request-scoped variable
    pub async fn set_variable(&self, key: &str, value: Value) {
        self.variables.write().await.insert(key.to_string(), value);
    }

    /// Get a request-scoped variable
    pub async fn get_variable(&self, key: &str) -> Option<Value> {
        self.variables.read().await.get(key).cloned()
    }

    /// Get elapsed time
    pub fn elapsed_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Get summary for logging
    pub fn summary(&self) -> RequestSummary {
        RequestSummary {
            request_id: self.request_id.clone(),
            tools_loaded: self.tools.len(),
            turns_used: self.turn_count(),
            max_turns: self.config.max_turns,
            elapsed_secs: self.elapsed_secs(),
        }
    }
}

impl Drop for RequestContext {
    fn drop(&mut self) {
        info!(
            request_id = %self.request_id,
            tools_loaded = self.tools.len(),
            turns_used = self.turn_count(),
            elapsed_secs = self.elapsed_secs(),
            "Request context dropped, unloading {} tools",
            self.tools.len()
        );
        // Tools are automatically dropped here, freeing memory
    }
}

/// Error when turn limit is exceeded
#[derive(Debug, Clone)]
pub struct TurnLimitError {
    pub current: u32,
    pub max: u32,
}

impl std::fmt::Display for TurnLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Turn limit exceeded: {} of {} maximum tool calls used",
            self.current, self.max
        )
    }
}

impl std::error::Error for TurnLimitError {}

impl From<TurnLimitError> for anyhow::Error {
    fn from(e: TurnLimitError) -> Self {
        anyhow::anyhow!(e.to_string())
    }
}

/// Request summary for logging/metrics
#[derive(Debug, Clone)]
pub struct RequestSummary {
    pub request_id: String,
    pub tools_loaded: usize,
    pub turns_used: u32,
    pub max_turns: u32,
    pub elapsed_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_limit() {
        let config = RequestConfig {
            max_turns: 3,
            ..Default::default()
        };
        let ctx = RequestContext::new("test".to_string(), config);
        
        assert!(ctx.increment_turn().is_ok()); // 1
        assert!(ctx.increment_turn().is_ok()); // 2
        assert!(ctx.increment_turn().is_ok()); // 3
        assert!(ctx.increment_turn().is_err()); // 4 - exceeds limit
    }

    #[test]
    fn test_remaining_turns() {
        let config = RequestConfig {
            max_turns: 10,
            ..Default::default()
        };
        let ctx = RequestContext::new("test".to_string(), config);
        
        assert_eq!(ctx.remaining_turns(), 10);
        ctx.increment_turn().unwrap();
        assert_eq!(ctx.remaining_turns(), 9);
    }
}
