# Operation D-Bus UI Implementation Tasks

**Version**: 1.1.0
**Date**: 2026-01-31
**Location**: `crates/op-web/ui/` (embedded via rust-embed)

---

## Phase 0: Plugin Infrastructure (Schema-as-Code)

### 0. Web UI Plugin Implementation
- [x] 0.1 Create `crates/op-plugins/src/state_plugins/web_ui.rs` with 3-section structure
- [x] 0.2 Define `WebUiIdentity` struct (immutable: name, version, plugin_type, driver)
- [x] 0.3 Define `WebUiTunables` struct (mutable: enabled, cors, compression, theme, etc.)
- [x] 0.4 Define `WebUiCapabilities` struct (read-only capabilities)
- [x] 0.5 Implement JSON Schema methods: `identity_schema()`, `tunables_schema()`, `capabilities_schema()`
- [x] 0.6 Implement `property_schema()` for append-only field tracking
- [x] 0.7 Implement `StatePlugin` trait for WebUiPlugin
- [x] 0.8 Implement `validate_tunables()` with jsonschema validation
- [x] 0.9 Implement `is_path_immutable()` for immutability enforcement
- [ ] 0.10 Add D-Bus interface `org.opdbus.plugins.WebUi` with zbus
- [x] 0.11 Register WebUiPlugin in `default_registry.rs`
- [x] 0.12 Add unit tests for schema validation and state management

---

## Phase 1: Project Setup & Embedding Infrastructure

### 1. Rust Embedding Setup
- [x] 1.1 Add rust-embed and mime_guess to op-web Cargo.toml
- [x] 1.2 Create `crates/op-web/src/embedded_ui.rs` with RustEmbed handler
- [x] 1.3 Create `crates/op-web/build.rs` to compile UI before Rust
- [x] 1.4 Update router.rs to add embedded UI fallback route
- [x] 1.5 Create ui/dist directory for rust-embed (React build target)

### 2. React Project Scaffolding
- [x] 2.1 Initialize Vite + React + TypeScript in `crates/op-web/ui/`
- [x] 2.2 Configure Tailwind CSS with dark theme
- [x] 2.3 Set up ESLint + Prettier configuration
- [x] 2.4 Configure path aliases (@/ for src/)
- [x] 2.5 Configure Vite proxy for dev server
- [x] 2.6 Add base dependencies (zustand, react-query, react-router)

### 3. API Layer Setup
- [x] 3.1 Set up Connect (gRPC-Web) transport
- [x] 3.2 Create gRPC client wrapper with auth interceptor
- [x] 3.3 Implement WebSocket client connecting to /ws
- [x] 3.4 Create REST client for /api/* endpoints
- [x] 3.5 Set up React Query provider and default options

### 4. State Management
- [x] 4.1 Implement authStore (user, token, roles)
- [x] 4.2 Implement quotaStore (quotas, usage, warnings)
- [x] 4.3 Implement uiStore (sidebar, theme, recent searches)
- [x] 4.4 Create store persistence middleware

---

## Phase 2: Core Components

### 5. Layout Components
- [x] 5.1 Create AppShell with sidebar and header
- [x] 5.2 Implement Sidebar with navigation items
- [x] 5.3 Create Header with search, quota meter, user menu
- [x] 5.4 Add Breadcrumb component
- [x] 5.5 Implement responsive layout breakpoints

### 6. Data Display Components
- [x] 6.1 Implement VirtualList using @tanstack/virtual
- [x] 6.2 Create VirtualTree for hierarchical data
- [x] 6.3 Build DataTable with sorting and filtering
- [x] 6.4 Implement cursor-based Pagination component
- [x] 6.5 Create LoadingSpinner and skeleton loaders

### 7. Form Components
- [x] 7.1 Build DynamicForm from JSON Schema
- [x] 7.2 Create FilterBar with chips and clear
- [x] 7.3 Implement TimeRangeSelector (presets + custom)
- [x] 7.4 Build SearchInput with suggestions
- [x] 7.5 Create ConfirmModal with snowball comment

### 8. Security Components
- [x] 8.1 Implement RBACGate component
- [x] 8.2 Create QuotaMeter visualization
- [x] 8.3 Build QuotaCostBadge for actions
- [x] 8.4 Implement SnowballComment input
- [x] 8.5 Create ProtectedRoute wrapper

### 9. Visualization Components
- [x] 9.1 Set up Recharts with dark theme
- [x] 9.2 Create MetricChart (line, area, bar)
- [x] 9.3 Build Timeline component (Gantt-style)
- [x] 9.4 Implement basic NetworkGraph placeholder

---

## Phase 3: Chat & Communication

### 10. Chat Components
- [x] 10.1 Create ChatPanel container
- [x] 10.2 Implement ChatMessage with markdown
- [x] 10.3 Build ChatInput with suggestions
- [x] 10.4 Create ChatActions for suggested actions
- [x] 10.5 Add streaming response support

### 11. WebSocket Integration
- [x] 11.1 Implement useWebSocket hook for /ws endpoint
- [x] 11.2 Create subscription management
- [x] 11.3 Add client-side rate limiting
- [x] 11.4 Implement reconnection with backoff
- [x] 11.5 Create LiveLogTail component

---

## Phase 4: Agent & Tool Pages

### 12. Agent Pages
- [x] 12.1 Create AgentCatalog with category filters
- [x] 12.2 Build AgentCard component
- [x] 12.3 Implement AgentDetail page
- [x] 12.4 Create AgentExecution form
- [x] 12.5 Add AgentStatus dashboard

### 13. Tool Pages
- [x] 13.1 Create ToolCatalog with virtual list
- [x] 13.2 Build ToolCard component
- [x] 13.3 Implement ToolDetail with schema
- [x] 13.4 Create ToolExecution with dynamic form
- [x] 13.5 Add execution history panel

---

## Phase 5: D-Bus Browser

### 14. D-Bus Service Browser
- [x] 14.1 Create ServiceBrowser with tree view
- [x] 14.2 Implement lazy loading for tree nodes
- [x] 14.3 Build ObjectDetail inspector
- [x] 14.4 Create InterfacePanel component
- [x] 14.5 Implement MethodInvoke form

### 15. D-Bus Search
- [x] 15.1 Add full-text search across objects
- [x] 15.2 Implement search result highlighting
- [x] 15.3 Create search filters (bus, service, interface)
- [x] 15.4 Add recent searches history

---

## Phase 6: Workflow Builder

### 16. Workflow Canvas
- [x] 16.1 Set up dagre for layout
- [x] 16.2 Create DAGCanvas component
- [x] 16.3 Implement node drag-and-drop
- [x] 16.4 Build ConnectionLine rendering
- [x] 16.5 Add zoom/pan controls

### 17. Workflow Nodes
- [x] 17.1 Create NodePalette with categories
- [x] 17.2 Build WorkflowNodeCard component
- [x] 17.3 Implement NodeEditor sidebar
- [x] 17.4 Add connection validation
- [x] 17.5 Create node status indicators

### 18. Workflow Execution
- [x] 18.1 Create WorkflowList page
- [x] 18.2 Build WorkflowRun monitor
- [x] 18.3 Implement execution timeline
- [x] 18.4 Add node output inspection
- [x] 18.5 Create workflow save/load

---

## Phase 7: MCP & Snowball

### 19. MCP Pages
- [x] 19.1 Create McpList with search
- [x] 19.2 Build McpCard component
- [x] 19.3 Implement McpDetail dashboard
- [x] 19.4 Create McpActionPanel
- [x] 19.5 Build McpPolicyEditor

### 20. Snowball Pages
- [x] 20.1 Create AuditTrail browser
- [x] 20.2 Build EventCard component
- [x] 20.3 Implement EventChain visualization
- [x] 20.4 Add event filtering
- [x] 20.5 Create export functionality

---

## Phase 8: State & Network

### 21. State Pages
- [x] 21.1 Create StateDiff viewer
- [x] 21.2 Build side-by-side diff display
- [x] 21.3 Implement PluginState browser
- [x] 21.4 Add checkpoint history
- [x] 21.5 Create rollback confirmation

### 22. Network Pages
- [x] 22.1 Create Topology visualization
- [x] 22.2 Build bridge/port display
- [x] 22.3 Implement OpenFlow rules table
- [x] 22.4 Add rule editor form
- [x] 22.5 Create flow statistics view

---

## Phase 9: Execution & Metrics

### 23. Execution Pages
- [x] 23.1 Create Timeline page
- [x] 23.2 Build execution Gantt chart
- [x] 23.3 Implement Metrics dashboard
- [x] 23.4 Add latency percentile charts
- [x] 23.5 Create error rate visualization

---

## Phase 10: WASM Decoder & Payload

### 24. WASM Decoder
- [x] 24.1 Create Rust WASM project in ui/wasm/decoder/
- [x] 24.2 Implement decode_payload function with simd-json
- [x] 24.3 Build with wasm-pack
- [x] 24.4 Create TypeScript loader
- [x] 24.5 Add fallback pure-TS decoder

### 25. Payload Viewer
- [x] 25.1 Create PayloadViewer component
- [x] 25.2 Implement masked state
- [x] 25.3 Add unmask with confirmation
- [x] 25.4 Integrate WASM decoder
- [x] 25.5 Add syntax highlighting

---

## Phase 11: Dashboard & Polish

### 26. Dashboard
- [x] 26.1 Create Dashboard layout
- [x] 26.2 Build system overview cards
- [x] 26.3 Integrate chat panel
- [x] 26.4 Add quick actions
- [x] 26.5 Implement real-time updates via WebSocket

### 27. Settings & Auth
- [ ] 27.1 Create Settings page
- [ ] 27.2 Implement login flow
- [ ] 27.3 Add OIDC SSO support
- [ ] 27.4 Build quota management
- [ ] 27.5 Create user profile

---

## Phase 12: Testing & Build Integration

### 28. Testing
- [ ] 28.1 Set up Vitest + RTL in ui/
- [ ] 28.2 Write VirtualList tests
- [ ] 28.3 Test RBACGate behavior
- [ ] 28.4 Test WebSocket reconnection
- [x] 28.5 Verify embedded build works

### 29. Build Integration
- [x] 29.1 Verify build.rs compiles UI correctly
- [x] 29.2 Test rust-embed serves all assets
- [x] 29.3 Verify SPA routing fallback works
- [x] 29.4 Test cache headers for hashed assets
- [x] 29.5 Verify single-binary deployment

---

## Phase 13: Documentation

### 30. Documentation
- [x] 30.1 Update op-web README with UI build instructions
- [x] 30.2 Document embedded UI architecture
- [x] 30.3 Add development workflow (npm run dev with proxy)
- [x] 30.4 Document API endpoints used by UI
- [x] 30.5 Add troubleshooting guide

---

## Task Dependencies

```
Phase 0 (Plugin Infrastructure) → Phase 1 (Embedding Setup) → Phase 2 (Core Components) → Phase 3 (Chat)
                                                                                        ↘
Phase 4 (Agents/Tools) ← Phase 2                                                         Phase 5 (D-Bus)
                                                                                        ↗
Phase 6 (Workflows) ← Phase 2 + Phase 5

Phase 7 (MCP/Snowball) ← Phase 2
Phase 8 (State/Network) ← Phase 2
Phase 9 (Execution) ← Phase 2

Phase 10 (WASM) ← Phase 1
Phase 11 (Dashboard) ← All Pages
Phase 12 (Testing) ← All Components
Phase 13 (Docs) ← All
```

---

## Estimated Timeline

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Phase 0 | 1 day | None |
| Phase 1 | 2 days | Phase 0 |
| Phase 2 | 3 days | Phase 1 |
| Phase 3 | 2 days | Phase 2 |
| Phase 4 | 2 days | Phase 2 |
| Phase 5 | 3 days | Phase 2 |
| Phase 6 | 4 days | Phase 2, 5 |
| Phase 7 | 2 days | Phase 2 |
| Phase 8 | 2 days | Phase 2 |
| Phase 9 | 2 days | Phase 2 |
| Phase 10 | 2 days | Phase 1 |
| Phase 11 | 2 days | All Pages |
| Phase 12 | 2 days | All |
| Phase 13 | 1 day | All |

**Total: ~30 days**

---

## Priority Order

1. **Critical Path**: Phase 0 → 1 → 2 → 4 → 5 → 11 (Plugin + Core functionality)
2. **High Value**: Phase 3 (Chat), Phase 6 (Workflows)
3. **Medium Value**: Phase 7-9 (MCP, State, Execution)
4. **Support**: Phase 10 (WASM), Phase 12-13 (Testing, Docs)

---

## Build Commands

```bash
# Development (with hot reload)
cd crates/op-web/ui
npm install
npm run dev  # Starts Vite dev server with proxy to :8080

# Production build (embedded)
cd crates/op-web
cargo build --release  # build.rs compiles UI first

# Run with embedded UI
./target/release/op-web  # Serves UI from embedded assets
```