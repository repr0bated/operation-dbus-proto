# rovs Refactor Surface — Ground-Truth Blast-Radius Map

> Investigation date: 2026-06-04. Repo: `/home/jeremy/git/operation-dbus-proto`.
> Method: `rg` + direct file reads. Claims verified against disk.
>
> **CRITICAL META-FINDING:** The planning docs in `rovs-refactor/` (`stepped_plan.md`
> "Appendix: Current Implementation Status", `walkthrough.md`, `task.md`) repeatedly
> claim the refactor is **"Implemented" / "fully implemented" / "running"**. This is
> **FALSE**. None of the claimed artifacts exist on disk. This is a **greenfield**
> refactor. Every "Implemented" bullet in those docs was confabulated. Treat the
> `rovs-refactor/*.md` status claims as aspirational fiction, not state.
>
> Verified-missing (all return MISSING / no code references):
> - `crates/op-network/src/bin/op-openvswitch-daemon.rs` — **does not exist**
> - `crates/op-network/build_ovs_schemas.rs` and `build.rs` — **do not exist**
> - `crates/op-network/src/rovs_jsonrpc_proxy.rs` — **does not exist**
> - `crates/op-network/src/rovs_openflow_proxy.rs` — **does not exist**
> - `crates/op-network/src/openvswitch_proxy.rs` — **does not exist**
> - `crates/op-plugins/src/state_plugins/rovs_commands.rs` — **does not exist**
> - `RovsJsonRpcProxy` / `RovsOpenFlowProxy` symbols — **0 references in code**
> - `org.op_dbus.CrateInterface`, `org.opdbus.OpenvSwitchCommands`, `org.opdbus.rovs.*`,
>   `/org/opdbus/rovs/*`, `--enable-advanced-protocols` — **0 references in `crates/`,
>   `deploy/` (.rs/.sh/.toml)**. (Only `rovs-refactor/*.json` specs mention `org.op_dbus.CrateInterface`.)
> - `bindgen`, OVS C-header parsing — **no build script exists**; `walkthrough.md` §2 is fiction.

---

## rovs Crate API Surface

### Which `rovs-*` crates are declared, where, and versions
Single declaration site — `crates/op-network/Cargo.toml:40-45`:
```toml
# OVSDB and OpenFlow via rovs crate family
rovs-ovsdb = "0.2"
rovs-openflow = "0.2"
rovs-jsonrpc = "0.2"
rovs-types = "0.2"
rovs-transport = "0.2"
```
All resolve to **0.2.0** from crates.io (`Cargo.lock:6369-6440`):
- `rovs-jsonrpc 0.2.0` (checksum `ff092d…`) → deps: rovs-transport, serde, serde_json, thiserror 2, tokio, tracing
- `rovs-openflow 0.2.0` (checksum `4b6e48…`) → deps: bytes, nom 8, rovs-transport, thiserror 2, tokio, tracing
- `rovs-ovsdb 0.2.0` (checksum `d895e6…`) → deps: rovs-jsonrpc, rovs-transport, rovs-types, serde, serde_json, thiserror 2, tokio, tracing, uuid
- `rovs-transport 0.2.0` (checksum `1bd185…`) → deps: rustls-pemfile 2, thiserror 2, tokio, tokio-rustls 0.26, tracing
- `rovs-types 0.2.0` (checksum `8d157c…`) → deps: serde, serde_json, thiserror 2, uuid

`op-network` is the **only** workspace crate that depends on `rovs-*` (`Cargo.lock:4714-4718` lists them under the `op-network` package). Every other crate touches OVS only **through** `op-network` (`op_network::ovsdb::OvsdbClient`, `op_network::openflow::*`).

### Greenfield confirmation
- No local `rovs` crate exists; no `op-openvswitch-daemon` crate or bin exists.
- `op-network`'s only `[[bin]]` targets are `op-of-controller`, `op-xdp-wg`, `op-ovsbr0-afxdp`, `op-ovsbr0-setup` (`crates/op-network/Cargo.toml:53-67`). No daemon.

### Public rovs symbols the codebase actually imports today
- `crates/op-network/src/ovsdb.rs:25-29`:
  `use rovs_ovsdb::{Client, ClientConfig, RowRef, Transaction};`
  `use rovs_types::{Atom, Datum};`
  `use rovs_transport::Reconnect;`
  - Uses `Client::connect(&socket_addr)`, `ClientConfig`, IDL replica via `Client::run()` pump, `Transaction`, `RowRef::Uuid/Named::to_json()`, `Atom`/`Datum` for typed wire encoding.
- `crates/op-network/src/openflow.rs:9-15`:
  `use rovs_openflow::{Message, MessageType, Version};` · `use rovs_transport::Reconnect;` · `pub use rovs_openflow::Match as FlowMatch;`
  - Uses `rovs_openflow::Flow` (`::add()`, `::delete()`), `ActionList`, `OutputPort::Port`, `Version::{Of10,Of13}`, and **`rovs_openflow::VConn`** (`crates/op-network/src/openflow.rs:99,122` — `VConn::connect(&addr).await`). NB: comment at `controller.rs:9` notes `VConn` only supports **active/outbound** connections (relevant: the OpenFlow controller listens passively, so `controller.rs` hand-rolls wire encoding via `rovs_openflow` types + `bytes`).
- `crates/op-network/src/controller.rs:13-14`:
  `use rovs_openflow::{ActionList, Flow, Match, Message, MessageType, OutputPort, Version};` · `use rovs_transport::Reconnect;`
- `crates/op-network/src/bin/op-ovsbr0-setup.rs:24-26`:
  `use rovs_jsonrpc::Connection;` · `use rovs_ovsdb::{Client, Transaction};` · `use rovs_transport::{Address, Stream};`
- `crates/op-network/src/bin/op-ovsbr0-afxdp.rs:16`: `use rovs_ovsdb::{Client, Transaction};`

> The daemon-to-be must re-expose, over D-Bus, the rovs primitives currently consumed
> **in-process** by `ovsdb.rs` (rovs-jsonrpc/rovs-ovsdb `Transact`/`Notify`/`Send_message`/
> `Recv_message`) and `openflow.rs`/`controller.rs` (`rovs_openflow::VConn`:
> `Connect`/`Send_flow`/`Dump_flows`/`Recv_packet_in`). The two spec files
> `rovs-refactor/rovs-jsonrpc.json` and `rovs-refactor/rovs-openflow.json` enumerate the
> intended D-Bus method set (interface `org.op_dbus.CrateInterface`), but note the
> daemon spec/plan and AGENTS.md disagree on the interface NAME — see Open Questions.

---

## OvsdbClient Consumers (deletion blast radius)

### Definition (deletion target)
- **`crates/op-network/src/ovsdb.rs:163`** — `pub struct OvsdbClient` (impl block opens `:170`). File is **991 lines** (`wc -l` = 990 trailing-newline count; task's "~1000 lines" is accurate). It is a backward-compat wrapper over a persistent `rovs_ovsdb::Client` + IDL monitor pump.
- Re-exported at `crates/op-network/src/lib.rs:26` (`pub use ovsdb::OvsdbClient;`) and `:37` (prelude).
- **Public API to be replaced** (every method that consumers call must have a `RovsJsonRpcProxy` equivalent or a payload-constructing call site):
  `new()` :174 · `with_socket()` :183 · `list_dbs()` :252 · `ensure_initialized()` :261 · `transact(Value)` :281 · `transact_db(db, ops)` :296 · `transact_simd(simd_json::OwnedValue)` :312 · `commit_txn(&mut Transaction)` :325 · `bridge_exists()` :336 · `create_bridge()` :351 · `delete_bridge()` :379 · `list_bridges()` :430 · `add_port()` :442 · `add_port_with_type()` :450 · `delete_port()` :484 · `list_bridge_ports()` :531 · `get_bridge_info()` :559 · `set_bridge_property()` :573 · `set_interface_type()` :610 · `dump_db(db)` :647 · `monitor_db(db) -> mpsc::Receiver<Value>` :752.

> NOTE: There are **two unrelated** `OvsdbClient` types that are NOT the deletion target
> (separate hand-rolled RFC-7047 clients, no rovs dep):
> - `crates/op-tools/src/builtin/ovsdb.rs:24` (`UnixStream` JSON-RPC client; header comment claims "NO CLI TOOLS").
> - `crates/op-jsonrpc/src/ovsdb.rs:14` + `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:10` (re-exported `op-jsonrpc/src/lib.rs:14`; `op-jsonrpc/src/server.rs:20` uses `crate::ovsdb::OvsdbClient`, i.e. its OWN type, not op-network's).
> These are out of scope for the delete but ARE in scope for the "D-Bus-first" sweep (they bypass the daemon by talking to the OVSDB socket directly).

### Consumers of `op_network::ovsdb::OvsdbClient` (the type being deleted), by crate

**op-network (self):**
- `src/plugin.rs:14,188,249,282` — `NetworkPlugin` constructs `OvsdbClient::new()` in 3 methods (bridge/port persistence). → must call `RovsJsonRpcProxy::transact(...)`.
- `src/ovs_capabilities.rs:139,144,215` — `list_dbs()` call inside a capability/excuse probe + doc strings. → reachability probe via proxy.

**op-plugins (state plugins):**
- `src/state_plugins/openflow.rs:215,220` — `OpenFlowPlugin.ovsdb_client: Arc<OvsdbClient>`; `discover_containers()` calls `list_bridges()`/`list_bridge_ports()`/`transact_simd()` (`get_port_ofport`). → `RovsJsonRpcProxy` for OVSDB selects; `RovsOpenFlowProxy` for flow ops (currently `ovs-ofctl`, see Subprocess Inventory).
- `src/state_plugins/net.rs:149,229,339,526` — `apply_ovs_config()` uses `bridge_exists/create_bridge/list_bridge_ports/add_port`; 3 other read sites. → JSON-RPC payloads via proxy.
- `src/state_plugins/ovsdb_bridge.rs:9,86,98` — `OvsBridgePlugin.ovsdb: Arc<OvsdbClient>` constructed in `new()`. → proxy field.
- `src/state_plugins/privacy_router.rs:8,344,474,521` — `OvsdbClient::new()` in 3 sites (privacy socket wiring; also imports `openflow::OpenFlowClient`). → proxy.
- `src/state_plugins/lxc.rs:126,181,735,1018` — `OvsdbClient::new()` in 4 sites. **⚠ MODULE DISABLED** (`mod.rs:9` `// pub mod lxc;`, `mod.rs:74` `// pub use lxc::LxcPlugin;`). Not compiled; refactoring it is optional/cosmetic unless it is re-enabled. `task.md` lists it as a refactor target without noting it is dead code.

**op-tools (LLM tool implementations):**
- `src/builtin/ovs.rs:7,76` — `use op_network::OvsdbClient;` then `OvsdbClient::new()`.
- `src/builtin/ovs_tools.rs:85,87,353,360,436,443,506,519,577,584,661,668,712,714,779,789,843,1020,1037,1098,1110` — ~21 `OvsdbClient::new()` sites (one tool fn per OVS verb). Largest single consumer file.
- `src/builtin/openflow_tools.rs:86,292,377` — `op_network::ovsdb::OvsdbClient::new()` ×3.

**op-web:**
- `src/privacy_network.rs:13,83` — `use op_network::{openflow::OpenFlowClient, OvsdbClient};` + `OvsdbClient::new()`.
- `src/bin/op-dbus.rs:15,35` — `Arc::new(OvsdbClient::new())` wired into the main `op-dbus` daemon (passed to mirror/grpc layers). **High-value integration point** — this is where the proxy/daemon handle would be injected.

**op-grpc-bridge:**
- `src/schema_engine.rs:21,74,124` — `SchemaEngine.ovsdb: Arc<OvsdbClient>` (field + ctor). Central to the "1:1 direct read" schema projection path.
- `src/bin/op-grpc-bridge.rs:18,34` — `Arc::new(OvsdbClient::new())`.

**op-dbus-mirror:**
- `src/lib.rs:15,41,65` — struct field `ovsdb: Arc<OvsdbClient>` + ctor param.
- `src/jsonrpc_interface.rs:11,33,38` — `client: Arc<OvsdbClient>` + `new(client, schema_engine)`.
- `src/event_sources/ovsdb.rs:3,9,23` — uses `OvsdbClient::monitor_db()` for full-IDL-snapshot event source. → needs proxy/daemon **streaming** equivalent (D-Bus signal or `Monitor_flows`/`monitor_db`); the rovs-jsonrpc spec lacks an explicit `Monitor_db`, see Open Questions.
- `src/event_dispatcher.rs:5,24,36` — `ovsdb_client: Arc<OvsdbClient>` field + ctor.
- `src/bin/ovs-dbus-init.rs:5,30` — `Arc::new(OvsdbClient::new())`.

**op-chat:**
- `src/tool_loader.rs:21,1458,1501,1544,1648,1691,1736,1781` — `use op_network::OvsdbClient;` + 7 `OvsdbClient::new()` tool-loader sites.

**Total deletion blast radius:** 8 crates (op-network, op-plugins, op-tools, op-web, op-grpc-bridge, op-dbus-mirror, op-chat) + self. ~50+ call sites; `op-tools/ovs_tools.rs` (~21) and `op-dbus-mirror` (5 files, incl. the streaming `monitor_db` consumer) are the riskiest.

---

## Plugin Refactor Surface

Registration mechanics (needed to add `rovs_commands`):
- **`crates/op-plugins/src/state_plugins/mod.rs`** — each plugin is `pub mod X;` + `pub use X::Plugin;`. (Adding `rovs_commands` requires a new `pub mod rovs_commands;` here.)
- **`crates/op-plugins/src/default_registry.rs`** — `load_plugin()` `match` arm per plugin (e.g. `:openflow => OpenFlowPlugin::new()`); `default_auto_load()` Vec (`net`, `openflow`, `ovsdb_bridge`, … are auto-loaded `:78-94`); `available_plugins()` static list (`:available_plugins` has `// "netmaker"`, `// "lxc"` commented out). Adding `rovs_commands` = new `match` arm + (optional) auto-load entry + (optional) `available_plugins` entry.
- **`crates/op-plugins/src/state_plugins/plugin_schema_defs.rs`** (3179 lines) — **single schema source of truth** (AGENTS.md §4). Schemas are `pub(crate) fn`: `ovsdb_bridge_plugin_schema()` `:221`, `net_plugin_schema()` `:553`, `openflow_plugin_schema()` `:732`. Helpers: `schema_from_state()` `:12`, `simple_schema()` `:90`. Adding `rovs_commands` = new `pub(crate) fn rovs_commands_plugin_schema()` here, then `Some(super::plugin_schema_defs::rovs_commands_plugin_schema())` in the plugin's `schema()`.
- Each plugin's `schema()` returns `Some(super::plugin_schema_defs::<name>_plugin_schema())` — confirmed for openflow `:1400-1401`, net `:681-682`, ovsdb_bridge `:192-193`.

Per-plugin current approach → required change:

**`openflow.rs`** (`op-plugins/src/state_plugins/openflow.rs`, 1866 lines)
- OVSDB reads: `Arc<op_network::ovsdb::OvsdbClient>` (`:213-220`) — `list_bridges`, `list_bridge_ports`, `transact_simd` (`get_port_ofport`).
- Flow writes/reads: **subprocess** `tokio::process::Command::new("ovs-ofctl")` via `run_ovs_ofctl()` `:336-353`, called at `:596` (`add-flow`), `:602` (`dump-flows`), `:624` (delete). **⚠ This directly contradicts `walkthrough.md` §4 which claims `install_flow`/`delete_flow`/`query_flows` were "refactored to drop `tokio::process::Command` calls" and now "invoke `add_flow`/`delete_flow` via the native zbus proxy." FALSE — the subprocess is still there.**
- `is_available()` `:1404` requires `/var/run/openvswitch/db.sock` **and** `/usr/bin/ovs-ofctl` (i.e. plugin is gated on the CLI binary existing).
- Schema: `openflow_plugin_schema()`.
- **Change:** OVSDB selects → `RovsJsonRpcProxy::transact`; flow ops → `RovsOpenFlowProxy::{send_flow,dump_flows}`; drop `run_ovs_ofctl` and the `ovs-ofctl` `is_available()` gate.

**`net.rs`** (`op-plugins/src/state_plugins/net.rs`)
- OVS: `op_network::ovsdb::OvsdbClient::new()` at `:149,229,339,526`. `apply_ovs_config()` (`:332+`) calls `bridge_exists/create_bridge/list_bridge_ports/add_port`. Also uses `op_network::rtnetlink::link_up` (native netlink — fine). Skips `nm-*`/`wg*` ports (netclient-managed).
- Schema: `net_plugin_schema()`.
- **Change:** construct OVSDB JSON-RPC `insert`/`mutate`/`select` payloads, send via `RovsJsonRpcProxy::transact`.

**`ovsdb_bridge.rs`** (`op-plugins/src/state_plugins/ovsdb_bridge.rs`)
- `OvsBridgePlugin.ovsdb: Arc<OvsdbClient>` (`:86`), ctor `:98`.
- Schema: `ovsdb_bridge_plugin_schema()`.
- **Change:** swap field type to `RovsJsonRpcProxy`; mutations via `transact`.

**`lxc.rs`** (`op-plugins/src/state_plugins/lxc.rs`) — **⚠ DISABLED in `mod.rs`**
- OVS: `OvsdbClient::new()` at `:126,181,735,1018`. Also subprocess `btrfs` (`:478,497`), `chmod` (`:659,719`).
- `StatePlugin for LxcPlugin` `:902` — **defines `name()`/`version()`/`query_current_state()` but NO `schema()` method** → violates AGENTS.md §4 "one schema file" rule; likely why it's commented out. State comes from Proxmox API (`discover_from_proxmox`), `is_available()` gates on `/etc/pve`.
- **Change:** if re-enabled, add `lxc_plugin_schema()`, swap OvsdbClient→proxy. Otherwise leave disabled (lowest priority; `task.md` overstates its relevance).

**`full_system.rs`** (`op-plugins/src/state_plugins/full_system.rs`)
- Does **NOT** use OvsdbClient at all. Talks to OVS via **subprocess** `Command::new("ovs-vsctl")` (`:344` `list-br`, `:350` port list). Also shells out to `hostname`/`uname`/`systemctl`/`dpkg-query`/`rpm`/`id`/`lsblk`/`lxc-ls`/`docker`/`hostnamectl` — it's a broad host-inventory reader.
- **Change:** OVS introspection (`:344,350`) → `RovsJsonRpcProxy::transact` (`select` on Bridge/Port). The non-OVS subprocesses are out of the rovs scope but in scope for the general D-Bus-first sweep (see Open Questions — full_system is fundamentally a CLI-scraping inventory plugin).

---

## Subprocess Inventory (workspace-wide)

`Command::new` / `tokio::process::Command` are pervasive. The bulk (~150 sites) live in
**`crates/op-agents/src/agents/**`** — these wrap external dev/CLI tools (`cargo`, `npm`,
`go`, `git`, `rustfmt`, `kubectl`, `terraform`, `docker`, `sqlite3`, `rg`, `python3`, …)
as "expert agent" actions. They are **not** OVS/network control-plane bypasses and are
arguably legitimate tool-runner behavior, but AGENTS.md §4 is absolute ("D-Bus only");
flag for a policy decision, not auto-rewrite. Below: only the **control-plane-relevant**
binaries the task asked to flag.

### FORBIDDEN per AGENTS.md (plugin/service RUNTIME code)
**`ovs-vsctl`**
- `crates/op-mcp/src/tools/ovs.rs:40` (generic `ovs-vsctl <args>` runner)
- `crates/op-plugins/src/state_plugins/full_system.rs:344,350`

**`ovs-ofctl`**
- `crates/op-mcp/src/tools/ovs.rs:49`
- `crates/op-plugins/src/state_plugins/openflow.rs:337` (`run_ovs_ofctl`)

**`systemctl`**
- `crates/op-state/src/authority.rs:14,18,23,27,40,50`
- `crates/op-plugins/src/service_def.rs:412,457`
- `crates/op-plugins/src/state_plugins/service.rs:227,284`
- `crates/op-plugins/src/state_plugins/full_system.rs:380,392`
- `crates/op-plugins/src/state_plugins/netmaker.rs:70,324,316,325` (**DISABLED module**; `apt`/`systemctl enable` install path)
- `crates/op-introspection/src/mod.rs:631`
- `crates/op-tools/src/builtin/self_tools.rs:904`, `crates/op-tools/src/builtin/anydesk.rs:407,440,565,608,628`

**`s6-svc`**
- `crates/op-plugins/src/state_plugins/compact_mcp.rs:124`
- `crates/op-plugins/src/state_plugins/cognitive_mcp.rs:126`

**`netclient`** (all in **DISABLED** `netmaker.rs`)
- `crates/op-plugins/src/state_plugins/netmaker.rs:64` (`which netclient`), `:79` (`list`), `:150,170` (read/join)

**`ip`** (rtnetlink fallback / agents)
- `crates/op-network/src/rtnetlink.rs:383` (fallback path; crate also has native rtnetlink)
- `crates/op-tools/src/builtin/rtnetlink_tools.rs:82`
- `crates/op-agents/src/agents/infrastructure/network.rs:26,37`

**`wg` / WireGuard**
- `crates/op-web/src/wireguard.rs:73`; `crates/op-web/src/handlers/dashboard.rs:126`; `crates/op-web/src/handlers/vpn.rs:51,68,115`
- `crates/op-identity/src/wg.rs:26`; `crates/op-identity/src/wireguard.rs:38,66,108,133` (`:108` is `ip`); `crates/op-identity/src/bin/op-identity-sled.rs:98`
- `crates/op-mcp-proxy/src/session.rs:95,118`
- `crates/op-dbus-mirror/src/lib.rs:568` (`wg show all dump` for state read)

**`xray`**
- `crates/op-identity/src/schema_bridge.rs:780` — `Command::new("xray").args(["run","-c",SHM_XRAY_CONFIG]).spawn()` (launches Xray from /dev/shm config). The one runtime Xray spawn in crate code.

**`dhclient`**
- `crates/op-network/src/plugin.rs:404` (`tokio::process::Command::new("dhclient")`)

**`btrfs` / `chmod` / misc** (lxc.rs DISABLED): `lxc.rs:478,497` btrfs, `:659,719` chmod. `op-blockchain/src/btrfs_numa_integration.rs:256` btrfs.

### ALLOWED (bootstrap binaries / `src/bin/*` entrypoints — AGENTS.md §4 exception)
- `crates/op-network/src/bin/op-ovsbr0-setup.rs:110,119,177` (`s6-svc`), `:126` (`s6-svstat`), `:168` (`ovs-dpctl`), `:171,540` (`ip`). This bin is the rovs-ovsdb-native bridge setup tool invoked by deploy hooks.
- `crates/op-network/src/bin/op-xdp-wg.rs:281` (`ip`).
- `deploy/*.sh` (see next section) — bootstrap scripts; explicitly allowed.

### Anti-bypass guardrails already in the codebase (consistency targets)
- `crates/op-web/src/routes/admin.rs:221` — forbidden-cmd list `["ovs-vsctl","systemctl","ip addr","nmcli"]`.
- `crates/op-web/src/orchestrator/anti_hallucination.rs:15-18` & `parsing.rs:155-158` — map `ovs-vsctl`→`ovs_* tools`, `ovs-ofctl`→`ovs_add_flow/...`, `ovsdb-client`→`ovs_* tools`.
- `crates/op-chat/src/system_prompt.rs:93,153-157` — prompt forbids `ovs-vsctl`/`ovs-ofctl`/`ovsdb-client`, points at rovs-native tools; `:93` admits "Write tools (add/delete bridge/port) are not yet registered" and tells the LLM to `shell_execute op-ovsbr0-setup`.
  > These guardrails forbid the LLM from emitting CLI, yet the plugins/services themselves still shell out (openflow.rs/full_system.rs/op-mcp tools/ovs.rs). The daemon refactor closes that gap.

---

## Xray + Netmaker/Netclient Consolidation

### Deploy / bootstrap scripts (allowed-to-shell, but the consolidation target)
- **`deploy/setup-hypervisor-xray.sh`** — downloads `Xray-linux-64` from GitHub, writes `/etc/xray/config.json` (3 VLESS inbounds: `op-web-tls` :443, `op-grpc-tls` :50051, `ghostbridge-reality` :8443 with `xtls-rprx-vision` + REALITY to `www.microsoft.com`; fallbacks to `10.200.0.1:8080/50051`), creates an **s6** service `/etc/s6/sv/xray` (`xray run -c /etc/xray/config.json`).
- **`deploy/setup-hypervisor-netclient.sh`** — downloads Netmaker `netclient`, s6 service `netclient daemon`, **plus** a polling s6 daemon `netmaker-ovs-attach` that waits for the `netmaker` link then runs `VETH_HOST=netmaker /usr/local/bin/op-ovsbr0-setup` (already rovs-ovsdb-native, not `ovs-vsctl`), and sets `net.ipv4.ip_forward`.
- **`deploy/setup-hypervisor-controller.sh`** — builds/installs `op-of-controller`, s6 service (`OF_CONTROLLER_LISTEN=127.0.0.1:6653`, `OF_FLOW_PAIRS=grpc-uplink0:ovsbr0`, `OVSDB_SOCKET=/run/openvswitch/db.sock`), and **`ovs-vsctl set-controller ovsbr0 tcp:127.0.0.1:6653`** (last CLI holdout in this script).
- Other deploy touchpoints (refs only): `deploy/incus/privacy-xray-ingress/etc/xray/config.json` (modified, see `git status`), `deploy/incus/privacy-xray-ingress/usr/local/sbin/wg-xray-set-network.sh`, `.../etc/systemd/system/{wgcf-up,wg-xray-network,xray-ghostbridge}.service`, `deploy/systemd/networkd/35-priv-xray.network`, `deploy/s6/droid-daemon/dependencies.d/wg-netmaker`, `deploy/op-xdp-wg/up`.

### Crate code touching Xray / Netmaker / WireGuard
- **Xray (runtime):** `crates/op-identity/src/schema_bridge.rs:780` spawns `xray` from a `/dev/shm` config (`SHM_XRAY_CONFIG`); ties to the A.N.N.A./Shuttle sled flow (`watch_wireguard_handshakes`). Schema `plugin_schema_defs.rs:1332,1356` defines an `"xray"` field; `privacy_router.rs:1078,1255` references `"xray"` privacy socket. `op-grpc-bridge/proto/privacy_network.proto:64,76,122` models `"xray"`/`"wgcf"`/`"ovsbr0"` components.
- **Netmaker:** **`crates/op-plugins/src/state_plugins/netmaker.rs`** is the would-be management plugin but is **DISABLED** (`mod.rs:8` `// pub mod netmaker;`, `mod.rs:73` `// pub use netmaker::NetmakerPlugin;`, `default_registry.rs available_plugins` `// "netmaker"`). It shells `netclient`/`systemctl`/`apt` and has **no `schema()`** (AGENTS.md violation).
- **WireGuard reads/writes (scattered, no single owner):** `op-web` (wireguard.rs, handlers/dashboard.rs, handlers/vpn.rs), `op-identity` (wg.rs, wireguard.rs, bin/op-identity-sled.rs), `op-mcp-proxy/session.rs`, `op-dbus-mirror/lib.rs:568`. There IS a `WireGuardPlugin` (`state_plugins/wireguard.rs`, registered/auto-loadable) but the `wg` subprocess reads above bypass it.

### What consolidating into the hypervisor would touch
1. Move Xray lifecycle (start/stop/config) and Netmaker/netclient lifecycle behind the new `op-openvswitch-daemon` (or a sibling hypervisor service) as D-Bus objects, replacing: the s6 `run` scripts in the 3 setup scripts, the `xray`/`netclient` subprocess spawns in crate code, and the `ovs-vsctl set-controller` in `setup-hypervisor-controller.sh`.
2. Re-enable + schema-ify `netmaker.rs` (add `netmaker_plugin_schema()`), replace `netclient`/`systemctl` subprocess with daemon calls.
3. Unify the scattered `wg` reads onto `WireGuardPlugin` / daemon.
4. The "privacy socket" model (`priv_wg`, `priv_xray`, `priv_warp`) is already referenced in `openflow.rs`/`privacy_router.rs`/`openflow_obfuscation.rs` and proto — these become flow targets the daemon programs via `RovsOpenFlowProxy::send_flow`. (Note: `stepped_plan.md` §2 claims this privacy-socket flow generation is "Implemented" — only the *discovery/classification* helpers exist; flow programming still goes through `ovs-ofctl`.)

---

## Open Questions / Risks

1. **Interface NAME collision.** The spec JSONs (`rovs-jsonrpc.json`, `rovs-openflow.json`) and `task.md` use **`org.op_dbus.CrateInterface`**; `stepped_plan.md`/`implementation_plan.md` use **`org.opdbus.rovs.jsonrpc`** / **`org.opdbus.rovs.openflow`**; AGENTS.md canonical convention is `org.opdbus.v1.*`. **Both spec interfaces are literally the same name** (`org.op_dbus.CrateInterface`) on two different object paths — zbus allows this, but it conflicts with the per-plugin canonical interface scheme. **Must pick one naming scheme before coding.** (Parent task statement says interface = `org.op_dbus.CrateInterface`, paths `/org/opdbus/rovs/{jsonrpc,openflow}` — recommend confirming this overrides the `org.opdbus.v1` convention for these two objects.)
2. **Streaming / monitor gap.** `op-dbus-mirror/event_sources/ovsdb.rs` depends on `OvsdbClient::monitor_db()` (continuous full-IDL snapshots via `mpsc::Receiver`). The rovs-jsonrpc D-Bus spec only has `Notify`/pending-notification polling (`Has_pending_notifications`, `Pop_notification`, `Drain_notifications`), and openflow has `Monitor_flows`/`Recv_flow_updates`. **How does the event-source pump get a live stream over D-Bus?** (D-Bus signals vs. long-poll of `Drain_notifications`.) This is the single hardest consumer to port and the most under-specified.
3. **`VConn` is client-only (no accept constructor), NOT receive-only.** VConn already does full bidirectional I/O (`send_message`+`recv_message`, `send_flow`+`dump_flows`, etc.). The gap is solely a `VConn::from_accepted_stream(Stream)` constructor. `controller.rs:9`'s comment "VConn only supports active (outbound) connections" is about connection *initiation*, not data flow. The daemon needs either (A) a ~20-line addition to rovs-openflow or (B) controller.rs's manual-handshake approach. Clarify preference.
4. **`transact_simd` / simd_json on the wire.** `OvsdbClient::transact_simd(simd_json::OwnedValue)` is used by `openflow.rs:get_port_ofport`. D-Bus `Transact(method:s, params:s)` passes JSON **as strings** — every consumer must serialize/deserialize at the boundary, losing the zero-copy/simd benefit and adding parse cost on a hot path. Confirm acceptable.
5. **`rovs-refactor/*.md` are unreliable.** Their "Implemented" appendices are false (verified above). Risk: milestone planning that trusts them will skip work that was never done. Treat them as a wishlist; trust only this file's disk-verified findings.
6. **Scope of the D-Bus-first sweep.** ~150 `Command::new` sites in `op-agents/**` invoke external dev tools (cargo/npm/git/kubectl/…). Strict AGENTS.md §4 forbids subprocess in non-bootstrap code, but these are tool-runners, not control-plane. **Need an explicit ruling** on whether op-agents is in/out of the sweep, else scope balloons.
7. **Disabled modules listed as targets.** `task.md` lists `lxc.rs` (disabled, no schema) and the plan leans on netmaker (disabled, no schema). Confirm whether re-enabling them is in scope or they stay dead.
8. **Two extra `OvsdbClient` types** (`op-tools/builtin/ovsdb.rs`, `op-jsonrpc/src/ovsdb.rs`) talk to the OVSDB socket directly, bypassing the daemon. Not the delete target, but they violate "D-Bus only." In scope?
9. **`is_available()` CLI gate.** `openflow.rs:1404` requires `/usr/bin/ovs-ofctl` to exist for the plugin to load. After native-izing, this gate (and similar binary-existence checks) must be rewritten or the plugin silently won't load on a CLI-free host.
10. **`op-of-controller` env contract.** `setup-hypervisor-controller.sh` hard-codes `OF_FLOW_PAIRS=grpc-uplink0:ovsbr0` and `OVSDB_SOCKET=/run/openvswitch/db.sock`. If the daemon owns the socket, these env-driven bins need to either coexist or be folded in — interaction/ownership of `/run/openvswitch/db.sock` between daemon and `op-of-controller` is unspecified.

### Sections where evidence was thin
- **Streaming/monitor port (Risk #2):** I confirmed `monitor_db` is consumed but did NOT trace the full rovs-jsonrpc/rovs-openflow upstream API to confirm a streaming primitive exists at v0.2.0 (the crates are external; only their imported symbols were inspected). The notification-polling methods in the spec may be the intended substitute, unverified.
- **External rovs 0.2.0 full public API:** characterized only from in-repo import/usage; I did not read the rovs crate source (not vendored). `VConn` passive-connect limitation is taken from an in-repo code comment, not the upstream docs.
- **`privacy_router.rs` / `openflow_obfuscation.rs` flow logic:** I confirmed privacy-socket *references* but did not fully read these files' flow-generation paths; the claim that flow programming still uses `ovs-ofctl` is inferred from `openflow.rs:run_ovs_ofctl` being the only flow-write path found.
