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

## ▶ Current
*Actively in flight.*

| ID | Task | Agent | Status | Notes |
|----|------|-------|--------|-------|
| OD-20 | Consolidate opdbus NotebookLM cluster → `Operation Dbus (Master)` (curate, dedup) | `claude` | WIP | master `01b00e9c…`; ~10 notebooks, heavy overlap |
| OD-23 | Knowledge pipeline: designated notebooks → semantic (Voyage→Qdrant) + learning graph (nodes/edges) | `general-purpose` | TODO | corpus in `knowledge/notebooks.manifest.json`; graph store tbd (Qdrant payload vs dedicated) |
| OD-10 | Repoint `op-mcp-shim` endpoint for laptop (xray door, not 10.200) + TLS channel | `general-purpose` | TODO | laptop is a WG peer, can't reach 10.200 |
| OD-09 | Restore A.N.N.A./OSCAL role cast in narrative + interceptor docs (authorizing-official = real-time approve) | `claude` | TODO | notebook `Identity-State Arbitrator` |
| OD-24 | **Nail down registration + accountability loop** — reconcile provision script (CozoDB+Bearer+HTTP) with runtime (sled+header+gRPC); wire Netmaker peer reg | `general-purpose` | TODO | template: `deploy/scripts/provision-workspace-subscriber.sh`; see SIGNALS concerns |

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
