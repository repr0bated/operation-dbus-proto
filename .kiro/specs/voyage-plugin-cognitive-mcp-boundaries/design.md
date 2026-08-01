# Design: voyage-plugin-cognitive-mcp-boundaries

## Resolved Open Questions (Evidence-Based)

The five open questions from the spec brief are resolved below. Each answer cites
the specific file evidence gathered during this session.

---

### Q1 — Should embedding_model absorb VoyageClient's HTTP logic, or stay generic?

**Answer: Stay generic. embedding_model is already correct. The duplicate moves to the executor.**

Evidence: `embedding_model.rs` already has `voyage_key_present()` (checks three env vars),
`config_state()` (reads provider/endpoint/model_id/dimensions from env), `live_state()` (sets
status=online iff key present, populates `available_models` with the VoyageModel::SHARED_SPACE trio),
and `VoyageModel` (the enum + shared-dim constant). That is the full config surface. The plugin's
role is schema + state mirror, not HTTP executor.

Absorbing `reqwest` calls into a plugin would violate the plugin contract: plugins are D-Bus
objects that declare state and accept mutations via `apply_state()`; they do not own runtime IO
pipelines. The correct direction is the inverse: `op-cognitive-mcp` reads its call parameters from
the plugin projection and executes the HTTP call itself.

---

### Q2 — Should rag-ingest and the rag/search MCP tool stop instantiating VoyageClient directly?

**Answer: Yes — but the executor stays in op-cognitive-mcp. Only the config source changes.**

Current state (the problem):

```
embedding_model.rs          voyage.rs             rag_pipeline.rs
─────────────────────       ─────────────────     ───────────────────
VOYAGE_PUBLIC_URL           VOYAGE_PUBLIC_URL      VOYAGE_PUBLIC_URL
VOYAGE_MONGODB_URL          VOYAGE_MONGODB_URL     VOYAGE_MONGODB_URL
voyage_key_present()        voyage_url_for_key()   voyage_url_for_key()  ← 3 copies
env var priority order      env var priority order env var priority order ← 3 copies
VoyageModel enum            (model via env only)   (model via env only)
```

Target state:

```
embedding_model.rs (op-plugins)       voyage.rs / rag_pipeline.rs (op-cognitive-mcp)
──────────────────────────────────    ──────────────────────────────────────────────
VOYAGE_PUBLIC_URL (only copy)         reads projection → {endpoint, model_id}
VOYAGE_MONGODB_URL (only copy)        constructs reqwest call with those params
voyage_key_present() (only copy)      fallback: reads env vars only if projection absent
VoyageModel enum (only copy)
```

The `reqwest` calls, `EmbeddingRequest`/`EmbeddingResponse` structs, Qdrant upsert logic, and
chunking pipeline all remain in `op-cognitive-mcp`. Only the config *source* changes.

---

### Q3 — Is cognitive_mcp.rs dead code (delete it) or a live seam (wire it)?

**Answer: Live seam — keep and wire it. It is not dead code.**

Evidence:

1. `cognitive_mcp.rs::apply_state()` writes real s6 env-dir files (`COGNITIVE_MCP_BIND`,
   `COGNITIVE_MCP_GRPC_BIND`, `WG_INTERFACE`, feature-flag env vars) and calls D-Bus reload via
   `opdbus.v1.S6.Systemctl`. This is real write-path behavior, not stub code.

2. `crates/op-grpc-bridge/src/mutation_engine.rs::seed_missing_plugin_projections()` calls
   `op_plugins::DefaultPluginRegistry::load_all_plugins()` which uses `inventory::iter::<PluginReg>`.
   Since `cognitive_mcp.rs` has `inventory::submit!` at its bottom, `CognitiveMcpPlugin` IS loaded
   and its projection IS seeded to `/dev/shm` at op-grpc-bridge startup.

3. `crates/op-plugins/src/lib.rs` exports `cognitive_mcp_plugin_schema()` publicly.

4. The fifteen schema methods (`get_config`, `set_config`, `get_health`, etc.) match the operations
   that `op-cognitive-mcp` actually performs.

What is currently MISSING is the consumer side: `op-cognitive-mcp`'s own server startup reads bind
address and enabled-flags from raw env vars, not from the plugin's D-Bus projection. The plugin
declares and owns that config; the server must read it back.

---

### Q4 — Which plugin genuinely has the most schema-declared methods?

**Answer: notebooklm.rs with 106 distinct methods. The premise about cognitive_mcp is false.**

Verified counts (grep `methods\.insert\(` in each plugin file):

| Plugin file       | methods.insert() count | Notes |
|-------------------|------------------------|-------|
| notebooklm.rs     | **106**                | 1685 lines; all distinct hand-declared methods |
| login1.rs         | 23                     | |
| dnsresolver.rs    | 20                     | |
| zeroclaw.rs       | 18                     | |
| json_render.rs    | 15                     | |
| cognitive_mcp.rs  | **15**                 | tied with json_render |
| ovsdb_bridge.rs   | 14                     | |
| btrfs_plugin.rs   | 12                     | |

The claim that cognitive_mcp.rs has the most methods is **false by 7×**. The 106 in notebooklm.rs
are not loop-generated — the file is 1685 lines of distinct methods covering notebook CRUD, source
management, label management, corpus management, auth, and cross-notebook queries.

---

### Q5 — Exact crate boundary

**Belongs in op-plugins (single source of truth, schema-declared):**

| Item | Current location | Status |
|------|-----------------|--------|
| `VoyageModel` enum (voyage-4-large/4/4-lite) | `embedding_model.rs` | ✅ correct |
| `VoyageModel::SHARED_EMBEDDING_DIMS = 1024` | `embedding_model.rs` | ✅ correct |
| `VoyageModel::SHARED_SPACE` constant array | `embedding_model.rs` | ✅ correct |
| `VOYAGE_PUBLIC_URL` constant | `embedding_model.rs` + **voyage.rs** + **rag_pipeline.rs** | ❌ 3 copies — delete from voyage.rs and rag_pipeline.rs |
| `VOYAGE_MONGODB_URL` constant | `embedding_model.rs` + **voyage.rs** + **rag_pipeline.rs** | ❌ 3 copies — same |
| `voyage_key_present()` / `voyage_url_for_key()` logic | `embedding_model.rs` + **voyage.rs** + **rag_pipeline.rs** | ❌ 3 copies — delete from voyage.rs and rag_pipeline.rs |
| env var priority (`COGNITIVE_MCP_VOYAGE_API_KEY` → `VOYAGE_API_KEY` → `VOYAGE_API_KEY_RUST`) | `embedding_model.rs` + **voyage.rs** + **rag_pipeline.rs** | ❌ 3 copies — delete from voyage.rs and rag_pipeline.rs |
| `provider`, `model_id`, `endpoint`, `dimensions` config fields | `embedding_model.rs` | ✅ correct |
| `EmbeddingModelState` schema struct | `embedding_model.rs` | ✅ correct |
| five schema methods: embed/list_models/get_config/set_model/update_config | `embedding_model.rs` | ✅ correct |
| bind-address / wg_interface config (`CognitiveMcpConfig`) | `cognitive_mcp.rs` | ✅ correct |
| fifteen cognitive_mcp schema methods | `cognitive_mcp.rs` | ✅ correct |

**Belongs in op-cognitive-mcp (executor and tool surface):**

| Item | Current location | Status |
|------|-----------------|--------|
| `reqwest::Client` HTTP calls to Voyage | `voyage.rs`, `rag_pipeline.rs` | ✅ correct crate, ❌ duplicated |
| `EmbeddingRequest` / `EmbeddingResponse` structs | `voyage.rs` + **rag_pipeline.rs** | ❌ duplicated — one copy only (in voyage.rs or a shared internal) |
| Qdrant upsert, chunking, RAG pipeline | `rag_pipeline.rs` | ✅ correct |
| MCP tool handlers (memory_store, code_search, etc.) | `grpc_service.rs`, `code_tools.rs`, etc. | ✅ correct |
| rag-ingest CLI orchestration | `src/bin/rag-ingest.rs` | ✅ correct |
| NotebookLM sidecar bridge | `src/notebooklm.rs` | ✅ correct |
| activity filter, soul_memory, gemini_fallback | respective files | ✅ correct |
| Server startup env-var reads for bind config | `src/server.rs` / `src/main.rs` | ❌ must read from cognitive_mcp projection instead |

---

## Architecture Diagram

```
op-plugins (schema authority)
┌─────────────────────────────────────────┐
│  embedding_model plugin                  │
│  ─────────────────────────────────────  │
│  EmbeddingModelState {                  │
│    provider, model_id, endpoint,        │
│    dimensions, status,                  │
│    available_models, model_digest       │
│  }                                       │
│  VoyageModel enum (4-large/4/4-lite)    │
│  VOYAGE_PUBLIC_URL  ◄── ONLY COPY       │
│  VOYAGE_MONGODB_URL ◄── ONLY COPY       │
│  voyage_key_present() ◄── ONLY COPY     │
│  Methods: embed/list_models/get_config/ │
│           set_model/update_config       │
│                         │               │
│  cognitive_mcp plugin   │               │
│  ──────────────────     │               │
│  CognitiveMcpState {    │  apply_state()│
│    http, grpc,          │  writes s6    │
│    wg_interface, ...    │  env-dir      │
│  }                      │               │
│  Methods: get_config/   │               │
│    set_config/get_health│               │
│    memory_*/code_*/...  │               │
└───────────┬─────────────┘               │
            │ /dev/shm projection seed    │
            │ (op-grpc-bridge startup)    │
            ▼                             │
    /dev/shm/opdbus/projections/          │
    ├── embedding_model  ◄────────────────┘
    └── cognitive_mcp ◄── server reads bind config from here

op-cognitive-mcp (executor, tool surface)
┌──────────────────────────────────────────────────────┐
│  reads /dev/shm/opdbus/projections/embedding_model   │
│  → {endpoint, model_id}  (fallback: env vars)        │
│                                                       │
│  voyage.rs (thin executor)                            │
│  ─────────────────────────────────────────────────   │
│  VoyageClient { api_key, model, endpoint }            │
│    constructed from projection params, not env vars   │
│  embed() → reqwest POST to endpoint                   │
│  (no more VOYAGE_PUBLIC_URL / al- detection here)     │
│                                                       │
│  rag_pipeline.rs (executor)                           │
│  ─────────────────────────────────────────────────   │
│  embed_document() / embed_query()                     │
│    → delegates to VoyageClient or shared embed fn     │
│  (no more inline VOYAGE_PUBLIC_URL / embed struct)    │
│                                                       │
│  server.rs                                            │
│  ─────────────────────────────────────────────────   │
│  reads /dev/shm/opdbus/projections/cognitive_mcp      │
│  → {http, grpc, wg_interface, http_enabled, ...}      │
│  → binds server to those addresses                    │
│  (fallback: existing env-var reads)                   │
│                                                       │
│  MCP tools: memory_store/retrieve/query/delete/...    │
│  code_search / code_index / code_context              │
│  gemini_fallback, soul_memory, notebooklm bridge      │
│  rag-ingest CLI                                       │
└──────────────────────────────────────────────────────┘
```

---

## Component Design

### Change 1: Extract a shared embed-params reader into embedding_model.rs

Add a `pub fn voyage_embed_params() -> Option<VoyageEmbedParams>` (or equivalent) to
`embedding_model.rs`. This function:

1. Attempts to read `/dev/shm/opdbus/projections/embedding_model` and deserialise `EmbeddingModelState`.
2. If the file is absent or malformed, falls back to reading env vars via the existing
   `config_state()` logic.
3. Returns `{ endpoint: String, model_id: String, api_key: String }` — the three call-time params.

This function is `pub` so `op-cognitive-mcp` can call it. It references nothing from `op-cognitive-mcp`
(no crate-cycle). The `api_key` is read from the env-var chain *inside embedding_model.rs* — it is
never stored in the projection (secrets do not live in shm). The projection provides endpoint +
model_id; the key is always read from env at call time via `voyage_key_present()` logic.

### Change 2: Refactor VoyageClient to take params, not read env

`src/voyage.rs::VoyageClient` gains a `VoyageClient::from_params(endpoint, api_key, model)` constructor
alongside (or replacing) `VoyageClient::new()`. The `from_params` path takes the three values
returned by `voyage_embed_params()`. The `new()` path (env-var bootstrap fallback) remains for cases
where the projection file is not yet seeded.

`VOYAGE_PUBLIC_URL`, `VOYAGE_MONGODB_URL`, and `voyage_url_for_key()` are deleted from `voyage.rs`.
The endpoint value already contains the resolved URL from the projection (or from embedding_model's
`config_state()` fallback, which applies the `al-` detection once and returns it).

### Change 3: RagPipeline reads from VoyageClient

`src/rag_pipeline.rs::RagPipeline` replaces its inline `embed()` function with a call to
`VoyageClient::embed()`. `RagPipeline::from_env()` constructs a `VoyageClient::from_params(...)` using
`op_plugins::state_plugins::embedding_model::voyage_embed_params()`. The inline `VOYAGE_PUBLIC_URL`,
`VOYAGE_MONGODB_URL`, local `voyage_url_for_key()`, `EmbeddingRequest`, and `EmbeddingResponse` structs
are deleted from `rag_pipeline.rs`.

### Change 4: Server reads cognitive_mcp projection for bind config

`src/server.rs` (or wherever the server binds its addresses) gains a helper:

```rust
fn cognitive_mcp_bind_config() -> CognitiveMcpBindConfig {
    // 1. Read /dev/shm/opdbus/projections/cognitive_mcp → deserialise
    // 2. If absent/malformed, fall back to env-var reads (existing code)
    // Returns { http, grpc, wg_interface, http_enabled, grpc_enabled, dbus_enabled }
}
```

`CognitiveMcpBindConfig` mirrors the fields of `CognitiveMcpConfig` in `cognitive_mcp.rs` but lives
in `op-cognitive-mcp` (no import of the plugin type required — just deserialise the same JSON shape).
The `op_plugins::state_plugins::cognitive_mcp::CognitiveMcpConfig` type can be re-used directly since
`op-cognitive-mcp` already depends on `op-plugins`.

### Change 5: Align schema methods with actual handlers (audit)

A one-pass audit of each of the fifteen `cognitive_mcp_schema()` methods confirms whether a matching
handler exists in `op-cognitive-mcp`. If any method has no handler, either:
- Add a minimal handler (preferred — the schema declares the contract), or
- Remove the method from the schema (only if the operation genuinely does not exist).

Based on reading `grpc_service.rs`, `code_tools.rs`, and `memory_store.rs`, the fifteen methods appear
to all have real backing operations. This audit confirms rather than changes anything — it is
verification that the schema and implementation are aligned.

---

## What Does NOT Change

| Item | Reason |
|------|--------|
| `embedding_model.rs` schema shape (5 methods, all fields) | Already correct; only adding `voyage_embed_params()` helper |
| `cognitive_mcp.rs` schema shape (15 methods, all fields) | Already correct; only adding projection-read in server.rs |
| HTTP executor code in `rag_pipeline.rs` (chunking, Qdrant upsert, batch logic) | Correct boundary |
| `src/notebooklm.rs` sidecar bridge | No embedding involved |
| `src/activity_filter.rs` | Schema-driven filter; no Voyage involvement |
| `src/client_config.rs` | Client-side pooling/retry; no Voyage involvement |
| `op-state-store` builtin catalog | Legacy list; cognitive_mcp and embedding_model not in it; leave alone |
| `register_plugin_projection_tools` in op-tools | Dead-letter function; not wired; separate task |
| Any Cargo.toml changes | No new dependencies introduced |

---

## Migration Sequencing

The five changes are sequenced to be independently compilable at each step:

1. **embedding_model.rs**: Add `pub fn voyage_embed_params()` (and `VoyageEmbedParams` struct).
   `cargo check -p op-plugins` must pass.

2. **voyage.rs**: Add `VoyageClient::from_params()`. Delete `VOYAGE_PUBLIC_URL`, `VOYAGE_MONGODB_URL`,
   `voyage_url_for_key()`. Update callers within the file to use `from_params()`.
   `cargo check -p op-cognitive-mcp` must pass.

3. **rag_pipeline.rs**: Replace inline `embed()` with VoyageClient delegation. Delete duplicate
   constants and structs. `cargo check -p op-cognitive-mcp` must pass.

4. **server.rs**: Add projection-read for bind config with env-var fallback.
   `cargo check -p op-cognitive-mcp` must pass.

5. **Full build**: `cargo build --workspace`. Then confirm `cargo run --bin rag-ingest -- --dry-run`
   produces the expected cost-estimate output.

---

## OSCAL Subid Assignments for New Items

| Item | Subid |
|------|-------|
| `VoyageEmbedParams` struct | `sch.software.embedding-model.embed-params.schema@v1` |
| `voyage_embed_params()` function | `obs.service.embedding-model.embed-params.resolve@v1` |
| `cognitive_mcp_bind_config()` helper | `obs.service.cognitive-mcp.bind-config.resolve@v1` |
