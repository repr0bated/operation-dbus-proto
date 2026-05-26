# Production Security & Quality Audit Report

## 1. Dependencies & Feature Inventory

### Direct Dependencies: `crates/op-compliance/Cargo.toml`
* **anyhow ("1")**: Pulled in with default features. Used for ad-hoc error generation in compliance validation.
* **serde ("1")**: Explicitly enables the `["derive"]` feature. Used for structural serialization/deserialization.
* **serde_json ("1")**: Pulled in with default features. Used for parsing and inspecting raw untyped JSON schemas.
* **jsonschema ("0.18")**: Pulled in with default features. Used for structural validation of plugin schemas against `opdbus-plugin-schema.json`.
* **tracing ("0.1")**: Pulled in with default features. Used for logging compliance warnings.
* **op-core (path: "../op-core")**: Local workspace path dependency.

---

### Direct Dependencies: Root `Cargo.toml` (Crate: `op-dbus`)
* **anyhow**: Workspace-managed. Pulled in with default features.
* **serde**: Workspace-managed. Explicitly enables `["derive"]`.
* **simd-json**: Workspace-managed. Explicitly enables `["serde", "serde_impl"]`.
* **sha2**: Workspace-managed. Pulled in with default features.
* **uuid**: Workspace-managed. Features managed via workspace: `["v4", "serde"]`.
* **tokio**: Workspace-managed. Explicitly enables `["full"]` via workspace.
* **tokio-stream**: Workspace-managed. Pulled in with default features.
* **tracing**: Workspace-managed. Pulled in with default features.
* **tracing-subscriber**: Workspace-managed. Pulled in with default features.
* **thiserror**: Workspace-managed. Pulled in with default features.
* **async-trait**: Workspace-managed. Pulled in with default features.
* **zbus**: Workspace-managed. Explicitly enables `["tokio"]` via workspace.
* **sqlx**: Workspace-managed. Explicitly enables `["sqlite", "runtime-tokio", "json"]` via workspace.
* **chrono**: Workspace-managed. Explicitly enables `["serde"]` via workspace.
* **base64**: Workspace-managed. Pulled in with default features.
* **futures**: Workspace-managed. Pulled in with default features.
* **regex**: Workspace-managed. Pulled in with default features.
* **quick-xml**: Workspace-managed. Explicitly enables `["serialize"]` via workspace.
* **reqwest**: Workspace-managed. Explicitly enables `["json", "stream"]` via workspace.
* **serde_json**: Workspace-managed. Pulled in with default features.
* **jsonschema**: Workspace-managed. Explicitly disables default features (`default-features = false`).
* **parking_lot**: Direct dependency pinned to `"0.12"`.
* **dashmap**: Direct dependency pinned to `"5.0"`.
* **bytes**: Direct dependency pinned to `"1.0"`.
* **hex**: Direct dependency pinned to `"0.4"`.
* **pin-project-lite**: Direct dependency pinned to `"0.2"`.
* **glob**: Direct dependency pinned to `"0.3"`.
* **libc**: Direct dependency pinned to `"0.2"`.
* **axum**: Workspace-managed. Explicitly enables `["ws", "macros", "tokio"]`.
* **tower**: Workspace-managed. Pulled in with default features.
* **tower-http**: Workspace-managed. Explicitly enables `["cors", "fs", "trace", "compression-gzip"]`.
* **tonic**: Workspace-managed. Explicitly enables `["tls", "tls-roots", "tls-webpki-roots"]`.
* **tonic-reflection**: Workspace-managed. Pulled in with default features.
* **tonic-web**: Workspace-managed. Pulled in with default features.
* **rust-embed**: Direct dependency pinned to `"8.0"`.
* **mime_guess**: Direct dependency pinned to `"2.0"`.

---

### Crate Features: `op-dbus`
```toml
[features]
default = ["grpc"]
grpc = []
```
* **grpc**: Gated in Cargo.toml. Gates the compilation of gRPC transport-specific features, bridging, and Web/gRPC microservice setups.

---

### Schema-as-Code Analysis
The workspace defines dependencies on **`prost` ("0.13")**, **`prost-types` ("0.13")**, and **`tonic-build` ("0.12")** for Protocol Buffer compilation. However, **`op-compliance` has zero dependencies on schema-generation utilities** (such as `schemars` or `jsonschema`'s generation macros) and lacks any `prost` implementation. No serialized OSCAL schemas are used; instead, validation is processed entirely on unstructured text and ad-hoc structs.

---

## 2. Schema-As-Code Compliance Audit

This codebase purports to use a schema-as-code discipline using Protocol Buffers and OSCAL. However, the compliance engine directly violates this paradigm:

### Ad-Hoc Structs & Untyped Validation Contracts
* **`crates/op-compliance/src/lib.rs:11`**: `OliviaScal::validate_controls` parses raw JSON fields using untyped `serde_json::Value` lookups (`schema.get("capabilities")`). Rather than utilizing an OSCAL-compliant, strongly typed domain model or a serialized Protocol Buffer contract, capability verification is implemented through ad-hoc dynamic lookups.
* **`crates/op-compliance/src/lib.rs:24`**: `EugeneRisk::validate_ai_risk` performs untyped structural checks (`schema.get("plugin_type")`, `meta.get("model_name")`) on raw `serde_json::Value` structures. AI Act declarations should be mapped to generated Rust structs matching versioned Protocol Buffer models instead of relying on stringly typed metadata fields.
* **`crates/op-compliance/src/lib.rs:60`**: `ReggieOpa::validate_policy` checks for the presence of a policy version using `schema.get("version").is_none()`. This bypasses structured Open Policy Agent (OPA) integration or strongly typed schema validation in favor of a raw key check.

### Violation of Schema-as-Code with Raw String Serialization
* **`crates/op-compliance/src/lib.rs:45`**: `PennyPrivacy::validate_privacy` performs a `.to_string().to_lowercase()` conversion on a nested `serde_json::Value` block. It then executes brute-force substring matching (`contains`) to validate GDPR compliance. This entirely departs from versioned schemas and introduces severe logic vulnerabilities (see Section 4).

---

## 3. Storage Backend Matrix

Based on the parsed root and workspace configurations, the following storage engine dependencies and interfaces are identified:

| Backend | Found at File:Line (Cargo/Source) | Role (KV / Graph / Cache / Queue / Relational) | Architectural Violation & Conflict Notes |
| :--- | :--- | :--- | :--- |
| **CozoDB** | `Cargo.toml` (Workspace dependency) | Relational-Graph-Vector Engine | Datalog relational-graph storage engine configured with `storage-sled` to prevent sqlite3 runtime linking conflicts with SQLx. Used by `op-cognitive-mcp` and `op-cozo-store`. |
| **SQLx (SQLite)** | `Cargo.toml` (Workspace dependency) | Relational Storage Backend | Configured with `sqlite` and `runtime-tokio` features. Used directly by `op-dbus` and `op-services`. |
| **rusqlite** | `Cargo.toml` (Workspace dependency) | Embedded SQLite Backend | Explicitly pins `["bundled"]` features to prevent host library pollution. Used as an embedded database fallback. |
| **Redis** | `Cargo.toml` (Workspace dependency) | Key-Value / Distributed Cache | Utilized with `tokio-comp` async integration. Active in `op-state-store`. |

---

## 4. Security & Quality Findings

### [High] GDPR Validation Bypass and False Positives via Raw String Substring Scanning
* **File:Line**: `crates/op-compliance/src/lib.rs:42-57`
* **Vulnerability Type**: Security Logic Bypass / Input Validation Failure
* **Description**: The GDPR validation engine checks for the presence of personal identifying information (PII) by scanning a raw string dump of the schema for the substrings `"email"`, `"user_id"`, or `"phone"`. If found, it requires that the substring `"retention"` also be present:
  ```rust
  let schema_str = s.to_string().to_lowercase();
  if (schema_str.contains("email")
      || schema_str.contains("user_id")
      || schema_str.contains("phone"))
      && !schema_str.contains("retention")
  ```
* **Exploitation / Impact**:
  1. **Bypass**: An attacker or developer handling PII can easily bypass the GDPR retention policy enforcement by inserting the word `"retention"` in any harmless location of the schema document (such as a description, a field label, or a key like `"retention_policy_bypass": true`, or even inside an email value such as `"test@retention.com"`). If the string contains both `"email"` and `"retention"`, the check passes.
  2. **False Positives**: Safe, non-PII fields such as `"headphone_jack": "boolean"` or `"microphone_enabled": true` contain the substring `"phone"`. This causes the engine to throw a false-positive GDPR violation error unless the developer artificially injects the word `"retention"` somewhere else in the document.
* **Remediation**: Parse the JSON into a strongly typed struct using `serde` or validate the payload against a precise JSON schema using JSONPath/JSON Pointer queries on the deserialized keys, rather than scanning the raw string serialization.

---

### [High] EU AI Act Compliance Bypass via Service Type Masquerading
* **File:Line**: `crates/op-compliance/src/lib.rs:24-39`
* **Vulnerability Type**: Validation Logic Bypass
* **Description**: `validate_ai_risk` only enforces the training data source declaration if the plugin's `plugin_type` equals `"custom"`:
  ```rust
  if schema.get("plugin_type").and_then(|v| v.as_str()) == Some("custom") {
  ```
* **Exploitation / Impact**: A plugin that operates an AI model (`model_name` specified) can bypass the transparency declaration requirement of the EU AI Act completely by declaring its `plugin_type` as `"service"` (a valid plugin type verified in `test_valid_schema_passes` on line 95). This allows unregistered AI models with hidden training datasets to bypass compliance checks.
* **Remediation**: Evaluate the presence of `model_name` at the schema level irrespective of whether the wrapping envelope is labeled `"custom"` or `"service"`. If model parameters exist, enforce transparency policies globally.

---

### [Medium] Root Escalation Warning lacks Enforcement
* **File:Line**: `crates/op-compliance/src/lib.rs:11-19`
* **Vulnerability Type**: Compliance Verification Failure
* **Description**: When a plugin demands root execution permissions (`requires_root == true`), the OSCAL compliance checker only issues a non-blocking tracing warning:
  ```rust
  if caps.get("requires_root").and_then(|v| v.as_bool()) == Some(true) {
      tracing::warn!("Plugin requires root; OSCAL assessment recommended");
  }
  ```
* **Exploitation / Impact**: Plugins requiring root privilege are allowed to proceed through validation without any automated security control check, automated risk mapping, or cryptographic signature verification of the OSCAL metadata block. This fails to block non-compliant, unassessed privileged payloads.
* **Remediation**: Upgrade this check to enforce a hard rejection if `requires_root` is true, unless a verified, signed OSCAL assessment record is attached to the validation manifest.

---

### [Medium] Denial of Service via Uncontrolled Recursion on JSON Parsing
* **File:Line**: `crates/op-compliance/src/lib.rs:74`
* **Vulnerability Type**: Denial of Service (DoS) / Stack Overflow
* **Description**: `LawFirm::review_schema` processes untrusted input strings using `serde_json::from_str`. By default, `serde_json` does not place a shallow limit on recursion depth during deserialization.
* **Exploitation / Impact**: An attacker submitting a deeply nested array or object structure (e.g., thousands of open brackets `[[[[...]]]]`) can trigger stack exhaustion, leading to an immediate crash (Stack Overflow) of the control plane process.
* **Remediation**: Use a depth-limited JSON deserializer or configure a recursion limit on the deserializer stream before executing validation passes. Given that `simd-json` is in the workspace, consider passing the input to a validated parser.

---

### [Low] Static Schemas are re-parsed on Every Validation Loop
* **File:Line**: `crates/op-compliance/src/lib.rs:77-81`
* **Vulnerability Type**: Performance Inefficiency / Resource Exhaustion
* **Description**: The meta-schema is parsed from string and compiled on every invocation of `review_schema`:
  ```rust
  let meta_schema = include_str!("../../../schemas/opdbus-plugin-schema.json");
  let meta_v: Value = serde_json::from_str(meta_schema)?;
  let compiled = JSONSchema::compile(&meta_v).map_err(|e| anyhow!("Schema error: {}", e))?;
  ```
* **Impact**: Under continuous high-frequency plugin registration or dynamic MCP routing checks, compiling the schema repeatedly creates severe CPU overhead and excessive heap allocations.
* **Remediation**: Use a `once_cell::sync::Lazy` or `std::sync::OnceLock` block to compile the JSON Schema statically once at startup.