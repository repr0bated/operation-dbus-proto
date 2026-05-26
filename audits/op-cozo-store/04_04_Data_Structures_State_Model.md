# Production Security & Quality Audit: `op-cozo-store`

---

## 1. Data Structure Statistics

### `crates/op-cozo-store/src/lib.rs`
* **`Arc` count**: 5
  * Line 3: `use std::sync::Arc;`
  * Line 31: `pub struct CozoGraphShuttle { pub(crate) db: Arc<DbInstance>, }`
  * Line 38: `let s = Self { db: Arc::new(db) };`
  * Line 48: `let s = Self { db: Arc::new(db) };`
  * Line 345: `pub fn db(&self) -> Arc<DbInstance> { self.db.clone() }`
* **`Rc` count**: 0
* **`RefCell` count**: 0
* **`RwLock` count**: 0
* **`Mutex` count**: 0
* **`OnceCell` count**: 0
* **`.clone()` count**: 2
  * Line 310: `h.clone()`
  * Line 346: `self.db.clone()`

---

## 2. Globally Mutable State & Large Structs

### Globally Mutable State
* **None**. No `static mut` or `lazy_static!` variables are declared in `crates/op-cozo-store/src/lib.rs`.

### Large Structs (> 5 Public Fields)
* **None**. The structs defined in `crates/op-cozo-store/src/lib.rs` are within safe size limits:
  * `PolicyVerdict` (2 public fields: `allow`, `reason`).
  * `CozoGraphShuttle` (0 public fields; its database handle is `pub(crate)`).

---

## 3. Schema-as-Code & Data Contracts

### Violation of Schema-as-Code Discipline
* **Ad-hoc Database Relations in Strings** (`crates/op-cozo-store/src/lib.rs:58-154`):
  The datalog relations (including `compliance_rule`, `subid_registry`, `graph_node`, `graph_edge`, `audit_event`, `users`, `sessions`, `memory_namespaces`, and `memory_entries`) are defined as raw, unversioned Datalog creation scripts embedded as string literals in the `seed_schema` method. They are not defined using unified, versioned serialization schemas (such as Protocol Buffers or compiled OSCAL taxonomy models), making schema evolution, migration, and contract enforcement highly fragile and error-prone.
* **Untyped JSON and Object Conversions** (`crates/op-cozo-store/src/lib.rs:252`, `crates/op-cozo-store/src/lib.rs:293`):
  Dynamic JSON maps are marshaled through arbitrary string parsing and `serde_json::Value` without static schema validation. This allows unvalidated payloads to be persisted in `props` columns across graph nodes and namespaces.

---

## 4. Security & Quality Audit Findings

### [CRITICAL] Fail-Open Policy Evaluation on Database/Query Failures
* **Reference**: `crates/op-cozo-store/src/lib.rs:188-202`
* **Vulnerability Type**: Security Decision Under Error / Fail-Open Logic
* **Description**: 
  In `evaluate_mutation`, the system queries the compliance graph database to verify whether a `Deny` rule exists for a given `plugin_id` and `operation`. If the database query executes successfully and finds no matching deny rule, it returns an approval verdict (`allow: true`). However, if `cozo_run` encounters **any** error (e.g., query syntax error, transient database lock, Sled backend write exhaustion, out-of-memory exception, or corrupt database state), the query match branch falls back to the `Err(_)` handler:
  ```rust
  Err(_) => PolicyVerdict { allow: true, reason: "compliance graph not seeded".into() },
  ```
  This defaults to `allow: true` (failing open), completely bypassing compliance, NIST, and EU-AI-Act governance controls. An attacker capable of inducing local resource exhaustion or database lock pressure can effectively neutralize the entire policy/compliance verification engine.
* **Remediation**: 
  Modify the function to return a `Result<PolicyVerdict>` or default to a secure **fail-closed** stance (`allow: false`) on database query errors to prevent untrusted execution when compliance state integrity cannot be verified.

---

### [HIGH] Absence of Session Expiration Validation during Retrieval
* **Reference**: `crates/op-cozo-store/src/lib.rs:322-337`
* **Vulnerability Type**: Session Management / Authorization Bypass
* **Description**: 
  The `create_session` function accepts an optional `expires_at` argument (stored as an RFC3339 string inside the database relation). However, `lookup_session` merely retrieves the raw string tuple `(wg, created, expires)` and returns it without validating if the current system time has exceeded the session's expiration timestamp:
  ```rust
  if let Some(row) = r.rows.first() {
      let wg = dv_as_str(&row[0]).unwrap_or("").to_string();
      let created = dv_as_str(&row[1]).unwrap_or("").to_string();
      let expires = dv_as_str(&row[2]).unwrap_or("").to_string();
      Ok(Some((wg, created, expires)))
  }
  ```
  If a consumer of `lookup_session` fails to parse and validate this timestamp, expired sessions will continue to be recognized as active. Session verification boundaries should be validated within the database layer to ensure unauthenticated expired sessions are immediately invalidated.
* **Remediation**: 
  Implement expiration checks directly in the database lookup query, or parse and compare `expires_at` within `lookup_session` before returning `Some`. If the session is expired, it should return `None` and/or programmatically trigger `delete_session`.

---

### [MEDIUM] Arbitrary Query Execution Vector in Public Store API
* **Reference**: `crates/op-cozo-store/src/lib.rs:163-168`
* **Vulnerability Type**: Untrusted Query Injection (RCE / Data Leakage Risks)
* **Description**: 
  The public function `run_query` accepts an unsanitized string `query` and executes it directly against the database instance:
  ```rust
  pub fn run_query(&self, query: &str, params: Option<Value>) -> Result<Value> {
      let p = params.map(json_obj_to_params).unwrap_or_default();
      let rows = cozo_run(&self.db, query, p)
          .map_err(|e| anyhow::anyhow!("CozoDB query failed: {e}"))?;
      Ok(named_rows_to_json(rows))
  }
  ```
  While designed for advanced diagnostics, this interface represents a major injection vector if exposed to non-privileged callers. Since Cozo scripts are Turing-complete Datalog variations capable of initiating database mutations, a caller can leverage this to bypass `evaluate_mutation` compliance rules entirely, overwrite policy tables, or exfiltrate session values and users' public keys.
* **Remediation**: 
  Restrict the visibility of `run_query`, isolate arbitrary script executions to privileged management components, or enforce strict read-only query mutability configurations (e.g., using `ScriptMutability::Immutable`) when invoking non-mutation queries.