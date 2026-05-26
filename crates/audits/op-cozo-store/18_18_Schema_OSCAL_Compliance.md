### 1. Schema-as-Code Audit

The following table documents violations where data contracts, database schemas, and message types are defined as ad-hoc structures or raw strings in Rust rather than versioned, declarative Protocol Buffer schemas.

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `PolicyVerdict` | Struct | `crates/op-cozo-store/src/lib.rs:19` | No | Custom Datalog-to-Rust conversion struct lacking a declarative `.proto` contract. Can easily fall out of sync with client-side requirements. |
| `compliance_rule` | Cozo Relation Schema | `crates/op-cozo-store/src/lib.rs:64` | No | Embedded raw string Datalog schema definition. No versioning or structural compatibility validation exists. |
| `subid_registry` | Cozo Relation Schema | `crates/op-cozo-store/src/lib.rs:71` | No | Ad-hoc flat string fields represent structured OSCAL taxonomy mappings, bypassing serialization-layer typing. |
| `graph_node` | Cozo Relation Schema | `crates/op-cozo-store/src/lib.rs:84` | No | Holds `props` as an untyped string with a default value of `"{}"`. No schema enforceability on graph attributes. |
| `graph_edge` | Cozo Relation Schema | `crates/op-cozo-store/src/lib.rs:90` | No | Directed edge properties are defined as untyped serialized JSON strings (`props`), preventing deterministic structural validation. |
| `audit_event` | Cozo Relation Schema | `crates/op-cozo-store/src/lib.rs:95` | No | Append-only security audit log payload uses loose strings for control references, timestamps, and verdicts. |
| `users` | Cozo Relation Schema | `crates/op-cozo-store/src/lib.rs:106` | No | Bare storage of public identity key without structured identity state or cryptographic profile contracts. |
| `sessions` | Cozo Relation Schema | `crates/op-cozo-store/src/lib.rs:111` | No | Session tracking structure defined implicitly via a multi-line Cozo creation script. No formal protocol buffers exist. |
| `memory_namespaces` | Cozo Relation Schema | `crates/op-cozo-store/src/lib.rs:117` | No | Stores named MCP memory metadata as untyped JSON strings (`metadata: String default "{}"`), violating static schema disciplines. |
| `memory_entries` | Cozo Relation Schema | `crates/op-cozo-store/src/lib.rs:129` | No | Key/value store elements representation uses raw string defaults (`value: String default "null"`, `tags: String default "[]"`). |
| `run_query` | Function API | `crates/op-cozo-store/src/lib.rs:166` | No | Exposes untyped arbitrary `serde_json::Value` both as input parameters and output results. |
| `json_obj_to_params` / `json_to_dv` | Conversion Helpers | `crates/op-cozo-store/src/lib.rs:435-456` | No | Hand-rolled conversion of arbitrary, untyped JSON structures into database values, prone to runtime boundary panic states. |
| `named_rows_to_json` / `dv_to_json` | Conversion Helpers | `crates/op-cozo-store/src/lib.rs:458-492` | No | Manual, nested decoding map loops translating database results into untyped JSON values. |

---

### 2. OSCAL Coverage Audit

The system implements core compliance and authorization checks in code but lacks explicit machine-readable linkages or validation logic back to localized OSCAL (Open Security Controls Assessment Language) system security plans, profiles, or component definitions.

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| Policy Decision Point (Authorization) | `crates/op-cozo-store/src/lib.rs:175-202` (`evaluate_mutation`) | Component Definition / System Security Plan (SSP) | The policy decision logic dynamically evaluates Datalog rule files matching a plugin/op pair. There is no automated connection validating that the `compliance_rule` database rows align with NIST 800-53 or EU-AI-Act control baselines defined in machine-readable OSCAL JSON/XML artifacts. |
| OSCAL Taxonomy Mapping | `crates/op-cozo-store/src/lib.rs:227` (`register_subid`) | Component Definition (Metadata Taxonomy) | Registers canonical subids into an internal index table using loose string parameters (`control_refs`, `statement_refs`). The system fails to verify whether the incoming `subid` or `control_refs` actually exist in the active OSCAL metadata profile. |
| Security Auditing | `crates/op-cozo-store/src/lib.rs:325` (`append_audit_event`) | SSP (NIST SP 800-53: AU-12 Audit Generation) | Appends structured records to the compliance logging partition (`audit_event`). However, these audit event definitions cannot be validated against the system's operational control declarations, meaning drift from system-level commitments is not caught. |
| Session & Access Termination | `crates/op-cozo-store/src/lib.rs:374` (`create_session`), `crates/op-cozo-store/src/lib.rs:391` (`lookup_session`) | SSP (NIST SP 800-53: AC-12 Session Termination / AC-2 Account Management) | The system writes session bounds including a string-based `expires_at` timestamp. However, `lookup_session` merely returns the expiry string as-is and relies on downstream consumer code to enforce termination. No validation, cryptographic signing, or reactive invalidation occurs in the storage manager. |
| User Identity Verification | `crates/op-cozo-store/src/lib.rs:351` (`upsert_user`) | SSP (NIST SP 800-53: IA-2 Identification and Authentication) | Key-based cryptographic user identities (`wg_pubkey`) are managed without enforcement of key expiration, rotation, or binding policies aligned with machine-readable system plans. |

---

### 3. Detailed Recommendations

#### Recommendation 1: Consolidate CozoDB Relation Definitions into Declarative Protocol Buffer Schemas
* **File Reference**: `crates/op-cozo-store/src/lib.rs:62-156` (Relation seed schema)
* **Status**: Major Gap
* **Remediation**: 
  Instead of hardcoding raw Datalog relation creation strings inside the `seed_schema` function, define all structures (e.g., `ComplianceRule`, `SubidRegistry`, `AuditEvent`, `Session`, `MemoryEntry`) inside versioned Protocol Buffer files (e.g., `proto/op/cozo/v1/store.proto`). 
  Use a `build.rs` script using `prost_build` to compile these into strongly-typed Rust structures. Write a procedural macro or generator module that maps these generated types to Cozo relations. This guarantees backward compatibility and prevents schema drift between the storage tier and the surrounding components.

```protobuf
syntax = "proto3";
package op.cozo.v1;

message ComplianceRule {
  string plugin = 1;
  string op = 2;
  string action = 3;
  string reason = 4;
  string control_ref = 5;
  string created_at = 6;
}
```

#### Recommendation 2: Eliminate Untyped `serde_json::Value` from External-Facing APIs
* **File Reference**: `crates/op-cozo-store/src/lib.rs:166` (`run_query`), `crates/op-cozo-store/src/lib.rs:254` (`store_node`)
* **Status**: Major Gap
* **Remediation**:
  Replace generic JSON values with compiled structures or `prost_types::Any` representations. For structured property maps, avoid storing serialized string objects like `props: String default "{}"` in Cozo. Force inputs to pass through concrete, statically-analyzed validation schemas generated directly from the Protocol Buffer contracts.

#### Recommendation 3: Implement Automated OSCAL Rule Reference Validation
* **File Reference**: `crates/op-cozo-store/src/lib.rs:204` (`store_compliance_rule`), `crates/op-cozo-store/src/lib.rs:227` (`register_subid`)
* **Status**: Major Gap
* **Remediation**:
  Incorporate an OSCAL profile validation check in the initialization phase of `CozoGraphShuttle`. On startup, load the system security plan or component definition files using the `op-compliance` dependency (listed in `Cargo.toml`). Modify `register_subid` and `store_compliance_rule` to parse these configurations and reject rules that cite non-existent control IDs or invalid statement paths.

#### Recommendation 4: Enforce Session Expiration Chronology Statically
* **File Reference**: `crates/op-cozo-store/src/lib.rs:391` (`lookup_session`)
* **Status**: Major Gap
* **Remediation**:
  The database interface presents a high risk of authentication bypass if external callers fail to implement check logic for the returned RFC3339 string. Enforce expiration directly inside the store. Modify `lookup_session` to validate the `expires_at` field against the current UTC timestamp prior to yielding the session, automatically executing `delete_session` and returning `Ok(None)` if the expiration boundary has been breached.

```rust
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
        
        if !expires.is_empty() {
            if let Ok(expires_dt) = chrono::DateTime::parse_from_rfc3339(&expires) {
                if chrono::Utc::now() > expires_dt.with_timezone(&chrono::Utc) {
                    self.delete_session(session_id)?;
                    return Ok(None);
                }
            }
        }
        Ok(Some((wg, created, expires)))
    } else {
        Ok(None)
    }
}
```