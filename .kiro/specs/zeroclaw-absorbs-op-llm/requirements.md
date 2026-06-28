# Requirements — zeroclaw-absorbs-op-llm

Feature: Zeroclaw absorbs `op-llm` and becomes the sole dynamic authority for LLM
routing, provider execution, model selection, tool-call translation, and
schema-driven execution authorization.

---

## REQ-01 — Zeroclaw is the single source of truth for all LLM execution

**WHEN** any component needs a provider, model, route, tool declaration, or
execution authorization,
**THEN** it MUST query the Zeroclaw D-Bus object at
`/org/opdbus/v1/plugins/zeroclaw` (bus `org.opdbus.v1`).

Acceptance criteria:
- No caller resolves a provider or model outside of a Zeroclaw D-Bus read.
- No fallback to environment-variable provider selection in plugin/service code
  after migration is complete (bootstrap scripts remain the sole exception).
- `rg 'op_llm::' crates -g '*.rs'` produces zero hits outside `op-llm` itself
  after Caller Migration is complete.

---

## REQ-02 — Provider/model/tool/route must be declared in Zeroclaw PluginSchema

**WHEN** any component attempts to execute an LLM call,
**THEN** the selected provider, model, route, and any tool being invoked MUST be
present in the Zeroclaw `PluginSchema` as projected via D-Bus.

Acceptance criteria:
- A call with a provider/model combination absent from `model_routes` MUST be
  rejected with `Err` before any network I/O.
- A tool call whose `name` does not appear in `tools[]` in the schema MUST be
  rejected.
- Tests demonstrate that attempts to execute undeclared routes return
  `RouteNotDeclared` errors.

---

## REQ-03 — Dynamic automatic model selection

**WHEN** a caller supplies task metadata (task class, effort, cost ceiling,
latency target, context requirement, privacy tier, tool needs),
**THEN** Zeroclaw MUST select provider and model entirely from live
schema/D-Bus state with no static match tables in Rust.

Acceptance criteria:
- The selector reads only from Zeroclaw `PluginSchema` fields projected at
  runtime; no `match provider_type` arms in the selection path.
- Hard filter: routes where `available == false` or `privacy_tier` violates the
  request MUST be excluded before scoring.
- Scoring order: capability match → quality/effort → cost → latency → route
  health.
- Output includes: `selected_provider`, `selected_model`, `reason`,
  `estimated_cost`, `effort_level`, `confidence`, `fallback_routes[]`,
  `event_metadata`.
- Tests cover: lowest-cost-wins, effort-ceiling-respected,
  unavailable-route-excluded, privacy-tier-enforced.

---

## REQ-04 — `op-llm` provider adapters absorbed into Zeroclaw architecture

**WHEN** migration is complete,
**THEN** the provider execution adapters currently in
`crates/op-llm/src/{anthropic,gemini,gemini_cli,factory,openclaw,assistant,gcloud_adc}.rs`
MUST live inside the existing crate boundary that preserves Zeroclaw
plugin/D-Bus/schema authority, with no duplicate authority crate.

Acceptance criteria:
- `op-llm` is removed from workspace `Cargo.toml` after callers are migrated.
- `cargo build --workspace` passes with `op-llm` removed.
- All adapter behavior (OpenAI-compatible chat, tool-call translation,
  Anthropic content-block format, streaming, GCloud ADC) is preserved.

---

## REQ-05 — D-Bus is the only control plane

**WHEN** any plugin or service code reads or writes provider selection,
route selection, model availability, tool declarations, policy metadata, or
execution authorization,
**THEN** it MUST do so through D-Bus methods/properties on
`/org/opdbus/v1/plugins/zeroclaw`.

Acceptance criteria:
- No `Command::new("...")` invocations for live state in plugin or service code.
- No direct live-state file reads outside bootstrap scripts.
- No JSON-RPC polling loops or D-Bus watchers for live state.

---

## REQ-06 — `/dev/shm/opdbus` files are derived projection artifacts, not authorities

**WHEN** `SchemaEngine` projects Zeroclaw to `/dev/shm/opdbus/schemas/zeroclaw.json`,
**THEN** that file MUST be a snapshot of the D-Bus-authoritative schema, not
the source of truth. Consumer reads of that file MUST NOT mutate it.

Acceptance criteria:
- `SchemaEngine` is the sole writer of `/dev/shm/opdbus/schemas/zeroclaw.json`.
- The monolithic all-plugin catalog at
  `/dev/shm/live-schema.json` is written by the schema
  aggregation layer, not by individual plugins.
- No consumer reads `/dev/shm` paths for live state decisions.

---

## REQ-07 — OSCAL subids on every new artifact

**WHEN** any new schema field, D-Bus method, mutation record, event, or MCP
tool is added,
**THEN** it MUST be registered in the `PluginSchema.subids` map (the single
subid authority — spec §13.1), keyed by field/method/signal name, with a value
in one of the seven categories (`src`, `prj`, `sch`, `mut`, `obs`, `evt`, `exp`)
per AGENTS.md §4a. `x-oscal-subid` source annotations are NOT the authority.

Acceptance criteria:
- The `zeroclaw_schema_golden` test asserts `PluginSchema.subids` values are unique.
- No new field, method, or signal exists without an entry in the `subids` map.

---

## REQ-08 — `op-chat`, `op-web`, MCP surfaces, and factory provider routes migrate off `op-llm`

**WHEN** migration is complete,
**THEN** `op-chat`, `op-web`, cognitive-mcp, and factory provider route code MUST
NOT import `op-llm` traits or types for provider selection or execution.

Acceptance criteria:
- `op-chat` calls Zeroclaw D-Bus for provider/model resolution before sending.
- Factory provider routes are declared in Zeroclaw's provider catalog. Factory
  BYOM discovery reads `model_routes[]` from Zeroclaw schema projection
  (D-Bus or `/dev/shm/opdbus/schemas/zeroclaw.json`). Factory is a provider
  category within Zeroclaw's schema, not a separate D-Bus control object.
- `cognitive-mcp` remains the external gateway; `compact-mcp` remains loopback
  only — neither is changed by this refactor.

---

## REQ-09 — No new crate, no new shim service

**WHEN** implementing this refactor,
**THEN** no new `op-zeroclaw` crate and no new shim/proxy service MUST be
introduced.

Acceptance criteria:
- Workspace `Cargo.toml` member count does not increase.
- No new binary service is registered with s6.

---

## REQ-11 — Three-layer boundary: Contract, Orchestration, Provider Adapter

**WHEN** provider adapters are absorbed and Zeroclaw execution logic is structured,
**THEN** the implementation MUST maintain three distinct layers with the dependency
direction: Contract → Orchestration → Provider Adapter (no back-calls).

Acceptance criteria:
- **Contract Layer** (`op-plugins` schema types): owns `PluginSchema`, `ZeroclawState`, schema
  declarations, OSCAL subids, generated D-Bus surface. Contains no HTTP client code.
- **Orchestration Layer** (selector, D-Bus method handlers, MutationEngine integration): reads
  state only through SchemaEngine/MutationEngine/D-Bus; does not implement provider wire formats.
- **Provider Adapter Layer** (absorbed adapter files): owns wire formats, streaming parsing, auth
  headers, network calls; takes only a schema-authorized execution plan from the Orchestration
  Layer. Does not select models, mutate routes, read live state, bypass D-Bus, or call back into
  Orchestration.
- Provider adapters are NOT placed inside `op-plugins` if doing so causes the schema crate to own
  runtime HTTP clients. The Provider Absorption Agent must document the confirmed target module in
  `handoff-provider-absorption.md` with explicit layer rationale before moving any files.
- Review checklist (T-80) must confirm all four layer-boundary conditions: no adapter owns
  selection; no orchestration layer owns wire formats; no contract layer owns HTTP clients; no
  adapter reads `/dev/shm` or D-Bus live state.

---

## REQ-10 — Handoff artifacts produced per agent

**WHEN** each agent completes its phase,
**THEN** it MUST deposit a handoff file in
`.kiro/specs/zeroclaw-absorbs-op-llm/` per the contract in spec.md §8.

Acceptance criteria:
- All eight handoff files exist and are non-empty at final acceptance.
- Each handoff records: scope, files changed, contract changes, verification
  commands run, known risks, next-agent dependencies.
