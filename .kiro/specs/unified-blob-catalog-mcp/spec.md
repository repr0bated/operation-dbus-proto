# Spec: unified-blob-catalog-mcp

This document is the single reference that ties requirements to design. It
spells out exactly what changes, in which files, and what the acceptance
criteria are for each piece.

## Scope

Backend only: two new MCP tools (`refresh_blob_vectors`, `search_blob_vectors`),
a new Qdrant collection wired into the existing `QdrantSemanticShuttle`, and
their registration. Chat handler wiring and frontend UI are explicitly deferred.

## Affected Files

| File | Change type |
|------|-------------|
| `crates/op-cognitive-mcp/src/qdrant_shuttle.rs` | Add field, constant, 2 methods, 2 helpers |
| `crates/op-cognitive-mcp/src/blob_vectors_tool.rs` | New file |
| `crates/op-cognitive-mcp/src/cognitive_tools.rs` | Add one `register_blob_vectors_tools` call |
| `crates/op-cognitive-mcp/src/lib.rs` | Add `pub mod blob_vectors_tool` + re-export |

No other crates require changes. `op-blob` and `op-state-store` are already
dependencies of `op-cognitive-mcp`; `qdrant-client` and `uuid` are already in
`Cargo.toml`.

## Detailed Changes

### `qdrant_shuttle.rs`

#### Constants (add alongside existing defaults, lines 18-27)

```rust
const DEFAULT_BLOB_VECTORS_COLLECTION: &str = "blob_vectors";
```

#### Struct field (add to `QdrantSemanticShuttle`, line 36-42 block)

```rust
blob_vectors_collection: String,
```

#### `new()` (add env read alongside `user_memory_collection`, lines ~52-54)

```rust
let blob_vectors_collection = std::env::var("COGNITIVE_MCP_BLOB_VECTORS_COLLECTION")
    .unwrap_or_else(|_| DEFAULT_BLOB_VECTORS_COLLECTION.into());
```

Pass to `new_with_clients(…, blob_vectors_collection, …)`.

#### `new_with_clients()` (add parameter; store in struct; add tracing field)

New parameter: `blob_vectors_collection: impl Into<String>`.
Tracing: add `blob_vectors_collection = %blob_vectors_collection` to the
`tracing::info!` call.

#### Free function `all_blob_embedding_texts()`

```rust
fn all_blob_embedding_texts() -> Result<Vec<(String, String)>> {
    let ids = op_blob::catalog::read_manifest_plugin_ids_shm()
        .context("SHM blob manifest unavailable")?;
    Ok(ids
        .into_iter()
        .filter_map(|id| {
            op_blob::catalog::read_plugin_schema_shm(&id)
                .map(|schema| (id, render_schema_embedding_text(&schema)))
        })
        .collect())
}
```

#### Free function `plugin_id_to_uuid()`

```rust
fn plugin_id_to_uuid(plugin_id: &str) -> uuid::Uuid {
    // Stable namespace — must never change between releases.
    const NS: uuid::Uuid = uuid::Uuid::from_u128(0x6ba7b810_9dad_11d1_80b4_00c04fd430c8);
    uuid::Uuid::new_v5(&NS, plugin_id.as_bytes())
}
```

#### `RefreshBlobVectorsSummary` struct

```rust
#[derive(Debug, serde::Serialize)]
pub struct RefreshBlobVectorsSummary {
    pub embedded: usize,
    pub collection: String,
}
```

#### `refresh_blob_vectors` method on `QdrantSemanticShuttle`

```rust
pub async fn refresh_blob_vectors(&self) -> Result<RefreshBlobVectorsSummary> {
    let texts = all_blob_embedding_texts()?;
    let mut points = Vec::with_capacity(texts.len());
    for (plugin_id, text) in &texts {
        let vector = self.embed_document(text).await
            .with_context(|| format!("embed failed for plugin '{plugin_id}'"))?;
        let payload: Payload = serde_json::json!({
            "plugin_id": plugin_id,
            "text": text,
        })
        .try_into()
        .context("failed to build payload")?;
        points.push(PointStruct::new(
            plugin_id_to_uuid(plugin_id).to_string(),
            vector,
            payload,
        ));
    }
    let embedded = points.len();
    self.client
        .upsert_points(
            UpsertPointsBuilder::new(self.blob_vectors_collection.clone(), points)
        )
        .await
        .with_context(|| format!(
            "upsert into '{}' failed", self.blob_vectors_collection
        ))?;
    tracing::info!(collection = %self.blob_vectors_collection, embedded,
        "blob_vectors collection refreshed");
    Ok(RefreshBlobVectorsSummary { embedded, collection: self.blob_vectors_collection.clone() })
}
```

#### `search_blob_vectors` method on `QdrantSemanticShuttle`

```rust
pub async fn search_blob_vectors(&self, query: &str, limit: u64) -> Result<Vec<ScoredPoint>> {
    let embedding = self.embed_query_text(query).await
        .context("embed_query_text failed for blob_vectors search")?;
    let response = self.client
        .query(
            QueryPointsBuilder::new(self.blob_vectors_collection.clone())
                .query(embedding)
                .limit(limit)
                .with_payload(true),
        )
        .await
        .with_context(|| format!(
            "query against '{}' failed", self.blob_vectors_collection
        ))?;
    tracing::info!(collection = %self.blob_vectors_collection, matches = response.result.len(),
        "blob_vectors search completed");
    Ok(response.result)
}
```

### `blob_vectors_tool.rs` (new file)

Full file content (module-level doc, imports, two tool structs, registration fn):

```rust
//! MCP tools for the blob-vector Qdrant collection.
//!
//! `refresh_blob_vectors` — user-triggered wholesale rebuild of the
//!   `blob_vectors` collection (one embedding per active plugin schema).
//!
//! `search_blob_vectors` — semantic search over the collection using a
//!   free-text query; backing call for the "Vectors" context modifier.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_mcp::tool_registry::{BoxedTool, Tool, ToolRegistry};
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

use crate::qdrant_shuttle::QdrantSemanticShuttle;

pub async fn register_blob_vectors_tools(
    registry: &ToolRegistry,
    qdrant: Option<Arc<QdrantSemanticShuttle>>,
) -> Result<()> {
    registry
        .register(Arc::new(RefreshBlobVectorsTool { shuttle: qdrant.clone() }) as BoxedTool)
        .await?;
    registry
        .register(Arc::new(SearchBlobVectorsTool { shuttle: qdrant }) as BoxedTool)
        .await?;
    Ok(())
}

// ── refresh_blob_vectors ──────────────────────────────────────────────────────

struct RefreshBlobVectorsTool {
    shuttle: Option<Arc<QdrantSemanticShuttle>>,
}

#[async_trait]
impl Tool for RefreshBlobVectorsTool {
    fn name(&self) -> &str { "refresh_blob_vectors" }

    fn description(&self) -> &str {
        "Rebuild the blob_vectors Qdrant collection from scratch: embeds every \
         active plugin's schema text via Voyage and upserts all points. \
         User-triggered only — never runs automatically."
    }

    fn category(&self) -> &str { "schema" }
    fn namespace(&self) -> &str { "plugins" }
    fn tags(&self) -> Vec<String> {
        vec!["schema".into(), "blob".into(), "vectors".into(), "refresh".into()]
    }

    fn input_schema(&self) -> Value {
        json!({"type": "object", "additionalProperties": false})
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let shuttle = self.shuttle.as_ref().ok_or_else(|| anyhow::anyhow!(
            "refresh_blob_vectors unavailable: Qdrant Semantic Shuttle is not configured"
        ))?;
        let summary = shuttle.refresh_blob_vectors().await
            .context("refresh_blob_vectors failed")?;
        Ok(json!({
            "ok": true,
            "embedded": summary.embedded,
            "collection": summary.collection,
        }))
    }
}

// ── search_blob_vectors ───────────────────────────────────────────────────────

struct SearchBlobVectorsTool {
    shuttle: Option<Arc<QdrantSemanticShuttle>>,
}

#[async_trait]
impl Tool for SearchBlobVectorsTool {
    fn name(&self) -> &str { "search_blob_vectors" }

    fn description(&self) -> &str {
        "Semantic search over the blob_vectors collection. Returns the top-k \
         plugin schemas most relevant to the query. Backing call for the \
         Vectors context modifier; also usable standalone."
    }

    fn category(&self) -> &str { "schema" }
    fn namespace(&self) -> &str { "plugins" }
    fn tags(&self) -> Vec<String> {
        vec!["schema".into(), "blob".into(), "vectors".into(), "search".into()]
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Free-text query — embedded with voyage-4 for similarity search"
                },
                "limit": {
                    "type": "integer",
                    "default": 10,
                    "minimum": 1,
                    "description": "Maximum number of results to return"
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let shuttle = self.shuttle.as_ref().ok_or_else(|| anyhow::anyhow!(
            "search_blob_vectors unavailable: Qdrant Semantic Shuttle is not configured"
        ))?;
        let query = input["query"].as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: query"))?;
        let limit = input["limit"].as_i64().unwrap_or(10).max(1) as u64;

        let results = shuttle.search_blob_vectors(query, limit).await
            .context("search_blob_vectors failed")?;

        let items: Vec<Value> = results.into_iter().map(|pt| {
            let plugin_id = pt.payload.get("plugin_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let text = pt.payload.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            json!({
                "plugin_id": plugin_id,
                "score": pt.score,
                "text": text,
            })
        }).collect();

        Ok(json!({ "count": items.len(), "results": items }))
    }
}
```

### `cognitive_tools.rs`

In `CognitiveToolRegistry::register_all`, add after `register_blob_catalog_tool`:

```rust
crate::blob_vectors_tool::register_blob_vectors_tools(registry, qdrant).await?;
```

`qdrant` is already `Option<Arc<QdrantSemanticShuttle>>`; pass it directly. The
existing `qdrant.clone()` before `MemoryTool::new` must also clone before this
new call. Restructure as:

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
    register_blob_vectors_tools(registry, qdrant).await?;
    Ok(())
}
```

### `lib.rs`

Add:

```rust
pub mod blob_vectors_tool;
pub use blob_vectors_tool::register_blob_vectors_tools;
```

## Acceptance Criteria

### AC-1: `refresh_blob_vectors` MCP tool

- `cargo build -p op-cognitive-mcp` compiles with no errors or warnings.
- Tool is discoverable in the MCP tool list when the cognitive MCP server starts.
- When Qdrant is unavailable (`None` shuttle), calling the tool returns an error
  result with message containing "not configured", not a panic.
- When Qdrant is available, calling the tool returns `{ ok: true, embedded: N, collection: "blob_vectors" }`
  where `N` equals the number of plugin ids returned by `read_manifest_plugin_ids_shm()`.
- Calling the tool twice in succession produces the same `embedded` count and
  does not accumulate duplicate points (same Qdrant point id for same plugin id).

### AC-2: `search_blob_vectors` MCP tool

- After calling `refresh_blob_vectors`, calling `search_blob_vectors` with a
  query that matches a known plugin name returns that plugin in the top results
  with a score > 0.0.
- `limit` field defaults to 10; passing `limit: 3` returns at most 3 results.
- Missing `query` field returns an error result, not a panic.
- When Qdrant is unavailable, returns an error result containing "not configured".

### AC-3: Collection isolation

- No existing tests for `ctl_plane_reasoning_episodes` or `user_memory` behaviour
  change (compile-time check via `cargo test -p op-cognitive-mcp`).
- The new collection name is read from `COGNITIVE_MCP_BLOB_VECTORS_COLLECTION`
  env var when set; falls back to `"blob_vectors"` otherwise.

### AC-4: Idempotent point IDs

- `plugin_id_to_uuid("zeroclaw")` returns the same UUID on every call (unit test).

### AC-5: `render_schema_embedding_text` coverage

- `all_blob_embedding_texts()` returns a non-empty `Vec` when at least one
  plugin blob is present in SHM. This can be verified in integration context
  without a Qdrant instance (the function reads SHM only).

## Deferred (not in this spec)

- Chat handler source-selection wiring (MCP / Related / Live / Vectors toggles
  plumbed into `zeroclaw_chat_handler` request assembly).
- Frontend toggle UI (three-group layout per R5).
- "Related" one-hop dependency assembly (inline handler logic, not a tool).
- "Live" toggle path (already exists as `get_projection`; needs wiring).
