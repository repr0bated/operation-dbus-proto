# Factory task: make the op-dbus projected D-Bus tree deterministic (start with zbusctl)

Use agent orchestration. Goal: eliminate hardcoded D-Bus addressing so the projected
tree is the single source of truth, then enforce it with checks (a convention already
rotted once). Start with the concrete zbusctl bug, then the four determinism conditions.

## Repos
- Backplane / plugins / projection: `/home/jeremy/git/operation-dbus-proto` (Rust workspace)
- Client CLI (separate repo): `/home/jeremy/git/zbusctl`

## Principle (the architectural law)
The system exposes a self-describing, introspectable **projected D-Bus tree** rooted at the
well-known name `org.opdbus.v1.plugins`, plus a **gated mutation path** (MutationEngine via
`PluginService.CallMethod`, enforced by the Ghostbridge interceptor: requires
`x-ghostbridge-footprint` + `x-ghostbridge-trace-id`). Consumers must **DISCOVER** addresses
from the tree (introspection). They must **NOT hardcode** object paths, interfaces, or version
suffixes. Hardcoding only the single stable root anchor is allowed; everything below it is
discovered. A hardcoded leaf is a frozen snapshot that silently diverges on rename.

## The concrete bug (proximate, start here)
`zbusctl createsocket` (`/home/jeremy/git/zbusctl/src/main.rs`, ~lines 163-165) targets names
that no longer exist:
- service `org.opdbus.CognitiveMcp`  · object `/org/opdbus/v1/cognitive` · interface `org.opdbus.CognitiveMcpV1`  ← ALL WRONG

The real served names (source of truth:
`operation-dbus-proto/crates/op-cognitive-mcp/src/dbus_interface.rs` lines 3-5 and 27):
- service `org.opdbus.v1.plugins` · object `/org/opdbus/v1/plugins/cognitive_mcp` · interface `org.opdbus.v1.plugins.CognitiveMcp`

Result: `zbusctl createsocket` fails `ServiceUnknown` (system bus) / `BrokenPipe` (session bus).
It predates the `.v1.plugins` rename and rotted silently because the path was never exercised
(the `unix_socket` plugin projection shows `{"sockets":[]}` — `createunixsocket` has never run
for any container).

## Primary task — rewrite `zbusctl createsocket`
1. Hardcode ONLY the stable root anchor (`org.opdbus.v1.plugins`); DISCOVER the object path,
   interface, and method by introspecting the projected tree.
2. Route socket creation through the GENERIC mutation surface —
   `PluginService.CallMethod{plugin_id="unix_socket", method="createunixsocket"}` (the single
   gated MutationEngine authority) — NOT a bespoke `cognitive` object/interface. Rename-proof
   and plugin-agnostic.
3. **CORRECTED PREMISE (recon result):** the gated mutation path is **gRPC, not D-Bus.** D-Bus
   `org.opdbus.v1.plugins` is **discovery/read-only** (ProjectedObject props + `updated` signal;
   no CallMethod). The mutation is `PluginService.CallMethod` (operation.proto) →
   `MutationEngine.mutate(MethodCall)` → `unix_socket/createunixsocket`, mounted by
   `build_operation_routes` on op-dbus `:50051` and the zeroclaw bridge `:8090` +
   `/run/ghostbridge/container.sock`, all behind `ghostbridge_interceptor`. Also: the *current*
   zbusctl createsocket routes through `CallTool("cognitive_memory", …)` — it stores a memory
   record and never reaches the socket plugin. So this is a **full reimplementation.**

   **DECIDED resolution (dual-transport):**
   - zbusctl = D-Bus for **discovery**, gRPC for the **mutation**. Add `tonic` + reuse
     op-grpc-bridge's generated `PluginService` client + `op-identity`. Do NOT add a D-Bus
     CallMethod proxy (that's a second ungated mutation door — Zero-Trust hole).
   - Endpoint: local backplane gRPC over UDS `/run/ghostbridge/container.sock` (fall back `:50051`),
     from config/discovery — no hardcoded service/object/interface leaf.
   - Auth: inject `x-ghostbridge-footprint` (hex of live sled `hashed_footprint`) +
     `x-ghostbridge-trace-id`, read via `op-identity::read_sled`. Host-root admin tool
     self-presenting the live footprint is the intended model (local equivalent of xraqy).

Backend reference: `crates/op-plugins/src/state_plugins/unix_socket.rs::create_unix_socket(name, ports)`
binds the shared host socket `SHARED_CONTAINER_SOCKET = /run/ghostbridge/container.sock` and
registers `name` as the demux tag. Validation target: provision netmaker =
`--name netmaker --path /run/ghostbridge/container.sock --port 8081,1883`.

## Broader goal — 4 conditions for a DETERMINISTIC projected tree (SIGNALS OD-31)
Each was observed broken this session. Make each true AND enforced by a check:
1. ONE producer derived from one source.
2. Reliably refreshed to present-state ON READ. (Observed stale: projection reported
   `qdrant: unavailable / dimensions 1536` and `netmaker: Stopped` lagging reality.)
3. Each well-known name UNAMBIGUOUSLY OWNED. (Observed: `org.opdbus.v1.plugins` owned by
   `projection_serv` on the SYSTEM bus AND `ovs-dbus-init` on the SESSION bus.)
4. EVERY consumer DISCOVERS from the tree — zero hardcoded leaves. (zbusctl is one; audit ALL.)

## Acceptance criteria (CHECKS, not conventions)
- A CI/lint or Rust integration test that FAILS if any backplane consumer hardcodes a D-Bus
  object path, interface, or version suffix. Only the root well-known name
  `org.opdbus.v1.plugins` may appear as a literal.
- A check that each well-known name has exactly one owner per bus.
- `zbusctl createsocket --name netmaker --path /run/ghostbridge/container.sock --port 8081,1883`
  succeeds end-to-end (when the backend is up) via discovery + CallMethod, and the `unix_socket`
  plugin projection then lists `netmaker`.
- No regression to the Ghostbridge-gated mutation path.

## Suggested sub-agent orchestration
- **Agent A — zbusctl rewrite**: discover-from-tree + CallMethod; delete hardcoded leaves;
  `cargo build` + `cargo clippy` clean.
- **Agent B — consumer audit + the check**: grep both repos for hardcoded `org.opdbus.*` object
  paths / interfaces / version suffixes; list them; fix or file follow-ups; author the
  lint/test that enforces "no hardcoded leaves".
- **Agent C — name ownership**: why `org.opdbus.v1.plugins` is double-owned (system vs session
  bus), and why `op-cognitive-mcp` isn't connected to the session bus / doesn't claim its name;
  implement single-owner resolution.
- **Agent D — projection freshness**: make knowledge/incus projections reflect present-state on
  read (the stale qdrant/netmaker statuses); reconcile the reactive refresh.
- **Verifier**: run the checks; confirm createsocket end-to-end; open SIGNALS follow-ups for
  anything deferred.

## Conventions / guardrails
- Single source of truth; no hardcoding. Same class as recent fixes: inventory self-registration
  replaced a hardcoded plugin dispatch table; one shared gRPC route-builder so reflection matches
  the served surface (SIGNALS OD-29).
- Do NOT break the gated mutation path (Ghostbridge interceptor headers).
- Keep commits scoped; follow each repo's commit conventions.
- Note: knowledge plugin config was just corrected to `dimensions=1024 / provider=voyage /
  model=voyage-4` in `crates/op-plugins/src/state_plugins/knowledge_plugin.rs` (needs
  `projection_serv` redeploy to show live) — don't revert it.
- Append observations/decisions to `operation-dbus-proto/SIGNALS.md` (OD-31 is the tracking row).

## Skills / droids / MCP to load
LOAD (directly serve this task):
- skill **`simplify`** — "review changed code for reuse, quality, efficiency, then fix" — this task IS dedup/de-hardcode; use on every diff.
- skill **`review`** — code review for the verifier.
- droid-control skill **`verify`** — run the acceptance checks.
- droid **`worker`** — the parallel implementer sub-agents (Agents A/B/C/D).
- MCP **`cognitive-grpc`** — REQUIRED: introspect the live projected tree + actually call
  `createunixsocket` end-to-end against the backplane (don't reason about names — discover them).
- MCP **`sequential-thinking`** — orchestration planning. MCP **`memory`** — shared context across agents.

RUN AS A MISSION (so the validation droids auto-engage):
- **`scrutiny-feature-reviewer`** and **`user-testing-flow-validator`** are "used only within missions"
  — they map exactly to the verifier/acceptance step. Running this as a Factory *mission* (not a
  plain task) turns the acceptance criteria above into enforced mission validation.

SKIP (irrelevant here): skill `init`, `session-navigation`; droid-control TUI/desktop skills
(`agent-browser`, `capture`, `compose`, `desktop-control`, `pty-capture`, `showcase`, `true-input`,
`tuistory`); MCP `hugging-face`.
