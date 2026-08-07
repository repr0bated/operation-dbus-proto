> **SUPERSEDED — DO NOT IMPLEMENT.**
>
> See [`SUPERSEDED.md`](./SUPERSEDED.md).
> Successor: [`.kiro/specs/remove-projection-static-tree/`](../remove-projection-static-tree/).

# Implementation Plan: D-Bus Mirror Event-Session Refactoring

## Overview

This refactoring transforms `crates/op-dbus-mirror` from a 30-second polling loop to a fully event-driven, session-scoped architecture. The new design uses event feeds from all data sources (OVSDB, NonNetDb, procfs, StateManager, ComponentRegistry) to publish only deltas with per-peer session management.

## Tasks

- [x] 1. Update Cargo.toml dependencies
  - [x] 1.1 Upgrade zbus from 4.0 to 5.12 in workspace and op-dbus-mirror Cargo.toml
    - Update `[workspace.dependencies]` zbus version
    - Update `crates/op-dbus-mirror/Cargo.toml` zbus dependency
    - _Requirements: 9.1, 9.2_
  
  - [x] 1.2 Add procfs 0.17 and inotify 0.10 to op-dbus-mirror Cargo.toml
    - Add `procfs = "0.17"` dependency
    - Add `inotify = "0.10"` dependency
    - _Requirements: 5.1, 5.2_
  
  - [ ] 1.3 Remove simd-json dependency from op-dbus-mirror Cargo.toml
    - Remove `simd-json = { version = "0.13", features = ["serde"] }` from dependencies
    - _Requirements: 10.1_

- [-] 2. Create MirrorSession struct and DashMap storage
  - [x] 2.1 Create session.rs module with MirrorSession struct
    - Define `MirrorSession` struct with peer_name, subscribed_paths, last_acked_sequence, pending_events, created_at, event_count
    - Derive Debug for all public types
    - _Requirements: 1.1, 1.2, 1.6_
  
  - [x] 2.2 Add sessions field to DbusMirror struct
    - Add `pub sessions: DashMap<String, MirrorSession>` to DbusMirror
    - Initialize in `DbusMirror::new()` with `DashMap::new()`
    - _Requirements: 1.1, 1.6_
  
  - [x] 2.3 Implement session creation on peer activity
    - Create method to check and create session if not exists
    - Set initial last_acked_sequence to 0 for all paths
    - _Requirements: 1.1, 1.2_

- [x] 3. Define MirrorEvent enum
  - [x] 3.1 Create event.rs module with MirrorEvent enum
    - Define all variants: OvsdbRow, NonNet, Plugin, Registry, ProcMem, ProcLoad, ProcStatic
    - Each variant includes sequence number and delta JSON
    - Derive Debug and Clone
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8_

- [x] 4. Implement EventDispatcher with all event sources
  - [x] 4.1 Create event dispatcher struct
    - Define `EventDispatcher` with broadcast_tx and mirror fields
    - Add fields for ovsdb_client, nonnet_db, state_manager, grpc_server
    - _Requirements: 3.1, 4.1, 5.1, 6.1, 2.5_
  
  - [x] 4.2 Implement run_event_loop method
    - Subscribe to broadcast channel
    - Receive MirrorEvent and call publish_delta
    - Run indefinitely until explicitly stopped
    - _Requirements: 8.6, 8.7, 9_
  
  - [x] 4.3 Implement publish_delta method
    - Compute delta from stored data
    - Emit PropertiesChanged with only changed fields
    - Increment and store sequence number
    - _Requirements: 7.3, 7.4, 7.6_

- [-] 5. Wire OVSDB event feed
  - [x] 5.1 Implement OVSDB monitor integration
    - Call `OvsdbClient::monitor_db("Open_vSwitch")` on startup
    - Convert OVSDB updates to MirrorEvent::OvsdbRow
    - Send to broadcast channel
    - Implemented as polling loop (dump_db every 5s) since daemon
      notification methods are stubs.
    - _Requirements: 3.1, 3.2, 3.5, 15_

- [x] 6. Wire NonNetDb watch feed
  - [x] 6.1 Add broadcast::Sender to NonNetDb struct
    - Add `broadcast_tx: broadcast::Sender<NonNetChanged>` field
    - Initialize in NonNetDb construction
    - _Requirements: 4.1, 11.1_
  
  - [x] 6.2 Implement NonNetDb::watch() method
    - Return `broadcast::Receiver<NonNetChanged>`
    - _Requirements: 4.2, 11.2, 13.1, 13.2, 13.3, 13.4_
  
  - [x] 6.3 Fire broadcast sender on write operations
    - Update insert/update/delete operations to fire broadcast
    - Send NonNetChanged with key and operation type
    - _Requirements: 4.3, 4.4, 4.5, 11.3, 11.4, 11.1_

- [x] 7. Wire procfs event feeds
  - [x] 7.1 Implement procfs inotify integration
    - Use inotify crate to watch /proc/meminfo and /proc/stat for ACCESS events
    - Read procfs::Meminfo on each notification
    - Convert to MirrorEvent::ProcMem
    - _Requirements: 5.3, 5.4, 12.12_
  
  - [x] 7.2 Implement procfs timer for /proc/loadavg
    - Use tokio::time::interval with 5-second duration
    - Read procfs::LoadAverage on each tick
    - Convert to MirrorEvent::ProcLoad
    - _Requirements: 5.5, 12.12_
  
  - [x] 7.3 Replace gather_meminfo, gather_cpuinfo, gather_loadavg with procfs crate
    - Replace gather_meminfo with procfs::Meminfo read
    - Replace gather_cpuinfo with procfs::CpuInfo read
    - Replace gather_loadavg with procfs::LoadAverage read
    - Convert typed structs to serde_json::Value
    - _Requirements: 13.1, 13.2, 13.3, 13.4, 13.5_

- [x] 8. Wire StateManager plugin feed
  - [x] 8.1 Add broadcast::Sender to StateManager struct
    - Add `broadcast_tx: broadcast::Sender<PluginEvent>` field
    - Initialize in StateManager construction
    - _Requirements: 6.1, 12.1_
  
  - [x] 8.2 Implement StateManager::watch() method
    - Return `broadcast::Receiver<PluginEvent>`
    - _Requirements: 6.2, 12.2, 14.1, 14.2, 14.3, 14.4_
  
  - [x] 8.3 Fire broadcast sender on register/deregister
    - Update register/deregister operations to fire broadcast
    - Send PluginEvent with plugin_id and operation type
    - _Requirements: 6.3, 6.4, 6.5, 12.3, 12.4_

- [x] 9. Wire ComponentRegistry broadcast
  - [x] 9.1 Implement ComponentRegistry event watcher
    - Use existing registry_watch() broadcast channel
    - Convert RegistryEvent to MirrorEvent::Registry
    - Send to broadcast channel
    - _Requirements: 2.5, 3.4, 16_

- [x] 10. Implement delta-only publication
  - [x] 10.1 Add current_data field to DbusMirror
    - Add `current_data: DashMap<String, (serde_json::Value, u64)>`
    - Initialize in DbusMirror::new()
    - _Requirements: 7.1_
  
  - [x] 10.2 Implement delta computation
    - Compare new data with stored data
    - Emit PropertiesChanged with only changed fields
    - _Requirements: 7.3, NFR 1.3_

- [x] 11. Implement heartbeat safety net
  - [x] 11.1 Create heartbeat.rs module
    - Implement heartbeat task that fires every 300 seconds
    - Resync objects whose sequence numbers have not advanced
    - _Requirements: 8.8, 10_

- [x] 12. Replace poll loop with event loop
  - [x] 12.1 Update start() method
    - Remove tokio::time::interval(Duration::from_secs(30)) polling loop
    - Call spawn_event_sources() to wire all event feeds
    - Call run_event_loop() to receive MirrorEvents
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7_
  
  - [x] 12.2 Update publish_object method
    - Use delta-only publication with sequence tracking
    - Update session pending event queues
    - _Requirements: 7.2, 7.5, 7.6_

- [-] 13. Replace simd_json with serde_json
  - [-] 13.1 Replace simd_json imports with serde_json
    - Remove `use simd_json::prelude::*`
    - Replace `use simd_json::OwnedValue as Value` with `use serde_json::Value`
    - _Requirements: 10.4_
  
  - [-] 13.2 Replace simd_json macros with serde_json
    - Replace `simd_json::json!` with `serde_json::json!`
    - Replace `simd_json::owned::Object` with `serde_json::Map`
    - _Requirements: 10.2, 10.3_
  
  - [-] 13.3 Replace simd_json serialization
    - Replace `simd_json::to_string` with `serde_json::to_string`
    - Replace `simd_json::from_str` with `serde_json::from_str`
    - _Requirements: 10.5, 10.6_

- [-] 14. Upgrade zbus signal emissions
  - [x] 14.1 Update InterfacesAdded/Removed signal emissions
    - Use interface proxy methods instead of SignalContext
    - Follow op-identity crate usage pattern
    - _Requirements: 9.3, 9.4, 9.5_
  
  - [x] 14.2 Update PropertiesChanged signal emissions
    - Use interface proxy methods for PropertiesChanged
    - Include only changed fields in delta publication
    - _Requirements: 7.3, 9.3_

- [-] 15. Architecture documentation
  - [ ] 15.1 Create session lifecycle documentation
    - Explain MirrorSession lifecycle (creation, subscription, event queue management, destruction)
    - Document 500-event queue limit and InterfacesRemoved on overflow
    - Document NameOwnerChanged signal handling for session cleanup
    - _Requirements: 14.1_
  
  - [ ] 15.2 Create event source mapping documentation
    - Map each data source to its event feed mechanism
    - Document OVSDB monitor_db, NonNetDb watch, procfs inotify/timer, StateManager watch, ComponentRegistry broadcast
    - Explain how poll loop is replaced by event-driven dispatch
    - _Requirements: 14.2_

- [-] 16. Property-based testing for correctness properties
  - [ ] 16.1 Write property test for Property 1: Session creation on peer activity
    - **Property 1: Session Creation on Peer Activity**
    - **Validates: Requirements 1.1, 1.2**
    - Test that sessions are created when peers call methods or subscribe to signals
    - _Requirements: 1.1, 1.2_
  
  - [ ] 16.2 Write property test for Property 2: Session isolation
    - **Property 2: Session Isolation**
    - **Validates: Requirements 1.6**
    - Test that distinct peer names maintain independent sessions
    - _Requirements: 1.6_
  
  - [ ] 16.3 Write property test for Property 3: Event queue limit enforcement
    - **Property 3: Event Queue Limit Enforcement**
    - **Validates: Requirements 1.4**
    - Test that sessions exceeding 500 events emit InterfacesRemoved and are destroyed
    - _Requirements: 1.4_
  
  - [ ] 16.4 Write property test for Property 4: Session cleanup on peer disconnection
    - **Property 4: Session Cleanup on Peer Disconnection**
    - **Validates: Requirements 1.5**
    - Test that sessions are destroyed when NameOwnerChanged indicates peer is gone
    - _Requirements: 1.5_
  
  - [ ] 16.5 Write property test for Property 5: Event enum completeness
    - **Property 5: Event Enum Completeness**
    - **Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8**
    - Test that all data source changes emit corresponding MirrorEvent variants
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8_
  
  - [ ] 16.6 Write property test for Property 6: Event sequence number inclusion
    - **Property 6: Event Sequence Number Inclusion**
    - **Validates: Requirements 7.6**
    - Test that all MirrorEvents include current sequence number for target path
    - _Requirements: 7.6_
  
  - [ ] 16.7 Write property test for Property 7: Delta-only publication
    - **Property 7: Delta-Only Publication**
    - **Validates: Requirements 7.3, NFR 1.3**
    - Test that PropertiesChanged signals contain only changed fields
    - _Requirements: 7.3, NFR 1.3_
  
  - [ ] 16.8 Write property test for Property 8: Sequence number monotonicity
    - **Property 8: Sequence Number Monotonicity**
    - **Validates: Requirements 7.4**
    - Test that sequence numbers are strictly increasing per path
    - _Requirements: 7.4_
  
  - [ ] 16.9 Write property test for Property 9: Event loop continuity
    - **Property 9: Event Loop Continuity**
    - **Validates: Requirements 8.6, 8.7**
    - Test that run_event_loop processes events indefinitely
    - _Requirements: 8.6, 8.7_
  
  - [ ] 16.10 Write property test for Property 10: Heartbeat resync
    - **Property 10: Heartbeat Resync**
    - **Validates: Requirements 8.8**
    - Test that objects with stale sequence numbers are resynced
    - _Requirements: 8.8_
  
  - [ ] 16.11 Write property test for Property 11: Event source broadcast firing
    - **Property 11: Event Source Broadcast Firing**
    - **Validates: Requirements 4.5, 6.5**
    - Test that NonNetDb and StateManager write operations fire broadcast exactly once
    - _Requirements: 4.5, 6.5_
  
  - [ ] 16.12 Write property test for Property 12: Procfs typed reading
    - **Property 12: Procfs Typed Reading**
    - **Validates: Requirements 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 13.1, 13.2, 13.3, 13.4, 13.5**
    - Test that procfs files use typed reads from procfs crate
    - _Requirements: 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 13.1, 13.2, 13.3, 13.4, 13.5_
  
  - [ ] 16.13 Write property test for Property 13: NonNetDb watch feed
    - **Property 13: NonNetDb Watch Feed**
    - **Validates: Requirements 4.1, 4.2, 4.3, 4.4, 4.5, 11.1, 11.2, 11.3, 11.4**
    - Test that NonNetDb::watch() returns receiver and write operations fire broadcast
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 11.1, 11.2, 11.3, 11.4_
  
  - [ ] 16.14 Write property test for Property 14: StateManager watch feed
    - **Property 14: StateManager Watch Feed**
    - **Validates: Requirements 6.1, 6.2, 6.3, 6.4, 6.5, 12.1, 12.2, 12.3, 12.4**
    - Test that StateManager::watch() returns receiver and register/deregister fire broadcast
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 12.1, 12.2, 12.3, 12.4_
  
  - [ ] 16.15 Write property test for Property 15: OVSDB monitor feed
    - **Property 15: OVSDB Monitor Feed**
    - **Validates: Requirements 3.1, 3.2, 3.5**
    - Test that OVSDB row changes emit MirrorEvent::OvsdbRow
    - _Requirements: 3.1, 3.2, 3.5_
  
  - [ ] 16.16 Write property test for Property 16: ComponentRegistry event propagation
    - **Property 16: ComponentRegistry Event Propagation**
    - **Validates: Requirements 2.5, 3.4**
    - Test that ComponentRegistry events emit MirrorEvent::Registry
    - _Requirements: 2.5, 3.4_

- [-] 17. Unit and integration tests
  - [ ] 17.1 Write unit tests for MirrorSession
    - Test session creation with peer activity
    - Test session isolation between peers
    - Test session destruction on peer disconnection
    - _Requirements: 1.1, 1.2, 1.4, 1.5, 1.6_
  
  - [ ] 17.2 Write unit tests for MirrorEvent
    - Test all event variant construction
    - Test event serialization/deserialization
    - Test event sequence number handling
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8_
  
  - [ ] 17.3 Write unit tests for EventDispatcher
    - Test event loop processing
    - Test delta computation
    - Test broadcast channel subscription
    - _Requirements: 3.1, 3.2, 3.5, 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 6.1, 2.5_
  
  - [ ] 17.4 Write unit tests for delta publication
    - Test delta-only PropertiesChanged emission
    - Test sequence number increment
    - Test session pending queue management
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, NFR 1.3_
  
  - [ ] 17.5 Write unit tests for heartbeat task
    - Test 300-second resync interval
    - Test stale object detection
    - Test full resync on timeout
    - _Requirements: 8.8, 10_
  
  - [ ] 17.6 Write integration tests for end-to-end flow
    - Test full event flow from source to D-Bus publication
    - Test multiple peers with independent sessions
    - Test reconnection scenarios with broadcast lag
    - _Requirements: All requirements_
  
  - [ ] 17.7 Write performance tests
    - Test latency < 100ms for delta publication
    - Test 500-event queue limit
    - Test concurrent peer handling
    - _Requirements: NFR 1.1, NFR 1.2, NFR 1.3_

- [ ] 18. Checkpoint - Ensure all tests pass
  - Run `cargo test --workspace --all-targets --all-features`
  - Ensure all property tests pass (100+ iterations each)
  - Ensure all unit tests pass
  - Ensure all integration tests pass
  - Ensure performance tests meet latency requirements
  - Ask the user if questions arise.

- [ ] 19. Final checkpoint - Ensure all tests pass
  - Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - Run `cargo fmt --all -- --check`
  - Verify no simd-json references remain
  - Verify zbus 5.12 API patterns are used
  - Verify all correctness properties are tested
  - Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties (16 properties, 100+ iterations each)
- Unit tests validate specific examples and edge cases
- Integration tests validate end-to-end flows and multi-peer scenarios
- Performance tests validate latency < 100ms requirement
- All public types derive Debug as per NFR 1.1
- All code uses serde_json instead of simd-json as per Requirement 10
- All zbus signal emissions use interface proxy methods as per Requirement 9