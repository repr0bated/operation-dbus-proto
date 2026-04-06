# Requirements — Blockchain Hashed Footprint for System Mutations

## Introduction

Every mutation applied to the system through the plugin + schema flow must produce a
cryptographically hashed footprint that is appended to the immutable blockchain audit trail
maintained by `op-blockchain`. The footprint plugin (`mutation_footprint`) is the canonical
plugin that owns this audit trail. It registers once at startup, produces a `PluginCatalogDocument`
describing its schema, and receives mutation events from every other state plugin via a shared
async channel.

Key architectural invariants:
- The `mutation_footprint` plugin is the single schema authority for mutation audit records.
  No other component invents a competing audit record shape.
- Every mutation is captured as a `PluginFootprint` — containing the source plugin ID, operation
  type, SHA-256 hashes of the old and new state, a chained block hash linking to the previous
  footprint, and full metadata — before being appended to the `StreamingBlockchain`.
- The blockchain's `timing_subvol` is append-only. Once a footprint block is written it is
  never modified; only new blocks are added. Snapshots and retention policy control pruning.
- The plugin schema is the ground truth for validation, rendering, vectorization, and compliance
  queries. All downstream consumers (gRPC projections, JSON renderers, embedding workers) resolve
  the audit record shape through `SchemaCatalog::get_copies("mutation_footprint")` — not
  hardcoded field lists.
- Mutations must not bypass the plugin + schema flow per the design note in the plugin catalog.

---

## 1. Canonical Plugin Document & Schema Flow

### Intent

The `mutation_footprint` plugin owns its canonical schema. That schema defines every field in an
audit record: identifiers, state hashes, chain linkage, actor context, and semantic/privacy tags.
The shared catalog indexes it so all projections read the same contract.

### Acceptance Criteria

1. WHEN `mutation_footprint` registers at startup THEN it persists a `PluginCatalogDocument`
   containing:
   - `schema`: a full `PluginSchema` with fields:
     - `footprint_id` (UUID, immutable, semantic)
     - `plugin_source` (string — the originating plugin name, semantic)
     - `operation_type` (enum: `create` | `update` | `delete` | `apply`, semantic)
     - `old_state_hash` (SHA-256 hex, semantic)
     - `new_state_hash` (SHA-256 hex, semantic)
     - `content_hash` (SHA-256 hex of the full footprint payload, semantic — dedup key)
     - `prev_block_hash` (SHA-256 hex of the preceding block, semantic — chain link)
     - `timestamp_ms` (u64 milliseconds since epoch, semantic)
     - `actor` (string — principal/user/service that triggered the mutation, optional)
     - `diff_summary` (JSON object — the computed diff between old and new state, pii-capable)
     - `metadata` (JSON object — arbitrary plugin-supplied key-value pairs)
   - `footprint_tags`: `footprint_id`, `plugin_source`, `operation_type`, `old_state_hash`,
     `new_state_hash`, `content_hash`, `prev_block_hash`, `timestamp_ms` are marked semantic.
     `diff_summary` and `actor` may be tagged `pii` if the plugin source marks them sensitive.
   - `privacy_config`: `diff_summary` and `actor` are redacted from public payloads when the
     originating plugin marks its mutation data as sensitive.
   - `service_name`, `storage_path`, and `source` metadata so projections can trace origin.

2. WHEN schema registration succeeds THEN `SchemaCatalog` indexes the schema and every consumer
   resolves audit record shape through the catalog, not hardcoded field lists.

3. WHEN schema fields change THEN the change is introduced in `mutation_footprint::schema()` first;
   all downstream projections follow automatically via catalog lookup.

### Outcome

A single, auditable schema owned by `mutation_footprint` governs every blockchain record. No
component can drift from the canonical field set.

---

## 2. Mutation Interception & Footprint Generation

### Intent

Every `apply_state` call on any registered state plugin must produce a footprint before the state
change is considered complete. The footprint captures before/after hashes and chains to the
previous block.

### Acceptance Criteria

1. WHEN any state plugin executes `apply_state` THEN the plugin runtime captures the old state
   (pre-apply) and new state (post-apply) and sends a `MutationEvent` to the
   `mutation_footprint` plugin's inbound channel before returning success to the caller.

2. WHEN the `MutationEvent` is received THEN the plugin:
   a. Computes `old_state_hash = SHA-256(canonical_json(old_state))`.
   b. Computes `new_state_hash = SHA-256(canonical_json(new_state))`.
   c. Reads the last known `prev_block_hash` from the chain head (in-memory, protected by a
      `RwLock`; on first block uses the genesis sentinel `"0000…0000"`).
   d. Computes `content_hash = SHA-256(footprint_id || plugin_source || operation_type ||
      old_state_hash || new_state_hash || prev_block_hash || timestamp_ms)`.
   e. Constructs a `PluginFootprint` with all schema-required fields.
   f. Updates the chain head to `content_hash`.

3. WHEN `content_hash` equals the chain head for an already-seen record THEN the footprint is a
   duplicate; it is logged and discarded without appending to the chain.

4. WHEN the originating plugin has `sensitive = true` in its metadata THEN `diff_summary` and
   `actor` are marked `pii_flagged = true` before persisting.

5. WHEN any step in footprint generation fails THEN the error is logged at `error` level with
   `plugin_source`, `operation_type`, and `footprint_id`; the mutation itself is not rolled back
   but the audit gap is surfaced via structured telemetry.

### Outcome

Every system mutation has a corresponding blockchain entry with a verifiable hash chain. Observers
can detect tampering by recomputing hashes and checking chain continuity.

---

## 3. Blockchain Persistence & BTRFS Storage

### Intent

Footprints are written to the `StreamingBlockchain`'s immutable `timing_subvol` and optionally
to `vector_subvol` when semantic features are available. Snapshots and retention policy preserve
the audit history within configurable rolling windows.

### Acceptance Criteria

1. WHEN a footprint is ready THEN it is submitted to `StreamingBlockchain::add_footprint` which
   writes a JSON block file to `timing_subvol/block-{N:012}.json` atomically.

2. WHEN the `ml` cargo feature is enabled THEN `FootprintGenerator` uses transformer embeddings
   for `vector_features`; otherwise heuristic 64-dimensional features are used. The vector is
   stored in `vector_subvol`.

3. WHEN a snapshot interval elapses (configurable via `OPDBUS_SNAPSHOT_INTERVAL`, defaulting to
   `every-15-minutes`) THEN the blockchain creates a read-only BTRFS snapshot of `state_subvol`.
   The `timing_subvol` is never snapshotted-and-pruned; it is append-only for the life of the
   audit trail.

4. WHEN the retention policy fires THEN old `state_subvol` snapshots are pruned according to
   `RetentionPolicy` (hourly/daily/weekly/quarterly windows configurable via env vars). Block
   files in `timing_subvol` are never pruned; they constitute the permanent audit ledger.

5. WHEN BTRFS is unavailable THEN the system falls back to regular directories and the audit
   trail continues operating with degraded snapshot capability. The footprint record logs the
   storage backend in use.

6. WHEN `btrfs send` / remote replication is triggered THEN the snapshot is streamed to the
   configured remote path, keeping an off-site copy of the audit trail.

### Outcome

The audit ledger is durable, append-only, and replicated. Snapshots allow point-in-time system
state recovery without truncating the block history.

---

## 4. Chain Integrity & Tamper Detection

### Intent

The chain-link property (`prev_block_hash`) makes the audit trail tamper-evident. Any inserted,
deleted, or modified block breaks the hash chain and can be detected by a verification pass.

### Acceptance Criteria

1. WHEN chain verification is requested THEN the system replays `timing_subvol/block-*.json`
   files in sequence, recomputes each `content_hash`, and confirms each block's `prev_block_hash`
   equals the preceding block's `content_hash`.

2. WHEN a broken link is found THEN verification reports the first block index where the chain
   breaks, the expected hash, and the stored hash.

3. WHEN genesis verification runs THEN the first block's `prev_block_hash` must equal the
   configured genesis sentinel value (default `"0" × 64`).

4. WHEN the chain head is queried THEN the plugin returns the `content_hash` of the most recently
   appended block without re-reading all block files.

5. WHEN the system restarts THEN `mutation_footprint` replays the last block file to restore the
   in-memory chain head before accepting new mutations.

### Outcome

Any tampering with the blockchain record — modification, insertion, or deletion of blocks — is
detectable without external infrastructure.

---

## 5. Vectorization & Semantic Search

### Intent

Mutation footprints are optionally vectorized so operators can perform semantic similarity queries
across the audit trail (e.g., "find mutations similar to this network config change").

### Acceptance Criteria

1. WHEN the `ml` feature is active THEN the embedding worker resolves the `mutation_footprint`
   schema from `SchemaCatalog`, constructs embedding text from fields tagged `semantic` (excluding
   `pii`-flagged content), and calls the configured vector backend.

2. WHEN `pii_flagged = true` THEN `diff_summary` and `actor` are omitted from the embedding text.

3. WHEN vectorization completes THEN the vector is stored in `vector_subvol` alongside the block
   for semantic retrieval.

4. WHEN vectorization fails THEN the block is still committed to `timing_subvol`; the vector is
   queued for retry with exponential backoff. Audit integrity does not depend on vectorization.

5. WHEN a semantic search is issued THEN results include only schema-approved fields; no raw state
   payloads or PII-flagged content is surfaced.

### Outcome

Operators gain a semantic search surface over the system mutation history without exposing
sensitive state values.

---

## 6. Observability & Priority

### Intent

Mutation footprint recording must be low-latency and non-blocking relative to the plugin mutation
itself. Telemetry surfaces chain health, throughput, and any audit gaps.

### Acceptance Criteria

1. WHEN a footprint is appended THEN emit a `mutation_footprint.recorded` tracing span with
   attributes: `footprint_id`, `plugin_source`, `operation_type`, `content_hash`, `block_num`,
   `chain_valid` (bool), and write latency.

2. WHEN a duplicate footprint is dropped THEN emit a `mutation_footprint.deduped` event with
   `content_hash` and `plugin_source`.

3. WHEN chain verification fails THEN emit a `mutation_footprint.chain_broken` event at `error`
   level with `block_num`, `expected_hash`, and `actual_hash`.

4. WHEN the inbound mutation channel back-pressures THEN log a `warn` with current queue depth;
   do not drop mutations silently.

5. WHEN the mutation footprint worker is under load THEN it runs at lower priority than direct
   control-plane operations and reasoning embedding, but higher than schema footprint embedding.
   NUMA affinity for queue/storage is applied using `OptimizedBlockchain` where available.

### Outcome

The audit trail is observable and auditable in real-time. Chain health, throughput, and any
recording failures are surfaced to operators without exposing mutation payloads.

---

## Plugin Completion Checklist

This checklist captures everything the plugin implementation must satisfy so the blockchain-footprint
flow is a finished, catalog-first product that can be used as an example.

1. `mutation_footprint` implements `StatePlugin::schema()` (or ships a compatibility helper) defining:
   - all fields listed in Section 1 (`footprint_id`, `plugin_source`, `operation_type`, `old_state_hash`,
     `new_state_hash`, `content_hash`, `prev_block_hash`, `timestamp_ms`, `actor`, `diff_summary`,
     `metadata`) with semantic/PII tags matching the acceptance criteria.
   - metadata fields such as `service_name`, `storage_path`, and `source` so projections can trace origin.
2. The plugin is registered via `PluginCatalog::register` (auto-loaded or manual). That routine:
   - persists the plugin’s `PluginCatalogDocument` to `op_dbus_model::SqlitePluginCatalog`,
   - indexes the schema into the shared `SchemaCatalog`,
   - exports D-Bus/grpc projections derived from the schema.
3. Each mutation path captures old/new state, sends `MutationEvent`s to the plugin, and the plugin uses
   `SchemaCatalog::get_copies("mutation_footprint")` to resolve the schema before computing all hashes,
   privacy flags, and chain linkage.
4. The blockchain writer appends to `StreamingBlockchain::timing_subvol` using the schema-defined fields,
   emits the tracing spans required in Section 6, and schedules vectorization (if enabled) under the
   schema-authorized semantic tags.
5. Observability, priority, and NUMA guidance from Section 6 are enforced so the worker stays in the
   intended priority window and records every event described above.

When all checklist items are satisfied, the plugin is ready as a finished catalog-aware example.
