# Handoff — Router/Selector Agent (T-30, T-31)

## Scope Completed
WS3 model selection. `select_model` is a **pure** function that reads only the
in-memory `ZeroclawState` (`model_routes`, `tools`, `selector_policy`) and
returns a `SelectionOutput` (REQ-03). No I/O, no env reads, no D-Bus calls, and
**no `match` arm on provider or model name** — selection is entirely data-driven
from schema state.

- T-30: `common/selector.rs` added; `pub mod selector;` declared in
  `common/mod.rs`.
- T-31: scoring + hard filters + explicit-override path implemented with
  deterministic ordering and 8 unit tests.

## Design
- **Cost is unit-neutral.** Comparison uses the Zeroclaw-native `cost_profile`
  budget-class string mapped to an ordinal rank (`free<low<medium<high<premium`).
  Never `cost_per_token` or currency math (REQ-03 hard rule).
- **Ordinal helpers** for effort/cost/privacy/latency map free-form class
  strings to ranks; unknown values map to the neutral middle so an unannotated
  route is neither preferred nor penalized.
- **Hard filters** (reject before scoring): route unavailable, privacy tier
  below requested, context window exceeded, requested tool not declared,
  effort/cost/latency ceilings.
- **Explicit override**: if the caller names `explicit_provider`/`explicit_model`
  it must be declared AND pass the hard filters, else a specific `ZeroclawError`
  is returned (never a silent substitution).
- **Determinism**: identical `state` + `input` ⇒ identical selected route,
  ordering, and confidence. Audit fields (`trace_id`, `timestamp`) are left
  **empty** here and stamped by the dispatch handler (see handoff-dbus.md), so
  the selector stays pure.

## Files Changed
- `crates/op-plugins/src/state_plugins/common/selector.rs` (new) — `select_model`
  + ordinal helpers + 8 tests.
- `crates/op-plugins/src/state_plugins/common/mod.rs` — `pub mod selector;`.

## Verification Commands Run
```
cargo check -p op-plugins                                       # green
cargo test -p op-plugins --lib state_plugins::common::selector  # ok (8 passed)
cargo clippy -p op-plugins --lib                                # clean (only pre-existing op-state warn)
```

## Known Risks / Blocked Items
- `#[allow(clippy::too_many_arguments)]` on the route helper is intentional
  (the §8 route signature is wide by spec); the public `select_model` signature
  is `(&SelectionInput, &ZeroclawState)`.
- Pre-existing `op-state` clippy `empty_line_after_doc_comments` warning blocks a
  clean workspace `-D warnings`; out of scope here, flag for T-80.

## Next-Agent Dependencies
- D-Bus Agent (T-20…T-22): wire `select_model` into the `SelectModel` handler
  and stamp `trace_id`/`timestamp` (done — see handoff-dbus.md).
