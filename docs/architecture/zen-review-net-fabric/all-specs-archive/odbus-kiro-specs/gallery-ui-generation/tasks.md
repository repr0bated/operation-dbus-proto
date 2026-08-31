# Tasks — Model-Agnostic Generative UI Gallery

## Phase 1: Static Documentation + Context Assembly

### Task 1.1 — Write access-instructions.md
- [ ] Create `docs/gallery-gen/access-instructions.md`
- [ ] Document `PluginSchema` structure: fields, methods, subids, constraints
- [ ] Document field types and their rendering implications
- [ ] Document method side-effects and their UI meaning
- [ ] Document `bind` path syntax and how specs reference live state
- [ ] Keep under 2K tokens for context efficiency

### Task 1.2 — Write json-render-catalog.md
- [ ] Create `docs/gallery-gen/json-render-catalog.md`
- [ ] Extract the legal component list from the `json_render` plugin's `ComponentDecl` entries
- [ ] Document each component's prop schema (from the interpreter's match arms + the plugin's catalog)
- [ ] Mark which components are `StableCore` vs available for novelty
- [ ] Document action types and payloads
- [ ] Keep under 3K tokens

### Task 1.3 — Write spec-grammar.md
- [ ] Create `docs/gallery-gen/spec-grammar.md`
- [ ] Document the flat-tree format (`root`, `elements`, element fields)
- [ ] Document `SpecContract` fields from the `json_render` plugin defaults
- [ ] Document streaming patch ops
- [ ] Document validation rules (what makes a spec valid/invalid)
- [ ] Keep under 1K tokens

### Task 1.4 — Implement Context Assembler
- [ ] Create `GenerationContext` struct with all fields from design
- [ ] Implement `SchemaPayload::Inline` assembly from live blob catalog
- [ ] Implement `SchemaPayload::Summary` assembly for context-constrained models
- [ ] Implement context-budget decision logic (query ZeroClaw `/v1/models` for context window)
- [ ] Implement system message builder (concatenates docs + schema payload)
- [ ] Test: context assembles correctly from a mock blob catalog

---

## Phase 2: Inference Loop

### Task 2.1 — ZeroClaw HTTP Client
- [ ] Implement HTTP client for ZeroClaw's `/v1/chat/completions` endpoint
- [ ] Support: messages array, tools, response_format, temperature, max_tokens
- [ ] Support: streaming (SSE) for progress reporting to chat
- [ ] Support: tool_calls in response → execute → re-call flow
- [ ] No provider-specific SDK — raw HTTP + serde_json only
- [ ] Test: mock ZeroClaw endpoint returns a valid spec

### Task 2.2 — Per-Slot Generation Logic
- [ ] Implement single-slot generation: build messages → call → validate → admit/reject
- [ ] Implement retry logic: up to 3 attempts per slot, then skip
- [ ] Implement tool-call handling: execute MCP/Qdrant tool, append result, re-call (depth cap: 5)
- [ ] Implement progress reporting: emit events for chat to display
- [ ] Test: slot generation with valid output → admission
- [ ] Test: slot generation with invalid output → retry → eventual skip

### Task 2.3 — Gallery Fill Loop
- [ ] Implement loop over empty novelty slots up to 200 cap
- [ ] Implement signature-hash dedup against existing gallery
- [ ] Implement parallelism (up to 4 concurrent slots, sequential admission)
- [ ] Implement cancellation (operator cancel → stop loop, keep admitted specs)
- [ ] Implement FIFO novelty retirement when gallery is full
- [ ] Test: fill loop produces N specs and stops at cap

---

## Phase 3: Spec Validation

### Task 3.1 — Implement validate_spec()
- [ ] Validate structure: `root` exists, `elements` is a map
- [ ] Validate root reference: root ID exists in elements map
- [ ] Validate component types: every `element.type` is in the known catalog
- [ ] Validate props: every element's props match the component's schema (type-check, not deep semantic)
- [ ] Validate children: every child ID resolves, no dangling refs
- [ ] Validate bind paths: syntactically valid JSON pointers
- [ ] Validate DAG: no cycles in the children graph
- [ ] Test: valid specs pass, each invalid case produces the correct error

### Task 3.2 — Component Catalog Registry
- [ ] Build a runtime registry of known component types + their prop schemas
- [ ] Source from the `json_render` plugin's `ComponentDecl` list + the interpreter's match arms
- [ ] Include the ~40 stable-core primitives as the baseline
- [ ] Expose as a `ComponentCatalog` struct queryable by type name
- [ ] Test: all interpreter-known kinds are in the registry

---

## Phase 4: MCP Tool Layer

### Task 4.1 — Implement MCP Tool Definitions
- [ ] Define `list_plugins` tool schema (JSON Schema for args + returns)
- [ ] Define `get_plugin_schema` tool schema
- [ ] Define `search_fields` tool schema
- [ ] Define `search_methods` tool schema
- [ ] Define `search_subids` tool schema
- [ ] Define `find_related` tool schema
- [ ] Package as OpenAI-compatible `tools` array for the inference call

### Task 4.2 — Implement MCP Tool Handlers
- [ ] Implement `list_plugins`: iterate catalog snapshot, return summaries
- [ ] Implement `get_plugin_schema`: look up by ID, return full JSON
- [ ] Implement `search_fields`: filter fields by type/name pattern across all plugins
- [ ] Implement `search_methods`: filter methods by side-effect/capability/input type
- [ ] Implement `search_subids`: filter by subid category prefix
- [ ] Implement `find_related`: find shared struct types between plugins
- [ ] All handlers are read-only, operating on the immutable catalog snapshot
- [ ] Test: each tool returns correct results from a test catalog

---

## Phase 5: Qdrant Integration

### Task 5.1 — Schema Vectorization Indexer
- [ ] Create background indexer that watches `catalog_hash` changes
- [ ] On change: re-read all plugin schemas, chunk into fragments (per-field, per-method, per-struct)
- [ ] Embed fragments using the embedding model routed through ZeroClaw
- [ ] Upsert into `gallery-gen-schemas` Qdrant collection with metadata: plugin_id, fragment_type, domain_tag
- [ ] Domain tagging: classify each fragment by category (privacy-ops, network-engineering, compliance, etc.)
- [ ] Test: indexer populates collection from mock blobs

### Task 5.2 — Semantic Search Tool
- [ ] Define `semantic_search` tool schema (query string → results with scores)
- [ ] Implement handler: query Qdrant collection, return top-K results with metadata
- [ ] Include in the tools array when Qdrant toggle is on
- [ ] Test: search returns relevant fragments for domain queries

---

## Phase 6: Antigravity Chat Integration

### Task 6.1 — Gallery Generation Session Mode
- [ ] Define a `Route::GalleryGen` or session-mode flag in the chat system
- [ ] On enter: display current gallery state (slots filled, stable core count, catalog stats)
- [ ] Display tier toggles (MCP, Qdrant) as interactive elements
- [ ] Display text input for operator guidance
- [ ] Display Generate and Cancel action buttons
- [ ] On exit: clear all session state

### Task 6.2 — Progress Streaming
- [ ] Define progress event types: `Assembling`, `Generating(n, total)`, `Admitted(id)`, `Rejected(reason)`, `Complete(stats)`
- [ ] Render progress as a `repeat`-bound log in the chat (same DSL as the rest of the UI)
- [ ] Handle cancellation: stop the inference loop, report final stats
- [ ] Handle errors: display inline with the failing spec fragment

### Task 6.3 — Session Isolation
- [ ] Ensure generation context is dropped on session end (no Arc leaks, no background references)
- [ ] Ensure no state persists to disk, cognitive-mcp, or any memory store
- [ ] Ensure operator guidance is not logged to the reasoning-audit pipeline (generation sessions are ephemeral)
- [ ] Test: two consecutive sessions have zero shared state

---

## Phase 7: Migration + Cleanup

### Task 7.1 — Remove op-gemma Generator Functions
- [ ] Remove `all_generators()` and all `gen_*` functions from `ui_gallery.rs`
- [ ] Remove `GemmaSpecGallery` / `GemmaSpecEntry` types (replaced by direct `CatalogStore` admission)
- [ ] Remove `/dev/shm/gemma-ui-specs.json` file-based output
- [ ] Remove the `lovable` gallery page polling endpoints (`/api/gemma/*`)
- [ ] Keep `op-gemma` crate if it has other responsibilities; remove entirely if gallery was its only purpose

### Task 7.2 — Update Documentation
- [ ] Update `CLAUDE.md` / `AGENTS.md` to reflect model-agnostic gallery
- [ ] Remove references to `gemma_brain` as sole gallery writer
- [ ] Document the new generation flow for operators
- [ ] Update `PLUGIN-RENDER-CONTRACT.md` to reference the new docs path

### Task 7.3 — Integration Test
- [ ] End-to-end test: assemble context → call mock inference → validate spec → admit to gallery
- [ ] Test with MCP tools enabled: model issues tool call → handler responds → model produces spec
- [ ] Test gallery protection: stable-core elements never displaced
- [ ] Test cancellation: partial fill leaves gallery in consistent state
- [ ] Test failure: all attempts fail → gallery unchanged, no crash
