# D-Bus Service Manager - Design

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    ServiceManagerPlugin                      │
│                   (implements StatePlugin)                   │
├─────────────────────────────────────────────────────────────┤
│                    ServiceManager Trait                      │
│  - list_services()  - start()  - stop()  - status()        │
├──────────────────────┬──────────────────────────────────────┤
│   SystemdBackend     │        DinitBackend                  │
│   (D-Bus to systemd) │     (D-Bus to dinit)                 │
├──────────────────────┴──────────────────────────────────────┤
│                      D-Bus Connection                        │
│                        (zbus)                                │
└─────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. ServiceManager Trait

```rust
#[async_trait]
pub trait ServiceManager: Send + Sync {
    /// Detect if this backend is available
    async fn is_available(&self) -> bool;
    
    /// List all services
    async fn list_services(&self) -> Result<Vec<ServiceInfo>>;
    
    /// Get status of a specific service
    async fn status(&self, name: &str) -> Result<ServiceStatus>;
    
    /// Start a service
    async fn start(&self, name: &str) -> Result<()>;
    
    /// Stop a service
    async fn stop(&self, name: &str) -> Result<()>;
    
    /// Restart a service
    async fn restart(&self, name: &str) -> Result<()>;
    
    /// Enable service at boot
    async fn enable(&self, name: &str) -> Result<()>;
    
    /// Disable service at boot
    async fn disable(&self, name: &str) -> Result<()>;
    
    /// Subscribe to state changes
    fn subscribe(&self) -> broadcast::Receiver<ServiceEvent>;
}
```

### 2. Data Types

```rust
pub struct ServiceInfo {
    pub name: String,
    pub description: Option<String>,
    pub status: ServiceStatus,
    pub enabled: bool,
}

pub enum ServiceStatus {
    Running,
    Stopped,
    Failed { reason: String },
    Unknown,
}

pub struct ServiceEvent {
    pub service: String,
    pub old_status: ServiceStatus,
    pub new_status: ServiceStatus,
    pub timestamp: DateTime<Utc>,
}
```

### 3. Systemd Backend

Uses D-Bus interface `org.freedesktop.systemd1`:

| Operation | D-Bus Method |
|-----------|--------------|
| list | `ListUnits()` on Manager |
| status | `Get` property on Unit |
| start | `StartUnit()` on Manager |
| stop | `StopUnit()` on Manager |
| enable | `EnableUnitFiles()` on Manager |
| disable | `DisableUnitFiles()` on Manager |

### 4. Dinit Backend

Uses D-Bus interface (if dinit exposes one) or falls back to:
- `dinitctl` command wrapper
- Direct socket communication

### 5. Backend Detection

```rust
pub async fn detect_backend(conn: &Connection) -> Box<dyn ServiceManager> {
    // Try systemd first (most common)
    if SystemdBackend::is_available(conn).await {
        return Box::new(SystemdBackend::new(conn.clone()));
    }
    
    // Try dinit
    if DinitBackend::is_available(conn).await {
        return Box::new(DinitBackend::new(conn.clone()));
    }
    
    // Fallback to stub
    Box::new(StubBackend::new())
}
```

## State Plugin Integration

### State Schema

```json
{
  "services": {
    "nginx": {
      "status": "running",
      "enabled": true
    },
    "postgresql": {
      "status": "running", 
      "enabled": true
    }
  }
}
```

### Diff Calculation

```rust
impl StatePlugin for ServiceManagerPlugin {
    async fn query_current_state(&self) -> Result<Value> {
        let services = self.manager.list_services().await?;
        // Convert to JSON
    }
    
    async fn calculate_diff(&self, desired: &Value) -> Result<Vec<StateDiff>> {
        let current = self.query_current_state().await?;
        // Compare and generate diffs
    }
    
    async fn apply_state(&self, desired: &Value) -> Result<ApplyResult> {
        for diff in self.calculate_diff(desired).await? {
            match diff.operation {
                ChangeOperation::Update => {
                    // Start/stop/enable/disable as needed
                }
            }
        }
    }
}
```

## D-Bus Signal Handling

Subscribe to systemd signals for real-time updates:

```rust
// org.freedesktop.systemd1.Manager
// Signal: UnitNew(s name, o path)
// Signal: UnitRemoved(s name, o path)
// Signal: JobRemoved(u id, o job, s unit, s result)

async fn watch_signals(conn: &Connection, tx: broadcast::Sender<ServiceEvent>) {
    let proxy = ManagerProxy::new(conn).await?;
    let mut stream = proxy.receive_job_removed().await?;
    
    while let Some(signal) = stream.next().await {
        let args = signal.args()?;
        tx.send(ServiceEvent {
            service: args.unit,
            // ...
        });
    }
}
```

## File Structure

```
crates/op-service-manager/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── traits.rs          # ServiceManager trait
│   ├── types.rs           # ServiceInfo, ServiceStatus, etc.
│   ├── backends/
│   │   ├── mod.rs
│   │   ├── systemd.rs     # Systemd D-Bus backend
│   │   ├── dinit.rs       # Dinit backend
│   │   └── stub.rs        # Fallback stub
│   ├── plugin.rs          # StatePlugin implementation
│   └── signals.rs         # D-Bus signal handling
```

## Migration Path

1. Create new `op-service-manager` crate
2. Implement ServiceManager trait + systemd backend
3. Create StatePlugin wrapper
4. Update op-plugins to use new crate
5. Deprecate old systemd.rs plugin
6. Add dinit backend
7. Remove old code

## Testing Strategy

1. Unit tests with mock D-Bus connection
2. Integration tests against real systemd (CI)
3. Manual testing with dinit on test VM
