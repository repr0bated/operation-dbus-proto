### Async & Concurrency Metrics

*   **`async fn` Count:** 4
    *   `create_schema` (`crates/op-dbus-model/src/lib.rs:8`)
    *   `upsert_document` (`crates/op-dbus-model/src/lib.rs:53`)
    *   `get_document` (`crates/op-dbus-model/src/lib.rs:75`)
    *   `list_documents` (`crates/op-dbus-model/src/lib.rs:90`)
*   **`tokio::spawn` Count:** 0
*   **`spawn_blocking` Count:** 0

---

### Schema-as-Code Compliance & Security Audit

#### 1. Ad-Hoc Data Contracts and Unstructured JSON Definitions
*   **Severity:** Medium
*   **Citations:** 
    *   `crates/op-dbus-model/src/models.rs:5-11`
    *   `crates/op-dbus-model/src/models.rs:13-21`
    *   `crates/op-dbus-model/src/models.rs:26-44`
*   **Description:** 
    The codebase violates the schema-as-code discipline. Instead of defining the plugin configurations, schemas, and footprint definitions via structured, versioned Protocol Buffers or OSCAL-compliant formats, they are defined using ad-hoc Rust structs (`PluginCatalogDocument`) and raw, unstructured JSON payloads (`simd_json::OwnedValue` for `Plugin::base_object` and `Schema::definition`).
*   **Impact:** 
    Using unversioned, unstructured payloads introduces structural drift between the persisting database catalog layer, D-Bus/gRPC serialization logic, and downstream projection consumers. Upgrades to plugin footprints risk silent breakage or runtime parsing failures.

---

#### 2. Dead / Incomplete Database Schema for `schemas` Table
*   **Severity:** Low
*   **Citations:** 
    *   `crates/op-dbus-model/src/lib.rs:20-33`
*   **Description:** 
    The `create_schema` function creates a `schemas` table with a foreign key referencing the `plugins` table. However, the persistence engine struct `SqlitePluginCatalog` only contains methods for upserting, getting, and listing records within the `plugins` table. There are no functions provided to insert, query, or manage records in the `schemas` table, rendering the database schema dead code.
*   **Impact:** 
    Unused tables increase database maintenance overhead and indicate a fragmented implementation where plugin schema details may not be correctly recorded or indexed.

---

#### 3. Silent Deserialization Failures with Direct `stderr` Logging
*   **Severity:** Medium
*   **Citations:** 
    *   `crates/op-dbus-model/src/lib.rs:98-103`
*   **Description:** 
    In `list_documents`, if a retrieved database record fails to deserialize from JSON via `serde_json::from_str(&encoded)`, the execution catches the error, writes a diagnostics message to `stderr` using `eprintln!`, and silently skips the invalid record:
    ```rust
    Err(error) => {
        eprintln!(
            "Skipping stale plugin catalog document '{}': {}",
            name, error
        );
    }
    ```
*   **Impact:** 
    Silently discarding stale or corrupted database records hides underlying schema migration errors or data corruption events from the rest of the application. Downstream consumers will observe a partially empty catalog list instead of a clear, actionable propagation error. Additionally, using `eprintln!` bypasses structured workspace diagnostics layers like `tracing`.

---

#### 4. Unbounded Query & Sync JSON Processing inside Async Reactor
*   **Severity:** Low
*   **Citations:** 
    *   `crates/op-dbus-model/src/lib.rs:90-108`
*   **Description:** 
    The `list_documents` function performs an unbounded query returning all rows sorted by name, then loops through the result set executing synchronous `serde_json::from_str` CPU-bound parsing operations on the Tokio reactor thread.
*   **Impact:** 
    If the plugin catalog grows significantly, retrieving and synchronously parsing all documents concurrently can exhaust memory and block the Tokio executor's thread, causing latency spikes for unrelated concurrent asynchronous tasks.