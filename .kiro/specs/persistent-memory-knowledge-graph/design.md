# Design Document: Persistent Memory and Knowledge Graph System

## System Architecture Overview

The Persistent Memory and Knowledge Graph System replaces OpenClaw's file-based memory store with a layered architecture built on immutable events. The system provides cryptographic audit trails, semantic search, and user memory stores with session persistence.

## 1. System Architecture

### 1.1 Layered Architecture
```
┌─────────────────────────────────────────────────────────────┐
│                    Layer 3: Consumer Layer                  │
│  ┌─────────────────────────────────────────────────────┐  │
│  │              OpenClaw GUI & Memory Stores          │  │
│  │  • Memory store management (work/family/projects)  │  │
│  │  • Semantic search interface                       │  │
│  │  • Chatbot Q&A with context                        │  │
│  │  • Audit and accountability views                  │  │
│  └─────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌─────────────────────────────────────────────────────┐  │
│  │            Layer 2: Graph Composition Layer         │  │
│  │  ┌─────────────────┐  ┌─────────────────────────┐  │  │
│  │  │  System Graph   │  │   User Memory Graphs    │  │  │
│  │  │  • Audit view   │  │  • Per-user schemas     │  │  │
│  │  │  • Chatbot mem  │  │  • Namespace isolation  │  │  │
│  │  │  • Causal graph │  │  • Session persistence  │  │  │
│  │  └─────────────────┘  └─────────────────────────┘  │  │
│  │                 │                    │              │  │
│  │          ┌──────▼──────┐     ┌──────▼──────┐       │  │
│  │          │ Vector Store│     │ Vector Store│       │  │
│  │          │ (Qdrant)    │     │ (Qdrant)    │       │  │
│  │          └─────────────┘     └─────────────┘       │  │
│  └─────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌─────────────────────────────────────────────────────┐  │
│  │            Layer 1: Vector Store Layer              │  │
│  │  • Semantic embeddings from event capture          │  │
│  │  • Reusable vector artifacts                       │  │
│  │  • Real-time similarity indexing                   │  │
│  │  • Multi-consumer vector access                    │  │
│  └─────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌─────────────────────────────────────────────────────┐  │
│  │            Layer 0: Event Capture Layer             │  │
│  │  • Immutable event capture (D-Bus, gRPC, user)     │  │
│  │  • Cryptographic hashing and chaining              │  │
│  │  • Real-time vectorization at capture              │  │
│  │  • Append-only immutable ledger                    │  │
│  └─────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

### 1.2 Component Architecture

#### 1.2.1 Layer 0: Event Capture & Immutable Ledger
- **Event Sources**: D-Bus events, gRPC calls, user interactions, system mutations
- **Cryptographic Hashing**: SHA-256 hashing of all event content
- **Hash Chaining**: Each event includes hash of previous event for tamper detection
- **Vectorization at Capture**: Embeddings generated immediately during event capture
- **Immutable Storage**: Append-only ledger with cryptographic proofs

#### 1.2.2 Layer 1: Vector Store
- **Qdrant Vector Database**: High-performance vector similarity search
- **Reusable Embeddings**: Vectors generated once at capture, reused by all consumers
- **Multi-Modal Support**: Text, structured data, and future image/audio embeddings
- **Real-time Indexing**: Vectors indexed immediately for similarity search

#### 1.2.3 Layer 2: Graph Composition Layer
- **Embedded Graph Database**: CozoDB for embedded operation with transactional support
- **Multiple Graph Schemas**: Different views over same event stream
  - **Control Plane Graph**: System events + anonymized user-system interactions
  - **User Memory Graph**: User-only personal data with privacy isolation
  - **Audit Graph**: Complete event stream for accountability
- **Real-time Composition**: Graphs built incrementally from event stream
- **Session Persistence**: Graph state serialized and restored across sessions
- **Namespace Isolation**: Separate graph partitions for work/family/projects
- **Privacy-Preserving Learning**: Control plane learns from user interactions without personal data
- **Non-SQL Constraint**: Graph nodes, edges, traversals, and pattern matching live in CozoDB. SQLite or other relational SQL stores are not used to model the graph itself.

#### 1.2.4 SQL Boundary
- SQL is outside this subsystem's storage model.
- SQL may be used elsewhere in the platform for users, WireGuard keys, auth/session records, configuration, and higher-level hosted applications such as CRM-style or Slack-like systems.
- Those SQL-backed domains are consumers of the platform, not the implementation of persistent memory, semantic memory, or the knowledge graph.

#### 1.2.5 Layer 3: Consumer Layer
- **OpenClaw GUI**: Memory store management and semantic search interface
- **Control Plane Chatbot**: System-focused Q&A using control plane graph
- **User Chatbot**: Personal memory Q&A using user memory graph  
- **Audit Views**: Cryptographic verification and accountability interfaces
- **Semantic Q&A**: Question answering over graph context and vector similarity
- **Session Management**: Session persistence and continuity across restarts

## 1.3 Fit Within Existing MCP Servers

### 1.3.1 Authoritative Server Placement
- **Authoritative service**: `op-cognitive-mcp`
- **Responsibility**:
  - Layer 0 immutable event capture and ledger coordination
  - Layer 1 vectorization and Qdrant indexing
  - Layer 2 CozoDB graph composition
  - Memory-store management APIs for OpenClaw and internal consumers
- **Deployment posture**: Remains a dedicated MCP server, but its implementation changes from SQLite key/value storage to event ledger + Qdrant + CozoDB.

### 1.3.2 Existing MCP Servers After Integration
- **`op-mcp`**: remains the unified MCP protocol adapter and generic tool server. It may expose helper tools or proxy calls into `op-cognitive-mcp`, but it does not own persistent memory state.
- **`op-mcp-aggregator`**: remains the multi-server aggregation layer. It includes `op-cognitive-mcp` as an upstream and filters/routes tools, but stores no memory state.
- **`op-mcp-proxy`**: unchanged in role; it is not part of persistent memory ownership.
- **`op-web`**: becomes a consumer of `op-cognitive-mcp` for memory store management, semantic search, chatbot context, and audit views.
- **`op-chat`**: consumes graph-backed persistent memory via cognitive server APIs; its current in-memory memory service is no longer authoritative.

### 1.3.3 Replacement and Removal Map

| Existing implementation | Current behavior | Action |
|---|---|---|
| `crates/op-cognitive-mcp/src/memory_store.rs` | SQLite namespace/key-value memory store | Remove and replace with graph-native persistent memory implementation |
| `crates/op-cognitive-mcp/src/cognitive_tools.rs` | Single `cognitive_memory` CRUD tool over SQLite | Replace with graph/event/vector-aware memory management and query tools |
| `crates/op-chat/src/orchestration/services/memory_service.rs` | In-memory gRPC memory HashMap | Remove as authoritative long-term memory; keep only ephemeral session cache if still needed |
| `crates/op-chat/src/orchestration/services/mod.rs` memory state | Holds global in-memory memory map | Remove persistent-memory responsibility |
| `crates/op-web/src/handlers/mcp.rs` memory handlers | Placeholder fake responses | Replace with live calls to `op-cognitive-mcp` |
| `docs/specs/mcp-servers.md` op-cognitive-mcp section | Describes SQLite cognitive memory server | Update to graph-native architecture |
| Deploy config entries for standalone `memory` servers | Legacy or duplicate memory routing | Remove duplication or redirect to `op-cognitive-mcp` |

### 1.3.4 Storage Responsibilities
- **CozoDB**: graph nodes, edges, graph schemas, graph traversal, namespace isolation, session graph state
- **Qdrant**: vector artifacts generated at capture time and reused by all consumers
- **Immutable event ledger**: append-only event history with hash chaining and replay
- **SQL outside subsystem**: users, WireGuard keys, auth/session data, configuration, and hosted application data models

## 2. Session Persistence & Chatbot Design

### 2.1 Session Persistence
- **Session Tokens**: Cryptographic tokens for session continuity
- **Graph State Serialization**: Complete graph state saved on session end
- **Fast Restoration**: Graph loaded from serialized state on session start
- **Cross-Session Search**: Vector search includes all historical sessions
- **Context Preservation**: Conversation context maintained across sessions

### 2.2 Control Plane Chatbot
- **Data Source**: Control plane graph (system events + user-system interactions)
- **Memory**: Uses graph context for conversation history
- **Capabilities**:
  - Answer questions about system operations
  - Provide audit trail references
  - Explain system decisions and patterns
  - Suggest optimizations based on learned patterns
- **Privacy**: No access to user-only personal data
- **Auditability**: All responses reference source events

### 2.3 User Chatbot
- **Data Source**: User memory graph (personal data only)
- **Memory**: User's personal conversations and decisions
- **Capabilities**:
  - Answer questions about personal memories
  - Find similar past decisions
  - Provide context from previous conversations
  - Search across all memory stores (work/family/projects)
- **Isolation**: Completely isolated from control plane data

### 2.4 Mutation Processing Flow
```
1. Mutation Event (chat message, system decision, user action)
   │
2. Simultaneous Processing:
   ├── Graph Database (CozoDB): Create node with properties and relationships
   ├── Vector Store: Generate embedding from content
   └── Audit Trail: Add to hash chain with cryptographic proof
   │
3. Reference Linking:
   ├── Graph node stores vector_id
   ├── Vector stores graph_node_id  
   └── Audit entry references graph node
   │
4. Transaction Commit: All operations atomic
```

### 2.2 Query Processing Flow
```
1. User Query: "Find similar decisions about database migration"
   │
2. Vector Search: Qdrant finds similar embeddings
   │
3. Graph Retrieval: Get full context from graph nodes
   │
4. Response Assembly: Complete memory with relationships
   │
5. Display: OpenClaw GUI shows results with semantic context
```

## 3. Data Model

### 3.1 System Graph Schema
```yaml
SystemGraph:
  nodes:
    - type: "audit_entry"
      properties:
        - hash: "sha256_previous_hash"
        - timestamp: "2024-01-15T14:30:00Z"
        - mutation_type: "chat_message|system_decision|user_action"
        - content_hash: "sha256_content"
        - vector_id: "vec_123"
    
    - type: "chatbot_conversation"
      properties:
        - id: "conv_456"
        - participants: ["system", "user123"]
        - context: "database_migration_discussion"
        - timestamp: "2024-01-15T14:30:00Z"
        - vector_id: "vec_789"
    
  edges:
    - type: "HAS_AUDIT_ENTRY"
      from: "system_state"
      to: "audit_entry"
    
    - type: "REFERENCES_CONVERSATION"
      from: "audit_entry"
      to: "chatbot_conversation"
```

### 3.2 User Memory Store Schema
```yaml
UserMemoryStore:
  namespace: "work"  # or "family", "projects", etc.
  nodes:
    - type: "conversation"
      properties:
        - id: "conv_user_123"
        - participants: ["user123", "colleague456"]
        - topic: "project_planning"
        - timestamp: "2024-01-15T10:00:00Z"
        - vector_id: "vec_user_456"
    
    - type: "decision"
      properties:
        - id: "dec_789"
        - conversation_id: "conv_user_123"
        - decision: "Use PostgreSQL for new project"
        - rationale: "Better JSON support than MySQL"
        - timestamp: "2024-01-15T10:15:00Z"
        - vector_id: "vec_user_789"
    
  edges:
    - type: "HAS_DECISION"
      from: "conversation"
      to: "decision"
    
    - type: "REFERENCES_DOCUMENT"
      from: "decision"
      to: "document"
```

## 4. OpenClaw Integration

### 4.1 Replacement Strategy
```rust
// Current OpenClaw memory access
let memory = openclaw::memory::file_based::get("work:project_x");

// New graph-based memory access
let memory = graph::memory_store("work")
    .namespace("project_x")
    .with_vector_search("similar decisions")
    .with_session_persistence()
    .execute();
```

### 4.1.1 Control Plane Integration Strategy
- OpenClaw and other clients talk to the memory system through `op-cognitive-mcp`
- `op-web` calls into `op-cognitive-mcp` for management/search/audit interfaces
- `op-chat` requests long-term memory from `op-cognitive-mcp` instead of its internal in-memory service
- `op-mcp` may expose discovery/proxy tooling, but does not duplicate storage

### 4.2 GUI Integration
```
OpenClaw Memory Management Interface:
┌─────────────────────────────────────────┐
│ Memory Store Manager                    │
├─────────────────────────────────────────┤
│ [Create New Memory Store]               │
│                                         │
│ Existing Memory Stores:                 │
│ • work (3 conversations, 5 decisions)   │
│ • family (12 conversations)             │
│ • projects (8 projects, 45 decisions)   │
│                                         │
│ [Search Across All Stores]              │
│ [___________________________________]   │
│                                         │
│ Search Results:                         │
│ • "Database migration decision"         │
│   (work:project_x, 2024-01-15)         │
│ • "Similar discussion about scaling"    │
│   (projects:backend, 2024-01-10)       │
└─────────────────────────────────────────┘
```

### 4.3 Session Persistence
- **Session Tokens**: Cryptographic tokens for session continuity
- **Graph State Serialization**: Complete graph state saved on session end
- **Fast Restoration**: Graph loaded from serialized state on session start
- **Cross-Session Search**: Vector search includes all historical sessions

## 5. Performance Considerations

### 5.1 Real-time Requirements
- **Mutation Processing**: < 100ms end-to-end latency

## 6. Explicit Non-Goals and Constraints

- The knowledge graph is not implemented as SQL tables and joins.
- The current SQLite namespace/key-value store is not evolved into the final graph layer; it is replaced.
- The in-memory `op-chat` memory service is not retained as long-term memory.
- Duplicate MCP memory servers with overlapping authority are not retained.
- SQL-backed user/session/WireGuard/application tables are outside this subsystem and do not weaken the non-SQL requirement for persistent memory.
- **Vector Search**: < 50ms for typical queries
- **Graph Retrieval**: < 30ms for full context retrieval
- **GUI Updates**: < 200ms for complete result display

### 5.2 Resource Optimization
- **Memory Management**: LRU caching for frequently accessed graph nodes
- **Vector Cache**: Frequently queried vectors kept in memory
- **Connection Pooling**: Efficient database and vector store connections
- **Query Optimization**: Indexed queries for common patterns

### 5.3 Scalability
- **Horizontal Scaling**: User graphs can be distributed across nodes
- **Vector Store Sharding**: By user ID for isolation
- **Read Replicas**: For high-read scenarios
- **Load Balancing**: Based on user activity patterns

## 6. Security Design

### 6.1 Access Control
- **User Isolation**: Complete graph isolation between users
- **Namespace Permissions**: Fine-grained control per memory store
- **Audit Trail Access**: Cryptographic verification required
- **API Authentication**: Token-based authentication for all operations

### 6.2 Data Protection
- **Encryption at Rest**: AES-256 for graph data storage
- **Transport Security**: TLS 1.3 for all communications
- **Hash Chain Integrity**: Cryptographic verification of audit trail
- **Key Management**: Hardware Security Module (HSM) for production

## 7. Deployment Architecture

### 7.1 Development Environment
- **Single-Node Deployment**: All components on local machine
- **Embedded Databases**: CozoDB and Qdrant embedded for simplicity
- **Mock D-Bus Events**: For testing without full D-Bus setup
- **Development GUI**: OpenClaw with development mode features

### 7.2 Production Environment
- **Containerized Deployment**: Docker containers for all components
- **High Availability**: Multiple instances with load balancing
- **Automated Backups**: Regular graph and vector store backups
- **Monitoring Stack**: Prometheus, Grafana, and structured logging

## 8. Migration Strategy

### 8.1 Phase 1: Coexistence
- Graph system runs alongside file-based system
- Read operations from both systems
- Write operations to graph system only
- Migration tool for gradual data transfer

### 8.2 Phase 2: Cutover
- All reads from graph system
- File system becomes read-only backup
- Validation of migrated data
- Performance benchmarking

### 8.3 Phase 3: Decommission
- File system archived
- Graph system as primary storage
- Rollback capability maintained for 30 days
- Final cleanup of file system artifacts

## 9. Testing Strategy

### 9.1 Unit Testing
- Schema validation tests
- Graph operation tests
- Vector embedding tests
- Performance benchmark tests

### 9.2 Integration Testing
- End-to-end mutation processing
- Cross-component consistency tests
- Session persistence tests
- Migration tool validation

### 9.3 Security Testing
- Access control validation
- Cryptographic verification tests
- Penetration testing
- Data isolation tests

## 10. Success Criteria

### 10.1 Performance Metrics
- Mutation processing: < 100ms P95 latency
- Vector search: < 50ms P95 latency
- Session restoration: < 500ms for typical user
- System availability: 99.9% uptime

### 10.2 User Experience
- Seamless migration from file-based system
- Improved search capabilities with vector search
- Session persistence across OpenClaw restarts
- Intuitive memory store management

### 10.3 System Reliability
- Zero data loss during migration
- Complete session persistence
- Cryptographic integrity of audit trail
- Graceful degradation under load

This design provides a comprehensive architecture for replacing OpenClaw's file-based memory system with a unified graph-based system that provides cryptographic audit trails, per-user memory stores, and real-time vector search across all conversations and decisions.
