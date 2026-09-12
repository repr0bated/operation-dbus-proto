# Mission Readiness Validation Report

**Date:** 2026-06-04  
**Workspace:** /home/jeremy/git/operation-dbus-proto  
**Branch:** main

---

## Toolchain Availability

| Tool | Version | Status |
|------|---------|--------|
| cargo | 1.95.0 (f2d3ce0bd 2026-03-21) Artix Linux | ✅ OK |
| rustc | 1.95.0 (59807616e 2026-04-14) Artix Linux | ✅ OK |
| clippy | 0.1.95 | ✅ OK |
| rustfmt | 1.9.0 | ✅ OK |
| node | v26.2.0 | ✅ OK |
| npm | 11.14.1 | ✅ OK |

**Patch overrides in Cargo.toml:** zbus, zbus_xml, zbus_macros, zbus_names, zvariant, zvariant_derive, zvariant_utils are patched to `/home/jeremy/git/zbus/` — these must remain accessible for builds.

---

## fmt Baseline

**Command:** `cargo fmt --all -- --check`  
**Exit code:** 1 (FAIL)  
**Duration:** ~30s

### Summary

- **1 internal rustfmt error:** trailing whitespace in `crates/op-agents/src/agents/orchestration/memory.rs:195:98`
  - This blocks rustfmt from even processing that file fully; rustfmt emits "left behind trailing whitespace" as an internal error and skips further formatting.
- **172 diff locations** across **32 unique files** in 3 crates:

| Crate | Unformatted files |
|-------|-------------------|
| op-cognitive-mcp | 7 files (examples/context_aware_client.rs, examples/external_client.rs, src/client_config.rs, src/context_awareness.rs, src/context_server.rs, src/interceptor.rs, src/lib.rs) |
| op-plugins | 23 files (src/canonical.rs, src/default_registry.rs, src/schema_loader.rs, + 20 state_plugins/*.rs) |
| op-web | 4 files (handlers/schema.rs, handlers/zeroclaw.rs, projection_client.rs, routes/mod.rs) |

**Pre-mission fix required:** Run `cargo fmt --all` before any milestone validation; otherwise gate 3 will always fail. The 32 files are heavily concentrated in the WIP crates the mission touches (op-cognitive-mcp, op-plugins, op-web).

---

## clippy / Build Baseline

### cargo check --workspace (time-boxed 120s)

**Exit code:** 0 ✅  
**Duration:** 1m 54s (incremental; cold build would be longer)

**Result:** Compiles cleanly with **0 errors**. Warnings present:

- **op-web** lib: 24 warnings (unused imports, unused variables, unused functions). Mostly in `handlers/`, `orchestrator/`. `cargo fix --lib -p op-web` would auto-resolve 15 of them.
- **redis v0.25.4**: future-incompat warning (upstream, not actionable in this workspace).

### Existing clippy artifacts (from May 19)

Old batch artifacts exist in the repo root:
- `clippy_batch1.json` (680 lines, 37 warnings, 0 errors)
- `clippy_batch2.json` (718 lines, 0 warnings, 2 errors)
- `clippy_batch3.json` (817 lines, 66 warnings, 0 errors)
- `clippy_op-mcp-aggregator.json` (21 errors in op-tools — from an older code state)
- `clippy_op-mcp-proxy.json` (0 errors)
- `clippy.log`, `clippy_final.json`, `clippy_err.log`, `clippy_error.log` — all **0 bytes** (empty)

**Note:** op-mcp-aggregator and op-mcp-proxy are not current workspace members; the errors were from a prior workspace layout. The current `cargo check` shows 0 errors.

### clippy gate prognosis

The `cargo check` succeeded, but `cargo clippy --workspace --all-targets --all-features -- -D warnings` will **FAIL** because:
1. The 24 warnings in op-web become errors under `-D warnings`.
2. Any clippy-specific lints not visible in `cargo check` will also surface as errors.

**Pre-mission fix required:** Either fix the op-web warnings or temporarily allow warnings in those modules. A full `cargo clippy --workspace --all-targets --all-features -- -D warnings` was not run (would take >5 min) but the known baseline is "warnings present → clippy gate fails."

---

## cognitive-MCP Reachability

### Service Status

| Check | Result |
|-------|--------|
| Process running? | ✅ PID 24314 (`/usr/local/bin/op-cognitive-mcp --db /var/lib/op-dbus/cognitive.db`), supervised by s6 (PID 331) |
| Listening on :3003? | ✅ `LISTEN 0 4096 100.90.37.254:3003 0.0.0.0:*` |
| IP 100.90.37.254 on interface? | ✅ `inet 100.90.37.254/32 scope global netmaker` |
| Ping 100.90.37.254? | ✅ 0% loss, RTT 0.075ms |

### HTTP Endpoint Probes

| Endpoint | HTTP Status | Notes |
|----------|-------------|-------|
| `GET /health` | **200** | `{"status":"ok","service":"op-mcp","version":"0.4.0"}` |
| `GET /healthz` | 401 | Requires ghostbridge auth |
| `GET /status` | 401 | Requires ghostbridge auth |
| `GET /mcp` | 401 | Requires ghostbridge auth |
| `GET /tools` | 401 | Requires ghostbridge auth |
| `GET /context/status/test` | 401 | Requires ghostbridge auth |

### Concrete Validation Command

The mission can validate cognitive-mCP with:

```bash
curl -sf http://100.90.37.254:3003/health | jq -e '.status == "ok"'
```

This requires no auth header and confirms the service is alive and reporting healthy. For deeper validation (tools, session, memory), a `X-Ghostbridge-Footprint` header is required; that is environment-specific and not suitable for an automated gate unless the header value can be injected from the WireGuard identity.

### Client Configuration

`deploy/config/cognitive-mcp-clients.json` documents:
- **cognitive_mcp** endpoint: `http://100.90.37.254:3003` (HTTP/SSE/JSON-RPC, ghostbridge auth)
- **compact_mcp** endpoint: `http://127.0.0.1:11436` (loopback only, never external)
- **grpc_cognitive** endpoint: `http://100.90.37.254:50052` (gRPC with reflection)

---

## Workspace Crate Inventory

33 workspace members from root `Cargo.toml`:

| # | Crate | Refactor-touches? |
|---|-------|-------------------|
| 1 | op-services | |
| 2 | op-gateway | |
| 3 | op-core | |
| 4 | op-tools | ✅ |
| 5 | op-introspection | |
| 6 | op-chat | ✅ |
| 7 | op-http | |
| 8 | op-web | ✅ |
| 9 | op-cache | |
| 10 | op-state | |
| 11 | op-state-store | |
| 12 | op-jsonrpc | ✅ |
| 13 | op-llm | |
| 14 | op-network | ✅ |
| 15 | op-inspector | |
| 16 | op-agents | ✅ |
| 17 | op-plugins | ✅ |
| 18 | op-workflows | |
| 19 | op-ml | |
| 20 | op-snowball | |
| 21 | op-deployment | |
| 22 | op-mcp | |
| 23 | op-mcp-aggregator | |
| 24 | op-identity | |
| 25 | op-execution-tracker | |
| 26 | op-dynamic-loader | |
| 27 | op-cognitive-mcp | |
| 28 | op-cozo-store | |
| 29 | op-dbus-model | |
| 30 | op-grpc-bridge | ✅ |
| 31 | op-dbus-mirror | ✅ |
| 32 | op-compliance | |
| 33 | op-projection | |
| 34 | op-assistant-grpc | |

Plus the root `op-dbus` package (depends on many of the above).

---

## Validation Risks / Gaps

| ID | Category | Description | Severity | Mitigation |
|----|----------|-------------|----------|------------|
| V1 | **BLOCKER** | `cargo fmt --all -- --check` FAILS on 32 files (172 diff locations + 1 internal error in op-agents). Gate 3 cannot pass without `cargo fmt --all` first. | BLOCKER | Run `cargo fmt --all` as a pre-mission step. |
| V2 | **BLOCKER** | `cargo clippy --workspace --all-targets --all-features -- -D warnings` will FAIL because op-web has 24 warnings that become errors under `-D warnings`. | BLOCKER | Fix op-web warnings (mostly unused imports/vars) or scope clippy gate to affected crates only. |
| V3 | RISK | Full workspace build time is ~2 min incremental, but could be 10+ min from clean. The clippy gate is especially slow (all targets + all features). | RISK | Consider running clippy only on changed crates per milestone, or accept the time cost. |
| V4 | RISK | `zbus` patched to local `/home/jeremy/git/zbus/` — if that directory is missing or stale, builds break. | RISK | Verify `/home/jeremy/git/zbus/` exists and is up-to-date before mission start. |
| V5 | OK | cognitive-mcp `/health` endpoint is reachable and returns 200. The validation command `curl -sf http://100.90.37.254:3003/health | jq -e '.status == "ok"'` works today. | OK | — |
| V6 | RISK | Deep cognitive-mcp validation (tools, sessions) requires `X-Ghostbridge-Footprint` auth which is environment-specific and not easily automated. | RISK | Use `/health` as the gate; defer deeper validation to manual testing. |
| V7 | RISK | `redis v0.25.4` has a future-incompat warning. Not a build error today, but will become one in a future Rust edition. | RISK | Monitor; not blocking for this mission. |
| V8 | OK | `cargo check --workspace` passes with 0 errors. | OK | — |

---

## Summary

1. **Toolchain:** Rust 1.95.0, clippy 0.1.95, rustfmt 1.9.0, Node 26.2.0 — all present and functional.
2. **fmt:** FAILS — 32 files need formatting, 1 internal rustfmt error in `op-agents/memory.rs`. **BLOCKER for gate 3.**
3. **clippy/build:** `cargo check` passes (0 errors), but clippy with `-D warnings` will fail due to 24 warnings in op-web. **BLOCKER for gate 2.**
4. **cognitive-mcp:** Service is running, `/health` returns 200, IP reachable. Gate 4 is viable via `curl -sf http://100.90.37.254:3003/health`.
5. **Build time:** ~2 min incremental; clippy gate is the slowest.
6. **zbus local patch:** Dependency on `/home/jeremy/git/zbus/` must remain intact.
7. **Two BLOCKERS must be resolved before milestones can pass gates:** fmt and clippy baselines are dirty.
8. **33 workspace crates** in scope; 8 are directly touched by the refactor.
