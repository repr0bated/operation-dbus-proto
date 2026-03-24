//! Session management using WireGuard pubkey as identity.
//!
//! Sessions are created when a WireGuard peer connects and
//! destroyed on disconnect or timeout.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::gcloud_auth::GCloudAuth;
use crate::wireguard::WireGuardIdentity;

const SESSION_TIMEOUT_SECS: i64 = 3600; // 1 hour

/// Represents an active session
#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: String,
    pub pubkey: String,
    pub user_email: Option<String>,
    pub oauth_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

/// Manages sessions and their lifecycle
#[derive(Clone)]
pub struct SessionManager {
    db: Arc<Mutex<Connection>>,
    gcloud_auth: GCloudAuth,
    wireguard: WireGuardIdentity,
    current_session_id: Arc<Mutex<Option<String>>>,
}

impl SessionManager {
    pub fn new() -> anyhow::Result<Self> {
        Self::with_wireguard_interface("wg0")
    }

    pub fn with_wireguard_interface(interface: &str) -> anyhow::Result<Self> {
        let db_path = Self::db_path()?;

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;

        // Initialize schema
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                pubkey TEXT NOT NULL,
                user_email TEXT,
                oauth_token TEXT,
                token_expires_at INTEGER,
                created_at INTEGER NOT NULL,
                last_seen_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_pubkey ON sessions(pubkey);

            CREATE TABLE IF NOT EXISTS wireguard_users (
                pubkey TEXT PRIMARY KEY,
                user_email TEXT NOT NULL,
                allowed_ip TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
        ",
        )?;

        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            gcloud_auth: GCloudAuth::new(),
            wireguard: WireGuardIdentity::with_interface(interface),
            current_session_id: Arc::new(Mutex::new(None)),
        })
    }

    fn db_path() -> anyhow::Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("identity");
        Ok(data_dir.join("sessions.db"))
    }

    /// Get the GCloud auth provider
    pub fn gcloud_auth(&self) -> &GCloudAuth {
        &self.gcloud_auth
    }

    /// Get the WireGuard identity provider
    pub fn wireguard(&self) -> &WireGuardIdentity {
        &self.wireguard
    }

    /// Create or retrieve session based on WireGuard identity
    pub async fn get_or_create_session_from_wireguard(&self) -> anyhow::Result<Session> {
        let pubkey = self.wireguard.get_local_pubkey()?;
        self.get_or_create_session(&pubkey).await
    }

    /// Get or create a session for a given pubkey
    pub async fn get_or_create_session(&self, pubkey: &str) -> anyhow::Result<Session> {
        let db = self.db.lock().await;
        let now = Utc::now().timestamp();

        // Check for existing valid session
        let existing: Option<Session> = db
            .query_row(
                "SELECT session_id, pubkey, user_email, oauth_token, token_expires_at,
                    created_at, last_seen_at
             FROM sessions
             WHERE pubkey = ? AND last_seen_at > ?",
                params![pubkey, now - SESSION_TIMEOUT_SECS],
                |row| {
                    Ok(Session {
                        session_id: row.get(0)?,
                        pubkey: row.get(1)?,
                        user_email: row.get(2)?,
                        oauth_token: row.get(3)?,
                        token_expires_at: row
                            .get::<_, Option<i64>>(4)?
                            .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                        created_at: DateTime::from_timestamp(row.get::<_, i64>(5)?, 0)
                            .unwrap_or_default(),
                        last_seen_at: DateTime::from_timestamp(row.get::<_, i64>(6)?, 0)
                            .unwrap_or_default(),
                    })
                },
            )
            .ok();

        if let Some(mut session) = existing {
            debug!("Found existing session: {}", session.session_id);

            // Update last_seen
            db.execute(
                "UPDATE sessions SET last_seen_at = ? WHERE session_id = ?",
                params![now, session.session_id],
            )?;
            session.last_seen_at = Utc::now();

            // Store current session ID
            *self.current_session_id.lock().await = Some(session.session_id.clone());

            return Ok(session);
        }

        // Create new session
        let session_id = Uuid::new_v4().to_string();
        info!(
            "Creating new session: {} for pubkey: {}",
            session_id, pubkey
        );

        // Try to get user email from WireGuard user mapping
        let user_email: Option<String> = db
            .query_row(
                "SELECT user_email FROM wireguard_users WHERE pubkey = ?",
                params![pubkey],
                |row| row.get(0),
            )
            .ok();

        // Try to get OAuth token
        drop(db); // Release lock before async call
        let (oauth_token, token_expires_at) = match self.gcloud_auth.get_token().await {
            Ok((token, expires)) => (Some(token), Some(expires)),
            Err(e) => {
                warn!("Could not get OAuth token: {}", e);
                (None, None)
            }
        };

        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO sessions (session_id, pubkey, user_email, oauth_token,
                                   token_expires_at, created_at, last_seen_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                session_id,
                pubkey,
                user_email,
                oauth_token,
                token_expires_at.map(|t| t.timestamp()),
                now,
                now
            ],
        )?;

        let session = Session {
            session_id: session_id.clone(),
            pubkey: pubkey.to_string(),
            user_email,
            oauth_token,
            token_expires_at,
            created_at: Utc::now(),
            last_seen_at: Utc::now(),
        };

        // Store current session ID
        *self.current_session_id.lock().await = Some(session_id);

        Ok(session)
    }

    /// Get the current session ID
    pub async fn current_session_id(&self) -> Option<String> {
        self.current_session_id.lock().await.clone()
    }

    /// Update last_seen timestamp for current session
    pub async fn touch_session(&self) -> anyhow::Result<()> {
        let session_id = self.current_session_id.lock().await.clone();

        if let Some(id) = session_id {
            let db = self.db.lock().await;
            let now = Utc::now().timestamp();

            db.execute(
                "UPDATE sessions SET last_seen_at = ? WHERE session_id = ?",
                params![now, id],
            )?;
        }

        Ok(())
    }

    /// Get a valid OAuth token, refreshing if necessary
    pub async fn get_valid_token(&self) -> anyhow::Result<String> {
        let session_id = self.current_session_id.lock().await.clone();

        if let Some(id) = session_id {
            let db = self.db.lock().await;
            let now = Utc::now().timestamp();

            // Check if we have a valid cached token
            let cached: Option<(String, i64)> = db
                .query_row(
                    "SELECT oauth_token, token_expires_at FROM sessions
                 WHERE session_id = ? AND oauth_token IS NOT NULL",
                    params![id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();

            if let Some((token, expires_at)) = cached {
                // Token valid for at least 5 more minutes
                if expires_at > now + 300 {
                    return Ok(token);
                }
            }

            drop(db); // Release lock before async call
        }

        // Refresh token
        let (token, expires_at) = self.gcloud_auth.get_token().await?;

        // Update in database
        if let Some(id) = self.current_session_id.lock().await.clone() {
            let db = self.db.lock().await;
            db.execute(
                "UPDATE sessions SET oauth_token = ?, token_expires_at = ? WHERE session_id = ?",
                params![token, expires_at.timestamp(), id],
            )?;
        }

        Ok(token)
    }

    /// Register a WireGuard user mapping
    pub async fn register_wireguard_user(
        &self,
        pubkey: &str,
        user_email: &str,
        allowed_ip: &str,
    ) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        let now = Utc::now().timestamp();

        db.execute(
            "INSERT OR REPLACE INTO wireguard_users (pubkey, user_email, allowed_ip, created_at)
             VALUES (?, ?, ?, ?)",
            params![pubkey, user_email, allowed_ip, now],
        )?;

        info!("Registered WireGuard user: {} -> {}", pubkey, user_email);
        Ok(())
    }

    /// Get user email for a pubkey
    pub async fn get_user_for_pubkey(&self, pubkey: &str) -> anyhow::Result<Option<String>> {
        let db = self.db.lock().await;

        let email: Option<String> = db
            .query_row(
                "SELECT user_email FROM wireguard_users WHERE pubkey = ?",
                params![pubkey],
                |row| row.get(0),
            )
            .ok();

        Ok(email)
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> anyhow::Result<usize> {
        let db = self.db.lock().await;
        let cutoff = Utc::now().timestamp() - SESSION_TIMEOUT_SECS;

        let deleted = db.execute(
            "DELETE FROM sessions WHERE last_seen_at < ?",
            params![cutoff],
        )?;

        if deleted > 0 {
            info!("Cleaned up {} expired sessions", deleted);
        }

        Ok(deleted)
    }

    /// Invalidate a specific session
    pub async fn invalidate_session(&self, session_id: &str) -> anyhow::Result<()> {
        let db = self.db.lock().await;

        db.execute(
            "DELETE FROM sessions WHERE session_id = ?",
            params![session_id],
        )?;

        // Clear current session if it matches
        let mut current = self.current_session_id.lock().await;
        if current.as_deref() == Some(session_id) {
            *current = None;
        }

        info!("Invalidated session: {}", session_id);
        Ok(())
    }
}
