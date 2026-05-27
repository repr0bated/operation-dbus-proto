# Full SDD workflow

## Configuration
- **Artifacts Path**: `.zenflow/tasks/new-task-b808`

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

### [ ] Step: Task 1 - Proto Definitions and Code Generation
- Create `crates/op-assistant-grpc/proto/assistant/` directory structure
- Define `agent.proto` with AgentService, ListAgentsRequest/Response, Agent message
- Define `session.proto` with SessionService, ListSessionsRequest/Response, Session message
- Define `task.proto` with TaskService, ListToolsRequest/Response, ExecuteTaskRequest/Response
- Define `model.proto` with ModelService, ListModelsRequest/Response, Model message
- Define `cron.proto` with CronService, ListCronJobsRequest/Response, CronJob message
- Create main `assistant.proto` that imports all service definitions
- Configure `build.rs` with `tonic-build` to compile proto files
- Generate Rust code from proto definitions
- Write tests verifying proto code generation

### [ ] Step: Task 2 - gRPC Server Setup and Authentication
- Create `crates/op-assistant-grpc/src/server.rs` with tonic server setup
- Implement `WireGuardAuth` middleware to extract and validate WireGuard identity
- Create `crates/op-assistant-grpc/src/auth.rs` with identity extraction logic
- Configure gRPC server to use authentication middleware
- Set up gRPC server to listen on configurable port (default 50051)
- Write tests verifying server startup and authentication middleware

### [ ] Step: Task 3 - Assistant HTTP Client Wrapper
- Create `crates/op-assistant-grpc/src/client.rs` with Assistant HTTP client
- Implement request conversion: gRPC → Assistant HTTP
- Implement response conversion: Assistant HTTP → gRPC
- Handle Assistant's JSON-RPC response format
- Implement error mapping: Assistant errors → gRPC status codes
- Add configurable timeout for Assistant HTTP requests
- Write tests verifying HTTP client functionality

### [ ] Step: Task 4 - AgentService Implementation
- Create `crates/op-assistant-grpc/src/agents.rs` with AgentService implementation
- Implement `ListAgents` - list all configured agents
- Implement `GetAgent` - get agent by ID
- Implement `CreateAgent` - create new agent
- Implement `UpdateAgent` - update existing agent
- Implement `DeleteAgent` - delete agent
- Implement `StartRun` - start agent run
- Implement `StreamRunEvents` - stream run events
- Implement `CancelRun` - cancel running run
- Write tests for each AgentService method

### [ ] Step: Task 5 - SessionService Implementation
- Create `crates/op-assistant-grpc/src/sessions.rs` with SessionService implementation
- Implement `ListSessions` - list all active sessions
- Implement `GetSession` - get session by ID
- Implement `CreateSession` - create new session
- Implement `DeleteSession` - delete session
- Implement `GetSessionHistory` - get session history
- Implement `SendMessage` - send message to session
- Write tests for each SessionService method

### [ ] Step: Task 6 - TaskService Implementation
- Create `crates/op-assistant-grpc/src/tasks.rs` with TaskService implementation
- Implement `ListTools` - list available tools
- Implement `ExecuteTask` - execute a task
- Implement `StreamTaskExecution` - stream task execution events
- Implement `GetTaskResult` - get task result
- Write tests for each TaskService method

### [ ] Step: Task 7 - ModelService Implementation
- Create `crates/op-assistant-grpc/src/models.rs` with ModelService implementation
- Implement `ListModels` - list available models
- Implement `GetModel` - get model details
- Implement `SwitchModel` - switch active model
- Write tests for each ModelService method

### [ ] Step: Task 8 - CronService Implementation
- Create `crates/op-assistant-grpc/src/cron.rs` with CronService implementation
- Implement `ListCronJobs` - list scheduled cron jobs
- Implement `CreateCronJob` - create new cron job
- Implement `DeleteCronJob` - delete cron job
- Implement `TriggerCronJob` - trigger cron job
- Write tests for each CronService method

### [ ] Step: Task 9 - Integration Tests
- Create integration test suite in `crates/op-assistant-grpc/tests/`
- Test full request flow: gRPC → proxy → Assistant HTTP
- Test authentication flow with WireGuard identity
- Test error scenarios (Assistant unavailable, invalid requests)
- Test concurrent requests
- Test streaming endpoints

### [ ] Step: Task 10 - Documentation and Cleanup
- Update README.md with gRPC API documentation
- Add examples for using the gRPC client
- Document configuration options
- Remove or deprecate HTTP handlers in favor of gRPC (optional)
- Run `cargo clippy --all-targets` and fix any warnings
- Run `cargo test --workspace` to verify no regressions