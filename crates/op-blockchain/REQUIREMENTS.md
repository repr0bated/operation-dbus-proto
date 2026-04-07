# op-blockchain — Requirements

**Crate**: `op-blockchain`  
**Scope**: Mutation footprint capture, blockchain persistence, chain integrity, vectorization, and the `mutation_footprint` plugin

---

## Introduction

Every mutation applied to the system through the plugin + schema flow must produce a
cryptographically hashed footprint that is appended to the immutable blockchain audit trail
maintained by `op-blockchain`. The `mutation_footprint` plugin is the canonical plugin that
owns this audit trail. It registers once at startup, produces a `PluginCatalogDocument`
describing its schema, and receives mutation events from every other state plugin via a shared
async channel.

**Key architectural invariants:**

- The `mutation_footprint` plugin is the **single schema authority** for mutation audit records.
  No other component invents a competing audit record shape.
- Every mutation is captured as a `PluginFootprint` — containing the source plugin ID, operation
  type, SHA-256 hashes of the old and new state, a chained block hash linking to the previous
  footprint, and full metadata — before being appended to the `StreamingBlockchain`.
- The blockchain's `timing_subvol` is **append-only**. Once a footprint block is written it is
  never modified; only new blocks are added. Snapshots and retention policy control pruning.
- The plugin schema is the **ground truth** for validation, rendering, vectorization, and
  compliance queries. All downstream consumers resolve the audit record shape through
  `SchemaCatalog::get_copies("mutation_footprint")` — not hardcoded field lists.
- Mutations must not bypass the plugin + schema flow.

---

## 1. Canonical Plugin Document & Schema Flow

### Intent

The `mutation_footprint` plugin owns its canonical schema. That schema defines every field in an
audit record: identifiers, state hashes, chain linkage, actor context, and semantic/privacy tags.
The shared catalog indexes it so all projections read the same contract.

### Acceptance Criteria

1. WHEN `mutation_footprint` registers at startup THEN it persists a `PluginCatalogDocument`
   containing a full `PluginSchema` with these fields:
   - `footprint_id` — UUID v4, immutable, semantic
   - `plugin_source` — originating plugin name, semantic
   - `operation_type` — enum: `create | update | delete | apply | rollback`, semantic
   - `old_state_hash` — SHA-256 hex of pre-mutation state, semantic
   - `new_state_hash` — SHA-256 hex of post-mutation state, semantic
   - `content_hash` — SHA-256 of the full footprint payload, semantic (dedup key)
   - `prev_block_hash` — SHA-256 of the preceding block, semantic (chain link)
   - `block_num` — monotonically increasing sequence number, semantic
   - `timestamp_ms` — u64 milliseconds since epoch, semantic
   - `actor` — principal/user/service that triggered mutation, optional, PII-capable
   - `diff_summary` — JSON object with computed diff, optional, PII-capable
   - `metadata` — arbitrary plugin-supplied key-value pairs, optional

2. WHEN schema registration succeeds THEN `SchemaCatalog` indexes the schema and every consumer
   resolves audit record shape through the catalog, not hardcoded field lists.

3. WHEN schema fields change THEN the change is introduced in `mutation_footprint::schema()`
   first; all downstream projections follow automatically via catalog lookup.

4. WHEN `StatePlugin::schema()` is called THEN it returns `Some(PluginSchema)` — not `None`.
   The compat fallback in `plugin_schema.rs` does NOT satisfy this requirement.

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
   - Computes `old_state_hash = SHA-256(canonical_json(old_state))`
   - Computes `new_state_hash = SHA-256(canonical_json(new_state))`
   - Reads the last known `prev_block_hash` from the in-memory chain head (protected by a
     `RwLock`; genesis sentinel = `"0" × 64`)
   - Computes `content_hash = SHA-256(footprint_id || plugin_source || operation_type ||
     old_state_hash || new_state_hash || prev_block_hash || timestamp_ms)`
   - Constructs a `PluginFootprint` with all schema-required fields
   - Updates the chain head to `content_hash`

3. WHEN `content_hash` equals the chain head for an already-seen record THEN the footprint is a
   duplicate; it is logged and discarded without appending to the chain.

4. WHEN the originating plugin has `sensitive = true` in its metadata THEN `diff_summary` and
   `actor` are marked `pii_flagged = true` before persisting.

5. WHEN any step in footprint generation fails THEN the error is logged at `error` level with
   `plugin_source`, `operation_type`, and `footprint_id`; the mutation itself is not rolled
   back but the audit gap is surfaced via structured telemetry.

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
   The `timing_subvol` is never snapshotted-and-pruned; it is append-only.

4. WHEN the retention policy fires THEN old `state_subvol` snapshots are pruned according to
   `RetentionPolicy` (hourly/daily/weekly/quarterly windows configurable via env vars). Block
   files in `timing_subvol` are never pruned; they constitute the permanent audit ledger.

5. WHEN BTRFS is unavailable THEN the system falls back to regular directories and the audit
   trail continues with degraded snapshot capability. The footprint record logs the storage
   backend in use.

6. WHEN `btrfs send` / remote replication is triggered THEN the snapshot is streamed to the
   configured remote path, keeping an off-site copy of the audit trail.

---

## 4. Chain Integrity & Tamper Detection

### Intent

The chain-link property (`prev_block_hash`) makes the audit trail tamper-evident. Any inserted,
deleted, or modified block breaks the hash chain and is detectable by a verification pass.

### Acceptance Criteria

1. WHEN chain verification is requested THEN the system replays `timing_subvol/block-*.json`
   files in sequence, recomputes each `content_hash`, and confirms each block's
   `prev_block_hash` equals the preceding block's `content_hash`.

2. WHEN a broken link is found THEN verification reports the first block index where the chain
   breaks, the expected hash, and the stored hash.

3. WHEN genesis verification runs THEN the first block's `prev_block_hash` must equal the
   genesis sentinel value (default `"0" × 64`).

4. WHEN the chain head is queried THEN the plugin returns the `content_hash` of the most
   recently appended block without re-reading all block files.

5. WHEN the system restarts THEN `mutation_footprint` replays the last block file to restore the
   in-memory chain head before accepting new mutations.

---

## 5. Vectorization & Semantic Search

### Intent

Mutation footprints are optionally vectorized so operators can perform semantic similarity
queries across the audit trail (e.g., "find mutations similar to this network config change").

### Acceptance Criteria

1. WHEN the `ml` feature is active THEN the embedding worker resolves the `mutation_footprint`
   schema from `SchemaCatalog`, constructs embedding text from fields tagged `semantic`
   (excluding `pii`-flagged content), and calls the configured vector backend.

2. WHEN `pii_flagged = true` THEN `diff_summary` and `actor` are omitted from the embedding text.

3. WHEN vectorization completes THEN the vector is stored in `vector_subvol` alongside the block
   for semantic retrieval.

4. WHEN vectorization fails THEN the block is still committed to `timing_subvol`; the vector is
   queued for retry with exponential backoff. Audit integrity does not depend on vectorization.

5. WHEN a semantic search is issued THEN results include only schema-approved fields; no raw
   state payloads or PII-flagged content is surfaced.

---

## 6. Observability & Priority

### Intent

Mutation footprint recording must be low-latency and non-blocking relative to the plugin
mutation itself. Telemetry surfaces chain health, throughput, and any audit gaps.

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
   control-plane operations, but higher than schema footprint embedding. NUMA affinity for
   queue/storage is applied using `OptimizedBlockchain` where available.

---

## Implementation Checklist

Before the plugin is considered complete:

- [ ] `mutation_footprint` implements `StatePlugin::schema()` returning `Some(PluginSchema)` with
      all 12 fields (footprint_id, plugin_source, operation_type, old_state_hash, new_state_hash,
      content_hash, prev_block_hash, block_num, timestamp_ms, actor, diff_summary, metadata)
      with correct FieldType, constraints, and read_only flags.
- [ ] The schema is tagged `"immutable"` so `to_json_schema()` emits `readOnly: true` on every property.
- [ ] `actor` and `diff_summary` have explicit `privacy_index.redaction.rules` entries (path, action=drop/mask)
      because their names do not match the auto-PII detection patterns.
- [ ] The plugin is added to `DefaultPluginRegistry` and listed in `default_auto_load`.
- [ ] `PluginCatalog::register` persists the `PluginCatalogDocument` and indexes the schema into
      `SchemaCatalog` on startup.
- [ ] `op-state` intercepts `apply_state` and sends `MutationEvent` through a shared async channel
      to the mutation_footprint worker.
- [ ] The blockchain writer appends to `StreamingBlockchain::timing_subvol` using schema-defined
      fields and emits the tracing spans from Section 6.
- [ ] Chain verification (`verify_chain`) is callable independently of the write path.
- [ ] On restart, the chain head is restored from the last block file before new mutations are accepted.
- [ ] Vectorization worker reads semantic fields from `SchemaCatalog` — no hardcoded field lists.
