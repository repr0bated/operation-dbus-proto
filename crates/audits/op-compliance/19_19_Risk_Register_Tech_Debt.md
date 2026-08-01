| Severity | Issue | Evidence (file:line) | Recommendation |
| :--- | :--- | :--- | :--- |
| **High** | **Denial of Service (DoS) via Dynamic Meta-Schema Compilation** | `crates/op-compliance/src/lib.rs:81-86` | Parse and compile the JSON schema exactly once at startup. Use `std::sync::OnceLock` or `lazy_static` to store the compiled `JSONSchema` instead of re-parsing and re-compiling on every invocation. |
| **High** | **Bypassable GDPR PII Detection via Ad-Hoc Serialized String Search** | `crates/op-compliance/src/lib.rs:44-53` | Avoid serialized string searches. Transition to a schema-as-code approach where data contracts are defined in versioned Protobuf or structured schemas, allowing precise programmatic inspection of field annotations and metadata. |
| **High** | **Failure of OSCAL Compliance Integration (No-op Warnings)** | `crates/op-compliance/src/lib.rs:14-16` | Replace logging-only warnings with structured enforcement. Validate incoming schemas against machine-readable, versioned OSCAL XML/JSON compliance controls (e.g. NIST SP 800-53 profiles) and return concrete errors. |
| **High** | **Ad-Hoc untyped EU AI Act Validation Lacking Schema-as-Code Discipline** | `crates/op-compliance/src/lib.rs:25-33` | Enforce model transparency policies using strongly typed, versioned data contracts. Parse schemas into concrete Rust structs or Protocol Buffer messages rather than utilizing untyped `serde_json::Value` lookups. |
| **Medium** | **Mismatched Crate Dependencies (Duplicated Dependency Graph)** | `crates/op-compliance/Cargo.toml:8`<br>`Cargo.toml:29` | Align dependency versions by utilizing workspace inheritance. Change the local manifest dependency definition to `jsonschema.workspace = true` to resolve duplicate dependencies and bloat. |
| **Medium** | **Ad-hoc Key Presence Checks Mimicking OPA Policy Execution** | `crates/op-compliance/src/lib.rs:65-68` | Implement a genuine OPA Rego engine runtime to dynamically run policy documents, or map incoming schemas to structured data contracts that enforce policies declaratively. |

---

### Detailed Findings & Technical Context

#### 1. Denial of Service (DoS) via Dynamic Meta-Schema Compilation
* **File/Line Reference**: `crates/op-compliance/src/lib.rs:81-86`
* **Impact**: Compiling a `JSONSchema` using the `jsonschema` crate is a CPU-intensive operation involving AST generation and constraint parsing. Because `LawFirm::review_schema` compiles the validator on *every* call, submitting numerous large or nested schemas will rapidly exhaust control-plane CPU resources.
* **Exploit Vector**: An attacker submitting multiple plugin schemas for registration can trigger server-wide CPU starvation, causing the control plane to hang.

#### 2. Bypassable GDPR PII Detection via Ad-Hoc Serialized String Search
* **File/Line Reference**: `crates/op-compliance/src/lib.rs:44-53`
* **Impact**: The GDPR compliance checker converts the JSON schema to a flat string and performs basic `.contains()` searches for `"email"`, `"user_id"`, and `"phone"`.
* **Exploit/Bypass Vector**:
  1. **Bypass Check**: If a plugin defines a `"user_id"` field but includes the word `"retention"` in any comment, key name, or description (e.g., `"description": "retention is handled elsewhere"`), the condition `!schema_str.contains("retention")` evaluates to `false`. The engine will allow the plugin to bypass compliance without possessing a structured retention policy.
  2. **Evasion**: An attacker can use synonymous terms (e.g. `usr_id`, `msisdn`, `address`, `social_security_number`) to register PII-extracting plugins, evading the naive substring check.

#### 3. Failure of OSCAL Compliance Integration (No-op Warnings)
* **File/Line Reference**: `crates/op-compliance/src/lib.rs:14-16`
* **Impact**: The managing partner `OliviaScal` is structurally named after OSCAL but contains zero automated compliance mappings. It issues a non-blocking warn-level trace when a plugin requests root access. This completely bypasses schema-as-code validation frameworks and provides no cryptographic or structured assurance.

#### 4. Ad-Hoc untyped EU AI Act Validation Lacking Schema-as-Code Discipline
* **File/Line Reference**: `crates/op-compliance/src/lib.rs:25-33`
* **Impact**: Navigating untyped `serde_json::Value` structures using ad-hoc string indexing is highly fragile. Any subtle schema variation (e.g. naming the model `llm_model` instead of `model_name`) will evade the validation rules, allowing the deployment of non-compliant models with unknown training sources into production.

#### 5. Mismatched Crate Dependencies (Duplicated Dependency Graph)
* **File/Line Reference**: `crates/op-compliance/Cargo.toml:8`, `Cargo.toml:29`
* **Impact**: `op-compliance` targets `jsonschema = "0.18"`, but the main workspace registers `jsonschema = "0.29"`. This mismatch forces Cargo to build both versions, resulting in code duplication, larger binaries, longer compilation times, and type incompatibility if `jsonschema` entities are passed across boundaries.

#### 6. Ad-hoc Key Presence Checks Mimicking OPA Policy Execution
* **File/Line Reference**: `crates/op-compliance/src/lib.rs:65-68`
* **Impact**: `ReggieOpa` executes a naive check for the presence of the `"version"` string. It lacks integration with an actual Open Policy Agent runtime or Rego interpreter. This architecture presents a false assurance of robust policy-based authorization.