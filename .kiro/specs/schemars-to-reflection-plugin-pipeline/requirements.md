# Requirements — schemars-to-reflection-plugin-pipeline

## Context

Every plugin in `op-dbus` is the single source of truth for its own contract. That contract begins with typed Rust structs, flows through `PluginSchema` (the published interface object), is registered on D-Bus at `/org/opdbus/v1/plugins/<name>`, serialized into `/dev/shm/live-schema.json`, compiled by `build.rs` into gRPC proto services, and exposed through `tonic-reflection` so any client can discover methods without a `.proto` file.

The pipeline works end-to-end today for method inputs. The gaps are:
- **State-field schemas** for most plugins are still hand-rolled with `simple_schema()` / `any_field()` instead of derived from typed structs via `schemars`.
- **Method input schemas** in `build.rs` generate `google.protobuf.Struct` I/O, not field-typed proto messages. Only the runtime-frozen `PerMethodGrpcServices` descriptor has real field types.
- **`method_decl_from_schemars`** is marked `#[deprecated]` — callers should use the `_with_output` variant everywhere.
- The canonical pattern (illustrated by `unix_socket.rs`) is not yet documented or enforced per-plugin.

This spec defines the requirements for completing and hardening the pipeline so schemars is the seed for every plugin's full contract and `PluginSchema` remains the sole published source of truth.

---

## Functional Requirements

### REQ-1 — The Plugin Is the Schema

**REQ-1.1** Every plugin MUST own its schema function (`<plugin>_schema() -> PluginSchema`) co-located in its own file. Schemas MUST NOT be defined inline in any other module.

**REQ-1.2** The re-export aggregator `plugin_schema_defs.rs` MUST remain a thin re-export-only module. No schema logic belongs there. Utility helpers (`method_decl_from_schemars_with_output`, `AckOutput`, `materialize_state_from_schema`, etc.) may remain as shared tooling.

**REQ-1.3** `PluginSchema` remains the single published contract object. Schemars is the derivation tool, not a replacement for `PluginSchema`. The `schema()` method on `StatePlugin` always returns a `PluginSchema`, never raw schemars output.

---

### REQ-2 — State Structs Must Derive `JsonSchema`

**REQ-2.1** Every plugin's primary state struct (the one whose fields become `PluginSchema.fields`) MUST derive `schemars::JsonSchema`. It MUST also derive `serde::Serialize` and `serde::Deserialize`.

**REQ-2.2** Nested structs referenced from a state struct (e.g., `SocketEndpoint` inside `UnixSocketState`) MUST also derive `schemars::JsonSchema`.

**REQ-2.3** The schema function MUST call `schemars_adapter::plugin_schema_from_json(name, version, description, &serde_json::to_value(schemars::schema_for!(StateStruct)).unwrap())` to produce `PluginSchema.fields`. Manual `FieldSchema` construction for state fields is forbidden for new and migrated plugins.

**REQ-2.4** Where the struct's `Default` impl carries meaningful values, `apply_state_defaults(&mut schema, &simd_json::serde::to_owned_value(StateStruct::default()))` MUST be called after `plugin_schema_from_json` to propagate defaults into the schema.

**REQ-2.5** The schemars-derived schema MUST be guarded by a test using `schemars_adapter::schema_diffs()` that proves the derived schema matches the previous hand-rolled reference (or, for new plugins, the intended contract). This test must be co-located in the plugin file.

---

### REQ-3 — OSCAL Subids Must Be Carried by the Struct

**REQ-3.1** Every schema-level subid (the schema's own OSCAL identifier) MUST be declared on the state struct via `#[schemars(extend("x-oscal-subid" = "sch.<type>.<subject>.describe@vN"))]`.

**REQ-3.2** Every field-level subid MUST be declared on the struct field via `#[schemars(extend("x-oscal-subid" = "<category>.<type>.<subject>.<verb>@vN"))]`.

**REQ-3.3** `schemars_adapter::plugin_schema_from_json` already reads `x-oscal-subid` from the schemars JSON output and populates `PluginSchema.subids`. No post-hoc `schema.subids.insert(...)` is needed for fields that carry the annotation on the struct.

**REQ-3.4** Subids that refer to method-level artifacts (e.g., `createunixsocket`) and have no corresponding struct field MAY be inserted into `schema.subids` imperatively after schema construction. This is the only acceptable use of imperative `subids.insert`.

---

### REQ-4 — Method Declarations Must Be Typed

**REQ-4.1** Every `MethodDecl` MUST be constructed via `method_decl_from_schemars_with_output::<Input, Output>(...)`. The deprecated `method_decl_from_schemars::<Input>()` variant MUST NOT be used in new or migrated code.

**REQ-4.2** Every method input type MUST be a dedicated named struct (e.g., `SetDeviceInput`) deriving `schemars::JsonSchema`, `serde::Serialize`, `serde::Deserialize`. Anonymous `json!({...})` as `args` is forbidden for plugins that have typed structs.

**REQ-4.3** Every method output type MUST be a named struct. For methods that return only success/failure, `AckOutput` from `plugin_schema_defs` MUST be used. For methods with richer return data, a dedicated `<Method>Output` struct deriving `schemars::JsonSchema` MUST be defined.

**REQ-4.4** `MethodDecl.returns` MUST always be `Some(...)`. A `None` returns value is a contract gap. Callers that only need to know whether the call succeeded MUST return `AckOutput`.

**REQ-4.5** Every `MethodDecl` MUST carry a valid OSCAL subid per the taxonomy in AGENTS.md § 4a. The subid uniqueness CI gate (`all_plugin_subids_are_valid_and_unique` in `default_registry.rs`) must pass.

---

### REQ-5 — `PluginSchema.methods` Drives `build.rs` Proto Generation

**REQ-5.1** `build.rs` in `op-grpc-bridge` MUST continue to instantiate all plugins at compile time, read `PluginSchema.methods`, and generate `plugin_methods.proto` and `plugin_method_routes.rs`.

**REQ-5.2** The generated `plugin_methods.proto` currently uses `google.protobuf.Struct` for all I/O. This is acceptable at the build-time level because the runtime `PerMethodGrpcServices` freezes proper typed descriptors. The two layers are complementary: build-time routes provide type-checked Rust trait dispatch; runtime descriptors provide typed reflection for clients. Both MUST remain in sync with `PluginSchema.methods`.

**REQ-5.3** `build.rs` MUST emit `cargo:rerun-if-changed=../op-plugins/src/state_plugins` so that adding or modifying a plugin's method declarations triggers a rebuild. This is already present and MUST be preserved.

**REQ-5.4** If a plugin's method set is empty, `build.rs` MUST silently skip it (no empty proto service). This is already the behavior and MUST be preserved.

---

### REQ-6 — `PerMethodGrpcServices` Produces Typed gRPC Reflection Descriptors

**REQ-6.1** `PerMethodGrpcServices::create_frozen_services()` MUST produce `FileDescriptorProto` entries with field-typed input/output messages (not `google.protobuf.Struct`), derived from `MethodDecl.args` and `MethodDecl.returns` JSON schemas.

**REQ-6.2** `freeze_plugin_method_reflection()` on `OperationGrpcServer` MUST be called before `build_operation_routes()`. The combined descriptor snapshot passed to `tonic_reflection::server::Builder` must include both the static domain proto descriptors and all per-method typed descriptors.

**REQ-6.3** `tonic-reflection` v1 and v1alpha endpoints MUST both be mounted and must agree on the same combined descriptor.

**REQ-6.4** A `grpcurl -plaintext <addr> describe` on the running server MUST enumerate every plugin service for every plugin that has at least one `MethodDecl`. This is the acceptance criterion for reflection completeness.

---

### REQ-7 — D-Bus Object Export

**REQ-7.1** Every registered plugin MUST be exported as a D-Bus object at the canonical path `/org/opdbus/v1/plugins/<plugin_name>` (underscored, lowercase) by `PluginRegistry::register()`. This is already enforced.

**REQ-7.2** The D-Bus object interface name MUST be `org.opdbus.v1.Plugin`. Methods on the object are dispatched through `PluginDbusHost` using `PluginSchema.methods` as the authority. D-Bus XML introspection is NOT used to derive method shapes — `PluginSchema` is the sole authority.

**REQ-7.3** The D-Bus method dispatch path MUST read `PluginSchema.methods` for capability enforcement, input validation, and routing. It MUST NOT read from `simple_schema()`-style anonymous JSON objects.

---

### REQ-8 — `/dev/shm/live-schema.json` and Manifest

**REQ-8.1** `SchemaEngine` MUST write the aggregated `PluginSchema` JSON (as produced by `PluginSchema::to_json_schema()`) for every registered plugin to `/dev/shm/live-schema.json` keyed by plugin name.

**REQ-8.2** The live-schema file is the runtime canonical source for `grpc_server.rs` capability enforcement and for `UnixSocketPlugin::read_desired()`. It MUST remain the single runtime read-path; no second shm file per plugin.

**REQ-8.3** Writes to `/dev/shm/live-schema.json` MUST NOT go through Btrfs I/O. tmpfs/ramfs is the correct mount.

---

### REQ-9 — No Bypasses

**REQ-9.1** No plugin MAY read live state by spawning a subprocess (`Command::new("wg")`, `Command::new("ip")`, etc.) in its schema or dispatch path. The D-Bus object owns plugin state.

**REQ-9.2** No plugin MAY define its method contract via a hardcoded `json!({"type": "object", "additionalProperties": true})` in a `MethodDecl.args` field. All `args` must be derived from a typed struct.

**REQ-9.3** Legacy `SqlitePluginCatalog` and JSON-RPC polling loops MUST NOT be introduced. The `SchemaCatalog` / `SchemaEngine` path is the only valid catalog path.

---

### REQ-10 — Migration Contract

**REQ-10.1** Plugins fall into three migration tiers:

| Tier | State | Action required |
|------|-------|-----------------|
| **A — Complete** | State struct derives `schemars::JsonSchema`; `schema()` calls `plugin_schema_from_json`; all methods use `_with_output` | No action |
| **B — Methods only** | Method input structs derive `schemars::JsonSchema` but state struct is hand-rolled | Add `#[derive(schemars::JsonSchema)]` to state struct; replace `simple_schema()` with derived path; write drift test |
| **C — Legacy** | State is `any_field()` / `json!({})` throughout | Full migration: type the state struct, derive schemars, replace schema fn, write tests |

**REQ-10.2** Migration MUST be done plugin-by-plugin. Each migration commit MUST pass `cargo test -p op-plugins` and the OSCAL subid gate before merging.

**REQ-10.3** Tier A compliance is verified by the presence of the `schemars::JsonSchema` derive on the state struct AND a `schema_diffs()` test in the plugin file. CI MUST enforce this check for any plugin file that calls `plugin_schema_from_json`.

---

## Non-Functional Requirements

**NFR-1 — Zero-Copy**: Schema materialization from `schemars_adapter::plugin_schema_from_json` runs once at plugin load. The resulting `PluginSchema` is shared via `Arc`. No re-derivation occurs on the hot dispatch path.

**NFR-2 — Build Determinism**: `build.rs` must produce the same `plugin_methods.proto` and `plugin_method_routes.rs` for the same plugin source. The plugin instantiation in `build.rs` is side-effect-free (no network, no fs writes, no D-Bus calls).

**NFR-3 — No New Dependencies**: This pipeline uses only crates already in the workspace (`schemars`, `serde_json`, `simd_json`, `prost`, `prost-types`, `tonic-reflection`). No new crates are introduced.

**NFR-4 — Backward Compatibility**: Existing callers of `plugin_schema_from_json` and `method_decl_from_schemars_with_output` are unaffected by migration of other plugins. The `PluginSchema` wire format is stable.
