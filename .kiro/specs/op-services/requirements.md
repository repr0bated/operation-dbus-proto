# op-services Requirements

## Problem Statement

Replace systemd with a lightweight, D-Bus-native service manager using dinit as PID 1. Must support schema-as-code service definitions, gRPC for internal comms, D-Bus for system integration.

## Functional Requirements

### FR-1: Service Definition Schema
- Services defined as code (Rust structs, TOML/JSON config)
- Schema includes: name, command, dependencies, restart policy, environment, resources
- Validation at compile-time where possible
- Migration path from systemd unit files

### FR-2: Service Lifecycle
- Start, stop, restart, reload services
- Dependency-ordered startup/shutdown
- Health checks and auto-restart
- Graceful shutdown with configurable timeout

### FR-3: dinit Integration
- D-Bus proxy to dinit (org.dinit.Manager)
- Fallback to direct process management if dinit unavailable
- Service file generation for dinit.d/

### FR-4: gRPC Interface (Internal)
- ServiceManager service for op-web/op-mcp
- Streaming status updates
- Batch operations

### FR-5: D-Bus Interface (External)
- org.opdbus.services.v1.Manager
- systemctl compatibility (drop-in replacement)
- Signal emission for state changes

### FR-6: Persistence
- SQLite for service definitions and state
- Audit log of all operations
- Checkpoint/rollback support

## Non-Functional Requirements

### NFR-1: Performance
- Boot time < 5s for core services
- < 10ms service start latency
- Minimal memory footprint (< 20MB resident)

### NFR-2: Reliability
- Survive service crashes without manager restart
- Atomic state transitions
- No orphan processes

### NFR-3: Security
- Capability-based permissions
- Audit trail for all operations
- No shell injection vectors

## Existing Code

Recovered from server:
- `dinit_proxy.rs` - D-Bus proxy to dinit (needs cleanup)
- `service_manager.rs` - Core logic (needs gRPC)
- `service_definition.rs` - Types (needs schema-as-code refinement)
- `dbus_interface.rs` - D-Bus interface (keep)
- `monitoring.rs` - Health checks (keep)

## Out of Scope (Separate Features)
- WireGuard auth (GhostBridge signup)
- Encrypted storage
- MCP gateway
- Web interface (handled by op-web)
