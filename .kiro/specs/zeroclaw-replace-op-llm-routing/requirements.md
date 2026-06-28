# Requirements: zeroclaw-replace-op-llm-routing

## Overview

Replace Zeroclaw-owned LLM execution and routing logic with `crates/op-llm`,
while preserving Zeroclaw as the exclusive D-Bus/PluginSchema projection
authority for LLM routing state visible to the UI, MCP, and policy surfaces.

---

## Functional Requirements

### FR-1 — op-llm owns all LLM execution

**WHEN** any code path needs to execute a chat request or query model
availability, **IT SHALL** call `op_llm::provider::LlmProvider` (or
`op_llm::chat::ChatManager`) from `crates/op-llm` and **SHALL NOT** perform
provider-specific HTTP, auth, tool-call conversion, or model enumeration
outside that crate.

Acceptance criteria:
- `grep -rn "reqwest\|Client::new\|hyper" crates/op-plugins/src/state_plugins/zeroclaw.rs` returns zero matches.
- `grep -rn "reqwest\|Client::new\|hyper" crates/op-plugins/src/state_plugins/common/llm_projection.rs` returns zero matches.
- `cargo test -p op-llm` passes with tests covering `OpenClawProvider`, `FactoryProvider`, and at least one provider route resolution case.

### FR-2 — Zeroclaw owns only declarative D-Bus projection state

**WHEN** Zeroclaw's `StatePlugin` is active, **IT SHALL** publish and maintain
only:
- selected provider/model (read from env, written to D-Bus state),
- route hint declarations,
- transport metadata,
- MCP-visible tool declarations (`zeroclaw.chat`, `zeroclaw.models.list`),
- UI surface descriptors,
- OSCAL subid/control mapping metadata,
- schema projection (`PluginSchema` derived from `ZeroclawState`).

**IT SHALL NOT** contain provider-specific auth, HTTP client code, model
enumeration network calls, or request/response translation logic.

Acceptance criteria:
- `ZeroclawState` has no field or method that initiates network I/O.
- `zeroclaw.rs` imports nothing from `op_llm` except, optionally, type-only re-exports that are part of the projection contract (e.g., `ProviderType` used purely for schema labelling). If no import is needed, none exists.

### FR-3 — D-Bus is the only control plane for live routing state

**WHEN** any consumer (bridge, UI, MCP tool) reads the active provider/model
or route table, **IT SHALL** read the Zeroclaw D-Bus object at
`/org/opdbus/v1/plugins/zeroclaw` **AND SHALL NOT** read files, environment
variables directly at request time, or poll JSON-RPC endpoints for live state.

Acceptance criteria:
- No new `std::env::var` calls for live routing state added outside bootstrap paths.
- No new `std::fs::read*` calls for live routing state added in plugin or service code.
- `/dev/shm` schema file (`zeroclaw.json`) is written only as a derived projection cache by `ZeroclawPlugin::write_schema_file*()` and is never treated as the live-state source of truth.

### FR-4 — PluginSchema definition stays in the canonical location

**WHEN** `ZeroclawPlugin::schema()` is called, **IT SHALL** return a schema
derived from `ZeroclawState` via schemars, consistent with the pattern already
in place (`schemars_adapter::plugin_schema_from_json`).

**IT SHALL NOT** define a schema inline in any plugin file other than by
calling the already-established `zeroclaw_schema()` function defined in
`zeroclaw.rs`.

`plugin_schema_defs.rs` **SHALL** re-export `zeroclaw_schema` as it already
does; no additional inline definitions are introduced.

Acceptance criteria:
- `cargo check -p op-plugins` passes without new inline `PluginSchema::builder` calls in `zeroclaw.rs`.
- Golden test `derived_schema_matches_hand_rolled` in `zeroclaw.rs` continues to pass.

### FR-5 — op-chat and op-grpc-bridge use op-llm, not Zeroclaw, for execution

**WHEN** `op-chat` dispatches a chat request, **IT SHALL** call
`op_llm::provider::LlmProvider` or `op_llm::chat::ChatManager` directly.
**IT SHALL NOT** duplicate an `LlmProvider` trait or add a second chat
abstraction in `op-chat` or in `op-plugins`.

Acceptance criteria:
- No new trait named `LlmProvider`, `ChatProvider`, or equivalent is introduced outside `crates/op-llm`.
- `op-chat`'s existing use of `op_llm::provider::LlmProvider` and `op_llm::chat::ChatManager` is preserved.

### FR-6 — MCP gateway architecture is unchanged

**WHEN** an external MCP client (NotebookLM, Cursor, Codex, Gemini CLI)
connects, **IT SHALL** reach `cognitive-mcp` at `:3003`.
`compact-mcp` **SHALL** remain loopback/chatbot only.
No new shim service **SHALL** be created.

Acceptance criteria:
- No new service or binary is introduced by this feature.
- `compact-mcp` bind address remains `127.0.0.1:11436`.

### FR-7 — Existing golden/schema tests remain green

**WHEN** the migration is complete, **IT SHALL** be true that:
- `cargo test -p op-plugins -- zeroclaw` passes (covers `derived_schema_matches_hand_rolled`, `all_subids_are_valid`, `should_write_zeroclaw_schema_to_shm`).
- `cargo test -p op-llm` passes.

---

## Non-Goals

- **No new shim service.** Do not create any new binary, proxy, or bridge crate.
- **No compact-mcp external exposure.** `compact-mcp` stays loopback-only.
- **No CLI subprocesses in plugin/service code.** `Command::new("systemctl")`, `Command::new("ip")`, and similar are forbidden.
- **No duplicate LLM trait.** The `LlmProvider` trait exists once in `op-llm`.
- **No live-state file authority.** `/dev/shm/opdbus/schemas/zeroclaw.json` is a projection cache, not the source of truth.
- **No SQLite plugin catalog resurrection.** `SqlitePluginCatalog` stays removed.
- **No broad refactors.** Changes are scoped to the boundary between Zeroclaw's schema projection and `op-llm`'s execution path; unrelated plugins are not touched.
