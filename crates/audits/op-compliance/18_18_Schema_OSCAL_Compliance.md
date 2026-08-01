# OP-Compliance Security and Quality Audit

## 1. Schema-as-Code Analysis

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `LawFirm::review_schema` / `serde_json::Value` | Input Validation | `crates/op-compliance/src/lib.rs:80` | No | Validates untyped JSON strings using raw `serde_json::Value` and ad-hoc traversal instead of a strongly-typed, versioned Protobuf schema. |
| `OliviaScal::validate_controls` | Control Policy Verification | `crates/op-compliance/src/lib.rs:11` | No | Ad-hoc field inspection on untyped JSON without compile-time contract definitions. |
| `EugeneRisk::validate_ai_risk` | Risk Category Auditing | `crates/op-compliance/src/lib.rs:23` | No | Hardcoded conditional checks on model properties using loose string lookups on dynamic JSON maps. |
| `PennyPrivacy::validate_privacy` | GDPR Control Mapping | `crates/op-compliance/src/lib.rs:42` | No | Fragile substring analysis on a flattened JSON string rather than schema-enforced attributes. |
| `ReggieOpa::validate_policy` | OPA Policy Checking | `crates/op-compliance/src/lib.rs:61` | No | Fallible property existence validation using untyped JSON checks instead of compiled schema validation. |

---

## 2. OSCAL Coverage Analysis

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| Privileged Access Control (NIST AC-6 Least Privilege) | `crates/op-compliance/src/lib.rs:13` | None | Warnings are printed via `tracing::warn!` but do not generate OSCAL assessment-results or reference a System Security Plan (SSP) component definition. |
| Algorithmic Risk Transparency (EU AI Act Controls) | `crates/op-compliance/src/lib.rs:25` | None | Strict failures are returned but are completely disconnected from a machine-readable compliance profile or component-definition artifact. |
| PII and Information Retention (NIST MP-6 / GDPR) | `crates/op-compliance/src/lib.rs:44` | None | Evaluates fields programmatically, leaving privacy controls absent from machine-readable OSCAL data protection definitions. |
| Configuration Baseline and Tracking (NIST CM-2 / CM-8) | `crates/op-compliance/src/lib.rs:64` | None | Checks for schema versions programmatically without updating or validating against an OSCAL component-definition registry. |

---

## 3. Major Findings and Security Vulnerabilities

### Finding 1 [MAJOR]: Fragile Substring Matching Bypasses GDPR Enforcement
* **File/Line**: `crates/op-compliance/src/lib.rs:46-52`
* **Impact**: Compliance Evasion / Data Privacy Leak
* **Description**:
  The `PennyPrivacy` GDPR check attempts to validate the presence of a "retention" policy whenever PII fields (like `email`, `user_id`, or `phone`) are detected:
  ```rust
  let schema_str = s.to_string().to_lowercase();
  if (schema_str.contains("email")
      || schema_str.contains("user_id")
      || schema_str.contains("phone"))
      && !schema_str.contains("retention")
  ```
  This is implemented using a naive substring check on the serialized representation of the JSON schema. This is highly bypassable in two ways:
  1. **Compliance Evasion (False Negatives)**: A plugin developer can collect PII fields by renaming keys or utilizing synonyms (e.g. `e_mail`, `usr_id`, `cell_number`, `contact_phone`) which bypasses the validation entirely while still processing raw PII.
  2. **Bypass of Protection Requirement (False Positives)**: If any field, metadata, description, or comment in the JSON structure contains the word `"retention"` (for example, a field description containing `"retention is not configured"`), `!schema_str.contains("retention")` evaluates to `false`. This completely satisfies the validation logic and lets the plugin pass GDPR validation even though no programmatically enforceable retention policy exists.
* **Remediation**:
  Define a strongly-typed schema model using versioned structs or Protocol Buffers where PII data classification and retention periods are explicit, first-class fields (e.g., using `protovalidate` annotations) rather than performing loose substring searches on raw JSON data.

### Finding 2 [MAJOR]: Hardcoded Compliance Decisions Lacking Machine-Readable Policy Enforcement
* **File/Line**: `crates/op-compliance/src/lib.rs:25-33`
* **Impact**: Maintainability & Configuration Rigidity
* **Description**:
  Policy decisions—specifically AI Act transparency requirements (mapping `model_name` and `training_data_source`) and version checks—are hardcoded directly into the Rust code. Any change in EU AI Act requirements, GDPR interpretations, or enterprise security baseline requires modifying, recompiling, and redeploying the compliance binary. This defeats the purpose of an engine like Open Policy Agent (OPA) or OSCAL, which decouples policy definitions from system binaries.
* **Remediation**:
  Leverage WebAssembly-compiled Rego policies via an OPA runner or ingest external OSCAL profiles at runtime to evaluate plugin compliance dynamically.

---

## 4. Recommendations

### 1. Shift from Untyped `serde_json::Value` to Structured Protocol Buffers
Replace raw JSON metadata checks with a versioned Protocol Buffer schema definition (e.g., `plugin_schema.proto`).
```protobuf
syntax = "proto3";
package op.compliance.v1;

message PluginSchema {
  string name = 1;
  string version = 2;
  string plugin_type = 3;
  Capabilities capabilities = 4;
  DataSchema schema = 5;
}

message Capabilities {
  bool requires_root = 1;
  bool can_read = 2;
}

message DataSchema {
  string model_name = 1;
  string training_data_source = 2;
  repeated PiiField pii_fields = 3;
  RetentionPolicy retention_policy = 4;
}

message PiiField {
  string field_name = 1;
  string classification_type = 2; // e.g., email, phone, user_id
}

message RetentionPolicy {
  int64 retain_duration_seconds = 1;
}
```
Compile this with `tonic-build` or `prost-build` in the `Cargo.toml` workspace environment, and implement strict, type-safe matching on the generated Rust structures.

### 2. Standardize Compliance Output with OSCAL Component Definitions
Rather than utilizing basic tracing warnings (`tracing::warn!("Plugin requires root; OSCAL assessment recommended")` at `crates/op-compliance/src/lib.rs:16`), output standard OSCAL Assessment Results (`assessment-results` JSON/YAML documents) containing the exact control implementation mappings (e.g. NIST SP 800-53 AC-6, MP-6). This ensures downstream SIEM and auditing systems can automatically parse compliance violations.