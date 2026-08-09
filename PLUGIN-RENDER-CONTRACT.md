# Plugin Render Contract — what every plugin needs to render properly

Verified against the live pipeline 2026-08-07 (`antigravity_chat`, schema_hash
`51f64efb…`, served complete through `/api/ui-model/plugin-schema/`). This is
the authoring contract for making any plugin renderable by the UI-model loop
(`op-gallery-gen` model-agnostic inference → json-render Spec → SPA) and by
schema-driven forms.

## The golden rule

**The render source is the sealed blob's `schema_json` — never gRPC reflection
descriptors.**

| Consumer path | What it gets |
|---|---|
| `op_blob::catalog::read_plugin_schema_shm(id)` (on-host, e.g. `op-gallery-gen` context assembler) | FULL schema |
| `GET /api/ui-model/plugin-schema/:plugin` (op-web, remote agents) | FULL schema + `schema_hash` |
| `GET /api/ui-model/plugins` | every sealed plugin id (state-only plugins included) |
| `PluginService.GetSchema` (gRPC) | FULL `schema_json` verbatim |
| gRPC reflection descriptors | **methods only**, type-skeleton: no state fields at all, enums/unions/maps → `string`, constraints/descriptions/defaults/subids dropped |

Reflection descriptors exist to encode RPC bytes (`DynamicMessage`). Any
renderer or model that reads them instead of the schema will see a "collapsed"
plugin. Don't fix that by enriching descriptors — point the consumer at the
schema.

## Authoring template

One file: `crates/op-plugins/src/state_plugins/<name>.rs`. The struct IS the
schema (`docs/schema-from-structs.md`); never hand-build `FieldSchema` maps or
hand-write `serde_json::json!` schemas.

```rust
use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

/// Doc comment → rendered description of the nested section.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.<name>.widget.schema@v1"))]
pub struct Widget {
    /// Doc comment → field label/tooltip in the UI.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.<name>.widget-id@v1"))]
    pub id: String,

    /// Constraints render as validation (and sliders where sensible).
    #[serde(default)]
    #[schemars(range(min = 1, max = 65535),
               extend("x-oscal-subid" = "mut.software.<name>.widget-port@v1"))]
    pub port: u16,

    /// Option<T> renders as an optional field, not a union.
    pub note: Option<String>,
}

/// Top-level state struct: its top-level properties become the plugin's fields.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.software.plugin.<name>.schema@v1"))]
#[schemars(extend("x-oscal-category" = "<category>"))]   // what this plugin IS
pub struct MyPluginState {
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "obs.software.<name>.status@v1"))]
    pub status: String,
    /// Vec<struct> renders as a table; nested structs as sections/cards.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "exp.software.<name>.widgets@v1"))]
    pub widgets: Vec<Widget>,
}

pub struct MyPlugin;
impl MyPlugin { pub fn new() -> Self { Self } }
impl Default for MyPlugin { fn default() -> Self { Self } }

#[async_trait]
impl StatePlugin for MyPlugin {
    fn name(&self) -> &str { "<name>" }
    fn version(&self) -> &str { "1.0.0" }
    fn schema(&self) -> Option<PluginSchema> { Some(my_plugin_schema()) }
    // calculate_diff / apply_state / verify_state / create_checkpoint /
    // rollback / capabilities: copy the declarative no-op shape from
    // crates/op-plugins/src/state_plugins/antigravity_chat.rs
}

pub(crate) fn my_plugin_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(MyPluginState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "<name>", "1.0.0", "<one-line description — shown as the plugin's category line>",
        &root,
    );

    use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, EmptyInput};
    use op_state_store::SideEffect;

    // EVERY method: dedicated typed Input + Output structs. No AckOutput for
    // new methods; no hand-written json! schemas. EmptyInput for no-arg reads.
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ListWidgetsOutput { pub widgets: Vec<Widget> }

    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigureInput {
        /// Optional fields (`Option<T>` + serde default) render as optional form inputs.
        #[serde(default)] pub port: Option<u16>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ConfigureOutput { pub widget: Widget }

    schema.methods.insert(
        "list_widgets".to_string(),
        method_decl_from_schemars_with_output::<EmptyInput, ListWidgetsOutput>(
            "list_widgets",
            SideEffect::Read,      // Read → safe refresh; Mutation → confirm + accountability
            true,                  // idempotent
            "<name>.read",         // required_capability: <plugin>.read / <plugin>.write
            "obs.software.<name>.widgets.list@v1",   // subid — taxonomy + registry mandatory
        ),
    );
    schema.methods.insert(
        "configure".to_string(),
        method_decl_from_schemars_with_output::<ConfigureInput, ConfigureOutput>(
            "configure", SideEffect::Mutation, false,
            "<name>.write", "mut.software.<name>.config.set@v1",
        ),
    );

    schema
}

// Self-registration — no central dispatch list.
inventory::submit! {
    crate::default_registry::PluginReg::new("<name>", |_ctx| std::sync::Arc::new(MyPlugin::new()))
}
```

## Render contract — schema element → UI behavior

What the adapter (`schemars_adapter.rs`) extracts and what renderers do with it.
If you want the UI behavior in the left column, you must author the thing in
the right column.

| Renders as | Author in Rust |
|---|---|
| Field label + tooltip | `/// doc comment` on the field |
| Section/card description | `/// doc comment` on the struct |
| Dropdown (`enumValues`) | plain Rust `enum` (unit variants) or `#[schemars(enum ...)]` — `FieldType::Enum` |
| Discriminated-union switcher | `#[serde(tag = "...")]` enum → `FieldType::OneOf` (all variants render) |
| Optional input | `Option<T>` (+ `#[serde(default)]`) |
| Validation / slider bounds | `#[schemars(range(min = …, max = …))]` → `Constraint::Min/Max`; emit the `pattern` keyword (e.g. `#[schemars(extend("pattern" = "…"))]`) → `Pattern` |
| Initial value | `#[serde(default)]` (+ `Default` impl) → `default` |
| Example placeholder | `#[schemars(example = …)]` → `example` |
| Required-field marker | non-`Option` field listed in `required` |
| Display-only field | `#[schemars(extend("readOnly" = true))]` (see `procfs.rs`) |
| Table | `Vec<Struct>` (nested `$defs` are resolved inline — full column schema) |
| Nested section | nested struct field (`FieldType::Object`, recursive) |
| Generated method form | `MethodDecl.args` — full JSON Schema of the typed Input struct |
| Result view | `MethodDecl.returns` — typed Output struct (never `None`, never bare `{"type":"object"}`) |
| Safe "Refresh" vs guarded "Apply" | `SideEffect::Read` vs `SideEffect::Mutation` + `idempotent` |
| Audit/identity binding | `x-oscal-subid` on every field + struct + method (`subids` map, method `subid`) |
| Immutable-path guard | struct-level `#[schemars(extend("x-immutable-paths" = [...]))]` |

Auto-derived (do NOT author manually): `actor_id`/`capability_id` fields appear
whenever any `mut.*` subid exists; `source_system`/`source_locator` for `src.*`;
`__schema__` subid is derived if the struct-level one is missing. The adapter
applies `ensure_category_metadata_fields` itself.

## Subid + capability rules (CI-enforced)

- Subid format: `<category>.<component-type>.<subject>.<verb>[@vN]` — exactly
  the seven categories (`src prj sch mut obs evt exp`), OSCAL component types.
- Register every subid in
  `crates/op-plugins/src/state_plugins/oscal_subid_registry.rs`.
- Capabilities follow `<plugin>.read` / `<plugin>.write` (see PLUGIN-METHOD-SPEC.md).
- `mut.*` methods must carry actor/capability accountability — declaring the
  subid is what triggers the auto-fields; don't skip it.

## What the model/producer sees (json-render loop)

The producer reads the FULL `PluginSchema` and emits a Spec:

```json
{ "root": "card-1",
  "elements": {
    "card-1": { "type": "PluginStateCard", "props": { "pluginId": "...", "title": "..." }, "children": ["sf-0"] },
    "sf-0":   { "type": "SchemaField", "props": { "name": "...", "fieldType": "string",
                 "description": "...", "readOnly": false, "required": false, "enumValues": [] },
                 "children": [] }
  } }
```

`SchemaField` props map 1:1 to `FieldSchema` — a field with an empty
description, no constraints, and no subid renders as a bare unlabeled input.
Schema quality IS render quality.

## Static linter / complete-plugin emitter (`op-plugin-lint`)

File in / file out — no paste UX. Primary mode: read a plugin `.rs` and emit a
**full contract-shaped plugin document** (identity, state fields, typed methods
with args/returns, audit findings). With `--introspect`, also include upstream
gap findings (what introspection found that the plugin does not have).

```bash
# Complete plugin document (default --format complete)
cargo run -p op-plugin-lint -- \
  --input crates/op-plugins/src/state_plugins/zeroclaw.rs \
  --output /tmp/zeroclaw.complete.json \
  --format complete \
  --introspect /path/to/upstream/repomix-output.xml

# Markdown form of the same
cargo run -p op-plugin-lint -- \
  --input crates/op-plugins/src/state_plugins/zeroclaw.rs \
  --output /tmp/zeroclaw.complete.md \
  --format complete \
  --introspect /path/to/upstream/repomix-output.xml

# Lint findings only
cargo run -p op-plugin-lint -- \
  --input path/to/plugin.rs --output /tmp/plugin.lint.md --format md

# Reviewable Rust candidate from Inspector Gadget + Repomix gaps. The input is
# never overwritten; inferred ownership/types/side effects remain marked for review.
cargo run -p op-plugin-lint -- \
  --input crates/op-plugins/src/state_plugins/zeroclaw.rs \
  --output /tmp/zeroclaw.generated.rs \
  --format rust \
  --introspect /path/to/upstream/repomix-output.xml
```

`--format rust` preserves the input source and appends Schemars-derived candidate
fields plus dedicated method Input/Output types. Repomix proves that a surface
exists, but cannot prove plugin ownership, runtime dispatch, mutation semantics,
or exact Rust types. Generated candidates therefore must be promoted deliberately:
register owned fields/methods, choose `SideEffect`, implement dispatch, register
subids, and pass the verification checklist below before replacing a live plugin.

### `--introspect` (external discovery — prefer Repomix)

Discover what an **upstream** project exposes — not our sealed schema (circular).
Alias: `--intospec`.

**Preferred / universal path:** pack any upstream checkout with `repomix` at the
repo root, then point `--introspect` at `repomix-output.xml`. The linter walks
`<file path="…">` entries and identifies **schema-convertible structured data**:

| Kind | Sources | Path prefix |
|---|---|---|
| Rust | `.rs` structs/enums | `struct.` / `enum.` |
| TOML / YAML / JSON | `.toml` `.yml` `.yaml` `.json` | `toml.` `yaml.` `json.` |
| OpenAPI / JSON Schema / Avro | sniffed from YAML/JSON | `openapi.` `jsonschema.` `avro.` |
| SQL | `.sql` `CREATE TABLE/TYPE/VIEW` | `sql.` |
| Protobuf / GraphQL / Prisma | `.proto` `.graphql` `.prisma` | `proto.` `graphql.` `prisma.` |
| Thrift / Cap'n / FlatBuffers / XSD / CSV | `.thrift` `.capnp` `.fbs` `.xsd` `.csv` | matching prefix |

Works for ZeroClaw, Antigravity SDKs, or any upstream tree — one workflow.

| Target | Behavior |
|---|---|
| `repomix-output.xml` (or any Repomix pack) | **primary** — source-level struct/enum catalog |
| prior `.json` surface dump | reload element paths from a previous run |
| path to a CLI binary | secondary — recursive `--help` walk; `--ssh host` if remote |
| `binary:/path/to/tool` | same, explicit |
| `gcloud` | shallow `gcloud --help` GROUPS |
| plugin id | **drift only** (warns) — fetches sealed schema we already authored |

```bash
# Universal: pack upstream source, then discover
cd /path/to/upstream && repomix
cargo run -p op-plugin-lint -- \
  --introspect /path/to/upstream/repomix-output.xml \
  --surface-out /tmp/upstream.surface.json

# Diff that surface against our plugin authoring
cargo run -p op-plugin-lint -- \
  --input crates/op-plugins/src/state_plugins/zeroclaw.rs \
  --output /tmp/zeroclaw.lint.md \
  --introspect /home/jeremy/zeroclaw/repomix-output.xml \
  --surface-out /tmp/zeroclaw.repomix.surface.json

# Optional: CLI help-walk when you only have a binary
cargo run -p op-plugin-lint -- \
  --introspect /fast/zeroclaw/bin/zeroclaw \
  --ssh root@192.168.1.1 \
  --introspect-depth 2 \
  --surface-out /tmp/zeroclaw.cli.surface.json
```

`--surface-out` writes the discovery catalog. Path vocabularies differ from
schemars fields (`struct.zeroclaw_config.V1Config.default_model` vs
`selected_model`) — unmapped external paths are WARN until an explicit mapping
layer exists. Use the surface JSON as the authoritative catalog.

Exit code `0` = no FAIL findings (WARN/HINT allowed); `1` = at least one FAIL.

## Cross-plugin model catalogs (Antigravity)

Antigravity product plugins (`antigravity`, `antigravity_chat`) must **not** embed a
Gemini `models[]` catalog. They hold:

| Field | Role |
|---|---|
| `llm_plugin` | Plugin that owns the catalog (default `large_language_model`) |
| `provider_route` | Provider id/route on that plugin (default `gemini`) |
| `selected_model` | Session preference only; empty = provider default |

Render model pickers by resolving `llm_plugin` + `provider_route` (e.g.
`list_models` / `list_providers` on `large_language_model`), not from local
state. ZeroClaw is a router/orchestrator — do not treat it as the default model
surface. `op-plugin-lint` classifies Gemini CLI/SDK paths as `delegated_gemini`
when auditing antigravity* sources.

## Verification checklist (per plugin)

1. Run `op-plugin-lint --input <file> --output <report>` (above) — clear FAILs.
2. `cargo test -p op-plugins <name>` — schema seeds + subid validity tests
   (copy the two tests from `antigravity_chat.rs`).
3. `cargo build -p op-grpc-bridge` — regenerates `plugin_methods.proto`;
   methods appear on the gRPC diagnostic page iff `schema.methods` non-empty
   AND the blob is sealed.
4. On the host after seal:
   - `GET /api/ui-model/plugins` → id present.
   - `GET /api/ui-model/plugin-schema/<name>` → verify: `fields` tree complete,
     every method's `args`/`returns` contain the real JSON Schema (`title`,
     `properties`, constraints), `subids` map populated, `schema_hash` present.
5. If something is missing at the endpoint but present in Rust: the blob is
   stale — reseal; the endpoint reads only the sealed catalog.

## Failure modes that look like "the schema collapsed"

| Symptom | Actual cause |
|---|---|
| No state fields visible at all | consumer is reading gRPC reflection (descriptors carry methods only) |
| enum/oneOf became `string` | reflection path, or non-string enum variants (adapter keeps only string values) |
| min/max/format/descriptions missing | reflection path — descriptors have no slot for them |
| Method form is one blob field | method authored with hand-written `json!` args instead of a typed schemars Input |
| Plugin missing from method index / diagnostic page but state renders | plugin has zero methods — fine for state-only, add methods when it needs controls |
| Old fields at the endpoint | stale sealed blob — reseal, check `schema_hash` against the manifest |
