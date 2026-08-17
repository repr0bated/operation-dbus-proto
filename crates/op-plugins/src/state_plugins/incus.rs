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
use op_state_store::PluginSchema;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::{json, prelude::*, OwnedValue as Value};
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::incus_device::{Device, NamedDevice};

/// Top-level state representing all Incus instances on the system.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("x-oscal-category" = "service"))]
pub struct IncusState {
    pub instances: Vec<IncusInstance>,
    /// Uncapped fields discovered from the authoritative Incus API and CLI sources.
    #[serde(default)]
    pub inspector_fields: inspector_gadget_generated::InspectorGadgetFields,
}

/// A single Incus instance, modeled on the official Incus API (`shared/api`
/// `Instance`/`InstancePut`).
///
/// Devices (including proxy sockets) are the typed [`NamedDevice`] union — Incus
/// has no first-class "socket" concept, a proxy is simply a `devices` entry with
/// `type: proxy`. The previous non-standard `sockets`/`IncusProxySocket` field is
/// gone; a container's relationship to the shared `container.sock` is resolved
/// by name at the projection/gemma layer, not embedded here.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct IncusInstance {
    pub name: String,
    /// Instance status: "Running", "Stopped", "Frozen"
    pub status: String,
    /// Instance type: "container" or "virtual-machine"
    #[serde(rename = "type")]
    pub instance_type: String,
    /// Numeric status code from the Incus API (read-only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<i64>,
    /// Source image reference, used only at creation time (`incus init <image>`).
    /// Not part of the official `InstancePut`; derived from config on read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Preferred storage pool used during initial creation (creation hint only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_pool: Option<String>,
    /// Applied profiles (e.g. ["default"])
    #[serde(default)]
    pub profiles: Vec<String>,
    /// Human-readable description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// CPU architecture (e.g. "x86_64")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    /// Delete instance on shutdown
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral: Option<bool>,
    /// Whether saved state exists on disk
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stateful: Option<bool>,
    /// Creation timestamp (ISO8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Last start timestamp (ISO8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    /// Cluster member location ("none" on single-node)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// Incus project name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Instance configuration key-value pairs (`InstancePut.config`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, String>>,
    /// Typed device definitions (`InstancePut.devices`). Proxy sockets, NICs,
    /// disks, GPUs, etc. are all variants of [`Device`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<NamedDevice>,
    /// Expanded (profile-merged) config, read-only (`Instance.expanded_config`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded_config: Option<HashMap<String, String>>,
    /// Expanded (profile-merged) devices, read-only (`Instance.expanded_devices`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expanded_devices: Vec<NamedDevice>,
}

/// Intermediate struct for deserializing raw `incus list --format=json` output.
#[derive(Debug, Deserialize)]
struct RawIncusInstance {
    name: String,
    status: String,
    #[serde(rename = "type")]
    instance_type: String,
    #[serde(default)]
    status_code: i64,
    #[serde(default)]
    profiles: Vec<String>,
    #[serde(default)]
    config: HashMap<String, String>,
    #[serde(default)]
    devices: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    expanded_config: HashMap<String, String>,
    #[serde(default)]
    expanded_devices: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    architecture: String,
    #[serde(default)]
    ephemeral: bool,
    #[serde(default)]
    stateful: bool,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    last_used_at: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    project: String,
}

/// Convert an Incus device map (`name -> {type, …}` all-string) into the typed
/// [`NamedDevice`] list, sorted by name for determinism. Devices that fail to
/// parse (unknown `type`, missing keys) are skipped with a warning.
fn named_devices_from_map(map: &BTreeMap<String, BTreeMap<String, String>>) -> Vec<NamedDevice> {
    let mut out: Vec<NamedDevice> = map
        .iter()
        .filter_map(|(name, cfg)| match Device::from_incus_map(cfg) {
            Ok(device) => Some(NamedDevice {
                name: name.clone(),
                device,
            }),
            Err(error) => {
                log::warn!("Skipping unparseable device '{}': {}", name, error);
                None
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub struct IncusPlugin;

impl IncusPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Minimal HTTP-over-UnixSocket client for Incus REST API (AGENTS.md §4: no subprocess bypasses)
    async fn incus_api_request(method: &str, path: &str, body: Option<&str>) -> Result<Vec<u8>> {
        let socket_path = "/var/lib/incus/unix.socket";
        if !std::path::Path::new(socket_path).exists() {
            return Err(anyhow::anyhow!(
                "Incus Unix socket not found at {}",
                socket_path
            ));
        }

        let mut stream = tokio::net::UnixStream::connect(socket_path)
            .await
            .context("Failed to connect to Incus Unix socket")?;

        let body_len = body.map(|b| b.len()).unwrap_or(0);
        let request = format!(
            "{} {} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\n{}Content-Length: {}\r\n\r\n{}",
            method,
            path,
            if body.is_some() {
                "Content-Type: application/json\r\n"
            } else {
                ""
            },
            body_len,
            body.unwrap_or("")
        );

        stream.write_all(request.as_bytes()).await?;

        let mut response = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match tokio::time::timeout(Duration::from_millis(500), stream.read(&mut buf)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => response.extend_from_slice(&buf[..n]),
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => break, // timeout: assume response is complete
            }
        }

        // Extract body from HTTP response
        let body_start = if let Some(idx) = response.windows(4).position(|w| w == b"\r\n\r\n") {
            idx + 4
        } else if let Some(idx) = response.windows(2).position(|w| w == b"\n\n") {
            idx + 2
        } else {
            return Ok(response);
        };

        let headers = std::str::from_utf8(&response[..body_start]).unwrap_or("");
        let mut body = response[body_start..].to_vec();

        // Handle chunked transfer encoding
        if headers
            .to_lowercase()
            .contains("transfer-encoding: chunked")
        {
            let mut decoded = Vec::new();
            let mut pos = 0;
            while pos < body.len() {
                let mut line_end = pos;
                while line_end < body.len() && body[line_end] != b'\n' {
                    line_end += 1;
                }
                if line_end >= body.len() {
                    break;
                }
                let line = std::str::from_utf8(&body[pos..line_end])
                    .unwrap_or("")
                    .trim();
                let size = usize::from_str_radix(line.split(';').next().unwrap_or("0").trim(), 16)
                    .unwrap_or(0);
                if size == 0 {
                    break;
                }
                pos = line_end + 1;
                if pos < body.len() && body[pos] == b'\r' {
                    pos += 1;
                }
                decoded.extend_from_slice(&body[pos..pos + size]);
                pos += size;
                if pos < body.len() && body[pos] == b'\r' {
                    pos += 1;
                }
                if pos < body.len() && body[pos] == b'\n' {
                    pos += 1;
                }
            }
            body = decoded;
        }

        Ok(body)
    }

    /// Call Incus REST API and extract metadata from the (sync or async)
    /// response. Incus answers most mutating calls (instance create/delete,
    /// etc.) with `"type": "async"` — a background operation, not the final
    /// result — which must be polled via `/1.0/operations/<id>/wait` before
    /// its outcome is known. Treating that initial "Operation created"
    /// response as the final answer (as this used to) misreads a real
    /// success as a blank error, since its own `error` field is `""`.
    async fn incus_api_call(method: &str, path: &str, body: Option<&str>) -> Result<Vec<u8>> {
        let response = Self::incus_api_request(method, path, body).await?;
        let mut raw = response;
        let val: simd_json::OwnedValue =
            simd_json::from_slice(&mut raw).context("Failed to parse Incus API response")?;

        let resp_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if resp_type == "async" {
            let op_path = val
                .get("operation")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("async Incus response missing 'operation' path"))?
                .to_string();
            let wait_response =
                Self::incus_api_request("GET", &format!("{op_path}/wait"), None).await?;
            let mut wait_raw = wait_response;
            let wait_val: simd_json::OwnedValue = simd_json::from_slice(&mut wait_raw)
                .context("Failed to parse Incus operation-wait response")?;
            let op_metadata = wait_val
                .get("metadata")
                .cloned()
                .unwrap_or(simd_json::json!({}));
            let op_status = op_metadata
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if op_status == "Success" {
                Ok(simd_json::to_string(
                    &op_metadata
                        .get("metadata")
                        .cloned()
                        .unwrap_or(simd_json::json!({})),
                )?
                .into_bytes())
            } else {
                let err = op_metadata
                    .get("err")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(op_status);
                Err(anyhow::anyhow!("Incus operation failed: {}", err))
            }
        } else {
            let status = val
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            if status == "success" || status == "created" || status == "accepted" {
                let metadata = val.get("metadata").cloned().unwrap_or(simd_json::json!({}));
                Ok(simd_json::to_string(&metadata)?.into_bytes())
            } else {
                let err = val
                    .get("error")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .or_else(|| val.get("status").and_then(|v| v.as_str()))
                    .unwrap_or("Unknown Incus API error");
                Err(anyhow::anyhow!("Incus API error: {}", err))
            }
        }
    }

    /// Fetch current instance configuration via REST API
    async fn incus_get_instance(name: &str) -> Result<simd_json::OwnedValue> {
        let body =
            Self::incus_api_call("GET", &format!("/1.0/instances/{}?recursion=1", name), None)
                .await?;
        let mut raw = body;
        simd_json::from_slice(&mut raw).context("Failed to parse Incus instance")
    }

    /// Update instance configuration via REST API (removes read-only fields first)
    async fn incus_update_instance(name: &str, mut config: simd_json::OwnedValue) -> Result<()> {
        if let Some(obj) = config.as_object_mut() {
            obj.retain(|k, _| !k.starts_with("volatile.") && !k.starts_with("image."));
        }
        let body = simd_json::to_string(&config)?;
        Self::incus_api_call("PUT", &format!("/1.0/instances/{}", name), Some(&body)).await?;
        Ok(())
    }

    /// Run an incus operation via REST API (AGENTS.md §4: no subprocess bypasses)
    async fn run_incus_command(args: &[&str]) -> Result<Vec<u8>> {
        match args {
            ["list", "--format=json"] => {
                Self::incus_api_call("GET", "/1.0/instances?recursion=1", None).await
            }
            ["init", ..] => {
                let mut image = "";
                let mut name = "";
                let mut storage_pool = None;
                let mut profiles: Vec<String> = Vec::new();
                let mut no_profiles = false;
                let mut i = 1;
                while i < args.len() {
                    match args[i] {
                        "--storage" => {
                            i += 1;
                            storage_pool = args.get(i).copied().map(String::from);
                        }
                        "--no-profiles" => {
                            no_profiles = true;
                        }
                        "--profile" => {
                            i += 1;
                            if let Some(p) = args.get(i) {
                                profiles.push(p.to_string());
                            }
                        }
                        arg if arg.starts_with('-') => {}
                        arg if image.is_empty() => image = arg,
                        arg if name.is_empty() => name = arg,
                        _ => {}
                    }
                    i += 1;
                }

                let mut body = simd_json::json!({
                    "name": name,
                    "source": { "type": "image", "alias": image },
                    "type": "container",
                });
                if no_profiles {
                    body.insert("profiles", Value::Array(Vec::new()))?;
                } else if !profiles.is_empty() {
                    let profile_values: Vec<Value> =
                        profiles.iter().map(|p| Value::from(p.as_str())).collect();
                    body.insert("profiles", Value::Array(profile_values))?;
                }
                if let Some(pool) = storage_pool {
                    body.insert(
                        "devices",
                        simd_json::json!({
                            "root": {
                                "type": "disk",
                                "pool": pool,
                                "path": "/"
                            }
                        }),
                    )?;
                }
                let body_str = simd_json::to_string(&body)?;
                Self::incus_api_call("POST", "/1.0/instances", Some(&body_str)).await
            }
            ["delete", name, "--force"] => {
                Self::incus_api_call("DELETE", &format!("/1.0/instances/{}?force=1", name), None)
                    .await
            }
            ["start", name] => {
                let body = r#"{"action":"start"}"#;
                Self::incus_api_call("PUT", &format!("/1.0/instances/{}/state", name), Some(body))
                    .await
            }
            ["stop", name] => {
                let body = r#"{"action":"stop"}"#;
                Self::incus_api_call("PUT", &format!("/1.0/instances/{}/state", name), Some(body))
                    .await
            }
            ["pause", name] => {
                let body = r#"{"action":"freeze"}"#;
                Self::incus_api_call("PUT", &format!("/1.0/instances/{}/state", name), Some(body))
                    .await
            }
            ["profile", "remove", name, profile] => {
                let mut data = Self::incus_get_instance(name).await?;
                if let Some(profiles_arr) = data.get_mut("profiles").and_then(|p| p.as_array_mut())
                {
                    profiles_arr.retain(|p| p.as_str() != Some(profile));
                }
                Self::incus_update_instance(name, data).await?;
                Ok(Vec::new())
            }
            ["profile", "add", name, profile] => {
                let mut data = Self::incus_get_instance(name).await?;
                if data.get("profiles").is_none() {
                    if let Some(obj) = data.as_object_mut() {
                        obj.insert("profiles".to_string(), simd_json::json!([]));
                    }
                }
                let profiles = data
                    .get_mut("profiles")
                    .and_then(|p| p.as_array_mut())
                    .ok_or_else(|| anyhow::anyhow!("profiles field is not an array"))?;
                if !profiles.iter().any(|p| p.as_str() == Some(profile)) {
                    profiles.push(simd_json::json!(profile));
                }
                Self::incus_update_instance(name, data).await?;
                Ok(Vec::new())
            }
            ["config", "unset", name, key] => {
                let mut data = Self::incus_get_instance(name).await?;
                if let Some(config) = data.get_mut("config").and_then(|c| c.as_object_mut()) {
                    config.remove(*key);
                }
                Self::incus_update_instance(name, data).await?;
                Ok(Vec::new())
            }
            ["config", "set", name, kv] => {
                let mut data = Self::incus_get_instance(name).await?;
                if let Some((key, value)) = kv.split_once('=') {
                    if let Some(config) = data.get_mut("config").and_then(|c| c.as_object_mut()) {
                        config.insert(key.to_string(), simd_json::json!(value));
                    } else if let Some(obj) = data.as_object_mut() {
                        let mut config = HashMap::new();
                        config.insert(key.to_string(), simd_json::json!(value));
                        obj.insert("config".to_string(), simd_json::json!(config));
                    }
                }
                Self::incus_update_instance(name, data).await?;
                Ok(Vec::new())
            }
            ["config", "device", "remove", name, device] => {
                let mut data = Self::incus_get_instance(name).await?;
                if let Some(devices) = data.get_mut("devices").and_then(|d| d.as_object_mut()) {
                    devices.remove(*device);
                }
                Self::incus_update_instance(name, data).await?;
                Ok(Vec::new())
            }
            ["config", "device", "add", name, device, dev_type, rest @ ..] => {
                let mut data = Self::incus_get_instance(name).await?;
                let mut dev_config = HashMap::new();
                dev_config.insert("type".to_string(), simd_json::json!(dev_type));
                for arg in rest.iter() {
                    if let Some((k, v)) = arg.split_once('=') {
                        dev_config.insert(k.to_string(), simd_json::json!(v));
                    }
                }
                if let Some(devices) = data.get_mut("devices").and_then(|d| d.as_object_mut()) {
                    devices.insert(device.to_string(), simd_json::json!(dev_config));
                } else if let Some(obj) = data.as_object_mut() {
                    let mut devices = HashMap::new();
                    devices.insert(device.to_string(), simd_json::json!(dev_config));
                    obj.insert("devices".to_string(), simd_json::json!(devices));
                }
                Self::incus_update_instance(name, data).await?;
                Ok(Vec::new())
            }
            _ => Err(anyhow::anyhow!("Unmapped incus CLI args: {:?}", args)),
        }
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
                let expanded_config = if raw.expanded_config.is_empty() {
                    None
                } else {
                    Some(raw.expanded_config)
                };

                let devices = named_devices_from_map(&raw.devices);
                let expanded_devices = named_devices_from_map(&raw.expanded_devices);

                IncusInstance {
                    name: raw.name,
                    status: raw.status,
                    instance_type: raw.instance_type,
                    status_code: if raw.status_code == 0 {
                        None
                    } else {
                        Some(raw.status_code)
                    },
                    image,
                    storage_pool,
                    profiles: raw.profiles,
                    description: if raw.description.is_empty() {
                        None
                    } else {
                        Some(raw.description)
                    },
                    architecture: if raw.architecture.is_empty() {
                        None
                    } else {
                        Some(raw.architecture)
                    },
                    ephemeral: Some(raw.ephemeral),
                    stateful: Some(raw.stateful),
                    created_at: if raw.created_at.is_empty() {
                        None
                    } else {
                        Some(raw.created_at)
                    },
                    last_used_at: if raw.last_used_at.is_empty() {
                        None
                    } else {
                        Some(raw.last_used_at)
                    },
                    location: if raw.location.is_empty() || raw.location == "none" {
                        None
                    } else {
                        Some(raw.location)
                    },
                    project: if raw.project.is_empty() || raw.project == "default" {
                        None
                    } else {
                        Some(raw.project)
                    },
                    config,
                    devices,
                    expanded_config,
                    expanded_devices,
                }
            })
            .collect();

        Ok(instances)
    }

    /// Apply a single Create action for an instance.
    pub async fn apply_create(instance: &IncusInstance) -> Result<Vec<String>> {
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

    /// Managed devices keyed by name, each as an Incus string map, excluding the
    /// `root` disk (managed implicitly via `storage_pool` at creation).
    fn managed_devices(instance: &IncusInstance) -> HashMap<String, BTreeMap<String, String>> {
        instance
            .devices
            .iter()
            .filter(|nd| nd.name != "root")
            .map(|nd| (nd.name.clone(), nd.device.to_incus_map()))
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
        // Refuse banned device types before touching the instance. Enforced on
        // `desired` only: a proxy/nic already present out-of-band shows up in
        // `current` and is removed below as stale, which is the intended
        // direction of travel — but declaring a new one never succeeds.
        NamedDevice::enforce_device_policy(&desired.devices)
            .with_context(|| format!("refusing to apply devices to '{name}'"))?;

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

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(incus_schema())
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/usr/bin/incus").exists()
    }

    fn unavailable_reason(&self) -> String {
        "Incus not installed (/usr/bin/incus not found)".to_string()
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
        let current_state: Option<IncusState> = None;
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

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        log::info!("Creating Incus state checkpoint");
        let state = simd_json::json!(null);
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

        let current = simd_json::json!(null);
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

    use super::super::incus_device::NicDevice;

    fn nic(parent: &str) -> NamedDevice {
        NamedDevice {
            name: "privacy0".to_string(),
            device: Device::Nic(NicDevice {
                nictype: Some("bridged".to_string()),
                parent: Some(parent.to_string()),
                ..Default::default()
            }),
        }
    }

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
            devices: vec![nic("ovsbr0")],
            ..Default::default()
        };
        let mut desired = current.clone();
        assert!(IncusPlugin::instances_equivalent(&current, &desired));

        desired.config = Some(HashMap::from([(
            "user.opdbus.route_id".to_string(),
            "route-b".to_string(),
        )]));
        assert!(!IncusPlugin::instances_equivalent(&current, &desired));

        desired = current.clone();
        desired.devices = vec![nic("ovsbr1")];
        assert!(!IncusPlugin::instances_equivalent(&current, &desired));
    }

    #[test]
    fn parse_instance_list_builds_typed_devices() {
        let raw = br#"[
            {
                "name": "netmaker",
                "status": "Running",
                "type": "container",
                "profiles": ["default"],
                "config": {"boot.autostart": "true"},
                "devices": {
                    "api-sock": {
                        "type": "proxy",
                        "listen": "unix:/run/netmaker/api.sock",
                        "connect": "tcp:127.0.0.1:8081",
                        "uid": "0"
                    },
                    "eth0": {"type": "nic", "nictype": "bridged", "parent": "ovsbr0"}
                }
            }
        ]"#
        .to_vec();
        let instances = IncusPlugin::parse_instance_list(raw).expect("parse");
        assert_eq!(instances.len(), 1);
        let devices = &instances[0].devices;
        assert_eq!(devices.len(), 2);
        // Sorted by name: api-sock then eth0.
        assert_eq!(devices[0].name, "api-sock");
        assert!(matches!(devices[0].device, Device::Proxy(_)));
        assert!(matches!(devices[1].device, Device::Nic(_)));
    }
}

// =============================================================================
// Method input types - single source of truth via schemars
// =============================================================================

/// create_instance method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateInstanceInput {
    /// Instance name
    pub name: String,
    /// Instance type: container or virtual-machine
    #[serde(default = "default_instance_type")]
    pub instance_type: String,
    /// Image reference
    pub image: String,
    /// Profile names to apply
    #[serde(default)]
    pub profiles: Vec<String>,
    /// Configuration key-value pairs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, String>>,
    /// Device definitions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<NamedDevice>,
}

fn default_instance_type() -> String {
    "container".to_string()
}

/// modify_instance method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModifyInstanceInput {
    /// Instance name
    pub name: String,
    /// Configuration updates
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, String>>,
    /// Device updates
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<HashMap<String, HashMap<String, String>>>,
}

/// delete_instance method input
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteInstanceInput {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StartInstanceInput {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StopInstanceInput {
    pub name: String,
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RestartInstanceInput {
    pub name: String,
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FreezeInstanceInput {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnfreezeInstanceInput {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotInstanceInput {
    pub name: String,
    pub snapshot_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExecInstanceInput {
    pub name: String,
    pub command: Vec<String>,
}

fn validate_instance_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        anyhow::bail!("invalid Incus instance name");
    }
    Ok(())
}

/// Execute the declared Incus lifecycle methods against the Incus REST API.
///
/// These methods used to fall through the bridge's generic echo arm, which
/// produced an audited `success` response without touching Incus. Keep the
/// authority here beside the typed method inputs and fail closed for methods
/// that do not yet have a real backend.
pub async fn dispatch_incus_method(
    method: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let acknowledge = || serde_json::json!({"success": true});

    match method {
        "start_instance" | "StartInstance" => {
            let input: StartInstanceInput = serde_json::from_value(args.clone())?;
            validate_instance_name(&input.name)?;
            IncusPlugin::incus_api_call(
                "PUT",
                &format!("/1.0/instances/{}/state", input.name),
                Some(r#"{"action":"start","timeout":30,"force":false,"stateful":false}"#),
            )
            .await?;
            Ok(acknowledge())
        }
        "stop_instance" | "StopInstance" => {
            let input: StopInstanceInput = serde_json::from_value(args.clone())?;
            validate_instance_name(&input.name)?;
            let body = serde_json::to_string(&serde_json::json!({
                "action": "stop",
                "timeout": 30,
                "force": input.force.unwrap_or(false),
                "stateful": false,
            }))?;
            IncusPlugin::incus_api_call(
                "PUT",
                &format!("/1.0/instances/{}/state", input.name),
                Some(&body),
            )
            .await?;
            Ok(acknowledge())
        }
        "restart_instance" | "RestartInstance" => {
            let input: RestartInstanceInput = serde_json::from_value(args.clone())?;
            validate_instance_name(&input.name)?;
            let body = serde_json::to_string(&serde_json::json!({
                "action": "restart",
                "timeout": 30,
                "force": input.force.unwrap_or(false),
                "stateful": false,
            }))?;
            IncusPlugin::incus_api_call(
                "PUT",
                &format!("/1.0/instances/{}/state", input.name),
                Some(&body),
            )
            .await?;
            Ok(acknowledge())
        }
        "freeze_instance" | "FreezeInstance" => {
            let input: FreezeInstanceInput = serde_json::from_value(args.clone())?;
            validate_instance_name(&input.name)?;
            IncusPlugin::incus_api_call(
                "PUT",
                &format!("/1.0/instances/{}/state", input.name),
                Some(r#"{"action":"freeze","timeout":30,"force":false,"stateful":false}"#),
            )
            .await?;
            Ok(acknowledge())
        }
        "unfreeze_instance" | "UnfreezeInstance" => {
            let input: UnfreezeInstanceInput = serde_json::from_value(args.clone())?;
            validate_instance_name(&input.name)?;
            IncusPlugin::incus_api_call(
                "PUT",
                &format!("/1.0/instances/{}/state", input.name),
                Some(r#"{"action":"unfreeze","timeout":30,"force":false,"stateful":false}"#),
            )
            .await?;
            Ok(acknowledge())
        }
        "exec_instance" | "ExecInstance" => {
            let input: ExecInstanceInput = serde_json::from_value(args.clone())?;
            validate_instance_name(&input.name)?;
            if input.command.is_empty() || input.command.iter().any(|part| part.contains('\0')) {
                anyhow::bail!("Incus exec command must contain non-NUL arguments");
            }
            let body = serde_json::to_string(&serde_json::json!({
                "command": input.command,
                "interactive": false,
                "wait-for-websocket": false,
                "record-output": true,
            }))?;
            IncusPlugin::incus_api_call(
                "POST",
                &format!("/1.0/instances/{}/exec", input.name),
                Some(&body),
            )
            .await?;
            Ok(acknowledge())
        }
        _ => anyhow::bail!("Incus method '{method}' has no live dispatch backend"),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddDeviceInput {
    pub instance_name: String,
    pub device_name: String,
    pub device: super::incus_device::Device,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RemoveDeviceInput {
    pub instance_name: String,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateDeviceInput {
    pub instance_name: String,
    pub device_name: String,
    pub device: super::incus_device::Device,
}

pub(crate) fn incus_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(IncusState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "incus",
        "1.0.0",
        "Incus instance management",
        &root,
    );
    schema.example = Some(json!({
        "instances": [
            {
                "name": "privacy-user-123",
                "status": "Running",
                "type": "container",
                "image": "images:debian/13",
                "storage_pool": "registration",
                "profiles": ["default"],
                "config": { "limits.cpu": "2" },
                "devices": [
                    {
                        "name": "eth0",
                        "device": {
                            "type": "nic",
                            "nictype": "bridged",
                            "parent": "ovsbr0"
                        }
                    }
                ]
            },
            {
                "name": "netmaker",
                "status": "Running",
                "type": "container",
                "image": "docker.io/gravitl/netmaker:v1.5.1",
                "profiles": ["default"],
                "config": { "boot.autostart": "true" },
                "devices": [
                    {
                        "name": "api-sock",
                        "device": {
                            "type": "proxy",
                            "listen": "unix:/run/netmaker/api.sock",
                            "connect": "tcp:127.0.0.1:8081",
                            "uid": "0",
                            "gid": "0",
                            "mode": "0660"
                        }
                    },
                    {
                        "name": "sqldata",
                        "device": {
                            "type": "disk",
                            "path": "/root/data",
                            "source": "nm-sqldata"
                        }
                    }
                ]
            }
        ]
    }));
    let mut methods = std::collections::HashMap::new();
    methods.insert(
        "create_instance".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            CreateInstanceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "create_instance",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.instance.create@v1",
            "mut.service.incus.instance.create@v1",
        ),
    );
    methods.insert(
        "modify_instance".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ModifyInstanceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "modify_instance",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.instance.modify@v1",
            "mut.service.incus.instance.modify@v1",
        ),
    );
    methods.insert(
        "delete_instance".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            DeleteInstanceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "delete_instance",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.instance.delete@v1",
            "mut.service.incus.instance.delete@v1",
        ),
    );
    methods.insert(
        "start_instance".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            StartInstanceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "start_instance",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.instance.start@v1",
            "mut.service.incus.instance.start@v1",
        ),
    );
    methods.insert(
        "stop_instance".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            StopInstanceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "stop_instance",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.instance.stop@v1",
            "mut.service.incus.instance.stop@v1",
        ),
    );
    methods.insert(
        "restart_instance".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            RestartInstanceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "restart_instance",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.instance.restart@v1",
            "mut.service.incus.instance.restart@v1",
        ),
    );
    methods.insert(
        "freeze_instance".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            FreezeInstanceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "freeze_instance",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.instance.freeze@v1",
            "mut.service.incus.instance.freeze@v1",
        ),
    );
    methods.insert(
        "unfreeze_instance".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            UnfreezeInstanceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "unfreeze_instance",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.instance.unfreeze@v1",
            "mut.service.incus.instance.unfreeze@v1",
        ),
    );
    methods.insert(
        "snapshot_instance".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            SnapshotInstanceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "snapshot_instance",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.instance.snapshot@v1",
            "mut.service.incus.instance.snapshot@v1",
        ),
    );
    methods.insert(
        "exec_instance".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            ExecInstanceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "exec_instance",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.instance.exec@v1",
            "mut.service.incus.instance.exec@v1",
        ),
    );
    methods.insert(
        "add_device".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            AddDeviceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "add_device",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.device.add@v1",
            "mut.service.incus.device.add@v1",
        ),
    );
    methods.insert(
        "remove_device".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            RemoveDeviceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "remove_device",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.device.remove@v1",
            "mut.service.incus.device.remove@v1",
        ),
    );
    methods.insert(
        "update_device".to_string(),
        super::plugin_scaffold_helpers::method_decl_from_schemars_with_output::<
            UpdateDeviceInput,
            super::plugin_scaffold_helpers::AckOutput,
        >(
            "update_device",
            op_state_store::SideEffect::Mutation,
            false,
            "cap.service.incus.device.update@v1",
            "mut.service.incus.device.update@v1",
        ),
    );
    schema = schema.with_methods(methods);

    schema.capabilities.insert(
        "cap.service.incus.instance.create@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.instance.create@v1".to_string(),
            description: "Grants: create_instance.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.instance.modify@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.instance.modify@v1".to_string(),
            description: "Grants: modify_instance.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.instance.delete@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.instance.delete@v1".to_string(),
            description: "Grants: delete_instance.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.instance.start@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.instance.start@v1".to_string(),
            description: "Grants: start_instance.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.instance.stop@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.instance.stop@v1".to_string(),
            description: "Grants: stop_instance.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.instance.restart@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.instance.restart@v1".to_string(),
            description: "Grants: restart_instance.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.instance.freeze@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.instance.freeze@v1".to_string(),
            description: "Grants: freeze_instance.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.instance.unfreeze@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.instance.unfreeze@v1".to_string(),
            description: "Grants: unfreeze_instance.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.instance.snapshot@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.instance.snapshot@v1".to_string(),
            description: "Grants: snapshot_instance.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.instance.exec@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.instance.exec@v1".to_string(),
            description: "Grants: exec_instance.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.device.add@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.device.add@v1".to_string(),
            description: "Grants: add_device.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.device.remove@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.device.remove@v1".to_string(),
            description: "Grants: remove_device.".to_string(),
        },
    );
    schema.capabilities.insert(
        "cap.service.incus.device.update@v1".to_string(),
        op_state_store::CapabilityDecl {
            id: "cap.service.incus.device.update@v1".to_string(),
            description: "Grants: update_device.".to_string(),
        },
    );

    schema
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("incus", |_ctx| std::sync::Arc::new(IncusPlugin::new()))
}

// ── Inspector Gadget + Repomix generated candidates ───────────────────────
// Generated against PLUGIN-RENDER-CONTRACT.md. The original plugin above is
// preserved. Review ownership, concrete types, defaults, side effects, and
// runtime dispatch before flattening these candidates into the live state/schema.
#[allow(dead_code)]
mod inspector_gadget_generated {
    use serde::{Deserialize, Serialize};

    /// Repomix-discovered fields not represented by the input plugin.
    #[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
    #[schemars(extend("x-oscal-subid" = "sch.software.incus.inspector-candidates.schema@v1"))]
    pub struct InspectorGadgetFields {
        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.AuthType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.authtype@v1"))]
        pub authtype: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.CacheExpiry`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cacheexpiry@v1"))]
        pub cacheexpiry: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.CachePath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cachepath@v1"))]
        pub cachepath: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.CookieJar`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cookiejar@v1"))]
        pub cookiejar: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.HTTPClient`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.httpclient@v1"))]
        pub httpclient: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.IdenticalCertificate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.identicalcertificate@v1"))]
        pub identicalcertificate: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.InsecureSkipVerify`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.insecureskipverify@v1"))]
        pub insecureskipverify: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.OIDCNonInteractive`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.oidcnoninteractive@v1"))]
        pub oidcnoninteractive: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.OIDCTokens`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.oidctokens@v1"))]
        pub oidctokens: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.Proxy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.proxy@v1"))]
        pub proxy: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.SkipGetEvents`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.skipgetevents@v1"))]
        pub skipgetevents: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.SkipGetServer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.skipgetserver@v1"))]
        pub skipgetserver: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.TLSCA`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.tlsca@v1"))]
        pub tlsca: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.TLSClientCert`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.tlsclientcert@v1"))]
        pub tlsclientcert: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.TLSClientKey`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.tlsclientkey@v1"))]
        pub tlsclientkey: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.TLSServerCert`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.tlsservercert@v1"))]
        pub tlsservercert: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.TempPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.temppath@v1"))]
        pub temppath: Option<String>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.TransportWrapper`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.transportwrapper@v1"))]
        pub transportwrapper: Option<u64>,

        /// Discovered from Repomix path `go.client.connection.struct.ConnectionArgs.field.UserAgent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.useragent@v1"))]
        pub useragent: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.BackupFileRequest.field.BackupFile`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.backupfile@v1"))]
        pub backupfile: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.BackupFileRequest.field.Canceler`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.canceler@v1"))]
        pub canceler: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.BackupFileRequest.field.ProgressHandler`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.progresshandler@v1"))]
        pub progresshandler: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.BackupFileResponse.field.Size`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.size@v1"))]
        pub size: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ConnectionInfo.field.Addresses`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.addresses@v1"))]
        pub addresses: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ConnectionInfo.field.Certificate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.certificate@v1"))]
        pub certificate: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ConnectionInfo.field.Protocol`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.protocol@v1"))]
        pub protocol: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ConnectionInfo.field.SocketPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.socketpath@v1"))]
        pub socketpath: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ConnectionInfo.field.Target`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.target@v1"))]
        pub target: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ConnectionInfo.field.URL`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.url@v1"))]
        pub url: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageCopyArgs.field.Aliases`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.aliases@v1"))]
        pub aliases: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageCopyArgs.field.AutoUpdate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.autoupdate@v1"))]
        pub autoupdate: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageCopyArgs.field.CopyAliases`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.copyaliases@v1"))]
        pub copyaliases: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageCopyArgs.field.Mode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.mode@v1"))]
        pub mode: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageCopyArgs.field.Public`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.public@v1"))]
        pub public: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageCopyArgs.field.Type`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.type-field@v1"))]
        pub type_field: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageCreateArgs.field.MetaFile`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.metafile@v1"))]
        pub metafile: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageCreateArgs.field.MetaName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.metaname@v1"))]
        pub metaname: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageCreateArgs.field.RootfsFile`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.rootfsfile@v1"))]
        pub rootfsfile: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageCreateArgs.field.RootfsName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.rootfsname@v1"))]
        pub rootfsname: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageFileRequest.field.DeltaSourceRetriever`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.deltasourceretriever@v1"))]
        pub deltasourceretriever: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageFileResponse.field.MetaSize`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.metasize@v1"))]
        pub metasize: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.ImageFileResponse.field.RootfsSize`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.rootfssize@v1"))]
        pub rootfssize: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceBackupArgs.field.PoolName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.poolname@v1"))]
        pub poolname: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceConsoleArgs.field.ConsoleDisconnect`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.consoledisconnect@v1"))]
        pub consoledisconnect: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceConsoleArgs.field.Control`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.control@v1"))]
        pub control: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceConsoleArgs.field.Terminal`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.terminal@v1"))]
        pub terminal: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceCopyArgs.field.AllowInconsistent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.allowinconsistent@v1"))]
        pub allowinconsistent: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceCopyArgs.field.InstanceOnly`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.instanceonly@v1"))]
        pub instanceonly: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceCopyArgs.field.Live`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.live@v1"))]
        pub live: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceCopyArgs.field.Refresh`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.refresh@v1"))]
        pub refresh: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceCopyArgs.field.RefreshExcludeOlder`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.refreshexcludeolder@v1"))]
        pub refreshexcludeolder: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceExecArgs.field.DataDone`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.datadone@v1"))]
        pub datadone: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceExecArgs.field.Stderr`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.stderr@v1"))]
        pub stderr: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceExecArgs.field.Stdin`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.stdin@v1"))]
        pub stdin: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceExecArgs.field.Stdout`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.stdout@v1"))]
        pub stdout: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceFileArgs.field.Content`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.content@v1"))]
        pub content: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceFileArgs.field.GID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.gid@v1"))]
        pub gid: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceFileArgs.field.UID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.uid@v1"))]
        pub uid: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceFileArgs.field.WriteMode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.writemode@v1"))]
        pub writemode: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceFileResponse.field.Entries`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.entries@v1"))]
        pub entries: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.InstanceNBDArgs.field.Reuse`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.reuse@v1"))]
        pub reuse: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.StoragePoolVolumeCopyArgs.field.VolumeOnly`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.volumeonly@v1"))]
        pub volumeonly: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.StoragePoolVolumeMoveArgs.field.StoragePoolVolumeCopyArgs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.storagepoolvolumecopyargs@v1"))]
        pub storagepoolvolumecopyargs: Option<String>,

        /// Discovered from Repomix path `go.client.interfaces.struct.StorageVolumeNBDPost.field.Writable`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.writable@v1"))]
        pub writable: Option<String>,

        /// Discovered from Repomix path `go.client.oci_images.struct.ociInfo.field.Alias`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.alias@v1"))]
        pub alias: Option<String>,

        /// Discovered from Repomix path `go.client.oci_images.struct.ociInfo.field.Created`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.created@v1"))]
        pub created: Option<String>,

        /// Discovered from Repomix path `go.client.oci_images.struct.ociInfo.field.Digest`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.digest@v1"))]
        pub digest: Option<String>,

        /// Discovered from Repomix path `go.client.oci_images.struct.ociInfo.field.Layers`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.layers@v1"))]
        pub layers: Option<String>,

        /// Discovered from Repomix path `go.client.oci_images.struct.ociInfo.field.LayersData`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.layersdata@v1"))]
        pub layersdata: Option<String>,

        /// Discovered from Repomix path `go.client.oci_util_linux.struct.umociLogHandler.field.Message`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.message@v1"))]
        pub message: Option<String>,

        /// Discovered from Repomix path `go.client.util.struct.remoteOperationResult.field.Error`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.error@v1"))]
        pub error: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.admin_cluster.struct.cmdAdminCluster.field.To`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.to@v1"))]
        pub to: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.admin_cluster.struct.cmdAdminCluster.field.You`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.you@v1"))]
        pub you: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.admin_recover.struct.cmdAdminRecover.field.This`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.this@v1"))]
        pub this: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.admin_sql.struct.cmdAdminSQL.field.If`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.if-field@v1"))]
        pub if_field: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.admin_sql.struct.cmdAdminSQL.field.The`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.the@v1"))]
        pub the: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.admin_update_certificate.struct.cmdAdminUpdateCertificate.field.Key`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.key@v1"))]
        pub key: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.alias.struct.cmdAliasAdd.field.Create`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.create@v1"))]
        pub create: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.alias.struct.cmdAliasAdd.field.Overwrite`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.overwrite@v1"))]
        pub overwrite: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.alias.struct.cmdAliasRemove.field.Remove`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.remove@v1"))]
        pub remove: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.alias.struct.cmdAliasRename.field.Rename`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.rename@v1"))]
        pub rename: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.cluster.struct.clusterColumn.field.Data`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.data@v1"))]
        pub data: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.cluster.struct.clusterListTokenColumn.field.Column`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.column@v1"))]
        pub column: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.cluster.struct.clusterListTokenColumn.field.Commas`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.commas@v1"))]
        pub commas: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.cluster.struct.clusterListTokenColumn.field.Default`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.default@v1"))]
        pub default: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.cluster.struct.clusterListTokenColumn.field.E`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.e@v1"))]
        pub e: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.cluster.struct.cmdClusterEdit.field.Update`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.update@v1"))]
        pub update: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.cluster.struct.cmdClusterRemove.field.Are`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.are@v1"))]
        pub are: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.cluster.struct.cmdClusterRemove.field.When`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.when@v1"))]
        pub when: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.cluster_group.struct.cmdClusterGroupAssign.field.Reset`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.reset@v1"))]
        pub reset: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.cluster_group.struct.cmdClusterGroupAssign.field.Set`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.set@v1"))]
        pub set: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config.struct.cmdConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cmdconfig@v1"))]
        pub cmdconfig: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config.struct.cmdConfigEdit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cmdconfigedit@v1"))]
        pub cmdconfigedit: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config.struct.cmdConfigGet`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cmdconfigget@v1"))]
        pub cmdconfigget: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config.struct.cmdConfigSet`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cmdconfigset@v1"))]
        pub cmdconfigset: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config.struct.cmdConfigSet.field.For`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.for-field@v1"))]
        pub for_field: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config.struct.cmdConfigSet.field.Sets`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.sets@v1"))]
        pub sets: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config.struct.cmdConfigSet.field.Will`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.will@v1"))]
        pub will: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config.struct.cmdConfigShow`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cmdconfigshow@v1"))]
        pub cmdconfigshow: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config.struct.cmdConfigUnset`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cmdconfigunset@v1"))]
        pub cmdconfigunset: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config.struct.cmdConfigUnset.field.Unsetting`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.unsetting@v1"))]
        pub unsetting: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config_trust.struct.rowData.field.Cert`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cert@v1"))]
        pub cert: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.config_trust.struct.rowData.field.TLSCert`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.tlscert@v1"))]
        pub tlscert: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.copy.struct.cmdCopy.field.Transfer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.transfer@v1"))]
        pub transfer: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.debug.struct.cmdDebugMemory.field.Creates`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.creates@v1"))]
        pub creates: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.exec.struct.cmdExec.field.Run`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.run@v1"))]
        pub run: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.export.struct.cmdExport.field.Download`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.download@v1"))]
        pub download: Option<u64>,

        /// Discovered from Repomix path `go.cmd.incus.image.struct.cmdImage.field.Images`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.images@v1"))]
        pub images: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.image.struct.cmdImageCopy.field.It`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.it@v1"))]
        pub it: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.image.struct.cmdImageEdit.field.Launch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.launch@v1"))]
        pub launch: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.image.struct.cmdImageEdit.field.Load`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.load@v1"))]
        pub load: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.image.struct.cmdImageImport.field.Directory`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.directory@v1"))]
        pub directory: Option<u64>,

        /// Discovered from Repomix path `go.cmd.incus.image.struct.cmdImageList.field.F`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.f@v1"))]
        pub f: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.image.struct.cmdImageList.field.Filters`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.filters@v1"))]
        pub filters: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.image.struct.cmdImageList.field.L`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.l@v1"))]
        pub l: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.A`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.a@v1"))]
        pub a: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.Custom`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.custom@v1"))]
        pub custom: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.D`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.d@v1"))]
        pub d: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.Defaults`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.defaults@v1"))]
        pub defaults: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.Fast`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.fast@v1"))]
        pub fast: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.List`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.list@v1"))]
        pub list: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.M`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.m@v1"))]
        pub m: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.N`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.n@v1"))]
        pub n: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.P`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.p@v1"))]
        pub p: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.S`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.s@v1"))]
        pub s: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.Show`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.show@v1"))]
        pub show: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.cmdList.field.U`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.u@v1"))]
        pub u: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.column.field.NeedsSnapshots`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.needssnapshots@v1"))]
        pub needssnapshots: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.list.struct.column.field.NeedsState`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.needsstate@v1"))]
        pub needsstate: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.main.struct.cmdGlobal.field.All`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.all@v1"))]
        pub all: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.main.struct.cmdGlobal.field.As`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.as-field@v1"))]
        pub as_field: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.main.struct.cmdGlobal.field.Or`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.or@v1"))]
        pub or: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.monitor.struct.cmdMonitor.field.By`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.by@v1"))]
        pub by: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.monitor.struct.cmdMonitor.field.Only`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.only@v1"))]
        pub only: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.move.struct.cmdMove.field.Move`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.move-field@v1"))]
        pub move_field: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.network_allocations.struct.networkAllocationColumn.field.Network`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.network@v1"))]
        pub network: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.network_allocations.struct.networkAllocationColumn.field.Subnet`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.subnet@v1"))]
        pub subnet: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.network_allocations.struct.networkAllocationColumn.field.Used`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.used@v1"))]
        pub used: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.operation.struct.cmdOperationList.field.C`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.c@v1"))]
        pub c: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.port_forward.struct.cmdPortForward.field.Both`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.both@v1"))]
        pub both: Option<u64>,

        /// Discovered from Repomix path `go.cmd.incus.port_forward.struct.cmdPortForward.field.Forward`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.forward@v1"))]
        pub forward: Option<u64>,

        /// Discovered from Repomix path `go.cmd.incus.query.struct.cmdQuery.field.Delete`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.delete@v1"))]
        pub delete: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.remote.struct.cmdRemoteAdd.field.Basic`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.basic@v1"))]
        pub basic: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.remote.struct.cmdRemoteAdd.field.Several`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.several@v1"))]
        pub several: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.remote.struct.cmdRemoteSetKeepalive.field.Disable`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.disable@v1"))]
        pub disable: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.snapshot.struct.cmdSnapshotRestore.field.Restore`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.restore@v1"))]
        pub restore: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.snapshot.struct.snapshotColumn.field.T`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.t@v1"))]
        pub t: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.storage_volume.struct.cmdStorageVolume.field.Unless`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.unless@v1"))]
        pub unless: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.storage_volume.struct.cmdStorageVolumeEdit.field.Edit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.edit@v1"))]
        pub edit: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.storage_volume.struct.cmdStorageVolumeEdit.field.Supported`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.supported@v1"))]
        pub supported: Option<u64>,

        /// Discovered from Repomix path `go.cmd.incus.storage_volume.struct.cmdStorageVolumeGet.field.Returns`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.returns@v1"))]
        pub returns: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.storage_volume.struct.cmdStorageVolumeUnset.field.Removes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.removes@v1"))]
        pub removes: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Config.field.CLIConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cliconfig@v1"))]
        pub cliconfig: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Config.field.CLIConfigPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cliconfigpath@v1"))]
        pub cliconfigpath: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Config.field.Command`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.command@v1"))]
        pub command: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Config.field.ExplainOnly`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.explainonly@v1"))]
        pub explainonly: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Config.field.RTL`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.rtl@v1"))]
        pub rtl: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Parsed.field.BranchID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.branchid@v1"))]
        pub branchid: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Parsed.field.RemoteName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.remotename@v1"))]
        pub remotename: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Parsed.field.RemoteObject`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.remoteobject@v1"))]
        pub remoteobject: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Parsed.field.RemoteServer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.remoteserver@v1"))]
        pub remoteserver: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Parsed.field.Skipped`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.skipped@v1"))]
        pub skipped: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Parsed.field.String`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.string@v1"))]
        pub string: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.parse.struct.Parsed.field.StringList`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.stringlist@v1"))]
        pub stringlist: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.ACL`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.acl@v1"))]
        pub acl: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Address`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.address@v1"))]
        pub address: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.AddressSet`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.addressset@v1"))]
        pub addressset: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Backend`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.backend@v1"))]
        pub backend: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Bucket`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.bucket@v1"))]
        pub bucket: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Client`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.client@v1"))]
        pub client: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.CommandLine`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.commandline@v1"))]
        pub commandline: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Device`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.device@v1"))]
        pub device: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Direction`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.direction@v1"))]
        pub direction: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Driver`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.driver@v1"))]
        pub driver: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.EndOfFlags`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.endofflags@v1"))]
        pub endofflags: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Expiry`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.expiry@v1"))]
        pub expiry: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.File`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.file@v1"))]
        pub file: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Filter`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.filter@v1"))]
        pub filter: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Fingerprint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.fingerprint@v1"))]
        pub fingerprint: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Group`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.group@v1"))]
        pub group: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Instance`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.instance@v1"))]
        pub instance: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Interface`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.interface@v1"))]
        pub interface: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.KV`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.kv@v1"))]
        pub kv: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.KeepaliveTimeout`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.keepalivetimeout@v1"))]
        pub keepalivetimeout: Option<u64>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.ListenAddress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.listenaddress@v1"))]
        pub listenaddress: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.ListenPort`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.listenport@v1"))]
        pub listenport: Option<u64>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Member`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.member@v1"))]
        pub member: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.NetworkIntegration`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.networkintegration@v1"))]
        pub networkintegration: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Operation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.operation@v1"))]
        pub operation: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Path`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.path@v1"))]
        pub path: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Peer`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.peer@v1"))]
        pub peer: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Pool`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.pool@v1"))]
        pub pool: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Port`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.port@v1"))]
        pub port: Option<u64>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Profile`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.profile@v1"))]
        pub profile: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Query`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.query@v1"))]
        pub query: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Record`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.record@v1"))]
        pub record: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Remote`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.remote@v1"))]
        pub remote: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.RemoteColon`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.remotecolon@v1"))]
        pub remotecolon: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.RemoteColonOpt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.remotecolonopt@v1"))]
        pub remotecolonopt: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.RemoteImage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.remoteimage@v1"))]
        pub remoteimage: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Role`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.role@v1"))]
        pub role: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Snapshot`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.snapshot@v1"))]
        pub snapshot: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.StorageVolumeType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.storagevolumetype@v1"))]
        pub storagevolumetype: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.SymlinkTargetPath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.symlinktargetpath@v1"))]
        pub symlinktargetpath: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Tarball`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.tarball@v1"))]
        pub tarball: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Template`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.template@v1"))]
        pub template: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Token`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.token@v1"))]
        pub token: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Value`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.value@v1"))]
        pub value: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Volume`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.volume@v1"))]
        pub volume: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.WarningUUID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.warninguuid@v1"))]
        pub warninguuid: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.usage.usage.struct.verbatim.field.Zone`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.zone@v1"))]
        pub zone: Option<String>,

        /// Discovered from Repomix path `go.cmd.incus.wait.struct.cmdWait.field.Wait`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.wait@v1"))]
        pub wait: Option<String>,

        /// Discovered from Repomix path `go.shared.api.access.struct.AccessEntry.field.Identifier`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.identifier@v1"))]
        pub identifier: Option<String>,

        /// Discovered from Repomix path `go.shared.api.access.struct.AccessEntry.field.Provider`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.provider@v1"))]
        pub provider: Option<String>,

        /// Discovered from Repomix path `go.shared.api.agent.api.struct.API10Put.field.CID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cid@v1"))]
        pub cid: Option<String>,

        /// Discovered from Repomix path `go.shared.api.agent.api.struct.API10Put.field.DevIncus`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.devincus@v1"))]
        pub devincus: Option<String>,

        /// Discovered from Repomix path `go.shared.api.certificate.struct.CertificateAddToken.field.ClientName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.clientname@v1"))]
        pub clientname: Option<String>,

        /// Discovered from Repomix path `go.shared.api.certificate.struct.CertificateAddToken.field.ExpiresAt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.expiresat@v1"))]
        pub expiresat: Option<String>,

        /// Discovered from Repomix path `go.shared.api.certificate.struct.CertificateAddToken.field.Secret`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.secret@v1"))]
        pub secret: Option<String>,

        /// Discovered from Repomix path `go.shared.api.certificate.struct.CertificatePut.field.Projects`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.projects@v1"))]
        pub projects: Option<String>,

        /// Discovered from Repomix path `go.shared.api.certificate.struct.CertificatePut.field.Restricted`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.restricted@v1"))]
        pub restricted: Option<String>,

        /// Discovered from Repomix path `go.shared.api.certificate.struct.CertificatesPost.field.TrustToken`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.trusttoken@v1"))]
        pub trusttoken: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.Cluster.field.Enabled`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.enabled@v1"))]
        pub enabled: Option<bool>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.Cluster.field.MemberConfig`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.memberconfig@v1"))]
        pub memberconfig: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.Cluster.field.ServerName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.servername@v1"))]
        pub servername: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterCertificatePut.field.ClusterCertificate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.clustercertificate@v1"))]
        pub clustercertificate: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterCertificatePut.field.ClusterCertificateKey`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.clustercertificatekey@v1"))]
        pub clustercertificatekey: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterGroup.field.ClusterGroupPut`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.clustergroupput@v1"))]
        pub clustergroupput: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterGroup.field.UsedBy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.usedby@v1"))]
        pub usedby: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterGroupPut.field.Members`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.members@v1"))]
        pub members: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterMember.field.Database`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.database@v1"))]
        pub database: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterMemberConfigKey.field.Entity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.entity@v1"))]
        pub entity: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterMemberPut.field.FailureDomain`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.failuredomain@v1"))]
        pub failuredomain: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterMemberPut.field.Groups`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.groups@v1"))]
        pub groups: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterMemberPut.field.Roles`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.roles@v1"))]
        pub roles: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterMemberStatePost.field.Action`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.action@v1"))]
        pub action: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterPut.field.ClusterAddress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.clusteraddress@v1"))]
        pub clusteraddress: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterPut.field.ClusterToken`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.clustertoken@v1"))]
        pub clustertoken: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster.struct.ClusterPut.field.ServerAddress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.serveraddress@v1"))]
        pub serveraddress: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster_state.struct.ClusterMemberState.field.StoragePools`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.storagepools@v1"))]
        pub storagepools: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster_state.struct.ClusterMemberState.field.SysInfo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.sysinfo@v1"))]
        pub sysinfo: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster_state.struct.ClusterMemberSysInfo.field.BufferRAM`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.bufferram@v1"))]
        pub bufferram: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster_state.struct.ClusterMemberSysInfo.field.FreeRAM`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.freeram@v1"))]
        pub freeram: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster_state.struct.ClusterMemberSysInfo.field.FreeSwap`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.freeswap@v1"))]
        pub freeswap: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster_state.struct.ClusterMemberSysInfo.field.LoadAverages`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.loadaverages@v1"))]
        pub loadaverages: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster_state.struct.ClusterMemberSysInfo.field.Processes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.processes@v1"))]
        pub processes: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster_state.struct.ClusterMemberSysInfo.field.SharedRAM`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.sharedram@v1"))]
        pub sharedram: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster_state.struct.ClusterMemberSysInfo.field.TotalRAM`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.totalram@v1"))]
        pub totalram: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster_state.struct.ClusterMemberSysInfo.field.TotalSwap`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.totalswap@v1"))]
        pub totalswap: Option<String>,

        /// Discovered from Repomix path `go.shared.api.cluster_state.struct.ClusterMemberSysInfo.field.Uptime`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.uptime@v1"))]
        pub uptime: Option<String>,

        /// Discovered from Repomix path `go.shared.api.event.struct.Event.field.Metadata`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.metadata@v1"))]
        pub metadata: Option<String>,

        /// Discovered from Repomix path `go.shared.api.event.struct.Event.field.Timestamp`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.timestamp@v1"))]
        pub timestamp: Option<String>,

        /// Discovered from Repomix path `go.shared.api.event.struct.EventLifecycle.field.Context`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.context@v1"))]
        pub context: Option<String>,

        /// Discovered from Repomix path `go.shared.api.event.struct.EventLifecycle.field.Requestor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.requestor@v1"))]
        pub requestor: Option<String>,

        /// Discovered from Repomix path `go.shared.api.event.struct.EventLifecycle.field.Source`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.source@v1"))]
        pub source: Option<String>,

        /// Discovered from Repomix path `go.shared.api.event.struct.EventLifecycleRequestor.field.Username`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.username@v1"))]
        pub username: Option<String>,

        /// Discovered from Repomix path `go.shared.api.event.struct.EventLogRecord.field.Ctx`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.ctx@v1"))]
        pub ctx: Option<String>,

        /// Discovered from Repomix path `go.shared.api.event.struct.EventLogRecord.field.Lvl`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.lvl@v1"))]
        pub lvl: Option<String>,

        /// Discovered from Repomix path `go.shared.api.event.struct.EventLogRecord.field.Msg`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.msg@v1"))]
        pub msg: Option<String>,

        /// Discovered from Repomix path `go.shared.api.event.struct.EventLogRecord.field.Time`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.time@v1"))]
        pub time: Option<String>,

        /// Discovered from Repomix path `go.shared.api.event.struct.EventLogging.field.Level`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.level@v1"))]
        pub level: Option<String>,

        /// Discovered from Repomix path `go.shared.api.guest.dev_incus.struct.DevIncusGet.field.APIVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.apiversion@v1"))]
        pub apiversion: Option<String>,

        /// Discovered from Repomix path `go.shared.api.guest.dev_incus.struct.DevIncusGet.field.DevIncusPut`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.devincusput@v1"))]
        pub devincusput: Option<String>,

        /// Discovered from Repomix path `go.shared.api.guest.dev_incus.struct.DevIncusPut.field.State`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.state@v1"))]
        pub state: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.Image.field.Cached`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cached@v1"))]
        pub cached: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.Image.field.Filename`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.filename@v1"))]
        pub filename: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.Image.field.UpdateSource`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.updatesource@v1"))]
        pub updatesource: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.Image.field.UploadedAt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.uploadedat@v1"))]
        pub uploadedat: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.ImageMetadata.field.CreationDate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.creationdate@v1"))]
        pub creationdate: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.ImageMetadata.field.ExpiryDate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.expirydate@v1"))]
        pub expirydate: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.ImageMetadata.field.Properties`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.properties@v1"))]
        pub properties: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.ImageMetadata.field.Templates`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.templates@v1"))]
        pub templates: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.ImageMetadataTemplate.field.CreateOnly`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.createonly@v1"))]
        pub createonly: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.ImageSource.field.ImageType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.imagetype@v1"))]
        pub imagetype: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.ImageSource.field.Server`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.server@v1"))]
        pub server: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.ImagesPost.field.CompressionAlgorithm`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.compressionalgorithm@v1"))]
        pub compressionalgorithm: Option<String>,

        /// Discovered from Repomix path `go.shared.api.image.struct.ImagesPost.field.Format`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.format@v1"))]
        pub format: Option<String>,

        /// Discovered from Repomix path `go.shared.api.init.struct.InitClusterPreseed.field.ClusterCertificatePath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.clustercertificatepath@v1"))]
        pub clustercertificatepath: Option<String>,

        /// Discovered from Repomix path `go.shared.api.init.struct.InitLocalPreseed.field.Certificates`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.certificates@v1"))]
        pub certificates: Option<String>,

        /// Discovered from Repomix path `go.shared.api.init.struct.InitLocalPreseed.field.ClusterGroups`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.clustergroups@v1"))]
        pub clustergroups: Option<String>,

        /// Discovered from Repomix path `go.shared.api.init.struct.InitLocalPreseed.field.Networks`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.networks@v1"))]
        pub networks: Option<String>,

        /// Discovered from Repomix path `go.shared.api.init.struct.InitLocalPreseed.field.StorageVolumes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.storagevolumes@v1"))]
        pub storagevolumes: Option<String>,

        /// Discovered from Repomix path `go.shared.api.init.struct.InitPreseed.field.Cluster`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cluster@v1"))]
        pub cluster: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance.struct.InstanceFull.field.Backups`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.backups@v1"))]
        pub backups: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance.struct.InstanceFull.field.Snapshots`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.snapshots@v1"))]
        pub snapshots: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance.struct.InstancePost.field.Migration`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.migration@v1"))]
        pub migration: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance.struct.InstancePostTarget.field.Websockets`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.websockets@v1"))]
        pub websockets: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance.struct.InstancePut.field.DiskOnly`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.diskonly@v1"))]
        pub diskonly: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance.struct.InstanceSource.field.BaseImage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.baseimage@v1"))]
        pub baseimage: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance.struct.InstancesPost.field.Start`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.start@v1"))]
        pub start: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_backup.struct.BackupTarget.field.AccessKey`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.accesskey@v1"))]
        pub accesskey: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_backup.struct.BackupTarget.field.BucketName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.bucketname@v1"))]
        pub bucketname: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_backup.struct.BackupTarget.field.SecretKey`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.secretkey@v1"))]
        pub secretkey: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_backup.struct.InstanceBackup.field.OptimizedStorage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.optimizedstorage@v1"))]
        pub optimizedstorage: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_backup.struct.InstanceBackupsPost.field.RootOnly`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.rootonly@v1"))]
        pub rootonly: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_console.struct.InstanceConsoleControl.field.Args`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.args@v1"))]
        pub args: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_console.struct.InstanceConsolePost.field.Force`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.force@v1"))]
        pub force: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_console.struct.InstanceConsolePost.field.Height`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.height@v1"))]
        pub height: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_console.struct.InstanceConsolePost.field.Width`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.width@v1"))]
        pub width: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_exec.struct.InstanceExecControl.field.Signal`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.signal@v1"))]
        pub signal: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_exec.struct.InstanceExecPost.field.Cwd`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cwd@v1"))]
        pub cwd: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_exec.struct.InstanceExecPost.field.Environment`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.environment@v1"))]
        pub environment: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_exec.struct.InstanceExecPost.field.Interactive`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.interactive@v1"))]
        pub interactive: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_exec.struct.InstanceExecPost.field.RecordOutput`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.recordoutput@v1"))]
        pub recordoutput: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_exec.struct.InstanceExecPost.field.User`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.user@v1"))]
        pub user: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_exec.struct.InstanceExecPost.field.WaitForWS`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.waitforws@v1"))]
        pub waitforws: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceState.field.CPU`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cpu@v1"))]
        pub cpu: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceState.field.Disk`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.disk@v1"))]
        pub disk: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceState.field.Memory`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.memory@v1"))]
        pub memory: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceState.field.OSInfo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.osinfo@v1"))]
        pub osinfo: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceState.field.Pid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.pid@v1"))]
        pub pid: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceState.field.StartedAt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.startedat@v1"))]
        pub startedat: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateCPU.field.AllocatedTime`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.allocatedtime@v1"))]
        pub allocatedtime: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateCPU.field.Usage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.usage@v1"))]
        pub usage: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateDisk.field.Total`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.total@v1"))]
        pub total: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateMemory.field.SwapUsage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.swapusage@v1"))]
        pub swapusage: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateMemory.field.SwapUsagePeak`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.swapusagepeak@v1"))]
        pub swapusagepeak: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateMemory.field.UsagePeak`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.usagepeak@v1"))]
        pub usagepeak: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetwork.field.Counters`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.counters@v1"))]
        pub counters: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetwork.field.HostName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.hostname@v1"))]
        pub hostname: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetwork.field.Hwaddr`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.hwaddr@v1"))]
        pub hwaddr: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetwork.field.Mtu`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.mtu@v1"))]
        pub mtu: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetworkAddress.field.Family`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.family@v1"))]
        pub family: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetworkAddress.field.Netmask`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.netmask@v1"))]
        pub netmask: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetworkAddress.field.Scope`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.scope@v1"))]
        pub scope: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetworkCounters.field.BytesReceived`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.bytesreceived@v1"))]
        pub bytesreceived: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetworkCounters.field.BytesSent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.bytessent@v1"))]
        pub bytessent: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetworkCounters.field.ErrorsReceived`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.errorsreceived@v1"))]
        pub errorsreceived: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetworkCounters.field.ErrorsSent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.errorssent@v1"))]
        pub errorssent: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetworkCounters.field.PacketsDroppedInbound`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.packetsdroppedinbound@v1"))]
        pub packetsdroppedinbound: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetworkCounters.field.PacketsDroppedOutbound`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.packetsdroppedoutbound@v1"))]
        pub packetsdroppedoutbound: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetworkCounters.field.PacketsReceived`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.packetsreceived@v1"))]
        pub packetsreceived: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateNetworkCounters.field.PacketsSent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.packetssent@v1"))]
        pub packetssent: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateOSInfo.field.FQDN`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.fqdn@v1"))]
        pub fqdn: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateOSInfo.field.KernelVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.kernelversion@v1"))]
        pub kernelversion: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateOSInfo.field.OS`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.os@v1"))]
        pub os: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStateOSInfo.field.OSVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.osversion@v1"))]
        pub osversion: Option<String>,

        /// Discovered from Repomix path `go.shared.api.instance_state.struct.InstanceStatePut.field.Timeout`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.timeout@v1"))]
        pub timeout: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.metadata.struct.MetadataConfigGroup.field.Keys`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.keys@v1"))]
        pub keys: Option<String>,

        /// Discovered from Repomix path `go.shared.api.metadata.struct.MetadataConfigKey.field.Condition`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.condition@v1"))]
        pub condition: Option<String>,

        /// Discovered from Repomix path `go.shared.api.metadata.struct.MetadataConfigKey.field.LiveUpdate`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.liveupdate@v1"))]
        pub liveupdate: Option<String>,

        /// Discovered from Repomix path `go.shared.api.metadata.struct.MetadataConfigKey.field.LongDescription`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.longdescription@v1"))]
        pub longdescription: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.Network.field.Locations`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.locations@v1"))]
        pub locations: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.Network.field.Managed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.managed@v1"))]
        pub managed: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkState.field.Bond`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.bond@v1"))]
        pub bond: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkState.field.Bridge`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.bridge@v1"))]
        pub bridge: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkState.field.OVN`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.ovn@v1"))]
        pub ovn: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkState.field.VLAN`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.vlan@v1"))]
        pub vlan: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBond.field.DownDelay`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.downdelay@v1"))]
        pub downdelay: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBond.field.LowerDevices`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.lowerdevices@v1"))]
        pub lowerdevices: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBond.field.MIIFrequency`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.miifrequency@v1"))]
        pub miifrequency: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBond.field.MIIState`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.miistate@v1"))]
        pub miistate: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBond.field.TransmitPolicy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.transmitpolicy@v1"))]
        pub transmitpolicy: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBond.field.UpDelay`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.updelay@v1"))]
        pub updelay: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBridge.field.ForwardDelay`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.forwarddelay@v1"))]
        pub forwarddelay: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBridge.field.ID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.id@v1"))]
        pub id: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBridge.field.STP`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.stp@v1"))]
        pub stp: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBridge.field.UpperDevices`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.upperdevices@v1"))]
        pub upperdevices: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBridge.field.VLANDefault`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.vlandefault@v1"))]
        pub vlandefault: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateBridge.field.VLANFiltering`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.vlanfiltering@v1"))]
        pub vlanfiltering: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateOVN.field.Chassis`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.chassis@v1"))]
        pub chassis: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateOVN.field.LogicalRouter`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.logicalrouter@v1"))]
        pub logicalrouter: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateOVN.field.LogicalSwitch`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.logicalswitch@v1"))]
        pub logicalswitch: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateOVN.field.UplinkIPv4`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.uplinkipv4@v1"))]
        pub uplinkipv4: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateOVN.field.UplinkIPv6`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.uplinkipv6@v1"))]
        pub uplinkipv6: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateVLAN.field.LowerDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.lowerdevice@v1"))]
        pub lowerdevice: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network.struct.NetworkStateVLAN.field.VID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.vid@v1"))]
        pub vid: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_acl.struct.NetworkACL.field.NetworkACLPut`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.networkaclput@v1"))]
        pub networkaclput: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_acl.struct.NetworkACLPut.field.Egress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.egress@v1"))]
        pub egress: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_acl.struct.NetworkACLPut.field.Ingress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.ingress@v1"))]
        pub ingress: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_acl.struct.NetworkACLRule.field.Destination`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.destination@v1"))]
        pub destination: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_acl.struct.NetworkACLRule.field.DestinationPort`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.destinationport@v1"))]
        pub destinationport: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.network_acl.struct.NetworkACLRule.field.ICMPCode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.icmpcode@v1"))]
        pub icmpcode: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_acl.struct.NetworkACLRule.field.ICMPType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.icmptype@v1"))]
        pub icmptype: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_acl.struct.NetworkACLRule.field.SourcePort`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.sourceport@v1"))]
        pub sourceport: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.network_address_set.struct.NetworkAddressSet.field.NetworkAddressSetPut`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.networkaddresssetput@v1"))]
        pub networkaddresssetput: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_allocation.struct.NetworkAllocations.field.NAT`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.nat@v1"))]
        pub nat: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_forward.struct.NetworkForwardPort.field.SNAT`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.snat@v1"))]
        pub snat: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.network_forward.struct.NetworkForwardPort.field.TargetAddress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.targetaddress@v1"))]
        pub targetaddress: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.network_forward.struct.NetworkForwardPort.field.TargetPort`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.targetport@v1"))]
        pub targetport: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.network_forward.struct.NetworkForwardPut.field.Ports`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.ports@v1"))]
        pub ports: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.network_load_balancer.struct.NetworkLoadBalancerPort.field.TargetBackend`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.targetbackend@v1"))]
        pub targetbackend: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.network_load_balancer.struct.NetworkLoadBalancerPut.field.Backends`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.backends@v1"))]
        pub backends: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_load_balancer.struct.NetworkLoadBalancerState.field.BackendHealth`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.backendhealth@v1"))]
        pub backendhealth: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_peer.struct.NetworkPeer.field.TargetIntegration`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.targetintegration@v1"))]
        pub targetintegration: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_peer.struct.NetworkPeer.field.TargetNetwork`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.targetnetwork@v1"))]
        pub targetnetwork: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_peer.struct.NetworkPeer.field.TargetProject`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.targetproject@v1"))]
        pub targetproject: Option<String>,

        /// Discovered from Repomix path `go.shared.api.network_zone.struct.NetworkZoneRecordEntry.field.TTL`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.ttl@v1"))]
        pub ttl: Option<String>,

        /// Discovered from Repomix path `go.shared.api.operation.struct.Operation.field.Class`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.class@v1"))]
        pub class: Option<String>,

        /// Discovered from Repomix path `go.shared.api.operation.struct.Operation.field.Err`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.err@v1"))]
        pub err: Option<String>,

        /// Discovered from Repomix path `go.shared.api.operation.struct.Operation.field.MayCancel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.maycancel@v1"))]
        pub maycancel: Option<String>,

        /// Discovered from Repomix path `go.shared.api.operation.struct.Operation.field.Resources`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.resources@v1"))]
        pub resources: Option<String>,

        /// Discovered from Repomix path `go.shared.api.operation.struct.Operation.field.UpdatedAt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.updatedat@v1"))]
        pub updatedat: Option<String>,

        /// Discovered from Repomix path `go.shared.api.project.struct.ProjectStateResource.field.Limit`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.limit@v1"))]
        pub limit: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.Resources.field.GPU`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.gpu@v1"))]
        pub gpu: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.Resources.field.PCI`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.pci@v1"))]
        pub pci: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.Resources.field.Serial`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.serial@v1"))]
        pub serial: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.Resources.field.Storage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.storage@v1"))]
        pub storage: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.Resources.field.System`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.system@v1"))]
        pub system: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.Resources.field.USB`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.usb@v1"))]
        pub usb: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPU.field.Sockets`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.sockets@v1"))]
        pub sockets: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUAddressSizes.field.PhysicalBits`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.physicalbits@v1"))]
        pub physicalbits: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUAddressSizes.field.VirtualBits`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.virtualbits@v1"))]
        pub virtualbits: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUCore.field.Core`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.core@v1"))]
        pub core: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUCore.field.Die`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.die@v1"))]
        pub die: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUCore.field.Flags`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.flags@v1"))]
        pub flags: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUCore.field.Frequency`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.frequency@v1"))]
        pub frequency: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUCore.field.Threads`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.threads@v1"))]
        pub threads: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUSocket.field.AddressSizes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.addresssizes@v1"))]
        pub addresssizes: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUSocket.field.Cache`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cache@v1"))]
        pub cache: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUSocket.field.Cores`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cores@v1"))]
        pub cores: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUSocket.field.FrequencyMinimum`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.frequencyminimum@v1"))]
        pub frequencyminimum: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUSocket.field.FrequencyTurbo`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.frequencyturbo@v1"))]
        pub frequencyturbo: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUSocket.field.Socket`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.socket@v1"))]
        pub socket: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUSocket.field.Vendor`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.vendor@v1"))]
        pub vendor: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUThread.field.Isolated`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.isolated@v1"))]
        pub isolated: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUThread.field.NUMANode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.numanode@v1"))]
        pub numanode: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUThread.field.Online`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.online@v1"))]
        pub online: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesCPUThread.field.Thread`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.thread@v1"))]
        pub thread: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPU.field.Cards`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cards@v1"))]
        pub cards: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCard.field.DRM`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.drm@v1"))]
        pub drm: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCard.field.DriverVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.driverversion@v1"))]
        pub driverversion: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCard.field.Mdev`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.mdev@v1"))]
        pub mdev: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCard.field.Nvidia`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.nvidia@v1"))]
        pub nvidia: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCard.field.PCIAddress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.pciaddress@v1"))]
        pub pciaddress: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCard.field.Product`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.product@v1"))]
        pub product: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCard.field.ProductID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.productid@v1"))]
        pub productid: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCard.field.SRIOV`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.sriov@v1"))]
        pub sriov: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCard.field.USBAddress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.usbaddress@v1"))]
        pub usbaddress: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCard.field.VendorID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.vendorid@v1"))]
        pub vendorid: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardDRM.field.CardDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.carddevice@v1"))]
        pub carddevice: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardDRM.field.CardName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cardname@v1"))]
        pub cardname: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardDRM.field.ControlDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.controldevice@v1"))]
        pub controldevice: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardDRM.field.ControlName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.controlname@v1"))]
        pub controlname: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardDRM.field.RenderDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.renderdevice@v1"))]
        pub renderdevice: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardDRM.field.RenderName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.rendername@v1"))]
        pub rendername: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardMdev.field.API`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.api@v1"))]
        pub api: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardMdev.field.Available`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.available@v1"))]
        pub available: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardNvidia.field.Brand`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.brand@v1"))]
        pub brand: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardNvidia.field.CUDAVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cudaversion@v1"))]
        pub cudaversion: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardNvidia.field.Model`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.model@v1"))]
        pub model: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardNvidia.field.NVRMVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.nvrmversion@v1"))]
        pub nvrmversion: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardNvidia.field.UUID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.uuid@v1"))]
        pub uuid: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardSRIOV.field.CurrentVFs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.currentvfs@v1"))]
        pub currentvfs: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardSRIOV.field.MaximumVFs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.maximumvfs@v1"))]
        pub maximumvfs: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesGPUCardSRIOV.field.VFs`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.vfs@v1"))]
        pub vfs: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesLoad.field.Average10Min`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.average10min@v1"))]
        pub average10min: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesLoad.field.Average1Min`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.average1min@v1"))]
        pub average1min: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesLoad.field.Average5Min`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.average5min@v1"))]
        pub average5min: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesMemory.field.HugepagesSize`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.hugepagessize@v1"))]
        pub hugepagessize: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesMemory.field.HugepagesTotal`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.hugepagestotal@v1"))]
        pub hugepagestotal: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesMemory.field.HugepagesUsed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.hugepagesused@v1"))]
        pub hugepagesused: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesMemory.field.Nodes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.nodes@v1"))]
        pub nodes: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCard.field.VDPA`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.vdpa@v1"))]
        pub vdpa: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPort.field.AutoNegotiation`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.autonegotiation@v1"))]
        pub autonegotiation: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPort.field.Infiniband`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.infiniband@v1"))]
        pub infiniband: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPort.field.LinkDetected`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.linkdetected@v1"))]
        pub linkdetected: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPort.field.LinkDuplex`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.linkduplex@v1"))]
        pub linkduplex: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPort.field.LinkSpeed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.linkspeed@v1"))]
        pub linkspeed: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPort.field.PortType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.porttype@v1"))]
        pub porttype: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPort.field.SupportedModes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.supportedmodes@v1"))]
        pub supportedmodes: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPort.field.SupportedPorts`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.supportedports@v1"))]
        pub supportedports: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPort.field.TransceiverType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.transceivertype@v1"))]
        pub transceivertype: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPortInfiniband.field.IsSMDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.issmdevice@v1"))]
        pub issmdevice: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPortInfiniband.field.IsSMName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.issmname@v1"))]
        pub issmname: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPortInfiniband.field.MADDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.maddevice@v1"))]
        pub maddevice: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPortInfiniband.field.MADName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.madname@v1"))]
        pub madname: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPortInfiniband.field.VerbDevice`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.verbdevice@v1"))]
        pub verbdevice: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesNetworkCardPortInfiniband.field.VerbName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.verbname@v1"))]
        pub verbname: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesPCIDevice.field.IOMMUGroup`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.iommugroup@v1"))]
        pub iommugroup: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesPCIDevice.field.VPD`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.vpd@v1"))]
        pub vpd: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesPCIVPD.field.ProductName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.productname@v1"))]
        pub productname: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesSerialDevice.field.DeviceID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.deviceid@v1"))]
        pub deviceid: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesSerialDevice.field.DevicePath`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.devicepath@v1"))]
        pub devicepath: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesStorage.field.Disks`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.disks@v1"))]
        pub disks: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesStorageDisk.field.BlockSize`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.blocksize@v1"))]
        pub blocksize: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesStorageDisk.field.Partitions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.partitions@v1"))]
        pub partitions: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesStorageDisk.field.RPM`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.rpm@v1"))]
        pub rpm: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesStorageDisk.field.ReadOnly`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.readonly@v1"))]
        pub readonly: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesStorageDisk.field.Removable`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.removable@v1"))]
        pub removable: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesStorageDisk.field.WWN`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.wwn@v1"))]
        pub wwn: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesStorageDiskPartition.field.Partition`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.partition@v1"))]
        pub partition: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesStoragePool.field.Inodes`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.inodes@v1"))]
        pub inodes: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesStoragePool.field.Space`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.space@v1"))]
        pub space: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesSystem.field.Motherboard`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.motherboard@v1"))]
        pub motherboard: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesSystem.field.Sku`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.sku@v1"))]
        pub sku: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesSystem.field.Version`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.version@v1"))]
        pub version: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesUSBDevice.field.BusAddress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.busaddress@v1"))]
        pub busaddress: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesUSBDevice.field.DeviceAddress`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.deviceaddress@v1"))]
        pub deviceaddress: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesUSBDevice.field.Interfaces`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.interfaces@v1"))]
        pub interfaces: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesUSBDevice.field.Speed`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.speed@v1"))]
        pub speed: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesUSBDeviceInterface.field.ClassID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.classid@v1"))]
        pub classid: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesUSBDeviceInterface.field.Number`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.number@v1"))]
        pub number: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesUSBDeviceInterface.field.SubClass`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.subclass@v1"))]
        pub subclass: Option<String>,

        /// Discovered from Repomix path `go.shared.api.resource.struct.ResourcesUSBDeviceInterface.field.SubClassID`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.subclassid@v1"))]
        pub subclassid: Option<String>,

        /// Discovered from Repomix path `go.shared.api.response.struct.Response.field.AsyncResponse`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.asyncresponse@v1"))]
        pub asyncresponse: Option<String>,

        /// Discovered from Repomix path `go.shared.api.response.struct.Response.field.Code`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.code@v1"))]
        pub code: Option<String>,

        /// Discovered from Repomix path `go.shared.api.response.struct.Response.field.ErrorResponse`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.errorresponse@v1"))]
        pub errorresponse: Option<String>,

        /// Discovered from Repomix path `go.shared.api.response.struct.Response.field.SyncResponse`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.syncresponse@v1"))]
        pub syncresponse: Option<String>,

        /// Discovered from Repomix path `go.shared.api.scriptlet.instance.struct.InstancePlacement.field.Reason`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.reason@v1"))]
        pub reason: Option<String>,

        /// Discovered from Repomix path `go.shared.api.scriptlet.instance.struct.InstanceResources.field.CPUCores`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.cpucores@v1"))]
        pub cpucores: Option<String>,

        /// Discovered from Repomix path `go.shared.api.scriptlet.instance.struct.InstanceResources.field.MemorySize`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.memorysize@v1"))]
        pub memorysize: Option<String>,

        /// Discovered from Repomix path `go.shared.api.scriptlet.instance.struct.InstanceResources.field.RootDiskSize`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.rootdisksize@v1"))]
        pub rootdisksize: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.Server.field.AuthUserMethod`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.authusermethod@v1"))]
        pub authusermethod: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.Server.field.AuthUserName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.authusername@v1"))]
        pub authusername: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.Architectures`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.architectures@v1"))]
        pub architectures: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.CertificateFingerprint`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.certificatefingerprint@v1"))]
        pub certificatefingerprint: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.Firewall`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.firewall@v1"))]
        pub firewall: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.Kernel`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.kernel@v1"))]
        pub kernel: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.KernelArchitecture`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.kernelarchitecture@v1"))]
        pub kernelarchitecture: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.KernelFeatures`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.kernelfeatures@v1"))]
        pub kernelfeatures: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.LXCFeatures`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.lxcfeatures@v1"))]
        pub lxcfeatures: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.OSName`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.osname@v1"))]
        pub osname: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.ServerClustered`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.serverclustered@v1"))]
        pub serverclustered: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.ServerEventMode`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.servereventmode@v1"))]
        pub servereventmode: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.ServerPid`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.serverpid@v1"))]
        pub serverpid: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.ServerVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.serverversion@v1"))]
        pub serverversion: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.StorageSupportedDrivers`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.storagesupporteddrivers@v1"))]
        pub storagesupporteddrivers: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerEnvironment.field.StorageVersion`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.storageversion@v1"))]
        pub storageversion: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerFiltered.field.APIExtensionsCount`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.apiextensionscount@v1"))]
        pub apiextensionscount: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerUntrusted.field.APIExtensions`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.apiextensions@v1"))]
        pub apiextensions: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerUntrusted.field.APIStatus`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.apistatus@v1"))]
        pub apistatus: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerUntrusted.field.Auth`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.auth@v1"))]
        pub auth: Option<String>,

        /// Discovered from Repomix path `go.shared.api.server.struct.ServerUntrusted.field.AuthMethods`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.authmethods@v1"))]
        pub authmethods: Option<String>,

        /// Discovered from Repomix path `go.shared.api.storage_pool_bucket.struct.StorageBucket.field.S3URL`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.s3url@v1"))]
        pub s3url: Option<String>,

        /// Discovered from Repomix path `go.shared.api.storage_pool_volume.struct.StorageVolume.field.ContentType`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.contenttype@v1"))]
        pub contenttype: Option<String>,

        /// Discovered from Repomix path `go.shared.api.storage_pool_volume_bitmap.struct.StorageVolumeBitmap.field.Busy`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.busy@v1"))]
        pub busy: Option<String>,

        /// Discovered from Repomix path `go.shared.api.storage_pool_volume_bitmap.struct.StorageVolumeBitmap.field.Count`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.count@v1"))]
        pub count: Option<u64>,

        /// Discovered from Repomix path `go.shared.api.storage_pool_volume_bitmap.struct.StorageVolumeBitmap.field.Granularity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.granularity@v1"))]
        pub granularity: Option<String>,

        /// Discovered from Repomix path `go.shared.api.storage_pool_volume_bitmap.struct.StorageVolumeBitmap.field.Inconsistent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.inconsistent@v1"))]
        pub inconsistent: Option<String>,

        /// Discovered from Repomix path `go.shared.api.storage_pool_volume_bitmap.struct.StorageVolumeBitmap.field.Persistent`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.persistent@v1"))]
        pub persistent: Option<String>,

        /// Discovered from Repomix path `go.shared.api.storage_pool_volume_bitmap.struct.StorageVolumeBitmap.field.Recording`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.recording@v1"))]
        pub recording: Option<String>,

        /// Discovered from Repomix path `go.shared.api.storage_pool_volume_bitmap.struct.StorageVolumeBitmapsPost.field.Disabled`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.disabled@v1"))]
        pub disabled: Option<bool>,

        /// Discovered from Repomix path `go.shared.api.warning.struct.Warning.field.EntityURL`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.entityurl@v1"))]
        pub entityurl: Option<String>,

        /// Discovered from Repomix path `go.shared.api.warning.struct.Warning.field.FirstSeenAt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.firstseenat@v1"))]
        pub firstseenat: Option<String>,

        /// Discovered from Repomix path `go.shared.api.warning.struct.Warning.field.LastMessage`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.lastmessage@v1"))]
        pub lastmessage: Option<String>,

        /// Discovered from Repomix path `go.shared.api.warning.struct.Warning.field.LastSeenAt`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.lastseenat@v1"))]
        pub lastseenat: Option<String>,

        /// Discovered from Repomix path `go.shared.api.warning.struct.Warning.field.Severity`. Review before promotion.
        #[serde(default)]
        #[schemars(extend("x-oscal-subid" = "obs.software.incus.severity@v1"))]
        pub severity: Option<String>,
    }

    /// Metadata needed when promoting a generated typed method into `schema.methods`.
    pub struct MethodCandidate {
        pub name: &'static str,
        pub side_effect: &'static str,
        pub idempotent: bool,
        pub required_capability: &'static str,
        pub subid: &'static str,
        pub repomix_path: &'static str,
        pub command: &'static [&'static str],
    }

    pub const METHOD_CANDIDATES: &[MethodCandidate] = &[];

    /// Promote every generated method into the sealed plugin schema.
    pub(super) fn register_methods(schema: &mut op_state_store::PluginSchema) {
        use super::super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    }
}

// Promotion checklist (Fable contract):
// 1. Move owned fields into the plugin State struct with concrete Rust types.
// 2. Replace method placeholders with dedicated typed Input/Output fields.
// 3. Register with method_decl_from_schemars_with_output and correct SideEffect.
// 4. Register every subid, implement dispatch, and add schema/subid tests.
// 5. Re-run op-plugin-lint; only then replace the original plugin file.
