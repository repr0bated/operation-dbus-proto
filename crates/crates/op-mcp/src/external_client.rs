//! External MCP Client - Connect to and introspect other MCP servers

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::RwLock;

/// External MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalMcpConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,

    /// Environment variables to pass to the server
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// API key (will be set as env var or header based on auth_method)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// API key environment variable name (default: API_KEY)
    #[serde(default = "default_api_key_env")]
    pub api_key_env: String,

    /// Authentication method
    #[serde(default)]
    pub auth_method: AuthMethod,

    /// Custom headers for HTTP-based MCP servers
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

fn default_api_key_env() -> String {
    "API_KEY".to_string()
}

/// Authentication method for MCP servers
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// No authentication
    #[default]
    None,

    /// API key in environment variable
    EnvVar,

    /// Bearer token in Authorization header (for HTTP-based MCP)
    BearerToken,

    /// Custom header (specify in headers field)
    CustomHeader,
}

/// External MCP server tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    pub server_name: String,
}

/// External MCP client
pub struct ExternalMcpClient {
    config: ExternalMcpConfig,
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    tools: RwLock<Vec<ExternalTool>>,
    next_id: RwLock<u64>,
}

impl ExternalMcpClient {
    /// Create new external MCP client
    pub fn new(config: ExternalMcpConfig) -> Self {
        Self {
            config,
            process: None,
            stdin: None,
            stdout: None,
            tools: RwLock::new(Vec::new()),
            next_id: RwLock::new(1),
        }
    }

    /// Start the external MCP server process
    pub async fn start(&mut self) -> Result<()> {
        let start_time = std::time::Instant::now();
        tracing::info!("Starting external MCP server: {}", self.config.name);

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        // Add base environment variables
        cmd.envs(&self.config.env);

        // Handle API key authentication
        if let Some(api_key) = &self.config.api_key {
            match self.config.auth_method {
                AuthMethod::None => {
                    tracing::debug!("API key provided but auth_method is None");
                }
                AuthMethod::EnvVar => {
                    tracing::debug!("Setting API key in env var: {}", self.config.api_key_env);
                    cmd.env(&self.config.api_key_env, api_key);
                }
                AuthMethod::BearerToken | AuthMethod::CustomHeader => {
                    tracing::debug!("API key will be used in HTTP headers (not env)");
                    // For HTTP-based MCP, headers are handled at protocol level
                }
            }
        }

        let mut child = cmd
            .spawn()
            .context(format!("Failed to spawn MCP server: {}", self.config.name))?;

        let stdin = child.stdin.take().context("Failed to open stdin")?;
        let stdout = child.stdout.take().context("Failed to open stdout")?;

        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.process = Some(child);

        // Initialize the MCP server with timeout and retry logic
        let init_start = std::time::Instant::now();
        let max_retries = 3;
        let mut retry_count = 0;

        let init_result = loop {
            match tokio::time::timeout(std::time::Duration::from_secs(10), self.initialize()).await {
                Ok(Ok(_)) => {
                    let init_duration = init_start.elapsed();
                    tracing::info!("External MCP server initialized in {:.2}s", init_duration.as_secs_f32());
                    break Ok(());
                }
                Ok(Err(e)) => {
                    tracing::error!("Failed to initialize external MCP server {}: {}", self.config.name, e);
                    break Err(e);
                }
                Err(_) => {
                    retry_count += 1;
                    if retry_count >= max_retries {
                        tracing::error!("External MCP server {} initialization timed out after {} attempts", self.config.name, max_retries);
                        break Err(anyhow::anyhow!("Initialization timeout after {} attempts", max_retries));
                    }
                    tracing::warn!("Initialization attempt {} timed out, retrying...", retry_count);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }
            }
        };

        if let Err(e) = init_result {
            return Err(e);
        }

        // List available tools with timeout
        let tools_start = std::time::Instant::now();
        let tools_result = tokio::time::timeout(std::time::Duration::from_secs(15), self.refresh_tools()).await;

        match tools_result {
            Ok(Ok(_)) => {
                let tools_duration = tools_start.elapsed();
                tracing::info!("External MCP server tools loaded in {:.2}s", tools_duration.as_secs_f32());
            }
            Ok(Err(e)) => {
                tracing::error!("Failed to load tools from external MCP server {}: {}", self.config.name, e);
                return Err(e);
            }
            Err(_) => {
                tracing::error!("External MCP server {} tools loading timed out (15s)", self.config.name);
                return Err(anyhow::anyhow!("Tools loading timeout"));
            }
        }

        let total_duration = start_time.elapsed();
        tracing::info!(
            "External MCP server started: {} ({} tools) in {:.2}s total",
            self.config.name,
            self.tools.read().await.len(),
            total_duration.as_secs_f32()
        );

        if total_duration.as_secs() > 5 {
            tracing::warn!("External MCP server {} took longer than expected to start (>5s). Consider optimizing or checking for startup issues.", self.config.name);
        }

        Ok(())
    }

    /// Initialize the MCP server
    async fn initialize(&mut self) -> Result<()> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_id().await,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "op-dbus-mcp-aggregator",
                    "version": "0.1.0"
                }
            }
        });

        let response = self.send_request(request).await?;

        if response.get("error").is_some() {
            anyhow::bail!("Failed to initialize MCP server: {:?}", response);
        }

        tracing::debug!("MCP server initialized: {}", self.config.name);
        Ok(())
    }

    /// Refresh tools list from the MCP server
    pub async fn refresh_tools(&mut self) -> Result<()> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_id().await,
            "method": "tools/list",
            "params": {}
        });

        let response = self.send_request(request).await?;

        if let Some(error) = response.get("error") {
            anyhow::bail!("Failed to list tools: {:?}", error);
        }

        let tools_array = response
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
            .context("Invalid tools response")?;

        let mut tools = Vec::new();
        for tool in tools_array {
            let name = tool
                .get("name")
                .and_then(|n| n.as_str())
                .context("Tool missing name")?;
            let description = tool
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            let input_schema = tool.get("inputSchema").cloned().unwrap_or(json!({}));

            tools.push(ExternalTool {
                name: format!("{}:{}", self.config.name, name),
                description: format!("[{}] {}", self.config.name, description),
                input_schema,
                server_name: self.config.name.clone(),
            });
        }

        *self.tools.write().await = tools;
        Ok(())
    }

    /// Get all tools from this MCP server
    pub async fn get_tools(&self) -> Vec<ExternalTool> {
        self.tools.read().await.clone()
    }

    /// Call a tool on the external MCP server
    pub async fn call_tool(&mut self, tool_name: &str, arguments: Value) -> Result<Value> {
        // Strip server prefix if present
        let tool_name = tool_name
            .strip_prefix(&format!("{}:", self.config.name))
            .unwrap_or(tool_name);

        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_id().await,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        let response = self.send_request(request).await?;

        if let Some(error) = response.get("error") {
            anyhow::bail!("Tool call failed: {:?}", error);
        }

        response
            .get("result")
            .cloned()
            .context("Missing result in response")
    }

    /// Send request to MCP server and get response
    async fn send_request(&mut self, request: Value) -> Result<Value> {
        let stdin = self.stdin.as_mut().context("MCP server not started")?;
        let stdout = self.stdout.as_mut().context("MCP server not started")?;

        // Send request
        let request_str = simd_json::to_string(&request)?;
        stdin.write_all(request_str.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        tracing::debug!("Sent request to {}: {}", self.config.name, request_str);

        // Read response
        let mut response_line = String::new();
        stdout.read_line(&mut response_line).await?;

        tracing::debug!(
            "Received response from {}: {}",
            self.config.name,
            response_line
        );

        let response: Value =
            simd_json::from_str(&response_line).context("Failed to parse MCP response")?;

        Ok(response)
    }

    /// Get next request ID
    async fn next_id(&self) -> u64 {
        let mut id = self.next_id.write().await;
        let current = *id;
        *id += 1;
        current
    }

    /// Stop the MCP server
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            tracing::info!("Stopping external MCP server: {}", self.config.name);
            process.kill().await?;
        }
        Ok(())
    }
}

impl Drop for ExternalMcpClient {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.start_kill();
        }
    }
}

/// External MCP manager - manages multiple external MCP servers
pub struct ExternalMcpManager {
    clients: RwLock<HashMap<String, ExternalMcpClient>>,
}

impl ExternalMcpManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
        }
    }

    /// Add and start an external MCP server
    pub async fn add_server(&self, config: ExternalMcpConfig) -> Result<()> {
        let name = config.name.clone();
        let mut client = ExternalMcpClient::new(config);

        client.start().await?;

        self.clients.write().await.insert(name, client);
        Ok(())
    }

    /// Load servers from config file
    pub async fn load_from_file(&self, path: &str) -> Result<()> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context("Failed to read MCP config file")?;

        let configs: Vec<ExternalMcpConfig> =
            simd_json::from_str(&content).context("Failed to parse MCP config")?;

        for config in configs {
            if let Err(e) = self.add_server(config.clone()).await {
                tracing::error!("Failed to start MCP server {}: {}", config.name, e);
            }
        }

        Ok(())
    }

    /// Get all tools from all external MCP servers
    pub async fn get_all_tools(&self) -> Vec<ExternalTool> {
        let clients = self.clients.read().await;
        let mut all_tools = Vec::new();

        for client in clients.values() {
            all_tools.extend(client.get_tools().await);
        }

        all_tools
    }

    /// Call a tool (format: "server:tool" or just "tool")
    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        let (server_name, actual_tool_name) = if let Some(idx) = tool_name.find(':') {
            (&tool_name[..idx], &tool_name[idx + 1..])
        } else {
            // Try to find which server has this tool
            return Err(anyhow::anyhow!(
                "Tool name must include server prefix: server:tool"
            ));
        };

        let mut clients = self.clients.write().await;
        let client = clients
            .get_mut(server_name)
            .context(format!("MCP server not found: {}", server_name))?;

        client.call_tool(actual_tool_name, arguments).await
    }

    /// Stop all MCP servers
    pub async fn stop_all(&self) -> Result<()> {
        let mut clients = self.clients.write().await;
        for (name, client) in clients.iter_mut() {
            if let Err(e) = client.stop().await {
                tracing::error!("Failed to stop MCP server {}: {}", name, e);
            }
        }
        clients.clear();
        Ok(())
    }
}

impl Default for ExternalMcpManager {
    fn default() -> Self {
        Self::new()
    }
}
