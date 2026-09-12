# Production Quality and Security Audit Report
**Crate:** `op-state-store`  
**Auditor:** Senior Rust Systems Architect

---

## 1. Public API Surface

### 1.1 Enumeration of Public Items

An analysis of the `op-state-store` crate reveals a massive, highly exposed public API surface. By exposing internal structural details, configuration structs, database record models, and validation primitives, the crate increases the maintenance burden and expands the attack surface.

The table below catalogs all `pub` items (including structs, enums, traits, functions, constants, and module re-exports) within the provided files.

| Module | Item Type | Name / Signature | Description |
| :--- | :--- | :--- | :--- |
| `disaster_recovery` | `struct` | `SystemDependency` | Model for tracking package manager dependencies. |
| `disaster_recovery` | `struct` | `PluginStateExport` | Holds captured state and metadata for a single plugin. |
| `disaster_recovery` | `struct` | `DisasterRecoveryExport` | Complete wrapper for a DR snapshot. |
| `disaster_recovery` | `struct` | `HostInfo` | OS and kernel context details. |
| `disaster_recovery` | `struct` | `RestoreResult` | Audit log of successes and failures during DR import. |
| `disaster_recovery` | `struct` | `InstallResult` | Individual status of package kit execution. |
| `disaster_recovery` | `fn` | `get_plugin_dependencies` | Translates plugin identifiers to standard dependency sets. |
| `disaster_recovery` | `fn` | `get_global_dependencies` | System-wide package requirements. |
| `disaster_recovery` | `fn` | `install_dependencies_via_packagekit` | D-Bus interface integration wrapper. |
| `disaster_recovery` | `fn` | `is_package_installed` | Checks PackageKit for existing packages. |
| `disaster_recovery` | `fn` | `restore_from_export` | Executes the recovery process. |
| `error` | `enum` | `StateStoreError` | Unified error enum for the crate. |
| `error` | `type` | `Result<T>` | Crate-specific result alias. |
| `event_chain` | `enum` | `OperationType` | Enumerates compliant transition types. |
| `event_chain` | `enum` | `Decision` | Audit trail binary decision (`Allow`/`Deny`). |
| `event_chain` | `enum` | `ActionOrigin` | Captures AI autonomy provenance vs human request. |
| `event_chain` | `enum` | `DenyReason` | Specific failures for auditing. |
| `event_chain` | `struct` | `ChainEvent` | Representational record in the ledger. |
| `event_chain` | `struct` | `MerkleNode` | Binary tree node for Merkle roots. |
| `event_chain` | `struct` | `EventBatch` | Merkle-aggregated set of events. |
| `event_chain` | `struct` | `MerkleProof` | Verification envelope for log inclusion. |
| `event_chain` | `struct` | `StateSnapshot` | High-speed reconstruction point. |
| `event_chain` | `struct` | `EventChain` | Ledger registry container. |
| `event_chain` | `struct` | `ChainVerificationResult` | Cryptographic check summary. |
| `event_chain` | `struct` | `TagImmutabilityProof` | Proof of tag safety. |
| `event_chain` | `struct` | `ChainConfig` | Configuration parameters for limits. |
| `execution_job` | `enum` | `ExecutionStatus` | Job execution state machine. |
| `execution_job` | `struct` | `ExecutionResult` | Result encapsulation of an MPC tool execution. |
| `execution_job` | `struct` | `ExecutionJob` | Job container tracked inside the database. |
| `lib` | `struct` | `StoredObject` | Generic wrapper for state objects. |
| `lib` | `struct` | `CanonicalDbExport` | Backup archive format. |
| `metrics` | `static ref` | `REGISTRY` / counters / gauges | Global Prometheus registration primitives. |
| `metrics` | `fn` | `register_metrics` | Entry point for metric telemetry. |
| `metrics` | `struct` | `OperationTimer` | RAII guard for telemetry timings. |
| `metrics` | `fn` | Various record helpers | Primitives like `record_job_transition`, `record_plugin_apply`. |
| `metrics` | `fn` | `gather_metrics` | Rendered output for scraper endpoints. |
| `redis_stream` | `struct` | `RedisStream` | Async broker connection. |
| `redis_stream` | `struct` | `JobEvent` | Stream record for job events. |
| `redis_stream` | `struct` | `PluginEvent` | Stream record for plugin updates. |
| `redis_stream` | `struct` | `StreamInfo` | Operational lengths. |
| `redis_stream` | `fn` | `try_connect` | Soft connection helper. |
| `schema_validator` | `struct` | `ValidationReport` | Detailed results of json checks. |
| `schema_validator` | `struct` | `ValidationError` | Error point in the schema. |
| `schema_validator` | `struct` | `SchemaValidator` | Engine cache of compiled JSON Schema validators. |
| `schema_validator` | `enum` | `ValidatorError` | Errors from compilation. |
| `schema_validator` | `fn` | `canonicalize_json` | Deterministic ordering for hashes. |
| `sqlite_store` | `struct` | `SqliteStore` | Persistent SQLite engine. |
| `sqlite_store` | `struct` | `CheckpointRecord` | Database model for Rollback. |
| `sqlite_store` | `struct` | `AuditEntry` | Database representation of transitions. |
| `sqlite_store` | `struct` | `JobCounts` | Database job metrics. |
| `sqlite_store` | `struct` | `StoreStats` | Crate diagnostics metrics. |
| `state_store` | `struct` | `ToolRecord` | Database record for an MPC tool. |
| `state_store` | `trait` | `StateStore` | Read-write interface for job registry. |
| `plugin_schema` | `const` | `DEFAULT_SCHEMA_DIALECT` | Identifier for the default dialect. |
| `plugin_schema` | `mod` | `dialects` | Known JSON schema specification paths. |
| `plugin_schema` | `enum` | `FieldType` | Field type enumerations. |
| `plugin_schema` | `struct` | `FieldSchema` | Constraints on individual entries. |
| `plugin_schema` | `struct` | `ReadOnlyCondition` | Immutability conditions. |
| `plugin_schema` | `enum` | `Constraint` | Specific validation bounds. |
| `plugin_schema` | `struct` | `PluginSchema` | Core schema engine document. |
| `plugin_schema` | `struct` | `ValidationResult` | Simple boolean report. |
| `plugin_schema` | `struct` | `PluginSchemaBuilder` | Builder interface. |
| `plugin_schema` | `struct` | `StoredSchemaCopies` | Multiple formats cache. |
| `plugin_schema` | `struct` | `SchemaRegistry` | (Alias `SchemaCatalog`) Schema index. |
| `plugin_schema` | `fn` | `builtin_plugin_schema` | Specific hardcoded definition resolver. |
| `plugin_schema` | `fn` | `builtin_plugin_schemas` | Catalog of legacy structures. |
| `plugin_schema` | `enum` | `SchemaLoadError` | File system schema read errors. |
| `schema_shuttle` | `struct` | `IdentitySled` | Zero-copy shared memory layout. |
| `schema_shuttle` | `struct` | `SchemaShuttle` | Orchestrator for key forging. |
| `schema_shuttle` | `fn` | `run_shuttle` | Main active monitoring thread loop. |

**Total Public Items (including all nested fields and methods):** **224**

---

### 1.2 Top 10 Most Impactful Public Items

The following items represent the core architectural interface of the state store crate. Vulnerabilities or design flaws in these items have immediate system-wide impact.

1. **`StateStore`** (`crates/op-state-store/src/state_store.rs:16`)  
   *Architectural Impact:* This trait defines the standard transaction and storage interface. All database adapters (such as `SqliteStore`) implement it, making it the central read/write bottleneck.
2. **`SqliteStore`** (`crates/op-state-store/src/sqlite_store.rs:19`)  
   *Architectural Impact:* The primary persistent engine. It initializes system schemas, performs raw SQL execution, and handles backups, checkpoints, and tool configurations.
3. **`EventChain`** (`crates/op-state-store/src/event_chain.rs:369`)  
   *Architectural Impact:* An append-only ledger designed to provide snowball-style immutability. If compromised, the system's entire security audit log can be falsified.
4. **`PluginSchema`** (`crates/op-state-store/src/plugin_schema.rs:90`)  
   *Architectural Impact:* The authoritative data contract. It handles runtime type constraints, default value generation, and defines translation structures for compliance validations.
5. **`SchemaRegistry`** (`crates/op-state-store/src/plugin_schema.rs:495`)  
   *Architectural Impact:* The runtime catalog of all validated schemas. It parses files, manages categories, and provides alias resolution for system operations.
6. **`DisasterRecoveryExport`** (`crates/op-state-store/src/disaster_recovery.rs:48`)  
   *Architectural Impact:* Represents the system-wide state snapshot used to reconstruct a system during a disaster. Flaws here can lead to remote code execution or state corruption during recovery.
7. **`SchemaValidator`** (`crates/op-state-store/src/schema_validator.rs:97`)  
   *Architectural Impact:* The interface to the `jsonschema` engine. It performs type validation and normalizes JSON structures for cryptographic hashing.
8. **`IdentitySled`** (`crates/op-state-store/src/schema_shuttle.rs:10`)  
   *Architectural Impact:* A zero-copy shared memory block (`#[repr(C)]`) mapped across the system boundary. It binds a node's physical WireGuard key to its schema execution state.
9. **`RedisStream`** (`crates/op-state-store/src/redis_stream.rs:20`)  
   *Architectural Impact:* The real-time messaging client used to coordinate job execution. If this stream is poisoned, attackers can broadcast fraudulent commands to listening nodes.
10. **`ExecutionJob`** (`crates/op-state-store/src/execution_job.rs:21`)  
    *Architectural Impact:* Represents an active task in the execution ledger. It tracks tool names, arguments, execution state, and results across the control plane.

---

### 1.3 Glob Re-exports (`pub use *`)

There are **no glob re-exports** (`pub use *`) in `src/lib.rs` or any of the provided sub-modules. The codebase enforces highly explicit module exports (e.g., `pub use disaster_recovery::{get_global_dependencies, ...}` in `src/lib.rs:21`). This prevents namespace pollution and maintains clear boundaries.

---

### 1.4 Encapsulation Breaks (Public Struct Fields)

Several critical data structures break encapsulation by exposing internal fields directly as `pub`. This allows arbitrary, validation-bypassing modifications of system states from outside the modules.

* **`DisasterRecoveryExport`** (`crates/op-state-store/src/disaster_recovery.rs:48`)
  ```rust
  pub struct DisasterRecoveryExport {
      pub format_version: String,
      pub export_id: String,
      pub created_at: DateTime<Utc>,
      pub host_info: HostInfo,
      pub plugins: HashMap<String, PluginStateExport>,
      pub global_dependencies: Vec<SystemDependency>,
      pub apply_order: Vec<String>,
      pub checksum: String,
  }
  ```
  *Risk:* Exposing `plugins`, `apply_order`, and `checksum` allows third-party modules to mutate files, inject malicious plugin entries, or alter the installation order without re-computing the cryptographic checksum. This bypasses the validation checks in `finalize()`. These fields must be private and exposed only through safe constructors.
* **`ChainEvent`** (`crates/op-state-store/src/event_chain.rs:114`)
  ```rust
  pub struct ChainEvent {
      pub event_id: u64,
      pub prev_hash: String,
      pub event_hash: String,
      ...
  }
  ```
  *Risk:* Exposing `prev_hash` and `event_hash` as public, mutable fields completely undermines the immutability of the event ledger. External code can modify the hash link of an event *after* creation, breaking the chain's integrity.
* **`IdentitySled`** (`crates/op-state-store/src/schema_shuttle.rs:10`)
  ```rust
  pub struct IdentitySled {
      pub wireguard_pubkey: [u8; 32],
      pub mutation_index: u64,
      pub is_valid: bool,
      pub hashed_footprint: [u8; 32],
  }
  ```
  *Risk:* This struct is explicitly marked `#[repr(C)]` for raw memory sharing. Exposing fields like `is_valid` and `hashed_footprint` as public allow arbitrary local threads to invalidate the token or rewrite the memory footprint without authorization, bypassing the tracking loop in `run_shuttle()`.

---

## 2. Dead Code Audit

### 2.1 Unreferenced Items & Dead Code Table

While no instances of the `#[allow(dead_code)]` attribute exist in the provided source files, several functions, methods, and structures are defined but never referenced anywhere in the active codebase.

| Item Name | Item Type | Defined At | Recommendation / Remediation |
| :--- | :--- | :--- | :--- |
| `run_shuttle` | `fn` (async) | `src/schema_shuttle.rs:51` | **Expose/Test**: This is an infinite monitoring loop that is never spawned. If it is meant to run as an active service, spawn it during system initialization. Otherwise, remove it. |
| `try_connect` | `fn` (async) | `src/redis_stream.rs:356` | **Remove**: Unused helper. Standard initialization relies on `RedisStream::new()`, which is directly called by clients. |
| `with_install_command` | `method` | `src/disaster_recovery.rs:194` | **Expose/Test**: The `install_command` fallback is never executed in the provided `disaster_recovery.rs` file. Integrate the fallback script runner or remove the dead property. |
| `delete_old_jobs` | `method` | `src/sqlite_store.rs:408` | **Test**: Job history pruning is never called by the store or its dependencies. Implement a periodic maintenance task. |
| `cleanup_checkpoints` | `method` | `src/sqlite_store.rs:418` | **Test**: Pruning of rollback checkpoints is defined but unused. Create a cleanup worker. |
| `export_all_draft07` | `method` | `src/plugin_schema.rs:597` | **Remove**: Generates legacy Draft-07 schemas. All active validators use the V2026 dialect (`to_json_schema()`). |
| `export_all_contract` | `method` | `src/plugin_schema.rs:605` | **Remove**: Unused helper. The active contract serialization occurs in `export_contract_for`. |
| `load_from_directory` | `method` | `src/plugin_schema.rs:562` | **Expose**: Designed to load dynamic schema configurations from disk on startup, but is never invoked. Integrate it into store configuration. |
| `by_category` | `method` | `src/plugin_schema.rs:648` | **Remove**: Categorized lookup is not used by active modules. |
| `categories` | `method` | `src/plugin_schema.rs:642` | **Remove**: Diagnostic category helper is unreferenced. |
| `with_builtin_schemas_and_spec_path` | `method` | `src/plugin_schema.rs:530` | **Remove**: Duplicate constructor. Standard tests and runtime use `with_builtin_schemas()`. |
| `with_keyword` | `method` | `src/schema_validator.rs:90` | **Remove**: Unused builder-pattern helper for validation reports. |
| `expand_property_dependencies` | `method` | `src/schema_validator.rs:242` | **Expose**: Intended to normalize legacy engines that do not natively support V2026 conditional validation, but is never called. |

---

## 3. Production Security & Quality Audit

### 3.1 Critical & High Vulnerabilities

#### Finding 1: Cryptographic Integrity Bypass via MD5 Collision Attacks
* **File & Line:** `crates/op-state-store/src/disaster_recovery.rs:113`, `crates/op-state-store/src/event_chain.rs:430`, `crates/op-state-store/src/schema_shuttle.rs:37`, `crates/op-state-store/src/schema_shuttle.rs:85`
* **Vulnerability Class:** Weak Cryptographic Primitive (CWE-328 / CWE-916)
* **Severity:** **Critical** (Directly Exploitable)
* **Risk Analysis:**
  The immutable ledger (`EventChain`), disaster recovery engine (`DisasterRecoveryExport`), and shared-memory access controls (`IdentitySled`) rely entirely on MD5 to compute integrity hashes and identity signatures. MD5 is highly vulnerable to collision attacks, where two distinct inputs generate the exact same hash.
  
  An attacker can craft a malicious state change or system recovery configuration (e.g., modifying dependencies to run arbitrary code or backdooring LXC container states) that results in the same MD5 hash as a valid state. 
  
  In `disaster_recovery.rs:113`, the system computes the final snapshot checksum over plugin MD5 hashes. In `schema_shuttle.rs:37` and `85`, the shared-memory identity footprint is verified using MD5:
  ```rust
  let genesis_hash = md5::compute(payload.as_bytes());
  ```
  An attacker who tampers with the database file or intercepts a DR export can easily generate a colliding payload, allowing malicious configurations to pass hash checks.
* **Remediation:**
  Replace MD5 with a secure cryptographic hashing algorithm like SHA-256 throughout the entire crate. Use the `sha2` crate (which is already present in the workspace dependencies).
  
  *Example Fix:*
  ```rust
  use sha2::{Sha256, Digest};
  
  fn compute_hash_sha256(data: &[u8]) -> String {
      let mut hasher = Sha256::new();
      hasher.update(data);
      format!("{:x}", hasher.finalize())
  }
  ```

---

#### Finding 2: Identity Impersonation via Hardcoded Ephemeral WireGuard Key
* **File & Line:** `crates/op-state-store/src/schema_shuttle.rs:60`
* **Vulnerability Class:** Use of Hardcoded Cryptographic Key / Credentials (CWE-798 / CWE-321)
* **Severity:** **Critical** (Directly Exploitable)
* **Risk Analysis:**
  The `run_shuttle` background loop is responsible for updating the system's identity headers and reload actions. However, the WireGuard public key (which is supposed to uniquely identify the node) is hardcoded to a static string:
  ```rust
  let active_wg_key = "EPHEMERAL_WG_PUBKEY";
  ```
  This hardcoded string is passed directly to `SchemaShuttle::forge_sled` (line 69). Because the resulting identity sled is shared system-wide and maps the cryptographic boundary of active nodes, any compromised local user or malicious container can easily forge valid transits by using this hardcoded string. 
  
  Since the key is not dynamically read from the active system configurations (such as standard network interfaces or D-Bus systemd properties), the entire node authentication scheme is compromised by default.
* **Remediation:**
  Ensure the key is dynamically resolved from the actual WireGuard configuration on startup. If the key cannot be read, immediately abort with a descriptive error. Do not fall back to a hardcoded default string.
  
  *Example Fix:*
  ```rust
  // Read key securely from system configuration
  let active_wg_key = std::env::var("OP_WIREGUARD_PUBKEY")
      .or_else(|_| std::fs::read_to_string("/etc/wireguard/public.key").map(|s| s.trim().to_string()))
      .context("Failed to retrieve actual WireGuard public key. Aborting identity forge.")?;
  ```

---

#### Finding 3: Memory Corruption (Use-After-Free) via Unsafe `simd_json::from_str` on Temporary Local Buffers
* **File & Line:** `crates/op-state-store/src/disaster_recovery.rs:125`, `crates/op-state-store/src/redis_stream.rs:293`, `crates/op-state-store/src/sqlite_store.rs:327`, `crates/op-state-store/src/sqlite_store.rs:350`, `crates/op-state-store/src/sqlite_store.rs:438`
* **Vulnerability Class:** Use-After-Free / Memory Safety Violation (CWE-416)
* **Severity:** **High**
* **Risk Analysis:**
  `simd_json::from_str` is an optimized parser that mutates the input buffer in place. It is marked `unsafe` because any deserialized objects (such as strings or nested objects) borrow directly from the input buffer's lifetime to avoid allocation overhead.
  
  In several locations, the code copies data into a local temporary string, parses it using `unsafe { simd_json::from_str(&mut temp) }`, and returns the deserialized object out of the function scope while dropping the temporary string.
  
  For example, in `disaster_recovery.rs:125`:
  ```rust
  pub fn from_json(json: &str) -> Result<Self> {
      let mut json_mut = json.to_string(); // json_mut is allocated on the stack
      Ok(unsafe { simd_json::from_str(&mut json_mut) }?) // Deserialized struct borrows from json_mut
  } // json_mut is dropped here. The returned structure contains references to a freed stack buffer!
  ```
  This creates a classic **Use-After-Free** scenario. Any subsequent read of the deserialized properties (such as during disaster recovery restore) reads unallocated stack memory, leading to heap/stack corruption, access violations, or potential arbitrary code execution.
* **Remediation:**
  Avoid using the unsafe `simd_json::from_str` parser inside functions that return borrowed properties, unless the input buffer's lifetime is explicitly tied to the output lifetime. Alternatively, use standard, safe deserialization methods (such as `simd_json::serde::from_str` or `serde_json::from_str`), which allocate owned strings and prevent lifetime-related memory bugs.
  
  *Example Fix:*
  ```rust
  pub fn from_json(json: &str) -> Result<Self> {
      // Use the safe, non-destructive serde deserializer
      Ok(serde_json::from_str(json)?)
  }
  ```

---

#### Finding 4: Security Level Downgrade via Fallible Sensitivity Defaults
* **File & Line:** `crates/op-state-store/src/plugin_schema.rs:360`
* **Vulnerability Class:** Insufficient Security Granularity (CWE-200)
* **Severity:** **Medium**
* **Risk Analysis:**
  The `to_contract_json_schema_as` method is responsible for determining the privacy sensitivity of a schema's fields. If sensitive data is incorrectly classified, it can be leaked during vector database indexing (such as Qdrant) or dynamic compliance audits.
  
  The logic relies on a naive heuristic that defaults to "internal" if no exact string match is found in the field name:
  ```rust
  let sensitivity = if secret_paths.is_empty() {
      "internal"
  } else {
      "secret"
  };
  ```
  If a plugin contains sensitive user credentials or personally identifiable information (PII) using custom terminology (e.g. `auth_hash`, `session_token`, or `user_id_hash`), it fails to match `is_secret_field_name` and is categorized as `internal`. This allows sensitive data to bypass masking, hashing, and redaction rules, exposing it to lower-tier services.
* **Remediation:**
  Enforce a strict default sensitivity policy of `secret` for all unmapped or unclassified data fields. Developers must explicitly whitelist fields as `public` or `internal` in the schema definition.
  
  *Example Fix:*
  ```rust
  // Enforce secure-by-default categorization
  let sensitivity = if secret_paths.is_empty() && pii_paths.is_empty() {
      "secret" // Fallback securely to secret if we can't guarantee privacy safety
  } else {
      "secret"
  };
  ```

---

### 3.2 Schema-as-Code Compliance Violations

The codebase has an architectural requirement to follow a strict **schema-as-code** discipline. All data contracts must be expressed as versioned, strongly-typed schemas (e.g., Protocol Buffers or OSCAL documents) rather than ad-hoc Rust structs, free-form JSON objects (`simd_json::OwnedValue`), or raw string slices. 

The audit identified several violations where unstructured data structures are used to represent core control plane contracts:

#### Violation 1: Ad-hoc Representation of Host Environment Details
* **File & Line:** `crates/op-state-store/src/disaster_recovery.rs:69`
* **Description:**  
  The system context is represented using an ad-hoc Rust struct (`HostInfo`) that collects environment values (hostname, kernel, os version) via raw file system reads (`/proc/version`, `/etc/os-release`). Exposing this context as a free-form struct rather than a standardized, versioned system inventory schema (such as an OSCAL Asset Characterization schema) violates the schema-as-code discipline and limits interoperability.

#### Violation 2: Unstructured Plugin State Storage
* **File & Line:** `crates/op-state-store/src/disaster_recovery.rs:32`
* **Description:**  
  The `PluginStateExport` struct stores the state of system plugins using a free-form `simd_json::OwnedValue` block:
  ```rust
  pub struct PluginStateExport {
      pub plugin_name: String,
      pub version: String,
      pub state: Value, // Value is simd_json::OwnedValue
      ...
  }
  ```
  This represents a complete break from the schema-as-code discipline. Instead of defining versioned data models (e.g. via Protocol Buffers), the control plane allows arbitrary, unstructured JSON payloads to represent the authoritative system state.

#### Violation 3: Unstructured Compliance Ledger Logs
* **File & Line:** `crates/op-state-store/src/event_chain.rs:114`
* **Description:**  
  The `ChainEvent` struct represents transition records in the ledger. It uses unstructured JSON payloads for input patches and effective states. Additionally, it tracks autonomy provenance (the semantic context of AI agent decisions) using a custom, ad-hoc enum (`ActionOrigin` on line 80). These structures must be defined as strongly-typed, versioned schema definitions to ensure compliance audits can be verified consistently.

#### Violation 4: Untyped Task and Argument Definitions
* **File & Line:** `crates/op-state-store/src/execution_job.rs:24`
* **Description:**  
  The `ExecutionJob` struct tracks active task executions. It defines arguments and execution results as unstructured, untyped JSON objects:
  ```rust
  pub struct ExecutionJob {
      pub id: Uuid,
      pub tool_name: String,
      pub arguments: simd_json::OwnedValue,
      ...
      pub result: Option<ExecutionResult>,
  }
  ```
  Allowing arbitrary JSON objects as execution parameters bypasses type validation. This makes it impossible to statically verify system transactions or enforce consistent state checks across the system boundary.