# ROVS Suite: Phase 2 Primitive Translation Plan

Based on the latest directive, we are discarding the "high-level wrapper" approach (like `add_bridge`). Instead, the `op-openvswitch-daemon` will act as a literal D-Bus proxy for the raw `rovs` primitives. 

This means the D-Bus interface will exactly mirror the Rust APIs of the `rovs-jsonrpc` and `rovs-openflow` crates.

## Core Architectural Shift
- **No Wrappers:** The daemon will not implement `add_bridge` or `delete_bridge`. It will implement `Transact(method, params)` and `Notify(method, params)` directly from the `rovs-jsonrpc` crate.
- **Plugins construct payloads:** `net.rs`, `lxc.rs`, etc., will construct their own OVSDB JSON-RPC transaction payloads and send them via the DBus `Transact` method. This maintains the daemon's role as the transport/proxy layer without baking business logic into it.

## Proposed Interfaces

### 1. `org.opdbus.rovs.jsonrpc`
We will implement the exact interface from `rovs-jsonrpc.json`:
- `New(stream) -> result`
- `Transact(method, params) -> result`
- `Notify(method, params) -> result`
- `Send_message(msg) -> result`
- `Recv_message() -> result`
- Notification polling methods (`Has_pending_notifications`, etc.)

### 2. `org.opdbus.rovs.openflow`
We will implement the exact interface from `rovs-openflow.json`:
- `Connect(addr) -> result`
- `Send_flow(flow) -> result`
- `Dump_flows() -> result`
- `Echo()`, `Barrier()`, `Recv_packet_in()`, etc.

## Proposed Execution Steps
1. **Remove Old Interfaces:** Delete the stubbed `org.opdbus.OpenvSwitchCommands` interface from `build_ovs_schemas.rs` and `op-openvswitch-daemon.rs`.
2. **Implement `rovs` DBus Objects:** Create native `zbus::interface` blocks inside `op-openvswitch-daemon.rs` that strictly adhere to the 1:1 translations for `jsonrpc` and `openflow`.
3. **Refactor Plugins:** Update all state plugins (`net.rs`, `openflow.rs`, etc.) to call these low-level D-Bus interfaces instead of instantiating local instances of `OvsdbClient` or `rovs_openflow::VConn`.

## User Review Required
> [!IMPORTANT]
> Since we are mapping 1:1, do you want these implemented on separate D-Bus Object Paths (e.g. `/org/opdbus/rovs/jsonrpc` and `/org/opdbus/rovs/openflow`), or should they all live on a single object like `/org/opdbus/rovs` under different interfaces?
