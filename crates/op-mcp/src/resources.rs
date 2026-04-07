//! Resource Registry for MCP
//!
//! Provides documentation resources served via MCP resources protocol.

use serde::{Deserialize, Serialize};

/// Resource information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// Resource registry
pub struct ResourceRegistry {
    resources: Vec<ResourceInfo>,
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceRegistry {
    pub fn new() -> Self {
        let resources = vec![
            ResourceInfo {
                uri: "docs://system-prompt".to_string(),
                name: "System Prompt".to_string(),
                description: Some("System prompt for op-mcp".to_string()),
                mime_type: Some("text/plain".to_string()),
            },
            ResourceInfo {
                uri: "docs://architecture".to_string(),
                name: "Architecture".to_string(),
                description: Some("System architecture documentation".to_string()),
                mime_type: Some("text/markdown".to_string()),
            },
        ];
        Self { resources }
    }

    pub fn add_resource(&mut self, resource: ResourceInfo) {
        self.resources.push(resource);
    }

    pub fn list_resources(&self) -> &[ResourceInfo] {
        &self.resources
    }

    pub fn get_resource(&self, uri: &str) -> Option<&ResourceInfo> {
        self.resources.iter().find(|r| r.uri == uri)
    }

    pub async fn read_resource(&self, uri: &str) -> Option<String> {
        match uri {
            "docs://system-prompt" => Some(self.generate_system_prompt().await),
            "docs://architecture" => Some(ARCHITECTURE_DOC.to_string()),
            _ => None,
        }
    }

    async fn generate_system_prompt(&self) -> String {
        // Try to get from op_chat if available
        #[cfg(feature = "op-chat")]
        {
            let msg = op_chat::generate_system_prompt().await;
            return msg.content;
        }

        #[cfg(not(feature = "op-chat"))]
        {
            "You are a helpful assistant with access to system tools.".to_string()
        }
    }
}

const ARCHITECTURE_DOC: &str = r#"# op-mcp Architecture

## Overview

op-mcp is a unified MCP (Model Context Protocol) server supporting multiple transports:

- **Stdio**: Standard input/output for CLI integration
- **HTTP**: REST endpoints with SSE support
- **WebSocket**: Full-duplex communication
- **gRPC**: High-performance RPC (optional)

## Components

### McpServer
Core server handling all MCP protocol logic. Transport-agnostic.

### Transport Layer
Abstract `Transport` trait with implementations for each protocol.

### Tool System
`ToolExecutor` trait allows pluggable tool backends.

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `compact_mode` | false | Use 4 meta-tools instead of all |
| `max_tools` | 500 | Maximum tools to expose |
| `blocked_patterns` | [...] | Tool patterns to block |
"#;
