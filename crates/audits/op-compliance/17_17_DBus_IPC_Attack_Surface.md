# D-Bus & IPC Attack Surface Audit

## 1. D-Bus & IPC Attack Surface Analysis

Based strictly on the provided source code in the `FILES` section:
* **Registered Interfaces, Methods, and Signals**: No D-Bus interfaces, methods, or signals are registered or implemented in the provided code. 
* **Connection Type**: The provided files do not contain connection logic to either the system bus or session bus.
* **Workspace Context**: The `Cargo.toml` file declares a dependency on `zbus = { version = "5.12", features = ["tokio"] }` and contains sub-crates such as `op-dbus-model` and `op-dbus-mirror`, indicating that D-Bus communication is utilized elsewhere in the wider workspace. However, since those crates are not provided in the `FILES` section, their specific interfaces, method permissions, caller identity checks, and policy rules cannot be verified and are excluded from this audit to prevent speculation.

---

## 2. Compliance & Schema-as-Code Violations

The codebase claims to validate plugin schemas against legal and security frameworks (such as OSCAL, EU AI Act, GDPR, and OPA). However, the implementation does not adhere to the "schema-as-code" discipline of using strongly-typed, versioned schemas (e.g., Protocol Buffers or strictly defined OSCAL structs). Instead, it relies on ad-hoc runtime JSON indexing and fragile string-based substring matching.

### Finding 1: Extremely Brittle Substring Matching for GDPR Compliance
* **Severity**: High (Security Design Defect / Quality Violation)
* **Citation**: `crates/op-compliance/src/lib.rs:43-60`
* **Description**: The `PennyPrivacy::validate_privacy` function checks for PII handling without a retention policy by serializing the schema to a raw JSON string, converting it to lowercase, and performing substring matching:
  ```rust
  let schema_str = s.to_string().to_lowercase();
  if (schema_str.contains("email")
      || schema_str.contains("user_id")
      || schema_str.contains("phone"))
      && !schema_str.contains("retention")
  ```
* **Impact**: 
  1. **False Positives**: Any field or model name containing the letters "email", "user_id", or "phone" (even if not representing PII) will trigger a failure unless the string "retention" is present somewhere in the entire JSON payload (even if unrelated to the field).
  2. **Evasion (False Negatives)**: An attacker or developer can easily bypass this check by obfuscating field names or using synonyms (e.g., `e_address`, `usr_key`, `telephone_num`). Alternatively, simply adding a useless field or description with the word `"retention"` anywhere in the JSON satisfies the condition, completely evading GDPR control validation.
* **Remediation**: Avoid ad-hoc serialization and string-contains checks. Define a typed, versioned schema (such as a Protobuf schema or structured OSCAL model) where PII sensitivity and retention policies are declared as explicit, typed metadata fields.

### Finding 2: Untyped, Ad-Hoc JSON Traversal for EU AI Act Verification
* **Severity**: Medium (Design Defect)
* **Citation**: `crates/op-compliance/src/lib.rs:24-38`
* **Description**: The `EugeneRisk::validate_ai_risk` function parses and traverses untyped `serde_json::Value` objects dynamically:
  ```rust
  if schema.get("plugin_type").and_then(|v| v.as_str()) == Some("custom") {
      if let Some(meta) = schema.get("schema") {
          if meta.get("model_name").is_some()
              && meta.get("training_data_source").is_none()
  ```
* **Impact**: This dynamic indexing bypasses Rust's compile-time type safety. If the layout of the schema changes in future versions (e.g., if `model_name` is moved to a nested object or renamed to `model`), the compliance check will silently fail or skip validation without a compiler error.
* **Remediation**: Deserialize the incoming payload directly into a versioned Rust struct generated from a schema, allowing compile-time enforcement of fields.

### Finding 3: Lack of Compile-Time Schema Compilation
* **Severity**: Medium (Performance & Quality Defect)
* **Citation**: `crates/op-compliance/src/lib.rs:81-83`
* **Description**: Within `LawFirm::review_schema`, the JSON meta-schema is parsed and compiled on every invocation:
  ```rust
  let meta_schema = include_str!("../../../schemas/opdbus-plugin-schema.json");
  let meta_v: Value = serde_json::from_str(meta_schema)?;

  let compiled = JSONSchema::compile(&meta_v).map_err(|e| anyhow!("Schema error: {}", e))?;
  ```
* **Impact**: Parsing and compiling JSON schemas repeatedly at runtime under load is computationally expensive. It also risks runtime failures if there are any hidden structural syntax errors in the embedded schema document.
* **Remediation**: Use `lazy_static` or `once_cell::sync::Lazy` to compile the schema exactly once at startup, or leverage code generation to turn the JSON schema into static Rust verification code.