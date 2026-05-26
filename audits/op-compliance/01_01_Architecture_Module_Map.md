# Architecture & Module Map

### Overview
The `op-compliance` crate (dubbed "The Law Firm") is designed to perform validation checks on dynamic plugin schemas (`PluginSchema`) to ensure compliance with security and regulatory frameworks, including Open Security Controls Assessment Language (OSCAL), EU AI Act, GDPR, and Open Policy Agent (OPA). 

### Module Tree
```
op-compliance (lib)
 ├── attorneys (mod)
 │    ├── OliviaScal (struct) — OSCAL validation
 │    ├── EugeneRisk (struct) — EU AI Act counsel
 │    ├── PennyPrivacy (struct) — GDPR validation
 │    └── ReggieOpa (struct) — OPA engine
 └── LawFirm (struct) — Central entry point
```

### Entry Points
* **`crates/op-compliance/src/lib.rs`**: Exposes the main interface struct `LawFirm` with its core validation method `review_schema`.

---

# Schema-as-Code Violations

The codebase attempts to enforce structured compliance policies, but relies heavily on ad-hoc string inspections and raw JSON key checks instead of typed, versioned, and contract-backed schemas (such as Protocol Buffers or strongly typed validation schemas). 

### 1. Ad-Hoc Regulatory Validation via Raw Substring Matching
* **Citation**: `crates/op-compliance/src/lib.rs:44-53`
* **Defect**: The GDPR validation engine (`PennyPrivacy::validate_privacy`) serializes the internal JSON representation of the schema to a lowercase string (`s.to_string().to_lowercase()`) and performs ad-hoc substring matches for sensitive fields (`"email"`, `"user_id"`, `"phone"`).
* **Impact**: Doing validation based on raw string lookups is fragile, error-prone, and easily bypassed. Structural contracts should be enforced via declarative, versioned schemas (e.g., OSCAL XML/JSON profiles, protobuf-validated metadata fields, or precise JSON Schema checks) rather than unstructured substring matching on serialized representations.

### 2. Loose Un-Versioned Control Enforcement for AI Risk
* **Citation**: `crates/op-compliance/src/lib.rs:24-31`
* **Defect**: The EU AI Act validation engine (`EugeneRisk::validate_ai_risk`) checks for model transparency constraints by manually parsing and matching arbitrary JSON keys (`"plugin_type"`, `"model_name"`, `"training_data_source"`).
* **Impact**: Changes in spelling, casing, or schema nesting in external plugins will cause silent compliance failures or false negatives. This policy logic should be bound to a formal Protocol Buffer schema or structured policy engine (e.g., actual OPA policies defined via Rego) instead of ad-hoc Rust-native conditional branches.

---

# Security & Quality Findings

## 1. Global Bypass of GDPR Retention Policy Enforcement [Critical]
* **Location**: `crates/op-compliance/src/lib.rs:44-55`
* **Description**: The GDPR compliance checker employs a flawed validation condition to ensure that any schema containing PII fields (such as `email`, `user_id`, or `phone`) also declares a retention policy:
  ```rust
  let schema_str = s.to_string().to_lowercase();
  if (schema_str.contains("email")
      || schema_str.contains("user_id")
      || schema_str.contains("phone"))
      && !schema_str.contains("retention")
  ```
* **Exploitability**: Because this check is evaluated globally over the entire serialized schema string `s`, **any** occurrence of the word `"retention"` in any metadata field or unrelated property description will globally mute GDPR violations for *all* actual PII fields in that block. 
  For example, a malicious or non-compliant plugin can completely bypass the retention requirement for sensitive keys like `user_id` and `email` by simply defining an arbitrary, unused field named `"retention_not_needed"`:
  ```json
  {
    "schema": {
      "user_email": "malicious-harvester@evil.com",
      "user_phone": "123-456-7890",
      "some_unrelated_field_retention": "suppress_gdpr_warning"
    }
  }
  ```
  Since `schema_str` now contains `"retention"`, `!schema_str.contains("retention")` evaluates to `false`, causing the error block to be skipped. This allows direct extraction of PII without mandatory retention policies.
* **Remediation**: Parse the schema into strongly typed structures, or use precise JSON Schema path checks to assert that for every individual node identifying as a PII type, a corresponding `retention` sibling node is strictly defined.

---

## 2. Denial of Service via Dynamic Schema Re-compilation [Medium]
* **Location**: `crates/op-compliance/src/lib.rs:77-83`
* **Description**: Within the hot validation path, `LawFirm::review_schema` dynamically compiles the JSON schema on every single function call:
  ```rust
  let meta_schema = include_str!("../../../schemas/opdbus-plugin-schema.json");
  let meta_v: Value = serde_json::from_str(meta_schema)?;

  let compiled = JSONSchema::compile(&meta_v).map_err(|e| anyhow!("Schema error: {}", e))?;
  ```
  Compiling a JSON schema via the `jsonschema` library is a highly resource-intensive operation involving parser generation, validation DAG construction, and memory allocation.
* **Exploitability**: If this validation endpoint is exposed to untrusted user input (such as validating custom plugin schemas registered by third parties or sent via DBus/network messages), an attacker can flood the engine with rapid validation requests to induce CPU starvation and cause a Denial of Service (DoS) of the control plane.
* **Remediation**: Use `lazy_static` or `std::sync::OnceLock` to compile the metadata schema exactly once during initialization:
  ```rust
  use std::sync::OnceLock;
  use jsonschema::JSONSchema;

  static COMPILED_SCHEMA: OnceLock<JSONSchema> = OnceLock::new();

  let compiled = COMPILED_SCHEMA.get_or_try_init(|| {
      let meta_schema = include_str!("../../../schemas/opdbus-plugin-schema.json");
      let meta_v: serde_json::Value = serde_json::from_str(meta_schema)?;
      JSONSchema::compile(&meta_v).map_err(|e| anyhow::anyhow!("Schema error: {}", e))
  })?;
  ```

---

## 3. Version Mismatch of critical `jsonschema` dependency [Low]
* **Location**: `crates/op-compliance/Cargo.toml:8` vs `Cargo.toml:42`
* **Description**: The subcrate `crates/op-compliance/Cargo.toml` overrides the workspace configuration by explicitly requesting an outdated version of the `jsonschema` crate:
  ```toml
  jsonschema = "0.18"
  ```
  Meanwhile, the main workspace definition in the root `Cargo.toml` requests version `"0.29"`:
  ```toml
  jsonschema = { version = "0.29", default-features = false }
  ```
* **Impact**: This mismatch forces Cargo to compile two completely separate versions of the `jsonschema` dependency tree. This increases compilation times, expands the binary footprint, and exposes the compliance engine to potential parser differentials or validation bugs that were patched between versions `0.18` and `0.29`.
* **Remediation**: Remove the explicit version definition from `crates/op-compliance/Cargo.toml` and inherit the dependency from the workspace configuration:
  ```toml
  jsonschema.workspace = true
  ```