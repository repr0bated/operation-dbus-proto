# Requirements: NetMaker Custom json-render UI

## Overview

Custom `json-render` Schema UI renderer in OP-DBUS (`crates/op-web` / `crates/zeroclaw-gui`) that exposes the complete gRPC and socket networking surface area of the control plane. The renderer integrates with the sealed SHM blob catalog (`OPBLOB01`) to derive schemas, drive dynamic gRPC execution, visualize socket/network topology, and expand the `json-render` component catalog with networking-specific DSL components.

---

## REQ-1: SHM Blob Schema & Proto Extraction

### Functional Requirements

- **REQ-1.1**: Implement a parser/reader for `OPBLOB01` binary blobs located at `/dev/shm/opdbus/plugin-blobs/`.
- **REQ-1.2**: Extract Section 1 (`PluginSchema` JSON) — the canonical schema derived via `schemars` that defines state structs and method signatures.
- **REQ-1.3**: Extract Section 3 (`FileDescriptorSet`) — the protobuf reflection descriptor payload synthesized from `PluginSchema.methods`.
- **REQ-1.4**: Parse `BlobManifest` (Section 2) for plugin identity, version, and dependency metadata.
- **REQ-1.5**: Detect catalog changes via `catalog_hash` + `generation` fields in `.manifest.json` to trigger UI schema refresh without polling.

### Non-Functional Requirements

- **REQ-1.6**: Blob reads MUST be zero-copy or memory-mapped where possible — blobs are sealed and immutable once written.
- **REQ-1.7**: SHA256 integrity check of Section 1 against the 64-byte header MUST pass before any schema is accepted.
- **REQ-1.8**: Graceful degradation when blob catalog is re-indexing (generation mismatch) — display stale schema with a "refreshing" indicator.

---

## REQ-2: gRPC Reflection & Execution Surface

### Functional Requirements

- **REQ-2.1**: Provide interactive dynamic gRPC method invocation widgets that allow users to:
  - Browse available services/methods via reflection tree.
  - Construct request payloads from schema-derived forms.
  - Execute unary RPCs and display responses inline.
- **REQ-2.2**: Implement streaming gRPC response visualizers:
  - Server-streaming: live message log with timestamps.
  - Client-streaming: batch message composer with send controls.
  - Bidirectional: combined send/receive panes.
- **REQ-2.3**: gRPC reflection tree navigation using `tonic-reflection` v1 and v1alpha descriptors exposed by `PerMethodGrpcServices`.
- **REQ-2.4**: Support connections to all configured gRPC endpoints:
  - Native Unix Socket: `/run/ghostbridge/grpc.sock` (owned by `op-grpc-bridge`).
  - gRPC-Web TCP backend: `127.0.0.1:8090`.
  - External Xray listener/proxy: `188.68.58.237:8090` / `127.0.0.1:10809`.
- **REQ-2.5**: Dynamic method resolution from `FileDescriptorSet` (Section 3 of blob) — no compile-time codegen dependency for UI rendering.

### Non-Functional Requirements

- **REQ-2.6**: RPC execution latency visualization (round-trip time indicator per call).
- **REQ-2.7**: Request/response payload serialization MUST support both JSON and binary protobuf views.
- **REQ-2.8**: Connection status indicators per endpoint (connected/disconnected/error) with automatic reconnection.

---

## REQ-3: Socket & Network Topology Surface

### Functional Requirements

- **REQ-3.1**: Live telemetry widgets for Unix domain socket availability:
  - `/run/ghostbridge/grpc.sock`
  - `/run/opdbus/session-bus.sock`
  - `/run/openvswitch/db.sock`
  - `/run/rust-network-manager/rust-network-manager.sock`
- **REQ-3.2**: TCP port health indicators for container listeners:
  - Netmaker API (`8081`)
  - EMQX MQTT (`1883`), WebSocket (`8083`), MQTT/TLS (`8883`)
  - Rust Network Manager health (`9100`)
  - Proxy (`3128`)
- **REQ-3.3**: OVS bridge (`ovsbr0`) and physical adapter (`eth0`) state telemetry visualization.
- **REQ-3.4**: Xray routing state display (active routes, traffic stats, connection status).
- **REQ-3.5**: WireGuard client status (peer connections, handshake recency, transfer stats).
- **REQ-3.6**: Network topology graph rendering showing relationships between OVS bridge, physical interfaces, containers, and external endpoints.

### Non-Functional Requirements

- **REQ-3.7**: Socket availability checks MUST NOT block the UI render loop — use async polling with configurable intervals.
- **REQ-3.8**: Stale telemetry MUST be visually distinguished (greyed/timestamped) when data exceeds configured freshness threshold.
- **REQ-3.9**: Topology graph MUST update incrementally (no full re-render on single-node state change).

---

## REQ-4: json-render Component Catalog Expansion

### Functional Requirements

- **REQ-4.1**: Define formal `json-render` UI DSL schemas for the following networking components:
  - `GrpcMethodCaller` — form-based unary RPC invocation with schema-derived inputs.
  - `GrpcStreamViewer` — real-time streaming message display with scroll/pause controls.
  - `ReflectionTreeExplorer` — hierarchical service/method/message browser.
  - `SocketStatusPill` — compact availability indicator for Unix domain sockets.
  - `TcpHealthBadge` — port reachability indicator with latency annotation.
  - `NetworkTopologyGraph` — interactive graph layout of network infrastructure.
  - `SchemaFormBuilder` — auto-generated form from `schemars` JSON Schema (request message construction).
- **REQ-4.2**: All components MUST adhere to the existing catalog structure (`root`, `data`, `actions`) — no non-catalog props or actions.
- **REQ-4.3**: Components MUST be composable — e.g., `ReflectionTreeExplorer` selection feeds `GrpcMethodCaller` which feeds `GrpcStreamViewer`.
- **REQ-4.4**: Each component MUST declare its JSON Schema for validation by the `json-render` interpreter pipeline.

### Non-Functional Requirements

- **REQ-4.5**: Component rendering MUST be deterministic given the same input schema — no side effects during render phase.
- **REQ-4.6**: New components MUST NOT break existing `json-render` catalog consumers — backward compatibility is mandatory.
- **REQ-4.7**: Component schemas MUST include `x-oscal-subid` annotations for compliance tracking.

---

## REQ-5: Action Dispatching & Transport Layer

### Functional Requirements

- **REQ-5.1**: Implement `json-render` action type handlers for:
  - `grpc.call` — execute a unary RPC given service, method, and payload.
  - `grpc.stream_subscribe` — open a server-streaming RPC and pipe messages to the bound viewer.
  - `socket.check_health` — probe a Unix domain or TCP socket and report status.
  - `schema.mutate` — apply a validated schema mutation (e.g., update form state from server response).
- **REQ-5.2**: Action dispatching MUST route through the existing `json-render` action bus — no out-of-band side channels.
- **REQ-5.3**: Transport layer MUST abstract over Unix socket vs TCP vs gRPC-Web — the component DSL specifies the logical target, not the physical transport.
- **REQ-5.4**: Actions MUST carry correlation IDs for request/response matching in async and streaming contexts.

### Non-Functional Requirements

- **REQ-5.5**: Action handlers MUST validate payloads against the declared schema before dispatch — reject malformed actions with structured error responses.
- **REQ-5.6**: Transport failures MUST surface as first-class action error results (not silent drops).
- **REQ-5.7**: All action dispatches MUST be auditable — emit structured logs with correlation ID, timestamp, target, and result status.

---

## Policy Constraints (Cross-Cutting)

- **PC-1**: Host services MUST be managed via `sudo sv <command> <service>` (runit PID 1). No `systemctl`, `s6-rc`, or foreign service manager CLIs.
- **PC-2**: Container application lifecycle MUST use D-Bus via `busctl` for service-manager operations.
- **PC-3**: Xray live configuration MUST exist ONLY at `/etc/xray/xray_config.json` inside the container.
- **PC-4**: `json-render` output format MUST adhere to catalog components (`root`, `data`, `actions`). Never invent non-catalog props or actions.
- **PC-5**: `OPBLOB01` sealed blob contract is immutable — readers MUST NOT write to blob files.

---

## Acceptance Criteria

1. All `OPBLOB01` sections (1–3) are parseable and their schemas drive UI rendering without compile-time codegen.
2. gRPC dynamic execution works across all three endpoint types (Unix socket, TCP, gRPC-Web proxy).
3. Socket and network telemetry widgets display live status with graceful staleness handling.
4. All new `json-render` components have formal JSON Schema declarations and pass catalog validation.
5. Action dispatching is end-to-end functional for `grpc.call`, `grpc.stream_subscribe`, `socket.check_health`, and `schema.mutate`.
6. No policy constraint violations in the implementation (runit, D-Bus, Xray path, catalog format).
