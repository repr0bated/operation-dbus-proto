| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `format_json_manual` | `crates/op-workflows/src/context.rs:119` | Performs manual string search-and-replace using `${name}` patterns to populate variables dynamically. | Use schema-driven parsing and serialization (e.g., passing structured context objects directly into templating engines or AST evaluators). | Ad-hoc string templating bypasses type safety and risks malformed outputs if strings contain unescaped characters. | Major Gap |
| `format_json_manual` | `crates/op-workflows/src/engine.rs:143` | Concatenates unstructured error strings dynamically inside an ad-hoc workflow result struct. | Utilize structured diagnostic schemas or enum-defined error variants from Protobuf or OSCAL specs. | Dynamic unstructured text representation limits machine-readability of system errors. | Minor Gap |
| `unwrap_expect` | `crates/op-workflows/src/engine.rs:265` | Invokes `.unwrap()` to assert successful registration during setup in a test environment. | Standard practice in test contexts where panicking on setup failure is appropriate. | None (appropriate context). | Compliant |
| `format_json_manual` | `crates/op-workflows/src/flow.rs:207` | Flattens map keys manually using dynamic string concatenation with dot-notation (`node_id.port_id`). | Model data topology explicitly using structured schemas with defined fields for routing paths. | Bypasses structured schema boundaries by encoding relationship metadata into ad-hoc string keys. | Major Gap |
| `format_json_manual` | `crates/op-workflows/src/orchestrator.rs:136` | Generates combined names dynamically using prefix strings and unchecked hash substrings. | Use structured, unique ID generators or formal schemas to model naming constraints. | Ad-hoc generated strings can lead to unexpected namespace collisions. | Minor Gap |
| `format_json_manual` | `crates/op-workflows/src/workflows.rs:130` | Passes dynamic, unstructured error logs as failure state metadata in workflow state transitions. | Transition states with structured failure codes or serialized error schemas. | Violates Schema-as-Code consistency by mixing raw string errors with formal workflow states. | Major Gap |
| `unwrap_expect` | `crates/op-workflows/src/workflows.rs:393` | Uses `.unwrap()` to check standard workflow construction in test cases. | Standard practice in test contexts for assertions. | None (appropriate context). | Compliant |

---

### Actionable Recommendations for Major Gaps

#### 1. Implement Schema-Driven Variable Interpolation
* **Location:** `crates/op-workflows/src/context.rs:119`
* **Remedy:** Replace manual dynamic string formatting and replacement with a structured evaluation step. Instead of treating variables as arbitrary `${name}` string substitutions, define an explicit schema for context properties (e.g., using a compiled template engine like `handlebars` or structured JSON-pointer evaluation). Ensure that `Value` types are serialized and validated against an OSCAL or Protobuf data contract during substitution to guarantee typing invariant checks.

#### 2. Define Explicit Topology Structures Instead of String Keys
* **Location:** `crates/op-workflows/src/flow.rs:207`
* **Remedy:** Eliminate dot-separated dynamic keys (`node_id.port_id`). Replace the ad-hoc key structure with a formal schema definition representing structural outputs, such as a strongly typed data structure:
  ```rust
  #[derive(Serialize, Deserialize)]
  pub struct PortReference {
      pub node_id: String,
      pub port_id: String,
  }
  ```
  Map these structures directly to your Protocol Buffer boundaries to enforce contract safety at compilation time.

#### 3. Standardize State Transition Error Payloads
* **Location:** `crates/op-workflows/src/workflows.rs:130`
* **Remedy:** Migrate `McpWorkflowState::Failure` parameters from unstructured dynamic strings to structured error objects defined in the workflow system schema. Establish a schema containing machine-readable error codes (e.g., enum variants), affected system elements, and structured metadata rather than manually formatting string logs (`format!("Code review failed: {}", e)`).