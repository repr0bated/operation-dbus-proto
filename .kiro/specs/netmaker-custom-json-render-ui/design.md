# Design: NetMaker Custom json-render UI

## Overview

This design extends the `zeroclaw-gui` json-render.dev interpreter pipeline and `op-web` backend to expose gRPC execution, socket telemetry, and network topology through the existing catalog-driven DSL framework. All new components integrate with the sealed `OPBLOB01` blob catalog and route actions through the established `json-render` action bus.

---

## Architectural Decisions

### D1 — New DSL kinds live in `interpret.rs`, not a plugin system

The json-render interpreter (`zeroclaw-gui/src/catalog/interpret.rs`) is the ONLY code path allowed to draw widgets. Adding new networking component kinds means adding match arms to the `walk()` function. There is no plugin/extension system for renderers — this is deliberate (prevents gemma from inventing arbitrary render paths). Each new component kind documented here becomes a new match arm.

### D2 — Action dispatching uses an `ActionBus` trait object, not direct transport calls

Components emit action requests (e.g., `grpc.call`) into a typed `ActionBus`. The bus routes to registered handlers based on the action type prefix. This keeps the interpreter pure (no I/O during render) and allows transport abstraction (Unix socket vs TCP vs gRPC-Web resolved at the handler level, not the component level).

### D3 — Blob schema extraction reuses `op-blob` crate directly

`zeroclaw-gui` already depends on tonic/prost/serde_json. Adding `op-blob` as a dependency gives zero-copy `BlobRef` access to schemas and `FileDescriptorSet` bytes. No new extraction code needed — `op_blob::catalog::read_plugin_schema_shm()` and `BlobRef::new()` are the read paths.

### D4 — gRPC dynamic execution uses `prost-reflect` DynamicMessage, not codegen

Runtime gRPC invocation from the UI uses `prost-reflect::DynamicMessage` constructed from the `FileDescriptorSet` (blob section 3). Request fields are populated from `SchemaFormBuilder` output. This matches REQ-2.5 (no compile-time codegen dependency).

### D5 — Network telemetry is polled via the existing `PageSource` mechanism

Socket/TCP health checks and OVS/WireGuard state already fit the `static_pages.rs` `PageSource` pattern: a named plugin method polled at intervals. The new telemetry components bind to data populated by these polls. No new polling mechanism — extend the existing one with network-specific methods.

### D6 — Topology graph uses egui's painter API with a force-directed layout

`NetworkTopologyGraph` renders using `egui::Painter` for node/edge drawing. Layout is computed incrementally (force-directed with cached positions). No external graph library — the topology is small (< 50 nodes) and the egui painter is sufficient.

### D7 — `op-web` provides a WebSocket relay for streaming gRPC responses

For the web-based path (non-native GUI), `op-web` exposes a WebSocket endpoint that proxies gRPC server-streaming responses. The native `zeroclaw-gui` connects directly via tonic. Both paths emit the same JSON frame format so `GrpcStreamViewer` is transport-agnostic.

### D8 — Component schemas are declared in a `schemas/` directory as JSON files

Each new component's formal JSON Schema lives in `zeroclaw-gui/schemas/<component_kind>.json`. The interpreter validates incoming specs against these schemas at admission time (when `CatalogStore::admit()` is called), not at render time. This keeps render hot-path allocation-free.

---

## Component Architecture

### Layer 0 — Blob Schema Reader (existing, reused)

```
/dev/shm/opdbus/plugin-blobs/*.blob
    │
    ├── op_blob::catalog::read_manifest_plugin_ids_shm()  → Vec<plugin_id>
    ├── op_blob::catalog::read_plugin_schema_shm(id)      → PluginSchema
    ├── op_blob::BlobRef::new(bytes)                      → zero-copy sections
    │       .schema_json()        → &str (Section 1)
    │       .manifest()           → BlobManifest (Section 2)
    │       .descriptor_set()     → &[u8] (Section 3 - FileDescriptorSet)
    └── .manifest.json            → { catalog_hash, generation, plugins }
```

The GUI reads `catalog_hash` + `generation` on a 1-second timer. When generation changes, it reloads affected plugin schemas and rebuilds reflection trees.

### Layer 1 — Reflection Registry (extended)

File: `zeroclaw-gui/src/grpc.rs` (existing `ReflectionRegistry`)

Extend to hold `prost_reflect::DescriptorPool` built from blob section 3:

```rust
pub struct ReflectionRegistry {
    /// Existing: descriptor pool from the gRPC reflection service.
    pool: prost_reflect::DescriptorPool,
    /// New: per-plugin descriptor pools built from sealed blob FileDescriptorSets.
    blob_pools: HashMap<String, prost_reflect::DescriptorPool>,
    /// Generation tracker for change detection.
    last_generation: u64,
}

impl ReflectionRegistry {
    /// Rebuild blob_pools from SHM catalog when generation changes.
    pub fn refresh_from_blobs(&mut self) -> Result<bool> {
        let manifest = read_manifest(DEFAULT_SHM_DIR)?;
        if manifest.generation == self.last_generation {
            return Ok(false);
        }
        self.blob_pools.clear();
        for (plugin_id, _hash) in &manifest.plugins {
            if let Some(bytes) = read_blob_descriptor_set(DEFAULT_SHM_DIR, plugin_id) {
                let pool = DescriptorPool::decode(bytes.as_slice())?;
                self.blob_pools.insert(plugin_id.clone(), pool);
            }
        }
        self.last_generation = manifest.generation;
        Ok(true)
    }

    /// List all services across all blob pools.
    pub fn all_services(&self) -> Vec<ServiceDescriptor> { ... }

    /// Resolve a method by fully-qualified path.
    pub fn resolve_method(&self, path: &str) -> Option<MethodDescriptor> { ... }
}
```

### Layer 2 — Action Bus

New file: `zeroclaw-gui/src/actions.rs`

```rust
/// Action request emitted by a component during interaction (NOT during render).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub correlation_id: Uuid,
    pub action_type: String,       // "grpc.call", "grpc.stream_subscribe", "socket.check_health", "schema.mutate"
    pub payload: Value,            // validated against handler's input schema
    pub timestamp: SystemTime,
}

/// Action result delivered back to the originating component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub correlation_id: Uuid,
    pub status: ActionStatus,
    pub payload: Value,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionStatus {
    Success,
    Error { code: String, message: String },
    Streaming { stream_id: Uuid },
}

/// Trait for action handlers. Each handler owns one action type prefix.
pub trait ActionHandler: Send + Sync {
    fn action_prefix(&self) -> &str;
    fn input_schema(&self) -> &Value;
    fn dispatch(&self, request: ActionRequest) -> ActionResult;
}

/// Central bus that routes ActionRequests to registered handlers.
pub struct ActionBus {
    handlers: HashMap<String, Box<dyn ActionHandler>>,
    pending: VecDeque<(ActionRequest, oneshot::Sender<ActionResult>)>,
    audit_log: Vec<AuditEntry>,
}
```

### Layer 3 — Action Handlers

#### `grpc.call` Handler

File: `zeroclaw-gui/src/actions/grpc_call.rs`

```rust
pub struct GrpcCallHandler {
    registry: Arc<RwLock<ReflectionRegistry>>,
    connections: ConnectionPool,  // manages Unix/TCP/gRPC-Web channels
}

impl ActionHandler for GrpcCallHandler {
    fn action_prefix(&self) -> &str { "grpc.call" }

    fn dispatch(&self, req: ActionRequest) -> ActionResult {
        // 1. Extract service, method, payload from req.payload
        // 2. Resolve method descriptor from registry
        // 3. Build DynamicMessage from payload + method.input_type()
        // 4. Select transport from connection pool (Unix > TCP > gRPC-Web)
        // 5. Execute unary call via tonic::transport::Channel
        // 6. Decode response DynamicMessage → JSON
        // 7. Return ActionResult with latency
    }
}
```

#### `grpc.stream_subscribe` Handler

File: `zeroclaw-gui/src/actions/grpc_stream.rs`

```rust
pub struct GrpcStreamHandler {
    registry: Arc<RwLock<ReflectionRegistry>>,
    connections: ConnectionPool,
    active_streams: Arc<RwLock<HashMap<Uuid, StreamHandle>>>,
}

// Opens server-streaming RPC, returns stream_id.
// Subsequent frames arrive via StreamHandle → GrpcStreamViewer binding.
```

#### `socket.check_health` Handler

File: `zeroclaw-gui/src/actions/socket_health.rs`

```rust
pub struct SocketHealthHandler;

impl ActionHandler for SocketHealthHandler {
    fn action_prefix(&self) -> &str { "socket.check_health" }

    fn dispatch(&self, req: ActionRequest) -> ActionResult {
        // Extract target: { "type": "unix"|"tcp", "path"|"host": ..., "port": ... }
        // For Unix: tokio::net::UnixStream::connect(path).await with timeout
        // For TCP: tokio::net::TcpStream::connect((host, port)).await with timeout
        // Return: { "reachable": bool, "latency_ms": u64, "error": Option<String> }
    }
}
```

#### `schema.mutate` Handler

File: `zeroclaw-gui/src/actions/schema_mutate.rs`

```rust
pub struct SchemaMutateHandler {
    form_state: Arc<RwLock<HashMap<String, Value>>>,
}

// Applies validated mutations to form state (e.g., populating response fields
// back into a SchemaFormBuilder's data binding). Pure state update, no I/O.
```

### Layer 4 — New DSL Component Kinds

Added to `zeroclaw-gui/src/catalog/interpret.rs` as new match arms in `walk()`:

#### `grpc_method_caller`

```json
{
  "kind": "grpc_method_caller",
  "service": "opdbus.plugins.zeroclaw.v1.GetState",
  "method": "GetState",
  "endpoint": "/run/ghostbridge/grpc.sock",
  "schema_bind": "/request_schema",
  "response_bind": "/response"
}
```

Renders:
- Method name heading
- `SchemaFormBuilder` for request fields (derived from method input descriptor)
- "Execute" button → emits `grpc.call` action
- Response display area (JSON tree or formatted)
- Latency badge

#### `grpc_stream_viewer`

```json
{
  "kind": "grpc_stream_viewer",
  "stream_id_bind": "/active_stream",
  "max_messages": 100,
  "auto_scroll": true
}
```

Renders:
- Message list with timestamps (scrollable, pausable)
- Message count indicator
- Pause/Resume toggle
- Clear button

#### `reflection_tree_explorer`

```json
{
  "kind": "reflection_tree_explorer",
  "source": "blob_catalog",
  "selected_bind": "/selected_method",
  "filter": ""
}
```

Renders:
- Collapsible tree: Package → Service → Method
- Search/filter input
- Selection emits to `selected_bind` (consumed by `grpc_method_caller`)
- Badge per method: unary/server-stream/client-stream/bidi

#### `socket_status_pill`

```json
{
  "kind": "socket_status_pill",
  "target": { "type": "unix", "path": "/run/ghostbridge/grpc.sock" },
  "label": "gRPC Bridge",
  "poll_secs": 5
}
```

Renders:
- Compact pill: colored dot + label + latency
- Colors: green (< 10ms), yellow (< 100ms), red (unreachable)
- Tooltip: last check time, full path, error details

#### `tcp_health_badge`

```json
{
  "kind": "tcp_health_badge",
  "target": { "type": "tcp", "host": "127.0.0.1", "port": 8081 },
  "label": "Netmaker API",
  "poll_secs": 10
}
```

Renders:
- Badge with port number, service label, and status indicator
- Latency annotation (ms)
- Connection state history sparkline (last 60 checks)

#### `network_topology_graph`

```json
{
  "kind": "network_topology_graph",
  "nodes_bind": "/topology/nodes",
  "edges_bind": "/topology/edges",
  "layout": "force_directed",
  "interactive": true
}
```

Renders:
- Force-directed graph using egui Painter
- Node types: bridge, interface, container, endpoint (different shapes/colors)
- Edges: physical (solid), virtual (dashed), tunnel (dotted)
- Click node → detail panel (source for `socket_status_pill` / `tcp_health_badge`)
- Pan/zoom support

#### `schema_form_builder`

```json
{
  "kind": "schema_form_builder",
  "schema_bind": "/method_input_schema",
  "value_bind": "/form_state",
  "submit_action": "grpc.call"
}
```

Renders:
- Auto-generated form fields from JSON Schema:
  - `string` → text input
  - `integer`/`number` → numeric input with min/max
  - `boolean` → checkbox
  - `array` → repeatable field group with add/remove
  - `object` → nested fieldset
  - `enum` → dropdown select
- Validation indicators per field (from schema constraints)
- Submit button → emits configured action with form values as payload

### Layer 5 — Connection Pool

File: `zeroclaw-gui/src/conn.rs` (extend existing)

```rust
pub struct ConnectionPool {
    /// Unix socket channels (preferred for local).
    unix: HashMap<PathBuf, tonic::transport::Channel>,
    /// TCP channels.
    tcp: HashMap<SocketAddr, tonic::transport::Channel>,
    /// gRPC-Web channels (for remote/proxy).
    grpc_web: HashMap<String, tonic_web::GrpcWebClientLayer>,
}

impl ConnectionPool {
    /// Connect with fallback priority: Unix > TCP > gRPC-Web.
    pub async fn get_channel(&self, endpoint: &EndpointSpec) -> Result<Channel> {
        match endpoint {
            EndpointSpec::Unix(path) => self.get_or_connect_unix(path).await,
            EndpointSpec::Tcp(addr) => self.get_or_connect_tcp(addr).await,
            EndpointSpec::GrpcWeb(url) => self.get_or_connect_web(url).await,
            EndpointSpec::Auto => {
                // Try /run/ghostbridge/grpc.sock first, then 127.0.0.1:8090
                self.get_or_connect_unix(Path::new("/run/ghostbridge/grpc.sock"))
                    .await
                    .or_else(|_| self.get_or_connect_tcp("127.0.0.1:8090".parse().unwrap()).await)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EndpointSpec {
    #[serde(rename = "unix")]
    Unix { path: PathBuf },
    #[serde(rename = "tcp")]
    Tcp { host: String, port: u16 },
    #[serde(rename = "grpc_web")]
    GrpcWeb { url: String },
    #[serde(rename = "auto")]
    Auto,
}
```

### Layer 6 — Network Telemetry Data Provider

File: `zeroclaw-gui/src/telemetry.rs`

```rust
/// Async telemetry poller that populates data bindings for network components.
pub struct NetworkTelemetry {
    /// Socket health results keyed by target identifier.
    pub socket_health: Arc<RwLock<HashMap<String, SocketHealthResult>>>,
    /// TCP health results.
    pub tcp_health: Arc<RwLock<HashMap<String, TcpHealthResult>>>,
    /// OVS bridge state.
    pub ovs_state: Arc<RwLock<Option<OvsState>>>,
    /// WireGuard state.
    pub wg_state: Arc<RwLock<Option<WgState>>>,
    /// Topology graph data.
    pub topology: Arc<RwLock<TopologyData>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketHealthResult {
    pub target: String,
    pub reachable: bool,
    pub latency_ms: Option<u64>,
    pub last_check: SystemTime,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyData {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyNode {
    pub id: String,
    pub label: String,
    pub node_type: NodeType,  // Bridge, Interface, Container, Endpoint
    pub status: String,       // "up", "down", "degraded"
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyEdge {
    pub source: String,
    pub target: String,
    pub edge_type: EdgeType,  // Physical, Virtual, Tunnel
    pub label: Option<String>,
}
```

### Layer 7 — op-web WebSocket Relay (for web clients)

File: `crates/op-web/src/handlers/grpc_stream_relay.rs`

```rust
/// WebSocket handler that proxies gRPC server-streaming responses to web clients.
/// Native zeroclaw-gui connects directly via tonic; this is the web fallback.
pub async fn grpc_stream_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_stream_relay(socket, state))
}

async fn handle_stream_relay(mut socket: WebSocket, state: AppState) {
    // 1. Receive { service, method, payload, endpoint } from client
    // 2. Open gRPC server-streaming call via op-grpc-bridge
    // 3. Forward each response frame as JSON over WebSocket
    // 4. Handle client close / stream end gracefully
}
```

---

## Data Flow Diagrams

### gRPC Method Execution Flow

```
ReflectionTreeExplorer (select method)
    │  writes selected_method to data binding
    ▼
GrpcMethodCaller (reads method descriptor)
    │  renders SchemaFormBuilder for input
    ▼
SchemaFormBuilder (user fills fields)
    │  user clicks Execute
    ▼
ActionBus.dispatch(grpc.call { service, method, payload, endpoint })
    │
    ▼
GrpcCallHandler
    │  1. resolve method from ReflectionRegistry
    │  2. build DynamicMessage from payload
    │  3. select channel from ConnectionPool
    │  4. execute unary RPC
    │  5. decode response → JSON
    ▼
ActionResult { correlation_id, Success, response_json, latency_ms }
    │  written to response data binding
    ▼
GrpcMethodCaller (re-renders with response)
```

### Network Telemetry Flow

```
NetworkTelemetry (background poller, configurable intervals)
    │
    ├── Unix socket probes → socket_health map
    ├── TCP connect probes → tcp_health map
    ├── D-Bus query (busctl) → ovs_state, wg_state
    └── Aggregation → topology graph
    │
    ▼
Data bindings (Arc<RwLock<...>>)
    │
    ├── SocketStatusPill reads socket_health[target]
    ├── TcpHealthBadge reads tcp_health[target]
    └── NetworkTopologyGraph reads topology
```

### Catalog Change Detection Flow

```
1-second timer tick
    │
    ▼
Read /dev/shm/opdbus/plugin-blobs/.manifest.json
    │  compare generation vs last_generation
    │
    ├── (unchanged) → no-op
    │
    └── (changed) →
        │  ReflectionRegistry.refresh_from_blobs()
        │  Reload affected plugin schemas
        │  Rebuild ReflectionTreeExplorer data
        │  UI shows "Schema Updated" toast
        ▼
        Components re-render with new schema data
```

---

## JSON Schema Declarations (Component Catalog)

Each component's JSON Schema is stored at `zeroclaw-gui/schemas/<kind>.json` and validated at catalog admission time.

### Example: `grpc_method_caller` Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["kind", "service", "method"],
  "properties": {
    "kind": { "const": "grpc_method_caller" },
    "service": { "type": "string", "description": "Fully-qualified gRPC service name" },
    "method": { "type": "string", "description": "Method name within the service" },
    "endpoint": {
      "oneOf": [
        { "type": "string", "description": "Socket path or host:port" },
        { "$ref": "#/$defs/EndpointSpec" }
      ],
      "default": "auto"
    },
    "schema_bind": { "type": "string", "description": "JSON pointer to request schema in data" },
    "response_bind": { "type": "string", "description": "JSON pointer to write response" }
  },
  "x-oscal-subid": "ui.component.grpc-method-caller@v1",
  "$defs": {
    "EndpointSpec": {
      "type": "object",
      "properties": {
        "type": { "enum": ["unix", "tcp", "grpc_web", "auto"] },
        "path": { "type": "string" },
        "host": { "type": "string" },
        "port": { "type": "integer" },
        "url": { "type": "string" }
      }
    }
  }
}
```

### Example: `socket_status_pill` Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["kind", "target", "label"],
  "properties": {
    "kind": { "const": "socket_status_pill" },
    "target": {
      "type": "object",
      "required": ["type"],
      "properties": {
        "type": { "enum": ["unix", "tcp"] },
        "path": { "type": "string" },
        "host": { "type": "string" },
        "port": { "type": "integer" }
      }
    },
    "label": { "type": "string" },
    "poll_secs": { "type": "integer", "default": 5, "minimum": 1 }
  },
  "x-oscal-subid": "ui.component.socket-status-pill@v1"
}
```

---

## Security & Validation

### Input Sanitization

- All `bind` paths are validated as legal JSON pointers before use.
- Action payloads are validated against the handler's `input_schema()` before dispatch.
- gRPC method paths are resolved from the `ReflectionRegistry` — arbitrary paths rejected.
- Socket probe targets are restricted to paths under `/run/` and localhost TCP ports.

### OSCAL Compliance Tracking

Every new component schema carries `x-oscal-subid`:
- `ui.component.grpc-method-caller@v1`
- `ui.component.grpc-stream-viewer@v1`
- `ui.component.reflection-tree-explorer@v1`
- `ui.component.socket-status-pill@v1`
- `ui.component.tcp-health-badge@v1`
- `ui.component.network-topology-graph@v1`
- `ui.component.schema-form-builder@v1`

### D-Bus/gRPC Policy Validation

- Actions targeting host services validate against runit service names (never `systemctl`).
- Container lifecycle actions route through D-Bus (`busctl`) per policy PC-2.
- Xray state reads are permitted; writes are blocked (only the control-plane generator may write `/etc/xray/xray_config.json`).

---

## Failover & Error Handling

| Failure Mode | Behavior |
|---|---|
| SHM blob dir missing | Show "Catalog unavailable" banner; disable reflection tree; keep cached schema if available |
| Blob integrity failure (SHA256 mismatch) | Skip corrupted blob; log warning; show degraded indicator for affected plugin |
| Generation mismatch during read | Retry once after 100ms; if still mismatched, use stale data with "refreshing" indicator |
| gRPC endpoint unreachable | Connection indicator turns red; queued actions return Error result; auto-retry with exponential backoff |
| Unix socket gone | SocketStatusPill shows red; tooltip shows last-seen time and error |
| Action handler timeout (5s default) | Return Error result with "timeout" code; surface in UI; do NOT retry automatically |
| WebSocket relay disconnect | Web client shows reconnecting indicator; buffer up to 100 missed frames; replay on reconnect |

---

## File Layout (New/Modified)

```
crates/zeroclaw-gui/
├── src/
│   ├── actions/
│   │   ├── mod.rs              (ActionBus, ActionRequest, ActionResult, ActionHandler trait)
│   │   ├── grpc_call.rs        (grpc.call handler)
│   │   ├── grpc_stream.rs      (grpc.stream_subscribe handler)
│   │   ├── socket_health.rs    (socket.check_health handler)
│   │   └── schema_mutate.rs    (schema.mutate handler)
│   ├── catalog/
│   │   ├── interpret.rs        (MODIFIED: add 7 new match arms)
│   │   └── ... (existing, unchanged)
│   ├── telemetry.rs            (NEW: NetworkTelemetry poller)
│   ├── conn.rs                 (MODIFIED: add ConnectionPool, EndpointSpec)
│   └── grpc.rs                 (MODIFIED: extend ReflectionRegistry with blob_pools)
├── schemas/
│   ├── grpc_method_caller.json
│   ├── grpc_stream_viewer.json
│   ├── reflection_tree_explorer.json
│   ├── socket_status_pill.json
│   ├── tcp_health_badge.json
│   ├── network_topology_graph.json
│   └── schema_form_builder.json
└── pages/
    └── network.json            (NEW: static draft page composing networking components)

crates/op-web/src/
├── handlers/
│   └── grpc_stream_relay.rs    (NEW: WebSocket gRPC stream proxy)
└── routes/
    └── mod.rs                  (MODIFIED: mount grpc_stream_relay route)
```
