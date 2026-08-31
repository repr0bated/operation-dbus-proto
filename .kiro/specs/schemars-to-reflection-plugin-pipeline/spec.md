# Spec — schemars-to-reflection-plugin-pipeline

> **Canonical MCP/reflection ingress:** the sealed-PluginSchema → build.rs proto →
> reflection pipeline described here feeds the single authenticated ingress at
> `op-grpc-bridge` TLS `:8090` (see
> `.kiro/specs/unified-authenticated-mcp-cognitive-control-plane/`, which requires
> reflection/callable parity at `:8090`). Two notes: (1) references to
> `/dev/shm/live-schema.json` and a monolithic `plugin_schema_defs.rs` are historical
> per `CLAUDE.md` — the sealed blob catalog `/dev/shm/opdbus/plugin-blobs/` is
> authoritative and `plugin_schema.dat` is not a component; (2) any task to "migrate
> `cognitive_mcp.rs`" is subordinate to the canonical spec's cognitive-registry
> ownership and code-tool-routing requirements — coordinate there.

## Summary

This spec defines the complete plugin-owned pipeline from typed Rust structs through `PluginSchema` to D-Bus, `/dev/shm/live-schema.json`, `build.rs` gRPC proto generation, and `tonic-reflection` descriptor exposure.

The central question this spec answers: **Can `schemars` be the seed for every plugin while `PluginSchema` remains the published single source of truth?** Yes. Schemars is the derivation tool. `PluginSchema` is the published contract. The translation layer is `schemars_adapter::plugin_schema_from_json`. See [design.md](./design.md) for the full architecture answer and layer-by-layer breakdown.

---

## Linked Artifacts

| Artifact | File |
|---|---|
| Requirements | [requirements.md](./requirements.md) |
| Design | [design.md](./design.md) |
| Tasks | [tasks.md](./tasks.md) |

---

## Pipeline Overview

```
Plugin file (<plugin>.rs)
    │
    ├── State struct (#[derive(schemars::JsonSchema)])
    │       │
    │       └── schemars::schema_for!() → JSON Schema 2020-12
    │               │
    │               └── schemars_adapter::plugin_schema_from_json()
    │                       │
    │                       ▼
    ├── Method input/output structs (#[derive(schemars::JsonSchema)])
    │       │
    │       └── method_decl_from_schemars_with_output::<In, Out>()
    │               │
    │               ▼
    └── PluginSchema { fields, methods, subids, ... }   ← SINGLE SOURCE OF TRUTH
            │
            ├── plugin.schema() → StatePlugin trait
            │
            ├── PluginRegistry::register()
            │       ├── SchemaCatalog (in-process index)
            │       └── D-Bus object at /org/opdbus/v1/plugins/<name>
            │
            ├── SchemaEngine → /dev/shm/live-schema.json
            │       (runtime canonical read for capability enforcement,
            │        plugin desired-state reads, PluginInfo listings)
            │
            ├── build.rs (op-grpc-bridge, compile time)
            │       ├── plugin_methods.proto (Struct-typed routes)
            │       └── plugin_method_routes.rs (Rust trait dispatch)
            │
            └── OperationGrpcServer::freeze_plugin_method_reflection()
                    (reads /dev/shm/live-schema.json at startup)
                    │
                    └── PerMethodGrpcServices → FileDescriptorSet
                            (typed field descriptors from MethodDecl.args/returns)
                            │
                            └── tonic-reflection v1 + v1alpha
                                    (grpcurl, MCP clients, Postman discover services)
```

---

## Key Decisions

### 1. Schemars Seeds the Full Contract

Every plugin's state struct and every method's input/output struct derives `schemars::JsonSchema`. `schemars_adapter::plugin_schema_from_json` translates the derived JSON Schema 2020-12 document into `PluginSchema` fields. This is not optional for new or migrated plugins.

### 2. `PluginSchema` Is the Published Interface

No consumer reads schemars output directly. Consumers read `PluginSchema`. Adding a field to the state struct automatically appears in D-Bus dispatch, JSON contract rendering, GUI field rendering, and the live-schema file — because all of those read `PluginSchema`, which is derived from the struct.

### 3. `plugin_schema_defs.rs` Is a Re-Export Aggregator

The AGENTS.md rule "Never define a schema inline in a plugin's own file" means: do not define the schema **for another plugin** inline. Each plugin owns its own `<plugin>_schema()` function co-located in its own file. `plugin_schema_defs.rs` re-exports all of them and provides shared helpers. This is the current correct state of the file.

### 4. Two-Layer gRPC Descriptor Strategy

Build-time (`operation_descriptor.bin`): `google.protobuf.Struct`-typed routes, compiled by `tonic_build`. Provides Rust-level type dispatch.

Runtime (`PerMethodGrpcServices`): field-typed descriptors derived from `MethodDecl.args/returns` JSON Schema. Provides reflection fidelity for clients.

Both layers are required and complementary. Do not attempt to merge them into one.

### 5. D-Bus Introspection Is Not the Method Authority

D-Bus XML introspection reflects what is mounted on `PluginDbusHost`. `PluginSchema.methods` is the authority for method shapes, capability requirements, and dispatch routing. Introspection is read-only metadata, not a schema source.

---

## Canonical Plugin Pattern

Reference implementation: `crates/op-plugins/src/state_plugins/unix_socket.rs`

The fully migrated (Tier A) pattern:

1. **State struct** — derives `schemars::JsonSchema`, carries `#[schemars(extend("x-oscal-subid" = ...))]` at struct and field level
2. **Method input/output structs** — each derive `schemars::JsonSchema`
3. **Schema function** — calls `plugin_schema_from_json`, `apply_state_defaults`, `ensure_category_metadata_fields`, then inserts methods via `method_decl_from_schemars_with_output`
4. **`schema()` impl** — returns the schema function result (with `ensure_category_metadata_fields` called once)
5. **Drift test** — uses `schema_diffs()` to prove the derived schema matches the previous hand-rolled contract
6. **`inventory::submit!`** — self-registration so `DefaultPluginRegistry` discovers the plugin without a central list

---

## Migration Tiers

| Tier | State | When Complete |
|---|---|---|
| **A — Complete** | State struct derives `schemars::JsonSchema`; `schema()` calls `plugin_schema_from_json`; all methods use `_with_output`; drift test present | ✓ Done (e.g., `unix_socket`) |
| **B — Methods Only** | Method input structs typed; state is hand-rolled via `simple_schema`/`any_field` | Phase 1 tasks |
| **C — Legacy** | State and methods both use anonymous `any_field`/`json!({})` | Phase 2 tasks |

---

## Scope

**In scope:**
- Completing the schemars-derivation pipeline for all plugins in `crates/op-plugins/src/state_plugins/`
- Hardening the `build.rs` → proto → reflection chain
- OSCAL subid coverage and CI enforcement
- Integration test for reflection completeness

**Out of scope:**
- Changing the `PluginSchema` wire format or `op-state-store` API
- Modifying the gRPC proto surface (adding new services)
- The MCP tool rendering path (uses `field_input_schema()` — that already works)
- Changing D-Bus interface names or paths
- Any work in `deploy/`, `docs/`, or legacy SQL paths

---

## References

- `schemars` crate: [lib.rs/crates/schemars](https://lib.rs/crates/schemars) — `#[derive(JsonSchema)]`, `schema_for!()`, `SchemaSettings`, custom `extend(...)` attributes
- JSON Schema 2020-12: the dialect emitted by `schemars::schema_for!()` by default
- `tonic-reflection`: `tonic_reflection::server::Builder::register_encoded_file_descriptor_set()` + `build_v1()` / `build_v1alpha()`
- gRPC Server Reflection Protocol: bidirectional streaming `ServerReflectionInfo` RPC; `FileDescriptorSet` encoding via `prost::Message::encode`
- Canonical implementation: `crates/op-plugins/src/state_plugins/unix_socket.rs`
- Adapter: `crates/op-plugins/src/state_plugins/schemars_adapter.rs`
- Helpers: `crates/op-plugins/src/state_plugins/plugin_schema_defs.rs`
- Runtime descriptor freeze: `crates/op-grpc-bridge/src/grpc_server.rs::freeze_plugin_method_reflection()`
- Per-method typed descriptors: `crates/op-grpc-bridge/src/plugin_grpc_gen.rs`
