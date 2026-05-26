# Production Security and Quality Audit

---

## 1. Security Audit & Vulnerability Analysis

This section identifies security deficiencies within `crates/op-cozo-store/src/lib.rs` and classifies them by threat severity.

### Finding 1 [CRITICAL]: Case-Sensitivity Flaw in Policy Evaluation Bypass
* **Location**: `crates/op-cozo-store/src/lib.rs:189-211` and `crates/op-cozo-store/src/lib.rs:214-232`
* **Vulnerability Type**: Input Validation / Authorization Bypass
* **Mechanism**: 
  The Datalog policy query in `evaluate_mutation` restricts matched rule blocks using a hardcoded case-sensitive string literal:
  ```rust
  deny_rule[reason] :=
      *compliance_rule[plugin, op, action, reason, _, _],
      action = 'Deny',
      (plugin = $plugin || plugin = '*'),
      (op = $op || op = '*')
  ```
  However, `store_compliance_rule` takes `action: &str` without performing validation, normalization, or casing constraints. It directly inserts whatever string the caller provides into the `$action` variable of the `compliance_rule` relation.
* **Exploit Vector**: 
  If an administrator or automated system registers a compliance deny rule using `"deny"`, `"DENY"`, or any other variation instead of exactly `"Deny"`, the policy query will fail to match. The mutation will bypass the block list entirely and return `PolicyVerdict { allow: true, ... }`.

---

### Finding 2 [HIGH]: Compliance Engine Fails Open on Internal Database Errors
* **Location**: `crates/op-cozo-store/src/lib.rs:207`
* **Vulnerability Type**: Insufficient Error Handling / Fail-Open
* **Mechanism**: 
  The `evaluate_mutation` method evaluates policy logic inside a `match` expression:
  ```rust
  match cozo_run(&self.db, query, p) {
      Ok(rows) if rows.rows.is_empty() => {
          PolicyVerdict { allow: true, reason: "no deny rule matched".into() }
      }
      Ok(rows) => { ... }
      Err(_) => PolicyVerdict { allow: true, reason: "compliance graph not seeded".into() },
  }
  ```
  If `cozo_run` returns an `Err` due to a Sled engine lock contention, corruption, full disk, or timeout, the match arm falls back to the `Err(_)` pattern and issues `PolicyVerdict { allow: true, ... }`.
* **Exploit Vector**: 
  An attacker can intentionally trigger performance degradation, filesystem saturation, or database thread exhaustion to force database errors. Under these conditions, the compliance engine will fail open and authorize operations that should be strictly blocked under NIST/EU-AI-Act compliance requirements.

---

### Finding 3 [HIGH]: Injection Vulnerability via Arbitrary Datalog Query Execution
* **Location**: `crates/op-cozo-store/src/lib.rs:180-185`
* **Vulnerability Type**: Datalog/SQL Injection Risk
* **Mechanism**: 
  The public method `run_query` exposes direct, unparameterized database query execution:
  ```rust
  pub fn run_query(&self, query: &str, params: Option<Value>) -> Result<Value> {
      let p = params.map(json_obj_to_params).unwrap_or_default();
      let rows = cozo_run(&self.db, query, p)
          .map_err(|e| anyhow::anyhow!("CozoDB query failed: {e}"))?;
      Ok(named_rows_to_json(rows))
  }
  ```
  If this interface is exposed to any untrusted network edge, gateway API, or proxy, it allows remote users to execute arbitrary Datalog queries.
* **Exploit Vector**: 
  An attacker can bypass parameterized interfaces to read out active session tokens from the `sessions` relation, alter the `users` directory, modify `compliance_rule` parameters to disable validation logic, or dump user memory contexts.

---

### Finding 4 [MEDIUM]: Plaintext Storage of Session Tokens and Compliance Metrics
* **Location**: `crates/op-cozo-store/src/lib.rs:49-56`
* **Vulnerability Type**: Cryptographic Storage Deficiency
* **Mechanism**: 
  Persistent database storage is initiated with `DbInstance::new("sled", &ps, Default::default())`. Sled stores all keys and values in plaintext on disk by default.
* **Exploit Vector**: 
  In shared tenant environments or systems with local file access, any compromise of the hosting filesystem exposes plain-text WireGuard keys (`wg_pubkey`), session keys (`session_id`), and compliance log payloads.

---

## 2. Schema-As-Code Compliance Audit

The `op-cozo-store` database layer violates the schema-as-code discipline. Data contracts are defined as ad-hoc strings and unstructured JSON fields rather than strongly-typed, versioned serialization schemas.

### Violation 1: Ad-hoc Relational Declarations via Raw Datalog Strings
* **Location**: `crates/op-cozo-store/src/lib.rs:66-176`
* **Deficiency**: 
  The schemas for all nine tables (`compliance_rule`, `subid_registry`, `graph_node`, `graph_edge`, `audit_event`, `users`, `sessions`, `memory_namespaces`, and `memory_entries`) are defined using raw multiline string slices inside the private method `seed_schema()`. 
* **Impact**: 
  Any schema evolution must be manually managed as string manipulations. There is no automated versioning, compilation validation, or compatibility validation against existing database migrations.

### Violation 2: Unstructured JSON Blobs Stored as Database Strings
* **Location**: 
  * `crates/op-cozo-store/src/lib.rs:95` (`props: String default "{}"` in `graph_node`)
  * `crates/op-cozo-store/src/lib.rs:101` (`props: String default "{}"` in `graph_edge`)
  * `crates/op-cozo-store/src/lib.rs:134` (`metadata: String default "{}"` in `memory_namespaces`)
  * `crates/op-cozo-store/src/lib.rs:141` (`value: String default "null"` in `memory_entries`)
  * `crates/op-cozo-store/src/lib.rs:142` (`tags: String default "[]"` in `memory_entries`)
* **Deficiency**: 
  The codebase persists highly structured application data using ad-hoc JSON strings.
* **Impact**: 
  These properties bypass database verification. Schema changes are not compiled or validated, increasing the risk of data deserialization panics at runtime.

### Violation 3: Degradation of OSCAL and Compliance Taxonomy Contracts
* **Location**: `crates/op-cozo-store/src/lib.rs:236-241` (`register_subid`) and `crates/op-cozo-store/src/lib.rs:334-340` (`append_audit_event`)
* **Deficiency**: 
  OSCAL and NIST compliance structures (e.g. `control_refs`, `statement_refs`, `control_source`) are passed as ad-hoc, untyped `&str` values. 
* **Impact**: 
  This structure allows invalid or corrupted OSCAL compliance statements to bypass schema verification, violating the strict requirements of OSCAL-compliance systems.

### Remediation Blueprint:
1. Replace all unstructured data formats with strongly-typed **Protocol Buffers** (utilizing `prost` via workspace dependencies).
2. Define a versioned `.proto` definition for graph nodes, edge properties, and OSCAL-compliant audit events:
   ```protobuf
   syntax = "proto3";
   package compliance.v1;

   message GraphNodeProps {
     map<string, string> attributes = 1;
   }

   message OscalControlReference {
     string source = 1;
     repeated string control_refs = 2;
     repeated string statement_refs = 3;
   }
   ```
3. Serialize these structs into binary or strict JSON formats before storing them in the Cozo database, ensuring compile-time schema verification.

---

## 3. Public API Surface Enumeration

The public API surface of the `op-cozo-store` library is listed below.

### Summary
* **Public Structs**: 2
* **Public Struct Fields**: 2
* **Public Implementation Methods**: 21
* **Public Functions**: 1
* **Glob Re-exports (`pub use *`)**: 0

---

### Top 10 Most Impactful Public API Elements

| Element | Type | File:Line | Impact |
| :--- | :--- | :--- | :--- |
| `CozoGraphShuttle` | Struct | `crates/op-cozo-store/src/lib.rs:37` | Core wrapper managing the underlying CozoDB instance. |
| `evaluate_mutation` | Method | `crates/op-cozo-store/src/lib.rs:189` | Evaluates system mutations against registered compliance rules. |
| `run_query` | Method | `crates/op-cozo-store/src/lib.rs:180` | Executes arbitrary query scripts directly on the database instance. |
| `store_compliance_rule` | Method | `crates/op-cozo-store/src/lib.rs:214` | Writes or updates active policy rules inside the database. |
| `register_subid` | Method | `crates/op-cozo-store/src/lib.rs:236` | Registers canonical OSCAL taxonomies. |
| `traverse_graph` | Method | `crates/op-cozo-store/src/lib.rs:317` | Performs a BFS traversal across system identity relations. |
| `append_audit_event` | Method | `crates/op-cozo-store/src/lib.rs:334` | Appends record payloads to the immutable audit log table. |
| `lookup_session` | Method | `crates/op-cozo-store/src/lib.rs:401` | Resolves active user sessions. |
| `db` | Method | `crates/op-cozo-store/src/lib.rs:430` | Exposes a direct, shared handle (`Arc<DbInstance>`) to the underlying database engine. |
| `PolicyVerdict` | Struct | `crates/op-cozo-store/src/lib.rs:20` | Output model containing policy validation results. |

---

### Complete List of Public Elements

```
crates/op-cozo-store/src/lib.rs
├── pub struct PolicyVerdict (line 20)
│   ├── pub allow: bool (line 21)
│   └── pub reason: String (line 22)
├── pub struct CozoGraphShuttle (line 37)
├── impl CozoGraphShuttle (line 40)
│   ├── pub fn new_in_memory() -> Result<Self> (line 41)
│   ├── pub fn new_persistent(path: PathBuf) -> Result<Self> (line 49)
│   ├── pub fn from_env() -> Result<Self> (line 58)
│   ├── pub fn run_query(&self, query: &str, params: Option<Value>) -> Result<Value> (line 180)
│   ├── pub fn evaluate_mutation(&self, plugin_id: &str, operation: &str) -> PolicyVerdict (line 189)
│   ├── pub fn store_compliance_rule(&self, plugin: &str, op: &str, action: &str, reason: &str, control_ref: &str) -> Result<()> (line 214)
│   ├── pub fn register_subid(&self, subid: &str, category: &str, component_type: &str, subject: &str, verb: &str, facet: &str, version: u8, control_source: &str, control_refs: &str, statement_refs: &str) -> Result<()> (line 236)
│   ├── pub fn store_node(&self, id: &str, label: &str, props: Value) -> Result<()> (line 260)
│   ├── pub fn store_edge(&self, src: &str, rel: &str, dst: &str, props: Option<Value>) -> Result<()> (line 271)
│   ├── pub fn query_edges_from(&self, src: &str) -> Result<Value> (line 286)
│   ├── pub fn query_edges_to(&self, dst: &str) -> Result<Value> (line 296)
│   ├── pub fn query_node(&self, id: &str) -> Result<Value> (line 306)
│   ├── pub fn traverse_graph(&self, start_node: &str, max_depth: u32) -> Result<Value> (line 317)
│   ├── pub fn append_audit_event(&self, event_id: &str, subid: &str, plugin_id: &str, operation: &str, actor: &str, verdict: bool, reason: &str, control_ref: &str) -> Result<()> (line 334)
│   ├── pub fn upsert_user(&self, wg_pubkey: &str) -> Result<()> (line 358)
│   ├── pub fn user_exists(&self, wg_pubkey: &str) -> Result<bool> (line 370)
│   ├── pub fn create_session(&self, session_id: &str, wg_pubkey: &str, expires_at: Option<&str>) -> Result<()> (line 383)
│   ├── pub fn lookup_session(&self, session_id: &str) -> Result<Option<(String, String, String)>> (line 401)
│   ├── pub fn delete_session(&self, session_id: &str) -> Result<()> (line 418)
│   └── pub fn db(&self) -> Arc<DbInstance> (line 430)
└── pub fn named_rows_to_json(rows: NamedRows) -> Value (line 475)
```

### Glob Re-exports
* **None detected.**

### Struct Field Access Audit
* `PolicyVerdict` (lines 20-23) exposes both `allow` and `reason` as public fields. 
  * *Critical Assessment*: These fields should be private to prevent callers from modifying execution outputs. They should be accessed via getter methods:
  ```rust
  pub struct PolicyVerdict {
      allow: bool,
      reason: String,
  }
  impl PolicyVerdict {
      pub fn allow(&self) -> bool { self.allow }
      pub fn reason(&self) -> &str { &self.reason }
  }
  ```

---

## 4. Dead Code Analysis

This section analyzes unreferenced logic and compiler warning overrides in `crates/op-cozo-store`.

### Attributes Override
* There are no active `#![allow(dead_code)]` or `#![allow(unused_imports)]` overrides in the source files.

---

### Unused Code Table

| Item | Type | File:Line | Recommendation |
| :--- | :--- | :--- | :--- |
| `memory_namespaces` Relation | Database Schema | `crates/op-cozo-store/src/lib.rs:133-144` | Implement programmatic CRUD APIs in `CozoGraphShuttle` or remove the schema definition. |
| `memory_entries` Relation | Database Schema | `crates/op-cozo-store/src/lib.rs:146-159` | Implement programmatic CRUD APIs in `CozoGraphShuttle` or remove the schema definition. |

*Note: Sled schemas for `memory_namespaces` and `memory_entries` are registered during `seed_schema` but have no corresponding public wrapper functions inside `CozoGraphShuttle`. Unless they are queried dynamically via `run_query` from separate modules, these definitions are dead code.*