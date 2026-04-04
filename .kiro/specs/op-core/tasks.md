# op-core Tasks

## Phase 1: Foundation
- [ ] Implement `OpError` using `thiserror`.
- [ ] Define the `SelfIdentity` structure and `BusType` enum.
- [ ] Implement core security levels and permission checks.

## Phase 2: Communication and State
- [ ] Implement the `message.rs` module for internal communication formats.
- [ ] Implement the `connection.rs` module for `zbus` connection management.
- [ ] Provide unified configuration loading and validation.

## Phase 3: Execution and Telemetry
- [ ] Implement the `execution.rs` module for task orchestration.
- [ ] Integrate with `op-execution-tracker` for job telemetry and monitoring.

## Phase 4: Utilities and Quality
- [ ] Add comprehensive unit and integration tests for all modules.
- [ ] Ensure full JSON-serializable structures for all core models.
- [ ] Conduct final performance audit of JSON parsing and D-Bus calls.

## Success Metrics
- Successful end-to-end communication between two services using `op-core` types.
- < 10ms latency for core message parsing and validation.
- Zero compile-time warnings and passing all tests.
