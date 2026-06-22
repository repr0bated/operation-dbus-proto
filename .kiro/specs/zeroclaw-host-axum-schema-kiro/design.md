# Design — zeroclaw-host-axum-schema-kiro

## 1. Crate Placement

```
crates/
  op-plugins/            ← schema source of truth lives here
    src/state_plugins/
      plugin_schema_defs.rs   ← zeroclaw_schema() defined here
      zeroclaw.rs             ← calls Some(super::plugin_schema_defs::zeroclaw_schema())
  op-grpc-bridge/        ← new Axum host lives here
    src/
      lib.rs
      server.rs          ← Axum router + tonic-web setup
      schema_loader.rs   ← reads /dev/shm/opdbus/schemas/zeroclaw.json
      dbus_object.rs     ← registers /org/opdbus/v1/services/zeroclaw_axum_host
      grpc/
        zeroclaw.proto   ← generated from plugin schema; DO NOT hand-edit structs
        zeroclaw.rs      ← tonic generated (via build.rs)
      tracing.rs         ← X-Ghostbridge-Footprint injection middleware
```

No `src/` at the workspace root. No schema definitions duplicated outside
`plugin_schema_defs.rs`.

---

## 2. Data-Flow: The Sled → Schema File → Axum Host

```
 ┌─────────────────────┐
 │  SchemaEngine        │  (op-plugins, /dev/shm — The Sled)
 │  zeroclaw plugin     │
 │  schema() → JSON     │
 └────────┬────────────┘
          │ writes on startup / schema change
          ▼
 /dev/shm/opdbus/schemas/zeroclaw.json   ← plugin-owned file
          │
          │ reads at startup + SIGHUP (The Shuttle reads)
          ▼
 ┌─────────────────────┐
 │  op-grpc-bridge      │
 │  Axum host           │
 │  Arc<RwLock<Schema>> │
 └──────┬──────────────┘
        │
        ├──── Shared Unix socket /run/ghostbridge/container.sock  (native gRPC; tonic-web + reflection demuxes every service)
        └──── TCP 0.0.0.0:8090  (HTTP + gRPC-Web via tonic-web)
```

**Btrfs is never in this path.** Btrfs (`@zeroclaw` subvolume) is only written
when the zeroclaw plugin commits a schema snapshot for rollback; the Axum host
has no Btrfs dependency.

---

## 3. Component Responsibilities

### 3a. `op-plugins` (unchanged boundary)
- Owns `zeroclaw_schema()` in `plugin_schema_defs.rs`.
- Owns the OSCAL subid registry (all `subid` props).
- Writes `/dev/shm/opdbus/schemas/zeroclaw.json` via the plugin's own startup
  hook.
- Manages the Btrfs `@zeroclaw` subvolume for install/cache/rollback — no other
  crate touches it.

### 3b. `op-grpc-bridge` — `schema_loader`
- `SchemaLoader` holds an `Arc<RwLock<serde_json::Value>>`.
- On startup: reads the plugin-owned JSON file. Panics with a clear message if
  the file is absent (plugin must start first).
- On `SIGHUP` (via `tokio::signal`): reloads within 50 ms.
- Never writes to `/dev/shm/opdbus/`.

### 3c. `op-grpc-bridge` — `dbus_object`
- Connects to the system D-Bus.
- Exports `/org/opdbus/v1/services/zeroclaw_axum_host` with properties:
  - `BindAddr: String` (rw)
  - `SchemaPath: String` (rw)
  - `ReloadIntervalSecs: u32` (rw)
  - `HealthStatus: String` (ro)
- On `BindAddr` or `SchemaPath` write: triggers internal reload without process
  restart.

### 3d. `op-grpc-bridge` — `server`
- Builds an `axum::Router` that:
  1. Mounts the tonic gRPC service at `/zeroclaw.ZeroclawService/…`
  2. Wraps it with `tonic_web::enable(…)` for gRPC-Web + HTTP/1.1 support.
  3. Applies `tower_http::cors::CorsLayer` with configurable origins.
  4. Runs `tracing` middleware that stamps every response with
     `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID`.
- Binds two listeners concurrently via `tokio::join!`:
  - Shared Unix socket `/run/ghostbridge/container.sock` (from `ZEROCLAW_UNIX_SOCKET`;
    native gRPC, demuxed by `tonic_web::enable` + gRPC server reflection). This
    host owns the single shared socket every service routes through.
  - TCP address (HTTP + gRPC-Web)

### 3e. `op-grpc-bridge` — `tracing` middleware
- Extracts `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID` from incoming
  request metadata (passed by OpenClaw Trusted Proxy).
- If absent, generates a new `Trace-ID` (UUID v7) and logs a warning.
- Injects both headers into response trailing metadata.
- Conforms to The Accountability Loop requirement.

---

## 4. Proto Contract

```proto
// crates/op-grpc-bridge/src/grpc/zeroclaw.proto
syntax = "proto3";
package zeroclaw;

// Schema is derived from PluginSchema; fields map 1:1 to zeroclaw_schema() keys.
// Re-generating this file requires running `cargo build` (build.rs invokes prost).
service ZeroclawService {
  rpc GetSchema    (GetSchemaRequest)    returns (SchemaResponse);
  rpc WatchSchema  (WatchSchemaRequest)  returns (stream SchemaEvent);
}

message GetSchemaRequest  {}
message WatchSchemaRequest{}

message SchemaResponse {
  string schema_json = 1;   // serialised PluginSchema JSON
  string trace_id    = 2;
  string footprint   = 3;
}

message SchemaEvent {
  string schema_json = 1;
  string event_type  = 2;   // "initial" | "reload"
}
```

`build.rs` invokes `tonic_build::compile_protos("src/grpc/zeroclaw.proto")`.

---

## 5. OSCAL Subid Assignments (all registered in `op-plugins`)

| subid | what |
|-------|------|
| `src.service.zeroclaw-schema.file@v1` | plugin-owned JSON file in `/dev/shm` |
| `prj.service.zeroclaw-axum-host.bind@v1` | Axum server startup projection step |
| `obs.service.zeroclaw-schema.fetch@v1` | `GetSchema` RPC call |
| `obs.service.zeroclaw-schema.watch@v1` | `WatchSchema` streaming call |
| `mut.service.zeroclaw-axum-host.reload@v1` | SIGHUP / D-Bus triggered reload |
| `evt.service.zeroclaw-schema.reloaded@v1` | emitted after successful reload |
| `exp.service.zeroclaw-schema.grpc-web@v1` | tonic-web consumer surface |

---

## 6. Dependency Additions (`op-grpc-bridge/Cargo.toml`)

New deps require PR justification per AGENTS.md §7. Pinned exact versions.

| crate | version | justification |
|-------|---------|---------------|
| `tonic` | `=0.12.3` | gRPC transport; already in workspace |
| `tonic-web` | `=0.12.3` | gRPC-Web + HTTP/1.1 bridge |
| `tonic-build` | `=0.12.3` | build-time proto compilation |
| `prost` | `=0.13.3` | protobuf runtime |
| `axum` | `=0.7.9` | HTTP router |
| `tower-http` | `=0.5.2` | CORS + tracing layers |
| `tokio` | `=1.40.0` | async runtime (features: full) |
| `serde_json` | `=1.0.133` | schema JSON handling |
| `zbus` | `=4.4.0` | D-Bus (already in workspace) |
| `uuid` | `=1.11.0` | trace-id generation (features: v7) |
| `anyhow` | `=1.0.93` | error handling |
| `tracing` | `=0.1.41` | structured logging |
| `tracing-subscriber` | `=0.3.19` | log formatting |

---

## 7. Startup Order

1. `op-plugins` starts → zeroclaw plugin D-Bus object published →
   `/dev/shm/opdbus/schemas/zeroclaw.json` written.
2. `op-grpc-bridge` starts → `SchemaLoader` reads JSON → D-Bus object registered
   → Axum server binds.
3. Clients connect to Unix socket (native gRPC) or TCP (gRPC-Web/HTTP).

S6 service definitions live in `deploy/s6/`.  `op-grpc-bridge` has a `needs`
dependency on `op-plugins`.
