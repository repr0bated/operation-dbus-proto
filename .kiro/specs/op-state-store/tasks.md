# op-state-store - Tasks

## Phase 1: Core Job Persistence (SQLite)

- [ ] Implement the `ExecutionJob` model and its SQLite schema (`execution_job.rs`, `sqlite_store.rs`).
- [ ] Develop the `SqliteStore` with `sqlx` support for persistent job tracking.
- [ ] Implement atomic status updates and result persistence.
- [ ] Add unit tests for job lifecycle (save, update, retrieve).
- [ ] Verify database schema creation and field types.

## Phase 2: Plugin State and Checkpoints

- [ ] Implement the `plugin_state` and `checkpoints` tables in SQLite.
- [ ] Develop methods for saving and retrieving plugin state snapshots.
- [ ] Implement named and timed checkpoints with rollback support.
- [ ] Create the `PluginSchema` registry for schema-aware state management.
- [ ] Add unit tests for plugin state persistence and checkpointing.

## Phase 3: Verifiable Audit Trail (EventChain)

- [ ] Implement the blockchain-style `EventChain` for the audit trail (`event_chain.rs`).
- [ ] Develop Merkle batching and canonical hashing for audit log entries.
- [ ] Create the `audit_log` table in SQLite with footprint hash support.
- [ ] Implement methods for logging audit entries and verifying chain integrity.
- [ ] Verify audit log immutability and reproducibility.

## Phase 4: Schema Validation and Real-time Streaming

- [ ] Implement the `SchemaValidator` for JSON Schema enforcement (`schema_validator.rs`).
- [ ] Integrate with `jsonschema` for validating job arguments and plugin state.
- [ ] Develop the `RedisStream` for real-time notifications of state changes (`redis_stream.rs`).
- [ ] Integrate Prometheus metrics and OpenTelemetry tracing (`metrics.rs`).
- [ ] Add unit tests for schema validation and Redis streaming.

## Phase 5: Disaster Recovery and Integration

- [ ] Implement `DisasterRecovery` logic for canonical database export/import (`disaster_recovery.rs`).
- [ ] Develop the `objects` table for general object storage persistence.
- [ ] Integrate `op-state-store` with `op-mcp` and `op-gateway`.
- [ ] Conduct final security review of database performance and crypto primitive usage.
- [ ] Write end-to-end integration tests for the full state store lifecycle.

## Success Metrics

- [ ] Successful persistence and retrieval of execution jobs across restarts.
- [ ] Verified integrity of the audit log via the `EventChain`.
- [ ] Accurate JSON schema enforcement for all incoming job and plugin data.
- [ ] Real-time state updates successfully propagated to Redis consumers.
- [ ] Disaster recovery exports correctly capture and restore system state.
