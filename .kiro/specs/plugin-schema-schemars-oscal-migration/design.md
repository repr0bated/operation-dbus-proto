# Design — plugin-schema-schemars-oscal-migration

## 1. Crate Placement

```
crates/
  op-plugins/
    src/state_plugins/
      schemars_adapter.rs          ← extended to read OSCAL subids and recursive diffs
      plugin_schema_defs.rs          ← re-exports derived schemas; no hand-rolled aliases
      oscal_subid_registry.rs      ← typed OscalSubidRegistryEntry, derived schema
      unix_socket.rs                ← retrofitted with OSCAL annotations
      cron.rs                       ← typed CronJob, CronSchedules, CronConfig
      zeroclaw.rs                   ← typed ZeroclawProjection + plugin-specific fields
      antigravity.rs                ← reuses projection + adds SDK/Auth/Usage fields
      antigravity_chat.rs           ← typed chat config
      common/
        llm_projection.rs           ← shared providers, model_routes, router, tools, etc.
        oscal.rs                    ← subid validation helpers and constants
      adc.rs, mcp.rs, ...           ← 14 mechanical conversions
      lxc.rs, procfs.rs, ...        ← last: structs authored from existing schemas
  op-grpc-bridge/
    src/server.rs                   ← unchanged; consumer of zeroclaw schema
    src/schema_loader.rs            ← unchanged; reads /dev/shm/opdbus/schemas/zeroclaw.json
  deploy/s6/                        ← unchanged service definitions
```

No `src/` at the workspace root. No hand-rolled schema definitions outside test modules.

---

## 2. Schema Flow

```
┌─────────────────────────────────────┐
│  Plugin state structs (typed)         │  ← single source of truth
│  #[derive(JsonSchema)]                │
└────────────┬────────────────────────┘
             │ serde_json + schemars
             ▼
┌─────────────────────────────────────┐
│  schemars_adapter                   │  ← resolves $refs, maps types, constraints,
│  plugin_schema_from_json()          │    descriptions, defaults, examples, readOnly,
│                                     │    immutable_paths, x-oscal-subid
└────────────┬────────────────────────┘
             │
             ▼
┌─────────────────────────────────────┐
│  PluginSchema (op-state-store)        │  ← includes subids HashMap
│  plugin.schema()                      │
└────────────┬────────────────────────┘
             │ D-Bus projection / SHM
             ▼
┌─────────────────────────────────────┐
│  op-grpc-bridge (for zeroclaw)       │  ← reads /dev/shm JSON, serves RPCs
│  MCP tools / UI renderers           │
└─────────────────────────────────────┘
```

**Btrfs is never in the schema flow.** The `/dev/shm/opdbus/schemas/zeroclaw.json` file remains tmpfs-only for the Axum host.

---

## 3. schemars_adapter Extensions

### 3a. Recursive `schema_diffs`

The existing `schema_diffs` helper compares top-level fields only. It will be rewritten to recurse into `FieldType::Object` and `FieldType::Array` so that nested details (e.g., `SocketEndpoint` inside `sockets`) are compared with full fidelity.

```rust
#[cfg(test)]
pub(crate) fn schema_diffs(reference: &PluginSchema, derived: &PluginSchema) -> Vec<String>;
```

Empty result means the derived schema reproduces the reference exactly.

### 3b. OSCAL subid ingestion

The adapter currently reads `x-immutable-paths`. It will be extended to read:

- `x-oscal-subid` on the root JSON Schema object → stored in `PluginSchema.subids` under a reserved key (e.g., `""` or `"__schema__"`).
- `x-oscal-subid` on each property → stored in `PluginSchema.subids` under the field name.

Example struct annotation:

```rust
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.unix_socket.schema@v1",
                 "x-immutable-paths" = ["sockets"]))]
pub struct UnixSocketState {
    /// Declared unix socket endpoints.
    #[schemars(extend("x-oscal-subid" = "mut.service.unix_socket.bind@v1"))]
    pub sockets: Vec<SocketEndpoint>,
}
```

### 3c. Constraint and metadata mapping

Existing mappings remain:

| schemars / JSON Schema | `op_state_store` |
|---|---|
| `string` / `integer` / `number` / `boolean` | `FieldType::String/Integer/Float/Boolean` |
| `array` (`items`) | `FieldType::Array` (recursed) |
| `object` (`properties`, `$ref`/`$defs`) | `FieldType::Object` (recursed) |
| `enum` | `FieldType::Enum` |
| `minimum` / `maximum` / `pattern` | `Constraint::Min` / `Max` / `Pattern` |
| doc comment / `description` | `FieldSchema.description` |
| `required: []` | `FieldSchema.required` |
| `default` / `examples[0]` | `FieldSchema.default` / `.example` |
| `readOnly` | `FieldSchema.read_only` |
| `#[schemars(extend("x-immutable-paths" = [...]))]` | `PluginSchema.immutable_paths` |
| `#[schemars(extend("x-oscal-subid" = ...))]` | `PluginSchema.subids` |

---

## 4. OSCAL Subid Taxonomy and Validation

The taxonomy from AGENTS.md §4a is enforced:

```
<category>.<component-type>.<subject>.<verb>[.<facet>][@vN]
```

Seven categories: `src`, `prj`, `sch`, `mut`, `obs`, `evt`, `exp`.
Component types reuse OSCAL vocabulary: `software`, `service`, `network`, `hardware`, `process-procedure`, `standard`, `validation`, `policy`, `plan`, `guidance`, `physical`, `this-system`, `system`, `interconnection`.

Validation helpers in `crates/op-plugins/src/state_plugins/common/oscal.rs`:

```rust
pub fn validate_subid(subid: &str) -> Result<(), SubidError>;
pub fn category_required_fields(category: &str) -> &'static [&'static str];
```

Tests assert that every subid registered in a schema passes validation and that required fields are present in the schema or in the registry entry.

---

## 5. Migration Sequencing

| Phase | Plugin(s) | Rationale |
|---|---|---|
| 0 | Adapter: recursive diff + OSCAL ingestion | Foundation for all conversions. |
| 1 | `oscal_subid_registry` | The registry itself must be schema-native before it can validate others. |
| 2 | `unix_socket` | Already converted; retrofit OSCAL annotations as the worked example. |
| 3 | `cron` | Smallest opaque plugin; proves the opaque-to-typed pattern. |
| 4 | `common::llm_projection` + `zeroclaw` + `antigravity` | Extract shared projection; both become typed together. |
| 5 | `antigravity_chat` | Typed chat config using `LocalAgentConfig`-like shapes from the downloaded SDK. |
| 6 | 14 mechanical candidates | Structs already mirror schema; golden-reference tests are one-liners. |
| 7 | 5 no-struct plugins (`lxc`, `procfs`, `web_ui`, `notebooklm`, `oscal_subid_registry` already done) | Author structs from existing schemas; highest risk, last. |

The 14 mechanical candidates identified in the transcript: `adc`, `mcp`, `compact_mcp`, `cognitive_mcp`, `agent_config`, `keypair`, `endpoint`, `net`, `hardware`, `software`, `sessdecl`, `gcloud_adc`, `config`, `ctl_plane_chatbot`.

---

## 6. Shared LLM Projection Module

```rust
// crates/op-plugins/src/state_plugins/common/llm_projection.rs

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.llm-projection.schema@v1"))]
pub struct LlmProjection {
    #[schemars(extend("x-oscal-subid" = "src.software.llm-projection.providers@v1"))]
    pub providers: Vec<Provider>,
    #[schemars(extend("x-oscal-subid" = "sch.software.llm-projection.model-routes@v1"))]
    pub model_routes: Vec<ModelRoute>,
    #[schemars(extend("x-oscal-subid" = "mut.service.llm-projection.route@v1"))]
    pub router: Router,
    #[schemars(extend("x-oscal-subid" = "exp.service.llm-projection.tools@v1"))]
    pub tools: Vec<LlmTool>,
    pub config_schema: ConfigSchema,
    pub ui_surfaces: Vec<UiSurface>,
    pub structured_output: StructuredOutput,
}
```

`zeroclaw.rs` and `antigravity.rs` embed `LlmProjection` and add plugin-specific fields. The golden-reference tests compare the combined derived schema to each plugin's current hand-rolled schema.

---

## 7. Dependency Additions

Only `schemars = "1"` is already present in `crates/op-plugins/Cargo.toml`. No new runtime dependencies are required for the migration. The downloaded SDK and cloned zeroclaw source are reference material only and must not be added to `Cargo.toml`.

---

## 8. Integration with the Zeroclaw Axum Host

The existing `.kiro/specs/zeroclaw-host-axum-schema-kiro` spec remains valid. The migration only affects how the schema is authored inside `op-plugins`. Verification:

1. `zeroclaw.rs` continues to write `/dev/shm/opdbus/schemas/zeroclaw.json` on startup.
2. The serialised JSON still round-trips to `PluginSchema`.
3. `op-grpc-bridge` `GetSchema` returns the migrated schema.
4. No host-side changes are required.
