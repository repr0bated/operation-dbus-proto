# Production Security and Quality Audit: op-cozo-store

## Build Role Check

### Cargo Manifest Analysis
*   **Edition**: The workspace package defines `edition = "2021"` (`Cargo.toml:26`). The `op-cozo-store` crate inherits this via `edition.workspace = true` (`crates/op-cozo-store/Cargo.toml:5`).
*   **Rust Version**: No `rust-version` is specified in either the workspace manifest (`Cargo.toml`) or the crate manifest (`crates/op-cozo-store/Cargo.toml`).
*   **Binaries and Examples**: No binaries or examples are configured in `crates/op-cozo-store/Cargo.toml`.
*   **Workspace Inheritance**: The crate relies heavily on workspace inheritance:
    *   Inherited package metadata: `version`, `edition`, and `license` (`crates/op-cozo-store/Cargo.toml:4-6`).
    *   Inherited dependencies: `anyhow`, `chrono`, `cozo`, `serde_json`, and `tracing` are all resolved via workspace specifications (`crates/op-cozo-store/Cargo.toml:9-13`).

### Schema-As-Code Build Check
*   **Codegen Risks**: No `build.rs` is present or invoked for the `op-cozo-store` crate.
*   **Protobuf / Tonic Compilation**: The `op-cozo-store` crate does not invoke `prost-build` or `tonic-build` at build time. No `.proto` files are checked into the `crates/op-cozo-store` directory.
*   **Runtime vs Build-Time Compilation**: No runtime compilation of protobuf files occurs in this crate.
*   **Schema-as-Code Violations**: 
    *   **Ad-hoc Database Schemas**: The schema for CozoDB relations (`compliance_rule`, `subid_registry`, `graph_node`, `graph_edge`, `audit_event`, `users`, `sessions`, `memory_namespaces`, `memory_entries`) is defined as ad-hoc, unversioned raw string literals inside the Rust codebase (`crates/op-cozo-store/src/lib.rs:62-142`) rather than loaded from versioned, declarative schema files (such as Protocol Buffers or OSCAL JSON/YAML schemas).

---

## Security and Quality Findings

### Critical Severity

#### Compliance Engine Fails Open on Database/Query Error
*   **Location**: `crates/op-cozo-store/src/lib.rs:191`
*   **Impact**: Direct bypass of the entire policy compliance engine.
*   **Description**: In `evaluate_mutation`, the system evaluates a mutation against compliance rules (Datalog rules marked with `action = 'Deny'`). However, the `match` statement handles query execution errors as follows:
    ```rust
    Err(_) => PolicyVerdict { allow: true, reason: "compliance graph not seeded".into() },
    ```
    If `cozo_run` fails due to transient database locking, file system corruption, resource exhaustion (Out of Memory), or database engine errors, the function catches the error and explicitly returns `allow: true`.
*   **Exploitability**: An attacker capable of triggering database contention, a transient lock, or inducing resource exhaustion on the underlying Sled storage engine can bypass all compliance/deny checks entirely, allowing unauthorized mutations to execute.
*   **Remediation**: The compliance evaluator must fail-closed. If a database query fails, return `allow: false` with the underlying error details to prevent actions from executing when the policy decision cannot be reliably determined.

---

### Medium Severity

#### Session Expiry Validation Deferred to Callers
*   **Location**: `crates/op-cozo-store/src/lib.rs:350-362`
*   **Impact**: Session hijacking/replay via expired sessions if callers omit validation.
*   **Description**: The `lookup_session` function queries the database for a session matching the requested `session_id`. It extracts and returns the raw `expires_at` string (RFC3339) along with the WireGuard public key and creation time:
    ```rust
    if let Some(row) = r.rows.first() {
        let wg = dv_as_str(&row[0]).unwrap_or("").to_string();
        let created = dv_as_str(&row[1]).unwrap_or("").to_string();
        let expires = dv_as_str(&row[2]).unwrap_or("").to_string();
        Ok(Some((wg, created, expires)))
    }
    ```
    The function does *not* validate whether the session has expired relative to the current system time, nor does it filter out expired sessions in the Datalog query itself. 
*   **Exploitability**: If a downstream developer calling `lookup_session` assumes that retrieving a session implies it is currently valid, expired sessions will be accepted as active.
*   **Remediation**: Perform the date-time comparison internally within `lookup_session` using `chrono::Utc::now()` and return `None` (or an explicit error) if the current time exceeds `expires_at`. Alternatively, restrict the query in Datalog to exclude expired sessions.

---

### Low Severity

#### Schema Violation and Unstructured JSON Storage
*   **Location**: `crates/op-cozo-store/src/lib.rs:240`, `crates/op-cozo-store/src/lib.rs:249`
*   **Impact**: Data corruption, logical inconsistencies, and lack of type safety.
*   **Description**: `store_node` and `store_edge` allow saving arbitrary JSON properties (`props: Value`) by calling `props.to_string()` and writing them into the CozoDB String fields (`props: String default "{}"`). There is no schema validation or contract enforcement performed on these strings prior to storage.
*   **Remediation**: Standardize structured properties under a versioned, schema-defined format (such as JSON Schema or Protocol Buffers) and validate the payload before persisting it to the database.