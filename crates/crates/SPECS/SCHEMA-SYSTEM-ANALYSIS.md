# Operation D-Bus Schema System - Detailed Analysis

## Executive Summary

The operation-dbus project implements a sophisticated multi-layered schema system that provides:
- **D-Bus Introspection**: Runtime discovery of D-Bus services, objects, and interfaces
- **Plugin Schema Registry**: JSON Schema-based validation for state plugins
- **Schema-as-Code Contracts**: Unified envelope structure for all plugin state
- **Database Schema Management**: SQLite schema for plugin and interface metadata
- **Dynamic Schema Generation**: Runtime schema generation from discovered services

This analysis examines the architecture, components, data flow, and integration points of the schema system.

---

## 1. Schema System Architecture

### 1.1 Four-Layer Schema Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Layer 4: Application Schema (Plugin Contracts)              │
│ - Unified envelope: stub/immutable/tunable/observed         │
│ - 32 plugin-specific schemas                                │
│ - Privacy/semantic indexing metadata                        │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 3: Validation Schema (JSON Schema 2026)               │
│ - PluginSchema with FieldSchema definitions                 │
│ - Constraint validation (min/max/pattern/oneOf)             │
│ - Conditional readOnly via propertyDependencies             │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 2: D-Bus Interface Schema (Introspection)             │
│ - InterfaceIntrospection with methods/signals/properties    │
│ - Hierarchical object tree discovery                        │
│ - JSON caching to BTRFS @cache/introspection/               │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 1: Database Schema (SQLite)                           │
│ - plugins table: service registration                       │
│ - schemas table: discovered interface definitions           │
│ - Foreign key relationships                                 │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 Key Design Principles

1. **Schema Versioning**: Every schema has a version field for migration support
2. **Dialect Flexibility**: Support for JSON Schema 2026 and Draft-07
3. **Lazy Loading**: Schemas loaded on-demand to minimize startup overhead
4. **Caching Strategy**: Multi-level caching (memory, BTRFS, SQLite)
5. **Validation Layers**: Progressive validation from database to application
6. **Immutability Support**: Schema-level and field-level immutability

---

## 2. Core Components

### 2.1 Plugin Schema Registry (`op-state-store/plugin_schema.rs`)

**Purpose**: Central registry for all plugin state schemas with validation.

**Key Types**:

```rust
pub struct PluginSchema {
    pub name: String,
    pub version: String,
    pub description: String,
    pub fields: HashMap<String, FieldSchema>,
    pub dependencies: Vec<String>,
    pub example: Option<Value>,
    pub immutable_paths: Vec<String>,
    pub tags: Vec<String>,
    pub dialect: String,  // JSON Schema dialect
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

pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    Array(Box<FieldType>),
    Object(HashMap<String, FieldSchema>),
    Enum(Vec<String>),
    Any,
}
```

**Capabilities**:
- Schema validation against JSON Schema meta-schemas
- Instance validation against plugin schemas
- Template generation with default values
- JSON Schema 2026 export with propertyDependencies
- Conditional immutability support

**SchemaRegistry**:
```rust
pub struct SchemaRegistry {
    schemas: HashMap<String, PluginSchema>,
}

impl SchemaRegistry {
    pub fn register(&mut self, schema: PluginSchema)
    pub fn get(&self, name: &str) -> Option<&PluginSchema>
    pub fn validate(&self, plugin: &str, state: &Value) -> ValidationResult
    pub fn list_schemas(&self) -> Vec<String>
    pub fn export_json_schema(&self, plugin: &str) -> Option<Value>
}
```

### 2.2 D-Bus Introspection System (`op-introspection/`)

**Purpose**: Discover and cache D-Bus service interfaces at runtime.

**Hierarchical Introspection** (`hierarchical.rs`):

```rust
pub struct HierarchicalIntrospection {
    pub timestamp: String,
    pub system_bus: BusIntrospection,
    pub session_bus: BusIntrospection,
    pub summary: IntrospectionSummary,
}

pub struct ServiceIntrospection {
    pub name: String,
    pub bus_type: String,
    pub objects: HashMap<String, ObjectIntrospection>,
    pub used_object_manager: bool,
    pub root_path: String,
}

pub struct ObjectIntrospection {
    pub path: String,
    pub interfaces: Vec<InterfaceIntrospection>,
    pub children: Vec<String>,
    pub introspectable: bool,
    pub error: Option<String>,
}

pub struct InterfaceIntrospection {
    pub name: String,
    pub methods: Vec<MethodIntrospection>,
    pub properties: Vec<PropertyIntrospection>,
    pub signals: Vec<SignalIntrospection>,
}
```

**Discovery Methods**:
1. **Recursive Traversal**: Walk object tree using zbus_xml::Node
2. **ObjectManager**: Bulk discovery via GetManagedObjects
3. **Property Introspection**: Full property metadata extraction
4. **Signal Discovery**: Signal definitions with arguments

**Caching Strategy**:
- **Memory Cache**: IntrospectionCache with RwLock
- **BTRFS Cache**: JSON snapshots in @cache/introspection/
- **Timestamp-based**: Snapshots named by timestamp for versioning

### 2.3 Schema Contract System (`op-plugins/state_plugins/schema_contract.rs`)

**Purpose**: Unified envelope structure for all plugin state objects.

**Contract Structure**:
```json
{
  "schema_version": "1.0.0",
  "plugin": "net",
  "object_type": "network_config",
  "object_id": "unique-id",
  
  "stub": {
    "system_id": "...",
    "source": "...",
    "source_ref": "...",
    "discovered_at": "2026-02-14T03:00:00Z"
  },
  
  "immutable": {
    "created_at": "2026-02-14T03:00:00Z",
    "created_by_plugin": "net",
    "identity_keys": ["name", "type"],
    "provider": "networkmanager"
  },
  
  "tunable": {
    // Plugin-specific configuration
  },
  
  "observed": {
    "last_observed_at": "2026-02-14T03:00:00Z",
    "status": "active",
    "drift_detected": false,
    "metrics": {}
  },
  
  "meta": {
    "dependencies": ["other-plugin"],
    "include_in_recovery": true,
    "recovery_priority": 10,
    "sensitivity": "internal",
    "tags": [],
    "enabled": true
  },
  
  "semantic_index": {
    "include_paths": ["/tunable/interfaces"],
    "exclude_paths": ["/stub/discovered_at"],
    "chunking": {
      "strategy": "json-path-group",
      "max_tokens": 512
    },
    "redaction": {
      "enabled": true
    }
  },
  
  "privacy_index": {
    "redaction": {
      "rules": [],
      "default_action": "mask",
      "secret_paths": [],
      "pii_paths": [],
      "hash_salt_ref": "vault://op-dbus/privacy/hash-salt",
      "reversible": false
    }
  }
}
```

**32 Plugin Schemas**:
- adc, agent_config, config, dinit, dnsresolver
- endpoint, full_system, gcloud_adc, hardware, keypair
- keyring, login1, lxc, mcp, net
- netmaker, openflow, openflow_obfuscation, ovsdb_bridge, packagekit
- pcidecl, privacy, privacy_router, proxmox, proxy_server
- service, sess_decl, software, systemd, users
- web_ui, wireguard

**Contract Features**:
- **Uniform Envelope**: All plugins follow same structure
- **Dependency Tracking**: Explicit plugin dependencies
- **Recovery Priority**: 0-100 priority for disaster recovery
- **Sensitivity Levels**: public, internal, secret
- **Semantic Indexing**: ML embedding configuration
- **Privacy Controls**: PII/secret path redaction

### 2.4 Database Schema (`op-dbus-model/`)

**Purpose**: Persistent storage for plugin and schema metadata.

**Models** (`models.rs`):
```rust
pub struct Plugin {
    pub name: String,
    pub service_name: String,
    pub base_object: OwnedValue,  // JSON
    pub created_at: DateTime<Utc>,
}

pub struct Schema {
    pub id: String,
    pub plugin_name: String,
    pub definition: OwnedValue,  // JSON
    pub discovered_from: Option<String>,
    pub discovered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
```

**Database Schema**:
```sql
CREATE TABLE plugins (
    name TEXT PRIMARY KEY,
    service_name TEXT NOT NULL,
    base_object TEXT NOT NULL,  -- JSON
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE schemas (
    id TEXT PRIMARY KEY,
    plugin_name TEXT NOT NULL,
    definition TEXT NOT NULL,  -- JSON
    discovered_from TEXT,
    discovered_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (plugin_name) REFERENCES plugins(name)
);
```

**Schema Creation**:
```rust
pub async fn create_schema(pool: &SqlitePool) -> Result<()> {
    // Creates both tables with foreign key relationships
    // Idempotent - safe to call multiple times
}
```

---

## 3. Schema Discovery and Validation Flow

### 3.1 Discovery Flow

```
1. D-Bus Service Detected
   ↓
2. IntrospectionService.introspect_service()
   ↓
3. Hierarchical Discovery
   - Recursive object traversal
   - ObjectManager bulk discovery
   - Interface/method/property extraction
   ↓
4. Cache to BTRFS
   - JSON snapshot: @cache/introspection/{timestamp}.json
   ↓
5. Store in Database
   - Plugin record in plugins table
   - Schema record in schemas table
   ↓
6. Register in SchemaRegistry
   - PluginSchema created
   - Validation rules configured
```

### 3.2 Validation Flow

```
1. State Update Request
   ↓
2. Schema Lookup
   - SchemaRegistry.get(plugin_name)
   ↓
3. Field Validation
   - Required fields present
   - Type checking
   - Constraint validation (min/max/pattern)
   ↓
4. Immutability Check
   - Check immutable_paths
   - Check read_only fields
   - Check read_only_when conditions
   ↓
5. Dependency Validation
   - Verify dependent plugins exist
   - Check dependency state
   ↓
6. Custom Validation
   - Plugin-specific validators
   ↓
7. Return ValidationResult
   - valid: bool
   - errors: Vec<String>
   - warnings: Vec<String>
```

---

## 4. Integration Points

### 4.1 State Management Integration

**StateManager** (`op-state/manager.rs`):
```rust
pub struct StateManager {
    schema_registry: Arc<SchemaRegistry>,
    strict_schema_validation: bool,
    // ...
}

impl StateManager {
    pub async fn materialize_state(&self, plugin: &str, state: &Value) -> Result<()> {
        // Validate against schema
        let validation = self.schema_registry.validate(plugin, state)?;
        
        if !validation.valid && self.strict_schema_validation {
            return Err(anyhow!("Schema validation failed: {:?}", validation.errors));
        }
        
        // Materialize state
        // ...
    }
}
```

### 4.2 gRPC Bridge Integration

**Proto Generation** (`op-grpc-bridge/proto_gen.rs`):
```rust
pub fn generate_proto_from_schema(schema: &PluginSchema) -> String {
    // Convert PluginSchema to protobuf message definition
    // Enables dynamic gRPC services from schemas
}
```

### 4.3 Tool Discovery Integration

**D-Bus Tool Discovery** (`op-tools/discovery/sources/dbus.rs`):
```rust
pub struct DbusDiscoverySource {
    introspection_service: IntrospectionService,
    well_known_services: Vec<String>,
}

impl DbusDiscoverySource {
    pub async fn discover_tools(&self) -> Result<Vec<Tool>> {
        // Use introspection to discover D-Bus methods as tools
    }
}
```

### 4.4 Inspector Gadget Integration

**Schema Generation** (`op-inspector/introspective_gadget.rs`):
```rust
pub struct IntrospectiveGadget {
    // Generates schemas from unknown data structures
    pub fn generate_schema(&self, data: &Value) -> SchemaDefinition
}
```

---

## 5. Advanced Features

### 5.1 Conditional Immutability

**propertyDependencies** (JSON Schema 2026):
```json
{
  "properties": {
    "status": {"type": "string"},
    "config": {"type": "object"}
  },
  "propertyDependencies": {
    "config": {
      "status": {
        "const": "locked",
        "readOnly": true
      }
    }
  }
}
```

When `status == "locked"`, the `config` field becomes read-only.

**Implementation**:
```rust
pub struct ReadOnlyCondition {
    pub property: String,  // "status"
    pub value: String,     // "locked"
}
```

### 5.2 Schema Versioning

**Version Field**:
```rust
pub struct PluginSchema {
    pub version: String,  // "1.2.0"
    // ...
}
```

**Migration Support**:
- Schemas stored with version
- Migration functions registered per version
- Automatic migration on load

### 5.3 Schema Dialects

**Supported Dialects**:
- `https://json-schema.org/v1/2026` (default)
- `http://json-schema.org/draft-07/schema#`

**Per-Schema Configuration**:
```rust
pub struct PluginSchema {
    pub dialect: String,  // Override default
    // ...
}
```

### 5.4 Template Generation

**Auto-generate State Templates**:
```rust
impl PluginSchema {
    pub fn generate_template(&self) -> Value {
        // Creates template with default/example values
        // Useful for plugin initialization
    }
}
```

---

## 6. Performance Considerations

### 6.1 Caching Strategy

**Three-Level Cache**:
1. **Memory**: IntrospectionCache (RwLock<HashMap>)
   - O(1) lookup
   - Cleared on service restart
   
2. **BTRFS**: JSON snapshots
   - Persistent across restarts
   - Copy-on-write efficiency
   - Timestamp-based versioning
   
3. **SQLite**: Structured storage
   - Queryable metadata
   - Foreign key relationships
   - Transaction support

### 6.2 Lazy Loading

**SchemaRegistry**:
- Schemas loaded on first access
- Cached after load
- Minimal startup overhead

**IntrospectionService**:
- Services introspected on-demand
- Results cached for subsequent access

### 6.3 Validation Optimization

**Early Exit**:
- Required field check first (fast)
- Type checking before constraints
- Skip validation if schema unchanged

**Batch Validation**:
- Validate multiple fields in parallel
- Aggregate errors for single response

---

## 7. Security and Privacy

### 7.1 Sensitivity Levels

**Three Levels**:
- **public**: Safe for external exposure
- **internal**: Internal use only
- **secret**: Contains credentials/keys

**Enforcement**:
```rust
pub struct PluginSchema {
    pub tags: Vec<String>,  // ["secret"]
}
```

### 7.2 PII/Secret Path Redaction

**Privacy Index**:
```json
{
  "privacy_index": {
    "redaction": {
      "secret_paths": ["/tunable/api_key"],
      "pii_paths": ["/tunable/email"],
      "default_action": "mask"
    }
  }
}
```

**Redaction Actions**:
- **drop**: Remove field entirely
- **mask**: Replace with "***"
- **hash**: One-way hash with salt

### 7.3 Immutability Enforcement

**Schema-Level**:
```rust
pub struct PluginSchema {
    pub tags: Vec<String>,  // ["immutable"]
}
```

**Field-Level**:
```rust
pub struct FieldSchema {
    pub read_only: bool,
    pub read_only_when: Option<ReadOnlyCondition>,
}
```

**Path-Level**:
```rust
pub struct PluginSchema {
    pub immutable_paths: Vec<String>,  // ["/id", "/metadata"]
}
```

---

## 8. Testing and Validation

### 8.1 Schema Validation Tests

**Contract Tests** (`schema_contract.rs`):
```rust
#[test]
fn test_all_plugins_have_contract_schema() {
    let schemas = all_contract_schemas();
    assert_eq!(schemas.len(), 32);
}

#[test]
fn test_dependency_targets_are_known_plugins() {
    // Verify all dependencies reference valid plugins
}

#[test]
fn test_recovery_priority_is_bounded() {
    // Ensure priority in 0-100 range
}
```

### 8.2 Introspection Tests

**Integration Tests**:
- Mock D-Bus services
- Verify hierarchical discovery
- Test caching behavior
- Validate JSON output

### 8.3 Validation Tests

**Unit Tests**:
- Required field validation
- Type checking
- Constraint validation
- Immutability enforcement

---

## 9. Future Enhancements

### 9.1 Schema Evolution

- **Automatic Migration**: Generate migration code from schema diffs
- **Backward Compatibility**: Support multiple schema versions simultaneously
- **Schema Diff Tool**: Compare schemas across versions

### 9.2 Advanced Validation

- **Cross-Field Validation**: Validate relationships between fields
- **Async Validators**: External validation (API calls, database checks)
- **Custom Validators**: Plugin-specific validation logic

### 9.3 Schema Discovery

- **Auto-Schema Generation**: Generate schemas from observed data
- **Schema Inference**: ML-based schema inference
- **Schema Merging**: Combine schemas from multiple sources

### 9.4 Performance

- **Schema Compilation**: Compile schemas to bytecode for faster validation
- **Parallel Validation**: Validate multiple instances concurrently
- **Incremental Validation**: Only validate changed fields

---

## 10. Conclusion

The operation-dbus schema system provides a comprehensive, multi-layered approach to schema management that spans from low-level D-Bus introspection to high-level application contracts. Key strengths include:

1. **Unified Architecture**: Consistent schema handling across all layers
2. **Flexibility**: Support for multiple dialects and custom validation
3. **Performance**: Multi-level caching and lazy loading
4. **Security**: Built-in privacy and immutability controls
5. **Extensibility**: Easy to add new plugins and schemas

The system successfully balances flexibility with structure, enabling both dynamic discovery and strict validation while maintaining excellent performance characteristics.
