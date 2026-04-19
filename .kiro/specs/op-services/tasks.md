# op-services Implementation Tasks

## Phase 0: Project Setup
- [ ] Create crate structure with modules
- [ ] Add to workspace Cargo.toml
- [ ] Setup proto/ with services.proto
- [ ] Configure build.rs for tonic-build
- [ ] Add dependencies (tonic, zbus, sqlx, etc.)

## Phase 1: Schema-as-Code
- [ ] Define ServiceDef struct with validation
- [ ] Define ServiceName newtype with validation
- [ ] Define ServiceType, ExecConfig, RestartPolicy
- [ ] Define ResourceLimits, HealthCheck
- [ ] Implement From<systemd::Unit> for migration
- [ ] Unit tests for schema validation

## Phase 2: Storage Layer
- [ ] SQLite schema for services table
- [ ] SQLite schema for audit_log table
- [ ] CRUD operations for ServiceDef
- [ ] Audit log insertion
- [ ] Migration scripts

## Phase 3: Service Manager Core
- [ ] ServiceManager struct
- [ ] Start/stop/restart logic
- [ ] Dependency resolution (topological sort)
- [ ] State machine for service lifecycle
- [ ] Health check runner

## Phase 4: dinit Integration
- [ ] DinitProxy D-Bus client (zbus)
- [ ] Service file generation for dinit.d/
- [ ] Fallback ProcessManager for direct exec
- [ ] Signal handling (SIGTERM, SIGCHLD)

## Phase 5: gRPC Interface
- [ ] Generate Rust from services.proto
- [ ] Implement ServiceManager gRPC service
- [ ] Streaming WatchStatus implementation
- [ ] Integration with ServiceManager core

## Phase 6: D-Bus Interface
- [ ] org.opdbus.services.v1.Manager interface
- [ ] Signal emission for state changes
- [ ] systemctl compatibility methods

## Phase 7: Binaries
- [ ] op-services daemon (main.rs)
- [ ] systemctl compat wrapper (bin/systemctl.rs)
- [ ] CLI argument parsing

## Phase 8: Integration
- [ ] Add gRPC client to op-web
- [ ] Wire up UI services page
- [ ] Test with dinit on server
- [ ] Boot sequence testing

## Dependencies

```toml
[dependencies]
# gRPC
tonic = "0.12"
prost = "0.13"
prost-types = "0.13"

# D-Bus
zbus = { version = "4.0", features = ["tokio"] }

# Database
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }

# Async
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
simd-json = "0.13"

# Process management
nix = { version = "0.29", features = ["signal", "process"] }

# Logging
tracing = "0.1"

[build-dependencies]
tonic-build = "0.12"
```
