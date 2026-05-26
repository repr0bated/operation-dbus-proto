# Production Security and Quality Audit - Prioritized Risk Register

| Severity | Issue | Evidence (file:line) | Recommendation |
| :--- | :--- | :--- | :--- |
| **Critical** | Command Validation Bypass leading to Arbitrary Shell Command Execution | `crates/op-tools/src/security.rs:381`<br>`crates/op-tools/src/builtin/shell.rs:125`<br>`crates/op-tools/src/builtin/shell.rs:268` | Refactor validation to parse the command string into a proper shell AST (or array of arguments) and validate the actual executable program and every argument individually, rather than splitting on whitespace. Avoid `sh -c` or `bash -c` string interpolation entirely. |
| **Critical** | Axum HTTP Router Router Bypasses Input Sanitization & Path Traversal Protections | `crates/op-tools/src/router.rs:118` | Intercept execution requests in the HTTP router or tool executor and route them through the `InputValidator::validate_input` pipeline before invoking `tool.execute()`. |
| **Critical** | Path Traversal / Arbitrary File Read and Write via `/proc/self/cwd` Virtual Symlink Bypass | `crates/op-tools/src/builtin/procfs.rs:14`<br>`crates/op-tools/src/builtin/procfs.rs:145`<br>`crates/op-tools/src/builtin/procfs.rs:271` | Strictly validate that the target path does not resolve to or contain `/proc/self/` subdirectories, and canonicalize the final resolved path against a strict, safe root directory to ensure it does not escape. |
| **Critical** | Command Injection in Legacy Shell Tool Base Command Extraction | `crates/op-tools/src/builtin_old.rs:173`<br>`crates/op-tools/src/builtin_old.rs:206` | Remove or completely rewrite `builtin_old.rs`. Avoid passing raw, interpolated strings with shell metacharacters to `/bin/sh -c`. Use structured process execution with discrete arguments instead. |
| **High** | Absence of Versioned Schemas (Protocol Buffers) / Widespread Schema-as-Code Violations | `crates/op-tools/src/registry.rs:16`<br>`crates/op-tools/src/builtin/agent_tool.rs:567`<br>`crates/op-tools/src/builtin/dbus.rs:29` | Replace ad-hoc JSON-schema definitions constructed at runtime with unified, version-controlled Protocol Buffers or centralized JSON schema files. |
| **High** | Sync Blocking OS Commands within Async Tokio Context causing Threadpool Starvation | `crates/op-tools/src/builtin/anydesk.rs:400`<br>`crates/op-tools/src/builtin/anydesk.rs:434`<br>`crates/op-tools/src/builtin/anydesk.rs:460` | Replace `std::process::Command` usage with `tokio::process::Command` throughout all tool implementations to keep operations entirely asynchronous and non-blocking. |
| **High** | Lack of OSCAL Compliance & Control Traceability in Access Gating | `crates/op-tools/src/security.rs:59`<br>`crates/op-tools/src/registry.rs:16` | Embed NIST SP 800-53 security control identifiers and OSCAL-compliant metadata directly into the `ToolDefinition` structure and the configuration loading mechanism. |

---

## Detailed Findings & Mitigation Blueprints

### 1. Command Validation Bypass leading to Arbitrary Shell Command Execution
* **Severity:** Critical (Directly Exploitable)
* **Evidence:** 
  * `crates/op-tools/src/security.rs:381`
  * `crates/op-tools/src/builtin/shell.rs:125`
  * `crates/op-tools/src/builtin/shell.rs:268`

#### Vulnerability Analysis
In `SecurityValidator::check_command`, restricted/custom access level commands are validated as follows:
```rust
let base_cmd = command
    .split_whitespace()
    .next()
    .ok_or_else(|| SecurityError::ValidationFailed("Empty command".to_string()))?;
```
The validator then checks if `base_cmd` is within the allowed commands list (such as `ls`, `cat`, or `free`).

However, inside `ShellExecuteTool::execute` and `execute_command`, the *entire* raw command string is executed inside `bash -c`:
```rust
let mut child = Command::new("bash")
    .arg("-c")
    .arg(command)
    // ...
```
If a user in a restricted session inputs `ls ; rm -rf /`, the whitespace tokenizer splits the command. The first segment parsed by the validator is `ls`, which is fully allowed. The validation succeeds, and the entire payload (`ls ; rm -rf /`) is sent to `/bin/bash -c`. Bash processes the semicolon as a command separator and executes `rm -rf /` with the permissions of the parent process (which runs as root). This is a trivial validation bypass leading to remote code execution.

#### Mitigation Blueprint
1. Avoid passing shell command strings to shell executors. Implement structured command execution using discrete arguments.
2. If raw shell execution must be supported, parse the command string into a concrete AST (Abstract Syntax Tree) using a safe parser and reject any command that contains control operators (such as `;`, `&`, `|`, `&&`, `||`, `$()`, backticks, etc.).
3. Restrict execution strictly to the validated command executable and its positional arguments:
```rust
// Use structured execution:
let mut child = tokio::process::Command::new(base_cmd);
child.args(parsed_args);
```

---

### 2. Axum HTTP Router Bypasses Input Sanitization & Path Traversal Protection
* **Severity:** Critical (Directly Exploitable)
* **Evidence:** 
  * `crates/op-tools/src/router.rs:118`

#### Vulnerability Analysis
The endpoint `execute_tool_handler` accepts a raw, unchecked JSON `Value` payload (`params`) from the request body and routes it directly to the matching tool:
```rust
async fn execute_tool_handler(
    State(state): State<ToolsState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(params): Json<Value>,
) -> impl IntoResponse {
    if let Some(tool) = state.registry.get(&name).await {
        match tool.execute(params).await {
            // ...
```
This execution flow completely bypasses `InputValidator` (`crates/op-tools/src/validation.rs`), which is designed to validate schemas, run recursive input sanitization to strip null bytes and injection characters, and block directory traversal attempts. An untrusted HTTP client can invoke any filesystem or shell tool (such as `file_read` or `shell_execute`) with highly destructive arguments, rendering the validation layer useless.

#### Mitigation Blueprint
Ensure the HTTP router handler initializes and invokes the validation layer before routing the request to the tool:
```rust
// Retrieve the InputValidator and validate the inputs first:
let validator = InputValidator::new(); // Or pull from shared state
let validated = validator.validate_input(&name, &params, &tool.input_schema(), Some(session_id)).await?;

if !validated.should_proceed() {
    return Err(StatusCode::BAD_REQUEST);
}

let result = tool.execute(validated.into_input()).await?;
```

---

### 3. Path Traversal / Arbitrary File Read and Write via `/proc/self/cwd` Virtual Symlink Bypass
* **Severity:** Critical (Directly Exploitable)
* **Evidence:** 
  * `crates/op-tools/src/builtin/procfs.rs:14`
  * `crates/op-tools/src/builtin/procfs.rs:145`
  * `crates/op-tools/src/builtin/procfs.rs:271`

#### Vulnerability Analysis
The `ProcFsReadTool` and `ProcFsWriteTool` validate user-supplied relative paths using the following function:
```rust
fn validate_relative_path(path: &str) -> anyhow::Result<()> {
    if path.is_empty() || path.starts_with('/') || path.contains("..") || path.contains('\\') {
        return Err(anyhow::anyhow!("Invalid path"));
    }
    Ok(())
}
```
The validated path is then joined to `/proc`:
```rust
let full_path = Path::new("/proc").join(path);
```
While this successfully blocks path traversal using `..`, it fails to account for virtual symlinks within `/proc`. A malicious actor can request the path `self/cwd/etc/passwd` or `self/root/etc/shadow`. This maps to `/proc/self/cwd/etc/passwd`. Because `/proc/self/cwd` is a virtual symlink pointing to the current working directory of the process, the path resolves directly to a location outside the `/proc` directory. This allows arbitrary reading and writing of critical system files, bypassing all filesystem isolation controls.

#### Mitigation Blueprint
1. Explicitly block any paths containing virtual symlinks, such as `self`, `thread`, or numeric PID directories inside the relative path validation:
```rust
if path.split('/').any(|segment| segment == "self" || segment == "thread" || segment.chars().all(char::is_numeric)) {
    return Err(anyhow::anyhow!("Access to process-internal symlinks is forbidden"));
}
```
2. Canonicalize the final path and assert that it remains strictly inside the target root directory (`/proc` or `/sys`):
```rust
let full_path = Path::new("/proc").join(path);
let canonical = full_path.canonicalize()?;
if !canonical.starts_with("/proc") {
    return Err(anyhow::anyhow!("Path escaped sandbox root"));
}
```

---

### 4. Command Injection in Legacy Shell Tool Base Command Extraction
* **Severity:** Critical (Directly Exploitable)
* **Evidence:** 
  * `crates/op-tools/src/builtin_old.rs:173`
  * `crates/op-tools/src/builtin_old.rs:206`

#### Vulnerability Analysis
In `builtin_old.rs`, the deprecated `ShellTool::validate` method attempts to isolate the base command using whitespace splitting:
```rust
let base_cmd = command.split_whitespace()
    .next()
    .unwrap_or(command);
```
The system checks if this base command is in `allowed_commands` (for example, `cat`). However, during execution, the command is run using shell interpolation:
```rust
match tokio::process::Command::new("sh")
    .arg("-c")
    .arg(format!("{} {}", command, args.join(" ")))
    .output()
    .await
```
An attacker can exploit this by supplying a payload such as `cat; rm -rf /`. The extracted `base_cmd` is `cat;`. If the allowed list contains `cat;` (or if whitespace separation is bypassed via other shell control operators), the entire string is evaluated by `sh -c`. This leads to shell injection and arbitrary system command execution.

#### Mitigation Blueprint
1. Completely delete or disable the `builtin_old.rs` module in production environments.
2. For all valid command executions, avoid using shell wrapper binaries (`sh`, `bash`) and direct command string formatting. Run target executables directly via `tokio::process::Command::new(executable)` and pass arguments as a safe, un-evaluated vector (`args(&[])`).

---

### 5. Absence of Versioned Schemas (Protocol Buffers) / Widespread Schema-as-Code Violations
* **Severity:** High (Quality / Compliance Gap)
* **Evidence:** 
  * `crates/op-tools/src/registry.rs:16`
  * `crates/op-tools/src/builtin/agent_tool.rs:567`
  * `crates/op-tools/src/builtin/dbus.rs:29`
  * `crates/op-tools/src/builtin/dbus_introspection.rs:389`

#### Vulnerability Analysis
The codebase claims to follow a strict "schema-as-code" discipline using Protocol Buffers and OSCAL. However, data contracts, input validations, and metadata definitions across all tools are defined using ad-hoc, untyped JSON structures via the `json!` macro at runtime.

For example, `ToolDefinition` relies on a generic `simd_json::OwnedValue` for its `input_schema` instead of a compiled, typed contract:
```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value, // Ad-hoc JSON value
    // ...
}
```
This makes it impossible to guarantee contract compatibility or compile-time schema safety between the tool providers and the orchestrators.

#### Mitigation Blueprint
1. Refactor `ToolDefinition` and tool execution inputs to use strictly typed Rust structs auto-generated from shared Protocol Buffers (`.proto` files).
2. Distribute the versioned schemas as part of a centralized contract repository to ensure consistency across services.

---

### 6. Threadpool Starvation via Blocking Synchronous System Commands in Async Context
* **Severity:** High (Performance / Stability Risk)
* **Evidence:** 
  * `crates/op-tools/src/builtin/anydesk.rs:400`
  * `crates/op-tools/src/builtin/anydesk.rs:434`
  * `crates/op-tools/src/builtin/anydesk.rs:460`
  * `crates/op-tools/src/builtin/anydesk.rs:471`

#### Vulnerability Analysis
The `anydesk` tool suite executes synchronous blocking operations using `std::process::Command` within asynchronous tasks:
```rust
match Command::new("anydesk").arg("--get-id").output() { ... }
```
In a multi-threaded async runtime such as Tokio, executing blocking system processes on the main executor threads stalls those threads. If multiple concurrent requests trigger these tools, the entire thread pool can quickly become starved. This leads to high latency, connection timeouts, and potential application crashes.

#### Mitigation Blueprint
Replace all instances of `std::process::Command` in async tools with `tokio::process::Command`:
```rust
// Non-blocking asynchronous implementation:
let output = tokio::process::Command::new("anydesk")
    .arg("--get-id")
    .output()
    .await?;
```

---

### 7. Lack of OSCAL Compliance & Control Traceability in Access Gating
* **Severity:** High (Compliance Gap)
* **Evidence:** 
  * `crates/op-tools/src/security.rs:59`
  * `crates/op-tools/src/registry.rs:16`

#### Vulnerability Analysis
The authorization and access gating architecture defined in `security.rs` lacks traceability to standardized controls (such as NIST SP 800-53 or ISO 27001). While the system categorizes tools into access levels (`Unrestricted`, `Restricted`, `Custom`), these boundaries are not mapped to formal control requirements. This breaks the security compliance chain and complicates security audits.

#### Mitigation Blueprint
1. Extend `ToolDefinition` to include explicit OSCAL control mappings:
```rust
pub struct ToolDefinition {
    // ...
    #[serde(default)]
    pub oscal_controls: Vec<String>, // e.g., ["AC-3", "AC-6"]
}
```
2. Export the generated tool registry definitions to a standardized OSCAL Component Definition JSON file during the build process to provide automated, auditable verification of your security controls.