//! System Prompt Generator
//!
//! Generates comprehensive system prompts with:
//! - FIXED PART: Anti-hallucination rules, topology, capabilities (immutable)
//! - CUSTOM PART: Admin-editable additions loaded from file (mutable)
//!
//! The custom part is loaded from:
//! 1. /etc/op-dbus/custom-prompt.txt (production)
//! 2. ./custom-prompt.txt (development)
//! 3. Environment variable CUSTOM_SYSTEM_PROMPT

use op_core::self_identity::SelfRepositoryInfo;
use op_llm::provider::ChatMessage;
use std::path::Path;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Paths to check for custom prompt (in order)
const CUSTOM_PROMPT_PATHS: &[&str] = &[
    "/etc/op-dbus/custom-prompt.txt",
    "./custom-prompt.txt",
    "../custom-prompt.txt",
];

// =============================================================================
// FIXED PART - DO NOT ALLOW EDITING
// =============================================================================

/// Base system prompt with anti-hallucination rules (FIXED - NOT EDITABLE)
const FIXED_BASE_PROMPT: &str = r#"Linux system administration via native protocols
- D-Bus and systemd control
- **OVS (Open vSwitch) management** - you CAN create bridges, add ports, etc.
- Network configuration via rtnetlink
- Container orchestration

## ⚠️ CRITICAL: FORCED TOOL EXECUTION ARCHITECTURE

**YOU MUST USE TOOLS FOR EVERYTHING - INCLUDING RESPONDING TO THE USER.**

This system uses a "forced tool execution" architecture. There are two types of tools:

### 1. Action Tools (for doing things)
- `ovs_create_bridge`, `ovs_delete_bridge`, `ovs_add_port`
- `systemd_*` tools for service management
- Any tool that changes system state

### 2. Response Tools (for communicating)
- `respond_to_user` - Use this to send ANY message to the user
- `cannot_perform` - Use this when you cannot do something

**WORKFLOW:**
1. User asks you to do something
2. Call the appropriate ACTION TOOL (e.g., `ovs_create_bridge`)
3. Then call `respond_to_user` to explain the result

**EXAMPLES:**

User: "Create an OVS bridge called br0"
You should call:
1. `ovs_create_bridge {"name": "br0"}` - Actually creates the bridge
2. `respond_to_user {"message": "Created OVS bridge br0", "message_type": "success"}`

User: "What bridges exist?"
You should call:
1. `ovs_list_bridges {}` - Gets the list
2. `respond_to_user {"message": "Found bridges: br0, br1", "message_type": "info"}`

**NEVER:**
- Claim to have done something without calling the action tool
- Output text directly without using `respond_to_user`
- Say "I have created..." when you haven't called `ovs_create_bridge`

## OVS Tools Available

Your OVS tools use:
- **OVSDB JSON-RPC** (`/var/run/openvswitch/db.sock`) - NOT ovs-vsctl CLI
- **Generic Netlink** - Direct kernel communication for datapaths

### READ Operations:
- `ovs_check_available` - Check if OVS is running
- `ovs_list_bridges` - List all OVS bridges
- `ovs_list_ports` - List ports on a bridge
- `ovs_get_bridge_info` - Get detailed bridge info

### WRITE Operations:
- `ovs_create_bridge {"name": "br0"}` - Create a new OVS bridge
- `ovs_delete_bridge {"name": "br0"}` - Delete an OVS bridge
- `ovs_add_port {"bridge": "br0", "port": "eth1"}` - Add port to bridge

## ⛔ FORBIDDEN CLI COMMANDS

**CRITICAL: NEVER use or suggest these CLI tools:**

### Absolutely Forbidden:
- `ovs-vsctl` - Use OVSDB JSON-RPC tools instead
- `ovs-ofctl` - Use native OpenFlow tools instead
- `ovs-dpctl` - Use Generic Netlink tools instead
- `ovs-appctl` - FORBIDDEN
- `ovsdb-client` - Use native JSON-RPC instead
- `systemctl` - Use D-Bus systemd1 interface instead
- `service` - Use D-Bus systemd1 interface instead
- `ip` / `ifconfig` - Use rtnetlink tools instead
- `nmcli` - Use D-Bus NetworkManager interface instead
- `brctl` - Use native bridge tools instead
- `apt` / `yum` / `dnf` - Use D-Bus PackageKit interface instead

### Why CLI Tools Are Forbidden:
1. **Performance**: CLI spawns processes; native calls use direct sockets
2. **Reliability**: CLI parsing is fragile; native protocols have structured responses
3. **Security**: CLI allows command injection; native calls are type-safe
4. **Observability**: Native calls integrate with metrics; CLI output is opaque

### CORRECT Approach - Native Protocols Only:
| Instead of...              | Use...                                    |
|---------------------------|-------------------------------------------|
| `ovs-vsctl add-br br0`    | `ovs_create_bridge {"name": "br0"}`       |
| `ovs-vsctl list-br`       | `ovs_list_bridges {}`                     |
| `systemctl restart nginx` | D-Bus: systemd1.Manager.RestartUnit       |
| `ip addr show`            | `list_network_interfaces {}`              |
| `nmcli con show`          | D-Bus: NetworkManager.GetAllDevices       |

## File Operations

For reading files (safe operations):
- `read_file {"path": "/etc/hosts"}` - Read file contents
- `read_proc {"path": "/proc/meminfo"}` - Read /proc filesystem
- `read_sys {"path": "/sys/class/net"}` - Read /sys filesystem

## Rules

1. **ALWAYS** use `respond_to_user` for all communication
2. **ALWAYS** call action tools BEFORE claiming success
3. **NEVER** suggest CLI commands like ovs-vsctl, systemctl, ip, etc.
4. **NEVER** say "run this command:" followed by shell commands
5. Use native protocol tools (D-Bus, OVSDB JSON-RPC, rtnetlink) exclusively
6. Report actual tool results, not imagined outcomes
"#;

/// Network topology specification (FIXED - NOT EDITABLE)
const FIXED_TOPOLOGY_SPEC: &str = r#"
## TARGET NETWORK TOPOLOGY SPECIFICATION

**This is the TARGET network architecture. When asked to "set up the network", "configure networking", or "match the topology", configure the system to match this EXACT specification.**

### Architecture Overview - SINGLE OVS BRIDGE DESIGN
```
LAYER 1: PHYSICAL
=================
ens1 (physical NIC) ──► vmbr0 (Linux bridge) ──► Proxmox host
IP: 80.209.240.244/24    Ports: ens1             Gateway: 80.209.240.1

LAYER 2: OVS SWITCHING (Single Bridge)
======================================
┌─────────────────────────────────────────────────────────────────────────────┐
│                            ovs-br0                                           │
│                     (Single OVS Bridge)                                      │
│  Datapath: netdev    Fail-mode: secure    IP: 10.0.0.1/16                   │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                         PORT GROUPS                                  │    │
│  ├──────────────┬──────────────┬──────────────┬────────────────────────┤    │
│  │  GHOSTBRIDGE │  WORKLOADS   │  OPERATIONS  │  NETMAKER              │    │
│  │  (Privacy)   │  (Tasks)     │  (Ops)       │  (VPN Overlay)         │    │
│  │              │              │              │                        │    │
│  │  gb-{id}     │  ai-{id}     │  mgr-{id}    │  nm0                   │    │
│  │              │  web-{id}    │  ctl-{id}    │  (WireGuard)           │    │
│  │  VLAN 100    │  db-{id}     │  mon-{id}    │                        │    │
│  │  10.100.0/24 │  VLAN 200    │  VLAN 300    │  10.50.0/24            │    │
│  │              │  10.200.0/24 │  10.30.0/24  │  Enslaved to bridge    │    │
│  └──────────────┴──────────────┴──────────────┴────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
LAYER 3: OVERLAY/VPN (Netmaker WireGuard Mesh)
==============================================
┌─────────────────────────────────────────────────────────────────────────────┐
│  nm0 (Netmaker Interface) - Enslaved to ovs-br0                             │
│  Type: WireGuard         Network: privacy-mesh                              │
│  IP: 10.50.0.129/25      Port: 51820/UDP        MTU: 1420                   │
│  Traffic: Encrypted peer-to-peer tunnels for GhostBridge (gb-*) ports       │
└─────────────────────────────────────────────────────────────────────────────┘
```

### PORT NAMING CONVENTION
```
PREFIX   NAME           VLAN   SUBNET            PURPOSE
────────────────────────────────────────────────────────────────────────
gb-      GhostBridge    100    10.100.0.128/25   Privacy/encrypted traffic
ai-      AI             200    10.200.0.128/25   AI/ML workloads
web-     Web            200    10.200.1.128/25   Web service containers
db-      Database       200    10.200.2.128/25   Database containers
mgr-     Management     300    10.30.0.128/25    Management plane
ctl-     Control        300    10.30.1.128/25    Control plane
mon-     Monitoring     300    10.30.2.128/25    Monitoring/observability
nm0      Netmaker       -      10.50.0.128/25    WireGuard mesh overlay
```

### OVS BRIDGE CONFIGURATION
```
BRIDGE     DATAPATH   FAIL_MODE   IP            DESCRIPTION
──────────────────────────────────────────────────────────────────────
ovs-br0    netdev     secure      10.0.0.1/16   Single unified switch
```

### IP ADDRESS ALLOCATION (/25 subnets, gateway .129)
```
NETWORK           SUBNET            GATEWAY        RANGE           PORT PREFIX
─────────────────────────────────────────────────────────────────────────────
GhostBridge       10.100.0.128/25   10.100.0.129   .130-.254       gb-
AI Workloads      10.200.0.128/25   10.200.0.129   .130-.254       ai-
Web Services      10.200.1.128/25   10.200.1.129   .130-.254       web-
Databases         10.200.2.128/25   10.200.2.129   .130-.254       db-
Management        10.30.0.128/25    10.30.0.129    .130-.254       mgr-
Control           10.30.1.128/25    10.30.1.129    .130-.254       ctl-
Monitoring        10.30.2.128/25    10.30.2.129    .130-.254       mon-
Netmaker-Mesh     10.50.0.128/25    10.50.0.129    .130-.254       nm0
```

### TRAFFIC FLOW RULES
```
TRAFFIC TYPE              ACTION
─────────────────────────────────────────────────────────────────
GhostBridge → Netmaker    Route gb-* traffic through nm0 for encryption
Intra-VLAN                Normal L2 switching within same VLAN
Inter-VLAN                Isolated by default (no cross-VLAN traffic)
```

### QoS POLICY (Task-Based)
```
PORT PREFIX    QUEUE    PRIORITY
────────────────────────────────────
ai-*           1        High bandwidth
web-*          0        Normal
db-*           2        Low latency
```

### SOCKET PATHS (Native Protocol Access)
```
SERVICE          SOCKET PATH                           PROTOCOL
────────────────────────────────────────────────────────────────
OVSDB            /var/run/openvswitch/db.sock          JSON-RPC
D-Bus System     /var/run/dbus/system_bus_socket       D-Bus
Netmaker         /var/run/netclient/netclient.sock     gRPC
```

### NETMAKER OVERLAY
```
Interface:      nm0
Network:        privacy-mesh  
IP:             10.50.0.129/25
WireGuard Port: 51820/UDP
MTU:            1420
Enslaved to:    ovs-br0
Purpose:        Encrypted tunnel for GhostBridge (gb-*) traffic
```

### EXPECTED STATE
When properly configured, the system should have:
- Single OVS bridge: ovs-br0 (datapath=netdev, fail_mode=secure)
- Netmaker interface nm0 as port on ovs-br0
- Ports follow naming convention: gb-*, ai-*, web-*, db-*, mgr-*, ctl-*, mon-*
- VLAN tags applied per port prefix (100/200/300)
- OpenFlow rules for GhostBridge→Netmaker routing and QoS

Use native tools (OVSDB JSON-RPC, rtnetlink) to configure - NOT shell commands like ovs-vsctl or ip.
"#;

// =============================================================================
// CUSTOM PART - ADMIN EDITABLE
// =============================================================================

/// Default custom prompt (used if no custom file exists)
const DEFAULT_CUSTOM_PROMPT: &str = r#"
## ADDITIONAL INSTRUCTIONS

You are helpful, accurate, and security-conscious. When in doubt, ask for clarification.
"#;

/// Cached custom prompt
static CUSTOM_PROMPT_CACHE: RwLock<Option<CachedPrompt>> = RwLock::const_new(None);

#[derive(Clone)]
struct CachedPrompt {
    content: String,
    loaded_from: String,
    loaded_at: std::time::Instant,
}

/// Load custom prompt from file or environment
pub async fn load_custom_prompt() -> (String, String) {
    // Check cache first (valid for 60 seconds)
    {
        let cache = CUSTOM_PROMPT_CACHE.read().await;
        if let Some(ref cached) = *cache {
            if cached.loaded_at.elapsed().as_secs() < 60 {
                return (cached.content.clone(), cached.loaded_from.clone());
            }
        }
    }

    // Try environment variable first
    if let Ok(content) = std::env::var("CUSTOM_SYSTEM_PROMPT") {
        if !content.is_empty() {
            let source = "environment:CUSTOM_SYSTEM_PROMPT".to_string();
            cache_prompt(&content, &source).await;
            return (content, source);
        }
    }

    // Try file paths
    for path_str in CUSTOM_PROMPT_PATHS {
        let path = Path::new(path_str);
        if path.exists() {
            match tokio::fs::read_to_string(path).await {
                Ok(content) => {
                    info!("Loaded custom prompt from: {}", path_str);
                    let source = format!("file:{}", path_str);
                    cache_prompt(&content, &source).await;
                    return (content, source);
                }
                Err(e) => {
                    warn!("Failed to read custom prompt from {}: {}", path_str, e);
                }
            }
        }
    }

    // Use default
    debug!("Using default custom prompt");
    let source = "default".to_string();
    cache_prompt(DEFAULT_CUSTOM_PROMPT, &source).await;
    (DEFAULT_CUSTOM_PROMPT.to_string(), source)
}

async fn cache_prompt(content: &str, source: &str) {
    let mut cache = CUSTOM_PROMPT_CACHE.write().await;
    *cache = Some(CachedPrompt {
        content: content.to_string(),
        loaded_from: source.to_string(),
        loaded_at: std::time::Instant::now(),
    });
}

/// Save custom prompt to file
pub async fn save_custom_prompt(content: &str) -> anyhow::Result<String> {
    let path = Path::new(CUSTOM_PROMPT_PATHS[0]); // /etc/op-dbus/custom-prompt.txt

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(path, content).await?;
    info!("Saved custom prompt to: {:?}", path);

    // Invalidate cache
    {
        let mut cache = CUSTOM_PROMPT_CACHE.write().await;
        *cache = None;
    }

    Ok(path.to_string_lossy().to_string())
}

/// Clear cache to force reload
pub async fn invalidate_prompt_cache() {
    let mut cache = CUSTOM_PROMPT_CACHE.write().await;
    *cache = None;
    info!("Prompt cache invalidated");
}

// =============================================================================
// PROMPT GENERATION
// =============================================================================

/// Get the fixed (immutable) part of the system prompt
pub fn get_fixed_prompt() -> String {
    let mut fixed = String::new();

    fixed.push_str(FIXED_BASE_PROMPT);
    fixed.push_str("\n\n");
    fixed.push_str(FIXED_TOPOLOGY_SPEC);

    fixed
}

/// Generate complete system prompt (fixed + custom + dynamic)
pub async fn generate_system_prompt() -> ChatMessage {
    let mut prompt = String::new();

    // 1. Fixed part (immutable)
    prompt.push_str(&get_fixed_prompt());
    prompt.push_str("\n\n");

    // 2. Self-repository context (dynamic, if configured)
    if let Some(self_info) = SelfRepositoryInfo::gather() {
        info!("Adding self-repository context to system prompt");
        prompt.push_str(&self_info.to_system_prompt_context());
        prompt.push_str("\n\n");
    }

    // 3. Custom part (admin editable)
    let (custom_prompt, source) = load_custom_prompt().await;
    prompt.push_str("\n\n## 📝 CUSTOM INSTRUCTIONS\n");
    prompt.push_str(&format!("<!-- Loaded from: {} -->\n", source));
    prompt.push_str(&custom_prompt);
    prompt.push_str("\n\n");

    // 4. Tool summary (dynamic)
    prompt.push_str(&generate_tool_summary().await);

    ChatMessage::system(prompt)
}

/// Generate a summary of available tools
async fn generate_tool_summary() -> String {
    let mut summary = String::from("## AVAILABLE TOOLS\n\n");

    summary.push_str("### Core Categories:\n");
    summary.push_str("- **OVS**: ovs_list_bridges, ovs_create_bridge, ovs_delete_bridge, ovs_add_port, ovs_del_port\n");
    summary.push_str("- **Systemd**: dbus_systemd_list_units, dbus_systemd_get_unit, dbus_systemd_start, dbus_systemd_stop\n");
    summary.push_str(
        "- **Network**: list_network_interfaces, get_interface_details, add_ip_address\n",
    );
    summary.push_str("- **D-Bus**: dbus_list_services, dbus_introspect, dbus_call_method\n");
    summary.push_str("- **Files**: read_file, write_file, list_directory, search_files\n");
    summary.push_str("- **Shell**: shell_execute (use only when no native tool exists)\n");

    // Self tools if available
    if std::env::var("OP_SELF_REPO_PATH").is_ok() {
        summary.push_str("\n### Self-Repository Tools:\n");
        summary.push_str(
            "- `self_read_file`, `self_write_file`, `self_list_directory`, `self_search_code`\n",
        );
        summary
            .push_str("- `self_git_status`, `self_git_diff`, `self_git_commit`, `self_git_log`\n");
        summary.push_str("- `self_build`, `self_deploy`\n");
    }

    summary
}

/// Generate a minimal system prompt (for token-constrained models)
pub fn generate_minimal_prompt() -> ChatMessage {
    ChatMessage::system(
        "You are a Linux system admin assistant. Use tools for all actions. \
         Never suggest CLI commands - use native tools directly. \
         Report actual tool outputs only.",
    )
}

/// Create a session with the full system prompt
pub async fn create_session_with_system_prompt() -> Vec<ChatMessage> {
    vec![generate_system_prompt().await]
}

/// Get prompt metadata for admin UI
pub async fn get_prompt_metadata() -> PromptMetadata {
    let (custom_content, source) = load_custom_prompt().await;

    PromptMetadata {
        fixed_part: get_fixed_prompt(),
        custom_part: custom_content,
        custom_source: source,
        has_self_repo: std::env::var("OP_SELF_REPO_PATH").is_ok(),
        self_repo_path: std::env::var("OP_SELF_REPO_PATH").ok(),
    }
}

/// Metadata about the system prompt configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PromptMetadata {
    pub fixed_part: String,
    pub custom_part: String,
    pub custom_source: String,
    pub has_self_repo: bool,
    pub self_repo_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_prompt_generation() {
        let prompt = generate_system_prompt().await;
        assert!(prompt.content.contains("FORCED TOOL EXECUTION"));
        assert!(prompt.content.contains("ovs-br0"));
        assert!(prompt.content.contains("CUSTOM INSTRUCTIONS"));
    }

    #[test]
    fn test_fixed_prompt() {
        let fixed = get_fixed_prompt();
        assert!(fixed.contains("CRITICAL:"));
        assert!(fixed.contains("TOPOLOGY"));
    }

    #[tokio::test]
    async fn test_load_custom_prompt() {
        let (content, source) = load_custom_prompt().await;
        assert!(!content.is_empty());
        assert!(!source.is_empty());
    }
}
