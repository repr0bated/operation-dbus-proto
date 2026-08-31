# Spec: NetMaker Custom json-render UI

> **Canonical MCP/gRPC ingress:** the only authenticated network ingress for gRPC/MCP
> is `op-grpc-bridge` on TLS `:8090` — see
> `.kiro/specs/unified-authenticated-mcp-cognitive-control-plane/`. This UI drives gRPC
> against that ingress (TLS `:8090`, or the host/container UDS). It MUST NOT target
> `:3003`, `:50051`, `:50052`, `:11438`, or any standalone MCP daemon; those are
> retired. Where this doc lists other endpoints, treat `:8090` (+ UDS) as authoritative.

## Summary

Extend the `zeroclaw-gui` json-render.dev interpreter and `op-web` backend with 7 new networking-specific DSL component kinds, a typed action bus for gRPC/socket operations, and a network telemetry subsystem. This enables operator-console visualization of the full gRPC and socket networking surface area exposed by OP-DBUS's control plane.

## Scope

| Area | In Scope | Out of Scope |
|---|---|---|
| json-render DSL | 7 new component kinds, JSON Schema declarations | Modifying existing stable-core components |
| gRPC execution | Dynamic unary + streaming via prost-reflect | Compile-time codegen, proto file generation |
| Socket telemetry | Unix domain + TCP health polling | Raw packet capture, pcap integration |
| Network topology | Force-directed graph of OVS/WG/containers | Full SDN controller, flow programming |
| Action dispatching | 4 action types via ActionBus | Direct D-Bus mutation from UI (goes through plugins) |
| op-web relay | WebSocket proxy for gRPC streams | Full gRPC-Web protocol implementation |
| Blob integration | Read-only access to OPBLOB01 sections 1-3 | Writing blobs, modifying catalog |

## Target Crates

| Crate | Changes |
|---|---|
| `zeroclaw-gui` | Action bus, 7 component kinds in interpret.rs, telemetry poller, reflection registry extension, connection pool, component JSON schemas, network.json page |
| `op-web` | WebSocket gRPC stream relay handler + route |
| `op-blob` | None (consumed as dependency, no modifications) |
| `op-grpc-bridge` | None (consumed via gRPC channel, no modifications) |

## New Component Catalog

| Component Kind | Category | Purpose |
|---|---|---|
| `grpc_method_caller` | gRPC Execution | Form-based unary RPC invocation with schema-derived inputs |
| `grpc_stream_viewer` | gRPC Execution | Real-time streaming message display with scroll/pause |
| `reflection_tree_explorer` | gRPC Navigation | Hierarchical service/method/message browser from blob descriptors |
| `socket_status_pill` | Network Status | Compact Unix socket availability indicator |
| `tcp_health_badge` | Network Status | TCP port reachability indicator with latency |
| `network_topology_graph` | Network Topology | Interactive force-directed graph of infrastructure |
| `schema_form_builder` | Schema Utilities | Auto-generated form from schemars JSON Schema |

## New Action Types

| Action Type | Handler | Transport |
|---|---|---|
| `grpc.call` | GrpcCallHandler | Unix socket / TCP / gRPC-Web (via ConnectionPool) |
| `grpc.stream_subscribe` | GrpcStreamHandler | Unix socket / TCP (native), WebSocket relay (web) |
| `socket.check_health` | SocketHealthHandler | Direct async connect probe |
| `schema.mutate` | SchemaMutateHandler | In-process state update (no I/O) |

## Key Architectural Constraints

1. **Interpreter is the ONLY renderer** — all new components are match arms in `walk()` inside `interpret.rs`. No plugin system, no WASM, no external renderers.
2. **Render phase is pure** — no I/O, no network calls during `walk()`. All side effects happen via ActionBus dispatch triggered by user interaction (button clicks).
3. **Catalog format compliance** — every component spec uses `root`, `data`, `actions` structure. No non-catalog props invented.
4. **Zero-copy blob reads** — `BlobRef` hands out borrowed slices. The GUI never writes to `/dev/shm/opdbus/plugin-blobs/`.
5. **runit for host services** — any service restart triggered from telemetry or actions uses `sudo sv restart <service>`. No systemctl.
6. **D-Bus for containers** — container lifecycle via `busctl`. No direct container management CLIs.
7. **Xray path** — Xray config is read-only from the GUI's perspective. Path is always `/etc/xray/xray_config.json` inside the container.

## Implementation Order (15 tasks)

**Phase A — Infrastructure (Tasks 1-2)**: Action bus core + connection pool. These are dependencies for all action handlers.

**Phase B — Action Handlers (Tasks 3-4)**: gRPC call + stream handlers. These enable the gRPC execution components.

**Phase C — Data Layer (Tasks 5-6)**: Reflection registry blob integration + network telemetry poller. These provide data for all components.

**Phase D — Components (Tasks 7-12)**: All 7 DSL component kinds implemented as interpret.rs match arms.

**Phase E — Web Support (Task 13)**: op-web WebSocket relay for browser-based stream consumption.

**Phase F — Composition & Validation (Tasks 14-15)**: Static page wiring all components together + integration tests.

## Acceptance Criteria

1. ✅ All `OPBLOB01` sections (1–3) parseable; schemas drive UI rendering without compile-time codegen
2. ✅ gRPC dynamic execution works across Unix socket, TCP, and gRPC-Web proxy endpoints
3. ✅ Socket and network telemetry widgets show live status with graceful staleness handling
4. ✅ All 7 new components have formal JSON Schema declarations validated at catalog admission
5. ✅ Action dispatching end-to-end for all 4 action types with correlation IDs and audit logging
6. ✅ Zero policy violations (runit, D-Bus, Xray path, catalog format, blob immutability)
7. ✅ `cargo test -p zeroclaw-gui -p op-web` green; `cargo clippy` clean on new code
8. ✅ `network.json` static page renders without error, composing multiple networking components

## Commit Artifacts

Upon completion, the following are committed to the repository:

```
crates/zeroclaw-gui/
├── src/actions/mod.rs           # ActionBus, traits, types
├── src/actions/grpc_call.rs     # grpc.call handler
├── src/actions/grpc_stream.rs   # grpc.stream_subscribe handler
├── src/actions/socket_health.rs # socket.check_health handler
├── src/actions/schema_mutate.rs # schema.mutate handler
├── src/telemetry.rs             # NetworkTelemetry poller
├── src/catalog/interpret.rs     # +7 match arms (modified)
├── src/conn.rs                  # ConnectionPool (modified)
├── src/grpc.rs                  # ReflectionRegistry blob_pools (modified)
├── schemas/*.json               # 7 component JSON Schemas
├── pages/network.json           # Composite networking page
├── tests/action_bus_integration.rs
└── tests/component_render_integration.rs

crates/op-web/src/
├── handlers/grpc_stream_relay.rs  # WebSocket gRPC relay
└── routes/mod.rs                  # Route mount (modified)
```

## API Contracts

### Action Request Format (internal)
```json
{
  "correlation_id": "uuid-v4",
  "action_type": "grpc.call",
  "payload": {
    "service": "opdbus.plugins.zeroclaw.v1.GetState",
    "method": "GetState",
    "payload": {},
    "endpoint": "auto"
  },
  "timestamp": "2026-08-04T12:00:00Z"
}
```

### Action Result Format (internal)
```json
{
  "correlation_id": "uuid-v4",
  "status": "success",
  "payload": { "state": { ... } },
  "latency_ms": 12
}
```

### WebSocket Stream Protocol (op-web)
```json
// Client → Server
{ "action": "subscribe", "service": "...", "method": "...", "payload": {}, "endpoint": "auto" }
{ "action": "cancel" }

// Server → Client
{ "frame": 1, "data": { ... }, "timestamp": "2026-08-04T12:00:00.123Z" }
{ "status": "complete" }
{ "status": "error", "message": "connection refused" }
```

## Related Specs

- `#[[file:.kiro/specs/schemars-to-reflection-plugin-pipeline/design.md]]` — Defines the PluginSchema → blob → reflection pipeline this spec reads from
- `#[[file:.kiro/specs/unified-authenticated-mcp-cognitive-control-plane/design.md]]` — Canonical MCP/blob-catalog/vectors architecture and the single `:8090` ingress this UI drives (supersedes the former `unified-blob-catalog-mcp` spec, now consolidated there)
