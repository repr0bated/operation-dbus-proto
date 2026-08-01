| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `command_new` | `crates/op-tools/src/builtin_old.rs:194` | Spawns a shell (`sh -c`) using formatted/joined user strings. | Invoke binaries directly with structures arguments (`Command::new("bin").arg("arg")`). | **Arbitrary Shell Injection:** Concatenating user arguments directly into a shell context allows arbitrary command execution. | **Critical Gap** |
| `format_json_manual` | `crates/op-tools/src/builtin_old.rs:196` | Uses `format!` to interpolate arguments into a single shell command string. | Pass individual elements as safe elements in an argument vector (`std::process::Command::arg`). | Avoids shell string tokenization entirely; prevents arguments from breaking out of context. | **Critical Gap** |
| `format_json_manual` | `crates/op-tools/src/builtin_old.rs:217` | Formats an ad-hoc error response dynamically as a string: `format!("Command failed: {}", stderr)`. | Define and version all data contracts and error formats as Protocol Buffers or standardized schemas. | **Schema-as-Code Violation:** Ad-hoc error serialization ignores the system schema, leading to raw text parser dependencies. | **Major Gap** |
| `format_json_manual` | `crates/op-tools/src/builtin_old.rs:225` | Formats error message with `format!("Failed to execute command: {}", e)`. | Use structured protobuf messages or centralized versioned error schema structures. | **Schema-as-Code Violation:** Uses unstructured error messages instead of typed error structures. | **Major Gap** |
| `format_json_manual` | `crates/op-tools/src/builtin_old.rs:324` | Generates dynamic error messages directly: `format!("Failed to read file: {}", e)`. | Serialize response contracts via formal typed schemas (such as schema-registry or proto definitions). | **Schema-as-Code Violation:** Relies on unstructured runtime-formatted string outputs. | **Major Gap** |
| `std_fs_in_async` | `crates/op-tools/src/builtin_old.rs:285` | Uses async non-blocking execution: `tokio::fs::read(path).await`. | Perform file I/O asynchronously to avoid blocking the Tokio executor thread pool. | None (compliant with async I/O best practices). | Compliant |
| `format_json_manual` | `crates/op-tools/src/dynamic_tool.rs:49` | Implements ad-hoc custom casing code: `format!("_{}", c.to_lowercase())`. | Rely on robust casing libraries (`heck` crate) or formal string serialization schemes. | Custom manual casing code is fragile compared to structured external utilities. | Minor Gap |
| `unsafe_block` | `crates/op-tools/src/mcptools.rs:205` | Performs parsing within an `unsafe` block using `simd_json::from_str`. | Minimize unsafe blocks, documenting soundness invariants clearly when unsafe is required. | Missing safety documentation/guarantees for `simd_json` mutable lifetime requirements. | Minor Gap |
| `unsafe_block` | `crates/op-tools/src/mcptools.rs:214` | Parses single server configurations using `unsafe { simd_json::from_str }`. | Use safe deserialization methods or write explicit safety assertions. | Unsafe code is used without justifying performance improvements or proving bounds. | Minor Gap |
| `unsafe_block` | `crates/op-tools/src/mcptools.rs:225` | Deserializes config file content in-place with `unsafe { simd_json::from_str }`. | Leverage safe `serde_json` for file parsing unless strict micro-optimizations are required. | Unsafe deserialization of external files can pose memory safety risks if inputs are malicious. | Minor Gap |
| `unsafe_block` | `crates/op-tools/src/mcptools.rs:277` | Deserializes standard output using `unsafe { simd_json::from_str }`. | Use safe JSON parsers for process output boundaries. | Risk of memory corruption if the subprocess output is corrupted or malformed. | Minor Gap |
| `unsafe_block` | `crates/op-tools/src/mcptools.rs:341` | Parses stdout using `unsafe` execution. | Ensure memory safety through standard, safe parsers. | Same as above. | Minor Gap |
| `simd_json_from_str` | `crates/op-tools/src/mcptools.rs:205` | Mutates raw strings to parse JSON configuration elements. | Implement robust safe schemas (serde) to safely deserialize config payloads. | Relies on raw unchecked JSON formats without schema-level checks. | Minor Gap |
| `simd_json_from_str` | `crates/op-tools/src/mcptools.rs:214` | Uses mutable JSON parsing over raw buffers. | Perform structural validation on configuration structures. | Raw string manipulation. | Minor Gap |
| `simd_json_from_str` | `crates/op-tools/src/mcptools.rs:225` | Parses environmental files via raw SIMD-JSON interfaces. | Implement versioned schemas for environment parameters. | Lack of explicit schema definition layer for environment components. | Minor Gap |
| `simd_json_from_str` | `crates/op-tools/src/mcptools.rs:277` | Reads tool output through direct `simd_json` transformations. | Apply formalized schema-driven contract processing. | Raw output decoding. | Minor Gap |
| `simd_json_from_str` | `crates/op-tools/src/mcptools.rs:341` | Raw payload extraction via unsafe JSON parsing. | Deserialize directly to structurally validated schema models. | Raw payload output extraction. | Minor Gap |
| `command_new` | `crates/op-tools/src/mcptools.rs:264` | Configures structured invocation via `Command::new(mcp_bin).arg("tools")`. | Use structural executable invocation instead of raw string interpreters. | None (successfully avoids shell context spawning). | Compliant |
| `command_new` | `crates/op-tools/src/mcptools.rs:322` | Spawns external binaries safely: `Command::new(mcp_bin).arg("call")`. | Pass sub-parameters as independent positional arguments. | None. | Compliant |
| `std_fs_in_async` | `crates/op-tools/src/mcptools.rs:223` | Calls blocking standard I/O: `std::fs::read_to_string(&config_path)`. | Use non-blocking async file tools (`tokio::fs::read_to_string`) or defer to `spawn_blocking`. | **Blocking thread pool:** The blocking standard library call can starve Tokio's worker threads during load. | **Major Gap** |
| `unwrap_expect` | `crates/op-tools/src/orchestration_plugin.rs:267` | panics with `.expect("Orchestration registry not initialized")` if global state is missing. | Handle missing components gracefully via fallbacks or return custom Result errors. | Expect call can trigger unplanned runtime panics if initialization order fails. | Minor Gap |
| `unwrap_expect` | `crates/op-tools/src/orchestration_plugin.rs:431` | Panics on unwrap: `.register(...).await.unwrap()`. | Allowed in test contexts where immediate failure is desired. | None (test context). | Compliant |
| `unwrap_expect` | `crates/op-tools/src/orchestration_plugin.rs:439` | Panics on unwrap: `.register(...).await.unwrap()`. | Allowed in test contexts. | None (test context). | Compliant |
| `unwrap_expect` | `crates/op-tools/src/registry.rs:184` | Panics on unwrap: `.await.unwrap()`. | Handle runtime initialization and errors cleanly using proper Result types. | Potential panic vector during application configuration setup. | Minor Gap |
| `unwrap_expect` | `crates/op-tools/src/registry.rs:209` | Panics on unwrap: `.await.unwrap()`. | Propagate configuration/async errors up the call stack. | Potential panic vector. | Minor Gap |
| `command_new` | `crates/op-tools/src/builtin/anydesk.rs:396` | Invokes `Command::new("anydesk").arg("--get-id")` directly. | Isolate executions to safe, explicitly structured executable paths. | None. | Compliant |
| `command_new` | `crates/op-tools/src/builtin/anydesk.rs:407` | Invokes `Command::new("systemctl")` safely with discrete args. | Avoid executing shell-level processing for parameter lookups. | None. | Compliant |
| `std_fs_in_async` | `crates/op-tools/src/builtin/file.rs:216` | Asynchronously reads file: `tokio::fs::read_to_string(path).await`. | Ensure non-blocking async behavior for file access. | None. | Compliant |
| `std_fs_in_async` | `crates/op-tools/src/builtin/file.rs:252` | Asynchronously writes to file: `tokio::fs::OpenOptions::new()`. | Ensure non-blocking async behavior. | None. | Compliant |
| `std_fs_in_async` | `crates/op-tools/src/builtin/file.rs:259` | Asynchronously writes: `tokio::fs::write(path, content).await`. | Ensure non-blocking async behavior. | None. | Compliant |

---

### Actionable Recommendations for Major & Critical Gaps

#### 1. Fix Arbitrary Shell Injection (Critical Gap)
* **Location:** `crates/op-tools/src/builtin_old.rs:194` & `196`
* **Vulnerability:** Passing formatted user-provided arguments directly to `sh -c` bypasses standard process boundaries. If any argument contains shell-active characters (such as `;`, `&&`, or `$()`), it allows arbitrary shell command execution in the context of the running application.
* **Remedy:** Do not spawn a shell (`sh`). Instead, split the command into its base executable name and separate positional arguments. Execute the target program directly with discrete parameters.
```rust
// Insecure:
// match tokio::process::Command::new("sh").arg("-c").arg(format!("{} {}", command, args.join(" ")))

// Secure:
match tokio::process::Command::new(command)
    .args(args)
    .output()
    .await
```

#### 2. Enforce Schema-as-Code for Outputs & Errors (Major Gap)
* **Location:** `crates/op-tools/src/builtin_old.rs:217`, `225`, and `324`
* **Deficiency:** The codebase structures key data contracts, errors, and process outcomes using ad-hoc text formatting (`format!("Command failed: {}", stderr)`). This deviates from a disciplined schema-driven system.
* **Remedy:** Map all response components (both successful results and structured failures) to strongly-typed structures generated from formal, versioned schemas (e.g. Protocol Buffers or shared JSON-schema definitions).
```rust
// Define a structured, versioned schema contract (e.g., in a shared protobuf definition)
/*
message ToolResponse {
  string request_id = 1;
  uint64 duration_ms = 2;
  oneof result {
    string success_payload = 3;
    ErrorDetails error = 4;
  }
}
*/

// Instantiate structured models instead of raw strings:
let response = ToolResponse {
    request_id: request.id.clone(),
    duration_ms: start.elapsed().as_millis() as u64,
    result: Some(Result::Error(ErrorDetails {
        code: ErrorCode::CommandFailed as i32,
        message: stderr.trim().to_string(),
    })),
};
```

#### 3. Eliminate Blocking File I/O in Async Executor Contexts (Major Gap)
* **Location:** `crates/op-tools/src/mcptools.rs:223`
* **Deficiency:** `std::fs::read_to_string` blocks the executing operating system thread. When invoked inside a Tokio async function, this blocks a pool executor thread, hurting parallel task performance and causing resource starvation.
* **Remedy:** Replace the blocking call with `tokio::fs::read_to_string`.
```rust
// Instead of:
// let mut raw = std::fs::read_to_string(&config_path)...

// Use:
let mut raw = tokio::fs::read_to_string(&config_path)
    .await
    .with_context(|| format!("Failed to read {}", config_path))?;
```