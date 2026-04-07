# op-state-store - Requirements

## Problem Statement

The system requires a centralized and persistent way to track the state of execution jobs, manage plugin configurations, and maintain a verifiable audit trail of all system mutations. Without a robust state store, long-running operations could be lost on restart, and the system would lack the necessary transparency and reproducibility for enterprise-grade D-Bus orchestration.

## Goals

1.  **Durable Job Ledger**: Provide a persistent record of all MCP execution jobs and their state transitions (Pending → Running → Completed/Failed).
2.  **Plugin State Management**: Securely store and retrieve snapshots of plugin-specific state to support recovery and synchronization.
3.  **Verifiable Audit Trail**: Maintain a tamper-evident audit log of all system operations, including a blockchain-style event chain for compliance.
4.  **Schema Enforcement**: Validate plugin configurations and job arguments against strictly defined JSON schemas.
5.  **Disaster Recovery**: Enable full export and import of the system state for backup and recovery scenarios.
6.  **High-Performance Persistence**: Support high-concurrency database operations using SQLite and real-time event streaming via Redis.

## Functional Requirements

### FR1: Execution Job Tracking
- Store detailed information for each execution job: tool name, arguments, current status, result, and timestamps.
- Support atomic status updates and result persistence.
- Provide APIs for querying, filtering, and counting jobs by status.
- Implement automated cleanup of old, completed/failed jobs.

### FR2: Plugin and Tool Persistence
- Manage a registry of available tools with their definitions and schemas.
- Store and retrieve plugin-specific state snapshots (JSON).
- Support versioned tool definitions and schema-aware migrations.

### FR3: Audit and Compliance
- Log every state-changing operation to a persistent audit log.
- Generate a blockchain-style "event chain" with cryptographic hashes (Merkle batches) to ensure the integrity of the audit trail.
- Store system footprints and hashes for verifiable state transitions.
- Provide a queryable audit log for specific plugins or operations.

### FR4: Rollback and Checkpoints
- Support creating named or timed checkpoints for plugin state.
- Provide a mechanism to retrieve and restore previous state snapshots to support rollbacks.

### FR5: Disaster Recovery
- Implement canonical export of all objects, execution records, and the event chain to a portable format.
- Support importing canonical exports to restore system state.
- Track host-specific information and system dependencies for recovery context.

### FR6: Real-time Streaming
- Integrate with Redis Streams to provide real-time notifications of state changes and job updates.

## Non-Functional Requirements

### NFR1: Data Integrity
- Enforce referential integrity between jobs, plugins, and audit records.
- Use atomic transactions (SQLite) to ensure state consistency during failures.
- Implement cryptographic hashing for the event chain to detect tampering.

### NFR2: Performance
- High-throughput asynchronous database operations using `sqlx`.
- Efficient JSON processing with `simd-json`.
- Low-latency real-time streaming via Redis.

### NFR3: Security
- Use prepared statements to prevent SQL injection.
- Ensure sensitive state data is handled securely during export/import.

### NFR4: Observability
- Integrated Prometheus metrics for job counts and operation latency.
- OpenTelemetry tracing for database and stream operations.

## Success Criteria

1.  Execution jobs correctly transition through all states and persist across system restarts.
2.  Plugin state is accurately stored, retrieved, and recoverable via checkpoints.
3.  The audit log provides a complete, verifiable record of all system mutations.
4.  JSON schemas are strictly enforced for all job arguments and plugin configurations.
5.  Disaster recovery exports are complete and can be successfully imported to restore state.
6.  Real-time state updates are correctly propagated via Redis Streams.
