# Mission Proposal — D-Bus OpenVSwitch Hypervisor (op-openvswitch-daemon)

> Generated: 2026-06-04. Repo: `/home/jeremy/git/operation-dbus-proto`.
> Status: **Ready for user acceptance.** All planning artifacts verified; no blockers.

---

## Mission statement

Build a native Rust D-Bus OpenVSwitch hypervisor daemon (`op-openvswitch-daemon`) that proxies the external `rovs` crate suite (rovs-jsonrpc, rovs-openflow, rovs-ovsdb, rovs-transport, rovs-types v0.2.0) over D-Bus + gRPC, delete the deprecated `OvsdbClient` wrapper (~991 lines), replace all `ovs-*` CLI subprocess calls with native proxy/gRPC calls, consolidate Xray + Netmaker management into the daemon, and enforce the AGENTS.md "D-Bus first, D-Bus only" rule workspace-wide.

---

## Scope (EVERYTHING — user-confirmed)

| Item | Status |
|---|---|
| `op-openvswitch-daemon` bin: 2 D-Bus object paths + passive OF controller | **IN** |
| gRPC transport: typed payloads + subscription streams as mutate triggers | **IN** |
| Delete `OvsdbClient` (991 lines) + migrate all ~50 consumers across 8 crates | **IN** |
| Plugin native-ization (openflow, net, ovsdb_bridge, full_system, privacy_router) | **IN** |
| `rovs_commands` schema plugin projected at `/org/opdbus/v1/plugins/rovs_commands` | **IN** |
| Re-enable + schema-ify `lxc.rs` + `netmaker.rs` (currently disabled) | **IN** |
| Workspace subprocess sweep (~150 sites incl. op-agents) | **IN** |
| Migrate 2 extra direct-socket `OvsdbClient` types onto daemon | **IN** |
| `--enable-advanced-protocols` bindgen path (raw OVS C structs) | **IN** |
| Xray/Netmaker/netclient consolidation + s6 deploy | **IN** |

---

## 8 Milestones (LOCKED)

### M0 — Pre-flight (implicit)
Fix existing fmt/clippy baseline so milestone gates can pass:
- Run `cargo fmt --all`
- Fix 24 op-web warnings that block `cargo clippy -- -D warnings`

### M1 — Hypervisor daemon (core)
- New `[[bin]] op-openvswitch-daemon` in `crates/op-network/Cargo.toml`
- Implement `org.opdbus.rovs.jsonrpc` on `/org/opdbus/rovs/jsonrpc` (10 methods: New, Next_id, Transact, Notify, Send_message, Recv_message, notification polling)
- Implement `org.opdbus.rovs.openflow` on `/org/opdbus/rovs/openflow` (15 methods: Connect, Version, Send_flow, Send_flow_sync, Echo, Barrier, Dump_flows, Dump_flows_filtered, Recv_packet_in, Try_recv_packet_in, Monitor_flows, Recv_flow_updates, Send_packet_out + passive listen on :6653)
- Absorb passive OpenFlow controller from `op-of-controller` using `controller.rs` TcpListener pattern
- System-bus registration; D-Bus introspection green

### M2 — gRPC transport + projection wiring
- Typed protobuf surface for rovs primitives (replaces `Transact(s,s)` string-passing on hot paths)
- gRPC server-streaming subscriptions for OVSDB monitor + OpenFlow flow updates
- Subscription-signal → mutate-trigger path into SchemaEngine
- Establish projection rule: **only `/org/opdbus/v1/plugins/` is projected** (no plugin = no verified schema = entity doesn't exist)

### M3 — Consumer proxies + OvsdbClient delete
- `RovsJsonRpcProxy` + `RovsOpenFlowProxy` (zbus proxies in op-network)
- Migrate all ~50+ `OvsdbClient` consumers (8 crates) to proxies/gRPC
- Port `op-dbus-mirror/event_sources/ovsdb.rs` monitor onto gRPC subscriptions (gRPC path ready from M2)
- Delete `crates/op-network/src/ovsdb.rs`
- `cargo build --workspace` green

### M4 — Plugin native-ization
- `openflow.rs`: drop `run_ovs_ofctl`/`ovs-ofctl` subprocess + `/usr/bin/ovs-ofctl` is_available gate → `RovsJsonRpcProxy`/`RovsOpenFlowProxy`
- `net.rs`, `ovsdb_bridge.rs`: swap `OvsdbClient` → proxy, construct raw OVSDB `select/insert/mutate` payloads
- `full_system.rs`: drop `ovs-vsctl` subprocess → `RovsJsonRpcProxy::transact`
- `privacy_router.rs`: swap OvsdbClient → proxy

### M5 — `rovs_commands` schema plugin
- `rovs_commands_plugin_schema()` in `crates/op-plugins/src/state_plugins/plugin_schema_defs.rs`
- `crates/op-plugins/src/state_plugins/rovs_commands.rs` (command plugin, NOT StatePlugin — no apply_state)
- Wire in `mod.rs` + `default_registry.rs`
- Projected at `/org/opdbus/v1/plugins/rovs_commands`

### M6 — Re-enable lxc.rs + netmaker.rs
- Add `lxc_plugin_schema()` + `netmaker_plugin_schema()` in plugin_schema_defs.rs
- Swap OvsdbClient → proxy; swap netclient/systemctl subprocess → daemon calls
- Un-comment in `mod.rs` + `default_registry.rs`

### M7 — Workspace subprocess sweep + extra clients + advanced-protocols
- Eliminate all forbidden runtime `Command::new` in `crates/` (ovs-*, systemctl, s6-svc, ip, wg, dhclient, xray, chmod, btrfs) per AGENTS.md §4
- ~150 sites in `op-agents/**` (cargo/npm/git/kubectl/…) — route through D-Bus or justify as tool-runner exception
- Migrate 2 extra direct-socket `OvsdbClient` types (`op-tools/builtin/ovsdb.rs`, `op-jsonrpc/src/ovsdb.rs`) onto daemon
- Add `build_ovs_schemas.rs` build script with `bindgen` on `/usr/include/openflow/*.h` + `/usr/share/openvswitch/vswitch.ovsschema`
- `--enable-advanced-protocols` CLI flag: exposes raw `ofp1x_flow_mod` C-struct D-Bus methods

### M8 — Xray/Netmaker consolidation + deploy
- Fold Xray lifecycle (start/stop/config) + Netmaker/netclient lifecycle behind daemon as D-Bus objects
- Remove `ovs-vsctl set-controller` holdout from `deploy/setup-hypervisor-controller.sh`
- Remove s6 `run` shell-outs → daemon-managed lifecycle
- Create s6 service for `op-openvswitch-daemon`
- End-to-end validation: D-Bus introspection, gRPC bridge, cognitive-MCP

---

## Validation posture

- **Per-milestone gate:** `cargo build --workspace` + `cargo clippy --workspace --all-targets --all-features -- -D warnings` + `cargo fmt --all -- --check` green + cognitive-MCP health check (`curl -sf http://100.90.37.254:3003/health | jq -e '.status == "ok"'`)
- Full D-Bus introspection / gRPC e2e NOT mandatory per milestone (useful at integration points)

---

## Key artifacts (planning documents produced)

| File | Purpose |
|---|---|
| `.mission-planning/refactor-surface.md` | Disk-verified blast-radius map (50+ OvsdbClient sites, subprocess inventory, Xray/Netmaker surface) |
| `.mission-planning/transcript-digest.md` | Antigravity session decisions digest (locked/rejected/overclaim analysis) |
| `.mission-planning/architecture-design.md` | Synthesized design with resolved decisions, scope, milestones, readiness |
| `.mission-planning/readiness-dependencies.md` | rovs crate API verification, gRPC infra audit, build graph |
| `.mission-planning/readiness-validation.md` | Toolchain, fmt/clippy baseline, cognitive-MCP health, crate inventory |
| `rovs-refactor/rovs-jsonrpc.json` | D-Bus method spec for jsonrpc path (10 methods) |
| `rovs-refactor/rovs-openflow.json` | D-Bus method spec for openflow path (15 methods) |

---

## Risks

1. **zbus local patch** at `/home/jeremy/git/zbus/` — workspace depends on it; if path breaks, builds fail.
2. **OvsdbClient deletion is 50+ call sites across 8 crates** — largest surface area; M3 is the riskiest milestone.
3. **op-agents ~150 `Command::new` sites** — policy decision on whether tool-runners are D-Bus-first exempt or must be routed; scope interpretation risk.
4. **TLS unimplemented in rovs-transport v0.2.0** — remote OVS over TCP/TLS deferred.
5. **No OpenFlow gRPC proto yet** — must be created in M2.

---

## Locked decisions (do not revisit)

- Daemon name: `op-openvswitch-daemon`
- Two separate object paths (NOT single multiplexed)
- Per-path interfaces: `org.opdbus.rovs.jsonrpc` + `org.opdbus.rovs.openflow`
- Projection tree = plugins only (`/org/opdbus/v1/plugins/`)
- D-Bus = authority/registration; gRPC = typed transport + subscriptions
- Daemon is pure pass-through (no business logic / high-level wrappers)
- Daemon is NOT a StatePlugin (rovs_commands is a command plugin)
- OvsdbClient is deprecated and deleted
- Absorb passive OF controller into daemon
- JSON introspection (not XML)
