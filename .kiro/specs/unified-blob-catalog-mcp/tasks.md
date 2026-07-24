# Tasks: unified-blob-catalog-mcp

Ordered by dependency. Tasks 1-3 are purely additive to `qdrant_shuttle.rs`
and have no dependencies on each other beyond T1 landing first. T4-T5 depend on
T1-T3. T6 is gating-free but logically last.

---

## T1 — Add `blob_vectors_collection` to `QdrantSemanticShuttle`

**File**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs`

- [ ] Add constant `DEFAULT_BLOB_VECTORS_COLLECTION: &str = "blob_vectors"` near
      the other defaults (line ~21).
- [ ] Add `blob_vectors_collection: String` field to the `QdrantSemanticShuttle`
      struct (line ~36-42 block).
- [ ] In `new()`: read `COGNITIVE_MCP_BLOB_VECTORS_COLLECTION` env var with
      fallback to the constant; pass to `new_with_clients`.
- [ ] In `new_with_clients()`: add `blob_vectors_collection: impl Into<String>`
      parameter; store in struct; add
      `blob_vectors_collection = %blob_vectors_collection` to `tracing::info!`.
- [ ] `cargo build -p op-cognitive-mcp` passes with no warnings.

**Why first**: every subsequent task uses the new field or the new methods that
depend on it.

---

## T2 — Add `all_blob_embedding_texts` and `plugin_id_to_uuid` helpers

**File**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs`

- [ ] Add free function `all_blob_embedding_texts() -> Result<Vec<(String, String)>>`:
      reads `read_manifest_plugin_ids_shm()`, maps each id through
      `read_plugin_schema_shm` + `render_schema_embedding_text`, skips missing
      schemas with `filter_map`.
- [ ] Add free function `plugin_id_to_uuid(plugin_id: &str) -> uuid::Uuid` using
      `Uuid::new_v5` with a fixed project-local namespace constant.
- [ ] Add unit test `plugin_id_to_uuid_is_stable`: assert
      `plugin_id_to_uuid("zeroclaw") == plugin_id_to_uuid("zeroclaw")` and
      `plugin_id_to_uuid("zeroclaw") != plugin_id_to_uuid("antigravity")`.
- [ ] Add `RefreshBlobVectorsSummary { embedded: usize, collection: String }`
      struct with `#[derive(Debug, serde::Serialize)]` and `pub` fields.
- [ ] `cargo test -p op-cognitive-mcp` passes.

**Depends on**: nothing (no `self` involved in these helpers).

---

## T3 — Add `refresh_blob_vectors` and `search_blob_vectors` methods

**File**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs`

- [ ] Add `pub async fn refresh_blob_vectors(&self) -> Result<RefreshBlobVectorsSummary>`:
      calls `all_blob_embedding_texts()`, embeds each with `embed_document`,
      builds `PointStruct` with `plugin_id_to_uuid` as point id and
      `{plugin_id, text}` payload, upserts in one `UpsertPointsBuilder` call,
      logs result, returns summary.
- [ ] Add `pub async fn search_blob_vectors(&self, query: &str, limit: u64) -> Result<Vec<ScoredPoint>>`:
      embeds `query` with `embed_query_text`, queries `blob_vectors_collection`
      via `QueryPointsBuilder` with no filter, returns scored points.
- [ ] Export `RefreshBlobVectorsSummary` from `lib.rs` pub use (or keep crate-internal
      — confirm based on whether `blob_vectors_tool.rs` needs it across modules).
- [ ] `cargo build -p op-cognitive-mcp` passes.

**Depends on**: T1 (field), T2 (helpers + struct).

---

## T4 — Create `blob_vectors_tool.rs`

**File**: `crates/op-cognitive-mcp/src/blob_vectors_tool.rs` (new)

- [ ] Create file with module-level doc explaining both tools and the
      "user-triggered only" constraint for refresh.
- [ ] Implement `RefreshBlobVectorsTool { shuttle: Option<Arc<QdrantSemanticShuttle>> }`:
      - `name()` → `"refresh_blob_vectors"`
      - `description()` — includes "user-triggered only, never automatic"
      - `category()` → `"schema"`, `namespace()` → `"plugins"`
      - `tags()` → `["schema", "blob", "vectors", "refresh"]`
      - `input_schema()` → empty object schema
      - `execute()` → calls `shuttle.refresh_blob_vectors()`, returns
        `{ok, embedded, collection}`. Returns error (not panic) when shuttle is `None`.
- [ ] Implement `SearchBlobVectorsTool { shuttle: Option<Arc<QdrantSemanticShuttle>> }`:
      - `name()` → `"search_blob_vectors"`
      - `input_schema()` → `{required: [query], properties: {query: string, limit: integer}}`
      - `execute()` → calls `shuttle.search_blob_vectors(query, limit)`, returns
        `{count, results: [{plugin_id, score, text}]}`. Returns error when shuttle is `None`.
- [ ] `pub async fn register_blob_vectors_tools(registry, qdrant: Option<Arc<…>>) -> Result<()>`:
      registers both tools.
- [ ] `cargo build -p op-cognitive-mcp` passes.

**Depends on**: T3 (the two methods being called).

---

## T5 — Register tools and export module

**Files**: `cognitive_tools.rs`, `lib.rs`

- [ ] In `lib.rs`: add `pub mod blob_vectors_tool;` alongside `pub mod blob_catalog_tool;`.
- [ ] In `lib.rs`: add `pub use blob_vectors_tool::register_blob_vectors_tools;` if
      external callers need it (at minimum for symmetry with existing re-exports).
- [ ] In `cognitive_tools.rs::register_all`: add
      `crate::blob_vectors_tool::register_blob_vectors_tools(registry, qdrant).await?;`
      after `register_blob_catalog_tool(registry).await?;`.
      Ensure `qdrant` is not moved before this call — the existing `MemoryTool::new`
      call must clone `qdrant` so it remains available here.
- [ ] `cargo build --workspace` passes (checks no other crate is broken by the
      `register_all` signature change — there is none, since `new_with_clients` is
      private and the only signature change in the public API is the new
      `blob_vectors_collection` field which is constructed internally).
- [ ] `cargo test -p op-cognitive-mcp` passes (all existing tests still green).

**Depends on**: T4.

---

## T6 — Smoke-test against running Qdrant (manual / CI with Qdrant service)

This task is manual verification; it does not modify source.

- [ ] Start cognitive MCP server (`cargo run -p op-cognitive-mcp`), confirm
      startup log includes `blob_vectors_collection = "blob_vectors"`.
- [ ] Call `refresh_blob_vectors` tool (e.g. via `op-cog-admin` or direct MCP
      JSON-RPC): confirm response `{ok: true, embedded: N}` where `N > 0`.
- [ ] Call `refresh_blob_vectors` a second time: confirm `embedded` count is the
      same; confirm Qdrant collection point count is the same (no duplicates).
- [ ] Call `search_blob_vectors` with `{query: "zeroclaw", limit: 3}`: confirm
      response contains a result with `plugin_id: "zeroclaw"`.
- [ ] Call `search_blob_vectors` with `{query: "wireguard"}`: confirm results are
      plausibly related to network/identity plugins, not random.
- [ ] With `COGNITIVE_MCP_BLOB_VECTORS_COLLECTION=test_blobs` set, restart and
      confirm startup log and tool responses reference `"test_blobs"`.
- [ ] Kill Qdrant, restart server, call both tools: confirm error responses
      contain "not configured" or "unavailable", no panics.

**Depends on**: T5 (all source changes complete).

---

## Not in scope (no task created)

- Chat handler source-selection wiring — requires its own spec.
- Frontend toggle UI — frontend work, requires its own spec.
- "Related" one-hop traversal in the chat handler — inline logic, no new tool.
- "Live" toggle path — `get_projection` already exists; wiring is part of chat
  handler work.
