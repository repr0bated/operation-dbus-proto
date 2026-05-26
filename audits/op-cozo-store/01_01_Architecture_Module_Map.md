# Architecture & Module Map

### Overview
The `op-cozo-store` crate serves as the central graph-database shuttle and policy evaluation layer for the system. It leverages **CozoDB** (using the pure-Rust embedded `sled` storage engine) to maintain relations for compliance rules, identity graphs, audit logs, user sessions, and named memory namespaces.

### Module Tree
```
op-cozo-store (lib)
 └── (Flat library structure, no submodules declared via `mod`)
```

### Entry Points
* **`crates/op-cozo-store/src/lib.rs`**: Core library entry point. Declares `CozoGraphShuttle` and its associated query, mutation, audit logging, and compliance evaluation methods.

### Notes
* The codebase integrates with the broader workspace via the central virtual manifest (`Cargo.toml`).
* `CozoGraphShuttle` manages a hybrid relational-graph data schema seeded directly at startup using embedded Datalog scripts.

---

# Production Security & Quality Audit

## 1. Critical Vulnerabilities

### Fail-Open Policy Bypass on Database Error
* **File**: `crates/op-cozo-store/src/lib.rs:178`
* **Vulnerability Type**: Access Control Bypass / Fail-Open Logic Error
* **Impact**: Critical
* **Description**:
  The compliance engine evaluates critical operations against security policies (such as NIST or EU-AI-Act restrictors) via the `evaluate_mutation` method. However, if CozoDB encounters any operational error (e.g., query timeouts, internal index corruption, storage lockups, or database exhaustion), the `match` statement defaults to a fail-open state:
  ```rust
  Err(_) => PolicyVerdict { allow: true, reason: "compliance graph not seeded".into() },
  ```
  An attacker who can artificially induce database load, lock the database, or trigger a temporary query error can bypass all active security policies. Security decisions must always fail closed.
* **Remediation**:
  Modify the `evaluate_mutation` fallback match arm to return a strict reject decision (`allow: false`) with a clear error reason indicating that database or policy state could not be read safely.

---

## 2. High Vulnerabilities

### Arbitrary Datalog Injection & Execution Vector
* **File**: `crates/op-cozo-store/src/lib.rs:147-152`
* **Vulnerability Type**: Injection / Privilege Bypass
* **Impact**: High
* **Description**:
  The `run_query` function accepts an unvalidated raw query string:
  ```rust
  pub fn run_query(&self, query: &str, params: Option<Value>) -> Result<Value> {
      let p = params.map(json_obj_to_params).unwrap_or_default();
      let rows = cozo_run(&self.db, query, p)
  ```
  If this interface is exposed directly to higher-level tools (such as through the Model Context Protocol or DBus/RPC interfaces), an untrusted input can easily execute arbitrary Datalog scripts. This allows read/write access to internal system relations, including user authentication parameters (`users` and `sessions`).
* **Remediation**:
  Expose only strictly structured, pre-defined, and parameter-bound query routines. Avoid exposing raw dynamic query executors directly to internal tools or external systems without strict verification boundaries.

---

## 3. Schema-as-Code & Quality Violations

### Ad-Hoc Database Schemas & Unstructured JSON Text Columns
* **File**: `crates/op-cozo-store/src/lib.rs:66-135`
* **Vulnerability Type**: Schema-as-Code / Unversioned Schema Contract
* **Impact**: Quality & Compliance Violation
* **Description**:
  Data contracts are expressed as raw, ad-hoc inline Datalog strings in `seed_schema` rather than versioned Protocol Buffers or standardized JSON schemas:
  1. The schema defines generic string columns (`props` default `"{}"` in `graph_node` and `graph_edge`, `metadata` default `"{}"` in `memory_namespaces`, `value` and `tags` in `memory_entries`) which bypass type safety.
  2. The taxonomy of OSCAL properties under `subid_registry` (e.g., `control_refs`, `statement_refs`, `control_source`) is structured as flat strings without validation against the official OSCAL formats or Protocol Buffers.
* **Remediation**:
  Enforce a strict code-as-schema discipline. Define all entities (such as nodes, edge attributes, compliance rules, and OSCAL taxonomies) as versioned Protocol Buffer structures. Serialize these payloads deterministically into bytes or validated JSON before writing them into storage.

### Passive Session Expiration Parsing Defect
* **File**: `crates/op-cozo-store/src/lib.rs:369-382`
* **Vulnerability Type**: Logic / Session Expiration Smells
* **Impact**: Medium
* **Description**:
  The `lookup_session` function fetches raw string values of session objects:
  ```rust
  let created = dv_as_str(&row[1]).unwrap_or("").to_string();
  let expires = dv_as_str(&row[2]).unwrap_or("").to_string();
  ```
  However, `lookup_session` does not parse or validate the returned expiration timestamp string against the current clock time (`Utc::now()`). If the calling context delegates checking session expiration or forgets to execute a datetime validation check, expired sessions will be passively accepted as authorized.
* **Remediation**:
  Integrate the expiration validation logic natively inside the query structure (e.g., using Datalog filters) or within the `lookup_session` wrapper to prevent returning expired sessions to caller targets.

### Unvalidated Public Keys and Sanitization Absence
* **File**: `crates/op-cozo-store/src/lib.rs:328-333`
* **Vulnerability Type**: Input Validation / Data Sanitization
* **Impact**: Low / Quality Defect
* **Description**:
  `upsert_user` and `user_exists` accept WireGuard public keys as generic `&str` objects. There are no format, checksum, base64 validation, or character-length assertions. This permits dirty data or abnormally sized string values to populate indexed fields, potentially leading to query execution performance issues.
* **Remediation**:
  Incorporate a parser stage that validates WireGuard public keys against expected standards (e.g., exactly 32 raw bytes or 44 base64 characters) before executing queries.