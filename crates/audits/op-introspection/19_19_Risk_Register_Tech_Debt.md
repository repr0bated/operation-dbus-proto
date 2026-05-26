| Severity | Issue | Evidence (file:line) | Recommendation |
| :--- | :--- | :--- | :--- |
| **High** | Path Traversal / Arbitrary File Read in Introspection Loader | `crates/op-introspection/src/hierarchical.rs:638` | Sanitize the `timestamp` parameter by stripping directory traversal sequences (`..`) or validating it using `Path::file_name` to prevent escaping the cache directory. |
| **High** | Path-Relative Command Execution (Binary Hijacking & Privilege Escalation) | `crates/op-introspection/src/cpu_features.rs:392`, `crates/op-introspection/src/cpu_features.rs:499`, `crates/op-introspection/src/cpu_features.rs:556`, `crates/op-introspection/src/mod.rs:458`, `crates/op-introspection/src/mod.rs:596` | Replace all relative binary invocations with fully qualified absolute system paths (e.g., `/usr/sbin/modprobe`, `/usr/bin/rdmsr`, `/usr/bin/systemctl`). |
| **High** | Broken Mutex & Concurrent SQLite Lockups (`SQLITE_BUSY`) in Async Manager | `crates/op-introspection/src/indexer_manager.rs:18`, `crates/op-introspection/src/indexer_manager.rs:31`, `crates/op-introspection/src/indexer_manager.rs:46` | Redesign the manager to safely borrow the mutex-protected `_indexer` connection, and configure SQLite with WAL mode and a busy timeout to resolve database access collisions. |
| **High** | Schema-as-Code and OSCAL Gaps: Ad-Hoc Structs & Lack of Standardized Compliance Serialization | `crates/op-introspection/src/cpu_features.rs:18`, `crates/op-introspection/src/hierarchical.rs:19`, `crates/op-introspection/src/mod.rs:18`, `crates/op-introspection/src/projection.rs:120` | Refactor ad-hoc Rust structs into versioned Protocol Buffers schemas (`prost`). Map the security posture metadata to OSCAL (Open Security Controls Assessment Language) Component Definitions. |
| **Medium** | Unbounded Cache Memory Growth leading to Denial of Service (OOM) | `crates/op-introspection/src/cache.rs:10`, `crates/op-introspection/src/cache.rs:30`, `crates/op-introspection/src/lib.rs:59` | Replace the plain unbounded `HashMap` in `IntrospectionCache` with an LRU cache or set a strict maximum capacity limit combined with a TTL eviction policy. |
| **Low** | Dead/Redundant Placeholder Parser Code | `crates/op-introspection/src/parser.rs:12`, `crates/op-introspection/src/scanner.rs:120` | Consolidate the XML parsing inside `IntrospectionParser::parse` and refactor `ServiceScanner` to consume it, rather than utilizing dummy implementations. |

---

### Detailed Findings & Remediation

#### 1. Path Traversal / Arbitrary File Read in Introspection Loader
* **Severity**: High
* **Description**: 
  The function `load_by_timestamp` takes a `timestamp: &str` argument and formats it into a filename: `let filename = format!("{}.json", timestamp.replace(':', "-"));`. It then joins it with `cache_dir`: `let path = self.cache_dir.join("introspection").join(&filename);`. 
  Because there is no sanitization of directory traversal characters (such as `../`), an attacker or malicious actor supplying input to this method could traverse out of the intended directory and read any arbitrary `.json` file on the system (e.g., `../../../../etc/some_config`).
* **Remediation**:
  Ensure that only the base filename is used. You can extract the file name using `std::path::Path` or validate that the canonicalized path starts with the base cache directory:
  ```rust
  let path = self.cache_dir.join("introspection").join(&filename);
  let canonical_path = tokio::fs::canonicalize(&path).await?;
  if !canonical_path.starts_with(&self.cache_dir) {
      anyhow::bail!("Path traversal attempt detected");
  }
  ```

#### 2. Path-Relative Command Execution (Binary Hijacking & Privilege Escalation)
* **Severity**: High
* **Description**:
  The application utilizes `Command::new("modprobe")`, `Command::new("rdmsr")`, `Command::new("dmesg")`, `Command::new("pgrep")`, and `Command::new("systemctl")` without specifying absolute paths. Since this daemon performs systems-administration operations (like writing MSRs or loading modules), it likely executes under root privileges or with high capabilities (e.g., `CAP_SYS_RAWIO`). A local attacker capable of altering the environment `PATH` variable can plant a malicious binary matching one of these names, achieving arbitrary code execution with elevated control plane privileges.
* **Remediation**:
  Define system binaries with hardcoded, absolute paths:
  ```rust
  const MODPROBE_PATH: &str = "/usr/sbin/modprobe";
  const RDMSR_PATH: &str = "/usr/sbin/rdmsr";
  const SYSTEMCTL_PATH: &str = "/usr/bin/systemctl";
  ```

#### 3. Broken Mutex & Concurrent SQLite Lockups (`SQLITE_BUSY`) in Async Manager
* **Severity**: High
* **Description**:
  The `IndexerManager` declares `_indexer: Arc<Mutex<Option<DbusIndexer>>>` to supposedly serialize database access. However, inside every async method (e.g., `build_index`, `search_methods`), the code completely ignores this locked variable. It clones `db_path` and instantiates a new database connection inside `spawn_blocking` via `DbusIndexer::new(&db_path)`. SQLite does not allow concurrent write transactions on separate in-process connections without throwing a `database is locked` (`SQLITE_BUSY`) error, meaning parallel search or build operations will crash the caller.
* **Remediation**:
  Access the guarded `DbusIndexer` connection inside the mutex instead of creating new instances. If multiple read-write connections are absolutely required, configure SQLite's Write-Ahead Logging (WAL) and set a busy timeout during connection initiation:
  ```rust
  let conn = Connection::open(db_path.as_ref())?;
  conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
  ```

#### 4. Schema-as-Code and OSCAL Gaps: Ad-Hoc Structs & Lack of Standardized Compliance Serialization
* **Severity**: High
* **Description**:
  The codebase represents complex system states, security mitigation details, and D-Bus payloads using ad-hoc, unversioned Rust structs or raw JSON maps (`simd_json::OwnedValue`). This violates the schema-as-code discipline. Furthermore, because the control plane tracks high-value compliance-relevant conditions (such as CPU vulnerabilities, active hardware locks, and loaded modules), exposing this metadata as ad-hoc payloads prevents interoperability with GRC toolchains.
* **Remediation**:
  Define data contracts within formal Protocol Buffer files (`.proto`) and auto-generate the Rust definitions. Align the introspection and vulnerability outputs with OSCAL Component Definitions to ensure the generated security states are compliant and easily consumable by automated GRC platforms.

#### 5. Unbounded Cache Memory Growth leading to Denial of Service (OOM)
* **Severity**: Medium
* **Description**:
  `IntrospectionCache` inserts every discovered D-Bus path/interface into an unbounded `HashMap`. Because D-Bus services can theoretically produce an infinite number of dynamic paths or temporary endpoints, a user who triggers introspection on these arbitrary endpoints will cause the application cache to consume memory indefinitely, culminating in an Out-Of-Memory (OOM) crash.
* **Remediation**:
  Integrate a bounded caching library (such as `lru` or `dashmap` with size limits) to evict outdated entries, or enforce a strict maximum size limit inside `IntrospectionCache::set`.

#### 6. Dead/Redundant Placeholder Parser Code
* **Severity**: Low
* **Description**:
  `IntrospectionParser::parse` in `parser.rs` contains a stub that returns empty structures. The real XML parsing is done inside `scanner.rs` with `parse_introspection_xml`. This redundancy complicates API usage and violates the principle of separation of concerns.
* **Remediation**:
  Move the logic from `parse_introspection_xml` in `scanner.rs` into `parser.rs` under `IntrospectionParser::parse`, keeping modules focused and clean.