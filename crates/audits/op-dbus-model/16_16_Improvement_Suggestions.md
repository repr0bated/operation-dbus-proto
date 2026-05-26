1. Schema-as-Code Compliance | Model data contracts as versioned schemas (e.g., Protocol Buffers) rather than using ad-hoc Rust structs with unstructured JSON types (`simd_json::OwnedValue`). Currently, the core contracts use loose JSON definitions, lacking structural versioning and API compatibility guarantees across dynamic loads. | crates/op-dbus-model/src/models.rs:5

2. Architectural Improvement | Separate database schema definition and management into formal SQLx migrations rather than executing raw, ad-hoc `CREATE TABLE IF NOT EXISTS` strings directly during library setup. This allows for schema version tracking, rolling forward/backward, and proper database state control. | crates/op-dbus-model/src/lib.rs:8

3. Architectural Alignment | Resolve the structural mismatch between the `plugins` database schema and the data serialized into it. The column is named `base_object`, but `upsert_document` writes the entire JSON-serialized representation of `PluginCatalogDocument` (which duplicates fields like `service_name` and contains its own nested `PluginSchema`) into it. | crates/op-dbus-model/src/lib.rs:50

4. API Ergonomics | Replace the catch-all `anyhow::Result` type in public database traits and catalog methods with a strongly typed custom enum using `thiserror`. Downstream projection and rendering layers cannot programmatically match against specific failures like constraint violations versus serialization errors. | crates/op-dbus-model/src/lib.rs:9

5. API Ergonomics | Introduce a validated Builder pattern or constructor validation for path variables in `PluginCatalogDocument` to prevent the persistence of malformed D-Bus or file storage paths. | crates/op-dbus-model/src/models.rs:27

6. Performance | Optimize performance and reduce memory allocations by using zero-copy types (such as `Arc<str>` or `Bytes` rather than heap-allocated `String` instances) inside parsed structures, especially when deserializing large numbers of plugins in rapid succession. | crates/op-dbus-model/src/models.rs:27

7. Performance | Implement a batch upsert API for `SqlitePluginCatalog` to register multiple plugins in a single SQLite transaction. The current design executes isolated serial queries, introducing excessive filesystem sync overhead during workspace plugin discovery. | crates/op-dbus-model/src/lib.rs:50

8. Observability | Add structured tracing using `#[tracing::instrument]` on database access operations, capturing crucial metadata fields (such as `plugin_name` and `service_name`) as structured keys rather than letting them run un-monitored. | crates/op-dbus-model/src/lib.rs:50

9. Observability | Eliminate raw `eprintln!` statements in library code when deserialization fails, replacing them with standard structured `tracing::warn!` calls so that validation errors can be correctly ingested and routed by the centralized logging system. | crates/op-dbus-model/src/lib.rs:96

10. Storage | Create explicit indices on foreign key constraints (such as `schemas(plugin_name)`) in the database initialization sequence to prevent table scans as the count of dynamic plugin schemas grows. | crates/op-dbus-model/src/lib.rs:22