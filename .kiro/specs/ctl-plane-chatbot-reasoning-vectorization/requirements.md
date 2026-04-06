# Requirements

## Introduction
The control-plane chatbot is defined by the `ctl-plane-chatbot` plugin and its canonical schema.
That schema is the single authoritative contract for reasoning episodes, conversation rendering,
vectorization footprints, and any enumerated privacy tags. This spec documents how the emitted
conversation schema flows through the shared schema catalog, the event log, the Voyage vectorizer,
and the Qdrant search surface without introducing a competing data model.

Key architectural invariants:
- The plugin owns one canonical JSON-RCP document (`PluginCatalogDocument`) containing its schema,
  footprint policy, privacy/redaction annotations, and metadata.
- The persisted `op_dbus_model::SqlitePluginCatalog` is the ground-truth store; the in-memory
  `SchemaCatalog`/`SchemaRegistry` is a derived index/caching layer used by validation, rendering,
  vectorization, and compatibility adapters.
- Every downstream artifact (EpisodeRecord helper, JSON renderer, vector payload, logs) is a
  projection of this canonical document; no new schema is invented outside the plugin.
- Schema drift is prevented by ensuring registration writes the canonical document first,
  hydrates the catalog, and then exports to D-Bus/gRPC/embedding components.

## 1. Canonical Plugin Document & Schema Flow

### Intent
The plugin is the schema. The schema is the footprint. The footprint produces the JSON renderable
conversation document and the vectorized footprint. The shared catalog simply indexes that chain.

### Acceptance Criteria
1. WHEN the chatbot plugin registers at startup THEN it builds its schema and persists a
   `PluginCatalogDocument` describing:
   - `schema`: the full `PluginSchema` with fields such as `episode_id`, `goal_text`, `reasoning_summary`,
     `tools_consulted`, `decision_output`, `outcome_class`, `conversation_id`, `content_hash`, and privacy tags.
   - `footprint_tags`: significance hints for semantic/PII filtering (e.g., `reasoning_summary` is `pii` by default).
   - `privacy_config`: which fields must be redacted or masked before embedding or public payloads.
   - `service_name`, `dbus_path`, `storage_path`, and `source` metadata so projections can trace origin.
2. WHEN persistence succeeds THEN the shared `SchemaCatalog` indexes the schema so every consumer reads
   the same JSON Schema, redaction rules, and semantic footprint definition.
3. WHEN schema changes occur (new fields, tags, privacy rules) THEN they are introduced in the
   plugin schema first; all registry projections, JSON outputs, vectors, and compatibility contracts
   automatically follow because they are derived copies of that schema.
4. WHEN a consumer needs schema data (rendering, vectorizing, validation, compatibility exports) THEN
   it resolves the schema through `SchemaCatalog::get_copies` or a similar lookup rather than hardcoding
   the field list.

### Outcome
A single, auditable plugin schema bootstraps reasoning, vectorization, and JSON rendering, with
no segmentation or conflicting schema copies.

## 2. Conversation Episode Lifecycle

### Intent
Reasoning episodes produce schema-conformant documents (`EpisodeRecord` is a typed helper) that
reference the shared schema. Each episode is durable, auditable, and ready for vectorization, while
never inventing new fields outside the schema.

### Acceptance Criteria
1. WHEN reasoning begins THEN the chatbot enters a schema-defined episode state and tracks triggers,
   nested parents, and policy flags (`allow_nested_reasoning`).
2. WHEN reasoning closes THEN the system materializes a schema-conformant episode document containing
   all required schema fields (IDs, timestamps, trigger/exit enums, summary, decision output, tools,
   outcome class, conversation ID, content hash, pii flag, optional confidence, and plugin context).
3. WHEN `pii_flagged` is true THEN schema-owned privacy rules redact or mask the sensitive text
   before storing or exposing the document.
4. WHEN the episode document is ready THEN it is written to the persistent event log before any
   downstream vectorization.
5. WHEN the document is serialized THEN `content_hash` is computed over the canonical serialized form—
   not a subset—ensuring deduplication works reliably.

### Outcome
The system produces deterministic, schema-backed conversation records while never drifting from the
plugin-owned schema contract.

## 3. Vectorization & Footprint Projection

### Intent
Each reasoning episode is vectorized asynchronously, and the embedding worker derives the list of
fields to include/exclude/redact from the shared schema catalog rather than hardcoded logic.

### Acceptance Criteria
1. WHEN the embedding job runs THEN it resolves the schema from `SchemaCatalog`, pulls the
   `semantic_index`, `privacy_index`, and field metadata, and constructs the embedding text accordingly.
2. WHEN a field is tagged `pii` or flagged sensitive THEN the worker omits or masks it before
   embedding input/ public payloads.
3. WHEN fields such as `reasoning_summary`, `decision_output`, `outcome_class`, and `tools_consulted`
   are marked semantic THEN they are eligible for inclusion in the embedding text; otherwise they stay out.
4. WHEN the worker talks to Voyage THEN it uses `input_type="document"` for episodes and `"query"` for
   user queries, and the RPC payload is constrained to the schema-approved footprint.
5. WHEN vectorization completes THEN it immediately upserts the vector+payload into Qdrant; failures
   trigger exponential backoff (max 72h) with structured logging and retry metadata.
6. WHEN deduplication occurs THEN the worker uses `content_hash` (persisted in the document) and the
   schema-defined dedup window to skip redundant upserts.

### Outcome
The embedding pipeline remains schema-driven, deterministic, and consistent with privacy rules.

## 4. Qdrant & Search Payloads

### Intent
The dedicated Qdrant collection remains downstream of the schema catalog. Public/queryable payload
fields come only from schema-approved annotations.

### Acceptance Criteria
1. WHEN the system initialises THEN it creates (if absent) `ctl_plane_reasoning_episodes` with 1024 dims.
2. WHEN writing payloads THEN it includes only the schema-approved fields such as
   `episode_id`, `conversation_id`, `started_at`, `outcome_class`, `trigger`, `tools_consulted`,
   `decision_output`, `reasoning_summary`, `content_hash`, and `pii_flagged` (respecting privacy).
3. WHEN a field is tagged `pii` or redacted THEN it is left out of the payload even if the worker
   previously included it.
4. WHEN search filtering runs THEN the collection supports filters on `outcome_class`, `plugin_id`,
   `conversation_id`, and `started_at` ranges.
5. WHEN schema tags change (e.g., a field becomes semantic) THEN the Qdrant payload automatically
   follows because the worker recomputes field inclusion via the catalog lookup.

### Outcome
Operators get a schema-consistent search surface without a second schema definition.

## 5. Observability & Priority

### Intent
Trace and log vectorization steps with schema context, and keep the embedding worker at the right priority.

### Acceptance Criteria
1. WHEN embedding runs THEN emit `reasoning_episode.vectorized` span attributes from the schema (IDs,
   outcome class, PII flag, latencies, retry count).
2. WHEN Voyage or Qdrant errors occur THEN log at `warn`/`error` respectively with `episode_id` and retries.
3. WHEN the worker is under load THEN keep it at higher priority than mutation embedding but below direct
   control-plane operations, and respect NUMA affinities for queues/storage.
4. WHEN dedup/PII suppression occurs THEN log the schema reason without exposing sensitive values.

### Outcome
Schema-driven logging and priority keep reasoning search responsive and explainable.
