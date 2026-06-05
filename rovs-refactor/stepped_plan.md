# ROVS Architecture Refactor: Stepped Implementation & Deployment Plan

This plan captures the exact architectural boundary established for the D-Bus Hypervisor regarding the `rovs` crate suite (`rovs-jsonrpc`, `rovs-openflow`, etc.).

## Phase 1: The D-Bus Hypervisor Step
**Goal:** Establish `op-openvswitch-daemon` as the strict **Hypervisor** for the Open vSwitch protocol suite. It must act purely as a transport layer proxy without baking in high-level network logic (e.g., `add_bridge`, `add_port`).
1. **Remove Wrappers:** Strip all high-level D-Bus interfaces from the hypervisor daemon.
2. **Implement Primitives:** Implement native DBus objects representing the 1:1 translations of the `rovs` primitives based on their introspection schemas.
   - `/org/opdbus/rovs/jsonrpc` (Interface: `org.opdbus.rovs.jsonrpc`) -> `Transact`, `Notify`, `New`
   - `/org/opdbus/rovs/openflow` (Interface: `org.opdbus.rovs.openflow`) -> `Send_flow`, `Dump_flows`, `Connect`
3. **Execution:** The daemon forwards these raw DBus commands into the underlying `rovs` Unix socket handlers without inspection.

## Phase 2: Client Proxy Refactor
**Goal:** Upper-level state plugins must communicate with the daemon via these new strict proxies.
1. **Generate DBus Proxies:** Create `RovsJsonRpcProxy` and `RovsOpenFlowProxy` via `zbus`.
2. **Drop `OvsdbClient`:** Completely delete the ~1000-line legacy wrapper `op_network::ovsdb::OvsdbClient`.
3. **Refactor `OpenFlowPlugin`:** Update `discover_containers` to construct raw JSON-RPC `select` operations and send them via `RovsJsonRpcProxy::transact`.
4. **Refactor `OvsBridgePlugin` & `NetPlugin`:** Update to perform structural mutations (e.g., bridge creation) natively over the D-Bus JSON-RPC proxy.

## Phase 3: The `rovs_commands` Schema Plugin
**Goal:** Expose the D-Bus primitives to the consumer/orchestrator layer as a validated JSON schema.
1. **Create Plugin:** Create a new state plugin `crates/op-plugins/src/state_plugins/rovs_commands.rs`.
2. **Schema Projection:** In `plugin_schema_defs.rs`, implement `rovs_commands_plugin_schema()` to project the valid shapes of JSON-RPC `transact` requests and `send_flow` objects.
3. **Orchestrator Integration:** Upper-layer consumers (e.g., LLM tooling, UI) query this schema to validate low-level JSON-RPC commands before dispatching them to the state manager.

## Phase 4: Verification & Deployment
1. Build the workspace to verify the removal of `OvsdbClient` does not break any unmigrated tools.
2. Deploy the `op-openvswitch-daemon` systemd service.
3. Restart `op-dbus` to load the newly rewritten state plugins.
4. Verify end-to-end flow generation via the D-Bus proxy abstraction layer.

## Appendix: Current Implementation Status

### 1. The D-Bus Hypervisor
- **Implemented:** `op-openvswitch-daemon` has been successfully rewritten to expose `org.opdbus.rovs.jsonrpc` and `org.opdbus.rovs.openflow` natively.
- **Implemented:** The legacy `OpenvSwitchCommands` interface has been purged from the build scripts and the daemon.
- **Implemented:** `RovsJsonRpcProxy` and `RovsOpenFlowProxy` traits have been generated for consumer access.
- **Implemented:** `OpenFlowPlugin` has been refactored to use the raw JSON-RPC `transact` schema for OVSDB queries, and `send_flow` schema for OpenFlow updates.

### 2. Xray & Netmaker (Privacy Tunnels)
- **Implemented:** The `OpenFlowPlugin` natively tracks "Privacy Sockets" (`priv_wg`, `priv_xray`, `priv_warp`). 
- **Implemented:** Xray and Netmaker (Wireguard) are treated as predefined, immutable socket targets for traffic obfuscation, bypassing the need for legacy veth pairs or kernel routing.
- **Implemented:** Flows are dynamically generated to route matched container sockets (`sock_*`) securely into these privacy sockets based on D-Bus orchestrated policies.
