# op-dbus-model - Requirements

## Problem Statement

The system requires a persistent data layer to store plugin metadata and D-Bus interface schemas. This data must be easily accessible to other services (like `op-introspection` and `op-mcp`) and support asynchronous database operations.

## Goals

1.  **Plugin Persistence**: Store and manage metadata for registered D-Bus plugins/services.
2.  **Schema Storage**: Persist discovered D-Bus interface definitions (JSON).
3.  **Asynchronous Access**: Provide non-blocking database access via `sqlx` and `tokio`.
4.  **Schema Versioning**: Support automated database migrations and schema management.
5.  **Data Integrity**: Enforce referential integrity between plugins and their associated schemas.

## Functional Requirements

### FR1: Plugin Registration
- Store unique plugin identifiers (name) and D-Bus service names.
- Persist base object paths (JSON) for plugin-to-object mapping.
- Track registration timestamps.

### FR2: Schema Persistence
- Store unique schema identifiers (UUIDs).
- Maintain a foreign key relationship to the parent plugin.
- Persist full D-Bus interface definitions as JSON (using `simd-json`).
- Track schema discovery metadata (source and timestamp).

### FR3: Database Schema Management
- Provide an idempotent `create_schema` function to initialize the database.
- Support automated migrations for future schema changes.
- Ensure efficient indexing of primary and foreign key fields.

### FR4: Query API
- Provide asynchronous methods for:
    - Registering/updating plugins.
    - Storing/retrieving interface schemas.
    - Querying schemas by plugin name or service identifier.
    - Listing all registered plugins and their associated schemas.

## Non-Functional Requirements

### NFR1: Performance
- Use `sqlx` connection pooling for concurrent database access.
- Leverage `simd-json` for high-performance JSON serialization/deserialization.
- Ensure low-latency query performance for metadata and schema retrieval.

### NFR2: Reliability
- Robust error handling using `thiserror` and `anyhow`.
- Atomic database operations to maintain data consistency.
- Automatic database recovery and integrity checks provided by SQLite.

### NFR3: Maintainability
- Clear separation between data models and database access logic.
- Well-documented schema definitions and query methods.
- Comprehensive unit tests for model serialization and database operations.

## Success Criteria

1.  Successful database initialization and schema creation.
2.  Correct persistence and retrieval of plugin and schema records.
3.  Referential integrity enforced between plugins and schemas.
4.  Smooth integration with `op-introspection` and other data-dependent services.
