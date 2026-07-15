# Factory Handoff — ghostbridge-mux + OpenFlow D-Bus conversion

Pick up from here. The prior factory session (droid CLI, session
`f35ebd29-b6cc-4dbf-9454-e7a41782f822`, logs in
`~/.factory/sessions/-home-jeremy-git-operation-dbus-proto/`) died mid-task from
repeated BYOK/provider 429s, not from a design dead-end — the last output before it
degenerated into garbage text was a stuck `paseo create` heredoc, not a real blocker.
Everything listed as "DONE" below is live on disk, uncommitted, and compiles clean
(`cargo check -p op-plugins -p op-network` = pass, only pre-existing cosmetic warnings).

## Hard architecture rules (do NOT violate)
1. **busctl/zcall only, never raw `ovs-vsctl`/`ovs-ofctl`/shell subprocess for control-plane
   state.** This was violated and corrected multiple times last session — the user has said
   this many times. `rovs_commands` (OVSDB) and `openflow` (flows, as of this session) are the
   only two D-Bus doors for network state.
2. **No container gets a NIC or IP.** All container I/O is UDS. `op-ghostbridge-mux` exists
   specifically to keep this true while still letting governed connections reach a TCP target.
3. **PluginSchema is the single source of truth** — D-Bus methods, MCP tools, and (as of this
   session) generated Rust types via `op-schema-codegen --subid-subject` all derive from it.
   Every field needs a valid `x-oscal-subid` (see subid-taxonomy.md) — don't hand-wave this.
4. **Reactive, not polled.** No watch loops. The mux daemon's flow-sync (next task) reacts to
   `unix_socket` registrations, it doesn't poll them on a timer.

## DONE this session (verified, don't redo)
- **OpenFlow moved onto the D-Bus control plane**, closing the last raw-CLI exception:
  - Vendored the real OpenFlow JSON Schema into `schemas/openflow/` (`openflow15.json` +
    `definitions.json/yaml`, extended with OF1.5 `encap`/`decap`/`packet_type`).
  - `crates/op-plugins/src/state_plugins/openflow_generated.rs` — schema-generated Rust types
    (structs/enums for `Action`, `OfpFlowMod`, `OfpMatch`, `OxmField`, etc.), wired into
    `openflow.rs`'s `openflow_schema()` and `call_method`'s `add_flow`/`delete_flow`/`modify_flow`
    handlers. Blob resealed.
  - `crates/op-network/src/bin/op-of-controller.rs` now implements a real
    `org.opdbus.rovs.openflow` D-Bus interface backing those calls (was hand-rolled flow logic
    before). New s6 services: `deploy/s6/op-of-controller-srv/`, `-log/`.
  - `control-plane-network/bin/control-plane-network`'s flow-install step now calls
    `busctl -> openflow.add_flow` instead of `ovs-ofctl add-flow`. **Not yet live-tested
    end-to-end on the real box** — verify with a real `rovs_call`/`busctl` round-trip before
    trusting it at boot.
  - All other OVS state in `control-plane-network` (`ensure_bridge`, `enslave_eth0`/deslave,
    `netmaker_up`'s port-add) already goes through `rovs_commands` via busctl (`rovs_call()`
    helper near the top of the script) — this was fixed in an earlier session, still holds.

- **`op-ghostbridge-mux`** (`crates/op-network/src/bin/op-ghostbridge-mux.rs`) — new daemon
  replacing the ad-hoc per-container socat loopback bridges (the pattern
  `deploy/s6/netmaker-api-loopback/`, `qdrant-grpc-loopback` used):
  - Accepts on a shared internal UDS, reads `SO_PEERCRED` to get the peer UID, looks up the
    `unix_socket` plugin's registration whose `uid_base <= peer_uid < uid_base + 65536`.
  - Two modes per registration (`unix_socket.rs:89`, `Registration.mode`): `control` (bare
    relay) and `governed` (`proxy_governed()` at `op-ghostbridge-mux.rs:163` — binds
    `bind_address`, connects `target`, relays; TCP-mode governed connections run inside
    `relay_netns` via `ip netns exec` so outbound traffic originates from the isolated netns,
    not the host's own network namespace).
  - `crates/op-grpc-bridge/src/bin/op-grpc-bridge.rs` updated to listen on the mux's internal
    socket instead of `container.sock` directly.
  - `unix_socket` plugin (`crates/op-plugins/src/state_plugins/unix_socket.rs`) got
    `register`/`unregister`/`list_registrations` `CallMethod` handlers (~line 325, 612) so the
    mux can look up registrations without touching SHM/files directly — reads through D-Bus.
  - New s6 service: `deploy/s6/op-ghostbridge-mux-srv/`, `-log/`.
  - Schema (`unix_socket.rs`) has the mux fields already: `mode`, `uid_base`, `bind_address`,
    `target`, `relay_netns` (default via `default_relay_netns()`).

- **`op-schema-codegen`** (`crates/op-schema-codegen/`) — new standalone crate, captures the
  ad-hoc `cargo expand`-based codegen the factory session used to build `openflow_generated.rs`
  as a real, reusable, `--help`-documented CLI:
  - Wraps `schemafy_lib::Expander` directly (no macro/build.rs/cargo-expand dance).
  - JSON and YAML input (`--root <TypeName>`, `--output <path>`, `--no-format`).
  - Fixed two real `schemafy_lib` limitations discovered by running it against real upstream
    specs (Incus's `doc/rest-api.yaml`, Netmaker's `swagger.yaml`) rather than just the vendored
    OpenFlow schema: (1) it panics on non-string enum values (draft-4 permits them,
    `schemafy_lib` doesn't handle them without a synthetic `enumNames` sibling — now
    auto-synthesized); (2) it silently emits duplicate/conflicting top-level items when two
    definitions collapse to the same PascalCase identifier — now deduped with a stderr warning.
  - **New**: `--subid-subject <slug> [--subid-component-type <type>]` mechanically injects
    `#[derive(schemars::JsonSchema)]` and a taxonomy-conformant `x-oscal-subid` on every
    generated type (`sch.<component-type>.<subject>.<type>.schema@v1`) and field
    (`obs.<component-type>.<subject>.describe.<type>-<field>@v1`), validated against a local
    mirror of `op_blob::subid::validate_subid` before being emitted — a malformed subid aborts
    generation rather than shipping bad compliance metadata. Verified compiling clean with real
    `schemars` against the OpenFlow schema and the full upstream Incus REST spec.
  - **Known remaining gap, not fixed**: a third, unrelated `schemafy_lib` bug surfaced while
    testing against Netmaker's real `swagger.yaml` — a generated type (`SchemaSeverity`) ends up
    without its `Serialize`/`Deserialize` derives, ~48 downstream build errors. Suspected cause:
    self-referential/circular `$ref` elision (`schemafy_lib::lib.rs:747`, "Skip self-referential
    types") producing an incomplete definition. Not investigated further — flagged, not blocking
    anything since nothing currently depends on generating Netmaker's types.

## DONE this session (continuation) — first cut, not fully live-tested
- **`sync_governed_flows`** in `control-plane-network/bin/control-plane-network`:
  - `unix_socket_call` + `busctl_json` helpers (busctl → ProjectedObject CallMethod).
  - Stage `sync-governed-flows` after bridge ports: list_registrations → for each
    mode=governed with bind_address+ports, `openflow.add_flow` table 0 priority 200:
    match eth_type=0x0800, ip_proto=6, nw_src, tcp_src; actions set_field reg0=uid_base +
    NORMAL; cookie `6d7578%08x(uid_base)`.
  - Reactive first cut = boot-once (no poll). Live register/unregister reconcile still TODO.
- **Atomic multi-port attach at boot (shell owns interface names):**
  - `rovs_commands.add_ports` (generic: `{bridge_name, ports:[{port_name,interface_type}]}`)
    — one OVSDB transaction; **no host interface names in Rust**.
  - control-plane-network stage `enslave-bridge-ports` after `netmaker-mesh`:
    one busctl `add_ports` for `BRIDGE_SYSTEM_PORTS` (default `$UPLINK_NIC $NETMAKER_IFACE`
    from network.conf). Then uplink ip -batch + gateway verify.
  - `netmaker_flows` runs after attach (OF1.5 protocols + L3 encap flows).
- **op-of-controller** encoder: OXM nw_src/nw_dst/tcp_*/udp_*/ip_proto; set_field regN via
  NXM_1; load_register action.
- **SIGNALS.md** mux + sync_governed_flows entry (2026-07-15 | grok-4.5).
- **Deploy note:** live `/etc/s6/sv/op-projection/run` still missing `OVSDB_SOCK` export
  (repo has it). Without that, list_bridges can see empty OVSDB. Install repo run script +
  restart op-projection before trusting boot enslavement. Rebuild projection_server after
  `add_ports` lands so the method exists on the bus.

## NOT done — pick up here
1. **Restore ovsbr0 safely (console/noVNC only for eth0 attach).** All container I/O is
   UDS — no container NICs — so Incus never attaches anything to ovsbr0 and will not
   populate OVSDB. `ensure_bridge` now creates via `rovs_commands.create_bridge` if
   missing (idempotent). Then boot path:
   `ensure_bridge` → `netmaker_up` (iface only) → `enslave_bridge_ports` (one busctl
   `add_ports` for `BRIDGE_SYSTEM_PORTS`) → `netmaker_flows` → `sync_governed_flows`.
   **Do NOT live-test eth0 attach over SSH alone** — 2026-07-15 incident: bare
   `create_bridge`+`add_ports` without preflight blackholed the host until ovsbr0 was
   deleted from noVNC.
2. **One-shot busctl form (names from config only):**
   ```sh
   busctl --system call org.opdbus.v1.plugins \
     /org/opdbus/v1/plugins/rovs_commands \
     org.opdbus.v1.plugins.ProjectedObject CallMethod ss add_ports \
     '{"bridge_name":"ovsbr0","ports":[{"port_name":"eth0","interface_type":"system"},{"port_name":"netmaker","interface_type":"system"}]}'
   ```
   Requires: projection with `OVSDB_SOCK` set (live run.user fixed this session),
   `projection_server` built with `add_ports`, bridge already exists (Incus), gateway
   ping OK before attach, uplink `ip -batch` applied before/after.
3. **Live-test** OpenFlow D-Bus + sync_governed_flows only after #1 is green.
4. **Live-reconcile** for sync_governed_flows on register/unregister (no poll).
5. **Remove socat loopbacks** only after mux + flows proven live.
5. **DONE this session**: vendored `incus_generated.rs` — `schemas/incus/rest-api.yaml` (real
   upstream Incus REST spec) → `crates/op-plugins/src/state_plugins/incus_generated.rs` (212
   types, full OSCAL subid coverage via `--subid-subject incus`), wired in via
   `#[path = "incus_generated.rs"] mod incus_generated; pub use incus_generated::*;` in
   `incus.rs` (same pattern as `openflow_generated.rs`). Verified: no name collisions with
   `incus.rs`'s own hand-authored types, `cargo check -p op-plugins` passes clean. Along the way,
   fixed a real gap in `op-schema-codegen` itself: generated files had no `use
   serde::{Deserialize, Serialize};` header, so anything using `#[path=...] mod` (which doesn't
   inherit imports from the parent file) failed with "cannot find derive macro" — now emitted
   automatically in every generated file's header.
   `IncusInstance`/`CreateInstanceInput`/etc. in `incus.rs` were deliberately left untouched
   (hand-tailored to this project's D-Bus method contracts, already have their own subid tags;
   a wholesale swap would break method wiring and lose compliance coverage).
6. Explicitly **not** in scope, per user decision this session: replacing `zeroclaw.rs` /
   `antigravity.rs` with generated types. Those aren't 1:1 wrappers of an external API with an
   upstream schema to codegen from — they're this repo's own hand-authored, subid-annotated
   routing/config plugins. Don't revisit this without a real upstream schema to point at.

## Reference: real upstream schemas already fetched (scratchpad, not in repo)
Fetched during this session's op-schema-codegen testing, not checked in:
`/tmp/claude-1000/.../scratchpad/incus-rest-api.yaml` (github.com/lxc/incus doc/rest-api.yaml)
and `netmaker-swagger.yaml` (github.com/gravitl/netmaker swagger.yaml). Re-fetch if the
scratchpad's been cleaned — it's session-scoped, not durable.
