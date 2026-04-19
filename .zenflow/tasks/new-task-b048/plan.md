# Spec and build

## Configuration
- **Artifacts Path**: {@artifacts_path} → `.zenflow/tasks/{task_id}`

---

## Agent Instructions

Ask the user questions when anything is unclear or needs their input. This includes:
- Ambiguous or incomplete requirements
- Technical decisions that affect architecture or user experience
- Trade-offs that require business context

Do not make assumptions on important decisions — get clarification first.

---

## Workflow Steps

### [x] Step: Technical Specification
<!-- chat-id: 2f430ca4-8946-423d-bbd5-6c99962873ba -->

Assess the task's difficulty, as underestimating it leads to poor outcomes.
- easy: Straightforward implementation, trivial bug fix or feature
- medium: Moderate complexity, some edge cases or caveats to consider
- hard: Complex logic, many caveats, architectural considerations, or high-risk changes

Create a technical specification for the task that is appropriate for the complexity level:
- Review the existing codebase architecture and identify reusable components.
- Define the implementation approach based on established patterns in the project.
- Identify all source code files that will be created or modified.
- Define any necessary data model, API, or interface changes.
- Describe verification steps using the project's test and lint commands.

Save the output to `{@artifacts_path}/spec.md` with:
- Technical context (language, dependencies)
- Implementation approach
- Source code structure changes
- Data model / API / interface changes
- Verification approach

If the task is complex enough, create a detailed implementation plan based on `{@artifacts_path}/spec.md`:
- Break down the work into concrete tasks (incrementable, testable milestones)
- Each task should reference relevant contracts and include verification steps
- Replace the Implementation step below with the planned tasks

Rule of thumb for step size: each step should represent a coherent unit of work (e.g., implement a component, add an API endpoint, write tests for a module). Avoid steps that are too granular (single function).

Important: unit tests must be part of each implementation task, not separate tasks. Each task should implement the code and its tests together, if relevant.

Save to `{@artifacts_path}/plan.md`. If the feature is trivial and doesn't warrant this breakdown, keep the Implementation step below as is.

---

### [x] Step: Requirements Document
<!-- chat-id: b15d25be-a9a5-4605-aff5-67c8821b07db -->

Generate `.kiro/specs/openclaw-cognitive-platform/requirements.md` covering all 11 architectural areas (A–K from the brief):
- A. Cognitive MCP / MCP surfaces
- B. gRPC topology, trust boundaries, and server reflection (`tonic-reflection` — all services must register in combined `operation_descriptor.bin`; client-side reflection-driven dispatch in `op-chat/grpc_client.rs`)
- C. RCP / JSON-RPC live-state substrate
- D. ASP (Application Service Plane)
- E. Tools and tool registry governance
- F. Plugin registry lifecycle
- G. Agent registry and orchestration
- H. Memory architecture
- I. Dashboard and JSON stream
- J. Isolation and trust boundaries
- K. Interface strategy (chatbot-first)

Each area must include: user stories, acceptance criteria, constraints, and non-goals.
Reference existing crate specs in `crates/crates/SPECS/` and `OPENCLAW-CONTEXT.md`.

---

### [ ] Step: Design Document

Generate `.kiro/specs/openclaw-cognitive-platform/design.md` mapping each requirement to architecture:
- D-Bus authority model and object hierarchy
- MCP surface topology (internal vs external, compact vs full vs cognitive)
- gRPC service mesh and trust contracts
- JSON-RPC live-state mirror (OVSDB/NonNet as queryable operational graph)
- ASP definition and interaction with schema/plugins/orchestration
- Tool registry schema, permissions, execution pathways, audit logging
- Plugin registry lifecycle, schema publication, validation hooks
- Agent registry: identity, roles, orchestration hierarchy, allowed tools
- Memory architecture: long-term, operational, contextual; governance rules
- Dashboard/JSON stream: origin, transport (SSE/WebSocket), typed event schema
- Trust zones: chatbot cognition / orchestration plane / execution plane / external
- Chatbot-first interface: query, inspect, invoke, manage via chat + dashboard

---

### [ ] Step: Tasks Document

Generate `.kiro/specs/openclaw-cognitive-platform/tasks.md` with ordered, dependency-sequenced implementation tasks:
- Start with schema foundations and registries (D-Bus object model, plugin schemas, tool schemas)
- Progress through protocol surfaces (D-Bus, gRPC, MCP, JSON-RPC)
- Include memory subsystem, policy enforcement, execution brokering
- Cover dashboard/JSON stream pipeline
- End with UI/chat integration and chatbot-first interface
- Each task: references relevant contracts, includes verification steps, bundles unit tests

---

### [ ] Step: Openclaw LLM Provider Integration

Implement the openclaw LLM backend wiring in the existing codebase:
- Create `crates/crates/op-llm/src/openclaw.rs` implementing `LlmProvider` trait (OpenAI-compatible, hits openclaw gateway at `OPENCLAW_BASE_URL`)
- Add `OpenClaw` variant to `ProviderType` enum in `crates/crates/op-llm/src/provider.rs`
- Export from `crates/crates/op-llm/src/lib.rs`
- Update `crates/crates/op-chat/src/llm.rs` `create_provider()` to handle `"openclaw"` type
- Update `src/chatbot/mod.rs` and `src/main.rs` to wire provider selection via env/config (`LLM_PROVIDER=openclaw`, `OPENCLAW_BASE_URL`, `OPENCLAW_DEFAULT_MODEL`)
- Write unit tests for the new provider (mock HTTP, model listing, tool call serialization)
- Run `cargo test -p op-llm` and `cargo test -p op-chat`

---

### [ ] Step: Verify and Lint

Run full CI-equivalent verification:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets --all-features`
- `cd crates && npm run typecheck`
- `cd crates && npm run lint`
- Write `{@artifacts_path}/report.md` summarising what was implemented, how it was tested, and any issues encountered
