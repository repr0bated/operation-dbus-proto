# Production Security and Quality Audit: op-compliance

## 1. Crate License Audit

### Workspace License Extraction
* **Workspace License**: Defined as `Apache-2.0` in the root `Cargo.toml` at line 43.
* **Inherited Licenses**: The package `op-dbus` explicitly inherits the workspace license at `Cargo.toml:139` (`license.workspace = true`).

### Undefined Crate Licenses
* **Crate `op-compliance`**: The package manifest `crates/op-compliance/Cargo.toml` (lines 1–11) does not define a `license` field, nor does it inherit from the workspace via `license.workspace = true`. Its license remains completely undefined.

### Copyleft & Incompatibility Scan
* **GPL/AGPL/SSPL Scan**: A scan of `Cargo.lock` shows no GPL, AGPL, or SSPL copyleft crates. 
* **Weak Copyleft Note**: The workspace utilizes `cozo` version `0.7.6` (`Cargo.lock` package entry `cozo`), which is licensed under the Mozilla Public License 2.0 (MPL-2.0). MPL-2.0 is a weak copyleft license. While compatible with the primary workspace license (`Apache-2.0`), any modifications to the Cozo source code itself must be made available under the MPL-2.0.

---

## 2. Technical Findings & Security Analysis

### [Medium] Bypasseable GDPR Compliance Verification via Ad-Hoc String Matching
* **Reference**: `crates/op-compliance/src/lib.rs:42-56`
* **Mechanism**: The `PennyPrivacy` GDPR check attempts to detect PII fields without a corresponding retention policy by casting the JSON value to a lowercase string and checking for substrings:
  ```rust
  let schema_str = s.to_string().to_lowercase();
  if (schema_str.contains("email")
      || schema_str.contains("user_id")
      || schema_str.contains("phone"))
      && !schema_str.contains("retention")
  ```
* **Vulnerability / Quality Defect**:
  1. **False Negatives**: PII fields named with synonyms (e.g., `"e-mail"`, `"usr_id"`, `"telephone"`, `"contact_number"`) will completely bypass the validation.
  2. **Bypass via String Poisoning**: Any schema containing an email but completely lacking a retention policy can evade this check by including the word `"retention"` anywhere in its structure (e.g., in a description or field name like `"pretention"`, `"retention_not_applicable": true`).
* **Schema-as-Code Violation**: Rather than validating unstructured data using ad-hoc, error-prone string manipulation on untyped `serde_json::Value` buffers, the system must enforce compliance rules using structured, versioned schemas (such as Protocol Buffers or strongly-typed Rust structs generated from OSCAL JSON schemas).

---

### [Low] Fragile Ad-Hoc Enforcement of EU AI Act Compliance
* **Reference**: `crates/op-compliance/src/lib.rs:22-39`
* **Mechanism**: The `EugeneRisk` structural validator uses manual, nested `get()` calls on `serde_json::Value` to determine if a model has declared its training data source:
  ```rust
  if schema.get("plugin_type").and_then(|v| v.as_str()) == Some("custom") {
      if let Some(meta) = schema.get("schema") {
          if meta.get("model_name").is_some()
              && meta.get("training_data_source").is_none()
          {
              return Err(anyhow!(...));
          }
      }
  }
  ```
* **Vulnerability / Quality Defect**: The logic assumes specific naming structures (`"custom"`, `"schema"`, `"model_name"`, `"training_data_source"`). If the client schema uses different casing or structured wrappers, the compliance check fails to run but returns `Ok(())`, silently permitting non-compliant AI/ML models to pass unchecked.
* **Schema-as-Code Violation**: This is a direct consequence of treating data contracts as arbitrary JSON bags rather than codifying them into versioned Protocol Buffers or formal OSCAL control profiles.

---

### [Low] Non-Enforcing Policy Assertion for Privileged Capabilities
* **Reference**: `crates/op-compliance/src/lib.rs:11-19`
* **Mechanism**: In `OliviaScal::validate_controls`, if a plugin claims to require root privileges, the engine merely issues a warning log:
  ```rust
  if caps.get("requires_root").and_then(|v| v.as_bool()) == Some(true) {
      tracing::warn!("Plugin requires root; OSCAL assessment recommended");
  }
  ```
* **Vulnerability / Quality Defect**: If this check is intended to enforce legal/security boundaries, logging a warning rather than returning a policy violation error means privileged plugins can bypass deeper security reviews automatically in production pipelines.

---

## 3. Schema-As-Code Recommendations

To align the compliance engine with modern schema-as-code disciplines, the following refactoring must be applied:

1. **Eliminate Untyped Serde Value Parsing**: Replace `serde_json::Value` parameter types in `crates/op-compliance/src/lib.rs` with strongly-typed, auto-generated structures compiled from Protocol Buffers or versioned JSON schemas (OSCAL).
2. **Implement Decoupled Policy Evaluation**: Instead of writing imperative Rust checks (`contains("retention")`), implement validation policies using a unified declarative engine (e.g., Open Policy Agent (OPA) WebAssembly modules or Rego rules) validated against the compiled schema definitions.