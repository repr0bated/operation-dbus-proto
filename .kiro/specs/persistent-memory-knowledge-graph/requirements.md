# Requirements Document: Persistent Memory and Knowledge Graph System

## Introduction

The Persistent Memory and Knowledge Graph System replaces OpenClaw's current file-based memory store with a layered architecture that provides cryptographic audit trails, semantic search, and user memory stores. The system is built on an immutable event stream with vectorization at capture time, with semantic graphs built as overlay views.

## Architecture Overview

### Layer Architecture:
1. **Layer 0: Event Capture & Immutable Ledger**
   - All system mutations captured as immutable events
   - Cryptographic hashing and chaining for auditability
   - Real-time vectorization at capture time

2. **Layer 1: Vector Store**
   - Semantic embeddings generated at event capture
   - Reusable vector artifacts for multiple consumers
   - Real-time indexing for similarity search

3. **Layer 2: Graph Composition Layer**
   - Semantic overlay built on immutable events
   - Multiple graph schemas for different views
   - Real-time graph composition from event stream

4. **Layer 3: Consumer Layer**
   - Chatbot memory and context
   - User memory stores with namespace isolation
   - Audit and accountability views
   - Semantic search and Q&A

## Canonical Technology Choices

- **Graph database**: CozoDB (embedded, graph-native) is the required implementation target for the graph composition layer.
- **Graph database fallback for evaluation only**: IndraDB may be evaluated during design validation, but the delivered implementation SHALL standardize on one embedded graph database and SHALL NOT model the knowledge graph in SQLite or any other relational SQL schema.
- **Vector database**: Qdrant is the required vector store.
- **Immutable event ledger**: append-only event log with cryptographic hashing/chaining. This is a ledger layer, not the graph database.
- **SQL databases**: optional for ancillary operational data only. Examples include users, WireGuard keys, auth/session state, configuration, and higher-level hosted applications such as CRM-style or Slack-like systems built on the platform. SQL SHALL NOT be used as the system of record for graph nodes, edges, graph queries, semantic memory, or knowledge graph composition.

## MCP Placement and Replacement Scope

- The Persistent Memory and Knowledge Graph System SHALL live in `op-cognitive-mcp` as the dedicated cognitive memory server.
- `op-mcp` SHALL remain a protocol and tool exposure layer. It SHALL NOT own persistent memory state.
- `op-mcp-aggregator` SHALL remain an upstream aggregation layer. It SHALL route to the cognitive memory server but SHALL NOT duplicate memory storage.
- `op-web` SHALL act as an API/UI consumer of the persistent memory server and SHALL NOT keep separate placeholder or shadow memory state.
- `op-chat` SHALL consume persistent memory through the new graph-backed cognitive memory interfaces and SHALL NOT retain an in-memory authoritative memory service for long-term memory.

### Mandatory Replacements
1. The new system SHALL replace OpenClaw's current file-based memory store.
2. The new system SHALL replace the current SQLite namespace/entry memory implementation in `crates/op-cognitive-mcp/src/memory_store.rs`.
3. The new system SHALL replace the current in-memory gRPC memory service in `crates/op-chat/src/orchestration/services/memory_service.rs`.
4. The new system SHALL replace the placeholder cognitive memory HTTP handlers in `crates/op-web/src/handlers/mcp.rs`.
5. Any duplicated "memory" MCP server configuration that points to legacy persistent memory implementations SHALL be removed or redirected to the graph-backed `op-cognitive-mcp` server.

## Core Requirements

### R1: Immutable Event Capture
**User Story:** As a system operator, I need all system mutations captured immutably so that we have a complete, tamper-evident audit trail.

**Acceptance Criteria:**
1. WHEN any system mutation occurs, THE system SHALL capture it as an immutable event
2. ALL events SHALL be hashed and chained for cryptographic verification
3. EACH event SHALL be vectorized at capture time for semantic search
4. THE event stream SHALL be append-only and immutable

### R2: Vectorization at Capture
**User Story:** As a system architect, I want vector embeddings generated at event capture time so they can be reused across multiple consumers.

**Acceptance Criteria:**
1. WHEN an event is captured, THE system SHALL generate vector embeddings
2. VECTOR embeddings SHALL be stored in a high-performance vector database
3. THE vector store SHALL support semantic similarity search
4. VECTOR embeddings SHALL be reusable by multiple consumers

### R3: Graph Composition Layer
**User Story:** As a knowledge engineer, I want to compose semantic graphs from the event stream so that I can build multiple specialized views.

**Acceptance Criteria:**
1. THE system SHALL use an embedded graph database for graph composition
2. THE embedded graph database SHALL be CozoDB in the delivered implementation
3. GRAPH composition SHALL be real-time and incremental
4. MULTIPLE graph views SHALL be composable from the same event stream
5. GRAPHS SHALL be rebuildable from the immutable event stream
6. CONTROL plane graph SHALL include user-system interaction events for learning
7. USER memory graph SHALL include user-only personal data with privacy isolation
8. GRAPH nodes, edges, and graph queries SHALL NOT be implemented as relational SQL tables/joins

### R4: User Memory Stores
**User Story:** As a user, I want isolated memory stores for different contexts (work, family, projects) with session persistence.

**Acceptance Criteria:**
1. USERS SHALL create and manage multiple memory stores
2. EACH memory store SHALL have namespace isolation
3. SESSION state SHALL persist across OpenClaw restarts
4. CROSS-session search SHALL include all historical sessions

### R5: System Learning from User Interactions
**User Story:** As a system operator, I want the control plane to learn from user interactions to improve system performance and user experience.

**Acceptance Criteria:**
1. CONTROL plane SHALL include anonymized user-system interaction events
2. SYSTEM SHALL learn patterns from successful/failed user interactions
3. LEARNING SHALL be privacy-preserving with user data anonymization
4. SYSTEM SHALL adapt based on user preferences and interaction patterns
5. USERS SHALL be able to opt-out of interaction learning

### R6: Semantic Search & Q&A
**User Story:** As a user, I want semantic search across all my conversations and decisions.

**Acceptance Criteria:**
1. SEMANTIC search SHALL find similar conversations across all memory stores
2. VECTOR similarity SHALL be combined with graph context
3. QUESTION-ANSWERING SHALL use both vector search and graph context
4. SEARCH results SHALL include provenance to source events

### R6: Audit and Accountability
**User Story:** As an auditor, I need cryptographic proof of all system mutations.

**Acceptance Criteria:**
1. ALL events SHALL be cryptographically hashed and chained
2. AUDIT trails SHALL be verifiable against the hash chain
3. NO event SHALL be modifiable after capture
4. ALL graph views SHALL be derivable from the immutable event stream

### R7: OpenClaw Integration
**User Story:** As an OpenClaw user, I want seamless migration from file-based to graph-based memory.

**Acceptance Criteria:**
1. EXISTING file-based memories SHALL be migratable to the new system
2. THE GUI SHALL provide memory store management
3. REAL-TIME vector search SHALL be integrated into OpenClaw
4. SESSION persistence SHALL work across OpenClaw restarts

### R8: MCP Server Integration and Decommissioning
**User Story:** As a platform maintainer, I need the new persistent memory system to fit cleanly into the existing MCP topology so that there is exactly one authoritative memory subsystem.

**Acceptance Criteria:**
1. `op-cognitive-mcp` SHALL become the authoritative persistent memory and knowledge graph server
2. `op-mcp` SHALL expose or proxy memory capabilities without storing its own long-term memory state
3. `op-mcp-aggregator` SHALL aggregate the new cognitive memory server without duplicating its state
4. THE current SQLite key/value memory store in `op-cognitive-mcp` SHALL be removed or fully replaced
5. THE current in-memory memory service in `op-chat` SHALL be removed or reduced to ephemeral session cache only
6. THE placeholder memory endpoints in `op-web` SHALL be replaced with live integrations to the graph-backed server
7. Deployment and server registration docs/configuration SHALL reflect the replacement

### R9: Session Persistence
**User Story:** As a user, I want my memory stores and conversation context to persist across sessions so I can continue where I left off.

**Acceptance Criteria:**
1. USER memory stores SHALL persist across OpenClaw restarts
2. CONVERSATION context SHALL be preserved between sessions
3. GRAPH state SHALL be serializable and restorable
4. CROSS-session search SHALL include all historical sessions
5. SESSION tokens SHALL provide continuity across restarts

### R10: Control Plane Chatbot
**User Story:** As a system operator, I want a chatbot that can answer questions about system state and history using the control plane graph.

**Acceptance Criteria:**
1. CONTROL plane chatbot SHALL answer questions about system operations
2. CHATBOT SHALL use control plane graph for context and memory
3. RESPONSES SHALL be based on system events and user-system interactions
4. CHATBOT SHALL provide audit trail references for its answers
5. PRIVACY SHALL be maintained (no user personal data in responses)

### R11: Performance and Scalability
**User Story:** As a system operator, I need the system to work efficiently on resource-constrained VPS environments.

**Acceptance Criteria:**
1. THE system SHALL operate efficiently on resource-constrained VPS instances
2. VECTOR search SHALL return results in < 100ms for typical queries
3. GRAPH composition SHALL be incremental and real-time
4. THE system SHALL scale horizontally for user and memory store growth

### R12: Plugin and Schema System
**User Story:** As a developer, I want to extend the system with new event types and schemas.

**Acceptance Criteria:**
1. PLUGINS SHALL define new event schemas
2. SCHEMA evolution SHALL be backward compatible
3. PLUGINS SHALL be isolated and sandboxed
4. SCHEMA registry SHALL be versioned and discoverable

### R13: Security and Access Control
**User Story:** As a security-conscious user, I want fine-grained access control for my memory stores.

**Acceptance Criteria:**
1. MEMORY stores SHALL be isolated by user and namespace
2. ACCESS control SHALL be fine-grained and auditable
3. ENCRYPTION SHALL protect data at rest and in transit
4. AUDIT trails SHALL track all access attempts

## Non-Functional Requirements

### Performance
- Vector search response: < 50ms P95
- Graph query response: < 100ms P95
- Event ingestion: < 10ms P99
- Memory usage: < 1GB for 1M events

### Scalability
- Support 10,000+ concurrent users
- Handle 1M+ events per day
- Scale horizontally across multiple nodes

### Reliability
- 99.9% availability
- Data durability: 99.999% (5 nines)
- Recovery time objective: < 5 minutes
- Point-in-time recovery capability

### Security
- End-to-end encryption
- Role-based access control
- Audit logging for all operations
- Regular security audits and penetration testing

## Success Metrics
- 95% of vector searches complete in < 50ms
- 99.9% system availability
- Zero data loss in event stream
- Sub-second graph composition for 1M events
- Seamless migration from file-based storage
