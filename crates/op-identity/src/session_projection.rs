//! Read-only access to the authoritative per-session identity projection.
//!
//! Identity is session-scoped. Consumers must either name a session explicitly
//! or run in an environment where exactly one current session is available;
//! they must never infer a process-wide identity from a last-writer-wins file.

use serde::Deserialize;
use thiserror::Error;

/// Environment variable used by host-side clients to select their identity.
pub const SESSION_SELECTOR_ENV: &str = "OP_IDENTITY_SESSION_ID";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SessionIdentity {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub wireguard_pubkey: String,
    #[serde(default)]
    pub mutation_index: u64,
    #[serde(default, alias = "hashed_footprint")]
    pub genesis: Option<String>,
    #[serde(default)]
    pub trace_id: String,
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub arrival_timestamp: i64,
    #[serde(default)]
    pub chain_head_at_arrival: String,
}

impl SessionIdentity {
    /// A record is usable by an identity gate only after genesis was minted
    /// with its durable arrival inputs.
    pub fn is_anchored(&self) -> bool {
        self.genesis
            .as_deref()
            .is_some_and(|value| !value.is_empty())
            && self.arrival_timestamp != 0
            && !self.chain_head_at_arrival.is_empty()
            && !self.trace_id.is_empty()
    }

    pub fn genesis(&self) -> Result<&str, SessionProjectionError> {
        self.genesis
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| SessionProjectionError::Unanchored(self.session_id.clone()))
    }

    /// Whether this session may currently authenticate a new request.
    pub fn is_current_at(&self, now: i64) -> bool {
        self.is_anchored()
            && self.active
            && self
                .expires_at
                .is_none_or(|expires_at| expires_at == 0 || expires_at > now)
    }

    pub fn is_current(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default();
        self.is_current_at(now)
    }

    fn matches(&self, selector: &str) -> bool {
        self.session_id == selector
            || self.wireguard_pubkey == selector
            || self.trace_id == selector
            || self.genesis.as_deref() == Some(selector)
    }
}

#[derive(Debug, Deserialize)]
struct SessionProjection {
    #[serde(default)]
    sleds: Vec<SessionIdentity>,
}

#[derive(Debug, Error)]
pub enum SessionProjectionError {
    #[error("identity session projection is unavailable at {path}: {source}")]
    Unavailable {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("identity session projection is invalid: {0}")]
    Invalid(#[from] serde_json::Error),
    #[error("identity session '{0}' was not found")]
    NotFound(String),
    #[error("identity session '{0}' has no anchored genesis")]
    Unanchored(String),
    #[error("identity session '{0}' is inactive")]
    Inactive(String),
    #[error("identity session '{0}' has expired")]
    Expired(String),
    #[error(
        "identity is ambiguous across {0} current sessions; set OP_IDENTITY_SESSION_ID explicitly"
    )]
    Ambiguous(usize),
}

pub fn read_identity_sessions() -> Result<Vec<SessionIdentity>, SessionProjectionError> {
    let path = op_core::projection_shm::projection_file_path("identity_sled");
    let bytes = std::fs::read(&path).map_err(|source| SessionProjectionError::Unavailable {
        path: path.clone(),
        source,
    })?;
    Ok(serde_json::from_slice::<SessionProjection>(&bytes)?.sleds)
}

/// Resolve a current session by session id, WireGuard key, trace id, or
/// genesis. An empty selector is accepted only when exactly one current
/// session exists, which avoids recreating a process-wide "current identity".
pub fn resolve_identity_session(
    selector: Option<&str>,
) -> Result<SessionIdentity, SessionProjectionError> {
    let sessions = read_identity_sessions()?;
    let selector = selector.map(str::trim).filter(|value| !value.is_empty());

    if let Some(selector) = selector {
        let record = sessions
            .into_iter()
            .find(|record| record.matches(selector))
            .ok_or_else(|| SessionProjectionError::NotFound(selector.to_string()))?;
        if !record.is_anchored() {
            return Err(SessionProjectionError::Unanchored(record.session_id));
        }
        if !record.active {
            return Err(SessionProjectionError::Inactive(record.session_id));
        }
        if !record.is_current() {
            return Err(SessionProjectionError::Expired(record.session_id));
        }
        return Ok(record);
    }

    let mut current = sessions.into_iter().filter(SessionIdentity::is_current);
    let first = current
        .next()
        .ok_or_else(|| SessionProjectionError::NotFound("<current>".to_string()))?;
    let extra = current.count();
    if extra != 0 {
        return Err(SessionProjectionError::Ambiguous(extra + 1));
    }
    Ok(first)
}

/// Resolve the identity selected for a host-side client process.
///
/// `OP_IDENTITY_SESSION_ID` is canonical. `IDENTITY_SLED_HOST_SESSION_ID` is
/// accepted as the existing host-session configuration name during migration.
pub fn configured_identity_session() -> Result<SessionIdentity, SessionProjectionError> {
    let selector = std::env::var(SESSION_SELECTOR_ENV)
        .ok()
        .or_else(|| std::env::var("IDENTITY_SLED_HOST_SESSION_ID").ok());
    resolve_identity_session(selector.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchored(session_id: &str) -> SessionIdentity {
        SessionIdentity {
            session_id: session_id.to_string(),
            wireguard_pubkey: "pubkey".to_string(),
            mutation_index: 1,
            genesis: Some("ab".repeat(32)),
            trace_id: "cd".repeat(16),
            schema_version: 3,
            active: true,
            expires_at: None,
            arrival_timestamp: 1,
            chain_head_at_arrival: "ef".repeat(32),
        }
    }

    #[test]
    fn anchored_requires_mint_inputs() {
        let mut record = anchored("session-a");
        assert!(record.is_anchored());
        record.chain_head_at_arrival.clear();
        assert!(!record.is_anchored());
    }

    #[test]
    fn current_requires_active_unexpired_term() {
        let mut record = anchored("session-a");
        assert!(record.is_current_at(100));
        record.active = false;
        assert!(!record.is_current_at(100));
        record.active = true;
        record.expires_at = Some(100);
        assert!(!record.is_current_at(100));
        record.expires_at = Some(101);
        assert!(record.is_current_at(100));
        record.expires_at = Some(0);
        assert!(record.is_current_at(100));
    }

    #[test]
    fn selector_matches_only_session_handles() {
        let record = anchored("session-a");
        assert!(record.matches("session-a"));
        assert!(record.matches("pubkey"));
        assert!(record.matches(&"cd".repeat(16)));
        assert!(!record.matches("some-other-session"));
    }
}
