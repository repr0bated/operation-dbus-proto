### Environment Variable Inventory

| Environment Variable | File & Line Citation | Default Value / Fallback | Error Handling / Panic Risk |
| :--- | :--- | :--- | :--- |
| `COGNITIVE_MCP_COZO_DB_PATH` | `crates/op-cozo-store/src/lib.rs:60` | Fallback to in-memory instance via `Self::new_in_memory()` | **Safe**: Handled via `if let Ok(p)` wrapper. If the variable is absent, it gracefully falls back without panicking or returning an error. |

---

### Cargo Features Analysis

#### Crate: `op-cozo-store`
* **Local Features**: None defined in `crates/op-cozo-store/Cargo.toml`.
* **Dependency Feature Additivity**: 
  * Features in Cargo are additive. Because `op-cozo-store` does not define any features of its own, it inherits all dependency configurations resolved at the workspace level.
  * In `Cargo.toml` (workspace level), the `cozo` dependency explicitly disables default features and activates specific backends:
    ```toml
    cozo = { version = "0.7.6", default-features = false, features = ["rayon", "storage-sled"] }
    ```
    This configuration ensures the `sled` pure-Rust embedded storage engine is compiled, avoiding potential C-linkage conflicts with `rusqlite` used elsewhere in the workspace.

---

### Hardcoded Paths, Ports, and Addresses

No hardcoded IP addresses, port numbers, or absolute host file paths were found in the provided files. However, the following database engine configuration identifiers are hardcoded:

* **In-Memory Engine Type**: `crates/op-cozo-store/src/lib.rs:44`
  * Hardcoded to `"mem"` engine.
* **Persistent Engine Type**: `crates/op-cozo-store/src/lib.rs:52`
  * Hardcoded to `"sled"` engine.

---

### Schema-as-Code Violations

The codebase bypasses the workspace's schema-as-code discipline (which mandates Protocol Buffers or versioned OSCAL schemas) by defining ad-hoc structures, untyped JSON envelopes, and raw embedded Datalog strings.

#### 1. Embedded Schema Definitions via Datalog String Slices
* **Citation**: `crates/op-cozo-store/src/lib.rs:67-158`
* **Finding**: The database schemas for nine core relations (including `compliance_rule`, `subid_registry`, `audit_event`, `users`, and `sessions`) are declared as raw, unversioned string literals inside the `seed_schema` method. There is no mapping to unified protobuf message definitions or OSCAL taxonomies.

#### 2. Ad-hoc Struct Definitions
* **Citation**: `crates/op-cozo-store/src/lib.rs:17-20`
* **Finding**: `PolicyVerdict` is declared as an ad-hoc Rust struct:
  ```rust
  pub struct PolicyVerdict {
      pub allow: bool,
      pub reason: String,
  }
  ```
  Instead of utilizing a versioned, schema-generated contract, this state is hand-written, making it difficult to safely evolve or serialize uniformly across process boundaries.

#### 3. Untyped JSON Envelopes
* **Citation**: `crates/op-cozo-store/src/lib.rs:169-174` and `crates/op-cozo-store/src/lib.rs:375-385`
* **Finding**: The `run_query` interface and its helper `named_rows_to_json` marshal database outputs directly into untyped `serde_json::Value` structures. This facilitates contract drift since clients cannot rely on a deterministic schema compiler to validate output structures.

#### 4. Raw String-Serialized Data Fields
* **Citation**: `crates/op-cozo-store/src/lib.rs:104`, `crates/op-cozo-store/src/lib.rs:109`, `crates/op-cozo-store/src/lib.rs:136`, and `crates/op-cozo-store/src/lib.rs:145`
* **Finding**: Structured properties are stored as text fields containing serialized raw JSON strings (defaulting to `"{}"`, `"[]"`, or `"null"`). This sidesteps both relational and serialized-binary schemas, resulting in raw string manipulation at runtime.

---

### Security and Quality Findings

#### [CRITICAL] Fail-Open Policy Evaluation on Database Query Errors
* **Citation**: `crates/op-cozo-store/src/lib.rs:198-200`
* **Description**: The compliance system evaluates authorization decisions by querying the `compliance_rule` relation for matching `Deny` rules. If the query execution fails (e.g., due to database lockups, storage exhaustion under load, schema mismatches, or resource constraints), the match arm falls back to an error catch-all that returns an **allow** decision:
  ```rust
  Err(_) => PolicyVerdict { allow: true, reason: "compliance graph not seeded".into() },
  ```
* **Exploitability**: Directly exploitable. An attacker can bypass the system's entire security compliance and deny-list policy engine by intentionally triggering a database engine error. This can be achieved by causing transaction conflicts, exhausting storage limits, or overloading the embedded database to force a query evaluation error. Because the system fails open, the mutation will be permitted.
* **Remediation**: Change the query failure branch to fail closed:
  ```rust
  Err(e) => PolicyVerdict { allow: false, reason: format!("compliance evaluation error: {e}") },
  ```

#### [HIGH] SQL/Datalog Injection Vulnerability via Unvalidated Raw Queries
* **Citation**: `crates/op-cozo-store/src/lib.rs:169-174`
* **Description**: `run_query` accepts an unvalidated `&str` and executes it directly against the database engine via `cozo_run`. If higher-level components pass untrusted input into this query parameter, it allows arbitrary Datalog injection.
* **Remediation**: Bind query parameters strictly using the `params` argument, or restrict direct access to `run_query` to administrative tasks.