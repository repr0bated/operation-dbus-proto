# Design — schemars-to-reflection-plugin-pipeline

## Architecture Question: Can Schemars Seed Every Plugin While `PluginSchema` Remains the Published Source of Truth?

**Yes. The answer is yes, and this is the settled architecture.**

Schemars is the **derivation tool**. `PluginSchema` is the **published contract object**. They occupy different layers:

```
Rust struct (schemars::JsonSchema derive)
        │
        │  schemars::schema_for!()  →  JSON Schema 2020-12 document
        │
        ▼
schemars_adapter::plugin_schema_from_json()
        │
        │  Walks JSON, resolves $defs/$ref, maps types, extracts
        │  x-oscal-subid, x-immutable-paths, constraints
        │
        ▼
PluginSchema { fields, methods, subids, ... }  ←── PUBLISHED SOURCE OF TRUTH
        │
        ├──► /dev/shm/live-schema.json   (runtime canonical read)
        ├──► D-Bus object at /org/opdbus/v1/plugins/<name>
        ├──► build.rs  →  plugin_methods.proto  +  plugin_method_routes.rs
        └──► PerMethodGrpcServices  →  FileDescriptorSet  →  tonic-reflection
```

The struct cannot be queried directly by callers. The `PluginSchema` is what every consumer sees. Schemars disappears at the boundary of `plugin_schema_from_json`. This means:
- Adding a field to the state struct automatically surfaces it in D-Bus, in the JSON contract, in the GUI, and in `live-schema.json`.
- Removing a field from the struct removes it everywhere — no possibility of drift.
- The struct is the documentation; doc comments become field `description` strings.

The migration path is plugin-by-plugin. Nothing breaks during migration. The `PluginSchema` interface contract is unchanged.

---

## Layer Map

### Layer 0 — Rust Structs (Plugin-Owned)

```rust
// In crates/op-plugins/src/state_plugins/<plugin>.rs

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.<name>.schema@v1"))]
pub struct MyPluginState {
    /// Human-readable description → FieldSchema.description
    #[schemars(
        description = "...",
        example = &"example_value",
        extend("x-oscal-subid" = "obs.software.<name>.field@v1")
    )]
    pub field_name: String,

    /// Optional<T> maps to anyOf: [{type: T}, {type: null}] → FieldType::String (non-null branch)
    pub optional_field: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetFieldInput {
    pub field_name: String,
    pub new_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetFieldOutput {
    pub success: bool,
    pub previous_value: Option<String>,
}
```

**What schemars derives (JSON Schema 2020-12):**
- `"type": "object"` at the root with `"properties"` for each field
- `"$defs"` for referenced nested types, resolved by `plugin_schema_from_json`
- `"required": [...]` based on non-`Option` fields and fields without `#[serde(default)]`
- `"description"` from doc comments or `#[schemars(description = "...")]`
- `"examples"` array from `#[schemars(example = &value)]`
- `"minimum"` / `"maximum"` from `#[schemars(range(min = N, max = M))]`
- `"x-oscal-subid"` from `#[schemars(extend("x-oscal-subid" = "..."))]`
- `"x-immutable-paths"` from a struct-level `extend` on the root

---

### Layer 1 — `schemars_adapter::plugin_schema_from_json` (Translator)

Lives in `crates/op-plugins/src/state_plugins/schemars_adapter.rs`.

**Input**: `serde_json::Value` from `serde_json::to_value(schemars::schema_for!(StateStruct)).unwrap()`

**What it does**:
1. Extracts `$defs` / `definitions` for `$ref` resolution
2. Walks root `properties` → one `FieldSchema` per field
3. For each field: resolves `$ref`, determines `FieldType` (handles `anyOf`/`oneOf` for `Option<T>` and tagged enums), reads `description`, `examples[0]`/`example`, `default`, `minimum`, `maximum`, `pattern`, `readOnly`, `x-oscal-subid`
4. Recursively expands nested `object` types and `array` items
5. Reads root-level `x-oscal-subid` → `subids["__schema__"]`
6. Reads root-level `x-immutable-paths` → `schema.immutable_paths`

**Output**: `PluginSchema` with `fields` and `subids` populated. `methods` is empty — the plugin schema function populates it imperatively.

**Type mapping**:

| JSON Schema type | `FieldType` |
|---|---|
| `string` | `String` |
| `integer` | `Integer` |
| `number` | `Float` |
| `boolean` | `Boolean` |
| `array` (+ items) | `Array(Box<inner>)` |
| `object` (+ properties) | `Object(HashMap<name, FieldSchema>)` |
| `enum` (string variants) | `Enum(Vec<String>)` |
| `anyOf`/`oneOf` with null + T | collapses to T |
| `anyOf`/`oneOf` multi-branch | `OneOf(Vec<FieldType>)` |
| unknown / null | `Any` |

---

### Layer 2 — `PluginSchema` (Published Contract)

Lives in `crates/op-state-store/src/plugin_schema.rs`. **This is the interface. Do not reach through it.**

Key fields relevant to this pipeline:

```rust
pub struct PluginSchema {
    pub name: String,
    pub version: String,
    pub description: String,
    pub fields: HashMap<String, FieldSchema>,   // ← from schemars_adapter
    pub methods: HashMap<String, MethodDecl>,   // ← set imperatively in schema fn
    pub signals: Vec<SignalDecl>,               // ← optional, set imperatively
    pub subids: HashMap<String, String>,        // ← from x-oscal-subid annotations
    pub immutable_paths: Vec<String>,           // ← from x-immutable-paths
    pub dialect: String,                        // defaults to JSON Schema 2020-12
    pub guarantees: PluginCapabilities,
    // ...
}

pub struct MethodDecl {
    pub name: String,
    pub args: OwnedValue,      // JSON Schema of input (from schemars::schema_for!(InputStruct))
    pub returns: Option<OwnedValue>,  // JSON Schema of output
    pub side_effect: SideEffect,
    pub idempotent: bool,
    pub required_capability: Option<String>,
    pub subid: String,         // OSCAL subid, e.g. "mut.network.wireguard.peer.add@v1"
}
```

`PluginSchema::to_json_schema()` renders the full plugin state schema as JSON Schema 2020-12 with `readOnly`, `propertyDependencies` (conditional immutability), and `x-plugin-category`. This output goes to `/dev/shm/live-schema.json`.

`PluginSchema::field_input_schema(field_name)` renders a single field's schema for MCP tool `input_schema()` — it carries the OSCAL subid as `x-oscal-subid`.

---

### Layer 3 — Plugin Schema Function (Canonical Pattern)

The canonical pattern — exemplified by `unix_socket.rs` — for a migrated plugin:

```rust
pub(crate) fn my_plugin_schema() -> PluginSchema {
    // 1. Derive state schema from struct
    let root = serde_json::to_value(schemars::schema_for!(MyPluginState))
        .expect("schemars serializes cleanly");
    let mut schema = schemars_adapter::plugin_schema_from_json(
        "my_plugin",
        "1.0.0",
        "Human-readable description of what this plugin manages",
        &root,
    );

    // 2. Apply struct Default values to schema defaults (if non-trivial)
    let defaults = simd_json::serde::to_owned_value(MyPluginState::default())
        .expect("default serializes");
    schemars_adapter::apply_state_defaults(&mut schema, &defaults);

    // 3. Ensure OSCAL category metadata fields are present (call only once)
    super::common::oscal::ensure_category_metadata_fields(&mut schema);

    // 4. Register any method-level subids that have no struct field
    schema.subids.insert(
        "my_action".to_string(),
        "mut.software.my-plugin.my-action@v1".to_string(),
    );

    // 5. Declare methods
    schema.methods.insert(
        "set_field".to_string(),
        method_decl_from_schemars_with_output::<SetFieldInput, SetFieldOutput>(
            "SetField",
            SideEffect::Mutation,
            false,
            "my_plugin.write",
            "mut.software.my-plugin.set-field@v1",
        ),
    );

    schema
}
```

The `schema()` method on `StatePlugin` calls this function and returns the result.

---

### Layer 4 — D-Bus Object Export

`PluginRegistry::register()` in `crates/op-plugins/src/registry.rs`:

1. Calls `plugin.schema()` to get the `PluginSchema`
2. Writes it to `SchemaCatalog`
3. Exports a `PluginDbusHost` object at `/org/opdbus/v1/plugins/<name>` (via `connection.object_server().at(...)`)

Path canonicalization: hyphens → underscores, lowercase. E.g., `cognitive-mcp` → `/org/opdbus/v1/plugins/cognitive_mcp`.

`PluginDbusHost` is the zbus-exported object. Its D-Bus methods are dispatched through `PluginSchema.methods` — **not** derived from D-Bus XML introspection. D-Bus introspection for a `PluginDbusHost` object reflects the generic host interface, not per-plugin method shapes. Per-plugin method shapes are the exclusive domain of `PluginSchema.methods`.

This is intentional: D-Bus introspection is a read-time reflection of what is mounted, but `PluginSchema` is the write-time authority that determines what gets mounted and what gets dispatched.

---

### Layer 5 — `/dev/shm/live-schema.json`

The `SchemaEngine` writes this file. Format:

```json
{
  "plugin_name": [
    { "name": "plugin_name", "version": "1.0.0", "fields": {...}, "methods": {...}, ... }
  ]
}
```

The array wrapper is for versioned history — `grpc_server.rs` reads `entries[0]` (or the value directly if it's not an array). `UnixSocketPlugin::read_desired()` reads `catalog["unix_socket"].as_array()?.last()?.get("example")`.

This file is the runtime read-path for:
- `grpc_server.rs` capability enforcement (`method_capability_for_plugin`)
- `plugin_info_from_schema` (PluginInfo listing for gRPC)
- Any plugin that reads its own schema-declared desired state (zero-copy Sled pattern)

---

### Layer 6 — `build.rs` Proto Generation

At `cargo build` time for `op-grpc-bridge`:

```
DefaultPluginRegistry::available_plugins()
        │
        │  For each plugin, load_plugin() → plugin.schema()
        │
        ▼
collect_plugin_methods() → Vec<PluginMethodSet>
        │
        ├── generate_plugin_methods_proto() → plugin_methods.proto
        │       Per plugin: service <Plugin>PluginMethods { rpc <Method>(Struct) returns (Struct) }
        │       All I/O typed as google.protobuf.Struct at this layer
        │
        └── generate_plugin_method_routes() → plugin_method_routes.rs
                Per plugin: impl <Plugin>PluginMethods for OperationGrpcServer
                Dispatches to self.call_generated_plugin_method(plugin_id, method_name, request)
```

Then `tonic_build::configure()` compiles `plugin_methods.proto` (+ all static domain protos) into Rust code and writes the combined `operation_descriptor.bin` (`FileDescriptorSet`).

**Key constraint**: `tonic_build` runs `protoc` or `prost-build`. The build-time proto uses `Struct` I/O because the method argument types are `OwnedValue` (runtime JSON). Converting `OwnedValue` JSON Schema to proto field types at `build.rs` time is the job of `plugin_grpc_gen.rs` at runtime — it is correct to keep this separation.

---

### Layer 7 — Runtime Typed Descriptor Freeze

Before mounting tonic-reflection routes, `OperationGrpcServer::freeze_plugin_method_reflection()` runs:

```
For each plugin from plugin_provider.list_plugins():
    get_schema(plugin_id) → schema_json from /dev/shm/live-schema.json
    serde_json::from_str::<PluginSchema>(&schema_json)
    register_plugin_methods(plugin_id, &schema)
        │
        └── PerMethodGrpcServices::create_frozen_services(plugin_id, schema)
                For each MethodDecl:
                    generate_method_file_descriptor_proto(plugin_id, method_name, method_decl)
                    → FileDescriptorProto with typed Input/Output messages
                    (field types derived from MethodDecl.args/returns JSON Schema properties)
```

After `freeze_plugin_method_reflection()`, `generate_combined_reflection_descriptor()` merges:
- `operation_descriptor.bin` (compile-time: static domain protos + Struct-typed plugin routes)
- `PerMethodGrpcServices::frozen_descriptor_set_bytes()` (runtime: typed plugin method descriptors)

The merged `Vec<u8>` is leaked to `'static` and passed to `tonic_reflection::server::Builder`. This is the snapshot. **It cannot be mutated after `build_operation_routes()` is called.**

#### Why two descriptor layers?

| Layer | When generated | I/O types | Purpose |
|---|---|---|---|
| `operation_descriptor.bin` | Cargo build | `google.protobuf.Struct` | Rust trait dispatch routes (type-checked at compile time) |
| `PerMethodGrpcServices` bytes | Runtime, before route mount | Typed fields from JSON Schema | Client reflection (grpcurl, MCP clients can see field names and types) |

Both must be present for the system to work correctly. Clients that call methods get type-safe Rust dispatch. Clients that `describe` methods via reflection get real field types.

---

### Layer 8 — tonic-reflection

The gRPC Server Reflection Protocol (standardized in `grpc/reflection/v1/reflection.proto`) exposes a bidirectional streaming RPC `ServerReflectionInfo`. Clients send `ServerReflectionRequest` messages (list services, get file by name, get file containing symbol) and receive `ServerReflectionResponse` messages carrying `FileDescriptorProto` bytes.

`tonic-reflection` implements this protocol. The `Builder::register_encoded_file_descriptor_set(bytes)` call accepts an encoded `FileDescriptorSet` and provides it to all queries. Both `build_v1()` and `build_v1alpha()` are mounted (v1alpha for compatibility with older `grpcurl` and Postman versions).

**Reflection completeness gate**: after `freeze_plugin_method_reflection()`, every plugin that has `!schema.methods.is_empty()` must have a corresponding service visible in the descriptor. This can be verified with:

```sh
grpcurl -plaintext 127.0.0.1:50051 describe | grep -E "operation\.(method|plugin)"
```

---

### `plugin_schema_defs.rs` Role — Clarified

The AGENTS.md rule "Never define a schema inline in a plugin's own file — it will not be registered" refers to defining schemas outside the plugin's own module. The current `plugin_schema_defs.rs` is a pure re-export aggregator — this is correct. It does NOT define schemas.

The utility helpers it provides (`method_decl_from_schemars_with_output`, `AckOutput`, `EmptyInput`, `materialize_state_from_schema`, `cap_method`, `simple_schema`, etc.) are legitimate shared tooling. They live here because they are schema-construction helpers that every plugin uses.

**What belongs in `plugin_schema_defs.rs`:**
- Re-exports of `<plugin>_schema` functions
- Helper functions for building `PluginSchema` and `MethodDecl` objects
- `AckOutput`, `EmptyInput`, shared IO structs

**What does NOT belong there:**
- Any `fn some_plugin_schema() -> PluginSchema { ... }` body that constructs fields for a specific plugin

---

### Architecture Decision: Should `build.rs` Generate Typed Proto Messages?

The current design keeps build-time proto as `Struct`-typed and runtime descriptor as field-typed. This is the right call because:

1. **Build-time type-checking is at the Rust level**, not at the proto level. The `impl <Plugin>PluginMethods for OperationGrpcServer` dispatches to `call_generated_plugin_method` which handles `prost_types::Struct` and converts via simd_json. The Rust type safety is enforced by the `MethodDecl` schema, not by proto field types.

2. **Runtime reflection needs real field types** for tools like `grpcurl` and the MCP gateway to know what fields to send. This is provided by `PerMethodGrpcServices`.

3. **Attempting to generate typed proto from `OwnedValue` JSON Schema at `build.rs` time** would require replicating `plugin_grpc_gen.rs`'s JSON Schema → proto descriptor logic in build.rs. This would create a maintenance burden and a second divergeable path. The current single-authority runtime path is correct.

**Decision: keep the two-layer approach. Do not merge them.**

---

### Migration Path (Plugin by Plugin)

#### Tier B — Methods Only (state struct not yet schemars-derived)

Example: `wireguard.rs` uses `simple_schema()` + `any_field()` for state but has typed method input structs.

Steps:
1. Add `schemars::JsonSchema` to `WireGuardState`, `WireGuardInterface`, `WireGuardPeer`
2. Add `#[schemars(extend("x-oscal-subid" = "sch.network.plugin.wireguard.schema@v1"))]` to `WireGuardState`
3. Add per-field `#[schemars(description = "...", extend("x-oscal-subid" = "..."))]` annotations
4. Replace `wireguard_schema()` body: call `plugin_schema_from_json` with `schema_for!(WireGuardState)`
5. Call `apply_state_defaults` with `WireGuardState::default()`
6. Re-insert methods (unchanged — already use `method_decl_from_schemars`)
7. Write `#[test] fn derived_schema_matches_hand_rolled()` using `schema_diffs()`
8. Delete the hand-rolled field construction
9. Run `cargo test -p op-plugins` — green

#### Tier C — Legacy (`any_field` / anonymous JSON throughout)

Steps 1-6 above, plus:
- Define the state struct from scratch with appropriate field types
- The "hand-rolled reference" for the drift test is the old schema — compare, accept deliberate differences, update test

---

### File Ownership Summary

| File | Purpose | Owner |
|---|---|---|
| `crates/op-plugins/src/state_plugins/<plugin>.rs` | State struct, method input/output structs, `<plugin>_schema()`, `inventory::submit!` | Plugin |
| `crates/op-plugins/src/state_plugins/plugin_schema_defs.rs` | Re-export aggregator + shared helpers | Infrastructure |
| `crates/op-plugins/src/state_plugins/schemars_adapter.rs` | JSON Schema → `PluginSchema` translator | Infrastructure |
| `crates/op-state-store/src/plugin_schema.rs` | `PluginSchema`, `MethodDecl`, `FieldType`, etc. | Infrastructure |
| `crates/op-plugins/src/registry.rs` | D-Bus object export, `SchemaCatalog` write | Infrastructure |
| `crates/op-grpc-bridge/build.rs` | Build-time proto + route generation | Infrastructure |
| `crates/op-grpc-bridge/src/plugin_grpc_gen.rs` | Runtime typed descriptor generation | Infrastructure |
| `crates/op-grpc-bridge/src/grpc_server.rs` | Descriptor freeze + tonic-reflection mount | Infrastructure |
| `/dev/shm/live-schema.json` | Runtime canonical read | SchemaEngine writes, all consumers read |
