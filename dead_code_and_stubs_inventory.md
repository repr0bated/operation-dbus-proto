# Dead Code and Stubs Inventory

## 1. Dead Code (Unused Code, Variables, Imports)
### crates/op-gateway/src/mcp_gateway.rs
- Line 11: unused imports: `error` and `warn` (`use tracing::{debug, error, info, warn};`)
- Line 11: unused imports: `error` and `warn` (`use tracing::{debug, error, info, warn};`)
- Line 14: unused imports: `ClientInfo` and `WireGuardSession` (`use crate::wireguard_auth::{ClientInfo, SessionFilter, WireGuardAuthManager, WireGuardSession};`)
- Line 14: unused imports: `ClientInfo` and `WireGuardSession` (`use crate::wireguard_auth::{ClientInfo, SessionFilter, WireGuardAuthManager, WireGuardSession};`)

### crates/op-gateway/src/wireguard_auth.rs
- Line 13: unused import: `simd_json::OwnedValue` (`use simd_json::OwnedValue;`)

### crates/op-llm/src/gcloud_adc.rs
- Line 9: unused import: `std::path::PathBuf` (`use std::path::PathBuf;`)

### crates/op-llm/src/gemini_cli.rs
- Line 23: unused imports: `Deserialize` and `Serialize` (`use serde::{Deserialize, Serialize};`)
- Line 23: unused imports: `Deserialize` and `Serialize` (`use serde::{Deserialize, Serialize};`)
- Line 26: unused import: `debug` (`use tracing::{debug, info, warn};`)
- Line 29: unused import: `ChatRequest` (`ChatMessage, ChatRequest, ChatResponse, LlmProvider, ModelInfo, ProviderType,`)

### crates/op-llm/src/mcp_proxy.rs
- Line 8: unused import: `std::time::Duration` (`use std::time::Duration;`)

### crates/op-network/src/ovsdb.rs
- Line 9: unused import: `AsyncReadExt` (`use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};`)

## 2. Stubs and Placeholders (TODO, FIXME, todo!, unimplemented!, stub)
### crates/op-agents/src/agent_registry.rs
- Line 338: `// TODO: Start health check task if configured`

### crates/op-agents/src/router.rs
- Line 144: `// TODO: Implement send_task`

### crates/op-cache/src/btrfs_cache.rs
- Line 103: `// Create directory structure (BTRFS subvolumes stubbed as regular dirs)`

### crates/op-cache/src/capability_resolver.rs
- Line 313: `// TODO: topological sort based on requires/provides`

### crates/op-cache/src/grpc/cache_service.rs
- Line 253: `compressed: false, // TODO: add compression`

### crates/op-cache/src/grpc/orchestrator_service.rs
- Line 730: `numa_nodes: 1, // TODO: get from actual NUMA topology`

### crates/op-cache/src/workflow_executor.rs
- Line 249: `retries: 0, // TODO: track retries`

### crates/op-chat/src/agent_tools.rs
- Line 297: `// For now, return a stub response`
- Line 304: `// TODO: Integrate with actual agent execution via D-Bus`
- Line 313: `"message": "Agent operation queued (stub - integrate with D-Bus agent executor)"`

### crates/op-chat/src/orchestrated_executor.rs
- Line 362: `agents_involved: vec![], // TODO: Track agents`

### crates/op-chat/src/orchestration/workstacks.rs
- Line 549: `// TODO: Implement rollback logic`

### crates/op-chat/src/router.rs
- Line 175: `// TODO: Implement SSE streaming`

### crates/op-chat/src/tool_loader.rs
- Line 2249: `// TODO: Add D-Bus discovered tools from introspection service`
- Line 2250: `// TODO: Add agent tools from op_agents`

### crates/op-grpc-bridge/src/grpc_server.rs
- Line 889: `stub_hash: snapshot.stub_hash.clone(),`

### crates/op-identity/src/wg.rs
- Line 27: `.arg("wg0") // TODO: Make interface configurable`

### crates/op-inspector/src/introspective_gadget.rs
- Line 33: `// Stub types for KnowledgeBase and SchemaDefinition until op-mcp is built`

### crates/op-mcp/src/builtin_trait_agents.rs
- Line 254: `// TODO: Add more built-in agents as needed`

### crates/op-mcp/src/tools/plugin.rs
- Line 46: `// TODO: Integrate with the authoritative plugin catalog / canonical`

### crates/op-ml/src/downloader.rs
- Line 141: `/// Stub implementation when ml feature is disabled`

### crates/op-ml/src/embedder.rs
- Line 213: `/// Stub implementation when ml feature is disabled`
- Line 252: `session: todo!(), // Mock for test`
- Line 253: `tokenizer: todo!(),`

### crates/op-ml/src/model_manager.rs
- Line 52: `/// Get or initialize global instance (stub for non-ml)`
- Line 80: `/// Embed text (stub for non-ml)`
- Line 104: `/// Embed batch (stub for non-ml)`

### crates/op-network/src/openflow.rs
- Line 243: `// TODO: Implement ovs-ofctl format parsing`
- Line 274: `// TODO: Implement proper flow statistics parsing`

### crates/op-network/src/ovs_netlink.rs
- Line 823: `// TODO: Implement datapath creation`
- Line 829: `// TODO: Implement datapath deletion`
- Line 904: `// TODO: Implement vport creation`
- Line 909: `// TODO: Implement vport deletion`

### crates/op-network/src/plugin.rs
- Line 402: `// TODO: Replace with native DHCP client library (e.g., dhcproto)`

### crates/op-network/src/rtnetlink.rs
- Line 436: `// Minimal, compile-safe stub; route filtering can be added later.`

### crates/op-plugins/src/state_plugins/mod.rs
- Line 79: `// pub use systemd_networkd::SystemdNetworkdPlugin; // TODO: Plugin not yet implemented`

### crates/op-plugins/src/state_plugins/net.rs
- Line 142: `// TODO: Implement validation logic`

### crates/op-plugins/src/state_plugins/netmaker.rs
- Line 127: `Ok(Vec::new()) // TODO: Implement actual peer discovery`

### crates/op-plugins/src/state_plugins/schema_contract.rs
- Line 53: `"stub",`

### crates/op-plugins/src/state_plugins/systemd.rs
- Line 103: `masked: None, // TODO: Query mask state`

### crates/op-state-store/src/event_chain.rs
- Line 390: `/// Stub hash`
- Line 391: `pub stub_hash: String,`
- Line 410: `// For now, stub/wrapper/tunable hashes are derived from effective`
- Line 412: `let stub_hash = effective_hash.clone();`
- Line 421: `stub_hash,`
- Line 433: `snapshot.stub_hash,`

### crates/op-state-store/src/plugin_schema.rs
- Line 360: `"stub",`
- Line 385: `"stub": {`
- Line 473: `"default": ["/stub/discovered_at"]`
- Line 5078: `assert!(required.iter().any(|value| value == "stub"));`
- Line 5081: `assert!(contract["properties"]["stub"].is_object());`

### crates/op-state/src/dbus_plugin_base.rs
- Line 5: `// Blockchain module not yet added - stub the type for now`

### crates/op-state/src/plugin_workflow.rs
- Line 330: `// TODO: Store the node for later workflow creation`

### crates/op-web/src/handlers/dashboard.rs
- Line 38: `mail_queue: 0,   // TODO`
- Line 39: `mcp_services: 0, // TODO`
- Line 42: `network: 0.0, // TODO`

### crates/op-web/src/handlers/mail.rs
- Line 50: `sent_today: 0, // TODO: Parse from maddy logs`
- Line 58: `// TODO: Query maddy's queue directory or database`

### crates/op-web/src/handlers/mcp.rs
- Line 164: `// TODO: Query actual memory store`
- Line 196: `// TODO: Delete from actual memory store`
- Line 206: `// TODO: Get actual stats`

### crates/op-web/src/handlers/privacy.rs
- Line 547: `registered_users: 0, // TODO: Add user count method`
- Line 645: `// TODO: Implement proper session management`

### crates/op-web/src/handlers/users.rs
- Line 43: `last_seen: None, // TODO: Track from WireGuard handshakes`

### crates/op-web/src/handlers/vpn.rs
- Line 106: `// TODO: Match WireGuard peers with users from database`

### crates/op-web/src/orchestrator/process.rs
- Line 411: `turns: 0, // TODO: track actual turns`

### crates/op-workflows/src/flow.rs
- Line 271: `// TODO: Implement proper cycle detection`

