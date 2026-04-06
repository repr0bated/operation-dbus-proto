# Control-Plane Chatbot Reasoning Episode Vectorization — Design

## Architecture Overview

```mermaid
graph TD
    CB[ctl-plane-chatbot plugin]
    CB -->|registers canonical document| PD[PluginCatalogDocument]
    PD -->|persists schema| SC[SqlitePluginCatalog]
    SC -->|hydrates| CA[SchemaCatalog (in-memory)]
    CA -->|consumed by| ER[EpisodeRecord projection]
    ER -->|writes| EL[Event Log]
    ER -->|enqueues| EQ[EmbeddingQueue]
    EQ -->|uses schema| VA[Voyage API]
    VA -->|vector + payload| QD[Qdrant ctl_plane_reasoning_episodes]
    QD -->|searched by| HO[Human Operator]
    HO -->|query via| VA2[Voyage API (query)]
```

Every node downstream is a projection of the canonical plugin document. No component invents its own schema—
the catalog is the shared index, and the embedding/Qdrant surfaces follow its footprint rules.

## Crate Placement

| Component | Crate | Notes |
|---|---|---|
| Canonical plugin document and schema registration | `op-plugins` | `ctl-plane-chatbot` plugin builds schema-as-code and persists `PluginCatalogDocument`.
| Schema catalog and compatibility helpers | `op-state-store` | Provides `SchemaCatalog`, `PluginSchema`, and the `SqlitePluginCatalog` store.
| Reasoning lifecycle + embedding queue | `op-cognitive-mcp` | Hosts `ReasoningStateManager`, `EpisodeRecord`, `EmbeddingQueue`, `voyage.rs`, `qdrant.rs`.
| D-Bus/gRPC projections | `op-state` / `op-dbus` | Consume the shared catalog for validation and serialization.
| Human tooling & search | `op-web` / `op-grpc-bridge` | Resolve schema via catalog for rendering/tooling.

## Module Details

### 1. Plugin Schema Document (`op-plugins`)
- Builds the schema once via `StatePlugin::schema()` with fields such as `episode_id`, `conversation_id`, `goal_text`, `reasoning_summary`, `decision_output`, `tools_consulted`, `reasoning_trace`, `outcome_class`, `started_at`, `ended_at`, `content_hash`, `pii_flagged`, and optional `confidence`.
- Marks sensitive fields with privacy tags (e.g., `pii`) and provides a `semantic_index` section describing which fields belong in embeddings or public payloads.
- Persists the document as `PluginCatalogDocument` → stored by `op_dbus_model::SqlitePluginCatalog` → read by `op_plugins::PluginCatalog` to hydrate `SchemaCatalog`.
- Exposes metadata such as `service_name`, `dbus_path`, `storage_path`, and `source` for projections.

### 2. Schema Catalog (`op-state-store`)
- `SchemaCatalog` indexes persisted schema copies and exposes derived data (`json_schema`, `contract_schema`).
- Consumers call `catalog.get_copies("ctl-plane-chatbot")` to obtain schema properties, privacy annotations, and derived tags.
- No new schema definitions are introduced here; this is purely a caching/index layer.

### 3. Reasoning Lifecycle (`op-cognitive-mcp`)
- `ReasoningStateManager` tracks nested episodes, applies the `allow_nested_reasoning` policy, and emits transition spans.
- When closing an episode, it materializes a schema-conformant helper (`EpisodeRecord`) and writes it to the event log before enqueueing embedding.
- `EpisodeRecord` mirrors the schema fields; it never adds new fields that are not in the canonical plugin schema.

### 4. Embedding Queue & Voyage (`op-cognitive-mcp`)
- The queue persists pending episodes in SQLite (reusing patterns from `op-state-store`).
- Worker resolves schema through `SchemaCatalog`, applies privacy tags, builds deterministic embedding text, and calls `VoyageClient` (`voyage-4-lite`, `input_type="document"`).
- PII-flagged episodes skip sensitive fields; semantic tags determine which fields go into the text.
- Success triggers immediate Qdrant upsert; failures log retries, apply exponential backoff (max 72h), and emit structured spans.

### 5. Vector Storage (`op-cognitive-mcp`)
- Qdrant collection `ctl_plane_reasoning_episodes` is created with 1024 dimensions.
- Payload fields are taken from schema-approved public/queryable footprint (`episode_id`, `conversation_id`, `started_at`, `outcome_class`, `trigger`, `decision_output`, `tools_consulted`, `content_hash`, `pii_flagged`).
- Dedup checks use `content_hash` and the catalog-provided dedup window (default 24h). Filterable fields include `outcome_class`, `plugin_id`, `conversation_id`, `started_at`.

### 6. Human Search & Querying
- Human operators call Voyage with `input_type="query"`; the query text is derived from schema-defined search fields.
- Search results include schema-approved fields only.
- JSON renderers and gRPC endpoints resolve output structures via the same shared catalog to avoid drift.

### 7. Observability & Priority
- Embedding spans (`reasoning_episode.vectorized`) include schema-based attributes (`episode_id`, `outcome_class`, `pii_flagged`, latencies).
- Voyage/Qdrant failures log at `warn`/`error` with `episode_id` and retry metadata.
- Worker scheduling respects priority order: control-plane ops > reasoning embedding > schema footprint embedding > mutation embedding. NUMA affinity covers local queue/storage, not remote inference.

### 8. Compatibility Exposure
- Legacy contract adapters (e.g., `op-plugins::state_plugins::schema_contract`) wrap catalog lookups to tempt older consumers without introducing new schema.
- `SchemaCatalog::export_contract_for("ctl-plane-chatbot")` can still provide compatibility views derived from the canonical schema.

## Data Flow

1. Plugin registers `ctl-plane-chatbot` schema → canonical document persisted → catalog hydrated.
2. Chatbot reasoning lifecycle produces a schema-conformant record → writes to event log → enqueues embedding.
3. Embedding worker resolves schema → constructs text → calls Voyage → upserts Qdrant.
4. Human queries hit Voyage with schema-approved query text and read results from schema-derived payloads.

Every step reads through the same shared catalog, so there is only one schema source of truth.
