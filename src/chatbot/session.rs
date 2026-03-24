//! Session management for chatbot.
//!
//! Sessions track conversation state and execution history.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ChatResponse;

/// Session identifier
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Exchange record in session
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Exchange {
    pub user_input: String,
    pub response: ChatResponse,
    pub timestamp_ns: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp_ns: u128,
}

impl ChatMessage {
    pub fn user(content: &str) -> Self {
        Self {
            role: MessageRole::User,
            content: content.to_string(),
            timestamp_ns: now_ns(),
        }
    }

    pub fn assistant(content: &str) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.to_string(),
            timestamp_ns: now_ns(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Chat session state
#[derive(Debug, Clone)]
pub struct ChatSession {
    /// Session identifier
    pub id: SessionId,
    /// Session creation time
    pub created_at_ns: u128,
    /// Last activity time
    pub last_activity_ns: u128,
    /// Exchange history
    pub exchanges: Vec<Exchange>,
    /// Message history
    pub messages: Vec<ChatMessage>,
    /// Tools executed in this session
    pub executed_tools: Vec<String>,
    /// Entities mentioned in conversation
    pub entities: Vec<String>,
    /// Session metadata
    pub metadata: simd_json::OwnedValue,
}

impl ChatSession {
    pub fn new(id: SessionId) -> Self {
        let now = now_ns();

        Self {
            id,
            created_at_ns: now,
            last_activity_ns: now,
            exchanges: Vec::new(),
            messages: Vec::new(),
            executed_tools: Vec::new(),
            entities: Vec::new(),
            metadata: simd_json::json!({}),
        }
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.last_activity_ns = message.timestamp_ns;
        self.messages.push(message);
    }

    pub fn context_summary(&self) -> String {
        // Simple summary of last few messages
        self.messages.iter().rev().take(5).rev()
            .map(|m| format!("{:?}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Record an exchange
    pub fn record_exchange(&mut self, user_input: String, response: ChatResponse) {
        let now = now_ns();

        self.last_activity_ns = now;
        
        // Track executed tool
        // Note: ChatResponse might not have tool_executed directly, adapting...
        if !response.actions_taken.is_empty() {
             for action in &response.actions_taken {
                 if !self.executed_tools.contains(&action.tool) {
                     self.executed_tools.push(action.tool.clone());
                 }
             }
        }

        self.exchanges.push(Exchange {
            user_input,
            response,
            timestamp_ns: now,
        });
    }

    /// Get recent exchanges
    pub fn recent_exchanges(&self, count: usize) -> &[Exchange] {
        let start = self.exchanges.len().saturating_sub(count);
        &self.exchanges[start..]
    }

    /// Add an entity
    pub fn add_entity(&mut self, entity: String) {
        if !self.entities.contains(&entity) {
            self.entities.push(entity);
        }
    }
}

fn now_ns() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

/// Session manager
pub struct SessionManager {
    sessions: DashMap<SessionId, ChatSession>,
    /// Maximum session age in nanoseconds (default: 1 hour)
    max_age_ns: u128,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
            max_age_ns: 3600 * 1_000_000_000, // 1 hour
        }
    }

    /// Get or create a session
    pub fn get_or_create(&self, id: &SessionId) -> ChatSession {
        self.sessions
            .entry(id.clone())
            .or_insert_with(|| ChatSession::new(id.clone()))
            .clone()
    }

    /// Get a session if it exists
    pub fn get(&self, id: &SessionId) -> Option<ChatSession> {
        self.sessions.get(id).map(|s| s.clone())
    }

    /// Update a session
    pub fn update(&self, session: ChatSession) {
        self.sessions.insert(session.id.clone(), session);
    }

    /// Record an exchange in a session
    pub fn record_exchange(
        &self,
        id: &SessionId,
        user_input: String,
        response: ChatResponse,
    ) {
        if let Some(mut session) = self.sessions.get_mut(id) {
            session.record_exchange(user_input, response);
        }
    }


    /// Remove a session
    pub fn remove(&self, id: &SessionId) -> Option<ChatSession> {
        self.sessions.remove(id).map(|(_, s)| s)
    }

    /// List all session IDs
    pub fn list_sessions(&self) -> Vec<SessionId> {
        self.sessions.iter().map(|e| e.key().clone()).collect()
    }

    /// Cleanup expired sessions
    pub fn cleanup_expired(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        self.sessions.retain(|_, session| {
            now - session.last_activity_ns < self.max_age_ns
        });
    }

    /// Get session count
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
