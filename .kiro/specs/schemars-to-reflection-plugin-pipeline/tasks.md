# Tasks — schemars-to-reflection-plugin-pipeline

Each task is self-contained, targets one layer of the pipeline, and ends with a `cargo check` / `cargo test` green gate. Complete tasks in order — later tasks build on earlier ones.

---

## Phase 0 — Infrastructure Hardening

### Task 0.1 — Enforce `method_decl_from_schemars_with_output` Everywhere

**Requirements**: REQ-4.1, REQ-4.4

**Context**: `method_decl_from_schemars` is marked `#[deprecated]`. All call sites must migrate to `method_decl_from_schemars_with_output::<Input, Output>`.

**Steps**:
1. Run `grep -rn "method_decl_from_schemars::<" crates/op-plugins/src/state_plugins/` to enumerate all deprecated call sites.
2. For each call site: identify whether the method has a meaningful return value or just success/failure.
   - If just success/failure: replace with `method_decl_from_schemars_with_output::<XyzInput, AckOutput>`.
   - If richer return: define a `<Method>Output` struct deriving `schemars::JsonSchema` and use it.
3. Verify no `method_decl_from_schemars::<` calls remain: `grep -rn "method_decl_from_schemars::<" crates/ | grep -v "method_decl_from_schemars_with_output"`.
4. Run `cargo check -p op-plugins`.
5. Run `cargo test -p op-plugins -- all_plugin_subids_are_valid_and_unique`.

**Acceptance**: Zero deprecated-function call sites. CI subid gate passes.

---

### Task 0.2 — Add `PluginSchemaBuilder::method` Convenience Method

**Requirements**: REQ-4.1, REQ-5.2

**Context**: Several plugins construct `schema.methods.insert(...)` imperatively. A builder method makes the chain ergonomic. This already exists on `PluginSchemaBuilder` (`fn method(mut self, decl: MethodDecl) -> Self`) — verify it is present and used consistently.

**Steps**:
1. Check `crates/op-state-store/src/plugin_schema.rs` for `PluginSchemaBuilder::method`.
2. If missing, add:
   ```rust
   pub fn method(mut self, decl: MethodDecl) -> Self {
       self.methods.insert(decl.name.clone(), decl);
       self
   }
   ```
3. No functional change — builder pattern for ergonomics only.
4. Run `cargo check -p op-state-store`.

**Acceptance**: Builder has `method()`. No new deps introduced.

---

### Task 0.3 — Document the Canonical Plugin Pattern in a Steering File

**Requirements**: REQ-1.1, REQ-2.3, REQ-10.1

**Context**: The canonical pattern (illustrated by `unix_socket.rs`) needs to be a steering file so agents and developers can reference it without reading source.

**Steps**:
1. Create `.kiro/steering/plugin-schema-pattern.md` with:
   - The canonical struct annotation pattern (`#[derive(schemars::JsonSchema)]`, `#[schemars(extend(...))]`)
   - The canonical schema function body (`plugin_schema_from_json` + `apply_state_defaults` + method inserts)
   - The drift test pattern (`schema_diffs`)
   - Tier classification table from REQ-10.1
   - The forbidden patterns (`simple_schema` for new plugins, `any_field` for typed state, deprecated `method_decl_from_schemars`)
2. Add `inclusion: auto` front-matter so the steering file is always included.

**Acceptance**: File exists at `.kiro/steering/plugin-schema-pattern.md` with front-matter `inclusion: auto`.

---

## Phase 1 — Tier B Plugin Migrations (Methods-Only → Full Struct)

### Task 1.1 — Migrate `wireguard.rs` to Full Struct Derivation

**Requirements**: REQ-2.1 – REQ-2.5, REQ-3.1 – REQ-3.4

**Context**: `wireguard.rs` has typed method input structs but the state (`WireGuardState`, `WireGuardInterface`, `WireGuardPeer`) is hand-rolled via `simple_schema()` + `any_field()`. The state schema is currently `any_field(true, "WireGuard interfaces", Some(json!([])))` — a single `Any`-typed array field.

**Steps**:
1. Add `schemars::JsonSchema` to `WireGuardState`, `WireGuardInterface`, `WireGuardPeer`.
2. Add `#[schemars(extend("x-oscal-subid" = "sch.network.plugin.wireguard.schema@v1"))]` to `WireGuardState`.
3. Add field-level `#[schemars(...)]` annotations: descriptions on all fields, OSCAL subids on state-significant fields (`interfaces` → `"obs.network.wireguard.interfaces@v1"`), `readOnly` on derived fields if any.
4. Replace `wireguard_schema()` body:
   ```rust
   pub(crate) fn wireguard_schema() -> PluginSchema {
       let root = serde_json::to_value(schemars::schema_for!(WireGuardState)).unwrap();
       let mut schema = schemars_adapter::plugin_schema_from_json(
           "wireguard", "1.0.0",
           "WireGuard interface state", &root,
       );
       common::oscal::ensure_category_metadata_fields(&mut schema);
       // Methods — keep existing, migrate to _with_output in Task 0.1
       schema.methods.insert("set_device".to_string(), ...);
       // ...
       schema
   }
   ```
5. Write the drift guard test:
   ```rust
   #[test]
   fn derived_schema_matches_declared_contract() {
       let schema = wireguard_schema();
       // interfaces field must be Array of Object
       let ifaces = schema.fields.get("interfaces").unwrap();
       assert!(matches!(ifaces.field_type, FieldType::Array(_)));
       // subid present
       assert!(schema.subids.contains_key("__schema__"));
   }
   ```
6. Run `cargo test -p op-plugins -- wireguard`.

**Acceptance**: `wireguard_schema()` uses `plugin_schema_from_json`. All wireguard tests pass. Subid gate passes.

---

### Task 1.2 — Migrate `net.rs` to Full Struct Derivation

**Requirements**: REQ-2.1 – REQ-2.5

**Context**: The `net` plugin is the most queried plugin. Its state schema must be typed.

**Steps** (same pattern as Task 1.1):
1. Read current `net_schema()` to identify all state fields.
2. Define `NetState` struct (or rename existing) deriving `schemars::JsonSchema` with all current fields typed correctly.
3. Define nested structs for any object-typed fields.
4. Add OSCAL subids: `"sch.network.plugin.net.schema@v1"` on root, per-field subids.
5. Replace schema fn body with `plugin_schema_from_json` call.
6. Write drift test comparing derived schema to current field set.
7. `cargo test -p op-plugins -- net`.

**Acceptance**: `net_schema()` uses `plugin_schema_from_json`. Test passes.

---

### Task 1.3 — Migrate Remaining Tier B Plugins

**Requirements**: REQ-2.1 – REQ-2.5, REQ-10.1

**Context**: Any plugin that has typed method input structs but hand-rolled state fields. Candidates include: `s6.rs`, `incus.rs`, `hardware.rs`, `software.rs`, `users.rs`, `service.rs`, `proxmox.rs`, `rtnetlink.rs`, `ovsdb_bridge.rs`.

For each plugin:
1. Run `grep -n "simple_schema\|any_field\|schema_from_state" crates/op-plugins/src/state_plugins/<plugin>.rs`.
2. If present: follow Task 1.1 steps.
3. Run `cargo test -p op-plugins -- <plugin_name>`.

**Acceptance**: No `simple_schema(` calls remain in any plugin that has typed method input structs. All tests pass.

---

## Phase 2 — Tier C Plugin Migrations (Legacy → Full Struct)

### Task 2.1 — Audit All Tier C Plugins

**Requirements**: REQ-10.1, REQ-9.2

**Context**: Tier C plugins use `any_field()` / anonymous `json!({})` for both state and methods.

**Steps**:
1. Run `grep -rn 'any_field\|json!({"type": "object"\|additionalProperties.*true' crates/op-plugins/src/state_plugins/ | grep -v test | grep -v plugin_schema_defs` to list candidates.
2. For each candidate: assess whether a typed state struct exists or must be created. Create a sub-task for each plugin that requires full typing.
3. Prioritize plugins that have D-Bus clients or gRPC callers (check `grpc_server.rs` call sites).

**Acceptance**: Audit list produced. Sub-tasks created for each Tier C plugin.

---

### Task 2.2 — Migrate `cognitive_mcp.rs` (Representative Tier C)

**Requirements**: REQ-2.1, REQ-4.2, REQ-4.3

**Steps**:
1. Define `CognitiveMcpState` struct with fields for all state the plugin manages.
2. Define method input/output structs for every `MethodDecl` that uses anonymous `json!`.
3. Migrate schema function to `plugin_schema_from_json`.
4. Write drift test.
5. `cargo test -p op-plugins -- cognitive_mcp`.

**Acceptance**: `cognitive_mcp_schema()` uses `plugin_schema_from_json`. Zero anonymous `json!` in method args.

---

## Phase 3 — Build-Time Proto Fidelity

### Task 3.1 — Validate `build.rs` Descriptor Coverage

**Requirements**: REQ-5.1, REQ-5.2, REQ-5.3

**Context**: Verify that `build.rs` generates one service per plugin that has methods, and that the service names match what `grpc_server.rs` expects.

**Steps**:
1. Build the crate: `cargo build -p op-grpc-bridge 2>&1 | head -50`.
2. Inspect `$OUT_DIR/plugin_methods.proto` — verify one `service` block per plugin with methods.
3. Inspect `$OUT_DIR/plugin_method_routes.rs` — verify one `impl` block per service.
4. Confirm `operation_descriptor.bin` is non-empty: `ls -lh $OUT_DIR/operation_descriptor.bin`.
5. Add a build-time assertion (in `build.rs`) that the generated proto is non-empty when any plugin has methods.

**Acceptance**: `plugin_methods.proto` has exactly N service blocks for N plugins with methods. Build passes.

---

### Task 3.2 — Verify Struct-Type I/O Does Not Appear in Runtime Reflection

**Requirements**: REQ-6.1, REQ-6.4

**Context**: `grpcurl describe` on a running server should show typed fields for plugin method services (from the runtime frozen descriptor), not `google.protobuf.Struct`.

**Steps**:
1. Start the grpc bridge locally (or use the existing running instance).
2. Run: `grpcurl -plaintext 127.0.0.1:50051 describe operation.method.wireguard.set_device.SetDeviceService`
3. Expected: the service shows `input_type: .operation.method.wireguard.set_device.SetDeviceInput` with fields `interface: string`, `private_key: string`, `listen_port: int64`, `fwmark: int64`.
4. If `google.protobuf.Struct` appears instead: the runtime freeze did not run or the combined descriptor was not built correctly. Debug `freeze_plugin_method_reflection()` sequence in `run_grpc_server`.
5. Document the test command and expected output in `crates/op-grpc-bridge/SPEC.md`.

**Acceptance**: `grpcurl describe` shows typed fields for at least `wireguard` and `unix_socket` method services.

---

## Phase 4 — Schema Fidelity Gates

### Task 4.1 — Add `schemars::JsonSchema` Lint to CI

**Requirements**: REQ-2.1, REQ-10.3

**Context**: Enforce that all plugins calling `plugin_schema_from_json` have a `schemars::JsonSchema` derive on their state struct.

**Steps**:
1. In `crates/op-plugins/src/state_plugins/mod.rs` or a CI check script, add a compile-time check that the state struct type passed to `schemars::schema_for!()` implements `schemars::JsonSchema`. This is already enforced by the macro — no additional check needed beyond ensuring `plugin_schema_from_json` is called with the output of `schema_for!`.
2. Add a workspace-level `cargo clippy` check: `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
3. Add to the existing `all_plugin_subids_are_valid_and_unique` test: assert that every plugin whose `schema()` result has non-empty `fields` also has a `subids["__schema__"]` entry (the schema-level OSCAL subid).

**Acceptance**: `cargo clippy` passes with `-D warnings`. `all_plugin_subids_are_valid_and_unique` extended check passes.

---

### Task 4.2 — Extend Schema Drift Test Coverage

**Requirements**: REQ-2.5

**Context**: Every migrated plugin must have a test using `schema_diffs()`. Currently only `unix_socket.rs` has this. `schemars_adapter.rs` tests verify the adapter mechanics but not per-plugin contracts.

**Steps**:
1. For every plugin migrated in Phase 1/2: verify a drift test exists in the plugin file.
2. Add a workspace-level test that iterates `DefaultPluginRegistry::available_plugins()` and asserts each plugin's `schema()` returns a `PluginSchema` with non-empty `name` and at least one field or method (no empty schema contracts).
3. Run `cargo test --workspace`.

**Acceptance**: Every Tier A/B plugin has a drift test. Workspace test passes.

---

### Task 4.3 — OSCAL Subid Registry Completeness Check

**Requirements**: REQ-3.1 – REQ-3.4, REQ-4.5

**Context**: The `all_plugin_subids_are_valid_and_unique` test in `default_registry.rs` validates format and uniqueness. Extend it to check completeness for migrated plugins.

**Steps**:
1. Add assertion: every `MethodDecl` subid must have category `mut`, `obs`, or `exp` (not `src`, `prj`, `sch`, `evt` — those are for methods that emit or observe, but method invocations are `mut`/`obs`/`exp`).
2. Add assertion: every plugin that has `PluginSchema.methods` non-empty must have a `subids["__schema__"]` entry.
3. Run `cargo test -p op-plugins -- all_plugin_subids_are_valid_and_unique`.

**Acceptance**: Extended subid gate passes for all plugins.

---

## Phase 5 — Reflection End-to-End Smoke Test

### Task 5.1 — Write Integration Test for Reflection Coverage

**Requirements**: REQ-6.3, REQ-6.4

**Context**: There is no automated test that verifies `tonic-reflection` exposes all plugin services. Add one.

**Steps**:
1. In `crates/op-grpc-bridge/tests/`, create `reflection_coverage.rs`.
2. The test:
   a. Instantiates `OperationGrpcServer` with a test `PluginSchemaProvider` that returns all built-in plugin schemas.
   b. Calls `freeze_plugin_method_reflection()`.
   c. Calls `generate_combined_reflection_descriptor()`.
   d. Decodes the returned bytes as `prost_types::FileDescriptorSet`.
   e. For each plugin that has `schema.methods.is_empty() == false`: asserts that at least one `FileDescriptorProto` in the set has a service with a name matching the expected pattern (`operation.method.<plugin>.*`).
3. Run `cargo test -p op-grpc-bridge -- reflection_coverage`.

**Acceptance**: Test exists and passes. All plugins with methods have a corresponding service in the reflection descriptor.

---

### Task 5.2 — `grpcurl` Smoke Test Script

**Requirements**: REQ-6.4

**Context**: Manual verification tool for operators.

**Steps**:
1. Create `deploy/smoke/grpc-reflection-check.sh`:
   ```sh
   #!/bin/sh
   # Smoke test: verify gRPC reflection covers all plugin services.
   # Usage: ./grpc-reflection-check.sh [host:port]
   ADDR=${1:-127.0.0.1:50051}
   set -e
   echo "==> Listing all services via reflection..."
   grpcurl -plaintext "$ADDR" list
   echo "==> Describing wireguard SetDevice..."
   grpcurl -plaintext "$ADDR" describe operation.method.wireguard.set_device.SetDeviceService
   echo "==> Describing unix_socket Bind..."
   grpcurl -plaintext "$ADDR" describe operation.method.unix_socket.bind.BindService
   echo "==> All checks passed."
   ```
2. `chmod +x deploy/smoke/grpc-reflection-check.sh`.

**Acceptance**: Script exists and is executable. Runs without errors against a live bridge.

---

## Completion Criteria

The pipeline is complete when:

- [ ] Zero calls to deprecated `method_decl_from_schemars::<` remain in the codebase
- [ ] Every plugin migrated to Tier A has a `schema_diffs()` drift test
- [ ] `cargo test --workspace --all-targets --all-features` passes
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes
- [ ] `all_plugin_subids_are_valid_and_unique` passes with the extended completeness checks
- [ ] `reflection_coverage` integration test passes
- [ ] `grpcurl describe` on a live bridge shows typed field names (not `google.protobuf.Struct`) for `wireguard` and `unix_socket` method services
