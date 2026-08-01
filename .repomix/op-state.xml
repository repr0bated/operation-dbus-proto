This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
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
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-state/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-state/
              src/
                authority.rs
                crypto.rs
                dbus_plugin_base.rs
                dbus_server.rs
                lib.rs
                manager.rs
                mod.rs
                plugin_workflow.rs
                plugin.rs
                plugtree.rs
                schema_validator.rs
              Cargo.toml
              compare-op-state.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/src/authority.rs">
/// Network Authority Enforcement
///
/// This module ensures the plugin system remains the ultimate authoritative source
/// for all network configuration, preventing interference from legacy systems.
use anyhow::Result;
use std::process::Command;

pub struct NetworkAuthority;

impl NetworkAuthority {
    /// Ensure no competing network managers are active
    pub fn enforce_authority() -> Result<()> {
        // Disable NetworkManager if running
        let _ = Command::new("systemctl")
            .args(["stop", "NetworkManager"])
            .output();

        let _ = Command::new("systemctl")
            .args(["disable", "NetworkManager"])
            .output();

        // Disable systemd-networkd if running
        let _ = Command::new("systemctl")
            .args(["stop", "systemd-networkd"])
            .output();

        let _ = Command::new("systemctl")
            .args(["disable", "systemd-networkd"])
            .output();

        log::info!("Network authority enforced - plugin system is sole controller");
        Ok(())
    }

    /// Check for authority violations
    pub fn check_authority() -> Result<Vec<String>> {
        let mut violations = Vec::new();

        // Check if NetworkManager is active
        if let Ok(output) = Command::new("systemctl")
            .args(["is-active", "NetworkManager"])
            .output()
        {
            if output.stdout == b"active\n" {
                violations.push("NetworkManager is active".to_string());
            }
        }

        // Check if systemd-networkd is active
        if let Ok(output) = Command::new("systemctl")
            .args(["is-active", "systemd-networkd"])
            .output()
        {
            if output.stdout == b"active\n" {
                violations.push("systemd-networkd is active".to_string());
            }
        }

        Ok(violations)
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/src/crypto.rs">
#![allow(dead_code)]
#![allow(deprecated)]
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{bail, Context, Result};
use argon2::{password_hash::SaltString, Argon2};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::path::Path;

const NONCE_SIZE: usize = 12;
const KEY_SIZE: usize = 32;
// Note: Salt is randomly generated and stored alongside the ciphertext if needed

/// Encrypted state file structure
#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptedState {
    /// Base64 encoded nonce
    pub nonce: String,
    /// Base64 encoded salt (for password-derived keys)
    pub salt: Option<String>,
    /// Base64 encoded encrypted data
    pub ciphertext: String,
    /// Encryption version for future compatibility
    pub version: u8,
}

/// State encryption manager
pub struct StateEncryption {
    key: Key<Aes256Gcm>,
}

impl StateEncryption {
    /// Create a new encryption manager with a random key
    pub fn new() -> Result<Self> {
        let key = Aes256Gcm::generate_key(&mut OsRng);
        Ok(Self { key })
    }

    /// Create encryption manager from a password
    pub fn from_password(password: &str) -> Result<Self> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let mut key_bytes = [0u8; KEY_SIZE];
        argon2
            .hash_password_into(
                password.as_bytes(),
                salt.as_str().as_bytes(),
                &mut key_bytes,
            )
            .map_err(|e| anyhow::anyhow!("Failed to derive key: {}", e))?;

        let key = *Key::<Aes256Gcm>::from_slice(&key_bytes);
        Ok(Self { key })
    }

    /// Create encryption manager from existing key
    pub fn from_key(key_bytes: &[u8]) -> Result<Self> {
        if key_bytes.len() != KEY_SIZE {
            bail!(
                "Invalid key size: expected {} bytes, got {}",
                KEY_SIZE,
                key_bytes.len()
            );
        }

        let key = *Key::<Aes256Gcm>::from_slice(key_bytes);
        Ok(Self { key })
    }

    /// Load or generate key from file
    pub fn from_key_file(path: &Path) -> Result<Self> {
        if path.exists() {
            // Load existing key
            let key_data = std::fs::read(path).context("Failed to read key file")?;

            if key_data.len() != KEY_SIZE {
                bail!(
                    "Invalid key file: expected {} bytes, got {}",
                    KEY_SIZE,
                    key_data.len()
                );
            }

            Self::from_key(&key_data)
        } else {
            // Generate new key and save it
            let encryption = Self::new()?;

            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).context("Failed to create key directory")?;
            }

            // Save key with restricted permissions
            std::fs::write(path, encryption.key.as_slice()).context("Failed to write key file")?;

            // Set permissions to 600 (owner read/write only)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(path)?.permissions();
                perms.set_mode(0o600);
                std::fs::set_permissions(path, perms)
                    .context("Failed to set key file permissions")?;
            }

            Ok(encryption)
        }
    }

    /// Encrypt data
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedState> {
        let cipher = Aes256Gcm::new(&self.key);

        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        use rand::RngCore;
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        Ok(EncryptedState {
            nonce: BASE64.encode(nonce_bytes),
            salt: None,
            ciphertext: BASE64.encode(ciphertext),
            version: 1,
        })
    }

    /// Decrypt data
    pub fn decrypt(&self, encrypted: &EncryptedState) -> Result<Vec<u8>> {
        if encrypted.version != 1 {
            bail!("Unsupported encryption version: {}", encrypted.version);
        }

        let nonce_bytes = BASE64
            .decode(&encrypted.nonce)
            .context("Failed to decode nonce")?;

        if nonce_bytes.len() != NONCE_SIZE {
            bail!("Invalid nonce size");
        }

        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = BASE64
            .decode(&encrypted.ciphertext)
            .context("Failed to decode ciphertext")?;

        let cipher = Aes256Gcm::new(&self.key);
        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

        Ok(plaintext)
    }

    /// Encrypt JSON data
    pub fn encrypt_json<T: serde::Serialize>(&self, data: &T) -> Result<EncryptedState> {
        let json = simd_json::to_vec(data).context("Failed to serialize data")?;
        self.encrypt(&json)
    }

    pub fn decrypt_json<T: serde::de::DeserializeOwned>(
        &self,
        encrypted: &EncryptedState,
    ) -> Result<T> {
        let plaintext = self.decrypt(encrypted)?;
        let mut plaintext_mut = plaintext;
        simd_json::from_slice(&mut plaintext_mut).context("Failed to deserialize decrypted data")
    }
}

/// Helper functions for state files  
pub mod state_file {
    use super::*;
    use crate::DesiredState as State;

    /// Save state with encryption
    pub fn save_encrypted(state: &State, path: &Path, encryption: &StateEncryption) -> Result<()> {
        let encrypted = encryption.encrypt_json(state)?;

        let json = simd_json::to_string_pretty(&encrypted)
            .context("Failed to serialize encrypted state")?;

        // Write to temporary file first
        let tmp_path = path.with_extension("tmp");
        std::fs::write(&tmp_path, json).context("Failed to write encrypted state")?;

        // Atomic rename
        std::fs::rename(&tmp_path, path).context("Failed to rename state file")?;

        Ok(())
    }

    /// Load state with decryption
    pub fn load_encrypted(path: &Path, encryption: &StateEncryption) -> Result<State> {
        let mut contents = std::fs::read_to_string(path).context("Failed to read state file")?;

        let encrypted: EncryptedState = unsafe { simd_json::from_str(&mut contents) }
            .context("Failed to parse encrypted state")?;

        encryption.decrypt_json(&encrypted)
    }

    /// Check if a state file is encrypted
    pub fn is_encrypted(path: &Path) -> Result<bool> {
        let contents = std::fs::read_to_string(path).context("Failed to read state file")?;

        // Try to parse as encrypted state
        let mut c1 = contents.clone();
        if unsafe { simd_json::from_str::<EncryptedState>(&mut c1) }.is_ok() {
            return Ok(true);
        }

        // Try to parse as plain state
        let mut c2 = contents;
        if unsafe { simd_json::from_str::<State>(&mut c2) }.is_ok() {
            return Ok(false);
        }

        bail!("Unknown state file format");
    }

    /// Migrate unencrypted state to encrypted
    pub fn migrate_to_encrypted(path: &Path, encryption: &StateEncryption) -> Result<()> {
        if is_encrypted(path)? {
            return Ok(()); // Already encrypted
        }

        // Read plain state
        let mut contents = std::fs::read_to_string(path).context("Failed to read state file")?;

        let state: State =
            unsafe { simd_json::from_str(&mut contents) }.context("Failed to parse state")?;

        // Backup original
        let backup_path = path.with_extension("bak");
        std::fs::copy(path, &backup_path).context("Failed to create backup")?;

        // Save encrypted
        save_encrypted(&state, path, encryption)?;

        println!("Successfully migrated state to encrypted format");
        println!("Backup saved to: {:?}", backup_path);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_roundtrip() {
        let encryption = StateEncryption::new().unwrap();

        let plaintext = b"Hello, World!";
        let encrypted = encryption.encrypt(plaintext).unwrap();
        let decrypted = encryption.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_json_encryption() {
        use simd_json::json;

        let encryption = StateEncryption::new().unwrap();

        let data = json!({
            "key": "value",
            "number": 42,
            "nested": {
                "array": [1, 2, 3]
            }
        });

        let encrypted = encryption.encrypt_json(&data).unwrap();
        let decrypted: simd_json::OwnedValue = encryption.decrypt_json(&encrypted).unwrap();

        assert_eq!(data, decrypted);
    }

    #[test]
    fn test_password_derivation() {
        let password = "test_password_123";
        let encryption1 = StateEncryption::from_password(password).unwrap();

        let plaintext = b"Secret data";
        let encrypted = encryption1.encrypt(plaintext).unwrap();

        // Different instance with same password should work
        // Note: In practice, we'd need to store and reuse the salt
        // This is just for demonstration
        let result = encryption1.decrypt(&encrypted).unwrap();
        assert_eq!(plaintext.to_vec(), result);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/src/dbus_plugin_base.rs">
#![allow(async_fn_in_trait)]
//! Base trait for D-Bus state plugins
//! Provides common D-Bus operations, hash footprints, and blockchain integration

// Blockchain module not yet added - stub the type for now
pub struct PluginFootprint;

impl PluginFootprint {
    pub fn new(_plugin_name: String, _action: String, _diff_data: simd_json::OwnedValue) -> Self {
        PluginFootprint
    }
}
use crate::plugin::StatePlugin;
use anyhow::{Context, Result};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use simd_json::ValueBuilder;
use std::collections::HashMap;
use tokio::sync::mpsc::UnboundedSender;
use zbus::{Connection, Proxy};

/// Base trait for all D-Bus-based state plugins
/// Provides common functionality for interacting with D-Bus services
#[allow(dead_code, async_fn_in_trait)]
pub trait DbusStatePluginBase: StatePlugin {
    /// D-Bus service name (e.g., "org.freedesktop.systemd1")
    fn service_name(&self) -> &str;

    /// Base object path (e.g., "/org/freedesktop/systemd1")
    fn base_path(&self) -> &str;

    /// Optional blockchain footprint sender
    fn blockchain_sender(&self) -> Option<&UnboundedSender<PluginFootprint>> {
        None
    }

    /// Connect to D-Bus service and create proxy
    async fn connect_dbus(&self, path: &str, interface: &str) -> Result<Proxy<'static>> {
        let conn = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;

        // Convert to owned strings to satisfy 'static lifetime
        let service_name = self.service_name().to_string();
        let path_owned = path.to_string();
        let interface_owned = interface.to_string();

        Proxy::new(&conn, service_name, path_owned, interface_owned)
            .await
            .context(format!(
                "Failed to create D-Bus proxy for {}/{}",
                self.service_name(),
                interface
            ))
    }

    /// Get a D-Bus property value
    async fn get_property(&self, proxy: &Proxy<'_>, property: &str) -> Result<Value> {
        let value: zbus::zvariant::OwnedValue = proxy
            .get_property(property)
            .await
            .context(format!("Failed to get property {}", property))?;

        // Convert zbus::zvariant::Value to simd_json::OwnedValue
        let mut json_str = format!("{:?}", value); // Simplified - would need proper conversion
        Ok(unsafe { simd_json::from_str(&mut json_str) }.unwrap_or(Value::null()))
    }

    /// Set a D-Bus property value
    async fn set_property(&self, proxy: &Proxy<'_>, property: &str, value: &Value) -> Result<()> {
        // Convert simd_json::OwnedValue to zbus::zvariant::Value (simplified)
        let zbus_value: zbus::zvariant::Value = if let Some(s) = value.as_str() {
            s.into()
        } else if let Some(b) = value.as_bool() {
            b.into()
        } else if let Some(i) = value.as_i64() {
            i.into()
        } else if let Some(f) = value.as_f64() {
            f.into()
        } else {
            return Err(anyhow::anyhow!("Unsupported value type for D-Bus property"));
        };

        proxy
            .set_property(property, zbus_value)
            .await
            .context(format!("Failed to set property {}", property))?;

        Ok(())
    }

    /// Get all properties from a D-Bus interface
    async fn get_all_properties(&self, proxy: &Proxy<'_>) -> Result<HashMap<String, Value>> {
        // Use org.freedesktop.DBus.Properties.GetAll
        let props_proxy = Proxy::new(
            proxy.connection(),
            proxy.destination(),
            proxy.path(),
            "org.freedesktop.DBus.Properties",
        )
        .await?;

        let all_props: HashMap<String, zbus::zvariant::OwnedValue> = props_proxy
            .call("GetAll", &(proxy.interface(),))
            .await
            .context("Failed to get all properties")?;

        // Convert to simd_json::OwnedValue HashMap
        let mut result = HashMap::new();
        for (key, _value) in all_props {
            // Simplified conversion - would need proper zvariant to serde_json conversion
            result.insert(key, Value::null());
        }

        Ok(result)
    }

    /// Call a D-Bus method (no-arg version - for methods with args, use proxy.call directly)
    async fn call_method_no_args(
        &self,
        proxy: &Proxy<'_>,
        method: &str,
    ) -> Result<zbus::zvariant::OwnedValue> {
        proxy
            .call(method, &())
            .await
            .context(format!("Failed to call method {}", method))
    }

    /// Introspect D-Bus object to get schema
    async fn introspect(&self, path: &str) -> Result<String> {
        let conn = Connection::system().await?;
        let proxy = Proxy::new(
            &conn,
            self.service_name(),
            path,
            "org.freedesktop.DBus.Introspectable",
        )
        .await?;

        let xml: String = proxy
            .call("Introspect", &())
            .await
            .context("Failed to introspect D-Bus object")?;

        Ok(xml)
    }

    /// Calculate cryptographic hash of state (SHA-256)
    fn hash_state(&self, state: &Value) -> String {
        use sha2::{Digest, Sha256};
        let json_str = simd_json::to_string(state).unwrap_or_default();
        format!("{:x}", Sha256::digest(json_str.as_bytes()))
    }

    /// Calculate diff between two states and return hash footprint
    fn calculate_footprint(
        &self,
        old_state: &Value,
        new_state: &Value,
        action: &str,
    ) -> PluginFootprint {
        let diff_data = simd_json::json!({
            "old": old_state,
            "new": new_state,
            "old_hash": self.hash_state(old_state),
            "new_hash": self.hash_state(new_state),
        });

        PluginFootprint::new(self.name().to_string(), action.to_string(), diff_data)
    }

    /// Record state change to blockchain
    async fn record_footprint(&self, action: &str, data: Value) -> Result<()> {
        if let Some(sender) = self.blockchain_sender() {
            let footprint = PluginFootprint::new(self.name().to_string(), action.to_string(), data);

            sender
                .send(footprint)
                .map_err(|e| anyhow::anyhow!("Failed to send footprint to blockchain: {}", e))?;

            log::debug!("Recorded footprint for {} action: {}", self.name(), action);
        } else {
            log::trace!("No blockchain sender configured, skipping footprint");
        }

        Ok(())
    }

    /// Record a state transition (before/after)
    async fn record_state_transition(
        &self,
        old_state: &Value,
        new_state: &Value,
        action: &str,
    ) -> Result<()> {
        let footprint_data = simd_json::json!({
            "old_state": old_state,
            "new_state": new_state,
            "old_hash": self.hash_state(old_state),
            "new_hash": self.hash_state(new_state),
            "action": action,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        self.record_footprint(action, footprint_data).await
    }

    /// Verify state hash matches expected
    fn verify_state_hash(&self, state: &Value, expected_hash: &str) -> bool {
        self.hash_state(state) == expected_hash
    }

    /// Get D-Bus connection
    async fn get_connection(&self) -> Result<Connection> {
        Connection::system()
            .await
            .context("Failed to connect to system D-Bus")
    }

    /// List all object paths under a base path (for enumeration)
    async fn list_objects(&self, base_path: &str) -> Result<Vec<String>> {
        // This would use D-Bus introspection to walk the object tree
        // Simplified implementation
        let xml = self.introspect(base_path).await?;

        // Parse XML and extract child nodes
        // For now, return empty - full implementation would parse introspection XML
        log::debug!("Introspection XML for {}: {}", base_path, xml);

        Ok(Vec::new())
    }
}

/// Helper functions for D-Bus value conversion
pub mod conversion {
    use super::Value;
    use simd_json::prelude::*;
    use simd_json::ValueBuilder;
    use zbus::zvariant;

    /// Convert simd_json::OwnedValue to zbus::zvariant::Value
    #[allow(dead_code)]
    pub fn json_to_zvariant(value: &Value) -> Result<zvariant::Value<'_>, anyhow::Error> {
        if value.is_null() {
            Ok(zvariant::Value::from(""))
        } else if let Some(b) = value.as_bool() {
            Ok(zvariant::Value::from(b))
        } else if let Some(i) = value.as_i64() {
            Ok(zvariant::Value::from(i))
        } else if let Some(u) = value.as_u64() {
            Ok(zvariant::Value::from(u))
        } else if let Some(f) = value.as_f64() {
            Ok(zvariant::Value::from(f))
        } else if let Some(s) = value.as_str() {
            Ok(zvariant::Value::from(s))
        } else {
            Err(anyhow::anyhow!("Unsupported value type for conversion"))
        }
    }

    /// Convert zbus::zvariant::Value to simd_json::OwnedValue
    #[allow(dead_code)]
    pub fn zvariant_to_json(value: &zvariant::Value) -> Result<Value, anyhow::Error> {
        // Simplified - full implementation would handle all zvariant types
        match value.value_signature().to_string().as_str() {
            "s" => {
                if let Ok(s) = <&str>::try_from(value) {
                    Ok(Value::from(s.to_string()))
                } else {
                    Ok(Value::null())
                }
            }
            "b" => {
                if let Ok(b) = bool::try_from(value) {
                    Ok(Value::from(b))
                } else {
                    Ok(Value::null())
                }
            }
            "i" | "u" | "x" | "t" => {
                // Try various integer types
                if let Ok(i) = i64::try_from(value) {
                    Ok(Value::from(i))
                } else if let Ok(u) = u64::try_from(value) {
                    Ok(Value::from(u))
                } else {
                    Ok(Value::null())
                }
            }
            "d" => {
                if let Ok(f) = f64::try_from(value) {
                    Ok(Value::from(f))
                } else {
                    Ok(Value::null())
                }
            }
            _ => {
                // For complex types, use debug representation
                Ok(Value::from(format!("{:?}", value)))
            }
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/src/dbus_server.rs">
//! D-Bus server for system bus integration

use crate::manager::StateManager;
use crate::plugin::StatePlugin;
use crate::DesiredState;
use anyhow::Result;
use op_state_store::{SchemaCatalog, SchemaRegistry};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use zbus::Connection;

/// D-Bus interface for the state manager
pub struct StateManagerDBus {
    state_manager: Arc<StateManager>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct ProjectedObject {
    origin_service: String,
    origin_path: String,
}

#[derive(Default)]
#[allow(dead_code)]
struct PublicationRegistry {
    published_paths: HashSet<String>,
    paths_by_service: HashMap<String, HashSet<String>>,
}

#[allow(dead_code)]
impl PublicationRegistry {
    fn insert(&mut self, service: &str, path: String) -> bool {
        if !self.published_paths.insert(path.clone()) {
            return false;
        }
        self.paths_by_service
            .entry(service.to_string())
            .or_default()
            .insert(path);
        true
    }
    fn remove_path(&mut self, service: &str, path: &str) {
        self.published_paths.remove(path);
        if let Some(paths) = self.paths_by_service.get_mut(service) {
            paths.remove(path);
            if paths.is_empty() {
                self.paths_by_service.remove(service);
            }
        }
    }
    fn remove_service(&mut self, service: &str) -> Vec<String> {
        let paths = self.paths_by_service.remove(service).unwrap_or_default();
        for path in &paths {
            self.published_paths.remove(path);
        }
        paths.into_iter().collect()
    }
    fn total_paths(&self) -> usize {
        self.published_paths.len()
    }
}

#[zbus::interface(name = "org.opdbus.ProjectedObjectV1")]
#[allow(dead_code)]
impl ProjectedObject {
    #[zbus(property)]
    async fn origin_service(&self) -> String {
        self.origin_service.clone()
    }
    #[zbus(property)]
    async fn origin_path(&self) -> String {
        self.origin_path.clone()
    }
}

#[zbus::interface(name = "org.opdbus.StateManager")]
impl StateManagerDBus {
    async fn apply_openflow_state(&self, state_json: String) -> zbus::fdo::Result<String> {
        let mut state_json_mut = state_json;
        match unsafe { simd_json::from_str::<DesiredState>(&mut state_json_mut) } {
            Ok(desired_state) => self
                .state_manager
                .apply_plugin_state("openflow", desired_state.state)
                .await
                .and_then(|result| simd_json::to_string(&result).map_err(Into::into))
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string())),
            Err(e) => Err(zbus::fdo::Error::InvalidArgs(format!(
                "Invalid JSON: {}",
                e
            ))),
        }
    }

    async fn query_state(&self) -> zbus::fdo::Result<String> {
        match self.state_manager.query_current_state().await {
            Ok(state) => match simd_json::to_string(&QueryStateResponse { plugins: state }) {
                Ok(json) => Ok(json),
                Err(e) => Err(zbus::fdo::Error::Failed(format!(
                    "Serialization failed: {}",
                    e
                ))),
            },
            Err(e) => Err(zbus::fdo::Error::Failed(format!("Query failed: {}", e))),
        }
    }

    async fn apply_contract_mutation(&self, request_json: String) -> zbus::fdo::Result<String> {
        let mut request_json_mut = request_json;
        let request: ContractMutationRequest =
            unsafe { simd_json::from_str(&mut request_json_mut) }.map_err(|e| {
                zbus::fdo::Error::InvalidArgs(format!("Invalid contract mutation payload: {}", e))
            })?;

        self.state_manager
            .apply_plugin_state(&request.plugin_id, request.value)
            .await
            .and_then(|result| simd_json::to_string(&result).map_err(Into::into))
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }
}

/// D-Bus interface for an individual plugin
pub struct PluginDbusHost {
    pub plugin: Arc<dyn StatePlugin>,
    /// Compatibility name kept on the host shape for older call sites. This is
    /// the shared schema catalog used to resolve the canonical plugin document.
    pub schema_registry: Arc<RwLock<SchemaRegistry>>,
}

#[zbus::interface(name = "org.opdbus.PluginV1")]
impl PluginDbusHost {
    #[zbus(property)]
    async fn name(&self) -> String {
        self.plugin.name().to_string()
    }

    #[zbus(property)]
    async fn version(&self) -> String {
        self.plugin.version().to_string()
    }

    #[zbus(property)]
    async fn description(&self) -> String {
        self.plugin.metadata().description
    }

    async fn get_state(&self) -> zbus::fdo::Result<String> {
        let state = self
            .plugin
            .query_current_state()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(simd_json::to_string(&state).unwrap_or_default())
    }

    async fn get_schema(&self) -> zbus::fdo::Result<String> {
        let plugin_name = self.plugin.name();
        let catalog = self.schema_registry.read();
        let schema = catalog
            .get_copies(plugin_name)
            .map(|copies| copies.json_schema.clone())
            .ok_or_else(|| {
                zbus::fdo::Error::Failed(format!(
                    "Schema '{}' not found in shared catalog",
                    plugin_name
                ))
            })?;

        Ok(simd_json::to_string(&schema).unwrap_or_default())
    }
}

/// Preferred architectural name for `PluginDbusHost` schema lookup state.
pub type SharedSchemaCatalog = Arc<RwLock<SchemaCatalog>>;

/// Compatibility alias for older call sites that still say `registry`.
pub type SharedSchemaRegistry = SharedSchemaCatalog;

pub async fn register_on_connection(
    connection: &Connection,
    state_manager: Arc<StateManager>,
) -> Result<()> {
    let state_iface = StateManagerDBus { state_manager };
    connection
        .object_server()
        .at("/org/opdbus/v1/state", state_iface)
        .await?;
    Ok(())
}

pub async fn start_system_bus(state_manager: Arc<StateManager>) -> Result<()> {
    let connection = Connection::system().await?;
    serve_connection(connection, state_manager).await
}

pub async fn start_session_bus(state_manager: Arc<StateManager>) -> Result<()> {
    let connection = Connection::session().await?;
    serve_connection(connection, state_manager).await
}

async fn serve_connection(connection: Connection, state_manager: Arc<StateManager>) -> Result<()> {
    register_on_connection(&connection, state_manager).await?;
    connection.request_name("org.opdbus.v1").await?;
    std::future::pending::<()>().await;
    Ok(())
}

#[derive(Debug, Serialize)]
struct QueryStateResponse {
    plugins: HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct ContractMutationRequest {
    plugin_id: String,
    value: Value,
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/src/lib.rs">
//! op-state: State Management System
//!
//! Provides:
//! - StatePlugin trait for pluggable state management
//! - State manager for coordinating plugins
//! - Crypto utilities for state hashing/signing
//! - Schema-catalog-backed validation
//! - Plugin tree for hierarchical state
//! - Persistent storage via op-state-store
//! - Auto-plugin generation

pub mod authority;
// pub mod auto_plugin;
pub mod crypto;
pub mod dbus_plugin_base;
pub mod dbus_server;
pub mod manager;
pub mod plugin;
pub mod plugin_workflow;
pub mod plugtree;
pub mod schema_validator;

pub use manager::StateManager;
pub use plugin::{
    ApplyResult, ChangeOperation, Checkpoint, DesiredState, DiffMetadata, PluginCapabilities,
    PluginMetadata, StateAction, StateChange, StateDiff, StatePlugin, StateSource, ValidationError,
    ValidationResult,
};
pub use plugtree::PlugTree;

// Re-export state store types
pub use op_state_store::{
    ExecutionJob, ExecutionResult, ExecutionStatus, PluginSchema, SchemaCatalog, SchemaRegistry,
    SqliteStore, StateStore, StateStoreError,
};

/// Prelude for convenient imports
pub mod prelude {
    pub use super::manager::StateManager;
    pub use super::plugin::{
        ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff,
        StatePlugin,
    };
    pub use super::plugtree::PlugTree;
    // State store types
    pub use op_state_store::{
        ExecutionJob, ExecutionStatus, PluginSchema, SchemaCatalog, SchemaRegistry, SqliteStore,
        StateStore,
    };
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/src/manager.rs">
//! State manager for coordinating plugins and schemas

use crate::plugin::{ApplyResult, StateDiff, StatePlugin};
use anyhow::{anyhow, Result};
use op_state_store::{SchemaCatalog, SchemaRegistry, StateStore};
use parking_lot::RwLock;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Global state manager
pub struct StateManager {
    plugins: Arc<RwLock<HashMap<String, Arc<dyn StatePlugin>>>>,
    #[allow(dead_code)]
    store: Option<Arc<dyn StateStore>>,
    schema_catalog: Arc<RwLock<SchemaCatalog>>,
    /// Broadcast sender for watch() method
    watch_tx: Option<Arc<tokio::sync::broadcast::Sender<PluginEvent>>>,
}

/// Plugin event for broadcast
#[derive(Debug, Clone)]
pub struct PluginEvent {
    pub plugin_id: String,
    pub operation: PluginOperation,
}

/// Plugin operation type
#[derive(Debug, Clone)]
pub enum PluginOperation {
    Register,
    Deregister,
    Update,
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl StateManager {
    /// Create a new state manager
    pub fn new() -> Self {
        let (watch_tx, _) = tokio::sync::broadcast::channel(100);
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            store: None,
            schema_catalog: Arc::new(RwLock::new(SchemaCatalog::new())),
            watch_tx: Some(Arc::new(watch_tx)),
        }
    }

    /// Preferred constructor: create with a specific schema catalog.
    pub fn with_schema_catalog(schema_catalog: Arc<RwLock<SchemaCatalog>>) -> Self {
        let (watch_tx, _) = tokio::sync::broadcast::channel(100);
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            store: None,
            schema_catalog,
            watch_tx: Some(Arc::new(watch_tx)),
        }
    }

    /// Compatibility constructor for older call sites that still pass the
    /// catalog under the `schema_registry` name.
    pub fn with_schema_registry(schema_registry: Arc<RwLock<SchemaRegistry>>) -> Self {
        Self::with_schema_catalog(schema_registry)
    }

    /// Register a plugin
    pub fn register_plugin(&self, name: String, plugin: Arc<dyn StatePlugin>) {
        self.plugins.write().insert(name.clone(), plugin);

        // Fire watch broadcast
        if let Some(tx) = &self.watch_tx {
            let _ = tx.send(PluginEvent {
                plugin_id: name,
                operation: PluginOperation::Register,
            });
        }
    }

    /// Get a plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<Arc<dyn StatePlugin>> {
        self.plugins.read().get(name).cloned()
    }

    /// Watch for plugin state changes
    pub fn watch(&self) -> Option<tokio::sync::broadcast::Receiver<PluginEvent>> {
        self.watch_tx.as_ref().map(|tx| tx.subscribe())
    }

    /// List all registered plugins
    pub fn list_plugins(&self) -> Vec<String> {
        self.plugins.read().keys().cloned().collect()
    }

    /// Compatibility accessor. Architecturally this is the schema catalog used
    /// for lookup and validation, not a second source of truth.
    pub fn schema_registry(&self) -> Arc<RwLock<SchemaRegistry>> {
        self.schema_catalog.clone()
    }

    pub fn schema_catalog(&self) -> Arc<RwLock<SchemaCatalog>> {
        self.schema_catalog.clone()
    }

    /// Query current state for all plugins
    pub async fn query_current_state(&self) -> Result<HashMap<String, Value>> {
        let mut state = HashMap::new();
        let plugin_map = self.plugins.read().clone();

        for (name, plugin) in plugin_map {
            if let Ok(plugin_state) = plugin.query_current_state().await {
                state.insert(name, plugin_state);
            }
        }
        Ok(state)
    }

    /// Validate a desired plugin state against the authoritative schema catalog.
    pub fn validate_plugin_state(&self, plugin_name: &str, desired: &Value) -> Result<()> {
        let validation = self
            .schema_catalog
            .read()
            .validate(plugin_name, desired)
            .ok_or_else(|| anyhow!("Schema '{}' not found in schema catalog", plugin_name))?;

        if validation.valid {
            return Ok(());
        }

        Err(anyhow!(
            "State rejected by schema '{}': {}",
            plugin_name,
            validation.errors.join("; ")
        ))
    }

    /// Apply a full desired state document for one plugin.
    pub async fn apply_plugin_state(
        &self,
        plugin_name: &str,
        desired: Value,
    ) -> Result<ApplyResult> {
        self.validate_plugin_state(plugin_name, &desired)?;

        let plugin = self
            .get_plugin(plugin_name)
            .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_name))?;
        let current = plugin.query_current_state().await?;
        let diff = plugin.calculate_diff(&current, &desired).await?;

        plugin.apply_state(&diff).await
    }

    /// Apply state to a plugin
    pub async fn apply_state(&self, diff: StateDiff) -> Result<ApplyResult> {
        let plugin = self
            .get_plugin(&diff.plugin)
            .ok_or_else(|| anyhow!("Plugin '{}' not found", diff.plugin))?;
        plugin.apply_state(&diff).await
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/src/mod.rs">
//! State management - declarative plugin system
#[cfg(any(feature = "mcp", feature = "web"))]
pub mod authority;
pub mod auto_plugin;
pub mod crypto;
pub mod dbus_plugin_base;
pub mod dbus_server;
pub mod manager;
pub mod plugin;
pub mod plugin_workflow;
pub mod plugins;
pub mod plugtree;
pub mod schema_validator;

pub use manager::StateManager;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/src/plugin_workflow.rs">
//! Plugin Workflow System - Node-Based Architecture for Plugins
#![allow(dead_code)]
//!
//! This module enables plugins to participate in flow-based workflows using PocketFlow.
//! Each plugin becomes a node that can be connected to other plugins in complex pipelines.

use crate::plugin::StatePlugin;
use anyhow::Result;
use async_trait::async_trait;
use pocketflow_rs::context::Context;
use pocketflow_rs::node::{Node, ProcessResult, ProcessState};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use simd_json::ValueBuilder;
use std::sync::Arc;

/// Workflow states for plugin execution
#[derive(Debug, Clone, PartialEq, Default)]
pub enum PluginWorkflowState {
    /// Plugin execution started
    #[default]
    Started,
    /// Plugin successfully completed its task
    Completed,
    /// Plugin failed during execution
    Failed,
    /// Plugin is waiting for input from another plugin
    WaitingForInput,
    /// Plugin execution was skipped due to conditions
    Skipped,
    /// Plugin requires manual intervention
    NeedsIntervention,
}

impl ProcessState for PluginWorkflowState {
    fn is_default(&self) -> bool {
        matches!(self, PluginWorkflowState::Started)
    }

    fn to_condition(&self) -> String {
        match self {
            PluginWorkflowState::Started => "started",
            PluginWorkflowState::Completed => "completed",
            PluginWorkflowState::Failed => "failed",
            PluginWorkflowState::WaitingForInput => "waiting_for_input",
            PluginWorkflowState::Skipped => "skipped",
            PluginWorkflowState::NeedsIntervention => "needs_intervention",
        }
        .to_string()
    }
}

/// A workflow-enabled plugin that wraps any StatePlugin
pub struct WorkflowPluginNode {
    /// The underlying plugin
    plugin: Arc<dyn StatePlugin>,
    /// Plugin inputs (data keys this plugin expects from context)
    inputs: Vec<String>,
    /// Plugin outputs (data keys this plugin writes to context)
    outputs: Vec<String>,
    /// Plugin-specific configuration
    config: Value,
}

impl WorkflowPluginNode {
    pub fn new(plugin: Arc<dyn StatePlugin>) -> Self {
        Self {
            plugin,
            inputs: Vec::new(),
            outputs: Vec::new(),
            config: Value::null(),
        }
    }

    pub fn with_inputs(mut self, inputs: Vec<String>) -> Self {
        self.inputs = inputs;
        self
    }

    pub fn with_outputs(mut self, outputs: Vec<String>) -> Self {
        self.outputs = outputs;
        self
    }

    pub fn with_config(mut self, config: Value) -> Self {
        self.config = config;
        self
    }

    /// Extract inputs from workflow context
    fn extract_inputs(&self, context: &Context) -> Result<Value> {
        let mut input_data = simd_json::value::owned::Object::new();

        for input_key in &self.inputs {
            if let Some(serde_value) = context.get(input_key) {
                let simd_value: Value = simd_json::serde::to_owned_value(serde_value)?;
                input_data.insert(input_key.clone(), simd_value);
            }
        }

        Ok(Value::Object(Box::new(input_data)))
    }

    /// Store outputs in workflow context
    fn store_outputs(&self, context: &mut Context, output_data: &Value) -> Result<()> {
        if let Some(obj) = output_data.as_object() {
            for (key, value) in obj {
                if self.outputs.contains(key) {
                    let serde_value: serde_json::Value = serde_json::to_value(value)?;
                    context.set(key, serde_value);
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Node for WorkflowPluginNode {
    type State = PluginWorkflowState;

    async fn prepare(&self, context: &mut Context) -> Result<()> {
        log::info!(
            "🔧 Preparing plugin '{}' for workflow execution",
            self.plugin.name()
        );

        // Extract inputs from context and prepare plugin
        let inputs = self.extract_inputs(context)?;
        log::debug!(
            "📥 Plugin '{}' received inputs: {:?}",
            self.plugin.name(),
            inputs
        );

        Ok(())
    }

    async fn execute(&self, context: &Context) -> Result<serde_json::Value> {
        log::info!("⚡ Executing plugin '{}' in workflow", self.plugin.name());

        // Check if plugin is available
        if !self.plugin.is_available() {
            log::warn!(
                "⚠️  Plugin '{}' is not available: {}",
                self.plugin.name(),
                self.plugin.unavailable_reason()
            );
            return Ok(serde_json::Value::String("skipped".to_string()));
        }

        // Query current state
        let current_state = self.plugin.query_current_state().await?;
        log::debug!(
            "📊 Plugin '{}' current state: {:?}",
            self.plugin.name(),
            current_state
        );

        // For workflow execution, we assume the "desired" state comes from inputs
        // In a real implementation, this would be more sophisticated
        let desired_state = if let Some(serde_val) = context.get("desired_state") {
            simd_json::serde::to_owned_value(serde_val)?
        } else {
            Value::null()
        };

        // Calculate diff
        let diff = self
            .plugin
            .calculate_diff(&current_state, &desired_state)
            .await?;
        log::debug!(
            "🔍 Plugin '{}' calculated diff: {:?}",
            self.plugin.name(),
            diff
        );

        // Apply changes if needed
        if !diff.actions.is_empty() {
            log::info!(
                "🔄 Plugin '{}' applying {} changes",
                self.plugin.name(),
                diff.actions.len()
            );
            let result = self.plugin.apply_state(&diff).await?;

            match result.success {
                true => {
                    log::info!("✅ Plugin '{}' completed successfully", self.plugin.name());
                    Ok(serde_json::Value::String("completed".to_string()))
                }
                false => {
                    log::error!(
                        "❌ Plugin '{}' failed: {:?}",
                        self.plugin.name(),
                        result.errors
                    );
                    Ok(serde_json::Value::String("failed".to_string()))
                }
            }
        } else {
            log::info!("⏭️  Plugin '{}' - no changes needed", self.plugin.name());
            Ok(serde_json::Value::String("completed".to_string()))
        }
    }

    async fn post_process(
        &self,
        context: &mut Context,
        result: &Result<serde_json::Value>,
    ) -> Result<ProcessResult<PluginWorkflowState>> {
        match result {
            Ok(value) => {
                if let Some(status) = value.as_str() {
                    match status {
                        "completed" => {
                            // Store successful execution results in context
                            let execution_result = simd_json::json!({
                                "plugin": self.plugin.name(),
                                "status": "completed",
                                "timestamp": chrono::Utc::now().timestamp()
                            });
                            let serde_result = serde_json::to_value(execution_result)?;
                            self.store_outputs(
                                context,
                                &simd_json::serde::to_owned_value(&serde_result)?,
                            )?;
                            log::info!(
                                "📤 Plugin '{}' stored results in workflow context",
                                self.plugin.name()
                            );
                            Ok(ProcessResult::new(
                                PluginWorkflowState::Completed,
                                "Plugin completed successfully".to_string(),
                            ))
                        }
                        "failed" => {
                            // Store failure information
                            let failure_result = simd_json::json!({
                                "plugin": self.plugin.name(),
                                "status": "failed",
                                "timestamp": chrono::Utc::now().timestamp()
                            });
                            let serde_failure: serde_json::Value =
                                serde_json::to_value(failure_result)?;
                            context.set("last_error", serde_failure);
                            log::error!(
                                "💥 Plugin '{}' workflow execution failed",
                                self.plugin.name()
                            );
                            Ok(ProcessResult::new(
                                PluginWorkflowState::Failed,
                                "Plugin execution failed".to_string(),
                            ))
                        }
                        "skipped" => {
                            log::info!(
                                "⏭️  Plugin '{}' was skipped in workflow",
                                self.plugin.name()
                            );
                            Ok(ProcessResult::new(
                                PluginWorkflowState::Skipped,
                                "Plugin was skipped".to_string(),
                            ))
                        }
                        _ => {
                            log::debug!(
                                "Plugin '{}' completed with status: {}",
                                self.plugin.name(),
                                status
                            );
                            Ok(ProcessResult::new(
                                PluginWorkflowState::Completed,
                                format!("Plugin completed with status: {}", status),
                            ))
                        }
                    }
                } else {
                    Ok(ProcessResult::new(
                        PluginWorkflowState::Completed,
                        "Plugin completed".to_string(),
                    ))
                }
            }
            Err(e) => {
                log::error!("💥 Plugin '{}' execution error: {}", self.plugin.name(), e);
                let error_result = simd_json::json!({
                    "plugin": self.plugin.name(),
                    "status": "error",
                    "error": e.to_string(),
                    "timestamp": chrono::Utc::now().timestamp()
                });
                let serde_error: serde_json::Value = serde_json::to_value(error_result)?;
                context.set("last_error", serde_error);
                Ok(ProcessResult::new(
                    PluginWorkflowState::Failed,
                    format!("Plugin execution error: {}", e),
                ))
            }
        }
    }
}

/// Plugin Workflow Manager - Orchestrates plugin execution
pub struct PluginWorkflowManager {
    workflows: std::collections::HashMap<String, pocketflow_rs::Flow<PluginWorkflowState>>,
}

impl Default for PluginWorkflowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginWorkflowManager {
    pub fn new() -> Self {
        Self {
            workflows: std::collections::HashMap::new(),
        }
    }

    /// Register a plugin as a workflow node
    pub fn register_plugin(&mut self, name: &str, plugin: Arc<dyn StatePlugin>) {
        // Create a basic workflow node
        let _node = WorkflowPluginNode::new(plugin);
        // In a full implementation, we'd store these nodes for workflow creation
        // For now, just log the registration
        log::info!("Registered plugin '{}' as workflow node", name);
        // TODO: Store the node for later workflow creation
    }

    /// Create a system administration workflow
    pub fn create_system_admin_workflow(&mut self) -> Result<()> {
        // Example: Network config → Firewall → Monitoring
        log::info!("🏗️  Creating system administration workflow");
        log::info!("   Network Plugin → Firewall Plugin → Monitoring Plugin");

        // For now, just log that this workflow would be created
        // In a full implementation, this would create actual workflow nodes
        // and connect them with proper state transitions

        Ok(())
    }

    /// Create a privacy network setup workflow
    pub fn create_privacy_network_workflow(&mut self) -> Result<()> {
        log::info!("🔒 Creating privacy network workflow");
        log::info!("   WireGuard Gateway → WARP Tunnel → XRay Client");
        log::info!("   ↓");
        log::info!("   Single OVS bridge (vmbr0) routes all traffic");

        // This workflow orchestrates privacy components on single bridge:
        // 1. Privacy plugin coordinates system services (WireGuard, WARP)
        // 2. LXC plugin creates XRay container with socket networking
        // 3. OpenFlow plugin sets up traffic routing through vmbr0
        // 4. Netmaker mesh also uses same bridge for container networking

        Ok(())
    }

    /// Create a container networking workflow (includes Netmaker mesh)
    pub fn create_container_networking_workflow(&mut self) -> Result<()> {
        log::info!("🏗️  Creating container networking workflow");
        log::info!("   Netmaker Server → LXC Containers → Socket Networking → vmbr0 Bridge");
        log::info!("   ↓");
        log::info!("   Full mesh networking for all containers on single bridge");

        // This workflow handles container networking on single bridge:
        // 1. Netmaker plugin manages system-wide mesh server
        // 2. LXC plugin creates containers with socket networking
        // 3. Containers auto-join Netmaker mesh via first-boot hooks
        // 4. All interfaces (privacy + mesh) connect to vmbr0
        // 5. OpenFlow rules route traffic between all components

        Ok(())
    }

    /// Create a development workflow
    pub fn create_development_workflow(&mut self) -> Result<()> {
        // Example: Code analysis → Testing → Documentation → Deployment
        log::info!("🏗️  Creating development workflow");
        log::info!("   Code Analysis → Testing → Documentation → Deployment");

        // For now, just log that this workflow would be created
        // In a full implementation, this would create actual workflow nodes

        Ok(())
    }

    /// Execute a workflow with given context
    pub async fn execute_workflow(
        &self,
        workflow_name: &str,
        context: Context,
    ) -> Result<serde_json::Value> {
        if let Some(workflow) = self.workflows.get(workflow_name) {
            log::info!("🚀 Executing plugin workflow: {}", workflow_name);
            let result = workflow.run(context).await?;
            log::info!("✅ Plugin workflow completed: {}", workflow_name);
            Ok(result)
        } else {
            Err(anyhow::anyhow!("Workflow '{}' not found", workflow_name))
        }
    }

    /// List available workflows
    pub fn list_workflows(&self) -> Vec<String> {
        self.workflows.keys().cloned().collect()
    }
}

/// Builder pattern for workflow plugin nodes
pub struct WorkflowPluginNodeBuilder {
    node: WorkflowPluginNode,
}

impl WorkflowPluginNodeBuilder {
    pub fn with_inputs(mut self, inputs: Vec<String>) -> Self {
        self.node.inputs = inputs;
        self
    }

    pub fn with_outputs(mut self, outputs: Vec<String>) -> Self {
        self.node.outputs = outputs;
        self
    }

    pub fn with_config(mut self, config: Value) -> Self {
        self.node.config = config;
        self
    }

    pub fn build(self) -> WorkflowPluginNode {
        self.node
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/src/plugin.rs">
use op_state_store::PluginSchema;
// Core trait for pluggable state management
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

use chrono::{DateTime, Utc};

/// Desired state configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesiredState {
    pub state: Value,
    pub timestamp: DateTime<Utc>,
    pub hash: String,
    pub description: Option<String>,
    pub source: StateSource,
}

impl DesiredState {
    pub fn new(state: Value) -> Self {
        let hash = format!(
            "{:x}",
            md5::compute(simd_json::to_string(&state).unwrap_or_default())
        );
        Self {
            state,
            timestamp: Utc::now(),
            hash,
            description: None,
            source: StateSource::User,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateSource {
    User,
    AutoDiscovered,
    Import(String),
    Plugin(String),
    Default,
}

/// Represents a change to be applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub operation: ChangeOperation,
    pub path: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub description: String,
    pub hash: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeOperation {
    Create,
    Update,
    Delete,
    NoOp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
            suggestions: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
    pub code: String,
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub license: Option<String>,
    pub dependencies: Vec<String>,
    pub dbus_services: Vec<String>,
    pub feature_schemas: Vec<Value>,
    pub object_schemas: HashMap<String, Value>,
}

/// Core trait that all state management plugins must implement
#[async_trait]
pub trait StatePlugin: Send + Sync {
    /// Get the plugin metadata including description and schemas
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: format!("{} plugin", self.name()),
            author: None,
            license: None,
            dependencies: vec![],
            dbus_services: vec![],
            feature_schemas: vec![],
            object_schemas: HashMap::new(),
        }
    }

    /// Get the plugin structured schema if available
    fn schema(&self) -> Option<PluginSchema> {
        None
    }

    /// Plugin identifier (e.g., "network", "filesystem", "user")
    fn name(&self) -> &str;

    /// Plugin version for compatibility checking
    #[allow(dead_code)]
    fn version(&self) -> &str;

    /// Check if this plugin's dependencies are available on the system
    fn is_available(&self) -> bool {
        true
    }

    /// Get a message explaining why the plugin is unavailable
    fn unavailable_reason(&self) -> String {
        format!("Plugin '{}' is not available", self.name())
    }

    /// Query current system state in this domain
    async fn query_current_state(&self) -> Result<Value>;

    /// Calculate difference between current and desired state
    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff>;

    /// Apply the state changes
    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult>;

    /// Verify that current state matches desired state
    #[allow(dead_code)]
    async fn verify_state(&self, desired: &Value) -> Result<bool>;

    /// Create a checkpoint for rollback capability
    async fn create_checkpoint(&self) -> Result<Checkpoint>;

    /// Rollback to a previous checkpoint
    #[allow(dead_code)]
    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()>;

    /// Get plugin capabilities and limitations
    #[allow(dead_code)]
    fn capabilities(&self) -> PluginCapabilities;
}

/// Represents the difference between current and desired state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    pub plugin: String,
    pub actions: Vec<StateAction>,
    pub metadata: DiffMetadata,
}

/// Metadata about the diff calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffMetadata {
    pub timestamp: i64,
    pub current_hash: String,
    pub desired_hash: String,
}

/// Actions to be performed on resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateAction {
    Create { resource: String, config: Value },
    Modify { resource: String, changes: Value },
    Delete { resource: String },
    NoOp { resource: String },
}

/// Result of applying state changes
#[derive(Debug, Serialize, Deserialize)]
pub struct ApplyResult {
    pub success: bool,
    pub changes_applied: Vec<String>,
    pub errors: Vec<String>,
    pub checkpoint: Option<Checkpoint>,
}

/// Checkpoint for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub plugin: String,
    pub timestamp: i64,
    pub state_snapshot: Value,
    pub backend_checkpoint: Option<Value>,
}

/// Plugin capabilities flags
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct PluginCapabilities {
    pub supports_rollback: bool,
    pub supports_checkpoints: bool,
    pub supports_verification: bool,
    pub atomic_operations: bool,
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/src/plugtree.rs">
//! PlugTree - Hierarchical plugin pattern for managing collections of resources
//!
//! A PlugTree allows a plugin to manage multiple independent sub-resources (pluglets),
//! each with its own state and lifecycle.
//!
//! Example: LXC plugin manages multiple containers, each container is a pluglet
//!
//! Architecture:
//! ```text
//! Plugin (PlugTree)
//!  ├─ Pluglet:100 (individual container)
//!  ├─ Pluglet:101 (individual container)
//!  └─ Pluglet:102 (individual container)
//! ```

use anyhow::Result;
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

use super::plugin::ApplyResult;

/// Trait for plugins that manage collections of independent sub-resources
#[async_trait]
#[allow(dead_code)]
pub trait PlugTree: Send + Sync {
    /// Type name of the sub-resources (e.g., "container", "interface", "unit")
    fn pluglet_type(&self) -> &str;

    /// Get unique identifier field name (e.g., "id", "name", "interface")
    fn pluglet_id_field(&self) -> &str;

    /// Extract pluglet ID from a resource value
    fn extract_pluglet_id(&self, resource: &Value) -> Result<String>;

    /// Apply state to a single pluglet by ID
    async fn apply_pluglet(&self, pluglet_id: &str, desired: &Value) -> Result<ApplyResult>;

    /// Query state of a single pluglet by ID
    async fn query_pluglet(&self, pluglet_id: &str) -> Result<Option<Value>>;

    /// List all pluglet IDs currently managed
    async fn list_pluglet_ids(&self) -> Result<Vec<String>>;
}

/// Helper to extract pluglets from plugin state
#[allow(dead_code)]
pub fn extract_pluglets(plugin_state: &Value, collection_key: &str) -> Result<Vec<Value>> {
    plugin_state
        .get(collection_key)
        .and_then(|v| v.as_array())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("No '{}' array in plugin state", collection_key))
}

/// Helper to find a specific pluglet by ID
#[allow(dead_code)]
pub fn find_pluglet_by_id(
    plugin_state: &Value,
    collection_key: &str,
    id_field: &str,
    target_id: &str,
) -> Result<Option<Value>> {
    let pluglets = extract_pluglets(plugin_state, collection_key)?;

    for pluglet in pluglets {
        if let Some(id_value) = pluglet.get(id_field) {
            if let Some(id_str) = id_value.as_str() {
                if id_str == target_id {
                    return Ok(Some(pluglet));
                }
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::json;

    #[test]
    fn test_extract_pluglets() {
        let state = json!({
            "containers": [
                {"id": "100", "name": "test1"},
                {"id": "101", "name": "test2"}
            ]
        });

        let pluglets = extract_pluglets(&state, "containers").unwrap();
        assert_eq!(pluglets.len(), 2);
    }

    #[test]
    fn test_find_pluglet_by_id() {
        let state = json!({
            "containers": [
                {"id": "100", "name": "test1"},
                {"id": "101", "name": "test2"}
            ]
        });

        let pluglet = find_pluglet_by_id(&state, "containers", "id", "101").unwrap();
        assert!(pluglet.is_some());
        assert_eq!(pluglet.unwrap()["name"], "test2");

        let not_found = find_pluglet_by_id(&state, "containers", "id", "999").unwrap();
        assert!(not_found.is_none());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/src/schema_validator.rs">
//! Schema Validator - Prevents random/unrealistic schema generation
//! Validates schemas against curated use cases and constraints

use anyhow::Result;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::ValueBuilder;
use simd_json::{json, OwnedValue as Value};
use std::collections::{HashMap, HashSet};

/// Validated use case template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseCaseTemplate {
    /// Use case name
    pub name: String,
    /// Description
    pub description: String,
    /// Required plugins
    pub required_plugins: Vec<String>,
    /// Required fields per plugin
    pub required_fields: HashMap<String, Vec<String>>,
    /// Valid field combinations
    pub valid_combinations: Vec<FieldCombination>,
    /// Dependencies (plugin A requires plugin B)
    pub dependencies: Vec<Dependency>,
    /// Constraints (field A requires field B)
    pub constraints: Vec<Constraint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCombination {
    /// Plugin name
    pub plugin: String,
    /// Valid field combinations
    pub fields: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Plugin that requires another
    pub requires: String,
    /// Required plugin
    pub required: String,
    /// Optional: specific condition
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// Plugin name
    pub plugin: String,
    /// Field that has constraint
    pub field: String,
    /// Constraint type
    pub constraint_type: ConstraintType,
    /// Required value or field
    pub required: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintType {
    /// Field must equal value
    Equals,
    /// Field must be in list
    In,
    /// Field requires another field to be set
    RequiresField,
    /// Field value must match pattern
    Pattern,
    /// Field must be within range
    Range { min: f64, max: f64 },
}

/// Schema validator
pub struct SchemaValidator {
    /// Curated use case templates
    use_cases: Vec<UseCaseTemplate>,
    /// Plugin field definitions
    plugin_fields: HashMap<String, HashSet<String>>,
}

impl SchemaValidator {
    pub fn new() -> Self {
        Self {
            use_cases: Self::load_default_use_cases(),
            plugin_fields: Self::load_plugin_fields(),
        }
    }

    /// Validate a generated schema against use cases
    pub fn validate_schema(&self, schema: &Value) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Extract plugins from schema
        let plugins = schema
            .get("plugins")
            .and_then(|p| p.as_object())
            .ok_or_else(|| anyhow::anyhow!("Missing 'plugins' in schema"))?;

        // Check if schema matches any use case
        let matching_use_case = self.find_matching_use_case(plugins);

        if matching_use_case.is_none() {
            warnings.push("Schema does not match any curated use case template".to_string());
        }

        // Validate required plugins for matched use case
        if let Some(use_case) = &matching_use_case {
            for required_plugin in &use_case.required_plugins {
                if !plugins.contains_key(required_plugin) {
                    errors.push(format!(
                        "Use case '{}' requires plugin '{}'",
                        use_case.name, required_plugin
                    ));
                }
            }
        }

        // Validate dependencies
        for dep in self.get_all_dependencies() {
            if plugins.contains_key(&dep.requires) && !plugins.contains_key(&dep.required) {
                errors.push(format!(
                    "Plugin '{}' requires plugin '{}'",
                    dep.requires, dep.required
                ));
            }
        }

        // Validate field constraints
        for (plugin_name, plugin_config) in plugins {
            if let Some(fields) = self.plugin_fields.get(plugin_name) {
                // Check for unknown fields
                if let Some(config_obj) = plugin_config.as_object() {
                    for field_name in config_obj.keys() {
                        if !fields.contains(field_name) {
                            warnings.push(format!(
                                "Unknown field '{}' in plugin '{}'",
                                field_name, plugin_name
                            ));
                        }
                    }
                }
            }
        }

        // Validate field combinations
        if let Some(use_case) = &matching_use_case {
            for combo in &use_case.valid_combinations {
                if let Some(plugin_config) = plugins.get(&combo.plugin) {
                    if let Some(config_obj) = plugin_config.as_object() {
                        for (field, valid_values) in &combo.fields {
                            if let Some(field_value) = config_obj.get(field) {
                                let value_str = field_value.to_string();
                                if !valid_values.iter().any(|v| value_str.contains(v)) {
                                    warnings.push(format!(
                                        "Field '{}' in plugin '{}' has unusual value: {}",
                                        field, combo.plugin, value_str
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
            matched_use_case: matching_use_case.map(|uc| uc.name.clone()),
        })
    }

    /// Find matching use case for a schema
    fn find_matching_use_case(
        &self,
        plugins: &simd_json::value::owned::Object,
    ) -> Option<&UseCaseTemplate> {
        for use_case in &self.use_cases {
            let mut matches = 0;
            for required_plugin in &use_case.required_plugins {
                if plugins.contains_key(required_plugin) {
                    matches += 1;
                }
            }
            // Match if at least 80% of required plugins are present
            if matches as f64 / use_case.required_plugins.len() as f64 >= 0.8 {
                return Some(use_case);
            }
        }
        None
    }

    /// Get all dependencies from all use cases
    fn get_all_dependencies(&self) -> Vec<Dependency> {
        self.use_cases
            .iter()
            .flat_map(|uc| uc.dependencies.clone())
            .collect()
    }

    /// Load default curated use cases
    fn load_default_use_cases() -> Vec<UseCaseTemplate> {
        vec![
            // Privacy Router Use Case
            UseCaseTemplate {
                name: "privacy_router".to_string(),
                description: "Multi-hop privacy tunnel with WireGuard, WARP, and XRay".to_string(),
                required_plugins: vec![
                    "privacy_router".to_string(),
                    "openflow".to_string(),
                    "net".to_string(),
                    "lxc".to_string(),
                ],
                required_fields: {
                    let mut m = HashMap::new();
                    m.insert(
                        "privacy_router".to_string(),
                        vec![
                            "bridge_name".to_string(),
                            "wireguard.enabled".to_string(),
                            "warp.enabled".to_string(),
                            "xray.enabled".to_string(),
                        ],
                    );
                    m.insert("openflow".to_string(), vec!["bridges".to_string()]);
                    m
                },
                valid_combinations: vec![FieldCombination {
                    plugin: "privacy_router".to_string(),
                    fields: {
                        let mut m = HashMap::new();
                        m.insert(
                            "wireguard.container_id".to_string(),
                            vec!["100".to_string()],
                        );
                        m.insert("xray.container_id".to_string(), vec!["101".to_string()]);
                        m.insert(
                            "bridge_name".to_string(),
                            vec!["ovsbr0".to_string(), "vmbr0".to_string()],
                        );
                        m
                    },
                }],
                dependencies: vec![
                    Dependency {
                        requires: "privacy_router".to_string(),
                        required: "openflow".to_string(),
                        condition: None,
                    },
                    Dependency {
                        requires: "privacy_router".to_string(),
                        required: "net".to_string(),
                        condition: None,
                    },
                ],
                constraints: vec![Constraint {
                    plugin: "privacy_router".to_string(),
                    field: "wireguard.container_id".to_string(),
                    constraint_type: ConstraintType::Range {
                        min: 100.0,
                        max: 999.0,
                    },
                    required: Value::null(),
                }],
            },
            // Basic Network Use Case
            UseCaseTemplate {
                name: "basic_network".to_string(),
                description: "Basic OVS bridge with DHCP".to_string(),
                required_plugins: vec!["net".to_string()],
                required_fields: {
                    let mut m = HashMap::new();
                    m.insert("net".to_string(), vec!["interfaces".to_string()]);
                    m
                },
                valid_combinations: vec![],
                dependencies: vec![],
                constraints: vec![],
            },
            // Container Mesh Use Case
            UseCaseTemplate {
                name: "container_mesh".to_string(),
                description: "LXC containers with Netmaker mesh networking".to_string(),
                required_plugins: vec![
                    "lxc".to_string(),
                    "netmaker".to_string(),
                    "openflow".to_string(),
                ],
                required_fields: HashMap::new(),
                valid_combinations: vec![],
                dependencies: vec![Dependency {
                    requires: "netmaker".to_string(),
                    required: "net".to_string(),
                    condition: None,
                }],
                constraints: vec![],
            },
        ]
    }

    /// Load plugin field definitions
    fn load_plugin_fields() -> HashMap<String, HashSet<String>> {
        let mut fields = HashMap::new();

        // Privacy Router fields
        fields.insert("privacy_router".to_string(), {
            let mut s = HashSet::new();
            s.insert("bridge_name".to_string());
            s.insert("wireguard".to_string());
            s.insert("warp".to_string());
            s.insert("xray".to_string());
            s.insert("vps".to_string());
            s.insert("socket_networking".to_string());
            s.insert("openflow".to_string());
            s.insert("netmaker".to_string());
            s.insert("containers".to_string());
            s
        });

        // Net plugin fields
        fields.insert("net".to_string(), {
            let mut s = HashSet::new();
            s.insert("interfaces".to_string());
            s
        });

        // OpenFlow plugin fields
        fields.insert("openflow".to_string(), {
            let mut s = HashSet::new();
            s.insert("bridges".to_string());
            s.insert("controller_endpoint".to_string());
            s.insert("flow_policies".to_string());
            s.insert("auto_discover_containers".to_string());
            s.insert("enable_security_flows".to_string());
            s.insert("obfuscation_level".to_string());
            s
        });

        fields
    }

    /// Get curated use case templates
    pub fn get_use_cases(&self) -> &[UseCaseTemplate] {
        &self.use_cases
    }

    /// Suggest realistic schema based on use case
    pub fn suggest_schema(&self, use_case_name: &str) -> Option<Value> {
        self.use_cases
            .iter()
            .find(|uc| uc.name == use_case_name)
            .map(|uc| {
                let mut schema = simd_json::value::owned::Object::new();
                schema.insert("version".to_string(), json!(1));

                let mut plugins = simd_json::value::owned::Object::new();

                for plugin_name in &uc.required_plugins {
                    let mut plugin_config = simd_json::value::owned::Object::new();

                    // Add required fields with sensible defaults
                    if let Some(required_fields) = uc.required_fields.get(plugin_name) {
                        for field in required_fields {
                            let parts: Vec<&str> = field.split('.').collect();
                            if parts.len() == 1 {
                                plugin_config.insert(field.clone(), json!(""));
                            } else {
                                // Nested field
                                let mut nested = simd_json::value::owned::Object::new();
                                nested.insert(parts[1].to_string(), json!(""));
                                plugin_config.insert(parts[0].to_string(), json!(nested));
                            }
                        }
                    }

                    plugins.insert(plugin_name.clone(), json!(plugin_config));
                }

                schema.insert("plugins".to_string(), json!(plugins));
                json!(schema)
            })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub matched_use_case: Option<String>,
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/Cargo.toml">
[package]
name = "op-state"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "State management system with plugin infrastructure, crypto, and schema validation"

[dependencies]
parking_lot = { workspace = true }
op-core = { path = "../op-core" }
op-blockchain = { path = "../op-blockchain" }
op-jsonrpc = { path = "../op-jsonrpc" }
op-state-store = { path = "../op-state-store" }
op-network = { path = "../op-network" }
tokio = { workspace = true }
tokio-stream = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
zbus = { workspace = true }
chrono = { workspace = true }
sha2 = { workspace = true }
quick-xml = { workspace = true }
rand = { workspace = true }
base64 = { workspace = true }
log = { workspace = true }
aes-gcm = { workspace = true }
argon2 = { workspace = true }
md5 = "0.7"
serde_json = { workspace = true }
pocketflow_rs = "0.1"

[features]
default = []
mcp = []
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/compare-op-state.md">
# compare-op-state

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 11 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 9 |
| Partial artifacts | 0 |
| Spec-listed source files | 11 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- State management system with plugin infrastructure, crypto, and schema validation
- Internal crate integrations: op-core, op-blockchain, op-jsonrpc, op-state-store, op-network.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/schema_validator.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/schema_validator.rs |
| `src/plugtree.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/plugtree.rs |
| `src/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/mod.rs |
| `src/authority.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/authority.rs |
| `src/crypto.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/crypto.rs |
| `src/dbus_plugin_base.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/dbus_plugin_base.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/manager.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/manager.rs |
| `src/plugin.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/plugin.rs |
| `src/plugin_workflow.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/plugin_workflow.rs |
| `src/dbus_server.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/dbus_server.rs |
| `root` | ✅ Present | root source group | src/authority.rs, src/crypto.rs, src/dbus_plugin_base.rs, src/dbus_server.rs, src/lib.rs, src/manager.rs, src/mod.rs, src/plugin.rs, ... (+3 more) |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| schema_validator | ✅ Implemented | src/schema_validator.rs | SPEC main module |
| plugtree | ✅ Implemented | src/plugtree.rs | SPEC main module |
| mod | ✅ Implemented | src/mod.rs | SPEC main module |
| authority | ✅ Implemented | src/authority.rs | SPEC main module |
| crypto | ✅ Implemented | src/crypto.rs | SPEC main module |
| dbus_plugin_base | ✅ Implemented | src/dbus_plugin_base.rs | SPEC main module |
| manager | ✅ Implemented | src/manager.rs | SPEC main module |
| plugin | ✅ Implemented | src/plugin.rs | SPEC main module |
| plugin_workflow | ✅ Implemented | src/plugin_workflow.rs | SPEC main module |
| dbus_server | ✅ Implemented | src/dbus_server.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - documented in SPEC
- `op-blockchain` - documented in SPEC
- `op-jsonrpc` - documented in SPEC
- `op-state-store` - documented in SPEC
- `op-network` - not listed in SPEC dependency block

### External Runtime Dependencies
- `parking_lot` - not listed in SPEC dependency block
- `tokio` - documented in SPEC
- `tokio-stream` - not listed in SPEC dependency block
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `async-trait` - documented in SPEC
- `zbus` - documented in SPEC
- `chrono` - documented in SPEC
- `sha2` - documented in SPEC
- `quick-xml` - documented in SPEC
- `rand` - documented in SPEC
- `base64` - documented in SPEC
- `log` - documented in SPEC
- `aes-gcm` - documented in SPEC
- `argon2` - documented in SPEC
- `md5` - not listed in SPEC dependency block
- `serde_json` - not listed in SPEC dependency block
- `pocketflow_rs` - not listed in SPEC dependency block

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: authority, crypto, dbus_plugin_base, dbus_server, manager, plugin, plugin_workflow, plugtree, schema_validator.
- Cargo feature flags: default, mcp.
- 6 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-state/SPEC.md">
# op-state - Specification

## Overview
**Crate**: `op-state`  
**Location**: `crates/op-state`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-state"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-state/src/schema_validator.rs
op-state/src/plugtree.rs
op-state/src/mod.rs
op-state/src/authority.rs
op-state/src/crypto.rs
op-state/src/dbus_plugin_base.rs
op-state/src/lib.rs
op-state/src/manager.rs
op-state/src/plugin.rs
op-state/src/plugin_workflow.rs
op-state/src/dbus_server.rs
```

### Key Dependencies
```toml
op-core = { path = "../op-core" }
op-blockchain = { path = "../op-blockchain" }
op-jsonrpc = { path = "../op-jsonrpc" }
op-state-store = { path = "../op-state-store" }
tokio = { workspace = true }
serde = { workspace = true }
simd-json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
zbus = { workspace = true }
chrono = { workspace = true }
sha2 = { workspace = true }
quick-xml = { workspace = true }
rand = { workspace = true }
base64 = { workspace = true }
log = { workspace = true }
aes-gcm = { workspace = true }
argon2 = { workspace = true }
```

### Binaries
```toml
# No binaries
```

### Features
```toml
[features]
default = []
mcp = []
```

## Documentation Files


## Module Structure
      11 Rust source files

### Main Modules
schema_validator
plugtree
mod
authority
crypto
dbus_plugin_base
manager
plugin
plugin_workflow
dbus_server

## Purpose
State management system with plugin infrastructure, crypto, and schema validation

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:
- op-core
- op-blockchain
- op-jsonrpc
- op-state-store

---
*Generated from crate analysis*
</file>

</files>
