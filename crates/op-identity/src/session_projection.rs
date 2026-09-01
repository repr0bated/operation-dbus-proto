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
    /// MutationEngine-authored inline SID1 envelope (`sid1:<base64url>`).
    #[serde(default)]
    pub sealed_id: Option<String>,
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
    #[serde(default)]
    pub catalog_hash_at_arrival: String,
    #[serde(default)]
    pub head_timestamp_at_arrival: i64,
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

    /// Return the exact inline sealed ID the selected sled carries.
    /// Clients forward the base64url portion unchanged; they do not rebuild
    /// identity metadata locally.
    pub fn inline_sealed_id(&self) -> Result<&str, SessionProjectionError> {
        self.sealed_id
            .as_deref()
            .filter(|value| value.starts_with(crate::sealed_id::INLINE_PREFIX))
            .ok_or_else(|| SessionProjectionError::MissingSealedId(self.session_id.clone()))
    }

    /// Validate the selected sled's embedded SID1 envelope and return only its
    /// canonical base64url bytes for direct HTTP metadata injection.  This
    /// reads and forwards the MutationEngine-authored sealed ID; it never rebuilds
    /// claims or hashes a footprint/genesis/hash-chain value.
    pub fn mcp_sealed_id_header(&self) -> Result<&str, SessionProjectionError> {
        let inline = self.inline_sealed_id()?;
        let claims = crate::sealed_id::SealedId::from_inline_ref(inline).map_err(|error| {
            SessionProjectionError::InvalidSealedId {
                session_id: self.session_id.clone(),
                reason: error.to_string(),
            }
        })?;
        let matches_projection = claims.principal_kind == "wireguard-principal"
            && claims
                .transport_scope
                .split(',')
                .any(|scope| scope.trim() == "mcp")
            && claims.principal_id == crate::session::derive_principal_id(&self.wireguard_pubkey)
            && claims.session_id == self.session_id
            && claims.wireguard_pubkey == self.wireguard_pubkey
            && self.genesis.as_deref() == Some(claims.session_genesis.as_str())
            && claims.trace_id == self.trace_id
            && claims.schema_version == self.schema_version
            && claims.expires_at == self.expires_at.unwrap_or(0)
            && claims.issued_at == self.arrival_timestamp
            && claims.arrival_timestamp == self.arrival_timestamp
            && claims.chain_head_at_arrival == self.chain_head_at_arrival
            && claims.catalog_hash_at_arrival == self.catalog_hash_at_arrival
            && claims.head_timestamp_at_arrival == self.head_timestamp_at_arrival;
        if !matches_projection {
            return Err(SessionProjectionError::InvalidSealedId {
                session_id: self.session_id.clone(),
                reason: "sealed claims do not match the selected sled".to_string(),
            });
        }
        inline
            .strip_prefix(crate::sealed_id::INLINE_PREFIX)
            .ok_or_else(|| SessionProjectionError::MissingSealedId(self.session_id.clone()))
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
    #[error("identity session '{0}' has no sealed SID1 identity")]
    MissingSealedId(String),
    #[error("identity session '{session_id}' has an invalid SID1 sealed identity: {reason}")]
    InvalidSealedId { session_id: String, reason: String },
    #[error(
        "identity is ambiguous across {0} current sessions; set OP_IDENTITY_SESSION_ID explicitly"
    )]
    Ambiguous(usize),
}

pub fn read_identity_sessions() -> Result<Vec<SessionIdentity>, SessionProjectionError> {
    let path = op_core::projection_shm::projection_file_path("identity_sled");
    read_identity_sessions_at(path)
}

/// Read the protected credential-bearing identity projection. Only local
/// SID1-aware clients should use this path; generic identity validation and
/// display continue to use [`read_identity_sessions`].
pub fn read_identity_credential_sessions() -> Result<Vec<SessionIdentity>, SessionProjectionError> {
    let path = op_core::projection_shm::credential_projection_file_path("identity_sled");
    read_identity_sessions_at(path)
}

fn read_identity_sessions_at(path: String) -> Result<Vec<SessionIdentity>, SessionProjectionError> {
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
    resolve_from_sessions(read_identity_sessions()?, selector)
}

/// Resolve one selected credential-bearing sled. This fails closed when the
/// private projection is unavailable and never falls back to public or legacy
/// state, where `sealed_id` is intentionally absent.
pub fn resolve_identity_credential_session(
    selector: Option<&str>,
) -> Result<SessionIdentity, SessionProjectionError> {
    resolve_from_sessions(read_identity_credential_sessions()?, selector)
}

fn resolve_from_sessions(
    sessions: Vec<SessionIdentity>,
    selector: Option<&str>,
) -> Result<SessionIdentity, SessionProjectionError> {
    let selector = selector.map(str::trim).filter(|value| !value.is_empty());

    if let Some(selector) = selector {
        // An exact session id is authoritative. Alternate handles are useful
        // to interactive readers, but they must never silently pick the first
        // of multiple PSK-derived sessions sharing a WireGuard public key.
        let mut exact_matches: Vec<_> = sessions
            .iter()
            .filter(|record| record.session_id == selector)
            .cloned()
            .collect();
        let record = if exact_matches.len() == 1 {
            exact_matches.pop().expect("one exact match")
        } else if exact_matches.len() > 1 {
            return Err(SessionProjectionError::Ambiguous(exact_matches.len()));
        } else {
            let mut matches = sessions
                .into_iter()
                .filter(|record| record.matches(selector));
            let first = matches
                .next()
                .ok_or_else(|| SessionProjectionError::NotFound(selector.to_string()))?;
            let extra = matches.count();
            if extra != 0 {
                return Err(SessionProjectionError::Ambiguous(extra + 1));
            }
            first
        };
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
            sealed_id: Some("sid1:test".to_string()),
            schema_version: 3,
            active: true,
            expires_at: None,
            arrival_timestamp: 1,
            chain_head_at_arrival: "ef".repeat(32),
            catalog_hash_at_arrival: "12".repeat(32),
            head_timestamp_at_arrival: 0,
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

    #[test]
    fn duplicate_exact_session_ids_fail_closed() {
        let sessions = vec![anchored("session-a"), anchored("session-a")];
        assert!(matches!(
            resolve_from_sessions(sessions, Some("session-a")),
            Err(SessionProjectionError::Ambiguous(2))
        ));
    }

    #[test]
    fn mcp_header_forwards_exact_sled_sealed_id_and_rejects_projection_drift() {
        let mut record = anchored("session-a");
        let sealed = crate::sealed_id::SealedId {
            principal_id: crate::session::derive_principal_id(&record.wireguard_pubkey),
            principal_kind: "wireguard-principal".into(),
            session_id: record.session_id.clone(),
            wireguard_pubkey: record.wireguard_pubkey.clone(),
            session_genesis: record.genesis.clone().unwrap(),
            trace_id: record.trace_id.clone(),
            schema_version: record.schema_version,
            issued_at: record.arrival_timestamp,
            expires_at: 0,
            arrival_timestamp: record.arrival_timestamp,
            chain_head_at_arrival: record.chain_head_at_arrival.clone(),
            catalog_hash_at_arrival: record.catalog_hash_at_arrival.clone(),
            head_timestamp_at_arrival: record.head_timestamp_at_arrival,
            transport_scope: "dbus,grpc,mcp".into(),
        };
        let inline = sealed.to_inline_ref().unwrap();
        record.sealed_id = Some(inline.clone());
        assert_eq!(
            record.mcp_sealed_id_header().unwrap(),
            inline
                .strip_prefix(crate::sealed_id::INLINE_PREFIX)
                .unwrap()
        );

        record.trace_id.push('0');
        assert!(matches!(
            record.mcp_sealed_id_header(),
            Err(SessionProjectionError::InvalidSealedId { .. })
        ));
    }
}
