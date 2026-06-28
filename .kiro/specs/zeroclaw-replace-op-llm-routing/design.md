# Design: zeroclaw-replace-op-llm-routing

## Architectural Boundary

```
┌──────────────────────────────────────────────────────────────┐
│  crates/op-plugins  (D-Bus projection layer)                 │
│                                                              │
│  ZeroclawPlugin (zeroclaw.rs)                                │
│  ├─ ZeroclawState            ← schema-only struct            │
│  │   ├─ selected_provider    ← env-seeded, D-Bus-written     │
│  │   ├─ selected_model                                       │
│  │   ├─ transport: LlmTransport                              │
│  │   └─ projection: LlmProjection  ← route table, tools, UI │
│  ├─ zeroclaw_schema()        ← schemars-derived PluginSchema │
│  └─ write_schema_file*()     ← tmpfs projection cache only   │
│                                                              │
│  common/llm_projection.rs    ← shared schema structs only    │
└──────────────────────────────────────────────────────────────┘
         │  D-Bus object: /org/opdbus/v1/plugins/zeroclaw
         │  (consumers read routing state here)
         ▼
┌──────────────────────────────────────────────────────────────┐
│  crates/op-llm  (execution layer)                            │
│                                                              │
│  provider.rs    → LlmProvider trait, ChatRequest/Response    │
│  chat.rs        → ChatManager (provider selection + routing) │
│  openclaw.rs    → OpenClawProvider (OpenAI-compat endpoint)  │
│  factory.rs     → FactoryProvider                            │
│  gemini.rs      → GeminiClient                               │
│  anthropic.rs   → AnthropicClient                            │
│  … (all other providers)                                     │
└──────────────────────────────────────────────────────────────┘
         │  consumed by:
         ├─ crates/op-chat  (ChatManager / LlmProvider)
         └─ crates/op-grpc-bridge  (forwarded chat RPC)
```

---

## Crate Ownership Table

| Concern | Owner crate | Forbidden in |
|---|---|---|
| LLM network execution | `op-llm` | `op-plugins`, `op-chat` |
| Provider-specific auth (API key, OAuth) | `op-llm` | `op-plugins` |
| Model enumeration (live network calls) | `op-llm` | `op-plugins` |
| Request/response conversion (OpenAI compat) | `op-llm` | `op-plugins` |
| Tool-call format conversion | `op-llm` | `op-plugins` |
| Provider/model route declarations | `op-plugins/zeroclaw` | `op-llm` |
| Selected provider/model (D-Bus state) | `op-plugins/zeroclaw` | `op-llm` |
| MCP-visible tool declarations | `op-plugins/zeroclaw` | `op-llm` |
| UI surface descriptors | `op-plugins/zeroclaw` | `op-llm` |
| PluginSchema projection | `op-plugins/zeroclaw` | `op-llm` |
| tmpfs schema projection cache | `op-plugins/zeroclaw` | `op-llm` |
| OSCAL subid metadata | `op-plugins/zeroclaw` | `op-llm` |
| Chat dispatch | `op-chat` via `op-llm` | `op-plugins` |

---

## What Is Already Correct

The current `zeroclaw.rs` is already largely a projection plugin:
- `current_state()` builds `ZeroclawState` from env vars at startup — no network I/O.
- `ZeroclawState` holds declarative structs (`Provider`, `ModelRoute`, `Router`, etc.) with no execution logic.
- `zeroclaw_schema()` derives `PluginSchema` from `ZeroclawState` via schemars.
- `write_schema_file_to()` writes the tmpfs projection cache.
- `StatePlugin` methods (`apply_state`, `verify_state`) are stubs — correct.

The `common/llm_projection.rs` file holds shared schema structs; it contains
no network I/O.

---

## What Needs Clarification / Cleanup

### 1. `zeroclaw.tools` — execution vs. declaration

The two tools declared in `ZeroclawState.projection.tools` (`zeroclaw.chat`,
`zeroclaw.models.list`) are **tool declarations** for the MCP/UI surfaces —
they describe what tools exist and their parameter schemas. They do not execute
anything. This is correct as-is.

Execution of `zeroclaw.chat` **SHALL** be handled by `op-chat`/`op-llm` when
an MCP client calls the tool. Zeroclaw declares; `op-llm` executes.

No change is needed here unless a future MCP handler in Zeroclaw is found to
call into a provider directly — that would be the violation to remove.

### 2. `ModelRoute.available` / `status_reason` — projection vs. liveness

All model routes are currently declared with `available: false` and
`status_reason` describing they need backend projection. This is correct
declarative behaviour.

Liveness projection (flipping `available: true` after `op-llm` confirms the
backend is reachable) is **out of scope** for this spec. That is a future
`obs.*` mutation. Do not introduce a polling loop here.

### 3. `ZeroclawPlugin::current_state()` reads env vars at construction time

This is a startup-time bootstrap read, not a live-state file read. Permitted by
the architecture rules (bootstrap scripts/init are the exception). No change
needed.

### 4. op-llm has no explicit `ollama` provider module

The current provider list in `zeroclaw.rs` declares `ollama` as a route target.
`op-llm` does not have a dedicated `ollama.rs`, but `OpenClawProvider` connects
to any OpenAI-compatible endpoint including Ollama's `/v1/chat/completions`.
The route declaration is correct; no new module is required.

---

## Data Flow: Chat Request

```
User/MCP client
    │
    ▼
cognitive-mcp (:3003) — external gateway
    │  reads Zeroclaw D-Bus object for route hints
    │
    ▼
op-chat :: ChatManager
    │  calls op_llm::provider::LlmProvider (selected by hint)
    │
    ▼
op-llm (e.g. OpenClawProvider, FactoryProvider, GeminiClient)
    │  HTTP/gRPC to upstream
    │
    ▼
upstream LLM (Ollama, Gemini, OpenRouter, …)
```

Zeroclaw D-Bus object is **read** by the gateway and `op-chat` for hint
resolution. It is **never** in the request execution path.

---

## OSCAL Subid Taxonomy for This Feature

New artifacts introduced by this spec (if any schema structs are adjusted):

| Artifact | subid |
|---|---|
| Zeroclaw schema contract | `sch.software.plugin.zeroclaw.schema@v1` (existing) |
| Zeroclaw transport metadata | `sch.software.zeroclaw-transport.schema@v1` (existing) |
| Selected provider mutation | `mut.service.zeroclaw.selected-provider@v1` (existing) |
| Selected model export | `exp.service.zeroclaw.selected-model@v1` (existing) |
| Schema projection publish | `prj.service.projected-object.publish@v1` (existing pattern) |

No new subids are required unless new fields are added to `ZeroclawState`.
If new fields are added, use the format `<category>.service.zeroclaw.<field>@v1`.

---

## Dependency Graph (unchanged by this feature)

```
op-plugins  →  op-state, op-state-store   (no dep on op-llm — correct)
op-chat     →  op-llm, op-plugins          (existing)
op-grpc-bridge → op-llm                   (existing)
cognitive-mcp  → op-cognitive-mcp         (reads Zeroclaw D-Bus state)
```

`op-plugins` **MUST NOT** gain a dependency on `op-llm`. If Zeroclaw needs to
reference `ProviderType` for schema labelling, use a local string enum or
mirror only the string representation. Do not add `op-llm` to
`crates/op-plugins/Cargo.toml`.
