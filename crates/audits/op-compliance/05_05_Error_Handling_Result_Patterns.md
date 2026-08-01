# Production Quality & Security Audit: Error Handling & Schema-As-Code

This audit focuses on error handling patterns, panic risks, and schema-as-code discipline within the `op-compliance` crate.

---

## 1. Error Handling Metrics

### Macro and Operator Counts

| Construct | Count | Comments |
| :--- | :--- | :--- |
| `.unwrap()` | 0 | None in production code. |
| `.unwrap_err()` | 2 | Used exclusively in test cases to assert error messages. |
| `.expect()` | 0 | None. |
| `.unwrap_or()` | 0 | None. |
| `?` operator | 7 | Utilized properly for control flow across the compliance pipeline. |
| `todo!()` | 0 | None. |
| `unimplemented!()` | 0 | None. |
| `panic!()` | 0 | None. |

---

## 2. Unwrap/Expect Site Analysis

Below are the only unwrap-related calls in the provided codebase. Both reside in the `#[cfg(test)]` block.

### Site 1
* **File & Line:** `crates/op-compliance/src/lib.rs:144`
* **Context:** `assert!(result.unwrap_err().to_string().contains("GDPR"));`
* **Lock Poisoning Risk:** None. No mutex or read-write locks are involved.
* **Recommendation (Result vs Panic):** In a test suite, utilizing `.unwrap_err()` is appropriate to explicitly verify that a pipeline fails when expected. No change is recommended here as panicking on a test assertion failure is standard practice.

### Site 2
* **File & Line:** `crates/op-compliance/src/lib.rs:158`
* **Context:** `assert!(result.unwrap_err().to_string().contains("AI Act"));`
* **Lock Poisoning Risk:** None.
* **Recommendation (Result vs Panic):** Standard test behavior. No change is recommended.

---

## 3. Schema-As-Code Violations

The codebase claims to validate plugin schemas against legal and security frameworks but relies heavily on ad-hoc, untyped parsing of JSON values. This violates strict schema-as-code discipline.

### Ad-hoc String & Struct Contract Parsing
* **File & Line:** `crates/op-compliance/src/lib.rs:16`
  * **Violation:** OliviaScal accesses raw metadata keys via unstructured path traversal: `schema.get("capabilities")`.
* **File & Line:** `crates/op-compliance/src/lib.rs:31`
  * **Violation:** EugeneRisk inspects untyped strings directly: `schema.get("plugin_type").and_then(|v| v.as_str()) == Some("custom")`.
* **File & Line:** `crates/op-compliance/src/lib.rs:51-57`
  * **Violation:** PennyPrivacy performs an extremely brittle, ad-hoc string-containment check on a serialized JSON structure to identify PII:
    ```rust
    let schema_str = s.to_string().to_lowercase();
    if (schema_str.contains("email") || schema_str.contains("user_id") || schema_str.contains("phone")) && !schema_str.contains("retention")
    ```
* **File & Line:** `crates/op-compliance/src/lib.rs:90`
  * **Violation:** The structural validation contract is read from a relative JSON path at runtime: `include_str!("../../../schemas/opdbus-plugin-schema.json")` and compiled on the fly rather than using code-generated versioned models.

### Recommendation
Replace ad-hoc `serde_json::Value` lookups with strongly typed, versioned models compiled from a single source of truth (e.g., Protocol Buffers or an OSCAL-derived schema rust-crate).
* Implement a `PluginSchema` struct using `prost` or `serde` annotations that directly defines field contracts (such as `requires_root` or `training_data_source`).
* Leverage validation-attribute derive macros (e.g., `validator` or custom structural validation traits) instead of performing manual string searches on serialized payloads.