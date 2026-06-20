# Requirements — plugin-schema-schemars-oscal-migration

## Scope

Convert every `op-plugins` schema from hand-rolled `FieldSchema`/`PluginSchema::builder` maps to `schemars::JsonSchema`-derived structs, with OSCAL subids declared at the source-of-truth level. The struct is the schema; the schema is the interface contract.

This migration must remain compatible with the existing zeroclaw Axum schema host (`.kiro/specs/zeroclaw-host-axum-schema-kiro`), which consumes the zeroclaw plugin schema from `/dev/shm/opdbus/schemas/zeroclaw.json`.

---

## Functional Requirements

### FR-01 — Structs are the single source of truth
- Every plugin schema must be derived from `#[derive(schemars::JsonSchema)]` structs defined in the plugin file.
- No hand-rolled `FieldSchema` maps may remain in production code for migrated plugins.
- Existing hand-rolled schemas may be kept as `#[cfg(test)]` golden references only.

### FR-02 — Recursive full-fidelity schema diff
- The `schemars_adapter` must provide a recursive `schema_diffs` helper that compares derived and golden schemas at every nesting level (objects, arrays, enums, constraints, descriptions, defaults, examples, readOnly, immutable_paths).
- Every converted plugin must have a test asserting the derived schema matches the golden reference with zero diffs.

### FR-03 — OSCAL subids baked into the schema
- Every plugin schema (the root struct) must carry its own `subid` via `#[schemars(extend("x-oscal-subid" = "..."))]`.
- Every field/tool in a plugin schema must carry its own `subid` via `#[schemars(extend("x-oscal-subid" = "..."))]` on the corresponding struct field.
- The `schemars_adapter` must read both root-level and field-level `x-oscal-subid` attributes and populate `PluginSchema.subids`.

### FR-04 — OSCAL subid validation
- Subids must match the regex already defined in `oscal_subid_registry_schema()`.
- Category-specific required fields must be enforced:
  - `mut.*` entries require `actor_id` and `capability_id`.
  - `evt.*` entries require `event_id` or `event_hash`.
  - `src.*` entries require `source_system` and `source_locator`.
- Compliance mappings (`control_refs`, `statement_refs`) must live in metadata arrays, never inside the `subid` string.

### FR-05 — OSCAL subid registry is schema-native
- The `oscal_subid_registry` plugin (currently hand-rolled) must be converted to a typed `OscalSubidRegistryEntry` struct with derived schema.
- The registry itself must carry OSCAL annotations and become the authoritative contract for validating other subids.

### FR-06 — No runtime behavior change without deliberate typing
- Plugins whose state is currently read as opaque `Value` may keep `Value` at runtime until the plugin's own state machine is updated to use typed structs.
- The schema, however, must be fully typed from the existing JSON examples.

### FR-07 — D-Bus remains the control plane
- Schema mutations continue to flow through D-Bus objects at `/org/opdbus/v1/plugins/<name>`.
- No plugin code may read config files or poll sockets directly for live state; schema state is read from The Sled (`/dev/shm`) or via D-Bus.

### FR-08 — Zeroclaw Axum schema host compatibility
- The migrated `zeroclaw` schema must remain serialisable to the same `PluginSchema` JSON shape consumed by `op-grpc-bridge`.
- `/dev/shm/opdbus/schemas/zeroclaw.json` must continue to be written by the zeroclaw plugin and readable by the Axum host without changes to the host.
- The host's `GetSchema` and `WatchSchema` RPCs must return the migrated schema successfully.

### FR-09 — Common projection shared between zeroclaw and antigravity
- The duplicated fields (`providers`, `model_routes`, `router`, `tools`, `config_schema`, `ui_surfaces`, `structured_output`) between `zeroclaw.rs` and `antigravity.rs` must be extracted into a shared typed module.
- Both plugins derive their schemas from the shared projection structs, with plugin-specific extensions layered on top.

### FR-10 — External source references only, not dependencies
- The downloaded `google-antigravity` SDK (`/tmp/antigravity-sdk`) and cloned `zeroclaw` source (`~/git/zeroclaw`) may be used for naming and semantics but must not be added as crate dependencies.
- No Python code may be introduced; the migration is Rust-only.

---

## Non-Functional Requirements

| ID | Requirement |
|----|-------------|
| NFR-01 | `cargo check -p op-plugins --all-targets` passes. |
| NFR-02 | `cargo test -p op-plugins --all-targets --all-features` passes with no new failures. |
| NFR-03 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes. |
| NFR-04 | `cargo fmt --all -- --check` passes. |
| NFR-05 | Every converted plugin has a golden-reference equivalence test. |
| NFR-06 | CI includes a subid uniqueness check across all plugin schemas. |
| NFR-07 | No new generic `src/` directories; all code stays under `crates/`. |

---

## Out of Scope

- Re-vectorization of Qdrant collections (already tracked in SIGNALS OD-22).
- Changes to `op-grpc-bridge` host implementation beyond verifying compatibility.
- Runtime behavior changes for plugins whose state is still opaque `Value` (addressed per-plugin after the schema is typed).
- UI rendering changes; the schema output shape remains the same.
