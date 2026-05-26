# op-tools Documentation & Security Audit

## 1. Documentation Audit

### Crate-Level Documentation
* **`crates/op-tools/src/lib.rs`**: **Present**. Crate-level `//!` documentation is present and describes the tool registry, execution mechanisms, security philosophy, and the orchestration plugin architecture.

### README.md Presence
* **`README.md`**: **Absent**. No `README.md` file was provided in the source files of the `op-tools` crate.

### Public Unsafe Functions
* No public unsafe functions were found in the provided source files. Thus, there are no instances of undocumented public safety invariants.

### Sample of 10 Public Items Missing `///` rustdoc
The following public items were found to be missing `///` rustdoc comments:

1. **`crates/op-tools/src/builtin_old.rs:77`**
   ```rust
   pub fn new(allowed_commands: Vec<String>) -> Self
   ```
2. **`crates/op-tools/src/builtin_old.rs:81`**
   ```rust
   pub fn with_defaults() -> Self
   ```
3. **`crates/op-tools/src/mcptools.rs:49`**
   ```rust
   pub async fn register_mcp_tools(registry: &ToolRegistry) -> Result<usize>
   ```
4. **`crates/op-tools/src/orchestration_plugin.rs:113`**
   ```rust
   pub enum SessionEventType
   ```
5. **`crates/op-tools/src/orchestration_plugin.rs:242`**
   ```rust
   pub fn init_orchestration_registry()
   ```
6. **`crates/op-tools/src/builtin/agent_tool.rs:43`**
   ```rust
   pub struct AgentConnectionRegistry
   ```
7. **`crates/op-tools/src/builtin/agent_tool.rs:48`**
   ```rust
   pub fn new(bus_type: BusType) -> Self
   ```
8. **`crates/op-tools/src/builtin/anydesk.rs:16`**
   ```rust
   pub async fn register_anydesk_tools(registry: &crate::ToolRegistry) -> Result<()>
   ```
9. **`crates/op-tools/src/builtin/dbus_hybrid.rs:12`**
   ```rust
   pub struct DbusMethodTool
   ```
10. **`crates/op-tools/src/builtin/dbus_introspection.rs:252`**
    ```rust
    pub struct DbusListServicesTool
    ```

---

## 2. Security & Quality Audit Findings

### Critical Findings

#### [CRITICAL] Command Injection & Security Bypass via Shell Chaining in Restricted Mode
* **File/Line**: `crates/op-tools/src/security.rs:385` and `crates/op-tools/src/builtin/shell.rs:98`
* **Vulnerability Type**: Input Validation Bypass / Arbitrary Command Execution
* **Description**: In `crates/op-tools/src/security.rs`, the `SecurityValidator::check_command` method validates whether a command can be executed under the `Restricted` or `Custom` access levels. However, it only extracts and validates the **first** whitespace-separated token (`base_cmd`) of the command string:
  ```rust
  let base_cmd = command
      .split_whitespace()
      .next()
      .ok_or_else(|| SecurityError::ValidationFailed("Empty command".to_string()))?;
  ```
  If `base_cmd` is allowed (e.g., `ls` or `cat`), the validation passes.
  However, in `crates/op-tools/src/builtin/shell.rs:98`, `ShellExecuteTool` and `ShellExecuteBatchTool` execute the **entire** un-sanitized command string directly via `bash -c`:
  ```rust
  let mut child = Command::new("bash")
      .arg("-c")
      .arg(command)
  ```
  An attacker with restricted access can bypass command restriction policies entirely by chaining commands with shell metacharacters. For example, executing:
  `command = "ls ; cat /etc/shadow"`
  will bypass validation (since `base_cmd` is `ls`), but executes `cat /etc/shadow` on the host system as the running user (often root).
* **Remediation**: Avoid raw shell evaluation via `bash -c` or `sh -c` for restricted contexts. If shell evaluation is required, use a strict parser to validate all chained commands and enforce validation on the entire input, or utilize the `InputValidator` in `validation.rs` which rejects forbidden characters such as `;`, `&`, and `|`.

#### [CRITICAL] Path Traversal Bypass & Arbitrary File Write in Self-Repository Tools
* **File/Line**: `crates/op-tools/src/builtin/self_tools.rs:206`
* **Vulnerability Type**: Path Traversal / Arbitrary File Write
* **Description**: In `SelfWriteFileTool::execute`, the parent directory validation is designed to prevent escaping the self-repository path. However, when the parent directory of the target file path does not yet exist, and `create_dirs` is `true` (which is the default), the code skips path-containment validation:
  ```rust
  let parent = full_path.parent();
  if let Some(p) = parent {
      if p.exists() {
          let canonical_parent = p.canonicalize().unwrap_or(p.to_path_buf());
          if !canonical_parent.starts_with(&canonical_repo) {
              return Err(anyhow::anyhow!(
                  "Path '{}' would escape the self-repository. Access denied.",
                  path
              ));
          }
      } else if !create_dirs {
          return Err(anyhow::anyhow!("Parent directory does not exist: {:?}", p));
      }
  }
  ```
  If `p.exists()` is `false`, and `create_dirs` is `true`, no validation is performed. The tool then proceeds to recursively create the non-existent parent directory and write the file:
  ```rust
  if create_dirs {
      if let Some(parent) = full_path.parent() {
          tokio::fs::create_dir_all(parent).await?;
      }
  }
  tokio::fs::write(&full_path, content).await?;
  ```
  This allows an attacker to specify a path containing traversal components targeting non-existent subdirectories outside the repository (e.g., `../../../../etc/cron.d/nonexistent_subdir/malicious`). The code bypasses validation, recursively creates `/etc/cron.d/nonexistent_subdir`, and writes arbitrary files there, leading to host takeover.
* **Remediation**: Perform path-containment checks on the normalized parent path regardless of whether the directory currently exists. Resolve and normalize directory structures prior to checking if the path starts with `canonical_repo`.

#### [CRITICAL] Shell Command Injection in Legacy Shell Tool
* **File/Line**: `crates/op-tools/src/builtin_old.rs:137`
* **Vulnerability Type**: Shell Command Injection
* **Description**: The legacy `ShellTool::execute` command executes a shell command and its arguments by formatting them directly into a raw string passed to `sh -c`:
  ```rust
  match tokio::process::Command::new("sh")
      .arg("-c")
      .arg(format!("{} {}", command, args.join(" ")))
  ```
  The legacy `validate` method only checks the first whitespace-separated token of the command and performs no validation on the elements of the `args` array. This allows easy shell command injection through both the command string and its arguments.
* **Remediation**: Avoid executing commands through a shell interpreter. Instead, execute the binary directly using `tokio::process::Command::new(command).args(args)` to avoid argument evaluation and injection.

---

### High/Medium Severity Findings

#### [HIGH] Input Validation Bypass / Missing Integration of InputValidator
* **File/Line**: `crates/op-tools/src/builtin/shell.rs` and `crates/op-tools/src/validation.rs`
* **Vulnerability Type**: Missing Security Control
* **Description**: The `validation.rs` module contains a robust `InputValidator` that correctly implements security validation on commands and paths, including checking against `FORBIDDEN_CHARS` (`;`, `&`, `|`, etc.). However, this `InputValidator` is **never integrated or called** inside the active shell execution tools in `builtin/shell.rs` (which only use `SecurityValidator` from `security.rs`). The shell execution tools are left entirely unprotected from shell metacharacter injection.
* **Remediation**: Integrate `InputValidator` into the execution path of the shell and filesystem tools, ensuring that all input strings are sanitized and validated against forbidden characters before execution.