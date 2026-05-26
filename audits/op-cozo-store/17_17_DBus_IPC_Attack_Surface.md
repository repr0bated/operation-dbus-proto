# Production Security & Quality Audit: op-cozo-store

## 1. D-Bus & IPC Attack Surface Audit

### Registered Interfaces, Methods, and Signals
No D-Bus interfaces, methods, or signals are registered in the files provided in the FILES section. 

The `crates/op-cozo-store` library acts as a low-level database abstraction layer wrapping CozoDB. It does not contain any `#[dbus_interface]` attributes, `zbus` object registrations, or direct IPC network transport listeners. The parent workspace configuration in `Cargo.toml` lists multiple dependent crates (such as `op-dbus` and `op-grpc-bridge`) that integrate `zbus`, but the underlying implementation of D-Bus endpoints resides outside the audited source scope.

### Authorization & Identity Verification
Because no direct D-Bus bindings are implemented in `crates/op-cozo-store/src/lib.rs`, there are no D-Bus caller identity checks (`connection.peer_credentials()`) within this crate. 

The public APIs exposed by `CozoGraphShuttle` (e.g., `run_query`, `store_compliance_rule`, `register_subid`, `create_session`, `delete_session`) do not enforce internal authentication or access control checks. They assume that any calling component has already performed authorization. If these raw methods are exposed to the D-Bus system or session bus in upstream integration crates without strict authorization checks, it will lead to privilege escalation.

### Mutating Methods & Process Spawning
The following methods in `crates/op-cozo-store/src/lib.rs` mutate the database state without performing caller authorization:
*   `CozoGraphShuttle::run_query` (line 175) — Can execute arbitrary mutating Datalog scripts.
*   `CozoGraphShuttle::store_compliance_rule` (line 212) — Inserts or replaces compliance deny/allow rules.
*   `CozoGraphShuttle::register_subid` (line 235) — Registers sub-identities in the taxonomy registry.
*   `CozoGraphShuttle::store_node` (line 267) — Inserts or updates identity-graph nodes.
*   `CozoGraphShuttle::store_edge` (line 277) — Inserts or updates graph edges.
*   `CozoGraphShuttle::append_audit_event` (line 319) — Appends audit trail entries.
*   `CozoGraphShuttle::upsert_user` (line 343) — Inserts user identities.
*   `CozoGraphShuttle::create_session` (line 367) — Creates active sessions.
*   `CozoGraphShuttle::delete_session` (line 396) — Destroys active sessions.

No process-spawning capabilities are present in the provided source files.

### Bus Connections & Deserialization Validation
*   **Bus Connection Type**: The audited crate does not establish bus connections. 
*   **Deserialization Vulnerabilities**: The crate does not directly deserialize raw caller-supplied byte buffers. However, the `run_query` method (line 175) converts arbitrary JSON values into database parameters using `json_obj_to_params` (line 424) and `json_to_dv` (line 434) without schema validation. If the query parameters contain structurally malformed objects, they can lead to query failures or unexpected type coercion issues within the CozoDB engine.

---

## 2. Schema-as-Code & OSCAL Compliance Discipline Audit

The codebase violates the schema-as-code discipline by expressing critical data contracts, taxonomy relations, and system models as ad-hoc strings and unstructured JSON values rather than versioned Protocol Buffers or generated OSCAL schemas.

### Ad-hoc Schema Definitions
In `crates/op-cozo-store/src/lib.rs`, the schema initialization is driven by raw Datalog string literals executed during database seeding:

```rust
// crates/op-cozo-store/src/lib.rs:59-139
        let relations = [
            r#":create compliance_rule { ... }"#,
            r#":create subid_registry { ... }"#,
            r#":create graph_node { ... }"#,
            r#":create graph_edge { ... }"#,
            ...
```

Instead of using code-generated data access objects from a versioned schema file, columns are dynamically structured, preventing any static validation of database migrations or data invariants.

### Raw JSON Column Injections
Multiple schemas rely on string columns containing raw, unvalidated JSON blobs rather than typed, nested sub-schemas:
*   `graph_node` (line 92): `props: String default "{}"`
*   `graph_edge` (line 98): `props: String default "{}"`
*   `memory_namespaces` (line 125): `metadata: String default "{}"`
*   `memory_entries` (line 134): `tags: String default "[]"`

This approach bypasses type safety and relational constraints, leaving serialization consistency to runtime application code.

### Ad-hoc OSCAL/Sub-identity Taxonomies
The `register_subid` method (line 235) handles critical mapping of NIST / EU-AI-Act compliance metadata:

```rust
// crates/op-cozo-store/src/lib.rs:235-242
    pub fn register_subid(
        &self,
        subid: &str, category: &str, component_type: &str,
        subject: &str, verb: &str, facet: &str, version: u8,
        control_source: &str, control_refs: &str, statement_refs: &str,
    ) => Result<()>
```

Crucial compliance elements, such as `control_refs` and `statement_refs`, are received as primitive `&str` objects rather than formal OSCAL XML or JSON schema definitions. This compromises compliance metadata integrity and prevents verification against standard OSCAL profiles.

---

## 3. Actionable Security & Code Quality Findings

### CRITICAL: Fail-Open Compliance Policy Bypass on Database Query Error
*   **File**: `crates/op-cozo-store/src/lib.rs`
*   **Lines**: 209–210
*   **Vulnerability Type**: Insecure Fail-Open Logic / Error-Handling Bypass

#### Description
The `evaluate_mutation` method determines if a mutation should be blocked based on security and compliance rules stored in the database. When executing the policy query, the function uses a catch-all `match` block. If *any* database error occurs (such as transaction conflicts, table locks, temporary memory exhaustion, disk write failures, or engine corruption), the logic defaults to an `Err(_)` handler:

```rust
// crates/op-cozo-store/src/lib.rs:183-211
    pub fn evaluate_mutation(&self, plugin_id: &str, operation: &str) -> PolicyVerdict {
        let query = r#"
            deny_rule[reason] :=
                *compliance_rule[plugin, op, action, reason, _, _],
                action = 'Deny',
                (plugin = $plugin || plugin = '*'),
                (op = $op || op = '*')
            ?[reason] := deny_rule[reason]
        "#;
        ...
        match cozo_run(&self.db, query, p) {
            Ok(rows) if rows.rows.is_empty() => {
                PolicyVerdict { allow: true, reason: "no deny rule matched".into() }
            }
            Ok(rows) => {
                ...
                PolicyVerdict { allow: false, reason }
            }
            Err(_) => PolicyVerdict { allow: true, reason: "compliance graph not seeded".into() },
        }
    }
```

#### Exploitability & Risk
This logic is fail-open. An attacker who can trigger any transient database error or resource exhaustion (e.g., by executing a heavy concurrent query, locking the database engine, or triggering a temporary read/write failure) can cause `cozo_run` to return an `Err`. 

Because of the fail-open fallback, the application will return `PolicyVerdict { allow: true, ... }`, bypassing all compliance deny-rules. This allows forbidden mutations to execute unchallenged.

#### Remediation
Refactor `evaluate_mutation` to return a `Result<PolicyVerdict>` or default to `allow: false` on error. In security enforcement logic, error paths must always fail-closed:

```rust
            Err(e) => PolicyVerdict { 
                allow: false, 
                reason: format!("Compliance engine error: {}", e) 
            },
```

---

### HIGH: Arbitrary Database Mutation and Read via Unrestricted `run_query`
*   **File**: `crates/op-cozo-store/src/lib.rs`
*   **Lines**: 175–180
*   **Vulnerability Type**: Arbitrary Script Execution / Privilege Escalation

#### Description
The public method `run_query` exposes an interface that executes raw Datalog strings:

```rust
// crates/op-cozo-store/src/lib.rs:175-180
    pub fn run_query(&self, query: &str, params: Option<Value>) -> Result<Value> {
        let p = params.map(json_obj_to_params).unwrap_or_default();
        let rows = cozo_run(&self.db, query, p)
            .map_err(|e| anyhow::anyhow!("CozoDB query failed: {e}"))?;
        Ok(named_rows_to_json(rows))
    }
```

This method executes scripts with `ScriptMutability::Mutable` via `cozo_run` (line 12). There is no validation, sanitization, or abstract syntax tree (AST) inspection performed on the incoming `query` parameter.

#### Exploitability & Risk
If `run_query` is exposed directly or indirectly to untrusted clients via D-Bus, gRPC, or MCP, an attacker can pass arbitrary Datalog commands. Because scripts are executed with `ScriptMutability::Mutable`, an attacker can modify data, delete tables, read sensitive keys (e.g., sessions, users), or insert unauthorized compliance rules.

#### Remediation
1. Avoid exposing raw execution interfaces. Use parameterized, pre-defined functions for database interactions.
2. If raw queries are necessary, restrict them to `ScriptMutability::Immutable` for read-only operations, or implement strict AST parse-filtering before execution.

---

### HIGH: Insecure Session Expiry Storage and Validation Failure
*   **File**: `crates/op-cozo-store/src/lib.rs`
*   **Lines**: 376–379, 381–393
*   **Vulnerability Type**: Insecure Session Management / Broken Authentication

#### Description
The `sessions` table schema defines `expires_at` as a string type (line 114), representing an RFC3339 timestamp. However, `lookup_session` merely retrieves this string value without validating if the current system time has passed the expiration time:

```rust
// crates/op-cozo-store/src/lib.rs:381-393
    pub fn lookup_session(&self, session_id: &str) -> Result<Option<(String, String, String)>> {
        let mut p: Params = BTreeMap::new();
        p.insert("sid".into(), DataValue::Str(session_id.into()));
        let r = cozo_run(
            &self.db,
            "?[wg_pubkey, created_at, expires_at] := \
             *sessions[sid, wg_pubkey, created_at, expires_at], sid = $sid",
            p,
        ).map_err(|e| anyhow::anyhow!("lookup session: {e}"))?;
        if let Some(row) = r.rows.first() {
            let wg = dv_as_str(&row[0]).unwrap_or("").to_string();
            let created = dv_as_str(&row[1]).unwrap_or("").to_string();
            let expires = dv_as_str(&row[2]).unwrap_or("").to_string();
            Ok(Some((wg, created, expires)))
        } else {
            Ok(None)
        }
    }
```

#### Exploitability & Risk
Since `lookup_session` returns expired sessions without validating the time constraint, upstream authentication filters that assume `lookup_session` only returns active sessions may accept expired credentials. This allows expired sessions to persist indefinitely, increasing the window of opportunity for token replay attacks.

#### Remediation
Incorporate chronological validation within `lookup_session` using the `chrono` library (already present in the crate's dependencies). If `expires_at` is populated and has passed, delete the session and return `Ok(None)`:

```rust
            let expires = dv_as_str(&row[2]).unwrap_or("").to_string();
            if !expires.is_empty() {
                if let Ok(expiry_time) = chrono::DateTime::parse_from_rfc3339(&expires) {
                    if chrono::Utc::now() > expiry_time {
                        self.delete_session(session_id)?;
                        return Ok(None);
                    }
                }
            }
```

---

### MEDIUM: JSON Number Type Coercion Inconsistency
*   **File**: `crates/op-cozo-store/src/lib.rs`
*   **Lines**: 438–444
*   **Vulnerability Type**: Data Representation Inconsistency / Validation Bypass

#### Description
The parameter helper function `json_to_dv` maps JSON numbers into CozoDB values:

```rust
// crates/op-cozo-store/src/lib.rs:438-444
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                dv_int(i)
            } else {
                DataValue::Num(cozo::Num::Float(n.as_f64().unwrap_or(0.0)))
            }
        }
```

#### Risk
If an application schema expects a `Float` but the parameter value has no fractional part, `n.as_i64()` will succeed and serialize the float as `Num::Int`. Conversely, numbers that overflow standard float parsers default to `0.0` rather than returning a validation error. 

This type coercion inconsistency can cause query failures or logic bypasses in Datalog queries that perform exact type matching (such as `DataValue::Num(cozo::Num::Int)` vs. `DataValue::Num(cozo::Num::Float)`).

#### Remediation
Enforce explicit type declarations on incoming JSON payloads, or perform strict boundary validation to prevent silent truncation or precision loss during numeric mapping.