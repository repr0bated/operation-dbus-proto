# Production Security and Quality Audit

## 1. License Audit

### Extracted License
* **Crate**: `op-cozo-store` (`crates/op-cozo-store/Cargo.toml`)
* **License**: `Apache-2.0` (inherited from workspace package definition in `Cargo.toml`)

### GPL/AGPL/SSPL Scan
A complete scan of `Cargo.lock` was performed against all registered dependencies.
* **Result**: **No GPL, AGPL, or SSPL licensed crates were found.**
* **Copyleft Note**: The crate list contains `cozo` (version `0.7.6`), which is licensed under the **MPL-2.0** (Mozilla Public License 2.0). MPL-2.0 is a weak copyleft license and is fully compatible with `Apache-2.0` when integrated as an external dependency/library without modifying the Cozo source code itself.

### Crates with No License Field
* **Result**: **None.** 
All crates declared within the workspace cleanly inherit their licensing information via the `license.workspace = true` configuration referring to the root `Cargo.toml`.

---

## 2. Schema-as-Code Compliance

This codebase utilizes Protocol Buffers and OSCAL for its unified schema-as-code discipline. Ad-hoc data structures, raw query projections, and in-code schema definition strings violate this rule by creating implicit data contracts that cannot be validated or versioned out-of-band.

### Finding 1: Ad-hoc Rust Struct for Compliance Policy Verdicts
* **File**: `crates/op-cozo-store/src/lib.rs`
* **Lines**: 16-20
* **Discipline Violation**: The `PolicyVerdict` struct is defined as an ad-hoc Rust struct:
  ```rust
  pub struct PolicyVerdict {
      pub allow: bool,
      pub reason: String,
  }
  ```
* **Remediation**: This contract should be generated via Protocol Buffers (e.g., `compliance.proto` defining a `PolicyVerdict` message) to ensure consistency across the gateway, services, and the database shuttle.

### Finding 2: In-Code Database Schema Declared via Raw Datalog Strings
* **File**: `crates/op-cozo-store/src/lib.rs`
* **Lines**: 71-163
* **Discipline Violation**: All relation schemas (such as `compliance_rule`, `subid_registry`, `audit_event`, `sessions`, etc.) are declared as ad-hoc raw Datalog strings inside the `seed_schema` function rather than being validated against and generated from versioned schemas (such as OSCAL Component Definitions for `subid_registry` or Protobuf messages for database records).
* **Remediation**: Move the relational schema definitions into versioned external schema files or code-generate schema migration steps from the central Protocol Buffer/OSCAL definitions.

### Finding 3: Ad-hoc JSON-to-Relational Mapping
* **File**: `crates/op-cozo-store/src/lib.rs`
* **Lines**: 412-427, 442-454
* **Discipline Violation**: The helper functions `json_to_dv` and `dv_to_json` perform ad-hoc mappings between untyped `serde_json::Value` objects and `cozo::DataValue` variants. This bypasses structured schema-as-code contracts, leading to potential type mismatch runtime errors if raw JSON payloads drift.
* **Remediation**: Avoid passing untyped JSON values. Use strongly typed, versioned serialized structures generated from the schema-as-code definitions.

---

## 3. Security and Quality Findings

### Finding 4: Silent Seeding Failures Bypasses Compliance Logic (CRITICAL)
* **File**: `crates/op-cozo-store/src/lib.rs`
* **Lines**: 165-173, 204
* **Vulnerability Type**: Faulty Error Handling / Silent Fail-Open Bypasses Policy
* **Description**: In `seed_schema`, when the database attempts to create relations, any execution error (e.g. disk full, read-only file system, transient database initialization issues) other than "AlreadyExists" is caught, printed to stderr, and logged as a warning. Crucially, **the function does not return an error** and silently succeeds, returning `Ok(())`.
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
  Because the boot sequence silently succeeds even if schemas fail to initialize, any subsequent call to `evaluate_mutation` will fail on execution because the `compliance_rule` table does not exist. Inside `evaluate_mutation` (line 204), the query's `Err(_)` arm is hit, which returns `PolicyVerdict { allow: true, reason: "compliance graph not seeded".into() }`.
* **Exploitability**: Directly exploitable. An attacker can leverage any systemic, filesystem, or database error at boot to force the application into a state where **all compliance checks and deny rules are completely bypassed**, failing open silently.
* **Remediation**: Change `seed_schema` to return an error if `cozo_run` fails (unless the error is strictly an "AlreadyExists" variant). Fail the system boot closed:
  ```rust
  if !msg.contains("already exists") && !msg.contains("AlreadyExists") {
      return Err(anyhow::anyhow!("Failed to seed schema: {msg}"));
  }
  ```

### Finding 5: Fail-Open Strategy on Policy Query Failures (HIGH)
* **File**: `crates/op-cozo-store/src/lib.rs`
* **Lines**: 204
* **Vulnerability Type**: Weak Security Policy Enforcement (Fail-Open)
* **Description**: In `evaluate_mutation`, if the query `cozo_run` fails for any reason (database lock contention, corruption, connection exhaustion, etc.), the database returns `PolicyVerdict { allow: true, ... }`:
  ```rust
  Err(_) => PolicyVerdict { allow: true, reason: "compliance graph not seeded".into() },
  ```
  Security policy evaluations must always fail-closed.
* **Impact**: If the database engine becomes overwhelmed or encounters an internal error, all compliance rules are bypassed, allowing prohibited mutations.
* **Remediation**: Change the recovery match arm to default to `allow: false` with an explicit system error description:
  ```rust
  Err(e) => PolicyVerdict { allow: false, reason: format!("Internal error during policy evaluation: {e}") },
  ```

### Finding 6: Missing Verification of Session Expiry (MEDIUM)
* **File**: `crates/op-cozo-store/src/lib.rs`
* **Lines**: 341-356
* **Vulnerability Type**: Lack of Token/Session Expiration Enforcement
* **Description**: The `lookup_session` method retrieves the `wg_pubkey`, `created_at`, and `expires_at` values from the session registry, but it does **not** evaluate whether the current system time exceeds the retrieved `expires_at` date.
* **Impact**: Downstream components may assume the database shuttle is only returning valid, active sessions. If those components fail to manually parse and validate the expiration timestamp, the system will accept expired sessions.
* **Remediation**: Validate the RFC3339 `expires_at` date within the lookup method, or perform the filtration directly inside the Datalog query using a `$now` parameter:
  ```rust
  // Inside lookup_session query
  "?[wg_pubkey, created_at, expires_at] := \
   *sessions[sid, wg_pubkey, created_at, expires_at], sid = $sid, \
   (expires_at = '' || expires_at > $now_ts)"
  ```