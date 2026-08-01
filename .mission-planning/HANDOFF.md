# Mission Planning Handoff — D-Bus Hypervisor / rovs Refactor

> **For the next Droid session:** Re-invoke the `mission-planning` and `define-mission-skills` skills first, then read this file. You are the **orchestrator** (plan/design only, no direct implementation).

## How to resume
Open a fresh session in `/home/jeremy/git/operation-dbus-proto` and say:
"continue the hypervisor/rovs mission planning". Then read this file and everything in `/home/jeremy/git/operation-dbus-proto/rovs-refactor/`.

## Why this handoff exists
The previous session's Task/subagent spawner failed persistently with "Premature close". Root cause: that session was a stale CLI process (telemetry reported v0.140.1 while the on-disk binary is v0.141.0). The BYOM machine itself was successfully reconnected (see "Infra state" below), but the stale session could not spawn subagents. A fresh session should restore subagent spawning — **verify this first** by launching a trivial Task subagent before proceeding.

## Infra state (already fixed — do not redo unless broken)
- Machine registered as Droid Computer **"tched"** (`b8549cdc-7ae0-46d8-a37d-0b267c46ce03`). Previous red dot was caused by stale local config pointing at a deleted computer id; cleared via `droid computer register -y`.
- s6 service `droid-daemon` runs `droid daemon --remote-access` (loads `FACTORY_API_KEY` from `/home/jeremy/.bash_secrets`). Restart with `sudo s6-svc -r /run/s6-rc/servicedirs/droid-daemon`.
- `relay.factory.ai` reachable (HTTP 404 on root is normal — not a web server).

## Mission scope (LOCKED with user)
Build greenfield to the user's pasted "Changelog: D-Bus Hypervisor, Netmaker, & Xray Architecture" end-state:
1. Native Rust D-Bus **OpenVSwitch hypervisor daemon** (`op-openvswitch-daemon`) that proxies `rovs` crate primitives over D-Bus. **No CLI wrappers** — the daemon IS a literal D-Bus proxy over the rovs crate API.
2. Replace **all** `ovs-*` CLI subprocess wrappers with native calls.
3. Consolidate **OVS + Xray + Netmaker netclient** management into the hypervisor.
4. Workspace-wide **subprocess-bypass sweep** (align with AGENTS.md "D-Bus first, D-Bus only" rule).
5. Create a **`rovs_commands`** schema state-plugin (base/schema source of truth for OVS commands).

### Locked design decisions
- **Two separate D-Bus object paths** (NOT one multiplexed object):
  - `/org/opdbus/rovs/jsonrpc` — JSON-RPC is used system-wide (UI / command-sending), gets its own path.
  - `/org/opdbus/rovs/openflow` — OpenFlow path.
- Interface name in the rovs JSON specs: `org.op_dbus.CrateInterface`.
- Delete `OvsdbClient` (`crates/op-network/src/ovsdb.rs`, ~1000 lines) and the daemon multiplexer; introduce `RovsJsonRpcProxy` / `RovsOpenFlowProxy`.
- Refactor `openflow.rs` to native RFC 7047 (OVSDB) where applicable; add privacy sockets.

### Intended D-Bus method surface (from rovs-refactor/*.json)
- **jsonrpc** (`rovs-jsonrpc.json`): New, Next_id, Transact, Notify, Send_message, Recv_message, notification polling.
- **openflow** (`rovs-openflow.json`): Connect, Version, Send_flow, Send_flow_sync, Echo, Barrier, Dump_flows, Dump_flows_filtered, Recv_packet_in, Monitor_flows, Recv_flow_updates, Send_packet_out.

## Critical reality check
The `rovs-refactor/` planning docs CLAIM the daemon/proxies/plugin are "Implemented" — **this is false**. Confirmed greenfield: no `op-openvswitch-daemon` / `rovs` crate exists on disk. Only external `rovs-*` v0.2 crates are used as dependencies (wrapped today by `op-network`). Plan accordingly.

## Key files / references
- Planning docs + schemas: `/home/jeremy/git/operation-dbus-proto/rovs-refactor/` (`implementation_plan.md`, `stepped_plan.md`, `walkthrough.md`, `schema_comparison.md`, `rovs-jsonrpc.json`, `rovs-openflow.json`, `transcript.jsonl` [4.2MB Antigravity transcript — NOT yet read]).
- Today-modified bootstrap scripts (central to consolidation): `deploy/setup-hypervisor-{controller,netclient,xray}.sh`.
- Code slated for refactor/deletion: `crates/op-network/src/{ovsdb.rs,controller.rs,openflow.rs,plugin.rs}`; `crates/op-plugins/src/state_plugins/{openflow.rs,net.rs,ovsdb_bridge.rs,lxc.rs,full_system.rs}`.
- Repo architecture rules: root `AGENTS.md` (schema = single source of truth in `crates/op-plugins/src/state_plugins/plugin_schema_defs.rs`; D-Bus-only control plane; OSCAL subid taxonomy; MCP gateway = cognitive-mcp on :3003).

## Validation surface (per user)
Validate via: D-Bus introspection (op-inspector), gRPC bridge, cognitive MCP. End-to-end is the default posture.

## Next steps (resume here)
1. **Verify subagent spawning works** in the fresh session (trivial Task call).
2. Launch the prepared investigation subagent to produce `/home/jeremy/git/operation-dbus-proto/.mission-planning/refactor-surface.md` with these H2 sections: rovs Crate API Surface; OvsdbClient Consumers (deletion blast radius); Plugin Refactor Surface; Subprocess Inventory (workspace-wide `Command::new`, `ovs-vsctl`/`ovs-ofctl`/`ovsdb-client`/`ovs-appctl`/`ip`/`systemctl`/`s6-svc`); Xray + Netmaker/Netclient Consolidation; Open Questions/Risks.
3. (Optional) Subagent to read/summarize `rovs-refactor/transcript.jsonl` (Antigravity planning transcript) + extract any additional rovs API details.
4. Synthesize architecture design → present to user for confirmation.
5. Infrastructure, boundaries, credentials, validation strategy, **milestones** (get explicit user agreement on milestone count).
6. Mission readiness checks (dependency + validation subagents).
7. Create mission proposal via `propose_mission`.
