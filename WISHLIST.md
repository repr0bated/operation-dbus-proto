# 👻 GHOSTBRIDGE LIVE! — Working Wishlist / Task Board

**Ghostbridge Live!** is the name of the whole effort: taking the architecture from
deployed-but-dark to *running live* — registration → Qdrant/Gemma → accountability loop →
routing → chatbot → demo. This board is the single source of truth for all of it.
Replaces scattered TODO fragments across `.zenflow/tasks/`, `.kiro/specs/`, and notebook snapshots.

## How to use this board
- **Priority buckets:** Critical/Urgent → Current → Next → Future → When I Have Staff.
- **Status:** `TODO` · `WIP` · `BLOCKED` · `DONE`.
- **Agent:** who's assigned — an `op-agents` role (`devops`, `policy_enforcer`, `schema_as_code`)
  or a Claude Code subagent (`Explore`, `Plan`, `general-purpose`, `claude`).
- **Dispatch:** tell Claude *"dispatch <ID> to <agent>"* — it spawns that agent with the task
  context and updates Status here when done.
- **Add work:** append a row under the right bucket with a fresh `OD-##` id.

---

## 🔴 Critical / Urgent
*On fire or blocking the live system.*

| ID | Task | Agent | Status | Notes |
|----|------|-------|--------|-------|
| OD-DEMO | **Get a demo out** — minimal end-to-end path that shows the system working | `general-purpose` | TODO | top priority; scope/deadline TBD — see subtasks once defined |
| OD-05 | Dynamic xray cutover: wire `run_schema_shuttle` → live (sled → /dev/shm config → D-Bus start) | `general-purpose` | BLOCKED | needs `/etc/ghostbridge/xray.env` secrets + op-xray-daemon running as service |
| OD-08 | netmaker server down: resolve broker token timeout (stay on decoy `129.153.134.63` vs relocate) | `devops` | BLOCKED | egress works; server relocation decision open |
| OD-34 | **OIA1 has a validator and no issuer — no human can authenticate at all.** `op-decoy-issuer` is installed at `/usr/local/bin/op-decoy-issuer` (6.7 MB, Sep 1) but has **no source in this tree** and **no runit service anywhere**. It must run on the Oracle decoy (the only WG terminator), not on this host. Until it runs, `x-oracle-identity-assertion-bin` is never minted, so every human login path is dead and the only working credential is the chatbot's SID1. | `devops` | TODO | **Blocks all human identity.** Done: a human WG peer connecting to the decoy receives a signed OIA1 (Ed25519, <=900s, nonce) and `op-grpc-bridge` validates it end-to-end. Recover or rewrite the source first — a binary we cannot rebuild is itself a stub. |
| OD-35 | **Arm the fail-closed identity guardrail — it is currently inert.** `SessionIdentity.principal_kind` exists and gates the implicit fallback (`may_resolve_implicitly`), but **nothing writes it**: all 4 live sleds are unlabelled, so the pairing/service-principal refusal never fires. Add the field to the `identity_sled` plugin schema + `write_identity`, persist in Cozo, project to SHM, then label `bea37ecb-92be-197c-660f-09e806f1a34f` (chatbot) as `service` and the three human sleds as `human`. | `schema_as_code` | TODO | Schema change -> blob reseal. Also resolve the name collision: `SealedId.principal_kind` is always `wireguard-principal` (credential kind) and is a *different* axis from the sled's actor class — never seal the sled value into SID1 or `mcp_frontend` will reject the envelope. Done: `resolve_identity_session(None)` resolves a human sled and refuses the chatbot. |
| OD-36 | **Rotate the exposed chatbot keypair (G-5).** `/var/lib/opdbus-runtime/identities/chatbot/private.key` + `mcp_token` were printed into a session transcript 2026-09-04; treat as compromised. New keypair -> new blake3 `session_id`/`principal_id` -> `OP_MCP_IDENTITY_*` in `/etc/runit/sv/op-grpc-bridge/run` -> incus `user.opdbus.*` -> `capability-grants.json` -> identity dir. | `devops` | TODO | Operator action (needs console + `sv restart op-grpc-bridge`). Done: pubkey `VaRh9EUieQxA3zIoOj3qNiNIqZoPGpqztPU4muyF1zM=` is no longer accepted anywhere. |
| OD-37 | **No run script sets `OP_IDENTITY_SESSION_ID`, so every service silently loses its identity.** `configured_identity_session()` is called by op-grpc-bridge, op-cognitive-mcp, op-llm, op-gallery-gen and op-assistant-grpc; with no selector it falls through to the implicit path and (with 4 anchored sleds) already errors `Ambiguous`. Each service must name its principal explicitly. | `devops` | TODO | Done: every `/etc/runit/sv/*/run` that hosts a `configured_identity_session()` caller exports `OP_IDENTITY_SESSION_ID`, and no caller depends on implicit resolution. Pairs with OD-35. |
| OD-38 | **Delete the retired Argon2(PSK) session-id path.** `op_identity::session::derive_session_id_from_psk` (`session.rs:29`) is still reachable via an explicit `psk` arg to `write_identity`. All live sleds are blake3-derived; this is a second, repudiated derivation still compiled in. | `general-purpose` | TODO | Done: the function and its `write_identity` argument are gone, and no call site remains. |
| OD-39 | **Kill the legacy `/api/chat` orchestrator path — it runs tools as nobody.** op-web's orchestrator executes `op_tools` with op-web's ambient bus credentials, a random UUIDv4 session and a free-text `user_id`: no principal, no `actor_id`, no audit trail. | `general-purpose` | TODO | Done: the path is removed (or refuses without a validated principal). Chat runs as the human via `ChatService.Send`. |
| OD-40 | **Prove the chatbot cannot reach direct execution (G-3).** No test asserts the negative today. The chatbot principal must be denied any `tools/call` outside HOT union its selected set, and any `shell_*`/`file_*`/raw `dbus_call` regardless of selection (FR-14.6). | `policy_enforcer` | TODO | Done: a failing-closed test exists and passes against `mcp-audience-policy.json` + `mcp-toolsets.json`. |
| OD-41 | **Reasoning episodes carry no identity (G-4).** `ctl_plane_chatbot` episodes must record `principal_id`, `session_id`, `session_genesis` and `on_behalf_of` so an episode can name whose conversation it came from and who acted. | `schema_as_code` | TODO | Done: episode schema carries the four fields and they are populated on every write. |


## ▶ Current
*Actively in flight.*

| ID | Task | Agent | Status | Notes |
|----|------|-------|--------|-------|
| OD-20 | Consolidate opdbus NotebookLM cluster → `Operation Dbus (Master)` (curate, dedup) | `claude` | WIP | master `01b00e9c…`; ~10 notebooks, heavy overlap |
| OD-23 | Knowledge pipeline: designated notebooks → semantic (Voyage→Qdrant) + learning graph (nodes/edges) | `general-purpose` | TODO | corpus in `knowledge/notebooks.manifest.json`; graph store tbd (Qdrant payload vs dedicated) |
| OD-10 | Repoint `op-mcp-shim` endpoint for laptop (xray door, not 10.200) + TLS channel | `general-purpose` | TODO | laptop is a WG peer, can't reach 10.200 |
| OD-09 | Restore A.N.N.A./OSCAL role cast in narrative + interceptor docs (authorizing-official = real-time approve) | `claude` | TODO | notebook `Identity-State Arbitrator` |
| OD-24 | **Nail down registration + accountability loop** — reconcile provision script (CozoDB+Bearer+HTTP) with runtime (sled+header+gRPC); wire Netmaker peer reg | `general-purpose` | TODO | template: `deploy/scripts/provision-workspace-subscriber.sh`; see SIGNALS concerns |
| OD-32 | **Blob-first projection** — blobs sealed in SHM but unread at runtime; single writer to `/dev/shm/opdbus/plugin-blobs`; delete op-blob narrow `PluginSchema`, seal canonical `op_state_store::PluginSchema`; op-projection + reflection read ONLY from blob catalog (not `live-schema.json`); atomic manifest + tmp+rename + stale sweep | `general-purpose` | TODO | Fable 5 audit in SIGNALS.md (2026-07-03); synthesis: `docs/fable-handoff-synthesis.md`; factory: `FACTORY-PROMPT-op-blob-unification.md` |

## ⏭ Next
*Queued; start when Current clears.*

| ID | Task | Agent | Status | Notes |
|----|------|-------|--------|-------|
| OD-06 | Gemma as single routing brain: subid classification + OpenFlow tags + subdomain resolution | `general-purpose` | TODO | design recorded, not built |
| OD-07 | Owned-domain DNS split-horizon: `*.ghostbridge.tech` carve-out → internal targets | `devops` | BLOCKED | needs Gemma map (OD-06) |
| OD-01 | OVSDB event-driven: `monitor` (RFC 7047) in `OvsdbClient` + listener in `DbusMirror` | `devops` | TODO | replaces periodic reconciliation |
| OD-02 | Enterprise event-driven: `inotify`/`SQLITE_UPDATE_HOOK` on `state.db` → re-projection | `devops` | TODO | |
| OD-03 | SyncEngine: route all `op-web` tool exec through `ApplyContractMutation` + audit log | `policy_enforcer` | TODO | every mutation = enforcement point |

## 🔮 Future
*Wanted, not urgent.*

| ID | Task | Agent | Status | Notes |
|----|------|-------|--------|-------|
| OD-33 | **User-configurable MCP tool-set preferences** — let a principal pin a default warm set / preferred ordering on top of the HOT five, so a CLI opens on the tools that person actually uses | `general-purpose` | TODO | Builds on the shipped projection in `crates/op-grpc-bridge/src/mcp_policy.rs` + `/etc/opdbus/mcp-toolsets.json` (gen 3). **Hard constraint from `.kiro/specs/standalone-emqx-identity-mcp/design.md` §9–10: a preference may only NARROW, never grant.** Visible = exact grants ∩ audience ∩ selected set ∩ provider health; a preference is one more ∩ term, never a ∪. Also per §10.2 promotion/demotion between HOT/WARM/COLD stays a reviewed manifest change with a catalog generation bump — never automatic frequency-based mutation, so "preferences" must not silently re-tier tools. Preference storage should be principal-keyed server-side (same shape as the audience policy), not client-asserted, since `clientInfo.name` is not authentication. |
| OD-22 | Fold op-xray-daemon into mirror-projected plugin (currently standalone owning `opdbus.v1`) | `general-purpose` | TODO | path fixed; deeper fold deferred |
| OD-21 | Fill Inception narrative `TODO(jeremy)` blocks (background, dates, lightbulb moments) | `claude` | TODO | `docs/inception-narrative-plan.md` |

## 👥 When I Have Staff
*Needs more hands / parallel effort than solo allows.*

| ID | Task | Agent | Status | Notes |
|----|------|-------|--------|-------|
| OD-04 | Schema-driven D-Bus UI: typed view-models, dynamic inspector panes, json-render eval | `schema_as_code` | TODO | big front-end surface |
| OD-30 | Lovable UI polish: React hooks → gRPC status endpoints, real-time D-Bus→SSE updates | — | TODO | |
| OD-31 | OSCAL/compliance agent build-out (8 agents) + memory/knowledge/schema_renderer plugins | — | TODO | enterprise/EU regulatory target |

---

## ✅ Done

| ID | Task | Agent | Notes |
|----|------|-------|-------|
| OD-00 | xray D-Bus path violation → `/org/opdbus/v1/plugins/xray` (daemon + caller + literal), rebuilt+deployed | `general-purpose` | 2026-06-12 |
| OD-00b | DNS host repoint → NextDNS (`127.0.0.1`); netmaker egress verified + persisted | `devops` | 2026-06-12 |
