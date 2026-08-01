# ROVS Suite: Phase 2 Execution

- `[ ]` **Implement DBus Server Primitives**
  - `[ ]` Clean up `build_ovs_schemas.rs` (remove legacy `OpenvSwitchCommands` interface).
  - `[ ]` Update `op-openvswitch-daemon.rs` with `org.op_dbus.CrateInterface` implementations for `rovs-jsonrpc` and `rovs-openflow`.

- `[ ]` **Generate DBus Proxies**
  - `[ ]` Create `crates/op-network/src/rovs_jsonrpc_proxy.rs` mapping to the `Transact`/`Notify` DBus API.
  - `[ ]` Create `crates/op-network/src/rovs_openflow_proxy.rs` mapping to the `Send_flow`/`Dump_flows` DBus API.
  - `[ ]` Delete old `openvswitch_proxy.rs`.

- `[ ]` **Refactor Client Plugins**
  - `[ ]` Refactor `openflow.rs` to use `rovs_openflow_proxy` instead of `OvsdbClient` or legacy DBus proxy.
  - `[ ]` Refactor `net.rs` to use `rovs_jsonrpc_proxy` via JSON-RPC payloads.
  - `[ ]` Refactor `lxc.rs` to use `rovs_jsonrpc_proxy`.
  - `[ ]` Refactor `ovsdb_bridge.rs` to use `rovs_jsonrpc_proxy`.
  - `[ ]` Refactor `full_system.rs` to use `rovs_jsonrpc_proxy` instead of shelling out.

- `[ ]` **Verification**
  - `[ ]` Run `cargo check --workspace`.
