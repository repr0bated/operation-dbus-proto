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
- [ ] **D-Bus Signals:**
    - [ ] Emit D-Bus signals from `MirrorObject` when properties change.
    - [ ] Bridge D-Bus signals to gRPC streaming for UI "hotwire" updates.

---

### [BACKLOG] Long-term
- [ ] Migration of legacy shell scripts to native Rust tools.
- [ ] Hardened security policy for D-Bus object access (polkit integration).
- [ ] Multi-node state synchronization via SyncEngine clusters.
