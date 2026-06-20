# Tasks — plugin-schema-schemars-oscal-migration

Tasks are ordered by dependency. Each task lists acceptance criteria and the requirement IDs it satisfies.

---

## T-01 — Recursive full-fidelity `schema_diffs`

**Crate:** `crates/op-plugins`  
**Satisfies:** FR-02, NFR-05

### Work
1. Rewrite `schemars_adapter::schema_diffs` to recurse into `FieldType::Object` and `FieldType::Array`.
2. Compare field set, recursive types, `required`, descriptions, defaults, examples, constraints, `read_only`, and `immutable_paths` at every nesting level.
3. Add a test using the `unix_socket` hand-rolled golden reference as a regression guard.

### Acceptance
- `cargo test -p op-plugins schemars` passes.
- `schema_diffs(&hand, &derived)` for `unix_socket` returns an empty vector.
- A synthetic mismatch test returns a non-empty diff list.

---

## T-02 — OSCAL subid ingestion and validation

**Crate:** `crates/op-plugins`  
**Satisfies:** FR-03, FR-04, NFR-06

### Work
1. Extend `plugin_schema_from_json` to read `x-oscal-subid` from the root schema object and from each property.
2. Populate `PluginSchema.subids` with field names and a reserved root key.
3. Create `state_plugins/common/oscal.rs` with `validate_subid` and `category_required_fields` helpers.
4. Move the subid regex from `oscal_subid_registry_schema()` into a shared constant in `common/oscal.rs`.

### Acceptance
- A test struct annotated with `#[schemars(extend("x-oscal-subid" = ...))]` produces a `PluginSchema` with the expected `subids` map.
- Invalid subids cause a test failure or a clear diff entry.
- `cargo clippy -p op-plugins -D warnings` passes.

---

## T-03 — Convert `oscal_subid_registry` to a typed struct

**Crate:** `crates/op-plugins`  
**Satisfies:** FR-05, FR-04

### Work
1. Define `OscalSubidRegistryEntry` struct matching the existing hand-rolled schema.
2. Add `JsonSchema` derive and OSCAL annotations to the struct and its fields.
3. Replace `oscal_subid_registry_schema()` with a derived schema function.
4. Keep the old hand-rolled schema as a `#[cfg(test)]` golden reference and add an equivalence test.

### Acceptance
- `oscal_subid_registry` schema is derived from a typed struct.
- `schema_diffs` between the golden reference and derived schema is empty.
- The regex pattern in the schema is preserved.

---

## T-04 — Retrofit OSCAL annotations on `unix_socket`

**Crate:** `crates/op-plugins`  
**Satisfies:** FR-03, FR-04

### Work
1. Add `#[schemars(extend("x-oscal-subid" = "..."))]` to `UnixSocketState` and `SocketEndpoint` fields.
2. Ensure `immutable_paths` remains declared at the struct level.
3. Update the golden-reference equivalence test to also assert subids.

### Acceptance
- `unix_socket` derived schema contains the expected `subids` for the schema and each field.
- `schema_diffs` remains empty.
- `cargo test -p op-plugins unix_socket` passes.

---

## T-05 — Convert `cron` (smallest opaque plugin)

**Crate:** `crates/op-plugins`  
**Satisfies:** FR-01, FR-03, FR-06, NFR-05

### Work
1. Define typed structs from the existing JSON examples:
   - `CronJob`, `CronSchedules`, `CronConfig`.
2. Rewrite `CronState` to use the typed structs instead of `Value`.
3. Add `JsonSchema` derive and OSCAL annotations.
4. Keep the old hand-rolled schema as `#[cfg(test)]` and add a full-fidelity equivalence test.
5. Point `cron_schema()` and `plugin_schema_defs.rs` at the derived schema.

### Acceptance
- `cron` schema is derived from typed structs.
- `schema_diffs` between golden and derived is empty.
- All subids are valid and present.
- `cargo test -p op-plugins cron` passes.

---

## T-06 — Extract shared LLM projection and convert `zeroclaw`

**Crate:** `crates/op-plugins`  
**Satisfies:** FR-01, FR-03, FR-08, FR-09

### Work
1. Create `state_plugins/common/llm_projection.rs` with typed structs for `Provider`, `ModelRoute`, `Router`, `LlmTool`, `ConfigSchema`, `UiSurface`, `StructuredOutput`.
2. Add OSCAL annotations to every struct and field.
3. Rewrite `ZeroclawState` to embed `LlmProjection` plus plugin-specific fields (`status`, `selected_provider`, `selected_model`, `transport`).
4. Derive `zeroclaw_schema` from the typed state.
5. Keep the old hand-rolled schema as `#[cfg(test)]` and add a full-fidelity equivalence test.
6. Ensure `/dev/shm/opdbus/schemas/zeroclaw.json` is still written by the plugin with the same shape.

### Acceptance
- `zeroclaw` schema is derived from typed structs.
- `schema_diffs` between golden and derived is empty.
- The schema JSON round-trips to `PluginSchema`.
- The schema file is still written by `ZeroclawPlugin::write_schema_file`.
- `cargo test -p op-plugins zeroclaw` passes.

---

## T-07 — Convert `antigravity` using the shared projection

**Crate:** `crates/op-plugins`  
**Satisfies:** FR-01, FR-03, FR-09

### Work
1. Rewrite `AntigravityState` to embed `LlmProjection` plus antigravity-specific fields (`auth`, `project`, `models`, `generation_config`, `safety_settings`, `usage`, `endpoints`, `config_schema`, `ui_surfaces`).
2. Define typed structs for the antigravity-specific fields, borrowing naming from the downloaded `google-antigravity` SDK where applicable.
3. Add OSCAL annotations.
4. Keep the old hand-rolled schema as `#[cfg(test)]` and add a full-fidelity equivalence test.
5. Point `antigravity_schema()` and `plugin_schema_defs.rs` at the derived schema.

### Acceptance
- `antigravity` schema is derived from typed structs.
- `schema_diffs` between golden and derived is empty.
- All subids are valid and present.
- `cargo test -p op-plugins antigravity` passes.

---

## T-08 — Convert `antigravity_chat`

**Crate:** `crates/op-plugins`  
**Satisfies:** FR-01, FR-03, FR-10

### Work
1. Define typed structs for `Bridge`, `Auth`, `Model`, `ChatConfig` from the existing JSON examples.
2. Align `ChatConfig` naming with `LocalAgentConfig` / `CapabilitiesConfig` from the downloaded SDK where applicable.
3. Add `JsonSchema` derive and OSCAL annotations.
4. Add a golden-reference equivalence test.
5. Point `antigravity_chat_schema()` and `plugin_schema_defs.rs` at the derived schema.

### Acceptance
- `antigravity_chat` schema is derived from typed structs.
- `schema_diffs` between golden and derived is empty.
- All subids are valid and present.
- `cargo test -p op-plugins antigravity_chat` passes.

---

## T-09 — Convert the 14 mechanical candidates

**Crate:** `crates/op-plugins`  
**Satisfies:** FR-01, FR-03, NFR-05

### Work
For each plugin in the mechanical list (`adc`, `mcp`, `compact_mcp`, `cognitive_mcp`, `agent_config`, `keypair`, `endpoint`, `net`, `hardware`, `software`, `sessdecl`, `gcloud_adc`, `config`, `ctl_plane_chatbot`):

1. Confirm the existing struct already matches the hand-rolled schema with no opaque `Value` fields.
2. Add `JsonSchema` derive and OSCAL annotations.
3. Add `*_schema_derived()` using the adapter.
4. Keep the old hand-rolled schema as `#[cfg(test)]` and add a one-line equivalence test using `assert!(schema_diffs(...).is_empty())`.
5. Update `plugin_schema_defs.rs` and the plugin's `schema()` method to use the derived schema.

### Acceptance
- Each converted plugin's derived schema matches its golden reference.
- `cargo test -p op-plugins` passes for all converted plugins.
- No new warnings from `cargo clippy`.

---

## T-10 — Author structs for the remaining no-struct plugins

**Crate:** `crates/op-plugins`  
**Satisfies:** FR-01, FR-03, FR-06

### Work
For the remaining plugins without backing structs (`lxc`, `procfs`, `web_ui`, `notebooklm`, plus any others surfaced during the migration):

1. Define typed structs from the existing hand-rolled schemas.
2. Add `JsonSchema` derive and OSCAL annotations.
3. Add golden-reference equivalence tests.
4. Convert one plugin at a time; these are the highest-risk conversions because they often involve runtime-touching `Value` fields.

### Acceptance
- Each plugin's schema is derived from a typed struct.
- Equivalence tests pass.
- No runtime regressions in plugins that are actively used.

---

## T-11 — Update migration documentation and SIGNALS

**Paths:** `docs/schema-from-structs.md`, `SIGNALS.md`  
**Satisfies:** FR-01, FR-03, NFR-04

### Work
1. Update `docs/schema-from-structs.md` with the OSCAL annotation recipe and the recursive-diff test pattern.
2. Add a section on the shared LLM projection module.
3. Close SIGNALS OD-25 and add a new signal when the migration is complete.

### Acceptance
- Documentation accurately reflects the new standard.
- `SIGNALS.md` is updated with the migration outcome.

---

## T-12 — Verify zeroclaw Axum host compatibility

**Crate:** `crates/op-grpc-bridge` (consumer)  
**Satisfies:** FR-08

### Work
1. After `zeroclaw.rs` is migrated (T-06), run the existing integration tests in `op-grpc-bridge`:
   - `cargo test -p op-grpc-bridge should_serve_schema_over_grpc_web` or equivalent.
   - Verify `/dev/shm/opdbus/schemas/zeroclaw.json` is written and readable.
2. If no such integration test exists, add a lightweight test that reads the schema file and calls `serde_json::from_str::<PluginSchema>`.

### Acceptance
- The migrated `zeroclaw` schema is still served by the Axum host without host-side changes.
- Existing `op-grpc-bridge` tests pass.

---

## T-13 — CI subid uniqueness gate

**Path:** `.github/workflows/` or CI scripts  
**Satisfies:** FR-04, NFR-06

### Work
1. Add a Rust test or CI script that loads every plugin schema and collects all `subid` values.
2. Assert no duplicates.
3. Assert every subid passes `validate_subid()`.
4. Assert every `mut.*`/`evt.*`/`src.*` subid has the required metadata fields in the registry or in the schema itself.

### Acceptance
- CI fails if a duplicate or invalid subid is introduced.
- The check runs as part of `cargo test` or a dedicated CI job.

---

## T-14 — Final lint and test gate

**Satisfies:** NFR-01, NFR-02, NFR-03, NFR-04

### Work
1. Run `cargo fmt --all -- --check`.
2. Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
3. Run `cargo test --workspace --all-targets --all-features`.
4. Fix any failures before considering the migration complete.

### Acceptance
- All three commands pass.
- No new warnings are introduced.
