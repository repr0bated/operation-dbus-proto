# Production Security and Quality Audit: op-introspection

## 1. Executive Summary

This security and quality audit evaluates the `op-introspection` crate, a critical component of the `op-dbus` native control plane. The scope of this audit focuses on security vulnerabilities, architectural consistency, concurrency model bugs, and compliance with schema-as-code principles.

We have identified **one Critical vulnerability** relating to unbounded recursive traversal over D-Bus interfaces that allows local unprivileged users to crash the control plane via Denial of Service (DoS), alongside several high-impact concurrency defects and architecture-to-schema mismatches.

---

## 2. Storage Backend Inventory

As mandated by the system architecture, we scanned all source files to map out active storage backends, their locations, and structural roles:

| Backend | Found at File:Line | Role (KV / Graph / Cache / Queue) | Analysis & Violations |
| :--- | :--- | :--- | :--- |
| **rusqlite** | `crates/op-introspection/src/indexer.rs:16` | Full-Text Search (FTS5) Index of D-Bus objects, methods, properties, and signals. | Appropriate use of SQLite FTS5 for document indexing. No architectural violations found (CozoDB is reserved for the graph/knowledge layer). |
| **rusqlite** | `crates/op-introspection/src/indexer_manager.rs:13` | Management and instantiation of the SQLite database. | Violates connection safety by bypassing the serialized access mutex (see Finding 2). |
| **op-snowball** | `crates/op-introspection/src/projection.rs:9` | System state persistence on BTRFS state subvolumes and event snowball. | Manages restorable state projection for system rollback and disaster recovery. |

---

## 3. Schema-as-Code Compliance & Dependencies

The `op-dbus` workspace utilizes a unified Protocol Buffer schema paradigm to enforce data contract compatibility and enable strict OSCAL compliance. However, a significant gap exists within the `op-introspection` crate:

* **Ad-Hoc Struct Definitions**: The core data contracts of the system introspection subsystem are expressed using ad-hoc, unversioned Rust structs deriving standard Serde serialization:
  * `IntrospectionReport`, `SystemConfiguration`, and `HardwareInfo` at `crates/op-introspection/src/mod.rs:18-125`.
  * `HierarchicalIntrospection` and its associated children (`ServiceIntrospection`, `ObjectIntrospection`) at `crates/op-introspection/src/hierarchical.rs:21-140`.
  * `CpuFeatureAnalysis` and its nested items at `crates/op-introspection/src/cpu_features.rs:21-125`.
* **Missing Schema Versioning**: There are no Protocol Buffer schema definitions, `.proto` files, or versioned OSCAL models defined within this crate's context. Changes to these structs will cause breaking deserialization failures when reading cached introspection data from BTRFS subvolumes (`@cache/introspection`).
* **Dependency Audit**:
  * `zbus` and `zbus_xml` are locked to `4.0` in the workspace.
  * `simd-json` version `0.13` is correctly configured to use `serde` and `serde_impl` features for fast, drop-in JSON parsing.
  * `quick-xml` version `0.36` is used directly in `crates/op-introspection/src/scanner.rs:163` for manual stream parsing.

---

## 4. Detailed Findings

### Finding 1: Unbounded Recursive D-Bus Introspection (Denial of Service) — CRITICAL
* **Location**: `crates/op-introspection/src/hierarchical.rs:327-353`
* **Exploitability**: Directly exploitable by local, unprivileged users.
* **Mechanism**: 
  The hierarchical introspector recursively traverses D-Bus objects by fetching child node names from the parsed XML and calling itself with the nested path:
  ```rust
  // Recurse into children
  for child_name in children {
      let child_path = if path == "/" {
          format!("/{}", child_name)
      } else {
          format!("{}/{}", path, child_name)
      };

      // Recursive call (boxed to avoid infinite-sized future)
      Box::pin(self.introspect_recursively(conn, service_name, &child_path, objects)).await?;
  }
  ```
  Any local user can register a service on the user session bus that generates circular child relationships (e.g., returning child node names that point back to the parent or generating infinite sub-paths). When the system-level control plane calls `introspect_all()`, it traverses this cycle indefinitely, allocating heap memory inside the `objects` `HashMap` until the orchestrator is terminated by the kernel's Out-Of-Memory (OOM) killer.
* **Remediation**:
  Enforce a hard recursion depth limit and implement cycle tracking (a `HashSet` of already visited object paths):
  ```rust
  if depth > MAX_INTROSPECTION_DEPTH || !visited_paths.insert(path.to_string()) {
      return Ok(());
  }
  ```

---

### Finding 2: Bypass of DB Connection Safety and SQLite File Contention in `IndexerManager` — HIGH
* **Location**: `crates/op-introspection/src/indexer_manager.rs:40-120`
* **Exploitability**: High concurrency trigger.
* **Mechanism**:
  `IndexerManager` instantiates an internal serialized cache `_indexer: Arc<Mutex<Option<DbusIndexer>>>` to ensure thread-safe single-connection access to the SQLite database. However, every single asynchronous method in `IndexerManager` completely ignores this field:
  ```rust
  pub async fn build_index(&self, bus_type: BusType) -> Result<IndexStatistics> {
      let db_path = self.db_path.clone();

      tokio::task::spawn_blocking(move || {
          let rt = tokio::runtime::Handle::current();
          rt.block_on(async {
              let indexer = DbusIndexer::new(&db_path).await?; // Bypasses Mutex
              indexer.build_index(bus_type).await
          })
      })
      .await?
  }
  ```
  Each concurrent query or build request instantiates a new `DbusIndexer`, opening a separate SQLite file handle. Because SQLite does not support concurrent write locks on the same database file without WAL configuration (and even then, concurrent writes will block), this completely bypasses connection safety. Concurrent indexing tasks will trigger immediate `SQLITE_BUSY` ("database is locked") thread crashes.
* **Remediation**:
  Retrieve the mutex lock on the inner `DbusIndexer` connection rather than creating brand new connections inside the blocking thread block, or utilize a proper connection pool pooler (such as `r2d2` or `sqlx` sqlite connection pools).

---

### Finding 3: Missing Schema-as-Code Implementation for System Configs — MEDIUM
* **Location**: `crates/op-introspection/src/cpu_features.rs:21-125`, `crates/op-introspection/src/hierarchical.rs:21-140`
* **Exploitability**: Non-Exploitable (Architectural Violation).
* **Mechanism**:
  Data contracts detailing CPU vulnerabilities, microcode versions, and raw system profiles are written as ad-hoc Rust structs. They lack Protocol Buffer schemas and cannot be processed by generic orchestration or external audit tools. This directly violates the defined control plane specification, which mandates OSCAL-compliant, versioned schemas for configuration definitions.
* **Remediation**:
  Define the data structures under Proto3 schemas (e.g., `cpu_features.proto` and `dbus_introspection.proto`), compiling them with `prost-build` to generate structured Rust models with guaranteed backward compatibility.

---

### Finding 4: Incomplete Parallel Interface Discovery Implementation — MEDIUM
* **Location**: `crates/op-introspection/src/projection.rs:188-233`
* **Exploitability**: Non-Exploitable (Quality Issue).
* **Mechanism**:
  The function `discover_service` contains the following documentation:
  `"Discover and persist all interfaces for a managed service ... Recursively discover children in parallel"`.
  However, the implementation only reads `root_info.children` (representing the immediate children at `/` depth) and processes them:
  ```rust
  // Recursively discover children in parallel
  iter(root_info.children)
      .for_each_concurrent(None, |child: String| {
          // ... calls introspect_and_persist directly on child path ...
      })
  ```
  It does not recurse into sub-children (e.g. `/org/freedesktop/NetworkManager/Devices/0`). As a result, deep D-Bus trees are left completely undiscovered and un-persisted inside the BTRFS state subvolume.
* **Remediation**:
  Fix the traversal to execute a true parallel recursive descent:
  ```rust
  async fn discover_recursive_inner(
      self_clone: DbusProjection, 
      bus_type: BusType, 
      service: String, 
      path: String, 
      schemas: Arc<Mutex<Vec<ObjectSchemaRef>>>
  ) -> Result<()> {
      let info = self_clone.introspect_object(bus_type, &service, &path).await?;
      // ... persist this path ...
      // Spawn concurrent sub-tasks for children
      // ...
  }
  ```

---

### Finding 5: Pseudo-Subvolume BTRFS Operations — LOW
* **Location**: `crates/op-introspection/src/hierarchical.rs:149-160`
* **Exploitability**: Non-Exploitable (Quality Issue).
* **Mechanism**:
  The comments inside `HierarchicalIntrospector::new` state:
  `// Create @cache/introspection subvolume if needed`.
  However, the code executes standard directory operations:
  ```rust
  if !cache_dir.exists() {
      tokio::fs::create_dir_all(&cache_dir).await?;
  }
  ```
  No actual BTRFS subvolumes are provisioned. If the storage management layer expects transactional snapshot rollbacks of `@cache/introspection`, these operations will fail silently at the OS level because standard directories do not support BTRFS subvolume transactions.
* **Remediation**:
  If a dedicated subvolume is required, spawn a `Command` calling `btrfs subvolume create` rather than falling back to standard `fs::create_dir_all` directories.

---

### Finding 6: Unescaped FTS5 Full-Text Search Queries — LOW
* **Location**: `crates/op-introspection/src/indexer.rs:521-526`
* **Exploitability**: Low (Internal Query Panic).
* **Mechanism**:
  User queries are passed directly into SQLite FTS5 `MATCH` queries:
  ```rust
  "SELECT ... FROM methods_fts WHERE methods_fts MATCH ?1 LIMIT ?2"
  ```
  FTS5 MATCH query syntax has unique structural operators (such as `"`, `*`, `AND`, `OR`). If a user inputs unescaped special characters, the SQLite virtual table engine will fail to compile the FTS query and return a database error. While handled gracefully by the Rust `?` error propagation, it can be abused to spam internal trace logs and trigger unexpected query failures.
* **Remediation**:
  Sanitize user inputs passed into FTS queries, stripping or escaping unbalanced quotation marks and special FTS query control characters.