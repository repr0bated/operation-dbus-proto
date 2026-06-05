# OpenvSwitch Native D-Bus Hypervisor

The `op-openvswitch-daemon` is fully implemented and running entirely on native Rust DBus bindings, removing the legacy technical debt of shelling out to `ovs-vsctl` and `ovs-ofctl`.

## Completed Work

### 1. Unified DBus Interface
We implemented the `org.opdbus.OpenvSwitchCommands` interface backed by the `rovs-openflow` schema.
- **Consumer Simplicity:** Orchestrators like `op-plugins` can send a single cross-version `FlowEntry` JSON payload.
- **Native Implementation:** `crates/op-network/src/bin/op-openvswitch-daemon.rs` serves this interface natively on the System Bus.

### 2. Advanced Low-Level Protocol Interface
Using `bindgen`, we parse the upstream Open vSwitch C headers (`include/openflow/*.h`) directly into Rust structures at compile time!
- **Runtime Toggle:** The daemon accepts an `--enable-advanced-protocols` CLI flag which dynamically activates the `/org/opdbus/OpenvSwitch/Advanced` object path.
- **Complete Perspective:** Exposes native `ofp10_flow_mod`, `ofp13_flow_mod`, etc., for components that need exact memory layout control.

### 3. Build Script Clarity
Addressed the naming ambiguity:
- Renamed `build.rs` to [`build_ovs_schemas.rs`](file:///home/jeremy/git/operation-dbus-proto/crates/op-network/build_ovs_schemas.rs)
- Updated `Cargo.toml` to explicitly declare this custom build script.
- The script automatically re-runs if `/usr/share/openvswitch/vswitch.ovsschema` changes, generating the database schemas.

### 4. Caller Refactoring
We refactored `crates/op-plugins/src/state_plugins/openflow.rs` to drop `tokio::process::Command` calls.
- `install_flow`, `delete_flow`, and `query_flows` now securely negotiate with the System Bus and invoke `add_flow` / `delete_flow` using the native zbus proxy.

## Next Steps
The native OpenFlow hypervisor is fully integrated. If you deploy it to testing, the plugin layer will automatically route OpenFlow instructions through DBus instead of spawning sub-processes!
