# Tasks: NetMaker Custom json-render UI

## Task Ordering & Dependencies

```
Task 1 (ActionBus core)
    └── Task 2 (Connection pool)
         └── Task 3 (gRPC call handler)
              └── Task 4 (gRPC stream handler)
Task 5 (Reflection registry extension) ──┐
Task 6 (Telemetry poller)                │
    └── Task 7 (Socket/TCP components)   │
Task 8 (Schema form builder) ────────────┤
    └── Task 9 (gRPC method caller)      │
         └── Task 10 (Reflection tree)───┘
              └── Task 11 (Stream viewer)
Task 12 (Topology graph)
Task 13 (op-web WebSocket relay)
Task 14 (Static page composition)
Task 15 (Integration tests)
```

---

## Task 1: Action Bus Core Infrastructure

**File**: `crates/zeroclaw-gui/src/actions/mod.rs`

**Deliverables**:
1. Define `ActionRequest` struct (correlation_id: Uuid, action_type: String, payload: Value, timestamp: SystemTime)
2. Define `ActionResult` struct (correlation_id: Uuid, status: ActionStatus, payload: Value, latency_ms: u64)
3. Define `ActionStatus` enum (Success, Error { code, message }, Streaming { stream_id })
4. Define `ActionHandler` trait (action_prefix, input_schema, dispatch)
5. Implement `ActionBus` struct with handler registration, dispatch routing by prefix, and audit logging
6. Add `AuditEntry` struct (correlation_id, timestamp, action_type, target, status)
7. Wire `ActionBus` into `zeroclaw-gui/src/app.rs` app state

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: register a mock handler, dispatch an action, receive correct result
- Unit test: dispatch to unregistered prefix returns Error
- Unit test: audit log contains entry after dispatch

---

## Task 2: Connection Pool

**File**: `crates/zeroclaw-gui/src/conn.rs` (modify existing)

**Deliverables**:
1. Define `EndpointSpec` enum (Unix { path }, Tcp { host, port }, GrpcWeb { url }, Auto)
2. Implement `ConnectionPool` struct managing tonic `Channel` instances per endpoint
3. `get_channel(endpoint: &EndpointSpec) -> Result<Channel>` with fallback: Unix > TCP > gRPC-Web
4. Lazy connection creation (connect on first use, cache thereafter)
5. Connection health tracking (mark dead on error, attempt reconnect on next request)
6. `status(endpoint: &EndpointSpec) -> ConnectionStatus` (Connected/Disconnected/Error)

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: EndpointSpec serialization round-trips
- Integration test (requires running socket): connect to Unix socket at `/run/ghostbridge/grpc.sock`

---

## Task 3: gRPC Call Action Handler

**File**: `crates/zeroclaw-gui/src/actions/grpc_call.rs`

**Deliverables**:
1. Implement `GrpcCallHandler` struct holding `Arc<RwLock<ReflectionRegistry>>` and `ConnectionPool`
2. Implement `ActionHandler` for `GrpcCallHandler`:
   - `action_prefix()` → `"grpc.call"`
   - `input_schema()` → JSON Schema requiring `service`, `method`, `payload`, optional `endpoint`
   - `dispatch()`:
     a. Extract service/method/payload/endpoint from request
     b. Resolve method descriptor from ReflectionRegistry
     c. Build `prost_reflect::DynamicMessage` from payload + input descriptor
     d. Get channel from ConnectionPool
     e. Execute unary call via `tonic::client::Grpc::unary()`
     f. Decode response DynamicMessage → JSON Value
     g. Return ActionResult with latency measurement
3. Error handling: method not found, connection failure, decode error, timeout (5s)

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: validates input schema rejects missing `method` field
- Integration test: call a known method on op-grpc-bridge and verify response shape

---

## Task 4: gRPC Stream Action Handler

**File**: `crates/zeroclaw-gui/src/actions/grpc_stream.rs`

**Deliverables**:
1. Define `StreamHandle` struct (stream_id: Uuid, receiver channel, cancel token)
2. Implement `GrpcStreamHandler` struct with active_streams map
3. Implement `ActionHandler`:
   - `action_prefix()` → `"grpc.stream_subscribe"`
   - `dispatch()`:
     a. Open server-streaming RPC via tonic
     b. Spawn background task reading frames into channel
     c. Register StreamHandle in active_streams
     d. Return ActionResult::Streaming { stream_id }
4. `read_frame(stream_id) -> Option<Value>` for viewer consumption
5. `cancel_stream(stream_id)` for cleanup

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: subscribe → receive stream_id → cancel → stream removed from active map
- Integration test: subscribe to a streaming method, receive at least one frame

---

## Task 5: Reflection Registry Extension (Blob Pools)

**File**: `crates/zeroclaw-gui/src/grpc.rs` (modify existing)

**Deliverables**:
1. Add `blob_pools: HashMap<String, prost_reflect::DescriptorPool>` field to `ReflectionRegistry`
2. Add `last_generation: u64` field for change detection
3. Implement `refresh_from_blobs()`:
   - Read `.manifest.json` from `/dev/shm/opdbus/plugin-blobs/`
   - Compare generation; return early if unchanged
   - For each plugin: read blob file, extract section 3, decode into `DescriptorPool`
   - Update `blob_pools` and `last_generation`
4. Implement `all_services() -> Vec<ServiceDescriptor>` (merged from all pools)
5. Implement `resolve_method(path: &str) -> Option<MethodDescriptor>` (search all pools)
6. Implement `all_methods() -> Vec<MethodDescriptor>` for tree population
7. Add `op-blob` to `zeroclaw-gui` Cargo.toml dependencies

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: create a test blob in a temp dir, refresh_from_blobs, verify services listed
- Unit test: resolve_method returns correct descriptor for known path
- Unit test: generation check skips reload when unchanged

---

## Task 6: Network Telemetry Poller

**File**: `crates/zeroclaw-gui/src/telemetry.rs` (new)

**Deliverables**:
1. Define `NetworkTelemetry` struct with Arc<RwLock<...>> fields for socket_health, tcp_health, ovs_state, wg_state, topology
2. Define `SocketHealthResult` struct (target, reachable, latency_ms, last_check, error)
3. Define `TcpHealthResult` struct (same shape)
4. Define `OvsState` struct (bridges: Vec, ports: Vec, status)
5. Define `WgState` struct (interface, peers: Vec<WgPeer>, last_handshake)
6. Define `TopologyData`, `TopologyNode`, `TopologyEdge` structs
7. Implement `NetworkTelemetry::spawn()`:
   - Start async task polling Unix sockets every 5s (configurable)
   - Start async task polling TCP ports every 10s (configurable)
   - Start async task reading OVS state via `ovs-vsctl` output parsing
   - Start async task reading WireGuard state via `wg show` parsing
   - Aggregate into topology graph on each cycle
8. All polling MUST be non-blocking (tokio async, never block UI thread)

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: NetworkTelemetry creates with empty state
- Integration test: spawn poller, wait 6s, verify socket_health contains results for existing sockets
- Verify: polling does not panic when sockets don't exist (graceful error in result)

---

## Task 7: Socket Status & TCP Health Components

**Files**: 
- `crates/zeroclaw-gui/src/catalog/interpret.rs` (add match arms)
- `crates/zeroclaw-gui/schemas/socket_status_pill.json`
- `crates/zeroclaw-gui/schemas/tcp_health_badge.json`

**Deliverables**:
1. Add `"socket_status_pill"` match arm to `walk()`:
   - Read target from spec
   - Look up `socket_health[target]` from telemetry data binding
   - Render: colored dot (green/yellow/red) + label + latency text
   - Tooltip on hover: last_check time, full path, error message
2. Add `"tcp_health_badge"` match arm to `walk()`:
   - Read target (host, port) from spec
   - Look up `tcp_health[host:port]` from telemetry data binding
   - Render: badge frame with port number, label, status dot, latency annotation
3. Create JSON Schema files for both components
4. Add `x-oscal-subid` annotations to schemas

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: render socket_status_pill with mock data value → no RenderError
- Unit test: render tcp_health_badge with mock data value → no RenderError
- Unit test: socket_status_pill with missing data → shows "—" (graceful)
- Schema files validate against JSON Schema meta-schema

---

## Task 8: Schema Form Builder Component

**Files**:
- `crates/zeroclaw-gui/src/catalog/interpret.rs` (add match arm)
- `crates/zeroclaw-gui/schemas/schema_form_builder.json`

**Deliverables**:
1. Add `"schema_form_builder"` match arm to `walk()`:
   - Read JSON Schema from `schema_bind` data pointer
   - Read current form values from `value_bind` data pointer
   - For each schema property, render appropriate input widget:
     - `string` → `egui::TextEdit`
     - `integer`/`number` → `egui::DragValue` with min/max from schema
     - `boolean` → `egui::Checkbox`
     - `array` → repeatable group with +/- buttons
     - `object` → indented nested fieldset
     - `enum` → `egui::ComboBox`
   - Show validation state per field (red border + message on constraint violation)
   - Submit button emits action specified in `submit_action` with current form values
2. Implement recursive schema-to-widget mapping function
3. Handle `$ref` and `$defs` resolution within the schema
4. Create JSON Schema file for the component
5. Add `x-oscal-subid` annotation

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: render with simple string/integer schema → produces valid egui response
- Unit test: render with nested object schema → recursively renders fields
- Unit test: render with enum schema → produces ComboBox
- Unit test: validation violation shows error indicator

---

## Task 9: gRPC Method Caller Component

**Files**:
- `crates/zeroclaw-gui/src/catalog/interpret.rs` (add match arm)
- `crates/zeroclaw-gui/schemas/grpc_method_caller.json`

**Deliverables**:
1. Add `"grpc_method_caller"` match arm to `walk()`:
   - Read service/method from spec
   - Resolve method descriptor from ReflectionRegistry
   - Render method name as heading
   - Render embedded `schema_form_builder` for request input (schema from method input descriptor)
   - Render "Execute" button → emits `grpc.call` action via ActionBus
   - Render response area (JSON tree view of last response from `response_bind`)
   - Render latency badge from last action result
   - Render connection status indicator for configured endpoint
2. Wire to ActionBus: button click → ActionRequest → await ActionResult → update response binding
3. Create JSON Schema file
4. Add `x-oscal-subid` annotation

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: render with valid service/method spec → no RenderError
- Unit test: render with invalid service → shows "method not found" in UI
- Integration test: render → click Execute → verify gRPC call dispatched via ActionBus

---

## Task 10: Reflection Tree Explorer Component

**Files**:
- `crates/zeroclaw-gui/src/catalog/interpret.rs` (add match arm)
- `crates/zeroclaw-gui/schemas/reflection_tree_explorer.json`

**Deliverables**:
1. Add `"reflection_tree_explorer"` match arm to `walk()`:
   - Source data from ReflectionRegistry (all services/methods from blob pools)
   - Build tree structure: Package → Service → Method
   - Render collapsible tree with `egui::CollapsingHeader`
   - Render search/filter text input (filters tree in real-time)
   - Per-method badge: [U]nary, [S]erver-stream, [C]lient-stream, [B]idi
   - On method selection: write fully-qualified path to `selected_bind` data pointer
2. Tree state persists across frames (expanded/collapsed nodes)
3. Selection highlighting
4. Create JSON Schema file
5. Add `x-oscal-subid` annotation

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: render with mock registry containing 3 services → tree renders without error
- Unit test: filter "Get" → only methods containing "Get" visible
- Unit test: selection writes to bound data path

---

## Task 11: gRPC Stream Viewer Component

**Files**:
- `crates/zeroclaw-gui/src/catalog/interpret.rs` (add match arm)
- `crates/zeroclaw-gui/schemas/grpc_stream_viewer.json`

**Deliverables**:
1. Add `"grpc_stream_viewer"` match arm to `walk()`:
   - Read `stream_id_bind` from spec to get active stream ID
   - Read messages from GrpcStreamHandler via stream_id
   - Render scrollable message list:
     - Each message: timestamp + formatted JSON (collapsible)
     - Max messages cap (default 100, configurable via `max_messages`)
     - Auto-scroll to bottom (toggleable via `auto_scroll`)
   - Render control bar: message count, Pause/Resume button, Clear button
   - Pause: stop reading from channel (buffer continues filling)
   - Clear: discard displayed messages (does not affect stream)
2. Efficient rendering: only re-render visible messages (egui ScrollArea clipping)
3. Create JSON Schema file
4. Add `x-oscal-subid` annotation

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: render with no active stream → shows "No stream" placeholder
- Unit test: render with mock messages → displays formatted JSON entries
- Unit test: max_messages cap enforced (oldest dropped)
- Unit test: pause flag stops message consumption

---

## Task 12: Network Topology Graph Component

**Files**:
- `crates/zeroclaw-gui/src/catalog/interpret.rs` (add match arm)
- `crates/zeroclaw-gui/schemas/network_topology_graph.json`

**Deliverables**:
1. Add `"network_topology_graph"` match arm to `walk()`:
   - Read nodes/edges from data bindings (`nodes_bind`, `edges_bind`)
   - Implement force-directed layout algorithm:
     - Repulsion between all nodes (Coulomb's law)
     - Attraction along edges (Hooke's law)
     - Damping to reach equilibrium
     - Cache positions between frames (incremental update)
   - Render using `egui::Painter`:
     - Nodes: different shapes by type (circle=container, square=bridge, diamond=interface, triangle=endpoint)
     - Node colors by status (green=up, red=down, yellow=degraded)
     - Edges: solid=physical, dashed=virtual, dotted=tunnel
     - Labels on nodes and edges
   - Interaction:
     - Pan: drag background
     - Zoom: scroll wheel
     - Select node: click → highlight, show detail tooltip
     - Drag node: reposition (breaks force layout for that node)
2. Node detail tooltip shows: label, type, status, associated telemetry
3. Create JSON Schema file
4. Add `x-oscal-subid` annotation

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Unit test: render with 5 nodes, 4 edges → no panic, positions converge
- Unit test: empty topology → shows "No topology data" placeholder
- Unit test: incremental update (add node) → only new node re-positioned
- Visual inspection: run with mock data, verify graph is readable

---

## Task 13: op-web WebSocket gRPC Stream Relay

**Files**:
- `crates/op-web/src/handlers/grpc_stream_relay.rs` (new)
- `crates/op-web/src/routes/mod.rs` (modify to mount route)

**Deliverables**:
1. Implement WebSocket upgrade handler at `/ws/grpc-stream`
2. Protocol:
   - Client sends: `{ "action": "subscribe", "service": "...", "method": "...", "payload": {...}, "endpoint": "auto" }`
   - Server streams: `{ "frame": <n>, "data": {...}, "timestamp": "..." }`
   - Client sends: `{ "action": "cancel" }` → server closes stream
   - Server sends: `{ "status": "complete" }` or `{ "status": "error", "message": "..." }`
3. Connect to gRPC backend via op-grpc-bridge's existing channel infrastructure
4. Frame buffering: if client is slow, buffer up to 100 frames before dropping oldest
5. Mount route in op-web router

**Verification**:
- `cargo build -p op-web` compiles
- Unit test: WebSocket protocol message serialization
- Integration test: connect via WebSocket, subscribe to streaming method, receive frames
- Integration test: send cancel → stream closes cleanly

---

## Task 14: Static Draft Page Composition

**File**: `crates/zeroclaw-gui/pages/network.json` (new)

**Deliverables**:
1. Create `network.json` composing all networking components into a single page:
   ```json
   {
     "spec": {
       "kind": "stack",
       "dir": "v",
       "children": [
         { "kind": "heading", "text": "Network Control Surface", "size": 20 },
         { "kind": "stack", "dir": "h", "children": [
           { "kind": "socket_status_pill", "target": {"type":"unix","path":"/run/ghostbridge/grpc.sock"}, "label": "gRPC Bridge" },
           { "kind": "socket_status_pill", "target": {"type":"unix","path":"/run/opdbus/session-bus.sock"}, "label": "Session Bus" },
           { "kind": "socket_status_pill", "target": {"type":"unix","path":"/run/openvswitch/db.sock"}, "label": "OVS DB" },
           { "kind": "tcp_health_badge", "target": {"type":"tcp","host":"127.0.0.1","port":8081}, "label": "Netmaker API" },
           { "kind": "tcp_health_badge", "target": {"type":"tcp","host":"127.0.0.1","port":1883}, "label": "EMQX MQTT" }
         ]},
         { "kind": "separator" },
         { "kind": "stack", "dir": "h", "children": [
           { "kind": "reflection_tree_explorer", "source": "blob_catalog", "selected_bind": "/selected_method" },
           { "kind": "grpc_method_caller", "service_bind": "/selected_method/service", "method_bind": "/selected_method/method", "endpoint": "auto", "response_bind": "/response" }
         ]},
         { "kind": "separator" },
         { "kind": "network_topology_graph", "nodes_bind": "/topology/nodes", "edges_bind": "/topology/edges", "layout": "force_directed", "interactive": true }
       ]
     },
     "data": {
       "selected_method": null,
       "response": null,
       "topology": { "nodes": [], "edges": [] }
     },
     "source": {
       "plugin": "network_telemetry",
       "method": "get_network_state",
       "args": [],
       "poll_secs": 5
     }
   }
   ```
2. Add `Network` variant to `nav::Route` enum if not present
3. Verify page loads without RenderError (requires all component kinds to be registered)

**Verification**:
- `cargo build -p zeroclaw-gui` compiles
- Page JSON is valid (serde_json::from_str succeeds)
- Static page loader finds and parses the file
- With all components registered: full render produces no `RenderError`

---

## Task 15: Integration Tests & Spec Validation

**Files**:
- `crates/zeroclaw-gui/tests/action_bus_integration.rs` (new)
- `crates/zeroclaw-gui/tests/component_render_integration.rs` (new)

**Deliverables**:
1. **Action bus round-trip test**:
   - Register all 4 handlers (grpc.call, grpc.stream_subscribe, socket.check_health, schema.mutate)
   - Dispatch one action of each type
   - Verify correlation IDs match
   - Verify audit log has 4 entries
2. **Component render smoke test**:
   - For each of the 7 new component kinds, construct a minimal valid spec
   - Call `render_spec(ui, &spec, &mock_data)` in a headless egui context
   - Assert no `RenderError` returned
3. **Reflection registry blob test**:
   - Create temp dir with a test blob (use `op_blob::blobify` with a test schema)
   - Call `refresh_from_blobs()` pointing at temp dir
   - Verify services and methods are listed
4. **gRPC reflection verification** (manual gate):
   - `grpcurl -plaintext -unix /run/ghostbridge/grpc.sock list` shows expected services
   - `grpcurl -plaintext 127.0.0.1:8090 list` shows same services
5. **Policy compliance check**:
   - grep all new source files for `systemctl`, `s6-rc`, `s6-svc` → must be zero hits
   - grep for `/dev/shm/xray_config` or non-`/etc/xray/xray_config.json` Xray paths → must be zero
   - Verify no writes to blob files (all blob access is read-only)

**Verification**:
- `cargo test -p zeroclaw-gui` passes all new tests
- `cargo test -p op-web` passes
- `cargo clippy -p zeroclaw-gui -p op-web` has no warnings on new code
- Policy grep assertions pass

---

## Summary Checklist

| # | Task | Status |
|---|---|---|
| 1 | Action Bus core (mod.rs) | ☐ |
| 2 | Connection Pool (conn.rs) | ☐ |
| 3 | gRPC Call Handler | ☐ |
| 4 | gRPC Stream Handler | ☐ |
| 5 | Reflection Registry + blob pools | ☐ |
| 6 | Network Telemetry Poller | ☐ |
| 7 | Socket/TCP Status Components | ☐ |
| 8 | Schema Form Builder Component | ☐ |
| 9 | gRPC Method Caller Component | ☐ |
| 10 | Reflection Tree Explorer Component | ☐ |
| 11 | gRPC Stream Viewer Component | ☐ |
| 12 | Network Topology Graph Component | ☐ |
| 13 | op-web WebSocket Relay | ☐ |
| 14 | Static Draft Page Composition | ☐ |
| 15 | Integration Tests & Validation | ☐ |
