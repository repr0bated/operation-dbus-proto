# Production Security & Quality Audit: op-cozo-store

## 1. Executive Summary

This audit assesses the security posture, concurrency safety, and schema-as-code compliance of the `op-cozo-store` crate. The crate utilizes CozoDB (with the pure-Rust `sled` embedded storage engine) to manage identity-graph nodes, compliance rules, OSCAL taxonomies, audit logs, and sessions. 

Multiple critical architectural flaws were identified, most notably a **fail-open validation bypass** in the compliance engine and **blocking synchronous storage operations** executed within an asynchronous workspace. Additionally, the codebase relies on ad-hoc string formatting, inline raw Datalog schemas, and dynamically parsed JSON rather than versioned, strongly typed Protocol Buffer contracts or OSCAL schemas.

---

## 2. Async & Concurrency Audit

### Quantitative Analysis
* **`async fn`**: 0
* **`tokio::spawn`**: 0
* **`spawn_blocking`**: 0

### Reactor-Blocking Analysis
While there are no asynchronous functions defined directly in `crates/op-cozo-store/src/lib.rs`, the workspace utilizes asynchronous engines (`axum`, `tokio`, `tonic`) as declared in the root `Cargo.toml`. 

The `CozoGraphShuttle` executes synchronous, heavy-weight database transactions directly on the calling thread. The underlying Sled storage engine performs blocking disk I/O operations (flushing write-ahead logs, page compaction) within synchronous functions such as `cozo_run` (`crates/op-cozo-store/src/lib.rs:11-15`) and `DbInstance::new` (`crates/op-cozo-store/src/lib.rs:45-51`). 

**Reactor Starvation Hazard:** If any `CozoGraphShuttle` methods are called directly inside an asynchronous task (e.g., within an HTTP route handler or gRPC service), they will block the Tokio worker thread. Under high write load or heavy graph traversal (`traverse_graph` at `crates/op-cozo-store/src/lib.rs:315-329`), this will starve the async reactor, leading to severe latency spikes and potential connection dropouts.

### Send/Sync and Trait Safety
Because the crate defines no public asynchronous traits, there are no missing `Send` or `Sync` bounds on async trait methods. However, the synchronous nature of this storage library mandates that all interactions from asynchronous calling code must be wrapped in `tokio::task::spawn_blocking`.

---

## 3. Schema-as-Code Compliance Audit

The workspace is intended to enforce a strict schema-as-code discipline using Protocol Buffers and OSCAL. However, `op-cozo-store` frequently violates this model by resorting to ad-hoc structures, unstructured raw string fields, and unvalidated JSON values.

### Violations of Schema-as-Code Discipline

* **Ad-hoc String-Based Database Seeding (`crates/op-cozo-store/src/lib.rs:62-151`):**
  The schemas for Datalog relations (such as `compliance_rule`, `subid_registry`, `audit_event`, and `memory_namespaces`) are defined as unversioned, inline Datalog creation strings. Changes to these schemas must be manually synchronized across the codebase, violating the principle of generating relational layouts from single-source-of-truth Protocol Buffer definitions.
  
* **OSCAL Taxonomy Degradation to Strings (`crates/op-cozo-store/src/lib.rs:72-85`):**
  The `subid_registry` is specifically designed to store canonical OSCAL taxonomies. Instead of mapping these properties to strongly typed, generated OSCAL Rust structs, fields such as `control_refs` and `statement_refs` are declared as unstructured strings:
  ```rust
  control_refs: String default "",
  statement_refs: String default "",
  ```
  This forces calling code to manually serialize and deserialize references, removing compile-time and runtime validation.

* **Untyped Graph Properties (`crates/op-cozo-store/src/lib.rs:255` & `crates/op-cozo-store/src/lib.rs:267`):**
  The identity-graph endpoints accept arbitrary, untyped JSON `serde_json::Value` inputs:
  ```rust
  pub fn store_node(&self, id: &str, label: &str, props: Value) -> Result<()>
  ```
  These values are converted directly into strings inside `store_node` and `store_edge` without being validated against any schema definition or Protocol Buffer contract. This allows callers to write arbitrary payload configurations to the database, breaking system invariants.

* **Ad-hoc Domain Payload (`crates/op-cozo-store/src/lib.rs:17-21`):**
  The `PolicyVerdict` struct is defined as an ad-hoc local struct:
  ```rust
  pub struct PolicyVerdict {
      pub allow: bool,
      pub reason: String,
  }
  ```
  This is a critical boundary payload that governs mutation compliance. It must be a versioned schema definition (e.g., a Protobuf message) to ensure binary compatibility if this compliance check is queried across RPC boundaries.

---

## 4. Security & Quality Vulnerability Findings

### [CRITICAL] Fail-Open Policy Evaluation on Database Query Errors
* **File Citation:** `crates/op-cozo-store/src/lib.rs:171-197` (specifically line 195)
* **Vulnerability Type:** Protection Mechanism Bypass (CWE-269 / CWE-639)
* **Impact:** 
  The `evaluate_mutation` function queries the compliance graph to verify whether an operation should be denied. If the underlying Datalog query execution fails (due to database lock contention, schema mismatch, temporary OOM, or disk exhaustion), the function catches the error in the `Err(_)` wildcard match branch and returns a fail-open response:
  ```rust
  Err(_) => PolicyVerdict { allow: true, reason: "compliance graph not seeded".into() },
  ```
  An attacker can bypass any compliance or security rule in the system by intentionally triggering a transient database error (such as concurrent write exhaustion or memory exhaustion during recursive graph traversal).
* **Exploitability:**
  Directly exploitable. Any failure in the database layer defaults to `allow: true`, completely neutralizing the compliance checking mechanism.
* **Remediation:**
  Modify `evaluate_mutation` to return a `Result<PolicyVerdict>` or change the default fallback on query failure to fail-closed (`allow: false`).
  ```rust
  pub fn evaluate_mutation(&self, plugin_id: &str, operation: &str) -> Result<PolicyVerdict> {
      // ...
      cozo_run(&self.db, query, p)
          .map(|rows| {
              if rows.rows.is_empty() {
                  PolicyVerdict { allow: true, reason: "no deny rule matched".into() }
              } else {
                  let reason = rows.rows[0].first()
                      .and_then(dv_as_str)
                      .unwrap_or("compliance rule violated")
                      .to_string();
                  PolicyVerdict { allow: false, reason }
              }
          })
  }
  ```

---

### [HIGH] Suppressed Seeding Failures Enable Startup with Disabled Security Rules
* **File Citation:** `crates/op-cozo-store/src/lib.rs:152-157`
* **Vulnerability Type:** Improper Error Handling (CWE-391 / CWE-252)
* **Impact:**
  During database initialization in `seed_schema`, errors encountered while running creation scripts are silently logged as warnings rather than being propagated to halt system startup:
  ```rust
  if let Err(e) = cozo_run(&self.db, script, BTreeMap::new()) {
      let msg = e.to_string();
      if !msg.contains("already exists") && !msg.contains("AlreadyExists") {
          eprintln!("COZO_SCHEMA_ERR: {}", msg); warn!(error = %msg, "CozoDB schema init warning");
      }
  }
  ```
  If the `compliance_rule` or `subid_registry` tables fail to initialize (for example, due to database corruption, Sled write failures, or lock contention), the system will print a warning and boot normally. Because these tables do not exist, downstream queries inside `evaluate_mutation` will fail, triggering the `Err(_)` branch and allowing all mutations to run without verification (combining with the fail-open exploit).
* **Exploitability:**
  Highly exploitable. If the database initialization is disrupted or fails to write to disk, the application runs in a completely unprotected state.
* **Remediation:**
  Propagate all unexpected errors encountered during database seeding using the `?` operator to ensure that the application fails to start if database tables cannot be guaranteed to exist.
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

### [HIGH] Forced Write Mutability on Read-Only Queries
* **File Citation:** `crates/op-cozo-store/src/lib.rs:11-15` & `crates/op-cozo-store/src/lib.rs:161-169`
* **Vulnerability Type:** Improper Privilege Assignment (CWE-250 / CWE-732)
* **Impact:**
  The `run_query` function executes queries using the `cozo_run` helper. This helper permanently configures transaction execution with mutable script permissions:
  ```rust
  fn cozo_run(db: &DbInstance, script: &str, params: Params) -> Result<NamedRows> {
      db.run_script(script, params, ScriptMutability::Mutable)
          .map_err(|e| anyhow::anyhow!("{e}"))
  }
  ```
  Even when `run_query` is called for a read-only request (e.g., retrieving query edges or checking node existence), it runs under `ScriptMutability::Mutable`. This allows any raw script string accepted by `run_query` to mutate the database, drop collections, or insert rogue data, exposing the system to arbitrary write operations through endpoints that should be read-only.
* **Exploitability:**
  Exploitable. Any vulnerability in the calling code that exposes raw query interfaces will allow attackers to execute writes, even if the API was designed only for reading data.
* **Remediation:**
  Expose a separate query runner that enforces `ScriptMutability::Immutable` for read-only queries, and restrict `ScriptMutability::Mutable` strictly to transaction-writing methods.
  ```rust
  fn cozo_run_read_only(db: &DbInstance, script: &str, params: Params) -> Result<NamedRows> {
      db.run_script(script, params, ScriptMutability::Immutable)
          .map_err(|e| anyhow::anyhow!("{e}"))
  }
  ```

---

### [MEDIUM] Session Lifetime Expiration Is Unchecked on Lookup
* **File Citation:** `crates/op-cozo-store/src/lib.rs:402-419`
* **Vulnerability Type:** Insufficient Session Expiration (CWE-613)
* **Impact:**
  The `lookup_session` method retrieves session records matching a given `session_id`. However, the function returns the expiration timestamp without validating whether that timestamp lies in the past:
  ```rust
  if let Some(row) = r.rows.first() {
      let wg = dv_as_str(&row[0]).unwrap_or("").to_string();
      let created = dv_as_str(&row[1]).unwrap_or("").to_string();
      let expires = dv_as_str(&row[2]).unwrap_or("").to_string();
      Ok(Some((wg, created, expires)))
  } else {
      Ok(None)
  }
  ```
  If the calling service retrieves the session but fails to manually verify the `expires` timestamp against the current time, expired sessions will be accepted as active. Furthermore, expired sessions are never deleted, resulting in storage accumulation in the persistent Sled store.
* **Exploitability:**
  Directly impacts session management. Exploitable if upstream calling logic assumes the database layer filters out expired sessions.
* **Remediation:**
  Incorporate expiration timestamp checking inside the `lookup_session` query using Datalog logic, or perform a real-time comparison against UTC inside `lookup_session` and return `Ok(None)` if the current time exceeds the expiration date.
  ```rust
  pub fn lookup_session(&self, session_id: &str) -> Result<Option<(String, String, String)>> {
      // ...
      if let Some(row) = r.rows.first() {
          let expires = dv_as_str(&row[2]).unwrap_or("").to_string();
          if !expires.is_empty() {
              if let Ok(exp_time) = chrono::DateTime::parse_from_rfc3339(&expires) {
                  if chrono::Utc::now() > exp_time {
                      self.delete_session(session_id)?; // Clean up expired session
                      return Ok(None);
                  }
              }
          }
          let wg = dv_as_str(&row[0]).unwrap_or("").to_string();
          let created = dv_as_str(&row[1]).unwrap_or("").to_string();
          Ok(Some((wg, created, expires)))
      } else {
          Ok(None)
      }
  }
  ```

---

### [MEDIUM] Destructive Asymmetrical Type Conversion for Nested JSON Objects
* **File Citation:** `crates/op-cozo-store/src/lib.rs:454-469`
* **Vulnerability Type:** Improper Type Conversion (CWE-704)
* **Impact:**
  The `json_to_dv` helper function processes JSON inputs for ingestion into CozoDB. When it encounters a nested `Value::Object`, it falls back to stringifying the entire object into a raw JSON string:
  ```rust
  Value::Object(_) => DataValue::Str(v.to_string().into()),
  ```
  However, when handling a `Value::Array`, it recursively maps the array elements to `DataValue::List`:
  ```rust
  Value::Array(arr) => DataValue::List(arr.into_iter().map(json_to_dv).collect()),
  ```
  This creates an asymmetrical conversion process. An array containing objects will be parsed into a `DataValue::List` containing stringified items, while a direct nested object is stored as a raw JSON string. Consequently, data read back from the database cannot be deterministically validated or queried using uniform Datalog path expressions, leading to structural data corruption or query mismatch errors.
* **Exploitability:**
  High likelihood of causing runtime query panic or parsing failures when handling dynamically generated graph metadata.
* **Remediation:**
  Support native nested structures uniformly if supported by the database, or consistently serialize/deserialize both complex types using standardized schema models. If CozoDB does not support direct nested structures, serialize the parent schema to a validated Protobuf byte stream or structured schema payload before storage.