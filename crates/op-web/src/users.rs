//! Privacy Router User Storage
//!
//! Manages user accounts for the privacy router VPN service.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use op_identity::generate_magic_link_token;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// A privacy router user account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyUser {
    pub id: String,
    pub email: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub wg_public_key: String,
    pub wg_private_key_encrypted: String,
    pub assigned_ip: String,
    /// Allocated privacy network quota in bytes (default: 1 GiB)
    #[serde(default = "default_privacy_quota_bytes")]
    pub privacy_quota_bytes: u64,
    /// Used privacy network quota in bytes
    #[serde(default)]
    pub privacy_quota_used_bytes: u64,
    /// Provisioned per-user privacy container name (if any)
    #[serde(default)]
    pub privacy_container_name: Option<String>,
    /// Stable published privacy route object identifier
    #[serde(default)]
    pub privacy_route_id: Option<String>,
    /// True when a user container has been provisioned and started
    #[serde(default)]
    pub privacy_network_connected: bool,
    /// Timestamp when user was connected to the privacy network
    #[serde(default)]
    pub privacy_network_connected_at: Option<DateTime<Utc>>,
    /// Google OAuth identity (optional)
    pub google_id: Option<String>,
    pub google_email: Option<String>,
    /// User API credentials for AI services
    pub api_credentials: Option<UserApiCredentials>,
}

/// User-specific API credentials for AI services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserApiCredentials {
    pub gemini_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub preferred_provider: Option<String>,
}

/// A magic link for email verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicLink {
    pub token: String,
    pub user_id: String,
    pub expires_at: DateTime<Utc>,
}

/// User storage with persistence
pub struct UserStore {
    users: RwLock<HashMap<String, PrivacyUser>>,
    users_by_email: RwLock<HashMap<String, String>>, // email -> user_id
    users_by_google_id: RwLock<HashMap<String, String>>, // google_id -> user_id
    magic_links: RwLock<HashMap<String, MagicLink>>,
    next_ip: RwLock<u8>, // Last octet for IP assignment (10.100.0.x)
    storage_path: String,
}

fn default_privacy_quota_bytes() -> u64 {
    1024 * 1024 * 1024 // 1 GiB
}

impl UserStore {
    /// Create a new user store with persistence path
    pub async fn new(storage_path: impl Into<String>) -> Result<Self> {
        let storage_path = storage_path.into();
        let store = Self {
            users: RwLock::new(HashMap::new()),
            users_by_email: RwLock::new(HashMap::new()),
            users_by_google_id: RwLock::new(HashMap::new()),
            magic_links: RwLock::new(HashMap::new()),
            next_ip: RwLock::new(2), // Start at 10.100.0.2
            storage_path,
        };

        // Load existing users if file exists
        store.load().await.ok();

        Ok(store)
    }

    /// Load users from disk
    async fn load(&self) -> Result<()> {
        let path = Path::new(&self.storage_path);
        if !path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(path).await?;
        let mut raw = content.clone();
        let data: StoredData = unsafe { simd_json::from_str(&mut raw) }?;

        let mut users = self.users.write().await;
        let mut users_by_email = self.users_by_email.write().await;
        let mut users_by_google_id = self.users_by_google_id.write().await;
        let mut next_ip = self.next_ip.write().await;

        for user in data.users {
            users_by_email.insert(user.email.clone(), user.id.clone());
            if let Some(ref google_id) = user.google_id {
                users_by_google_id.insert(google_id.clone(), user.id.clone());
            }
            users.insert(user.id.clone(), user);
        }

        *next_ip = data.next_ip;
        info!("Loaded {} users from {}", users.len(), self.storage_path);

        Ok(())
    }

    /// Save users to disk
    async fn save(&self) -> Result<()> {
        let users = self.users.read().await;
        let next_ip = self.next_ip.read().await;

        let data = StoredData {
            users: users.values().cloned().collect(),
            next_ip: *next_ip,
        };

        let content = simd_json::to_string_pretty(&data)?;

        // Ensure parent directory exists
        if let Some(parent) = Path::new(&self.storage_path).parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }

        tokio::fs::write(&self.storage_path, content).await?;
        Ok(())
    }

    /// Get the next available IP address
    pub async fn allocate_ip(&self) -> String {
        let mut next_ip = self.next_ip.write().await;
        let ip = format!("10.100.0.{}/32", *next_ip);
        *next_ip = next_ip.wrapping_add(1);
        if *next_ip < 2 {
            *next_ip = 2; // Wrap around but skip .0 and .1
        }
        ip
    }

    /// Create a new user (unverified)
    pub async fn create_user(
        &self,
        email: &str,
        wg_public_key: String,
        wg_private_key_encrypted: String,
    ) -> Result<PrivacyUser> {
        // Check if email already exists
        {
            let users_by_email = self.users_by_email.read().await;
            if users_by_email.contains_key(email) {
                anyhow::bail!("Email already registered");
            }
        }

        let ip = self.allocate_ip().await;
        let user = PrivacyUser {
            id: uuid::Uuid::new_v4().to_string(),
            email: email.to_string(),
            email_verified: false,
            created_at: Utc::now(),
            wg_public_key,
            wg_private_key_encrypted,
            assigned_ip: ip,
            privacy_quota_bytes: default_privacy_quota_bytes(),
            privacy_quota_used_bytes: 0,
            privacy_container_name: None,
            privacy_route_id: None,
            privacy_network_connected: false,
            privacy_network_connected_at: None,
            google_id: None,
            google_email: None,
            api_credentials: None,
        };

        // Store user
        {
            let mut users = self.users.write().await;
            let mut users_by_email = self.users_by_email.write().await;
            users_by_email.insert(user.email.clone(), user.id.clone());
            users.insert(user.id.clone(), user.clone());
        }

        // Persist
        self.save().await.context("Failed to save user")?;

        info!("Created user {} with IP {}", user.id, user.assigned_ip);
        Ok(user)
    }

    /// Create a magic link for a user
    pub async fn create_magic_link(&self, user_id: &str) -> Result<MagicLink> {
        let token = generate_magic_link_token(32);

        let link = MagicLink {
            token: token.clone(),
            user_id: user_id.to_string(),
            expires_at: Utc::now() + Duration::minutes(15),
        };

        let mut links = self.magic_links.write().await;
        links.insert(token, link.clone());

        Ok(link)
    }

    /// Verify a magic link and mark user as verified
    pub async fn verify_magic_link(&self, token: &str) -> Result<PrivacyUser> {
        // Find and remove the magic link
        let link = {
            let mut links = self.magic_links.write().await;
            links.remove(token).context("Invalid or expired link")?
        };

        // Check expiration
        if link.expires_at < Utc::now() {
            anyhow::bail!("Link has expired");
        }

        // Mark user as verified
        let user = {
            let mut users = self.users.write().await;
            let user = users.get_mut(&link.user_id).context("User not found")?;
            user.email_verified = true;
            user.clone()
        };

        // Persist
        self.save().await?;

        info!("User {} verified via magic link", user.id);
        Ok(user)
    }

    /// Get user by ID
    pub async fn get_user(&self, user_id: &str) -> Option<PrivacyUser> {
        let users = self.users.read().await;
        users.get(user_id).cloned()
    }

    /// Get user by email
    pub async fn get_user_by_email(&self, email: &str) -> Option<PrivacyUser> {
        let users_by_email = self.users_by_email.read().await;
        let user_id = users_by_email.get(email)?;
        let users = self.users.read().await;
        users.get(user_id).cloned()
    }

    /// Get user by Google ID
    pub async fn get_user_by_google_id(&self, google_id: &str) -> Option<PrivacyUser> {
        let users_by_google_id = self.users_by_google_id.read().await;
        let user_id = users_by_google_id.get(google_id)?;
        let users = self.users.read().await;
        users.get(user_id).cloned()
    }

    /// Create or link user with Google identity
    pub async fn create_or_link_google_user(
        &self,
        google_id: &str,
        google_email: &str,
        wg_public_key: String,
        wg_private_key_encrypted: String,
    ) -> Result<PrivacyUser> {
        // Check if user already exists with this Google ID
        if let Some(existing) = self.get_user_by_google_id(google_id).await {
            return Ok(existing);
        }

        // Check if user exists with this email (link Google account)
        if let Some(mut existing) = self.get_user_by_email(google_email).await {
            // Link Google identity to existing user
            existing.google_id = Some(google_id.to_string());
            existing.google_email = Some(google_email.to_string());
            existing.email_verified = true; // Google accounts are pre-verified

            {
                let mut users = self.users.write().await;
                let mut users_by_google_id = self.users_by_google_id.write().await;
                users.insert(existing.id.clone(), existing.clone());
                users_by_google_id.insert(google_id.to_string(), existing.id.clone());
            }

            self.save().await?;
            info!("Linked Google identity to existing user {}", existing.id);
            return Ok(existing);
        }

        // Create new user with Google identity
        let ip = self.allocate_ip().await;
        let user = PrivacyUser {
            id: uuid::Uuid::new_v4().to_string(),
            email: google_email.to_string(),
            email_verified: true, // Google accounts are pre-verified
            created_at: Utc::now(),
            wg_public_key,
            wg_private_key_encrypted,
            assigned_ip: ip,
            privacy_quota_bytes: default_privacy_quota_bytes(),
            privacy_quota_used_bytes: 0,
            privacy_container_name: None,
            privacy_route_id: None,
            privacy_network_connected: false,
            privacy_network_connected_at: None,
            google_id: Some(google_id.to_string()),
            google_email: Some(google_email.to_string()),
            api_credentials: None,
        };

        // Store user
        {
            let mut users = self.users.write().await;
            let mut users_by_email = self.users_by_email.write().await;
            let mut users_by_google_id = self.users_by_google_id.write().await;
            users_by_email.insert(user.email.clone(), user.id.clone());
            users_by_google_id.insert(google_id.to_string(), user.id.clone());
            users.insert(user.id.clone(), user.clone());
        }

        // Persist
        self.save().await.context("Failed to save user")?;

        info!("Created new user {} with Google identity", user.id);
        Ok(user)
    }

    /// Clean up expired magic links
    pub async fn cleanup_expired_links(&self) {
        let mut links = self.magic_links.write().await;
        let now = Utc::now();
        let before = links.len();
        links.retain(|_, v| v.expires_at > now);
        let removed = before - links.len();
        if removed > 0 {
            warn!("Cleaned up {} expired magic links", removed);
        }
    }

    /// Set API credentials for a user
    pub async fn set_user_api_credentials(
        &self,
        user_id: &str,
        credentials: UserApiCredentials,
    ) -> Result<()> {
        let mut users = self.users.write().await;
        if let Some(user) = users.get_mut(user_id) {
            user.api_credentials = Some(credentials);
            self.save().await?;
            info!("Updated API credentials for user {}", user_id);
            Ok(())
        } else {
            anyhow::bail!("User not found: {}", user_id);
        }
    }

    /// Mark a user as connected to the privacy network and persist container mapping.
    pub async fn mark_privacy_network_connected(
        &self,
        user_id: &str,
        container_name: String,
        route_id: String,
    ) -> Result<PrivacyUser> {
        let updated_user = {
            let mut users = self.users.write().await;
            let user = users.get_mut(user_id).context("User not found")?;
            user.privacy_container_name = Some(container_name.clone());
            user.privacy_route_id = Some(route_id.clone());
            user.privacy_network_connected = true;
            user.privacy_network_connected_at = Some(Utc::now());
            user.clone()
        };

        self.save().await?;
        info!(
            "User {} connected to privacy network via container {} route {}",
            user_id, container_name, route_id
        );
        Ok(updated_user)
    }

    /// Get API credentials for a user
    pub async fn get_user_api_credentials(&self, user_id: &str) -> Option<UserApiCredentials> {
        let users = self.users.read().await;
        users.get(user_id)?.api_credentials.clone()
    }

    /// List all users
    pub async fn list_users(&self) -> Vec<PrivacyUser> {
        let users = self.users.read().await;
        users.values().cloned().collect()
    }
}

#[derive(Serialize, Deserialize)]
struct StoredData {
    users: Vec<PrivacyUser>,
    next_ip: u8,
}
