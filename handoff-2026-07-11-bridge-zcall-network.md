# Handoff: zcall, Ghostbridge, identity_sled, bridge collapse, network script

Date: 2026-07-11
Repo: `/home/jeremy/git/operation-dbus-proto`

## Current live state

- `zcall` live endpoint is `10.200.0.1:50051`, owned by `/usr/local/bin/op-dbus`.
- `op-grpc-bridge-zeroclaw` live endpoint is `0.0.0.0:8090`, pid was still the old `/usr/local/bin/op-grpc-bridge-zeroclaw` process when this handoff was written.
- `opdbus` release build completed after the latest `identity_sled.get_identity {}` host-read patch, but it has NOT been installed/restarted yet.
- Unified `op-grpc-bridge` source compiles in dev checks, but the release `op-grpc-bridge` binary has NOT been built/installed/restarted yet after the Cargo alias change.

## Verified before latest identity_sled patch

After adding Ghostbridge dispatch and bounding projection D-Bus crawling:

- `zcall ghostbridge get_identity --capability cap.service.ghostbridge.identity.read@v1 --actor audit --arguments '{}'` returned successfully.
- `zcall ghostbridge get_state --capability cap.service.ghostbridge.state.read@v1 --actor audit --arguments '{}'` returned successfully.
- `zcall ghostbridge list_endpoints --capability cap.service.ghostbridge.endpoints.read@v1 --actor audit --arguments '{}'` returned successfully.
- `zcall ghostbridge get_ghostrunner --capability cap.service.ghostbridge.ghostrunner.read@v1 --actor audit --arguments '{}'` returned successfully.
- `zeroclaw.get_state` with correct capability returned instead of hanging.

Remaining at that point:

- `identity_sled.get_identity` with `identity_sled.read` still timed out.

## Latest source changes not yet live

Identity sled:

- `crates/op-grpc-bridge/src/identity_sled_dispatch.rs`
- `identity_sled.get_identity {}` now reads host/container-zero identity directly from `/dev/shm/plugin_schema.dat` via `op_identity::schema_bridge::read_sled()`.
- This avoids Cozo hydration for host identity.
- Input requirements:
  - `{}` means host/container-zero, should return live raw sled identity.
  - `{"session_id":"..."}` means persisted container identity, still uses identity-sled cache/Cozo hydration.

Bridge collapse:

- `crates/op-grpc-bridge/src/bin/op-grpc-bridge.rs` now supports modes:
  - default/`bridge`: old `op-grpc-bridge` behavior.
  - `OP_GRPC_BRIDGE_MODE=zeroclaw` or argv0 containing `zeroclaw`: old `op-grpc-bridge-zeroclaw` behavior.
- `crates/op-grpc-bridge/Cargo.toml` now aliases both bin names to one source file:
  - `op-grpc-bridge`
  - `op-grpc-bridge-zeroclaw`
- `crates/op-grpc-bridge/src/bin/op-grpc-bridge-zeroclaw.rs` was deleted.
- Cargo emits the expected warning that the same file is present in multiple bin targets.

Deploy/service source:

- `deploy/s6/op-grpc-bridge-zeroclaw/run` now sets `OP_GRPC_BRIDGE_MODE=zeroclaw` and execs `/usr/local/bin/op-grpc-bridge`.
- `deploy/deploy.sh` now builds `op-grpc-bridge` for service `op-grpc-bridge-zeroclaw`.

## Checks already run

- `rustfmt --edition 2021 --check crates/op-grpc-bridge/src/bin/op-grpc-bridge.rs crates/op-grpc-bridge/src/identity_sled_dispatch.rs crates/op-grpc-bridge/src/mutation_engine.rs crates/op-plugins/src/state_plugins/ghostbridge.rs` passed.
- `cargo check -p op-grpc-bridge --bins` passed.
- `cargo test -p op-plugins ghostbridge --lib` passed.
- `cargo build --release -p op-web --bin opdbus` completed successfully after the latest identity-sled patch.

## Commands to finish bridge/identity live deploy

Install/restart `op-dbus` so `zcall :50051` gets the latest identity-sled host-read patch:

```sh
sudo -n install -m 0755 target/release/opdbus /usr/local/bin/op-dbus
sudo -n s6-svc -r /run/service/op-dbus
sleep 6
sudo -n s6-svstat /run/service/op-dbus
zcall --timeout 8 identity_sled get_identity --arguments '{}' --capability identity_sled.read --actor audit
```

Build/install/restart unified Zeroclaw bridge:

```sh
cargo build --release -p op-grpc-bridge --bin op-grpc-bridge
sudo -n install -m 0755 target/release/op-grpc-bridge /usr/local/bin/op-grpc-bridge
sudo -n install -m 0755 deploy/s6/op-grpc-bridge-zeroclaw/run /etc/s6/sv/op-grpc-bridge-zeroclaw/run
sudo -n install -m 0755 deploy/s6/op-grpc-bridge-zeroclaw/run /run/service/op-grpc-bridge-zeroclaw/run.user
sudo -n s6-svc -r /run/service/op-grpc-bridge-zeroclaw
sleep 3
sudo -n s6-svstat /run/service/op-grpc-bridge-zeroclaw
sudo -n ss -ltnp | awk '$4 ~ /:50051$|:8090$/ {print}'
```

Expected:

- `10.200.0.1:50051` still owned by `op-dbus`.
- `0.0.0.0:8090` owned by `op-grpc-bridge`.
- `/run/ghostbridge/container.sock` still served by Zeroclaw mode.

## zcall wrapper state

Changed files:

- `bin/zcall`
- `completions/zcall.bash`

Verified earlier:

- `/usr/local/bin/zcall` and `/home/jeremy/.local/bin/zcall` are symlinks to repo `bin/zcall`.
- `zcall check-catalog` passed with `plugins=68 callable_plugins=58 methods=503 failures=0`.
- Missing plugin/method/required args fail nonzero.
- `--print` redacts Ghostbridge headers unless `--show-headers`.
- actual calls use `timeout`, parse `CallMethodResponse`, and return nonzero on bridge errors.

## Network script still needs finishing

Files:

- Source: `control-plane-network/bin/control-plane-network`
- Installed live: `/usr/local/sbin/control-plane-network`
- s6 oneshot: `/etc/s6/sv/control-plane-network/up`
- Config: `/etc/control-plane-network/network.conf`

Problem:

- User forbids raw OVS commands.
- Script still uses raw:
  - `ovs-vsctl add-br`
  - `ovs-vsctl del-port`
  - `ovs-vsctl add-port`
  - `ovs-vsctl set bridge`
  - `ovs-ofctl add-flow`
  - report collection with `ovs-vsctl list-ports` and `ovs-ofctl dump-flows`
- It also still uses raw `ip`, `wg`, and `wg-quick strip`. Earlier conclusion: `wg-quick up` is not needed; `wg-quick strip` is only being used as a parser for `/etc/wireguard/netmaker.conf`.

Available zcall method surface:

- `rovs_commands`
  - `create_bridge` cap `cap.network.ovsdb.bridge.create@v1`
  - `add_port` cap `cap.network.ovsdb.port.add@v1`
  - `remove_port` cap `cap.network.ovsdb.port.delete@v1`
  - `list_ports` cap `cap.network.ovsdb.port.list@v1`
  - `list_bridges` cap `cap.network.ovsdb.bridge.list@v1`
- `rtnetlink`
  - `add_link` cap `cap.network.rtnetlink.link.add@v1`
  - `set_link_state` cap `cap.network.rtnetlink.link-state.set@v1`
  - `add_ipv4_address` cap `cap.network.rtnetlink.ipv4-address.add@v1`
  - `add_route` cap `cap.network.rtnetlink.route.add@v1`
  - `set_mac_address` cap `cap.network.rtnetlink.mac-address.set@v1`
- `wireguard`
  - `set_device` cap `wireguard.write`
  - `set_config` cap `wireguard.write`
  - `get_device` cap `wireguard.read`
  - `list_peers` cap `wireguard.read`
- `openflow`
  - `AddFlow` cap `openflow.write`
  - `DeleteFlow` cap `openflow.write`
  - `ModifyFlow` cap `openflow.write`

Suggested network-script finishing order:

1. Add helper in `control-plane-network/bin/control-plane-network`:
   - `zcall_json plugin method cap actor json`
   - always pass `--timeout 10`
   - actor should be `control-plane-network`
   - fail hard on zcall nonzero.
2. Replace OVS bridge setup:
   - `ovs-vsctl add-br "$BRIDGE"` -> `zcall rovs_commands create_bridge`.
   - `ovs-vsctl del-port "$BRIDGE" "$port"` -> `zcall rovs_commands remove_port`.
   - `ovs-vsctl add-port "$BRIDGE" "$UPLINK_PORT"` -> `zcall rovs_commands add_port`.
   - `ovs-vsctl add-port "$BRIDGE" "$NETMAKER_IFACE"` -> `zcall rovs_commands add_port`.
3. Replace link/address/route setup:
   - `ip link add "$NETMAKER_IFACE" type wireguard` -> `zcall rtnetlink add_link`.
   - `ip link set ... up` -> `zcall rtnetlink set_link_state`.
   - `ip addr replace ...` -> `zcall rtnetlink add_ipv4_address` if method supports replace/idempotent semantics; otherwise inspect/patch plugin.
   - `ip route replace ...` -> `zcall rtnetlink add_route` if idempotent; otherwise inspect/patch plugin.
4. Replace WireGuard config:
   - Keep `wg-quick strip` only temporarily if no parser exists.
   - Prefer `zcall wireguard set_config` or `set_device` with parsed config JSON.
5. Replace OpenFlow:
   - `ovs-ofctl add-flow` -> `zcall openflow AddFlow`.
6. Replace report commands:
   - `ovs-vsctl list-ports` -> `zcall rovs_commands list_ports`.
   - `ovs-ofctl dump-flows` needs a read/list method; if absent, add one or omit flow dump.
7. Test manually before s6/reboot:
   - `sudo -n /usr/local/sbin/control-plane-network`
   - `ip -brief addr`
   - `wg show netmaker`
   - `zcall --timeout 8 rovs_commands list_ports --capability cap.network.ovsdb.port.list@v1 --actor audit --arguments '{"bridge_name":"ovsbr0"}'`
   - DNS: `dig @10.200.0.1 google.com`
8. Only after manual success:
   - install source to `/usr/local/sbin/control-plane-network`
   - if service graph changed, recompile s6-rc
   - reboot only after zcall-based script succeeds manually.

Important caution:

- Earlier authorized `rovs_commands` calls timed out before the projection crawl timeout patch. Re-test now before rewriting the network script. If `rovs_commands list_ports` still times out, fix plugin dispatch before relying on it for boot.

## Dirty tree notes

Relevant changed files from this work:

- `bin/zcall`
- `completions/zcall.bash`
- `crates/op-grpc-bridge/Cargo.toml`
- `crates/op-grpc-bridge/src/bin/op-grpc-bridge.rs`
- `crates/op-grpc-bridge/src/bin/op-grpc-bridge-zeroclaw.rs` deleted
- `crates/op-grpc-bridge/src/identity_sled_dispatch.rs`
- `crates/op-grpc-bridge/src/mutation_engine.rs`
- `crates/op-plugins/src/state_plugins/ghostbridge.rs`
- `deploy/deploy.sh`
- `deploy/s6/op-grpc-bridge-zeroclaw/run`
- `control-plane-network/` untracked

Unrelated dirty files existed and were not touched/reverted.
