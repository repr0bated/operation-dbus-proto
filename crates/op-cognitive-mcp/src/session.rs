//! Session Manager — Conversation Memory (R2, R10)
//!
//! Provides conversation_id-based session tracking for follow-up queries.
//! Sessions are stored in SQLite alongside the memory store for durability.
//! Each session holds the conversation context and query history.
//!
//! Operations: create, get, list, reset, close.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// A conversation session for NotebookLM follow-up queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSession {
    pub id: String,
    pub notebook_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub query_count: u32,
    /// Last N queries for context window (sliding window).
    pub history: Vec<QueryTurn>,
    pub active: bool,
}

/// A single query/answer turn within a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryTurn {
    pub query: String,
    pub answer: String,
    pub timestamp: DateTime<Utc>,
    pub citations_count: u32,
    pub grounded: bool,
}

/// Session manager backed by an explicitly bounded, in-memory DashMap.
/// Durable memory belongs to the canonical Cozo memory store rather than an
/// unbounded conversation map.
pub struct SessionManager {
    sessions: Arc<DashMap<String, ConversationSession>>,
    /// Maximum turns kept per conversation before eviction.
    max_history: usize,
    /// Maximum retained session records before least-recently-used eviction.
    max_sessions: usize,
}

impl SessionManager {
    pub fn new(max_history: usize) -> Self {
        Self::with_limits(max_history, 1_000)
    }

    pub fn with_limits(max_history: usize, max_sessions: usize) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            max_history,
            max_sessions: max_sessions.max(1),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(20)
    }

    /// Get or create a session for a conversation_id.
    /// If conversation_id is empty, generates a new one.
    pub fn get_or_create(&self, conversation_id: &str, notebook_id: &str) -> ConversationSession {
        let id = if conversation_id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            conversation_id.to_string()
        };

        if let Some(mut existing) = self.sessions.get_mut(&id) {
            existing.updated_at = Utc::now();
            return existing.clone();
        }

        self.evict_for_new_session();
        let session = ConversationSession {
            id: id.clone(),
            notebook_id: notebook_id.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            query_count: 0,
            history: Vec::new(),
            active: true,
        };
        // If another request created the same supplied ID while this request
        // chose an eviction candidate, preserve that conversation rather than
        // replacing its history.
        self.sessions.entry(id).or_insert_with(|| session).clone()
    }

    /// Ensure an existing caller-selected conversation remains bound to the
    /// notebook that created it.  A session carries history and ephemeral
    /// context signals, so allowing it to silently cross notebook boundaries
    /// would make both query history and proactive context misleading.
    ///
    /// Empty IDs are intentionally allowed: `get_or_create` will mint a fresh
    /// opaque conversation ID for those requests.
    pub fn ensure_notebook_binding(&self, conversation_id: &str, notebook_id: &str) -> Result<()> {
        if conversation_id.is_empty() {
            return Ok(());
        }
        let Some(existing) = self.sessions.get(conversation_id) else {
            return Ok(());
        };
        if existing.notebook_id == notebook_id {
            return Ok(());
        }
        anyhow::bail!(
            "conversation '{}' is already bound to notebook '{}'; use a new conversation ID for '{}'",
            conversation_id,
            existing.notebook_id,
            notebook_id,
        );
    }

    /// Atomically enough for callers to prevent a race between a preflight
    /// binding check and `get_or_create`: if another request bound the same
    /// caller-selected ID in between, this returns an error before any query
    /// history or context activity can be attached to the wrong notebook.
    pub fn get_or_create_bound(
        &self,
        conversation_id: &str,
        notebook_id: &str,
    ) -> Result<ConversationSession> {
        self.ensure_notebook_binding(conversation_id, notebook_id)?;
        let session = self.get_or_create(conversation_id, notebook_id);
        self.ensure_notebook_binding(&session.id, notebook_id)?;
        Ok(session)
    }

    fn evict_for_new_session(&self) {
        while self.sessions.len() >= self.max_sessions {
            // Closed sessions are evicted first; within either class evict the
            // least recently updated record. This keeps active follow-ups
            // stable while placing a hard bound on process memory.
            let candidate = self
                .sessions
                .iter()
                .map(|entry| (entry.key().clone(), entry.active, entry.updated_at))
                .min_by_key(|(_, active, updated_at)| (*active, *updated_at))
                .map(|(id, _, _)| id);
            let Some(id) = candidate else {
                break;
            };
            self.sessions.remove(&id);
        }
    }

    /// Append a query turn to the conversation and return the updated session.
    pub fn append_turn(
        &self,
        conversation_id: &str,
        turn: QueryTurn,
    ) -> Result<ConversationSession> {
        let mut entry = self
            .sessions
            .get_mut(conversation_id)
            .context(format!("session '{}' not found", conversation_id))?;

        entry.query_count += 1;
        entry.updated_at = Utc::now();
        entry.history.push(turn);

        // Evict oldest turns beyond max_history
        while entry.history.len() > self.max_history {
            entry.history.remove(0);
        }

        Ok(entry.clone())
    }

    /// List all active sessions.
    pub fn list_sessions(&self) -> Vec<ConversationSession> {
        self.sessions
            .iter()
            .filter(|entry| entry.active)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Reset a session (clear history, keep ID).
    pub fn reset_session(&self, conversation_id: &str) -> Result<ConversationSession> {
        let mut entry = self
            .sessions
            .get_mut(conversation_id)
            .context(format!("session '{}' not found", conversation_id))?;

        entry.history.clear();
        entry.query_count = 0;
        entry.updated_at = Utc::now();
        Ok(entry.clone())
    }

    /// Close a session (marks inactive, retains for audit).
    pub fn close_session(&self, conversation_id: &str) -> Result<()> {
        let mut entry = self
            .sessions
            .get_mut(conversation_id)
            .context(format!("session '{}' not found", conversation_id))?;

        entry.active = false;
        entry.updated_at = Utc::now();
        Ok(())
    }

    /// Get a specific session.
    pub fn get_session(&self, conversation_id: &str) -> Option<ConversationSession> {
        self.sessions.get(conversation_id).map(|e| e.clone())
    }

    /// Total session count.
    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    /// Active session count.
    pub fn active_count(&self) -> usize {
        self.sessions.iter().filter(|e| e.active).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_create_session_with_generated_id() {
        let mgr = SessionManager::with_defaults();
        let session = mgr.get_or_create("", "notebook-1");
        assert!(!session.id.is_empty());
        assert_eq!(session.notebook_id, "notebook-1");
        assert!(session.active);
    }

    #[test]
    fn should_reuse_existing_session() {
        let mgr = SessionManager::with_defaults();
        let s1 = mgr.get_or_create("conv-abc", "notebook-1");
        let s2 = mgr.get_or_create("conv-abc", "notebook-1");
        assert_eq!(s1.id, s2.id);
    }

    #[test]
    fn should_append_and_evict_turns() {
        let mgr = SessionManager::new(2);
        let session = mgr.get_or_create("conv-x", "nb-1");
        assert_eq!(session.query_count, 0);

        for i in 0..5 {
            mgr.append_turn(
                "conv-x",
                QueryTurn {
                    query: format!("q{}", i),
                    answer: format!("a{}", i),
                    timestamp: Utc::now(),
                    citations_count: 0,
                    grounded: true,
                },
            )
            .unwrap();
        }

        let updated = mgr.get_session("conv-x").unwrap();
        assert_eq!(updated.query_count, 5);
        // Only last 2 turns kept
        assert_eq!(updated.history.len(), 2);
        assert_eq!(updated.history[0].query, "q3");
    }

    #[test]
    fn should_reset_session() {
        let mgr = SessionManager::with_defaults();
        mgr.get_or_create("conv-r", "nb-1");
        mgr.append_turn(
            "conv-r",
            QueryTurn {
                query: "q".into(),
                answer: "a".into(),
                timestamp: Utc::now(),
                citations_count: 0,
                grounded: true,
            },
        )
        .unwrap();

        let reset = mgr.reset_session("conv-r").unwrap();
        assert_eq!(reset.query_count, 0);
        assert!(reset.history.is_empty());
    }

    #[test]
    fn should_close_session() {
        let mgr = SessionManager::with_defaults();
        mgr.get_or_create("conv-c", "nb-1");
        mgr.close_session("conv-c").unwrap();

        let session = mgr.get_session("conv-c").unwrap();
        assert!(!session.active);
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn evicts_closed_sessions_before_active_sessions_at_capacity() {
        let mgr = SessionManager::with_limits(20, 2);
        mgr.get_or_create("closed", "nb-1");
        mgr.get_or_create("active", "nb-1");
        mgr.close_session("closed").unwrap();

        mgr.get_or_create("new", "nb-1");

        assert_eq!(mgr.count(), 2);
        assert!(mgr.get_session("closed").is_none());
        assert!(mgr.get_session("active").is_some());
        assert!(mgr.get_session("new").is_some());
    }

    #[test]
    fn existing_session_is_reused_without_eviction_at_capacity() {
        let mgr = SessionManager::with_limits(20, 1);
        let original = mgr.get_or_create("active", "nb-1");
        let reused = mgr.get_or_create("active", "nb-2");

        assert_eq!(mgr.count(), 1);
        assert_eq!(reused.id, original.id);
        assert_eq!(reused.notebook_id, "nb-1");
    }

    #[test]
    fn conversation_binding_rejects_a_second_notebook() {
        let mgr = SessionManager::with_defaults();
        mgr.get_or_create("bound", "nb-1");

        assert!(mgr.ensure_notebook_binding("bound", "nb-1").is_ok());
        let error = mgr
            .ensure_notebook_binding("bound", "nb-2")
            .expect_err("a conversation must not silently cross notebook boundaries");
        assert!(error
            .to_string()
            .contains("already bound to notebook 'nb-1'"));
    }

    #[test]
    fn bound_session_creation_never_returns_a_different_notebook() {
        let mgr = SessionManager::with_defaults();
        mgr.get_or_create("bound", "nb-1");

        assert!(mgr.get_or_create_bound("bound", "nb-2").is_err());
        assert_eq!(
            mgr.get_or_create_bound("bound", "nb-1")
                .expect("original binding remains usable")
                .notebook_id,
            "nb-1"
        );
    }
}
