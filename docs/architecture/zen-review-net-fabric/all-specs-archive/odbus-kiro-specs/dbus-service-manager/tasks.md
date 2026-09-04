# D-Bus Service Manager - Implementation Tasks

## Phase 0: Setup

- [ ] 0.1 Create `crates/op-service-manager/` crate
- [ ] 0.2 Add to workspace Cargo.toml
- [ ] 0.3 Set up basic dependencies (zbus, tokio, simd-json)

## Phase 1: Core Traits & Types

- [ ] 1.1 Define `ServiceManager` trait in `traits.rs`
- [ ] 1.2 Define `ServiceInfo`, `ServiceStatus`, `ServiceEvent` in `types.rs`
- [ ] 1.3 Add serde derives for JSON serialization
- [ ] 1.4 Unit tests for type serialization

## Phase 2: Systemd Backend

- [ ] 2.1 Create `backends/systemd.rs`
- [ ] 2.2 Implement `is_available()` - check D-Bus service exists
- [ ] 2.3 Implement `list_services()` via `ListUnits()`
- [ ] 2.4 Implement `status()` via unit properties
- [ ] 2.5 Implement `start()` / `stop()` / `restart()`
- [ ] 2.6 Implement `enable()` / `disable()`
- [ ] 2.7 Implement signal subscription for state changes
- [ ] 2.8 Integration tests (require systemd)

## Phase 3: Backend Detection

- [ ] 3.1 Create `backends/mod.rs` with `detect_backend()`
- [ ] 3.2 Create `backends/stub.rs` fallback
- [ ] 3.3 Test detection logic

## Phase 4: State Plugin

- [ ] 4.1 Create `plugin.rs` implementing `StatePlugin`
- [ ] 4.2 Implement `query_current_state()`
- [ ] 4.3 Implement `calculate_diff()`
- [ ] 4.4 Implement `apply_state()` with proper ordering
- [ ] 4.5 Add rollback support
- [ ] 4.6 Integrate with snowball footprint

## Phase 5: Dinit Backend

- [ ] 5.1 Research dinit D-Bus interface (if any)
- [ ] 5.2 Create `backends/dinit.rs`
- [ ] 5.3 Implement ServiceManager trait for dinit
- [ ] 5.4 Test on dinit system

## Phase 6: Integration

- [ ] 6.1 Register plugin in `op-plugins/default_registry.rs`
- [ ] 6.2 Add gRPC service methods in `op-grpc-bridge`
- [ ] 6.3 Add tools in `op-tools/builtin/`
- [ ] 6.4 Update UI to use new service manager

## Phase 7: Migration & Cleanup

- [ ] 7.1 Deprecate `op-plugins/src/state_plugins/systemd.rs`
- [ ] 7.2 Deprecate `op-plugins/src/systemd.rs`
- [ ] 7.3 Update all imports
- [ ] 7.4 Remove old code
- [ ] 7.5 Update documentation

## Dependencies

```
Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 6 → Phase 7
                                    ↘
                              Phase 5 (parallel)
```

## Estimated Effort

| Phase | Effort |
|-------|--------|
| 0 | 0.5 day |
| 1 | 0.5 day |
| 2 | 2 days |
| 3 | 0.5 day |
| 4 | 1.5 days |
| 5 | 1-2 days |
| 6 | 1 day |
| 7 | 0.5 day |

**Total: ~7-8 days**
