# Tasks: voyage-plugin-cognitive-mcp-boundaries

Each task is independently verifiable with `cargo check -p <crate>`. Complete them in order — each
step's output is the next step's input. No implementation code is written in this spec; tasks
reference the design decisions from `design.md`.

---

## Task 1 — Add `voyage_embed_params()` to embedding_model.rs

**Crate:** `op-plugins`
**File:** `crates/op-plugins/src/state_plugins/embedding_model.rs`

### What to add

Add a public struct `VoyageEmbedParams` and a public function `voyage_embed_params()`:

```rust
/// OSCAL subid: sch.software.embedding-model.embed-params.schema@v1
pub struct VoyageEmbedParams {
    /// Resolved Voyage endpoint URL (public or MongoDB, based on al- key prefix).
    pub endpoint: String,
    /// Active model identifier (e.g. "voyage-4").
    pub model_id: String,
    /// Voyage API key, sourced from env vars in priority order.
    pub api_key: String,
}
```

`voyage_embed_params()` (subid `obs.service.embedding-model.embed-params.resolve@v1`):

1. Attempts to read `/dev/shm/opdbus/projections/embedding_model` as UTF-8 JSON.
2. Deserialises into `EmbeddingModelState` via `serde_json::from_str`.
3. If successful: uses `state.endpoint` and `state.model_id`; reads the API key fresh from env
   (key is never stored in the projection — env only).
4. If the file is absent or unparseable: falls back to `Self::config_state()` to get endpoint and
   model_id, and reads the API key from env.
5. Returns `Some(VoyageEmbedParams { endpoint, model_id, api_key })` when a key is found, `None`
   when no Voyage API key exists in env.

### Acceptance criteria

- `cargo check -p op-plugins` passes.
- `VoyageEmbedParams` and `voyage_embed_params` are `pub` and importable as
  `op_plugins::state_plugins::embedding_model::voyage_embed_params`.
- No new crate dependencies in `op-plugins/Cargo.toml`.
- `VoyageEmbedParams` carries `x-oscal-subid` annotations on its fields per §4a.

---

## Task 2 — Refactor VoyageClient to accept params; delete duplicate constants

**Crate:** `op-cognitive-mcp`
**File:** `crates/op-cognitive-mcp/src/voyage.rs`

### What to change

1. Add `VoyageClient::from_params(endpoint: String, api_key: String, model: String) -> Self`
   constructor. This is the primary path used when a projection is available.

2. Rewrite `VoyageClient::new()` to call
   `op_plugins::state_plugins::embedding_model::voyage_embed_params()`:
   - If `Some(params)` returned: construct via `from_params`.
   - If `None` (no key): return the existing `context!("Voyage API key not found")` error.
   This preserves the existing error message and fallback behaviour.

3. **Delete** the following from `voyage.rs`:
   - `const VOYAGE_PUBLIC_URL: &str`
   - `const VOYAGE_MONGODB_URL: &str`
   - `fn voyage_url_for_key(key: &str) -> String`
   - The direct `env::var("COGNITIVE_MCP_VOYAGE_API_KEY")` / `VOYAGE_API_KEY` / `VOYAGE_API_KEY_RUST`
     chain (now inside `voyage_embed_params()`).
   - The direct `env::var("COGNITIVE_MCP_VOYAGE_MODEL")` / `VOYAGE_MODEL` chain (ditto).

4. `VoyageClient.base_url` is now set from `params.endpoint` rather than computed here.

### Acceptance criteria

- `cargo check -p op-cognitive-mcp` passes.
- `voyage.rs` no longer defines `VOYAGE_PUBLIC_URL` or `VOYAGE_MONGODB_URL`.
- `VoyageClient::new()` still works end-to-end: calling it without a projection present falls back
  to env-var reading via `voyage_embed_params()`'s fallback path.
- The `embed()` method signature is unchanged — callers do not change.

---

## Task 3 — Remove duplicate embed logic from rag_pipeline.rs

**Crate:** `op-cognitive-mcp`
**File:** `crates/op-cognitive-mcp/src/rag_pipeline.rs`

### What to change

1. **Delete** from `rag_pipeline.rs`:
   - `const VOYAGE_PUBLIC_URL: &str`
   - `const VOYAGE_MONGODB_URL: &str`
   - The private `fn voyage_url_for_key(key: &str) -> String` at the bottom of the file
   - The private `struct EmbeddingRequest<'a>` (local to rag_pipeline.rs)
   - The private `struct EmbeddingResponse` and `struct EmbData` (local to rag_pipeline.rs)

2. Remove the `voyage_key` and `voyage_model` fields from `RagPipeline` struct. Replace with a
   `voyage: VoyageClient` field, constructed in `from_env()`.

3. Rewrite `RagPipeline::from_env()`:
   - Replace the manual env-var reads for key and model with `VoyageClient::new()?`.
   - Store the constructed `VoyageClient` in `self.voyage`.

4. Replace `RagPipeline::embed_document()` and `embed_query()` with calls to `self.voyage.embed()`:
   ```rust
   async fn embed_document(&self, text: &str) -> Result<Vec<f32>> {
       self.voyage.embed(text, Some("document")).await
   }
   async fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
       self.voyage.embed(text, Some("query")).await
   }
   ```

5. The private `embed()` method on `RagPipeline` that constructs the reqwest request directly
   is deleted entirely — it is replaced by the delegation above.

### Acceptance criteria

- `cargo check -p op-cognitive-mcp` passes.
- `rag_pipeline.rs` contains no mention of `VOYAGE_PUBLIC_URL`, `VOYAGE_MONGODB_URL`, or
  `voyage_url_for_key`.
- `RagPipeline::from_env()` no longer reads Voyage env vars directly.
- `rag-ingest --dry-run` produces unchanged output (no HTTP calls in dry-run; cost estimate
  calculation uses chunk counts from zip, which is unaffected).

---

## Task 4 — Wire server.rs to read bind config from cognitive_mcp projection

**Crate:** `op-cognitive-mcp`
**Files:** `crates/op-cognitive-mcp/src/server.rs` (and/or `src/main.rs` — confirm which file
reads bind addresses before editing)

### What to add

Add a module-private helper:

```rust
/// OSCAL subid: obs.service.cognitive-mcp.bind-config.resolve@v1
fn cognitive_mcp_bind_config() -> op_plugins::state_plugins::cognitive_mcp::CognitiveMcpConfig {
    // 1. Read /dev/shm/opdbus/projections/cognitive_mcp
    // 2. Deserialise as CognitiveMcpConfig (re-use the type from op-plugins)
    // 3. On any error: fall back to CognitiveMcpConfig::current_config()
    //    (the existing env-dir reading logic already on CognitiveMcpPlugin)
    //    or construct Default with env-var overrides.
}
```

Replace the hard-coded / env-var-only bind-address reads at server startup with a call to
`cognitive_mcp_bind_config()`. The fallback must preserve all existing behaviour when the
projection is absent so that cold-starts before the first seed work correctly.

### Acceptance criteria

- `cargo check -p op-cognitive-mcp` passes.
- When `/dev/shm/opdbus/projections/cognitive_mcp` contains a valid projection (e.g. written by
  `apply_state()` via `set_config`), the server binds to the addresses in that projection.
- When the projection file is absent, the server falls back to env-var reads (existing behaviour).
- No new `Cargo.toml` dependency added (`op-cognitive-mcp` already depends on `op-plugins`).

---

## Task 5 — Audit: confirm all fifteen cognitive_mcp schema methods have backing handlers

**Crate:** `op-cognitive-mcp`
**Files:** `src/grpc_service.rs`, `src/code_tools.rs`, `src/memory_store.rs`, and any other handler
files — read them to confirm.

### What to verify

For each of the fifteen methods declared in `cognitive_mcp_schema()`:

| Schema method | Expected handler location |
|---|---|
| `get_config` | grpc_service.rs or server.rs |
| `set_config` | grpc_service.rs |
| `get_health` | grpc_service.rs or server.rs |
| `list_tools` | grpc_service.rs or main.rs |
| `register_tool` | grpc_service.rs or tool_profiles.rs |
| `memory_store` | memory_store.rs or grpc_service.rs |
| `memory_retrieve` | memory_store.rs or grpc_service.rs |
| `memory_query` | memory_store.rs or grpc_service.rs |
| `memory_delete` | memory_store.rs or grpc_service.rs |
| `memory_list_namespaces` | memory_store.rs or grpc_service.rs |
| `code_search` | code_tools.rs or grpc_service.rs |
| `code_index` | code_tools.rs or grpc_service.rs |
| `code_context` | context_awareness.rs or code_tools.rs |
| `gemini_query` | gemini_fallback.rs or grpc_service.rs |
| `restart_service` | grpc_service.rs or dbus_interface.rs |

### What to do for each method

- If a real handler is found: record the file path in a comment at the top of
  `cognitive_mcp_schema()` (one line per method, e.g. `// code_search → code_tools.rs:search_code`).
- If a method has no handler: add a minimal returning handler that returns
  `Err(Status::unimplemented("..."))` to establish the contract. Do not remove the method from
  the schema — the schema is the authority; the implementation must catch up.

### Acceptance criteria

- `cargo check -p op-cognitive-mcp` passes.
- Every schema method either has a confirmed handler (annotated in comment) or a new
  `unimplemented!`-style stub.
- No methods removed from `cognitive_mcp_schema()`.

---

## Task 6 — Full workspace build and smoke-test

**Verify the complete change set compiles and behaves correctly.**

```bash
# 1. Full workspace build
cargo build --workspace

# 2. Clippy (no new warnings)
cargo clippy -p op-plugins -p op-cognitive-mcp --all-targets -- -D warnings

# 3. Confirm rag-ingest dry-run still works
cargo run --bin rag-ingest -- --list 2>/dev/null || true   # list entries
cargo run --bin rag-ingest -- --all --dry-run 2>/dev/null || true

# 4. Confirm embedding_model plugin loads
cargo test -p op-plugins -- embedding_model --nocapture 2>/dev/null || true
```

### Acceptance criteria

- `cargo build --workspace` exits 0.
- `cargo clippy` produces no new `-D warnings` failures in `op-plugins` or `op-cognitive-mcp`.
- `rag-ingest --dry-run` prints the cost estimate table and exits cleanly.
- No `VOYAGE_PUBLIC_URL` or `VOYAGE_MONGODB_URL` constants remain outside `embedding_model.rs`:

```bash
grep -r 'VOYAGE_PUBLIC_URL\|VOYAGE_MONGODB_URL' crates/op-cognitive-mcp/src/
# Expected: no output
```

---

## Summary Table

| Task | Crate(s) | File(s) | Type |
|------|----------|---------|------|
| 1 — Add `voyage_embed_params()` | op-plugins | embedding_model.rs | Add |
| 2 — Refactor VoyageClient | op-cognitive-mcp | voyage.rs | Refactor + delete |
| 3 — Remove duplicate embed in RagPipeline | op-cognitive-mcp | rag_pipeline.rs | Refactor + delete |
| 4 — Server reads cognitive_mcp projection | op-cognitive-mcp | server.rs / main.rs | Wire |
| 5 — Audit schema↔handler alignment | op-cognitive-mcp | grpc_service.rs etc. | Audit + annotate |
| 6 — Full build + smoke-test | both | — | Verify |
