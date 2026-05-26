# Production Security and Quality Audit: op-introspection

---

## 1. License & Dependency Audit

### 1.1 License Field Extraction
* **Crate:** `op-introspection`
* **Cargo.toml Path:** `crates/op-introspection/Cargo.toml`
* **Result:** `license.workspace = true`
* **Workspace Cargo.toml Path:** `/Cargo.toml`
* **Workspace License:** `Apache-2.0`
* **Resolved License:** `op-introspection` is licensed under the **Apache-2.0** license.

### 1.2 Copyleft and SSPL License Scan
A scan of `Cargo.lock` was conducted to identify any GPL, AGPL, SSPL, or other strong copyleft dependencies.
* **GPL/AGPL/SSPL:** None found.
* **Weak Copyleft / Dual-Licensed Alerts:**
  * `cozo` (version `0.7.6`) is licensed under **MPL-2.0** (Mozilla Public License 2.0).
  * `option-ext` (version `0.2.0`) is licensed under **MPL-2.0**.
  * `webpki-root-certs` (version `1.0.7`) is licensed under **MPL-2.0**.
  * `priority-queue` (version `1.4.0`) is dual-licensed under **LGPL-3.0 OR MPL-2.0**. Under the dual-licensing scheme, it can be consumed under **MPL-2.0**, which is compatible with your proprietary/Apache-2.0 workspace, provided any direct modifications to `priority-queue` files are disclosed under MPL terms.

### 1.3 Missing License Fields
* All visible internal packages and dependencies resolve to valid licenses. No crates with completely missing license fields were found in the analyzed configuration scope.

---

## 2. Schema-as-Code Violations

The codebase claims to enforce a strict *schema-as-code* discipline using Protocol Buffers and OSCAL. However, several system-level boundary contracts are expressed as ad-hoc, unversioned Rust structs serialized directly to JSON:

### 2.1 Ad-Hoc CPU Feature Contracts
* **File:** `crates/op-introspection/src/cpu_features.rs`
* **Lines:** 21–125
* **Violation:** Structs such as `CpuFeatureAnalysis`, `CpuModel`, `CpuFeature`, `BiosLock`, `UnlockMethod`, and `Recommendation` define critical hardware state representations using ad-hoc `serde::Serialize` and `serde::Deserialize` annotations rather than code-generated protobuf definitions.

### 2.2 Ad-Hoc Hierarchical D-Bus Snapshot Contracts
* **File:** `crates/op-introspection/src/hierarchical.rs`
* **Lines:** 21–177
* **Violation:** D-Bus service blueprints, interfaces, properties, and signals are structured as ad-hoc nested structs (`HierarchicalIntrospection`, `BusIntrospection`, `ServiceIntrospection`, `ObjectIntrospection`, `InterfaceIntrospection`, etc.) and cached directly as raw JSON inside BTRFS subvolumes, bypassing versioned schema declarations.

### 2.3 Ad-Hoc Introspection Report Contracts
* **File:** `crates/op-introspection/src/mod.rs`
* **Lines:** 18–124
* **Violation:** The root `IntrospectionReport` and nested system configuration payloads (`SystemConfiguration`, `CpuMitigation`, `VirtualizationConfig`, `HardwareInfo`, `DbusServiceInfo`) use ad-hoc unversioned Rust structs to represent boundary contracts across the system management plane.

---

## 3. Security Vulnerability Audit

### 3.1 Path Traversal Vulnerability
* **File:** `crates/op-introspection/src/hierarchical.rs`
* **Line:** 515
* **Severity: Critical**
* **Vulnerability Description:**
  The function `load_by_timestamp` takes an unvalidated user-controlled parameter `timestamp: &str` and directly constructs a filesystem path:
  ```rust
  pub async fn load_by_timestamp(&self, timestamp: &str) -> Result<HierarchicalIntrospection> {
      let filename = format!("{}.json", timestamp.replace(':', "-"));
      let path = self.cache_dir.join("introspection").join(&filename);
  ```
  The string sanitization only replaces the colon (`:`) character with a hyphen (`-`). It does not filter or validate path traversal sequences like `..` or `/`.
* **Exploitation Vector:**
  An attacker with access to the JSON-RPC or HTTP API invoking this function can supply a payload such as `../../../../var/lib/private_config` (which resolves to `/var/lib/private_config.json`). If the target file ends in `.json` and exists, the service will read and attempt to deserialize it. This allows unauthorized reading of sensitive structured configuration data anywhere on the system.
* **Remediation:**
  Enforce strict validation on the `timestamp` parameter. Ensure it strictly matches an expected RFC 3339 datetime pattern or UUID format using a regular expression, and verify that the canonicalized path resides entirely within `self.cache_dir.join("introspection")`.

### 3.2 Thread-Safety Bypass via Unsound Manual `Send` & `Sync` Implementations
* **File:** `crates/op-introspection/src/indexer_manager.rs`
* **Lines:** 131–132
* **Severity: High**
* **Vulnerability Description:**
  The `IndexerManager` manually implements `Send` and `Sync`:
  ```rust
  unsafe impl Send for IndexerManager {}
  unsafe impl Sync for IndexerManager {}
  ```
  This overrides the compiler's safety analysis. However, `IndexerManager` delegates work to `DbusIndexer`, which holds a raw SQLite `rusqlite::Connection` (which is explicitly `!Send` and `!Sync` due to thread-safety invariants of SQLite's C API). Bypassing these safety boundaries manually is highly unsound and can lead to memory corruption, undefined behavior (UB), and segfaults when SQLite handles are concurrently accessed across thread boundaries.
* **Remediation:**
  Remove the manual `unsafe impl Send` and `unsafe impl Sync` markers. Let the compiler correctly infer thread-safety based on safe synchronization primitives. Ensure that database connections are managed via a thread-safe connection pool (like `r2d2` or `sqlx`) rather than passing raw `!Send` pointers across asynchronous tasks.

### 3.3 Stack Overflow / Unbounded Recursion DoS
* **File:** `crates/op-introspection/src/hierarchical.rs`
* **Line:** 368
* **Severity: Medium**
* **Vulnerability Description:**
  The recursive introspection crawler explores D-Bus object trees by calling `Box::pin(self.introspect_recursively(...))` on children:
  ```rust
  // Recursive call (boxed to avoid infinite-sized future)
  Box::pin(self.introspect_recursively(conn, service_name, &child_path, objects)).await?;
  ```
  There is no depth limit or cycle detection. 
* **Exploitation Vector:**
  If a local service (or a malicious/misconfigured IPC service) exposes a deep or circular D-Bus object hierarchy, the recursive call will consume arbitrary amounts of memory and stack space, leading to process crash or Denial of Service (DoS) through memory exhaustion.
* **Remediation:**
  Introduce a maximum recursion depth limit (e.g., maximum depth of 16 levels) and track visited paths to prevent infinite traversal loops.

---

## 4. Concurrency, Integrity, and Logic Flaws

### 4.1 Ineffective Mutex Locking Leading to Concurrency Race Conditions
* **File:** `crates/op-introspection/src/indexer_manager.rs`
* **Lines:** 17, 34–110
* **Severity: High**
* **Issue Description:**
  The `IndexerManager` declares a `Mutex` to protect the database connection from concurrent accesses:
  ```rust
  pub struct IndexerManager {
      db_path: PathBuf,
      #[allow(clippy::arc_with_non_send_sync)]
      _indexer: Arc<Mutex<Option<DbusIndexer>>>,
  }
  ```
  However, the lock is never acquired by any of the worker functions (`build_index`, `search_methods`, `search_properties`, `search_all`, `get_statistics`, `clear_index`). Instead, every function spawns a blocking task that initializes a completely independent SQLite connection:
  ```rust
  pub async fn build_index(&self, bus_type: BusType) -> Result<IndexStatistics> {
      let db_path = self.db_path.clone();
      tokio::task::spawn_blocking(move || {
          let rt = tokio::runtime::Handle::current();
          rt.block_on(async {
              let indexer = DbusIndexer::new(&db_path).await?; // Brand new connection bypasses mutex
              indexer.build_index(bus_type).await
          })
      })
      .await?
  }
  ```
  By instantiating a new SQLite connection on every single call without locking, concurrent indexing runs can trigger `SQLITE_BUSY` database lock errors or corrupt SQLite database pages due to uncoordinated write access.
* **Remediation:**
  Acquire the `Mutex` lock on `_indexer` before initiating database tasks inside `spawn_blocking`, or utilize a proper connection pooling mechanism configured for concurrent access.

### 4.2 Stale Full-Text Search (FTS5) Indexes due to Missing Triggers
* **File:** `crates/op-introspection/src/indexer.rs`
* **Lines:** 105–244
* **Severity: High**
* **Issue Description:**
  The database uses SQLite FTS5 external content tables (`methods_fts`, `properties_fts`, `signals_fts`, `interfaces_fts`) to index D-Bus entities. The FTS content is kept in sync via SQL triggers:
  * Triggers exist for `AFTER INSERT` (e.g., `methods_ai`).
  * Triggers exist for `AFTER UPDATE` (e.g., `methods_au`).
  * **No triggers are defined for `AFTER DELETE`.**
  
  If any row is deleted from the `methods`, `properties`, `signals`, or `interfaces` tables (e.g., during cleanups or incremental updates), the corresponding full-text search indexes are *never* pruned.
* **Impact:**
  This results in index drift and data corruption. Search queries on FTS5 tables will return stale results containing row IDs that do not exist in the underlying relational tables, leading to runtime failures or crashes when resolving relational details.
* **Remediation:**
  Add a complete set of `AFTER DELETE` triggers for all FTS5 tables to clean up indexed content when rows are removed:
  ```sql
  CREATE TRIGGER IF NOT EXISTS methods_ad AFTER DELETE ON methods BEGIN
      INSERT INTO methods_fts(methods_fts, rowid) VALUES('delete', OLD.id);
  END;
  ```

### 4.3 Dead Stub Parser Implementation
* **File:** `crates/op-introspection/src/parser.rs`
* **Lines:** 12–20
* **Severity: Low**
* **Issue Description:**
  The `IntrospectionParser` parser stub contains no actual XML parsing logic:
  ```rust
  pub fn parse(&self, _xml: &str, path: &str) -> Result<ObjectInfo> {
      // Parsing is done in scanner module
      Ok(ObjectInfo {
          path: path.to_string(),
          interfaces: Vec::new(),
          children: Vec::new(),
      })
  }
  ```
  Any consumer calling `IntrospectionParser::parse` will silently receive empty interfaces and children, leading to broken service mapping logic.
* **Remediation:**
  Either integrate the D-Bus XML parser logic inside the `parser` module or deprecate and remove the dead `parser.rs` file.