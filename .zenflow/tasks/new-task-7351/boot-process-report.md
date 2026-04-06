# Refactored Boot Process Review

## Scope

This review ignores legacy deploy-time service/config files and treats the refactored Rust code as the source of truth.

Focused requirements:

- `wg-quick` for `wgcf` must run very early so the interface exists before OVS attach.
- `ens3` must remain a standalone host uplink managed by `systemd-networkd`, not enslaved to `ovsbr0`.
- `grpc-bridge` must be present on `ovsbr0`.
- OVS objects already persisted in OVSDB should be brought up through native control paths, not `ovs-vsctl`.
- DNS and service workloads live in Incus containers; OpenClaw is part of the higher service layer, not the base host fabric bootstrap.

## Refactored Source Of Truth

- `src/main.rs`
  - Loads plugins into `StateManager`.
  - Now boots `privacy_router` automatically by default unless `OP_DBUS_ENABLE_PRIVACY_ROUTER_BOOTSTRAP=false`.
- `crates/op-plugins/src/state_plugins/privacy_router.rs`
  - Owns host privacy fabric bootstrap.
  - Starts `wg-quick` for `wgcf` if the interface is missing.
  - Attaches `wgcf` to `ovsbr0` through native OVSDB JSON-RPC.
  - Brings up `ovsbr0`, `ovsbr0-mgmt`, `ovsbr0-sock`, and `grpc-bridge`.
  - Publishes system Incus containers for privacy ingress/egress.
  - Publishes OpenFlow chain state after the base fabric is ready.
- `crates/op-web/src/privacy_network.rs`
  - Reuses the same model during user-facing provisioning.
- `crates/op-grpc-bridge/src/grpc_server.rs`
  - Runtime inspection only; not the fabric bootstrap authority.

## Required Boot Order

1. `op-session-bus`
2. `op-dbus`
3. `privacy_router` bootstrap inside `op-dbus`
4. `wg-quick up wgcf`
5. Verify `wgcf` exists, then attach it to `ovsbr0` through OVSDB JSON-RPC
6. Bring up existing OVS interfaces on the bridge:
   - `ovsbr0`
   - `ovsbr0-mgmt`
   - `ovsbr0-sock`
   - `grpc-bridge`
7. Apply host L3 with `systemd-networkd`
   - `ens3` stays standalone and up
   - management/control-plane addressing stays on the OVS internal side
8. Reconcile Incus system containers and privacy OpenFlow policy
9. Start higher-layer services that depend on the privacy fabric

## Code Changes Made

### `crates/op-plugins/src/state_plugins/privacy_router.rs`

- Changed the default uplink from the invalid placeholder `"o"` to `ens3`.
- Added `PRIVACY_ATTACH_UPLINK_TO_BRIDGE` with default `false`.
  - Default behavior now keeps `ens3` standalone.
  - Bridge attachment is opt-in only.
- Added `PRIVACY_GRPC_BRIDGE_PORT` with default `grpc-bridge`.
- Ensures `grpc-bridge` exists on `ovsbr0` and is brought up.
- Brings the standalone uplink link up explicitly.
- Keeps `wg-quick` validation strict:
  - `[Interface]` required
  - `PrivateKey` required
  - `Table = off` required before bridging

### `src/main.rs`

- Replaced the disabled `privacy_router` bootstrap stub with a real bootstrap loop.
- Bootstrap now:
  - reads current privacy-router config from the plugin when available
  - falls back to `PrivacyRouterConfig::default()`
  - retries until `apply_plugin_state("privacy_router", ...)` succeeds
- Bootstrap is now enabled by default.
  - Disable only with `OP_DBUS_ENABLE_PRIVACY_ROUTER_BOOTSTRAP=false`

## Remaining Architectural Notes

- The current `privacy_router` plugin still creates missing internal OVS ports if absent.
  - That is acceptable as a recovery path, but the intended steady state is preseeded OVSDB objects that are only brought up.
- `crates/op-web/src/privacy_container.rs` still supports bridged Incus NIC publication when explicitly enabled.
  - Default flow remains state-driven and does not require that bridged path.
- DNS/OpenClaw container ordering is not part of the base host bootstrap in current Rust code.
  - They should be treated as service-layer dependencies after the privacy fabric is healthy.
