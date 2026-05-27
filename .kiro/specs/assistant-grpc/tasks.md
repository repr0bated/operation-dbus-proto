# Implementation Tasks: gRPC Gateway for Assistant Integration

## Completed Tasks

- [x] Task 1 - Proto Definitions and Code Generation
  - 9 proto files in `proto/assistant/` (common, agent, session, task, model, cron, soul, namespace, memory)
  - `build.rs` with tonic-build compiles all protos
  - `assistant_descriptor` file descriptor set for tonic-reflection

- [x] Task 2 - gRPC Server Setup and Authentication
  - `server.rs` — tonic server with WireGuard interceptor, health checks, reflection
  - `auth.rs` — extracts `x-wireguard-pubkey` from gRPC metadata
  - Binary entry point in `src/bin/op-assistant-grpc.rs`

- [x] Task 3 - Transport Layer
  - `transport.rs` — D-Bus first with HTTP-RPC fallback and auto-failover
  - `client.rs` — AssistantClient wraps transport, unwraps JSON-RPC envelopes
  - `incus.rs` — reads IdentitySled from `/dev/shm/plugin_schema.dat`, provides `x-ghostbridge-footprint` and `x-ghostbridge-trace-id` headers for Xray OpenFlow routing
  - Default RPC endpoint: `http://10.200.0.1:50051` (wg-xray container's op-grpc-bridge)

- [x] Task 4-8 - Service Implementations
  - `agents.rs` — AgentService (CRUD + StreamRunEvents)
  - `sessions.rs` — SessionService
  - `tasks.rs` — TaskService (StreamTaskExecution)
  - `models.rs` — ModelService
  - `cron.rs` — CronService
  - `convert.rs` — shared prost_types::Struct ↔ serde_json::Value, RFC3339 timestamps

- [x] Task 11-13 - Memory Services (CozoDB-backed, no HTTP round-trip)
  - `soul.rs` — SoulService → `SoulMemoryStore` (new in op-cognitive-mcp)
  - `namespace.rs` — NamespaceMemoryService → `SoulMemoryStore` bindings + `CognitiveMemoryStore`
  - `memory.rs` — MemoryService → `CognitiveMemoryStore` (CozoDB)
  - Two new Cozo relations in `op-cozo-store`: `soul_memories`, `agent_namespace_bindings`
  - `soul_memory.rs` in op-cognitive-mcp with typed SoulMemory + AgentNamespaceBinding APIs

- [x] Task 16-17 - D-Bus Integration
  - `dbus_service.rs` — zbus interface `ai.assistant.v1` with `call(method, json)` + `run_event` signal

- [x] Task 9/14/18 - Tests
  - 15 unit tests (auth, convert, client, transport, server, incus)
  - 3 integration tests (write/read memory, soul upsert/version, namespace binding)
  - All passing: `cargo test -p op-assistant-grpc`

- [x] Task 20 - s6 Deployment (Artix Linux)
  - `deploy/s6/op-assistant-grpc-srv/` — longrun with producer-for log pipeline
  - `deploy/s6/op-assistant-grpc-log/` — execlineb s6-log companion, consumer-for srv
  - `deploy/s6/config/op-assistant-grpc.conf` — log directives
  - `deploy/op-assistant-grpc-deploy.sh` — build + install + recompile-and-update

## Pending Tasks

- [ ] Task 21 - Cargo clippy pass
  - `cargo clippy -p op-assistant-grpc --all-targets -- -D warnings`
  - Fix any warnings

- [ ] Task 22 - Full workspace test
  - `cargo test --workspace --all-targets --all-features`
  - Verify no regressions from new dependencies

- [ ] Task 23 - Live deployment verification
  - `sudo ./deploy/op-assistant-grpc-deploy.sh`
  - `sudo s6-rc -u change op-assistant-grpc-srv`
  - Verify gRPC endpoint at `127.0.0.1:50052`
  - Verify logs at `/var/log/op-assistant-grpc/`
