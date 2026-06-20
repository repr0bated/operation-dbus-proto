# Spec — zeroclaw-host-axum-schema-kiro

## Purpose
Deliver an Axum-backed gRPC / gRPC-Web host in `crates/op-grpc-bridge` that
surfaces the zeroclaw plugin schema to external consumers.  This spec is the
binding contract between requirements, design decisions, and implementation
tasks.

---

## Architectural Constraints (binding)

| # | Constraint | Source |
|---|-----------|--------|
| C-01 | `zeroclaw_schema()` in `plugin_schema_defs.rs` is the only schema definition. | AGENTS.md §2, FR-01 |
| C-02 | The Axum host reads schema from a plugin-owned file; it never generates or writes schema. | FR-02 |
| C-03 | Btrfs `@zeroclaw` subvolume is install/cache/rollback only; no D-Bus, projection, or OSCAL artefacts there. | FR-03 |
| C-04 | Native gRPC on Unix socket `/run/opdbus/zeroclaw-grpc.sock`. | FR-04 |
| C-05 | gRPC-Web + HTTP/1.1 on TCP via `tonic_web::enable`. | FR-05 |
| C-06 | D-Bus object `/org/opdbus/v1/services/zeroclaw_axum_host` is the config authority. | FR-06, AGENTS.md §4 |
| C-07 | All `subid` props registered in `op-plugins`; none defined in `op-grpc-bridge`. | FR-07, AGENTS.md §4a |
| C-08 | `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID` stamped on every response. | FR-07, AGENTS.md §2 |
| C-09 | No Python; shell scripts only for deploy. | FR-08, AGENTS.md §4 |
| C-10 | No new `src/` at workspace root; all code under `crates/`. | AGENTS.md §3 |

---

## Interface Contract

### gRPC service: `zeroclaw.ZeroclawService`

| RPC | Request | Response | Stream |
|-----|---------|----------|--------|
| `GetSchema` | `GetSchemaRequest {}` | `SchemaResponse { schema_json, trace_id, footprint }` | unary |
| `WatchSchema` | `WatchSchemaRequest {}` | `SchemaEvent { schema_json, event_type }` | server-stream |

### D-Bus object: `/org/opdbus/v1/services/zeroclaw_axum_host`

| Property | Type | Access | Description |
|----------|------|--------|-------------|
| `BindAddr` | `String` | rw | TCP bind address (default `0.0.0.0:8090`) |
| `SchemaPath` | `String` | rw | Path to plugin-owned schema JSON |
| `ReloadIntervalSecs` | `u32` | rw | Periodic reload interval (0 = off) |
| `HealthStatus` | `String` | ro | `"ok"` or last error message |

### Schema file
- Path: `/dev/shm/opdbus/schemas/zeroclaw.json`
- Format: serialised `PluginSchema` JSON (canonical from `plugin_schema_defs.rs`)
- Written by: zeroclaw plugin
- Read by: `op-grpc-bridge` SchemaLoader

---

## Accepted Trade-offs

| Trade-off | Rationale |
|-----------|-----------|
| Axum host panics at startup if schema file absent | Plugin must start first; a soft fail would hide misconfigured s6 dependency. |
| Single `Arc<RwLock<…>>` for schema | Schema reloads are rare; contention is negligible. `DashMap` not needed. |
| No TLS on Unix socket | Loopback / same-host; TLS adds no security value on a UDS. |
| TCP endpoint unauthenticated by default | OpenClaw Trusted Proxy handles auth upstream; the Axum host is not the auth boundary. |

---

## Verification Criteria

| ID | Criterion | How to check |
|----|-----------|-------------|
| V-01 | `cargo clippy --workspace -D warnings` passes | CI |
| V-02 | `cargo fmt --all -- --check` passes | CI |
| V-03 | `cargo test --workspace --all-features` passes | CI |
| V-04 | `GetSchema` returns valid `PluginSchema` JSON matching zeroclaw plugin's schema | integration test |
| V-05 | `WatchSchema` emits `event_type: "reload"` within 1 s of `SIGHUP` | integration test |
| V-06 | D-Bus property `HealthStatus` returns `"ok"` after successful bind | manual / integration |
| V-07 | Response headers contain `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID` | integration test |
| V-08 | gRPC-Web call from a browser-like HTTP/1.1 client succeeds | integration test |
| V-09 | Schema JSON on disk matches `zeroclaw_schema()` serialisation | unit test in `op-plugins` |
