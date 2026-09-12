# Architecture & Module Map

### Overview
The `op-introspection` crate provides low-level system and D-Bus introspection capabilities for the `op-dbus` control plane. It contains sub-components to scan, parse, cache, and index D-Bus interfaces, alongside system-level hardware and CPU feature detectors designed to find hidden BIOS settings. 

### Module Tree
```
crates/op-introspection/
├── Cargo.toml
└── src/
    ├── lib.rs (Library Root)
    ├── cache.rs (Introspection Cache)
    ├── indexer.rs (rusqlite/FTS5 search index)
    ├── indexer_manager.rs (Async pool for FTS5 queries)
    ├── parser.rs (Ad-hoc parser placeholder)
    ├── projection.rs (Snowball-backed state persistence)
    ├── scanner.rs (D-Bus XML collector via quick-xml)
    └── mod.rs (Stray system introspection module root)
        ├── cpu_features.rs (CPU/BIOS security check)
        └── hierarchical.rs (BTRFS cached D-Bus tree scan)
```

### Entry Points
*   **Library Entry Point**: `crates/op-introspection/src/lib.rs`
*   **Stray Entry Point**: `crates/op-introspection/src/mod.rs` (Contains the implementation of the system introspector, but is not currently linked via a `mod` declaration in `lib.rs`).

### Notes
*   **Stray Module Root**: `crates/op-introspection/src/mod.rs` is named such that it acts as a module root, but it is never imported in `lib.rs`. Consequently, `cpu_features.rs` and `hierarchical.rs` are completely isolated from the compiled binary.
*   **Concurrency**: The crate mixes synchronous SQLite handles (`rusqlite`) with asynchronous runtimes via `spawn_blocking` wrappers.

---

# Security & Quality Audit

## 1. Critical Vulnerabilities

### Path Traversal in Timestamp Snapshot Loader
*   **Citation**: `crates/op-introspection/src/hierarchical.rs:608-612`
*   **Impact**: Arbitrary file read of any JSON file accessible to the process user.
*   **Description**:
    The function `load_by_timestamp` accepts an unsanitized `timestamp` parameter from the caller, replaces colons with hyphens, and directly joins it with `cache_dir`:
    ```rust
    let filename = format!("{}.json", timestamp.replace(':', "-"));
    let path = self.cache_dir.join("introspection").join(&filename);
    let json = tokio::fs::read_to_string(&path).await?;
    ```
    If `timestamp` contains path traversal sequences such as `../../../../etc/shadow` (or any other JSON-formatted configuration file), `PathBuf::join` will resolve the parent references and escape the sandbox directory. While the file is parsed as `HierarchicalIntrospection`, a failure to parse can leak content, and any valid JSON configuration or credentials files on the filesystem could potentially be successfully extracted.

### Unsanitized Service Names in DBus State Persistence
*   **Citation**: `crates/op-introspection/src/projection.rs:114-120`
*   **Impact**: Arbitrary file write / path traversal via `write_state`.
*   **Description**:
    In `introspect_and_persist`, the code constructs a key for BTRFS state persistence using the `service` and `path` variables:
    ```rust
    let state_key = format!(
        "dbus/{}/{}",
        service.replace('.', "_"),
        path.replace('/', "_")
    );
    ```
    While `path` replaces `/` with `_`, the `service` parameter is only replacing `.` with `_`. If a malicious actor passes a `service` name containing path traversal directory separators (e.g. `../../../../etc/cron.d/malicious_job`), `state_key` will resolve outside the intended BTRFS state subvolume. This key is then passed directly to `bc.write_state(&state_key, &json).await`, allowing an attacker to write arbitrary JSON structures into system directories.

---

## 2. Schema-as-Code Violations

### Ad-hoc JSON Serialization of System Configurations
*   **Citation**: `crates/op-introspection/src/mod.rs:16-107`
*   **Impact**: High maintenance overhead, schema drift, lack of interoperability with security scanners.
*   **Description**:
    The `SystemIntrospector` reports its entire layout via `IntrospectionReport`, `SystemConfiguration`, and `HardwareInfo`. Rather than serializing these structured security assessments using a defined schema standard (e.g., **NIST OSCAL** Component Definitions / Assessment Results), they are declared as ad-hoc, unversioned Rust structs with basic JSON serialization. 
*   **Remediation**: Re-express data contracts using Protocol Buffers or standardized OSCAL structures to ensure external systems can ingest vulnerability and BIOS locking configurations safely.

### Ad-hoc Serialization of DBus Hierarchical Tree
*   **Citation**: `crates/op-introspection/src/hierarchical.rs:20-137`
*   **Impact**: Potential state-restore incompatibility across software versions.
*   **Description**:
    The D-Bus tree structure is flattened into an ad-hoc JSON structure (`HierarchicalIntrospection`) and written to disk without versioning. Any change to the Rust structure fields will render old cached trees unparseable or corrupted, causing potential system restore failures when calling `load_latest`.

---

## 3. High & Medium Risk Findings

### Concurrent SQLite Connection Creation & DB Lock DoS
*   **Citation**: `crates/op-introspection/src/indexer_manager.rs:44-98`
*   **Impact**: Denial of Service (SQLITE_BUSY / locked database).
*   **Description**:
    The `IndexerManager` is designed as a thread-safe coordinator of the FTS5 search index. It holds a protective `_indexer` mutex containing a shared `DbusIndexer` instance. However, every public query function (`build_index`, `search_methods`, `search_properties`, `search_all`, etc.) completely ignores the `_indexer` mutex. Instead, they clone the `db_path` and call:
    ```rust
    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let indexer = DbusIndexer::new(&db_path).await?;
            ...
        })
    })
    ```
    Because `DbusIndexer::new(&db_path)` is executed on every single query inside parallel blocking threads, each call attempts to open a *new* SQLite connection and execute write-heavy operations via `execute_batch` (creating core tables, virtual FTS5 tables, and active triggers). Concurrent search queries will trigger multiple active connection allocations trying to write schemas simultaneously, leading to `database is locked` transactional panic/failures.

### Unbounded Cache Memory Growth
*   **Citation**: `crates/op-introspection/src/cache.rs:11-39`
*   **Impact**: Eventual Out-of-Memory (OOM) crash under prolonged runtime.
*   **Description**:
    `IntrospectionCache` utilizes a raw `HashMap` to store D-Bus interface scanning results. It provides no eviction mechanisms, size limits, or Time-To-Live (TTL) policies. In complex systems where services generate dynamic path locations (e.g., systemd transient scopes and processes), the cache will continually ingest unique keys and grow indefinitely, causing memory exhaustion.

---

## 4. Quality & Cleanliness Findings

### Stray Unreachable Source Files
*   **Citation**: `crates/op-introspection/src/lib.rs:1`
*   **Impact**: Developer confusion, uncompiled codebase sections, dead code.
*   **Description**:
    `mod.rs` acts as a second module root within `src/` but is never declared in `lib.rs`. As a result, both `cpu_features.rs` and `hierarchical.rs` are excluded from compilation. 
*   **Remediation**: Explicitly register `mod system;` inside `lib.rs` and rename `mod.rs` to `system.rs` to adhere to modern idiomatic Rust layout standards.

### Unused Field `_indexer` in `IndexerManager`
*   **Citation**: `crates/op-introspection/src/indexer_manager.rs:17`
*   **Impact**: Code pollution.
*   **Description**:
    The manager allocates and holds `_indexer: Arc<Mutex<Option<DbusIndexer>>>` inside its struct, but this field is never utilized. This represents dead code left behind by an incomplete refactoring of async-to-sync bridging.

### Non-Standard Nested Runtime Block-On
*   **Citation**: `crates/op-introspection/src/indexer_manager.rs:43-47`
*   **Impact**: Performance penalty.
*   **Description**:
    Spawning a `spawn_blocking` task only to capture a handle to the current tokio runtime and block on a nested async task is an anti-pattern. SQLite connections are naturally synchronous; blocking the executing thread pool with nested runtime context transitions incurs unnecessary overhead. The DB queries should be rewritten to run synchronously within the `spawn_blocking` thread rather than nesting an async executor inside a blocking task.