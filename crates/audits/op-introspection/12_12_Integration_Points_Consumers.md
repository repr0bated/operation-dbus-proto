# op-introspection Production Security & Quality Audit

## 1. Crate & Workspace Integration

### op-introspection Dependency Map
Based on the workspace `Cargo.lock` dependency trees, the following crates in the workspace directly depend on `op-introspection`:
*   `op-chat`
*   `op-dbus` (the root control-plane control package)
*   `op-inspector`
*   `op-mcp`
*   `op-tools`
*   `op-web`

---

### Cross-Crate Circular Dependency Risk
*   **Location**: `crates/op-introspection/Cargo.toml:11`
*   **Finding**: `op-introspection` maintains a direct path dependency on `op-blockchain` (`op-blockchain = { path = "../op-blockchain" }`). 
*   **Risk Analysis**: `op-blockchain` is a low-level ledger and state storage library, whereas `op-introspection` implements high-level system analysis and D-Bus scanning. 
    *   If `op-blockchain` ever needs to reference types, parse definitions, or use schemas defined in `op-introspection` (e.g., to validate blockchain block state modifications or `dbus.schema.update` event structures), a **circular dependency cycle** will prevent compilation.
    *   This forces `op-blockchain` to handle event schema payloads as raw, unvalidated JSON strings rather than typed schema objects, breaking compilation-level type guarantees across the Control Plane boundary.

---

## 2. D-Bus & API Endpoint Analysis

### D-Bus Queries and Traversal
`op-introspection` acts strictly as an **introspective scanner client** and does not register or expose its own system-level unique D-Bus service names or object paths. Instead, it queries and traverses external services. The hardcoded D-Bus entities referenced in the codebase are:

#### Standard Interfaces Inspected
*   `org.freedesktop.DBus.ObjectManager` (used in `hierarchical.rs:309` for bulk discovery)
*   `org.freedesktop.DBus.Introspectable` (used in `hierarchical.rs:359` and `scanner.rs:81` for node tree XML resolution)

#### Well-Known Services Analyzed & Mapped
*   `org.freedesktop.systemd1` (categorized as managed via built-in systemd plugin in `mod.rs:443`)
*   `org.freedesktop.login1` (categorized as managed via built-in login1 plugin in `mod.rs:447`)
*   `org.freedesktop.NetworkManager` (mapped to plugin recommendations in `mod.rs:481`)
*   `org.freedesktop.PackageKit` (mapped to plugin recommendations in `mod.rs:482`)
*   `org.freedesktop.UPower` (mapped to plugin recommendations in `mod.rs:483`)
*   `org.freedesktop.UDisks2` (mapped to plugin recommendations in `mod.rs:484`)
*   `org.bluez` (mapped to plugin recommendations in `mod.rs:485`)

#### Hardcoded Target Paths
*   `/` (fallback root path traversed in `hierarchical.rs:468` and `scanner.rs:66`)
*   `/<service_name_with_slashes>` (object path derived dynamically using reverse domain namespace notation in `hierarchical.rs:470` and `mod.rs:456`)

---

### Exposed HTTP/gRPC Endpoints
There are **no direct HTTP/gRPC endpoints** defined or exposed in the provided source files of the `op-introspection` crate. The crate serves as a pure library backend utilized by high-level workspace components (such as `op-web` or `op-dbus`) to populate API payloads.

---

## 3. Schema-as-Code Compliance Review

The codebase fails to adhere to a formal schema-as-code discipline. Rather than deriving data contracts from a single, versioned schema repository (such as Protocol Buffers or OSCAL components), **ad-hoc structs decorated with Serde attributes** are defined directly in Rust source code to specify external serializable contracts.

The following ad-hoc structs represent unversioned system data contracts:

### Ad-hoc Hardware and Vulnerability Contracts
*   `crates/op-introspection/src/cpu_features.rs:19`: `CpuFeatureAnalysis`
*   `crates/op-introspection/src/cpu_features.rs:33`: `CpuModel`
*   `crates/op-introspection/src/cpu_features.rs:43`: `CpuFeature`
*   `crates/op-introspection/src/cpu_features.rs:74`: `BiosLock`
*   `crates/op-introspection/src/cpu_features.rs:84`: `UnlockMethod`
*   `crates/op-introspection/src/cpu_features.rs:104`: `Recommendation`
*   `crates/op-introspection/src/mod.rs:18`: `IntrospectionReport`
*   `crates/op-introspection/src/mod.rs:36`: `SystemConfiguration`
*   `crates/op-introspection/src/mod.rs:58`: `CpuMitigation`
*   `crates/op-introspection/src/mod.rs:65`: `VirtualizationConfig`
*   `crates/op-introspection/src/mod.rs:74`: `HardwareInfo`
*   `crates/op-introspection/src/mod.rs:115`: `ConversionCandidate`

### Ad-hoc D-Bus Schema Mappings
*   `crates/op-introspection/src/hierarchical.rs:21`: `HierarchicalIntrospection`
*   `crates/op-introspection/src/hierarchical.rs:39`: `BusIntrospection`
*   `crates/op-introspection/src/hierarchical.rs:52`: `ServiceIntrospection`
*   `crates/op-introspection/src/hierarchical.rs:72`: `ObjectIntrospection`
*   `crates/op-introspection/src/hierarchical.rs:91`: `InterfaceIntrospection`
*   `crates/op-introspection/src/hierarchical.rs:110`: `MethodIntrospection`
*   `crates/op-introspection/src/hierarchical.rs:123`: `PropertyIntrospection`
*   `crates/op-introspection/src/hierarchical.rs:137`: `SignalIntrospection`
*   `crates/op-introspection/src/hierarchical.rs:148`: `ArgumentIntrospection`
*   `crates/op-introspection/src/hierarchical.rs:160`: `IntrospectionSummary`
*   `crates/op-introspection/src/indexer.rs:17`: `IndexStatistics`
*   `crates/op-introspection/src/indexer.rs:31`: `SearchResult`
*   `crates/op-introspection/src/mod.rs:82`: `DbusServiceInfo`
*   `crates/op-introspection/src/mod.rs:92`: `InterfaceInfo`

---

## 4. Quality & Security Findings

### [Critical] Path Traversal / Arbitrary JSON File Read via Cache Loader
*   **Location**: `crates/op-introspection/src/hierarchical.rs:527-531`
*   **Vulnerability**: 
    ```rust
    pub async fn load_by_timestamp(&self, timestamp: &str) -> Result<HierarchicalIntrospection> {
        let filename = format!("{}.json", timestamp.replace(':', "-"));
        let path = self.cache_dir.join("introspection").join(&filename);

        let json = tokio::fs::read_to_string(&path).await?;
    ```
*   **Exploitation**: The `timestamp` argument is received directly from the RPC/web router interfaces. The string replacement of `:` with `-` is insufficient to prevent relative path traversal. An attacker can pass `../../../../etc/some_config` as the timestamp, resolving the path to `cache_dir/introspection/../../../../etc/some_config.json`.
*   **Impact**: Arbitrary system-wide JSON file disclosure. An attacker can read any configuration, database backup, or secret file ending in `.json` that the application has read permissions for.
*   **Remediation**: Strictly validate the `timestamp` parameter using a regular expression that only allows standard RFC3339 character subsets (e.g., `^[0-9T\-:\+]+$`) before joining it to the base path.

---

### [High] Denial of Service via Runtime Thread Starvation and Deadlocks
*   **Location**: `crates/op-introspection/src/indexer_manager.rs:44-124`
*   **Vulnerability**:
    ```rust
    pub async fn build_index(&self, bus_type: BusType) -> Result<IndexStatistics> {
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let indexer = DbusIndexer::new(&db_path).await?;
                indexer.build_index(bus_type).await
            })
        })
        .await?
    }
    ```
*   **Risk**: All public accessors in `IndexerManager` (`build_index`, `search_methods`, `search_properties`, `search_all`, `get_statistics`, `clear_index`) invoke `spawn_blocking` and then immediately re-enter the async executor thread pool using `rt.block_on(...)`. This is a severe anti-pattern:
    1.  It spawns a blocking thread pool worker, consuming OS threads.
    2.  It re-submits the inner async task back onto the runtime, blocking the spawned thread until the async task is completed.
    3.  Under concurrent loads, this easily leads to **runtime pool starvation** and executor deadlocks, as worker threads waiting in `spawn_blocking` prevent other cooperative tasks from resolving.
*   **Remediation**: Migrate the database backend to a native async driver (such as `tokio-rusqlite`) or execute the queries synchronously inside `spawn_blocking` without resorting to `block_on` recursion.

---

### [High] Denial of Service via SQLite FTS5 Query Syntax Crashes
*   **Location**: `crates/op-introspection/src/indexer.rs:602`, `indexer.rs:634`, and `indexer.rs:661`
*   **Vulnerability**:
    ```rust
    WHERE methods_fts MATCH ?1
    ```
*   **Risk**: The `search_methods`, `search_properties`, and `search_all` functions pass user-supplied queries directly to SQLite's `MATCH` operator without validation or sanitization. If the query string contains unbalanced quotation marks (`"`), unmatched wildcards (`*`), or bare boolean operators (like `AND`, `OR`, `NOT` with missing operands), the SQLite FTS5 engine parser fails and returns a query syntax error.
*   **Impact**: The resulting `rusqlite::Error` propagates up the stack as an unhandled execution failure, crashing the API search endpoint and denying service to users.
*   **Remediation**: Sanitize search inputs before passing them to `MATCH` (e.g., stripping special FTS5 operators or enclosing terms in double quotes).

---

### [Medium] Write Amplification & High CPU Usage During Bulk Indexing
*   **Location**: `crates/op-introspection/src/indexer.rs:91-248`
*   **Vulnerability**:
    SQLite `AFTER INSERT` and `AFTER UPDATE` triggers are created on core tables (`methods`, `properties`, `signals`, `interfaces`) to keep virtual FTS5 search tables in sync. 
*   **Risk**: During the initial index scan (`build_index`), thousands of items are sequentially inserted. Because these triggers are active during the insert loop, SQLite must execute multiple complex `SELECT` joins with nested index lookups for *every single row* inserted. This causes heavy disk write amplification and CPU bottlenecks.
*   **Remediation**: Populate FTS tables in a single batch operation after scanning is complete, or drop/disable the triggers during bulk indexing.

---

### [Low] Non-atomic System command executions and Modprobe checks
*   **Location**: `crates/op-introspection/src/cpu_features.rs:242`
*   **Finding**:
    ```rust
    Command::new("modprobe").arg("msr").output().is_ok()
    ```
*   **Risk**: Spawning external shell commands (`modprobe`) to check if the `msr` driver is present incurs significant operating system process creation overhead and relies on path-resolved binaries. If the control plane runs as root, this increases the security footprint of command execution dependencies.
*   **Remediation**: Query `/proc/modules` directly in memory or check `/sys/module/msr` instead of spawning shell subprocesses.