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
- [x] **OVSDB Event-Driven:**
    - [x] Implement `monitor` method in `OvsdbClient` to subscribe to RFC 7047 update notifications.
    - [x] Add OVSDB update listener to `DbusMirror` to replace periodic reconciliation.
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

### [TODO] Projection-First State Control
- [ ] **Architectural vocabulary reset:**
    - [ ] Replace stale `desired state` / drift-correction language in active design docs with projection-first authority language.
    - [ ] Define `current authoritative state`, `constructed state`, `revision`, `snapshot`, `branch transition`, and `promote to current`.
    - [ ] Document explicitly that the live `org.opdbus.v1` tree is owned by `DbusMirror`, not by `DbusProjection`.
- [ ] **Legacy path inventory:**
    - [ ] Identify every path still using `org.opdbus.StateManager` and classify it as legacy, transitional, or still required.
    - [ ] Reconcile `TODO.md`, `docs/operations/dbus-projection-object-map.md`, and older architecture docs so they describe the same control model.
    - [ ] Update docs that still imply `StateManager` is the primary runtime authority.
- [ ] **Constructed-state branch control:**
    - [ ] Define the control object for whole-branch operations.
    - [ ] Pick naming for the control object:
        `ConstructedStateController`, `BranchRevisionController`, or `StateComposer`.
    - [ ] Define branch-scoped operations for:
        snapshot, list revisions, load revision, compose branch state, preview diff, commit/promote.
    - [ ] Support whole-subtree operations for business/logistical scopes such as departments, document classes, and plugin-owned object families.
- [ ] **Branch selector model:**
    - [ ] Define how branch selection works:
        path prefix, plugin, object type, tags, and business grouping keys.
    - [ ] Define mixed-branch selection rules when a branch spans NonNet-backed and OVSDB-backed objects.
    - [ ] Define whether selectors operate on D-Bus path hierarchy only or also on schema metadata tags.
- [ ] **Revision / snapshot backing model:**
    - [ ] Decide where branch revisions live:
        `EventChain`, `StreamingBlockchain`, DB-backed snapshots, or hybrid.
    - [ ] Define how a prior authoritative branch state is reconstructed.
    - [ ] Define whether revisions are full subtree snapshots, deltas, or composable fragments.
- [ ] **Commit model into the live projection tree:**
    - [ ] Define how a constructed branch state becomes the new current authoritative state.
    - [ ] Define backend-specific commit paths for NonNet-backed branches.
    - [ ] Define backend-specific commit paths for OVSDB-backed branches.
    - [ ] Define atomicity and ordering rules when a constructed branch spans multiple authoritative backends.
- [ ] **Preview and audit semantics:**
    - [ ] Add dry-run/preview for branch transitions.
    - [ ] Show adds, removals, property mutations, and incompatible merges before commit.
    - [ ] Record who constructed the branch state, which revisions were used, and which subtree was promoted.
    - [ ] Record resulting event hashes / commit references in the audit layer.
- [ ] **UI / operator workflow for branch control:**
    - [ ] Add UI flow for:
        select branch, browse revisions, compose candidate, preview diff, commit.
    - [ ] Make branch rollback / promotion a first-class operation in `op-web`.
    - [ ] Ensure branch-level actions are represented in a schema-driven way rather than as one-off hardcoded screens.
- [ ] **Triggers beyond manual clicks:**
    - [ ] Support workflow/orchestration-triggered branch transitions.
    - [ ] Support scheduled transitions and reactive rollback triggers.
    - [ ] Support policy-driven branch promotion/rollback actions.
- [ ] **Implementation path:**
    - [ ] Prototype branch control for a NonNet-only subtree first.
    - [ ] Add tests for whole-branch rollback, partial branch composition, preview-only mode, and commit audit trail.
    - [ ] After NonNet works, extend the model to mixed NonNet + OVSDB branches.

---

### [BACKLOG] Long-term
- [ ] Migration of legacy shell scripts to native Rust tools.
- [ ] Hardened security policy for D-Bus object access (polkit integration).
- [ ] Multi-node state synchronization via SyncEngine clusters.
