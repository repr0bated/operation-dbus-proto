# op-blockchain — Specification

**Crate**: `op-blockchain`  
**Location**: `crates/op-blockchain`  
**Purpose**: Streaming blockchain with BTRFS subvolumes for append-only mutation audit trails,
vectorized footprints, point-in-time snapshots, and tamper-evident chain integrity.

See `REQUIREMENTS.md` for what this crate must do and `DESIGN.md` for the implementation approach.

---

## Quick Reference

### Cargo.toml
```toml
[package]
name = "op-blockchain"
version.workspace = true
edition.workspace = true

[dependencies]
op-core    = { workspace = true }
op-cache   = { path = "../op-cache" }
tokio      = { workspace = true }
serde      = { workspace = true }
simd-json  = { workspace = true }
anyhow     = { workspace = true }
thiserror  = { workspace = true }
tracing    = { workspace = true }
chrono     = { workspace = true }
uuid       = { workspace = true }
sha2       = { workspace = true }
gethostname = { workspace = true }

[features]
default = []
ml      = []        # enables transformer-based vectorization via FootprintGenerator
```

### Source Structure
```
op-blockchain/src/
  lib.rs                      — crate root, re-exports
  blockchain.rs               — StreamingBlockchain, OptimizedBlockchain
  footprint.rs                — BlockEvent, PluginFootprint (current production struct)
  plugin_footprint.rs         — LegacyPluginFootprint, FootprintGenerator
  streaming_blockchain.rs     — StreamingBlockchain full implementation, SnapshotInterval
  retention.rs                — RetentionPolicy (hourly/daily/weekly/quarterly)
  snapshot.rs                 — SnapshotInterval enum and snapshot helpers
  btrfs_numa_integration.rs   — NUMA topology detection, OptimizedBlockchain wrapper
```

---

## Module Structure

### `blockchain` — Core Blockchain

- **`StreamingBlockchain`** — main struct managing three BTRFS subvolumes:
  - `timing_subvol` — append-only audit ledger (`block-{N:012}.json`)
  - `vector_subvol` — ML embedding vectors per block
  - `state_subvol` — current system state for disaster recovery (snapshotted)
- **`OptimizedBlockchain`** — NUMA-aware wrapper around `StreamingBlockchain` with BTRFS cache

Key methods:
- `new(base_path, snapshot_interval, retention_policy)` — initialize
- `add_footprint(footprint)` — append a new block (atomic write)
- `verify_chain()` — replay all blocks and check hash chain continuity
- `chain_head()` — return current head (`block_num`, `content_hash`)
- `trigger_snapshot()` — create read-only BTRFS snapshot of `state_subvol`
- `apply_retention()` — prune stale `state_subvol` snapshots per policy

### `footprint` — Block Types

- **`BlockEvent`** — timestamped event: `timestamp`, `category`, `action`, `data`, `hash`, `vector`
- **`PluginFootprint`** — production footprint record (see field gap analysis in DESIGN.md)

Current `PluginFootprint` fields:

| Field | Type | Notes |
|---|---|---|
| `plugin_id` | `String` | Source plugin name |
| `operation` | `String` | Operation string |
| `timestamp` | `u64` | Seconds since epoch (needs → ms) |
| `data_hash` | `String` | SHA-256 of data content |
| `content_hash` | `String` | SHA-256 of operation context |
| `metadata` | `HashMap<String, Value>` | Key-value context |
| `vector_features` | `Vec<f32>` | 64-dim heuristic or transformer embeddings |

**Missing vs. schema** (see DESIGN.md): `footprint_id` (UUID), `old_state_hash`,
`new_state_hash`, `prev_block_hash` (chain link), `block_num`, `actor`, `diff_summary`.

### `plugin_footprint` — FootprintGenerator

- **`LegacyPluginFootprint`** — older struct kept for compatibility
- **`FootprintGenerator`** — creates footprints from plugin operations:
  - `new(plugin_id)` — construct with plugin identity
  - `create_footprint(operation, data, metadata)` — heuristic features (64-dim)
  - When `ml` feature enabled: `generate_transformer_features()` uses `ModelManager::global()`

### `streaming_blockchain` — Storage Engine

- Implements the three-subvolume layout
- `SnapshotInterval` enum: `PerOperation` | `EveryMinute` | `Every5Minutes` | `Every15Minutes` |
  `Every30Minutes` | `Hourly` | `Daily` | `Weekly`
- Default interval: `Every15Minutes` (configurable via `OPDBUS_SNAPSHOT_INTERVAL`)
- BTRFS commands via `tokio::process::Command` (`btrfs subvol create/snapshot/delete`, `btrfs send`)
- Falls back to regular directories if BTRFS unavailable

### `retention` — Retention Policy

- **`RetentionPolicy`** — rolling windows for `state_subvol` snapshot pruning:
  - `hourly: usize` — keep last N hourly snapshots
  - `daily: usize` — keep last N daily snapshots
  - `weekly: usize` — keep last N weekly snapshots
  - `quarterly: usize` — keep last N quarterly snapshots
- Default: 5 for each window
- Configurable via env: `OPDBUS_RETAIN_HOURLY`, `OPDBUS_RETAIN_DAILY`, `OPDBUS_RETAIN_WEEKLY`, `OPDBUS_RETAIN_QUARTERLY`
- `timing_subvol` blocks are **never pruned** — permanent audit ledger

### `btrfs_numa_integration` — NUMA Support

- Detects NUMA topology from `/sys/devices/system/node/`
- `OptimizedBlockchain` assigns blockchain I/O to NUMA-local nodes
- Improves throughput on multi-socket systems for high-mutation workloads

---

## Storage Layout

```
{base_path}/
  timing_subvol/          ← append-only, never pruned
    block-000000000001.json
    block-000000000002.json
    …
  vector_subvol/          ← embedding vectors, one per block
    vec-000000000001.bin
    …
  state_subvol/           ← current system state, snapshotted
  snapshots/              ← read-only BTRFS snapshots of state_subvol
    snapshot-{ISO8601}/
    …
```

Block file format:
```json
{
  "footprint_id":    "uuid-v4",
  "plugin_source":   "net",
  "operation_type":  "update",
  "old_state_hash":  "sha256hex",
  "new_state_hash":  "sha256hex",
  "content_hash":    "sha256hex",
  "prev_block_hash": "sha256hex",
  "block_num":       42,
  "timestamp_ms":    1700000000000,
  "metadata":        {}
}
```

---

## Chain Integrity

Each block's `content_hash` is computed as:

```
SHA-256(footprint_id || plugin_source || operation_type ||
        old_state_hash || new_state_hash || prev_block_hash || timestamp_ms)
```

The chain is valid when every block's `prev_block_hash` equals the `content_hash` of the
immediately preceding block. The genesis block uses `"0" × 64` as `prev_block_hash`.

Verification (`verify_chain`): linear replay of `timing_subvol/block-*.json` in numeric order.
Any broken link produces a `mutation_footprint.chain_broken` error span.

---

## Environment Variables

| Variable | Default | Purpose |
|---|---|---|
| `OPDBUS_SNAPSHOT_INTERVAL` | `every-15-minutes` | Snapshot interval for `state_subvol` |
| `OPDBUS_RETAIN_HOURLY` | `5` | Hourly snapshot retention count |
| `OPDBUS_RETAIN_DAILY` | `5` | Daily snapshot retention count |
| `OPDBUS_RETAIN_WEEKLY` | `5` | Weekly snapshot retention count |
| `OPDBUS_RETAIN_QUARTERLY` | `5` | Quarterly snapshot retention count |

---

## Features

| Feature | Default | Effect |
|---|---|---|
| `default` | ✅ | Heuristic 64-dim vectorization |
| `ml` | ❌ | Transformer-based embeddings via `ModelManager::global()` |

---

## Related Crates

| Crate | Relationship |
|---|---|
| `op-cache` | BTRFS cache integration |
| `op-plugins` | Hosts `mutation_footprint` plugin that uses this crate |
| `op-state` | Sends `MutationEvent` to the footprint worker |
| `op-state-store` | `PluginSchema`, `SchemaCatalog` — schema for footprint records |
