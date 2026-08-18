# PR #30 Code Review — gemma-4 via gnoppix

**Date**: 2026-08-18
**PR**: [#30 — Port OpenCode tched_router migration onto main](https://github.com/repr0bated/operation-dbus-proto/pull/30)
**Model**: `google/gemma-4-26b-a4b-it:free` via gnoppix OpenAI-compatible proxy
**Method**: 5 parallel workers, each sent a diff group with project invariants and review methodology as system prompt. Pass 2 validation of all 12 candidates against the actual codebase.

## Files

| File | Description |
|---|---|
| `run_review.py` | Driver script: builds per-group payloads, calls gnoppix API with retry, saves responses |
| `g{1-5}.payload.json` | Full API request payload (system + user message with diff) for each group |
| `g{1-5}.response.json` | Raw API response JSON for each group |
| `g{1-5}.findings.txt` | Extracted model output (findings JSON) for each group |
| `REVIEW.md` | This file — validated summary |

## Groups

| Group | Focus | Diff bytes |
|---|---|---|
| g1 | tched_router plugin core (rename, vendor stubs, registry, config surface) | 69,097 |
| g2 | PluginSchema capability model + plugin-wide capability sweep | 39,762 |
| g3 | op-grpc-bridge and LLM consumers of the renamed plugin | 21,594 |
| g4 | op-web HTTP surface + frontend contract (LlmPage.tsx) | 17,472 |
| g5 | Network/OVS plugins + deploy scripts + merge resolution | 34,433 |

## Verdict

**No blocking (P0) issues found.** 3 minor findings survive validation. 9 candidates rejected as false positives or out of scope.

## Surviving findings

### [P2] `factory.rs` — `source` field says `tched_router` even when reading from the `zeroclaw` fallback path

**File**: `crates/op-plugins/src/state_plugins/factory.rs:113`

The BYOM projection hardcodes `"source": "tched_router"` unconditionally, but the fallback path reads `/dev/shm/plugin-zeroclaw.json` when the tched_router SHM file doesn't exist (pre-reseal host). Downstream consumers that use `source` for routing will see `tched_router` for data that actually came from the legacy `zeroclaw` projection. During the migration window this is a data inconsistency.

```rust
// The fallback reads zeroclaw data but labels it tched_router:
let projection_path = if std::path::Path::new("/dev/shm/plugin-tched_router.json").exists() {
    "/dev/shm/plugin-tched_router.json"
} else {
    "/dev/shm/plugin-zeroclaw.json"  // <-- reads zeroclaw
};
// ...
"source": "tched_router",  // <-- but says tched_router
```

**Fix**: Set `source` based on which path was actually read, or derive it from the blob catalog lookup that already resolved the plugin id.

### [P3] `factory.rs` — TOCTOU between `exists()` and `read()` on the SHM projection path

**File**: `crates/op-plugins/src/state_plugins/factory.rs:65-70`

The PR introduces a `Path::new(...).exists()` check before `std::fs::read()` on the projection path. The old code read a hardcoded path directly. The race window is tiny (SHM files in `/dev/shm` don't typically vanish between two syscalls), and the consequence is a silent `None` return (BYOM discovery fails for that cycle), not a crash. But it's a new pattern the PR introduces.

### [P3] `chat_service.rs` / `mutation_engine.rs` — stale "ZeroClaw" strings in error messages

**Files**: `crates/op-grpc-bridge/src/chat_service.rs:108`, `crates/op-grpc-bridge/src/mutation_engine.rs:1303`

Error messages in the gRPC dispatch path still say "not declared by ZeroClaw" and "invalid zeroclaw.Chat arguments". The PR's description says "Historical surfaces left as-is: proto ZeroclawService, HTTP /zeroclaw/*" — so legacy naming in the proto/HTTP surface is intentional. But these are runtime error strings in the dispatch path, not API surface names. An operator debugging a failed `tched_router.Chat` call will see "zeroclaw" in the error and look at the wrong plugin. Low impact, but worth a follow-up cleanup.

## Rejected candidates (9)

| Candidate | Verdict | Reason |
|---|---|---|
| G1: dispatch.inc direct file I/O (P0) | **Rejected** | The file I/O reads/writes the tched_router config file — that IS the plugin's job (it's a config-management plugin). The "no direct file reads for live state" invariant targets SHM/D-Bus state, not a plugin's own external config. The `std::fs::write` at `tched_router.rs:2254` is inside `#[cfg(test)]`. |
| G1: `/root/.zeroclaw` fallback (P1) | **Rejected** | `dirs::home_dir().unwrap_or_else(\|\| PathBuf::from("/root"))` is a standard Rust pattern; env vars (`TCHED_ROUTER_CONFIG_PATH`/`ZEROCLAW_CONFIG_PATH`) are checked first. Not a security issue. |
| G2: factory.rs TOCTOU (P1) | **Downgraded to P3** | See above — real but minimal impact. |
| G4: llm.rs inconsistent active/legacy keys (P1) | **Rejected** | `llm_status_handler` assigns the same final `provider`/`model` variables to both `active_*` and legacy `*` fields — no internal inconsistency within a single response. The SHM-override behavior is pre-existing (not introduced by this PR). |
| G4: zeroclaw.rs panic on SHM read (P2) | **Rejected** | The code uses `ok()?` chains throughout, not unwraps. Failure returns `None`, which the handler converts to a 503. No panic path. |
| G5: CRD patch systemctl subprocess (P0) | **Rejected** | The patch is for a desktop utility (Chrome Remote Desktop), not the D-Bus control plane. The patch's purpose IS to add a non-systemd fallback (`_start_portals_directly` via Popen). The `systemctl` check just detects whether systemd is available so it can skip it. This is correct behavior for a runit host. |
| G5: CRD patch Popen (P1) | **Rejected** | Same — desktop utility launching `xdg-desktop-portal` (a D-Bus-activated service), not control plane code. The invariant applies to plugin/service Rust code, not Python patches for third-party desktop tools. |
| G3: chat_service.rs error message (P2) | **Downgraded to P3** | See above — intentional legacy surface per PR description, but worth tracking. |
| G3: schema_passthrough.rs docstring (P2) | **Rejected** | Docstring, not executable code. Not a bug. |

## Summary

The PR is a well-scoped rename + capability-model addition. The merge resolution was clean (verified: all migration artifacts intact at the merge tip, tests pass). The main risk is the migration transition window where `tched_router` and `zeroclaw` coexist — the `source` field inconsistency in `factory.rs` is the only place where the fallback logic produces incorrect metadata. Everything else is either by design (legacy surface retention) or too low-impact to block.
