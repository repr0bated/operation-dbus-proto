# Production Security & Quality Audit: op-cozo-store

## Dependencies & Feature Inventory

### Direct Dependencies (`op-cozo-store`)
* **`anyhow`** (version: `1` via workspace)
  * *Features*: No explicit features enabled.
  * *Role*: Formats database errors into unified `anyhow::Result` bounds.
* **`chrono`** (version: `0.4` via workspace)
  * *Features*: `serde` enabled via workspace.
  * *Role*: Generates RFC3339 timestamps for audits, session metadata, and record creations.
* **`cozo`** (version: `0.7.6` via workspace)
  * *Features*: `default-features = false`, `rayon`, `storage-sled` explicitly enabled via workspace.
  * *Role*: Core Datalog engine with recursion capabilities; acts as the compliance and storage backend.
* **`serde_json`** (version: `1` via workspace)
  * *Features*: No explicit features enabled.
  * *Role*: Serializes and deserializes graph values, properties, and queries.
* **`tracing`** (version: `0.1` via workspace)
  * *Features*: No explicit features enabled.
  * *Role*: Emits schema validation and query warnings.

### Workspace Dependency Issues & Quality Risks
* **Unpinned Dependencies**: `tokio` is declared as version `"1"` in the workspace `Cargo.toml`. This resolves to version `1.49.0` in the Cargo lock file, but unpinned minor versions expose the workspace to build-time drifting.
* **Library Fragmentation**: The workspace maintains duplicate active major versions of `zbus` (e.g., `zbus 3.15.2`, `zbus 4.4.0`, and `zbus 5.13.2` are concurrently pulled into different crates in the Cargo lock file). This poses a severe risk of compile-time bloat and interface linkage mismatches on the DBus broker.

### Crate [features] Section
* No features defined in `crates/op-cozo-store/Cargo.toml`.

---

## Storage Backend Inventory

| Backend | Found at file:line | Role (KV/Graph/Cache/Queue) |
| :--- | :--- | :--- |
| **CozoDB (`"mem"`)** | `crates/op-cozo-store/src/lib.rs:40` | In-memory Relational-Graph DB (handles fast datalog rule matches) |
| **CozoDB (`"sled"`)** | `crates/op-cozo-store/src/lib.rs:49` | Persistent Relational-Graph DB using the pure-Rust embedded Sled engine |

### Architectural Compliance
`op-cozo-store` implements CozoDB backed by Sled or in-memory state. Comments at `crates/op-cozo-store/src/lib.rs:114` and `crates/op-cozo-store/src/lib.rs:125` confirm that this Cozo implementation explicitly replaces the legacy SQLite-based schema for memory namespaces and memory entries. There is no active usage of SQLx or SQLite in this crate, ensuring compliance with the mandated Cozo graph model.

---

## Detailed Audit Findings

### 1. Fail-Open Security Policy Bypass on Database Query Errors
* **File & Line**: `crates/op-cozo-store/src/lib.rs:175-177`
* **Severity**: Critical (Directly Exploitable)
* **Description**: The `evaluate_mutation` function runs a Datalog script checking incoming plugin operations against `compliance_rule` tables. If the database engine encounters any execution error (such as a database lock timeout, filesystem exhaustion, read-only filesystem transition, or unseeded table state), the match block handles the `Err(_)` branch by returning a permissive verdict:
  ```rust
  Err(_) => PolicyVerdict { allow: true, reason: "compliance graph not seeded".into() }
  ```
* **Impact**: An attacker who can trigger a transient query error or storage depletion can bypass all security checks (including NIST/EU-AI-Act compliance deny rules) and execute arbitrary operations. In security-sensitive code, policy evaluations must **fail-closed** (`allow: false`) when the state of the compliance rule-base cannot be evaluated.
* **Remediation**: Change the query failure logic to deny permission if query evaluation fails:
  ```rust
  Err(e) => PolicyVerdict { allow: false, reason: format!("Compliance engine failure: {e}") }
  ```

---

### 2. Silent Seeding Failures of Core Database Schema
* **File & Line**: `crates/op-cozo-store/src/lib.rs:139-147`
* **Severity**: Medium
* **Description**: During `seed_schema`, the initial creation Datalog scripts are executed for each table. If creating a schema relation fails with any error other than "AlreadyExists", the error is written to stderr and logged as a warning, but the iteration continues and returns `Ok(())`:
  ```rust
  for script in &relations {
      if let Err(e) = cozo_run(&self.db, script, BTreeMap::new()) {
          let msg = e.to_string();
          if !msg.contains("already exists") && !msg.contains("AlreadyExists") {
              eprintln!("COZO_SCHEMA_ERR: {}", msg); warn!(error = %msg, "CozoDB schema init warning");
          }
      }
  }
  ```
* **Impact**: Critical relations (such as the `compliance_rule` or `users` tables) can fail to initialize due to disk pressure, lock conflicts, or schema syntax changes, and the store initialization will silently succeed regardless. This leads to runtime panic situations or security evaluation fail-opens downstream when executing queries against non-existent tables.
* **Remediation**: Ensure that unexpected setup errors bubble up up to the caller to prevent starting the application with a corrupted or missing database schema:
  ```rust
  for script in &relations {
      if let Err(e) = cozo_run(&self.db, script, BTreeMap::new()) {
          let msg = e.to_string();
          if !msg.contains("already exists") && !msg.contains("AlreadyExists") {
              return Err(anyhow::anyhow!("CozoDB schema initialization failed: {msg}"));
          }
      }
  }
  ```

---

### 3. Session Expiration is Not Enforced During Session Lookup
* **File & Line**: `crates/op-cozo-store/src/lib.rs:326-343`
* **Severity**: Medium
* **Description**: The `lookup_session` method retrieves the `expires_at` value from the database as an unchecked string but does not validate whether that expiry time has already passed relative to `chrono::Utc::now()`. It simply returns the timestamp in a tuple of raw strings:
  ```rust
  let wg = dv_as_str(&row[0]).unwrap_or("").to_string();
  let created = dv_as_str(&row[1]).unwrap_or("").to_string();
  let expires = dv_as_str(&row[2]).unwrap_or("").to_string();
  Ok(Some((wg, created, expires)))
  ```
* **Impact**: If upstream callers (e.g., in gateways or API bridges) retrieve the session but omit manual timezone parsing or date evaluation, expired sessions will remain active indefinitely. This is a highly error-prone mechanism that easily leads to authentication bypasses.
* **Remediation**: Perform the validation check inside `lookup_session` itself and discard/delete the session if it has expired:
  ```rust
  if !expires.is_empty() {
      if let Ok(exp_time) = chrono::DateTime::parse_from_rfc3339(&expires) {
          if chrono::Utc::now() > exp_time {
              let _ = self.delete_session(session_id);
              return Ok(None);
          }
      }
  }
  ```

---

### 4. Schema-as-Code Gap: Ad-Hoc Data Contracts and String Schemas
* **File & Line**: `crates/op-cozo-store/src/lib.rs:19-22`, `crates/op-cozo-store/src/lib.rs:58-137`, `crates/op-cozo-store/src/lib.rs:152-156`, `crates/op-cozo-store/src/lib.rs:180-184`
* **Severity**: Low / Quality
* **Description**: The codebase bypasses the workspace's schema-as-code discipline (Protocol Buffers, tonic, and prost code-generation patterns defined in `Cargo.toml`). 
  * The `PolicyVerdict` struct is defined as an ad-hoc local struct without a schema.
  * Relations inside `seed_schema` are configured as inline unversioned string literals.
  * Methods such as `register_subid` unpack complex OSCAL-compliant subid records into ad-hoc parameter lists (`category: &str`, `component_type: &str`, etc.) rather than a versioned schema model.
* **Impact**: Breaking change vulnerabilities during updates to compliance frameworks or OSCAL requirements. Structural mismatches between Rust models and Datalog relations will only be discovered at runtime.
* **Remediation**: Define compliance schemas, subids, and verdicts inside versioned Protobuf `.proto` contracts and generate the corresponding Rust structures using `prost-build`.

---

### 5. Lossy JSON Object Deserialization in helper structures
* **File & Line**: `crates/op-cozo-store/src/lib.rs:374-386`, `crates/op-cozo-store/src/lib.rs:395-408`
* **Severity**: Low / Quality
* **Description**: The serialization utility `json_to_dv` converts any JSON object structure to a string representation via `v.to_string()` instead of transforming it recursively into native Cozo structured maps or returning an error:
  ```rust
  Value::Object(_) => DataValue::Str(v.to_string().into()),
  ```
  Correspondingly, the deserialization utility `dv_to_json` returns raw `Value::String` strings rather than parsing objects back to their structured equivalents.
* **Impact**: Structured JSON objects (such as `props` inside nodes and edges) are flattened into raw strings. This strips away query and indexing capabilities of Datalog on nested fields and forces client applications to perform secondary JSON parsing steps on every returned database property.
* **Remediation**: Adjust mapping functions to recursively parse nested elements or enforce schema validation checks on input parameters.