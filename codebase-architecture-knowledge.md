# OP-DBUS Codebase Architecture Knowledge

> **Generated**: 2026-04-16 — Reference document for developers working without direct codebase access.

---

## 1. System Architecture Overview

### Component Diagram & Data Flow

```
External caller
  │
  ├── gRPC (op-grpc-bridge:50051)   ← Primary mutation ingress
  │     schema_engine.rs → SchemaEngine.mutate()
  │
  └── JSON-RPC (op-jsonrpc:7020)    ← Legacy / tooling
        → ApplyContractMutation

            │
            ▼
    D-Bus ingress (org.opdbus.StateManager)
            │
            ▼
    StateManager → apply_state / apply_state_single_plugin
            │
            ▼
    SchemaEngine (schema materialization + validation)
    "Plugin IS the schema — schema drives everything"
            │
            ▼
    Plugin mutation/apply
            │
            ├── Network state     → OVSDB (JSON-RPC socket) — authoritative RCP store
            ├── Plugin state      → NonNet DB (in-process JSON-RPC) — authoritative RCP store
            ├── Persistent state  → op-state-store (SQLite)
            ├── Audit trail       → BTRFS timing_subvol (append-only, blockchain)
            └── DR state dump     → BTRFS state_subvol (current.json)
```

### Communication Stack

| Protocol | Purpose | Port/Location |
|---|---|---|
| D-Bus (zbus) | Local IPC, StateManager, plugin signals | System/Session bus |
| gRPC (tonic) | Primary remote mutation + streaming | :50051 |
| gRPC-Web (tonic-web) | Browser frontend (axon-trace-ui) | :50051 via Nginx |
| JSON-RPC | Legacy tool execution | :7020 |
| HTTP (axum) | Web UI, MCP server | :8080, :3000, :3001 |

### Service & Port Summary

| Service | Port | Protocol | Notes |
|---|---|---|---|
| op-grpc-bridge | 50051 | gRPC/TLS | Primary mutation ingress |
| op-jsonrpc | 7020 | HTTP+JSON | Legacy / tooling |
| op-mcp | 3000 | HTTP | MCP tool server |
| op-cognitive-mcp | 3001 | HTTP | Cognitive tools, memory, CozoDB graph |
| op-web | 8080 | HTTP | UI frontend |
| Qdrant REST | 6333 | HTTP | Collection management |
| Qdrant gRPC | 6334 | gRPC | Vector ops (Rust client) |

### Network Segments

```
incusbr0 (10.149.181.0/24) — internal Incus bridge
  ├── services  10.149.181.10   OpenClaw + NextDNS
  ├── qdrant    10.149.181.190  Qdrant vector DB (BTRFS-backed)
  └── xray-server               Privacy proxy

ovsbr0 — OVS bridge (privacy + container networking)
```

---

## 2. Workspace Structure

**31 crates** in the workspace. Root binary: `op-dbus`.

### Layer 1: Core Foundation
| Crate | Purpose |
|---|---|
| `op-core` | Core types: BusType, ToolResult, ToolRequest, security, config, error |
| `op-jsonrpc` | Native JSON-RPC protocol (OVSDB client, NonNet DB) |
| `op-execution-tracker` | Execution tracking without external tools |

### Layer 2: Protocol Implementations
| Crate | Purpose |
|---|---|
| `op-grpc-bridge` | gRPC ↔ D-Bus bidirectional bridge (tonic/prost), SchemaEngine |
| `op-http` | Native HTTP server with axum |
| `op-network` | Native network operations (replaces NetworkManager) |
| `op-mcp` | MCP protocol server (stdio, HTTP, WebSocket, gRPC) |
| `op-mcp-aggregator` | MCP aggregation |
| `op-mcp-proxy` | MCP proxy |

### Layer 3: System Integration
| Crate | Purpose |
|---|---|
| `op-introspection` | Native D-Bus introspection with zbus |
| `op-tools` | Native tool registry (16,000+ D-Bus tools) |
| `op-agents` | Native agent library (70+ agents) |
| `op-dbus-model` | Database schema operations, SqlitePluginCatalog |
| `op-dbus-mirror` | 1:1 D-Bus projection of OVSDB + NonNet |

### Layer 4: State & Storage
| Crate | Purpose |
|---|---|
| `op-state` | Native state management |
| `op-state-store` | SQLite persistent storage, EventChain, PluginSchema, SchemaValidator |
| `op-cache` | BTRFS operations with NUMA-aware optimization |
| `op-blockchain` | Streaming blockchain (BTRFS subvolumes, footprints) |

### Layer 5: Intelligence & Orchestration
| Crate | Purpose |
|---|---|
| `op-chat` | Chatbot with LLM integration (reasoning engine) |
| `op-llm` | Model management and inference |
| `op-workflows` | Workflow orchestration with DAG execution |
| `op-plugins` | Plugin system with 40+ state plugins |
| `op-cognitive-mcp` | CozoDB knowledge graph + Qdrant vectors |

### Layer 6: Additional
| Crate | Purpose |
|---|---|
| `op-inspector` | Inspector Gadget — universal data introspection |
| `op-identity` | WireGuard pubkey identity + OAuth token cache |
| `op-deployment` | Deployment automation |
| `op-services` | Service management (dinit) |
| `op-gateway` | Gateway |
| `op-web` | Web server and embedded React UI |
| `op-ml` | Local ONNX inference |
| `op-dynamic-loader` | Dynamic plugin loading |

### Root Package (`op-dbus`)
- Binary: `root-package-src/main.rs`
- Library: `root-package-src/lib.rs`
- Modules: policy, chatbot, cache, mcp, mcp_live, inspector_gadget, vectorization, numa_cache, security, disaster_recovery, dependency, work_stack, error, plugin (legacy), blockchain (glue)

---

## 3. Policy Engine (Existing Compliance Code)

**Location**: `root-package-src/policy/mod.rs`

### Key Types

```rust
/// A policy object that governs system changes
pub struct Policy {
    pub policy_id: String,
    pub name: String,
    pub version: String,
    pub rules: Vec<PolicyRule>,
    pub approval_requirements: Vec<ApprovalRequirement>,
    pub time_constraints: Option<TimeConstraints>,
    pub scope: PolicyScope,
    pub active: bool,
    pub created_at_ns: u128,
    pub content_hash: String,  // SHA-256 of name+version+rules
}

pub struct PolicyRule {
    pub rule_id: String,
    pub name: String,
    pub rule_type: RuleType,
    pub condition: Value,
    pub action: RuleAction,
}

pub enum RuleType {
    ToolWhitelist(Vec<String>),
    ToolBlacklist(Vec<String>),
    ParameterConstraint { param_path: String, constraint: ParameterConstraint },
    NetworkZone(Vec<AccessZone>),
    Custom(String),
}

pub enum SecurityLevel { Public, Standard, Elevated, Restricted }

pub enum AccessZone {
    Localhost,       // 127.0.0.1, ::1
    TrustedMesh,     // 100.64.x, 10.101.x, fd...
    PrivateNetwork,  // 192.168.x, 10.x, 172.16.x
    Public,          // Everything else
}

pub struct ParameterConstraint {
    pub constraint_type: ConstraintType,
    pub value: Value,
}

pub enum ConstraintType {
    Equals, NotEquals, Contains, LessThan, GreaterThan, InList, Regex,
}

pub enum RuleAction { Allow, Deny, RequireApproval, Log, Alert(String) }
pub enum RuleResult { Allow, Deny(String), NotApplicable }

pub enum PolicyDecision {
    Allowed { policy_id: String },
    Denied { policy_id: String, rule_id: String, reason: String },
    RequiresApproval { policy_id: String, approvers: Vec<String> },
}

pub enum PolicyScope {
    Global,
    Plugin(String),
    ObjectType(String),
    Namespace(String),
    Custom(Value),
}

pub struct ComplianceProfile {
    pub profile_id: String,
    pub name: String,        // e.g. "CIS-L1", "PCI-DSS", "HIPAA"
    pub version: String,
    pub controls: Vec<ComplianceControl>,
    pub active: bool,
}

pub struct ComplianceControl {
    pub control_id: String,
    pub title: String,
    pub description: String,
    pub severity: Severity,       // Low, Medium, High, Critical
    pub check_type: ComplianceCheckType,
    pub remediation: Option<String>,
}

pub enum ComplianceCheckType {
    ObjectExists { object_type: String, filter: Value },
    PropertyValue { object_type: String, property: String, expected: Value },
    ToolExecuted { tool_name: String, within_hours: u32 },
    Custom(String),
}

pub struct ComplianceReport {
    pub profile_id: String,
    pub timestamp_ns: u128,
    pub results: Vec<ControlResult>,
    pub summary: ComplianceSummary,  // { total, passed, failed, not_applicable }
}
```

### PolicyEngine

```rust
pub struct PolicyEngine {
    state_store: Arc<dyn StateStore>,
    active_policies: parking_lot::RwLock<HashMap<String, Policy>>,
    compliance_profiles: parking_lot::RwLock<HashMap<String, ComplianceProfile>>,
}

impl PolicyEngine {
    pub fn new(state_store: Arc<dyn StateStore>) -> Self;
    pub async fn load_policies(&self) -> Result<()>;
    pub fn register_policy(&self, policy: Policy);
    pub fn check_tool_execution(&self, tool_name: &str, params: &Value) -> PolicyDecision;
    pub fn get_effective_policy(&self, tool_name: &str) -> Option<Policy>;
    pub fn register_compliance_profile(&self, profile: ComplianceProfile);
    pub async fn check_compliance(&self, profile_id: &str) -> Result<ComplianceReport>;
}
```

### Gaps in Current Implementation
- `evaluate_control()` is stubbed — always returns `Pass` or `NotApplicable`
- No actual state querying in compliance checks (needs real StateStore integration)
- Policies loaded from hardcoded defaults, not persistent storage
- No CozoDB/graph-based policy storage or querying
- Missing: compliance dashboard gRPC service, continuous monitoring, drift detection

---

## 4. EventChain & Audit Trail

**Location**: `crates/op-state-store/src/event_chain.rs`

### Key Types

```rust
pub enum OperationType {
    ApplyImmutableWrapper, ApplyTunablePatch, Migrate, Reconcile,
    EmitSignal, PropertyGet, PropertySet, MethodCall,
    CreateSnapshot, Import, Export, Custom(String),
}

pub enum Decision { Allow, Deny }

/// Autonomy provenance — how an action came to be
pub enum ActionOrigin {
    Instructed { by: String, session_id: Option<String>, prompt_ref: Option<String> },
    Autonomous { reasoning_ref: Option<String>, confidence: Option<f32> },
    Reactive { trigger: String },
}

pub enum DenyReason {
    TagLock { tag: String, wrapper_id: String },
    ConstraintFail { constraint: String, message: String },
    CapabilityMissing { capability: String },
    SchemaValidation { errors: Vec<String> },
    ReadOnlyViolation { field: String },
    Custom { reason: String },
}

pub struct ChainEvent {
    pub event_id: u64,                          // Monotonic
    pub prev_hash: String,                      // Hash chain link
    pub event_hash: String,                     // H(prev_hash || canonical_payload)
    pub timestamp: DateTime<Utc>,
    pub actor_id: String,
    pub capability_id: Option<String>,
    pub plugin_id: String,
    pub schema_version: String,
    pub op: OperationType,
    pub target: String,                         // Object path / selector
    pub tags_touched: Vec<String>,              // Computed from schema
    pub decision: Decision,
    pub deny_reason: Option<DenyReason>,
    pub input_patch_hash: String,
    pub result_effective_hash: Option<String>,
    pub db_delta_hash: Option<String>,
    pub snapshot_ref: Option<String>,
    pub action_origin: Option<ActionOrigin>,    // Instructed/Autonomous/Reactive
    pub user_id: Option<String>,
    pub conversation_id: Option<String>,
}

pub struct EventBatch {
    pub batch_root: String,              // Merkle root
    pub first_event_id: u64,
    pub last_event_id: u64,
    pub prev_batch_root: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub event_count: usize,
}

pub struct MerkleProof {
    pub event_hash: String,
    pub event_id: u64,
    pub siblings: Vec<(String, bool)>,   // (hash, is_right)
    pub root: String,
}

pub struct StateSnapshot {
    pub snapshot_id: String,
    pub at_event_id: u64,
    pub plugin_id: String,
    pub schema_version: String,
    pub stub_hash: String,
    pub immutable_wrappers_hash: String,
    pub tunable_patch_hash: String,
    pub effective_hash: String,
    pub timestamp: DateTime<Utc>,
    pub state: Value,
}

pub struct TagImmutabilityProof {
    pub tag: String,
    pub is_immutable: bool,
    pub violations: Vec<u64>,
    pub total_events_checked: usize,
}

pub struct ChainVerificationResult {
    pub valid: bool,
    pub events_verified: usize,
    pub batches_verified: usize,
    pub errors: Vec<String>,
}
```

### EventChain

```rust
pub struct EventChain {
    events: Vec<ChainEvent>,
    batches: Vec<EventBatch>,
    snapshots: HashMap<String, StateSnapshot>,
    config: ChainConfig,  // { batch_size: 1000, auto_batch: true }
    genesis_hash: String,
}

impl EventChain {
    pub fn new(config: ChainConfig) -> Self;
    pub fn last_hash(&self) -> &str;
    pub fn next_event_id(&self) -> u64;
    pub fn append(&mut self, event: ChainEvent) -> &ChainEvent;
    pub fn record(&mut self, actor_id, plugin_id, schema_version, op, target,
                  tags_touched, decision, input_patch) -> &ChainEvent;
    pub fn create_batch(&mut self) -> Option<&EventBatch>;
    pub fn create_snapshot(&mut self, plugin_id, schema_version, state) -> &StateSnapshot;
    pub fn verify_chain(&self) -> ChainVerificationResult;
    pub fn events_touching_tag(&self, tag: &str) -> Vec<&ChainEvent>;
    pub fn events_for_plugin(&self, plugin_id: &str) -> Vec<&ChainEvent>;
    pub fn prove_tag_immutability(&self, tag: &str) -> TagImmutabilityProof;
    pub fn events(&self) -> &[ChainEvent];
    pub fn batches(&self) -> &[EventBatch];
    pub fn get_snapshot(&self, id: &str) -> Option<&StateSnapshot>;
}
```

### Hash Function
- Uses **md5** for event hashes and merkle proofs (via `md5::compute`)
- Canonical JSON serialization via `canonicalize_json()` (sorts keys, normalizes numbers)

---

## 5. Schema System

**Location**: `crates/op-state-store/src/plugin_schema.rs`

### Core Types

```rust
pub const DEFAULT_SCHEMA_DIALECT: &str = "https://json-schema.org/v1/2026";

pub enum FieldType {
    String, Integer, Float, Boolean,
    Array(Box<FieldType>),
    Object(HashMap<String, FieldSchema>),
    Enum(Vec<String>),
    Any,
}

pub struct FieldSchema {
    pub field_type: FieldType,
    pub required: bool,
    pub description: String,
    pub default: Option<Value>,
    pub example: Option<Value>,
    pub constraints: Vec<Constraint>,
    pub read_only: bool,
    pub read_only_when: Option<ReadOnlyCondition>,
}

pub struct ReadOnlyCondition {
    pub property: String,  // e.g. "status"
    pub value: String,     // e.g. "locked"
}

pub enum Constraint {
    Min { value: f64 },
    Max { value: f64 },
    Pattern { regex: String },
    OneOf { values: Vec<Value> },
    RequiresField { field: String },
    Custom { validator: String },
}

pub struct PluginSchema {
    pub name: String,
    pub category: String,
    pub version: String,
    pub description: String,
    pub fields: HashMap<String, FieldSchema>,
    pub dependencies: Vec<String>,
    pub example: Option<Value>,
    pub immutable_paths: Vec<String>,
    pub tags: Vec<String>,         // ["immutable"] = fully immutable
    pub dialect: String,
}
```

### PluginSchema Methods
```rust
impl PluginSchema {
    pub fn builder(name: &str) -> PluginSchemaBuilder;
    pub fn validate(&self, state: &Value) -> ValidationResult;
    pub fn generate_template(&self) -> Value;
    pub fn to_json_schema(&self) -> Value;           // JSON Schema 2026
    pub fn to_json_schema_draft07(&self) -> Value;   // Deprecated
    pub fn to_contract_json_schema(&self) -> Value;  // Full contract schema
}
```

### SchemaRegistry / SchemaCatalog

```rust
pub type SchemaCatalog = SchemaRegistry;

pub struct SchemaRegistry {
    schemas: HashMap<String, PluginSchema>,
    categorized: HashMap<String, HashMap<String, StoredSchemaCopies>>,
    meta_schemas: HashMap<String, Value>,
    spec_base_path: Option<PathBuf>,
}

impl SchemaRegistry {
    pub fn empty() -> Self;
    pub fn new() -> Self;
    pub fn with_builtin_schemas() -> Self;
    pub fn register(&mut self, schema: PluginSchema);
    pub fn get(&self, name: &str) -> Option<&PluginSchema>;
    pub fn list(&self) -> Vec<&str>;
    pub fn categories(&self) -> Vec<&str>;
    pub fn validate(&self, plugin_name: &str, state: &Value) -> Option<ValidationResult>;
    pub fn export_all(&self) -> HashMap<String, Value>;
    pub fn export_all_contract(&self) -> HashMap<String, Value>;
}
```

### Built-in Schemas (40 plugins)
`adc, agent_config, config, dnsresolver, endpoint, full_system, gcloud_adc, hardware, keypair, keyring, login1, mcp, openflow_obfuscation, ovsdb_bridge, packagekit, pcidecl, privacy, proxmox, proxy_server, service, sess_decl, software, users, web_ui, wireguard, lxc, incus, incus-wireguard-ingress, incus-xray-reality-client, incus-xray-reality-server, net, rtnetlink, openflow, dinit, privacy_router, privacy_routes, netmaker, directory, cms`

### SchemaValidator

```rust
pub struct SchemaValidator {
    validators: HashMap<String, jsonschema::Validator>,
}

impl SchemaValidator {
    pub fn new() -> Self;
    pub fn validate_schema_against_meta(&mut self, schema, catalog) -> Result<ValidationReport>;
    pub fn validate_instance(&mut self, schema, instance) -> Result<ValidationReport>;
    pub fn expand_property_dependencies(schema: &Value) -> Result<Value>;
}

pub struct ValidationReport {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub dialect: String,
    pub content_hash: Option<String>,
}
```

### Contract Schema Structure
Each plugin produces a full contract document with sections:
- `schema_version`, `plugin`, `object_type`, `object_id`
- `stub` — discovery metadata (system_id, source, source_ref, discovered_at)
- `immutable` — creation-time fields (created_at, created_by_plugin, identity_keys, provider)
- `tunable` — the plugin's actual JSON Schema (2026 dialect)
- `observed` — runtime observations (last_observed_at, status, drift_detected, metrics)
- `meta` — dependencies, include_in_recovery, recovery_priority, category, sensitivity, tags, enabled
- `semantic_index` — embedding paths, chunking strategy, redaction config
- `privacy_index` — redaction rules, secret_paths, pii_paths, hash_salt_ref

---

## 6. Blockchain Audit

**Location**: `crates/op-blockchain/`

### Key Types

```rust
pub struct PluginFootprint {
    pub plugin_id: String,
    pub operation: String,
    pub timestamp: u64,
    pub data_hash: String,      // SHA-256 of metadata
    pub content_hash: String,   // SHA-256 of plugin_id:operation:timestamp:data_hash
    pub metadata: HashMap<String, OwnedValue>,
    pub vector_features: Vec<f32>,  // 64-dim heuristic or 1024-dim transformer
}

pub struct BlockEvent {
    pub timestamp: u64,
    pub category: String,
    pub action: String,
    pub data: OwnedValue,
    pub hash: String,
    pub vector: Vec<f32>,
}

pub struct StreamingBlockchain {
    base_path: PathBuf,
    timing_subvol: PathBuf,    // Audit trail (immutable)
    vector_subvol: PathBuf,    // ML embeddings
    state_subvol: PathBuf,     // Current state (DR)
    snapshot_interval: SnapshotInterval,
    retention_policy: RetentionPolicy,
    // ...
}
```

### StreamingBlockchain API
```rust
impl StreamingBlockchain {
    pub async fn new(base_path) -> Result<Self>;
    pub async fn add_footprint(&self, footprint: PluginFootprint) -> Result<String>;
    pub async fn add_footprints_batch(&self, footprints) -> Result<Vec<String>>;
    pub async fn update_current_state(&self, state) -> Result<()>;  // DR
    pub async fn update_plugin_state(&self, plugin_name, state) -> Result<()>;
    pub async fn read_current_state(&self) -> Result<OwnedValue>;
    pub async fn start_footprint_receiver(&self, receiver) -> Result<()>;
    pub async fn stream_to_remote(&self, snapshot_name, remote_id) -> Result<()>;
    pub async fn stream_to_all_remotes(&self, snapshot_name) -> Result<()>;
    pub async fn list_state_snapshots(&self) -> Result<Vec<(String, String)>>;
    pub async fn rollback_to_snapshot(&self, snapshot_name) -> Result<PathBuf>;
}
```

### BTRFS Subvolume Architecture
```
/var/lib/op-dbus/cache/
├── timing/          ← Append-only audit ledger blocks
├── vectors/         ← ML embedding vectors
├── state/           ← DR state (current.json + plugins/*.json)
└── snapshots/       ← BTRFS read-only snapshots
    ├── SNP-state-000001
    ├── timing-{block_hash}
    └── vectors-{block_hash}
```

### Snapshot & Retention
- `SnapshotInterval`: PerOperation, EveryMinute, Every5/15/30Minutes, Hourly, Daily, Weekly
- `RetentionPolicy`: { hourly: 5, daily: 5, weekly: 5, quarterly: 5 }
- Rolling windows with auto-pruning
- Pinned snapshots (for incremental `btrfs send`) never deleted

### Replication
- `btrfs send -p <parent> <child> | ssh remote btrfs receive <path>` (incremental)
- `SendState` tracks per-remote last_sent_snapshot
- Parent snapshot pinned until all remotes confirm receipt

---

## 7. Plugin System

**Location**: `crates/op-plugins/`

### Plugin Trait

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn version(&self) -> &str;
    async fn get_state(&self) -> Result<Value>;
    async fn get_desired_state(&self) -> Result<DesiredState>;
    async fn set_desired_state(&self, desired: DesiredState) -> Result<()>;
    async fn apply_state(&self) -> Result<Vec<StateChange>>;
    async fn diff(&self) -> Result<Vec<StateChange>>;
    async fn validate(&self, config: &Value) -> Result<ValidationResult>;
    fn capabilities(&self) -> PluginCapabilities;
    fn metadata(&self) -> PluginMetadata;
    async fn handle_command(&self, command: &str, args: Value) -> Result<Value>;
    async fn initialize(&mut self, context: PluginContext) -> Result<()>;
    async fn cleanup(&mut self) -> Result<()>;
    fn state_hash(&self) -> String;
    fn as_any(&self) -> &dyn Any;
}
```

### FeatureSchema (with immutability)

```rust
pub struct FeatureSchema {
    pub feature_type: String,
    pub version: String,
    pub config: Value,
    pub tags: Vec<String>,            // ["immutable", "core", "optional"]
    pub immutable_paths: Vec<String>, // ["/metadata/id"]
}

impl FeatureSchema {
    pub fn is_fully_immutable(&self) -> bool;  // tags contains "immutable"
    pub fn is_path_immutable(&self, path: &str) -> bool;
}
```

### PluginCapabilities
```rust
pub struct PluginCapabilities {
    pub can_read: bool,
    pub can_write: bool,
    pub can_delete: bool,
    pub supports_dry_run: bool,
    pub supports_rollback: bool,
    pub supports_transactions: bool,
    pub requires_root: bool,
    pub supported_platforms: Vec<String>,
}
```

### PluginRegistry / PluginCatalog

```rust
pub struct PluginRecord {
    pub name: String,
    pub plugin: Arc<dyn StatePlugin>,
    pub storage_path: PathBuf,
    pub change_count: u64,
    pub schema: Option<PluginSchema>,
    pub dbus_path: String,
}

pub struct PluginRegistry {
    plugins: Arc<AsyncRwLock<HashMap<String, PluginRecord>>>,
    schema_catalog: Arc<RwLock<SchemaCatalog>>,
    schema_catalog_store: Option<Arc<SqlitePluginCatalog>>,
    base_path: PathBuf,
}

impl PluginRegistry {
    pub fn new(base_path) -> Self;
    pub fn with_schema_catalog(base_path, schema_catalog) -> Self;
    pub fn with_schema_catalog_and_store(base_path, schema_catalog, store) -> Self;
    pub async fn register(&self, plugin: Arc<dyn StatePlugin>) -> Result<()>;
    pub async fn get(&self, name: &str) -> Option<Arc<dyn StatePlugin>>;
    pub async fn get_record(&self, name: &str) -> Option<Arc<PluginRecord>>;
    pub async fn list_all(&self) -> Vec<Arc<PluginRecord>>;
}
```

### Registration Flow
1. Build canonical plugin document from plugin-owned schema
2. Persist document to SQLite catalog store (`SqlitePluginCatalog`)
3. Update in-memory schema catalog for local reference lookups
4. Create BTRFS subvolume for plugin storage

### State Plugins (40+)
Located in `crates/op-plugins/src/state_plugins/`:
adc, agent_config, config, dinit, dnsresolver, endpoint, full_system, gcloud_adc, hardware, incus, keypair, keyring, login1, lxc, mcp, net, netmaker, openflow, openflow_obfuscation, ovsdb_bridge, packagekit, pcidecl, privacy, privacy_router, privacy_routes, proxmox, proxy_server, rtnetlink, schema_contract, service, sessdecl, software, systemd, systemd_networkd, users, web_ui, wireguard

---

## 8. gRPC Services

**Location**: `crates/op-grpc-bridge/`

### Proto Modules (compiled via tonic)
```rust
pub mod proto {
    tonic::include_proto!("operation.v1");        // Core services
    pub mod mail { tonic::include_proto!("operation.mail.v1"); }
    pub mod privacy { tonic::include_proto!("operation.privacy.v1"); }
    pub mod registration { tonic::include_proto!("operation.registration.v1"); }
    pub mod registry { tonic::include_proto!("operation.registry.v1"); }
    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("operation_descriptor");
}
```

### Services Registered on gRPC Server

| Service | Proto Package | Key RPCs |
|---|---|---|
| **StateSync** | operation.v1 | Subscribe (stream), Mutate, GetState, BatchMutate |
| **PluginService** | operation.v1 | ListPlugins, GetSchema, CallMethod, GetProperty, SetProperty, SubscribeSignals (stream) |
| **EventChainService** | operation.v1 | GetEvents, SubscribeEvents (stream), VerifyChain, GetProof, ProveTagImmutability, GetSnapshot, CreateSnapshot |
| **OvsdbMirror** | operation.v1 | ListDbs, GetSchema, Transact, Monitor (stream), Echo, DumpDb, GetBridgeState |
| **RuntimeMirror** | operation.v1 | GetSystemInfo, ListServices, GetService, StreamMetrics (stream), ListInterfaces, GetNumaTopology |
| **ComponentRegistry** | operation.registry.v1 | Discover, Watch (stream), GetComponent |
| **MailService** | operation.mail.v1 | SendEmail, GetInbox, GetMessage, GetMailStatus, ListMailAccounts, AdminMailAction, CheckMailServer |
| **PrivacyNetworkService** | operation.privacy.v1 | EnsurePrivacyNetwork, GetNetworkStatus, ProvisionUser, GetPrivacyWireGuardConfig, ManageComponent, GetNetworkTopology, HealthCheck, ConfigurePacketRouting, GenerateWireGuardKeyPair |
| **RegistrationService** | operation.registration.v1 | SendMagicLink, VerifyMagicLink, RegisterUser, GetUserStatus, ListUsers, GetWireGuardConfig, AdminUserAction |

### gRPC Server Configuration
- `accept_http1(true)` — enables gRPC-Web
- All services wrapped with `tonic_web::enable()`
- tonic-reflection for grpcurl/service discovery
- tonic-health for liveness probes
- Binds to `0.0.0.0:50051`

### SchemaEngine (Mutation Coordinator)

```rust
pub struct SchemaEngine {
    pub event_chain: Arc<RwLock<EventChain>>,
    change_tx: broadcast::Sender<StateChange>,
    state_cache: Arc<RwLock<HashMap<String, OwnedValue>>>,
    pub dbus_connection: Arc<OnceCell<Connection>>,
    pub ovsdb: Arc<OvsdbClient>,
    pub nonnet: Arc<NonNetDb>,
}

impl SchemaEngine {
    pub fn new(event_chain, ovsdb, nonnet) -> Self;
    pub async fn mutate(&self, plugin_id, object_path, change_type,
                        member_name, value, actor_id, capability_id) -> Result<MutationResult>;
    pub async fn process_authoritative_change(&self, ...) -> Result<StateChange>;
    pub async fn get_state(&self, plugin_id: &str) -> Option<OwnedValue>;
    pub async fn start(self: Arc<Self>) -> Result<()>;  // Subscribe to NonNet + OVSDB
    pub fn change_tx(&self) -> broadcast::Sender<StateChange>;
}

pub struct StateChange {
    pub change_id: String,
    pub event_id: u64,
    pub plugin_id: String,
    pub object_path: String,
    pub change_type: ChangeType,
    pub member_name: Option<String>,
    pub old_value: Option<OwnedValue>,
    pub new_value: OwnedValue,
    pub tags_touched: Vec<String>,
    pub event_hash: String,
    pub timestamp: DateTime<Utc>,
    pub actor_id: String,
    pub source: ChangeSource,  // DBus | Grpc | Internal
}

pub enum ChangeType {
    PropertySet, PropertyDelete, MethodCall, Signal,
    ObjectAdded, ObjectRemoved, SchemaMigration,
}

pub struct MutationResult {
    pub success: bool,
    pub event_id: u64,
    pub event_hash: String,
    pub result: Option<OwnedValue>,
    pub error: Option<MutationError>,
}
```

### Mutation Flow
1. **OVSDB path** (plugin_id == "net" or path contains "/ovsdb/"): Route to `OvsdbClient` methods (create_bridge, add_port, set_bridge_property)
2. **NonNet/Generic path**: Update state cache, persist via `NonNetDb.update_table()`
3. **Record**: Write to EventChain, update state cache
4. **Broadcast**: Send `StateChange` via broadcast channel to all gRPC subscribers

---

## 9. Core Types

**Location**: `crates/op-core/src/`

### types.rs
```rust
pub enum BusType { System, Session }

pub struct ServiceInfo { name, bus_type, activatable, active, pid, uid }
pub struct ObjectInfo { path, interfaces: Vec<InterfaceInfo>, children }
pub struct InterfaceInfo { name, methods, signals, properties }
pub struct MethodInfo { name, in_args, out_args, annotations }
pub struct SignalInfo { name, args }
pub struct PropertyInfo { name, signature, access: PropertyAccess }
pub struct ArgInfo { name, signature, direction: ArgDirection }

pub enum PropertyAccess { Read, Write, ReadWrite }
pub enum ArgDirection { In, Out }

pub struct ToolDefinition { name, description, input_schema, schema_version, category, tags, namespace }
pub struct ToolRequest { id, tool_name, arguments, timeout_ms }
pub struct ToolResult { id, success, content, error, execution_time_ms }

pub struct AgentDefinition { id, name, description, capabilities, tools, model, config }
pub enum AgentStatus { Idle, Running, Paused, Error, Stopped }

pub struct ChatMessage { id, role: ChatRole, content, timestamp, tool_calls, metadata }
pub enum ChatRole { User, Assistant, System, Tool }
pub struct ToolCall { id, tool_name, arguments, result }

pub struct HealthStatus { healthy, version, uptime_secs, components: HashMap<String, ComponentHealth> }
pub enum ComponentStatus { Healthy, Degraded, Unhealthy, Unknown }

pub struct ObjectSchemaRef { object_type, namespace, path, schema_hash }
```

### security.rs
```rust
pub enum SecurityLevel { Public, Standard, Elevated, Restricted }
pub enum AccessZone { Localhost, TrustedMesh, PrivateNetwork, Public }
```

### error.rs
```rust
pub enum Error { ... }
pub type Result<T> = std::result::Result<T, Error>;
```

---

## 10. Inspector Gadget

**Location**: `crates/op-inspector/src/`

### InspectorGadget (wrapper)
```rust
pub struct InspectorGadget {
    introspection: Arc<IntrospectionService>,
}
```

### IntrospectiveGadget (universal object inspector)
```rust
pub struct IntrospectiveGadget {
    knowledge_base: Arc<RwLock<KnowledgeBase>>,
    parsers: Arc<RwLock<HashMap<String, Arc<dyn ObjectParser>>>>,
}

impl IntrospectiveGadget {
    pub async fn new(knowledge_base) -> Result<Self>;
    pub async fn inspect_object(&self, input: InspectionInput) -> Result<InspectionResult>;
    pub async fn inspect_docker_container(&self, name: &str) -> Result<ContainerInspectionWithKnowledge>;
}
```

Built-in parsers: JSON, XML, YAML, Docker, Binary, Text, Auto

### GCloud Parser
```rust
pub struct GCloudSchema {
    pub schema_version: String,
    pub gcloud_version: String,
    pub account: Option<String>,
    pub hierarchy: GCloudCommand,
    pub statistics: GCloudStats,  // total_groups, total_commands, total_flags
}

pub struct GCloudCommand {
    pub name: String,
    pub full_path: String,
    pub description: String,
    pub is_group: bool,
    pub flags: Vec<GCloudFlag>,
    pub positional_args: Vec<GCloudArg>,
    pub subcommands: HashMap<String, GCloudCommand>,
}

pub async fn introspect_gcloud(max_depth: usize) -> Result<GCloudSchema>;
```

---

## 11. Frontend Architecture (axon-trace-ui)

### Tech Stack
- **Framework**: React + TypeScript
- **Bundler**: Vite
- **State**: Zustand (event-store.ts)
- **Data fetching**: @tanstack/react-query
- **UI**: Radix UI primitives + shadcn/ui + Tailwind CSS
- **gRPC-Web**: @protobuf-ts/grpcweb-transport (binary framing)
- **Auth**: @supabase/supabase-js
- **Visualization**: recharts, react-flow, react-force-graph-2d
- **JSON rendering**: @json-render/core + @json-render/react

### Routes (App.tsx)
| Path | Page |
|---|---|
| `/` | OverviewPage |
| `/chat` | ChatPage |
| `/tools` | ToolsPage |
| `/agents` | AgentsPage |
| `/models` | RoutableModelsPage |
| `/llm` | LlmPage |
| `/services` | ServicesPage |
| `/security` | SecurityPage |
| `/config` | ConfigPage |
| `/inspector` | InspectorPage |
| `/state` | StatePage |
| `/logs` | LogsPage |
| `/workflows` | WorkflowsPage |
| `/orchestration` | OrchestrationPage |
| `/skills` | SkillsPage |
| `/containers` | ContainersPage |
| `/privacy-network` | PrivacyNetworkPage |
| `/ovs` | OpenSwitchPage |
| `/openflow` | OpenFlowPage |
| `/knowledge` | KnowledgePage |
| `/grpc` | GrpcDiagnosticsPage |
| `/accountability` | AccountabilityPage |
| `/btrfs` | BtrfsPage |
| `/data-stores` | DataStoresPage |
| `/embedding` | EmbeddingPipelinePage |

### gRPC-Web Client Services (src/grpc/client.ts)

All services use binary gRPC-Web framing over `VITE_GRPC_BASE_URL` (default: `https://dashboard.3tched.com`).

**Bridge Services (operation.v1):**
- `stateSync` — subscribe(), mutate(), getState(), batchMutate()
- `pluginService` — listPlugins(), getSchema(), callMethod(), getProperty(), setProperty(), subscribeSignals()
- `eventChainService` — getEvents(), subscribeEvents(), verifyChain(), getProof(), proveTagImmutability(), getSnapshot(), createSnapshot()
- `ovsdbMirror` — listDbs(), getSchema(), transact(), monitor(), echo(), dumpDb(), getBridgeState()
- `runtimeMirror` — getSystemInfo(), listServices(), getService(), streamMetrics(), listInterfaces(), getNumaTopology()
- `componentRegistry` — discover(), watch(), getComponent()

**Domain Services:**
- `mailService` — sendEmail(), getInbox(), getMessage(), getMailStatus(), listMailAccounts(), adminMailAction(), checkMailServer()
- `privacyService` — ensurePrivacyNetwork(), getNetworkStatus(), provisionUser(), getPrivacyWireGuardConfig(), manageComponent(), getNetworkTopology(), healthCheck(), configurePacketRouting(), generateWireGuardKeyPair()
- `registrationService` — sendMagicLink(), verifyMagicLink(), registerUser(), getUserStatus(), listUsers(), getWireGuardConfig(), adminUserAction()
- `serviceManager` — start(), stop(), restart(), reload(), create(), delete(), get(), list(), enable(), disable(), watchStatus()
- `mcpService` — health(), initialize(), listTools(), callTool(), callToolStreaming(), subscribe()
- `accountabilityService` — searchEpisodes(), getEpisode(), getCollectionStats(), chatWithContext(), subscribeEpisodes(), getPiiPolicy()
- `blockchainService` — getFootprints(), verifyChain(), getEmbeddingQueueStatus(), getQdrantRoles()
- `btrfsService` — getSubvolumes(), getSnapshots(), getSendState(), getDrStatus()
- `personaService` — listPersonas(), getPersona(), createPersona(), updatePersona(), deletePersona(), listAgentRoutes()
- `dataStoreService` — getDataStores(), getStoreDetail()
- `embeddingService` — getQueue(), getWorkerStatus(), previewEmbeddingText(), getChannelDiagnostics()

### State Management (Zustand)

```typescript
interface EventStore {
  connected: boolean;
  health: HealthSnapshot | null;
  logs: LogEntry[];           // max 1000
  agents: Agent[];
  events: EventLogEntry[];    // max 500
  lastError: string | null;
  eventCounts: Record<string, number>;
  latestState: Record<string, unknown>;
  latestStats: Record<string, unknown> | null;
  // + setters for each field
}
```

---

## 12. Existing CozoDB Integration

**Crate**: `op-cognitive-mcp` (separate MCP server on port 3001)

**Dependency**: `cozo = { version = "0.7.6", features = ["storage-sled", "rayon"] }`

### KnowledgeGraphStore

```rust
pub struct KnowledgeGraphStore {
    db: Arc<Mutex<DbInstance>>,
}

impl KnowledgeGraphStore {
    pub fn new_in_memory() -> Result<Self>;
    pub fn new_on_disk(path) -> Result<Self>;
    pub fn project_footprint(&self, block_hash: &str, footprint: &PluginFootprint) -> Result<()>;
    pub fn store_namespace(&self, name: &str, kind: &str) -> Result<()>;
    pub fn store_event_link(&self, source, relation, target) -> Result<()>;
    pub fn list_events(&self, limit) -> Result<Vec<ProjectedEvent>>;
    pub fn list_links(&self, limit) -> Result<Vec<EventLink>>;
    pub fn list_namespaces(&self) -> Result<Vec<NamespaceNode>>;
    pub fn stats(&self) -> Result<GraphStats>;
    pub fn run_script(&self, script, params, mutability) -> Result<NamedRows>;
}
```

### CozoDB Schema
```
:create events {
  block_hash: String =>
  plugin_id: String, operation: String, timestamp: Int,
  content_hash: String, data_hash: String,
  namespace: String, payload_json: String
}

:create event_links {
  source: String, relation: String => target: String
}

:create namespaces {
  name: String => kind: String
}
```

**Role**: Projects immutable blockchain footprints into a Cozo graph for knowledge-graph queries. The graph is a **projection** from the append-only blockchain — it can always be rebuilt from the ledger.

---

## 13. JSON Schemas

**Location**: `schemas/`

### opdbus-plugin-schema.json
Generic plugin schema (draft-07): requires `name`, `version`, `plugin_type` (service/network/storage/system/custom). Includes `capabilities`, `dependencies`, `schema`.

### service-plugin-schema.json
Service definition schema (draft-07): requires `name`, `exec_start.program`. Includes `exec_stop`, `working_dir`, `user`, `group`, `depends_on`, `environment`, `enabled`, `lifecycle` (last_active, days_since_active, is_orphaned).

### incus-wireguard-ingress.json
WireGuard ingress container schema: `container` (image, profiles, devices) + `wireguard` (listen_port, private_key_env, peers with public_key/allowed_ips/endpoint).

### incus-xray-reality-client.json / incus-xray-reality-server.json
XRAY Reality protocol containers (similar structure to WG ingress).

### jsonschema-meta.json
Empty object `{}` — placeholder.

---

## 14. Key Integration Points for Compliance Engine

### Where to Wire in CozoDB
- **Existing**: `crates/op-cognitive-mcp/src/graph_store.rs` — already has CozoDB 0.7.6 with sled storage
- **Compliance extension**: Add compliance-specific Cozo relations (policies, controls, audit results) alongside existing `events`, `event_links`, `namespaces`
- **Query surface**: Use Cozo's Datalog for compliance graph queries (e.g., "which policies were active when event X happened?")

### Where to Add Compliance Validation
- **PolicyEngine** (`root-package-src/policy/mod.rs`): Current `evaluate_control()` is stubbed — replace with real StateStore + CozoDB queries
- **SchemaEngine.mutate()** (`crates/op-grpc-bridge/src/schema_engine.rs`): Insert policy check **before** writing to authoritative stores; record decision in `ChainEvent.decision`
- **EventChain** (`crates/op-state-store/src/event_chain.rs`): Already has `Decision::Allow/Deny` + `DenyReason` — wire PolicyEngine decisions into event recording

### Where to Add Compliance gRPC Service
- **New service** in `crates/op-grpc-bridge/src/grpc_server.rs`: Add `ComplianceService` to the `run_grpc_server()` builder chain
- **Proto definition**: Add `compliance.proto` compiled in `build.rs` alongside existing operation.v1 protos
- **Service methods**: CheckCompliance, GetPolicies, CreatePolicy, GetComplianceReport, StreamComplianceEvents
- Register in `health_reporter` and wrap with `tonic_web::enable()`

### Where to Add Frontend Compliance Dashboard
- **New route**: `/compliance` in `axon-trace-ui/src/App.tsx`
- **New page**: `CompliancePage.tsx`
- **gRPC client**: Add `complianceService` to `src/grpc/client.ts` following existing pattern
- **State**: Could extend `event-store.ts` or create dedicated `compliance-store.ts`
- **Type definitions**: Add `src/grpc/types/compliance.ts`

### D-Bus Mirror Integration
The D-Bus mirror at `org.opdbus.v1` already projects OVSDB + NonNet state. Compliance status could be:
- Published as a new D-Bus interface `org.opdbus.ComplianceV1`
- Or added to existing NonNet DB and projected through the mirror automatically

### Data Stores Summary

| Store | Technology | What Lives There | Role |
|---|---|---|---|
| OVSDB | JSON-RPC socket | Network state (bridges, ports, interfaces) | Authoritative RCP |
| NonNet | In-process JSON-RPC | Non-network plugin state | Authoritative RCP |
| op-state-store | SQLite (sqlx) | Plugin state, execution jobs, metrics | Persistent |
| Enterprise SQLite | SQLite (rusqlite) | Schema catalog (SqlitePluginCatalog) | Catalog |
| BTRFS timing_subvol | Files on BTRFS | Blockchain footprints (audit, immutable) | Audit trail |
| BTRFS state_subvol | Files on BTRFS | DR current.json snapshots | DR |
| CozoDB | Sled (embedded) | Knowledge graph (events, links, namespaces) | Graph projection |
| Qdrant | Vector DB (gRPC) | Footprint + reasoning episode embeddings | Semantic search |
| Embedding channel | in-process mpsc | In-flight embed requests | Best-effort runtime |
