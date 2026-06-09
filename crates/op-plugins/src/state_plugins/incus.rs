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
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Top-level state representing all Incus instances on the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncusState {
    pub instances: Vec<IncusInstance>,
}

/// A proxy device exposed as a Unix socket on the host.
/// The `id` field is the Incus device name — used as the D-Bus object path segment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IncusProxySocket {
    /// Incus device name (e.g. "grpc-socket", "mail-imap") — D-Bus path segment.
    pub id: String,
    /// Host-side listen address (e.g. "unix:/run/assistant.sock")
    pub listen: String,
    /// Container-side connect address (e.g. "tcp:127.0.0.1:50051")
    pub connect: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
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
    /// Instance configuration key-value pairs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, String>>,
    /// Non-proxy device definitions (NICs, disks, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<HashMap<String, HashMap<String, String>>>,
    /// Proxy devices exposed as Unix sockets on the host.
    /// Each entry has an `id` field (the device name) for named D-Bus paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sockets: Vec<IncusProxySocket>,
}

/// Intermediate struct for deserializing raw `incus list --format=json` output.
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

/// Derive a human-readable socket id from a connect address.
/// "tcp:127.0.0.1:50051" → "grpc", "tcp:127.0.0.1:143" → "imap", etc.
/// Falls back to the port number string, then None if unparseable.
fn socket_id_from_connect(connect: &str) -> Option<String> {
    let port_str = connect.rsplit(':').next()?;
    let port: u16 = port_str.trim().parse().ok()?;
    let name = match port {
        21 => "ftp",
        22 => "ssh",
        25 => "smtp",
        53 => "dns",
        80 => "http",
        110 => "pop3",
        143 => "imap",
        443 => "https",
        465 => "smtps",
        587 => "submission",
        993 => "imaps",
        995 => "pop3s",
        1883 => "mqtt",
        3306 => "mysql",
        5432 => "postgres",
        5672 => "amqp",
        6333 => "qdrant-http",
        6334 => "qdrant-grpc",
        8080 | 8081 | 8443 => "http-alt",
        8883 => "mqtt-tls",
        18789 => "ghostbridge",
        50051 => "grpc",
        50052 => "grpc-mcp",
        50053 => "grpc-services",
        _ => return Some(port_str.to_string()),
    };
    Some(name.to_string())
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

    /// Call Incus REST API and extract metadata from sync response
    async fn incus_api_call(method: &str, path: &str, body: Option<&str>) -> Result<Vec<u8>> {
        let response = Self::incus_api_request(method, path, body).await?;
        let mut raw = response;
        let val: simd_json::OwnedValue =
            simd_json::from_slice(&mut raw).context("Failed to parse Incus API response")?;

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
                .or_else(|| val.get("status").and_then(|v| v.as_str()))
                .unwrap_or("Unknown Incus API error");
            Err(anyhow::anyhow!("Incus API error: {}", err))
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
                    body["profiles"] = simd_json::json!([]);
                } else if !profiles.is_empty() {
                    body["profiles"] = simd_json::json!(profiles);
                }
                if let Some(pool) = storage_pool {
                    body["devices"] = simd_json::json!({
                        "root": {
                            "type": "disk",
                            "pool": pool,
                            "path": "/"
                        }
                    });
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

                // Extract proxy devices as named sockets; leave rest in devices.
                let mut sockets = Vec::new();
                let mut non_proxy_devices: HashMap<String, HashMap<String, String>> = HashMap::new();

                for (device_name, device_config) in raw.devices {
                    if device_config.get("type").map(|t| t == "proxy").unwrap_or(false) {
                        if let (Some(listen), Some(connect)) = (
                            device_config.get("listen").cloned(),
                            device_config.get("connect").cloned(),
                        ) {
                            let id = socket_id_from_connect(&connect).unwrap_or(device_name);
                            sockets.push(IncusProxySocket {
                                id,
                                listen,
                                connect,
                                bind: device_config.get("bind").cloned(),
                                uid: device_config.get("uid").cloned(),
                                gid: device_config.get("gid").cloned(),
                                mode: device_config.get("mode").cloned(),
                            });
                        }
                    } else {
                        non_proxy_devices.insert(device_name, device_config);
                    }
                }

                sockets.sort_by(|a, b| a.id.cmp(&b.id));

                let devices = if non_proxy_devices.is_empty() {
                    None
                } else {
                    Some(non_proxy_devices)
                };

                IncusInstance {
                    name: raw.name,
                    status: raw.status,
                    instance_type: raw.instance_type,
                    image,
                    storage_pool,
                    profiles: raw.profiles,
                    description: if raw.description.is_empty() { None } else { Some(raw.description) },
                    architecture: if raw.architecture.is_empty() { None } else { Some(raw.architecture) },
                    ephemeral: Some(raw.ephemeral),
                    stateful: Some(raw.stateful),
                    created_at: if raw.created_at.is_empty() { None } else { Some(raw.created_at) },
                    last_used_at: if raw.last_used_at.is_empty() { None } else { Some(raw.last_used_at) },
                    location: if raw.location.is_empty() || raw.location == "none" { None } else { Some(raw.location) },
                    project: if raw.project.is_empty() || raw.project == "default" { None } else { Some(raw.project) },
                    config,
                    devices,
                    sockets,
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

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(super::plugin_schema_defs::incus_plugin_schema())
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
