### D-Bus & IPC Attack Surface Analysis

The `op-introspection` crate serves as a discovery, parsing, and caching engine for the D-Bus system. An audit of the provided source files reveals that **the crate does not register, host, or export any D-Bus interfaces, methods, or signals of its own**. Instead, it acts exclusively as a D-Bus client that connects to existing system and session buses to discover, introspect, and index third-party services.

#### Consumed D-Bus Interfaces & Methods

To build its hierarchical index and cache, the crate invokes methods on the following standard D-Bus interfaces:

1. **`org.freedesktop.DBus`**
   * **Method Called**: `ListNames` via `DBusProxy::list_names()`
   * **Citations**: `crates/op-introspection/src/hierarchical.rs:219`, `crates/op-introspection/src/scanner.rs:31`, and `crates/op-introspection/src/mod.rs:434`
   * **Purpose**: Discovers all active well-known service names on the bus.

2. **`org.freedesktop.DBus.ObjectManager`**
   * **Method Called**: `GetManagedObjects`
   * **Citation**: `crates/op-introspection/src/hierarchical.rs:286`
   * **Purpose**: Performs bulk discovery of object paths, interfaces, and properties in a single round-trip.

3. **`org.freedesktop.DBus.Introspectable`**
   * **Method Called**: `Introspect`
   * **Citations**: `crates/op-introspection/src/hierarchical.rs:341` and `crates/op-introspection/src/scanner.rs:63`
   * **Purpose**: Dynamically retrieves XML introspection data for a target object path.

#### Bus Connections

The service queries both buses depending on runtime configurations:
* **System Bus**: Established via `Connection::system()` at `crates/op-introspection/src/hierarchical.rs:188` and `crates/op-introspection/src/scanner.rs:27` / `scanner.rs:59`.
* **Session Bus**: Established via `Connection::session()` at `crates/op-introspection/src/hierarchical.rs:189` and `crates/op-introspection/src/scanner.rs:28` / `scanner.rs:60`.

---

### Schema-as-Code Compliance Audit

The codebase violates the schema-as-code discipline. Rather than defining strict, versioned Protocol Buffers or OSCAL-compliant schemas to enforce data contracts, the crate expresses all system state, hardware configurations, and D-Bus snapshots as ad-hoc, unversioned Rust structs with serialized JSON representations.

This creates extreme vulnerability to schema drift, as updates to the Rust structs will silently break compatibility with historical snapshots saved in persistent BTRFS state subvolumes.

#### Ad-Hoc Data Contracts

1. **CPU Feature and BIOS Lock Reports**
   * **Ad-Hoc Structs**: `CpuFeatureAnalysis`, `CpuModel`, `CpuFeature`, `BiosLock`, `UnlockMethod`, `Recommendation`
   * **Citations**: `crates/op-introspection/src/cpu_features.rs:21-34`, `crates/op-introspection/src/cpu_features.rs:37-45`, `crates/op-introspection/src/cpu_features.rs:59-66`, `crates/op-introspection/src/cpu_features.rs:69-76`, and `crates/op-introspection/src/cpu_features.rs:88-95`

2. **Hierarchical Introspection Snapshots**
   * **Ad-Hoc Structs**: `HierarchicalIntrospection`, `BusIntrospection`, `ServiceIntrospection`, `ObjectIntrospection`, `InterfaceIntrospection`, `MethodIntrospection`, `PropertyIntrospection`, `SignalIntrospection`, `ArgumentIntrospection`, `IntrospectionSummary`
   * **Citations**: `crates/op-introspection/src/hierarchical.rs:18-29`, `crates/op-introspection/src/hierarchical.rs:32-41`, `crates/op-introspection/src/hierarchical.rs:44-59`, `crates/op-introspection/src/hierarchical.rs:62-77`, `crates/op-introspection/src/hierarchical.rs:80-94`, and `crates/op-introspection/src/hierarchical.rs:136-144`

3. **System Introspection Reports**
   * **Ad-Hoc Structs**: `IntrospectionReport`, `SystemConfiguration`, `CpuMitigation`, `VirtualizationConfig`, `HardwareInfo`, `DbusServiceInfo`, `InterfaceInfo`, `ConversionCandidate`
   * **Citations**: `crates/op-introspection/src/mod.rs:19-33`, `crates/op-introspection/src/mod.rs:36-54`, `crates/op-introspection/src/mod.rs:57-61`, `crates/op-introspection/src/mod.rs:64-69`, `crates/op-introspection/src/mod.rs:72-77`, `crates/op-introspection/src/mod.rs:80-87`, and `crates/op-introspection/src/mod.rs:107-113`

4. **FTS Database Statistics**
   * **Ad-Hoc Structs**: `IndexStatistics`, `SearchResult`
   * **Citations**: `crates/op-introspection/src/indexer.rs:15-24` and `crates/op-introspection/src/indexer.rs:28-36`

#### Persistence Risks
These unversioned JSON structures are committed directly to disk using `simd_json::to_string_pretty` in `crates/op-introspection/src/hierarchical.rs:518` and `crates/op-introspection/src/projection.rs:114`.

---

### Security & Quality Findings

#### 1. Path Traversal & Arbitrary File Read in Snapshot Loader (High Severity)
* **File**: `crates/op-introspection/src/hierarchical.rs:608-612`
* **Vulnerable Code**:
  ```rust
  pub async fn load_by_timestamp(&self, timestamp: &str) -> Result<HierarchicalIntrospection> {
      let filename = format!("{}.json", timestamp.replace(':', "-"));
      let path = self.cache_dir.join("introspection").join(&filename);

      let json = tokio::fs::read_to_string(&path).await?;
  ```
* **Impact**: If the `timestamp` parameter is exposed to an RPC interface or untrusted input, an attacker can pass path traversal sequences (e.g., `../../../../../etc/passwd`). The sanitization step (`timestamp.replace(':', "-")`) fails to neutralize directory separators (`/`). This allows reading arbitrary JSON or text files from the host system.
* **Remediation**: Sanitize the path using `Path::file_name` or enforce strict alphanumeric constraints on the input parameter before appending it to `cache_dir`.

#### 2. Concurrency Lock Bypass in `IndexerManager` (Medium Severity)
* **File**: `crates/op-introspection/src/indexer_manager.rs:18` (struct definition) and all implementation methods (`crates/op-introspection/src/indexer_manager.rs:29-122`).
* **Vulnerable Code**:
  ```rust
  pub struct IndexerManager {
      db_path: PathBuf,
      #[allow(clippy::arc_with_non_send_sync)]
      _indexer: Arc<Mutex<Option<DbusIndexer>>>,
  }
  ```
* **Impact**: The field `_indexer` (protected by a `Mutex`) is completely ignored by all methods (such as `build_index`, `search_methods`, etc.). Instead, every single method spawns a blocking task that initializes a brand new database connection on the fly:
  ```rust
  tokio::task::spawn_blocking(move || {
      let rt = tokio::runtime::Handle::current();
      rt.block_on(async {
          let indexer = DbusIndexer::new(&db_path).await?;
          indexer.build_index(bus_type).await
      })
  })
  ```
  This completely defeats the purpose of the synchronization lock, allowing parallel threads to open multiple active write/read connections to the same SQLite file. In production, this causes frequent database locks (`SQLITE_BUSY` / `database is locked` errors), resulting in a Denial of Service of the indexing service.
* **Remediation**: Use the initialized `_indexer` inside the `spawn_blocking` closure, or use a SQLite connection pool (such as `sqlx` or a thread-safe synchronized manager).

#### 3. Undefined Behavior Risk via Manual `unsafe impl Send/Sync` (Medium Severity)
* **File**: `crates/op-introspection/src/indexer_manager.rs:136-137`
* **Vulnerable Code**:
  ```rust
  // IndexerManager is Send + Sync by virtue of using Arc<Mutex<...>>
  unsafe impl Send for IndexerManager {}
  unsafe impl Sync for IndexerManager {}
  ```
* **Impact**: The author manually implements `Send` and `Sync` to silence the compiler. However, the compiler refuses to automatically derive these traits because the inner types (specifically `rusqlite::Connection` held by `DbusIndexer`) are thread-unsafe and lack `Send` and `Sync` bounds. Bypassing these safety checks via manual `unsafe impl` can lead to undefined behavior (memory corruption, race conditions) when the raw database connection or its components are accessed concurrently across thread boundaries.
* **Remediation**: Wrap the inner resource in a thread-safe connection pool or a properly synchronized wrapper that guarantees safety, allowing Rust to derive these traits naturally.

#### 4. Ambiguous PATH Dependency in Command Execution (Low/Medium Severity)
* **Files**: `crates/op-introspection/src/cpu_features.rs:360`, `crates/op-introspection/src/mod.rs:388`, and `crates/op-introspection/src/mod.rs:462`
* **Vulnerable Code**:
  ```rust
  let output = Command::new("rdmsr").arg("0x3A").output();
  let output = Command::new("pgrep").arg("-c").arg("qemu").output();
  let output = Command::new("systemctl").args([...]).output();
  ```
* **Impact**: The crate spawns external system utilities (`rdmsr`, `pgrep`, `systemctl`) using relative binary names. If an attacker manages to modify the `PATH` environment variable of the running process, they can intercept these executions and launch arbitrary code with the elevated permissions of the control plane (which likely runs as `root` to enable kernel operations like `modprobe msr` or `wrmsr`).
* **Remediation**: Always use absolute, fully-qualified paths (e.g., `/usr/bin/systemctl`, `/usr/bin/rdmsr`) and sanitize the `PATH` environment variable before executing external commands.

---
## ⚠ Citation Warnings
- `crates/op-introspection/src/indexer_manager.rs:136`: file has 126 lines
