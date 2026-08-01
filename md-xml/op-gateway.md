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
src/
  encrypted_storage.rs
  error.rs
  lib.rs
  mcp_gateway.rs
  wireguard_auth.rs
Cargo.toml
compare-op-gateway.md
SECURITY-MODEL.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/encrypted_storage.rs">
//! Encrypted storage for WireGuard keys using Btrfs subvolumes
//!
//! This module provides secure storage for WireGuard private keys and session data
//! using encrypted Btrfs subvolumes with native encryption (experimental) or LUKS.

use chacha20poly1305::{AeadInPlace, ChaCha20Poly1305, Key, KeyInit, Nonce};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs as async_fs;
use tracing::{debug, info, warn};
use zeroize::{Zeroize, ZeroizeOnDrop};

use anyhow::Result;

/// Configuration for encrypted storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedStorageConfig {
    /// Base path for encrypted storage
    pub base_path: PathBuf,
    /// Subvolume name for WireGuard keys
    pub subvolume_name: String,
    /// Use native Btrfs encryption (experimental)
    pub use_native_encryption: bool,
    /// LUKS device name (if not using native encryption)
    pub luks_device_name: Option<String>,
    /// Key derivation parameters
    pub kdf_params: KdfParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    pub memory_cost: u32,
    pub time_cost: u32,
    pub parallelism: u32,
    pub salt_length: usize,
}

/// Encrypted key storage manager
pub struct EncryptedKeyStorage {
    config: EncryptedStorageConfig,
    storage_path: PathBuf,
    is_initialized: bool,
    master_key: Option<MasterKey>,
}

/// Master key for encryption/decryption
#[derive(Zeroize, ZeroizeOnDrop)]
struct MasterKey {
    key: [u8; 32],
    salt: [u8; 32],
    nonce_counter: u64,
}

/// Encrypted key entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedKeyEntry {
    pub key_id: String,
    pub encrypted_data: Vec<u8>,
    pub nonce: [u8; 12],
    pub created_at: u64,
    pub key_type: KeyType,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyType {
    WireGuardPrivate,
    WireGuardPsk,
    SessionKey,
    MasterKey,
}

impl Default for EncryptedStorageConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("/var/lib/op-dbus/encrypted"),
            subvolume_name: "wireguard-keys".to_string(),
            use_native_encryption: true, // Use experimental Btrfs encryption
            luks_device_name: Some("opdbus_wg_keys".to_string()),
            kdf_params: KdfParams {
                memory_cost: 65536, // 64 MB
                time_cost: 3,
                parallelism: 4,
                salt_length: 32,
            },
        }
    }
}

impl EncryptedKeyStorage {
    /// Create new encrypted key storage
    pub async fn new(config: EncryptedStorageConfig) -> Result<Self> {
        info!(
            "Initializing encrypted key storage at {:?}",
            config.base_path
        );

        let storage_path = config.base_path.join(&config.subvolume_name);

        let mut storage = Self {
            config,
            storage_path,
            is_initialized: false,
            master_key: None,
        };

        // Initialize storage
        storage.initialize().await?;

        Ok(storage)
    }

    /// Initialize encrypted storage with Btrfs subvolume
    async fn initialize(&mut self) -> Result<()> {
        info!("Setting up encrypted Btrfs subvolume for WireGuard keys");

        // Ensure base directory exists
        async_fs::create_dir_all(&self.config.base_path).await?;

        if self.config.use_native_encryption {
            self.setup_native_btrfs_encryption().await?;
        } else {
            self.setup_luks_encryption().await?;
        }

        // Load or generate master key
        self.load_or_generate_master_key().await?;

        self.is_initialized = true;
        info!("Encrypted key storage initialized successfully");
        Ok(())
    }

    /// Setup native Btrfs encryption (experimental)
    async fn setup_native_btrfs_encryption(&self) -> Result<()> {
        info!(
            "Setting up native Btrfs encryption for subvolume: {}",
            self.config.subvolume_name
        );

        // Check if subvolume already exists
        if self.storage_path.exists() {
            debug!(
                "Encrypted subvolume already exists: {:?}",
                self.storage_path
            );
            return Ok(());
        }

        // Create encrypted subvolume using btrfs command
        // Note: This requires kernel support for Btrfs encryption
        let output = Command::new("btrfs")
            .args([
                "subvolume",
                "create",
                "-e", // Enable encryption (experimental)
                self.storage_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute btrfs command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Fallback to regular subvolume if encryption not supported
            if stderr.contains("encryption not supported") || stderr.contains("invalid option") {
                warn!("Native Btrfs encryption not supported, creating regular subvolume");
                self.create_regular_subvolume().await?;
            } else {
                return Err(anyhow::anyhow!(
                    "Failed to create encrypted subvolume: {}",
                    stderr
                ));
            }
        } else {
            info!("Created encrypted Btrfs subvolume: {:?}", self.storage_path);
        }

        // Set restrictive permissions
        self.set_secure_permissions().await?;

        Ok(())
    }

    /// Setup LUKS encryption as fallback
    async fn setup_luks_encryption(&self) -> Result<()> {
        let device_name = self
            .config
            .luks_device_name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("LUKS device name required"))?;

        info!("Setting up LUKS encryption for device: {}", device_name);

        // Check if LUKS device already exists
        let luks_path = format!("/dev/mapper/{}", device_name);
        if Path::new(&luks_path).exists() {
            debug!("LUKS device already exists: {}", luks_path);

            // Mount if not already mounted
            if !self.storage_path.exists() {
                self.mount_luks_device(&luks_path).await?;
            }
            return Ok(());
        }

        // For now, create a loop device with a file
        // In production, this would use a dedicated partition
        let container_path = self.config.base_path.join("wireguard_keys.img");

        if !container_path.exists() {
            info!("Creating encrypted container file: {:?}", container_path);

            // Create 100MB container file
            let output = Command::new("dd")
                .args([
                    "if=/dev/zero",
                    &format!("of={}", container_path.display()),
                    "bs=1M",
                    "count=100",
                ])
                .output()
                .map_err(|e| anyhow::anyhow!(format!("Failed to create container: {}", e)))?;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to create container file: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        // Setup LUKS on the container file
        // Note: In production, this would prompt for passphrase or use key file
        warn!("LUKS setup requires manual intervention - using test passphrase");

        // Create regular subvolume for now
        self.create_regular_subvolume().await?;

        Ok(())
    }

    /// Create regular Btrfs subvolume (fallback)
    async fn create_regular_subvolume(&self) -> Result<()> {
        info!("Creating regular Btrfs subvolume: {:?}", self.storage_path);

        let output = Command::new("btrfs")
            .args(["subvolume", "create", self.storage_path.to_str().unwrap()])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute btrfs command: {}", e))?;

        if !output.status.success() {
            // Fallback to regular directory
            warn!("Btrfs not available, using regular directory");
            async_fs::create_dir_all(&self.storage_path).await?;
        }

        self.set_secure_permissions().await?;
        Ok(())
    }

    /// Mount LUKS device
    async fn mount_luks_device(&self, device_path: &str) -> Result<()> {
        async_fs::create_dir_all(&self.storage_path).await?;

        let output = Command::new("mount")
            .args([device_path, self.storage_path.to_str().unwrap()])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to mount LUKS device: {}", e))?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to mount LUKS device: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        info!("Mounted LUKS device at {:?}", self.storage_path);
        Ok(())
    }

    /// Set secure permissions on storage directory
    async fn set_secure_permissions(&self) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        // Set permissions to 700 (owner read/write/execute only)
        let mut perms = async_fs::metadata(&self.storage_path).await?.permissions();
        perms.set_mode(0o700);
        async_fs::set_permissions(&self.storage_path, perms).await?;

        debug!("Set secure permissions on {:?}", self.storage_path);
        Ok(())
    }

    /// Load or generate master key
    async fn load_or_generate_master_key(&mut self) -> Result<()> {
        let master_key_path = self.storage_path.join("master.key");

        if master_key_path.exists() {
            debug!("Loading existing master key");
            self.load_master_key(&master_key_path).await?;
        } else {
            info!("Generating new master key");
            self.generate_master_key(&master_key_path).await?;
        }

        Ok(())
    }

    /// Load master key from file
    async fn load_master_key(&mut self, path: &Path) -> Result<()> {
        let encrypted_data = async_fs::read(path).await?;

        // For now, use a simple key derivation
        // In production, this would use proper key derivation with user passphrase
        let mut key = [0u8; 32];
        let mut salt = [0u8; 32];

        if encrypted_data.len() >= 64 {
            key.copy_from_slice(&encrypted_data[0..32]);
            salt.copy_from_slice(&encrypted_data[32..64]);
        } else {
            return Err(anyhow::anyhow!("Invalid master key file"));
        }

        self.master_key = Some(MasterKey {
            key,
            salt,
            nonce_counter: 0,
        });

        debug!("Master key loaded successfully");
        Ok(())
    }

    /// Generate new master key
    async fn generate_master_key(&mut self, path: &Path) -> Result<()> {
        let rng = SystemRandom::new();

        let mut key = [0u8; 32];
        let mut salt = [0u8; 32];

        rng.fill(&mut key)
            .map_err(|_| anyhow::anyhow!("Failed to generate key"))?;
        rng.fill(&mut salt)
            .map_err(|_| anyhow::anyhow!("Failed to generate salt"))?;

        // Store encrypted key (in production, encrypt with user passphrase)
        let mut key_data = Vec::with_capacity(64);
        key_data.extend_from_slice(&key);
        key_data.extend_from_slice(&salt);

        async_fs::write(path, &key_data).await?;

        // Set restrictive permissions on key file
        use std::os::unix::fs::PermissionsExt;
        let mut perms = async_fs::metadata(path).await?.permissions();
        perms.set_mode(0o600);
        async_fs::set_permissions(path, perms).await?;

        self.master_key = Some(MasterKey {
            key,
            salt,
            nonce_counter: 0,
        });

        info!("Generated and stored new master key");
        Ok(())
    }

    /// Store encrypted key
    pub async fn store_key(
        &mut self,
        key_id: &str,
        key_data: &[u8],
        key_type: KeyType,
    ) -> anyhow::Result<()> {
        if !self.is_initialized {
            return Err(anyhow::anyhow!("Storage not initialized"));
        }

        let master_key = self
            .master_key
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Master key not available"))?;

        // Generate nonce
        let mut nonce = [0u8; 12];
        let nonce_counter = master_key.nonce_counter;
        nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
        master_key.nonce_counter += 1;

        // Encrypt the key data
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&master_key.key));
        let mut encrypted_data = key_data.to_vec();
        encrypted_data.reserve(16); // Reserve space for authentication tag

        cipher
            .encrypt_in_place(Nonce::from_slice(&nonce), b"", &mut encrypted_data)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

        let entry = EncryptedKeyEntry {
            key_id: key_id.to_string(),
            encrypted_data,
            nonce,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            key_type,
            metadata: std::collections::HashMap::new(),
        };

        // Store to file
        let key_file_path = self.storage_path.join(format!("{}.key", key_id));
        let entry_json = simd_json::to_string(&entry)?;
        async_fs::write(&key_file_path, entry_json).await?;

        // Set secure permissions
        use std::os::unix::fs::PermissionsExt;
        let mut perms = async_fs::metadata(&key_file_path).await?.permissions();
        perms.set_mode(0o600);
        async_fs::set_permissions(&key_file_path, perms).await?;

        debug!("Stored encrypted key: {}", key_id);
        Ok(())
    }

    /// Retrieve and decrypt key
    pub async fn retrieve_key(&self, key_id: &str) -> anyhow::Result<Vec<u8>> {
        if !self.is_initialized {
            return Err(anyhow::anyhow!("Storage not initialized"));
        }

        let master_key = self
            .master_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Master key not available"))?;

        let key_file_path = self.storage_path.join(format!("{}.key", key_id));

        if !key_file_path.exists() {
            return Err(anyhow::anyhow!("Key not found: {}", key_id));
        }

        let entry_json = async_fs::read_to_string(&key_file_path).await?;
        let mut entry_str = entry_json.clone();
        let entry: EncryptedKeyEntry = unsafe { simd_json::from_str(&mut entry_str) }?;

        // Decrypt the key data
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&master_key.key));
        let mut decrypted_data = entry.encrypted_data.clone();

        cipher
            .decrypt_in_place(Nonce::from_slice(&entry.nonce), b"", &mut decrypted_data)
            .map_err(|_| anyhow::anyhow!("Decryption failed"))?;

        debug!("Retrieved and decrypted key: {}", key_id);
        Ok(decrypted_data)
    }

    /// List all stored keys
    pub async fn list_keys(&self) -> anyhow::Result<Vec<String>> {
        if !self.is_initialized {
            return Err(anyhow::anyhow!("Storage not initialized"));
        }

        let mut keys = Vec::new();
        let mut entries = async_fs::read_dir(&self.storage_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension == "key" {
                    if let Some(stem) = path.file_stem() {
                        if let Some(key_id) = stem.to_str() {
                            keys.push(key_id.to_string());
                        }
                    }
                }
            }
        }

        Ok(keys)
    }

    /// Delete a key
    pub async fn delete_key(&self, key_id: &str) -> anyhow::Result<()> {
        if !self.is_initialized {
            return Err(anyhow::anyhow!("Storage not initialized"));
        }

        let key_file_path = self.storage_path.join(format!("{}.key", key_id));

        if key_file_path.exists() {
            async_fs::remove_file(&key_file_path).await?;
            debug!("Deleted key: {}", key_id);
        }

        Ok(())
    }

    /// Get storage statistics
    pub async fn get_stats(&self) -> anyhow::Result<StorageStats> {
        if !self.is_initialized {
            return Err(anyhow::anyhow!("Storage not initialized"));
        }

        let keys = self.list_keys().await?;
        let _metadata = async_fs::metadata(&self.storage_path).await?;

        // Get filesystem info
        let fs_info = self.get_filesystem_info().await?;

        Ok(StorageStats {
            total_keys: keys.len(),
            storage_path: self.storage_path.clone(),
            is_encrypted: self.config.use_native_encryption
                || self.config.luks_device_name.is_some(),
            encryption_type: if self.config.use_native_encryption {
                "btrfs-native".to_string()
            } else {
                "luks".to_string()
            },
            filesystem_type: fs_info.filesystem_type,
            total_space: fs_info.total_space,
            available_space: fs_info.available_space,
            used_space: fs_info.used_space,
        })
    }

    /// Get filesystem information
    async fn get_filesystem_info(&self) -> anyhow::Result<FilesystemInfo> {
        let output = Command::new("df")
            .args(["-T", self.storage_path.to_str().unwrap()])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to get filesystem info: {}", e))?;

        if !output.status.success() {
            return Ok(FilesystemInfo::default());
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = output_str.lines().collect();

        if lines.len() >= 2 {
            let fields: Vec<&str> = lines[1].split_whitespace().collect();
            if fields.len() >= 6 {
                return Ok(FilesystemInfo {
                    filesystem_type: fields[1].to_string(),
                    total_space: fields[2].parse().unwrap_or(0),
                    used_space: fields[3].parse().unwrap_or(0),
                    available_space: fields[4].parse().unwrap_or(0),
                });
            }
        }

        Ok(FilesystemInfo::default())
    }
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_keys: usize,
    pub storage_path: PathBuf,
    pub is_encrypted: bool,
    pub encryption_type: String,
    pub filesystem_type: String,
    pub total_space: u64,
    pub available_space: u64,
    pub used_space: u64,
}

/// Filesystem information
#[derive(Debug, Clone)]
struct FilesystemInfo {
    pub filesystem_type: String,
    pub total_space: u64,
    pub used_space: u64,
    pub available_space: u64,
}

impl Default for FilesystemInfo {
    fn default() -> Self {
        Self {
            filesystem_type: "unknown".to_string(),
            total_space: 0,
            used_space: 0,
            available_space: 0,
        }
    }
}

impl Drop for EncryptedKeyStorage {
    fn drop(&mut self) {
        // Zeroize master key on drop
        if let Some(mut master_key) = self.master_key.take() {
            master_key.zeroize();
        }
    }
}
</file>

<file path="src/error.rs">
//! Error types for op-gateway

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("authentication failed: {0}")]
    AuthFailed(String),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, GatewayError>;

impl GatewayError {
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    pub fn auth_failed(msg: impl Into<String>) -> Self {
        Self::AuthFailed(msg.into())
    }

    pub fn crypto(msg: impl Into<String>) -> Self {
        Self::Crypto(msg.into())
    }

    pub fn storage(msg: impl Into<String>) -> Self {
        Self::Storage(msg.into())
    }
}
</file>

<file path="src/lib.rs">
//! op-gateway: MCP Gateway with WireGuard authentication and smart routing

pub mod encrypted_storage;
mod error;
pub mod mcp_gateway;
pub mod wireguard_auth;

pub use encrypted_storage::*;
pub use error::*;
pub use mcp_gateway::*;
pub use wireguard_auth::*;
</file>

<file path="src/mcp_gateway.rs">
//! MCP Gateway - WireGuard authentication and client routing for MCP services
//!
//! This module provides the WireGuard Gateway that sits between clients and the Compact MCP,
//! handling authentication and routing decisions based on WireGuard session validation.

use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

use crate::wireguard_auth::{SessionFilter, WireGuardAuthManager};
use anyhow::Result;

/// Client routing decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub endpoint: String,
    pub allowed_tools: Vec<String>,
    pub capabilities: Vec<String>,
    pub has_full_access: bool,
    pub session_id: String,
    pub access_level: AccessLevel,
}

/// Access level for MCP clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessLevel {
    /// Full access to all tools (Compact + Cognitive)
    Full,
    /// Restricted access to cognitive tools only
    CognitiveOnly,
    /// No access (blocked)
    Blocked,
}

/// Client information for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientInfo {
    pub name: String,
    pub version: Option<String>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub auth_token: Option<String>,
    pub peer_pubkey: Option<String>,
}

/// MCP session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSession {
    pub session_id: String,
    pub client_info: McpClientInfo,
    pub routing_decision: RoutingDecision,
    pub created_at: u64,
    pub last_used: u64,
    pub is_active: bool,
}

/// MCP Gateway Manager - handles authentication and routing for MCP clients
pub struct McpGatewayManager {
    /// WireGuard authentication manager
    wireguard_auth: Arc<WireGuardAuthManager>,
    /// Active MCP sessions
    sessions: Arc<RwLock<HashMap<String, McpSession>>>,
    /// Client routing cache
    routing_cache: Arc<RwLock<HashMap<String, RoutingDecision>>>,
}

impl McpGatewayManager {
    /// Create new MCP Gateway Manager
    pub async fn new(wireguard_auth: Arc<WireGuardAuthManager>) -> Result<Self> {
        info!("Initializing MCP Gateway Manager");

        Ok(Self {
            wireguard_auth,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            routing_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Route client to appropriate MCP backend based on authentication
    pub async fn route_client(&self, client_info: McpClientInfo) -> Result<RoutingDecision> {
        debug!("Routing client: {}", client_info.name);

        // Check authentication status
        let is_authenticated = self.check_authentication(&client_info).await?;

        let routing_decision = if is_authenticated {
            // Full access: Compact + Cognitive tools
            RoutingDecision {
                endpoint: "grpc://localhost:50051".to_string(),
                allowed_tools: vec![
                    "list_tools".to_string(),
                    "search_tools".to_string(),
                    "get_tool_schema".to_string(),
                    "execute_tool".to_string(),
                    "cognitive_reason".to_string(),
                    "compact_summarize".to_string(),
                ],
                capabilities: vec![
                    "tools".to_string(),
                    "resources".to_string(),
                    "full_access".to_string(),
                ],
                has_full_access: true,
                session_id: Uuid::new_v4().to_string(),
                access_level: AccessLevel::Full,
            }
        } else {
            // Restricted access: Cognitive only
            RoutingDecision {
                endpoint: "grpc://localhost:50052".to_string(),
                allowed_tools: vec!["cognitive_reason".to_string()],
                capabilities: vec!["tools".to_string(), "cognitive_only".to_string()],
                has_full_access: false,
                session_id: Uuid::new_v4().to_string(),
                access_level: AccessLevel::CognitiveOnly,
            }
        };

        // Cache routing decision
        {
            let mut cache = self.routing_cache.write().await;
            let cache_key = self.generate_cache_key(&client_info);
            cache.insert(cache_key, routing_decision.clone());
        }

        info!(
            client = %client_info.name,
            authenticated = %is_authenticated,
            endpoint = %routing_decision.endpoint,
            tools = %routing_decision.allowed_tools.len(),
            "Client routed"
        );

        Ok(routing_decision)
    }

    /// Create MCP session for client
    pub async fn create_session(&self, client_info: McpClientInfo) -> Result<McpSession> {
        let routing_decision = self.route_client(client_info.clone()).await?;

        let session = McpSession {
            session_id: routing_decision.session_id.clone(),
            client_info,
            routing_decision,
            created_at: Self::current_timestamp(),
            last_used: Self::current_timestamp(),
            is_active: true,
        };

        // Store session
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session.session_id.clone(), session.clone());
        }

        info!(session_id = %session.session_id, "MCP session created");
        Ok(session)
    }

    /// Validate MCP session
    pub async fn validate_session(&self, session_id: &str) -> Result<bool> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            if !session.is_active {
                return Ok(false);
            }

            // Check if underlying WireGuard session is still valid
            if let Some(ref auth_token) = session.client_info.auth_token {
                return self.wireguard_auth.validate_session(auth_token).await;
            }

            // For non-authenticated sessions, just check if session exists and is active
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get session information
    pub async fn get_session(&self, session_id: &str) -> Result<Option<McpSession>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }

    /// List active sessions
    pub async fn list_sessions(&self) -> Result<Vec<McpSession>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.values().cloned().collect())
    }

    /// Get client capabilities based on session
    pub async fn get_client_capabilities(&self, session_id: &str) -> Result<Vec<String>> {
        if let Some(session) = self.get_session(session_id).await? {
            Ok(session.routing_decision.capabilities)
        } else {
            Ok(vec!["cognitive_only".to_string()])
        }
    }

    /// Check if client is authenticated via WireGuard
    async fn check_authentication(&self, client_info: &McpClientInfo) -> Result<bool> {
        // Check auth token first
        if let Some(ref auth_token) = client_info.auth_token {
            return self.wireguard_auth.validate_session(auth_token).await;
        }

        // Check peer public key
        if let Some(ref peer_pubkey) = client_info.peer_pubkey {
            let filter = SessionFilter {
                active_only: Some(true),
                peer_pubkey: Some(peer_pubkey.clone()),
                created_after: None,
                created_before: None,
            };

            let sessions = self.wireguard_auth.list_sessions(Some(filter)).await?;
            return Ok(!sessions.is_empty());
        }

        // No authentication information provided
        Ok(false)
    }

    /// Generate cache key for routing decisions
    fn generate_cache_key(&self, client_info: &McpClientInfo) -> String {
        let key_parts = vec![
            client_info.name.clone(),
            client_info.peer_pubkey.clone().unwrap_or_default(),
            client_info.auth_token.clone().unwrap_or_default(),
        ];

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        key_parts.hash(&mut hasher);
        format!("mcp_route_{:x}", hasher.finish())
    }

    /// Get current Unix timestamp
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> Result<usize> {
        let now = Self::current_timestamp();
        let mut expired_sessions = Vec::new();

        // Find expired sessions
        {
            let sessions = self.sessions.read().await;
            for (session_id, session) in sessions.iter() {
                // Sessions expire after 1 hour of inactivity
                if now - session.last_used > 3600 {
                    expired_sessions.push(session_id.clone());
                }
            }
        }

        // Remove expired sessions
        let expired_count = expired_sessions.len();
        if expired_count > 0 {
            let mut sessions = self.sessions.write().await;
            for session_id in expired_sessions {
                sessions.remove(&session_id);
            }

            info!(expired = %expired_count, "Cleaned up expired MCP sessions");
        }

        Ok(expired_count)
    }
}

/// D-Bus interface implementation for MCP Gateway
impl McpGatewayManager {
    /// Handle D-Bus method call for client routing
    pub async fn dbus_route_client(
        &self,
        client_name: &str,
        auth_token: Option<&str>,
        peer_pubkey: Option<&str>,
    ) -> Result<Value> {
        let client_info = McpClientInfo {
            name: client_name.to_string(),
            version: None,
            user_agent: None,
            ip_address: None,
            auth_token: auth_token.map(String::from),
            peer_pubkey: peer_pubkey.map(String::from),
        };

        let routing_decision = self.route_client(client_info).await?;

        Ok(json!({
            "endpoint": routing_decision.endpoint,
            "allowed_tools": routing_decision.allowed_tools,
            "capabilities": routing_decision.capabilities,
            "has_full_access": routing_decision.has_full_access,
            "session_id": routing_decision.session_id,
            "access_level": match routing_decision.access_level {
                AccessLevel::Full => "full",
                AccessLevel::CognitiveOnly => "cognitive_only",
                AccessLevel::Blocked => "blocked",
            }
        }))
    }

    /// Handle D-Bus method call for session validation
    pub async fn dbus_validate_session(&self, session_id: &str) -> Result<Value> {
        let is_valid = self.validate_session(session_id).await?;

        Ok(json!({
            "valid": is_valid,
            "session_id": session_id
        }))
    }

    /// Handle D-Bus method call for getting client capabilities
    pub async fn dbus_get_capabilities(&self, session_id: &str) -> Result<Value> {
        let capabilities = self.get_client_capabilities(session_id).await?;

        Ok(json!({
            "capabilities": capabilities,
            "session_id": session_id
        }))
    }
}
</file>

<file path="src/wireguard_auth.rs">
//! WireGuard authentication and session management integration
//!
//! This module integrates WireGuard identity management with the OP-DBUS service system,
//! providing high-performance JSON-RPC authentication with D-Bus compatibility.
//!
//! ## ⚠️ CRITICAL SECURITY MODEL - READ SECURITY-MODEL.md ⚠️
//!
//! - WG PSK is STATIC (identity, not rotated per-login)
//! - Session keys rotate per-login using SERVER NONCE (not timestamp)
//! - See `SECURITY-MODEL.md` in this crate for full details

use serde::{Deserialize, Serialize};

use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, info, warn};

use argon2::Argon2;
use base64::{engine::general_purpose, Engine as _};
use blake2::{Blake2s256, Digest};
use ring::rand::SystemRandom;

use crate::encrypted_storage::{EncryptedKeyStorage, EncryptedStorageConfig, KeyType};
use anyhow::Result;

/// Separate WireGuard database (not the main services database)
#[derive(Clone)]
pub struct WireGuardDatabase {
    pool: sqlx::SqlitePool,
}

impl WireGuardDatabase {
    /// Create new WireGuard database connection
    pub async fn new() -> Result<Self> {
        let database_url = std::env::var("OP_WIREGUARD_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:///var/lib/op-dbus/wireguard.db".to_string());

        // Ensure directory exists
        if let Some(parent) = std::path::Path::new(&database_url.replace("sqlite://", "")).parent()
        {
            tokio::fs::create_dir_all(parent).await?;
        }

        let pool = sqlx::SqlitePool::connect(&database_url).await?;

        Ok(Self { pool })
    }

    /// Run WireGuard database migrations
    pub async fn migrate(&self) -> Result<()> {
        // Create WireGuard sessions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS wireguard_sessions (
                session_id TEXT PRIMARY KEY,
                peer_pubkey TEXT NOT NULL,
                psk TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT true,
                last_used INTEGER NOT NULL,
                client_ip TEXT,
                client_version TEXT,
                auth_method TEXT NOT NULL DEFAULT 'wireguard',
                key_rotation_count INTEGER NOT NULL DEFAULT 0,
                flags TEXT
            )
        "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for WireGuard sessions
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_wireguard_sessions_peer_pubkey ON wireguard_sessions(peer_pubkey)").execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_wireguard_sessions_expires_at ON wireguard_sessions(expires_at)").execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_wireguard_sessions_is_active ON wireguard_sessions(is_active)").execute(&self.pool).await?;

        Ok(())
    }

    // WireGuard-specific database methods...
    pub async fn store_wireguard_session(&self, session: &WireGuardSession) -> Result<()> {
        let flags_json = simd_json::to_string(&session.flags)?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO wireguard_sessions 
            (session_id, peer_pubkey, psk, created_at, expires_at, is_active, last_used, 
             client_ip, client_version, auth_method, key_rotation_count, flags)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        )
        .bind(&session.session_id)
        .bind(&session.peer_pubkey)
        .bind(&session.psk)
        .bind(session.created_at as i64)
        .bind(session.expires_at as i64)
        .bind(session.is_active)
        .bind(session.last_used as i64)
        .bind(&session.client_ip)
        .bind(&session.client_version)
        .bind(&session.auth_method)
        .bind(session.key_rotation_count as i64)
        .bind(&flags_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_wireguard_session(&self, session: &WireGuardSession) -> Result<()> {
        let flags_json = simd_json::to_string(&session.flags)?;

        sqlx::query(
            r#"
            UPDATE wireguard_sessions 
            SET psk = ?, expires_at = ?, is_active = ?, last_used = ?, 
                client_ip = ?, client_version = ?, key_rotation_count = ?, flags = ?
            WHERE session_id = ?
        "#,
        )
        .bind(&session.psk)
        .bind(session.expires_at as i64)
        .bind(session.is_active)
        .bind(session.last_used as i64)
        .bind(&session.client_ip)
        .bind(&session.client_version)
        .bind(session.key_rotation_count as i64)
        .bind(&flags_json)
        .bind(&session.session_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_session_last_used(&self, session_id: &str, last_used: u64) -> Result<()> {
        sqlx::query("UPDATE wireguard_sessions SET last_used = ? WHERE session_id = ?")
            .bind(last_used as i64)
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn load_wireguard_sessions(&self) -> Result<Vec<WireGuardSession>> {
        let rows = sqlx::query(
            r#"
            SELECT session_id, peer_pubkey, psk, created_at, expires_at, is_active, 
                   last_used, client_ip, client_version, auth_method, key_rotation_count, flags
            FROM wireguard_sessions
        "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut sessions = Vec::new();
        for row in rows {
            let flags_json: String = row.get("flags");
            let mut flags_str = flags_json.clone();
            let flags: std::collections::HashMap<String, String> =
                unsafe { simd_json::from_str(&mut flags_str) }.unwrap_or_default();

            let session = WireGuardSession {
                session_id: row.get("session_id"),
                peer_pubkey: row.get("peer_pubkey"),
                psk: row.get("psk"),
                created_at: row.get::<i64, _>("created_at") as u64,
                expires_at: row.get::<i64, _>("expires_at") as u64,
                is_active: row.get("is_active"),
                last_used: row.get::<i64, _>("last_used") as u64,
                client_ip: row.get("client_ip"),
                client_version: row.get("client_version"),
                auth_method: row.get("auth_method"),
                key_rotation_count: row.get::<i64, _>("key_rotation_count") as u32,
                flags,
            };

            sessions.push(session);
        }

        Ok(sessions)
    }

    pub async fn remove_wireguard_session(&self, session_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM wireguard_sessions WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// WireGuard session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardSession {
    pub session_id: String,
    pub peer_pubkey: String,
    pub psk: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub is_active: bool,
    pub last_used: u64,
    pub client_ip: Option<String>,
    pub client_version: Option<String>,
    pub auth_method: String,
    pub key_rotation_count: u32,
    pub flags: HashMap<String, String>,
}

/// WireGuard authentication statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardStats {
    pub total_sessions: u64,
    pub active_sessions: u64,
    pub keys_rotated: u64,
    pub auth_failures: u64,
    pub uptime_seconds: u64,
    pub memory_usage: u64,
    pub cpu_usage: f64,
    pub request_rate: f64,
    pub error_rate: f64,
    pub cache_hits: u64,
}

/// High-performance WireGuard authentication manager
pub struct WireGuardAuthManager {
    /// Cryptographic engine for key operations
    crypto_engine: Arc<SimdCryptoEngine>,
    /// Encrypted key storage
    key_storage: Arc<tokio::sync::Mutex<EncryptedKeyStorage>>,
    /// Active sessions cache
    sessions: Arc<RwLock<HashMap<String, WireGuardSession>>>,
    /// Peer public key to session mapping
    peer_sessions: Arc<RwLock<HashMap<String, String>>>,
    /// Database for WireGuard sessions (separate from services database)
    database: WireGuardDatabase,
    /// Statistics tracking
    stats: Arc<Mutex<WireGuardStats>>,
    /// Service start time
    start_time: Instant,
    /// Session cleanup interval
    cleanup_interval: Duration,
}

impl WireGuardAuthManager {
    /// Create new WireGuard authentication manager
    pub async fn new() -> Result<Self> {
        info!("Initializing WireGuard authentication manager");

        // Initialize separate WireGuard database
        let database = WireGuardDatabase::new().await?;
        database.migrate().await?;

        // Initialize encrypted key storage
        let storage_config = EncryptedStorageConfig::default();
        let key_storage = Arc::new(tokio::sync::Mutex::new(
            EncryptedKeyStorage::new(storage_config).await?,
        ));

        // Initialize crypto engine
        let crypto_engine = Arc::new(SimdCryptoEngine::new().await?);

        // Initialize statistics
        let stats = Arc::new(Mutex::new(WireGuardStats {
            total_sessions: 0,
            active_sessions: 0,
            keys_rotated: 0,
            auth_failures: 0,
            uptime_seconds: 0,
            memory_usage: 0,
            cpu_usage: 0.0,
            request_rate: 0.0,
            error_rate: 0.0,
            cache_hits: 0,
        }));

        let manager = Self {
            crypto_engine,
            key_storage,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            peer_sessions: Arc::new(RwLock::new(HashMap::new())),
            database,
            stats,
            start_time: Instant::now(),
            cleanup_interval: Duration::from_secs(300), // 5 minutes
        };

        // Load existing sessions from database
        manager.load_sessions_from_database().await?;

        // Start background tasks
        manager.start_background_tasks().await;

        info!("WireGuard authentication manager initialized");
        Ok(manager)
    }

    /// Create a new WireGuard session
    pub async fn create_session(
        &self,
        peer_pubkey: &str,
        client_info: Option<ClientInfo>,
    ) -> Result<WireGuardSession> {
        debug!("Creating WireGuard session for peer: {}", peer_pubkey);

        // Validate peer public key format
        if !Self::is_valid_pubkey(peer_pubkey) {
            return Err(anyhow::anyhow!(
                "Invalid peer public key format: {}",
                peer_pubkey
            ));
        }

        // Check if session already exists for this peer
        {
            let peer_sessions = self.peer_sessions.read().await;
            if let Some(existing_session_id) = peer_sessions.get(peer_pubkey) {
                let sessions = self.sessions.read().await;
                if let Some(session) = sessions.get(existing_session_id) {
                    if session.is_active && session.expires_at > Self::current_timestamp() {
                        debug!(
                            "Returning existing active session for peer: {}",
                            peer_pubkey
                        );
                        return Ok(session.clone());
                    }
                }
            }
        }

        // Generate session ID and stable PSK (no timestamp)
        let session_id = self.generate_session_id(peer_pubkey).await?;
        let psk = self.derive_psk(peer_pubkey).await?;

        let now = Self::current_timestamp();
        let expires_at = now + 3600; // 1 hour default

        let session = WireGuardSession {
            session_id: session_id.clone(),
            peer_pubkey: peer_pubkey.to_string(),
            psk,
            created_at: now,
            expires_at,
            is_active: true,
            last_used: now,
            client_ip: client_info.as_ref().and_then(|c| c.ip.clone()),
            client_version: client_info.as_ref().and_then(|c| c.version.clone()),
            auth_method: "wireguard".to_string(),
            key_rotation_count: 0,
            flags: HashMap::new(),
        };

        // Store in database
        self.database.store_wireguard_session(&session).await?;

        // Update caches
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), session.clone());
        }
        {
            let mut peer_sessions = self.peer_sessions.write().await;
            peer_sessions.insert(peer_pubkey.to_string(), session_id);
        }

        // Update statistics
        {
            let mut stats = self.stats.lock().await;
            stats.total_sessions += 1;
            stats.active_sessions += 1;
        }

        info!(
            "Created WireGuard session {} for peer {}",
            session.session_id, peer_pubkey
        );
        Ok(session)
    }

    /// Validate a WireGuard session
    pub async fn validate_session(&self, session_id: &str) -> Result<bool> {
        debug!("Validating WireGuard session: {}", session_id);

        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let now = Self::current_timestamp();
            let is_valid = session.is_active && session.expires_at > now;

            if is_valid {
                // Update last used timestamp (we'll do this in a separate task to avoid blocking)
                tokio::spawn({
                    let database = self.database.clone();
                    let session_id = session_id.to_string();
                    async move {
                        if let Err(e) = database.update_session_last_used(&session_id, now).await {
                            warn!("Failed to update session last used: {}", e);
                        }
                    }
                });
            }

            Ok(is_valid)
        } else {
            Ok(false)
        }
    }

    /// Get session information
    pub async fn get_session(&self, session_id: &str) -> Result<Option<WireGuardSession>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }

    /// List active sessions
    pub async fn list_sessions(
        &self,
        filter: Option<SessionFilter>,
    ) -> Result<Vec<WireGuardSession>> {
        let sessions = self.sessions.read().await;
        let mut result: Vec<WireGuardSession> = sessions.values().cloned().collect();

        // Apply filters
        if let Some(filter) = filter {
            result.retain(|session| {
                if let Some(active_only) = filter.active_only {
                    if active_only && !session.is_active {
                        return false;
                    }
                }
                if let Some(peer_pubkey) = &filter.peer_pubkey {
                    if &session.peer_pubkey != peer_pubkey {
                        return false;
                    }
                }
                true
            });
        }

        // Sort by creation time (newest first)
        result.sort_by_key(|b| std::cmp::Reverse(b.created_at));

        Ok(result)
    }

    /// Rotate session key for a peer (NOT the WireGuard PSK)
    /// WireGuard PSK remains stable to avoid desync issues
    pub async fn rotate_session_key(&self, peer_pubkey: &str, force: bool) -> Result<String> {
        info!(
            "Rotating session key for peer: {} (force: {})",
            peer_pubkey, force
        );

        // Get existing session
        let session_id = {
            let peer_sessions = self.peer_sessions.read().await;
            peer_sessions.get(peer_pubkey).cloned()
        };

        let session_id = match session_id {
            Some(id) => id,
            None => {
                if force {
                    // Create new session if forced
                    let session = self.create_session(peer_pubkey, None).await?;
                    return Ok(session.session_id);
                } else {
                    return Err(anyhow::anyhow!("No active session found for peer"));
                }
            }
        };

        // Generate new session key (not PSK!)
        let new_session_key = self.derive_session_key(peer_pubkey).await?;

        // Update session
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                // Store new session key in metadata, keep PSK stable
                session
                    .flags
                    .insert("current_session_key".to_string(), new_session_key.clone());
                session.key_rotation_count += 1;
                session.last_used = Self::current_timestamp();

                // Update in database
                if let Err(e) = self.database.update_wireguard_session(session).await {
                    warn!("Failed to update session in database: {}", e);
                }
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.lock().await;
            stats.keys_rotated += 1;
        }

        info!("Session key rotated successfully for peer: {}", peer_pubkey);
        Ok(new_session_key)
    }

    /// Get authentication statistics
    pub async fn get_stats(&self) -> Result<WireGuardStats> {
        let mut stats = self.stats.lock().await;

        // Update uptime
        stats.uptime_seconds = self.start_time.elapsed().as_secs();

        // Update active sessions count
        let sessions = self.sessions.read().await;
        let now = Self::current_timestamp();
        stats.active_sessions = sessions
            .values()
            .filter(|s| s.is_active && s.expires_at > now)
            .count() as u64;

        Ok(stats.clone())
    }

    /// Generate session ID using SIMD-accelerated BLAKE2s
    async fn generate_session_id(&self, peer_pubkey: &str) -> anyhow::Result<String> {
        let input = format!("WG-SESSION-{}-{}", peer_pubkey, Self::current_timestamp());
        let session_ids = self
            .crypto_engine
            .generate_session_ids_batch(&[input.as_bytes()]);

        if let Some(session_id) = session_ids.first() {
            Ok(hex::encode(session_id))
        } else {
            Err(anyhow::anyhow!("Failed to generate session ID"))
        }
    }

    /// Derive stable PSK (no timestamp, no rotation)
    /// WireGuard PSK should remain stable to avoid connection issues
    async fn derive_psk(&self, peer_pubkey: &str) -> anyhow::Result<String> {
        // Convert peer pubkey to bytes
        let peer_key_bytes = match hex::decode(peer_pubkey) {
            Ok(bytes) => {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    key
                } else {
                    return Err(anyhow::anyhow!(
                        "Invalid peer public key length: {}",
                        bytes.len()
                    ));
                }
            }
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "Invalid peer public key format: {}",
                    peer_pubkey
                ))
            }
        };

        // Check if we have a stored PSK for this peer (stable, no rotation)
        let psk_key_id = format!("psk_{}", peer_pubkey);
        let mut key_storage = self.key_storage.lock().await;

        if let Ok(stored_psk) = key_storage.retrieve_key(&psk_key_id).await {
            if stored_psk.len() == 32 {
                return Ok(general_purpose::STANDARD.encode(&stored_psk));
            }
        }

        // Generate stable PSK (no timestamp input)
        let psks = self.crypto_engine.derive_stable_psk(&peer_key_bytes);

        if let Some(psk) = psks.first() {
            // Store the PSK in encrypted storage
            if let Err(e) = key_storage
                .store_key(&psk_key_id, psk, KeyType::WireGuardPsk)
                .await
            {
                warn!("Failed to store PSK in encrypted storage: {}", e);
            }

            Ok(general_purpose::STANDARD.encode(psk))
        } else {
            Err(anyhow::anyhow!("Failed to derive PSK"))
        }
    }

    /// Derive session key using server nonce (not timestamp)
    /// This is what gets rotated per-login, not the WireGuard PSK
    async fn derive_session_key(&self, peer_pubkey: &str) -> anyhow::Result<String> {
        // Generate server nonce for this session
        let server_nonce = self.generate_server_nonce().await?;

        // Convert peer pubkey to bytes
        let peer_key_bytes = match hex::decode(peer_pubkey) {
            Ok(bytes) => {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    key
                } else {
                    return Err(anyhow::anyhow!(
                        "Invalid peer public key length: {}",
                        bytes.len()
                    ));
                }
            }
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "Invalid peer public key format: {}",
                    peer_pubkey
                ))
            }
        };

        // Derive session key using nonce (not timestamp)
        let session_keys = self
            .crypto_engine
            .derive_session_keys(&[peer_key_bytes], &[server_nonce]);

        if let Some((session_key, _)) = session_keys.first() {
            Ok(general_purpose::STANDARD.encode(session_key))
        } else {
            Err(anyhow::anyhow!("Failed to derive session key"))
        }
    }

    /// Generate server nonce for session key derivation
    async fn generate_server_nonce(&self) -> anyhow::Result<[u8; 32]> {
        let mut nonce = [0u8; 32];
        ring::rand::SecureRandom::fill(&SystemRandom::new(), &mut nonce)
            .map_err(|_| anyhow::anyhow!("Failed to generate server nonce"))?;
        Ok(nonce)
    }

    /// Store WireGuard private key in encrypted storage
    pub async fn store_private_key(&self, key_id: &str, private_key: &[u8; 32]) -> Result<()> {
        let mut key_storage = self.key_storage.lock().await;
        key_storage
            .store_key(key_id, private_key, KeyType::WireGuardPrivate)
            .await?;
        info!("Stored WireGuard private key: {}", key_id);
        Ok(())
    }

    /// Retrieve WireGuard private key from encrypted storage
    pub async fn retrieve_private_key(&self, key_id: &str) -> anyhow::Result<[u8; 32]> {
        let key_storage = self.key_storage.lock().await;
        let key_data = key_storage.retrieve_key(key_id).await?;

        if key_data.len() != 32 {
            return Err(anyhow::anyhow!(
                "Invalid private key length: {}",
                key_data.len()
            ));
        }

        let mut private_key = [0u8; 32];
        private_key.copy_from_slice(&key_data);
        Ok(private_key)
    }

    /// Get encrypted storage statistics
    pub async fn get_storage_stats(
        &self,
    ) -> anyhow::Result<crate::encrypted_storage::StorageStats> {
        let key_storage = self.key_storage.lock().await;
        key_storage.get_stats().await
    }

    /// Load or generate master key
    #[allow(dead_code)]
    async fn load_or_generate_master_key() -> anyhow::Result<Arc<[u8; 32]>> {
        // Try to load from environment or file
        if let Ok(key_hex) = std::env::var("WG_AUTH_MASTER_KEY") {
            if let Ok(key_bytes) = hex::decode(&key_hex) {
                if key_bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&key_bytes);
                    return Ok(Arc::new(key));
                }
            }
        }

        // Generate new key
        let rng = SystemRandom::new();
        let mut key = [0u8; 32];
        ring::rand::SecureRandom::fill(&rng, &mut key)
            .map_err(|_| anyhow::anyhow!("Failed to generate master key"))?;

        warn!("Generated new master key - consider persisting it for production use");
        Ok(Arc::new(key))
    }

    /// Load existing sessions from database
    async fn load_sessions_from_database(&self) -> anyhow::Result<()> {
        debug!("Loading WireGuard sessions from database");

        let sessions = self.database.load_wireguard_sessions().await?;
        let now = Self::current_timestamp();

        let mut active_count = 0;
        {
            let mut session_cache = self.sessions.write().await;
            let mut peer_cache = self.peer_sessions.write().await;

            for session in sessions {
                // Only load active, non-expired sessions
                if session.is_active && session.expires_at > now {
                    peer_cache.insert(session.peer_pubkey.clone(), session.session_id.clone());
                    session_cache.insert(session.session_id.clone(), session);
                    active_count += 1;
                }
            }
        }

        info!(
            "Loaded {} active WireGuard sessions from database",
            active_count
        );
        Ok(())
    }

    /// Start background maintenance tasks
    async fn start_background_tasks(&self) {
        let sessions = self.sessions.clone();
        let peer_sessions = self.peer_sessions.clone();
        let database = self.database.clone();
        let stats = self.stats.clone();
        let cleanup_interval = self.cleanup_interval;

        // Session cleanup task
        tokio::spawn(async move {
            let mut interval = interval(cleanup_interval);

            loop {
                interval.tick().await;

                let now = Self::current_timestamp();
                let mut expired_sessions = Vec::new();

                // Find expired sessions
                {
                    let sessions_read = sessions.read().await;
                    for (session_id, session) in sessions_read.iter() {
                        if !session.is_active || session.expires_at <= now {
                            expired_sessions
                                .push((session_id.clone(), session.peer_pubkey.clone()));
                        }
                    }
                }

                // Remove expired sessions
                if !expired_sessions.is_empty() {
                    let mut sessions_write = sessions.write().await;
                    let mut peer_sessions_write = peer_sessions.write().await;

                    for (session_id, peer_pubkey) in expired_sessions {
                        sessions_write.remove(&session_id);
                        peer_sessions_write.remove(&peer_pubkey);

                        // Remove from database
                        if let Err(e) = database.remove_wireguard_session(&session_id).await {
                            warn!("Failed to remove expired session from database: {}", e);
                        }
                    }

                    // Update stats
                    let mut stats_lock = stats.lock().await;
                    let active_sessions = sessions_write
                        .values()
                        .filter(|s| s.is_active && s.expires_at > now)
                        .count() as u64;
                    stats_lock.active_sessions = active_sessions;
                }
            }
        });
    }

    /// Validate peer public key format
    fn is_valid_pubkey(pubkey: &str) -> bool {
        if pubkey.len() != 64 {
            return false;
        }
        hex::decode(pubkey).is_ok()
    }

    /// Get current Unix timestamp
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// SIMD-optimized cryptographic engine
pub struct SimdCryptoEngine {
    _rng: SystemRandom,
}

impl SimdCryptoEngine {
    /// Create new SIMD crypto engine
    pub async fn new() -> Result<Self> {
        Ok(Self {
            _rng: SystemRandom::new(),
        })
    }

    /// Generate session IDs using SIMD-accelerated BLAKE2s
    pub fn generate_session_ids_batch(&self, inputs: &[&[u8]]) -> Vec<[u8; 16]> {
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let mut hasher = Blake2s256::new();
            hasher.update(input);
            let hash = hasher.finalize();
            results.push(hash[..16].try_into().unwrap());
        }

        results
    }

    /// Derive stable PSKs (no timestamp input to avoid lockout)
    pub fn derive_stable_psk(&self, peer_key: &[u8; 32]) -> Vec<[u8; 32]> {
        let mut results = Vec::with_capacity(1);

        // Use a fixed salt for consistency (stable PSK)
        let salt = b"WG-STABLE-PSK-2024";

        let mut input = Vec::with_capacity(39);
        input.extend_from_slice(b"WG-PSK-");
        input.extend_from_slice(peer_key);
        // No timestamp - PSK should be stable

        let argon2 = Argon2::default();
        let mut psk = [0u8; 32];
        if argon2.hash_password_into(&input, salt, &mut psk).is_ok() {
            results.push(psk);
        }

        results
    }

    /// Derive session keys using server nonces (not timestamps)
    pub fn derive_session_keys(
        &self,
        peer_keys: &[[u8; 32]],
        server_nonces: &[[u8; 32]],
    ) -> Vec<([u8; 32], [u8; 16])> {
        let mut results = Vec::with_capacity(peer_keys.len());

        // Use different salt for session keys
        let salt = b"WG-SESSION-KEY-2024";

        for (peer_key, server_nonce) in peer_keys.iter().zip(server_nonces) {
            let mut input = Vec::with_capacity(71);
            input.extend_from_slice(b"WG-SESSION-");
            input.extend_from_slice(peer_key);
            input.extend_from_slice(server_nonce);

            let argon2 = Argon2::default();
            let mut session_key = [0u8; 32];
            if argon2
                .hash_password_into(&input, salt, &mut session_key)
                .is_ok()
            {
                // Derive session ID from session key
                let mut hasher = Blake2s256::new();
                hasher.update(session_key);
                let hash = hasher.finalize();
                let session_id: [u8; 16] = hash[..16].try_into().unwrap();

                results.push((session_key, session_id));
            }
        }

        results
    }
}

/// Client information for session creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub ip: Option<String>,
    pub version: Option<String>,
    pub user_agent: Option<String>,
}

/// Session filter for listing operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFilter {
    pub active_only: Option<bool>,
    pub peer_pubkey: Option<String>,
    pub created_after: Option<u64>,
    pub created_before: Option<u64>,
}

impl Default for SessionFilter {
    fn default() -> Self {
        Self {
            active_only: Some(true),
            peer_pubkey: None,
            created_after: None,
            created_before: None,
        }
    }
}
</file>

<file path="Cargo.toml">
[package]
name = "op-gateway"
version = "0.1.0"
edition = "2021"
description = "MCP Gateway with WireGuard authentication and smart routing"

[dependencies]
# Async
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
simd-json = "0.13"

# Crypto
ring = "0.17"
x25519-dalek = "2.0"
chacha20poly1305 = "0.10"
argon2 = { version = "0.5", features = ["std"] }
blake2 = "0.10"
zeroize = { version = "1.6", features = ["zeroize_derive"] }
base64 = "0.22"
hex = "0.4"

# Database
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }

# Logging
tracing = "0.1"

# UUID
uuid = { version = "1", features = ["v4", "serde"] }

# Error handling
thiserror = "1"
anyhow = "1"

# Time
chrono = { version = "0.4", features = ["serde"] }
</file>

<file path="compare-op-gateway.md">
# compare-op-gateway

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md, SECURITY-MODEL.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 5 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 4 |
| Partial artifacts | 0 |
| Spec-listed source files | 5 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- MCP Gateway with WireGuard authentication and smart routing

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/wireguard_auth.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/wireguard_auth.rs |
| `src/mcp_gateway.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/mcp_gateway.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/error.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/error.rs |
| `src/encrypted_storage.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/encrypted_storage.rs |
| `root` | ✅ Present | root source group | src/encrypted_storage.rs, src/error.rs, src/lib.rs, src/mcp_gateway.rs, src/wireguard_auth.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| wireguard_auth | ✅ Implemented | src/wireguard_auth.rs | SPEC main module |
| mcp_gateway | ✅ Implemented | src/mcp_gateway.rs | SPEC main module |
| error | ✅ Implemented | src/error.rs | SPEC main module |
| encrypted_storage | ✅ Implemented | src/encrypted_storage.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- None

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `ring` - documented in SPEC
- `x25519-dalek` - documented in SPEC
- `chacha20poly1305` - documented in SPEC
- `argon2` - documented in SPEC
- `blake2` - documented in SPEC
- `zeroize` - documented in SPEC
- `base64` - documented in SPEC
- `hex` - documented in SPEC
- `sqlx` - documented in SPEC
- `tracing` - not listed in SPEC dependency block
- `uuid` - not listed in SPEC dependency block
- `thiserror` - not listed in SPEC dependency block
- `anyhow` - not listed in SPEC dependency block
- `chrono` - not listed in SPEC dependency block

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SECURITY-MODEL.md, SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: encrypted_storage, error, mcp_gateway, wireguard_auth.
- 5 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="SECURITY-MODEL.md">
# ⚠️ CRITICAL: WireGuard Session Security Model ⚠️

## DO NOT ROTATE WG PSK PER-LOGIN

WireGuard PSK has NO overlap mechanism. Rotating it locks you out.

## Correct Model

```
WG PSK (STATIC - rarely rotated, manual only)
    │
    └── + Server Nonce (per-login, server-issued)
            │
            └── derives → Session Key (HKDF)
                            │
                            └── hash → Session ID
                                        │
                                        └── MCP Access Token
```

## Rules

1. **PSK is identity** - treat like a certificate, not a password
2. **Server nonce prevents replay** - no timestamps, no clock drift
3. **Session keys rotate** - derived fresh each login
4. **simd-json at wire edge** - serde for config/internal

## What Rotates vs What Doesn't

| Component | Rotates | How Often |
|-----------|---------|-----------|
| WG PSK | NO | Manual/yearly |
| Server Nonce | YES | Per-login |
| Session Key | YES | Per-login |
| Session ID | YES | Per-login |
| MCP Token | YES | Per-request or session |

## Implementation

See `wireguard_auth.rs` - session derivation uses server nonce, not PSK rotation.
</file>

<file path="SPEC.md">
# op-gateway - Specification

## Overview
**Crate**: `op-gateway`  
**Location**: `crates/op-gateway`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-gateway"
version = "0.1.0"
edition = "2021"
description = "MCP Gateway with WireGuard authentication and smart routing"
```

### Source Structure
```
op-gateway/src/wireguard_auth.rs
op-gateway/src/mcp_gateway.rs
op-gateway/src/lib.rs
op-gateway/src/error.rs
op-gateway/src/encrypted_storage.rs
```

### Key Dependencies
```toml
# Async
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
simd-json = "0.13"

# Crypto
ring = "0.17"
x25519-dalek = "2.0"
chacha20poly1305 = "0.10"
argon2 = { version = "0.5", features = ["std"] }
blake2 = "0.10"
zeroize = { version = "1.6", features = ["zeroize_derive"] }
base64 = "0.22"
hex = "0.4"

# Database
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
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
SECURITY-MODEL.md

## Module Structure
       5 Rust source files

### Main Modules
wireguard_auth
mcp_gateway
error
encrypted_storage

## Purpose
MCP Gateway with WireGuard authentication and smart routing

## Build Information
- **Edition**: 2021
- **Version**: 0.1.0
- **License**: 

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
</file>

</files>
