# Handoff — Provider Absorption Agent (T-40/T-41 resolved)

## Resolution adopted (user direction)
Keep **one schema source**: the zeroclaw plugin (op-plugins). op-llm **embeds/includes**
that schema via `src/schema.rs` instead of defining a divergent copy. This allows
op-llm to remain (providing its adapter runtime) while the zeroclaw plugin owns
the single authoritative contract. op-llm adds an `op-plugins` dependency
(acyclic: only op-chat/op-web depend on op-llm).

### Dependency facts
- `op-plugins` does **not** depend on `op-llm`; `op-llm` does **not** depend on
  `op-plugins`. So either could re-export from the other without a cycle.
- `op-plugins` already carries runtime deps (`reqwest`, `sha2`, `md5`, `uuid`,
  `chrono`) — it is **not** a pure schema crate. The pure contract/schema crate
  is `op-state-store` (home of `PluginSchema`). The LLM schema-contract types
  added in Phase 1 already live in `op-plugins`
  (`state_plugins/common/llm_projection.rs`), deriving `JsonSchema`.
- The zeroclaw plugin (Orchestration + dispatch) already lives in `op-plugins`
  (`state_plugins/zeroclaw.rs`). So `op-plugins` is the natural Orchestration +
  Adapter host for zeroclaw.

### Caller surface to retire
27 `op_llm::` references across 22 files: `op-chat` (10 files) and `op-web`
(12 files). Most are **runtime data types** used pervasively, not just imports:
`ChatMessage`, `ChatRequest`, `ChatResponse`, `ProviderType`, `ToolDefinition`,
`ModelInfo`, `ToolChoice`, `ToolCallInfo`, plus the `LlmProvider` trait and the
`ChatManager` runtime. `op-llm` itself is ~270KB across 17 modules (gemini.rs
alone is 43KB; the shared `provider.rs` data model is used by every adapter and
every caller).

### The constraint tension (why this needs a decision)
The spec contains three requirements that cannot all hold simultaneously:
1. T-41: move adapters out of `op-llm`.
2. T-40 guard: "if placing adapters in `op-plugins` would make the schema crate
   own runtime HTTP clients, host them in a different module/crate instead."
3. "Do NOT create a new crate" **and** T-53: "retire `op-llm` / remove from
   workspace members."

If `op-llm` is deleted and no new crate may be created, the **only** remaining
Adapter host is `op-plugins` — which is exactly what guard (2) cautions against
for a pure schema crate (though `op-plugins` is not actually pure-schema here).

### Recommended resolution (pending user decision)
`op-plugins` is already an orchestration/runtime crate (it has `reqwest` and the
zeroclaw plugin), and `op-state-store` is the true contract crate. So hosting the
adapter runtime in `op-plugins` under `state_plugins/zeroclaw/adapters/` does not
violate the real layer boundary. This lets `op-llm` be retired per T-53.

The alternative is to keep `op-llm` as a pure Adapter-Layer library (stripped of
any authority — no schema, no D-Bus object, no `ChatManager`-as-authority),
driven by zeroclaw's schema + selection, and NOT delete it (contradicting T-53).

This is the one decision that gates Phase 3/4 execution.
