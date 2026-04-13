# MISSION: Transform op-dbus into a Compliance-First Application Builder/PaaS

You are a coordinated swarm transforming the operation-dbus codebase into a natural-language-driven compliance application builder. The end state: a user says "build me a CRM that is EU AI Act compliant" or "build me a Slack-like app that is HIPAA compliant" and the system scaffolds, validates, and deploys a compliant application.

## WORKING CONTEXT

- **Primary codebase**: `operation-dbus-proto` (mounted as subvolume under lovable UI repo)
- **UI foundation**: OpenClaw (lovable UI repo) — React/TypeScript frontend with 50+ tool integrations
- **Compliance schema repos**: `~/git/*` — scan all repos for compliance schemas, reference implementations, regulatory mappings
- **Infrastructure templates**: Proxmox LXC repos — extract battle-tested schema layouts, form layouts, container templates
- **Migration tooling**: Inspector Gadget introspection/migration tool — build adapters for schema introspection and migration paths
- **Graph + Vector database**: CozoDB (embedded Datalog) — HNSW vectors, time-travel, graph algorithms, Rust-native
- **Architectural reference**: `docs/architectural-flow.md` — canonical gRPC data flow specification
- **Reference Documents**: Pre-ingested official compliance texts (EU AI Act, GDPR, HIPAA, SOC2) and CozoDB documentation are provided in the `_reference_docs/` directory. Rely on these files rather than hallucinated web searches.

## REFERENCE DOCUMENT: docs/architectural-flow.md

Agents MUST read and follow `docs/architectural-flow.md` for gRPC data flow patterns. Key architecture:

```
<object> ↔ <dbus> ↔ [gRPC] ↔ <dbus> ↔ <json-rpc>
 │ │ │
 │ │ │
System objects Translation External APIs
(Linux/D-Bus) Layer (MCP servers, etc)
```

All internal communication flows through gRPC as universal translation layer.

---

## CRITICAL: EU AI ACT COMPLIANCE IS #1 REQUIREMENT

Every architectural decision, every generated schema, every audit trail MUST satisfy EU AI Act requirements:
- Article 14: Human-in-the-loop oversight (mandatory intervention points in all AI-assisted workflows)
- Article 13: Transparency and explainability (audit trails, decision logging)
- Article 9: Risk management systems (classification, mitigation tracking)
- Article 17: Quality management (schema versioning, deployment validation)

---

## ARCHITECTURAL NON-NEGOTIABLES

These are load-bearing constraints. Do not deviate.

### Serialization: JSON ONLY
```
NO YAML. NO TOML. NO XML. JSON is the single serialization format.
- Configuration: JSON
- Schema definitions: JSON
- API payloads: JSON
- Compliance profiles: JSON
- Migration scripts: JSON
- Template definitions: JSON
```

### Single Source of Truth: Schema
```
Schema IS the contract. Everything derives from schema.
```

### Communication Stack (edge to core)
```
┌─────────────────────────────────────────────────────────────┐
│ EDGE (external/frontend) │
│ └── JSON-renderer: All frontend interfaces │
├─────────────────────────────────────────────────────────────┤
│ INTERNAL (service-to-service) │
│ └── gRPC (Tonic/Protobuf): ALL internal communication │
│ No exceptions. No REST between internal services. │
├─────────────────────────────────────────────────────────────┤
│ DATABASE LAYER │
│ └── Direct RPC calls ONLY: │
│ • JSON-RPC to OVSDB (network state) │
│ • JSON-RPC to NonNet (plugin state) │
│ NO ORM. NO CLI wrappers. Direct protocol calls. │
├─────────────────────────────────────────────────────────────┤
│ GRAPH LAYER: CozoDB (Read-Heavy Index) │
│ └── Embedded Datalog database (Rust-native) │
│ • Schema relationship graphs │
│ • Compliance dependency traversal │
│ • Entity relationship queries │
│ • Time-travel for audit history │
│ • HNSW vector indices for semantic search │
│ • Full-text search (v0.7+) │
│ • Native JSON value support │
└─────────────────────────────────────────────────────────────┘
```
**CRITICAL CLARIFICATION:** CozoDB is strictly a read-heavy index for semantic search, time-travel, and relationship graphs. Live operational state MUST remain strictly in authoritative RCP stores (OVSDB and NonNet) via JSON-RPC.

### Plugin Architecture: The Schema Pipeline
```
┌──────────────┐ ┌─────────────────────┐ ┌──────────────────┐
│ SCHEMA │ ──► │ FOOTPRINT FILTER │ ──► │ JSON-RENDERER │
│ (source) │ │ (blockchain audit) │ │ (presentation) │
└──────────────┘ └─────────────────────┘ └──────────────────┘
 │
 ▼
 ┌─────────────────────┐
 │ IMMUTABLE AUDIT │
 │ (blockchain trail) │
 └─────────────────────┘

Schema → Footprint Filter → Blockchain Audit (immutable)
 → JSON-Renderer Schema (UI generation)
```

Every plugin follows this flow. No shortcuts.

---

## REALTIME VECTORIZATION & KNOWLEDGE GRAPH PIPELINE

### Blockchain → Vector → Graph Flow

Every state change flows through this pipeline:

```
┌──────────────┐ ┌─────────────────────┐ ┌──────────────────┐
│ STATE │ ──► │ FOOTPRINT FILTER │ ──► │ BLOCKCHAIN │
│ CHANGE │ │ (audit record) │ │ (immutable) │
└──────────────┘ └─────────────────────┘ └──────────────────┘
 │
 ▼
 ┌─────────────────────┐
 │ REALTIME │
 │ VECTORIZATION │
 │ (Gemini embedding) │
 └─────────────────────┘
 │
 ▼
 ┌─────────────────────┐
 │ COZODB │
 │ (embedded Datalog) │
 │ • HNSW vectors │
 │ • Graph relations │
 │ • Time-travel │
 │ • Full-text search │
 └─────────────────────┘
 │
 ▼
 ┌─────────────────────┐
 │ MEMORY CONTEXT │
 │ (AI retrieval) │
 └─────────────────────┘
```

### CozoDB Configuration

```json
{
 "cozodb": {
 "storage_engine": "rocksdb",
 "data_path": "/var/lib/opdbus/cozo",
 "vector_config": {
 "dimensions": 3072,
 "distance_metric": "cosine",
 "hnsw_m": 16,
 "hnsw_ef_construction": 200
 },
 "embedding_model": "gemini-embedding-exp-03-07",
 "embedding_task_type": "RETRIEVAL_DOCUMENT",
 "batch_size": 50,
 "relations": {
 "entity": "code, name, type, schema_json, vector, created_at, updated_at",
 "compliance_req": "code, regulation, article, requirement_text, vector",
 "state_change": "tx_hash, entity_code, before_json, after_json, timestamp",
 "identity": "pubkey, name, permissions, created_at",
 "audit_record": "tx_hash, entity_code, action, actor_pubkey, timestamp, vector"
 },
 "graph_relations": [
 "REQUIRES",
 "SATISFIES",
 "DERIVED_FROM",
 "OWNED_BY",
 "AUDITED_BY",
 "REFERENCES"
 ],
 "time_travel": {
 "enabled": true,
 "retention_days": 365,
 "query_syntax": "?[x, y] <- relation[x, y] @ timestamp"
 }
 }
}
```

### Memory System Flow

```
User Query
 ↓
┌─────────────────────────────────────────────┐
│ DUAL-TIER RETRIEVAL │
├─────────────────────────────────────────────┤
│ │
│ TIER 1: Immediate Context │
│ └── Recent deltas from StateStore │
│ └── Fast keyword match (<10ms) │
│ └── Always available │
│ │
│ TIER 2: Semantic + Graph Context (CozoDB) │
│ └── HNSW vector search (embedded) │
│ └── Datalog graph traversal │
│ └── Time-travel audit queries │
│ └── Deep similarity (50-200ms) │
│ │
└─────────────────────────────────────────────┘
 ↓
Merged Response with Full Context
```

### Vectorization on State Change

```json
{
 "vectorization_trigger": {
 "events": [
 "schema_change",
 "entity_create",
 "entity_update",
 "compliance_validation",
 "deployment",
 "audit_record"
 ],
 "pipeline": {
 "1_capture": "StateStore delta recorded",
 "2_audit": "Blockchain footprint written",
 "3_chunk": "Content chunked (512 tokens, 64 overlap)",
 "4_embed": "Gemini embedding generated (3072 dim)",
 "5_store": "CozoDB upsert with HNSW index",
 "6_graph": "CozoDB Datalog relations updated",
 "7_index": "Full-text search indices refreshed"
 },
 "idempotency": "sha256(source + entity + chunk_index)"
 }
}
```

---

## OPENCLAW INTEGRATION (LOVABLE UI)

OpenClaw is the frontend framework providing 50+ tool integrations. All UI generation flows through OpenClaw.

### OpenClaw Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ OPENCLAW (Lovable UI) │
├─────────────────────────────────────────────────────────────┤
│ │
│ React/TypeScript Frontend │
│ ├── JSON-renderer components (schema-driven) │
│ ├── 50+ tool integrations │
│ ├── Real-time streaming log viewer │
│ ├── Compliance dashboard │
│ └── Audit trail explorer │
│ │
│ gRPC Edge Gateway │
│ ├── gRPC-Web for browser │
│ ├── JSON transcoding at edge │
│ └── Identity via WireGuard metadata │
│ │
└─────────────────────────────────────────────────────────────┘
 │
 │ gRPC (internal)
 ▼
┌─────────────────────────────────────────────────────────────┐
│ operation-dbus-proto (backend) │
│ ├── D-Bus orchestrator │
│ ├── StateStore (delta chain) │
│ ├── Blockchain audit │
│ └── MCP server integration │
└─────────────────────────────────────────────────────────────┘
```

### OpenClaw Tool Registry Integration

```json
{
 "openclaw_integration": {
 "tool_source": "operation-dbus MCP server",
 "registration_method": "gRPC service discovery",
 "tool_categories": [
 "container_management",
 "network_configuration",
 "identity_management",
 "compliance_validation",
 "audit_exploration",
 "schema_generation",
 "migration_tools"
 ],
 "ui_generation": {
 "source": "json_renderer_schema",
 "dynamic_forms": true,
 "compliance_indicators": true,
 "human_oversight_markers": true
 }
 }
}
```

### Frontend Requirements

OpenClaw MUST implement:

1. **Streaming Log Viewer** (Primary Section with Tabs)
 - Tab 1: Live Operation Logs (websocket/SSE)
 - Tab 2: System Prompt & AI Context
 - Tab 3: Audit Trail Explorer (blockchain-backed)

2. **Compliance Dashboard**
 - Active compliance profiles
 - Validation status per entity
 - Human oversight intervention points
 - EU AI Act Article 14 checkpoints

3. **Schema-Driven Forms**
 - Generated from JSON-renderer schema
 - Compliance field annotations
 - Validation rules from schema
 - Conditional visibility

4. **Identity Management UI**
 - WireGuard keypair display
 - Session history
 - State rollback controls
 - Blockchain verification

---

## INSPECTOR GADGET INTEGRATION

Inspector Gadget is the introspection/migration tool for analyzing existing systems and generating migration schemas.

### Introspection Pipeline
```
┌─────────────────────────────────────────────────────────────┐
│ INSPECTOR GADGET INTROSPECTION PIPELINE │
├─────────────────────────────────────────────────────────────┤
│ │
│ Source System │
│ ├── SQLite → sqlite_master │
│ ├── OVSDB → direct JSON-RPC schema query │
│ ├── PostgreSQL → (stub for future expansion) │
│ ├── MySQL/MariaDB → (stub for future expansion) │
│ ├── MongoDB → (stub for future expansion) │
│ └── Legacy REST APIs → (stub for future expansion) │
│ │
│ Output: Normalized op-dbus Schema │
│ ├── Entity definitions │
│ ├── Relationship mappings │
│ ├── Constraint translations │
│ ├── Index hints │
│ └── Compliance gap analysis │
│ │
└─────────────────────────────────────────────────────────────┘
```
**CRITICAL CLARIFICATION:** Implement introspection adapters fully for OVSDB and SQLite first to support the MVP transformation. Stub the remaining databases (PostgreSQL, MySQL, etc.) for future expansion but ensure the gRPC interface supports them natively.

### Adapter Interface Contract (gRPC)
```protobuf
service InspectorGadget {
 // Discover schema from external system
 rpc IntrospectSource(IntrospectRequest) returns (stream SchemaFragment);
 
 // Analyze compliance gaps in discovered schema
 rpc AnalyzeCompliance(SchemaFragment) returns (ComplianceReport);
 
 // Generate migration plan
 rpc PlanMigration(MigrationRequest) returns (MigrationPlan);
 
 // Execute migration with audit trail
 rpc ExecuteMigration(MigrationPlan) returns (stream MigrationEvent);
 
 // Validate migrated data integrity
 rpc ValidateMigration(ValidationRequest) returns (ValidationReport);
}

message IntrospectRequest {
 string source_type = 1; // sqlite, ovsdb, postgres, mysql, mongodb, rest_api
 string connection_string = 2;
 repeated string target_tables = 3; // empty = all
 ComplianceProfile compliance_target = 4;
}

message SchemaFragment {
 string entity_name = 1;
 repeated FieldDefinition fields = 2;
 repeated RelationshipDefinition relationships = 3;
 repeated ConstraintDefinition constraints = 4;
 ComplianceAnnotations compliance = 5;
}

message ComplianceAnnotations {
 bool contains_pii = 1;
 bool contains_phi = 2; // HIPAA protected health info
 bool ai_decision_point = 3; // EU AI Act oversight required
 repeated string data_categories = 4; // GDPR Article 9 special categories
 AuditRequirement audit_level = 5;
}
```

### Migration Workflow
```
1. INTROSPECT
 └── Inspector Gadget connects to source system
 └── Extracts schema, samples data patterns
 └── Identifies compliance-sensitive fields (PII, PHI, AI decision points)

2. ANALYZE
 └── Compare source schema against target compliance profile
 └── Flag gaps: missing audit fields, unencrypted PII, no consent tracking
 └── Generate compliance remediation plan

3. TRANSFORM
 └── Generate op-dbus schema from source
 └── Add compliance-required fields (audit_timestamp, consent_reference, etc.)
 └── Build gRPC service definitions
 └── Generate JSON-renderer UI schema

4. MIGRATE
 └── Stream data through footprint filter (blockchain audit every record)
 └── Transform data to new schema
 └── Validate referential integrity
 └── Generate migration audit report

5. VALIDATE
 └── Compare source record counts
 └── Verify compliance field population
 └── Test gRPC service endpoints
 └── Confirm blockchain audit trail completeness
```

### Proxmox LXC Template Extraction

Inspector Gadget MUST parse Proxmox LXC container configs to extract:

```
~/git/proxmox-lxc-*/
├── templates/
│ ├── *.conf → Container config (extract resource schemas)
│ ├── hooks/ → Lifecycle hooks (extract workflow patterns)
│ └── forms/ → UI form definitions (extract field layouts)
├── schemas/
│ ├── network.schema → Network config patterns
│ ├── storage.schema → Storage allocation patterns
│ └── security.schema → Security policy patterns
└── migrations/
 └── *.migration → Version migration scripts (extract upgrade patterns)
```

#### Extraction Targets
- **Form layouts**: Field ordering, grouping, validation rules, conditional visibility
- **Schema patterns**: Common entity structures (user, resource, permission, audit_log)
- **Workflow templates**: Approval flows, provisioning sequences, teardown procedures
- **Security policies**: RBAC patterns, network isolation rules, secret handling

---

## AGENT PHASED EXECUTION & DECOMPOSITION

To prevent race conditions and hallucinated dependencies, agents MUST execute in the following sequential phases, passing JSON artifacts forward as the source of truth for the next phase.

### PHASE 1: Codebase & Compliance Intelligence (Agent Groups 0, 1, 2)
- Read `docs/architectural-flow.md` and pre-ingested compliance texts from `_reference_docs/`.
- Parse existing `operation-dbus-proto` service definitions and map them to compliance requirements.
- Build compliance profile library (EU-AI-HIGH-RISK, HIPAA-COVERED-ENTITY, GDPR-CONTROLLER).
- Extract schema patterns and validation rules from Proxmox templates and `~/git` repos.
- Define core entities, workflows, and compliance touchpoints for Enterprise SaaS architecture patterns (e.g., CRM, ERP, HRIS).
- **OUTPUT**: `compliance_baseline.json` and `use_case_templates.json`.

### PHASE 2: Schema Architecture & Audit Pipeline (Agent Groups 3 & 5)
- Consume the outputs of Phase 1 to define the master schema format.
- Build schema-to-target compilers (Protobuf, JSON-RPC, JSON-renderer, Blockchain footprint).
- Implement the footprint filter mapping schema changes to immutable audit records.
- Ensure the schema meets EU AI Act Article 12 (automatic logging) and Article 14 (intervention points).
- **OUTPUT**: `master_schema_defs.json` and `audit_pipeline_schema.json`.

### PHASE 3: Integration & Graph Layer (Agent Groups 4 & 6)
- Consume `master_schema_defs.json` to scaffold Tonic/Protobuf gRPC service definitions.
- Build the edge gateway (gRPC → JSON) and streaming real-time state sync.
- Embed CozoDB (Rust-native) and design the Datalog schema for entity relationships, compliance dependency graphs, and time-travel audit queries.
- Implement HNSW indices for semantic search (gemini embeddings) and wire CozoDB time-travel to the blockchain audit.
- **OUTPUT**: gRPC proto definitions, CozoDB Datalog schemas, and `integration_contracts.json`.

### PHASE 4: OpenClaw UI & Natural Language Interface (Agent Groups 7 & 8)
- Consume `master_schema_defs.json` and `integration_contracts.json`.
- Wire JSON-renderer output to React components for dynamic form generation.
- Implement compliance indicators, dashboards, and the streaming log viewer (operations, AI context, audit trail).
- Build the natural language intent parser to map user requests ("Build me a CRM...") to use case templates and compliance profile activations.
- **OUTPUT**: Fully scaffolded UI components and natural language execution pipelines.

---

## COMPLIANCE PROFILE COMPOSITION

When user requests compliance, compose profiles:

```json
{
 "request": "CRM that is EU AI Act compliant",
 "composed_profile": {
 "base_use_case": "crm",
 "compliance_stack": [
 {
 "regulation": "eu_ai_act",
 "config": {
 "risk_level": "limited",
 "applicable_articles": [6, 9, 12, 13, 14, 26]
 }
 },
 {
 "regulation": "gdpr",
 "config": {
 "role": "controller",
 "special_categories": false,
 "cross_border": true
 }
 },
 {
 "regulation": "soc2",
 "config": {
 "trust_services": ["security", "availability", "confidentiality"]
 }
 }
 ],
 "generated_requirements": {
 "entities": [
 {
 "name": "customer",
 "compliance_fields": [
 "gdpr_consent_reference",
 "gdpr_legal_basis",
 "gdpr_retention_period"
 ]
 },
 {
 "name": "ai_recommendation",
 "compliance_fields": [
 "eu_ai_human_oversight_required",
 "eu_ai_explanation_available",
 "blockchain_audit_hash"
 ]
 }
 ],
 "workflows": [
 {
 "name": "customer_data_access",
 "compliance_checkpoints": [
 "audit_log_entry",
 "purpose_limitation_check"
 ]
 }
 ]
 }
 }
}
```

---

## INSPECTOR GADGET COMPLIANCE ANNOTATIONS (JSON)

```json
{
 "compliance_annotations": {
 "contains_pii": true,
 "contains_phi": false,
 "ai_decision_point": true,
 "data_categories": ["racial_ethnic_origin", "health_data"],
 "audit_level": "full",
 "retention_days": 2190,
 "encryption_required": true
 }
}
```

---

## OUTPUT EXPECTATIONS

At completion, the system should:

1. **Accept natural language**: "Build me a [X] that is [Y] compliant"
2. **Scaffold compliant application**: Full schema (JSON), services, UI, audit pipeline
3. **Enforce architectural constraints**: gRPC internal, JSON-RPC to databases, JSON-renderer to frontend
4. **Generate audit trail**: Every schema change, every deployment, every state mutation → blockchain
5. **Provide compliance evidence**: Exportable reports (JSON) for EU AI Act, HIPAA, etc.
6. **Support Inspector Gadget**: Introspect existing systems (starting with SQLite/OVSDB), generate migration schemas
7. **All configuration in JSON**: No YAML, no TOML, no XML anywhere

---

## SUCCESS CRITERIA

- [ ] Natural language command generates functional application scaffold
- [ ] All generated code uses gRPC for internal communication
- [ ] Database access is direct JSON-RPC (no ORM, no CLI)
- [ ] Every entity has blockchain audit footprint
- [ ] EU AI Act compliance checkpoints present in all AI-assisted workflows
- [ ] OpenClaw UI renders from JSON-renderer schema
- [ ] Inspector Gadget can introspect generated applications
- [ ] Compliance profile switching works (same app, different compliance targets)
- [ ] All configuration files are JSON (zero YAML/TOML/XML)
- [ ] CozoDB embedded with HNSW vector indices operational
- [ ] CozoDB Datalog graph relations synced on state changes
- [ ] CozoDB time-travel queries working for audit history
- [ ] Dual-tier retrieval working (immediate + semantic)
- [ ] Memory context reconstruction for AI sessions
- [ ] Streaming log viewer functional in OpenClaw
- [ ] gRPC data flow matches `docs/architectural-flow.md`

---

## COORDINATION NOTES

- Schema is the synchronization point. All agents read/write schema (JSON).
- Execution MUST follow the Phased Handoff sequence outlined in `AGENT PHASED EXECUTION`. Wait for prerequisite artifacts to be generated before moving to the next phase.
- Conflicts resolve toward EU compliance (most restrictive wins)
- When in doubt, add audit point (over-audit > under-audit)
- Preserve op-dbus architectural patterns (StateStore, D-Bus contracts, BTRFS deployment model)
- Read reference documents from the `_reference_docs/` folder. Do not attempt unguided web searches for massive compliance PDFs.
- `docs/architectural-flow.md` is canonical for gRPC data flow — do not deviate
- OpenClaw is the ONLY frontend — no alternative UI frameworks
- CozoDB is the single graph+vector store — embedded, no external services
- Every state change must trigger vectorization pipeline