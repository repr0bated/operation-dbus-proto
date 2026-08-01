# Requirements: unified-blob-catalog-mcp (Kiro re-derivation)

## Purpose

Extend the schema-driven chat pipeline so a single request can draw context
from multiple sources — cross-plugin blob data, semantic search over all plugin
schemas, and live D-Bus state — while keeping every request stateless (no
turn-to-turn memory).

## Verified Baseline

### What exists today

**SHM blob catalog reads** (`crates/op-blob/src/catalog.rs`):
- `read_plugin_schema_shm(plugin_id: &str) -> Option<PluginSchema>` — reads one
  plugin's sealed `PluginSchema` from `/dev/shm/opdbus/plugin-blobs/`.
- `read_manifest_plugin_ids_shm() -> Option<Vec<String>>` — enumerates every
  active plugin id from the same directory.
- Both return `Option`, not `Result`; callers must handle `None` gracefully.

**Unified catalog MCP tool** (`crates/op-cognitive-mcp/src/blob_catalog_tool.rs`):
- `BlobCatalogTool` (tool name `blob_catalog`) walks `read_manifest_plugin_ids_shm()`,
  serializes each `PluginSchema` via `read_plugin_schema_shm`, and returns a
  single JSON payload `{plugin_count, plugins: {id: schema}, missing_schemas}`.
- Registered in `CognitiveToolRegistry::register_all` with no parameters —
  always returns the full catalog.

**Schema embedding renderer** (`crates/op-cognitive-mcp/src/qdrant_shuttle.rs:498`):
- `fn render_schema_embedding_text(schema: &PluginSchema) -> String` — private
  function, renders name/category/version/description/tags/immutable_paths/
  dependencies and all fields (with constraints) into deterministic retrieval
  text. This is the canonical embedding surface; it does not include
  `DESCRIPTOR_SET` data, which is correct per the established SCHEMA_JSON-only
  convention.
- `current_schema_embedding_text()` calls it against the single sled-resident
  schema only — never against the full catalog.

**Qdrant collections today** (`qdrant_shuttle.rs:19-21`):
- `ctl_plane_reasoning_episodes` (`COGNITIVE_MCP_QDRANT_COLLECTION`) — reasoning
  trace episodes.
- `user_memory` (`COGNITIVE_MCP_USER_MEMORY_COLLECTION`) — per-container user
  memory.
- Neither collection is appropriate for "one embedding per plugin blob."

**Qdrant client API** (`QdrantSemanticShuttle`, `qdrant_shuttle.rs:36-318`):
- `embed_document(text) -> Vec<f32>` and `embed_query_text(text) -> Vec<f32>` 
  are public and generic — reusable outside the trace/user-memory paths.
- `upsert_user_memory` / `search_user_memory` show the established
  upsert-then-query pattern. Point IDs are `String` (passed as
  `point_id.into()` and constructed from `uuid::Uuid::new_v4().to_string()`).
- Voyage model: `voyage-4` at 1024 dimensions (`DEFAULT_VOYAGE_QUERY_MODEL`,
  `DEFAULT_VOYAGE_OUTPUT_DIMENSION`).

**Live projection** (`op-web/src/handlers/zeroclaw.rs:309`):
- `get_projection(&state.projection_cache, "zeroclaw")` reads live D-Bus plugin
  state with a SHM fallback. The projection carries `providers`, `model_routes`,
  `structured_output`, and `tools` that the sealed `PluginSchema` does not.

**Dependency graph** (`op-state-store/src/plugin_schema.rs:196-198`):
- `PluginSchema.dependencies: Vec<String>` — declared, populated, but never
  traversed programmatically. The field is included in `render_schema_embedding_text`.

**Tool registry** (`cognitive_tools.rs:21-36`):
- `CognitiveToolRegistry::register_all(registry, store, qdrant)` accepts
  `qdrant: Option<Arc<QdrantSemanticShuttle>>`. Adding a new Qdrant-dependent
  tool follows the `MemoryTool::new(store, qdrant)` pattern: hold `Option<Arc<…>>`
  and return an error result (not a panic) when `None`.

**`op-cognitive-mcp` dependency graph** (`Cargo.toml`):
- Already depends on `op-blob` (workspace), `qdrant-client = "1.17"`, and
  `reqwest` — no new crate dependencies are needed for anything in this spec.

### What is missing

1. No Qdrant collection stores plugin blob embeddings. The two existing
   collections are for different purposes and must not be reused.
2. `render_schema_embedding_text` is never called outside the single
   sled-schema path; no code iterates all plugin ids and embeds them.
3. No explicit rebuild path exists. Historical embeddings were lost because
   nothing triggers a fresh upsert. Rebuilding must be explicit and
   user-triggered, not automatic.
4. `PluginSchema.dependencies` is populated but never walked to pull adjacent
   plugin schemas into context.
5. No UI affordance exists to select which context sources a given chat request
   uses.

## Requirements

### R1 — Dedicated blob-vector Qdrant collection

A new collection named `blob_vectors` (default; overridable via
`COGNITIVE_MCP_BLOB_VECTORS_COLLECTION`, matching the existing
`COGNITIVE_MCP_QDRANT_COLLECTION` / `COGNITIVE_MCP_USER_MEMORY_COLLECTION`
naming pattern) holds one point per active plugin blob:

- Vector: `voyage-4` 1024-dim embedding of `render_schema_embedding_text(schema)`.
- Payload: `{ plugin_id: string, text: string }`. The rendered text is stored
  alongside the vector so inspection and debugging do not require a second SHM
  read.
- Point ID: derived deterministically from `plugin_id` (see Design) so
  re-running a refresh upserts in place rather than accumulating duplicates.

### R2 — Explicit, user-triggered rebuild

Building or rebuilding the collection is never implicit or automatic — no
background timer, no per-request auto-embed.

A single named action ("refresh blob vectors") re-reads
`read_manifest_plugin_ids_shm()`, embeds every plugin's current schema text,
and upserts all points. Points for plugin ids no longer in the manifest are
left to be overwritten on the next refresh (the corpus is <2MB total; a
wholesale replace-on-refresh is cheaper than staleness tracking).

### R3 — Chat context source selection

A chat request assembles context from a **base** read plus zero or more
optional layers:

- **Base** (always): `read_plugin_schema_shm(plugin_id)` — today's behavior.
  Cheap, always current, single-plugin.
- **MCP** toggle: invokes `blob_catalog` to pull every plugin's schema into
  context in one shot.
- **Related** toggle: for each plugin id already in context, reads its
  `PluginSchema.dependencies` and loads those plugin schemas too — one hop
  only (see R6).
- **Live** toggle: reads the live D-Bus projection (via `get_projection`)
  instead of or in addition to the sealed schema, per the existing zeroclaw
  fallback pattern.
- **Vectors** modifier: when on, embeds the current chat message as a query
  and ranks/filters whichever content-source layers are active against the
  `blob_vectors` collection. When no content-source toggle is on, Vectors
  alone queries the collection directly and returns the top-k matching plugin
  schemas.

Toggles are combinable with each other and with Vectors independently. Vectors
is a modifier, not a peer content source — it refines the set assembled by the
content-source toggles, or stands alone when none are active.

### R4 — Stateless request semantics

No context is carried between turns. Each request independently applies
whichever source layers are selected and assembles context from scratch.
The `blob_vectors` collection is persistent data infrastructure, not
conversation memory, and must not be used to carry state across turns.

### R5 — UI affordance

The chat UI exposes, in three visually distinct groups:

1. Content-source toggles: **MCP**, **Related**, **Live** (multiselect,
   any combination valid).
2. A separate **Vectors** modifier toggle (combines with any of group 1, or
   stands alone as a direct search when group 1 is empty).
3. A separate **"Refresh blob vectors"** action button (triggers R2; not a
   toggle; has no effect on which sources the next request uses).

### R6 — One-hop dependency traversal for "Related"

"Related" walks `PluginSchema.dependencies` exactly one level deep. The
newly-added schemas' own dependencies are not followed. One hop keeps
"Related" meaningfully narrower than "MCP" (the full dump) for any plugin
with a connected dependency chain.

## Out of Scope

- Recursive or configurable-depth dependency traversal (rejected; see R6
  rationale).
- Staleness detection via `schema_hash` (rejected; wholesale refresh is
  sufficient at <2MB corpus scale).
- Putting the refresh action anywhere other than `op-cognitive-mcp` (rejected;
  Qdrant + Voyage clients live there; `op-plugins` must not depend on runtime
  I/O pipelines).
- Including `DESCRIPTOR_SET` (proto binary) data in blob embeddings or the
  catalog payload (rejected; `SCHEMA_JSON` is the established rendering
  surface; descriptors have known fidelity loss for field/enum representation).
