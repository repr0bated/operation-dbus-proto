//! LXC plugin - Native Proxmox API LXC container management.
//!
//! Design
//! - Discovers LXC containers via native Proxmox REST API
//! - Creates, starts, stops, and deletes containers via API (no `pct` CLI)
//! - Correlates with OVS ports (vi{VMID}) for network integration
//! - Supports BTRFS golden images for instant container provisioning

use anyhow::Result;
use async_trait::async_trait;
use op_state::plugtree::PlugTree;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use simd_json::prelude::*;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxcState {
    pub containers: Vec<ContainerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerInfo {
    pub id: String,
    pub veth: String,
    pub bridge: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, Value>>, // extensible (includes network_type, template, etc.)
}

pub struct LxcPlugin;

impl LxcPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Apply state for a single container
    pub async fn apply_container_state(&self, container: &ContainerInfo) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        // Check if container exists
        let current_containers = self.discover_from_proxmox().await?;
        let exists = current_containers.iter().any(|c| c.id == container.id);

        if !exists {
            // Create container
            match Self::create_container(container).await {
                Ok(_) => {
                    changes_applied.push(format!("Created container {}", container.id));

                    // Start it
                    if let Err(e) = Self::start_container(&container.id).await {
                        errors.push(format!("Failed to start container {}: {}", container.id, e));
                    } else {
                        changes_applied.push(format!("Started container {}", container.id));
                    }
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to create container {}: {}",
                        container.id, e
                    ));
                }
            }
        } else {
            changes_applied.push(format!("Container {} already exists", container.id));
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    /// Check if container is running via Proxmox API
    async fn is_running_api(ct_id: &str) -> Result<bool> {
        let vmid: u32 = ct_id
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid container ID: {}", ct_id))?;

        let client = op_network::ProxmoxClient::from_env()?;
        client.is_running(vmid).await
    }

    /// Fallback: Check if container is running via cgroup (for when API is unavailable)
    fn is_running_cgroup(ct_id: &str) -> Option<bool> {
        // Proxmox systemd service path: pve-container@{vmid}.service (cgroup v2)
        let path = format!(
            "/sys/fs/cgroup/system.slice/pve-container@{}.service",
            ct_id
        );
        Some(fs::metadata(path).is_ok())
    }

    /// Discover containers from Proxmox API
    async fn discover_from_proxmox(&self) -> Result<Vec<ContainerInfo>> {
        let client = match op_network::ProxmoxClient::from_env() {
            Ok(c) => c,
            Err(_) => return Ok(Vec::new()),
        };

        // Check API availability
        if client.check_available().await.is_err() {
            log::debug!("Proxmox API not available, falling back to OVS discovery");
            return self.discover_from_ovs().await;
        }

        let containers = match client.list_containers().await {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to list containers via API: {}, falling back to OVS", e);
                return self.discover_from_ovs().await;
            }
        };

        let ovsdb = op_network::ovsdb::OvsdbClient::new();
        let bridges = ovsdb.list_bridges().await.unwrap_or_default();

        let mut results = Vec::new();
        for ct in containers {
            let ct_id = ct.vmid.to_string();
            let veth = format!("vi{}", ct_id);

            // Find which bridge this container's veth is on
            let mut found_bridge = String::new();
            for br in &bridges {
                if let Ok(ports) = ovsdb.list_bridge_ports(br).await {
                    if ports.contains(&veth) {
                        found_bridge = br.clone();
                        break;
                    }
                }
            }

            // Check running status
            let running = ct.status == "running";

            results.push(ContainerInfo {
                id: ct_id,
                veth,
                bridge: found_bridge,
                running: Some(running),
                properties: Some({
                    let mut props = HashMap::new();
                    if let Some(name) = ct.name {
                        props.insert("hostname".to_string(), Value::String(name));
                    }
                    props.insert("status".to_string(), Value::String(ct.status));
                    if let Some(mem) = ct.mem {
                        props.insert("memory_used".to_string(), json!(mem));
                    }
                    if let Some(maxmem) = ct.maxmem {
                        props.insert("memory_max".to_string(), json!(maxmem));
                    }
                    if let Some(cpu) = ct.cpu {
                        props.insert("cpu_usage".to_string(), json!(cpu));
                    }
                    if let Some(uptime) = ct.uptime {
                        props.insert("uptime".to_string(), json!(uptime));
                    }
                    props
                }),
            });
        }

        Ok(results)
    }

    /// Fallback: Discover containers from OVS ports (when API is unavailable)
    async fn discover_from_ovs(&self) -> Result<Vec<ContainerInfo>> {
        let client = op_network::ovsdb::OvsdbClient::new();
        // If OVSDB is not reachable, return empty list
        if client.list_dbs().await.is_err() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let bridges = client.list_bridges().await.unwrap_or_default();
        for br in bridges {
            let ports = client.list_bridge_ports(&br).await.unwrap_or_default();
            for p in ports {
                if let Some(ct_id) = p.strip_prefix("vi") {
                    // ensure ID is numeric-like
                    if ct_id.chars().all(|c| c.is_ascii_digit()) {
                        let running = Self::is_running_cgroup(ct_id);
                        results.push(ContainerInfo {
                            id: ct_id.to_string(),
                            veth: p.clone(),
                            bridge: br.clone(),
                            running,
                            properties: None,
                        });
                    }
                }
            }
        }
        Ok(results)
    }
}

impl Default for LxcPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlugTree for LxcPlugin {
    fn pluglet_type(&self) -> &str {
        "container"
    }

    fn pluglet_id_field(&self) -> &str {
        "id"
    }

    fn extract_pluglet_id(&self, resource: &Value) -> Result<String> {
        resource
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Container missing 'id' field"))
    }

    async fn apply_pluglet(&self, _pluglet_id: &str, desired: &Value) -> Result<ApplyResult> {
        let container: ContainerInfo = simd_json::serde::from_owned_value(desired.clone())?;
        self.apply_container_state(&container).await
    }

    async fn query_pluglet(&self, pluglet_id: &str) -> Result<Option<Value>> {
        let containers = self.discover_from_proxmox().await?;

        for container in containers {
            if container.id == pluglet_id {
                return Ok(Some(simd_json::serde::to_owned_value(container)?));
            }
        }

        Ok(None)
    }

    async fn list_pluglet_ids(&self) -> Result<Vec<String>> {
        let containers = self.discover_from_proxmox().await?;
        Ok(containers.into_iter().map(|c| c.id).collect())
    }
}

impl LxcPlugin {
    /// Find container's veth interface name
    async fn find_container_veth(ct_id: &str) -> Result<String> {
        // Standard Proxmox veth naming: vi{VMID}
        let veth_name = format!("vi{}", ct_id);

        // Check if it exists via rtnetlink
        let veth_interfaces = op_network::rtnetlink::list_veth_interfaces().await?;
        if veth_interfaces.contains(&veth_name) {
            return Ok(veth_name);
        }

        // Try to find any veth for this container
        for veth in veth_interfaces {
            if veth.contains(ct_id) {
                return Ok(veth);
            }
        }

        Err(anyhow::anyhow!(
            "Could not find veth interface for container {}",
            ct_id
        ))
    }

    /// Determine bridge based on network type
    fn get_bridge_for_network_type(container: &ContainerInfo) -> String {
        let network_type = container
            .properties
            .as_ref()
            .and_then(|p| p.get("network_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("bridge");

        match network_type {
            "netmaker" => "mesh".to_string(),     // Netmaker mesh bridge
            "bridge" => container.bridge.clone(), // Traditional bridge (ovsbr0)
            _ => container.bridge.clone(),
        }
    }

    /// Create LXC container via native Proxmox API
    async fn create_container(container: &ContainerInfo) -> Result<()> {
        log::info!("Creating LXC container {} via Proxmox API", container.id);

        let vmid: u32 = container
            .id
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid container ID: {}", container.id))?;

        // Select bridge based on network type
        let bridge = Self::get_bridge_for_network_type(container);
        log::info!("Container {} will use bridge {}", container.id, bridge);

        // Extract properties with sensible defaults
        let props = container.properties.as_ref();

        // Check if using BTRFS golden image (fast path) or tar.zst template (slow path)
        let golden_image = props
            .and_then(|p| p.get("golden_image"))
            .and_then(|v| v.as_str());

        if let Some(golden_image_name) = golden_image {
            // BTRFS snapshot path - instant container creation
            return Self::create_container_from_btrfs_snapshot(
                container,
                golden_image_name,
                &bridge,
            )
            .await;
        }

        // Use native Proxmox API for template-based creation
        let template = props
            .and_then(|p| p.get("template"))
            .and_then(|v| v.as_str())
            .unwrap_or("local-btrfs:vztmpl/debian-13-standard_13.1-2_amd64.tar.zst");

        let hostname = props
            .and_then(|p| p.get("hostname"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("ct{}", container.id));

        let memory = props
            .and_then(|p| p.get("memory"))
            .and_then(|v| v.as_u64())
            .unwrap_or(512) as u32;

        let swap = props
            .and_then(|p| p.get("swap"))
            .and_then(|v| v.as_u64())
            .unwrap_or(512) as u32;

        let storage = props
            .and_then(|p| p.get("storage"))
            .and_then(|v| v.as_str())
            .unwrap_or("local-btrfs");

        let rootfs_size = props
            .and_then(|p| p.get("rootfs_size"))
            .and_then(|v| v.as_u64())
            .unwrap_or(8);

        let rootfs = format!("{}:{}", storage, rootfs_size);

        let cores = props
            .and_then(|p| p.get("cores"))
            .and_then(|v| v.as_u64())
            .unwrap_or(2) as u32;

        let unprivileged = props
            .and_then(|p| p.get("unprivileged"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let features = props
            .and_then(|p| p.get("features"))
            .and_then(|v| v.as_str())
            .unwrap_or("nesting=1");

        let firewall = props
            .and_then(|p| p.get("firewall"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let net0 = format!(
            "name=eth0,bridge={},firewall={}",
            bridge,
            if firewall { "1" } else { "0" }
        );

        log::info!(
            "Creating container {}: template={}, memory={}MB, cores={}, rootfs={}",
            container.id,
            template,
            memory,
            cores,
            rootfs
        );

        // Build the request
        let config = op_network::CreateContainerRequest {
            vmid,
            ostemplate: template.to_string(),
            hostname: Some(hostname),
            memory: Some(memory),
            swap: Some(swap),
            cores: Some(cores),
            rootfs: Some(rootfs),
            net0: Some(net0),
            unprivileged: Some(unprivileged),
            features: Some(features.to_string()),
            onboot: props.and_then(|p| p.get("onboot")).and_then(|v| v.as_bool()),
            protection: props.and_then(|p| p.get("protection")).and_then(|v| v.as_bool()),
            nameserver: props.and_then(|p| p.get("nameserver")).and_then(|v| v.as_str()).map(String::from),
            searchdomain: props.and_then(|p| p.get("searchdomain")).and_then(|v| v.as_str()).map(String::from),
            storage: Some(storage.to_string()),
            ..Default::default()
        };

        // Execute via native API
        let client = op_network::ProxmoxClient::from_env()?;
        client.create_container_sync(&config, 300).await?;

        log::info!(
            "Container {} created successfully on bridge {} (via native API)",
            container.id,
            bridge
        );

        // Inject netmaker token for first-boot join (if netmaker network type)
        let network_type = props
            .and_then(|p| p.get("network_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("bridge");

        if network_type == "netmaker" {
            Self::inject_netmaker_token(container, storage).await?;
        }

        Ok(())
    }

    /// Create LXC container from BTRFS golden image snapshot (instant provisioning)
    async fn create_container_from_btrfs_snapshot(
        container: &ContainerInfo,
        golden_image_name: &str,
        bridge: &str,
    ) -> Result<()> {
        log::info!(
            "Creating container {} from BTRFS golden image: {}",
            container.id,
            golden_image_name
        );

        let props = container.properties.as_ref();

        // Storage backend (configurable per container)
        let storage = props
            .and_then(|p| p.get("storage"))
            .and_then(|v| v.as_str())
            .unwrap_or("local-btrfs");

        // Proxmox storage paths (adjust based on storage.cfg configuration)
        let storage_path = format!("/var/lib/pve/{}", storage);
        let golden_image_path = format!("{}/templates/subvol/{}", storage_path, golden_image_name);
        let container_rootfs = format!("{}/images/{}/rootfs", storage_path, container.id);
        let container_dir = format!("{}/images/{}", storage_path, container.id);

        // Verify golden image exists
        if tokio::fs::metadata(&golden_image_path).await.is_err() {
            return Err(anyhow::anyhow!(
                "Golden image not found: {}. Create it with: sudo ./create-btrfs-golden-image.sh {}",
                golden_image_path,
                golden_image_name
            ));
        }

        // Check if it's a BTRFS subvolume
        let check_output = tokio::process::Command::new("btrfs")
            .args(["subvolume", "show", &golden_image_path])
            .output()
            .await?;

        if !check_output.status.success() {
            return Err(anyhow::anyhow!(
                "Golden image is not a BTRFS subvolume: {}",
                golden_image_path
            ));
        }

        log::info!("✓ Golden image verified: {}", golden_image_path);

        // Create container directory
        tokio::fs::create_dir_all(&container_dir).await?;

        // Create BTRFS snapshot (instant copy-on-write)
        log::info!("Creating BTRFS snapshot...");
        let snapshot_output = tokio::process::Command::new("btrfs")
            .args([
                "subvolume",
                "snapshot",
                &golden_image_path,
                &container_rootfs,
            ])
            .output()
            .await?;

        if !snapshot_output.status.success() {
            let stderr = String::from_utf8_lossy(&snapshot_output.stderr);
            return Err(anyhow::anyhow!("BTRFS snapshot failed: {}", stderr));
        }

        log::info!("✓ BTRFS snapshot created in <1ms: {}", container_rootfs);

        // Extract properties
        let hostname = props
            .and_then(|p| p.get("hostname"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("ct{}", container.id));

        let memory = props
            .and_then(|p| p.get("memory"))
            .and_then(|v| v.as_u64())
            .unwrap_or(512);

        let swap = props
            .and_then(|p| p.get("swap"))
            .and_then(|v| v.as_u64())
            .unwrap_or(512);

        let cores = props
            .and_then(|p| p.get("cores"))
            .and_then(|v| v.as_u64())
            .unwrap_or(2);

        let unprivileged = props
            .and_then(|p| p.get("unprivileged"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let features = props
            .and_then(|p| p.get("features"))
            .and_then(|v| v.as_str())
            .unwrap_or("nesting=1");

        let firewall = props
            .and_then(|p| p.get("firewall"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Create Proxmox container configuration
        let config_path = format!("/etc/pve/lxc/{}.conf", container.id);
        let config_content = format!(
            r#"arch: amd64
cores: {}
hostname: {}
memory: {}
swap: {}
net0: name=eth0,bridge={},firewall={}
ostype: debian
rootfs: local-btrfs:images/{}/rootfs
unprivileged: {}
features: {}
"#,
            cores,
            hostname,
            memory,
            swap,
            bridge,
            if firewall { "1" } else { "0" },
            container.id,
            if unprivileged { "1" } else { "0" },
            features
        );

        // Add optional properties
        let mut config = config_content;

        if let Some(onboot) = props
            .and_then(|p| p.get("onboot"))
            .and_then(|v| v.as_bool())
        {
            config.push_str(&format!("onboot: {}\n", if onboot { "1" } else { "0" }));
        }

        if let Some(protection) = props
            .and_then(|p| p.get("protection"))
            .and_then(|v| v.as_bool())
        {
            config.push_str(&format!(
                "protection: {}\n",
                if protection { "1" } else { "0" }
            ));
        }

        if let Some(nameserver) = props
            .and_then(|p| p.get("nameserver"))
            .and_then(|v| v.as_str())
        {
            config.push_str(&format!("nameserver: {}\n", nameserver));
        }

        if let Some(searchdomain) = props
            .and_then(|p| p.get("searchdomain"))
            .and_then(|v| v.as_str())
        {
            config.push_str(&format!("searchdomain: {}\n", searchdomain));
        }

        // Write Proxmox config
        tokio::fs::write(&config_path, config).await?;

        log::info!("✓ Proxmox configuration written: {}", config_path);

        // Inject firstboot script if specified
        if let Some(firstboot_script) = props
            .and_then(|p| p.get("firstboot_script"))
            .and_then(|v| v.as_str())
        {
            Self::inject_firstboot_script(container, storage, firstboot_script).await?;
        }

        // Inject Netmaker token for netmaker network type
        let network_type = props
            .and_then(|p| p.get("network_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("bridge");

        if network_type == "netmaker" {
            Self::inject_netmaker_token(container, storage).await?;
        }

        log::info!(
            "✓ Container {} created from golden image '{}' (BTRFS snapshot)",
            container.id,
            golden_image_name
        );

        Ok(())
    }

    /// Inject firstboot script into container rootfs
    async fn inject_firstboot_script(
        container: &ContainerInfo,
        storage: &str,
        script_content: &str,
    ) -> Result<()> {
        let rootfs = format!("/var/lib/pve/{}/images/{}/rootfs", storage, container.id);
        let script_path = format!("{}/usr/local/bin/lxc-firstboot.sh", rootfs);
        let service_path = format!("{}/etc/systemd/system/lxc-firstboot.service", rootfs);

        // Create script directory if needed
        tokio::fs::create_dir_all(format!("{}/usr/local/bin", rootfs)).await?;

        // Write firstboot script
        tokio::fs::write(&script_path, script_content).await?;

        // Make executable
        tokio::process::Command::new("chmod")
            .args(["+x", &script_path])
            .output()
            .await?;

        // Create systemd service
        let service_content = r#"[Unit]
Description=LXC First Boot Initialization
After=network-online.target
Wants=network-online.target
ConditionPathExists=!/var/lib/lxc-firstboot-complete

[Service]
Type=oneshot
ExecStart=/usr/local/bin/lxc-firstboot.sh
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
"#
        .to_string();

        tokio::fs::create_dir_all(format!("{}/etc/systemd/system", rootfs)).await?;
        tokio::fs::write(&service_path, service_content).await?;

        // Enable service (create symlink)
        let symlink_dir = format!("{}/etc/systemd/system/multi-user.target.wants", rootfs);
        tokio::fs::create_dir_all(&symlink_dir).await?;

        let symlink_path = format!("{}/lxc-firstboot.service", symlink_dir);
        tokio::fs::symlink("../lxc-firstboot.service", &symlink_path)
            .await
            .ok(); // Ignore if exists

        log::info!(
            "✓ Firstboot script injected into container {}",
            container.id
        );

        Ok(())
    }

    /// Inject Netmaker enrollment token into container
    async fn inject_netmaker_token(container: &ContainerInfo, storage: &str) -> Result<()> {
        // Read token from host
        if let Ok(token_content) = tokio::fs::read_to_string("/etc/op-dbus/netmaker.env").await {
            for line in token_content.lines() {
                if let Some(token_value) = line.strip_prefix("NETMAKER_TOKEN=") {
                    let token_clean = token_value.trim_matches('"').trim();

                    let rootfs = format!("/var/lib/pve/{}/images/{}/rootfs", storage, container.id);
                    let token_path = format!("{}/etc/netmaker/enrollment-token", rootfs);

                    // Create netmaker directory
                    tokio::fs::create_dir_all(format!("{}/etc/netmaker", rootfs)).await?;

                    // Write token
                    tokio::fs::write(&token_path, token_clean).await?;

                    // Set permissions
                    tokio::process::Command::new("chmod")
                        .args(["600", &token_path])
                        .output()
                        .await?;

                    log::info!("✓ Netmaker token injected into container {}", container.id);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Cleanup OVS port for deleted container
    async fn cleanup_ovs_port_for_container(ct_id: &str) -> Result<String> {
        let client = op_network::ovsdb::OvsdbClient::new();

        // Find port names matching this container (vi{VMID} or internal_{VMID})
        let potential_ports = vec![
            format!("vi{}", ct_id),        // Proxmox veth pattern
            format!("internal_{}", ct_id), // Socket networking pattern
            format!("veth{}pl", ct_id),    // Alternative veth pattern
        ];

        // Try each potential port name
        for port_name in &potential_ports {
            // Check all bridges for this port
            if let Ok(bridges) = client.list_bridges().await {
                for bridge in bridges {
                    if let Ok(ports) = client.list_bridge_ports(&bridge).await {
                        if ports.contains(port_name) {
                            log::info!("Found port {} on bridge {}, removing", port_name, bridge);

                            // Delete the port using OVSDB
                            let operations = simd_json::json!([{
                                "op": "select",
                                "table": "Port",
                                "where": [["name", "==", port_name]],
                                "columns": ["_uuid"]
                            }]);

                            if let Ok(result) = client.transact(operations).await {
                                if let Some(rows) = result[0]["rows"].as_array() {
                                    if let Some(first_row) = rows.first() {
                                        if let Some(uuid_array) = first_row["_uuid"].as_array() {
                                            if uuid_array.len() == 2 && uuid_array[0] == "uuid" {
                                                let port_uuid = uuid_array[1].as_str().unwrap();

                                                // Get bridge UUID
                                                let bridge_ops = simd_json::json!([{
                                                    "op": "select",
                                                    "table": "Bridge",
                                                    "where": [["name", "==", &bridge]],
                                                    "columns": ["_uuid"]
                                                }]);

                                                if let Ok(bridge_result) =
                                                    client.transact(bridge_ops).await
                                                {
                                                    if let Some(bridge_rows) =
                                                        bridge_result[0]["rows"].as_array()
                                                    {
                                                        if let Some(bridge_row) =
                                                            bridge_rows.first()
                                                        {
                                                            if let Some(bridge_uuid_array) =
                                                                bridge_row["_uuid"].as_array()
                                                            {
                                                                if bridge_uuid_array.len() == 2
                                                                    && bridge_uuid_array[0]
                                                                        == "uuid"
                                                                {
                                                                    let bridge_uuid =
                                                                        bridge_uuid_array[1]
                                                                            .as_str()
                                                                            .unwrap();

                                                                    // Remove port from bridge and delete it
                                                                    let delete_ops = simd_json::json!([
                                                                        {
                                                                            "op": "mutate",
                                                                            "table": "Bridge",
                                                                            "where": [["_uuid", "==", ["uuid", bridge_uuid]]],
                                                                            "mutations": [
                                                                                ["ports", "delete", ["uuid", port_uuid]]
                                                                            ]
                                                                        },
                                                                        {
                                                                            "op": "delete",
                                                                            "table": "Port",
                                                                            "where": [["_uuid", "==", ["uuid", port_uuid]]]
                                                                        }
                                                                    ]);

                                                                    client
                                                                        .transact(delete_ops)
                                                                        .await?;
                                                                    return Ok(port_name.clone());
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(anyhow::anyhow!("No OVS port found for container {}", ct_id))
    }

    /// Start LXC container via native Proxmox API
    async fn start_container(ct_id: &str) -> Result<()> {
        log::info!("Starting container {} via Proxmox API", ct_id);

        let vmid: u32 = ct_id
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid container ID: {}", ct_id))?;

        let client = op_network::ProxmoxClient::from_env()?;
        client.start_container_sync(vmid, 60).await?;

        log::info!("Container {} started successfully (via native API)", ct_id);
        Ok(())
    }

    /// Stop LXC container via native Proxmox API
    async fn stop_container(ct_id: &str) -> Result<()> {
        log::info!("Stopping container {} via Proxmox API", ct_id);

        let vmid: u32 = ct_id
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid container ID: {}", ct_id))?;

        let client = op_network::ProxmoxClient::from_env()?;
        client.stop_container_sync(vmid, 60).await?;

        log::info!("Container {} stopped successfully (via native API)", ct_id);
        Ok(())
    }

    /// Delete LXC container via native Proxmox API
    async fn delete_container(ct_id: &str, force: bool) -> Result<()> {
        log::info!("Deleting container {} via Proxmox API (force={})", ct_id, force);

        let vmid: u32 = ct_id
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid container ID: {}", ct_id))?;

        let client = op_network::ProxmoxClient::from_env()?;

        // Stop if running
        if client.is_running(vmid).await.unwrap_or(false) {
            if force {
                client.stop_container_sync(vmid, 30).await?;
            } else {
                return Err(anyhow::anyhow!(
                    "Container {} is running. Stop it first or use force=true",
                    ct_id
                ));
            }
        }

        if force {
            let upid = client.force_delete_container(vmid).await?;
            client.wait_for_task(&upid, 120).await?;
        } else {
            client.delete_container_sync(vmid, 120).await?;
        }

        log::info!("Container {} deleted successfully (via native API)", ct_id);
        Ok(())
    }
}

#[async_trait]
impl StatePlugin for LxcPlugin {
    fn name(&self) -> &str {
        "lxc"
    }
    fn version(&self) -> &str {
        "2.0.0" // Version bump for native API support
    }

    fn is_available(&self) -> bool {
        // Try to check Proxmox API availability synchronously via environment
        // The actual check happens async in discovery
        std::path::Path::new("/etc/pve").exists()
    }

    fn unavailable_reason(&self) -> String {
        "Proxmox VE not detected (/etc/pve not found) - this plugin requires Proxmox VE".to_string()
    }

    async fn query_current_state(&self) -> Result<Value> {
        let containers = self.discover_from_proxmox().await?;
        Ok(simd_json::serde::to_owned_value(LxcState { containers })?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        // For now, emit a single modify if different; once lifecycle is defined, compute granular actions.
        let actions = if current != desired {
            vec![StateAction::Modify {
                resource: "lxc".into(),
                changes: desired.clone(),
            }]
        } else {
            vec![]
        };
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create {
                    resource: _,
                    config,
                } => {
                    let container: ContainerInfo = simd_json::serde::from_owned_value(config.clone())?;

                    // 1. Create LXC container via native API
                    match Self::create_container(&container).await {
                        Ok(_) => {
                            changes_applied.push(format!("Created container {} (via native Proxmox API)", container.id));

                            // 2. Start container to create veth interface
                            if let Err(e) = Self::start_container(&container.id).await {
                                errors.push(format!(
                                    "Failed to start container {}: {}",
                                    container.id, e
                                ));
                                continue;
                            }

                            // Wait for veth to appear
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                            // 3. Find and rename veth
                            let veth_name = format!("vi{}", container.id);
                            match Self::find_container_veth(&container.id).await {
                                Ok(old_veth) => {
                                    log::info!(
                                        "Found veth {} for container {}",
                                        old_veth,
                                        container.id
                                    );

                                    if old_veth != veth_name {
                                        match op_network::rtnetlink::link_set_name(
                                            &old_veth, &veth_name,
                                        )
                                        .await
                                        {
                                            Ok(_) => {
                                                changes_applied.push(format!(
                                                    "Renamed {} to {}",
                                                    old_veth, veth_name
                                                ));
                                            }
                                            Err(e) => {
                                                log::warn!("Failed to rename veth: {}", e);
                                                // Continue anyway, veth might work with original name
                                            }
                                        }
                                    }

                                    // 4. Network enrollment based on type
                                    let network_type = container
                                        .properties
                                        .as_ref()
                                        .and_then(|p| p.get("network_type"))
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("bridge");

                                    let target_bridge = match network_type {
                                        "netmaker" => "mesh".to_string(),
                                        _ => container.bridge.clone(),
                                    };

                                    if !target_bridge.is_empty() {
                                        let ovsdb_client = op_network::ovsdb::OvsdbClient::new();
                                        match ovsdb_client.add_port(&target_bridge, &veth_name).await {
                                            Ok(_) => {
                                                changes_applied.push(format!(
                                                    "Added {} to bridge {}",
                                                    veth_name, target_bridge
                                                ));
                                            }
                                            Err(e) => {
                                                errors.push(format!(
                                                    "Failed to add port to bridge: {}",
                                                    e
                                                ));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!(
                                        "Failed to find veth for container {}: {}",
                                        container.id,
                                        e
                                    );
                                    // Continue - container was created, just couldn't configure OVS
                                }
                            }
                        }
                        Err(e) => {
                            errors.push(format!(
                                "Failed to create container {}: {}",
                                container.id, e
                            ));
                        }
                    }
                }
                StateAction::Modify {
                    resource,
                    changes: _,
                } => {
                    // Handle container state changes (start/stop)
                    log::info!(
                        "Modify operation for container {} (not yet implemented)",
                        resource
                    );
                    changes_applied.push(format!("Skipped modify for {}", resource));
                }
                StateAction::Delete { resource } => {
                    // Delete container and cleanup OVS ports
                    log::info!("Deleting container {} and cleaning up OVS ports", resource);

                    // First, try to find and cleanup the OVS port for this container
                    let cleanup_result = Self::cleanup_ovs_port_for_container(resource).await;
                    match cleanup_result {
                        Ok(port_name) => {
                            log::info!(
                                "Cleaned up OVS port {} for container {}",
                                port_name,
                                resource
                            );
                            changes_applied.push(format!(
                                "Removed OVS port {} for container {}",
                                port_name, resource
                            ));
                        }
                        Err(e) => {
                            log::warn!(
                                "Could not cleanup OVS port for container {}: {}",
                                resource,
                                e
                            );
                        }
                    }

                    // Then delete the container via native API
                    match Self::delete_container(resource, true).await {
                        Ok(_) => {
                            changes_applied.push(format!("Deleted container {} (via native Proxmox API)", resource));
                        }
                        Err(e) => {
                            errors.push(format!("Failed to delete container {}: {}", resource, e));
                        }
                    }
                }
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
        Ok(Checkpoint {
            id: format!("lxc-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().into(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: json!({}),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: false,
            supports_verification: false,
            atomic_operations: false,
        }
    }
}
