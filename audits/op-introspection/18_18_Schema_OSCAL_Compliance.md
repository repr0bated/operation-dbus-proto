### Schema-as-Code Compliance

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `CpuFeatureAnalysis` | Struct | `crates/op-introspection/src/cpu_features.rs:18` | No | Core diagnostic layout for CPU hardware microcode and BIOS status is modeled entirely as ad-hoc Rust structs with manual Serde attributes. |
| `HierarchicalIntrospection` | Struct | `crates/op-introspection/src/hierarchical.rs:17` | No | Snapshot contract representing discovered system/session D-Bus topologies is serialized directly to JSON without a versioned Protocol Buffer schema. |
| `IntrospectionReport` | Struct | `crates/op-introspection/src/mod.rs:19` | No | System-wide introspection metadata (vulnerabilities, candidate systemd conversions, hardware mitigations) is structured as loose Rust schemas. |
| `list_services_json` | Function | `crates/op-introspection/src/lib.rs:54` | No | Returns untyped `simd_json::OwnedValue` representations of service payloads, violating schema-as-code interface boundaries. |
| `introspect_json` | Function | `crates/op-introspection/src/lib.rs:77` | No | Exposes unvalidated JSON structures for service schemas over public RPC/IPC gateways without interface contract enforcement. |

---

### OSCAL Compliance Coverage

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **Vulnerability Monitoring & Diagnostics** (NIST SP 800-53 RA-5 / SI-2) | `crates/op-introspection/src/cpu_features.rs:109` | None | CPU vulnerability checking (Spectre, Meltdown) and BIOS-level security configurations are hardcoded and not linked to machine-readable OSCAL component definition validation. |
| **System Backup & Recovery** (NIST SP 800-53 CP-9 / CP-10) | `crates/op-introspection/src/projection.rs:114` | None | Programmatic triggering of BTRFS state persistence and system recovery state changes via the blockchain layer lacks corresponding OSCAL mapping. |
| **Least Privilege & Cache Access Control** (NIST SP 800-53 AC-3 / AC-6) | `crates/op-introspection/src/hierarchical.rs:200` | None | Discovered system services, method structures, and parameters are cached directly to disk with loose permissions and without security boundary documentation. |

---

### Recommendations

#### 1. CRITICAL: Path Traversal Vulnerability in State Key Writing
* **Location:** `crates/op-introspection/src/projection.rs:130-134`
* **Vulnerability:** The function `DbusProjection::introspect_and_persist` computes the BTRFS destination path (`state_key`) by substituting periods with underscores in the `service` string, but **fails to sanitize directory delimiters (slashes `/`)**. If an attacker passes a crafted service string such as `../../etc/cron.d/evil_service`, the path resolving logic collapses to `dbus/../../etc/cron.d/evil_service`. This permits arbitrary directory traversal and file overwrites on the host filesystem when invoked under a privileged context (e.g., system-level introspection daemon).
* **Remediation:** Implement a strict validation check on input variables. Slashes must be rejected or stripped entirely. Use a regex validator matching valid D-Bus service constraints (`^[a-zA-Z0-9._-]+$`).

```rust
// Add to projection.rs or core validation helpers
pub fn validate_dbus_name(name: &str) -> Result<()> {
    let re = regex::Regex::new(r"^[a-zA-Z0-9._-]+$").unwrap();
    if !re.is_match(name) || name.contains("..") {
        anyhow::bail!("Invalid characters or path components in D-Bus service name");
    }
    Ok(())
}
```

#### 2. MAJOR: Runtime Re-entrancy & Thread Pool Starvation Antipattern
* **Location:** `crates/op-introspection/src/indexer_manager.rs:40-46` (and all search methods)
* **Vulnerability:** Calling `rt.block_on(async { ... })` inside a thread context spawned by `tokio::task::spawn_blocking` causes thread pool re-entrancy. The blocking pool is held hostage while scheduling async tasks back onto the same cooperative multi-threaded runtime. Under peak request traffic, this scheduling loop triggers thread pool starvation, high scheduling latency, and event-loop deadlocks.
* **Remediation:** Remove the `async` qualifier from `DbusIndexer::new` and its dependent methods since database creation and rusqlite execution are entirely synchronous. Execute the operations synchronously within `spawn_blocking` without re-entering the Tokio executor.

```rust
// In crates/op-introspection/src/indexer.rs: Change to sync signature
impl DbusIndexer {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let conn = Connection::open(db_path.as_ref())?;
        // ... SQLite initialization ...
        Ok(Self { conn: Arc::new(RwLock::new(conn)), scanner })
    }
}

// In crates/op-introspection/src/indexer_manager.rs: Execute synchronously
pub async fn search_methods(&self, query: String, limit: usize) -> Result<Vec<SearchResult>> {
    let db_path = self.db_path.clone();
    tokio::task::spawn_blocking(move || {
        let indexer = DbusIndexer::new(&db_path)?;
        indexer.search_methods(&query, limit)
    })
    .await?
}
```

#### 3. MAJOR: SQLite FTS5 Search Query Injection (MATCH Operator)
* **Location:** `crates/op-introspection/src/indexer.rs:564-586`
* **Vulnerability:** The raw string variable `query` is passed directly to the FTS5 virtual table query (`MATCH ?1`). FTS5 search patterns contain specific operators (such as `AND`, `NOT`, `OR`, `*`, `:` and parenthesis). Unsanitized input containing mismatched double-quotes or unbalanced brackets causes SQLite to fail with an execution error, returning a database error response. This can be exploited to systematically crash or bypass search endpoints (Denial of Service).
* **Remediation:** Sanitize user-provided search queries to strip out dangerous formatting tokens or construct structured, escaped search terms before execution.

```rust
fn sanitize_fts5_query(query: &str) -> String {
    // Strip special FTS5 operators or wrap alphanumeric strings safely
    query.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
}
```

#### 4. MEDIUM: Lack of File Permissions Constraints on Introspection Cache
* **Location:** `crates/op-introspection/src/hierarchical.rs:200-210`
* **Vulnerability:** Directory paths representing local caches of physical diagnostic layouts and active system-level IPC interfaces are created using `tokio::fs::create_dir_all`. These directories are initialized using standard Unix umask configurations, often granting global read permissions (`0o755`). This permits unprivileged local users to inspect systemd layouts, active system configs, and hardware vulnerabilities.
* **Remediation:** Enforce restrictive permissions (`0o700`) during target cache initialization.

```rust
use std::os::unix::fs::DirBuilderExt;

// Ensure directories are created strictly read-write-execute by the owner only
let mut builder = std::fs::DirBuilder::new();
builder.recursive(true).mode(0o700);
builder.create(&cache_path)?;
```

#### 5. SCHEMA-AS-CODE DISCIPLINE: Struct Serialization Refactoring
* **Vulnerability:** The structs containing system diagnostics and cached schemas are defined in code. When exchanging data across RPC or network borders, these fields drift if modified across service deployments.
* **Remediation:** Define the exact schemas in a centralized protocol file (`crates/op-introspection/proto/introspection/v1/introspection.proto`) using versioned definitions. Use `prost` to generate the matching structures within Rust and decouple the serialization layer from internal representation logic.