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
examples/
  aggregator.json
src/
  unused/
    context.rs
  aggregator.rs
  cache.rs
  client.rs
  compact.rs
  config.rs
  groups.rs
  groups.rs.patch
  lib.rs
  profile.rs
Cargo.toml
CLEANUP-CONTEXT-AWARE.md
compare-op-mcp-aggregator.md
README.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="examples/aggregator.json">
{
  "_comment": "MCP Aggregator Configuration",
  "_docs": "See README.md for full documentation",
  
  "servers": [
    {
      "id": "local",
      "name": "Local op-dbus",
      "url": "http://localhost:3001",
      "transport": "sse",
      "enabled": true,
      "priority": 100
    },
    {
      "id": "github",
      "name": "GitHub MCP",
      "url": "http://localhost:3002",
      "transport": "sse",
      "enabled": false,
      "tool_prefix": "github",
      "include_tools": [
        "search_repositories",
        "get_file_contents",
        "search_code",
        "list_commits"
      ],
      "priority": 50,
      "auth": {
        "type": "bearer",
        "token": "${GITHUB_TOKEN}"
      }
    }
  ],
  
  "profiles": {
    "default": {
      "description": "Default profile with local tools",
      "servers": ["local"],
      "max_tools": 40
    },
    "sysadmin": {
      "description": "System administration - D-Bus, systemd, network",
      "servers": ["local"],
      "include_namespaces": ["system", "systemd", "network", "dbus"],
      "max_tools": 35
    },
    "dev": {
      "description": "Development - GitHub, filesystem, shell",
      "servers": ["local", "github"],
      "include_categories": ["filesystem", "shell", "git"],
      "include_tools": ["github_*"],
      "max_tools": 35
    }
  },
  
  "tool_groups": {
    "_comment": "Toggle tool groups to stay under limits",
    "enabled": ["core", "shell", "systemd", "network"],
    "presets": {
      "minimal": ["core"],
      "sysadmin": ["core", "shell", "systemd", "network", "monitoring"],
      "developer": ["core", "shell", "filesystem", "git"],
      "network-admin": ["core", "shell", "network", "ovs", "dbus"]
    }
  },
  
  "compact_mode": {
    "_comment": "Reduces 750+ tools to 4 meta-tools for LLM efficiency",
    "enabled": true,
    "include_list": true,
    "include_execute": true,
    "include_schema": true,
    "include_search": true,
    "include_batch": false,
    "max_list_results": 50
  },
  
  "client_detection": {
    "_comment": "Auto-detect client type and select optimal mode",
    "enabled": true,
    "compact_mode_clients": [
      "gemini", "gemini-cli", "@google/gemini",
      "claude", "anthropic",
      "chatgpt", "openai", "gpt",
      "chatbot", "op-chat", "llm"
    ],
    "full_mode_clients": [
      "cursor", "vscode", "api", "direct"
    ],
    "default_mode": "compact"
  },
  
  "cache": {
    "schema_ttl_secs": 300,
    "max_entries": 1000,
    "background_refresh": true
  },
  
  "default_profile": "default",
  "default_mode": "compact",
  "max_tools_per_profile": 40
}
</file>

<file path="src/unused/context.rs">
//! Context-Aware Tool Loading
//!
//! Dynamically suggests and enables tool groups based on conversation context.
//! This bridges Compact Mode (lazy loading) with Tool Groups (domain organization).
//!
//! ## How It Works
//!
//! 1. **Analyze Context**: Extract signals from messages, files, commands
//! 2. **Match Groups**: Map context signals to relevant tool groups  
//! 3. **Suggest/Auto-Enable**: Recommend or auto-enable groups within limit
//!
//! ## Context Signals
//!
//! - File paths: `.service` → systemd, `.py` → python, `Dockerfile` → containers
//! - Keywords: "nginx" → services, "database" → db-read
//! - Commands: Recent `git` commands → git-read/git-write
//! - D-Bus paths: Specific services → dbus-intro
//! - Intent: "restart", "stop" → service-control

use crate::groups::{ToolGroups, ToolGroup, SecurityLevel, AccessZone};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};

/// Context signals extracted from conversation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationContext {
    /// File paths mentioned or being worked on
    pub files: Vec<String>,
    /// Keywords extracted from messages
    pub keywords: Vec<String>,
    /// Commands recently executed
    pub recent_commands: Vec<String>,
    /// D-Bus services mentioned
    pub dbus_services: Vec<String>,
    /// Detected intent (e.g., "read", "modify", "debug", "deploy")
    pub intent: Option<String>,
    /// Explicit domain request (e.g., user says "I'm working on networking")
    pub explicit_domain: Option<String>,
    /// Current working directory
    pub cwd: Option<String>,
    /// Open files in editor
    pub open_files: Vec<String>,
}

impl ConversationContext {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add a file path to context
    pub fn with_file(mut self, path: &str) -> Self {
        self.files.push(path.to_string());
        self
    }
    
    /// Add keywords from a message
    pub fn with_keywords(mut self, keywords: Vec<&str>) -> Self {
        self.keywords.extend(keywords.into_iter().map(String::from));
        self
    }
    
    /// Add a recent command
    pub fn with_command(mut self, cmd: &str) -> Self {
        self.recent_commands.push(cmd.to_string());
        self
    }
    
    /// Set intent
    pub fn with_intent(mut self, intent: &str) -> Self {
        self.intent = Some(intent.to_string());
        self
    }
    
    /// Set explicit domain
    pub fn for_domain(mut self, domain: &str) -> Self {
        self.explicit_domain = Some(domain.to_string());
        self
    }
    
    /// Extract context from a user message
    pub fn from_message(message: &str) -> Self {
        let mut ctx = Self::new();
        let lower = message.to_lowercase();
        
        // Extract file paths
        for word in message.split_whitespace() {
            if word.contains('/') || word.contains('.') {
                if looks_like_path(word) {
                    ctx.files.push(word.trim_matches(|c| c == '"' || c == '\'').to_string());
                }
            }
        }
        
        // Extract keywords
        let keywords: Vec<&str> = CONTEXT_KEYWORDS.iter()
            .filter(|&&kw| lower.contains(kw))
            .copied()
            .collect();
        ctx.keywords = keywords.into_iter().map(String::from).collect();
        
        // Detect intent
        ctx.intent = detect_intent(&lower);
        
        // Detect explicit domain
        ctx.explicit_domain = detect_domain(&lower);
        
        ctx
    }
    
    /// Merge with another context
    pub fn merge(&mut self, other: &ConversationContext) {
        self.files.extend(other.files.clone());
        self.keywords.extend(other.keywords.clone());
        self.recent_commands.extend(other.recent_commands.clone());
        self.dbus_services.extend(other.dbus_services.clone());
        if other.intent.is_some() {
            self.intent = other.intent.clone();
        }
        if other.explicit_domain.is_some() {
            self.explicit_domain = other.explicit_domain.clone();
        }
    }
}

/// Keywords that signal certain domains
const CONTEXT_KEYWORDS: &[&str] = &[
    // Systemd
    "service", "systemd", "unit", "daemon", "journalctl", "systemctl",
    // Network
    "network", "ip", "interface", "bridge", "route", "dns", "firewall",
    // Git
    "git", "commit", "branch", "merge", "pull", "push",
    // Containers
    "docker", "container", "kubernetes", "k8s", "pod", "deployment",
    // Database
    "database", "sql", "query", "table", "postgresql", "mysql", "mongodb",
    // Files
    "file", "directory", "folder", "read", "write", "create", "delete",
    // Security
    "security", "auth", "password", "secret", "certificate", "ssl", "tls",
    // D-Bus
    "dbus", "bus", "introspect",
    // OVS
    "ovs", "openvswitch", "vswitch",
];

fn looks_like_path(s: &str) -> bool {
    let trimmed = s.trim_matches(|c| c == '"' || c == '\'' || c == '`');
    trimmed.starts_with('/') || 
    trimmed.starts_with("./") ||
    trimmed.starts_with("../") ||
    trimmed.starts_with("~") ||
    (trimmed.contains('.') && !trimmed.contains(' '))
}

fn detect_intent(message: &str) -> Option<String> {
    if message.contains("restart") || message.contains("stop") || message.contains("start") || message.contains("enable") {
        Some("control".to_string())
    } else if message.contains("deploy") || message.contains("release") || message.contains("rollback") {
        Some("deploy".to_string())
    } else if message.contains("debug") || message.contains("troubleshoot") || message.contains("investigate") {
        Some("debug".to_string())
    } else if message.contains("monitor") || message.contains("watch") || message.contains("track") {
        Some("monitor".to_string())
    } else if message.contains("configure") || message.contains("setup") || message.contains("install") {
        Some("configure".to_string())
    } else if message.contains("list") || message.contains("show") || message.contains("get") || message.contains("read") {
        Some("read".to_string())
    } else if message.contains("create") || message.contains("write") || message.contains("add") || message.contains("modify") {
        Some("write".to_string())
    } else {
        None
    }
}

fn detect_domain(message: &str) -> Option<String> {
    // Explicit domain mentions
    if message.contains("working on network") || message.contains("networking") {
        Some("network".to_string())
    } else if message.contains("working on systemd") || message.contains("services") {
        Some("systemd".to_string())
    } else if message.contains("working on database") || message.contains("sql") {
        Some("database".to_string())
    } else if message.contains("working on docker") || message.contains("containers") {
        Some("devops".to_string())
    } else if message.contains("working on security") {
        Some("security".to_string())
    } else if message.contains("working on git") {
        Some("git".to_string())
    } else {
        None
    }
}

/// Suggested groups based on context analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSuggestion {
    /// Suggested group ID
    pub group_id: String,
    /// Group name for display
    pub group_name: String,
    /// Why this was suggested
    pub reason: String,
    /// Confidence score (0-100)
    pub confidence: u8,
    /// Tools this would add
    pub estimated_tools: usize,
    /// Auto-enable recommended?
    pub auto_enable: bool,
}

/// Context-aware tool manager
#[derive(Debug, Clone)]
pub struct ContextAwareTools {
    /// Accumulated context
    context: ConversationContext,
    /// File extension → group mapping
    file_mappings: HashMap<String, Vec<String>>,
    /// Keyword → group mapping
    keyword_mappings: HashMap<String, Vec<String>>,
    /// Intent → group mapping
    intent_mappings: HashMap<String, Vec<String>>,
    /// Maximum tools limit
    max_tools: usize,
    /// Currently enabled groups
    enabled: HashSet<String>,
}

impl ContextAwareTools {
    pub fn new(max_tools: usize) -> Self {
        Self {
            context: ConversationContext::new(),
            file_mappings: build_file_mappings(),
            keyword_mappings: build_keyword_mappings(),
            intent_mappings: build_intent_mappings(),
            max_tools,
            enabled: HashSet::new(),
        }
    }
    
    /// Update context from a message
    pub fn observe_message(&mut self, message: &str) {
        let new_ctx = ConversationContext::from_message(message);
        self.context.merge(&new_ctx);
        debug!("Updated context: {:?}", self.context);
    }
    
    /// Update context from file paths being edited
    pub fn observe_files(&mut self, files: &[String]) {
        self.context.files.extend(files.iter().cloned());
    }
    
    /// Update context from a command execution
    pub fn observe_command(&mut self, command: &str) {
        self.context.recent_commands.push(command.to_string());
        
        // Extract command type for keyword matching
        if let Some(cmd) = command.split_whitespace().next() {
            self.context.keywords.push(cmd.to_string());
        }
    }
    
    /// Suggest tool groups based on current context
    pub fn suggest_groups(&self, tool_groups: &ToolGroups) -> Vec<ContextSuggestion> {
        let mut suggestions: HashMap<String, ContextSuggestion> = HashMap::new();
        
        // 1. File-based suggestions
        for file in &self.context.files {
            let ext = file.rsplit('.').next().unwrap_or("");
            if let Some(groups) = self.file_mappings.get(ext) {
                for group_id in groups {
                    let entry = suggestions.entry(group_id.clone()).or_insert_with(|| {
                        ContextSuggestion {
                            group_id: group_id.clone(),
                            group_name: group_id.clone(), // Will be updated
                            reason: String::new(),
                            confidence: 0,
                            estimated_tools: 0,
                            auto_enable: false,
                        }
                    });
                    entry.confidence = entry.confidence.saturating_add(30);
                    if entry.reason.is_empty() {
                        entry.reason = format!("File '{}' suggests {}", file, group_id);
                    }
                }
            }
        }
        
        // 2. Keyword-based suggestions
        for keyword in &self.context.keywords {
            if let Some(groups) = self.keyword_mappings.get(keyword.to_lowercase().as_str()) {
                for group_id in groups {
                    let entry = suggestions.entry(group_id.clone()).or_insert_with(|| {
                        ContextSuggestion {
                            group_id: group_id.clone(),
                            group_name: group_id.clone(),
                            reason: String::new(),
                            confidence: 0,
                            estimated_tools: 0,
                            auto_enable: false,
                        }
                    });
                    entry.confidence = entry.confidence.saturating_add(25);
                    if entry.reason.is_empty() {
                        entry.reason = format!("Keyword '{}' suggests {}", keyword, group_id);
                    }
                }
            }
        }
        
        // 3. Intent-based suggestions
        if let Some(intent) = &self.context.intent {
            if let Some(groups) = self.intent_mappings.get(intent.as_str()) {
                for group_id in groups {
                    let entry = suggestions.entry(group_id.clone()).or_insert_with(|| {
                        ContextSuggestion {
                            group_id: group_id.clone(),
                            group_name: group_id.clone(),
                            reason: String::new(),
                            confidence: 0,
                            estimated_tools: 0,
                            auto_enable: false,
                        }
                    });
                    entry.confidence = entry.confidence.saturating_add(20);
                    if entry.reason.is_empty() {
                        entry.reason = format!("Intent '{}' suggests {}", intent, group_id);
                    }
                }
            }
        }
        
        // 4. Explicit domain request (highest confidence)
        if let Some(domain) = &self.context.explicit_domain {
            for group in tool_groups.list_by_domain(domain) {
                let entry = suggestions.entry(group.id.clone()).or_insert_with(|| {
                    ContextSuggestion {
                        group_id: group.id.clone(),
                        group_name: group.name.clone(),
                        reason: String::new(),
                        confidence: 0,
                        estimated_tools: 0,
                        auto_enable: false,
                    }
                });
                entry.confidence = entry.confidence.saturating_add(50);
                entry.auto_enable = true;
                if entry.reason.is_empty() {
                    entry.reason = format!("Working on {} domain", domain);
                }
            }
        }
        
        // Update group metadata and filter
        let mut result: Vec<_> = suggestions.into_iter()
            .filter_map(|(id, mut suggestion)| {
                // Get actual group info
                let all_groups = tool_groups.list_all();
                if let Some(status) = all_groups.iter().find(|g| g.id == id) {
                    suggestion.group_name = status.name.clone();
                    suggestion.estimated_tools = status.estimated_count;
                    
                    // Skip if already enabled
                    if status.enabled || self.enabled.contains(&id) {
                        return None;
                    }
                    
                    // Auto-enable if high confidence
                    suggestion.auto_enable = suggestion.confidence >= 70;
                    
                    Some(suggestion)
                } else {
                    None
                }
            })
            .collect();
        
        // Sort by confidence
        result.sort_by(|a, b| b.confidence.cmp(&a.confidence));
        result.truncate(10); // Top 10 suggestions
        
        result
    }
    
    /// Auto-enable groups based on context (respects tool limit)
    pub fn auto_enable(&mut self, tool_groups: &mut ToolGroups) -> Vec<String> {
        let suggestions = self.suggest_groups(tool_groups);
        let mut enabled = Vec::new();
        
        for suggestion in suggestions {
            if suggestion.auto_enable && tool_groups.remaining_capacity() >= suggestion.estimated_tools {
                if tool_groups.try_enable(&suggestion.group_id) {
                    self.enabled.insert(suggestion.group_id.clone());
                    enabled.push(suggestion.group_id);
                    info!("🧠 Auto-enabled '{}' based on context: {}", 
                          suggestion.group_name, suggestion.reason);
                }
            }
        }
        
        enabled
    }
    
    /// Get current context
    pub fn context(&self) -> &ConversationContext {
        &self.context
    }
    
    /// Clear context (e.g., new conversation)
    pub fn clear_context(&mut self) {
        self.context = ConversationContext::new();
        self.enabled.clear();
    }
}

fn build_file_mappings() -> HashMap<String, Vec<String>> {
    let mut m = HashMap::new();
    
    // Systemd
    m.insert("service".into(), vec!["services".into(), "service-control".into()]);
    m.insert("socket".into(), vec!["services".into()]);
    m.insert("timer".into(), vec!["services".into()]);
    m.insert("target".into(), vec!["services".into()]);
    
    // Git
    m.insert("gitignore".into(), vec!["git-read".into()]);
    
    // Shell
    m.insert("sh".into(), vec!["shell-safe".into()]);
    m.insert("bash".into(), vec!["shell-safe".into()]);
    
    // Config files
    m.insert("json".into(), vec!["read".into()]);
    m.insert("yaml".into(), vec!["read".into()]);
    m.insert("yml".into(), vec!["read".into()]);
    m.insert("toml".into(), vec!["read".into()]);
    m.insert("conf".into(), vec!["read".into()]);
    
    // Docker
    m.insert("Dockerfile".into(), vec!["containers".into()]);
    m.insert("dockerignore".into(), vec!["containers".into()]);
    
    // Kubernetes
    m.insert("k8s".into(), vec!["k8s-read".into()]);
    
    // SQL
    m.insert("sql".into(), vec!["db-read".into()]);
    
    // Network
    m.insert("network".into(), vec!["network-info".into()]);
    m.insert("firewall".into(), vec!["firewall".into()]);
    
    // Logs
    m.insert("log".into(), vec!["logs".into()]);
    
    m
}

fn build_keyword_mappings() -> HashMap<String, Vec<String>> {
    let mut m = HashMap::new();
    
    // Systemd
    m.insert("systemd".into(), vec!["services".into(), "journals".into()]);
    m.insert("service".into(), vec!["services".into()]);
    m.insert("systemctl".into(), vec!["services".into(), "service-control".into()]);
    m.insert("journalctl".into(), vec!["journals".into()]);
    
    // Network
    m.insert("network".into(), vec!["network-info".into()]);
    m.insert("interface".into(), vec!["network-info".into()]);
    m.insert("bridge".into(), vec!["network-info".into(), "ovs-info".into()]);
    m.insert("firewall".into(), vec!["firewall".into()]);
    m.insert("dns".into(), vec!["network-diag".into()]);
    
    // Git
    m.insert("git".into(), vec!["git-read".into()]);
    m.insert("commit".into(), vec!["git-write".into()]);
    m.insert("branch".into(), vec!["git-read".into(), "git-write".into()]);
    
    // Containers
    m.insert("docker".into(), vec!["containers".into()]);
    m.insert("container".into(), vec!["containers".into()]);
    m.insert("kubernetes".into(), vec!["k8s-read".into()]);
    m.insert("k8s".into(), vec!["k8s-read".into()]);
    m.insert("pod".into(), vec!["k8s-read".into()]);
    
    // Database
    m.insert("database".into(), vec!["db-read".into()]);
    m.insert("sql".into(), vec!["db-read".into()]);
    m.insert("query".into(), vec!["db-read".into()]);
    m.insert("postgresql".into(), vec!["db-read".into()]);
    m.insert("mysql".into(), vec!["db-read".into()]);
    
    // D-Bus
    m.insert("dbus".into(), vec!["dbus-intro".into()]);
    m.insert("bus".into(), vec!["dbus-intro".into()]);
    m.insert("introspect".into(), vec!["dbus-intro".into()]);
    
    // OVS
    m.insert("ovs".into(), vec!["ovs-info".into()]);
    m.insert("openvswitch".into(), vec!["ovs-info".into()]);
    
    // Security
    m.insert("security".into(), vec!["auth".into(), "audit".into()]);
    m.insert("auth".into(), vec!["auth".into()]);
    m.insert("password".into(), vec!["auth".into()]);
    m.insert("secret".into(), vec!["secrets".into()]);
    
    // Monitoring
    m.insert("monitor".into(), vec!["monitoring".into()]);
    m.insert("cpu".into(), vec!["monitoring".into()]);
    m.insert("memory".into(), vec!["monitoring".into()]);
    m.insert("disk".into(), vec!["monitoring".into()]);
    
    // Files
    m.insert("file".into(), vec!["read".into()]);
    m.insert("read".into(), vec!["read".into()]);
    m.insert("search".into(), vec!["search".into()]);
    
    m
}

fn build_intent_mappings() -> HashMap<String, Vec<String>> {
    let mut m = HashMap::new();
    
    // Read operations
    m.insert("read".into(), vec!["read".into(), "info".into()]);
    
    // Write operations
    m.insert("write".into(), vec!["write".into()]);
    
    // Control operations
    m.insert("control".into(), vec!["service-control".into(), "process-control".into()]);
    
    // Debug operations
    m.insert("debug".into(), vec!["logs".into(), "journals".into(), "monitoring".into()]);
    
    // Deploy operations
    m.insert("deploy".into(), vec!["deploy".into(), "containers".into()]);
    
    // Monitor operations
    m.insert("monitor".into(), vec!["monitoring".into(), "logs".into()]);
    
    // Configure operations
    m.insert("configure".into(), vec!["service-config".into(), "network-config".into()]);
    
    m
}

/// Response format for context-aware suggestions
#[derive(Debug, Serialize, Deserialize)]
pub struct ContextResponse {
    /// Current accumulated context
    pub context: ConversationContext,
    /// Suggested groups
    pub suggestions: Vec<ContextSuggestion>,
    /// Auto-enabled groups
    pub auto_enabled: Vec<String>,
    /// Current tool count
    pub current_tools: usize,
    /// Remaining capacity
    pub remaining_capacity: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groups::ToolGroups;
    
    #[test]
    fn test_context_from_message() {
        let ctx = ConversationContext::from_message(
            "I need to restart the nginx service and check the logs"
        );
        
        assert!(ctx.keywords.contains(&"service".to_string()));
        assert_eq!(ctx.intent, Some("control".to_string()));
    }
    
    #[test]
    fn test_file_path_detection() {
        let ctx = ConversationContext::from_message(
            "Please edit /etc/systemd/system/myapp.service"
        );
        
        assert!(ctx.files.iter().any(|f| f.contains("myapp.service")));
    }
    
    #[test]
    fn test_context_suggestions() {
        let groups = ToolGroups::new();
        let mut ctx_tools = ContextAwareTools::new(40);
        
        ctx_tools.observe_message("I want to check the systemd services");
        let suggestions = ctx_tools.suggest_groups(&groups);
        
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.group_id == "services"));
    }
    
    #[test]
    fn test_explicit_domain() {
        let groups = ToolGroups::new();
        let mut ctx_tools = ContextAwareTools::new(40);
        
        ctx_tools.observe_message("I'm working on networking today");
        let suggestions = ctx_tools.suggest_groups(&groups);
        
        // Should suggest network groups with high confidence
        assert!(suggestions.iter().any(|s| s.group_id == "network-info"));
    }
    
    #[test]
    fn test_auto_enable() {
        let mut groups = ToolGroups::new().with_limit(40).from_ip("127.0.0.1");
        let mut ctx_tools = ContextAwareTools::new(40);
        
        // Strong signal should auto-enable
        ctx_tools.context.explicit_domain = Some("systemd".to_string());
        ctx_tools.context.intent = Some("read".to_string());
        ctx_tools.context.keywords.push("service".to_string());
        ctx_tools.context.keywords.push("systemctl".to_string());
        
        let enabled = ctx_tools.auto_enable(&mut groups);
        
        // Should have auto-enabled some systemd groups
        assert!(!enabled.is_empty());
    }
}
</file>

<file path="src/aggregator.rs">
//! Main Aggregator - ties together clients, cache, and profiles
//!
//! This is the primary interface for the MCP aggregator.

use crate::cache::{cache_maintenance_loop, ToolCache};
use crate::client::{ClientManager, McpClient, ToolDefinition};
use crate::config::AggregatorConfig;
use crate::profile::ProfileManager;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// Re-export ToolMode from config
pub use crate::config::ToolMode;

/// The main MCP Aggregator
pub struct Aggregator {
    /// Configuration
    config: AggregatorConfig,
    /// Client manager
    clients: Arc<ClientManager>,
    /// Tool cache
    cache: Arc<ToolCache>,
    /// Profile manager
    profiles: Arc<ProfileManager>,
    /// Whether the aggregator is initialized
    initialized: RwLock<bool>,
    /// Current client info (set during initialize)
    client_info: RwLock<Option<ClientInfo>>,
    /// Detected tool mode for current client
    detected_mode: RwLock<Option<ToolMode>>,
}

/// Client information from MCP initialize request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: Option<String>,
}

impl Aggregator {
    /// Create a new aggregator from configuration
    pub async fn new(config: AggregatorConfig) -> Result<Self> {
        let cache = Arc::new(ToolCache::new(
            config.cache.max_entries,
            config.cache.schema_ttl(),
        ));

        let clients = Arc::new(ClientManager::new());
        let profiles = Arc::new(ProfileManager::new(&config, cache.clone()));

        let aggregator = Self {
            config,
            clients,
            cache,
            profiles,
            initialized: RwLock::new(false),
            client_info: RwLock::new(None),
            detected_mode: RwLock::new(None),
        };

        Ok(aggregator)
    }

    /// Create from default configuration
    pub async fn from_default_config() -> Result<Self> {
        let config = AggregatorConfig::load_default()?;
        Self::new(config).await
    }

    /// Initialize the aggregator (connects to all servers)
    pub async fn initialize(&self) -> Result<()> {
        if *self.initialized.read().await {
            return Ok(());
        }

        info!(
            "Initializing MCP aggregator with {} servers",
            self.config.servers.len()
        );

        // Create clients for each configured server
        for server_config in &self.config.servers {
            if !server_config.enabled {
                info!("Skipping disabled server: {}", server_config.name);
                continue;
            }

            match McpClient::new(server_config.clone()) {
                Ok(client) => {
                    let client = Arc::new(client);

                    // Try to connect and fetch tools
                    match client.list_tools().await {
                        Ok(tools) => {
                            info!(
                                "Connected to {} with {} tools",
                                server_config.name,
                                tools.len()
                            );

                            // Cache the tools
                            self.cache.insert_batch(tools, &server_config.id).await;
                            self.clients.add_client(client).await;
                        }
                        Err(e) => {
                            error!("Failed to connect to {}: {}", server_config.name, e);
                            // Continue with other servers
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to create client for {}: {}", server_config.name, e);
                }
            }
        }

        // Start background cache maintenance if configured
        if self.config.cache.background_refresh {
            let cache = self.cache.clone();
            tokio::spawn(async move {
                cache_maintenance_loop(cache, Duration::from_secs(60)).await;
            });
        }

        *self.initialized.write().await = true;

        let stats = self.stats().await;
        info!(
            "Aggregator initialized: {} servers, {} tools cached",
            stats.connected_servers, stats.total_tools
        );

        Ok(())
    }

    /// List tools for a specific profile
    pub async fn list_tools(&self, profile_name: &str) -> Result<Vec<ToolDefinition>> {
        self.ensure_initialized().await?;
        Ok(self.profiles.get_tools_for_profile(profile_name).await)
    }

    /// List tools for the default profile
    pub async fn list_default_tools(&self) -> Result<Vec<ToolDefinition>> {
        self.list_tools(self.profiles.default_profile()).await
    }

    /// Call a tool by name
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult> {
        self.ensure_initialized().await?;

        debug!("Calling tool: {}", name);

        // Find which server owns this tool
        let server_id = self
            .cache
            .get_server_id(name)
            .await
            .ok_or_else(|| anyhow!("Tool '{}' not found in any server", name))?;

        let client = self
            .clients
            .get_client(&server_id)
            .await
            .ok_or_else(|| anyhow!("Server '{}' not connected", server_id))?;

        // Call the tool
        let result = client
            .call_tool(name, arguments.clone())
            .await
            .with_context(|| format!("Failed to call tool '{}' on server '{}'", name, server_id))?;

        Ok(ToolCallResult {
            tool_name: name.to_string(),
            server_id,
            result,
            is_error: false,
        })
    }

    /// Call a tool with profile validation
    pub async fn call_tool_in_profile(
        &self,
        name: &str,
        arguments: Value,
        profile_name: &str,
    ) -> Result<ToolCallResult> {
        // Validate tool is available in profile
        if !self
            .profiles
            .tool_available_in_profile(name, profile_name)
            .await
        {
            return Err(anyhow!(
                "Tool '{}' not available in profile '{}'",
                name,
                profile_name
            ));
        }

        self.call_tool(name, arguments).await
    }

    /// Get available profiles
    pub async fn list_profiles(&self) -> Vec<String> {
        self.profiles.list_profiles().await
    }

    /// Get the default profile name
    pub fn default_profile(&self) -> &str {
        self.profiles.default_profile()
    }

    /// Refresh tools from all servers
    pub async fn refresh(&self) -> Result<()> {
        self.ensure_initialized().await?;

        info!("Refreshing tools from all servers");

        for client in self.clients.clients().await {
            match client.list_tools().await {
                Ok(tools) => {
                    self.cache.insert_batch(tools, client.server_id()).await;
                }
                Err(e) => {
                    warn!("Failed to refresh tools from {}: {}", client.server_id(), e);
                }
            }
        }

        Ok(())
    }

    /// Get aggregator statistics
    pub async fn stats(&self) -> AggregatorStats {
        let clients = self.clients.clients().await;
        let cache_stats = self.cache.stats().await;

        AggregatorStats {
            connected_servers: clients.len(),
            total_tools: self.cache.len().await,
            cache_hits: cache_stats.hits,
            cache_misses: cache_stats.misses,
            profiles: self.profiles.list_profiles().await,
        }
    }

    /// Health check
    pub async fn health_check(&self) -> HealthStatus {
        let mut server_status = Vec::new();

        for client in self.clients.clients().await {
            let healthy = client.health_check().await;
            server_status.push(ServerHealth {
                id: client.server_id().to_string(),
                name: client.config().name.clone(),
                healthy,
            });
        }

        let all_healthy = server_status.iter().all(|s| s.healthy);

        HealthStatus {
            healthy: all_healthy,
            servers: server_status,
        }
    }

    /// Add a server dynamically
    pub async fn add_server(&self, config: crate::config::UpstreamServer) -> Result<()> {
        let client = Arc::new(McpClient::new(config.clone())?);

        let tools = client
            .list_tools()
            .await
            .with_context(|| format!("Failed to connect to {}", config.name))?;

        self.cache.insert_batch(tools, &config.id).await;
        self.clients.add_client(client).await;

        info!("Added server: {}", config.name);
        Ok(())
    }

    // =========================================================================
    // CLIENT DETECTION & TOOL MODE
    // =========================================================================

    /// Set client info from MCP initialize request (auto-detects mode)
    pub async fn set_client_info(&self, name: &str, version: Option<&str>) {
        let client_info = ClientInfo {
            name: name.to_string(),
            version: version.map(String::from),
        };

        // Auto-detect mode based on client
        let mode = self.config.client_detection.detect_mode(name);

        info!(
            "Client connected: {} (v{}) -> {:?} mode",
            name,
            version.unwrap_or("unknown"),
            mode
        );

        *self.client_info.write().await = Some(client_info);
        *self.detected_mode.write().await = Some(mode);
    }

    /// Get the current tool mode (detected or default)
    pub async fn get_tool_mode(&self) -> ToolMode {
        self.detected_mode
            .read()
            .await
            .unwrap_or(self.config.default_mode)
    }

    /// Override the tool mode manually
    pub async fn set_tool_mode(&self, mode: ToolMode) {
        *self.detected_mode.write().await = Some(mode);
        info!("Tool mode set to: {:?}", mode);
    }

    /// Get current client info
    pub async fn get_client_info(&self) -> Option<ClientInfo> {
        self.client_info.read().await.clone()
    }

    /// Check if running in compact mode
    pub async fn is_compact_mode(&self) -> bool {
        matches!(self.get_tool_mode().await, ToolMode::Compact)
    }

    /// Get MCP tools based on current mode (for tools/list response)
    ///
    /// In Compact mode: Returns 4-5 meta-tools
    /// In Full mode: Returns all tools from the profile
    /// In Hybrid mode: Returns essential tools + meta-tools
    pub async fn get_mcp_tools(&self, mode: Option<ToolMode>) -> Result<Vec<McpToolDefinition>> {
        self.ensure_initialized().await?;

        let mode = mode.unwrap_or(self.get_tool_mode().await);

        match mode {
            ToolMode::Compact => self.get_compact_tools().await,
            ToolMode::Full => self.get_full_tools().await,
            ToolMode::Hybrid => self.get_hybrid_tools().await,
        }
    }

    /// Get compact mode meta-tools
    async fn get_compact_tools(&self) -> Result<Vec<McpToolDefinition>> {
        // We need Arc<Self> for the compact tools, so we return static definitions
        // The actual execution happens via execute_tool which has aggregator access
        Ok(vec![
            McpToolDefinition {
                name: "list_tools".to_string(),
                description: "List available tools. Use 'category' or 'namespace' to filter. Returns tool names and descriptions. Call 'get_tool_schema' to get full input schema before executing.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "category": {
                            "type": "string",
                            "description": "Filter by category (e.g., 'systemd', 'network', 'filesystem')"
                        },
                        "namespace": {
                            "type": "string",
                            "description": "Filter by namespace (e.g., 'system', 'dbus', 'external')"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum tools to return (default: 20)",
                            "default": 20
                        }
                    }
                }),
            },
            McpToolDefinition {
                name: "search_tools".to_string(),
                description: "Search for tools by keyword. Searches tool names and descriptions.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum results (default: 10)",
                            "default": 10
                        }
                    },
                    "required": ["query"]
                }),
            },
            McpToolDefinition {
                name: "get_tool_schema".to_string(),
                description: "Get the full input schema for a tool. Use this before calling execute_tool to understand required arguments.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "tool_name": {
                            "type": "string",
                            "description": "Name of the tool to get schema for"
                        }
                    },
                    "required": ["tool_name"]
                }),
            },
            McpToolDefinition {
                name: "execute_tool".to_string(),
                description: "Execute any available tool by name. First use list_tools/search_tools to find tools, then get_tool_schema to see required arguments.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "tool_name": {
                            "type": "string",
                            "description": "Name of the tool to execute"
                        },
                        "arguments": {
                            "type": "object",
                            "description": "Arguments to pass to the tool"
                        }
                    },
                    "required": ["tool_name"]
                }),
            },
        ])
    }

    /// Get all tools in full mode
    async fn get_full_tools(&self) -> Result<Vec<McpToolDefinition>> {
        let profile = self.profiles.default_profile();
        let tools = self.profiles.get_tools_for_profile(profile).await;

        Ok(tools
            .into_iter()
            .map(|t| McpToolDefinition {
                name: t.name,
                description: t.description.clone(),
                input_schema: t.input_schema,
            })
            .collect())
    }

    /// Get hybrid tools (essential + meta-tools)
    async fn get_hybrid_tools(&self) -> Result<Vec<McpToolDefinition>> {
        let mut tools = Vec::new();

        // Add essential tools (respond, system_info, etc.)
        let essential = ["respond", "respond_to_user", "system_info", "shell_exec"];
        let all_tools = self.list_default_tools().await?;

        for tool in all_tools {
            if essential.contains(&tool.name.as_str()) {
                tools.push(McpToolDefinition {
                    name: tool.name,
                    description: tool.description.clone(),
                    input_schema: tool.input_schema,
                });
            }
        }

        // Add compact meta-tools for everything else
        tools.extend(self.get_compact_tools().await?);

        Ok(tools)
    }

    /// Handle compact mode tool execution (called from MCP tools/call)
    pub async fn handle_compact_tool_call(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value> {
        match tool_name {
            "list_tools" => self.compact_list_tools(arguments).await,
            "search_tools" => self.compact_search_tools(arguments).await,
            "get_tool_schema" => self.compact_get_schema(arguments).await,
            "execute_tool" => self.compact_execute_tool(arguments).await,
            _ => {
                // Not a meta-tool, try direct execution
                let result = self.call_tool(tool_name, arguments).await?;
                Ok(result.result)
            }
        }
    }

    async fn compact_list_tools(&self, args: Value) -> Result<Value> {
        let category = args
            .as_object()
            .and_then(|obj| obj.get("category"))
            .and_then(|v| v.as_str());
        let namespace = args
            .as_object()
            .and_then(|obj| obj.get("namespace"))
            .and_then(|v| v.as_str());
        let limit = args
            .as_object()
            .and_then(|obj| obj.get("limit"))
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;

        let all_tools = self.list_default_tools().await?;

        let filtered: Vec<Value> = all_tools
            .iter()
            .filter(|t| {
                if let Some(cat) = category {
                    let tool_cat = t
                        .annotations
                        .as_ref()
                        .and_then(|a| a.as_object())
                        .and_then(|obj| obj.get("category"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("general");
                    if tool_cat != cat {
                        return false;
                    }
                }
                if let Some(ns) = namespace {
                    let tool_ns = t
                        .annotations
                        .as_ref()
                        .and_then(|a| a.as_object())
                        .and_then(|obj| obj.get("namespace"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("system");
                    if tool_ns != ns {
                        return false;
                    }
                }
                true
            })
            .take(limit)
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description.as_str(),
                    "category": t.annotations.as_ref()
                        .and_then(|a| a.get("category"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("general")
                })
            })
            .collect();

        Ok(json!({
            "tools": filtered,
            "count": filtered.len(),
            "total_available": all_tools.len(),
            "hint": "Use get_tool_schema to see arguments, then execute_tool to run"
        }))
    }

    async fn compact_search_tools(&self, args: Value) -> Result<Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("query is required"))?
            .to_lowercase();
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let all_tools = self.list_default_tools().await?;

        let mut scored: Vec<(i32, &ToolDefinition)> = all_tools
            .iter()
            .filter_map(|t| {
                let name_lower = t.name.to_lowercase();
                let desc_lower = t.description.as_str().to_lowercase();

                let mut score = 0;
                if name_lower == query {
                    score += 100;
                } else if name_lower.contains(&query) {
                    score += 50;
                }
                if desc_lower.contains(&query) {
                    score += 20;
                }

                if score > 0 {
                    Some((score, t))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));

        let results: Vec<Value> = scored
            .iter()
            .take(limit)
            .map(|(score, t)| {
                json!({
                    "name": t.name,
                    "description": t.description.as_str(),
                    "relevance": score
                })
            })
            .collect();

        Ok(json!({
            "query": query,
            "results": results,
            "count": results.len()
        }))
    }

    async fn compact_get_schema(&self, args: Value) -> Result<Value> {
        let tool_name = args
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("tool_name is required"))?;

        let (tool_def, server_id) = self
            .cache
            .get(tool_name)
            .await
            .ok_or_else(|| anyhow!("Tool '{}' not found", tool_name))?;

        Ok(json!({
            "tool": tool_name,
            "description": tool_def.description,
            "input_schema": tool_def.input_schema,
            "server": server_id
        }))
    }

    async fn compact_execute_tool(&self, args: Value) -> Result<Value> {
        let tool_name = args
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("tool_name is required"))?;

        let arguments = args.get("arguments").cloned().unwrap_or(json!({}));

        let result = self.call_tool(tool_name, arguments).await?;

        Ok(json!({
            "tool": tool_name,
            "result": result.result,
            "success": !result.is_error
        }))
    }

    /// Remove a server
    pub async fn remove_server(&self, server_id: &str) -> Result<()> {
        self.cache.remove_server(server_id).await;
        info!("Removed server: {}", server_id);
        Ok(())
    }

    async fn ensure_initialized(&self) -> Result<()> {
        if !*self.initialized.read().await {
            return Err(anyhow!(
                "Aggregator not initialized. Call initialize() first."
            ));
        }
        Ok(())
    }

    /// Get the profile manager
    pub fn profiles(&self) -> &Arc<ProfileManager> {
        &self.profiles
    }

    /// Get the tool cache
    pub fn cache(&self) -> &Arc<ToolCache> {
        &self.cache
    }
}

/// Result of a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub tool_name: String,
    pub server_id: String,
    pub result: Value,
    pub is_error: bool,
}

/// Aggregator statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatorStats {
    pub connected_servers: usize,
    pub total_tools: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub profiles: Vec<String>,
}

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub servers: Vec<ServerHealth>,
}

/// Individual server health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHealth {
    pub id: String,
    pub name: String,
    pub healthy: bool,
}

/// MCP tool definition for tools/list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Integration with op-tools ToolRegistry
impl Aggregator {
    /// Register aggregated tools with an op-tools ToolRegistry
    pub async fn register_with_tool_registry(
        &self,
        registry: &op_tools::ToolRegistry,
        profile_name: &str,
    ) -> Result<()> {
        let tools = self.list_tools(profile_name).await?;

        for tool_def in tools {
            let aggregator = self.clone_arc();
            let tool_name = tool_def.name.clone();

            // Create a tool that proxies to the aggregator
            let proxy_tool = AggregatorProxyTool {
                name: tool_def.name.clone(),
                description: tool_def.description.clone(),
                input_schema: tool_def.input_schema.clone(),
                aggregator,
            };

            registry.register_tool(Arc::new(proxy_tool)).await?;
            debug!("Registered proxy tool: {}", tool_name);
        }

        Ok(())
    }

    fn clone_arc(&self) -> Arc<Aggregator> {
        // This is a bit awkward - in practice you'd store Arc<Self>
        // For now, return a placeholder
        unimplemented!("Use Arc<Aggregator> directly")
    }
}

/// Proxy tool that delegates to the aggregator
struct AggregatorProxyTool {
    name: String,
    description: String,
    input_schema: Value,
    aggregator: Arc<Aggregator>,
}

#[async_trait::async_trait]
impl op_tools::tool::Tool for AggregatorProxyTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let result = self.aggregator.call_tool(&self.name, input).await?;
        Ok(result.result)
    }

    fn category(&self) -> &str {
        "aggregated"
    }

    fn namespace(&self) -> &str {
        "external"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_aggregator_creation() {
        let config = AggregatorConfig::default();
        let aggregator = Aggregator::new(config).await.unwrap();

        // Should not be initialized yet
        assert!(aggregator.ensure_initialized().await.is_err());
    }

    #[tokio::test]
    async fn test_aggregator_empty_init() {
        let config = AggregatorConfig::default();
        let aggregator = Aggregator::new(config).await.unwrap();

        // Initialize with no servers should work
        aggregator.initialize().await.unwrap();

        let stats = aggregator.stats().await;
        assert_eq!(stats.connected_servers, 0);
        assert_eq!(stats.total_tools, 0);
    }
}
</file>

<file path="src/cache.rs">
//! Tool Schema Cache with TTL and LRU eviction
//!
//! Caches tool definitions from upstream servers to reduce latency.

use crate::client::ToolDefinition;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Cached tool entry with TTL
#[derive(Debug, Clone)]
struct CachedTool {
    definition: ToolDefinition,
    /// Which server this tool came from
    server_id: String,
    /// When this entry was cached
    cached_at: Instant,
    /// How many times this tool was accessed
    access_count: u64,
}

impl CachedTool {
    fn new(definition: ToolDefinition, server_id: String) -> Self {
        Self {
            definition,
            server_id,
            cached_at: Instant::now(),
            access_count: 0,
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.cached_at.elapsed() > ttl
    }

    fn touch(&mut self) {
        self.access_count += 1;
    }
}

/// Tool cache with TTL and LRU eviction
pub struct ToolCache {
    /// Cached tools by name
    cache: RwLock<LruCache<String, CachedTool>>,
    /// Time-to-live for cached entries
    ttl: Duration,
    /// Statistics
    stats: RwLock<CacheStats>,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub refreshes: u64,
}

impl ToolCache {
    /// Create a new tool cache
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        let capacity = NonZeroUsize::new(max_entries).unwrap_or(NonZeroUsize::new(1000).unwrap());
        Self {
            cache: RwLock::new(LruCache::new(capacity)),
            ttl,
            stats: RwLock::new(CacheStats::default()),
        }
    }

    /// Get a tool definition from cache
    pub async fn get(&self, name: &str) -> Option<(ToolDefinition, String)> {
        let mut cache = self.cache.write().await;

        if let Some(entry) = cache.get_mut(name) {
            if entry.is_expired(self.ttl) {
                // Entry expired, remove it
                cache.pop(name);
                let mut stats = self.stats.write().await;
                stats.misses += 1;
                return None;
            }

            entry.touch();
            let mut stats = self.stats.write().await;
            stats.hits += 1;

            return Some((entry.definition.clone(), entry.server_id.clone()));
        }

        let mut stats = self.stats.write().await;
        stats.misses += 1;
        None
    }

    /// Insert or update a tool in the cache
    pub async fn insert(&self, tool: ToolDefinition, server_id: &str) {
        let name = tool.name.clone();
        let entry = CachedTool::new(tool, server_id.to_string());

        let mut cache = self.cache.write().await;
        cache.put(name, entry);
    }

    /// Insert multiple tools from a server
    pub async fn insert_batch(&self, tools: Vec<ToolDefinition>, server_id: &str) {
        let mut cache = self.cache.write().await;

        for tool in tools {
            let name = tool.name.clone();
            let entry = CachedTool::new(tool, server_id.to_string());
            cache.put(name, entry);
        }

        debug!("Cached {} tools from server {}", cache.len(), server_id);
    }

    /// Remove a tool from cache
    pub async fn remove(&self, name: &str) -> bool {
        let mut cache = self.cache.write().await;
        cache.pop(name).is_some()
    }

    /// Remove all tools from a specific server
    pub async fn remove_server(&self, server_id: &str) {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        // Collect keys to remove (can't remove while iterating)
        let to_remove: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.server_id == server_id)
            .map(|(name, _)| name.clone())
            .collect();

        for name in to_remove {
            cache.pop(&name);
            stats.evictions += 1;
        }
    }

    /// Get all cached tool definitions
    pub async fn list_all(&self) -> Vec<(ToolDefinition, String)> {
        let cache = self.cache.read().await;
        cache
            .iter()
            .filter(|(_, entry)| !entry.is_expired(self.ttl))
            .map(|(_, entry)| (entry.definition.clone(), entry.server_id.clone()))
            .collect()
    }

    /// Get all tool names
    pub async fn tool_names(&self) -> Vec<String> {
        let cache = self.cache.read().await;
        cache
            .iter()
            .filter(|(_, entry)| !entry.is_expired(self.ttl))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Check which server owns a tool
    pub async fn get_server_id(&self, tool_name: &str) -> Option<String> {
        let cache = self.cache.read().await;
        cache.peek(tool_name).map(|entry| entry.server_id.clone())
    }

    /// Clear all cached entries
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        info!("Tool cache cleared");
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Get cache size
    pub async fn len(&self) -> usize {
        self.cache.read().await.len()
    }

    /// Check if cache is empty
    pub async fn is_empty(&self) -> bool {
        self.cache.read().await.is_empty()
    }

    /// Evict expired entries
    pub async fn evict_expired(&self) -> usize {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        let to_remove: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.is_expired(self.ttl))
            .map(|(name, _)| name.clone())
            .collect();

        let count = to_remove.len();
        for name in to_remove {
            cache.pop(&name);
            stats.evictions += 1;
        }

        if count > 0 {
            debug!("Evicted {} expired cache entries", count);
        }

        count
    }
}

/// Background cache maintenance task
pub async fn cache_maintenance_loop(cache: Arc<ToolCache>, interval: Duration) {
    loop {
        tokio::time::sleep(interval).await;
        let evicted = cache.evict_expired().await;
        if evicted > 0 {
            debug!("Cache maintenance: evicted {} entries", evicted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::json;

    fn make_tool(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: "Test tool".to_string(),
            input_schema: json!({}),
            schema_version: String::new(),
            category: "test".to_string(),
            tags: vec![],
            namespace: "test".to_string(),
            annotations: None,
        }
    }

    #[tokio::test]
    async fn test_cache_insert_and_get() {
        let cache = ToolCache::new(100, Duration::from_secs(300));
        let tool = make_tool("test_tool");

        cache.insert(tool.clone(), "server1").await;

        let result = cache.get("test_tool").await;
        assert!(result.is_some());
        let (def, server) = result.unwrap();
        assert_eq!(def.name, "test_tool");
        assert_eq!(server, "server1");
    }

    #[tokio::test]
    async fn test_cache_expiry() {
        let cache = ToolCache::new(100, Duration::from_millis(10));
        let tool = make_tool("test_tool");

        cache.insert(tool, "server1").await;

        // Should be found immediately
        assert!(cache.get("test_tool").await.is_some());

        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Should be expired now
        assert!(cache.get("test_tool").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = ToolCache::new(100, Duration::from_secs(300));
        let tool = make_tool("test_tool");

        cache.insert(tool, "server1").await;

        // Hit
        cache.get("test_tool").await;
        // Miss
        cache.get("nonexistent").await;

        let stats = cache.stats().await;
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[tokio::test]
    async fn test_remove_server() {
        let cache = ToolCache::new(100, Duration::from_secs(300));

        cache.insert(make_tool("tool1"), "server1").await;
        cache.insert(make_tool("tool2"), "server1").await;
        cache.insert(make_tool("tool3"), "server2").await;

        assert_eq!(cache.len().await, 3);

        cache.remove_server("server1").await;

        assert_eq!(cache.len().await, 1);
        assert!(cache.get("tool3").await.is_some());
    }
}
</file>

<file path="src/client.rs">
//! MCP Client for connecting to upstream servers
//!
//! Supports SSE and stdio transports for communicating with MCP servers.

use crate::config::{ServerAuth, TransportType, UpstreamServer};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

fn transport_root(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    for suffix in ["/mcp", "/message", "/sse"] {
        if let Some(root) = trimmed.strip_suffix(suffix) {
            return root.trim_end_matches('/').to_string();
        }
    }
    trimmed.to_string()
}

fn canonical_mcp_endpoint(url: &str) -> String {
    format!("{}/mcp", transport_root(url))
}

fn legacy_message_endpoint(url: &str) -> String {
    format!("{}/message", transport_root(url))
}

/// MCP JSON-RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl McpRequest {
    pub fn new(method: &str, params: Option<Value>) -> Self {
        static REQUEST_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            jsonrpc: "2.0".to_string(),
            id: json!(REQUEST_ID.fetch_add(1, Ordering::SeqCst)),
            method: method.to_string(),
            params,
        }
    }
}

/// MCP JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpRpcError>,
}

/// MCP RPC Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Tool definition (local to avoid cycle)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub annotations: Option<Value>,
}

/// Client for communicating with an upstream MCP server
pub struct McpClient {
    /// Server configuration
    config: UpstreamServer,
    /// HTTP client (for SSE transport)
    http_client: reqwest::Client,
    /// Cached tools from this server
    cached_tools: RwLock<Vec<ToolDefinition>>,
    /// Whether the client is initialized
    initialized: RwLock<bool>,
}

impl McpClient {
    /// Create a new MCP client for the given server
    pub fn new(config: UpstreamServer) -> Result<Self> {
        let mut client_builder = reqwest::Client::builder().timeout(config.timeout());

        // Add auth if configured
        if let Some(auth) = &config.auth {
            let resolved = auth.resolve();
            match resolved {
                ServerAuth::Bearer { token } => {
                    let mut headers = reqwest::header::HeaderMap::new();
                    headers.insert(
                        reqwest::header::AUTHORIZATION,
                        format!("Bearer {}", token)
                            .parse()
                            .map_err(|_| anyhow!("Invalid bearer token"))?,
                    );
                    client_builder = client_builder.default_headers(headers);
                }
                ServerAuth::Header { name, value } => {
                    let mut headers = reqwest::header::HeaderMap::new();
                    headers.insert(
                        reqwest::header::HeaderName::from_bytes(name.as_bytes())
                            .map_err(|_| anyhow!("Invalid header name"))?,
                        value.parse().map_err(|_| anyhow!("Invalid header value"))?,
                    );
                    client_builder = client_builder.default_headers(headers);
                }
                ServerAuth::Basic { username, password } => {
                    let mut headers = reqwest::header::HeaderMap::new();
                    use base64::Engine;
                    let credentials = base64::engine::general_purpose::STANDARD
                        .encode(format!("{}:{}", username, password));
                    headers.insert(
                        reqwest::header::AUTHORIZATION,
                        format!("Basic {}", credentials)
                            .parse()
                            .map_err(|_| anyhow!("Invalid basic auth"))?,
                    );
                    client_builder = client_builder.default_headers(headers);
                }
            }
        }

        let http_client = client_builder
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            config,
            http_client,
            cached_tools: RwLock::new(vec![]),
            initialized: RwLock::new(false),
        })
    }

    /// Get the server ID
    pub fn server_id(&self) -> &str {
        &self.config.id
    }

    /// Get the server config
    pub fn config(&self) -> &UpstreamServer {
        &self.config
    }

    /// Initialize the connection to the upstream server
    pub async fn initialize(&self) -> Result<()> {
        if *self.initialized.read().await {
            return Ok(());
        }

        info!("Initializing MCP client for server: {}", self.config.name);

        match self.config.transport {
            TransportType::Sse => self.initialize_sse().await?,
            TransportType::Stdio => self.initialize_stdio().await?,
            TransportType::Websocket => {
                return Err(anyhow!("WebSocket transport not yet implemented"));
            }
        }

        *self.initialized.write().await = true;
        Ok(())
    }

    async fn initialize_sse(&self) -> Result<()> {
        let request = McpRequest::new(
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "op-mcp-aggregator",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
        );

        let response = self.send_request(&request).await?;

        if let Some(error) = response.error {
            return Err(anyhow!("Initialize failed: {}", error.message));
        }

        debug!(
            "Initialized connection to {}: {:?}",
            self.config.name, response.result
        );
        Ok(())
    }

    async fn initialize_stdio(&self) -> Result<()> {
        // For stdio, we'd spawn a child process
        // This is a simplified implementation
        warn!("Stdio transport initialization not fully implemented");
        Ok(())
    }

    /// Send a request to the upstream server
    async fn send_request(&self, request: &McpRequest) -> Result<McpResponse> {
        match self.config.transport {
            TransportType::Sse => self.send_sse_request(request).await,
            TransportType::Stdio => self.send_stdio_request(request).await,
            TransportType::Websocket => Err(anyhow!("WebSocket not implemented")),
        }
    }

    async fn send_sse_request(&self, request: &McpRequest) -> Result<McpResponse> {
        let url = canonical_mcp_endpoint(&self.config.url);
        let legacy_url = legacy_message_endpoint(&self.config.url);

        debug!("Sending MCP request to {}: {}", url, request.method);

        let mut response = self
            .http_client
            .post(&url)
            .json(request)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", self.config.name))?;

        if matches!(
            response.status(),
            reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::METHOD_NOT_ALLOWED
        ) && url != legacy_url
        {
            warn!(
                "Upstream {} does not expose /mcp, retrying legacy /message endpoint",
                self.config.name
            );
            response = self
                .http_client
                .post(&legacy_url)
                .json(request)
                .send()
                .await
                .with_context(|| format!("Failed legacy request to {}", self.config.name))?;
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("HTTP error {}: {}", status, body));
        }

        let mcp_response: McpResponse = response
            .json()
            .await
            .with_context(|| "Failed to parse MCP response")?;

        Ok(mcp_response)
    }

    async fn send_stdio_request(&self, _request: &McpRequest) -> Result<McpResponse> {
        // Stdio implementation would write to child process stdin
        // and read from stdout
        Err(anyhow!("Stdio transport not fully implemented"))
    }

    /// List tools from this server
    pub async fn list_tools(&self) -> Result<Vec<ToolDefinition>> {
        self.initialize().await?;

        let request = McpRequest::new("tools/list", None);
        let response = self.send_request(&request).await?;

        if let Some(error) = response.error {
            return Err(anyhow!("tools/list failed: {}", error.message));
        }

        let result = response.result.unwrap_or(json!({}));
        let tools: Vec<ToolDefinition> = result
            .as_object()
            .and_then(|obj| obj.get("tools"))
            .and_then(|t| simd_json::serde::from_owned_value(t.clone()).ok())
            .unwrap_or_default();

        // Filter tools based on server config
        let filtered: Vec<ToolDefinition> = tools
            .into_iter()
            .filter(|t| self.config.should_include_tool(&t.name))
            .map(|mut t| {
                // Apply prefix if configured
                t.name = self.config.prefixed_name(&t.name);
                t
            })
            .collect();

        // Cache the tools
        *self.cached_tools.write().await = filtered.clone();

        info!("Loaded {} tools from {}", filtered.len(), self.config.name);
        Ok(filtered)
    }

    /// Get cached tools (without refreshing)
    pub async fn get_cached_tools(&self) -> Vec<ToolDefinition> {
        self.cached_tools.read().await.clone()
    }

    /// Call a tool on this server
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value> {
        self.initialize().await?;

        // Remove prefix if we added one
        let actual_name = if let Some(prefix) = &self.config.tool_prefix {
            let prefix_with_underscore = format!("{}_", prefix);
            name.strip_prefix(&prefix_with_underscore)
                .unwrap_or(name)
                .to_string()
        } else {
            name.to_string()
        };

        debug!(
            "Calling tool {} (actual: {}) on {}",
            name, actual_name, self.config.name
        );

        let request = McpRequest::new(
            "tools/call",
            Some(json!({
                "name": actual_name,
                "arguments": arguments
            })),
        );

        let response = self.send_request(&request).await?;

        if let Some(error) = response.error {
            return Err(anyhow!("Tool call failed: {}", error.message));
        }

        Ok(response.result.unwrap_or(json!(null)))
    }

    /// Check if this server has a tool (by prefixed name)
    pub async fn has_tool(&self, name: &str) -> bool {
        let tools = self.cached_tools.read().await;
        let result = tools.iter().any(|t| t.name == name);
        result
    }

    /// Health check
    pub async fn health_check(&self) -> bool {
        match self.config.transport {
            TransportType::Sse => {
                let url = format!("{}/health", transport_root(&self.config.url));
                self.http_client
                    .get(&url)
                    .send()
                    .await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false)
            }
            _ => true, // Assume healthy for other transports
        }
    }
}

/// Manager for multiple MCP clients
pub struct ClientManager {
    clients: RwLock<Vec<Arc<McpClient>>>,
}

impl ClientManager {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(vec![]),
        }
    }

    /// Add a client
    pub async fn add_client(&self, client: Arc<McpClient>) {
        self.clients.write().await.push(client);
    }

    /// Get all clients
    pub async fn clients(&self) -> Vec<Arc<McpClient>> {
        self.clients.read().await.clone()
    }

    /// Get client by server ID
    pub async fn get_client(&self, server_id: &str) -> Option<Arc<McpClient>> {
        self.clients
            .read()
            .await
            .iter()
            .find(|c| c.server_id() == server_id)
            .cloned()
    }

    /// Find which client owns a tool
    pub async fn find_tool_owner(&self, tool_name: &str) -> Option<Arc<McpClient>> {
        for client in self.clients.read().await.iter() {
            if client.has_tool(tool_name).await {
                return Some(client.clone());
            }
        }
        None
    }

    /// Refresh all clients
    pub async fn refresh_all(&self) -> Result<()> {
        let clients = self.clients.read().await.clone();
        for client in clients {
            if let Err(e) = client.list_tools().await {
                error!("Failed to refresh tools from {}: {}", client.server_id(), e);
            }
        }
        Ok(())
    }
}

impl Default for ClientManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_request_creation() {
        let req = McpRequest::new("tools/list", None);
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "tools/list");
        assert!(req.params.is_none());
    }

    #[test]
    fn test_tool_prefix_stripping() {
        // Test that tool names are properly prefixed/unprefixed
        let config =
            UpstreamServer::sse("gh", "GitHub", "http://localhost:3000").with_prefix("github");

        assert_eq!(config.prefixed_name("search"), "github_search");
    }

    #[test]
    fn should_normalize_transport_urls() {
        assert_eq!(
            canonical_mcp_endpoint("http://localhost:3000"),
            "http://localhost:3000/mcp"
        );
        assert_eq!(
            canonical_mcp_endpoint("http://localhost:3000/sse"),
            "http://localhost:3000/mcp"
        );
        assert_eq!(
            legacy_message_endpoint("http://localhost:3000/mcp"),
            "http://localhost:3000/message"
        );
    }
}
</file>

<file path="src/compact.rs">
//! Compact Mode - Reduces 750+ tools to 4-5 meta-tools
//!
//! Instead of exposing every tool directly (which consumes massive context window),
//! compact mode exposes just a few meta-tools:
//!
//! 1. `list_tools` - List available tools with filtering
//! 2. `execute_tool` - Execute any tool by name
//! 3. `get_tool_schema` - Get schema for a specific tool
//! 4. `search_tools` - Search tools by keyword
//!
//! This design:
//! - Saves ~95% of context tokens
//! - Bypasses Cursor's 40-tool limit entirely
//! - Keeps all tools accessible via execute_tool
//! - Improves LLM reasoning (fewer choices = better decisions)

use crate::aggregator::Aggregator;
use crate::client::ToolDefinition;
use anyhow::Result;
use async_trait::async_trait;
use op_tools::tool::{SecurityLevel, Tool};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tracing::{debug, info};

/// Compact mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactModeConfig {
    /// Whether compact mode is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Include list_tools meta-tool
    #[serde(default = "default_true")]
    pub include_list: bool,

    /// Include execute_tool meta-tool
    #[serde(default = "default_true")]
    pub include_execute: bool,

    /// Include get_tool_schema meta-tool
    #[serde(default = "default_true")]
    pub include_schema: bool,

    /// Include search_tools meta-tool
    #[serde(default = "default_true")]
    pub include_search: bool,

    /// Include batch_execute meta-tool
    #[serde(default)]
    pub include_batch: bool,

    /// Maximum tools to return in list_tools (for context savings)
    #[serde(default = "default_max_list")]
    pub max_list_results: usize,

    /// Default profile for tool execution
    #[serde(default)]
    pub default_profile: Option<String>,
}

fn default_true() -> bool {
    true
}
fn default_max_list() -> usize {
    50
}

impl Default for CompactModeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            include_list: true,
            include_execute: true,
            include_schema: true,
            include_search: true,
            include_batch: false,
            max_list_results: 50,
            default_profile: None,
        }
    }
}

/// Create compact mode tools
pub fn create_compact_tools(
    aggregator: Arc<Aggregator>,
    config: &CompactModeConfig,
) -> Vec<Arc<dyn Tool>> {
    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();

    if config.include_list {
        tools.push(Arc::new(ListToolsTool::new(
            aggregator.clone(),
            config.max_list_results,
        )));
    }

    if config.include_execute {
        tools.push(Arc::new(ExecuteToolTool::new(aggregator.clone())));
    }

    if config.include_schema {
        tools.push(Arc::new(GetToolSchemaTool::new(aggregator.clone())));
    }

    if config.include_search {
        tools.push(Arc::new(SearchToolsTool::new(
            aggregator.clone(),
            config.max_list_results,
        )));
    }

    if config.include_batch {
        tools.push(Arc::new(BatchExecuteTool::new(aggregator.clone())));
    }

    info!("Created {} compact mode meta-tools", tools.len());
    tools
}

// ============================================================================
// META-TOOL 1: list_tools
// ============================================================================

/// Lists available tools with optional filtering
pub struct ListToolsTool {
    aggregator: Arc<Aggregator>,
    max_results: usize,
}

impl ListToolsTool {
    pub fn new(aggregator: Arc<Aggregator>, max_results: usize) -> Self {
        Self {
            aggregator,
            max_results,
        }
    }
}

#[async_trait]
impl Tool for ListToolsTool {
    fn name(&self) -> &str {
        "list_tools"
    }

    fn description(&self) -> &str {
        "List available tools. Use 'category' or 'namespace' to filter. Returns tool names and descriptions. Call 'get_tool_schema' to get full input schema before executing a tool."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "description": "Filter by category (e.g., 'systemd', 'network', 'filesystem')"
                },
                "namespace": {
                    "type": "string",
                    "description": "Filter by namespace (e.g., 'system', 'dbus', 'external')"
                },
                "profile": {
                    "type": "string",
                    "description": "Profile to list tools from (default: current profile)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of tools to return",
                    "default": 20
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let category = input
            .as_object()
            .and_then(|obj| obj.get("category"))
            .and_then(|v| v.as_str());
        let namespace = input
            .as_object()
            .and_then(|obj| obj.get("namespace"))
            .and_then(|v| v.as_str());
        let profile = input
            .as_object()
            .and_then(|obj| obj.get("profile"))
            .and_then(|v| v.as_str())
            .unwrap_or(self.aggregator.default_profile());
        let limit = input
            .as_object()
            .and_then(|obj| obj.get("limit"))
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;

        let limit = limit.min(self.max_results);

        debug!(
            "list_tools: profile={}, category={:?}, namespace={:?}, limit={}",
            profile, category, namespace, limit
        );

        let all_tools = self.aggregator.list_tools(profile).await?;

        // Filter
        let filtered: Vec<&ToolDefinition> = all_tools
            .iter()
            .filter(|t| {
                if let Some(cat) = category {
                    let tool_cat = t
                        .annotations
                        .as_ref()
                        .and_then(|a| a.as_object())
                        .and_then(|obj| obj.get("category"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("general");
                    if tool_cat != cat {
                        return false;
                    }
                }
                if let Some(ns) = namespace {
                    let tool_ns = t
                        .annotations
                        .as_ref()
                        .and_then(|a| a.as_object())
                        .and_then(|obj| obj.get("namespace"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("system");
                    if tool_ns != ns {
                        return false;
                    }
                }
                true
            })
            .take(limit)
            .collect();

        // Return compact format (name + description only, no schemas)
        let tools_list: Vec<Value> = filtered
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description.as_str(),
                    "category": t.annotations.as_ref()
                        .and_then(|a| a.as_object())
                        .and_then(|obj| obj.get("category"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("general")
                })
            })
            .collect();

        Ok(json!({
            "tools": tools_list,
            "count": filtered.len(),
            "total_available": all_tools.len(),
            "profile": profile,
            "hint": "Use 'get_tool_schema' to get the input schema before calling 'execute_tool'"
        }))
    }

    fn category(&self) -> &str {
        "meta"
    }
    fn namespace(&self) -> &str {
        "compact"
    }
    fn security_level(&self) -> SecurityLevel {
        SecurityLevel::ReadOnly
    }
}

// ============================================================================
// META-TOOL 2: execute_tool
// ============================================================================

/// Executes any tool by name
pub struct ExecuteToolTool {
    aggregator: Arc<Aggregator>,
}

impl ExecuteToolTool {
    pub fn new(aggregator: Arc<Aggregator>) -> Self {
        Self { aggregator }
    }
}

#[async_trait]
impl Tool for ExecuteToolTool {
    fn name(&self) -> &str {
        "execute_tool"
    }

    fn description(&self) -> &str {
        "Execute any available tool by name. First use 'list_tools' to see available tools, then 'get_tool_schema' to see required arguments."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": "Name of the tool to execute"
                },
                "arguments": {
                    "type": "object",
                    "description": "Arguments to pass to the tool (use get_tool_schema to see required args)"
                }
            },
            "required": ["tool_name"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let tool_name = input
            .as_object()
            .and_then(|obj| obj.get("tool_name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("tool_name is required"))?;

        let arguments = input
            .as_object()
            .and_then(|obj| obj.get("arguments"))
            .cloned()
            .unwrap_or(json!({}));

        debug!("execute_tool: {} with args {:?}", tool_name, arguments);

        let result = self.aggregator.call_tool(tool_name, arguments).await?;

        Ok(json!({
            "tool": tool_name,
            "result": result.result,
            "server": result.server_id,
            "success": !result.is_error
        }))
    }

    fn category(&self) -> &str {
        "meta"
    }
    fn namespace(&self) -> &str {
        "compact"
    }
    fn security_level(&self) -> SecurityLevel {
        SecurityLevel::Elevated
    }
}

// ============================================================================
// META-TOOL 3: get_tool_schema
// ============================================================================

/// Gets the full schema for a specific tool
pub struct GetToolSchemaTool {
    aggregator: Arc<Aggregator>,
}

impl GetToolSchemaTool {
    pub fn new(aggregator: Arc<Aggregator>) -> Self {
        Self { aggregator }
    }
}

#[async_trait]
impl Tool for GetToolSchemaTool {
    fn name(&self) -> &str {
        "get_tool_schema"
    }

    fn description(&self) -> &str {
        "Get the full input schema for a tool. Use this before calling execute_tool to understand required and optional arguments."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": "Name of the tool to get schema for"
                }
            },
            "required": ["tool_name"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let tool_name = input
            .as_object()
            .and_then(|obj| obj.get("tool_name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("tool_name is required"))?;

        debug!("get_tool_schema: {}", tool_name);

        // Search for the tool in cache
        let (tool_def, server_id) = self
            .aggregator
            .cache()
            .get(tool_name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found", tool_name))?;

        Ok(json!({
            "tool": tool_name,
            "description": tool_def.description,
            "input_schema": tool_def.input_schema,
            "server": server_id,
            "annotations": tool_def.annotations
        }))
    }

    fn category(&self) -> &str {
        "meta"
    }
    fn namespace(&self) -> &str {
        "compact"
    }
    fn security_level(&self) -> SecurityLevel {
        SecurityLevel::ReadOnly
    }
}

// ============================================================================
// META-TOOL 4: search_tools
// ============================================================================

/// Searches tools by keyword in name or description
pub struct SearchToolsTool {
    aggregator: Arc<Aggregator>,
    max_results: usize,
}

impl SearchToolsTool {
    pub fn new(aggregator: Arc<Aggregator>, max_results: usize) -> Self {
        Self {
            aggregator,
            max_results,
        }
    }
}

#[async_trait]
impl Tool for SearchToolsTool {
    fn name(&self) -> &str {
        "search_tools"
    }

    fn description(&self) -> &str {
        "Search for tools by keyword. Searches tool names and descriptions. Use this to find relevant tools for a task."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (searches in tool names and descriptions)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results to return",
                    "default": 10
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let query = input
            .as_object()
            .and_then(|obj| obj.get("query"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("query is required"))?
            .to_lowercase();

        let limit = input
            .as_object()
            .and_then(|obj| obj.get("limit"))
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;
        let limit = limit.min(self.max_results);

        debug!("search_tools: query='{}', limit={}", query, limit);

        let all_tools = self.aggregator.list_default_tools().await?;

        // Score and filter tools
        let mut scored: Vec<(i32, &ToolDefinition)> = all_tools
            .iter()
            .filter_map(|t| {
                let name_lower = t.name.to_lowercase();
                let desc_lower = Some(t.description.as_str()).unwrap_or("").to_lowercase();

                let mut score = 0;

                // Exact name match
                if name_lower == query {
                    score += 100;
                }
                // Name contains query
                else if name_lower.contains(&query) {
                    score += 50;
                }
                // Description contains query
                if desc_lower.contains(&query) {
                    score += 20;
                }
                // Word boundary match in name
                if name_lower.split('_').any(|w| w == query) {
                    score += 30;
                }

                if score > 0 {
                    Some((score, t))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.0.cmp(&a.0));

        let results: Vec<Value> = scored
            .iter()
            .take(limit)
            .map(|(score, t)| {
                json!({
                    "name": t.name,
                    "description": t.description.as_str(),
                    "relevance": score
                })
            })
            .collect();

        Ok(json!({
            "query": query,
            "results": results,
            "count": results.len(),
            "hint": "Use 'get_tool_schema' to see arguments, then 'execute_tool' to run"
        }))
    }

    fn category(&self) -> &str {
        "meta"
    }
    fn namespace(&self) -> &str {
        "compact"
    }
    fn security_level(&self) -> SecurityLevel {
        SecurityLevel::ReadOnly
    }
}

// ============================================================================
// META-TOOL 5: batch_execute (optional)
// ============================================================================

/// Executes multiple tools in sequence
pub struct BatchExecuteTool {
    aggregator: Arc<Aggregator>,
}

impl BatchExecuteTool {
    pub fn new(aggregator: Arc<Aggregator>) -> Self {
        Self { aggregator }
    }
}

#[async_trait]
impl Tool for BatchExecuteTool {
    fn name(&self) -> &str {
        "batch_execute"
    }

    fn description(&self) -> &str {
        "Execute multiple tools in sequence. Useful for multi-step operations. Each tool runs with its own arguments. If any tool fails, subsequent tools still run."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operations": {
                    "type": "array",
                    "description": "List of tool operations to execute",
                    "items": {
                        "type": "object",
                        "properties": {
                            "tool_name": {
                                "type": "string",
                                "description": "Name of tool to execute"
                            },
                            "arguments": {
                                "type": "object",
                                "description": "Arguments for this tool"
                            }
                        },
                        "required": ["tool_name"]
                    }
                },
                "stop_on_error": {
                    "type": "boolean",
                    "description": "Stop execution if a tool fails",
                    "default": false
                }
            },
            "required": ["operations"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let operations = input
            .as_object()
            .and_then(|obj| obj.get("operations"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("operations array is required"))?;

        let stop_on_error = input
            .as_object()
            .and_then(|obj| obj.get("stop_on_error"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        debug!(
            "batch_execute: {} operations, stop_on_error={}",
            operations.len(),
            stop_on_error
        );

        let mut results = Vec::new();
        let mut all_succeeded = true;

        for (i, op) in operations.as_slice().iter().enumerate() {
            let tool_name = op
                .as_object()
                .and_then(|obj| obj.get("tool_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            let arguments = op
                .as_object()
                .and_then(|obj| obj.get("arguments"))
                .cloned()
                .unwrap_or(json!({}));

            match self.aggregator.call_tool(tool_name, arguments).await {
                Ok(result) => {
                    results.push(json!({
                        "index": i,
                        "tool": tool_name,
                        "success": true,
                        "result": result.result
                    }));
                }
                Err(e) => {
                    all_succeeded = false;
                    results.push(json!({
                        "index": i,
                        "tool": tool_name,
                        "success": false,
                        "error": e.to_string()
                    }));

                    if stop_on_error {
                        break;
                    }
                }
            }
        }

        Ok(json!({
            "results": results,
            "total": operations.len(),
            "succeeded": results.iter().filter(|r| r.as_object().and_then(|obj| obj.get("success")) == Some(&json!(true))).count(),
            "all_succeeded": all_succeeded
        }))
    }

    fn category(&self) -> &str {
        "meta"
    }
    fn namespace(&self) -> &str {
        "compact"
    }
    fn security_level(&self) -> SecurityLevel {
        SecurityLevel::Elevated
    }
}

/// Summary of compact mode tools for documentation
pub fn compact_mode_summary() -> Value {
    json!({
        "mode": "compact",
        "description": "Reduces 750+ tools to 4-5 meta-tools for context efficiency",
        "tools": [
            {
                "name": "list_tools",
                "purpose": "Browse available tools by category/namespace"
            },
            {
                "name": "search_tools",
                "purpose": "Find tools by keyword search"
            },
            {
                "name": "get_tool_schema",
                "purpose": "Get input schema before executing a tool"
            },
            {
                "name": "execute_tool",
                "purpose": "Execute any tool by name with arguments"
            },
            {
                "name": "batch_execute",
                "purpose": "Run multiple tools in sequence (optional)"
            }
        ],
        "workflow": [
            "1. Use list_tools or search_tools to find relevant tools",
            "2. Use get_tool_schema to see required arguments",
            "3. Use execute_tool to run the tool"
        ],
        "benefits": [
            "~95% context token savings",
            "Bypasses 40-tool limit",
            "Clearer LLM reasoning",
            "All tools still accessible"
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AggregatorConfig;

    #[tokio::test]
    async fn test_compact_mode_config_default() {
        let config = CompactModeConfig::default();
        assert!(config.enabled);
        assert!(config.include_list);
        assert!(config.include_execute);
        assert!(config.include_schema);
        assert!(config.include_search);
        assert!(!config.include_batch);
        assert_eq!(config.max_list_results, 50);
    }

    #[tokio::test]
    async fn test_create_compact_tools() {
        let agg_config = AggregatorConfig::default();
        let aggregator = Arc::new(Aggregator::new(agg_config).await.unwrap());

        let compact_config = CompactModeConfig::default();
        let tools = create_compact_tools(aggregator, &compact_config);

        // Should have 4 tools (batch disabled by default)
        assert_eq!(tools.len(), 4);

        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(names.contains(&"list_tools"));
        assert!(names.contains(&"execute_tool"));
        assert!(names.contains(&"get_tool_schema"));
        assert!(names.contains(&"search_tools"));
    }

    #[test]
    fn test_compact_mode_summary() {
        let summary = compact_mode_summary();
        assert_eq!(
            summary.as_object().and_then(|obj| obj.get("mode")).unwrap(),
            "compact"
        );
        assert!(
            summary
                .as_object()
                .and_then(|obj| obj.get("tools"))
                .unwrap()
                .as_array()
                .unwrap()
                .len()
                >= 4
        );
    }
}
</file>

<file path="src/config.rs">
//! Configuration for MCP Aggregator
//!
//! Supports loading from JSON/YAML files or environment variables.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tracing::info;

/// Main configuration for the aggregator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatorConfig {
    /// Upstream MCP servers to aggregate
    #[serde(default)]
    pub servers: Vec<UpstreamServer>,

    /// Named profiles that select subsets of tools
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,

    /// Cache settings
    #[serde(default)]
    pub cache: CacheConfig,

    /// Default profile to use if none specified
    #[serde(default = "default_profile")]
    pub default_profile: String,

    /// Maximum tools to expose per profile (Cursor limit is 40)
    #[serde(default = "default_max_tools")]
    pub max_tools_per_profile: usize,

    /// Compact mode settings
    #[serde(default)]
    pub compact_mode: crate::compact::CompactModeConfig,

    /// Client auto-detection settings
    #[serde(default)]
    pub client_detection: ClientDetectionConfig,

    /// Default tool mode (compact/full/hybrid)
    #[serde(default)]
    pub default_mode: ToolMode,
}

fn default_profile() -> String {
    "default".to_string()
}

fn default_max_tools() -> usize {
    40
}

impl Default for AggregatorConfig {
    fn default() -> Self {
        Self {
            servers: vec![],
            profiles: HashMap::new(),
            cache: CacheConfig::default(),
            default_profile: default_profile(),
            max_tools_per_profile: default_max_tools(),
            compact_mode: crate::compact::CompactModeConfig::default(),
            client_detection: ClientDetectionConfig::default(),
            default_mode: ToolMode::default(),
        }
    }
}

impl AggregatorConfig {
    /// Load configuration from a JSON file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config from {}", path.display()))?;

        let config: Self = if path
            .extension()
            .map(|e| e == "yaml" || e == "yml")
            .unwrap_or(false)
        {
            serde_yaml::from_str(&content).with_context(|| "Failed to parse YAML config")?
        } else {
            let mut content = content;
            let mut content_bytes = unsafe { content.as_bytes_mut() };
            simd_json::from_slice(&mut content_bytes)
                .with_context(|| "Failed to parse JSON config")?
        };

        info!("Loaded aggregator config from {}", path.display());
        Ok(config)
    }

    /// Load from default paths, with fallbacks
    pub fn load_default() -> Result<Self> {
        let paths = [
            "/etc/mcp/mcp-servers.json",
            "/etc/op-dbus/aggregator.json",
            "/etc/op-dbus/mcp-aggregator.json",
            "aggregator.json",
        ];

        for path in paths {
            if Path::new(path).exists() {
                return Self::load(path);
            }
        }

        // Return default config if no file found
        info!("No aggregator config found, using defaults");
        Ok(Self::default())
    }

    /// Create a builder for programmatic configuration
    pub fn builder() -> AggregatorConfigBuilder {
        AggregatorConfigBuilder::default()
    }
}

/// Builder for AggregatorConfig
#[derive(Default)]
pub struct AggregatorConfigBuilder {
    config: AggregatorConfig,
}

impl AggregatorConfigBuilder {
    pub fn server(mut self, server: UpstreamServer) -> Self {
        self.config.servers.push(server);
        self
    }

    pub fn profile(mut self, name: &str, profile: ProfileConfig) -> Self {
        self.config.profiles.insert(name.to_string(), profile);
        self
    }

    pub fn max_tools(mut self, max: usize) -> Self {
        self.config.max_tools_per_profile = max;
        self
    }

    pub fn default_profile(mut self, name: &str) -> Self {
        self.config.default_profile = name.to_string();
        self
    }

    pub fn build(self) -> AggregatorConfig {
        self.config
    }
}

/// Configuration for an upstream MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamServer {
    /// Unique identifier for this server
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Server URL (http://host:port for SSE, or command for stdio)
    pub url: String,

    /// Transport type
    #[serde(default)]
    pub transport: TransportType,

    /// Whether this server is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Tool name prefix (e.g., "github_" for github server)
    #[serde(default)]
    pub tool_prefix: Option<String>,

    /// Only include these tools (empty = all)
    #[serde(default)]
    pub include_tools: Vec<String>,

    /// Exclude these tools
    #[serde(default)]
    pub exclude_tools: Vec<String>,

    /// Priority (higher = preferred when tools conflict)
    #[serde(default)]
    pub priority: i32,

    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Optional authentication
    #[serde(default)]
    pub auth: Option<ServerAuth>,
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    30
}

impl UpstreamServer {
    /// Create a new SSE-based upstream server
    pub fn sse(id: &str, name: &str, url: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            url: url.to_string(),
            transport: TransportType::Sse,
            enabled: true,
            tool_prefix: None,
            include_tools: vec![],
            exclude_tools: vec![],
            priority: 0,
            timeout_secs: default_timeout(),
            auth: None,
        }
    }

    /// Create a new stdio-based upstream server
    pub fn stdio(id: &str, name: &str, command: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            url: command.to_string(),
            transport: TransportType::Stdio,
            enabled: true,
            tool_prefix: None,
            include_tools: vec![],
            exclude_tools: vec![],
            priority: 0,
            timeout_secs: default_timeout(),
            auth: None,
        }
    }

    /// Add a tool prefix
    pub fn with_prefix(mut self, prefix: &str) -> Self {
        self.tool_prefix = Some(prefix.to_string());
        self
    }

    /// Include only specific tools
    pub fn with_include(mut self, tools: Vec<String>) -> Self {
        self.include_tools = tools;
        self
    }

    /// Exclude specific tools
    pub fn with_exclude(mut self, tools: Vec<String>) -> Self {
        self.exclude_tools = tools;
        self
    }

    /// Get timeout as Duration
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }

    /// Check if a tool should be included from this server
    pub fn should_include_tool(&self, tool_name: &str) -> bool {
        // Check excludes first
        if self.exclude_tools.iter().any(|t| t == tool_name) {
            return false;
        }

        // If includes specified, tool must be in the list
        if !self.include_tools.is_empty() {
            return self.include_tools.iter().any(|t| t == tool_name);
        }

        true
    }

    /// Apply prefix to a tool name
    pub fn prefixed_name(&self, tool_name: &str) -> String {
        match &self.tool_prefix {
            Some(prefix) => format!("{}_{}", prefix, tool_name),
            None => tool_name.to_string(),
        }
    }
}

/// Transport type for upstream servers
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    /// HTTP + Server-Sent Events
    #[default]
    Sse,
    /// Standard I/O (for local processes)
    Stdio,
    /// WebSocket
    Websocket,
}

/// Authentication configuration for upstream servers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerAuth {
    /// Bearer token authentication
    Bearer {
        /// Token value (can be env var reference like ${GITHUB_TOKEN})
        token: String,
    },
    /// Basic authentication
    Basic { username: String, password: String },
    /// Custom header
    Header { name: String, value: String },
}

impl ServerAuth {
    /// Resolve environment variable references in auth values
    pub fn resolve(&self) -> Self {
        match self {
            Self::Bearer { token } => Self::Bearer {
                token: resolve_env_var(token),
            },
            Self::Basic { username, password } => Self::Basic {
                username: resolve_env_var(username),
                password: resolve_env_var(password),
            },
            Self::Header { name, value } => Self::Header {
                name: name.clone(),
                value: resolve_env_var(value),
            },
        }
    }
}

/// Resolve environment variable references like ${VAR_NAME}
fn resolve_env_var(value: &str) -> String {
    if value.starts_with("${") && value.ends_with('}') {
        let var_name = &value[2..value.len() - 1];
        std::env::var(var_name).unwrap_or_else(|_| value.to_string())
    } else {
        value.to_string()
    }
}

/// Profile configuration - defines which tools are available
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileConfig {
    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// Which servers to include (empty = all)
    #[serde(default)]
    pub servers: Vec<String>,

    /// Specific tools to include (empty = all from included servers)
    #[serde(default)]
    pub include_tools: Vec<String>,

    /// Tools to exclude
    #[serde(default)]
    pub exclude_tools: Vec<String>,

    /// Tool categories to include
    #[serde(default)]
    pub include_categories: Vec<String>,

    /// Tool namespaces to include
    #[serde(default)]
    pub include_namespaces: Vec<String>,

    /// Maximum tools for this profile (overrides global)
    #[serde(default)]
    pub max_tools: Option<usize>,
}

impl ProfileConfig {
    /// Create a new empty profile
    pub fn new(description: &str) -> Self {
        Self {
            description: description.to_string(),
            ..Default::default()
        }
    }

    /// Include specific servers
    pub fn with_servers(mut self, servers: Vec<&str>) -> Self {
        self.servers = servers.into_iter().map(String::from).collect();
        self
    }

    /// Include specific tools
    pub fn with_tools(mut self, tools: Vec<&str>) -> Self {
        self.include_tools = tools.into_iter().map(String::from).collect();
        self
    }

    /// Exclude specific tools
    pub fn excluding(mut self, tools: Vec<&str>) -> Self {
        self.exclude_tools = tools.into_iter().map(String::from).collect();
        self
    }

    /// Set max tools
    pub fn with_max(mut self, max: usize) -> Self {
        self.max_tools = Some(max);
        self
    }
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// How long to cache tool schemas (seconds)
    #[serde(default = "default_schema_ttl")]
    pub schema_ttl_secs: u64,

    /// Maximum cached entries
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,

    /// Whether to refresh cache in background
    #[serde(default = "default_true")]
    pub background_refresh: bool,
}

/// Client auto-detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientDetectionConfig {
    /// Enable automatic client detection
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Clients that should use compact mode by default
    #[serde(default = "default_compact_clients")]
    pub compact_mode_clients: Vec<String>,

    /// Clients that should use full mode by default
    #[serde(default = "default_full_clients")]
    pub full_mode_clients: Vec<String>,

    /// Default mode when client is unknown
    #[serde(default = "default_mode")]
    pub default_mode: String,
}

fn default_compact_clients() -> Vec<String> {
    vec![
        // Claude/Anthropic clients
        "claude".to_string(),
        "anthropic".to_string(),
        "@anthropic".to_string(),
        // ChatGPT/OpenAI clients
        "chatgpt".to_string(),
        "openai".to_string(),
        "gpt".to_string(),
        // Generic LLM/AI clients
        "llm".to_string(),
        "ai-assistant".to_string(),
        "assistant".to_string(),
        // Chatbot mode
        "chatbot".to_string(),
        "op-chat".to_string(),
        "chat".to_string(),
        // CLI tools that benefit from compact
        "cli".to_string(),
        "terminal".to_string(),
    ]
}

fn default_full_clients() -> Vec<String> {
    vec![
        // Gemini CLI - ALL variations (Google's CLI tool)
        "gemini".to_string(),         // Base match
        "gemini-cli".to_string(),     // Hyphenated
        "gemini_cli".to_string(),     // Underscored
        "gemini cli".to_string(),     // Space
        "@google/gemini".to_string(), // NPM package style
        "google-gemini".to_string(),  // Google prefix
        // Cursor IDE - has 40 tool limit but can use full mode for small sets
        "cursor".to_string(),
        // VS Code extensions
        "vscode".to_string(),
        "code".to_string(),
        // Direct API access
        "api".to_string(),
        "direct".to_string(),
    ]
}

fn default_mode() -> String {
    "compact".to_string() // Default to compact for efficiency
}

impl Default for ClientDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            compact_mode_clients: default_compact_clients(),
            full_mode_clients: default_full_clients(),
            default_mode: default_mode(),
        }
    }
}

impl ClientDetectionConfig {
    /// Detect the appropriate mode for a client
    pub fn detect_mode(&self, client_name: &str) -> ToolMode {
        if !self.enabled {
            return self.parse_default_mode();
        }

        let client_lower = client_name.to_lowercase();

        // PRIORITY 1: Explicit Gemini CLI detection (always FULL mode for Gemini 3+)
        if Self::is_gemini_cli(&client_lower) {
            tracing::info!(
                "🔷 Gemini CLI detected: '{}' -> FULL mode (Vertex AI Compact Tool Defs supported)",
                client_name
            );
            return ToolMode::Full;
        }

        // PRIORITY 2: Check for compact mode clients
        for pattern in &self.compact_mode_clients {
            let pattern_lower = pattern.to_lowercase();
            if client_lower.contains(&pattern_lower) || pattern_lower.contains(&client_lower) {
                tracing::info!(
                    "Auto-detected compact mode for client: {} (matched: {})",
                    client_name,
                    pattern
                );
                return ToolMode::Compact;
            }
        }

        // PRIORITY 3: Check for full mode clients
        for pattern in &self.full_mode_clients {
            let pattern_lower = pattern.to_lowercase();
            if client_lower.contains(&pattern_lower) || pattern_lower.contains(&client_lower) {
                tracing::info!(
                    "Auto-detected full mode for client: {} (matched: {})",
                    client_name,
                    pattern
                );
                return ToolMode::Full;
            }
        }

        // Use default (compact for safety/efficiency)
        tracing::info!(
            "Unknown client '{}', using default mode: {}",
            client_name,
            self.default_mode
        );
        self.parse_default_mode()
    }

    /// Explicit check for Gemini CLI (Google's AI CLI tool)
    fn is_gemini_cli(client_name: &str) -> bool {
        let gemini_patterns = [
            "gemini",
            "google-ai",
            "google ai",
            "googleai",
            "@google/",
            "bard", // Old name for Gemini
        ];

        for pattern in gemini_patterns {
            if client_name.contains(pattern) {
                return true;
            }
        }
        false
    }

    fn parse_default_mode(&self) -> ToolMode {
        match self.default_mode.to_lowercase().as_str() {
            "full" => ToolMode::Full,
            "hybrid" => ToolMode::Hybrid,
            _ => ToolMode::Compact,
        }
    }

    /// Check if a client name matches Gemini CLI
    pub fn is_gemini(client_name: &str) -> bool {
        Self::is_gemini_cli(&client_name.to_lowercase())
    }
}

/// Tool mode - how tools are exposed to clients
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ToolMode {
    /// Compact mode: 4-5 meta-tools (list, search, schema, execute)
    /// Best for: LLMs, chatbots, context-limited clients
    #[default]
    Compact,

    /// Full mode: All tools exposed directly
    /// Best for: IDEs, direct API access, small tool sets
    Full,

    /// Hybrid mode: Essential tools direct + meta-tools for the rest
    /// Best for: When you need a few tools always available
    Hybrid,
}

fn default_schema_ttl() -> u64 {
    300 // 5 minutes
}

fn default_max_entries() -> usize {
    1000
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            schema_ttl_secs: default_schema_ttl(),
            max_entries: default_max_entries(),
            background_refresh: true,
        }
    }
}

impl CacheConfig {
    pub fn schema_ttl(&self) -> Duration {
        Duration::from_secs(self.schema_ttl_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upstream_server_tool_filtering() {
        let server = UpstreamServer::sse("test", "Test", "http://localhost:3000")
            .with_include(vec!["tool_a".into(), "tool_b".into()])
            .with_exclude(vec!["tool_c".into()]);

        assert!(server.should_include_tool("tool_a"));
        assert!(server.should_include_tool("tool_b"));
        assert!(!server.should_include_tool("tool_c"));
        assert!(!server.should_include_tool("tool_d")); // Not in include list
    }

    #[test]
    fn test_tool_prefix() {
        let server =
            UpstreamServer::sse("gh", "GitHub", "http://localhost:3000").with_prefix("github");

        assert_eq!(server.prefixed_name("search"), "github_search");
    }

    #[test]
    fn test_resolve_env_var() {
        std::env::set_var("TEST_TOKEN", "secret123");
        assert_eq!(resolve_env_var("${TEST_TOKEN}"), "secret123");
        assert_eq!(resolve_env_var("plain_value"), "plain_value");
        std::env::remove_var("TEST_TOKEN");
    }

    #[test]
    fn test_config_builder() {
        let config = AggregatorConfig::builder()
            .server(UpstreamServer::sse(
                "local",
                "Local",
                "http://localhost:3001",
            ))
            .profile("admin", ProfileConfig::new("Admin tools").with_max(30))
            .max_tools(40)
            .default_profile("admin")
            .build();

        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.profiles.len(), 1);
        assert_eq!(config.max_tools_per_profile, 40);
        assert_eq!(config.default_profile, "admin");
    }

    #[test]
    fn test_gemini_cli_detection() {
        let config = ClientDetectionConfig::default();

        // All these should detect as Gemini CLI -> Full mode
        let gemini_clients = [
            "gemini-cli",
            "Gemini CLI",
            "gemini",
            "@google/gemini-cli",
            "google-ai-cli",
            "GoogleAI",
            "bard", // Old Gemini name
        ];

        for client in gemini_clients {
            let mode = config.detect_mode(client);
            assert_eq!(mode, ToolMode::Full, "Failed for client: {}", client);
            assert!(
                ClientDetectionConfig::is_gemini(client),
                "is_gemini failed for: {}",
                client
            );
        }
    }

    #[test]
    fn test_cursor_detection() {
        let config = ClientDetectionConfig::default();

        // Cursor should get Full mode
        let cursor_clients = ["cursor", "Cursor IDE", "cursor-editor"];

        for client in cursor_clients {
            let mode = config.detect_mode(client);
            assert_eq!(mode, ToolMode::Full, "Failed for client: {}", client);
        }
    }

    #[test]
    fn test_claude_detection() {
        let config = ClientDetectionConfig::default();

        // Claude/Anthropic should get Compact mode
        let claude_clients = ["claude", "Claude", "anthropic", "@anthropic/cli"];

        for client in claude_clients {
            let mode = config.detect_mode(client);
            assert_eq!(mode, ToolMode::Compact, "Failed for client: {}", client);
        }
    }

    #[test]
    fn test_unknown_client_default() {
        let config = ClientDetectionConfig::default();

        // Unknown clients should get default (Compact)
        let mode = config.detect_mode("some-random-unknown-client");
        assert_eq!(mode, ToolMode::Compact);
    }
}
</file>

<file path="src/groups.rs">
//! Tool Groups - Granular, domain-specific tool sets
//!
//! Groups are designed to be ~5 tools each for flexibility.
//! Mix and match to create custom configurations under any limit.
//!
//! ## Security Levels
//!
//! | Level | Description | API Key Required |
//! |-------|-------------|------------------|
//! | public | Safe read-only tools | No |
//! | standard | Normal operations | No |
//! | elevated | System modifications | Optional |
//! | restricted | Dangerous commands | **YES** |
//!
//! ## Domain Groups
//!
//! - **Core**: Essential tools (respond, info)
//! - **DevOps**: Infrastructure, deployment
//! - **Security**: Auth, SSO, secrets
//! - **Business**: Marketing, HR, analytics
//! - **System**: Restricted admin commands

use op_core::security::{AccessZone, NetworkConfig, SecurityLevel};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::info;

/// A group of related tools (~5 tools each for granularity)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolGroup {
    /// Group identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what this group provides
    pub description: String,
    /// Domain category (core, devops, security, business, system)
    pub domain: String,
    /// Tool name patterns (exact or wildcard like "systemd_*")
    pub patterns: Vec<String>,
    /// Namespace filter
    pub namespace: Option<String>,
    /// Category filter
    pub category: Option<String>,
    /// Estimated tool count (~5 for granularity)
    pub estimated_count: usize,
    /// Priority (higher = more important)
    pub priority: i32,
    /// Dependencies (other groups required)
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Whether enabled by default
    #[serde(default)]
    pub default_enabled: bool,
    /// Security level
    #[serde(default)]
    pub security: SecurityLevel,
    /// Tags for filtering
    #[serde(default)]
    pub tags: Vec<String>,
}

impl ToolGroup {
    pub fn new(id: &str, name: &str, description: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            domain: "core".to_string(),
            patterns: vec![],
            namespace: None,
            category: None,
            estimated_count: 5,
            priority: 50,
            dependencies: vec![],
            default_enabled: false,
            security: SecurityLevel::Standard,
            tags: vec![],
        }
    }

    pub fn domain(mut self, domain: &str) -> Self {
        self.domain = domain.to_string();
        self
    }

    pub fn patterns(mut self, patterns: Vec<&str>) -> Self {
        self.patterns = patterns.into_iter().map(String::from).collect();
        self
    }

    pub fn namespace(mut self, ns: &str) -> Self {
        self.namespace = Some(ns.to_string());
        self
    }

    pub fn category(mut self, cat: &str) -> Self {
        self.category = Some(cat.to_string());
        self
    }

    pub fn count(mut self, count: usize) -> Self {
        self.estimated_count = count;
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn depends_on(mut self, deps: Vec<&str>) -> Self {
        self.dependencies = deps.into_iter().map(String::from).collect();
        self
    }

    pub fn default_on(mut self) -> Self {
        self.default_enabled = true;
        self
    }

    pub fn security_level(mut self, level: SecurityLevel) -> Self {
        self.security = level;
        self
    }

    pub fn restricted(mut self) -> Self {
        self.security = SecurityLevel::Restricted;
        self
    }

    pub fn tags(mut self, tags: Vec<&str>) -> Self {
        self.tags = tags.into_iter().map(String::from).collect();
        self
    }

    /// Check if a tool matches this group
    pub fn matches_tool(
        &self,
        tool_name: &str,
        tool_namespace: Option<&str>,
        tool_category: Option<&str>,
    ) -> bool {
        if let Some(ns) = &self.namespace {
            if tool_namespace != Some(ns.as_str()) {
                return false;
            }
        }

        if let Some(cat) = &self.category {
            if tool_category != Some(cat.as_str()) {
                return false;
            }
        }

        if self.patterns.is_empty() {
            return self.namespace.is_some() || self.category.is_some();
        }

        for pattern in &self.patterns {
            if pattern.ends_with('*') {
                let prefix = &pattern[..pattern.len() - 1];
                if tool_name.starts_with(prefix) {
                    return true;
                }
            } else if tool_name == pattern {
                return true;
            }
        }

        false
    }
}

/// Manager for tool groups with IP-based access control
#[derive(Debug, Clone)]
pub struct ToolGroups {
    groups: HashMap<String, ToolGroup>,
    enabled: HashSet<String>,
    max_tools: usize,
    /// Client's access zone (based on IP)
    access_zone: AccessZone,
    /// Client IP address (for logging)
    client_ip: Option<String>,
    /// Network configuration for trusted ranges
    network_config: NetworkConfig,
}

impl ToolGroups {
    pub fn new() -> Self {
        let mut manager = Self {
            groups: HashMap::new(),
            enabled: HashSet::new(),
            max_tools: 40,
            access_zone: AccessZone::Localhost, // Default to localhost for CLI
            client_ip: None,
            network_config: NetworkConfig::default(),
        };

        for group in builtin_groups() {
            if group.default_enabled {
                manager.enabled.insert(group.id.clone());
            }
            manager.groups.insert(group.id.clone(), group);
        }

        manager
    }

    pub fn with_limit(mut self, max: usize) -> Self {
        self.max_tools = max;
        self
    }

    /// Set network configuration for trusted networks
    pub fn with_network_config(mut self, config: NetworkConfig) -> Self {
        self.network_config = config;
        self
    }

    /// Add trusted network prefix (e.g., "10.50." for Netmaker)
    pub fn trust_network(mut self, prefix: &str) -> Self {
        self.network_config = self.network_config.trust_prefix(prefix);
        self
    }

    /// Set access zone from client IP address
    pub fn from_ip(mut self, ip: &str) -> Self {
        self.access_zone = AccessZone::from_ip_with_config(ip, &self.network_config);
        self.client_ip = Some(ip.to_string());
        info!("🌐 Client IP: {} -> {}", ip, self.access_zone.description());
        self
    }

    /// Set access zone directly
    pub fn with_zone(mut self, zone: AccessZone) -> Self {
        self.access_zone = zone;
        self
    }

    /// Get current access zone
    pub fn access_zone(&self) -> AccessZone {
        self.access_zone
    }

    /// Check if client can access a security level
    pub fn can_access(&self, level: SecurityLevel) -> bool {
        self.access_zone.can_access(level)
    }

    /// Enable a group (checks IP-based security)
    pub fn enable(&mut self, group_id: &str) -> Result<(), String> {
        let group_info = match self.groups.get(group_id) {
            Some(g) => (
                g.estimated_count,
                g.dependencies.clone(),
                g.security,
                g.name.clone(),
            ),
            None => return Err(format!("Unknown group: {}", group_id)),
        };

        let (estimated_count, dependencies, security, name) = group_info;

        // Check IP-based access
        if !self.access_zone.can_access(security) {
            let required = match security {
                SecurityLevel::Restricted => "localhost (127.0.0.1)",
                SecurityLevel::Elevated => "localhost or private network",
                _ => "any",
            };
            return Err(format!(
                "Group '{}' ({:?}) requires {} access. Your zone: {}",
                name,
                security,
                required,
                self.access_zone.description()
            ));
        }

        let current_count = self.estimated_tool_count();
        if current_count + estimated_count > self.max_tools {
            return Err(format!(
                "Cannot enable '{}' ({} tools) - exceeds limit ({} + {} > {})",
                group_id, estimated_count, current_count, estimated_count, self.max_tools
            ));
        }

        // Enable dependencies first
        for dep in dependencies {
            if !self.enabled.contains(&dep) {
                self.enable(&dep)?;
            }
        }

        self.enabled.insert(group_id.to_string());
        info!(
            "✅ Enabled group '{}' (~{} tools)",
            group_id, estimated_count
        );
        Ok(())
    }

    /// Try to enable (returns bool instead of Result)
    pub fn try_enable(&mut self, group_id: &str) -> bool {
        self.enable(group_id).is_ok()
    }

    pub fn disable(&mut self, group_id: &str) {
        self.enabled.remove(group_id);
    }

    pub fn estimated_tool_count(&self) -> usize {
        self.enabled
            .iter()
            .filter_map(|id| self.groups.get(id))
            .map(|g| g.estimated_count)
            .sum()
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_tools.saturating_sub(self.estimated_tool_count())
    }

    pub fn should_include(
        &self,
        tool_name: &str,
        namespace: Option<&str>,
        category: Option<&str>,
    ) -> bool {
        for group_id in &self.enabled {
            if let Some(group) = self.groups.get(group_id) {
                if group.matches_tool(tool_name, namespace, category) {
                    return true;
                }
            }
        }
        false
    }

    /// List groups by domain
    pub fn list_by_domain(&self, domain: &str) -> Vec<&ToolGroup> {
        self.groups
            .values()
            .filter(|g| g.domain == domain)
            .collect()
    }

    /// List all groups with status
    pub fn list_all(&self) -> Vec<GroupStatus> {
        let mut result: Vec<_> = self
            .groups
            .values()
            .map(|g| GroupStatus {
                id: g.id.clone(),
                name: g.name.clone(),
                description: g.description.clone(),
                domain: g.domain.clone(),
                estimated_count: g.estimated_count,
                enabled: self.enabled.contains(&g.id),
                security: g.security,
                requires_trusted: matches!(g.security, SecurityLevel::Restricted),
            })
            .collect();
        result.sort_by(|a, b| a.domain.cmp(&b.domain).then(b.enabled.cmp(&a.enabled)));
        result
    }

    /// Apply a preset
    pub fn apply_preset(&mut self, preset: &str) -> Result<(), String> {
        self.enabled.clear();

        let groups = match preset {
            "minimal" => vec!["respond", "info"],
            "safe" => vec!["respond", "info", "read", "search"],
            "developer" => vec!["respond", "info", "read", "write", "shell-safe", "git-read"],
            "sysadmin" => vec![
                "respond",
                "info",
                "read",
                "services",
                "network-info",
                "logs",
            ],
            "architect" => vec!["respond", "info", "dbus-intro", "services", "network-info"],
            "security" => vec!["respond", "info", "auth", "secrets", "audit"],
            "devops" => vec!["respond", "info", "deploy", "containers", "monitoring"],
            "full-safe" => vec![
                "respond",
                "info",
                "read",
                "search",
                "services",
                "network-info",
                "dbus-intro",
                "monitoring",
                "logs",
            ],
            _ => return Err(format!("Unknown preset: {}", preset)),
        };

        for group in groups {
            self.enable(group)?;
        }

        Ok(())
    }

    pub fn add_group(&mut self, group: ToolGroup) {
        self.groups.insert(group.id.clone(), group);
    }
}

impl Default for ToolGroups {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupStatus {
    pub id: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub estimated_count: usize,
    pub enabled: bool,
    pub security: SecurityLevel,
    /// True if this group requires localhost or trusted network
    pub requires_trusted: bool,
}

/// Built-in granular tool groups (~5 tools each)
pub fn builtin_groups() -> Vec<ToolGroup> {
    vec![
        // =====================================================================
        // CORE DOMAIN - Essential tools
        // =====================================================================
        ToolGroup::new(
            "respond",
            "Respond",
            "Response tools for user communication",
        )
        .domain("core")
        .patterns(vec!["respond", "respond_to_user", "reply", "answer"])
        .count(3)
        .priority(100)
        .default_on()
        .security_level(SecurityLevel::Public),
        ToolGroup::new(
            "info",
            "System Info",
            "Basic system information (read-only)",
        )
        .domain("core")
        .patterns(vec![
            "system_info",
            "get_info",
            "whoami",
            "hostname",
            "uname",
        ])
        .count(5)
        .priority(95)
        .default_on()
        .security_level(SecurityLevel::Public),
        ToolGroup::new("help", "Help & Docs", "Documentation and help tools")
            .domain("core")
            .patterns(vec!["help", "man", "docs", "explain", "describe"])
            .count(4)
            .priority(90)
            .security_level(SecurityLevel::Public),
        // =====================================================================
        // FILE DOMAIN - File operations (split by permission)
        // =====================================================================
        ToolGroup::new("read", "File Read", "Read files and directories (safe)")
            .domain("files")
            .patterns(vec![
                "read_file",
                "cat",
                "head",
                "tail",
                "list_dir",
                "ls",
                "find",
            ])
            .count(6)
            .priority(85)
            .security_level(SecurityLevel::Public),
        ToolGroup::new("write", "File Write", "Write and modify files")
            .domain("files")
            .patterns(vec!["write_file", "create_file", "append", "touch"])
            .count(4)
            .priority(75)
            .depends_on(vec!["read"])
            .security_level(SecurityLevel::Standard),
        ToolGroup::new("file-manage", "File Management", "Move, copy, delete files")
            .domain("files")
            .patterns(vec!["mv", "cp", "rm", "mkdir", "rmdir", "chmod", "chown"])
            .count(6)
            .priority(70)
            .depends_on(vec!["read"])
            .security_level(SecurityLevel::Elevated),
        ToolGroup::new("search", "Search", "Search files and content")
            .domain("files")
            .patterns(vec!["grep", "find_files", "search", "locate", "which"])
            .count(5)
            .priority(80)
            .security_level(SecurityLevel::Public),
        // =====================================================================
        // SHELL DOMAIN - Command execution
        // =====================================================================
        ToolGroup::new(
            "shell-safe",
            "Shell (Safe)",
            "Safe shell commands (read-only)",
        )
        .domain("shell")
        .patterns(vec!["shell_read", "echo", "pwd", "env", "date"])
        .count(5)
        .priority(70)
        .security_level(SecurityLevel::Standard),
        ToolGroup::new("shell-exec", "Shell (Execute)", "Execute shell commands")
            .domain("shell")
            .patterns(vec!["shell_exec", "run_command", "exec"])
            .count(3)
            .priority(60)
            .depends_on(vec!["shell-safe"])
            .security_level(SecurityLevel::Elevated),
        ToolGroup::new("shell-root", "Shell (Root)", "Root/sudo shell commands")
            .domain("shell")
            .patterns(vec!["sudo", "su", "shell_root"])
            .count(3)
            .priority(10)
            .depends_on(vec!["shell-exec"])
            .restricted()
            .tags(vec!["dangerous", "requires-key"]),
        // =====================================================================
        // SYSTEMD DOMAIN - Service management
        // =====================================================================
        ToolGroup::new("services", "Services", "List and query services")
            .domain("systemd")
            .patterns(vec!["systemd_list", "service_status", "unit_status"])
            .count(4)
            .priority(75)
            .security_level(SecurityLevel::Public),
        ToolGroup::new(
            "service-control",
            "Service Control",
            "Start/stop/restart services",
        )
        .domain("systemd")
        .patterns(vec![
            "systemd_start",
            "systemd_stop",
            "systemd_restart",
            "systemd_reload",
        ])
        .count(4)
        .priority(65)
        .depends_on(vec!["services"])
        .security_level(SecurityLevel::Elevated),
        ToolGroup::new(
            "service-config",
            "Service Config",
            "Enable/disable services",
        )
        .domain("systemd")
        .patterns(vec!["systemd_enable", "systemd_disable", "systemd_mask"])
        .count(4)
        .priority(55)
        .depends_on(vec!["services"])
        .restricted()
        .tags(vec!["system-config"]),
        ToolGroup::new("journals", "Journals", "View systemd logs")
            .domain("systemd")
            .patterns(vec!["journalctl", "logs", "systemd_logs"])
            .count(3)
            .priority(70)
            .security_level(SecurityLevel::Public),
        // =====================================================================
        // NETWORK DOMAIN
        // =====================================================================
        ToolGroup::new(
            "network-info",
            "Network Info",
            "Network information (read-only)",
        )
        .domain("network")
        .patterns(vec!["ip_addr", "ifconfig", "route", "netstat", "ss"])
        .count(5)
        .priority(75)
        .security_level(SecurityLevel::Public),
        ToolGroup::new(
            "network-diag",
            "Network Diagnostics",
            "Ping, traceroute, DNS",
        )
        .domain("network")
        .patterns(vec!["ping", "traceroute", "dig", "nslookup", "curl"])
        .count(5)
        .priority(70)
        .security_level(SecurityLevel::Standard),
        ToolGroup::new(
            "network-config",
            "Network Config",
            "Configure network interfaces",
        )
        .domain("network")
        .patterns(vec!["ip_link", "ip_route", "interface_*"])
        .count(5)
        .priority(50)
        .restricted()
        .tags(vec!["network-admin"]),
        ToolGroup::new("firewall", "Firewall", "Firewall rules and policies")
            .domain("network")
            .patterns(vec!["iptables", "nft", "firewall_*", "ufw"])
            .count(5)
            .priority(40)
            .restricted()
            .tags(vec!["security", "network-admin"]),
        // =====================================================================
        // DBUS DOMAIN
        // =====================================================================
        ToolGroup::new("dbus-intro", "D-Bus Introspect", "D-Bus service discovery")
            .domain("dbus")
            .patterns(vec!["dbus_list", "dbus_introspect", "bus_list"])
            .count(4)
            .priority(70)
            .security_level(SecurityLevel::Public),
        ToolGroup::new("dbus-call", "D-Bus Call", "Call D-Bus methods")
            .domain("dbus")
            .patterns(vec!["dbus_call", "dbus_method", "bus_call"])
            .count(4)
            .priority(60)
            .depends_on(vec!["dbus-intro"])
            .security_level(SecurityLevel::Elevated),
        ToolGroup::new("dbus-monitor", "D-Bus Monitor", "Monitor D-Bus signals")
            .domain("dbus")
            .patterns(vec!["dbus_monitor", "dbus_watch", "bus_monitor"])
            .count(3)
            .priority(55)
            .security_level(SecurityLevel::Standard),
        // =====================================================================
        // MONITORING DOMAIN
        // =====================================================================
        ToolGroup::new("monitoring", "System Monitoring", "CPU, memory, disk usage")
            .domain("monitoring")
            .patterns(vec!["top", "htop", "free", "df", "du", "uptime"])
            .count(6)
            .priority(75)
            .security_level(SecurityLevel::Public),
        ToolGroup::new(
            "processes",
            "Process Management",
            "List and manage processes",
        )
        .domain("monitoring")
        .patterns(vec!["ps", "pgrep", "process_*"])
        .count(4)
        .priority(70)
        .security_level(SecurityLevel::Public),
        ToolGroup::new(
            "process-control",
            "Process Control",
            "Kill and signal processes",
        )
        .domain("monitoring")
        .patterns(vec!["kill", "pkill", "killall", "nice", "renice"])
        .count(5)
        .priority(50)
        .depends_on(vec!["processes"])
        .security_level(SecurityLevel::Elevated),
        ToolGroup::new("logs", "Log Viewing", "View system and application logs")
            .domain("monitoring")
            .patterns(vec!["tail_log", "view_log", "log_*", "dmesg"])
            .count(4)
            .priority(70)
            .security_level(SecurityLevel::Public),
        // =====================================================================
        // GIT DOMAIN
        // =====================================================================
        ToolGroup::new("git-read", "Git Read", "Git status, log, diff (read-only)")
            .domain("git")
            .patterns(vec!["git_status", "git_log", "git_diff", "git_show"])
            .count(5)
            .priority(70)
            .security_level(SecurityLevel::Public),
        ToolGroup::new("git-write", "Git Write", "Git add, commit, branch")
            .domain("git")
            .patterns(vec!["git_add", "git_commit", "git_branch", "git_checkout"])
            .count(5)
            .priority(65)
            .depends_on(vec!["git-read"])
            .security_level(SecurityLevel::Standard),
        ToolGroup::new("git-remote", "Git Remote", "Git push, pull, fetch")
            .domain("git")
            .patterns(vec!["git_push", "git_pull", "git_fetch", "git_clone"])
            .count(4)
            .priority(60)
            .depends_on(vec!["git-read"])
            .security_level(SecurityLevel::Elevated),
        // =====================================================================
        // DEVOPS DOMAIN
        // =====================================================================
        ToolGroup::new("containers", "Containers", "Container management (read)")
            .domain("devops")
            .patterns(vec!["container_list", "container_inspect", "docker_ps"])
            .count(4)
            .priority(65)
            .security_level(SecurityLevel::Standard),
        ToolGroup::new(
            "container-control",
            "Container Control",
            "Start/stop containers",
        )
        .domain("devops")
        .patterns(vec!["container_start", "container_stop", "docker_*"])
        .count(5)
        .priority(55)
        .depends_on(vec!["containers"])
        .security_level(SecurityLevel::Elevated),
        ToolGroup::new("deploy", "Deployment", "Deployment and release tools")
            .domain("devops")
            .patterns(vec!["deploy_*", "release_*", "rollback"])
            .count(5)
            .priority(60)
            .security_level(SecurityLevel::Elevated),
        ToolGroup::new(
            "k8s-read",
            "Kubernetes Read",
            "K8s get, describe (read-only)",
        )
        .domain("devops")
        .patterns(vec!["kubectl_get", "kubectl_describe", "k8s_list"])
        .count(4)
        .priority(60)
        .security_level(SecurityLevel::Standard),
        // =====================================================================
        // SECURITY DOMAIN
        // =====================================================================
        ToolGroup::new("auth", "Authentication", "Auth and identity tools")
            .domain("security")
            .patterns(vec!["auth_*", "login", "logout", "session_*"])
            .count(5)
            .priority(70)
            .security_level(SecurityLevel::Standard),
        ToolGroup::new("sso", "SSO", "Single sign-on integration")
            .domain("security")
            .patterns(vec!["sso_*", "oauth_*", "saml_*", "oidc_*"])
            .count(5)
            .priority(65)
            .security_level(SecurityLevel::Elevated),
        ToolGroup::new("secrets", "Secrets", "Secret and credential management")
            .domain("security")
            .patterns(vec!["secret_*", "vault_*", "credential_*"])
            .count(5)
            .priority(60)
            .restricted()
            .tags(vec!["sensitive"]),
        ToolGroup::new("audit", "Audit", "Security audit and compliance")
            .domain("security")
            .patterns(vec!["audit_*", "compliance_*", "scan_*"])
            .count(4)
            .priority(65)
            .security_level(SecurityLevel::Standard),
        ToolGroup::new("crypto", "Cryptography", "Encryption and signing")
            .domain("security")
            .patterns(vec!["encrypt_*", "decrypt_*", "sign_*", "verify_*"])
            .count(5)
            .priority(55)
            .security_level(SecurityLevel::Elevated),
        // =====================================================================
        // BUSINESS DOMAIN
        // =====================================================================
        ToolGroup::new("analytics", "Analytics", "Data and analytics queries")
            .domain("business")
            .patterns(vec!["analytics_*", "report_*", "metrics_*"])
            .count(5)
            .priority(60)
            .security_level(SecurityLevel::Standard),
        ToolGroup::new("marketing", "Marketing", "Marketing automation tools")
            .domain("business")
            .patterns(vec!["marketing_*", "campaign_*", "email_*"])
            .count(5)
            .priority(50)
            .security_level(SecurityLevel::Standard),
        ToolGroup::new("hr", "HR", "Human resources tools")
            .domain("business")
            .patterns(vec!["hr_*", "employee_*", "payroll_*"])
            .count(5)
            .priority(50)
            .security_level(SecurityLevel::Elevated)
            .tags(vec!["pii", "sensitive"]),
        ToolGroup::new("crm", "CRM", "Customer relationship management")
            .domain("business")
            .patterns(vec!["crm_*", "customer_*", "contact_*"])
            .count(5)
            .priority(55)
            .security_level(SecurityLevel::Standard),
        ToolGroup::new("finance", "Finance", "Financial and billing tools")
            .domain("business")
            .patterns(vec!["finance_*", "billing_*", "invoice_*"])
            .count(5)
            .priority(50)
            .security_level(SecurityLevel::Elevated)
            .tags(vec!["sensitive"]),
        // =====================================================================
        // ARCHITECT DOMAIN
        // =====================================================================
        ToolGroup::new(
            "architect-view",
            "Architecture View",
            "View system architecture",
        )
        .domain("architect")
        .patterns(vec!["arch_*", "topology_*", "diagram_*"])
        .count(4)
        .priority(65)
        .security_level(SecurityLevel::Public),
        ToolGroup::new("dependencies", "Dependencies", "Dependency analysis")
            .domain("architect")
            .patterns(vec!["deps_*", "dependency_*", "import_*"])
            .count(4)
            .priority(60)
            .security_level(SecurityLevel::Public),
        ToolGroup::new("performance", "Performance", "Performance analysis tools")
            .domain("architect")
            .patterns(vec!["perf_*", "benchmark_*", "profile_*"])
            .count(5)
            .priority(60)
            .security_level(SecurityLevel::Standard),
        // =====================================================================
        // DATABASE DOMAIN
        // =====================================================================
        ToolGroup::new("db-read", "Database Read", "Query databases (read-only)")
            .domain("database")
            .patterns(vec!["db_query", "sql_select", "db_list"])
            .count(4)
            .priority(65)
            .security_level(SecurityLevel::Standard),
        ToolGroup::new("db-write", "Database Write", "Modify database data")
            .domain("database")
            .patterns(vec!["db_insert", "db_update", "db_delete", "sql_*"])
            .count(5)
            .priority(50)
            .depends_on(vec!["db-read"])
            .security_level(SecurityLevel::Elevated),
        ToolGroup::new("db-admin", "Database Admin", "Database administration")
            .domain("database")
            .patterns(vec!["db_create", "db_drop", "db_migrate", "db_backup"])
            .count(5)
            .priority(40)
            .depends_on(vec!["db-read"])
            .restricted()
            .tags(vec!["database-admin"]),
        // =====================================================================
        // SYSTEM/RESTRICTED DOMAIN - Dangerous commands requiring API key
        // =====================================================================
        ToolGroup::new("system-power", "System Power", "Reboot, shutdown, halt")
            .domain("system")
            .patterns(vec!["reboot", "shutdown", "halt", "poweroff"])
            .count(4)
            .priority(5)
            .restricted()
            .tags(vec!["dangerous", "system-critical"]),
        ToolGroup::new(
            "system-config",
            "System Config",
            "System configuration changes",
        )
        .domain("system")
        .patterns(vec!["sysctl", "modprobe", "system_config_*"])
        .count(5)
        .priority(5)
        .restricted()
        .tags(vec!["dangerous", "system-critical"]),
        ToolGroup::new(
            "disk-format",
            "Disk Format",
            "Disk partitioning and formatting",
        )
        .domain("system")
        .patterns(vec!["fdisk", "mkfs", "parted", "mount", "umount"])
        .count(5)
        .priority(5)
        .restricted()
        .tags(vec!["dangerous", "data-loss"]),
        ToolGroup::new("user-admin", "User Admin", "User and group management")
            .domain("system")
            .patterns(vec!["useradd", "userdel", "usermod", "groupadd", "passwd"])
            .count(5)
            .priority(10)
            .restricted()
            .tags(vec!["user-management"]),
        // =====================================================================
        // OVS DOMAIN (for your networking use case)
        // =====================================================================
        ToolGroup::new("ovs-info", "OVS Info", "OVS bridge and port information")
            .domain("ovs")
            .patterns(vec!["ovs_list", "ovs_show", "ovsdb_query"])
            .count(4)
            .priority(60)
            .security_level(SecurityLevel::Standard),
        ToolGroup::new(
            "ovs-config",
            "OVS Config",
            "Configure OVS bridges and ports",
        )
        .domain("ovs")
        .patterns(vec!["ovs_add_*", "ovs_del_*", "ovs_set_*"])
        .count(5)
        .priority(50)
        .depends_on(vec!["ovs-info"])
        .security_level(SecurityLevel::Elevated),
        // =====================================================================
        // AGENTS DOMAIN
        // =====================================================================
        ToolGroup::new("agents-safe", "Agents (Safe)", "Safe agent operations")
            .domain("agents")
            .patterns(vec!["agent_list", "agent_status", "agent_describe"])
            .count(4)
            .priority(55)
            .security_level(SecurityLevel::Standard),
        ToolGroup::new("agents-invoke", "Agents Invoke", "Invoke agent operations")
            .domain("agents")
            .patterns(vec!["invoke_agent", "agent_*"])
            .count(5)
            .priority(50)
            .depends_on(vec!["agents-safe"])
            .security_level(SecurityLevel::Elevated),
    ]
}

/// Built-in presets (curated group combinations)
pub fn builtin_presets() -> Vec<GroupPreset> {
    vec![
        GroupPreset {
            id: "minimal".into(),
            name: "Minimal".into(),
            description: "Only response tools (3 tools)".into(),
            groups: vec!["respond".into()],
            estimated_total: 3,
            requires_localhost: false,
        },
        GroupPreset {
            id: "safe".into(),
            name: "Safe".into(),
            description: "Read-only, no modifications (18 tools)".into(),
            groups: vec![
                "respond".into(),
                "info".into(),
                "read".into(),
                "search".into(),
            ],
            estimated_total: 18,
            requires_localhost: false,
        },
        GroupPreset {
            id: "developer".into(),
            name: "Developer".into(),
            description: "Developer workflow (28 tools)".into(),
            groups: vec![
                "respond".into(),
                "info".into(),
                "read".into(),
                "write".into(),
                "shell-safe".into(),
                "git-read".into(),
            ],
            estimated_total: 28,
            requires_localhost: false,
        },
        GroupPreset {
            id: "sysadmin".into(),
            name: "System Admin".into(),
            description: "System administration (32 tools)".into(),
            groups: vec![
                "respond".into(),
                "info".into(),
                "read".into(),
                "services".into(),
                "network-info".into(),
                "logs".into(),
                "monitoring".into(),
            ],
            estimated_total: 32,
            requires_localhost: false,
        },
        GroupPreset {
            id: "architect".into(),
            name: "Architect".into(),
            description: "Architecture analysis (26 tools)".into(),
            groups: vec![
                "respond".into(),
                "info".into(),
                "dbus-intro".into(),
                "services".into(),
                "network-info".into(),
                "architect-view".into(),
            ],
            estimated_total: 26,
            requires_localhost: false,
        },
        GroupPreset {
            id: "security".into(),
            name: "Security".into(),
            description: "Security operations (24 tools)".into(),
            groups: vec![
                "respond".into(),
                "info".into(),
                "auth".into(),
                "audit".into(),
                "logs".into(),
            ],
            estimated_total: 24,
            requires_localhost: false,
        },
        GroupPreset {
            id: "full-admin".into(),
            name: "Full Admin".into(),
            description: "Full admin - localhost or Netmaker/Tailscale only".into(),
            groups: vec![
                "respond".into(),
                "info".into(),
                "read".into(),
                "write".into(),
                "shell-exec".into(),
                "shell-root".into(),
                "service-control".into(),
                "service-config".into(),
                "network-config".into(),
            ],
            estimated_total: 40,
            requires_localhost: true,
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub groups: Vec<String>,
    pub estimated_total: usize,
    /// Requires localhost or trusted mesh network
    pub requires_localhost: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_granular_groups() {
        let groups = builtin_groups();

        // Most groups should be ~5 tools
        for group in &groups {
            assert!(
                group.estimated_count <= 6,
                "Group '{}' has {} tools, should be <=6",
                group.id,
                group.estimated_count
            );
        }
    }

    #[test]
    fn test_restricted_requires_localhost() {
        // From public IP - should fail for restricted
        let mut groups = ToolGroups::new().with_limit(40).from_ip("8.8.8.8");
        let result = groups.enable("shell-root");
        assert!(result.is_err());

        // From localhost - should succeed
        let mut groups2 = ToolGroups::new().with_limit(40).from_ip("127.0.0.1");
        assert!(groups2.enable("shell-root").is_ok());
    }

    #[test]
    fn test_elevated_requires_private() {
        // From public IP - should fail for elevated
        let mut groups = ToolGroups::new().with_limit(40).from_ip("8.8.8.8");
        let result = groups.enable("shell-exec");
        assert!(result.is_err());

        // From private network - should succeed
        let mut groups2 = ToolGroups::new().with_limit(40).from_ip("192.168.1.100");
        assert!(groups2.enable("shell-exec").is_ok());
    }

    #[test]
    fn test_presets_under_limit() {
        for preset in builtin_presets() {
            if !preset.requires_localhost {
                assert!(
                    preset.estimated_total <= 40,
                    "Preset '{}' has {} tools, should be <=40",
                    preset.id,
                    preset.estimated_total
                );
            }
        }
    }

    #[test]
    fn test_domains() {
        let groups = ToolGroups::new();

        // Should have groups in each domain
        let domains = [
            "core", "files", "shell", "systemd", "network", "security", "business", "system",
        ];
        for domain in domains {
            let domain_groups = groups.list_by_domain(domain);
            assert!(!domain_groups.is_empty(), "No groups in domain: {}", domain);
        }
    }
}
</file>

<file path="src/groups.rs.patch">
// ADD this to the builtin_groups() function in groups.rs
// Find the vec![] and add these entries:

        // =====================================================================
        // SELF DOMAIN - Self-repository tools
        // =====================================================================
        ToolGroup::new("self-read", "Self Read", "Read your own source code")
            .domain("self")
            .patterns(vec!["self_read_file", "self_list_directory", "self_search_code"])
            .count(3)
            .priority(80)
            .security_level(SecurityLevel::Standard)
            .tags(vec!["self".to_string(), "introspection".to_string()]),
        
        ToolGroup::new("self-write", "Self Write", "Modify your own source code")
            .domain("self")
            .patterns(vec!["self_write_file"])
            .count(1)
            .priority(70)
            .depends_on(vec!["self-read"])
            .security_level(SecurityLevel::Elevated)
            .tags(vec!["self".to_string(), "modify".to_string()]),
        
        ToolGroup::new("self-git", "Self Git", "Git operations on your source code")
            .domain("self")
            .patterns(vec!["self_git_status", "self_git_diff", "self_git_commit", "self_git_log"])
            .count(4)
            .priority(75)
            .depends_on(vec!["self-read"])
            .security_level(SecurityLevel::Elevated)
            .tags(vec!["self".to_string(), "git".to_string()]),
        
        ToolGroup::new("self-build", "Self Build", "Build and deploy yourself")
            .domain("self")
            .patterns(vec!["self_build", "self_deploy"])
            .count(2)
            .priority(60)
            .depends_on(vec!["self-read", "self-git"])
            .restricted()
            .tags(vec!["self".to_string(), "build".to_string(), "deploy".to_string()]),
</file>

<file path="src/lib.rs">
//! op-mcp-aggregator: MCP Server Aggregator
//!
//! This crate provides an aggregator that proxies multiple upstream MCP servers,
//! presenting a unified tool interface while staying under Cursor's 40-tool limit.
//!
//! ## Modes
//!
//! ### Full Mode (Traditional)
//! Exposes all tools directly. Good for small tool sets (<40 tools).
//!
//! ### Compact Mode (Recommended)
//! Reduces 750+ tools to 4-5 meta-tools:
//! - `list_tools` - Browse available tools
//! - `search_tools` - Find tools by keyword  
//! - `get_tool_schema` - Get input schema for a tool
//! - `execute_tool` - Execute any tool by name
//!
//! Benefits:
//! - ~95% context token savings
//! - Bypasses 40-tool limit entirely
//! - Works with Cursor, Gemini CLI, and any MCP client
//! - All tools remain accessible via execute_tool
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    op-mcp-aggregator                        │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │              Mode: Compact / Full                     │  │
//! │  │  Compact: 4 meta-tools (list, search, schema, exec)  │  │
//! │  │  Full: All tools from all servers                     │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! │                           │                                  │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │              Profile Manager                          │  │
//! │  │  /profile/sysadmin → [systemd, network, dbus]        │  │
//! │  │  /profile/dev      → [github, filesystem, shell]     │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! │                           │                                  │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │              Upstream Registry                        │  │
//! │  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐     │  │
//! │  │  │ GitHub  │ │ Postgres│ │ Custom  │ │ Local   │     │  │
//! │  │  │ MCP     │ │ MCP     │ │ Server  │ │ Tools   │     │  │
//! │  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘     │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! │                           │                                  │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │              Tool Cache (LRU + TTL)                   │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use op_mcp_aggregator::{Aggregator, AggregatorConfig, ToolMode};
//!
//! let config = AggregatorConfig::load("/etc/op-dbus/aggregator.json")?;
//! let aggregator = Aggregator::new(config).await?;
//! aggregator.initialize().await?;
//!
//! // Get MCP tools (compact mode returns 4 meta-tools)
//! let mcp_tools = aggregator.get_mcp_tools(ToolMode::Compact).await?;
//!
//! // Or use full mode for direct tool access
//! let all_tools = aggregator.get_mcp_tools(ToolMode::Full).await?;
//! ```

pub mod aggregator;
pub mod cache;
pub mod client;
pub mod compact;
pub mod config;
pub mod groups;
pub mod profile; // Used by op-web for IP-based security

// Re-exports
pub use aggregator::{Aggregator, AggregatorStats, HealthStatus, ToolMode};
pub use cache::ToolCache;
pub use client::McpClient;
pub use compact::{compact_mode_summary, create_compact_tools, CompactModeConfig};
pub use config::{AggregatorConfig, ProfileConfig, UpstreamServer};
pub use groups::{builtin_groups, builtin_presets};
pub use op_core::security::{AccessZone, NetworkConfig, SecurityLevel};
pub use profile::ProfileManager;

/// Prelude for convenient imports
pub mod prelude {
    pub use super::{
        create_compact_tools, AccessZone, Aggregator, AggregatorConfig, CompactModeConfig,
        McpClient, NetworkConfig, ProfileConfig, ProfileManager, SecurityLevel, ToolCache,
        ToolMode, UpstreamServer,
    };
}
</file>

<file path="src/profile.rs">
//! Profile Manager for tool selection
//!
//! Manages named profiles that select subsets of tools from the aggregated pool.

use crate::cache::ToolCache;
use crate::client::ToolDefinition;
use crate::config::{AggregatorConfig, ProfileConfig};
use simd_json::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Manages tool profiles
pub struct ProfileManager {
    /// Profile configurations
    profiles: RwLock<HashMap<String, ProfileConfig>>,
    /// Default profile name
    default_profile: String,
    /// Maximum tools per profile
    max_tools: usize,
    /// Reference to tool cache
    cache: Arc<ToolCache>,
}

impl ProfileManager {
    /// Create a new profile manager
    pub fn new(config: &AggregatorConfig, cache: Arc<ToolCache>) -> Self {
        let mut profiles = config.profiles.clone();

        // Ensure we have a default profile
        if !profiles.contains_key(&config.default_profile) {
            profiles.insert(
                config.default_profile.clone(),
                ProfileConfig::new("Default profile - all tools"),
            );
        }

        Self {
            profiles: RwLock::new(profiles),
            default_profile: config.default_profile.clone(),
            max_tools: config.max_tools_per_profile,
            cache,
        }
    }

    /// Get available profile names
    pub async fn list_profiles(&self) -> Vec<String> {
        self.profiles.read().await.keys().cloned().collect()
    }

    /// Get profile configuration
    pub async fn get_profile(&self, name: &str) -> Option<ProfileConfig> {
        self.profiles.read().await.get(name).cloned()
    }

    /// Add or update a profile
    pub async fn set_profile(&self, name: &str, config: ProfileConfig) {
        self.profiles.write().await.insert(name.to_string(), config);
        info!("Updated profile: {}", name);
    }

    /// Remove a profile
    pub async fn remove_profile(&self, name: &str) -> bool {
        if name == self.default_profile {
            warn!("Cannot remove default profile: {}", name);
            return false;
        }
        self.profiles.write().await.remove(name).is_some()
    }

    /// Get the default profile name
    pub fn default_profile(&self) -> &str {
        &self.default_profile
    }

    /// Get tools for a specific profile
    pub async fn get_tools_for_profile(&self, profile_name: &str) -> Vec<ToolDefinition> {
        let profiles = self.profiles.read().await;
        let profile = profiles.get(profile_name).cloned();
        drop(profiles);

        let profile = match profile {
            Some(p) => p,
            None => {
                warn!("Profile '{}' not found, using default", profile_name);
                self.profiles
                    .read()
                    .await
                    .get(&self.default_profile)
                    .cloned()
                    .unwrap_or_default()
            }
        };

        self.filter_tools(&profile).await
    }

    /// Filter tools based on profile configuration
    async fn filter_tools(&self, profile: &ProfileConfig) -> Vec<ToolDefinition> {
        let all_tools = self.cache.list_all().await;
        let max = profile.max_tools.unwrap_or(self.max_tools);

        let mut filtered: Vec<ToolDefinition> = all_tools
            .into_iter()
            .filter(|(tool, server_id)| self.matches_profile(tool, server_id, profile))
            .map(|(tool, _)| tool)
            .collect();

        // Sort by priority/relevance (for now, just alphabetically)
        filtered.sort_by(|a, b| a.name.cmp(&b.name));

        // Apply max limit
        if filtered.len() > max {
            debug!("Profile has {} tools, limiting to {}", filtered.len(), max);
            filtered.truncate(max);
        }

        filtered
    }

    /// Check if a tool matches the profile criteria
    fn matches_profile(
        &self,
        tool: &ToolDefinition,
        server_id: &str,
        profile: &ProfileConfig,
    ) -> bool {
        // Check server filter
        if !profile.servers.is_empty() && !profile.servers.contains(&server_id.to_string()) {
            return false;
        }

        // Check tool name include filter
        if !profile.include_tools.is_empty() {
            if !profile.include_tools.iter().any(|t| {
                // Support wildcards like "github_*"
                if t.ends_with('*') {
                    tool.name.starts_with(&t[..t.len() - 1])
                } else {
                    &tool.name == t
                }
            }) {
                return false;
            }
        }

        // Check tool name exclude filter
        if profile.exclude_tools.iter().any(|t| {
            if t.ends_with('*') {
                tool.name.starts_with(&t[..t.len() - 1])
            } else {
                &tool.name == t
            }
        }) {
            return false;
        }

        // Check category filter
        if !profile.include_categories.is_empty() {
            let category = tool
                .annotations
                .as_ref()
                .and_then(|a| a.as_object())
                .and_then(|obj| obj.get("category"))
                .and_then(|c| c.as_str())
                .unwrap_or("general");

            if !profile.include_categories.contains(&category.to_string()) {
                return false;
            }
        }

        // Check namespace filter
        if !profile.include_namespaces.is_empty() {
            let namespace = tool
                .annotations
                .as_ref()
                .and_then(|a| a.as_object())
                .and_then(|obj| obj.get("namespace"))
                .and_then(|n| n.as_str())
                .unwrap_or("system");

            if !profile.include_namespaces.contains(&namespace.to_string()) {
                return false;
            }
        }

        true
    }

    /// Check if a tool is available in a profile
    pub async fn tool_available_in_profile(&self, tool_name: &str, profile_name: &str) -> bool {
        let tools = self.get_tools_for_profile(profile_name).await;
        let result = tools.iter().any(|t| t.name == tool_name);
        result
    }

    /// Get profile stats
    pub async fn get_profile_stats(&self, profile_name: &str) -> ProfileStats {
        let tools = self.get_tools_for_profile(profile_name).await;

        let mut categories: HashMap<String, usize> = HashMap::new();
        for tool in &tools {
            let category = tool
                .annotations
                .as_ref()
                .and_then(|a| a.as_object())
                .and_then(|obj| obj.get("category"))
                .and_then(|c| c.as_str())
                .unwrap_or("general")
                .to_string();
            *categories.entry(category).or_insert(0) += 1;
        }

        ProfileStats {
            tool_count: tools.len(),
            max_tools: self.max_tools,
            categories,
        }
    }
}

/// Statistics about a profile
#[derive(Debug, Clone)]
pub struct ProfileStats {
    pub tool_count: usize,
    pub max_tools: usize,
    pub categories: HashMap<String, usize>,
}

impl ProfileStats {
    pub fn remaining_capacity(&self) -> usize {
        self.max_tools.saturating_sub(self.tool_count)
    }

    pub fn is_at_capacity(&self) -> bool {
        self.tool_count >= self.max_tools
    }
}

/// Create default profiles for common use cases
pub fn create_default_profiles() -> HashMap<String, ProfileConfig> {
    let mut profiles = HashMap::new();

    // Minimal profile - only essential tools
    profiles.insert(
        "minimal".to_string(),
        ProfileConfig {
            description: "Essential tools only".to_string(),
            max_tools: Some(10),
            include_categories: vec!["response".to_string(), "system".to_string()],
            ..Default::default()
        },
    );

    // Sysadmin profile - system management tools
    profiles.insert(
        "sysadmin".to_string(),
        ProfileConfig {
            description: "System administration tools".to_string(),
            max_tools: Some(35),
            include_namespaces: vec![
                "system".to_string(),
                "systemd".to_string(),
                "network".to_string(),
                "dbus".to_string(),
            ],
            ..Default::default()
        },
    );

    // Developer profile - development tools
    profiles.insert(
        "dev".to_string(),
        ProfileConfig {
            description: "Development tools".to_string(),
            max_tools: Some(35),
            include_categories: vec![
                "filesystem".to_string(),
                "shell".to_string(),
                "git".to_string(),
                "code".to_string(),
            ],
            ..Default::default()
        },
    );

    // Full profile - everything (may exceed limits)
    profiles.insert(
        "full".to_string(),
        ProfileConfig {
            description: "All available tools (may exceed Cursor limits)".to_string(),
            max_tools: Some(100),
            ..Default::default()
        },
    );

    profiles
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::json;
    use std::time::Duration;

    fn make_tool(name: &str, category: &str, namespace: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: "Test".to_string(),
            input_schema: json!({}),
            schema_version: String::new(),
            category: category.to_string(),
            tags: vec![],
            namespace: namespace.to_string(),
            annotations: Some(json!({
                "category": category,
                "namespace": namespace
            })),
        }
    }

    #[tokio::test]
    async fn test_profile_filtering() {
        let cache = Arc::new(ToolCache::new(100, Duration::from_secs(300)));

        // Add tools to cache
        cache
            .insert(make_tool("tool1", "system", "system"), "server1")
            .await;
        cache
            .insert(make_tool("tool2", "network", "network"), "server1")
            .await;
        cache
            .insert(make_tool("tool3", "dev", "dev"), "server2")
            .await;

        let config = AggregatorConfig::default();
        let manager = ProfileManager::new(&config, cache);

        // Add a restrictive profile
        manager
            .set_profile(
                "system_only",
                ProfileConfig {
                    description: "System tools".to_string(),
                    include_namespaces: vec!["system".to_string()],
                    ..Default::default()
                },
            )
            .await;

        let tools = manager.get_tools_for_profile("system_only").await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "tool1");
    }

    #[tokio::test]
    async fn test_wildcard_matching() {
        let cache = Arc::new(ToolCache::new(100, Duration::from_secs(300)));

        cache
            .insert(make_tool("github_search", "git", "git"), "gh")
            .await;
        cache
            .insert(make_tool("github_pr_list", "git", "git"), "gh")
            .await;
        cache
            .insert(make_tool("shell_exec", "shell", "system"), "local")
            .await;

        let config = AggregatorConfig::default();
        let manager = ProfileManager::new(&config, cache);

        manager
            .set_profile(
                "github",
                ProfileConfig {
                    description: "GitHub tools".to_string(),
                    include_tools: vec!["github_*".to_string()],
                    ..Default::default()
                },
            )
            .await;

        let tools = manager.get_tools_for_profile("github").await;
        assert_eq!(tools.len(), 2);
    }

    #[tokio::test]
    async fn test_max_tools_limit() {
        let cache = Arc::new(ToolCache::new(100, Duration::from_secs(300)));

        // Add many tools
        for i in 0..50 {
            cache
                .insert(
                    make_tool(&format!("tool{}", i), "general", "system"),
                    "server",
                )
                .await;
        }

        let mut config = AggregatorConfig::default();
        config.max_tools_per_profile = 20;
        let manager = ProfileManager::new(&config, cache);

        let tools = manager.get_tools_for_profile("default").await;
        assert_eq!(tools.len(), 20);
    }
}
</file>

<file path="Cargo.toml">
[package]
name = "op-mcp-aggregator"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "MCP Server Aggregator - proxies and aggregates multiple MCP servers behind a single endpoint"

[dependencies]
# Workspace crates
op-core = { workspace = true }
op-tools = { workspace = true }
op-plugins = { workspace = true }

# Async runtime
tokio = { workspace = true, features = ["full", "sync"] }
futures = { workspace = true }
async-trait = { workspace = true }

# Serialization
serde = { workspace = true, features = ["derive"] }
simd-json = { workspace = true }
serde_yaml = { workspace = true }

# HTTP client for upstream MCP servers
reqwest = { workspace = true, features = ["json"] }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# Logging
tracing = { workspace = true }

# Utils
uuid = { workspace = true, features = ["v4"] }
chrono = { workspace = true }

# Caching
lru = { workspace = true }

# Base64 for auth
base64 = { workspace = true }

[dev-dependencies]
tokio-test = "0.4"
</file>

<file path="CLEANUP-CONTEXT-AWARE.md">
# Context-Aware Code Cleanup

## Summary

Removed unused context-aware tool loading code from `op-mcp-aggregator` since the project uses the simpler compact mode implementation in `op-web/mcp_compact.rs` instead.

## What Was Removed

### Files Moved to `crates/op-mcp-aggregator/src/unused/`:

1. **`context.rs`** (632 lines)
   - Context-aware tool suggestion system
   - Analyzed conversation context (files, keywords, commands, intent)
   - Auto-enabled relevant tool groups based on confidence scores
   - **Why unused**: Compact mode doesn't need context analysis - it exposes all tools via meta-tools

2. **`groups.rs`** (likely similar size)
   - Tool group management and organization
   - Security levels and access zones
   - Network-based tool filtering
   - **Why unused**: Compact mode doesn't organize tools into groups - it provides search/execute instead

### Code Removed from `lib.rs`:

```rust
// Removed module declarations
pub mod groups;
pub mod context;

// Removed re-exports
pub use groups::{ToolGroups, ToolGroup, GroupStatus, SecurityLevel, AccessZone, NetworkConfig, builtin_groups, builtin_presets};
pub use context::{ContextAwareTools, ConversationContext, ContextSuggestion};
```

## Why This Was Safe

1. **No imports found**: Searched entire codebase - no files import these modules
2. **Different architecture**: `op-web` uses its own compact mode implementation
3. **Simpler is better**: Compact mode (4 meta-tools) is more effective than context-aware groups (still limited to 40 tools)

## The Two Approaches

### Context-Aware (Removed)
- ✅ Smart: Auto-detects what you're working on
- ✅ Suggests relevant tool groups
- ❌ Complex: Requires conversation analysis
- ❌ Still limited: Max 40 tools even with smart selection
- ❌ Not used: No code was calling it

### Compact Mode (Current)
- ✅ Simple: 4 meta-tools (list, search, schema, execute)
- ✅ Unlimited: All 138 tools accessible via execute_tool
- ✅ Fast: No context analysis overhead
- ✅ Universal: Works with all MCP clients
- ✅ Actually deployed: Running at `https://op-dbus.ghostbridge.tech/mcp/compact`

## Performance Impact

**Before**: ~1000 lines of unused context-aware code
**After**: Clean, focused codebase
**Build time**: Slightly faster (less code to compile)
**Runtime**: No change (code wasn't being called anyway)

## Recovery

If you ever want to restore the context-aware code:
```bash
mv crates/op-mcp-aggregator/src/unused/*.rs crates/op-mcp-aggregator/src/
# Then restore the lib.rs exports
```

## Recommendation

Keep using compact mode. It's simpler, more powerful, and actually works with your current setup.
</file>

<file path="compare-op-mcp-aggregator.md">
# compare-op-mcp-aggregator

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md, README.md, CLEANUP-CONTEXT-AWARE.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 9 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 7 |
| Partial artifacts | 1 |
| Spec-listed source files | 9 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- MCP Server Aggregator - proxies and aggregates multiple MCP servers behind a single endpoint
- Internal crate integrations: op-core, op-tools, op-plugins.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/unused/context.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/unused/context.rs |
| `src/config.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/config.rs |
| `src/compact.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/compact.rs |
| `src/client.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/client.rs |
| `src/cache.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/cache.rs |
| `src/aggregator.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/aggregator.rs |
| `src/groups.rs` | ⚠️ Partial | Declared in source inventory from spec/design docs | src/groups.rs; partial artifacts: src/groups.rs.patch |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/profile.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/profile.rs |
| `root` | ✅ Present | root source group | src/aggregator.rs, src/cache.rs, src/client.rs, src/compact.rs, src/config.rs, src/groups.rs, src/lib.rs, src/profile.rs |
| `unused` | ✅ Present | unused group | src/unused/context.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| config | ✅ Implemented | src/config.rs | SPEC main module |
| compact | ✅ Implemented | src/compact.rs | SPEC main module |
| client | ✅ Implemented | src/client.rs | SPEC main module |
| cache | ✅ Implemented | src/cache.rs | SPEC main module |
| aggregator | ✅ Implemented | src/aggregator.rs | SPEC main module |
| groups | ✅ Implemented | src/groups.rs | SPEC main module |
| profile | ✅ Implemented | src/profile.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-tools` - documented in SPEC
- `op-plugins` - documented in SPEC

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `futures` - documented in SPEC
- `async-trait` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `serde_yaml` - documented in SPEC
- `reqwest` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - not listed in SPEC dependency block
- `tracing` - not listed in SPEC dependency block
- `uuid` - not listed in SPEC dependency block
- `chrono` - not listed in SPEC dependency block
- `lru` - not listed in SPEC dependency block
- `base64` - not listed in SPEC dependency block

### Development and Build Dependencies
- `dev:tokio-test`

## Notes and Observations

- Local documentation files present: CLEANUP-CONTEXT-AWARE.md, README.md, SPEC.md.
- Transitional or partial artifacts detected: src/groups.rs.patch.
- Root module declarations found in `lib.rs`/`main.rs`: aggregator, cache, client, compact, config, groups, profile.
- 6 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="README.md">
# op-mcp-aggregator

MCP Server Aggregator - proxies and aggregates multiple MCP servers behind a single endpoint, with intelligent tool management to stay under Cursor's 40-tool limit.

## Features

| Feature | Description |
|---------|-------------|
| **Compact Mode** | Reduces 750+ tools to 4 meta-tools (~95% context savings) |
| **Tool Groups** | Organize tools into toggleable sets (systemd, network, etc.) |
| **Auto-Detection** | Automatically detects Gemini CLI, Cursor, Claude → optimal mode |
| **Profiles** | Named configurations for different use cases |
| **Multi-Server** | Aggregate tools from unlimited upstream MCP servers |

## Problem

Cursor IDE has a hard limit of ~40 MCP tools. If you have multiple MCP servers or a server with many tools, you quickly hit this limit.

## Solutions

### 1. Compact Mode (Recommended for LLMs)

Exposes only 4 meta-tools instead of hundreds:
- `list_tools` - Browse available tools
- `search_tools` - Find tools by keyword
- `get_tool_schema` - Get input schema
- `execute_tool` - Run any tool

**Auto-enabled for:** Gemini CLI, Claude, ChatGPT, any LLM client

### 2. Tool Groups (For Full Mode)

Organize tools into toggleable sets:

| Group | Description | ~Tools |
|-------|-------------|--------|
| core | Essential (respond, system_info) | 5 |
| shell | Command execution | 3 |
| filesystem | File operations | 10 |
| systemd | Service management | 12 |
| network | Network config | 10 |
| dbus | D-Bus introspection | 8 |
| packages | Package management | 6 |
| monitoring | System metrics | 8 |
| git | Version control | 10 |

**Example:** Enable `core + shell + systemd + network = 30 tools` (under 40!)

This crate provides an **aggregator** that:

1. **Connects to multiple upstream MCP servers** (SSE, stdio, websocket)
2. **Caches tool schemas** with TTL and LRU eviction
3. **Provides named profiles** that select subsets of tools
4. **Routes tool calls** to the correct upstream server
5. **Stays under Cursor's limits** per-profile

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Cursor IDE                               │
│                         │                                    │
│              ~/.cursor/mcp.json                             │
│              url: "http://localhost:3001/mcp/profile/dev"   │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                 op-mcp-aggregator                           │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Profile Manager                          │  │
│  │  /profile/sysadmin → [systemd, network, dbus]        │  │
│  │  /profile/dev      → [github, filesystem, shell]     │  │
│  │  /profile/minimal  → [respond, system_info]          │  │
│  └───────────────────────────────────────────────────────┘  │
│                           │                                  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Tool Cache (LRU + TTL)                   │  │
│  │  Schemas cached, routes tool calls to servers         │  │
│  └───────────────────────────────────────────────────────┘  │
│                           │                                  │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐           │
│  │ Local   │ │ GitHub  │ │ Postgres│ │ Custom  │           │
│  │ op-dbus │ │ MCP     │ │ MCP     │ │ Server  │           │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘           │
└─────────────────────────────────────────────────────────────┘
```

## Configuration

Create `/etc/op-dbus/aggregator.json`:

```json
{
  "servers": [
    {
      "id": "local",
      "name": "Local op-dbus",
      "url": "http://localhost:3001",
      "transport": "sse",
      "enabled": true,
      "priority": 100
    },
    {
      "id": "github",
      "name": "GitHub MCP",
      "url": "http://localhost:3002",
      "transport": "sse",
      "tool_prefix": "github",
      "include_tools": ["search_repositories", "search_code"],
      "auth": {
        "type": "bearer",
        "token": "${GITHUB_TOKEN}"
      }
    }
  ],
  "profiles": {
    "sysadmin": {
      "description": "System administration tools",
      "servers": ["local"],
      "include_namespaces": ["system", "systemd", "network"],
      "max_tools": 35
    },
    "dev": {
      "description": "Development tools", 
      "servers": ["local", "github"],
      "include_tools": ["github_*", "shell_*", "file_*"],
      "max_tools": 35
    }
  },
  "default_profile": "sysadmin",
  "max_tools_per_profile": 40
}
```

## Usage

### In Cursor

Update `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "op-dbus": {
      "url": "http://localhost:3001/mcp/profile/sysadmin",
      "transport": "sse"
    }
  }
}
```

Switch profiles by changing the URL path:
- `/mcp/profile/sysadmin` - System admin tools
- `/mcp/profile/dev` - Development tools  
- `/mcp/profile/minimal` - Essential tools only

### Programmatic

```rust
use op_mcp_aggregator::{Aggregator, AggregatorConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load config
    let config = AggregatorConfig::load("/etc/op-dbus/aggregator.json")?;
    
    // Create and initialize aggregator
    let aggregator = Aggregator::new(config).await?;
    aggregator.initialize().await?;
    
    // List tools for a profile
    let tools = aggregator.list_tools("sysadmin").await?;
    println!("Profile 'sysadmin' has {} tools", tools.len());
    
    // Call a tool
    let result = aggregator.call_tool("system_info", serde_json::json!({})).await?;
    println!("Result: {:?}", result);
    
    Ok(())
}
```

## Server Configuration

### Server Options

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier |
| `name` | string | Human-readable name |
| `url` | string | Server URL or command |
| `transport` | `sse`/`stdio`/`websocket` | Connection type |
| `enabled` | bool | Whether to use this server |
| `tool_prefix` | string? | Prefix added to tool names |
| `include_tools` | string[] | Only include these tools |
| `exclude_tools` | string[] | Exclude these tools |
| `priority` | int | Higher = preferred when tools conflict |
| `timeout_secs` | int | Connection timeout |
| `auth` | object? | Authentication config |

### Profile Options

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Human-readable description |
| `servers` | string[] | Which servers to include |
| `include_tools` | string[] | Specific tools (supports `*` wildcard) |
| `exclude_tools` | string[] | Tools to exclude |
| `include_categories` | string[] | Tool categories to include |
| `include_namespaces` | string[] | Namespaces to include |
| `max_tools` | int? | Max tools for this profile |

## Features

- **Multi-server aggregation**: Connect to unlimited upstream MCP servers
- **Profile-based filtering**: Define named profiles with different tool sets
- **Wildcard support**: Use `github_*` to match tool names
- **Tool prefixing**: Avoid name collisions with prefixes like `github_search`
- **LRU caching**: Efficient tool schema caching with TTL
- **Background refresh**: Keep schemas fresh automatically
- **Health checks**: Monitor upstream server status
- **Auth support**: Bearer tokens, basic auth, custom headers
- **Environment variables**: Use `${VAR_NAME}` in config values

## Integration with op-mcp

The aggregator integrates seamlessly with `op-mcp`. Add upstream servers and profiles to your existing setup without changing how Cursor connects.
</file>

<file path="SPEC.md">
# op-mcp-aggregator - Specification

## Overview
**Crate**: `op-mcp-aggregator`  
**Location**: `crates/op-mcp-aggregator`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-mcp-aggregator"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-mcp-aggregator/src/unused/context.rs
op-mcp-aggregator/src/config.rs
op-mcp-aggregator/src/compact.rs
op-mcp-aggregator/src/client.rs
op-mcp-aggregator/src/cache.rs
op-mcp-aggregator/src/aggregator.rs
op-mcp-aggregator/src/groups.rs
op-mcp-aggregator/src/lib.rs
op-mcp-aggregator/src/profile.rs
```

### Key Dependencies
```toml
# Workspace crates
op-core = { workspace = true }
op-tools = { workspace = true }
op-plugins = { workspace = true }

# Async runtime
tokio = { workspace = true, features = ["full", "sync"] }
futures = { workspace = true }
async-trait = { workspace = true }

# Serialization
serde = { workspace = true, features = ["derive"] }
simd-json = { workspace = true }
serde_yaml = { workspace = true }

# HTTP client for upstream MCP servers
reqwest = { workspace = true, features = ["json"] }

# Error handling
anyhow = { workspace = true }
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
README.md
CLEANUP-CONTEXT-AWARE.md

## Module Structure
       9 Rust source files

### Main Modules
config
compact
client
cache
aggregator
groups
profile

## Purpose
MCP Server Aggregator - proxies and aggregates multiple MCP servers behind a single endpoint

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
</file>

</files>
