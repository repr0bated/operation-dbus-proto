# D-Bus & IPC Attack Surface Security and Quality Audit

## 1. D-Bus & IPC Attack Surface Analysis

The `op-state-store` crate serves as the persistent storage, job ledger, and dependency installation orchestrator for the control plane. It acts as a D-Bus client rather than registering its own D-Bus service interfaces in the provided files.

### D-Bus Interface Registrations & Exposure
* **Exposed D-Bus Interfaces**: None. No D-Bus interfaces, methods, or signals are registered or exposed as a service via `#[dbus_interface]` or similar zbus macros within the provided files of this crate.
* **Outgoing Connections**: Connects to the **System Bus** via `zbus::Connection::system()` in `crates/op-state-store/src/disaster_recovery.rs:332` and `crates/op-state-store/src/disaster_recovery.rs:444`.

### Downstream D-Bus Interactions (as Client)
The disaster recovery module interacts with the system bus to manage system dependencies via **PackageKit**:
* **Target Interface**: `org.freedesktop.PackageKit` (Object Path: `/org/freedesktop/PackageKit`)
  * **Methods Invoked**: `CreateTransaction` (`crates/op-state-store/src/disaster_recovery.rs:344`)
* **Target Interface**: `org.freedesktop.PackageKit.Transaction` (Object Path dynamically obtained from `CreateTransaction`)
  * **Methods Invoked**:
    * `Resolve` (`crates/op-state-store/src/disaster_recovery.rs:361`)
    * `InstallPackages` (`crates/op-state-store/src/disaster_recovery.rs:403`)
    * `SearchNames` (`crates/op-state-store/src/disaster_recovery.rs:461`)

---

## 2. Findings & Vulnerabilities

### Finding 1 [CRITICAL]: Unauthenticated Arbitrary Package Installation via System PackageKit
* **File & Line**: `crates/op-state-store/src/disaster_recovery.rs:472` (in `restore_from_export`)
* **Impact**: Critical. An attacker who can supply or tamper with a Disaster Recovery (DR) JSON payload can force the system to install arbitrary software packages from configured repositories. Because this service connects to the system bus as a privileged client to invoke PackageKit, any unauthenticated restore trigger results in unauthorized system-wide software modifications.
* **Description**:
  The `restore_from_export` function takes a `DisasterRecoveryExport` struct and immediately iterates over its list of `global_dependencies` and plugin `dependencies` to install them via `install_dependencies_via_packagekit`. 
  There is **no authentication check, caller identity verification, or cryptographic signature verification** performed on the exported JSON before processing. While `SystemDependency` structs contain an `install_command` field (unused in the provided source), the `name` field is passed directly to the system D-Bus PackageKit service to install the packages. If an attacker controls the JSON input (e.g., via a restore endpoint or modified state cache), they can execute a supply-chain or local privilege escalation attack by inserting arbitrary package names.
* **Recommendation**:
  1. Implement cryptographic signing (e.g., Ed25519) on the generated DR exports.
  2. Verify the signature of the `DisasterRecoveryExport` file before deserialization and processing.
  3. Restrict access to the restore entry point to authorized system administrators.

---

### Finding 2 [CRITICAL]: Memory Corruption Risk via Unsafe Deserialization of Caller-Supplied JSON
* **File & Line**: `crates/op-state-store/src/disaster_recovery.rs:112-115`
* **Impact**: Critical. Memory safety compromise. Calling unsafe parsing functions on potentially untrusted or malicious JSON inputs can lead to heap corruption, out-of-bounds reads/writes, or segmentation faults.
* **Description**:
  The `DisasterRecoveryExport::from_json` function performs deserialization using `unsafe { simd_json::from_str(&mut json_mut) }`:
  ```rust
  pub fn from_json(json: &str) -> Result<Self> {
      let mut json_mut = json.to_string();
      Ok(unsafe { simd_json::from_str(&mut json_mut) }?)
  }
  ```
  `simd-json`'s in-place parsing mutates the input string and relies on strict structural guarantees. Using the `unsafe` variant of the parser on unvalidated, caller-supplied backup strings bypasses compiler bounds checks and exposes the process to memory corruption vulnerabilities if the payload is malformed or maliciously crafted.
* **Recommendation**:
  Replace `unsafe { simd_json::from_str }` with the safe `simd_json::from_str` or `simd_json::serde::from_str` parser. Avoid `unsafe` parsing blocks unless the input has been cryptographically validated first.

---

### Finding 3 [HIGH]: Use of Cryptographically Broken Hashing Algorithm (MD5) for Immutable Audits and Security Sleds
* **Files & Lines**: 
  * `crates/op-state-store/src/event_chain.rs:172`, `381`, `386` (in `EventChain`)
  * `crates/op-state-store/src/schema_shuttle.rs:44`, `104` (in `SchemaShuttle`)
  * `crates/op-state-store/src/disaster_recovery.rs:90`, `137` (in `PluginStateExport`)
* **Impact**: High. Tampering with compliance trails and security sled footprints.
* **Description**:
  The `EventChain` module implements a blockchain-style, tamper-evident audit trail with Merkle tree batching to enforce compliance. However, it relies entirely on **MD5** as its hashing algorithm (`md5::compute`).
  MD5 is cryptographically broken and highly vulnerable to collision attacks. An attacker can craft two different state-transition payloads that produce the identical MD5 digest. This allows an adversary to execute unauthorized state changes (e.g., modifying firewall rules or adding backdoors) while presenting a forged, matching event hash to the ledger, completely undermining the integrity of the compliance proofs.
  Additionally, the `SchemaShuttle` uses MD5 to forge the `hashed_footprint` injected into network components (`crates/op-state-store/src/schema_shuttle.rs:44`), creating a risk of fingerprint collisions and validation bypasses.
* **Recommendation**:
  Migrate the entire hashing architecture (including EventChain, Merkle roots, Sled fingerprints, and DR exports) from MD5 to a cryptographically secure hashing function such as **SHA-256** or **BLAKE3**.

---

### Finding 4 [MEDIUM]: Shell Spawning and Potential Command Injection in Schema Shuttle
* **File & Line**: `crates/op-state-store/src/schema_shuttle.rs:115-121`
* **Impact**: Medium. Remote command execution risk if formatting constraints fail.
* **Description**:
  The Schema Shuttle spawns a shell process using raw string formatting to pass variables to systemd:
  ```rust
  Command::new("sh")
      .arg("-c")
      .arg(format!(
          "export X_GHOSTBRIDGE_FOOTPRINT='{}' && export X_GHOSTBRIDGE_TRACE_ID='{}' && systemctl reload xray", 
          new_footprint_hex, trace_id
      ))
      .spawn()?;
  ```
  Although `new_footprint_hex` and `trace_id` are derived from hex-encoded digests and are currently safe from direct shell injection, relying on `sh -c` with raw string interpolation is a highly fragile pattern. If any future modification changes the formatting of `trace_id` or accepts unvalidated input, this becomes an immediate command injection vector. Moreover, spawning a shell process is slow and introduces unnecessary system overhead.
* **Recommendation**:
  Avoid spawning `sh` entirely. Set environment variables directly on the spawned process using `Command::envs` and call the executable directly:
  ```rust
  Command::new("systemctl")
      .arg("reload")
      .arg("xray")
      .env("X_GHOSTBRIDGE_FOOTPRINT", new_footprint_hex)
      .env("X_GHOSTBRIDGE_TRACE_ID", trace_id)
      .spawn()?;
  ```

---

### Finding 5 [LOW]: Brittle Raw SQL Parsing and Unparameterized Init Query Execution
* **File & Line**: `crates/op-state-store/src/sqlite_store.rs:159`
* **Impact**: Low. Initialization failures or potential SQL injection if schema files are writable.
* **Description**:
  The `initialize_schema` method manually parses schema files (e.g., `namespace_schema.sql`) line-by-line, splitting on semicolons to execute statements individually:
  ```rust
  if trimmed.ends_with(';') {
      let stmt = current_statement.trim();
      if !stmt.is_empty() {
          if let Err(e) = sqlx::query(stmt).execute(&self.pool).await { ... }
      }
      current_statement.clear();
  }
  ```
  This naive parser can break if statements contain embedded semicolons within strings or comments. Furthermore, running unparameterized dynamic queries built from text files poses a security risk if an attacker gets write access to the schema source directories.
* **Recommendation**:
  Use `sqlx::migrate!` to manage database migrations securely and robustly, rather than executing ad-hoc parsed SQL files on application startup.

---

## 3. Schema-As-Code Compliance Violations

The codebase claims to adhere to a schema-as-code discipline using Protocol Buffers and OSCAL. However, multiple modules violate this pattern by defining data contracts dynamically, programmatically, or using ad-hoc Structs/JSON maps:

### Violation 1: Ad-hoc Serialization of System Records instead of Versioned Schemas
* **Files**: 
  * `crates/op-state-store/src/disaster_recovery.rs:19` (`SystemDependency`, `PluginStateExport`, `DisasterRecoveryExport`)
  * `crates/op-state-store/src/execution_job.rs:21` (`ExecutionJob`, `ExecutionResult`)
  * `crates/op-state-store/src/lib.rs:39` (`StoredObject`, `CanonicalDbExport`)
* **Violation Details**:
  These structures define the critical control plane data contracts (such as database exports, job records, and backup schemas) as ad-hoc Rust structs with generic `serde` attributes. They are serialized directly to dynamic JSON maps without backing by versioned Protocol Buffer definitions or OSCAL-compliant system architecture representations.

### Violation 2: Programmatic Generation of Dynamic JSON Schemas
* **File**: `crates/op-state-store/src/plugin_schema.rs:45-181`
* **Violation Details**:
  Instead of compiling schemas from canonical schema sources (e.g., Protobuf or OSCAL source files), `FieldSchema`, `FieldType`, and `PluginSchema` are constructed using an ad-hoc builder pattern (`PluginSchemaBuilder`) and programmatically converted to JSON Schema Draft 2026 documents (`to_json_schema`). This manual construction of schema contracts within application logic introduces high drift risk between the runtime implementation and compliance specifications.