# OP State Store - Security & Quality Audit Report

---

## 1. Direct Dependency & Feature Inventory

The following table lists every direct dependency declared in `crates/op-state-store/Cargo.toml` and its status:

| Dependency Crate | Version / Source | Explicitly Enabled Features | Pulled in by Default | Flagged Issues / CVEs / Gaps |
|---|---|---|---|---|
| `tokio` | Workspace | `["full"]` | No | None |
| `sqlx` | Workspace | `["sqlite", "runtime-tokio", "chrono", "json"]` | No | None |
| `redis` | Workspace | `["tokio-comp"]` | No | None |
| `serde` | Workspace | None | Yes | None |
| `simd-json` | Workspace | None | Yes | None |
| `chrono` | Workspace | None | Yes | None |
| `uuid` | Workspace | None | Yes | None |
| `tracing` | Workspace | None | Yes | None |
| `md5` | `"0.7"` | None | Yes | **Weak Hash Algorithm** (Vulnerable to collision attacks) |
| `base64` | Workspace | None | Yes | None |
| `hex` | Workspace | None | Yes | None |
| `opentelemetry`| Workspace | None | Yes | None |
| `prometheus` | Workspace | None | Yes | None |
| `anyhow` | Workspace | None | Yes | None |
| `thiserror` | Workspace | None | Yes | None |
| `async-trait` | Workspace | None | Yes | None |
| `regex` | Workspace | None | Yes | None |
| `lazy_static` | Workspace | None | Yes | None |
| `zbus` | Workspace | None | Yes | None |
| `serde_json` | Workspace | None | Yes | None |
| `reqwest` | Workspace | `["json"]` | No | None |
| `jsonschema` | `"0.29"` | None | No (`default-features = false`) | None |

### Crate Features Gating
The `op-state-store` crate defines **no custom cargo features** in its `Cargo.toml`. It operates under a single, monolithic configuration compiling all dependencies unconditionally.

---

## 2. Storage Backend Inventory & Architectural Gaps

The control plane implements a hybrid storage model using SQLite and Redis. However, there are significant deviations from the expected architecture defined in the workspace:

### Storage Backend Table

| Backend | Found at File:Line | Role (KV / Graph / Cache / Queue) | Description |
|---|---|---|---|
| **sqlx (SQLite)** | `crates/op-state-store/src/sqlite_store.rs:18` | Persistent KV, Relational & Directory Hierarchy | Durable storage for execution jobs, plugin states, checkpoints, audit trails, AD hierarchies, and WordPress/Drupal schemas. |
| **redis** | `crates/op-state-store/src/redis_stream.rs:20` | Real-Time Queue & State Cache | Stream broker for real-time plugin/job notifications; temporary key-value cache with TTLs. |
| **sled** | *Absent from source* | (None) | There is no use of the `sled` embedded KV database inside this crate, despite the workspace dependency. Note that `IdentitySled` in `schema_shuttle.rs` is an ad-hoc shared memory layout, not the database. |
| **cozo** | *Absent from source* | (None) | Completely absent in this crate, despite being declared in the workspace `Cargo.toml`. |

### Architectural Violations & Gaps
* **Identity & Graph Storage Gap:** Active Directory (AD) represents a deeply nested, hierarchical, and highly connected graph structure. In `crates/op-state-store/src/sqlite_store.rs:224`, the AD schema is initialized inside SQLite tables. This violates sound directory services architectures. The AD hierarchy should instead be stored using **CozoDB** (Datalog relational-graph-vector DB), which is explicitly defined in the workspace dependencies but neglected in this crate.
* **Persistent Compliance Ledger Violation:** The compliance ledger (`event_chain.rs`) is built in-memory and serialized back into SQLite without leveraging an append-only B-tree or KV store like `sled` to enforce transaction isolation and durability guarantees on the compliant logs.

---

## 3. Schema-as-Code Compliance Review

The codebase follows an ad-hoc struct-definition discipline rather than a strict Schema-as-Code paradigm. This introduces significant contract drift risks:

### Gaps in Protocol Buffer Integration
* **Ad-hoc Serialization Contracts:** `DisasterRecoveryExport` (`disaster_recovery.rs:55`), `SystemDependency` (`disaster_recovery.rs:18`), and `ChainEvent` (`event_chain.rs:107`) are defined as ad-hoc Rust structs utilizing standard Serde derives instead of being derived from central, language-agnostic Protocol Buffer (`.proto`) schemas. This bypasses the workspace's schema-as-code capabilities (e.g., `prost` and `tonic`), which are readily available in the workspace but completely unused in this state-tracking module.
* **Lack of Field-Level Validation:** Struct fields rely entirely on manual procedural validation (`plugin_schema.rs:1145`) or the `jsonschema` library, rather than versioned validation rules generated via `protovalidate` or `protoc-gen-validate`.
* **Zero OSCAL Integration:** The compliance ledger (`event_chain.rs`) claims to provide "compliance and reproducibility," but fails to map audit logs or security states to standard OSCAL schemas (e.g., System Security Plans, Assessment Plans). All compliance definitions are represented as ad-hoc string categories and metadata tags.

---

## 4. Detailed Audit Findings

### [CRITICAL] Memory Safety & Out-Of-Bounds Read via Unsafe Unpadded `simd_json::from_str`
* **File Citation:** `crates/op-state-store/src/disaster_recovery.rs:114`, `crates/op-state-store/src/redis_stream.rs:341`, `crates/op-state-store/src/redis_stream.rs:380`, `crates/op-state-store/src/redis_stream.rs:403`, `crates/op-state-store/src/sqlite_store.rs:307`, `crates/op-state-store/src/sqlite_store.rs:373`
* **Impact:** Arbitrary memory corruption, denial of service (segmentation faults), or potential remote code execution.
* **Description:** 
  The codebase repeatedly invokes `unsafe { simd_json::from_str(&mut string) }` to parse JSON documents from SQLite, Redis streams, and Disaster Recovery import files. For example:
  ```rust
  // disaster_recovery.rs:114
  pub fn from_json(json: &str) -> Result<Self> {
      let mut json_mut = json.to_string();
      Ok(unsafe { simd_json::from_str(&mut json_mut) }?)
  }
  ```
  `simd-json` is a highly optimized parser that relies on SIMD instructions. These instructions require the input buffer to be padded with a minimum of `simd_json::SIMDJSON_PADDING` (typically 32 or 64 bytes) to prevent out-of-bounds memory reads when processing blocks of bytes. Converting a standard Rust `String` using `to_string()` does **not** allocate the required padding. 
  
  Furthermore, the input strings parsed via `DisasterRecoveryExport::from_json` and `RedisStream::read_job_events` (which processes events published onto Redis queues) can be influenced or fully controlled by attackers. Parsing an unpadded, potentially malformed JSON string using the `unsafe` API will cause the parser to read past allocated buffer boundaries, resulting in undefined behavior, heap corruption, or immediate process termination.
* **Remediation:** 
  Replace all `unsafe { simd_json::from_str(...) }` calls with `simd_json::from_slice` using a mutable `Vec<u8>` padded using `simd_json::to_padded_bin`, or utilize the safe wrapper APIs that guarantee internal padding allocation.

---

### [CRITICAL] Cryptographic Collision Vulnerability in Tamper-Evident Ledger Linkage (MD5 usage)
* **File Citation:** `crates/op-state-store/src/event_chain.rs:348`, `crates/op-state-store/src/event_chain.rs:353`, `crates/op-state-store/src/event_chain.rs:358`, `crates/op-state-store/src/disaster_recovery.rs:115`, `crates/op-state-store/src/disaster_recovery.rs:172`
* **Impact:** Evasion of audit trail integrity, silent state modification, and unauthorized privilege/capability escalation.
* **Description:**
  The `EventChain` module is designed to provide a "tamper-evident audit trail" using a snowball-style append-only architecture. However, the hashes linking previous events to current events are computed entirely using **MD5**:
  ```rust
  // event_chain.rs:348
  fn compute_hash(value: &Value) -> String {
      let canonical_str = simd_json::to_string(value).unwrap_or_default();
      format!("{:x}", md5::compute(canonical_str.as_bytes()))
  }
  ```
  MD5 is a cryptographically broken hashing algorithm vulnerable to collision attacks (where two distinct inputs yield the identical hash). Because the tamper-evidence guarantee of the ledger relies entirely on the cryptographic strength of the hash chain, an attacker with the ability to modify states can compute a collision. 
  
  This allows the attacker to replace a critical compliance event (e.g., swapping a `Deny` decision for a `Allow` decision, changing a sensitive tunable patch, or altering the execution target) without breaking the hash chain or triggering verification failures in `verify_chain()` (`event_chain.rs:530`).
* **Remediation:**
  Migrate all hashing functions inside `event_chain.rs` and `disaster_recovery.rs` to cryptographically secure alternatives such as SHA-256 (via the workspace-approved `sha2` crate) or SHA-3.

---

### [MAJOR] Non-Functional Dependency Installation & Status Check via PackageKit D-Bus
* **File Citation:** `crates/op-state-store/src/disaster_recovery.rs:290`, `crates/op-state-store/src/disaster_recovery.rs:416`
* **Impact:** Silent restore failures and failure to install required system dependencies during disaster recovery.
* **Description:**
  The dependency installation mechanism relies on D-Bus communication with PackageKit. However, the implementation contains critical API integration errors:
  
  1. **Package Verification Failure:** In `is_package_installed`, the code calls `SearchNames` on the PackageKit transaction and immediately checks `result.is_ok()` to determine if a package is installed:
     ```rust
     let result: std::result::Result<(), zbus::Error> = tx_proxy
         .call("SearchNames", &(2u64, vec![package_name.to_string()]))
         .await;
     Ok(result.is_ok())
     ```
     This is incorrect. PackageKit D-Bus methods like `SearchNames` do not return the search results directly in the method response; they return an empty tuple or a transaction path. The actual search results are returned asynchronously via `Package` signals emitted by the transaction object. Because the method call itself almost always succeeds (it merely registers the search request), `is_package_installed` will **always return `true`**, leading the restore process to falsely skip installing missing required dependencies.
  
  2. **Invalid PackageKit Parameters:** `install_dependencies_via_packagekit` attempts to install dependencies by passing bare package names to `InstallPackages`:
     ```rust
     let install_result: std::result::Result<(), zbus::Error> = install_proxy
         .call("InstallPackages", &(0u64, package_names.clone()))
         .await;
     ```
     The PackageKit `InstallPackages` D-Bus API does not accept plain package names (e.g., `"iptables"`). It strictly requires fully qualified Package IDs in the standard format `name;version;arch;repository`. Passing bare names will cause PackageKit to reject the transaction, making package installation impossible.
* **Remediation:**
  Re-architect the PackageKit integration to listen for D-Bus signals (`Package` and `Finished`) on the active transaction path to gather the search results and resolve bare package names to valid Package IDs before executing the installation call.

---

### [MEDIUM] Fragile SQL Parser & Schema Startup Warning Evasion
* **File Citation:** `crates/op-state-store/src/sqlite_store.rs:154`, `crates/op-state-store/src/sqlite_store.rs:224`, `crates/op-state-store/src/sqlite_store.rs:248`, `crates/op-state-store/src/sqlite_store.rs:271`
* **Impact:** Inconsistent database state, partial migrations, or silent application boot failures.
* **Description:**
  The `SqliteStore::initialize_schema` method parses embedded SQL files (`namespace_schema.sql`, `ad_full_schema.sql`, etc.) line-by-line using a naive parser that splits statements on any line ending with a semicolon:
  ```rust
  if trimmed.ends_with(';') {
      let stmt = current_statement.trim();
      if !stmt.is_empty() {
          if let Err(e) = sqlx::query(stmt).execute(&self.pool).await { ... }
      }
  }
  ```
  This custom parsing algorithm is highly fragile. If any schema definition contains a string literal containing a semicolon (such as complex JSON defaults), or if multi-line triggers or procedural constructs are added to the SQL files, the parser will split the statement mid-string. This leads to invalid SQL syntax and failing statement execution on startup.
* **Remediation:**
  Instead of manual file-splitting loops, utilize SQLx's built-in migration framework (`sqlx::migrate!`) to parse and manage system schema migrations robustly.

---

### [MEDIUM] Command Injection and Privilege Escalation Risk in `schema_shuttle.rs`
* **File Citation:** `crates/op-state-store/src/schema_shuttle.rs:112`
* **Impact:** Potential local privilege escalation or arbitrary system service disruption.
* **Description:**
  The schema shuttle invokes a shell process to modify environment variables and reload the Xray service:
  ```rust
  Command::new("sh")
      .arg("-c")
      .arg(format!(
          "export X_GHOSTBRIDGE_FOOTPRINT='{}' && export X_GHOSTBRIDGE_TRACE_ID='{}' && systemctl reload xray", 
          new_footprint_hex, trace_id
      ))
      .spawn()?;
  ```
  Spawning an interactive shell (`sh`) to load environment variables and invoke systemic commands (`systemctl`) is a dangerous anti-pattern. While `new_footprint_hex` is mathematically constrained to hex characters, wrapping execution in `sh -c` introduces unnecessary shell interpolation risk. Furthermore, if `op-state-store` runs as an unprivileged process, calling `systemctl` directly will fail, while running it as root creates an unnecessary attack surface.
* **Remediation:**
  Remove the interactive shell invocation. Communicate with systemd over the system D-Bus connection (via `zbus` or native systemd bindings) to reload the target service, and write configurations to a dedicated secure file instead of dynamically injecting system environment variables.

---
## ⚠ Citation Warnings
- `crates/op-state-store/src/redis_stream.rs:380`: file has 362 lines
- `crates/op-state-store/src/redis_stream.rs:403`: file has 362 lines
