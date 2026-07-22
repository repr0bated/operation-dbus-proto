# op-services Design

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         op-web / op-mcp                         │
│                              │                                  │
│                         gRPC (tonic)                            │
└──────────────────────────────┼──────────────────────────────────┘
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                        op-services                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │ gRPC Server │  │ D-Bus Iface │  │ ServiceManager          │  │
│  │ (tonic)     │  │ (zbus)      │  │                         │  │
│  └──────┬──────┘  └──────┬──────┘  │  ┌───────────────────┐  │  │
│         │                │         │  │ ServiceRegistry   │  │  │
│         └────────┬───────┘         │  │ (schema-as-code)  │  │  │
│                  ▼                 │  └───────────────────┘  │  │
│         ┌────────────────┐        │  ┌───────────────────┐  │  │
│         │ ServiceManager │◄───────┼──│ DinitProxy        │  │  │
│         └────────────────┘        │  └───────────────────┘  │  │
│                  │                │  ┌───────────────────┐  │  │
│                  ▼                │  │ ProcessManager    │  │  │
│         ┌────────────────┐        │  │ (fallback)        │  │  │
│         │ SQLite (sqlx)  │        │  └───────────────────┘  │  │
│         └────────────────┘        └─────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                               │
                    ┌──────────┴──────────┐
                    ▼                     ▼
            ┌─────────────┐       ┌─────────────┐
            │ dinit (D-Bus)│       │ Direct exec │
            │ PID 1        │       │ (fallback)  │
            └─────────────┘       └─────────────┘
```

## Schema-as-Code: Service Definition

```rust
/// Service definition - the core schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDef {
    /// Unique service identifier
    pub name: ServiceName,
    
    /// Service type
    pub service_type: ServiceType,
    
    /// Execution configuration
    pub exec: ExecConfig,
    
    /// Dependencies (start after these)
    pub depends_on: Vec<ServiceName>,
    
    /// Restart policy
    pub restart: RestartPolicy,
    
    /// Resource limits
    pub resources: Option<ResourceLimits>,
    
    /// Environment variables
    pub environment: HashMap<String, String>,
    
    /// Health check configuration
    pub health_check: Option<HealthCheck>,
}

/// Validated service name (compile-time where possible)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ServiceName(String);

impl ServiceName {
    pub fn new(name: impl Into<String>) -> Result<Self, ValidationError> {
        let name = name.into();
        // Validate: alphanumeric, dash, underscore, max 64 chars
        if !Self::is_valid(&name) {
            return Err(ValidationError::InvalidServiceName(name));
        }
        Ok(Self(name))
    }
    
    const fn is_valid(name: &str) -> bool {
        // Validation logic
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceType {
    /// Long-running daemon
    Simple,
    /// Forks, parent exits
    Forking { pid_file: Option<PathBuf> },
    /// One-shot script
    Oneshot { remain_after_exit: bool },
    /// Notify when ready via sd_notify
    Notify,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecConfig {
    /// Command to execute
    pub start: Command,
    /// Optional stop command
    pub stop: Option<Command>,
    /// Optional reload command  
    pub reload: Option<Command>,
    /// Working directory
    pub working_dir: Option<PathBuf>,
    /// User to run as
    pub user: Option<String>,
    /// Group to run as
    pub group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub program: PathBuf,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartPolicy {
    pub condition: RestartCondition,
    pub delay: Duration,
    pub max_retries: Option<u32>,
    pub reset_period: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RestartCondition {
    Never,
    Always,
    OnFailure,
    OnAbnormal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub memory_max: Option<u64>,
    pub cpu_quota: Option<f32>,
    pub tasks_max: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub command: Command,
    pub interval: Duration,
    pub timeout: Duration,
    pub retries: u32,
}
```

## gRPC Service Definition

```protobuf
syntax = "proto3";
package opdbus.services.v1;

service ServiceManager {
    // Service lifecycle
    rpc Start(StartRequest) returns (StartResponse);
    rpc Stop(StopRequest) returns (StopResponse);
    rpc Restart(RestartRequest) returns (RestartResponse);
    rpc Reload(ReloadRequest) returns (ReloadResponse);
    
    // Service configuration
    rpc Create(CreateRequest) returns (CreateResponse);
    rpc Update(UpdateRequest) returns (UpdateResponse);
    rpc Delete(DeleteRequest) returns (DeleteResponse);
    rpc Get(GetRequest) returns (GetResponse);
    rpc List(ListRequest) returns (ListResponse);
    
    // Enable/disable auto-start
    rpc Enable(EnableRequest) returns (EnableResponse);
    rpc Disable(DisableRequest) returns (DisableResponse);
    
    // Status streaming
    rpc WatchStatus(WatchRequest) returns (stream ServiceEvent);
}

message ServiceDef {
    string name = 1;
    ServiceType type = 2;
    ExecConfig exec = 3;
    repeated string depends_on = 4;
    RestartPolicy restart = 5;
    map<string, string> environment = 6;
    optional ResourceLimits resources = 7;
    optional HealthCheck health_check = 8;
    bool enabled = 9;
}

enum ServiceType {
    SIMPLE = 0;
    FORKING = 1;
    ONESHOT = 2;
    NOTIFY = 3;
}

message ServiceStatus {
    string name = 1;
    ServiceState state = 2;
    optional uint32 pid = 3;
    optional string error = 4;
    google.protobuf.Timestamp started_at = 5;
}

enum ServiceState {
    STOPPED = 0;
    STARTING = 1;
    RUNNING = 2;
    STOPPING = 3;
    FAILED = 4;
}

message ServiceEvent {
    string name = 1;
    ServiceState old_state = 2;
    ServiceState new_state = 3;
    google.protobuf.Timestamp timestamp = 4;
}
```

## D-Bus Interface

```xml
<node>
  <interface name="org.opdbus.services.v1.Manager">
    <!-- Methods -->
    <method name="Start">
      <arg name="name" type="s" direction="in"/>
    </method>
    <method name="Stop">
      <arg name="name" type="s" direction="in"/>
    </method>
    <method name="Restart">
      <arg name="name" type="s" direction="in"/>
    </method>
    <method name="GetStatus">
      <arg name="name" type="s" direction="in"/>
      <arg name="status" type="s" direction="out"/>
    </method>
    <method name="ListServices">
      <arg name="services" type="as" direction="out"/>
    </method>
    
    <!-- Signals -->
    <signal name="ServiceStateChanged">
      <arg name="name" type="s"/>
      <arg name="old_state" type="s"/>
      <arg name="new_state" type="s"/>
    </signal>
  </interface>
</node>
```

## Module Structure

```
crates/op-services/
├── Cargo.toml
├── build.rs                    # tonic-build for proto
├── proto/
│   └── services.proto          # gRPC definitions
└── src/
    ├── lib.rs                  # Public API
    ├── schema/
    │   ├── mod.rs
    │   ├── service_def.rs      # ServiceDef schema
    │   ├── validation.rs       # Schema validation
    │   └── migration.rs        # systemd → op-services
    ├── manager/
    │   ├── mod.rs
    │   ├── service_manager.rs  # Core logic
    │   ├── dinit_proxy.rs      # dinit D-Bus client
    │   └── process.rs          # Direct process fallback
    ├── grpc/
    │   ├── mod.rs
    │   └── server.rs           # tonic gRPC server
    ├── dbus/
    │   ├── mod.rs
    │   └── interface.rs        # zbus D-Bus server
    ├── store/
    │   ├── mod.rs
    │   └── sqlite.rs           # SQLite persistence
    └── bin/
        ├── op-services.rs      # Main daemon
        └── systemctl.rs        # Compat wrapper
```

## Key Design Decisions

1. **Schema-as-code**: ServiceDef is the source of truth, validated at compile-time
2. **gRPC primary**: Internal comms use gRPC (tonic), not REST
3. **D-Bus secondary**: External/system integration via D-Bus
4. **dinit backend**: Use dinit for actual process supervision
5. **Fallback mode**: Direct process management if dinit unavailable
6. **SQLite state**: Persist service definitions and audit log
