# Full SDD workflow

## Configuration
- **Artifacts Path**: {@artifacts_path} → `.zenflow/tasks/{task_id}`

---

## Agent Instructions

---

## Workflow Steps

### [x] Step: Requirements
<!-- chat-id: 85a49cb3-8fde-4f7f-afee-5d3daeb2d8e2 -->

Create a Product Requirements Document (PRD) based on the feature description.

1. Review existing codebase to understand current architecture and patterns
2. Analyze the feature definition and identify unclear aspects
3. Ask the user for clarifications on aspects that significantly impact scope or user experience
4. Make reasonable decisions for minor details based on context and conventions
5. If user can't clarify, make a decision, state the assumption, and continue

Focus on **what** the feature should do and **why**, not **how** it should be built. Do not include technical implementation details, technology choices, or code-level decisions — those belong in the Technical Specification.

Save the PRD to `{@artifacts_path}/requirements.md`.

### [x] Step: Technical Specification

Create a technical specification based on the PRD in `{@artifacts_path}/requirements.md`.

1. Review existing codebase architecture and identify reusable components
2. Define the implementation approach

Do not include implementation steps, phases, or task breakdowns — those belong in the Planning step.

Save to `{@artifacts_path}/spec.md` with:
- Technical context (language, dependencies)
- Implementation approach referencing existing code patterns
- Source code structure changes
- Data model / API / interface changes
- Verification approach using project lint/test commands

### [x] Step: Planning

Create a detailed implementation plan based on `{@artifacts_path}/spec.md`.

1. Break down the work into concrete tasks
2. Each task should reference relevant contracts and include verification steps
3. Replace the Implementation step below with the planned tasks

Rule of thumb for step size: each step should represent a coherent unit of work (e.g., implement a component, add an API endpoint). Avoid steps that are too granular (single function) or too broad (entire feature).

Important: unit tests must be part of each implementation task, not separate tasks. Each task should implement the code and its tests together, if relevant.

If the feature is trivial and doesn't warrant full specification, update this workflow to remove unnecessary steps and explain the reasoning to the user.

Save to `{@artifacts_path}/plan.md`.

### [ ] Step: Task 1 - Protobuf Plumbing and gRPC Server Scaffold
- Verify and ensure `.proto` files are correctly compiled via `tonic-build` in the workspace.
- Set up the base Tonic gRPC server structure in `crates/op-agents/src/server.rs` (or `dbus-agent-manager.rs`).
- Define the basic empty service implementations for `AgentExecution` and `AgentLifecycle`.
- [ ] Write tests verifying proto code generation and basic gRPC server startup.

### [ ] Step: Task 2 - Dynamic Persona Schema and Registry Migration
- Define JSON schemas (`AgentConfig`) for `PersonaAgent` capabilities and prompts.
- Create the unified `PersonaAgent` struct in `crates/op-agents/src/agents/persona.rs`.
- Implement configuration loading in `crates/op-agents/src/agent_catalog.rs` from YAML/JSON files.
- Remove the 50+ statically defined persona structs from `crates/op-agents/src/agents/*` and replace them with configuration files in `config/agents/personas.yaml`.
- [ ] Write unit tests for configuration parsing, schema validation, and dynamic loading into the `AgentRegistry`.

### [ ] Step: Task 3 - Backend Agent Dispatch Implementation
- Wire the Tonic server implementations in `crates/op-agents/src/server.rs` to the `AgentRegistry`.
- Implement incoming request routing based on `agent_id` to `Arc<dyn AgentTrait>`.
- Serialize `TaskResult` into the appropriate gRPC response format, including streaming logic if applicable.
- [ ] Write integration tests simulating client requests to the gRPC server and verifying registry routing.

### [ ] Step: Task 4 - Client Transport & Connection Pool Wiring
- Rewrite `crates/op-chat/src/grpc_client.rs` to replace `simulated: true` with a real `tonic::transport::Channel`.
- Wire up `start_session`, `end_session`, `execute`, and `execute_stream` using the generated client stubs.
- Implement connection pooling and lifecycle management using `AgentPoolConfig` in `crates/op-chat/src/orchestration/grpc_pool.rs`.
- [ ] Write tests with a mocked Tonic server to verify client request formatting, streaming handling, and connection fallback/circuit breaker logic.

### [ ] Step: Task 5 - Assistant Provider and Cognitive MCP Integration
- Implement `crates/op-llm/src/assistant.rs` to expose the Assistant container (Incus) as an `LlmProvider`.
- Update `crates/op-chat/src/cognitive_orchestrator.rs` to utilize Assistant for context propagation and memory retrieval.
- Wire up memory sync between `op-cognitive-mcp` and Assistant via HTTP/gRPC.
- [ ] Write tests to verify the Assistant provider correctly formats requests and parses tool-calling responses.

### [ ] Step: Task 6 - Workstacks and Workflows Implementation
- Implement `crates/op-chat/src/orchestration/workstacks.rs` to manage workflow executions using the pocketflow_rs engine.
- Implement `crates/op-chat/src/orchestration/skills.rs` to define concrete skill bindings.
- Interface with the `pocketflow_rs` workflow engine, ensuring schema validation aligns with plugin instantiation.
- [ ] Write unit tests verifying workstack execution state transitions and skill resolution.
