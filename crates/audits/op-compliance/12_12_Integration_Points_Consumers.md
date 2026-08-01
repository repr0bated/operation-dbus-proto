# Workspace Dependency & Integration Analysis

### Crates Depending on `op-compliance`
Based on the workspace configuration and the dependency tree resolved in `Cargo.lock`, the following crate depends on `op-compliance`:
* **`op-identity`** (referenced under its package dependencies list in `Cargo.lock`).

### Registered D-Bus Service Names & Object Paths
No D-Bus service names or object paths are registered in the provided implementation of the `op-compliance` crate (`crates/op-compliance/src/lib.rs`). 

### Exposed HTTP/gRPC Endpoints
The `op-compliance` crate acts as a local validation library ("The Law Firm") and does not expose any HTTP or gRPC endpoints in its provided source files.

### Cross-Crate Circular Dependency Risk
* **Dependency Flow**: `op-identity` -> `op-compliance` -> `op-core`.
* **Risk Assessment**: `op-core` does not depend on `op-compliance` or `op-identity`. Since the dependencies flow downward cleanly and do not loop back, there is no circular dependency risk associated with the `op-compliance` integration.

---

# Schema-as-Code Architectural Violations

### Ad-Hoc Regulatory Policy Checks
* **Citation**: `crates/op-compliance/src/lib.rs:25-68`
* **Description**: Instead of representing compliance policies (OSCAL, EU AI Act, GDPR, OPA) as external, versioned declarative schemas, the validation contracts are implemented using hardcoded ad-hoc string manipulation, structural JSON lookups, and fragile heuristic substring searching.
* **Impact**: Policy rules cannot be updated, versioned, or audited independently of compiling the Rust code. Modifications to compliance regimes require code rewrites and software redeployments.

---

# Security & Quality Audit Findings

### Finding 1 (Medium): Fragile Substring PII Detection Leading to GDPR Bypass
* **Citation**: `crates/op-compliance/src/lib.rs:53-57`
* **Description**: The `PennyPrivacy::validate_privacy` check performs a simple case-insensitive substring search over the serialized JSON representation of a schema to detect PII fields:
  ```rust
  let schema_str = s.to_string().to_lowercase();
  if (schema_str.contains("email")
      || schema_str.contains("user_id")
      || schema_str.contains("phone"))
      && !schema_str.contains("retention")
  ```
* **Risk**:
  1. **Compliance Bypass**: Any schema containing the substring `"retention"` anywhere in its fields, types, or descriptions (e.g., a field named `retention_not_needed` or a description containing the word `"pretention"`) will bypass the retention policy check, allowing PII fields to escape compliance oversight.
  2. **Evasion**: An untrusted plugin can trivially bypass the PII filter by naming fields slightly differently (e.g., `e_mail`, `phone_number`, `ssn`, `tax_id`), which are not caught by the hardcoded lists.
* **Recommendation**: Enforce compliance metadata natively within the JSON schema (`opdbus-plugin-schema.json`), requiring explicit classifications of PII fields and non-optional retention periods.

---

### Finding 2 (Medium): Fragile EU AI Act Transparency Enforcement
* **Citation**: `crates/op-compliance/src/lib.rs:33-41`
* **Description**: `EugeneRisk::validate_ai_risk` only validates training data transparency when the `plugin_type` equals `"custom"`:
  ```rust
  if schema.get("plugin_type").and_then(|v| v.as_str()) == Some("custom") {
  ```
* **Risk**: If an AI/ML model plugin is deployed under another valid `plugin_type` (such as `"service"`, `"mcp"`, or `"cognitive"`), the transparency check is skipped entirely. This allows unvetted AI models into production, violating regulatory EU AI Act compliance protocols.
* **Recommendation**: Decouple model validation checks from the high-level `plugin_type` taxonomy. Any plugin schema containing a `model_name` property must be subjected to transparency checks regardless of its packaging model.

---

### Finding 3 (Low): Duplicate Dependency Resolution via `jsonschema` Version Mismatch
* **Citation**: `crates/op-compliance/Cargo.toml:9` vs `Cargo.toml:48`
* **Description**: The `op-compliance` crate requests version `"0.18"` of the `jsonschema` library, while the workspace root requests version `"0.29"`.
* **Impact**: This forces Cargo to compile two distinct versions of the `jsonschema` library (`0.18.3` and `0.29.1` in `Cargo.lock`), bloating compiled binary sizes and introducing potential type mismatch compilation failures if validation types are passed across crate boundaries.
* **Recommendation**: Update `crates/op-compliance/Cargo.toml` to inherit the workspace dependency:
  ```toml
  jsonschema.workspace = true
  ```

---

### Finding 4 (Low): Out-of-Crate Path Traversal inside `include_str!`
* **Citation**: `crates/op-compliance/src/lib.rs:89`
* **Description**: The `LawFirm::review_schema` loads its structural schema using a hardcoded relative path that escapes the crate directory:
  ```rust
  let meta_schema = include_str!("../../../schemas/opdbus-plugin-schema.json");
  ```
* **Impact**: Navigating multiple directory levels up (`../../../`) makes the crate non-portable and reliant on monorepo layout specifics. If the crate is isolated for sandboxed compilation, packaging, or registry publishing, the build will fail immediately.
* **Recommendation**: Place the required JSON schema directly inside the `op-compliance` crate workspace folder or package it via a custom build script.