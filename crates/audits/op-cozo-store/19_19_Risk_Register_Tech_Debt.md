| Severity | Issue | Evidence (file:line) | Recommendation |
| :--- | :--- | :--- | :--- |
| **Critical** | Security Bypass via Fail-Open compliance evaluation on database errors | `crates/op-cozo-store/src/lib.rs:202` | Fail-closed by default. If a query error or database issue occurs during compliance evaluation, return a deny verdict (`allow: false`) or propagate the error to abort the transaction. |
| **High** | Unbounded Recursive Graph Traversal leading to Denial of Service / Resource Exhaustion | `crates/op-cozo-store/src/lib.rs:279` | Restrict `max_depth` with a hardcoded maximum threshold (e.g., `10`) at the API level and implement pagination/limits to prevent memory exhaustion from dense identity-graphs. |
| **High** | Schema-as-Code violation: Ad-hoc database tables and untyped raw string contracts | `crates/op-cozo-store/src/lib.rs:74` | Migrate to a centralized schema-as-code discipline using Protocol Buffers or versioned OSCAL schemas to define and enforce all data structures and relational tables, rather than using ad-hoc raw query strings. |
| **High** | Ad-hoc metadata and property fields stored as raw JSON strings without contract validation | `crates/op-cozo-store/src/lib.rs:95` | Apply strict JSON schema validation (using a tool like `jsonschema` or generated Protobuf types) to the `props` and `metadata` structures before serializing them as database strings. |
| **Med** | Mutable Execution Context for Read-Only Queries (Least Privilege Violation) | `crates/op-cozo-store/src/lib.rs:13` | Differentiate read-only queries from mutations. Implement a read-only execution helper that uses `ScriptMutability::Immutable` to restrict execution privileges during lookups. |
| **Med** | Incomplete Session Expiration Enforcement in Session Lookup | `crates/op-cozo-store/src/lib.rs:359` | Validate `expires_at` against the current timestamp inside `lookup_session` and return `None` if the session is expired. Implement automatic database-side pruning of expired sessions. |
| **Low** | Suppression of Schema Initialization Failures | `crates/op-cozo-store/src/lib.rs:144` | Propagate initialization errors instead of silently warning and returning `Ok(())`. Ensure that database startup fails fast if the schema cannot be fully seeded. |

---

### Detailed Findings & Technical Analysis

#### 1. Security Bypass via Fail-Open on Database Errors (Critical)
* **Evidence:** `crates/op-cozo-store/src/lib.rs:202`
* **Analysis:** The `evaluate_mutation` function runs a compliance Datalog script to check if a specific mutation matches any `Deny` action within the `compliance_rule` relation. However, if the query execution fails for any reason (e.g., database lock, disk full, corrupted index, memory exhaustion, or transient errors under load), the `Err(_)` match arm intercepts the result and returns `PolicyVerdict { allow: true, reason: "compliance graph not seeded".into() }`. This creates a severe security "fail-open" vulnerability. An attacker can deliberately trigger a transient database error (e.g., via resource exhaustion) to completely bypass the compliance guardrail and execute forbidden actions.
* **Exploit Vector:** An attacker can trigger a database resource exhaustion using the unbounded graph traversal API (`traverse_graph`) with a very high `max_depth`. When the CozoDB engine runs out of memory or times out, subsequent mutation requests evaluated via `evaluate_mutation` will encounter database errors and fail-open, completely bypassing all NIST/EU-AI-Act compliance policy checks.

#### 2. Unbounded Recursive Graph Traversal (High)
* **Evidence:** `crates/op-cozo-store/src/lib.rs:279`
* **Analysis:** The `traverse_graph` method performs recursive Datalog BFS traversals on the identity graph using user-controlled `max_depth` bounds. Because `max_depth` is typed as a raw `u32` with no validation or bounds checking inside the function, a user can supply an extremely large value. When executed against a cyclic or dense graph, this causes rapid recursion and path explosion, leading to unbounded memory growth and a Denial of Service (DoS) of the hosting process.
* **Remediation:** Introduce a hard-limit threshold for the graph traversal depth (e.g., `const MAX_ALLOWED_DEPTH: u32 = 10`) and reject any requests exceeding this limit.

#### 3. Schema-as-Code Violations (High)
* **Evidence:** `crates/op-cozo-store/src/lib.rs:74`, `crates/op-cozo-store/src/lib.rs:95`
* **Analysis:** The relational schemas in `seed_schema` (such as `compliance_rule`, `subid_registry`, `graph_node`, and `memory_namespaces`) are constructed dynamically using ad-hoc raw string literals rather than versioned contracts (such as Protocol Buffers or OSCAL schemas). Furthermore, fields like `props` (in `graph_node` and `graph_edge`) and `metadata` (in `memory_namespaces`) are stored as arbitrary strings initialized with `"{}"` or `"null"`. Bypassing structured types at the storage layer allows schema drift, invalid JSON syntax, and unvalidated payloads to compromise data integrity across different services or deployment upgrades.
* **Remediation:** Define standard Protobuf schemas representing these types, parse them strictly upon egress/ingress, and validate arbitrary JSON fields against a compiled schema using the workspace's `jsonschema` library before calling database insert operations.

#### 4. Mutable Query Execution by Default (Medium)
* **Evidence:** `crates/op-cozo-store/src/lib.rs:13`
* **Analysis:** The `cozo_run` database helper is hardcoded to execute all scripts with `ScriptMutability::Mutable`. This violates the principle of least privilege. Simple read-only operations, such as session lookup (`lookup_session`), policy checking (`evaluate_mutation`), or node query (`query_node`), are executed with write permissions. If any part of the query strings contains untrusted user input, it could permit write/delete execution blocks.
* **Remediation:** Implement a read-only query helper that invokes `db.run_script` with `ScriptMutability::Immutable` and use it exclusively for read operations.

#### 5. Session Expiration Missing Database-Side Enforcement (Medium)
* **Evidence:** `crates/op-cozo-store/src/lib.rs:359`
* **Analysis:** The `lookup_session` method retrieves the `expires_at` string (formatted as RFC3339) but does not validate if the current system time has surpassed the expiration date. It returns the raw session details directly. If the upstream consumer of `lookup_session` fails to implement validation, expired sessions will be accepted as valid. Additionally, expired sessions remain in the embedded database indefinitely, leading to unnecessary data persistence.
* **Remediation:** Parse the `expires_at` timestamp inside `lookup_session` using `chrono` and compare it to `Utc::now()`. Return `Ok(None)` if expired, and execute background prune queries (`:rm sessions`) to purge invalid sessions.