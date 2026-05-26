# Technical Specification: Agent Orchestration, Dynamic Personas & Assistant Integration

## 1. Technical Context
- **Languages & Frameworks**: Rust, Tonic/gRPC (utilizing the `prost` autocoding method for proto generation), Serde for configuration, JSON Schema for "schema as code" validation.
- **Dependencies**: `tonic`, `prost`, `serde_yaml`/`serde_json`, `anyhow`, `tokio`, `pocketflow_rs` (rewritten robust workflow engine, old name retained in `Cargo.toml`), existing `op-*` crates (`op-chat`, `op-agents`, `op-llm`, `op-tools`, `op-plugins`, `op-cognitive-mcp`).

## 2. Implementation Approach

### 2.1. True gRPC/Tonic Transport (Autocoding Method)
- **Code Generation**: Ensure the `.proto` files (`op_chat.orchestration.proto`, etc.) are compiled via `tonic-build` in a centralized `build.rs` or `op-grpc-bridge` (the existing autocoding method) and exported for both client and server consumption.
- **Client Implementation (`op-chat/src/grpc_client.rs`)**: 
  - Remove all `simulated: true` fallbacks.
  - Implement `connect()` to establish a `tonic::transport::Channel` using `config.address`.
  - Implement `start_session()`, `end_session()`, `execute()`, and `execute_stream()` by making real RPC calls to the `AgentLifecycle` and `AgentExecution` services.
- **Connection Pool (`op-chat/src/orchestration/grpc_pool.rs`)**:
  - Activate the existing `AgentPoolConfig` with circuit breaker, backoff, and concurrency semaphores to manage the established tonic channels.

### 2.2. Backend Agent Dispatch & Registries
- **gRPC Server**: Implement a Tonic gRPC server inside `op-agents` (e.g., `dbus-agent-manager.rs`) that serves the `AgentExecution` and `AgentLifecycle` protocols.
- **Agent Registry Wiring (`op-agents/src/agent_registry.rs`)**: 
  - The gRPC service will look up requested agents via `agent_id` in the `AgentRegistry`.
  - The registry will route the execution payload to the corresponding `Arc<dyn AgentTrait>`, wait for completion, and serialize the `TaskResult` into a gRPC response.
- **Protocol Consistency**: Ensure the agent and tool registries follow the exact same instantiation and validation protocols used in plugin creation (`op-plugins`), relying strictly on defined schema envelopes.

### 2.3. Schema as Code & Dynamic Personas Migration
- **Schema Validation**: All dynamic configuration files will be strictly validated against JSON Schema definitions at runtime, ensuring robust "schema as code" compliance across the platform.
- **Configuration Format**: Move all 50+ persona definitions from `op-agents/src/agents/*` into a single configuration directory (e.g., `config/agents/personas.yaml` or multiple JSON/YAML files).
- **`PersonaAgent` Struct**: Create a unified `PersonaAgent` that implements `AgentTrait`. It will load its identity, description, capabilities, operations, and system prompt dynamically from the validated schema definition.
- **AgentCatalog Initialization**: Update `AgentCatalog` (`op-agents/src/agent_catalog.rs`) to parse the config directory at startup and dynamically register all personas into the `AgentRegistry`.

### 2.4. Workstacks and Workflows Double Pass
- **Workflow Engine Refinement**: Make a second, robust pass over `pocketflow_rs` (the rewritten workflow engine retaining its GitHub TOML name) and the `op-workflows` definitions. 
- **Workstacks Implementation (`op-chat`)**: Implement the missing `workstacks.rs` and `skills.rs` in `op-chat`. Ensure these components directly interface with the validated registries and workflow engine, avoiding any feature loss from the original design documentation.

### 2.5. OpenClaw Integration (Cognitive MCP Backend)
- **Integration Role**: OpenClaw (running in the Incus container) will serve as the primary Cognitive MCP Backend.
- **Provider System (`op-llm`)**: Implement `crates/op-llm/src/openclaw.rs` to expose OpenClaw as an `LlmProvider` compliant with OpenAI’s API format, supporting tool-calling and model switching.
- **Cognitive Orchestration (`op-chat/src/cognitive_orchestrator.rs`)**: Integrate OpenClaw's memory retrieval and context propagation. OpenClaw's context awareness will drive the chat session's workstack execution and tool selection.
- **Memory Sync**: Connect `op-cognitive-mcp` to OpenClaw via HTTP/gRPC to synchronize session-based ephemeral memory and long-term memory retrieval.

## 3. Source Code Structure Changes
- **`crates/op-chat/src/grpc_client.rs`**: Full rewrite to implement actual tonic channel calls.
- **`crates/op-agents/src/agents/base.rs`**: Adjust to support data-driven system prompts.
- **`crates/op-agents/src/agent_catalog.rs`**: Add YAML/JSON schema loading logic.
- **`crates/op-agents/src/agents/*`**: Delete the 50+ static struct files and replace them with `config/agents/personas.yaml` and a generic `persona.rs`.
- **`crates/op-agents/src/server.rs`**: (New) Implements the agent gRPC listener.
- **`crates/op-chat/src/orchestration/workstacks.rs`**: (New) Missing workstack implementation.
- **`crates/op-chat/src/orchestration/skills.rs`**: (New) Missing skills implementation.
- **`crates/op-llm/src/assistant.rs`**: (New) Assistant provider implementation (replaces openclaw.rs)

## 4. Data Model / API / Interface Changes
- **AgentConfig Schema**: A new JSON schema defining the shape of `PersonaAgent` configurations (name, capabilities, prompt).
- **GrpcAgentClient Trait**: Method signatures remain the same, but internal structs will carry a persistent `tonic::transport::Channel`.
- **LlmProvider**: `ProviderType` extended with `Assistant` variant (replaces `OpenClaw`)

## 5. Verification Approach
- **Schema Validation**: Unit tests verifying that `personas.yaml` fully complies with the `AgentConfig` JSON Schema.
- **Tonic Tests**: Mocked gRPC server tests verifying that `GrpcAgentClient` properly formats requests and handles stream chunking.
- **Integration Tests**: Verify that `AgentRegistry` successfully loads all 50+ personas at startup without dropping configuration fields.
- **Commands**: 
  - `cargo test -p op-agents -p op-chat -p op-llm`
  - `cargo clippy --all-targets` to enforce strict project linting rules.