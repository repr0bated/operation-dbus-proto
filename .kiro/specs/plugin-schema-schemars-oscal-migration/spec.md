# Spec — plugin-schema-schemars-oscal-migration

## Purpose

Migrate all `op-plugins` schemas from hand-rolled `PluginSchema::builder` maps to `schemars`-derived structs, with OSCAL subids baked into the source of truth. This spec is the binding contract between requirements, design decisions, and implementation tasks.

The migration must remain compatible with the existing zeroclaw Axum schema host (`.kiro/specs/zeroclaw-host-axum-schema-kiro`), which consumes the zeroclaw plugin schema from `/dev/shm/opdbus/schemas/zeroclaw.json`.

---

## Architectural Constraints (binding)

| # | Constraint | Source |
|---|---|---|
| C-01 | Every plugin schema is derived from a `#[derive(schemars::JsonSchema)]` struct. | FR-01 |
| C-02 | Hand-rolled schemas may only exist as `#[cfg(test)]` golden references. | FR-01, NFR-05 |
| C-03 | OSCAL subids are declared via `#[schemars(extend("x-oscal-subid" = ...))]` on structs and fields. | FR-03 |
| C-04 | The `schemars_adapter` populates `PluginSchema.subids` from `x-oscal-subid` attributes. | FR-03 |
| C-05 | Subids must match the regex and category-specific rules from `oscal_subid_registry`. | FR-04, AGENTS.md §4a |
| C-06 | The `oscal_subid_registry` plugin itself must be schema-native. | FR-05 |
| C-07 | D-Bus is the only control plane; schemas are read from The Sled (`/dev/shm`). | FR-07, AGENTS.md §4 |
| C-08 | The zeroclaw Axum host's schema contract must remain unchanged. | FR-08, existing spec |
| C-09 | The `zeroclaw` and `antigravity` plugins share a common LLM projection module. | FR-09 |
| C-10 | No new `src/` at workspace root; all code under `crates/`. | AGENTS.md §3 |
| C-11 | No Python; Rust-only migration. | FR-10, AGENTS.md §4 |
| C-12 | External SDK/source (google-antigravity, zeroclaw) are reference only, not dependencies. | FR-10 |

---

## Interface Contract

### Schema generation adapter

```rust
pub fn plugin_schema_from_json(
    name: &str,
    version: &str,
    description: &str,
    root: &serde_json::Value,
) -> PluginSchema
```

Inputs:
- `name`, `version`, `description` for the plugin.
- `root`: the JSON Schema produced by `schemars::schema_for!(T)`.

Outputs:
- A `PluginSchema` with fields, types, constraints, descriptions, defaults, examples, `read_only`, `immutable_paths`, and `subids` populated.

### OSCAL annotations

| Location | Attribute | Destination |
|---|---|---|
| Root struct | `#[schemars(extend("x-oscal-subid" = ...))]` | `PluginSchema.subids["__schema__"]` |
| Struct field | `#[schemars(extend("x-oscal-subid" = ...))]` | `PluginSchema.subids["field_name"]` |
| Root struct | `#[schemars(extend("x-immutable-paths" = [...]))]` | `PluginSchema.immutable_paths` |

### Equivalence test contract

For every converted plugin:

```rust
#[test]
fn derived_schema_matches_hand_rolled() {
    let diffs = schemars_adapter::schema_diffs(&hand_rolled(), &derived());
    assert!(diffs.is_empty(), "schema drift: {:#?}", diffs);
}
```

### Downstream consumer contract

The zeroclaw Axum host expects:
- File: `/dev/shm/opdbus/schemas/zeroclaw.json`
- Format: serialised `PluginSchema` JSON
- Writer: `ZeroclawPlugin` in `op-plugins`
- Reader: `op-grpc-bridge` `SchemaLoader`

This contract must be preserved after migration.

---

## Accepted Trade-offs

| Trade-off | Rationale |
|---|---|
| Opaque `Value` fields may remain at runtime initially | The schema can be typed before the plugin's state machine is rewritten. This avoids a mega-PR. |
| Common LLM projection is shaped for the plugin UI, not the full zeroclaw Config | The plugin projection is a deliberate subset; trying to match the full upstream config would bloat the schema and break the host contract. |
| Hand-rolled schemas kept as test-only golden references | Gives us full-fidelity equivalence tests without duplicating production authority. |
| Subid validation is a test/CI gate, not a runtime panic | Schemas are static; validation belongs at build/test time. |
| External SDK/source are reference only | Adding `google-antigravity` or `zeroclaw` as dependencies would pull in huge dependency graphs and violate the Rust-only, no-new-deps rule. |

---

## Verification Criteria

| ID | Criterion | How to check |
|----|-----------|-------------|
| V-01 | `cargo fmt --all -- --check` passes | CI |
| V-02 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes | CI |
| V-03 | `cargo test --workspace --all-targets --all-features` passes | CI |
| V-04 | Every migrated plugin has a golden-reference equivalence test | unit test |
| V-05 | `schema_diffs` for `unix_socket` returns an empty vector | unit test |
| V-06 | All declared subids pass `validate_subid()` | unit test / CI gate |
| V-07 | No duplicate subids across all plugin schemas | unit test / CI gate |
| V-08 | The zeroclaw schema file is still written and served by the Axum host | integration test |
| V-09 | `oscal_subid_registry` schema is derived from a typed struct | unit test |
| V-10 | `cron` schema is derived from typed structs | unit test |
| V-11 | `zeroclaw` and `antigravity` share the LLM projection module | code review / grep |
| V-12 | No new hand-rolled production schemas remain in migrated plugins | code review |
