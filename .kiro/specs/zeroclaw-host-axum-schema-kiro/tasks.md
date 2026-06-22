# Tasks — zeroclaw-host-axum-schema-kiro

Tasks are ordered by dependency. Each task lists its acceptance criteria and the
spec/requirement IDs it satisfies.

---

## T-01 — Zeroclaw schema file writer in `op-plugins`

**Crate:** `crates/op-plugins`
**Satisfies:** FR-01, FR-02, C-01, C-02, V-09

### Work
1. Confirm `zeroclaw_schema()` exists in `plugin_schema_defs.rs`. If not, define
   it there.
2. In `zeroclaw.rs` startup path, serialise `zeroclaw_schema()` to JSON using
   `serde_json` and write to `/dev/shm/opdbus/schemas/zeroclaw.json`.
3. Create parent directory `/dev/shm/opdbus/schemas/` if absent (tmpfs, no
   Btrfs).
4. Register subids in OSCAL registry:
   - `src.service.zeroclaw-schema.file@v1`

### Acceptance
- `/dev/shm/opdbus/schemas/zeroclaw.json` exists after plugin starts.
- Content round-trips through `serde_json::from_str::<PluginSchema>` without
  error.
- Unit test `should_write_zeroclaw_schema_to_shm` passes.

---

## T-02 — Proto definition and `build.rs` in `op-grpc-bridge`

**Crate:** `crates/op-grpc-bridge`
**Satisfies:** FR-04, FR-05, C-04, C-05

### Work
1. Create `crates/op-grpc-bridge/src/grpc/zeroclaw.proto` (see design §4).
2. Create / update `crates/op-grpc-bridge/build.rs` to call
   `tonic_build::compile_protos("src/grpc/zeroclaw.proto")`.
3. Add `tonic`, `tonic-web`, `tonic-build`, `prost` at pinned versions to
   `Cargo.toml` (see design §6).
4. Verify `cargo build -p op-grpc-bridge` succeeds with generated code.

### Acceptance
- `cargo build -p op-grpc-bridge` exits 0.
- Generated `zeroclaw.rs` in `OUT_DIR` is present.
- No hand-written struct duplication of `PluginSchema` fields.

---

## T-03 — `SchemaLoader` in `op-grpc-bridge`

**Crate:** `crates/op-grpc-bridge/src/schema_loader.rs`
**Satisfies:** FR-02, C-02, NFR-02

### Work
1. Implement `SchemaLoader`:
   ```rust
   pub struct SchemaLoader {
       path: PathBuf,
       schema: Arc<RwLock<serde_json::Value>>,
   }
   impl SchemaLoader {
       pub fn new(path: impl Into<PathBuf>) -> anyhow::Result<Self>;
       pub fn load(&self) -> anyhow::Result<()>;          // reads file, updates RwLock
       pub fn get(&self) -> serde_json::Value;            // clone of current schema
       pub fn watch_sighup(self: Arc<Self>) -> tokio::task::JoinHandle<()>;
   }
   ```
2. `load()` must complete within 50 ms (log a warning if exceeded).
3. On `SIGHUP`: call `load()`, emit `evt.service.zeroclaw-schema.reloaded@v1`
   log line.
4. Panic with descriptive message if file is absent at startup.

### Acceptance
- Unit test `should_load_schema_from_file` passes.
- Unit test `should_reload_schema_on_sighup` passes (tokio test with temp file).
- `SchemaLoader` does not write to `/dev/shm/`.

---

## T-04 — D-Bus object in `op-grpc-bridge`

**Crate:** `crates/op-grpc-bridge/src/dbus_object.rs`
**Satisfies:** FR-06, C-06

### Work
1. Implement `ZeroclawAxumHostObject` using `zbus` `#[interface]`:
   - Properties: `BindAddr`, `SchemaPath`, `ReloadIntervalSecs`, `HealthStatus`.
   - Writing `BindAddr` or `SchemaPath` triggers server reload (via channel).
2. Register at `/org/opdbus/v1/services/zeroclaw_axum_host` on the system bus.
3. Register subids in OSCAL registry (in `op-plugins`):
   - `prj.service.zeroclaw-axum-host.bind@v1`
   - `mut.service.zeroclaw-axum-host.reload@v1`

### Acceptance
- `cargo check -p op-grpc-bridge` passes.
- Integration test `should_expose_dbus_properties` passes (zbus test client).
- `HealthStatus` reads `"ok"` after successful startup.

---

## T-05 — Tracing middleware in `op-grpc-bridge`

**Crate:** `crates/op-grpc-bridge/src/tracing.rs`
**Satisfies:** FR-07, C-07, C-08, V-07

### Work
1. Implement a `tower::Layer` (or `axum` middleware) that:
   - Reads `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID` from incoming
     metadata.
   - If absent, generates a UUID v7 `Trace-ID` and logs a `WARN`.
   - Inserts both headers into the response.
2. Apply the layer unconditionally to the Axum router.

### Acceptance
- Unit test `should_stamp_trace_headers_on_response` passes.
- Unit test `should_generate_trace_id_when_absent` passes.

---

## T-06 — Axum server and dual-listener in `op-grpc-bridge`

**Crate:** `crates/op-grpc-bridge/src/server.rs`
**Satisfies:** FR-04, FR-05, C-04, C-05, V-04, V-05, V-08

### Work
1. Build `axum::Router` with:
   - tonic gRPC service wrapped in `tonic_web::enable`.
   - CORS layer (`tower_http::cors::CorsLayer`).
   - Tracing middleware (T-05).
2. Spawn two `tokio` tasks via `tokio::join!`:
   - Shared Unix socket listener at `/run/ghostbridge/container.sock`
     (`ZEROCLAW_UNIX_SOCKET`). This host owns the single shared socket; every
     other service registers against it and `tonic_web::enable` + gRPC server
     reflection demuxes by service/method.
   - TCP listener at configured `BindAddr`.
3. Implement `ZeroclawService` using `SchemaLoader`:
   - `get_schema`: returns current schema JSON + trace headers.
   - `watch_schema`: streams initial schema then reloads.
4. Wire `SchemaLoader::watch_sighup` task.

### Acceptance
- `cargo build -p op-grpc-bridge --release` passes.
- Integration test `should_serve_schema_over_grpc_web` passes.
- Integration test `should_stream_reload_on_sighup` passes.

---

## T-07 — S6 service definition

**Path:** `deploy/s6/op-grpc-bridge/`
**Satisfies:** FR-06 (startup order), C-09

### Work
1. Create `deploy/s6/op-grpc-bridge/run` shell script (Artix / s6 style).
2. Create `deploy/s6/op-grpc-bridge/dependencies.d/op-plugins` (empty file
   declaring s6 dependency).
3. Add `deploy/README-s6.md` entry for `op-grpc-bridge`.

### Acceptance
- Script passes `shellcheck`.
- `op-plugins` is listed as a dependency so s6 orders startup correctly.

---

## T-08 — OSCAL subid registry update in `op-plugins`

**Crate:** `crates/op-plugins`
**Satisfies:** C-07, AGENTS.md §4a

### Work
Register all subids from design §5 in the canonical OSCAL registry file in
`op-plugins`. Exact file path depends on existing registry location — search
`op-plugins` for the registry before creating a new one.

Subids to register:
- `src.service.zeroclaw-schema.file@v1`
- `prj.service.zeroclaw-axum-host.bind@v1`
- `obs.service.zeroclaw-schema.fetch@v1`
- `obs.service.zeroclaw-schema.watch@v1`
- `mut.service.zeroclaw-axum-host.reload@v1`
- `evt.service.zeroclaw-schema.reloaded@v1`
- `exp.service.zeroclaw-schema.grpc-web@v1`

### Acceptance
- CI subid uniqueness check passes.
- All seven subids present in registry with `uuid`, `name`, `ns`, `value` fields.

---

## T-09 — CI lint and test gate

**Satisfies:** NFR-05, NFR-06, V-01, V-02, V-03

### Work
1. Confirm existing CI runs `cargo clippy --workspace --all-targets
   --all-features -- -D warnings`.
2. Confirm existing CI runs `cargo fmt --all -- --check`.
3. Confirm existing CI runs `cargo test --workspace --all-targets
   --all-features`.
4. Add integration test binary target in `op-grpc-bridge/Cargo.toml` under
   `[[test]]` if not already present.

### Acceptance
- All three commands pass on the feature branch with no new warnings.
- Release build `cargo build --workspace --release` passes.
