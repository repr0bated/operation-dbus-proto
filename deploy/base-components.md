# OP-DBUS Base Components

This document lists all components included in the base installation.

## Core Services (Always Running)

| Service | Port | Description |
|---------|------|-------------|
| op-chat-server | 8080 | Chat interface + LLM orchestration |
| op-mcp-server | stdio | MCP protocol aggregator |
| nginx | 80/443 | Reverse proxy + SSL termination |

## Included Crates

### op-chat
- ChatActor (orchestrator)
- ForcedToolChatLoop (anti-hallucination)
- SessionManager
- TrackedToolExecutor

### op-tools
- ToolRegistry
- Built-in tools (28)
- Tool middleware (logging, security)

### op-introspection
- ServiceScanner
- IntrospectionCache (SQLite)
- XML Parser

### op-agents (Unified Architecture)
- 8 Execution Agents (python, rust, go, js, c, cpp, sql, shell)
- 14 Persona Agents (django, fastapi, react, security, etc.)
- 2 Orchestration Agents (tdd, code-review)

### op-mcp
- MCP JSON-RPC protocol
- External server aggregation
- Tool forwarding

### op-llm
- HuggingFace provider
- Gemini provider
- Anthropic provider

## Pre-installed MCP Servers

| Server | Purpose |
|--------|--------|
| filesystem | File system access |
| memory | Persistent memory |
| sequential-thinking | Enhanced reasoning |
| github | GitHub operations (needs API key) |
| brave-search | Web search (needs API key) |
| fetch | HTTP requests |
| puppeteer | Browser automation |

## D-Bus Integration

### Bus Names
- org.opdbus.AgentManager
- org.opdbus.Orchestrator
- org.opdbus.Introspection

### Interfaces
- org.opdbus.AgentManager
  - SpawnAgent(type, config) → agent_id
  - KillAgent(agent_id) → bool
  - ListAgents() → [agent_id]
  - GetAgentStatus(agent_id) → status_json

- org.opdbus.Orchestrator
  - SendTask(agent_id, task_json) → task_id
  - GetPendingTasks(agent_id) → tasks_json
  - CompleteTask(task_id, result_json) → bool

## State Plugins → Tools (42)

Each plugin generates 3 tools: `_query`, `_diff`, `_apply`

| Plugin | Tools Generated |
|--------|----------------|
| systemd | plugin_systemd_{query,diff,apply} |
| net | plugin_net_{query,diff,apply} |
| packagekit | plugin_packagekit_{query,diff,apply} |
| login1 | plugin_login1_{query,diff,apply} |
| keyring | plugin_keyring_{query,diff,apply} |
| lxc | plugin_lxc_{query,diff,apply} |
| openflow | plugin_openflow_{query,diff,apply} |
| systemd_networkd | plugin_systemd_networkd_{query,diff,apply} |
| dnsresolver | plugin_dnsresolver_{query,diff,apply} |
| netmaker | plugin_netmaker_{query,diff,apply} |
| pcidecl | plugin_pcidecl_{query,diff,apply} |
| privacy_router | plugin_privacy_router_{query,diff,apply} |
| privacy | plugin_privacy_{query,diff,apply} |
| sessdecl | plugin_sessdecl_{query,diff,apply} |

## Built-in MCP Tools (28)

### D-Bus Introspection (12)
- dbus_list_services
- dbus_introspect_service
- dbus_list_objects
- dbus_introspect_object
- dbus_list_interfaces
- dbus_list_methods
- dbus_list_properties
- dbus_list_signals
- dbus_call_method
- dbus_get_property
- dbus_set_property
- dbus_get_all_properties

### System (9)
- systemd_status
- file_read
- file_write
- network_interfaces
- process_list
- exec_command
- discover_system
- detect_ssl_certificates
- get_cache_stats

### Agent Management (7)
- spawn_agent
- list_agents
- agent_executor_execute
- agent_file_operation
- agent_monitor_metrics
- agent_network_operation
- agent_systemd_manage

## Directory Structure

```
/opt/op-dbus/
├── bin/
│   ├── op-web-server
│   ├── op-mcp-server
│   └── op-agent-manager
├── lib/
└── share/

/etc/op-dbus/
├── environment
├── secrets.env
├── agents/
├── plugins/
└── mcp/
    └── servers.json

/var/lib/op-dbus/
├── cache/
│   └── introspection.db
├── sessions/
│   └── sessions.db
└── snapshots/

/var/log/op-dbus/
└── (service logs via journald)

/var/www/op-dbus/
└── static/
    ├── chat.html
    ├── css/
    └── js/
```

## What the Chatbot Manages (Post-Install)

After base installation, the chatbot handles:

1. **Dynamic Agents** (148 from ~/agents/)
   - Load markdown agent definitions
   - Register with agent registry
   - Enable/disable as needed

2. **Dynamic Commands** (42 from ~/commands/)
   - Load command templates
   - Make available as slash commands

3. **Additional MCP Servers**
   - Install via npm
   - Configure in servers.json
   - Enable/disable

4. **Custom Workflows**
   - TDD workflow
   - Code review workflow
   - Custom user workflows

5. **VM/Container Templates**
   - Proxmox template creation
   - Snapshot management
   - Deployment automation

6. **System Configuration**
   - API key management
   - Service tuning
   - Log rotation
   - Backup configuration
