# Design — Model-Agnostic Generative UI Gallery

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Antigravity Chat UI                           │
│  (DSL-rendered, same interpreter as gallery specs)              │
│  [Tier toggles] [Guidance input] [Generate] [Cancel]           │
└───────────────────────────┬─────────────────────────────────────┘
                            │ operator actions + guidance text
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Generation Harness                             │
│  (crates/op-gallery-gen or replaces op-gemma/ui_gallery.rs)     │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ Context      │  │ Inference    │  │ Validation +         │  │
│  │ Assembler    │  │ Loop         │  │ Gallery Admission    │  │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘  │
│         │                  │                     │              │
└─────────┼──────────────────┼─────────────────────┼──────────────┘
          │                  │                     │
          ▼                  ▼                     ▼
┌──────────────┐   ┌──────────────────┐   ┌──────────────────┐
│ Data Tiers   │   │ ZeroClaw         │   │ CatalogStore     │
│              │   │ /v1/chat/        │   │ (200 slots,      │
│ • SHM blobs  │   │ completions      │   │  stable core,    │
│ • MCP tools  │   │ (any model)      │   │  novelty tier)   │
│ • Qdrant     │   └──────────────────┘   └──────────────────┘
└──────────────┘
```

---

## Component Design

### 1. Context Assembler

Responsible for building the complete inference context for each generation turn. Runs once at session start, produces an immutable context snapshot for the run.

**Inputs:**
- Live sealed catalog (blob manifest → per-plugin `PluginSchema` JSON)
- Static instruction docs (versioned in repo at `docs/gallery-gen/`)
- json-render.dev catalog docs (component list, prop schemas, grammar)
- Operator tier toggles (MCP on/off, Qdrant on/off)
- Operator guidance text (from chat)

**Output:** A `GenerationContext` struct containing:

```rust
pub struct GenerationContext {
    /// The universal prompt — never changes.
    pub base_prompt: &'static str,
    /// Operator's additional guidance for this run.
    pub operator_guidance: String,
    /// System message with instructions + docs.
    pub system_message: String,
    /// Plugin schemas — inline JSON (baseline) or summary (when MCP is on).
    pub schema_payload: SchemaPayload,
    /// MCP tool definitions (empty vec when MCP is off).
    pub tools: Vec<ToolDefinition>,
    /// Catalog hash at context assembly time.
    pub catalog_hash: String,
    /// Number of empty novelty slots to fill.
    pub target_slots: usize,
}

pub enum SchemaPayload {
    /// Full inline schemas (baseline mode, or small catalogs).
    Inline(Vec<PluginSchemaJson>),
    /// Summary + MCP tool access (when catalog is too large for context).
    Summary { plugin_count: usize, categories: Vec<String>, sample_plugins: Vec<PluginSchemaJson> },
}
```

**Context budget strategy:**
- Models ≥32K tokens: inline all 64 plugin schemas + full docs
- Models 8K–32K: inline top-10 schemas by field count + summaries of the rest + MCP tools for on-demand access
- Models <8K: summary only + MCP tools mandatory (REQ-3.5)

The context assembler does NOT know what model is loaded. It builds both inline and summary variants; the inference loop picks based on ZeroClaw's reported context window (queried once at session start via `/v1/models`).

---

### 2. Inference Loop

Calls ZeroClaw's OpenAI-compatible endpoint iteratively until the gallery is full or max attempts exhausted.

**Per-slot flow:**

```
1. Build messages array:
   [system: instructions + docs + schema payload]
   [user: base_prompt + operator_guidance + "Generate spec {n} of {target}"]

2. POST /v1/chat/completions
   - model: (omitted — ZeroClaw uses its selected model)
   - response_format: { type: "json_object" } (if supported)
   - tools: [...] (if MCP/Qdrant enabled)
   - temperature: 0.9 (novelty > determinism)
   - max_tokens: 4096

3. Handle response:
   a. If tool_calls present → execute MCP/Qdrant tools, append results, re-call
   b. If content is valid JSON → validate as spec → admit or reject
   c. If content is not JSON → retry (up to 3 per slot)

4. Report to chat: "Spec {n} admitted" or "Spec {n} rejected: {reason}"
```

**Tool call handling (MCP/Qdrant):**

When the model emits tool calls, the harness:
1. Executes the tool (read-only catalog query or Qdrant search)
2. Appends the tool result as an assistant→tool message
3. Re-calls the model to continue generation

Tool call depth is capped at 5 per slot to prevent infinite loops.

**Parallelism:**

The loop MAY run up to 4 slots concurrently (configurable). Each slot has independent retry state. Results are collected and admitted sequentially to prevent signature-hash races.

---

### 3. Validation + Gallery Admission

Every candidate spec passes through:

```rust
pub fn validate_spec(spec: &Value, catalog: &ComponentCatalog) -> Result<(), Vec<ValidationError>> {
    let mut errors = vec![];

    // 1. Structure: root exists, elements is a map
    // 2. Root reference: root ID exists in elements
    // 3. Component types: every element.type is in the catalog
    // 4. Props: every element.props validates against the component's prop schema
    // 5. Children: every child ID resolves within elements (no dangling refs)
    // 6. Bind paths: syntactically valid JSON pointers
    // 7. No cycles: children graph is a DAG

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

On success:
- Compute structural signature hash
- Check for duplicates against existing gallery
- Admit as `Tier::Novelty` via `CatalogStore::admit()`
- If gallery is full, retire oldest novelty element first (FIFO)

On failure:
- Log errors
- Report to chat with the spec fragment and error details
- Increment retry counter for that slot

---

### 4. Antigravity Chat Integration

The gallery generation session is a mode within the existing Antigravity chat, not a separate surface.

**Session lifecycle:**

```
[Operator opens gallery-gen mode]
     │
     ▼
┌─ Chat displays ─────────────────────────────────────────┐
│ "Gallery generation session"                             │
│ Baseline: ON (64 plugins, 1847 fields, 312 methods)     │
│ MCP cross-discovery: [OFF] [toggle]                     │
│ Qdrant semantic search: [OFF] [toggle]                  │
│                                                         │
│ Current gallery: 142/200 slots filled (38 stable core)  │
│ Empty novelty slots to fill: 58                         │
│                                                         │
│ [text input: additional guidance]                        │
│ [Generate] [Cancel]                                     │
└─────────────────────────────────────────────────────────┘
     │
     │ operator types guidance + hits Generate
     ▼
┌─ Chat streams progress ─────────────────────────────────┐
│ ● Assembling context... (catalog_hash: a3f7c2...)       │
│ ● Starting generation (58 slots, model: <whatever>)     │
│ ✓ Spec 1/58 admitted: "network-topology-lens-001"       │
│ ✓ Spec 2/58 admitted: "privacy-chain-viewer-002"        │
│ ✗ Spec 3/58 rejected: unknown component "FancyChart"    │
│ ✓ Spec 3/58 retry admitted: "compliance-flow-003"       │
│ ...                                                     │
│ ● Complete: 55/58 filled, 3 failed permanently          │
│                                                         │
│ [session ended — memory cleared]                        │
└─────────────────────────────────────────────────────────┘
```

**Chat rendering:** The session UI is itself a json-render spec rendered by the same interpreter. The tier toggles, progress indicators, and generation log are DSL elements (`button`, `status_pill`, `label`, `repeat` over log entries).

---

### 5. Data Tier Implementation

#### Baseline (always on)

```rust
fn assemble_baseline(manifest: &BlobManifest) -> Vec<PluginSchemaJson> {
    manifest.plugin_ids()
        .filter_map(|id| op_blob::catalog::read_plugin_schema_shm(id).ok())
        .collect()
}
```

Static docs loaded from `docs/gallery-gen/`:
- `access-instructions.md` — how to read schemas, what fields mean
- `json-render-catalog.md` — component types, props, grammar
- `spec-grammar.md` — the SpecContract shape, patch ops, streaming

#### MCP Tools (optional)

When toggled on, the model receives these tool definitions:

| Tool | Description | Returns |
|------|-------------|---------|
| `list_plugins` | List all plugin IDs with category and field/method counts | `[{id, category, field_count, method_count}]` |
| `get_plugin_schema` | Full schema for one plugin by ID | `PluginSchema` JSON |
| `search_fields` | Find plugins with fields matching a type or name pattern | `[{plugin_id, field_name, field_type}]` |
| `search_methods` | Find methods by side-effect, capability, or input type | `[{plugin_id, method_name, side_effect, subid}]` |
| `search_subids` | Find plugins/fields by OSCAL subid category prefix | `[{plugin_id, field_or_method, subid}]` |
| `find_related` | Plugins sharing field types or nested struct shapes with a given plugin | `[{plugin_id, shared_types}]` |

All tools are read-only. They query the in-memory catalog snapshot taken at session start.

#### Qdrant Semantic Search (optional, requires MCP)

One additional tool when Qdrant is toggled on:

| Tool | Description | Returns |
|------|-------------|---------|
| `semantic_search` | Search vectorized schema fragments by natural language query | `[{plugin_id, fragment, domain_tag, score}]` |

Domain tags: `privacy-ops`, `network-engineering`, `compliance`, `accessibility`, `operations`, `development`, `security`, `identity`.

The Qdrant collection (`gallery-gen-schemas`) is populated by a background indexer that re-vectors whenever `catalog_hash` changes.

---

### 6. Static Documentation (Repo-Versioned)

Three files in `docs/gallery-gen/` provide the model's instruction context:

**`access-instructions.md`** (~2K tokens):
- What a `PluginSchema` is: fields (typed state), methods (typed RPC), subids (audit identity)
- What field types mean: `String`, `Number`, `Boolean`, `Array`, `Object`, `Enum`, `OneOf`
- What constraints mean: `min`/`max` → slider ranges, `pattern` → validation, `readOnly` → display-only
- What methods mean: `SideEffect::Read` → safe refresh, `SideEffect::Mutation` → guarded action
- How to bind: `"bind": "/field/path"` in spec props references live plugin state

**`json-render-catalog.md`** (~3K tokens):
- Every legal component `type` with prop schema (from `json_render` plugin's `ComponentDecl` list)
- Action types and their payloads
- Which components are `StableCore` (the ~40 the model must prefer for reliable rendering)

**`spec-grammar.md`** (~1K tokens):
- The flat-tree format: `{ "root": "id", "elements": { "id": { "type", "props", "children" } } }`
- Element fields: `type`, `props`, `children`, `visible`, `on`, `repeat`, `watch`
- Streaming patch ops for live updates
- What makes a spec valid (resolution rules, no cycles, no dangling refs)

---

## Removed / Replaced

| Current | Replacement |
|---------|-------------|
| `op-gemma/src/ui_gallery.rs` generator functions | Generation harness (inference loop) |
| `gemma_brain` as sole gallery writer | Any model via ZeroClaw |
| Hardcoded 40 generators | Model creativity bounded only by catalog |
| `/dev/shm/gemma-ui-specs.json` file output | Direct `CatalogStore::admit()` |
| `GemmaSpecGallery` / `GemmaSpecEntry` types | `Element` from `catalog/dsl.rs` (already the target) |

## Unchanged

| Component | Why |
|-----------|-----|
| `CatalogStore` (200 slots, tiers, versioning) | Already correct — generation fills it, doesn't redesign it |
| `interpret.rs` (DSL → egui) | Renderer is dumb; generation must target what it already supports |
| `dsl.rs` (Element, Tier, PinnedRef) | Spec shape is stable |
| `CatalogService` streaming | Distribution to GUI clients unchanged |
| Stable-core protection | 40 primitives remain immune to novelty churn |
| `SpecContract` validation | Already defined in the `json_render` plugin |
