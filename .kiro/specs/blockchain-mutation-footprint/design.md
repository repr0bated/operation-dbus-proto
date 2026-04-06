# Blockchain Mutation Footprint — Design

## Two Angles, One Plugin

The `mutation_footprint` plugin has two co-equal responsibilities that are inseparable:

1. **The plugin IS the schema.** `StatePlugin::schema()` returns the canonical `PluginSchema`
   that defines every field of a mutation footprint record. That schema is the single source of
   truth for what a blockchain block looks like.

2. **The schema IS the vectorization filter.** The `semantic_index.include_paths` and
   `privacy_index.redaction` sections of the generated contract document govern exactly which
   footprint fields the embedding worker ingests, and which are redacted from public payloads.
   The embedding worker calls `SchemaCatalog::get_copies("mutation_footprint")` — it never
   hardcodes field lists.

This follows the identical pattern to all other plugins in the system.

---

## Plugin Structure — 3-Section Pattern

Following `web_ui.rs` and other plugins, the module is structured in three sections:

```
SECTION 1: Immutable Identity  — set once at registration, never changes
SECTION 2: Footprint Record    — the schema for each blockchain block (all readOnly)
SECTION 3: Capabilities        — what this plugin can do (read-only)
```

Because blockchain footprint records are **append-only and immutable once written**, the entire
`FootprintRecord` is `readOnly`. The `PluginSchema` is tagged `"immutable"` so
`to_json_schema()` adds `"readOnly": true` to every property automatically.

---

## Crate Placement

| Component | Crate | File |
|---|---|---|
| Plugin definition, schema, `StatePlugin` impl | `op-plugins` | `src/state_plugins/mutation_footprint.rs` |
| `PluginSchema`, `FieldSchema`, `FieldType`, `Constraint`, `SchemaRegistry` | `op-state-store` | `src/plugin_schema.rs` (unchanged) |
| `StreamingBlockchain`, `PluginFootprint`, `FootprintGenerator` | `op-blockchain` | (unchanged) |
| Mutation interception | `op-state` | `src/dbus_plugin_base.rs` — `record_state_transition` sends `MutationEvent` |
| Schema catalog index | `op-state-store` | `SchemaRegistry` — indexes the persisted schema; no new schema defined here |

---

## Section 1 — Immutable Identity

```rust
/// Plugin identity — immutable after registration
pub struct MutationFootprintIdentity {
    pub name: String,        // const: "mutation_footprint"
    pub version: String,     // semver: "1.0.0"
    pub plugin_type: String, // const: "audit"
    pub driver: String,      // const: "op-blockchain"
}
```

JSON Schema (`$id: …/mutation-footprint/identity.json`):

```json
{
  "type": "object",
  "properties": {
    "name":        { "type": "string", "const": "mutation_footprint" },
    "version":     { "type": "string", "pattern": "^\\d+\\.\\d+\\.\\d+$" },
    "plugin_type": { "type": "string", "const": "audit" },
    "driver":      { "type": "string", "const": "op-blockchain" }
  },
  "required": ["name", "version", "plugin_type", "driver"],
  "additionalProperties": false
}
```

---

## Section 2 — Footprint Record Schema (the canonical `PluginSchema`)

This is what `StatePlugin::schema()` returns. It is built with `PluginSchemaBuilder` and becomes
the `tunable` section of the contract document. Since all fields are cryptographic records that
must never be altered, the schema is tagged `"immutable"` which causes `to_json_schema()` to
emit `"readOnly": true` on every property.

### Fields

| Field | `FieldType` | Required | Constraints | Notes |
|---|---|---|---|---|
| `footprint_id` | `String` | ✅ | `Pattern("^[0-9a-f]{8}-…$")` uuid-v4 | Unique block identifier |
| `plugin_source` | `String` | ✅ | `Min(1)` length | Originating plugin name |
| `operation_type` | `Enum(["create","update","delete","apply","rollback"])` | ✅ | `OneOf` | The `ChangeOperation` kind |
| `old_state_hash` | `String` | ✅ | `Pattern("^[0-9a-f]{64}$")` | SHA-256 of pre-mutation state JSON |
| `new_state_hash` | `String` | ✅ | `Pattern("^[0-9a-f]{64}$")` | SHA-256 of post-mutation state JSON |
| `content_hash` | `String` | ✅ | `Pattern("^[0-9a-f]{64}$")` | SHA-256 of full footprint payload — dedup key |
| `prev_block_hash` | `String` | ✅ | `Pattern("^[0-9a-f]{64}$")` | SHA-256 of preceding block; genesis = `"0"×64` |
| `timestamp_ms` | `Integer` | ✅ | `Min(0)` | Unix epoch in milliseconds |
| `block_num` | `Integer` | ✅ | `Min(1)` | Monotonically increasing sequence number |
| `actor` | `String` | ❌ | `Min(1)` length | Principal that triggered the mutation |
| `diff_summary` | `Object({})` | ❌ | — | Human-readable diff between old and new state |
| `metadata` | `Object({})` | ❌ | — | Arbitrary plugin-supplied key-value context |

### Builder (Rust)

```rust
pub fn schema() -> PluginSchema {
    PluginSchema::builder("mutation_footprint")
        .version("1.0.0")
        .category("audit")
        .description("Immutable blockchain footprint records for all system mutations")
        .tag("immutable")        // → readOnly: true on every property in to_json_schema()
        .tag("append-only")
        .immutable_paths(&[
            "/footprint_id", "/plugin_source", "/operation_type",
            "/old_state_hash", "/new_state_hash", "/content_hash",
            "/prev_block_hash", "/timestamp_ms", "/block_num",
        ])
        // Required identity fields
        .field("footprint_id", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "UUID v4 — unique identifier for this footprint block".into(),
            default: None,
            example: Some(json!("a1b2c3d4-e5f6-7890-abcd-ef1234567890")),
            constraints: vec![Constraint::Pattern {
                regex: r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$".into(),
            }],
            read_only: true,
            read_only_when: None,
        })
        .field("plugin_source", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Name of the state plugin that produced this mutation".into(),
            default: None,
            example: Some(json!("net")),
            constraints: vec![Constraint::Min { value: 1.0 }],
            read_only: true,
            read_only_when: None,
        })
        .field("operation_type", FieldSchema {
            field_type: FieldType::Enum(vec![
                "create".into(), "update".into(), "delete".into(),
                "apply".into(), "rollback".into(),
            ]),
            required: true,
            description: "The kind of mutation applied".into(),
            default: None,
            example: Some(json!("update")),
            constraints: vec![],
            read_only: true,
            read_only_when: None,
        })
        .field("old_state_hash", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "SHA-256 hex of the canonical JSON of the pre-mutation state".into(),
            default: None,
            example: Some(json!("e3b0c44298fc1c149afb...")),
            constraints: vec![Constraint::Pattern { regex: r"^[0-9a-f]{64}$".into() }],
            read_only: true,
            read_only_when: None,
        })
        .field("new_state_hash", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "SHA-256 hex of the canonical JSON of the post-mutation state".into(),
            default: None,
            example: Some(json!("6b86b273ff34fce19d6...")),
            constraints: vec![Constraint::Pattern { regex: r"^[0-9a-f]{64}$".into() }],
            read_only: true,
            read_only_when: None,
        })
        .field("content_hash", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "SHA-256 of the full footprint payload — dedup key and block identifier".into(),
            default: None,
            example: Some(json!("d4735e3a265e16eee03f...")),
            constraints: vec![Constraint::Pattern { regex: r"^[0-9a-f]{64}$".into() }],
            read_only: true,
            read_only_when: None,
        })
        .field("prev_block_hash", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "SHA-256 of the preceding block; genesis block uses 64 zeros".into(),
            default: Some(json!("0000000000000000000000000000000000000000000000000000000000000000")),
            example: Some(json!("a665a45920422f9d417e...")),
            constraints: vec![Constraint::Pattern { regex: r"^[0-9a-f]{64}$".into() }],
            read_only: true,
            read_only_when: None,
        })
        .field("timestamp_ms", FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Unix epoch timestamp in milliseconds when the mutation occurred".into(),
            default: None,
            example: Some(json!(1700000000000i64)),
            constraints: vec![Constraint::Min { value: 0.0 }],
            read_only: true,
            read_only_when: None,
        })
        .field("block_num", FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Monotonically increasing block sequence number within this chain".into(),
            default: None,
            example: Some(json!(42)),
            constraints: vec![Constraint::Min { value: 1.0 }],
            read_only: true,
            read_only_when: None,
        })
        // Optional fields — not required, redacted from public payloads
        .field("actor", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Principal or service that triggered the mutation (redacted from public payloads)".into(),
            default: None,
            example: Some(json!("admin@op-dbus")),
            constraints: vec![Constraint::Min { value: 1.0 }],
            read_only: true,
            read_only_when: None,
        })
        .field("diff_summary", FieldSchema {
            field_type: FieldType::Object(HashMap::new()),
            required: false,
            description: "Human-readable diff between old and new state (redacted when source plugin marks data sensitive)".into(),
            default: None,
            example: None,
            constraints: vec![],
            read_only: true,
            read_only_when: None,
        })
        .field("metadata", FieldSchema {
            field_type: FieldType::Object(HashMap::new()),
            required: false,
            description: "Arbitrary key-value context supplied by the source plugin".into(),
            default: Some(json!({})),
            example: None,
            constraints: vec![],
            read_only: true,
            read_only_when: None,
        })
        .build()
}
```

---

## Contract Document Envelope

`PluginSchema::to_contract_json_schema()` wraps the above into the standard contract with all
required top-level sections. The `mutation_footprint` plugin customises two of them:

### `stub` (standard)
```json
{
  "system_id":      "<host UUID>",
  "source":         "mutation_footprint",
  "source_ref":     "op-blockchain/timing_subvol",
  "discovered_at":  "<ISO-8601>"
}
```

### `immutable` (standard)
```json
{
  "created_at":         "<ISO-8601>",
  "created_by_plugin":  "mutation_footprint",
  "identity_keys":      ["footprint_id"],
  "provider":           "op-dbus"
}
```

### `tunable` — the PluginSchema JSON Schema
All fields emitted with `"readOnly": true` because the schema tag is `"immutable"`.

### `observed` (standard)
```json
{
  "last_observed_at": "<ISO-8601>",
  "status":           "active",
  "drift_detected":   false
}
```

### `meta`
```json
{
  "dependencies":        [],
  "include_in_recovery": true,
  "recovery_priority":   10,
  "category":            "audit",
  "sensitivity":         "internal",
  "tags":                ["immutable", "append-only"],
  "enabled":             true
}
```
`recovery_priority: 10` (high) — the audit trail should be restored before most other plugins.

### `semantic_index`

Governs which footprint fields the embedding worker includes in its text input. The worker reads
this from `SchemaCatalog::get_copies("mutation_footprint")` — it does **not** hardcode paths.

```json
{
  "include_paths": [
    "/tunable/footprint_id",
    "/tunable/plugin_source",
    "/tunable/operation_type",
    "/tunable/old_state_hash",
    "/tunable/new_state_hash",
    "/tunable/content_hash",
    "/tunable/prev_block_hash",
    "/tunable/timestamp_ms",
    "/tunable/block_num"
  ],
  "exclude_paths": [
    "/tunable/actor",
    "/tunable/diff_summary",
    "/stub/discovered_at"
  ],
  "chunking": {
    "strategy":   "json-path-group",
    "max_tokens": 256
  },
  "redaction": { "enabled": true }
}
```

### `privacy_index`

Controls what is redacted from public payloads. Because `actor` and `diff_summary` do not match
the auto-detected pii/secret name patterns (`is_pii_field_name`, `is_secret_field_name`), the
plugin must supply explicit redaction rules by overriding the privacy section:

```json
{
  "redaction": {
    "rules": [
      { "path": "/tunable/actor",        "action": "drop",   "reason": "PII — identifies the human operator" },
      { "path": "/tunable/diff_summary", "action": "mask",   "reason": "May contain sensitive state values from the source plugin" }
    ],
    "default_action": "mask",
    "secret_paths":   [],
    "pii_paths":      ["/tunable/actor", "/tunable/diff_summary"],
    "hash_salt_ref":  "vault://op-dbus/privacy/hash-salt",
    "reversible":     false
  }
}
```

---

## Section 3 — Capabilities

```rust
pub struct MutationFootprintCapabilities {
    pub supports_rollback:     bool,  // false — footprints are immutable records
    pub supports_checkpoints:  bool,  // true  — chain head is a natural checkpoint
    pub supports_verification: bool,  // true  — chain integrity can be verified
    pub atomic_operations:     bool,  // true  — each block write is atomic
    pub append_only:           bool,  // true  — blocks are never modified after write
}
```

---

## `StatePlugin` Implementation

### Required trait methods

| Method | Implementation |
|---|---|
| `name()` | `"mutation_footprint"` |
| `version()` | `"1.0.0"` |
| `metadata()` | `PluginMetadata` with `category: "audit"`, no `dbus_services` |
| `schema()` | Returns `Some(schema())` from Section 2 |
| `is_available()` | `true` — always available |
| `query_current_state()` | Returns chain head info: `{ block_num, chain_head_hash, last_timestamp_ms }` |
| `calculate_diff()` | No-op — footprints are generated by the worker, not diffed |
| `apply_state()` | No-op — state is applied by the `MutationFootprintWorker` directly |
| `verify_state()` | Runs chain integrity verification (replays block file hashes) |
| `create_checkpoint()` | Snapshots the current chain head hash |
| `rollback()` | Not supported — footprints are immutable |
| `capabilities()` | Returns `MutationFootprintCapabilities` |

### Validation

`PluginSchema::validate()` is called on every `FootprintRecord` before it is submitted to
`StreamingBlockchain::add_footprint`. Validation enforces:

- All required fields are present (`footprint_id`, `plugin_source`, `operation_type`,
  `old_state_hash`, `new_state_hash`, `content_hash`, `prev_block_hash`, `timestamp_ms`, `block_num`)
- Pattern constraints on all hash fields (64 hex chars, SHA-256)
- Pattern constraint on `footprint_id` (UUID v4 format)
- Enum constraint on `operation_type`
- `block_num >= 1`
- `timestamp_ms >= 0`
- No unknown fields (`additionalProperties: false` in contract schema)

A footprint that fails validation is **rejected** — it is not written to the chain and a
`mutation_footprint.validation_failed` error span is emitted with the validation errors.

---

## Data Flow

```
Any plugin::apply_state()
  → record_state_transition(old, new, action)
    → MutationEvent { plugin_source, operation_type, old_state, new_state, actor }
      → MutationFootprintWorker (async channel)
        → FootprintGenerator::create_footprint()
          → reads semantic_index from SchemaCatalog
          → computes old_state_hash, new_state_hash (SHA-256)
          → reads chain head (prev_block_hash, block_num) from RwLock
          → computes content_hash (SHA-256 of full payload)
          → constructs FootprintRecord
          → PluginSchema::validate(record) → reject if invalid
          → StreamingBlockchain::add_footprint(footprint)
            → writes timing_subvol/block-{N:012}.json
            → writes vector_subvol/vec-{N:012}.bin (if ml feature)
          → updates chain head RwLock
          → enqueues for vectorization (non-blocking)

EmbeddingWorker
  → SchemaCatalog::get_copies("mutation_footprint")
  → reads semantic_index.include_paths
  → builds embedding text from approved fields only
  → omits privacy_index.pii_paths (actor, diff_summary)
  → calls vector backend
  → stores in vector_subvol
```

Every step reads through the same shared catalog. The schema is written once at plugin
registration and consumed by every downstream component without duplication.
