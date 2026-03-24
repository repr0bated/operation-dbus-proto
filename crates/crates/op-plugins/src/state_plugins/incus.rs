//! Incus state plugin - manages Incus containers and virtual machines.
//!
//! Uses the `incus` CLI with `--format=json` for all operations.
//! Supports creating, starting, stopping, and deleting instances,
//! as well as profile and config management.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;

/// Top-level state representing all Incus instances on the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncusState {
    pub instances: Vec<IncusInstance>,
}

/// A single Incus instance (container or virtual-machine).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncusInstance {
    pub name: String,
    /// Instance status: "Running", "Stopped", "Frozen"
    pub status: String,
    /// Instance type: "container" or "virtual-machine"
    #[serde(rename = "type")]
    pub instance_type: String,
    /// Image description (extracted from config)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Preferred storage pool used during initial creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_pool: Option<String>,
    /// Applied profiles (e.g. ["default"])
    #[serde(default)]
    pub profiles: Vec<String>,
    /// Instance configuration key-value pairs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, String>>,
    /// Device definitions (device name -> device key-value config)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<HashMap<String, HashMap<String, String>>>,
}

/// Intermediate struct for deserializing raw `incus list --format=json` output.
/// The CLI returns more fields than we need; this captures the relevant ones.
#[derive(Debug, Deserialize)]
struct RawIncusInstance {
    name: String,
    status: String,
    #[serde(rename = "type")]
    instance_type: String,
    #[serde(default)]
    profiles: Vec<String>,
    #[serde(default)]
    config: HashMap<String, String>,
    #[serde(default)]
    devices: HashMap<String, HashMap<String, String>>,
}

pub struct IncusPlugin;

impl IncusPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Run an incus CLI command and return its stdout as bytes.
    async fn run_incus_command(args: &[&str]) -> Result<Vec<u8>> {
        let output = tokio::process::Command::new("/usr/bin/incus")
            .args(args)
            .output()
            .await
            .context("Failed to execute incus command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "incus {} failed (exit {}): {}",
                args.join(" "),
                output.status.code().unwrap_or(-1),
                stderr.trim()
            );
        }

        Ok(output.stdout)
    }

    /// Parse raw JSON output from `incus list --format=json` into IncusInstance structs.
    fn parse_instance_list(mut raw_json: Vec<u8>) -> Result<Vec<IncusInstance>> {
        let raw_instances: Vec<RawIncusInstance> =
            simd_json::from_slice(&mut raw_json).context("Failed to parse incus list JSON")?;

        let instances = raw_instances
            .into_iter()
            .map(|raw| {
                let storage_pool = raw
                    .devices
                    .get("root")
                    .and_then(|root| root.get("pool"))
                    .cloned();
                // Extract image description from config keys
                let image = raw
                    .config
                    .get("image.description")
                    .or_else(|| raw.config.get("volatile.base_image"))
                    .cloned();

                // Only include config if non-empty
                let config = if raw.config.is_empty() {
                    None
                } else {
                    Some(raw.config)
                };

                // Only include devices if non-empty
                let devices = if raw.devices.is_empty() {
                    None
                } else {
                    Some(raw.devices)
                };

                IncusInstance {
                    name: raw.name,
                    status: raw.status,
                    instance_type: raw.instance_type,
                    image,
                    storage_pool,
                    profiles: raw.profiles,
                    config,
                    devices,
                }
            })
            .collect();

        Ok(instances)
    }

    /// Apply a single Create action for an instance.
    async fn apply_create(instance: &IncusInstance) -> Result<Vec<String>> {
        let mut changes = Vec::new();
        let name = &instance.name;

        // Determine the image to use; fall back to a sensible default
        let image = instance.image.as_deref().unwrap_or("images:debian/12");

        let mut create_args = vec!["init".to_string(), image.to_string(), name.to_string()];
        if let Some(pool) = instance.storage_pool.as_deref() {
            create_args.push("--storage".to_string());
            create_args.push(pool.to_string());
        }
        if instance.profiles.is_empty() {
            create_args.push("--no-profiles".to_string());
        } else {
            for profile in Self::normalize_profiles(&instance.profiles) {
                create_args.push("--profile".to_string());
                create_args.push(profile);
            }
        }
        let create_args_ref: Vec<&str> = create_args.iter().map(String::as_str).collect();
        log::info!("Creating instance '{}' from image '{}'", name, image);
        Self::run_incus_command(&create_args_ref)
            .await
            .with_context(|| format!("Failed to create instance '{}'", name))?;
        changes.push(format!("Created instance '{}'", name));

        changes.extend(Self::sync_profiles(name, None, instance).await?);
        changes.extend(Self::sync_config(name, None, instance).await?);
        changes.extend(Self::sync_devices(name, None, instance).await?);
        changes.extend(Self::sync_status(name, None, instance).await?);

        Ok(changes)
    }

    async fn apply_modify(current: &IncusInstance, desired: &IncusInstance) -> Result<Vec<String>> {
        let mut changes = Vec::new();
        changes.extend(Self::sync_profiles(&desired.name, Some(current), desired).await?);
        changes.extend(Self::sync_config(&desired.name, Some(current), desired).await?);
        changes.extend(Self::sync_devices(&desired.name, Some(current), desired).await?);
        changes.extend(Self::sync_status(&desired.name, Some(current), desired).await?);
        Ok(changes)
    }

    /// Apply a single Delete action.
    async fn apply_delete(name: &str) -> Result<Vec<String>> {
        log::info!("Force-deleting instance '{}'", name);
        Self::run_incus_command(&["delete", name, "--force"])
            .await
            .with_context(|| format!("Failed to delete instance '{}'", name))?;
        Ok(vec![format!("Deleted instance '{}'", name)])
    }

    fn is_read_only_config_key(key: &str) -> bool {
        key.starts_with("volatile.") || key.starts_with("image.")
    }

    fn normalize_profiles(profiles: &[String]) -> Vec<String> {
        let mut normalized = profiles.to_vec();
        normalized.sort();
        normalized.dedup();
        normalized
    }

    fn normalized_config(instance: &IncusInstance) -> HashMap<String, String> {
        instance
            .config
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|(key, _)| !Self::is_read_only_config_key(key))
            .collect()
    }

    fn managed_devices(instance: &IncusInstance) -> HashMap<String, HashMap<String, String>> {
        instance
            .devices
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|(name, _)| name != "root")
            .collect()
    }

    fn instances_equivalent(current: &IncusInstance, desired: &IncusInstance) -> bool {
        current.status == desired.status
            && current.instance_type == desired.instance_type
            && current.storage_pool == desired.storage_pool
            && Self::normalize_profiles(&current.profiles)
                == Self::normalize_profiles(&desired.profiles)
            && Self::normalized_config(current) == Self::normalized_config(desired)
            && Self::managed_devices(current) == Self::managed_devices(desired)
    }

    async fn sync_profiles(
        name: &str,
        current: Option<&IncusInstance>,
        desired: &IncusInstance,
    ) -> Result<Vec<String>> {
        let current_profiles = current
            .map(|instance| Self::normalize_profiles(&instance.profiles))
            .unwrap_or_default();
        let desired_profiles = Self::normalize_profiles(&desired.profiles);
        let mut changes = Vec::new();

        for profile in &current_profiles {
            if desired_profiles.contains(profile) {
                continue;
            }
            Self::run_incus_command(&["profile", "remove", name, profile])
                .await
                .with_context(|| {
                    format!("Failed to remove profile '{}' from '{}'", profile, name)
                })?;
            changes.push(format!("Removed profile '{}' from '{}'", profile, name));
        }

        for profile in &desired_profiles {
            if current_profiles.contains(profile) {
                continue;
            }
            Self::run_incus_command(&["profile", "add", name, profile])
                .await
                .with_context(|| format!("Failed to add profile '{}' to '{}'", profile, name))?;
            changes.push(format!("Added profile '{}' to '{}'", profile, name));
        }

        Ok(changes)
    }

    async fn sync_config(
        name: &str,
        current: Option<&IncusInstance>,
        desired: &IncusInstance,
    ) -> Result<Vec<String>> {
        let current_config = current.map(Self::normalized_config).unwrap_or_default();
        let desired_config = Self::normalized_config(desired);
        let mut changes = Vec::new();

        for key in current_config.keys() {
            if !desired_config.contains_key(key) {
                Self::run_incus_command(&["config", "unset", name, key])
                    .await
                    .with_context(|| format!("Failed to unset config '{}' on '{}'", key, name))?;
                changes.push(format!("Unset config '{}' on '{}'", key, name));
            }
        }

        for (key, value) in desired_config {
            if current_config.get(&key) == Some(&value) {
                continue;
            }
            let kv = format!("{}={}", key, value);
            Self::run_incus_command(&["config", "set", name, &kv])
                .await
                .with_context(|| format!("Failed to set config '{}' on '{}'", kv, name))?;
            changes.push(format!("Set config '{}' on '{}'", kv, name));
        }

        Ok(changes)
    }

    async fn sync_devices(
        name: &str,
        current: Option<&IncusInstance>,
        desired: &IncusInstance,
    ) -> Result<Vec<String>> {
        let current_devices = current.map(Self::managed_devices).unwrap_or_default();
        let desired_devices = Self::managed_devices(desired);
        let mut changes = Vec::new();

        for device_name in current_devices.keys() {
            if desired_devices.contains_key(device_name) {
                continue;
            }
            Self::run_incus_command(&["config", "device", "remove", name, device_name])
                .await
                .with_context(|| {
                    format!(
                        "Failed to remove stale device '{}' from '{}'",
                        device_name, name
                    )
                })?;
            changes.push(format!(
                "Removed stale device '{}' from '{}'",
                device_name, name
            ));
        }

        for (device_name, desired_device) in desired_devices {
            if current_devices.get(&device_name) == Some(&desired_device) {
                continue;
            }

            if current_devices.contains_key(&device_name) {
                Self::run_incus_command(&["config", "device", "remove", name, &device_name])
                    .await
                    .with_context(|| {
                        format!("Failed to remove device '{}' from '{}'", device_name, name)
                    })?;
                changes.push(format!("Removed device '{}' from '{}'", device_name, name));
            }

            let device_type = desired_device
                .get("type")
                .cloned()
                .context("Incus device definition is missing required 'type'")?;
            let mut add_args = vec![
                "config".to_string(),
                "device".to_string(),
                "add".to_string(),
                name.to_string(),
                device_name.clone(),
                device_type,
            ];
            for (key, value) in desired_device {
                if key == "type" {
                    continue;
                }
                add_args.push(format!("{}={}", key, value));
            }
            let add_args_ref: Vec<&str> = add_args.iter().map(String::as_str).collect();
            Self::run_incus_command(&add_args_ref)
                .await
                .with_context(|| format!("Failed to add device '{}' to '{}'", device_name, name))?;
            changes.push(format!("Configured device '{}' on '{}'", device_name, name));
        }

        Ok(changes)
    }

    async fn sync_status(
        name: &str,
        current: Option<&IncusInstance>,
        desired: &IncusInstance,
    ) -> Result<Vec<String>> {
        let mut changes = Vec::new();
        if current.map(|instance| instance.status.as_str()) == Some(desired.status.as_str()) {
            return Ok(changes);
        }
        match desired.status.as_str() {
            "Running" => {
                Self::run_incus_command(&["start", name])
                    .await
                    .with_context(|| format!("Failed to start instance '{}'", name))?;
                changes.push(format!("Started instance '{}'", name));
            }
            "Stopped" => {
                Self::run_incus_command(&["stop", name])
                    .await
                    .with_context(|| format!("Failed to stop instance '{}'", name))?;
                changes.push(format!("Stopped instance '{}'", name));
            }
            "Frozen" => {
                Self::run_incus_command(&["pause", name])
                    .await
                    .with_context(|| format!("Failed to freeze instance '{}'", name))?;
                changes.push(format!("Frozen instance '{}'", name));
            }
            other => anyhow::bail!(
                "Unsupported desired status '{}' for instance '{}'",
                other,
                name
            ),
        }
        Ok(changes)
    }
}

impl Default for IncusPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for IncusPlugin {
    fn name(&self) -> &str {
        "incus"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/usr/bin/incus").exists()
    }

    fn unavailable_reason(&self) -> String {
        "Incus not installed (/usr/bin/incus not found)".to_string()
    }

    async fn query_current_state(&self) -> Result<Value> {
        log::info!("Querying current Incus instance state");

        let stdout = Self::run_incus_command(&["list", "--format=json"])
            .await
            .context("Failed to list Incus instances")?;

        let instances = Self::parse_instance_list(stdout)?;
        log::info!("Discovered {} Incus instance(s)", instances.len());

        let state = IncusState { instances };
        simd_json::serde::to_owned_value(state).context("Failed to serialize IncusState")
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_state: IncusState = simd_json::serde::from_owned_value(current.clone())
            .context("Failed to deserialize current IncusState")?;
        let desired_state: IncusState = simd_json::serde::from_owned_value(desired.clone())
            .context("Failed to deserialize desired IncusState")?;

        // Index current instances by name for O(1) lookups
        let current_by_name: HashMap<&str, &IncusInstance> = current_state
            .instances
            .iter()
            .map(|i| (i.name.as_str(), i))
            .collect();

        let desired_by_name: HashMap<&str, &IncusInstance> = desired_state
            .instances
            .iter()
            .map(|i| (i.name.as_str(), i))
            .collect();

        let mut actions = Vec::new();

        // Check desired instances against current state
        for desired_inst in &desired_state.instances {
            match current_by_name.get(desired_inst.name.as_str()) {
                None => {
                    // Instance does not exist yet -- needs creation
                    let config = simd_json::serde::to_owned_value(desired_inst.clone())
                        .context("Failed to serialize desired instance for Create action")?;
                    actions.push(StateAction::Create {
                        resource: desired_inst.name.clone(),
                        config,
                    });
                }
                Some(current_inst) => {
                    if !Self::instances_equivalent(current_inst, desired_inst) {
                        let changes = simd_json::serde::to_owned_value(desired_inst.clone())
                            .context("Failed to serialize desired instance for Modify action")?;
                        actions.push(StateAction::Modify {
                            resource: desired_inst.name.clone(),
                            changes,
                        });
                    }
                }
            }
        }

        // Instances in current but not in desired should be deleted
        for current_inst in &current_state.instances {
            if !desired_by_name.contains_key(current_inst.name.as_str()) {
                actions.push(StateAction::Delete {
                    resource: current_inst.name.clone(),
                });
            }
        }

        let current_hash = format!("{:x}", md5::compute(simd_json::to_string(current)?));
        let desired_hash = format!("{:x}", md5::compute(simd_json::to_string(desired)?));

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash,
                desired_hash,
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();
        let current_state = self
            .query_current_state()
            .await
            .ok()
            .and_then(|value| simd_json::serde::from_owned_value::<IncusState>(value).ok());
        let current_by_name: HashMap<String, IncusInstance> = current_state
            .map(|state| {
                state
                    .instances
                    .into_iter()
                    .map(|instance| (instance.name.clone(), instance))
                    .collect()
            })
            .unwrap_or_default();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, config } => {
                    let instance: IncusInstance =
                        simd_json::serde::from_owned_value(config.clone())
                            .context("Failed to deserialize instance config for creation")?;

                    match Self::apply_create(&instance).await {
                        Ok(changes) => changes_applied.extend(changes),
                        Err(e) => {
                            let msg = format!("Failed to create instance '{}': {}", resource, e);
                            log::error!("{}", msg);
                            errors.push(msg);
                        }
                    }
                }
                StateAction::Modify { resource, changes } => {
                    let desired: IncusInstance =
                        simd_json::serde::from_owned_value(changes.clone())
                            .context("Failed to deserialize instance config for modification")?;

                    match current_by_name.get(resource) {
                        Some(current) => match Self::apply_modify(current, &desired).await {
                            Ok(applied) => changes_applied.extend(applied),
                            Err(e) => {
                                let msg =
                                    format!("Failed to modify instance '{}': {}", resource, e);
                                log::error!("{}", msg);
                                errors.push(msg);
                            }
                        },
                        None => {
                            let msg = format!(
                                "Failed to modify instance '{}': current instance not found",
                                resource
                            );
                            log::error!("{}", msg);
                            errors.push(msg);
                        }
                    }
                }
                StateAction::Delete { resource } => match Self::apply_delete(resource).await {
                    Ok(applied) => changes_applied.extend(applied),
                    Err(e) => {
                        let msg = format!("Failed to delete instance '{}': {}", resource, e);
                        log::error!("{}", msg);
                        errors.push(msg);
                    }
                },
                StateAction::NoOp { .. } => {}
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        log::info!("Verifying Incus state matches desired");
        let current = self.query_current_state().await?;
        let diff = self.calculate_diff(&current, desired).await?;
        let in_sync = diff.actions.is_empty();

        if in_sync {
            log::info!("Incus state is in sync with desired state");
        } else {
            log::warn!(
                "Incus state drift detected: {} action(s) needed",
                diff.actions.len()
            );
        }

        Ok(in_sync)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        log::info!("Creating Incus state checkpoint");
        let state = self.query_current_state().await?;
        let id = format!("incus-{}", chrono::Utc::now().timestamp());

        Ok(Checkpoint {
            id,
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        log::info!("Rolling back Incus state to checkpoint '{}'", checkpoint.id);

        let current = self.query_current_state().await?;
        let diff = self
            .calculate_diff(&current, &checkpoint.state_snapshot)
            .await?;

        if diff.actions.is_empty() {
            log::info!("No rollback actions needed -- state already matches checkpoint");
            return Ok(());
        }

        let result = self.apply_state(&diff).await?;
        if result.success {
            log::info!(
                "Rollback to checkpoint '{}' completed successfully ({} change(s))",
                checkpoint.id,
                result.changes_applied.len()
            );
        } else {
            log::error!(
                "Rollback to checkpoint '{}' completed with errors: {:?}",
                checkpoint.id,
                result.errors
            );
            anyhow::bail!(
                "Rollback had {} error(s): {}",
                result.errors.len(),
                result.errors.join("; ")
            );
        }

        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instances_equivalent_detects_config_and_device_changes() {
        let current = IncusInstance {
            name: "privacy-user-1".to_string(),
            status: "Running".to_string(),
            instance_type: "container".to_string(),
            image: Some("images:alpine/3.19".to_string()),
            storage_pool: Some("registration".to_string()),
            profiles: vec!["default".to_string()],
            config: Some(HashMap::from([(
                "user.opdbus.route_id".to_string(),
                "route-a".to_string(),
            )])),
            devices: Some(HashMap::from([(
                "privacy0".to_string(),
                HashMap::from([
                    ("type".to_string(), "nic".to_string()),
                    ("nictype".to_string(), "bridged".to_string()),
                    ("parent".to_string(), "ovsbr0".to_string()),
                ]),
            )])),
        };
        let mut desired = current.clone();
        assert!(IncusPlugin::instances_equivalent(&current, &desired));

        desired.config = Some(HashMap::from([(
            "user.opdbus.route_id".to_string(),
            "route-b".to_string(),
        )]));
        assert!(!IncusPlugin::instances_equivalent(&current, &desired));

        desired = current.clone();
        desired.devices = Some(HashMap::from([(
            "privacy0".to_string(),
            HashMap::from([
                ("type".to_string(), "nic".to_string()),
                ("nictype".to_string(), "bridged".to_string()),
                ("parent".to_string(), "ovsbr1".to_string()),
            ]),
        )]));
        assert!(!IncusPlugin::instances_equivalent(&current, &desired));
    }
}
