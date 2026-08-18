# Requirements: voyage-plugin-cognitive-mcp-boundaries

## Purpose

Establish a clear, enforced boundary between `op-plugins` (the schema and config authority) and
`op-cognitive-mcp` (the HTTP executor and MCP tool surface), eliminating the three duplicate
embedding implementations that currently exist across those two crates, and wiring `op-cognitive-mcp`
to read its own runtime configuration from the D-Bus projection its plugin already owns.

## Context and Verified Baseline

These requirements are grounded in code read during this spec session. Every claim below references
a specific file and observation.

### What already exists and is correct

- `crates/op-plugins/src/state_plugins/embedding_model.rs` — generic embedding surface: `provider`,
  `model_id`, `endpoint`, `dimensions`, `status`, `available_models`, `model_digest`. Carries
  `VoyageModel` enum (voyage-4-large / voyage-4 / voyage-4-lite, shared 1024-dim space).
  `voyage_key_present()` checks three env vars. `config_state()` reads provider config from env.
  `live_state()` queries Ollama or sets status=online for Voyage iff key present. Five schema methods:
  `embed`, `list_models`, `get_config`, `set_model`, `update_config`. Self-registered via
  `inventory::submit!`. Seeds `/dev/shm` at op-grpc-bridge startup via `seed_missing_plugin_projections`.

- `crates/op-plugins/src/state_plugins/cognitive_mcp.rs` — `CognitiveMcpPlugin` with `apply_state()`
  that writes s6 env-dir files (`COGNITIVE_MCP_BIND`, `COGNITIVE_MCP_GRPC_BIND`, `WG_INTERFACE`, etc.)
  and calls D-Bus reload. Fifteen schema methods covering config, health, memory CRUD, code search /
  index / context, gemini_query, restart_service. Self-registered via `inventory::submit!`. Seeded
  to `/dev/shm` at startup alongside embedding_model. `op_plugins::cognitive_mcp_plugin_schema()` is
  publicly exported from `crates/op-plugins/src/lib.rs`.

- `crates/op-cognitive-mcp` already depends on `op-plugins` (confirmed in its `Cargo.toml`). No
  crate-cycle obstacle exists.

### What is broken (the duplications)

1. **`src/voyage.rs` — VoyageClient**: reads `COGNITIVE_MCP_VOYAGE_API_KEY` / `VOYAGE_API_KEY` /
   `VOYAGE_API_KEY_RUST` directly. Duplicates `al-` prefix → MongoDB URL detection with its own
   `VOYAGE_PUBLIC_URL` and `VOYAGE_MONGODB_URL` constants.

2. **`src/rag_pipeline.rs` — inline `embed()`**: also reads the same three key env vars, defines its
   own `VOYAGE_PUBLIC_URL`/`VOYAGE_MONGODB_URL` constants, and its own
   `EmbeddingRequest`/`EmbeddingResponse` structs. URL detection logic is copy-pasted verbatim from
   voyage.rs (`voyage_url_for_key(key)`).

3. **`src/rag_pipeline.rs` — `RagPipeline::from_env()`**: re-derives `voyage_model` from
   `COGNITIVE_MCP_VOYAGE_MODEL` / `VOYAGE_MODEL` env vars independently of the embedding_model plugin.

All three read the same env vars and derive the same endpoint URL through independent, unsynchronised
code paths. If the endpoint logic changes in one place, the others drift.

### What is NOT broken and must not be touched

- The `op-cognitive-mcp` HTTP execution path (`reqwest` calls, Qdrant upsert, chunking, context
  assembly, soul_memory, gemini_fallback, session tracking). HTTP execution lives here; that is correct.
- The `rag-ingest` CLI binary in `src/bin/rag-ingest.rs` — its orchestration logic, cost estimation,
  and batch flow are correct. Only the `VoyageClient`/embed wiring changes.
- `src/notebooklm.rs` — the MCP sidecar bridge. No embedding at all; not involved.
- `src/activity_filter.rs` — schema-driven significance logic against `PluginSchema`. Correct and
  not involved.
- `src/client_config.rs` — client-side connection pooling / circuit breaker. Not involved.
- The `op-state-store` legacy catalog (`builtin_plugin_schemas()`). Does not contain
  `cognitive_mcp` or `embedding_model`; those are purely inventory-discovered. Leave the legacy
  catalog alone.

---

## Functional Requirements

### FR-1: embedding_model remains the single Voyage config authority

The `embedding_model` plugin is the one and only place in the system that owns:
- provider selection (`provider` field, default `"voyage"`)
- API key env-var precedence (`COGNITIVE_MCP_VOYAGE_API_KEY` → `VOYAGE_API_KEY` → `VOYAGE_API_KEY_RUST`)
- endpoint URL derivation (public vs. MongoDB `al-` key prefix)
- active model ID (`EMBEDDING_MODEL_ID` → `COGNITIVE_MCP_VOYAGE_MODEL` → `VOYAGE_MODEL`)
- output dimensionality (`EMBEDDING_DIMENSIONS`, default 1024)
- the `VoyageModel` enum (voyage-4-large / voyage-4 / voyage-4-lite)

No other file in the repository shall define `VOYAGE_PUBLIC_URL`, `VOYAGE_MONGODB_URL`, or the
`al-` prefix branching. If a caller needs those values it reads the embedding_model D-Bus projection
from `/dev/shm/opdbus/projections/embedding_model` or passes them as constructor parameters.

### FR-2: VoyageClient and RagPipeline read config from the embedding_model projection

`src/voyage.rs::VoyageClient::new()` and `src/rag_pipeline.rs::RagPipeline::from_env()` must stop
reading Voyage env vars directly. Instead they read the embedding_model state from the shm projection
(JSON at `/dev/shm/opdbus/projections/embedding_model`) to obtain `endpoint`, `model_id`, and
`api_key_present` status. A fallback to env-var reading is permitted only when the projection file is
absent (bootstrap race before first seed), using the same priority order as `embedding_model.rs`.

This does not change the HTTP call surface — `reqwest` calls stay in `op-cognitive-mcp`. It changes
where the call *parameters* (endpoint, key, model) come from.

### FR-3: Duplicate URL detection constants removed

The constants `VOYAGE_PUBLIC_URL` and `VOYAGE_MONGODB_URL` and the function `voyage_url_for_key` that
currently exist in both `src/voyage.rs` and `src/rag_pipeline.rs` shall be deleted from both files.
The canonical implementation lives only in `embedding_model.rs` (or a shared private helper it calls).

### FR-4: op-cognitive-mcp server reads bind-address config from the cognitive_mcp plugin projection

`op-cognitive-mcp`'s server startup (`src/server.rs` or `src/main.rs`) reads its bind addresses,
WireGuard interface, and enabled-flags from the `/dev/shm/opdbus/projections/cognitive_mcp` projection
file, not directly from env vars. Env-var reading remains as a fallback when the projection is absent.

This completes the "plugin is the schema" contract: the plugin already owns this config and writes it
via `apply_state()`; the server must read back what the plugin declares rather than maintaining a
parallel env-var reader.

### FR-5: cognitive_mcp schema method surface matches the actual op-cognitive-mcp tool surface

The fifteen methods declared in `cognitive_mcp_schema()` (`get_config`, `set_config`, `get_health`,
`list_tools`, `register_tool`, `memory_store`, `memory_retrieve`, `memory_query`, `memory_delete`,
`memory_list_namespaces`, `code_search`, `code_index`, `code_context`, `gemini_query`,
`restart_service`) must each correspond to a real, callable handler in `op-cognitive-mcp`. Any schema
method with no backing handler is dead schema and must be removed or given a handler.

### FR-6: embedding_model schema method `embed` has a backing implementation path

The `embed` method in the `embedding_model` schema is callable via D-Bus. When invoked, op-grpc-bridge
dispatches to the plugin's apply path, which in turn calls the embed logic in op-cognitive-mcp (via
the existing D-Bus/gRPC dispatch chain). This does not require a synchronous cross-crate function call
— the D-Bus object is the authority; op-cognitive-mcp listens on its own D-Bus name. The schema
declares the contract; the crate implements it.

### FR-7: No new crate dependencies introduced

The boundary cleanup does not introduce any new entries in any `Cargo.toml`. The existing
`op-cognitive-mcp → op-plugins` dependency is the only cross-crate link required. op-plugins must not
gain a dependency on op-cognitive-mcp.

---

## Non-Functional Requirements

### NFR-1: No regression in rag-ingest CLI

`cargo run --bin rag-ingest -- --dry-run` must produce the same output and cost estimate as before.
The embedding path it uses internally changes from direct env-var reads to projection-sourced params;
behaviour is identical.

### NFR-2: cargo build --workspace passes

All changes must compile clean. No `#[allow(dead_code)]` additions to hide removed items.

### NFR-3: No /dev/shm write from op-cognitive-mcp

op-cognitive-mcp reads `/dev/shm/opdbus/projections/*` but does not write to it. Writing projections
is op-grpc-bridge's domain (MutationEngine, seed_missing_plugin_projections). Zero-btrfs-overhead rule
remains intact.

### NFR-4: OSCAL subid coverage

Any new public function or struct added to `embedding_model.rs` as a result of this work must carry
an appropriate `x-oscal-subid` annotation following the taxonomy in AGENTS.md §4a.

---

## Out of Scope

- Deleting `cognitive_mcp.rs` (it is a live seam — keep it).
- Changing the notebooklm plugin's 106 methods (correct as-is, separate concern).
- Wiring `register_plugin_projection_tools` in op-tools (a separate MCP tool discovery task).
- Adding new MCP tools to the external cognitive-mcp gateway surface.
- Modifying the op-state-store legacy catalog.
- Any change to the NotebookLM sidecar bridge (`src/notebooklm.rs` in op-cognitive-mcp).
