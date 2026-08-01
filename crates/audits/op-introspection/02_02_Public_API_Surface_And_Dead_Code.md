# Public API Surface & Dead Code

## Public API Surface Enumeration

The `op-introspection` crate exposes a total of **106** public items (modules, re-exports, structs, enums, fields, and associated functions) to external consumers across its files:

### Top 10 Most Impactful Public Items

| # | Item Name | Type | file:line | Impact Analysis |
|---|:---|:---|:---|:---|
| 1 | `DbusProjection` | `struct` | `crates/op-introspection/src/projection.rs:25` | Primary interface coordinating BTRFS state persistence and blockchain trigger events. |
| 2 | `IntrospectionService` | `struct` | `crates/op-introspection/src/lib.rs:23` | High-level interface providing service discovery, cache management, and JSON-RPC bridging. |
| 3 | `IndexerManager` | `struct` | `crates/op-introspection/src/indexer_manager.rs:11` | Handles thread-safe, non-blocking asynchronous access to the rusqlite FTS5 search indexer. |
| 4 | `HierarchicalIntrospector` | `struct` | `crates/op-introspection/src/hierarchical.rs:163` | Executes bulk D-Bus schema discovery, recursion, and BTRFS-backed JSON caching. |
| 5 | `DbusIndexer` | `struct` | `crates/op-introspection/src/indexer.rs:43` | Directly manages rusqlite connections, FTS5 virtual table schemas, and custom update triggers. |
| 6 | `SystemIntrospector` | `struct` | `crates/op-introspection/src/mod.rs:118` | Generates diagnostic profiles covering system kernel options, active CPU mitigations, and virtualization support. |
| 7 | `CpuFeatureAnalyzer` | `struct` | `crates/op-introspection/src/cpu_features.rs:136` | Coordinates hardware-level analysis by reading model properties and verifying raw CPU registers. |
| 8 | `ServiceScanner` | `struct` | `crates/op-introspection/src/scanner.rs:12` | Low-level coordinator reading raw D-Bus Introspectable XML payloads using the `zbus` protocol crate. |
| 9 | `IntrospectionCache` | `struct` | `crates/op-introspection/src/cache.rs:11` | Concurrent cache utilizing `tokio::sync::RwLock` to hold memory-resident XML profiles. |
| 10 | `HierarchicalIntrospection` | `struct` | `crates/op-introspection/src/hierarchical.rs:18` | Top-level representation of a system's D-Bus capability tree serialized to the storage database. |

### Glob Re-exports
The codebase uses glob re-exports (`pub use *`) in `crates/op-introspection/src/mod.rs` to expose raw structures, causing namespace pollution:
*   `crates/op-introspection/src/mod.rs:5`: `pub use cpu_features::*;`
*   `crates/op-introspection/src/mod.rs:8`: `pub use hierarchical::*;`

### Encapsulation Violations (Public Fields on Structs)
Diagnostic structures throughout `cpu_features.rs`, `hierarchical.rs`, and `mod.rs` expose all their fields as `pub` instead of keeping fields private and exposing read-only accessors. This allows external modules to arbitrarily mutate diagnostics and state properties.
*   `CpuFeatureAnalysis` (`crates/op-introspection/src/cpu_features.rs:16`)
*   `CpuModel` (`crates/op-introspection/src/cpu_features.rs:32`)
*   `CpuFeature` (`crates/op-introspection/src/cpu_features.rs:41`)
*   `BiosLock` (`crates/op-introspection/src/cpu_features.rs:78`)
*   `UnlockMethod` (`crates/op-introspection/src/cpu_features.rs:87`)
*   `Recommendation` (`crates/op-introspection/src/cpu_features.rs:105`)
*   `HierarchicalIntrospection` (`crates/op-introspection/src/hierarchical.rs:18`)
*   `BusIntrospection` (`crates/op-introspection/src/hierarchical.rs:33`)
*   `ServiceIntrospection` (`crates/op-introspection/src/hierarchical.rs:45`)
*   `ObjectIntrospection` (`crates/op-introspection/src/hierarchical.rs:65`)
*   `InterfaceIntrospection` (`crates/op-introspection/src/hierarchical.rs:82`)
*   `MethodIntrospection` (`crates/op-introspection/src/hierarchical.rs:98`)
*   `PropertyIntrospection` (`crates/op-introspection/src/hierarchical.rs:110`)
*   `SignalIntrospection` (`crates/op-introspection/src/hierarchical.rs:123`)
*   `ArgumentIntrospection` (`crates/op-introspection/src/hierarchical.rs:134`)
*   `IntrospectionSummary` (`crates/op-introspection/src/hierarchical.rs:145`)
*   `IntrospectionReport` (`crates/op-introspection/src/mod.rs:18`)
*   `SystemConfiguration` (`crates/op-introspection/src/mod.rs:35`)
*   `CpuMitigation` (`crates/op-introspection/src/mod.rs:55`)
*   `VirtualizationConfig` (`crates/op-introspection/src/mod.rs:62`)
*   `HardwareInfo` (`crates/op-introspection/src/mod.rs:70`)
*   `DbusServiceInfo` (`crates/op-introspection/src/mod.rs:78`)
*   `InterfaceInfo` (`crates/op-introspection/src/mod.rs:88`)
*   `ConversionCandidate` (`crates/op-introspection/src/mod.rs:111`)
*   `IntrospectionSummary` (`crates/op-introspection/src/mod.rs:126`)
*   `IndexStatistics` (`crates/op-introspection/src/indexer.rs:14`)
*   `SearchResult` (`crates/op-introspection/src/indexer.rs:27`)

---

## Dead Code Analysis

There are no `#[allow(dead_code)]` attributes present in the provided source files. However, multiple compiler warning suppressions exist via clippy attributes, and several modules, fields, and stub methods contain dead code as detailed below:

### Dead Code Table

| Item | Type | file:line | Recommendation |
|:---|:---|:---|:---|
| `_indexer` | Struct Field | `crates/op-introspection/src/indexer_manager.rs:16` | **Remove / Refactor**: The initialized indexer is never retrieved or used by the query methods; instead, methods dynamically construct a new `DbusIndexer` on every invocation. |
| `_cache` | Struct Field | `crates/op-introspection/src/scanner.rs:13` | **Remove**: Unused private cache hashmap in `ServiceScanner`. |
| `_current_property` | Local Variable | `crates/op-introspection/src/scanner.rs:103` | **Remove**: Unused temporary XML state tracking variable. |
| `IntrospectionParser` | Struct | `crates/op-introspection/src/parser.rs:5` | **Remove**: Empty placeholder structure. XML parser operations are already implemented directly in `scanner.rs`. |
| `IntrospectionParser::parse` | Method | `crates/op-introspection/src/parser.rs:11` | **Remove**: Stub parsing implementation that returns empty results. |
| `Default for IntrospectionParser` | Trait Impl | `crates/op-introspection/src/parser.rs:21` | **Remove**: Unused trait implementation for placeholder struct. |

---

# Security & Quality Findings

### Finding 1 (Critical): Path Traversal Vulnerability in Snapshot Cache Loading
*   **File**: `crates/op-introspection/src/hierarchical.rs`
*   **Lines**: 614-617
*   **Impact**: Arbitrary local file disclosure of JSON configurations and files across the host system.

#### Description
The `load_by_timestamp` method receives an unvalidated string argument (`timestamp`) and constructs a file system path by appending it directly to the snapshot cache directory:

```rust
    pub async fn load_by_timestamp(&self, timestamp: &str) -> Result<HierarchicalIntrospection> {
        let filename = format!("{}.json", timestamp.replace(':', "-"));
        let path = self.cache_dir.join("introspection").join(&filename);
```

#### Vulnerability Mechanics
If an attacker controls the `timestamp` parameter through an external system call, they can pass directory traversal sequences (e.g., `../../../../etc/config`). The `timestamp.replace(':', "-")` call will fail to filter or block these traversal elements. When passed to `.join()`, Rust's path resolver resolves the path relative to the root filesystem, allowing the program to read files outside the intended `@cache` subvolume.

Because the program reads the file contents and parses them as JSON:
```rust
        let json = tokio::fs::read_to_string(&path).await?;
        let data: HierarchicalIntrospection = simd_json::from_str(&json)?;
```
an attacker can successfully read any JSON-formatted system configuration, private workspace credentials, or system settings across the host filesystem. Even if parsing fails, error messages from the parser may leak sensitive structure information.

#### Remediation
Validate that the `timestamp` parameter is a strictly formatted ISO-8601 alphanumeric string without directory separators (`/` or `\`). Additionally, canonicalize the path and verify that the target file resides strictly within the resolved path of `self.cache_dir`:

```rust
let canonical_cache = fs::canonicalize(&self.cache_dir)?;
let resolved_path = fs::canonicalize(&path)?;
if !resolved_path.starts_with(&canonical_cache) {
    anyhow::bail!("Path traversal attempt detected");
}
```

---

### Finding 2 (High): Host Command Execution Hijacking via Unqualified Path Execution
*   **Files**: `crates/op-introspection/src/cpu_features.rs` and `crates/op-introspection/src/mod.rs`
*   **Lines**: `cpu_features.rs:190`, `cpu_features.rs:353`, `mod.rs:270`, `mod.rs:292`
*   **Impact**: Privilege escalation to root if the controller executes inside an environment with an untrusted or user-modifiable `PATH` variable.

#### Description
The codebase executes multiple host diagnostics utilities using unqualified binary names (i.e. without absolute directories):

*   `Command::new("modprobe").arg("msr").output()` (`cpu_features.rs:190`)
*   `Command::new("rdmsr").arg("0x3A").output()` (`cpu_features.rs:353`)
*   `Command::new("pgrep").arg("-c").arg("qemu").output()` (`mod.rs:270`)
*   `Command::new("systemctl").args([...]).output()` (`mod.rs:292`)

#### Vulnerability Mechanics
When launching commands without absolute paths, Rust searches the system directory paths mapped in the process's active `PATH` environment variable. If any folder listed in the local environment's `PATH` is writable by a low-privileged local user, they can perform binary planting (e.g., placing a malicious executable named `rdmsr` or `modprobe` inside that directory). 

Since this system control plane handles low-level hardware virtualization features and manages systemd units, it is expected to execute with highly privileged capabilities (often root/sudo). A hijacked command execution immediately escalates the attacker's privileges to root on the target host.

#### Remediation
Define absolute paths for all binary calls, or explicitly restrict and rebuild the environment variables passed to the sub-process:

```rust
// Safe Absolute Path Execution
Command::new("/usr/sbin/modprobe").arg("msr").output();
Command::new("/usr/bin/rdmsr").arg("0x3A").output();
Command::new("/usr/bin/pgrep").arg("-c").arg("qemu").output();
Command::new("/usr/bin/systemctl").args([...]).output();
```

---

### Finding 3 (High): SQLite Connection Pollution and Concurrency Bottleneck in `IndexerManager`
*   **File**: `crates/op-introspection/src/indexer_manager.rs`
*   **Lines**: 33, 47, 63, 79, 93, 107
*   **Impact**: Database exhaustion, deadlocks, write-amplification performance degradation, and frequent `database is locked` errors.

#### Description
The `IndexerManager` utilizes `spawn_blocking` to offload work to a synchronous executor thread pool. However, instead of retaining a connection pool or sharing an open SQLite database connection from its `_indexer` attribute, the manager instantiates a fresh database connection and indexer instance from scratch on **every single method invocation**:

```rust
    pub async fn search_methods(&self, query: String, limit: usize) -> Result<Vec<SearchResult>> {
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let indexer = DbusIndexer::new(&db_path).await?;
                indexer.search_methods(&query, limit)
            })
        })
        .await?
    }
```

#### Vulnerability Mechanics
Every single call to `DbusIndexer::new` runs a multi-statement transaction batch (`conn.execute_batch`) that checks and attempts to recreate 6 tables, 4 FTS5 virtual tables, 8 triggers, and 5 indexes:

```rust
        // Executed on EVERY SINGLE new instance creation
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS services (...);
            CREATE TABLE IF NOT EXISTS objects (...);
            -- ... triggers and indexes ...
            "#,
        )?;
```

If multiple asynchronous queries are performed concurrently (e.g. from an MCP or RPC worker), they will concurrently execute this database-altering DDL code. Because SQLite places exclusive write-locks during schema validation and table modifications, this results in continuous write conflicts, slow response times, and recurring `database is locked` errors.

#### Remediation
Initialize and migrate the SQLite schema exactly **once** during program startup, and share the connection using a connection pool (like `r2d2` or `sqlx`'s SQLite pool), or leverage the indexer connection within the `IndexerManager::new` initialization.

---

### Finding 4 (Medium): Nested Runtime Invocation Anti-Pattern
*   **File**: `crates/op-introspection/src/indexer_manager.rs`
*   **Lines**: 35-38, 49-52, 65-68, 81-84, 95-98, 109-112
*   **Impact**: Control plane deadlock potential and operating system thread pool resource starvation.

#### Description
All asynchronous methods inside the `IndexerManager` spawn a blocking task thread via `tokio::task::spawn_blocking` and then immediately re-enter the Tokio asynchronous runtime from within that blocking thread by using `rt.block_on`:

```rust
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let indexer = DbusIndexer::new(&db_path).await?;
                indexer.search_methods(&query, limit)
            })
        })
```

#### Vulnerability Mechanics
Using `spawn_blocking` is designed to run long, cpu-heavy synchronous tasks so that async worker threads do not stall. Re-acquiring a handle and calling `rt.block_on` inside that blocking thread forces the async runtime scheduler to block the current thread on nested async polling cycles. 

Under heavy traffic workloads, this context-switching overhead leads to asynchronous runtime resource exhaustion, threadpool starvation, and silent control plane deadlocks.

#### Remediation
Make the database interface synchronous where appropriate, or execute native asynchronous operations on the database connection directly (e.g. using `tokio-rusqlite` or `sqlx` instead of synchronous `rusqlite` connections inside blocking tasks).

---

# Schema-as-Code Compliance Review

The codebase contains several compliance failures relative to a schema-as-code discipline using Protocol Buffers and OSCAL.

### Ad-hoc JSON Value Representation (Untyped Data Contracts)
*   **File**: `crates/op-introspection/src/projection.rs`
*   **Lines**: 58-70, 72-80
*   **Violation**: The methods `list_services` and `introspect` return an untyped generic JSON representation (`simd_json::OwnedValue`). This defeats schema-as-code compile-time type validation, shifting interface contract verification to run-time JSON queries. D-Bus capabilities should be returned as compiled, versioned Protocol Buffer structures.

### Custom Rust Struct Schema Declarations (Non-Protobuf Alignment)
*   **File**: `crates/op-introspection/src/hierarchical.rs`
*   **Lines**: 18-142
*   **Violation**: Complete system schema representation profiles (such as D-Bus method interfaces, arguments, properties, and signals) are designed as ad-hoc, nested custom Rust structs (`struct InterfaceIntrospection`, `struct MethodIntrospection`, etc.). These are API contract declarations and should be modeled using versioned Protobuf `.proto` schema definitions.

### Non-OSCAL Compliance and Diagnostics State Modelling
*   **File**: `crates/op-introspection/src/cpu_features.rs`
*   **Lines**: 16-118
*   **Violation**: System security configurations and hardware assurance levels (such as microcode levels, active CPU vulnerabilities, CPU registers, and BIOS locks) are modeled via custom diagnostic structs. To conform to security-as-code compliance frameworks, these entities must be represented using versioned **OSCAL Component Definition** and **OSCAL Assessment Results** schemas to facilitate automatic machine-readable compliance audits.

### Unstructured System Inventory Configurations
*   **File**: `crates/op-introspection/src/mod.rs`
*   **Lines**: 18-99
*   **Violation**: The `IntrospectionReport` system profile and config structs (including `CpuMitigation`, `VirtualizationConfig`, and `HardwareInfo`) utilize ad-hoc hand-rolled structs. Alignment with system compliance requirements dictates that these metadata baselines conform directly to **OSCAL System Security Plan (SSP)** assets.

### Unvalidated Command Script Strings
*   **File**: `crates/op-introspection/src/cpu_features.rs`
*   **Lines**: 93
*   **Violation**: The `UnlockMethod` struct expresses executable steps as a generic `Vec<String>`. To avoid script injection and verify command intent, the schema must model execution tasks as typed, validated, and structured schemas instead of raw string arrays.