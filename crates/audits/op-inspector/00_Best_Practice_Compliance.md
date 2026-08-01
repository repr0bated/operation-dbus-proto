| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `command_new` | `crates/op-inspector/src/cli.rs:172` | Invokes dynamic subprocess with unvalidated executable name `self.program`. | Use fully resolved absolute paths and strict validation of target programs. | Lack of program path validation risks executing arbitrary binaries if inputs are tainted. | Minor Gap |
| `command_new` | `crates/op-inspector/src/cli.rs:184` | Runs fallback command on unvalidated binary executable name. | Use explicit verification of the binary location. | Lack of program resolution constraints allows execution hijacking if target binary is spoofed. | Minor Gap |
| `command_new` | `crates/op-inspector/src/cli.rs:217` | Generates a subprocess command array from split segments. | Avoid splitting commands dynamically from unvalidated vectors; use exact predefined argument lists. | Potential argument injection if dynamic paths are split unsafely. | Minor Gap |
| `format_json_manual` | `crates/op-inspector/src/cli.rs:176` | Construct error and context messages using ad-hoc formatted strings. | Use structured errors and centralized context formatting. | Direct string formatting reduces machine readability of errors. | Minor Gap |
| `format_json_manual` | `crates/op-inspector/src/cli.rs:188` | Construct dynamic error context using `format!`. | Use structured logging/errors. | Use of ad-hoc formatting instead of structured error mapping. | Minor Gap |
| `format_json_manual` | `crates/op-inspector/src/cli.rs:309` | Formats command string using standard spaces. | Use structured command builders or versioned representation. | Ad-hoc string generation of executed command lines can hide sensitive values. | Minor Gap |
| `unwrap_expect` | `crates/op-inspector/src/cli.rs:429` | Re-compiles Regex on demand using runtime `unwrap()`. | Compile regexes exactly once at initialization using thread-safe lazy evaluation (`OnceLock` / `OnceCell`). | Compiling regexes at runtime wastes cycles; panicking on failure can crash the tool. | Major Gap |
| `unwrap_expect` | `crates/op-inspector/src/cli.rs:431` | Compiles a fallback Regex using runtime `.unwrap()`. | Pre-compile patterns globally with safe initialization. | Redundant pattern compilation and unsafe panic risk on startup syntax errors. | Major Gap |
| `unwrap_expect` | `crates/op-inspector/src/cli.rs:456` | Extracts regex capture groups with direct `.unwrap()` calls. | Use structured matching or safe error propagation. | Unchecked matching assumes target CLI outputs will never change structure, risking panic. | Major Gap |
| `unwrap_expect` | `crates/op-inspector/src/cli.rs:457` | Directly calls `.unwrap()` on capture group matching. | Handle unexpected output profiles gracefully without crashing. | Potential panic if the target CLI output schema changes. | Major Gap |
| `unwrap_expect` | `crates/op-inspector/src/cli.rs:460` | Extracts from optional match captures using `.unwrap()`. | Safely extract optional fields with pattern matching. | Risk of runtime panic during output parsing. | Major Gap |
| `simd_json_from_str` | `crates/op-inspector/src/datadump.rs:165` | Parses dynamic output to an ad-hoc `simd_json::Value` object. | Define versioned structures via schemas (e.g., Protocol Buffers, OSCAL models) for all data contracts. | **Schema-as-Code Violation**: Expresses target interfaces as ad-hoc unstructured JSON. | Major Gap |
| `command_new` | `crates/op-inspector/src/datadump.rs:133` | Constructs shell sub-processes from array slices dynamically. | Enforce absolute binary paths and secure arguments. | Dynamic execution of executable arguments split from space-delimited text. | Minor Gap |
| `format_json_manual` | `crates/op-inspector/src/datadump.rs:93` | Formats command paths as dot-separated string patterns. | Use strongly-typed schema structures. | Dynamic ad-hoc string formatting for identifier creation. | Minor Gap |
| `format_json_manual` | `crates/op-inspector/src/datadump.rs:166` | Formats current timestamp with Utc and string concatenation. | Use typed event-driven logging models. | Ad-hoc serialization of metadata timestamps alongside payloads. | Minor Gap |
| `command_new` | `crates/op-inspector/src/gcloud.rs:133` | Direct execution of environmental `gcloud` executable. | Validate PATH environment and locate the binary explicitly. | Relies on external environmental binaries being safe on the system path. | Minor Gap |
| `unsafe_block` | `crates/op-inspector/src/introspective_gadget.rs:192` | Wraps `simd_json::from_str` parsing in an undocumented `unsafe` block. | Provide a strict `// SAFETY:` document explaining structural invariants. | Undocumented `unsafe` constructs reduce safety auditability. | Major Gap |
| `unsafe_block` | `crates/op-inspector/src/introspective_gadget.rs:868` | Parses dynamic byte payload inside an undocumented `unsafe` block. | Accompany every `unsafe` operation with documented invariant justifications. | Lack of safety comments during mutation and raw string mapping. | Major Gap |
| `unsafe_block` | `crates/op-inspector/src/introspective_gadget.rs:1000` | Instantiates raw `unsafe` parser for CLI output parsing. | Document validation criteria for the underlying memory mutations. | Missing safety context on dynamic terminal stream buffer conversions. | Major Gap |
| `simd_json_from_str` | `crates/op-inspector/src/introspective_gadget.rs:192` | Parses parsed Docker inspect JSON directly into a raw `Value`. | Enforce versioned, strongly-typed contracts using Protocol Buffers or standardized models. | **Schema-as-Code Violation**: Dynamic ad-hoc target structures bypass schema-as-code discipline. | Major Gap |
| `simd_json_from_str` | `crates/op-inspector/src/introspective_gadget.rs:868` | Runs dynamic schema generation over raw JSON strings. | Use a declarative model schema like OpenAPI or OSCAL. | **Schema-as-Code Violation**: Unversioned dynamic object creation instead of compiled definitions. | Major Gap |
| `simd_json_from_str` | `crates/op-inspector/src/introspective_gadget.rs:1000` | Deserializes JSON arrays into generic `ParsedObject` maps. | Compile schema structures directly from defined IDLs. | **Schema-as-Code Violation**: Ad-hoc JSON parsing bypasses declarative system interface definition. | Major Gap |
| `unwrap_on_lock` | `crates/op-inspector/src/introspective_gadget.rs:102` | Obtains rwlock and uses `.unwrap()` to handle poisoning. | Gracefully handle lock poisoning, or map locks with clean error chains. | Crash propagation if a thread panics while holding the parser lock. | Minor Gap |
| `unwrap_on_lock` | `crates/op-inspector/src/introspective_gadget.rs:112` | Unwraps poisoned read-lock on dynamic fallback parsing. | Safely manage synchronization locks. | Unhandled lock state could panics when reading fallback parsers. | Minor Gap |

---

### Recommendations

#### 1. Enforce Schema-as-Code Discipline
* **Gap**: The codebase routinely utilizes raw, unstructured `simd_json::Value` structures (`crates/op-inspector/src/datadump.rs:165`, `crates/op-inspector/src/introspective_gadget.rs:192`, `868`, `1000`) instead of compiled schemas.
* **Actionable Remediation**:
  * Define strict, versioned data contracts using Protocol Buffers (`.proto` files) or OSCAL profiles.
  * Use codegen libraries (such as `prost` or `prost-build`) to generate Rust types representing the expected output structures of docker inspections, command configurations, and gadget inputs.
  * Deserialize CLI output directly into these compiled, schema-validated structures rather than arbitrary `simd_json::Value` targets.

#### 2. Implement Lazy Pre-Compilation of Regular Expressions
* **Gap**: Regex compilation occurs dynamically at runtime using `.unwrap()` (`crates/op-inspector/src/cli.rs:429`, `431`).
* **Actionable Remediation**:
  * Migrate the regex patterns to global static initializers via the standard library `std::sync::OnceLock`.
  * Example:
    ```rust
    use std::sync::OnceLock;
    use regex::Regex;

    fn get_cmd_name_re() -> &'static Regex {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new(r"^\s{2,8}(\w[\w-]*)\s{2,}(.*)$").expect("Valid CLI pattern"))
    }
    ```

#### 3. Secure Dynamic Regex Captures and CLI Parsers
* **Gap**: Direct use of `.unwrap()` on captured match sequences (`crates/op-inspector/src/cli.rs:456`, `457`, `460`) can lead to dynamic application failure when third-party CLI text formatting changes.
* **Actionable Remediation**:
  * Replace the `.unwrap()` statements with pattern matching or explicit map logic:
    ```rust
    if let Some(caps) = cmd_name_re.captures(line) {
        let name = caps.get(1).map(|m| m.as_str().to_string()).ok_or_else(|| anyhow!("Missing name"))?;
        let desc = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        results.push((name, desc));
    }
    ```

#### 4. Audit and Document Safety Invariants for Unsafe Blocks
* **Gap**: Undocumented `unsafe` statements wrapped around raw `simd_json::from_str` invocations (`crates/op-inspector/src/introspective_gadget.rs:192`, `868`, `1000`).
* **Actionable Remediation**:
  * For every `unsafe` block, write a precise `// SAFETY:` block explaining why the invariants of `simd_json::from_str` are satisfied (e.g., confirming the input `&mut str` remains allocated and is valid UTF-8 for the lifetime of the parsed struct, and that the buffer modification behavior is safe under the current execution model).
  * If absolute maximum performance is not critical, migrate parsing workloads to safe parsers like `serde_json::from_str` to eliminate raw memory mutation risks entirely.