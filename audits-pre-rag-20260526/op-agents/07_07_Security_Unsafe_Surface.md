### Unsafe Blocks Audit

The following table lists all `unsafe` blocks identified within the provided source files. None of these blocks contain `// SAFETY:` comments explaining their safety guarantees.

| File Path | Line | Context | Finding / Missing `// SAFETY:` |
| :--- | :--- | :--- | :--- |
| `crates/op-agents/src/agent_registry.rs` | 245 | `let specs: Vec<AgentSpec> = unsafe { simd_json::from_str(&mut content) }` | **Missing `// SAFETY:`**. Modifies local string in-place; technically safe but lacks safety documentation. |
| `crates/op-agents/src/dbus_service.rs` | 115 | `let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }.map_err(...)` | **Missing `// SAFETY:`**. Deserializes temporary cloned string; safe but lacks documentation. |
| `crates/op-agents/src/agents/orchestration/memory.rs` | 126 | `let value: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut content_mut).unwrap_or_default() };` | **Missing `// SAFETY:`**. Deserializes local cognitive memory cache in-place; lacks safety documentation. |
| `crates/op-agents/src/agents/orchestration/memory.rs` | 210 | `let old_cache: HashMap<String, String> = unsafe { simd_json::from_str(&mut content_mut).unwrap_or_default() };` | **Missing `// SAFETY:`**. Deserializes local migration cache in-place; lacks safety documentation. |
| `crates/op-agents/src/generator/template.rs` | 475 | `let task: {struct_name}Task = match unsafe {{ simd_json::from_str(&mut task_json) }} {{` | **Missing `// SAFETY:`**. Raw template output block generated into target code lacks safety annotations. |
| `crates/op-agents/src/security/validation.rs` | 192 | `unsafe { simd_json::from_str(&mut json_mut).map_err(...) }` | **Missing `// SAFETY:`**. Modifies temporary local string; safe but lacks safety documentation. |

---

### Command Execution Audit

There are **98** direct/indirect invocations of `Command::new` or `tokio::process::Command::new` across the codebase.

#### Validation Status
- For almost all language-specific and infrastructure-specific agents, user input is passed to `args` or `path`.
- **Paths** are validated via `validation::validate_path(path, ALLOWED_DIRS)` which checks for forbidden characters (e.g., `;`, `&`, `|`, `$`) and restricts the execution directory to `/home`, `/tmp`, and `/opt`.
- **Arguments** are validated via `validation::validate_args(args)` which filters out the same injection characters.
- **Weakness**: Several tools (such as `git` in `code_reviewer.rs:68`, `aws` in `cloud.rs:23`, and `docker` in `deployment.rs:25`) validate the raw arguments string but then use `split_whitespace()` to dynamically build arguments. This allows **argument injection** (e.g. passing flags like `--exec`, `--config`, or `--override` that alter program behavior), even if shell injection characters are blocked.

---

### Forbidden Commands Audit

The following references to forbidden shell utilities and network exfiltration tools were identified:

#### `crates/op-agents/src/agents/language/bash_pro.rs:25`
- **Command**: `Command::new("bash")`
- **Severity**: High
- **Description**: Spawns a shell environment directly using `Command::new("bash")` which can bypass argument structure validation.

#### `crates/op-agents/src/agents/language/bash_pro.rs:78`
- **Command**: `Command::new("bash")`
- **Severity**: High
- **Description**: Spawns another instance of `bash` to perform manual syntax checks.

#### `crates/op-agents/src/agents/language/bash_pro.rs:18`
- **Command**: `profile: SecurityProfile::code_execution("bash-pro", vec!["bash", "sh", "shellcheck"])`
- **Severity**: High
- **Description**: Whitelists forbidden shells `bash` and `sh` in the security profile configuration.

#### `crates/op-agents/src/generator/template.rs:160`
- **Command**: `"bash-pro" \| "posix-shell-pro" => { commands.extend(["bash", "sh", "shellcheck"]...`
- **Severity**: High
- **Description**: Code-generator automatically whitelists forbidden shells `bash` and `sh` for generated shell agents.

#### `crates/op-agents/src/agents/operations/devops_troubleshooter.rs:39`
- **Command**: `commands_to_run.push("curl -v <endpoint>");`
- **Severity**: High
- **Description**: Recommends the use of `curl` (a forbidden network exfiltration tool) in output recommendation strings sent to users.

---

### Hardcoded Credentials & IPs

No hardcoded IP addresses, tokens, or passwords were found in the provided source files. The codebase references the environment variable `OPENAI_API_KEY` for OpenAI embeddings configuration but does not hardcode any keys.

---

### D-Bus Method Exposure & Privilege Escalation

#### `crates/op-agents/src/dbus_service.rs:113`
- **Method Exposed**: `execute(task_json: String) -> Result<String, zbus::fdo::Error>`
- **Callable By**: Any local peer on the system D-Bus.

#### `crates/op-agents/src/dbus_service.rs:149`
- **Method Exposed**: `run_operation(operation: String, path: String, args: String) -> Result<String, zbus::fdo::Error>`
- **Callable By**: Any local peer on the system D-Bus.

#### Critical Security Vulnerability: Unauthenticated Privilege Escalation
The `dbus-agent-manager` binary starts all agents (including `rust-pro`, `python-pro`, `network-engineer`, and `deployment`) on the **D-Bus System Bus** by default (`dbus-agent-manager.rs:260`):
```rust
let bus_type = if std::env::var("DBUS_AGENT_SESSION").is_ok() {
    BusType::Session
} else {
    BusType::System
};
```
If the manager or individual agents run as a privileged system daemon (such as `root`), **any unprivileged local user** connected to the system bus can call the `execute` or `run_operation` methods. 

Because `DbusAgentService` performs **no verification** of the caller's credentials (such as verifying the Unix UID of the sender via `zbus::Connection::get_field` or matching the sender to a privileged group), unprivileged users can execute arbitrary commands within the whitelisted sandboxes (e.g. running arbitrary Python scripts or calling `cargo build` which executes arbitrary code via build scripts) as the privileged daemon user, directly achieving local privilege escalation to `root`.