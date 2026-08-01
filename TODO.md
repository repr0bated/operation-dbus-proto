# OP-DBUS Project Roadmap & TODO

## Status Snapshot: 2026-02-12
**Core Goal:** Unified, high-performance D-Bus backbone (org.opdbus.v1) with gRPC mutation pipeline and React UI.

---

### [DONE] Foundation & Unification
- [x] Unify D-Bus namespace under `org.opdbus.v1`.
- [x] Rename interfaces to `StateManagerV1` and `ProjectedObjectV1`.
- [x] Implement 1:1 Mirror for OVSDB, NonNet, and Enterprise state.
- [x] Remove legacy Systemd plugin logic (Transition to native dinit/direct interfaces).
- [x] Restore Lovable UI source to `lovable/` and `crates/op-web/ui/`.
- [x] Wire `op-web` to use `RemoteOperationClient` (gRPC) for status dashboard.

---

### [IN PROGRESS] Event-Driven Architecture
- [x] **NonNet Event-Driven:** Implemented `broadcast` channel in `NonNetDb` and listener in `DbusMirror`.
- [ ] **OVSDB Event-Driven:**
    - [ ] Implement `monitor` method in `OvsdbClient` to subscribe to RFC 7047 update notifications.
    - [ ] Add OVSDB update listener to `DbusMirror` to replace periodic reconciliation.
- [ ] **Enterprise Event-Driven:**
    - [ ] Implement `inotify` or `SQLITE_UPDATE_HOOK` for `state.db` updates.
    - [ ] Trigger selective re-projection on database changes.

---

### [TODO] op-web & UI Integration
- [ ] **SyncEngine Hardening:** 
    - [ ] Ensure all tool executions via `op-web` flow through `ApplyContractMutation`.
    - [ ] Add audit logging for all gRPC-triggered state changes.
- [ ] **UI Polish (Lovable):**
    - [ ] Connect React hooks to the new gRPC status endpoints.
    - [ ] Implement real-time updates via D-Bus signals projected to SSE.
- [ ] **Schema-Driven D-Bus UI Rendering:**
    - [ ] Evaluate `json-render` as the dynamic renderer for D-Bus object views in the dashboard.
    - [ ] Keep navigation, search, selection, and virtualization in deterministic React code.
    - [ ] Define typed view-model schemas for D-Bus object detail, interface panels, tree node summaries, agent status, and orchestration views.
    - [ ] Render inspector/detail panes dynamically from route, selection, and task/use-case state instead of hand-building per-object UI variants.
    - [ ] Validate performance and UX for high-cardinality D-Bus object sets before widening usage beyond detail panes.
- [ ] **D-Bus Signals:**
    - [ ] Emit D-Bus signals from `MirrorObject` when properties change.
    - [ ] Bridge D-Bus signals to gRPC streaming for UI "hotwire" updates.
- [ ] **Schema-Driven Mutation Routing:**
    - [ ] Extend plugin and network schemas with per-field ownership metadata (`dbus`, `ovsdb`, other authoritative backend).
    - [ ] Extend schemas with per-field mutability metadata (`read_only`, `create_only`, `mutable`).
    - [ ] Validate D-Bus object creation and update paths against schema mutability and ownership rules.
    - [ ] Route writes from schema metadata, preferring D-Bus for D-Bus-owned fields and OVSDB JSON-RPC only for OVS-owned fields.
    - [ ] Generate or enforce writable versus read-only behavior from schema and introspection instead of ad hoc command choice.

---

### [BACKLOG] Long-term
- [ ] Migration of legacy shell scripts to native Rust tools.
- [ ] Hardened security policy for D-Bus object access (polkit integration).
- [ ] Multi-node state synchronization via SyncEngine clusters.

---

## Status Snapshot: 2026-05-29

### [DONE] Schema Single Source of Truth
- [x] Factory discovered schema was not in SHM and had multiple differing sources
- [x] Fixed: all components now read schema from registered SHM source — single ground truth

---

### [FUTURE] Tiered CoW Object Store + CozoDB as Primary Store
Future architecture — endgame, not current sprint. Factory velocity may bring this closer than expected.

- DBus object tree → Btrfs CoW tiered store (replaces state.db)
- Services registry → flat TOML on Btrfs (replaces services.db)
- DBus tree + services queryable via CozoDB graph engine
- SQLite eliminated entirely
- [ ] **Btrfs lower layer** — NVMe, subvolumes per tenant/namespace, `metacopy=off` for overlayfs compat
- [ ] **overlayfs upper = tmpfs/SHM** — hot working set, COW promotion on first write
- [ ] **Eviction daemon** — flush stale upper-layer objects back to lower (overlayfs has no LRU)
- [ ] **Boot prerequisite** — schema registration to SHM before any service starts; hard fail if absent
- [ ] **`btrfs send` replication** — incremental snapshot deltas as peer sync primitive
- [ ] **Snapshot-before-delete** — required for GDPR erasure audit trail

---

### [DESIGN] Schema-Based Tag Routing + Compliance
Three orthogonal tag classes, routing rules in TOML, enforced on every mutation:

**Tag classes:**
- [ ] OSCAL tags — NIST vocabulary (impact levels, information types, control baselines); operator read-only; ingestor accepts signed OSCAL docs only; tag store append-only
- [ ] 3tched tags — internal workflow routing (tier-hint, dbus-scope, replication-zone, queue, assigned-agent); operator-writable
- [ ] UI tags — presentation only (display-name, icon, pinned, color-group); user-writable; router discards entirely

**Enforcement:**
- [ ] Tag router as single enforcement point — pre-mutation (BLOCK / COERCE / PERMIT), post-mutation audit record
- [ ] Compliance rules NEVER reference 3tched or UI tags — enforced at rule parse time
- [ ] Cascading mutation closure — evaluate full object graph before any CoW lands (atomic Btrfs tx + tag router tx)
- [ ] Rule conflict resolution — `override: true` compliance rules always win (strictest constraint)
- [ ] OSCAL schema update path — Btrfs snapshot → re-evaluate all objects → mutation ledger records schema version delta

**Sample compliance rules to implement:**
- [ ] `gdpr-residency` — route eu-tagged objects to eu-west subvolume, deny non-EU replication
- [ ] `gdpr-pii` — force cold tier, credentialed-only DBus subscribers, erasure audit on delete
- [ ] `nis2-critical` — 60s snapshot interval, 7-year audit retention, SOC alert on change
- [ ] `soc2-confidential` — cold tier, encrypt at rest, immutable access log

---

### [TODO] OVS Tool Layer — Migrate CLI stubs to rovs/native

Discovered: `op-chat/src/tool_loader.rs` inline OVS tool stubs all call `ovs-vsctl`/`ovs-ofctl` via subprocess.
`op-network::OvsdbClient` (rovs-ovsdb backed) already exists and is correct — tools just need to use it.

- [ ] Replace `OvsListBridgesTool` — use `op_network::OvsdbClient` instead of `ovs-vsctl list-br`
- [ ] Replace `OvsListPortsTool` — use `op_network::OvsdbClient` instead of `ovs-vsctl list-ports`
- [ ] Replace `OvsShowBridgeTool` — native OVSDB query
- [ ] Replace `OvsDumpFlowsTool` — use rovs-openflow instead of `ovs-ofctl dump-flows`
- [ ] Register write tools: `OvsAddBridgeTool`, `OvsDelBridgeTool`, `OvsAddPortTool`, `OvsDelPortTool` — wired to `op_network::OvsdbClient`
- [ ] Remove `systemd_*` tools from registration — this host uses s6, they do nothing useful
- [ ] Add s6 service management tools (`s6_restart`, `s6_status`, `s6_start`, `s6_stop`)

---

### [BACKLOG] Infrastructure
- [ ] Fix `ovsbr0-static` `shell_up` — references `grpc-bridge` but interface is `grpc-uplink`
- [ ] Qdrant container setup — previous attempt failed; needs Debian trixie image
- [ ] Re-vectorize 97 repos lost in Btrfs failure (OSCAL + compliance repos)
- [ ] NVIDIA Inception application
- [ ] axon-trace-ui — connect to live DBus object tree via tag-routed signal feed
