# Relaunch Context: OP-DBUS Refactor

## Current Status
- **Mandate**: "SQL IS NOT USED. DIRECT RCP CALLS ONLY."
- **Implementation**:
    - **No SQL at Runtime**: Removed `PluginCatalog`, `SqlitePluginCatalog`, and `SqliteStore` from the `op-dbus` runtime path.
    - **Authoritative RCP Pool**: Components (especially gRPC `OpdbusPluginProvider`) now perform direct JSON-RPC calls to `authoritative_ovsdb` and `authoritative_nonnet` for plugin discovery and schema retrieval.
    - **Hierarchy Fix**: `DbusMirror` now projects `TableObject` at the `base_path` for every schema-derived spec, ensuring `busctl tree` shows the full hierarchy regardless of row counts.
    - **Memory Store**: Transient state now uses a new `MemoryStore` implemented in `op-state-store`.
- **Build**: `cargo build -p op-dbus` is passing.

## The "Missing Tree" Problem
`busctl tree` is likely not showing the new hierarchy because the system service (PID 1307) is still running the old binary. My attempts to restart it in the background as user `jeremy` failed to claim the D-Bus name and hit permission errors on `/var/lib/op-dbus`.

## Pending Tasks
1. **Deployment**: The new binary in `target/debug/op-dbus` needs to be moved to `/usr/local/sbin/op-dbus` and the service restarted (requires root).
2. **Deploy Script**: Run the `deploy.sh` script once located in the project tree.
3. **Verification**: Confirm the hierarchy with `busctl tree org.opdbus.v1`.

## Files Modified
- `root-package-src/main.rs`: Removed catalog/SQL dependencies, refactored gRPC provider.
- `crates/op-dbus-mirror/src/lib.rs`: Updated projection logic to always publish base paths.
- `crates/op-dbus-mirror/src/object.rs`: Added `TableObject`.
- `crates/op-state-store/src/state_store.rs`: Added `MemoryStore`.
- `crates/op-plugins/src/registry.rs`: Added helper methods for record lookups.
- `root-package-src/bin/inspector-onboard.rs`: Updated for new `InspectorGadget` signature.
