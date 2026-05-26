# Requirements Document: D-Bus Mirror Event-Session Refactoring

## Introduction

This feature refactors `crates/op-dbus-mirror` from its current poll-and-snapshot model to a fully event-driven, session-scoped architecture. The current implementation uses a 30-second polling loop to refresh the entire D-Bus object tree. The new architecture will use event feeds from all data sources (OVSDB, NonNetDb, procfs, StateManager, ComponentRegistry) to publish only deltas, with per-peer session management for efficient resource utilization.

## Glossary

- **DbusMirror**: The main service struct that manages D-Bus object publication
- **MirrorSession**: A per-peer session that tracks subscribed paths and sequence numbers
- **MirrorEvent**: Unified event enum representing all data source changes
- **OvsdbClient**: The rovs wrapper for OVSDB that provides `monitor_db()` event feed
- **NonNetDb**: Internal key-value store that needs a `watch()` change feed added
- **StateManager**: Plugin state manager that needs a `watch()` broadcast receiver
- **Sequence Number**: Monotonically increasing counter per object path for delta tracking
- **Delta Publication**: Publishing only changed fields rather than full object snapshots

## Requirements

### Requirement 1: Session Layer with Peer-Based Session Management

**User Story:** As a D-Bus client, I want to establish a session with the mirror service, so that I can subscribe to specific object paths and receive only relevant updates.

#### Acceptance Criteria

1. WHEN a peer calls any D-Bus method or subscribes to a signal, THE DbusMirror SHALL create a MirrorSession for that peer
2. THE MirrorSession SHALL track the peer's UniqueName, subscribed paths, and last acknowledged sequence numbers per path
3. WHILE a MirrorSession exists, THE DbusMirror SHALL maintain a pending event queue for that session
4. IF a session's pending event queue exceeds 500 events, THEN THE DbusMirror SHALL emit InterfacesRemoved for all subscribed paths and drop the session
5. WHEN the org.freedesktop.DBus.NameOwnerChanged signal indicates a peer is gone, THEN THE DbusMirror SHALL destroy the corresponding MirrorSession
6. WHERE multiple peers exist, THE DbusMirror SHALL maintain independent MirrorSession instances keyed by peer name string

### Requirement 2: Unified Event Enum for All Data Sources

**User Story:** As a D-Bus mirror service, I want a single event type that represents changes from all data sources, so that I can dispatch deltas through a unified event loop.

#### Acceptance Criteria

1. THE MirrorEvent enum SHALL contain variants for OVSDB row changes, NonNetDb key changes, StateManager plugin events, ComponentRegistry events, procfs memory info, procfs load average, and procfs static sections
2. WHEN an OVSDB row changes, THE EventDispatcher SHALL emit MirrorEvent::OvsdbRow with table name, UUID, and delta JSON
3. WHEN a NonNetDb key changes, THE EventDispatcher SHALL emit MirrorEvent::NonNet with key and delta JSON
4. WHEN a StateManager plugin state changes, THE EventDispatcher SHALL emit MirrorEvent::Plugin with plugin_id and delta JSON
5. WHEN the ComponentRegistry emits an event, THE EventDispatcher SHALL emit MirrorEvent::Registry with the RegistryEvent
6. WHEN /proc/meminfo changes, THE EventDispatcher SHALL emit MirrorEvent::ProcMem with delta JSON
7. WHEN /proc/loadavg changes, THE EventDispatcher SHALL emit MirrorEvent::ProcLoad with delta JSON
8. WHEN /proc/cpuinfo, /proc/version, or /proc/mounts changes, THE EventDispatcher SHALL emit MirrorEvent::ProcStatic with section name and data JSON

### Requirement 3: OVSDB Event Feed via monitor_db()

**User Story:** As a D-Bus mirror service, I want to receive OVSDB changes via the event feed, so that I can publish deltas immediately rather than waiting for the next poll cycle.

#### Acceptance Criteria

1. WHEN DbusMirror starts, THE EventDispatcher SHALL call OvsdbClient::monitor_db("Open_vSwitch") to establish an event feed
2. WHILE the event feed is active, THE EventDispatcher SHALL receive OVSDB update notifications and emit MirrorEvent::OvsdbRow
3. FOR the initial startup snapshot, THE EventDispatcher SHALL use OvsdbClient::idl() to read the in-memory IDL replica
4. IF the event feed connection is lost, THE EventDispatcher SHALL reconnect with backoff and resume receiving updates
5. WHERE multiple OVSDB tables exist, THE EventDispatcher SHALL emit separate MirrorEvent::OvsdbRow events for each table row change

### Requirement 4: NonNetDb Watch Change Feed

**User Story:** As a D-Bus mirror service, I want NonNetDb to provide a watch() method for change notifications, so that I can receive key updates without polling.

#### Acceptance Criteria

1. THE NonNetDb struct SHALL contain a broadcast::Sender<NonNetChanged> for change notifications
2. WHEN a key is inserted, updated, or deleted in NonNetDb, THE NonNetDb SHALL fire the broadcast sender with NonNetChanged
3. WHEN DbusMirror starts, THE EventDispatcher SHALL call NonNetDb::watch() to establish a change feed
4. WHILE the watch feed is active, THE EventDispatcher SHALL receive NonNetChanged notifications and emit MirrorEvent::NonNet
5. FOR ALL write operations in NonNetDb, THE NonNetDb SHALL fire the broadcast sender exactly once

### Requirement 5: Procfs Event Feeds Using inotify and procfs Crate

**User Story:** As a D-Bus mirror service, I want to receive procfs changes via inotify or timed intervals, so that I can publish host state deltas without blocking on file I/O in a polling loop.

#### Acceptance Criteria

1. THE procfs crate version 0.17 SHALL be added as a dependency to Cargo.toml
2. THE inotify crate version 0.10 SHALL be added as a dependency to Cargo.toml
3. FOR /proc/meminfo, THE EventDispatcher SHALL use inotify to watch for ACCESS events and read procfs::Meminfo on each notification
4. FOR /proc/stat, THE EventDispatcher SHALL use inotify to watch for ACCESS events and read procfs::Meminfo on each notification
5. FOR /proc/loadavg, THE EventDispatcher SHALL use a 5-second tokio::time::interval to read procfs::LoadAverage
6. FOR /proc/cpuinfo, /proc/version, and /proc/mounts, THE EventDispatcher SHALL read once at startup and re-read only on SIGHUP or explicit Refresh D-Bus call
7. WHEN procfs data changes, THE EventDispatcher SHALL emit MirrorEvent::ProcMem, MirrorEvent::ProcLoad, or MirrorEvent::ProcStatic as appropriate
8. WHERE hand-parsed text parsing currently exists in gather_meminfo, gather_cpuinfo, gather_loadavg, THE EventDispatcher SHALL replace it with typed reads from procfs crate

### Requirement 6: StateManager Plugin Feed

**User Story:** As a D-Bus mirror service, I want StateManager to provide a watch() method for plugin state changes, so that I can receive plugin registration/deregistration events without polling.

#### Acceptance Criteria

1. THE StateManager struct SHALL contain a broadcast::Sender<PluginEvent> for state change notifications
2. WHEN a plugin is registered or deregistered in StateManager, THE StateManager SHALL fire the broadcast sender with PluginEvent
3. WHEN DbusMirror starts, THE EventDispatcher SHALL call StateManager::watch() to establish a change feed
4. WHILE the watch feed is active, THE EventDispatcher SHALL receive PluginEvent notifications and emit MirrorEvent::Plugin
5. FOR ALL register/deregister operations in StateManager, THE StateManager SHALL fire the broadcast sender exactly once

### Requirement 7: Delta-Only Publication with Sequence Tracking

**User Story:** As a D-Bus mirror service, I want to publish only changed fields and track sequence numbers, so that clients can detect missed updates and I can optimize D-Bus traffic.

#### Acceptance Criteria

1. THE DbusMirror SHALL maintain a DashMap<String, (serde_json::Value, u64)> storing current data and sequence numbers per object path
2. WHEN publish_object is called, THE DbusMirror SHALL compare the new data with the stored data
3. IF fields have changed, THE DbusMirror SHALL emit PropertiesChanged for only the changed fields
4. FOR every published object, THE DbusMirror SHALL increment and store a sequence number
5. WHERE a MirrorSession exists for a peer, THE DbusMirror SHALL track the last acknowledged sequence number per path
6. FOR every MirrorEvent, THE EventDispatcher SHALL include the current sequence number for the target path
7. WHEN a client acknowledges a sequence number, THE DbusMirror SHALL update the session's last acknowledged sequence

### Requirement 8: Revised Start() with Event Loop Replacing Poll Loop

**User Story:** As a D-Bus mirror service, I want to replace the polling loop with an event-driven architecture, so that I can publish deltas immediately and reduce resource usage.

#### Acceptance Criteria

1. THE start() method SHALL call publish_startup_snapshot() once for initial publication
2. THE start() method SHALL call register_dbus_objects() to register all D-Bus interfaces
3. THE start() method SHALL call spawn_event_sources() to wire all event feeds to the broadcast sender
4. THE start() method SHALL call run_event_loop() to receive MirrorEvents and publish deltas
5. WHERE tokio::time::interval(Duration::from_secs(30)) currently exists, THE start() method SHALL NOT contain a polling loop
6. THE run_event_loop() method SHALL receive from broadcast::Receiver<MirrorEvent> and call publish_object for each delta
7. THE run_event_loop() method SHALL NOT return and SHALL run indefinitely
8. A separate heartbeat task SHALL fire every 300 seconds and resync objects whose sequence numbers have not advanced

### Requirement 9: zbus 5.12 Upgrade

**User Story:** As a D-Bus mirror service, I want to use zbus 5.12, so that I can use the updated API patterns and maintain consistency with other crates.

#### Acceptance Criteria

1. THE zbus dependency in Cargo.toml SHALL be upgraded from version 4.0 to 5.12
2. THE workspace Cargo.toml zbus pin SHALL be upgraded from 4.0 to 5.12
3. ALL InterfacesAdded, InterfacesRemoved, and PropertiesChanged signal emissions SHALL use interface proxy methods rather than SignalContext
4. WHERE zbus 4.0 patterns exist, THE DbusMirror SHALL be updated to use zbus 5.12 equivalents
5. FOR reference, THE op-identity crate's usage pattern in lib.rs SHALL be followed

### Requirement 10: Replace simd_json with serde_json

**User Story:** As a D-Bus mirror service, I want to use serde_json instead of simd_json, so that I can maintain consistency and avoid the simd-json dependency.

#### Acceptance Criteria

1. THE simd-json dependency SHALL be removed from Cargo.toml
2. ALL simd_json::json! macros SHALL be replaced with serde_json::json!
3. ALL simd_json::OwnedValue references SHALL be replaced with serde_json::Value
4. WHERE simd_json::prelude::* is imported, THE import SHALL be removed
5. FOR JSON serialization, THE serde_json::to_string SHALL be used instead of simd_json::to_string
6. FOR JSON deserialization, THE serde_json::from_str SHALL be used instead of simd_json::from_slice

### Requirement 11: NonNetDb Watch Stub Implementation

**User Story:** As a D-Bus mirror service, I want a watch() method stub in NonNetDb, so that I can wire it to the event dispatcher.

#### Acceptance Criteria

1. THE NonNetDb struct SHALL contain a broadcast::Sender<NonNetChanged> field
2. THE NonNetDb::watch() method SHALL return a broadcast::Receiver<NonNetChanged>
3. WHERE write operations (insert/update/delete) exist in NonNetDb, THE NonNetDb SHALL fire the broadcast sender
4. THE NonNetChanged type SHALL contain the key name and operation type (insert/update/delete)

### Requirement 12: StateManager Watch Stub Implementation

**User Story:** As a D-Bus mirror service, I want a watch() method stub in StateManager, so that I can wire it to the event dispatcher.

#### Acceptance Criteria

1. THE StateManager struct SHALL contain a broadcast::Sender<PluginEvent> field
2. THE StateManager::watch() method SHALL return a broadcast::Receiver<PluginEvent>
3. WHERE register/deregister operations exist in StateManager, THE StateManager SHALL fire the broadcast sender
4. THE PluginEvent type SHALL contain the plugin_id and operation type (register/deregister/update)

### Requirement 13: Procfs Helpers Using procfs Crate

**User Story:** As a D-Bus mirror service, I want to replace hand-parsed procfs functions with typed reads from the procfs crate, so that I can maintain correctness and reduce code complexity.

#### Acceptance Criteria

1. THE gather_meminfo function SHALL be replaced with a function that reads procfs::Meminfo
2. THE gather_cpuinfo function SHALL be replaced with a function that reads procfs::CpuInfo
3. THE gather_loadavg function SHALL be replaced with a function that reads procfs::LoadAverage
4. FOR each procfs read, THE helper SHALL convert the typed struct to serde_json::Value
5. WHERE /proc text parsing exists, THE helper SHALL NOT use manual string splitting

### Requirement 14: Architecture Documentation

**User Story:** As a D-Bus mirror developer, I want architecture documentation explaining the session lifecycle and event source map, so that I can understand and maintain the new architecture.

#### Acceptance Criteria

1. A document ≤ 20 lines SHALL explain the MirrorSession lifecycle (creation, subscription, event queue management, destruction)
2. A document ≤ 20 lines SHALL map each data source to its event feed mechanism (OVSDB monitor_db, NonNetDb watch, procfs inotify/timer, StateManager watch, ComponentRegistry broadcast)
3. WHERE the current poll loop exists, THE documentation SHALL explain how it is replaced by event-driven dispatch

## Non-Functional Requirements

### NFR 1: Performance

1. WHERE the current 30-second polling loop exists, THE new event-driven architecture SHALL reduce average latency from 30 seconds to under 100 milliseconds for delta publication
2. THE pending event queue per session SHALL be limited to 500 events to prevent memory exhaustion
3. FOR delta-only publication, THE PropertiesChanged signals SHALL contain only changed fields, not full object snapshots

### NFR 2: Reliability

1. WHERE an event feed connection is lost, THE EventDispatcher SHALL attempt reconnection with exponential backoff
2. IF a session's event queue overflows, THE DbusMirror SHALL emit InterfacesRemoved and drop the session to prevent resource exhaustion
3. FOR all broadcast channels, THE EventDispatcher SHALL handle lagged receivers by resyncing the full tree

### NFR 3: Maintainability

1. ALL new public types SHALL derive Debug
2. WHERE simd_json is replaced, THE code SHALL use serde_json consistently throughout
3. FOR all event sources, THE EventDispatcher SHALL have a clear mapping from source to event variant

### NFR 4: Compatibility

1. ALL existing D-Bus object paths (/org/opdbus/v1/...) SHALL remain unchanged
2. ALL existing interface names SHALL remain unchanged
3. THE GetManagedObjects, InterfacesAdded, and InterfacesRemoved methods SHALL continue to work correctly
4. THE DbusMirrorInterface::Refresh method SHALL trigger resync of a single named path only
