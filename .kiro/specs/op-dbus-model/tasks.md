# op-dbus-model - Tasks

## Phase 1: Database Schema and Models

- [ ] Define the `Plugin` model and its associated SQLite schema.
- [ ] Create the `Schema` model and its associated SQLite schema with foreign key constraints.
- [ ] Implement the `create_schema` function to initialize the database (idempotently).
- [ ] Add unit tests for `Plugin` and `Schema` serialization/deserialization with `simd-json`.
- [ ] Verify database schema creation and field types.

## Phase 2: Plugin Registration and Management

- [ ] Implement the `register_plugin` method for storing and updating plugin metadata.
- [ ] Develop methods for querying and listing registered plugins.
- [ ] Create a `PluginProvider` trait for high-level plugin access.
- [ ] Add unit tests for plugin registration and retrieval.

## Phase 3: Schema Persistence and Retrieval

- [ ] Implement the `store_schema` method for persisting discovered D-Bus interface definitions.
- [ ] Develop methods for querying schemas by plugin name or service identifier.
- [ ] Build a `SchemaProvider` trait for high-level schema access.
- [ ] Add unit tests for schema storage and retrieval with referential integrity checks.

## Phase 4: Integration and Advanced Features

- [ ] Integrate with `op-introspection` for automated schema discovery and persistence.
- [ ] Implement (optional) schema versioning and migration tracking.
- [ ] Add query builders for common access patterns (e.g., search schemas by method name).
- [ ] Perform a full review of database performance and indexing.
- [ ] Write integration tests for the full plugin-to-schema persistence lifecycle.
