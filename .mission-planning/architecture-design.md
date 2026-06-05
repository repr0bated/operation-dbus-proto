# Mission Architecture Design — D-Bus OpenVSwitch Hypervisor (`op-openvswitch-daemon`)

> Synthesis date: 2026-06-04. Last corrected: 2026-06-04 (SHM single-source-of-truth).
> Sources: `.mission-planning/refactor-surface.md` (disk-verified blast radius),
> `.mission-planning/transcript-digest.md` (Antigravity planning session decisions),
> `rovs-refactor/*` specs, root `AGENTS.md`.
>
> **Reality anchor:** This is **greenfield**. Every "Implemented" claim in `rovs-refactor/*.md`
> is confabulated (verified: no daemon, no proxies, no build script, no schema plugin exist on
> disk). Trust this doc + the two investigation artifacts, not the `rovs-refactor/*.md` status text.
>
> **SHM correction (2026-06-04):** The daemon is a transport proxy ONLY. It does NOT maintain
> an OVSDB IDL replica. The single source of truth is `PluginSchema → SchemaEngine → /dev/shm`.
> Any current code that owns an `OvsdbClient` + `monitor_db` subscription (e.g. `op-grpc-bridge::SchemaEngine`)
> is maintaining a second, competing source of truth and must be reworked to write into
> `/dev/shm` instead. See §3.5 below.

---

## 1. Target end-state (locked design)

```
                         ┌─────────────────────────────────────────────┐
   external rovs crates  │            op-openvswitch-daemon            │
   (crates.io v0.2.0)    │        (NEW bin in crates/op-network)        │
   rovs-ovsdb            │  PURE PASS-THROUGH PROXY — no business logic │
   rovs-openflow ───────▶│                                              │
   rovs-jsonrpc          │  /org/opdbus/rovs/jsonrpc  (OVSDB primitives)│
   rovs-transport        │      Transact · Notify · Send/Recv_message · │
   rovs-types            │      notification polling                    │
                         │  /org/opdbus/rovs/openflow (OpenFlow prims)  │
                         │      Connect · Send_flow · Dump_flows ·      │
                         │      Recv_packet_in · Monitor_flows · …      │
                         └───────────────▲─────────────────────────────┘
                                         │ zbus (system bus)
        RovsJsonRpcProxy / RovsOpenFlowProxy  (NEW in crates/op-network)
                                         │
   ┌─────────────────────────────────────────────────────────────────┐
   │  consuming plugins/services (craft raw payloads, keep biz logic)  │
   │  op-plugins: openflow.rs, net.rs, ovsdb_bridge.rs, full_system.rs │
   │  op-tools, op-web/op-dbus, op-grpc-bridge, op-dbus-mirror, op-chat │
   └───────────────────────────────────────────────────────────────────┘
                                         │ projects validated schema
                  rovs_commands plugin → /org/opdbus/v1/plugins/rovs_commands
                  (schema source of truth in plugin_schema_defs.rs)
```

**Daemon = literal D-Bus proxy over the rovs crate API.** No `add_bridge`/`add_port` wrappers.
Plugins construct raw OVSDB JSON-RPC (`{"op":"select"|"insert"|"mutate", ...}`) and call
`Transact`; flow ops call `Send_flow`. The daemon never inspects payloads. **The daemon
does NOT maintain an OVSDB IDL replica or any database mirror.** It holds a `rovs-jsonrpc::Connection`
for OVSDB and a `VConn` for OpenFlow — both are pure I/O transports. State lives exclusively
in `PluginSchema → SchemaEngine → /dev/shm/live-schema.json`. Consumers read from `/dev/shm`
(1:1 direct read, zero-copy), not from OVSDB monitors or IDL replicas.

### Locked decisions (from transcript + task statement)
- Daemon name: **`op-openvswitch-daemon`**.
- **Two separate object paths** `/org/opdbus/rovs/jsonrpc` and `/org/opdbus/rovs/openflow` (single multiplexed object **rejected**).
- **`OvsdbClient` (op-network/src/ovsdb.rs, 991 lines) deleted**; daemon uses rovs crates natively over `/run/openvswitch/db.sock` (RFC 7047) with optional remote TCP/TLS via `rovs-transport`.
- Daemon is **NOT a StatePlugin** (no `apply_state`/`PluginSchemaBuilder`). Schema is exposed via a separate **`rovs_commands` command plugin**.
- Introspection output is **JSON, not XML**.
- Optional `--enable-advanced-protocols` flag exposes raw bindgen C structs (`ofp1x_flow_mod`) alongside unified `Flow`. **(Proposed lower-priority / stretch — confirm.)**

### Rejected designs — do NOT resurrect
Single `/org/opdbus/rovs` multiplexed object · daemon-as-StatePlugin · high-level `add_bridge` wrappers · reusing `OvsdbClient` inside the daemon · standalone `op-vsctl`/`op-ofctl` clap CLIs · introspecting banned `ovs-*` CLIs · regex source parsing · XML introspection.

---

## 2. Deletion blast radius (verified)

`op_network::ovsdb::OvsdbClient` is consumed across **8 crates, ~50+ call sites**:
- op-network (plugin.rs, ovs_capabilities.rs), op-plugins (openflow.rs, net.rs, ovsdb_bridge.rs, privacy_router.rs, lxc.rs[disabled]), op-tools (**ovs_tools.rs ~21 sites**, ovs.rs, openflow_tools.rs), op-web (privacy_network.rs, **bin/op-dbus.rs** = main daemon wiring), op-grpc-bridge (schema_engine.rs, bin), op-dbus-mirror (**5 files incl. streaming `monitor_db` consumer**), op-chat (tool_loader.rs, 7 sites).

Two **unrelated** `OvsdbClient` types (`op-tools/builtin/ovsdb.rs`, `op-jsonrpc/src/ovsdb.rs`) are NOT delete targets but DO bypass the daemon (talk to OVSDB socket directly).

---

## 3. Resolved decisions (user-confirmed 2026-06-04)

1. **Interface naming — RESOLVED.** Per-path interfaces `org.opdbus.rovs.jsonrpc` + `org.opdbus.rovs.openflow`. Scratch `org.op_dbus.CrateInterface` is a generator placeholder only.
2. **Projection tree is plugins-only — RESOLVED (architectural rule).** Only `/org/opdbus/v1/plugins/<name>` is projected/rendered/verified. `plugin_schema_defs.rs` enumerates the plugins that ARE the projection tree: **if there is no plugin, there is no verified schema, therefore the entity does not exist on the system.** The raw `/org/opdbus/rovs/{jsonrpc,openflow}` paths are the daemon's transport/execution objects — they are NOT part of the projection. The `rovs_commands` plugin is what projects the verified rovs command schema into the plugins tree.
3. **gRPC is the transport layer for streaming + typed payloads — RESOLVED (corrects prior gap).** D-Bus is the registration/authority plane; **gRPC carries typed payloads and live streams.**
   - **Streaming/monitor (was hardest problem):** the daemon feeds rovs notifications (`Recv_flow_updates`/`Drain_notifications` drained internally) into **gRPC subscriptions**. `op-dbus-mirror`'s monitor consumer subscribes over gRPC instead of `OvsdbClient::monitor_db()`. **gRPC subscription signals act as mutate triggers** into the schema/projection engine.
   - **JSON-string serde cost (was Risk #4):** solved — typed protobuf over gRPC replaces passing JSON as D-Bus `Transact(s,s)` strings, restoring typed/zero-copy-friendly transport on hot paths. gRPC was being overlooked in the prior design; it is now first-class.
4. **`op-of-controller` — RESOLVED.** Absorb passive-listen (:6653) into `op-openvswitch-daemon` (single owner). **VConn is CLIENT-only (no `accept` constructor), NOT receive-only** — it already does full bidirectional I/O. **Option A locked:** Add `VConn::from_accepted_stream(Stream)` to rovs-openflow (~20 lines, upstream PR or local fork). Takes an accepted `Stream`, performs passive-side handshake (recv Hello first, then send Hello). Daemon uses a single `VConn` type for both active and passive connections — all VConn methods available identically. Daemon's passive-listen code: `TcpListener::accept()` → `Stream::Tcp(tcp_stream)` → `VConn::from_accepted_stream(stream)`.
5. **No monitoring, no DBs, single source of truth in `/dev/shm` — RESOLVED (architectural correction).** The daemon is a transport/execution proxy only. It does NOT maintain an OVSDB IDL replica via `rovs-ovsdb::Client::monitor_db()` or any similar subscription. The single source of truth is:
   - `PluginSchema` defines what exists (no schema = entity does not exist)
   - `op-projection::SchemaEngine` validates schemas, persists catalog to `/dev/shm/live-schema.json`
   - Consumers perform **1:1 direct reads** from `/dev/shm` (zero-copy, The Sled)
   - The daemon's `rovs-jsonrpc::Connection` is used **only for executing** OVSDB transact/notify — not for maintaining state
   - When a `transact` succeeds, the daemon writes the result into `SchemaEngine → /dev/shm` — that IS the state update. No OVSDB monitor needed.
   - gRPC subscription signals are "state changed in `/dev/shm`" notifications — they do NOT carry OVSDB update payloads. Consumers read the new state from `/dev/shm`.
   - The current `op-grpc-bridge::SchemaEngine` that owns `Arc<OvsdbClient>` + `monitor_db("Open_vSwitch")` + `Arc<NonNetDb>` is a **second competing source of truth** and must be reworked. Its `process_authoritative_change()` → `change_tx.broadcast()` path should feed into `op-projection::SchemaEngine → /dev/shm` instead of maintaining its own `state_cache: HashMap`.
   - The `OvsdbClient::monitor_db()` `mpsc::Receiver<serde_json::Value>` pattern in `op-dbus-mirror` is likewise wrong — it maintains an OVSDB replica in-process. Consumers should read from `/dev/shm` and subscribe to gRPC "state changed" signals.
6. **NonNetDb is deleted — RESOLVED.** `NonNetDb` (`op-jsonrpc/nonnet.rs`, ~480 lines) was an in-memory `HashMap` pretending to be an OVSDB database (`OpNonNet`) for non-OVS plugin state. It was a stopgap. Non-OVS plugins (netmaker, wireguard, hardware, software, etc.) are just plugins with their own `PluginSchema` entries in `plugin_schema_defs.rs`. Their state goes into `SchemaEngine → /dev/shm` alongside OVS state — no "NonNet" namespace, no `OpNonNet` JSON-RPC facade, no separate handling. The `NonNetDb` code, `nonnet_staging.rs` server, and all `Arc<NonNetDb>` references are deleted.

---

## 4. Scope — EVERYTHING (user-confirmed)

| Item | Status |
|---|---|
| Daemon + 2 proxies + delete OvsdbClient | **IN** |
| Absorb op-of-controller passive-listen into daemon | **IN** |
| gRPC transport: typed payloads + subscription streams as mutate triggers | **IN** |
| Refactor openflow.rs, net.rs, ovsdb_bridge.rs, full_system.rs, privacy_router.rs | **IN** |
| `rovs_commands` schema plugin (projected under /org/opdbus/v1/plugins/) | **IN** |
| op-tools / op-chat / op-grpc-bridge / op-dbus-mirror / op-web consumer migration | **IN** |
| Xray + Netmaker/netclient consolidation into hypervisor | **IN** |
| Re-enable + schema-ify disabled `lxc.rs`, `netmaker.rs` (add schemas) | **IN** |
| Workspace-wide subprocess sweep incl. ~150 `Command::new` in `op-agents/**` | **IN** |
| Migrate the 2 extra direct-socket `OvsdbClient` types (op-tools/builtin, op-jsonrpc) onto daemon | **IN** |
| `--enable-advanced-protocols` bindgen path (raw OVS C structs) | **IN** |

---

## 5. Proposed milestones (DRAFT — count needs user agreement)

Given EVERYTHING scope, proposed **8 milestones**:

- **M1 — Hypervisor daemon (core).** New `op-openvswitch-daemon` bin in `op-network`; native rovs-jsonrpc + rovs-openflow objects on the two paths (`org.opdbus.rovs.jsonrpc`/`.openflow`); absorb passive OpenFlow controller listen (:6653); system-bus registration; D-Bus introspection green.
- **M2 — gRPC transport + projection wiring.** Typed protobuf surface for the rovs primitives; gRPC subscription streams; subscription-signal → mutate-trigger path into the schema engine; establish that only `/org/opdbus/v1/plugins/` is projected.
- **M3 — Consumer proxies + OvsdbClient delete.** `RovsJsonRpcProxy`/`RovsOpenFlowProxy`; migrate all ~50+ OvsdbClient consumers (8 crates) to proxies/gRPC; port op-dbus-mirror monitor onto gRPC subscriptions; delete `ovsdb.rs`; `cargo build --workspace` green.
- **M4 — Plugin native-ization.** openflow.rs (drop `ovs-ofctl`/`run_ovs_ofctl` + CLI `is_available` gate), net.rs, ovsdb_bridge.rs, full_system.rs (drop `ovs-vsctl`), privacy_router.rs → all over proxies/gRPC.
- **M5 — `rovs_commands` schema plugin.** `rovs_commands_plugin_schema()` in plugin_schema_defs.rs; mod.rs + default_registry.rs wiring; projected at `/org/opdbus/v1/plugins/rovs_commands`.
- **M6 — Re-enable lxc.rs + netmaker.rs.** Add `lxc_plugin_schema()` + `netmaker_plugin_schema()`; swap OvsdbClient→proxy / netclient-subprocess→daemon; un-comment in mod.rs/default_registry.rs.
- **M7 — Workspace subprocess sweep + extra clients.** Eliminate forbidden runtime `Command::new` (ovs-*/systemctl/s6-svc/ip/wg/etc.) across op-agents and elsewhere per AGENTS.md; migrate the 2 extra direct-socket OvsdbClient types; bindgen `--enable-advanced-protocols` raw-struct path.
- **M8 — Xray/Netmaker consolidation + deploy.** Fold xray/netclient lifecycle behind daemon/D-Bus; remove `ovs-vsctl set-controller` holdout + s6 `run` shell-outs; s6 service for the daemon; end-to-end validation.

### Milestone count — LOCKED at 8 (user-delegated to recommendation, 2026-06-04).
- **Sequencing rule (user-confirmed):** gRPC subscription path (M2) MUST land before the `OvsdbClient` delete (M3), so op-dbus-mirror's monitor consumer has its gRPC subscription path ready before `monitor_db` is removed.

### Validation posture (user-confirmed)
- **Per-milestone "done" gate:** `cargo build --workspace` + `cargo clippy --workspace --all-targets --all-features -- -D warnings` + `cargo fmt --all -- --check` all green, **plus a cognitive-MCP (:3003) check**. Full D-Bus-introspection / gRPC end-to-end is NOT a mandatory per-milestone gate (run at integration points as useful).
- **Validation surface available:** D-Bus introspection (op-inspector), gRPC bridge, cognitive-MCP (:3003, Netmaker WG `100.90.37.254`).

### Credentials / infra (user-confirmed)
- Posture: **prompt if missing.** No credentials pre-provisioned. Daemon needs system-bus access + `/run/openvswitch/db.sock`; Xray/Netmaker (M8) need GitHub release downloads + s6 service install. Surface any missing prerequisite to the user at the point it is needed rather than blocking up front.

---

## 6. Readiness check results (2026-06-04)

### Pre-flight blockers (must fix BEFORE any milestone gate)
1. **fmt dirty:** `cargo fmt --all -- --check` fails — 172 diffs across 32 files (mostly op-cognitive-mcp, op-plugins, op-web). Run `cargo fmt --all` first.
2. **clippy gate will fail:** op-web has 24 warnings that become errors under `-D warnings`. Fix before M1 gate.

### Qdrant infra (resolved 2026-06-04)
- **Qdrant container:** Incus `qdrant` container (Debian trixie), Qdrant 1.18.2, no NIC
- **Container networking:** Socket-only — Incus proxy devices bridge host → container:
  - `host:/run/qdrant.sock` → `container:127.0.0.1:6334` (gRPC, for Xray UDS routing)
  - `host:127.0.0.1:6333` → `container:127.0.0.1:6333` (HTTP admin)
  - `host:127.0.0.1:6334` → `container:127.0.0.1:6334` (gRPC TCP, for cognitive-mcp)
- **cognitive-mcp connection:** `COGNITIVE_MCP_QDRANT_URL=http://127.0.0.1:6334` (set in s6 env)
- **RAG pipeline:** Voyage AI → Qdrant `repomix_rag` collection (1024-dim cosine). Smoke-tested: Voyage doc+query embed → Qdrant upsert → cosine search → ranked results verified.
- **Memory loop:** Qdrant is part of the chatbot's learning feedback loop. Decisions/code → SchemaEngine → /dev/shm → Voyage embed → Qdrant → chatbot semantic retrieval → new decisions → /dev/shm → embed → Qdrant (loop). Without this loop, the chatbot has zero persistent architectural memory and cannot learn from mission decisions. The SHM SSOT rule, framing bug findings, NonNetDb deletion — these become retrieval-queryable invariants that surface when the chatbot considers recreating a deleted pattern.
- **Xray routing:** Xray natively supports Unix Domain Sockets via `listen` (inbound socket path, e.g. `/dev/shm/xray_vless.sock,0666`) and `dest` (fallback destination socket path, e.g. `/run/qdrant.sock`). Use `/dev/shm/` for file-based sockets (bypasses disk I/O) or `@` prefix for abstract sockets (auto-cleanup on restart). The `UnixSocketPlugin` `apply_state()` should generate Xray routing rules from registered socket endpoints — mapping declared sockets to Xray inbound/outbound config with OpenFlow tags.
- **OpenFlow tag routing:** Will route tagged traffic through Xray to the correct socket endpoint. Tags defined in `plugin_schema_defs.rs` and projected into `/dev/shm`. This is mission work (M2+).
- **UnixSocketPlugin:** Already exists at `op-plugins/src/state_plugins/unix_socket.rs` with schema defining `path`, `port`, `protocol`, `label` — example is `/run/qdrant.sock`. Plugin's `apply_state()` is currently a no-op stub; wiring it to generate Xray routing config from declared socket endpoints is mission work.
- **VConn is CLIENT-only (no accept constructor), NOT receive-only — confirmed** from `rovs-openflow-0.2.0/src/vconn.rs`. `VConn` already does full bidirectional I/O (`send_message` + `recv_message`, `send_flow` + `dump_flows`, etc.). The only gap is `VConn::from_accepted_stream(Stream)` — locked Option A per user. `controller.rs` does NOT hand-roll message encoding — it reuses `Message::new().encode()`, `Flow::to_message()`, `Header::decode()`, etc. Only the listen/accept/handshake orchestration is manual.
- **Transport audit found 5 bypass sites** (see `.mission-planning/transport-audit.md`): `controller.rs` (passive OF, becomes VConn), 2 `op-jsonrpc` OVSDB clients (per-call `UnixStream`, route through daemon), `op-tools/builtin/ovsdb.rs` (3rd hand-rolled OVSDB client, route through daemon), `op-chat/tool_loader.rs` (raw TCP liveness probe, becomes D-Bus check). Plus 3 separate `OvsdbClient` implementations all hitting the same `db.sock` — the daemon unifies them.
- `OpenFlowClient` in `openflow.rs` **already uses VConn correctly** — it is the model pattern.
- **rovs-ovsdb/jsonrpc notification model is poll+blocking-wait** (no async stream). The daemon does NOT use `rovs-ovsdb::Client` with IDL monitor — it uses `rovs-jsonrpc::Connection` for execution only. When a `transact` succeeds, the result is written into `SchemaEngine → /dev/shm`. That IS the state update. gRPC subscriptions signal "state changed in /dev/shm" — consumers then read from `/dev/shm` directly (1:1 direct read).
- **gRPC subscription infrastructure already exists:** `OvsdbMirror::Monitor`, `SubscribeSignals`, `SubscribeEvents`, `Watch`, `DbusWatch` are server-streaming RPCs backed by `tokio::sync::broadcast`. **In the target architecture**, the daemon's gRPC subscription signals originate from `SchemaEngine → /dev/shm` writes (not from OVSDB monitor subscriptions). When the daemon writes a transact result to `/dev/shm`, it emits a gRPC "state changed" signal. Consumers receive the signal, then read current state from `/dev/shm`. **Missing:** OpenFlow flow-update broadcast path, OpenFlow gRPC proto service, `/dev/shm`-change → gRPC-signal wiring.
- **op-network is the sole rovs-* dependent.** No bindgen anywhere yet. OVS C headers (`/usr/include/openflow/`) and `vswitch.ovsschema` (`/usr/share/openvswitch/`) ARE present on host.
- **TLS unimplemented** in rovs-transport (risk for remote-OVS, not a blocker for local socket).

### Validation verification
- **Toolchain:** Rust 1.95.0, clippy 0.1.95, rustfmt 1.9.0, Node 26.2.0, npm 11.14.1.
- **cognitive-MCP HEALTHY:** PID 24314, listening `100.90.37.254:3003`, `/health` → `{"status":"ok","service":"op-mcp","version":"0.4.0"}`. Gate command: `curl -sf http://100.90.37.254:3003/health | jq -e '.status == "ok"'`.
- **zbus local patch** at `/home/jeremy/git/zbus/` must stay accessible or builds break.
- **33 workspace crates**, 8 directly touched by the refactor.
- **Build time:** ~2 min incremental.
