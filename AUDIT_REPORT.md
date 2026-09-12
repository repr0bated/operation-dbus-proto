# Factory AI Architectural Audit Report
## OP-DBUS: Rust/Tonic/gRPC Distributed Messaging, Routing, Compliance, and Identity Pipeline

**Auditor:** Factory AI Architectural Audit Agent  
**Date:** 2026-06-10  
**Branch:** feat/sled-source-port-salt  
**Commit:** 2c2cfb1d  

> **Historical audit notice (2026-08-31):** findings that describe the former
> raw shared-memory identity/schema file are retained as evidence of the
> retired design, not as current architecture. Active code now uses projected
> session identity and the sealed plugin-blob catalog.

---

## 1. Executive Summary

The implementation is **architecturally coherent in intent but fragmented in execution**. The codebase demonstrates a sophisticated understanding of the desired architecture: D-Bus as the control plane, gRPC as the external API surface, WireGuard-backed identity sleds, shared-memory schema catalogs, Xray tag-based routing, and vectorized audit trails. However, the implementation exhibits a significant gap between architectural vision and production-ready completeness.

**Verdict:** The architecture is **sound but incomplete**. It is **not production-ready** in its current state. The core control-plane abstractions exist, but critical enforcement layers (ZeroClaw, OSCAL compliance binding, deterministic routing policy) are either stubs or configuration declarations rather than active enforcement mechanisms. The UI layer duplicates schema definitions rather than generating from the canonical source. Multiple trust boundaries have gaps, and several protocol adapters bypass the core schema pipeline.

---

## 2. Critical Findings

### CF-1: D-Bus Object Path Drift and Non-Canonical Paths
- **Severity:** Critical
- **Affected files/modules:** `crates/op-xray-daemon/src/dbus.rs`, `crates/op-xray-daemon/src/main.rs`, `crates/op-identity/src/schema_bridge.rs`, `crates/op-projection/src/dbus_server.rs`
- **Why it matters:** The AGENTS.md mandate states: "D-Bus objects own plugin state. Every plugin is a D-Bus object at `/org/opdbus/v1/plugins/<name>`." However, the Xray daemon registers at `/opdbus/v1/xray` (missing `org` prefix and `plugins` segment) under bus name `opdbus.v1`. The ProjectionDbusServer registers under `org.opdbus.projection` (not `org.opdbus.v1`). The identity shuttle references `/opdbus/v1/plugins/zeroclaw` and `/opdbus/v1/plugins/oscal_subid_registry` in the ZeroclawPlugin state. This violates the canonical path system defined in `op_plugins::canonical` and creates multiple divergent path namespaces.
- **Recommended direction:** Enforce `plugin_path(name)` from `op_plugins::canonical` for ALL D-Bus object registrations. Add a CI check that fails on any hardcoded D-Bus path not generated through the canonical module.
- **Blocks production readiness:** Yes. Path divergence breaks introspection, routing, and schema binding.

### CF-2: ZeroClaw Is a Configuration Plugin, Not an Enforcement Layer
- **Severity:** Critical
- **Affected files/modules:** `crates/op-plugins/src/state_plugins/zeroclaw.rs`, `crates/op-grpc-bridge/src/interceptor.rs`
- **Why it matters:** The audit prompt requires ZeroClaw to be a policy/enforcement layer with deterministic fallback, schema-bound outputs, logged decisions, and explicit fail-closed behavior. The current `ZeroclawPlugin` is a `StatePlugin` that publishes a JSON state blob describing model routes. There is no actual request interception, no policy evaluation engine, no decision logging, and no enforcement. The `GhostbridgeInterceptor` performs identity validation but does not consult ZeroClaw for routing or policy decisions. The "router" object declares that gemma4 classifies requests, but no classification code exists.
- **Recommended direction:** Implement a `ZeroClawPolicyEngine` that intercepts gRPC requests after Ghostbridge identity validation and before handler dispatch. It must evaluate deterministic policy rules first, then optionally consult a model for classification, log every decision with trace_id, and default to deny on any failure.
- **Blocks production readiness:** Yes. Without policy enforcement, the system has no authorization beyond WireGuard identity.

### CF-3: UI Schema Definitions Are Manually Duplicated, Not Generated from Canonical Schema
- **Severity:** Critical
- **Affected files/modules:** `crates/op-web/ui/src/pages/StatePage.tsx`, `crates/op-web/ui/src/pages/ConfigPage.tsx` (and inferred others)
- **Why it matters:** The AGENTS.md rule states: "If it does not have a validated schema, it does not exist." The canonical schema source is `PluginSchema` in `op-plugins`, serialized to `/dev/shm/live-schema.json`. However, the React UI hardcodes `PLUGIN_DEFS` with inline JSON Schema objects (`interface`, `ip_address`, `mtu`, etc.) that duplicate the `net_plugin_schema()` and other Rust definitions. The UI does not fetch the canonical schema from the gRPC `PluginService.GetSchema` endpoint to drive rendering. This guarantees drift between backend and frontend.
- **Recommended direction:** Remove all hardcoded `PLUGIN_DEFS`. Implement a `SchemaRenderer` that fetches `GetSchema` for each plugin on mount, validates the returned `PluginSchema` against a meta-schema, and generates form controls dynamically.
- **Blocks production readiness:** Yes. Manual duplication violates the single source of truth and will cause rendering failures as schemas evolve.

### CF-4: gRPC DbusPassthrough Service Is Generic Blob Transport
- **Severity:** High
- **Affected files/modules:** `crates/op-grpc-bridge/proto/operation.proto` (DbusPassthrough service), `crates/op-grpc-bridge/src/grpc_server.rs`
- **Why it matters:** `DbusPassthrough.Call` accepts `string json_body` and returns `string json_result`. It is explicitly designed as an "extension point for services that register on the bus without needing dedicated gRPC proto definitions." This is precisely the anti-pattern the audit flags: gRPC used as a generic blob transport instead of a strongly typed API. It bypasses schema validation, OSCAL mapping, audit metadata attachment, and type safety. It creates a side-channel outside the schema pipeline.
- **Recommended direction:** Deprecate `DbusPassthrough` in production. Every service that needs gRPC exposure must have a dedicated proto definition generated from its `PluginSchema`. Use proto generation from schema as the only path to gRPC service definition.
- **Blocks production readiness:** Partially. It creates an unaudited bypass path.

### CF-5: OSCAL Sub-IDs Exist as a Taxonomy but Are Not Enforced or Rendered
- **Severity:** High
- **Affected files/modules:** `crates/op-identity/src/schema_bridge.rs` (SubidTaxonomy), `crates/op-compliance/src/lib.rs` (LawFirm), `crates/op-plugins/src/state_plugins/zeroclaw.rs`
- **Why it matters:** The `SubidTaxonomy` parser and the seven-category system (`src`, `prj`, `sch`, `mut`, `obs`, `evt`, `exp`) are well-designed and correctly formatted. However, they exist only as a Rust parsing utility and occasional inline string comments (e.g., `obs.service.code-rag.search@v1`). There is no central registry enforcing uniqueness. The `LawFirm` compliance engine is a stub that checks for PII strings and version fields but does not validate OSCAL control mappings. There is no `oscal_subid_registry` plugin implementation despite being referenced in Zeroclaw state. No CI enforces subid uniqueness.
- **Recommended direction:** Implement `op-oscal-registry` crate with a persistent registry (CozoDB or SQLite) of all subids. Add a proc-macro or build-script lint that requires every RPC method, D-Bus interface, and plugin to declare its subid. Make `LawFirm` validate that every schema has a corresponding OSCAL control mapping.
- **Blocks production readiness:** Yes. Compliance metadata is stored in code comments but not enforced.

### CF-6: GhostbridgeInterceptor Uses `unsafe` Mmap Pointer Dereference Without Concurrent-Access Safety
- **Severity:** High
- **Affected files/modules:** `crates/op-grpc-bridge/src/interceptor.rs`, `crates/op-identity/src/anna_scribe.rs`, `crates/op-identity/src/schema_bridge.rs`
- **Why it matters:** The interceptor reads `/dev/shm/plugin_schema.dat` via `memmap2::Mmap`, casts the pointer to `*const IdentitySled`, and dereferences it with `unsafe { &*sled_ptr }`. The `write_sled` function uses tmp-file + rename for atomicity, but there is no mechanism preventing a reader from holding the mmap across a rename. On Linux, the old inode remains valid, but if the file is truncated or the filesystem is tmpfs under memory pressure, the 152-byte read could observe partial writes or stale data. More critically, there is no versioning or sequence number in the sled, so a reader cannot detect a torn read.
- **Recommended direction:** Add a sequence number / CRC to `IdentitySled` and validate it before use. Or, better: use a `tokio::sync::RwLock<IdentitySled>` in a dedicated async task and replace the mmap with a message-passing read.
- **Blocks production readiness:** Yes. Unsafe pointer dereference on shared memory without synchronization is a memory safety and correctness risk.

### CF-7: Hardcoded IP Addresses, Ports, Tags, and OSCAL IDs Throughout the Stack
- **Severity:** High
- **Affected files/modules:** `crates/op-identity/src/schema_bridge.rs`, `crates/op-xray-daemon/src/dbus.rs`, `crates/op-grpc-bridge/src/grpc_server.rs`, `crates/op-cognitive-mcp/src/server.rs`, `crates/op-web/ui/src/hooks/use-event-stream.ts`
- **Why it matters:** Multiple hardcoded values: `127.0.0.1:8090`, `10.200.0.1`, `10.200.0.2:50052`, `127.0.0.1:11434`, `1080`, `12345`, `ovsbr0`, `gbr_wg`, `gbr_xray`, `wg-xray`. These appear in Rust source, generated Xray JSON strings, and UI hooks. There is no central configuration schema driving these. The Xray config is generated via `format!()` macro into a raw JSON string, making it impossible to validate against a schema.
- **Recommended direction:** Centralize all network endpoints in a `NetworkTopology` schema. Generate Xray config via `serde_json::Value` constructed from typed structs, not string formatting.
- **Blocks production readiness:** Partially. Hardcoded networking values prevent multi-environment deployment.

### CF-8: Unauthenticated gRPC Services with Permissive CORS
- **Severity:** High
- **Affected files/modules:** `crates/op-grpc-bridge/src/grpc_server.rs`, `crates/op-cognitive-mcp/src/server.rs`, `crates/op-assistant-grpc/src/server.rs`
- **Why it matters:** All gRPC servers configure `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)`. While the GhostbridgeInterceptor requires footprint/trace headers, gRPC-Web/browser clients from any origin are permitted. For services that do not have the interceptor (or if the interceptor is bypassed via direct service access), this is a significant security gap. There is no rate limiting, no token-based auth layer, and no mTLS configuration on gRPC listeners (tls feature is enabled in tonic but not configured).
- **Recommended direction:** Restrict CORS to known origins. Add a mandatory mTLS or JWT layer for external gRPC. Implement rate limiting via tower middleware.
- **Blocks production readiness:** Yes. Open CORS on control-plane APIs is a security risk.

### CF-9: Blocking I/O and Shell-Outs Inside Async Contexts
- **Severity:** High
- **Affected files/modules:** `crates/op-identity/src/wireguard.rs`, `crates/op-identity/src/schema_bridge.rs`, `crates/op-projection/src/plugin_reader.rs`
- **Why it matters:** `WireGuardIdentity` uses `Command::new("wg").output()` and `Command::new("ip").output()` in synchronous methods called from async contexts. `schema_bridge.rs` uses `Command::new("incus")` and `Command::new("wg")` in `watch_wireguard_handshakes`, which runs in a `std::thread::spawn`. However, `peer_source_port` is called from `write_sled_from_wg`, which is called synchronously and may block the async runtime if invoked from an async task. `SystemPluginReader::block_on` creates a new tokio runtime (`Builder::new_current_thread().build()`) inside a synchronous trait method, which is an anti-pattern that can panic if called within an existing runtime.
- **Recommended direction:** Replace all sync `Command` calls with `tokio::process::Command`. Replace `block_on` runtime creation with `tokio::task::spawn_blocking` or async trait methods.
- **Blocks production readiness:** Yes. Blocking I/O in async contexts causes runtime starvation and potential deadlocks.

### CF-10: Gemma Routing Intelligence Is Declared but Not Implemented
- **Severity:** High
- **Affected files/modules:** `crates/op-plugins/src/state_plugins/zeroclaw.rs`, `crates/op-cognitive-mcp/src/server.rs`
- **Why it matters:** The ZeroclawPlugin declares that "gemma4 is the universal router: it classifies EVERY request into a route/tag." There is no implementation of this classification. The `CognitiveGrpcService` has no routing intelligence layer. There are no logged decision objects with reasoning summaries, policy rules, confidence scores, or model versions. The system cannot explain routing decisions after the fact.
- **Recommended direction:** Implement a `GemmaRouter` behind a deterministic policy gate. Log every routing decision with: input schema, output routing class, confidence, reasoning summary, policy rule applied, model version, prompt/template ID, timestamp, and human override support.
- **Blocks production readiness:** Yes. Unaudited model-assisted routing is a compliance and security risk.

### CF-11: `anyhow` Used in Library APIs
- **Severity:** Medium
- **Affected files/modules:** `crates/op-grpc-bridge/src/grpc_server.rs` (internal_status), `crates/op-projection/src/schema_engine.rs`, `crates/op-dbus-model/src/lib.rs`
- **Why it matters:** The audit prompt specifically flags `anyhow used in library APIs` as a smell. `anyhow::Error` erases error types, making it impossible for callers to match on specific failures. The `grpc_server.rs` `internal_status` helper converts `anyhow::Error` into `tonic::Status::internal`, losing all error context. `SchemaEngine::register_schema` returns `anyhow::Result<u64>`. `SqlitePluginCatalog` methods return `anyhow`-based `ModelError` (which is good, using `thiserror`), but many other crates leak `anyhow`.
- **Recommended direction:** Replace `anyhow` in all library public APIs with `thiserror`-derived enums. Use `anyhow` only in binary entry points.
- **Blocks production readiness:** No, but it hinders debugging and reliability.

### CF-12: Protocol Adapters (Mail, MQTT, Netmaker) Lack Schema Binding and Audit Trail
- **Severity:** Medium
- **Affected files/modules:** `crates/op-grpc-adapters/proto/adapters.proto`, `crates/op-grpc-bridge/proto/operation.proto` (MailService, NetmakerService, MqService)
- **Why it matters:** The MailService, NetmakerService, and MqService in `op-grpc-adapters` define their own request/response types but do not carry schema IDs, OSCAL sub-IDs, identity metadata, or audit hashes. They appear to be standalone adapters without integration into the event chain. The audit prompt requires every protocol to be converted into a schema object with identity, routing, OSCAL, and audit metadata attached.
- **Recommended direction:** Extend the adapter proto definitions to include an `OperationMetadata` envelope (schema_id, subid, actor_id, trace_id, event_hash). Route all adapter calls through `SchemaEngine.mutate` to record them in the event chain.
- **Blocks production readiness:** Partially. These are side-channels outside the audit pipeline.

### CF-13: Missing Integration Tests for Identity Propagation, OSCAL Mapping, and Routing Tag Generation
- **Severity:** Medium
- **Affected files/modules:** Entire workspace test suites
- **Why it matters:** The test coverage observed is largely unit-test level: schema validation, path normalization, sled layout size checks, and xray status serialization. There are no end-to-end tests verifying that a WireGuard handshake results in a valid sled, that a gRPC mutation carries the correct trace_id through to the event chain, that an OSCAL subid is attached to every event, or that a routing tag derived from schema drives an Xray routing decision.
- **Recommended direction:** Add an `integration_test/` directory with tests for the full pipeline: WireGuard handshake -> sled write -> gRPC mutate -> event chain entry with trace_id -> D-Bus signal -> Qdrant vectorization -> UI render from schema.
- **Blocks production readiness:** Partially. Without integration tests, production failures are likely.

---

## 3. Architecture Map

### gRPC Layer
- **Entry Point:** `op-grpc-bridge::grpc_server::run_grpc_server` (port 8090), `op-cognitive-mcp::server::start_grpc_server` (port 50052), `op-assistant-grpc::server::serve` (port 50051).
- **Services:** StateSync, PluginService, EventChainService, OvsdbMirror, RuntimeMirror, ComponentRegistry, MailService, PrivacyNetworkService, RegistrationService, DbusPassthrough, CognitiveToolService, AgentService, SessionService, TaskService, ModelService, CronService, SoulService, NamespaceMemoryService, MemoryService.
- **Reflection:** Enabled via `tonic-reflection` with combined `FILE_DESCRIPTOR_SET`.
- **Health:** `tonic-health` reporters for all major services.
- **Web:** `tonic-web` with `accept_http1(true)` and permissive CORS.
- **Interceptor:** `GhostbridgeInterceptor` on primary bridge; `wireguard_auth_interceptor` on assistant-grpc.

### D-Bus Layer
- **Bus:** System bus (with session fallback for projection).
- **Canonical Paths:** `/org/opdbus/v1/plugins/{plugin_name}` (enforced in `op_plugins::canonical`).
- **Actual Paths:** Mix of canonical (`/org/opdbus/v1/plugins/...`), non-canonical (`/opdbus/v1/xray`), and projection-specific (`org.opdbus.projection` at `/org/opdbus/v1/plugins/.../...`).
- **Interfaces:** `org.opdbus.v1.Plugin.Plugins.{Name}`, `org.opdbus.projection.v1.Object`, `ai.assistant.v1`, `opdbus.v1.Xray`.

### Socket Layer
- **Unix Sockets:** `/run/qdrant.sock` (proxied via xray), `/run/netmaker/*.sock`, D-Bus system socket.
- **TCP Sockets:** gRPC (50051, 50052, 8090), Qdrant (6334 via xray proxy), Ollama (11434).
- **WireGuard:** `netmaker` interface (default), `wg0` in container.

### Schema Layer
- **Authority:** `PluginSchema` builder in `op-plugins/src/state_plugins/plugin_schema_defs.rs`.
- **Persistence:** `SqlitePluginCatalog` (deprecated per AGENTS.md, but still present).
- **Shared Memory:** `/dev/shm/live-schema.json` (canonical catalog).
- **Validation:** `SchemaEngine` / `SchemaValidator` in `op-projection`.

### OSCAL Layer
- **Taxonomy:** `SubidTaxonomy` / `SubidCategory` in `op-identity::schema_bridge`.
- **Validation:** `LawFirm` in `op-compliance` (stub).
- **Registry:** Referenced but not implemented (`oscal_subid_registry` plugin).

### Routing Layer
- **Xray:** Dokodemo-door inbounds (443, 1080, 12345), freedom outbounds with gRPC metadata injection.
- **Tags:** `reality-in`, `ovs-socks-in`, `ovs-tproxy-in`, `to-grpc-bridge`, `to-cognitive-mcp`, `to-{label}`.
- **Gemma:** Declared as classifier in ZeroclawPlugin, not implemented.
- **ZeroClaw:** Declared as router/enforcer, not implemented.

### Snowball/Vectorization Layer
- **Event Chain:** In-memory Merkle tree with Blake3 hashes in `op-state-store`.
- **Vector DB:** Qdrant with Voyage embeddings (`RagPipeline` in `op-cognitive-mcp`).
- **Graph DB:** CozoDB (relational-graph-vector) in `op-cognitive-mcp::CozoGraphShuttle`.
- **Snowball:** Referenced but no clear snowball write implementation found.

### UI Schema Rendering Layer
- **Framework:** React + Tailwind + shadcn/ui.
- **Schema Source:** Hardcoded `PLUGIN_DEFS` in `StatePage.tsx` (NOT the canonical schema).
- **Event Source:** gRPC-Web streams (`useEventStream` hook).
- **Rendering:** `SchemaRenderer` component with `inferSchema` fallback.

---

## 4. Pipeline Trace

| Step | Status | Schema-Bound | Identity Preserved | Routing Metadata | OSCAL Metadata | Audit Metadata | Replayable | UI Renderable | Vectorized | Gaps |
|------|--------|--------------|-------------------|------------------|----------------|----------------|------------|---------------|------------|------|
| 1. WireGuard login | Implemented | No (OS-level) | Yes (pubkey) | No | No | No (OS only) | No | No | No | No schema binding at OS layer |
| 2. WG identity validation | Implemented | Partial (sled) | Yes | No | No | No | No | No | No | Sled has no schema_version check |
| 3. Identity sled write | Implemented | Partial | Yes | No | No | No | No | No | No | Writes to /dev/shm only |
| 4. Session context attach | Partial | No | Yes (trace_id) | No | No | No | No | No | No | Session context is env var only |
| 5. D-Bus/gRPC msg attach | Partial | Partial | Yes (actor_id) | Partial (tags) | No | Partial (event_id) | Partial | Partial | No | OSCAL missing; schema_id missing |
| 6. Protocol adapter/plugin | Partial | Partial | No | No | No | No | No | No | No | Adapters bypass event chain |
| 7. Payload normalization | Partial | Yes (PluginSchema) | Partial | Partial | No | Partial | Yes | Yes | No | Normalization happens in plugin_reader |
| 8. OSCAL ID attach | Not Implemented | No | No | No | No | No | No | No | No | No OSCAL ID generation at this step |
| 9. Routing tags derived | Partial | No | No | Partial (Xray) | No | No | No | No | No | Tags are hardcoded in Xray JSON |
| 10. Gemma classification | Not Implemented | No | No | No | No | No | No | No | No | Gemma router is a declaration |
| 11. ZeroClaw enforcement | Not Implemented | No | No | No | No | No | No | No | No | ZeroClaw is a config plugin |
| 12. D-Bus/gRPC/socket route | Implemented | Partial | Partial | Partial | No | Partial | Yes | Yes | No | OSCAL missing |
| 13. Xray tag routing | Implemented | No | No | Partial | No | No | No | No | No | Xray config is raw JSON string |
| 14. Snowball/audit pipeline | Partial | Partial | Partial | Partial | No | Partial | Partial | Partial | No | Event chain exists; snowball unclear |
| 15. Evidence object | Partial | Partial | Partial | Partial | No | Partial | Partial | Partial | No | Evidence = ChainEvent proto |
| 16. Vectorization | Implemented | Yes | Partial | Partial | No | No | No | Yes | Yes | Qdrant upsert has payload metadata |
| 17. UI schema render | Partial | No | No | No | No | No | No | Yes | No | UI uses hardcoded schema, not canonical |

---

## 5. Source-of-Truth Analysis

| Artifact | Source of Truth | Status |
|----------|----------------|--------|
| Schema IDs | `op-plugins::state_plugins::plugin_schema_defs.rs` (PluginSchema builder) | **Exists** |
| Proto definitions | `crates/*/proto/*.proto` (multiple crates) | **Exists, but not generated from schema** |
| D-Bus paths | `op_plugins::canonical` (intended), but hardcoded paths exist | **Divergent** |
| Plugin paths | `op_plugins::canonical::plugin_path()` | **Exists, not universally used** |
| OSCAL IDs | `op-identity::schema_bridge::SubidTaxonomy` (parser only) | **No central registry** |
| OSCAL sub-IDs | Same as above | **No central registry** |
| Routing tags | Hardcoded in `schema_bridge.rs` Xray JSON template and ZeroclawPlugin state | **No schema binding** |
| UI schemas | Hardcoded in `StatePage.tsx PLUGIN_DEFS` | **Duplicated, not canonical** |
| Identity records | `/dev/shm/plugin_schema.dat` (IdentitySled) | **Exists** |
| Audit events | `op-state-store` event chain (in-memory Merkle tree) | **Exists** |

**Conclusion:** There is no single source of truth that generates Rust models, Protobuf, D-Bus models, UI forms, and OSCAL mappings from one canonical definition. The `PluginSchema` is the closest authority, but protos and UI are manually maintained in parallel.

---

## 6. Protocol Coverage Matrix

| Protocol | Entry Point | Schema Object | Identity Propagation | Routing Metadata | OSCAL Mapping | Audit Support | UI Rendering | Gaps |
|----------|-------------|---------------|---------------------|------------------|---------------|---------------|--------------|------|
| HTTP/HTTPS | `op-web` Axum server | None (REST is secondary) | Session cookie | None | No | No | React pages | Not schema-bound |
| gRPC | `op-grpc-bridge` (port 8090) | `MutateRequest`, `StateChange` | `actor_id`, `capability_id` | `plugin_id`, `tags_touched` | No | `event_id`, `event_hash` | gRPC-Web via `useEventStream` | OSCAL missing; no schema_id field |
| gRPC-Web | Same as gRPC (tonic-web) | Same | Same | Same | No | Same | React hooks | Same gaps |
| WebSocket | `op-web::websocket.rs` | Raw JSON | Session | None | No | No | Chat UI | No schema binding |
| Unix socket IPC | `/run/qdrant.sock`, `/run/netmaker/*.sock` | None | Unix perms | Xray tag | No | No | No | Not integrated into schema pipeline |
| D-Bus | System/session bus | `ProjectedObject` | D-Bus caller (unverified) | `entity_type`, `entity_id` | No | Signals emitted | No direct UI | No OSCAL; caller auth weak |
| SMTP/IMAP | `op-grpc-adapters::MailService` | `SendMessageRequest`, `MailHeader` | None | None | No | No | No | Completely outside pipeline |
| MQTT | `op-grpc-adapters::MqService` | `PublishRequest`, `MqMessage` | None | None | No | No | No | Completely outside pipeline |
| DNS | `dnsresolver.rs` plugin | Plugin state | None | None | No | No | No | Not exposed via gRPC |
| SOCKS5 | Xray `ovs-socks-in` (port 1080) | None | None | Xray inbound tag | No | No | No | Transparent proxy, no schema |
| Xray/VLESS/REALITY | Xray config (port 443) | None | UUID + Xray metadata | `reality-in` tag | No | No | No | No schema object for Xray flows |
| OVSDB | `OvsdbMirror` gRPC service | `OvsdbUpdate`, `OvsdbBridge` | `actor_id` (Transact only) | None | No | `event_id` (Transact only) | OVS page | Raw JSON passthrough for Transact |

---

## 7. gRPC Payload Matrix

| RPC / Service | Request Type | Response Type | Schema ID Support | Identity Support | Routing Tag Support | OSCAL Support | Audit Support | Problems |
|---------------|--------------|---------------|-------------------|------------------|---------------------|---------------|---------------|----------|
| StateSync.Subscribe | `SubscribeRequest` | `stream StateChange` | No | Partial (actor_id in change) | Yes (tags_touched) | No | Yes (event_id, event_hash) | No schema_id in request |
| StateSync.Mutate | `MutateRequest` | `MutateResponse` | No | Yes (actor_id, capability_id) | No | No | Yes (event_id, event_hash) | No schema_id; no subid |
| PluginService.GetSchema | `GetSchemaRequest` | `GetSchemaResponse` | Yes (plugin_id) | No | No | No | No | Returns raw JSON string, no metadata |
| EventChainService.GetEvents | `GetEventsRequest` | `GetEventsResponse` | No | Yes (actor_id) | Yes (tags) | No | Yes (full chain) | No OSCAL fields in ChainEvent |
| OvsdbMirror.Transact | `OvsdbTransactRequest` | `OvsdbTransactResponse` | No | Yes (actor_id) | No | No | Partial (event_id) | Raw JSON operations_json blob |
| DbusPassthrough.Call | `DbusCallRequest` | `dbusCallResponse` | No | No | No | No | No | Generic blob transport |
| PrivacyNetworkService.EnsurePrivacyNetwork | `EnsurePrivacyNetworkRequest` | `EnsurePrivacyNetworkResponse` | No | No | No | No | No | Hardcoded topology assumptions |
| ComponentRegistry.Register | `RegisterRequest` | `RegisterResponse` | Yes (schema_json) | No | Yes (capabilities) | No | No | No OSCAL mapping for components |
| CognitiveToolService.AskQuestion | `AskQuestionRequest` | `AskQuestionResponse` | No | No | No | No | No | No audit trail for queries |

---

## 8. D-Bus Contract Matrix

| Object Path | Interface | Methods | Signals | Properties | Schema Binding | gRPC Mapping | Problems |
|-------------|-----------|---------|---------|------------|--------------|--------------|----------|
| `/org/opdbus/v1/plugins/{plugin}` | `org.opdbus.v1.Plugin.Plugins.{Name}` | Varies per plugin | `updated` (ProjectedObject) | `entity_type`, `entity_id`, `state`, `data` | Yes (via projection) | Via `StateSync.Subscribe` | Paths correct per canonical module |
| `/org/opdbus/v1/plugins/{plugin}/{child}` | `org.opdbus.v1.Plugin.Plugins.{Name}` | - | `updated` | Same | Yes | Same | Child paths generated from nested objects |
| `/opdbus/v1/xray` | `opdbus.v1.Xray` | `start`, `stop`, `restart`, `status`, `reload`, `get_config` | None | None | No | No | **Non-canonical path** |
| `/org/opdbus/v1/cognitive` | `org.opdbus.CognitiveMcp` | `call_method` | None | None | No | CognitiveToolService | Bus name also non-canonical (`org.opdbus.CognitiveMcp`) |
| `/ai/assistant` (session bus) | `ai.assistant.v1` | `call` | `run_event` | None | No | Assistant gRPC services | Path not under `/org/opdbus/v1` |

---

## 9. Routing and Policy Analysis

| Dimension | Status |
|-----------|--------|
| Deterministic routing | **Partial.** Xray routing rules are deterministic (JSON field matching), but application-layer routing (which model/provider to use) is not enforced. |
| Model-assisted routing | **Not implemented.** Gemma is declared as a classifier but no classification code exists. |
| Auditable routing | **No.** Xray routing decisions are not logged. Application-layer routing decisions are not recorded. |
| Schema-backed routing | **No.** Routing tags in Xray config are hardcoded strings. There is no validation that a tag corresponds to a registered schema. |
| Explainable routing | **No.** No reasoning summary or policy rule log exists. |
| Overridable by deterministic rules | **N/A.** No model-assisted routing to override. |

**Combined Routing/Policy System Verdict:** The system has a transparent proxy (Xray) with static routing rules and a declared-but-unimplemented AI-assisted router (Gemma/ZeroClaw). The policy enforcement layer is absent. This is a **major architectural gap**.

---

## 10. Compliance and OSCAL Analysis

| Dimension | Status |
|-----------|--------|
| Canonical OSCAL ID/sub-ID system | **Partial taxonomy exists.** `SubidTaxonomy` parser is correct and follows the spec. |
| Enforceable | **No.** No CI gate or runtime check enforces subid presence on artifacts. |
| Renderable | **No.** No UI component renders OSCAL metadata. |
| Vectorizable | **No.** OSCAL mappings are not stored in Qdrant or CozoDB. |
| Crosswalk mappings | **No.** No mapping between NIST 800-53, FedRAMP, or CMMC controls and system artifacts. |

**Verdict:** The OSCAL sub-ID system is a **well-designed vocabulary without an enforcement mechanism**. It is not yet a compliance system.

---

## 11. UI Schema Rendering Analysis

| Dimension | Status |
|-----------|--------|
| Generated from schema | **No.** `StatePage.tsx` hardcodes `PLUGIN_DEFS` with inline JSON Schema fragments. |
| Manually duplicated | **Yes.** The UI re-declares fields for `net`, `dinit`, `wireguard`, `dbus`, and `system` that already exist in `plugin_schema_defs.rs`. |
| Schema renderer component | **Partial.** `SchemaRenderer` exists and can render from a schema object, but the parent page does not pass the canonical schema to it. It passes `plugin.schema ?? inferSchema(data)`, falling back to inference. |
| Validation logic | **Partial.** `SchemaRenderer` likely has validation, but the schema it receives is often inferred, not canonical. |
| Type-safe bindings | **No.** TypeScript types are manually defined or inferred; not generated from `PluginSchema`. |

**Verdict:** The UI rendering layer **violates the single source of truth**. The `PluginSchema` in Rust is the authority, but the UI does not consume it.

---

## 12. Recommended Refactor Plan

### Phase 1: Stabilize Source-of-Truth Schemas (Weeks 1-2)
1. **Schema Generation Pipeline:** Implement a build-time proc-macro or codegen tool that generates Protobuf messages, TypeScript interfaces, and D-Bus XML introspection from `PluginSchema` definitions.
2. **Remove UI Duplication:** Replace `PLUGIN_DEFS` in `StatePage.tsx` with a dynamic fetch of `PluginService.GetSchema` for each plugin. Cache schemas in a Zustand store.
3. **Meta-Schema CI Gate:** Add a CI step that verifies every plugin has a `schema()` method returning a valid `PluginSchema`, and that the schema name matches the plugin name.

### Phase 2: Normalize Identity Propagation (Weeks 2-3)
4. **Sled Synchronization:** Replace the `unsafe` mmap read in `GhostbridgeInterceptor` with a `tokio::sync::watch::Receiver<IdentitySled>` fed by a dedicated sled monitor task.
5. **Identity Envelope:** Add a mandatory `IdentityEnvelope` message to all gRPC requests containing `trace_id`, `actor_id`, `wireguard_pubkey_hash`, and `session_footprint`. Reject requests without it.

### Phase 3: Normalize gRPC Payload Metadata (Weeks 3-4)
6. **OperationMetadata:** Add an `OperationMetadata` field to every gRPC request/response in `operation.proto` carrying `schema_id`, `subid`, `trace_id`, `actor_id`, `capability_id`, `oscal_control_id`, and `event_hash`.
7. **Deprecate DbusPassthrough:** Mark `DbusPassthrough` as deprecated. For each service that currently relies on it, generate a dedicated proto from its `PluginSchema`.

### Phase 4: Normalize D-Bus Object Paths and Introspection (Week 4)
8. **Path Enforcement:** Add a `zbus` object server wrapper that rejects any object registration not under `/org/opdbus/v1/plugins/`.
9. **Fix Xray Path:** Move `op-xray-daemon` D-Bus object to `/org/opdbus/v1/plugins/xray` under bus name `org.opdbus.v1`.
10. **Introspection:** Auto-generate D-Bus XML introspection from `PluginSchema` and serve it via `org.freedesktop.DBus.Introspectable`.

### Phase 5: Bind Protocol Adapters to Schema (Weeks 5-6)
11. **Adapter Schema Binding:** Extend `adapters.proto` with `OperationMetadata`. Route all MailService, NetmakerService, and MqService calls through `SchemaEngine.mutate` to create event chain entries.
12. **Protocol Object Mapping:** Define `PluginSchema` equivalents for SMTP, IMAP, MQTT, and Netmaker protocols so their payloads are schema-bound.

### Phase 6: Bind Routing Tags to Schema (Week 6)
13. **Tag Registry:** Create a `RoutingTagRegistry` in `op-state-store` that maps tag strings to `PluginSchema` entries. Validate all Xray config tags against this registry before writing config.
14. **Typed Xray Config:** Replace the raw JSON string generation in `schema_bridge.rs` with a typed `XrayConfig` struct serialized via `serde_json`.

### Phase 7: Bind OSCAL IDs/Sub-IDs to Schema (Weeks 7-8)
15. **OSCAL Registry Crate:** Implement `op-oscal-registry` with a CozoDB-backed registry of all subids. Require every plugin, proto service, and D-Bus interface to register its subid at startup.
16. **Compliance Gate:** Upgrade `LawFirm` to query the OSCAL registry and validate that every mutation has a valid control mapping.

### Phase 8: Implement Gemma Routing Intelligence Behind Deterministic Policy (Weeks 8-9)
17. **Policy Engine:** Implement `ZeroClawPolicyEngine` with a rule-based first stage (e.g., "code queries -> openrouter/claude", "fast queries -> gemini/flash").
18. **Model-Assisted Stage:** Only if no deterministic rule matches, call the Gemma classifier. Log the full decision object with reasoning, confidence, and model version.
19. **Human Override:** Add a D-Bus method `OverrideRoutingDecision` for admins to correct model decisions.

### Phase 9: Implement ZeroClaw Enforcement with Fail-Closed Behavior (Week 9)
20. **Interceptor Chain:** Reorder gRPC interceptor chain to: `CORS` -> `RateLimit` -> `GhostbridgeIdentity` -> `ZeroClawPolicy` -> `Handler`.
21. **Fail-Closed:** Any error in `ZeroClawPolicy` (missing policy, rule evaluation failure, model timeout) returns `PERMISSION_DENIED`.

### Phase 10: Complete Snowball/Vectorization/Evidence Pipeline (Weeks 10-11)
22. **Snowball Writer:** Implement a `SnowballWriter` trait with a default Btrfs-backed append-only log. Attach the Merkle root of each event batch to the sled.
23. **Vectorize Events:** Upsert every `ChainEvent` into Qdrant with the full metadata payload so semantic search can find audit entries.

### Phase 11: Generate UI Schema Components from Canonical Schema (Week 11)
24. **Schema-Driven UI:** Delete `PLUGIN_DEFS`. Implement a `usePluginSchemas` hook that fetches and caches canonical schemas. Pass them to `SchemaRenderer`.
25. **Form Generation:** Extend `SchemaRenderer` to handle all `FieldType` variants (including `Enum`, `Array`, `Object`) with validation.

### Phase 12: Add Integration Tests for Full Pipeline (Week 12)
26. **E2E Test:** Write a test that: generates a WireGuard key -> triggers handshake -> verifies sled update -> sends gRPC mutate -> verifies event chain entry with trace_id -> verifies D-Bus signal -> verifies Qdrant upsert -> renders in UI from canonical schema.

---

## 13. Final Verdict

### Is this implementation production-ready?
**No.**

### Is the architecture sound but incomplete?
**Yes.** The architectural vision is sophisticated and largely correct: D-Bus control plane, schema-driven everything, WireGuard identity, shared-memory performance, event-chain audit, and vectorized memory. However, the enforcement layers, canonical generation pipelines, and integration testing are missing or incomplete.

### Is the architecture fragmented?
**Partially.** The fragmentation is most evident in three areas:
1. **D-Bus path divergence** (canonical vs. hardcoded paths).
2. **Schema duplication** (Rust `PluginSchema`, proto manual definitions, UI hardcoded definitions).
3. **Policy gap** (declared routing intelligence and enforcement vs. actual implementation).

### Top 5 Changes Required Before Serious Production Use:

1. **Implement ZeroClaw as a real policy enforcement layer** with deterministic rules, fail-closed behavior, and decision logging. Without this, the system has no authorization.

2. **Eliminate UI schema duplication** by making the React frontend consume `PluginService.GetSchema` dynamically. Without this, the system will suffer from frontend/backend drift on every schema change.

3. **Fix D-Bus path canonicalization** across all crates, especially `op-xray-daemon` and `op-projection`, and add CI gates to prevent regression. Without this, introspection and routing break.

4. **Add `OperationMetadata` to all gRPC payloads** carrying `schema_id`, `subid`, `trace_id`, `actor_id`, and `oscal_control_id`. Without this, the audit trail is incomplete and payloads are not self-describing.

5. **Remove the `unsafe` mmap dereference in `GhostbridgeInterceptor`** and replace it with an async-safe synchronization primitive (e.g., `tokio::sync::watch`). Without this, the primary security gate has a memory-safety risk.

---

*End of Audit Report*
