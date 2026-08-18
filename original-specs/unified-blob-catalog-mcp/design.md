# Design: unified-blob-catalog-mcp (Kiro re-derivation)

## Decisions

### D1 — Refresh action lives in `op-cognitive-mcp` as an MCP tool

`op-cognitive-mcp` already depends on `op-blob` (confirmed in `Cargo.toml:9`),
owns `QdrantSemanticShuttle` and `VoyageClient`, and has the established pattern
for optional-Qdrant tools (`MemoryTool`, `RegisterToolTool`). Adding Qdrant +
Voyage as dependencies to `op-plugins` would violate the declared boundary: that
crate declares state contracts, it does not own runtime I/O pipelines. An MCP
tool alongside `blob_catalog_tool.rs` is the correct placement.

### D2 — "Related" is one hop, not recursive

"Related" is intended to be a targeted, cheaper alternative to "MCP" (full dump).
Recursive traversal over a moderately connected dependency graph converges toward
the same set as "MCP", collapsing the distinction between the two toggles.
One hop keeps them meaningfully different in every realistic case.

### D3 — No staleness check; wholesale upsert on refresh

Total corpus is <2MB (all active plugin schemas combined). A `schema_hash`-based
skip-unchanged optimization adds conditional logic and state (the hash must be
stored somewhere) without measurable performance benefit at this scale.
Wholesale upsert on every refresh is simpler and correct.

### D4 — Point IDs derived from `plugin_id` via stable hash

Qdrant point IDs must be either `uint64` or UUID. `plugin_id` is a string
(e.g. `"zeroclaw"`). We derive a deterministic UUID from the plugin id using
`uuid::Uuid::new_v5(&UUID_NAMESPACE, plugin_id.as_bytes())`, which gives a
stable UUID for each plugin id so re-running refresh upserts the same point
rather than accumulating duplicates. No secondary deduplication step needed.

The namespace UUID for the Uuid v5 derivation is a fixed project-local constant
(not `Uuid::NAMESPACE_URL` or `NAMESPACE_DNS`) defined once in the new tool file.

### D5 — Sequential embedding, not concurrent

The corpus is small and refresh is an infrequent, user-triggered action.
A sequential `for` loop over plugin ids avoids semaphore/join complexity and
matches the precedent in the existing embedding paths. If the corpus grows to
the point where latency matters, a bounded `futures::stream::iter(...).buffered(N)`
can be substituted without changing the caller interface.

### D6 — `render_schema_embedding_text` visibility

The function is currently `fn` (module-private). The new callers are in the same
module (`qdrant_shuttle.rs`), so no visibility change is needed. If a future
caller in a different module needs it, promote to `pub(crate)` at that point.

### D7 — "Related" and "Live" are chat-layer assembly logic, not MCP tools

"Related" is a cheap SHM read loop (no I/O beyond the catalog) and "Live" is a
direct call to `get_projection`. Neither warrants a new MCP tool. They are
inline logic in the chat request handler where context is assembled. This keeps
the MCP tool surface minimal (only the two tools that need persistent state:
`blob_catalog` for the full dump, `refresh_blob_vectors` / `search_blob_vectors`
for the Qdrant-backed vector flow).

## Component Design

### 1. New collection field in `QdrantSemanticShuttle`

File: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs`

Add a constant and a new field, following the exact pattern of
`DEFAULT_USER_MEMORY_COLLECTION` / `user_memory_collection`:

```rust
const DEFAULT_BLOB_VECTORS_COLLECTION: &str = "blob_vectors";
// env: COGNITIVE_MCP_BLOB_VECTORS_COLLECTION

pub struct QdrantSemanticShuttle {
    client: Qdrant,
    collection_name: String,
    user_memory_collection: String,
    blob_vectors_collection: String,  // ← new
    sled_path: PathBuf,
    voyage_client: VoyageClient,
}
```

Thread through `new()` (reads env var, defaults to constant) and
`new_with_clients()` (new parameter). Both functions are `pub` / private
respectively; `new()` is the public entry point. `new_with_clients` signature
becomes:

```rust
async fn new_with_clients(
    qdrant_url: &str,
    collection_name: impl Into<String>,
    user_memory_collection: impl Into<String>,
    blob_vectors_collection: impl Into<String>,  // ← new
    sled_path: impl Into<PathBuf>,
    voyage_client: VoyageClient,
) -> Result<Self>
```

The `tracing::info!` call in `new_with_clients` gains a
`blob_vectors_collection = %blob_vectors_collection` field.

### 2. `all_blob_embedding_texts` helper

New private function in `qdrant_shuttle.rs`, alongside
`current_schema_embedding_text()`:

```rust
/// Returns (plugin_id, embedding_text) for every active plugin in the SHM
/// blob catalog. The multi-plugin counterpart to `current_schema_embedding_text`,
/// which only covers the single sled-resident schema.
fn all_blob_embedding_texts() -> Result<Vec<(String, String)>> {
    let ids = op_blob::catalog::read_manifest_plugin_ids_shm()
        .context("SHM blob manifest is unavailable")?;
    Ok(ids
        .into_iter()
        .filter_map(|id| {
            op_blob::catalog::read_plugin_schema_shm(&id)
                .map(|schema| (id, render_schema_embedding_text(&schema)))
        })
        .collect())
}
```

Note: this is a free function (not a method) because it needs no `self` — it
uses only the public catalog functions and the already-private
`render_schema_embedding_text`. `filter_map` silently skips plugins whose SHM
file is missing between manifest read and schema read (TOCTOU window is small
and the skip is safe — a fresh refresh will pick them up next time).

### 3. `refresh_blob_vectors` method

New `pub async fn` on `QdrantSemanticShuttle`:

```rust
pub async fn refresh_blob_vectors(&self) -> Result<RefreshBlobVectorsSummary> {
    let texts = all_blob_embedding_texts()?;
    let mut points = Vec::with_capacity(texts.len());

    for (plugin_id, text) in &texts {
        let vector = self.embed_document(text).await
            .with_context(|| format!("failed to embed schema text for plugin '{plugin_id}'"))?;

        let point_id = plugin_id_to_uuid(plugin_id).to_string();
        let payload: Payload = serde_json::json!({
            "plugin_id": plugin_id,
            "text": text,
        })
        .try_into()
        .context("failed to build blob_vectors payload")?;

        points.push(PointStruct::new(point_id, vector, payload));
    }

    let embedded = points.len();
    self.client
        .upsert_points(
            UpsertPointsBuilder::new(self.blob_vectors_collection.clone(), points)
        )
        .await
        .with_context(|| format!(
            "failed to upsert {} points into collection '{}'",
            embedded, self.blob_vectors_collection
        ))?;

    tracing::info!(
        collection = %self.blob_vectors_collection,
        embedded,
        "blob_vectors collection refreshed"
    );

    Ok(RefreshBlobVectorsSummary {
        embedded,
        collection: self.blob_vectors_collection.clone(),
    })
}
```

### 4. `search_blob_vectors` method

New `pub async fn` on `QdrantSemanticShuttle`:

```rust
pub async fn search_blob_vectors(
    &self,
    query: &str,
    limit: u64,
) -> Result<Vec<ScoredPoint>> {
    let embedding = self.embed_query_text(query).await
        .context("failed to embed query for blob_vectors search")?;

    let response = self.client
        .query(
            QueryPointsBuilder::new(self.blob_vectors_collection.clone())
                .query(embedding)
                .limit(limit)
                .with_payload(true),
        )
        .await
        .with_context(|| format!(
            "failed semantic query against collection '{}'",
            self.blob_vectors_collection
        ))?;

    tracing::info!(
        collection = %self.blob_vectors_collection,
        query_len = query.len(),
        matches = response.result.len(),
        "blob_vectors semantic search completed"
    );

    Ok(response.result)
}
```

No `container_id` filter — the whole collection is public catalog data.
`limit` of 0 should be normalised to a sane default (e.g. 10) by callers,
matching the pattern in `MemoryTool::op_semantic_query`.

### 5. Supporting types

```rust
#[derive(Debug, serde::Serialize)]
pub struct RefreshBlobVectorsSummary {
    pub embedded: usize,
    pub collection: String,
}
```

```rust
/// Deterministic UUID v5 from plugin_id, so upsert is idempotent.
fn plugin_id_to_uuid(plugin_id: &str) -> uuid::Uuid {
    // Project-local namespace; must remain stable across releases.
    const BLOB_VECTORS_NS: uuid::Uuid =
        uuid::Uuid::from_u128(0x6ba7b810_9dad_11d1_80b4_00c04fd430c8); // reuse DNS ns or define own
    uuid::Uuid::new_v5(&BLOB_VECTORS_NS, plugin_id.as_bytes())
}
```

(The namespace UUID above uses the standard DNS namespace UUID as a stand-in;
the actual constant can be any fixed value — the important property is that it
never changes.)

### 6. New file: `blob_vectors_tool.rs`

File: `crates/op-cognitive-mcp/src/blob_vectors_tool.rs`

Two tools in one file (same domain), following the `blob_catalog_tool.rs` shape.

**`RefreshBlobVectorsTool`**

```rust
struct RefreshBlobVectorsTool {
    shuttle: Option<Arc<QdrantSemanticShuttle>>,
}

impl Tool for RefreshBlobVectorsTool {
    fn name(&self) -> &str { "refresh_blob_vectors" }
    fn category(&self) -> &str { "schema" }
    fn namespace(&self) -> &str { "plugins" }
    fn input_schema(&self) -> Value {
        json!({"type": "object", "additionalProperties": false})
    }
    async fn execute(&self, _input: Value) -> Result<Value> {
        let shuttle = self.shuttle.as_ref().ok_or_else(|| anyhow::anyhow!(
            "blob vector refresh unavailable: Qdrant Semantic Shuttle is not configured"
        ))?;
        let summary = shuttle.refresh_blob_vectors().await?;
        Ok(json!({
            "ok": true,
            "embedded": summary.embedded,
            "collection": summary.collection,
        }))
    }
}
```

**`SearchBlobVectorsTool`**

```rust
struct SearchBlobVectorsTool {
    shuttle: Option<Arc<QdrantSemanticShuttle>>,
}

impl Tool for SearchBlobVectorsTool {
    fn name(&self) -> &str { "search_blob_vectors" }
    fn category(&self) -> &str { "schema" }
    fn namespace(&self) -> &str { "plugins" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer", "default": 10 }
            },
            "additionalProperties": false
        })
    }
    async fn execute(&self, input: Value) -> Result<Value> {
        let shuttle = self.shuttle.as_ref().ok_or_else(|| anyhow::anyhow!(
            "blob vector search unavailable: Qdrant Semantic Shuttle is not configured"
        ))?;
        let query = input["query"].as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: query"))?;
        let limit = input["limit"].as_i64().unwrap_or(10).max(1) as u64;
        let results = shuttle.search_blob_vectors(query, limit).await?;
        let items: Vec<Value> = results.into_iter().map(|pt| json!({
            "plugin_id": pt.payload.get("plugin_id")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "score": pt.score,
            "text": pt.payload.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
        })).collect();
        Ok(json!({ "count": items.len(), "results": items }))
    }
}
```

**Registration function**:

```rust
pub async fn register_blob_vectors_tools(
    registry: &ToolRegistry,
    qdrant: Option<Arc<QdrantSemanticShuttle>>,
) -> Result<()> {
    registry.register(Arc::new(RefreshBlobVectorsTool { shuttle: qdrant.clone() }) as BoxedTool).await?;
    registry.register(Arc::new(SearchBlobVectorsTool { shuttle: qdrant }) as BoxedTool).await?;
    Ok(())
}
```

### 7. Wire into `CognitiveToolRegistry::register_all`

File: `crates/op-cognitive-mcp/src/cognitive_tools.rs`

```rust
pub async fn register_all(
    registry: &ToolRegistry,
    store: Arc<CognitiveMemoryStore>,
    qdrant: Option<Arc<QdrantSemanticShuttle>>,
) -> Result<()> {
    registry.register(Arc::new(MemoryTool::new(store.clone(), qdrant.clone())) as BoxedTool).await?;
    registry.register(Arc::new(RegisterToolTool) as BoxedTool).await?;
    register_agent_tools(registry).await?;
    register_notebooklm_tools(registry).await?;
    register_blob_catalog_tool(registry).await?;
    register_blob_vectors_tools(registry, qdrant).await?;  // ← new
    Ok(())
}
```

The `qdrant` parameter is already `Option<Arc<…>>` — the new call passes it
through unchanged, consistent with `MemoryTool::new(store, qdrant)`.

### 8. "Related" context assembly (deferred to chat integration)

"Related" is not implemented as an MCP tool. When the chat integration work is
scoped, the assembly logic in the zeroclaw handler (or wherever context is built)
will:

```rust
// For each plugin_id already in context:
let mut related_ids: Vec<String> = vec![];
for id in &context_plugin_ids {
    if let Some(schema) = read_plugin_schema_shm(id) {
        related_ids.extend(schema.dependencies.iter().cloned());
    }
}
related_ids.sort();
related_ids.dedup();
// Load schemas for related_ids (excluding those already in context)
```

One hop; no recursion into `related_ids`' own dependencies.

### 9. `lib.rs` export

Add to `crates/op-cognitive-mcp/src/lib.rs`:

```rust
pub mod blob_vectors_tool;
pub use blob_vectors_tool::register_blob_vectors_tools;
```

## What this design does NOT cover

- **Chat handler wiring for source selection** (R3 toggle state + request
  assembly): the existing `zeroclaw_chat_handler` / `zeroclaw_schema_handler`
  are the likely integration points, but changing them is a separate scope of
  work after the vector-collection piece is built and tested standalone.
- **Frontend toggle UI** (R5): the three-group layout (content sources /
  Vectors modifier / Refresh button) is frontend work, separate from this
  backend design.
- **"Live" toggle wiring**: `get_projection` already exists and works; plumbing
  it into a toggle-controlled path belongs to the chat integration piece.
