# op-web UI Requirements
## Crate-by-Crate UI Analysis

**Version**: 1.1.0
**Date**: 2026-01-31
**Status**: DRAFT

---

> **Canonical MCP architecture:** MCP discovery/execution is owned solely by
> `op-grpc-bridge` on TLS `:8090` — see
> `.kiro/specs/unified-authenticated-mcp-cognitive-control-plane/`. The "op-mcp"
> section below is a **read-only monitoring dashboard** (status, tool-registry
> browser, metrics) that reads bridge state over the authenticated gRPC path. op-web
> MUST NOT host an MCP server, MCP transports, or any MCP execution endpoint on
> `:8080` (canonical FR-1). Any wording implying op-web runs the MCP protocol server
> is superseded.

---

## Overview

This document analyzes each crate in the op-dbus workspace to identify UI requirements. The UI will be built as an embedded React SPA (TypeScript + Vite) within the `op-web` crate that scales to ~16,000 D-Bus objects.

**Key Architecture Decision**: The UI is embedded into the `op-web` Rust binary using `rust-embed`. No external static files - single binary deployment.

---

## Crate Analysis Summary

| Crate | UI Priority | UI Elements Needed |
|-------|-------------|-------------------|
| op-agents | HIGH | Agent catalog, agent status, agent execution |
| op-blockchain | HIGH | Audit trail viewer, event chain, blockchain explorer |
| op-cache | MEDIUM | Cache stats, NUMA topology, workstack promotion |
| op-chat | HIGH | Chat interface, conversation history, tool execution |
| op-core | LOW | Core types display (internal) |
| op-introspection | HIGH | D-Bus browser, service tree, object inspector |
| op-mcp | HIGH | MCP server status, tool registry, transport config |
| op-tools | HIGH | Tool catalog, tool execution, security profiles |
| op-workflows | HIGH | Workflow builder, DAG visualization, execution |
| op-state | MEDIUM | State diff viewer, plugin state, checkpoints |
| op-network | MEDIUM | Network topology, OVS bridges, OpenFlow rules |
| op-llm | MEDIUM | Provider config, model selection, token usage |
| op-execution-tracker | HIGH | Execution timeline, metrics, telemetry |
| op-grpc-bridge | MEDIUM | Sync status, event stream, gRPC connections |
| op-inspector | HIGH | Inspector Gadget UI, schema viewer |
| op-state-store | MEDIUM | Job ledger, execution history, schema registry |
| op-plugins | HIGH | Plugin manager, state plugins, lifecycle |
| op-http | LOW | Server config (admin only) |
| op-mcp-aggregator | HIGH | Aggregator dashboard, upstream servers, profiles |
| op-deployment | LOW | Deployment status, image manager |

---

## 1. op-agents (HIGH PRIORITY)

**Purpose**: 70+ specialized domain agents with security sandboxing

### User Stories

#### 1.1 Agent Catalog Browser
**As a** user
**I want to** browse all available agents by category
**So that** I can discover and select the right agent for my task

**Acceptance Criteria**:
- [ ] Display agents grouped by category (Language, Infrastructure, Orchestration, etc.)
- [ ] Show agent metadata: name, description, capabilities, operations
- [ ] Filter agents by category, capability, or search term
- [ ] Support virtualized list for 70+ agents

#### 1.2 Agent Status Dashboard
**As an** operator
**I want to** see real-time status of running agents
**So that** I can monitor agent health and resource usage

**Acceptance Criteria**:
- [ ] Show active agent instances with status badges
- [ ] Display agent metrics: execution count, success rate, latency
- [ ] Real-time updates via WebSocket
- [ ] Alert on agent failures or resource exhaustion

#### 1.3 Agent Execution Interface
**As a** user
**I want to** execute agent operations with parameters
**So that** I can leverage agent capabilities

**Acceptance Criteria**:
- [ ] Select agent and operation from dropdowns
- [ ] Dynamic form generation based on operation schema
- [ ] Execute with confirmation for elevated operations
- [ ] Display execution results with syntax highlighting
- [ ] Blockchain logging for all executions

---

## 2. op-blockchain (HIGH PRIORITY)

**Purpose**: Streaming blockchain for audit trails and compliance

### User Stories

#### 2.1 Blockchain Explorer
**As an** admin
**I want to** browse the blockchain audit trail
**So that** I can verify system integrity and compliance

**Acceptance Criteria**:
- [ ] Paginated list of blockchain events
- [ ] Filter by event type, actor, target, time range
- [ ] Event detail view with full payload
- [ ] Merkle proof verification display
- [ ] Export events for compliance reporting

#### 2.2 Event Chain Viewer
**As an** operator
**I want to** trace event chains for specific operations
**So that** I can understand causality and debug issues

**Acceptance Criteria**:
- [ ] Visual event chain timeline
- [ ] Link related events by traceId
- [ ] Show state snapshots at each event
- [ ] Highlight anomalies or verification failures

---

## 3. op-cache (MEDIUM PRIORITY)

**Purpose**: BTRFS-based caching with NUMA awareness

### User Stories

#### 3.1 Cache Statistics Dashboard
**As an** operator
**I want to** monitor cache performance and utilization
**So that** I can optimize system performance

**Acceptance Criteria**:
- [ ] Display cache hit/miss ratios
- [ ] Show BTRFS subvolume usage
- [ ] NUMA node distribution visualization
- [ ] Workstack promotion candidates list

#### 3.2 NUMA Topology Viewer
**As an** admin
**I want to** visualize NUMA topology and cache placement
**So that** I can optimize memory locality

**Acceptance Criteria**:
- [ ] Visual NUMA node diagram
- [ ] Show CPU-to-node mapping
- [ ] Display cache allocation per node
- [ ] Memory bandwidth metrics

---

## 4. op-chat (HIGH PRIORITY)

**Purpose**: Chat orchestration with LLM integration

### User Stories

#### 4.1 Chat Interface
**As a** user
**I want to** interact with the system via natural language
**So that** I can perform operations without knowing CLI commands

**Acceptance Criteria**:
- [ ] Real-time chat with streaming responses
- [ ] Message history with session persistence
- [ ] Suggested actions based on context
- [ ] Tool execution inline with chat
- [ ] Markdown rendering for responses

#### 4.2 Conversation History
**As a** user
**I want to** browse and search past conversations
**So that** I can reference previous interactions

**Acceptance Criteria**:
- [ ] Session list with timestamps
- [ ] Full-text search across conversations
- [ ] Export conversation as markdown
- [ ] Resume previous sessions

#### 4.3 Tool Execution Log
**As an** operator
**I want to** see all tools executed during chat
**So that** I can audit AI-driven operations

**Acceptance Criteria**:
- [ ] Real-time tool execution feed
- [ ] Show tool name, parameters, result, duration
- [ ] Link to blockchain audit entry
- [ ] Filter by tool category or status

---

## 5. op-introspection (HIGH PRIORITY)

**Purpose**: D-Bus introspection and service discovery

### User Stories

#### 5.1 D-Bus Service Browser
**As a** user
**I want to** browse all D-Bus services on system/session bus
**So that** I can discover available interfaces

**Acceptance Criteria**:
- [ ] Tree view of services → objects → interfaces → methods
- [ ] Lazy loading for large service trees
- [ ] Search across 16,000+ objects
- [ ] Filter by bus type (system/session)

#### 5.2 Object Inspector
**As a** developer
**I want to** inspect D-Bus object details
**So that** I can understand interface contracts

**Acceptance Criteria**:
- [ ] Display object path, interfaces, methods, signals, properties
- [ ] Show method signatures with parameter types
- [ ] Property values with live updates
- [ ] Signal subscription and monitoring

#### 5.3 Method Invocation
**As a** developer
**I want to** invoke D-Bus methods directly
**So that** I can test and debug services

**Acceptance Criteria**:
- [ ] Dynamic form based on method signature
- [ ] Type-safe parameter input
- [ ] Execute with RBAC gating
- [ ] Display return values with type info
- [ ] Blockchain logging for mutations

---

## 6. op-mcp (HIGH PRIORITY)

**Purpose**: Read-only dashboard to **monitor** the MCP surface owned by
`op-grpc-bridge :8090` (canonical FR-1). op-web does not run an MCP protocol server
or any MCP transport; this section is telemetry/observability over the bridge, read
via the authenticated gRPC path.

### User Stories

#### 6.1 MCP Server Dashboard
**As an** operator
**I want to** monitor MCP server status and connections
**So that** I can ensure service availability

**Acceptance Criteria**:
- [ ] Server mode indicator (Compact/Agents/Full)
- [ ] Active transport connections
- [ ] Request/response metrics
- [ ] Error rate and latency charts

#### 6.2 Tool Registry Browser
**As a** user
**I want to** browse all registered MCP tools
**So that** I can discover available capabilities

**Acceptance Criteria**:
- [ ] Searchable tool list with categories
- [ ] Tool schema viewer with JSON Schema
- [ ] Security level indicators
- [ ] Usage statistics per tool

---

## 7. op-tools (HIGH PRIORITY)

**Purpose**: Tool registry with 16,000+ D-Bus tools

### User Stories

#### 7.1 Tool Catalog
**As a** user
**I want to** browse and search all available tools
**So that** I can find the right tool for my task

**Acceptance Criteria**:
- [ ] Virtualized list for 16,000+ tools
- [ ] Full-text search with fuzzy matching
- [ ] Filter by category, security level, namespace
- [ ] Tool detail panel with schema

#### 7.2 Tool Execution
**As a** user
**I want to** execute tools with parameters
**So that** I can perform system operations

**Acceptance Criteria**:
- [ ] Dynamic form from JSON Schema
- [ ] Parameter validation before execution
- [ ] RBAC gating with confirmation modal
- [ ] Result display with syntax highlighting
- [ ] Execution history with replay

#### 7.3 Security Profile Viewer
**As an** admin
**I want to** view and manage tool security profiles
**So that** I can control access to sensitive operations

**Acceptance Criteria**:
- [ ] Security level breakdown (ReadOnly, Modify, Elevated, Critical)
- [ ] Namespace permission matrix
- [ ] Audit log of security changes

---

## 8. op-workflows (HIGH PRIORITY)

**Purpose**: DAG-based workflow execution

### User Stories

#### 8.1 Workflow Builder
**As a** user
**I want to** create workflows by connecting nodes
**So that** I can automate multi-step operations

**Acceptance Criteria**:
- [ ] Drag-and-drop node canvas
- [ ] Node palette with available tools/plugins
- [ ] Connection validation (type compatibility)
- [ ] Save/load workflow definitions
- [ ] Export as JSON/YAML

#### 8.2 Workflow Execution
**As a** user
**I want to** execute workflows and monitor progress
**So that** I can run automated operations

**Acceptance Criteria**:
- [ ] Start/stop/pause workflow execution
- [ ] Real-time node status updates
- [ ] Execution timeline with durations
- [ ] Error handling with retry options
- [ ] Output inspection per node

#### 8.3 DAG Visualization
**As a** user
**I want to** visualize workflow as a directed graph
**So that** I can understand execution flow

**Acceptance Criteria**:
- [ ] Interactive DAG diagram
- [ ] Node status coloring (pending/running/success/failed)
- [ ] Zoom/pan controls
- [ ] Highlight critical path

---

## 9. op-state (MEDIUM PRIORITY)

**Purpose**: State management with plugins

### User Stories

#### 9.1 State Diff Viewer
**As an** operator
**I want to** see state differences before applying changes
**So that** I can review and approve modifications

**Acceptance Criteria**:
- [ ] Side-by-side diff view
- [ ] Syntax highlighting for JSON/YAML
- [ ] Approve/reject individual changes
- [ ] Dry-run validation results

#### 9.2 Plugin State Browser
**As an** admin
**I want to** browse state managed by each plugin
**So that** I can understand system configuration

**Acceptance Criteria**:
- [ ] Plugin list with state summary
- [ ] Hierarchical state tree per plugin
- [ ] State history with checkpoints
- [ ] Rollback to previous checkpoint

---

## 10. op-network (MEDIUM PRIORITY)

**Purpose**: Network management with OVS/OpenFlow

### User Stories

#### 10.1 Network Topology
**As an** operator
**I want to** visualize network topology
**So that** I can understand connectivity

**Acceptance Criteria**:
- [ ] Interactive network diagram
- [ ] Show bridges, ports, interfaces
- [ ] Link status indicators
- [ ] Traffic flow visualization

#### 10.2 OpenFlow Rules
**As an** admin
**I want to** manage OpenFlow rules
**So that** I can control traffic routing

**Acceptance Criteria**:
- [ ] Rule list with match/action display
- [ ] Add/edit/delete rules
- [ ] Rule priority ordering
- [ ] Flow statistics per rule

---

## 11. op-execution-tracker (HIGH PRIORITY)

**Purpose**: Execution monitoring and telemetry

### User Stories

#### 11.1 Execution Timeline
**As an** operator
**I want to** see execution timeline across the system
**So that** I can monitor activity and performance

**Acceptance Criteria**:
- [ ] Gantt-style timeline view
- [ ] Filter by execution type, status, duration
- [ ] Drill-down to execution details
- [ ] Correlation with traces

#### 11.2 Metrics Dashboard
**As an** operator
**I want to** view execution metrics
**So that** I can identify performance issues

**Acceptance Criteria**:
- [ ] Throughput charts (executions/sec)
- [ ] Latency percentiles (p50, p95, p99)
- [ ] Error rate trends
- [ ] Resource utilization

---

## 12. op-mcp-aggregator (HIGH PRIORITY)

**Purpose**: MCP server aggregation with profiles

### User Stories

#### 12.1 Aggregator Dashboard
**As an** operator
**I want to** monitor aggregator status
**So that** I can ensure upstream connectivity

**Acceptance Criteria**:
- [ ] Upstream server health status
- [ ] Tool count per server
- [ ] Request routing statistics
- [ ] Cache hit rates

#### 12.2 Profile Manager
**As an** admin
**I want to** manage tool profiles
**So that** I can control tool exposure per context

**Acceptance Criteria**:
- [ ] Profile list with tool counts
- [ ] Create/edit/delete profiles
- [ ] Assign tools to profiles
- [ ] Profile activation controls

---

## 13. op-plugins (HIGH PRIORITY)

**Purpose**: Plugin system with state management

### User Stories

#### 13.1 Plugin Manager
**As an** admin
**I want to** manage installed plugins
**So that** I can extend system capabilities

**Acceptance Criteria**:
- [ ] Plugin list with status badges
- [ ] Enable/disable plugins
- [ ] Plugin configuration editor
- [ ] Dependency visualization

#### 13.2 State Plugin Browser
**As an** operator
**I want to** browse state plugins
**So that** I can understand managed domains

**Acceptance Criteria**:
- [ ] Plugin categories (network, systemd, LXC, etc.)
- [ ] Capabilities per plugin
- [ ] State schema viewer
- [ ] Recent changes log

---

## Cross-Cutting Requirements

### Authentication & Authorization

#### 14.1 Token + OIDC Auth
**As a** user
**I want to** authenticate via token or OIDC SSO
**So that** I can access the system securely

**Acceptance Criteria**:
- [ ] Token-based login form
- [ ] OIDC SSO redirect flow
- [ ] Session management with refresh
- [ ] Role display in header

#### 14.2 RBAC Gating
**As an** admin
**I want to** enforce role-based access
**So that** sensitive operations are protected

**Acceptance Criteria**:
- [ ] RBACGate component for UI elements
- [ ] Role-based menu visibility
- [ ] Confirmation modal for elevated actions
- [ ] Blockchain comment input for audited actions

### Quota & Rate Limiting

#### 15.1 Quota Tracking
**As a** user
**I want to** see my quota usage
**So that** I can manage my resource consumption

**Acceptance Criteria**:
- [ ] QuotaMeter in header
- [ ] QuotaCostBadge on actions
- [ ] Quota warning notifications
- [ ] Request quota increase flow

### Live Streaming

#### 16.1 WebSocket Subscriptions
**As a** user
**I want to** subscribe to live data streams
**So that** I can monitor real-time activity

**Acceptance Criteria**:
- [ ] Subscribe/unsubscribe to channels
- [ ] Server-side sampling configuration
- [ ] Client-side rate limiting
- [ ] Reconnection handling

### Search & Navigation

#### 17.1 Global Search
**As a** user
**I want to** search across all entities
**So that** I can quickly find what I need

**Acceptance Criteria**:
- [ ] Unified search bar
- [ ] Search suggestions with categories
- [ ] Recent searches history
- [ ] Keyboard shortcuts (Cmd+K)

---

## Technical Requirements

### Performance
- Support ~16,000 D-Bus objects with virtualized lists
- Cursor pagination for all list endpoints
- Chunked tree loading (200 items per chunk)
- WebSocket message rate limiting (50 msg/sec per connection)

### Scalability
- Search-first patterns (no full client-side data)
- Server-side sampling for live streams (1/1000 default)
- Lazy loading for nested data structures

### Security
- All mutations require RBAC gating
- Sensitive actions require confirmation modal
- Blockchain logging for audited operations
- Payload masking with unmask confirmation

---

## UI Component Library

### Core Components
- TimeRangeSelector
- RealtimeToggle
- GlobalSearch
- VirtualObjectList
- VirtualTree
- PayloadViewerModal (WASM decoder)
- MetricChart
- FilterPill
- RBACGate
- QuotaMeter
- QuotaCostBadge
- LiveLogTail
- ChatMessage
- ChatActions

### MCP Components
- McpCard
- McpActionPanel
- McpPolicyEditor
- McpTracePanel
- McpLogTail
- McpJobList

### Workflow Components
- WorkflowCanvas
- NodePalette
- NodeEditor
- ConnectionLine
- ExecutionTimeline

---

## API Contract Summary

### Key Endpoints
- `GET /api/v1/overview` - Dashboard overview
- `GET /api/v1/nodes` - Node list with pagination
- `GET /api/v1/services` - Service list
- `GET /api/v1/objects` - Object search
- `GET /api/v1/traces` - Trace list
- `GET /api/v1/logs` - Log search
- `GET /api/v1/mcps` - MCP list
- `GET /api/v1/quotas` - Quota status
- `GET /api/v1/blockchain` - Audit trail
- `POST /api/v1/chat` - Chat message
- `POST /api/v1/jobs` - Create job

### WebSocket Channels
- `traces` - Live trace events
- `metrics` - System metrics
- `logs` - Log stream
- `mcp:<id>/traces` - MCP-specific traces
- `mcp:<id>/logs` - MCP-specific logs
- `mcp:<id>/events` - MCP events

---

## Next Steps

1. ✅ Create requirements.md with crate analysis
2. ✅ Create design.md with plugin architecture and component design
3. ✅ Create tasks.md with implementation plan
4. Implement Phase 0: Plugin Infrastructure
5. Implement Phase 1: Embedding Setup
6. Build core components
