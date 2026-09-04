# D-Bus Service Manager - Requirements

## Problem Statement

Replace systemd dependency with a D-Bus-native service manager that:
1. Uses D-Bus as the primary IPC mechanism (per AGENTS.md: "DBUS FIRST")
2. Supports dinit as an alternative init system
3. Provides a unified interface regardless of underlying init system
4. Integrates with the existing plugin architecture

## Goals

1. **D-Bus Native**: All service operations via D-Bus calls
2. **Init Agnostic**: Abstract over systemd/dinit/other init systems
3. **Plugin Compatible**: Implement as StatePlugin for state management
4. **Minimal Dependencies**: No direct systemd library dependencies

## Non-Goals

- Replacing systemd entirely on the system
- Implementing our own init system
- Supporting non-Linux platforms

## Functional Requirements

### FR1: Service Discovery
- List all services via D-Bus introspection
- Query service status (running, stopped, failed)
- Get service metadata (description, dependencies)

### FR2: Service Control
- Start/stop/restart services
- Enable/disable services (boot persistence)
- Reload service configuration

### FR3: Service Monitoring
- Subscribe to service state changes via D-Bus signals
- Track service health over time
- Emit events for snowball audit trail

### FR4: Init System Abstraction
- Detect available init system (systemd, dinit, other)
- Translate operations to init-specific D-Bus calls
- Provide consistent API regardless of backend

### FR5: State Plugin Integration
- Query current state of all managed services
- Calculate diff between current and desired state
- Apply desired state with rollback support

## Non-Functional Requirements

### NFR1: Performance
- Service list query < 100ms
- Service state change < 500ms
- Minimal memory footprint

### NFR2: Reliability
- Graceful degradation if D-Bus unavailable
- Retry logic for transient failures
- Clear error messages

### NFR3: Security
- Respect D-Bus policy permissions
- Audit all state-changing operations
- No privilege escalation

## Constraints

- Must use zbus for D-Bus communication
- Must integrate with op-state plugin system
- Must support gRPC bridge for remote operations
- Prefer simd-json for serialization

## Success Criteria

1. All existing systemd plugin tests pass with new implementation
2. Works with both systemd and dinit backends
3. No direct systemd library dependencies in final code
4. Full D-Bus introspection support

## Open Questions

1. Should we support socket activation?
2. How to handle user vs system services?
3. What's the migration path from existing systemd plugin?
