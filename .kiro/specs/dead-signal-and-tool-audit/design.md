# Design — signal-and-tool-audit

## What Actually Changed Functionally (Post-Deployment Audit)

`.kiro/specs/remove-projection-static-tree/` shipped live (commit `39bde466`, deployed
2026-07-29). This audit investigated functional impact by reading code after deployment.

---

## State Reading Path — Before vs. After

### Before
```
op-web route handler
  → projection_client.rs (5s polling loop)
  → D-Bus call to org.opdbus.ProjectedObjectV1 on SYSTEM bus
  → op-projection daemon (reads shm, reformats, re-serves)
```

### After
```
op-web route handler
  → state_tree.rs (direct fs read, no loop)
  → /dev/shm/opdbus/state/{plugin}/{key}
  → (shm file, written by mutation_engine)
```

**Functional change**: eliminated polling latency (5s → microseconds). Reads shm directly
instead of through a D-Bus-reformatting daemon. This works.

---

## Reactivity Path — Unchanged (Already Correct)

The D-Bus projection removal did NOT touch the already-working reactive path:

```
mutation_engine.rs:600
  → self.change_tx.send(change)
  → grpc_server.rs:881 ← subscribe() reads change_tx broadcast
  → op-web state.rs:321 ← gRPC stream feeds into sse_broadcaster
  → GET /api/events ← browser gets SSE stream
```

This path was **already live** before this spec (confirmed in SIGNALS.md 2026-07-22 entry,
"Subscribed to gRPC state updates"). The spec's deletions did not break it; it remains
untouched and working.

**Functional impact of the spec on this path**: zero. It was correct before, still correct.

---

## New `Updated` Signal — Built but Unused

The spec added `org.opdbus.v1.PluginV1::Updated(data_json)` signal on the session bus
(emitted from mutation_engine.rs:159-188, fires at lines 277, 571, 1253 after shm writes).

**What calls it**: nothing. Workspace-wide search confirms zero receivers.

**Why it exists**: per CLAUDE.md, "D-Bus is the only control plane." A D-Bus-native
consumer (operator with `dbus-monitor`, a future tool that only speaks D-Bus, not gRPC)
needs to know when state changes without polling. The signal serves that consumer.

**Functional impact**: zero on current system (no such consumer exists). The signal is
correctly built, correctly emitted, correctly ignored. Not a bug — it's infrastructure
for a consumer class that doesn't exist yet. A real D-Bus subscriber WOULD work correctly
if one existed (the payload carries state data; no polling required).

---

## op-tools Projection Discovery — Dead Branch

`projection_engine.rs:157-166` contains a branch:
```rust
if iface.name == "org.opdbus.ProjectedObjectV1" {
    let tool = PluginProjectionTool::new_generic(&service, path.clone());
    registry.register(tool, ...);
}
```

`ProjectionEngine` still runs at startup (`op-web/src/state.rs:638`, `:654`). It walks
D-Bus introspection results looking for interfaces to expose as tools. This branch
matches on `org.opdbus.ProjectedObjectV1`, which **no longer exists on any bus**
(op-projection is deleted).

**What happens**: the branch evaluates, never matches, registers zero tools. No error,
no log noise, no breakage — just a silent no-op.

**Functional impact**: zero. The discovery pipeline continues to work for all OTHER
interfaces; this one branch simply never fires.

**Related dead code**: `builtin/plugin_projection.rs` (209 lines) — the `PluginProjectionTool`
that would be constructed by the branch above, plus `register_plugin_projection_tools()`.
These have zero real callers (confirmed by workspace search; only doc comments reference
them). This entire file would never execute.

---

## Blockchain & Voyage Vectors — Separate, Live

`blockchain_plugin.rs` instantiates `StreamingBlockchain` and is actively running.
Voyage vectors flow end-to-end (confirmed by user). This is independent of the
projection removal and requires no changes.

The Btrfs seed volume path (`/var/lib/opdbus/snapshots/latest`) referenced in the prior
spec's Task 0.4 is a separate concern from both the state-tree reactivity and the
blockchain/voyage pipeline.

---

## Summary — What's Functionally True Today

| Component | Status | Impact |
|---|---|---|
| State reads (`state_tree.rs`) | **Working** | Correctly replaces polling client |
| Reactivity (`change_tx` → gRPC → SSE) | **Working** | Unchanged, was already correct |
| `Updated` signal | **Working** (unused) | Correct for D-Bus-native consumers; none exist yet |
| `ProjectionEngine` discovery | **Working** (dead branch) | Branch never matches; discovery continues for other interfaces |
| `PluginProjectionTool` code | **Dead** (zero callers) | No functional impact; never instantiated |
| Blockchain & vectors | **Working** | Independent of this spec; continues normally |

---

## Open Questions (Not in Scope of This Audit)

1. **Should the `Updated` signal have a real subscriber?** (No — using it would duplicate
   the already-working gRPC reactivity path. It's correct as infrastructure waiting for a
   D-Bus-native consumer.)

2. **Should the dead `ProjectionEngine` branch and `PluginProjectionTool` code be removed?**
   (Tentatively yes, but only if no other tool-discovery mechanism depends on them. Requires
   verification of MCP tool exposure in `compact_mcp.rs` and `cognitive_mcp.rs` first.)

3. **What is `/var/lib/opdbus/snapshots/latest` and when is it written?** (Out of scope for
   this audit. Related to blockchain/voyage, not state-tree reactivity.)

---

## Conclusion

The projection removal deployed correctly. The functional change is real and positive
(eliminated polling). The new signal is correctly built but correctly unused (no consumer
exists for D-Bus-native observers yet). The dead code in op-tools is harmless (silent
no-op). Everything else continues to work.
