# Requirements Document

## Introduction

The 3tched Schema Shuttle and Xray Injection Pipeline subsystem implements a state-aware network transport layer that cryptographically binds ephemeral WireGuard user sessions to the authoritative JSON-RPC mutation pipeline. This feature eliminates legacy SQL polling and D-Bus watchers in favor of zero-copy shared memory access, ensuring minimal overhead and maximum accountability.

The system consists of four core components:
- **The Sled**: A 1:1 zero-copy shared memory layout mapping directly to the active `PluginSchema`
- **The Shuttle**: A pure Rust binary that performs zero-copy reads and passes cryptographic footprints to Xray
- **Xray**: An in-memory payload carrier that injects Ghostbridge headers into gRPC metadata
- **JSON-RPC Mutation Pipeline**: The authoritative path for all state changes, mutation events, approvals, trace updates, and Xray injection triggers

## Glossary

- **The Sled**: A 1:1 zero-copy shared memory layout (`#[repr(C)]`) residing in `/dev/shm/plugin_schema.dat`. Contains the user's WireGuard public key, mutation index, and Blake3 hashed footprint (the "thought").
- **The Shuttle**: A pure Rust binary that performs zero-copy read of the Sled via raw pointer cast. Extracts the footprint and trace ID, and passes them to Xray securely via environment variables.
- **Xray**: A payload carrier that runs entirely in memory. Takes the full binary gRPC payload and injects `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID` into gRPC metadata via outbound `httpSettings`. Sits before the WARP tunnel and gRPC Bridge in the datapath.
- **JSON-RPC Mutation Pipeline**: The authoritative path for all state changes, mutation events, approvals, trace updates, and Xray injection triggers. If something is not represented in validated schema, it does not exist.
- **A.N.N.A. Scribe**: The Axon Network Notary Arbitrator who notarizes the WireGuard identity and handles the "Snowball" session.
- **The Compliance Engine**: A set of dedicated attorneys for regulatory compliance: Olivia Scal (OSCAL), E.U.gene Risk (EU AI Act), Penny Privacy (GDPR), and Reggie O.P.A. (Cloud Prosecutor).
- **The Strike/Etch**: The process of generating the cryptographic footprint hash.
- **The Snowball**: An appended session ledger tracking all identity mutations.
- **The Accountability Loop**: The mechanism that injects Ghostbridge headers into Xray's gRPC metadata, maintaining end-to-end auditability from the local client through the gRPC bridge.
- **gRPC Bridge**: The authoritative gateway that receives payloads via the WARP tunnel, validates cryptographic headers, and performs routing to internal services.

## Requirements

### Requirement 1: Zero-Copy Shared Memory Sled

**User Story:** As a system component, I want to access the active `PluginSchema` via zero-copy shared memory, so that I avoid expensive SQL polling and D-Bus watchers.

#### Acceptance Criteria

1. WHEN the system initializes, THE Sled SHALL map `/dev/shm/plugin_schema.dat` as a zero-copy shared memory region
2. WHILE the Sled is mapped, THE Sled SHALL provide direct access to the `PluginSchema` via `#[repr(C)]` layout
3. THE Sled SHALL contain the user's WireGuard public key, the mutation index, and the Blake3 hashed footprint
4. IF the shared memory region cannot be mapped, THEN THE Sled SHALL return an error with a descriptive message
5. WHERE the `PluginSchema` is updated, THE Sled SHALL reflect the changes in the shared memory region

### Requirement 2: The Shuttle Courier

**User Story:** As a Rust binary, I want to perform zero-copy reads of the Sled and extract cryptographic footprints, so that I can securely pass them to Xray without triggering any disk I/O.

#### Acceptance Criteria

1. WHEN the Shuttle starts, IT SHALL read the Sled via raw pointer cast without copying data
2. THE Shuttle SHALL extract the footprint (Blake3 hash) and trace ID from the Sled
3. THE Shuttle SHALL pass the footprint to Xray via the `GB_FOOTPRINT` environment variable
4. THE Shuttle SHALL pass the trace ID to Xray via the `GB_TRACE_ID` environment variable
5. IF the Sled is corrupted or unreadable, THEN THE Shuttle SHALL return an error with a descriptive message
6. WHERE the Shuttle detects any disk I/O that could trigger unintended state changes, THEN THE Shuttle SHALL abort and log the event

### Requirement 3: Xray gRPC Header Injection

**User Story:** As Xray, I want to inject Ghostbridge headers into gRPC metadata, so that the Accountability Loop is maintained across all service calls.

#### Acceptance Criteria

1. WHEN Xray receives a gRPC payload, IT SHALL extract the `GB_FOOTPRINT` and `GB_TRACE_ID` environment variables
2. THE Xray outbound `httpSettings` SHALL inject `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID` into gRPC metadata
3. THE Xray SHALL target the gRPC Bridge (e.g., `10.200.0.2:50051`) via the WARP tunnel
4. THE Xray SHALL sit before the WARP tunnel in the datapath
5. IF the environment variables are missing or invalid, THEN THE Xray SHALL return an error with a descriptive message
6. WHILE processing gRPC calls, THE Xray SHALL maintain the Accountability Loop by ensuring headers are present

### Requirement 4: gRPC Bridge Validation and Routing

**User Story:** As the gRPC Bridge, I want to validate cryptographic headers and route payloads to services, so that only notarized mutations reach the internal state.

#### Acceptance Criteria

1. THE gRPC Bridge SHALL receive gRPC payloads with Ghostbridge headers via the WARP tunnel
2. THE gRPC Bridge SHALL validate `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID` against the authoritative state
3. THE gRPC Bridge SHALL perform routing to internal services ONLY after successful validation
4. IF validation fails, THE gRPC Bridge SHALL reject the payload and log an accountability violation
5. THE gRPC Bridge SHALL ensure that internal services are only reachable through this validated path

### Requirement 5: JSON-RPC Mutation Pipeline as Sole Authoritative Path

**User Story:** As the JSON-RPC mutation pipeline, I want to be the sole authorized path for all state changes, so that every mutation event is auditable and traceable.

#### Acceptance Criteria

1. WHEN a mutation event occurs, IT SHALL be submitted to the JSON-RPC mutation pipeline as the ONLY authorized path
2. THE JSON-RPC mutation pipeline SHALL validate the mutation against the `PluginSchema` before committing
3. THE JSON-RPC mutation pipeline SHALL generate a mutation event record with timestamp, proposer, and validation result
4. WHERE a mutation is approved, THE JSON-RPC mutation pipeline SHALL update the `mutation_index` in the Sled
5. WHERE a mutation is rejected, THE JSON-RPC mutation pipeline SHALL log the rejection reason and maintain the current state
6. THE JSON-RPC mutation pipeline SHALL maintain an immutable audit trail of all mutation proposals, validations, approvals, and commits
7. IF a state change is attempted outside the JSON-RPC mutation pipeline, THEN IT SHALL be rejected

### Requirement 6: Schema-Driven Identity Management

**User Story:** As A.N.N.A. Scribe, I want to notarize WireGuard identities using the `PluginSchema` as the single source of truth, so that all identities are cryptographically bound to the system state.

#### Acceptance Criteria

1. WHEN a WireGuard identity is presented, THE A.N.N.A. Scribe SHALL validate it against the `PluginSchema`
2. THE A.N.N.A. Scribe SHALL generate a "Snowball" session ledger by appending the identity to the mutation index
3. THE A.N.N.A. Scribe SHALL perform the Strike/Etch to generate the Blake3 hashed footprint
4. WHERE the `PluginSchema` is updated, THE A.N.N.A. Scribe SHALL re-notarize all active identities
5. IF the `PluginSchema` is invalid or missing, THEN THE A.N.N.A. Scribe SHALL reject the identity
6. WHILE the Snowball is active, THE A.N.N.A. Scribe SHALL maintain the session ledger

### Requirement 7: Zero-Disk I/O Overhead

**User Story:** As the system, I want to avoid any disk I/O during identity extraction and Xray header injection, so that NVMe I/O is preserved for the Btrfs vectorized footprint transport.

#### Acceptance Criteria

1. WHEN identity data is extracted, THE system SHALL use in-memory environment variables (or tmpfs) instead of disk writes
2. WHERE the system detects any disk I/O that could trigger unintended state changes, THEN THE system SHALL abort and log the event
3. THE system SHALL preserve NVMe I/O strictly for the Btrfs vectorized footprint transport (blockchain)
4. WHILE processing identity data, THE system SHALL avoid any disk I/O that could trigger state changes
5. IF the system cannot avoid disk I/O, THEN THE system SHALL return an error with a descriptive message

### Requirement 8: Compliance Engine Integration

**User Story:** As the Compliance Engine, I want to validate all operations against regulatory requirements, so that the system remains compliant with OSCAL, EU AI Act, GDPR, and Cloud Prosecutor standards.

#### Acceptance Criteria

1. WHEN an operation is initiated, THE Compliance Engine SHALL validate it against the `PluginSchema`
2. THE Compliance Engine SHALL include dedicated attorneys: Olivia Scal (OSCAL), E.U.gene Risk (EU AI Act), Penny Privacy (GDPR), and Reggie O.P.A. (Cloud Prosecutor)
3. WHERE a regulatory violation is detected, THEN THE Compliance Engine SHALL return an error with a descriptive message
4. THE Compliance Engine SHALL maintain an audit log of all validation decisions
5. IF the Compliance Engine cannot validate an operation, THEN THE Compliance Engine SHALL return an error with a descriptive message
6. WHILE the system is active, THE Compliance Engine SHALL continuously monitor for regulatory violations

### Requirement 9: AI Accountability

**User Story:** As the AI accountability system, I want to ensure AI operations are constrained, auditable, traceable, explainable, and accountable by design, so that AI never operates as a black box.

#### Acceptance Criteria

1. WHEN AI generates a recommendation, IT SHALL include metadata about the recommendation source, confidence level, and supporting evidence
2. WHERE AI interprets a schema or state, IT SHALL provide an explainability record linking the interpretation to the source schema
3. WHEN AI assists with validation, IT SHALL flag any ambiguous or uncertain decisions for human review
4. THE AI accountability system SHALL maintain a trace ID linking all AI-assisted decisions to the original request
5. WHERE a human override occurs, THE AI accountability system SHALL record the override reason and maintain the original AI recommendation for audit
6. THE AI accountability system SHALL enforce that AI can recommend, interpret, or assist but never bypass validation and authorization controls

### Requirement 10: Trace Propagation

**User Story:** As the trace propagation system, I want to maintain trace linkage across all identity mutations, state transitions, and Xray injection events, so that audit and semantic memory linkage are preserved.

#### Acceptance Criteria

1. WHEN an identity mutation occurs, IT SHALL generate a new trace ID or propagate the existing trace ID
2. THE trace ID SHALL be included in all mutation event records, Xray injection headers, and audit log entries
3. WHERE a trace ID is propagated, IT SHALL maintain the causal chain linking all related events
4. THE trace ID SHALL be stored in the Sled's trace_id field for zero-copy access
5. THE trace ID SHALL be included in the Snowball session ledger for audit purposes
6. WHERE semantic memory linkage is required, THE trace ID SHALL be used to query the Qdrant index

### Requirement 11: Canonicalization and Hashing

**User Story:** As the canonicalization system, I want to canonicalize schema state before hashing, so that the hashed footprint is deterministic and reproducible.

#### Acceptance Criteria

1. WHEN schema state is prepared for hashing, IT SHALL be canonicalized to a deterministic byte representation
2. THE canonicalization process SHALL preserve all semantic meaning while eliminating non-essential variations
3. THE hashed footprint SHALL be computed using Blake3 or SHA-256 on the canonicalized schema state
4. WHERE the schema state changes, THE hashed footprint SHALL change deterministically
5. THE canonicalization and hashing process SHALL be idempotent and reproducible
6. THE hashed footprint SHALL be stored in the Sled's hashed_footprint field for zero-copy access

### Requirement 12: Mutation-Index-Driven Updates

**User Story:** As the mutation-index monitoring system, I want to detect mutation index changes from the JSON-RPC mutation pipeline and trigger state updates, so that runtime identity state stays in sync with schema state.

#### Acceptance Criteria

1. WHEN the mutation index in the Sled changes, IT SHALL trigger a state update event
2. THE state update event SHALL include the old mutation index, new mutation index, and timestamp
3. WHERE a state update is triggered, THE system SHALL recalculate the hashed footprint
4. WHERE the hashed footprint changes, THE system SHALL update the Sled and notify relevant components
5. THE mutation-index-driven update process SHALL be efficient and responsive
6. IF the update fails, THEN THE system SHALL return an error with a descriptive message

### Requirement 13: Xray Environment Injection and Reload

**User Story:** As the Xray management system, I want to dynamically inject environment variables into Xray and trigger reloads, so that Xray always uses the latest cryptographic footprints.

#### Acceptance Criteria

1. WHEN a new footprint is generated, THE system SHALL update the Xray environment variables
2. THE system SHALL trigger an Xray reload (e.g., via signal or API call) to pick up the new environment variables
3. THE reload process SHALL be seamless and non-disruptive to active gRPC calls
4. THE system SHALL verify that Xray is using the latest footprint after a reload
5. IF the reload fails, THEN THE system SHALL return an error with a descriptive message
6. WHILE the system is active, THE system SHALL ensure Xray environment variables are consistent with the Sled

### Requirement 14: Error Handling and Recovery

**User Story:** As the error handling system, I want to gracefully handle errors and recover from failures, so that the subsystem remains robust and reliable.

#### Acceptance Criteria

1. WHEN an error occurs, THE system SHALL log the error with a descriptive message and relevant context
2. THE system SHALL implement retry mechanisms for transient failures
3. WHERE a critical failure occurs, THE system SHALL transition to a safe state and notify administrators
4. THE system SHALL support automated recovery from common failure scenarios
5. THE error handling and recovery process SHALL be auditable and traceable
6. IF the system cannot recover, THEN IT SHALL fail gracefully and maintain data integrity

### Requirement 15: Observability and Monitoring

**User Story:** As a system administrator, I want to monitor the health and performance of the subsystem, so that I can ensure it is operating correctly and efficiently.

#### Acceptance Criteria

1. THE system SHALL provide health status indicators for all core components
2. THE system SHALL collect and expose performance metrics (e.g., latency, throughput, resource usage)
3. THE system SHALL support structured logging with configurable log levels
4. THE system SHALL provide dashboards for visualizing health and performance data
5. THE system SHALL trigger alerts for critical health or performance issues
6. THE observability and monitoring data SHALL be auditable and traceable

### Requirement 16: Security and Policy Enforcement

**User Story:** As a security administrator, I want to enforce security policies across the subsystem, so that the system remains secure and protected from unauthorized access.

#### Acceptance Criteria

1. THE system SHALL enforce least privilege access for all components
2. THE system SHALL validate all inputs and sanitize all outputs
3. THE system SHALL use secure communication channels for all inter-component interactions
4. THE system SHALL implement encryption for sensitive data at rest and in transit
5. THE security and policy enforcement mechanisms SHALL be auditable and traceable
6. IF a security violation is detected, THEN THE system SHALL block the operation and log the event

### Requirement 17: Compliance and Explainability

**User Story:** As a compliance officer, I want to ensure the system remains compliant with regulatory requirements and provides explainability for all decisions, so that I can demonstrate compliance to auditors.

#### Acceptance Criteria

1. THE system SHALL generate compliance records for all auditable operations
2. THE system SHALL provide explainability records for all AI-assisted decisions
3. THE system SHALL support automated compliance auditing
4. THE compliance and explainability records SHALL be immutable and tamper-evident
5. THE system SHALL provide a compliance dashboard for visualizing compliance status
6. IF a compliance violation is detected, THEN THE system SHALL flag the violation and notify administrators

### Requirement 18: Testing and Simulation

**User Story:** As a developer, I want to test and simulate the subsystem, so that I can ensure it is correct, reliable, and performant.

#### Acceptance Criteria

1. THE system SHALL include comprehensive unit tests for all components
2. THE system SHALL include integration tests for end-to-end flows
3. THE system SHALL include performance tests for evaluating latency and throughput
4. THE system SHALL support simulation of various failure scenarios
5. THE system SHALL include property-based tests for validating system invariants
6. THE testing and simulation results SHALL be auditable and traceable

### Requirement 19: Documentation

**User Story:** As a user or developer, I want to access comprehensive documentation, so that I can understand and use the subsystem effectively.

#### Acceptance Criteria

1. THE system SHALL include reference documentation for all APIs and components
2. THE system SHALL include developer documentation for building and extending the subsystem
3. THE system SHALL include user documentation for configuring and operating the subsystem
4. THE documentation SHALL be accurate, up-to-date, and easy to navigate
5. THE documentation SHALL include examples and tutorials for common use cases
6. THE documentation SHALL be auditable and traceable

### Requirement 20: Deployment

**User Story:** As a system administrator, I want to deploy and manage the subsystem effectively, so that I can ensure it is available and reliable.

#### Acceptance Criteria

1. THE system SHALL support automated installation and configuration
2. THE system SHALL support seamless upgrades with minimal downtime
3. THE system SHALL provide configuration management tools for managing system settings
4. THE deployment and management process SHALL be auditable and traceable
5. THE system SHALL support deployment in various environments (e.g., local, cloud, hybrid)
6. IF a deployment failure occurs, THEN THE system SHALL support automated rollback to a previous stable state
