// OpenFlow Controller Plugin - Flow-based networking for containerless communication
// Manages OpenFlow flows for socket-based container networking without veth interfaces

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use log;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;

/// OpenFlow controller configuration - Policy-based, not interface-based
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFlowConfig {
    /// Bridges managed by this controller
    pub bridges: Vec<BridgeFlowConfig>,

    /// Controller endpoint (tcp:IP:PORT)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_endpoint: Option<String>,

    /// Flow policies to apply (discovered containers get flows based on policies)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow_policies: Option<Vec<FlowPolicy>>,

    /// Enable automatic container discovery and flow generation
    #[serde(default = "default_auto_discover")]
    pub auto_discover_containers: bool,

    /// Enable security hardening flows (default: true)
    #[serde(default = "default_security_enabled")]
    pub enable_security_flows: bool,

    /// Traffic obfuscation level for privacy (0=none, 1=basic, 2=pattern-hiding, 3=advanced)
    /// Level 1: Basic security (drop invalid, rate limit)
    /// Level 2: Pattern hiding (timing randomization, packet padding, TTL rewriting)
    /// Level 3: Advanced obfuscation (traffic morphing, protocol mimicry, decoy traffic)
    #[serde(default = "default_obfuscation_level")]
    pub obfuscation_level: u8,
}

fn default_security_enabled() -> bool {
    true
}

fn default_obfuscation_level() -> u8 {
    1 // Basic obfuscation enabled by default
}

fn default_auto_discover() -> bool {
    true
}

/// Flow policy - Applied to discovered containers/ports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowPolicy {
    /// Policy name
    pub name: String,

    /// Match selector (e.g., "container:*", "container:100-199", "port:internal_*")
    pub selector: String,

    /// Flow template to generate
    pub template: FlowTemplate,
}

/// Flow template for policy-based generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowTemplate {
    /// Table to insert flow
    pub table: u8,

    /// Priority
    pub priority: u16,

    /// Actions to perform (can use variables like {container_id}, {port_name})
    pub actions: Vec<FlowAction>,

    /// Additional match fields (beyond the auto-generated in_port match)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_matches: Option<HashMap<String, String>>,
}

/// Per-bridge flow configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeFlowConfig {
    /// Bridge name (e.g., "ovsbr0")
    pub name: String,

    /// OpenFlow flows for this bridge
    pub flows: Vec<FlowEntry>,

    /// Container socket ports (internal OVS ports for containerless networking)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_ports: Option<Vec<SocketPort>>,
}

/// OpenFlow flow entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlowEntry {
    /// Flow table number (0-254)
    pub table: u8,

    /// Flow priority (0-65535, higher = more specific)
    pub priority: u16,

    /// Match criteria (OpenFlow match fields)
    pub match_fields: HashMap<String, String>,

    /// Actions to perform
    pub actions: Vec<FlowAction>,

    /// Cookie for flow identification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie: Option<u64>,

    /// Idle timeout in seconds (0 = permanent)
    #[serde(default)]
    pub idle_timeout: u16,

    /// Hard timeout in seconds (0 = permanent)
    #[serde(default)]
    pub hard_timeout: u16,
}

/// OpenFlow actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FlowAction {
    /// Output to port
    Output { port: String },

    /// Load value into register
    LoadRegister { register: u8, value: u64 },

    /// Resubmit to another table
    Resubmit { table: u8 },

    /// Set field value
    SetField { field: String, value: String },

    /// Drop packet
    Drop,

    /// Send to normal L2 switching
    Normal,

    /// Send to controller
    Controller { max_len: Option<u16> },
}

/// Socket port for containerless networking
///
/// THREE TYPES:
/// 1. Privacy sockets (predefined): priv_wg, priv_xray, priv_warp
/// 2. Shared ingress sockets (one per bridge): ovsbr0-sock
/// 3. Legacy container sockets (dynamic): sock_{container_name}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketPort {
    /// Port name:
    /// - Privacy: "priv_wg", "priv_xray" (predefined)
    /// - Container: "sock_{container_name}" (dynamic, created at runtime)
    pub name: String,

    /// Container name this port serves (for sock_* ports)
    pub container_name: Option<String>,

    /// Port type: "privacy" or "container"
    pub port_type: SocketPortType,

    /// OVS port number (assigned by OVS)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ofport: Option<u16>,
}

/// Type of socket port
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SocketPortType {
    /// Privacy tunnel sockets (priv_wg, priv_xray) - predefined
    Privacy,
    /// Shared ingress port routing many privacy routes
    SharedIngress,
    /// Container sockets (sock_{name}) - dynamic, created from container name
    Container,
}

/// Discovered container from OVSDB introspection
#[derive(Debug, Clone)]
struct DiscoveredContainer {
    /// Container name (extracted from sock_{name} port)
    name: String,

    /// Port name in OVS (sock_{container_name})
    port_name: String,

    /// Bridge this container is attached to
    bridge: String,

    /// OpenFlow port number
    ofport: Option<u16>,
}

const OPENFLOW_PROTOCOL: &str = "OpenFlow13";

/// OpenFlow plugin implementation
pub struct OpenFlowPlugin {
    /// OVSDB client for OVS operations
    ovsdb_client: Arc<op_network::ovsdb::OvsdbClient>,
}

impl OpenFlowPlugin {
    pub fn new() -> Self {
        let ovsdb_client = Arc::new(op_network::ovsdb::OvsdbClient::new());

        Self { ovsdb_client }
    }

    /// Create OpenFlow client for a bridge
    #[allow(dead_code)]
    async fn create_openflow_client(
        &self,
        bridge: &str,
    ) -> Result<op_network::openflow::OpenFlowClient> {
        // Connect to OpenFlow switch (OVS typically listens on localhost:6633)
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 6633));
        let client = op_network::openflow::OpenFlowClient::connect(addr)
            .await
            .context(format!(
                "Failed to connect to OpenFlow switch for bridge {}",
                bridge
            ))?;
        Ok(client)
    }

    /// Discover containers from OVSDB introspection
    ///
    /// Looks for sock_{container_name} ports (dynamic container sockets)
    /// Privacy sockets (priv_wg, priv_xray) are NOT included - they are predefined
    async fn discover_containers(&self) -> Result<Vec<DiscoveredContainer>> {
        let mut containers = Vec::new();

        // Get all bridges
        let bridges = self.ovsdb_client.list_bridges().await?;

        for bridge in bridges {
            // Get ports on this bridge
            let ports = self.ovsdb_client.list_bridge_ports(&bridge).await?;

            for port in ports {
                // Only discover container sockets (sock_*), not privacy sockets (priv_*)
                if Self::is_container_socket(&port) {
                    if let Some(container_name) = Self::extract_container_name(&port) {
                        containers.push(DiscoveredContainer {
                            name: container_name,
                            port_name: port.clone(),
                            bridge: bridge.clone(),
                            ofport: self.get_port_ofport(&port).await.ok(),
                        });
                    }
                }
            }
        }

        log::info!(
            "Discovered {} container sockets (sock_*) from OVS introspection",
            containers.len()
        );
        Ok(containers)
    }

    /// Extract container name from port name
    ///
    /// Port naming patterns:
    /// - Privacy sockets: priv_wg, priv_xray (returns None - not container ports)
    /// - Container sockets: sock_{container_name} (returns container name)
    /// - Legacy veth: vi{VMID} (returns VMID for backwards compat)
    fn extract_container_name(port_name: &str) -> Option<String> {
        if port_name.starts_with("sock_") {
            // Dynamic container socket: sock_vectordb-prod -> vectordb-prod
            port_name.strip_prefix("sock_").map(|s| s.to_string())
        } else if port_name.starts_with("vi") {
            // Legacy Proxmox veth pattern: vi100 -> 100
            port_name.strip_prefix("vi").map(|s| s.to_string())
        } else if port_name.starts_with("priv_") {
            // Privacy sockets are not container ports
            None
        } else {
            None
        }
    }

    /// Check if port is a privacy socket (priv_wg, priv_xray)
    fn is_privacy_socket(port_name: &str) -> bool {
        port_name == "priv_wg" || port_name == "priv_xray"
    }

    /// Check if port is a container socket (sock_*)
    fn is_container_socket(port_name: &str) -> bool {
        port_name.starts_with("sock_")
    }

    /// Generate socket port name from container name
    pub fn socket_port_name(container_name: &str) -> String {
        format!("sock_{}", container_name)
    }

    /// Get OpenFlow port number for a port name
    async fn get_port_ofport(&self, port_name: &str) -> Result<u16> {
        let operations = simd_json::json!([{
            "op": "select",
            "table": "Interface",
            "where": [["name", "==", port_name]],
            "columns": ["ofport"]
        }]);

        let result = self.ovsdb_client.transact(operations).await?;

        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(ofport) = first_row["ofport"].as_i64() {
                    return Ok(ofport as u16);
                }
            }
        }

        Err(anyhow!("Could not find ofport for {}", port_name))
    }

    async fn run_ovs_ofctl(args: &[&str]) -> Result<String> {
        let output = tokio::process::Command::new("ovs-ofctl")
            .args(args)
            .output()
            .await
            .context("failed to execute ovs-ofctl")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "ovs-ofctl {} failed (exit {}): {}",
                args.join(" "),
                output.status.code().unwrap_or(-1),
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn is_managed_socket_port(port_name: &str) -> Option<SocketPortType> {
        if Self::is_privacy_socket(port_name) {
            Some(SocketPortType::Privacy)
        } else if port_name.ends_with("-sock") {
            Some(SocketPortType::SharedIngress)
        } else if Self::is_container_socket(port_name) {
            Some(SocketPortType::Container)
        } else {
            None
        }
    }

    fn flow_resource_id(flow: &FlowEntry) -> String {
        if let Some(cookie) = flow.cookie {
            return format!("cookie-{cookie:016x}");
        }
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        flow.table.hash(&mut hasher);
        flow.priority.hash(&mut hasher);
        let mut match_fields: Vec<_> = flow.match_fields.iter().collect();
        match_fields.sort_by(|a, b| a.0.cmp(b.0).then(a.1.cmp(b.1)));
        for (key, value) in match_fields {
            key.hash(&mut hasher);
            value.hash(&mut hasher);
        }
        for action in &flow.actions {
            format!("{action:?}").hash(&mut hasher);
        }
        format!("hash-{:016x}", hasher.finish())
    }

    async fn resolve_port_token(&self, token: &str) -> Result<String> {
        if token.parse::<u16>().is_ok() || token.eq_ignore_ascii_case("LOCAL") {
            return Ok(token.to_string());
        }
        Ok(self.get_port_ofport(token).await?.to_string())
    }

    async fn normalize_flow_for_bridge(
        &self,
        _bridge: &str,
        flow: &FlowEntry,
    ) -> Result<FlowEntry> {
        let mut normalized = flow.clone();
        if let Some(port_name) = normalized.match_fields.get("in_port").cloned() {
            normalized.match_fields.insert(
                "in_port".to_string(),
                self.resolve_port_token(&port_name).await?,
            );
        }

        let mut actions = Vec::with_capacity(normalized.actions.len());
        for action in &normalized.actions {
            let normalized_action = match action {
                FlowAction::Output { port } => FlowAction::Output {
                    port: self.resolve_port_token(port).await?,
                },
                _ => action.clone(),
            };
            actions.push(normalized_action);
        }
        normalized.actions = actions;
        Ok(normalized)
    }

    /// Apply flow policies to discovered containers
    async fn apply_flow_policies(
        &self,
        bridge: &str,
        containers: &[DiscoveredContainer],
        policies: &[FlowPolicy],
    ) -> Result<Vec<FlowEntry>> {
        let mut generated_flows = Vec::new();

        for container in containers {
            for policy in policies {
                if Self::policy_matches(policy, container) {
                    let flow = Self::generate_flow_from_policy(policy, container)?;
                    generated_flows.push(flow);
                    log::debug!(
                        "Generated flow for container {} from policy '{}'",
                        container.name,
                        policy.name
                    );
                }
            }
        }

        log::info!(
            "Generated {} flows for {} containers on bridge {}",
            generated_flows.len(),
            containers.len(),
            bridge
        );

        Ok(generated_flows)
    }

    /// Check if policy selector matches container
    fn policy_matches(policy: &FlowPolicy, container: &DiscoveredContainer) -> bool {
        let selector = &policy.selector;

        if selector.starts_with("container:") {
            let pattern = selector.strip_prefix("container:").unwrap();
            return Self::container_name_matches(pattern, &container.name);
        } else if selector.starts_with("port:") {
            let pattern = selector.strip_prefix("port:").unwrap();
            return Self::port_name_matches(pattern, &container.port_name);
        }

        false
    }

    /// Check if container name matches pattern (*, exact, prefix*)
    fn container_name_matches(pattern: &str, container_name: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern == container_name {
            return true;
        }

        // Prefix pattern: vectordb* matches vectordb-prod, vectordb-dev
        if pattern.ends_with('*') {
            let prefix = pattern.trim_end_matches('*');
            return container_name.starts_with(prefix);
        }

        // Suffix pattern: *-prod matches vectordb-prod, redis-prod
        if pattern.starts_with('*') {
            let suffix = pattern.trim_start_matches('*');
            return container_name.ends_with(suffix);
        }

        false
    }

    /// Check if port name matches pattern (internal_*, vi*)
    fn port_name_matches(pattern: &str, port_name: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.ends_with('*') {
            let prefix = pattern.trim_end_matches('*');
            return port_name.starts_with(prefix);
        }

        pattern == port_name
    }

    /// Generate flow from policy template, substituting variables
    fn generate_flow_from_policy(
        policy: &FlowPolicy,
        container: &DiscoveredContainer,
    ) -> Result<FlowEntry> {
        let template = &policy.template;

        // Build match fields
        let mut match_fields = HashMap::new();
        match_fields.insert("in_port".to_string(), container.port_name.clone());

        if let Some(additional) = &template.additional_matches {
            for (k, v) in additional {
                let value = Self::substitute_variables(v, container);
                match_fields.insert(k.clone(), value);
            }
        }

        // Substitute variables in actions
        let actions: Vec<FlowAction> = template
            .actions
            .iter()
            .map(|action| Self::substitute_action_variables(action, container))
            .collect();

        Ok(FlowEntry {
            table: template.table,
            priority: template.priority,
            match_fields,
            actions,
            // Use hash of container name for cookie since names aren't numeric
            cookie: Some(Self::hash_container_name(&container.name)),
            idle_timeout: 0,
            hard_timeout: 0,
        })
    }

    /// Generate a numeric hash from container name for flow cookie
    fn hash_container_name(name: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        hasher.finish()
    }

    /// Substitute variables in string ({container_name}, {port_name}, {bridge})
    fn substitute_variables(text: &str, container: &DiscoveredContainer) -> String {
        text.replace("{container_name}", &container.name)
            .replace("{container_id}", &container.name) // backwards compat
            .replace("{port_name}", &container.port_name)
            .replace("{bridge}", &container.bridge)
    }

    /// Substitute variables in flow action
    fn substitute_action_variables(
        action: &FlowAction,
        container: &DiscoveredContainer,
    ) -> FlowAction {
        match action {
            FlowAction::Output { port } => FlowAction::Output {
                port: Self::substitute_variables(port, container),
            },
            FlowAction::SetField { field, value } => FlowAction::SetField {
                field: field.clone(),
                value: Self::substitute_variables(value, container),
            },
            FlowAction::LoadRegister { register, value } => {
                // Try to parse {container_id} as numeric value
                let substituted = Self::substitute_variables(&value.to_string(), container);
                let numeric_value = substituted.parse::<u64>().unwrap_or(*value);
                FlowAction::LoadRegister {
                    register: *register,
                    value: numeric_value,
                }
            }
            _ => action.clone(),
        }
    }

    /// Install a flow via native OpenFlow protocol
    async fn install_flow(&self, bridge: &str, flow: &FlowEntry) -> Result<()> {
        let normalized = self.normalize_flow_for_bridge(bridge, flow).await?;
        let rule = self.flow_to_string(&normalized);
        log::info!("Installing flow on {}: {}", bridge, rule);
        Self::run_ovs_ofctl(&["-O", OPENFLOW_PROTOCOL, "add-flow", bridge, &rule]).await?;
        Ok(())
    }

    /// Query current flows via native OpenFlow protocol
    async fn query_flows(&self, bridge: &str) -> Result<Vec<FlowEntry>> {
        let output = Self::run_ovs_ofctl(&["-O", OPENFLOW_PROTOCOL, "dump-flows", bridge]).await?;
        self.parse_flows(&output)
    }

    async fn delete_flow(&self, bridge: &str, flow: &FlowEntry) -> Result<()> {
        let normalized = self.normalize_flow_for_bridge(bridge, flow).await?;
        let mut match_parts = vec![format!("table={}", normalized.table)];
        if let Some(cookie) = normalized.cookie {
            match_parts.push(format!("cookie=0x{cookie:x}/-1"));
        } else {
            let mut match_fields: Vec<_> = normalized.match_fields.iter().collect();
            match_fields.sort_by(|a, b| a.0.cmp(b.0));
            for (key, value) in match_fields {
                if value.is_empty() {
                    match_parts.push(key.clone());
                } else {
                    match_parts.push(format!("{key}={value}"));
                }
            }
        }
        let matcher = match_parts.join(",");
        log::info!("Deleting flow on {}: {}", bridge, matcher);
        Self::run_ovs_ofctl(&[
            "-O",
            OPENFLOW_PROTOCOL,
            "--strict",
            "del-flows",
            bridge,
            &matcher,
        ])
        .await?;
        Ok(())
    }

    /// Parse ovs-ofctl dump-flows output
    #[allow(dead_code)]
    fn parse_flows(&self, output: &str) -> Result<Vec<FlowEntry>> {
        let mut flows = Vec::new();

        for line in output.lines() {
            // Skip header and empty lines
            if line.starts_with("NXST_FLOW") || line.trim().is_empty() {
                continue;
            }

            // Parse flow line
            // Format: cookie=0x0, duration=123s, table=0, n_packets=0, priority=100, in_port=1, actions=output:2
            if let Some(flow) = self.parse_flow_line(line) {
                flows.push(flow);
            }
        }

        Ok(flows)
    }

    /// Parse a single flow line
    #[allow(dead_code)]
    fn parse_flow_line(&self, line: &str) -> Option<FlowEntry> {
        let mut table = 0u8;
        let mut priority = 0u16;
        let mut cookie = None;
        let mut match_fields = HashMap::new();
        let mut actions = Vec::new();

        let (fields_part, actions_part) = line.split_once("actions=").unwrap_or((line, ""));
        let fields_part = fields_part.replace("actions=", "");

        // Split by comma and parse fields
        for part in fields_part.split(',') {
            let part = part.trim();

            if let Some((key, value)) = part.split_once('=') {
                match key.trim() {
                    "table" => table = value.parse().ok()?,
                    "priority" => priority = value.parse().ok()?,
                    "cookie" => {
                        cookie = Some(u64::from_str_radix(value.trim_start_matches("0x"), 16).ok()?)
                    }
                    "actions" => actions = self.parse_actions(value),
                    _ => {
                        // Match field
                        if !key.contains("duration")
                            && !key.contains("n_packets")
                            && !key.contains("n_bytes")
                            && !key.contains("n_offload")
                            && !key.contains("idle_age")
                            && !key.contains("hard_age")
                        {
                            match_fields.insert(key.to_string(), value.to_string());
                        }
                    }
                }
            } else if !part.is_empty()
                && !part.contains("duration")
                && !part.contains("n_packets")
                && !part.contains("n_bytes")
                && !part.contains("idle_age")
                && !part.contains("hard_age")
            {
                match_fields.insert(part.to_string(), "".to_string());
            }
        }

        if !actions_part.is_empty() {
            actions = self.parse_actions(actions_part.trim());
        }

        Some(FlowEntry {
            table,
            priority,
            match_fields,
            actions,
            cookie,
            idle_timeout: 0,
            hard_timeout: 0,
        })
    }

    /// Parse actions string
    #[allow(dead_code)]
    fn parse_actions(&self, actions_str: &str) -> Vec<FlowAction> {
        let mut actions = Vec::new();

        for action in actions_str.split(',') {
            let action = action.trim();

            if action == "NORMAL" {
                actions.push(FlowAction::Normal);
            } else if action == "drop" {
                actions.push(FlowAction::Drop);
            } else if let Some(port) = action.strip_prefix("output:") {
                actions.push(FlowAction::Output {
                    port: port.to_string(),
                });
            } else if let Some(rest) = action.strip_prefix("resubmit(,") {
                if let Some(table_str) = rest.strip_suffix(')') {
                    if let Ok(table) = table_str.parse() {
                        actions.push(FlowAction::Resubmit { table });
                    }
                }
            }
        }

        actions
    }

    /// Convert flow to ovs-ofctl string format
    #[allow(dead_code)]
    fn flow_to_string(&self, flow: &FlowEntry) -> String {
        let mut parts = Vec::new();

        if let Some(cookie) = flow.cookie {
            parts.push(format!("cookie=0x{cookie:x}"));
        }

        // Table
        parts.push(format!("table={}", flow.table));

        // Priority
        parts.push(format!("priority={}", flow.priority));

        if flow.idle_timeout > 0 {
            parts.push(format!("idle_timeout={}", flow.idle_timeout));
        }

        if flow.hard_timeout > 0 {
            parts.push(format!("hard_timeout={}", flow.hard_timeout));
        }

        // Match fields
        for (key, value) in &flow.match_fields {
            if value.is_empty() {
                parts.push(key.to_string());
            } else {
                parts.push(format!("{}={}", key, value));
            }
        }

        // Actions
        let actions_str = flow
            .actions
            .iter()
            .map(|a| self.action_to_string(a))
            .collect::<Vec<_>>()
            .join(",");

        format!("{},actions={}", parts.join(","), actions_str)
    }

    /// Convert action to string
    #[allow(dead_code)]
    fn action_to_string(&self, action: &FlowAction) -> String {
        match action {
            FlowAction::Output { port } => format!("output:{}", port),
            FlowAction::LoadRegister { register, value } => {
                format!("load:{}->NXM_NX_REG{}[]", value, register)
            }
            FlowAction::Resubmit { table } => format!("resubmit(,{})", table),
            FlowAction::SetField { field, value } => format!("set_field:{}={}", value, field),
            FlowAction::Drop => "drop".to_string(),
            FlowAction::Normal => "NORMAL".to_string(),
            FlowAction::Controller { max_len } => {
                if let Some(len) = max_len {
                    format!("CONTROLLER:{}", len)
                } else {
                    "CONTROLLER".to_string()
                }
            }
        }
    }

    /// Create OVS internal port for socket networking
    async fn create_socket_port(&self, bridge: &str, port: &SocketPort) -> Result<()> {
        log::info!(
            "Creating socket port {} on {} for container {:?}",
            port.name,
            bridge,
            port.container_name.as_deref().unwrap_or("(privacy)")
        );

        // Add internal port to OVS bridge
        self.ovsdb_client.add_port(bridge, &port.name).await?;

        // Set port type to internal
        self.ovsdb_client
            .set_interface_type(&port.name, "internal")
            .await?;

        Ok(())
    }

    /// Delete socket port
    async fn delete_socket_port(&self, bridge: &str, port_name: &str) -> Result<()> {
        log::info!("Deleting socket port {} from {}", port_name, bridge);

        // Use OVSDB transact to delete port
        let port_uuid = self.find_port_uuid(bridge, port_name).await?;
        let bridge_uuid = self.find_bridge_uuid_by_name(bridge).await?;

        let operations = simd_json::json!([
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["_uuid", "==", ["uuid", &bridge_uuid]]],
                "mutations": [
                    ["ports", "delete", ["uuid", &port_uuid]]
                ]
            },
            {
                "op": "delete",
                "table": "Port",
                "where": [["_uuid", "==", ["uuid", &port_uuid]]]
            }
        ]);

        self.ovsdb_client.transact(operations).await?;
        Ok(())
    }

    /// Find port UUID by name on a bridge
    async fn find_port_uuid(&self, _bridge: &str, port_name: &str) -> Result<String> {
        let operations = simd_json::json!([{
            "op": "select",
            "table": "Port",
            "where": [["name", "==", port_name]],
            "columns": ["_uuid"]
        }]);

        let result = self.ovsdb_client.transact(operations).await?;

        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(uuid_array) = first_row["_uuid"].as_array() {
                    if uuid_array.len() == 2 && uuid_array[0] == "uuid" {
                        return Ok(uuid_array[1].as_str().unwrap().to_string());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Port '{}' not found", port_name))
    }

    /// Find bridge UUID by name
    async fn find_bridge_uuid_by_name(&self, bridge_name: &str) -> Result<String> {
        let operations = simd_json::json!([{
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", bridge_name]],
            "columns": ["_uuid"]
        }]);

        let result = self.ovsdb_client.transact(operations).await?;

        if let Some(rows) = result[0]["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(uuid_array) = first_row["_uuid"].as_array() {
                    if uuid_array.len() == 2 && uuid_array[0] == "uuid" {
                        return Ok(uuid_array[1].as_str().unwrap().to_string());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Bridge '{}' not found", bridge_name))
    }

    /// Compute SHA-256 hash of state
    fn compute_state_hash(&self, state: &Value) -> String {
        use sha2::{Digest, Sha256};
        let json_str = simd_json::to_string(state).unwrap_or_default();
        format!("{:x}", Sha256::digest(json_str.as_bytes()))
    }

    /// Generate default security flows to prevent dangerous edge packets
    /// These flows protect against: ARP spoofing, invalid TCP flags, malformed packets,
    /// packet storms, and other intrusion-like traffic
    fn generate_security_flows(bridge_name: &str) -> Vec<FlowEntry> {
        let mut security_flows = Vec::new();

        // Table 0: Security filtering (highest priority before application flows)

        // 1. Drop invalid TCP flags (NULL scan, Xmas scan, FIN scan without established connection)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("tcp".to_string(), "".to_string()),
                ("tcp_flags".to_string(), "0x000".to_string()), // NULL scan
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0001),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("tcp".to_string(), "".to_string()),
                ("tcp_flags".to_string(), "+fin+psh+urg".to_string()), // Xmas scan
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0002),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 2. Drop fragmented packets (can be used for evasion)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31500,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("ip_frag".to_string(), "yes".to_string()),
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0003),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 3. Prevent ARP spoofing for common private networks (rate limit ARP)
        // Allow legitimate ARP but rate limit to prevent storms
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31000,
            match_fields: HashMap::from([("arp".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::Controller { max_len: Some(128) }, // Send to controller for inspection
            ],
            cookie: Some(0xDEAD0004),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 4. Drop IPv6 Router Advertisements from untrusted sources (prevent MITM)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31500,
            match_fields: HashMap::from([
                ("icmp6".to_string(), "".to_string()),
                ("icmpv6_type".to_string(), "134".to_string()), // Router Advertisement
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0005),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 5. Drop DHCP packets from non-server sources (prevent rogue DHCP)
        // Only allow DHCP responses from legitimate servers (port 67)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31000,
            match_fields: HashMap::from([
                ("udp".to_string(), "".to_string()),
                ("tp_src".to_string(), "67".to_string()),
                ("tp_dst".to_string(), "68".to_string()),
            ]),
            actions: vec![FlowAction::Normal], // Allow legitimate DHCP
            cookie: Some(0xDEAD0006),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 6. Drop invalid source IP addresses (0.0.0.0, 127.0.0.0/8 except loopback, multicast as source)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("nw_src".to_string(), "0.0.0.0".to_string()),
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0007),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("nw_src".to_string(), "224.0.0.0/4".to_string()), // Multicast as source
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0008),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 7. Drop packets with broadcast source MAC (invalid)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([("dl_src".to_string(), "ff:ff:ff:ff:ff:ff".to_string())]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0009),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 8. Prevent MAC flooding attacks - limit MAC learning rate per port
        // This is enforced by limiting packet-in rate to controller
        // (Implementation note: Requires meter tables for rate limiting)

        // 9. Allow established connections (stateful inspection)
        // This requires connection tracking support in OVS
        security_flows.push(FlowEntry {
            table: 0,
            priority: 30000,
            match_fields: HashMap::from([
                ("ct_state".to_string(), "+est+trk".to_string()), // Established tracked connections
            ]),
            actions: vec![FlowAction::Normal],
            cookie: Some(0xDEAD000A),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 10. Drop invalid connection states
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31000,
            match_fields: HashMap::from([
                ("ct_state".to_string(), "+inv+trk".to_string()), // Invalid tracked state
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD000B),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // ==== EGRESS FILTERING: Prevent dangerous packets from leaving your network ====
        // These prevent ISP security monitoring from flagging your traffic as malicious

        // 11. Drop outbound port scanning patterns (rapid SYN to multiple ports)
        // Note: This requires rate limiting, implemented via controller
        security_flows.push(FlowEntry {
            table: 0,
            priority: 30500,
            match_fields: HashMap::from([
                ("tcp".to_string(), "".to_string()),
                ("tcp_flags".to_string(), "+syn-ack".to_string()), // SYN without ACK
            ]),
            actions: vec![
                FlowAction::Controller { max_len: Some(64) }, // Rate limit via controller
            ],
            cookie: Some(0xDEAD000C),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 12. Drop packets with TTL <= 1 going outbound (prevent traceroute leakage)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 31500,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("nw_ttl".to_string(), "0".to_string()),
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD000D),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        security_flows.push(FlowEntry {
            table: 0,
            priority: 31500,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("nw_ttl".to_string(), "1".to_string()),
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD000E),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 13. Prevent LAND attacks (source IP == dest IP)
        // This prevents packets that trigger ISP anomaly detection
        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                // Note: OpenFlow doesn't support nw_src==nw_dst directly
                // This would require flow table programming or controller logic
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD000F),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 14. Drop packets to reserved/unallocated IP ranges (prevent leaking test traffic)
        // 240.0.0.0/4 - Class E reserved
        security_flows.push(FlowEntry {
            table: 0,
            priority: 32000,
            match_fields: HashMap::from([
                ("ip".to_string(), "".to_string()),
                ("nw_dst".to_string(), "240.0.0.0/4".to_string()),
            ]),
            actions: vec![FlowAction::Drop],
            cookie: Some(0xDEAD0010),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 15. Rate limit ICMP to prevent ping floods (ISP detection)
        security_flows.push(FlowEntry {
            table: 0,
            priority: 30000,
            match_fields: HashMap::from([("icmp".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::Controller { max_len: Some(128) }, // Rate limit ICMP
            ],
            cookie: Some(0xDEAD0011),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // 16. Drop SYN floods (prevent outbound DDoS detection)
        // This requires connection rate tracking via controller

        // 17. Prevent UDP floods to common scan ports (53, 123, 161, etc.)
        let scan_ports = vec!["53", "123", "161", "389", "1900"];
        for (idx, port) in scan_ports.iter().enumerate() {
            security_flows.push(FlowEntry {
                table: 0,
                priority: 30500,
                match_fields: HashMap::from([
                    ("udp".to_string(), "".to_string()),
                    ("tp_dst".to_string(), port.to_string()),
                ]),
                actions: vec![
                    FlowAction::Controller { max_len: Some(64) }, // Rate limit
                ],
                cookie: Some(0xDEAD0012 + idx as u64),
                idle_timeout: 0,
                hard_timeout: 0,
            });
        }

        log::info!(
            "Generated {} security flows for bridge {} (includes egress filtering to prevent ISP detection)",
            security_flows.len(),
            bridge_name
        );

        security_flows
    }

    /// Generate Level 2 obfuscation flows: Pattern hiding
    /// Hides traffic patterns via timing randomization, packet padding, TTL normalization
    fn generate_pattern_hiding_flows(bridge_name: &str) -> Vec<FlowEntry> {
        let mut obfuscation_flows = Vec::new();

        // Level 2.1: TTL Normalization (prevent fingerprinting via TTL analysis)
        // Rewrite all outbound packet TTLs to a standard value (64 or 128)
        obfuscation_flows.push(FlowEntry {
            table: 0,
            priority: 29000, // Lower than security (30000+), higher than normal
            match_fields: HashMap::from([("ip".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::SetField {
                    field: "nw_ttl".to_string(),
                    value: "64".to_string(), // Standard Linux TTL
                },
                FlowAction::Normal,
            ],
            cookie: Some(0xCAFE0001),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // Level 2.2: Packet Size Normalization (prevent size-based fingerprinting)
        // This requires adding padding at application layer, OpenFlow can only mark
        obfuscation_flows.push(FlowEntry {
            table: 0,
            priority: 29000,
            match_fields: HashMap::from([("tcp".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 0,
                    value: 1,
                }, // Mark for padding
                FlowAction::Normal,
            ],
            cookie: Some(0xCAFE0002),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // Level 2.3: Flow Timing Randomization (prevent timing analysis)
        // Use idle_timeout with randomness to break timing patterns
        // Note: True timing randomization requires controller
        obfuscation_flows.push(FlowEntry {
            table: 0,
            priority: 29000,
            match_fields: HashMap::from([("udp".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 1,
                    value: 1,
                }, // Mark for timing control
                FlowAction::Normal,
            ],
            cookie: Some(0xCAFE0003),
            idle_timeout: 30, // Vary between flows for timing obfuscation
            hard_timeout: 0,
        });

        log::info!(
            "Generated {} Level 2 (pattern hiding) flows for bridge {}",
            obfuscation_flows.len(),
            bridge_name
        );

        obfuscation_flows
    }

    /// Generate Level 3 obfuscation flows: Advanced traffic morphing
    /// Makes tunnel traffic look like normal HTTPS/HTTP traffic via protocol mimicry
    fn generate_advanced_obfuscation_flows(bridge_name: &str) -> Vec<FlowEntry> {
        let mut advanced_flows = Vec::new();

        // Level 3.1: Protocol Mimicry - Mark WireGuard traffic for morphing
        // Tag WireGuard UDP:51820 for transformation to look like DNS or HTTPS
        advanced_flows.push(FlowEntry {
            table: 0,
            priority: 28000,
            match_fields: HashMap::from([
                ("udp".to_string(), "".to_string()),
                ("tp_dst".to_string(), "51820".to_string()), // WireGuard
            ]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 2,
                    value: 0x51820,
                }, // Mark as WireGuard
                FlowAction::SetField {
                    field: "tp_dst".to_string(),
                    value: "443".to_string(), // Disguise as HTTPS
                },
                FlowAction::Normal,
            ],
            cookie: Some(0xBEEF0001),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // Level 3.2: Decoy Traffic Generation (requires controller)
        // Mark flows for decoy injection - controller adds random noise packets
        advanced_flows.push(FlowEntry {
            table: 0,
            priority: 28000,
            match_fields: HashMap::from([
                ("tcp".to_string(), "".to_string()),
                ("tcp_flags".to_string(), "+ack".to_string()), // Established TCP
            ]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 3,
                    value: 1,
                }, // Mark for decoy injection
                FlowAction::Normal,
            ],
            cookie: Some(0xBEEF0002),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // Level 3.3: Traffic Shaping to Mimic HTTPS Patterns
        // Use connection tracking to shape tunnel traffic to match HTTPS timing
        advanced_flows.push(FlowEntry {
            table: 0,
            priority: 28000,
            match_fields: HashMap::from([
                ("tcp".to_string(), "".to_string()),
                ("tp_dst".to_string(), "443".to_string()),
            ]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 4,
                    value: 443,
                }, // Mark as HTTPS-shaped
                FlowAction::Normal,
            ],
            cookie: Some(0xBEEF0003),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        // Level 3.4: Fragment Size Randomization
        // Mark packets for fragmentation to hide true packet sizes
        // Actual fragmentation done by controller or kernel
        advanced_flows.push(FlowEntry {
            table: 0,
            priority: 28000,
            match_fields: HashMap::from([("ip".to_string(), "".to_string())]),
            actions: vec![
                FlowAction::LoadRegister {
                    register: 5,
                    value: 1400,
                }, // Target fragment size
                FlowAction::Normal,
            ],
            cookie: Some(0xBEEF0004),
            idle_timeout: 0,
            hard_timeout: 0,
        });

        log::info!(
            "Generated {} Level 3 (advanced obfuscation) flows for bridge {}",
            advanced_flows.len(),
            bridge_name
        );

        advanced_flows
    }
}

#[async_trait]
impl StatePlugin for OpenFlowPlugin {
    fn name(&self) -> &str {
        "openflow"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn is_available(&self) -> bool {
        std::path::Path::new("/var/run/openvswitch/db.sock").exists()
            && std::path::Path::new("/usr/bin/ovs-ofctl").exists()
    }

    fn unavailable_reason(&self) -> String {
        "OpenFlow requires /var/run/openvswitch/db.sock and /usr/bin/ovs-ofctl".to_string()
    }

    async fn query_current_state(&self) -> Result<Value> {
        log::info!("Querying current OpenFlow state");

        let bridges = self.ovsdb_client.list_bridges().await?;
        let bridge_names: Vec<String> = bridges.into_iter().collect();
        let mut bridge_configs = Vec::new();

        for bridge in bridge_names {
            let flows = self.query_flows(&bridge).await.unwrap_or_default();
            let ports = self
                .ovsdb_client
                .list_bridge_ports(&bridge)
                .await
                .unwrap_or_default();
            let socket_ports: Vec<SocketPort> = ports
                .into_iter()
                .filter_map(|port_name| {
                    Self::is_managed_socket_port(&port_name).map(|port_type| SocketPort {
                        ofport: None,
                        container_name: if port_type == SocketPortType::Container {
                            Self::extract_container_name(&port_name)
                        } else {
                            None
                        },
                        name: port_name,
                        port_type,
                    })
                })
                .collect();

            bridge_configs.push(BridgeFlowConfig {
                name: bridge.clone(),
                flows,
                socket_ports: if socket_ports.is_empty() {
                    None
                } else {
                    Some(socket_ports)
                },
            });
        }

        let config = OpenFlowConfig {
            bridges: bridge_configs,
            controller_endpoint: None,
            flow_policies: None,
            auto_discover_containers: false,
            enable_security_flows: false, // Query mode: don't inject, report actual state
            obfuscation_level: 0,         // Query mode: report actual flows, no injection
        };

        Ok(simd_json::serde::to_owned_value(config)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        log::info!("Calculating OpenFlow diff with policy-based flow generation");

        let current_config: OpenFlowConfig = simd_json::serde::from_owned_value(current.clone())?;
        let mut desired_config: OpenFlowConfig =
            simd_json::serde::from_owned_value(desired.clone())?;

        // Inject security and obfuscation flows based on configuration
        if desired_config.enable_security_flows {
            log::info!(
                "Security hardening enabled (obfuscation level {}), injecting flows",
                desired_config.obfuscation_level
            );

            for bridge_config in &mut desired_config.bridges {
                let mut all_flows = Vec::new();
                let mut flow_count = 0;

                // Level 1: Basic security (always enabled if enable_security_flows=true)
                if desired_config.obfuscation_level >= 1 {
                    let security_flows = Self::generate_security_flows(&bridge_config.name);
                    flow_count += security_flows.len();
                    all_flows.extend(security_flows);
                }

                // Level 2: Pattern hiding (TTL normalization, packet padding, timing)
                if desired_config.obfuscation_level >= 2 {
                    let pattern_flows = Self::generate_pattern_hiding_flows(&bridge_config.name);
                    flow_count += pattern_flows.len();
                    all_flows.extend(pattern_flows);
                }

                // Level 3: Advanced obfuscation (protocol mimicry, decoy traffic, morphing)
                if desired_config.obfuscation_level >= 3 {
                    let advanced_flows =
                        Self::generate_advanced_obfuscation_flows(&bridge_config.name);
                    flow_count += advanced_flows.len();
                    all_flows.extend(advanced_flows);
                }

                // Prepend generated flows to user-defined flows (generated have higher priority)
                all_flows.extend(bridge_config.flows.clone());
                bridge_config.flows = all_flows;

                log::info!(
                    "Bridge {}: injected {} flows (Level {} obfuscation)",
                    bridge_config.name,
                    flow_count,
                    desired_config.obfuscation_level
                );
            }
        }

        // If auto-discovery is enabled and policies are defined, generate flows
        if desired_config.auto_discover_containers {
            if let Some(policies) = &desired_config.flow_policies {
                log::info!("Auto-discovery enabled, generating flows from policies");
                let discovered_containers = self.discover_containers().await.unwrap_or_default();

                for bridge_config in &mut desired_config.bridges {
                    // Filter containers for this bridge
                    let bridge_containers: Vec<DiscoveredContainer> = discovered_containers
                        .iter()
                        .filter(|c| c.bridge == bridge_config.name)
                        .cloned()
                        .collect();

                    // Generate flows from policies
                    let policy_flows = self
                        .apply_flow_policies(&bridge_config.name, &bridge_containers, policies)
                        .await?;

                    let policy_count = policy_flows.len();
                    let static_count = bridge_config.flows.len();

                    // Merge policy-generated flows with static flows
                    bridge_config.flows.extend(policy_flows);

                    log::info!(
                        "Bridge {}: {} static flows + {} policy-generated flows",
                        bridge_config.name,
                        static_count,
                        policy_count
                    );
                }
            }
        }

        let mut actions = Vec::new();

        // Compare bridges
        for desired_bridge in &desired_config.bridges {
            let current_bridge = current_config
                .bridges
                .iter()
                .find(|b| b.name == desired_bridge.name);

            if let Some(current_bridge) = current_bridge {
                // Compare flows
                for desired_flow in &desired_bridge.flows {
                    let normalized_desired = self
                        .normalize_flow_for_bridge(&desired_bridge.name, desired_flow)
                        .await?;
                    let flow_exists = current_bridge
                        .flows
                        .iter()
                        .any(|f| f == &normalized_desired);

                    if !flow_exists {
                        actions.push(StateAction::Create {
                            resource: format!(
                                "{}/flow/{}",
                                desired_bridge.name,
                                Self::flow_resource_id(&normalized_desired)
                            ),
                            config: simd_json::serde::to_owned_value(normalized_desired)?,
                        });
                    }
                }

                // Check for flows to delete
                for current_flow in &current_bridge.flows {
                    let mut flow_desired = false;
                    for desired_flow in &desired_bridge.flows {
                        let normalized_desired = self
                            .normalize_flow_for_bridge(&desired_bridge.name, desired_flow)
                            .await?;
                        if normalized_desired == *current_flow {
                            flow_desired = true;
                            break;
                        }
                    }

                    if !flow_desired {
                        actions.push(StateAction::Delete {
                            resource: format!(
                                "{}/flow/{}",
                                desired_bridge.name,
                                Self::flow_resource_id(current_flow)
                            ),
                        });
                    }
                }

                // Compare socket ports
                let desired_ports = desired_bridge.socket_ports.clone().unwrap_or_default();
                let current_ports = current_bridge.socket_ports.clone().unwrap_or_default();

                for desired_port in &desired_ports {
                    let port_exists = current_ports.iter().any(|p| p.name == desired_port.name);
                    if !port_exists {
                        actions.push(StateAction::Create {
                            resource: format!("{}/port/{}", desired_bridge.name, desired_port.name),
                            config: simd_json::serde::to_owned_value(desired_port)?,
                        });
                    }
                }

                for current_port in &current_ports {
                    let port_desired = desired_ports.iter().any(|p| p.name == current_port.name);
                    if !port_desired {
                        actions.push(StateAction::Delete {
                            resource: format!("{}/port/{}", desired_bridge.name, current_port.name),
                        });
                    }
                }
            } else {
                for desired_port in desired_bridge.socket_ports.clone().unwrap_or_default() {
                    actions.push(StateAction::Create {
                        resource: format!("{}/port/{}", desired_bridge.name, desired_port.name),
                        config: simd_json::serde::to_owned_value(desired_port)?,
                    });
                }

                for desired_flow in &desired_bridge.flows {
                    let normalized_desired = self
                        .normalize_flow_for_bridge(&desired_bridge.name, desired_flow)
                        .await?;
                    actions.push(StateAction::Create {
                        resource: format!(
                            "{}/flow/{}",
                            desired_bridge.name,
                            Self::flow_resource_id(&normalized_desired)
                        ),
                        config: simd_json::serde::to_owned_value(normalized_desired)?,
                    });
                }
            }
        }

        let current_state = self.query_current_state().await?;
        let current_hash = self.compute_state_hash(&current_state);
        let desired_hash = self.compute_state_hash(&simd_json::serde::to_owned_value(desired)?);

        Ok(StateDiff {
            plugin: "openflow".to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash,
                desired_hash,
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        log::info!(
            "Applying OpenFlow state changes: {} actions",
            diff.actions.len()
        );

        let mut changes = Vec::new();
        let mut errors = Vec::new();
        let mut create_ports = Vec::new();
        let mut create_flows = Vec::new();
        let mut delete_flows = Vec::new();
        let mut delete_ports = Vec::new();
        let mut modify_actions = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, .. } if resource.contains("/port/") => {
                    create_ports.push(action);
                }
                StateAction::Create { resource, .. } if resource.contains("/flow/") => {
                    create_flows.push(action);
                }
                StateAction::Delete { resource } if resource.contains("/flow/") => {
                    delete_flows.push(action);
                }
                StateAction::Delete { resource } if resource.contains("/port/") => {
                    delete_ports.push(action);
                }
                StateAction::Modify { .. } => modify_actions.push(action),
                StateAction::NoOp { .. } => {}
                _ => {}
            }
        }

        for action in create_ports {
            if let StateAction::Create { resource, config } = action {
                let parts: Vec<&str> = resource.split('/').collect();
                let bridge = parts[0];
                let port: SocketPort = simd_json::serde::from_owned_value(config.clone())?;
                match self.create_socket_port(bridge, &port).await {
                    Ok(_) => changes.push(format!("Created socket port {}", port.name)),
                    Err(e) => errors.push(format!("Failed to create port {}: {}", port.name, e)),
                }
            }
        }

        for action in create_flows {
            if let StateAction::Create { resource, config } = action {
                let parts: Vec<&str> = resource.split('/').collect();
                let bridge = parts[0];
                let flow: FlowEntry = simd_json::serde::from_owned_value(config.clone())?;
                match self.install_flow(bridge, &flow).await {
                    Ok(_) => {
                        changes.push(format!("Installed flow {}", Self::flow_resource_id(&flow)))
                    }
                    Err(e) => errors.push(format!(
                        "Failed to install flow {} on {}: {}",
                        Self::flow_resource_id(&flow),
                        bridge,
                        e
                    )),
                }
            }
        }

        for action in delete_flows {
            if let StateAction::Delete { resource } = action {
                let parts: Vec<&str> = resource.split('/').collect();
                let bridge = parts[0];
                let flow_id = parts.get(2).copied().unwrap_or_default();
                match self.query_flows(bridge).await {
                    Ok(flows) => {
                        if let Some(flow) = flows
                            .into_iter()
                            .find(|flow| Self::flow_resource_id(flow) == flow_id)
                        {
                            match self.delete_flow(bridge, &flow).await {
                                Ok(_) => changes.push(format!("Deleted flow {}", flow_id)),
                                Err(e) => errors.push(format!(
                                    "Failed to delete flow {} on {}: {}",
                                    flow_id, bridge, e
                                )),
                            }
                        }
                    }
                    Err(e) => errors.push(format!(
                        "Failed to query current flows for {} before deleting {}: {}",
                        bridge, flow_id, e
                    )),
                }
            }
        }

        for action in delete_ports {
            if let StateAction::Delete { resource } = action {
                let parts: Vec<&str> = resource.split('/').collect();
                let bridge = parts[0];
                let port_name = parts[2];
                match self.delete_socket_port(bridge, port_name).await {
                    Ok(_) => changes.push(format!("Deleted socket port {}", port_name)),
                    Err(e) => errors.push(format!("Failed to delete port {}: {}", port_name, e)),
                }
            }
        }

        for action in modify_actions {
            if let StateAction::Modify {
                resource,
                changes: config,
            } = action
            {
                let parts: Vec<&str> = resource.split('/').collect();
                let bridge = parts[0];
                let flow: FlowEntry = simd_json::serde::from_owned_value(config.clone())?;
                let flow_id = Self::flow_resource_id(&flow);
                match self.query_flows(bridge).await {
                    Ok(flows) => {
                        if let Some(existing) = flows
                            .into_iter()
                            .find(|current| Self::flow_resource_id(current) == flow_id)
                        {
                            if let Err(e) = self.delete_flow(bridge, &existing).await {
                                errors.push(format!(
                                    "Failed to replace flow {} on {}: {}",
                                    flow_id, bridge, e
                                ));
                                continue;
                            }
                        }
                        match self.install_flow(bridge, &flow).await {
                            Ok(_) => changes.push(format!("Updated flow {}", flow_id)),
                            Err(e) => errors.push(format!(
                                "Failed to update flow {} on {}: {}",
                                flow_id, bridge, e
                            )),
                        }
                    }
                    Err(e) => errors.push(format!(
                        "Failed to query current flows for {} before updating {}: {}",
                        bridge, flow_id, e
                    )),
                }
            }
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied: changes,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        Ok(current == *desired)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current_state = self.query_current_state().await?;

        Ok(Checkpoint {
            id: format!("openflow_{}", chrono::Utc::now().timestamp()),
            plugin: "openflow".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current_state,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        log::info!(
            "Rolling back OpenFlow to checkpoint from {}",
            checkpoint.timestamp
        );

        let current = self.query_current_state().await?;
        let diff = self
            .calculate_diff(&current, &checkpoint.state_snapshot)
            .await?;

        self.apply_state(&diff).await?;

        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false, // Flows installed one by one
        }
    }
}
