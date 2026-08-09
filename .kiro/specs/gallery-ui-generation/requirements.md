# Requirements — Model-Agnostic Generative UI Gallery

## Context

The gallery is a rolling set of 200 machine-generated json-render.dev UI specifications. Today, `op-gemma/src/ui_gallery.rs` fills it with deterministic Rust functions. This spec replaces that with a model-agnostic inference loop where any loaded model generates specs from the sealed blob catalog, guided by an operator through the Antigravity chat UI.

The fundamental prompt is deliberately vague and UI-agnostic: "make this dataset as accessible to as many people, industries, causes as possible." The model is never told it's building UI. It proposes novel lenses over structured data; the json-render.dev spec format happens to be the output encoding.

---

## Functional Requirements

### REQ-1 — Model-Agnostic Inference

**REQ-1.1** The generation harness MUST accept inference from any model routed through ZeroClaw. No model-specific prompting, tokenization assumptions, or provider-locked API shapes.

**REQ-1.2** The harness MUST use the OpenAI-compatible `/v1/chat/completions` wire format (the common denominator all ZeroClaw providers expose). Structured output (JSON mode) SHOULD be requested where the provider supports it; the harness MUST parse and validate output regardless.

**REQ-1.3** Model selection is NOT owned by the gallery harness. The `zeroclaw` plugin's `selected_provider` / `selected_model` fields are the routing authority. The harness calls inference through ZeroClaw's endpoint without choosing the model.

**REQ-1.4** If the loaded model cannot produce valid json-render.dev specs after 3 attempts per slot, the harness MUST skip that slot and log the failure. It MUST NOT crash, retry indefinitely, or fall back to a hardcoded generator.

---

### REQ-2 — Baseline Data Tier (Always Present)

**REQ-2.1** The generation context MUST always include:
- The full `PluginSchema` JSON for every sealed plugin in `/dev/shm/opdbus/plugin-blobs/` (read via `op_blob::catalog::read_plugin_schema_shm` or the HTTP equivalent)
- A data-access instruction block explaining: blob section layout, field meanings, method signatures, constraint semantics, subid taxonomy, and how to reference data in `bind` paths
- The json-render.dev component catalog: every legal component `type`, its props schema, and its rendering behavior
- The json-render.dev spec grammar: `root`, `elements`, element fields (`type`, `props`, `children`, `visible`, `on`, `repeat`, `watch`), patch ops for streaming
- The universal prompt: "make this dataset as accessible to as many people, industries, causes as possible"

**REQ-2.2** The baseline context MUST be assembled fresh each run from the live sealed catalog. Stale cached schemas are forbidden — the `catalog_hash` + `generation` from `.manifest.json` MUST be checked.

**REQ-2.3** The data-access instructions MUST be static documentation (not generated). They explain to the model what the schema fields mean and how specs bind to live data. They are written once and versioned in the repo.

---

### REQ-3 — Optional Tier: MCP Cross-Blob Discovery

**REQ-3.1** The operator MUST be able to toggle MCP integration on or off per run through the chat UI. Default: off.

**REQ-3.2** When enabled, the model MUST have access to MCP tool calls that enable cross-blob discovery:
- Query plugins by field type, subid category, method side-effect classification
- Find plugins that share common field names or nested struct shapes
- Search method signatures by input/output type
- List plugins by OSCAL category

**REQ-3.3** MCP tools MUST be read-only. They query the sealed catalog; they do not mutate state, call methods, or write to SHM.

**REQ-3.4** MCP tool schemas MUST be included in the model's tool/function-calling context. The harness MUST handle tool-call responses and feed results back into the generation turn.

**REQ-3.5** When MCP is off, the model receives the full schema dump inline (baseline). When MCP is on, the model MAY receive a summary + tool access instead of the full dump, to stay within context limits on smaller models.

---

### REQ-4 — Optional Tier: Qdrant Semantic Search

**REQ-4.1** The operator MUST be able to toggle Qdrant integration on or off per run through the chat UI. Default: off. Requires MCP to also be on (Qdrant is additive to MCP, not standalone).

**REQ-4.2** When enabled, the model MUST have access to a semantic search tool over a dedicated Qdrant collection containing vectorized blob views. This collection is:
- Separate from `cognitive_mcp`'s collection (shared Qdrant service, partitioned data)
- Populated from the same sealed blobs (so raw schema and vectors cannot disagree)
- Indexed with domain-framing metadata: privacy-ops, network-engineering, compliance, accessibility, operations, development

**REQ-4.3** Semantic search results MUST include: the original schema fragment, the domain framing tag, a relevance score, and the source plugin ID.

**REQ-4.4** The Qdrant collection MUST be refreshed when the sealed catalog changes (new `catalog_hash`). Stale vectors are as bad as stale schemas.

---

### REQ-5 — Antigravity Chat Interface

**REQ-5.1** The operator surface for gallery generation MUST be the Antigravity chat UI — the same DSL-rendered chat interface used elsewhere in the operator console. It is NOT a separate page, modal, or CLI tool.

**REQ-5.2** The chat MUST support a generation session flow:
1. Operator opens a gallery generation session (explicit action — not the default chat mode)
2. UI shows current tier configuration (baseline always on; MCP toggle; Qdrant toggle)
3. Operator optionally types additional guidance ("focus on network observability", "for a compliance auditor", "cross WireGuard and OSCAL plugins")
4. Operator triggers generation (explicit action)
5. Chat streams generation progress (specs produced, validation failures, slot fill count)
6. Session ends — memory cleared, context discarded

**REQ-5.3** The chat interface MUST NOT persist any state between generation sessions. Each run starts fresh. No conversation history, no learned preferences, no accumulated context.

**REQ-5.4** The chat MUST display validation errors inline when a generated spec fails `SpecContract` validation or uses a component not in the catalog. The operator can see what the model tried and why it was rejected.

**REQ-5.5** The operator MUST be able to cancel a generation run mid-flight. Specs already admitted to the gallery remain; unfilled slots are left for the next run.

---

### REQ-6 — Run Isolation

**REQ-6.1** Each generation run MUST be fully stateless. No memory, context, or preference carries over from previous runs.

**REQ-6.2** The generation context for each run is assembled from scratch: live catalog read, static docs loaded, operator guidance captured, tier toggles applied.

**REQ-6.3** If the operator wants the model to "remember" something, they type it into the chat as additional guidance. There is no implicit memory.

**REQ-6.4** Run isolation applies to the generation harness only. The gallery store itself (`CatalogStore`) is persistent and accumulates specs across runs as designed.

---

### REQ-7 — Spec Validation and Gallery Admission

**REQ-7.1** Every generated spec MUST pass validation before gallery admission:
- `root` field exists and references a valid element ID
- Every element has a `type` field matching a known catalog component
- Every element's `props` conform to that component's prop schema
- `children` references resolve to existing element IDs
- `bind` paths are syntactically valid JSON pointers
- No structural duplicate (signature hash) of an existing gallery spec

**REQ-7.2** Specs that fail validation MUST NOT be admitted. The failure is logged and reported to the operator through the chat.

**REQ-7.3** Admitted specs enter the gallery as `Tier::Novelty`. They MUST NOT displace `Tier::StableCore` elements.

**REQ-7.4** The gallery target remains 200 slots. A run fills empty novelty slots up to the cap. If the gallery is already full, new specs retire the oldest novelty elements (FIFO within novelty tier).

---

### REQ-8 — Universal Prompt

**REQ-8.1** The base generation prompt is fixed text, versioned in the repo:

> "Make this dataset as accessible to as many people, industries, causes as possible."

**REQ-8.2** The prompt MUST NOT name the output format, mention UI, dashboards, galleries, or any product shape. The model discovers that the output is json-render.dev specs from the documentation and instruction block — not from the prompt itself.

**REQ-8.3** Operator guidance from the chat is appended as additional context AFTER the base prompt. It supplements; it does not replace.

---

## Non-Functional Requirements

**NFR-1 — Context Efficiency:** The harness MUST work within the context window of models as small as 8K tokens. When context is tight, summarize schemas rather than truncating; prefer MCP tool access over inline dumps.

**NFR-2 — Generation Throughput:** Filling 200 slots should complete in under 10 minutes on a reasonably fast model. Parallelism across slots is acceptable where the provider supports concurrent requests.

**NFR-3 — No New Dependencies Beyond ZeroClaw:** The harness uses ZeroClaw for inference routing. It does NOT add a direct dependency on any provider SDK (no `anthropic` crate, no `openai` crate, no `google-genai` crate). Wire format is HTTP + JSON.

**NFR-4 — Catalog Freshness:** If the sealed catalog changes during a run (new `catalog_hash`), the run SHOULD complete with the catalog it started with. The next run picks up the new catalog.

**NFR-5 — Graceful Degradation:** If ZeroClaw is unreachable, or the model produces only invalid output, the gallery retains its existing specs. A failed run never blanks the gallery.
