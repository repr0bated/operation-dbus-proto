### Unsafe Blocks Audit

Below is the list of all `unsafe {` blocks in the audited codebase, cited by file and line number with context.

#### 1. `crates/op-tools/src/mcptools.rs:271`
* **Context**: `if let Ok(list) = unsafe { simd_json::from_str::<Vec<McpToolsServerConfig>>(&mut raw_mut) } {`
* **Missing `// SAFETY:`**: Yes

#### 2. `crates/op-tools/src/mcptools.rs:279`
* **Context**: `let single = unsafe { simd_json::from_str::<McpToolsServerConfig>(&mut raw_mut2) }`
* **Missing `// SAFETY:`**: Yes

#### 3. `crates/op-tools/src/mcptools.rs:290`
* **Context**: `let mut config: McpToolsConfig = unsafe { simd_json::from_str(&mut raw) }`
* **Missing `// SAFETY:`**: Yes

#### 4. `crates/op-tools/src/mcptools.rs:341`
* **Context**: `let payload: Value = unsafe { simd_json::from_str(&mut stdout_mut) }`
* **Missing `// SAFETY:`**: Yes

#### 5. `crates/op-tools/src/mcptools.rs:408`
* **Context**: `let payload: Value = unsafe { simd_json::from_str(&mut stdout_mut) }`
* **Missing `// SAFETY:`**: Yes

#### 6. `crates/op-tools/src/builtin/agent_tool.rs:327`
* **Context**: `let task: Value = match unsafe { simd_json::from_str(&mut task_json_mut) } {`
* **Missing `// SAFETY:`**: Yes

#### 7. `crates/op-tools/src/builtin/agent_tool.rs:475`
* **Context**: `let parsed: Value = unsafe { simd_json::from_str(&mut result_mut)? };`
* **Missing `// SAFETY:`**: Yes

#### 8. `crates/op-tools/src/builtin/rtnetlink_tools.rs:69`
* **Context**: `unsafe { simd_json::from_str(stdout_mut.as_mut_str()) }.map_err(`
* **Missing `// SAFETY:`**: Yes

---

### Command Spawning and Validation Audit

There are **26** total invocations of `Command::new` (or `tokio::process::Command::new`) in this crate. 

* **Static/Safe Arguments (15)**: Found in `anydesk.rs` and `rtnetlink_tools.rs` for querying system status (e.g., `pgrep`, `systemctl is-active`, `netstat`, `xauth list`, `ip addr show`).
* **User-Controlled/Unvalidated Arguments (11)**: Arguments are either retrieved directly from LLM/user input or environment variables. This is found in `builtin_old.rs`, `mcptools.rs`, `incus_tools.rs`, `self_tools.rs`, `shell_tool.rs`, `indexer_tools.rs`, and `shell.rs`.

---

### Forbidden Commands Violation Audit

The following spawn sites reference forbidden command binaries or shell invocations that bypass standard argument validation:

#### 1. `crates/op-tools/src/builtin_old.rs:169`
* **Forbidden Command**: `sh`
* **Context**: `match tokio::process::Command::new("sh").arg("-c").arg(format!("{} {}", command, args.join(" ")))`
* **Severity**: High

#### 2. `crates/op-tools/src/builtin/shell_tool.rs:78`
* **Forbidden Command**: `bash`
* **Context**: `let mut child = Command::new("bash").arg("-c").arg(command)`
* **Severity**: High

#### 3. `crates/op-tools/src/builtin/indexer_tools.rs:41`
* **Forbidden Command**: `bash`
* **Context**: `let mut command = Command::new("bash").arg("openclaw-indexer/run.sh").arg("search").arg(query);`
* **Severity**: High

#### 4. `crates/op-tools/src/builtin/shell.rs:351`
* **Forbidden Command**: `bash`
* **Context**: `let mut child = Command::new("bash").arg("-c").arg(command)`
* **Severity**: High

---

### Hardcoded Secret & Network Configuration Audit

#### 1. `crates/op-tools/src/builtin/code_search.rs:188`
* **Detail**: Hardcoded loopback IP endpoint `"http://127.0.0.1:6333"` is defined as the fallback Qdrant URL.
* **Severity**: Low

#### 2. `crates/op-tools/src/builtin/rtnetlink_tools.rs:242`
* **Detail**: Hardcoded sample gateway IP address `"148.113.204.1"` is embedded in the schema description for routing.
* **Severity**: Informational

---

### D-Bus Method Exposure

The following D-Bus method is registered on the system or session bus and is callable by peer processes:

* **Service Path**: `org.dbusmcp.Agent.[CapAgentName]` (e.g., `org.dbusmcp.Agent.RustPro`)
* **Interface**: `org.dbusmcp.Agent`
* **Exposed Methods**:
  * `name` (Returns agent name)
  * `description` (Returns agent description)
  * `operations` (Returns list of operations)
  * `execute(task_json: String) -> String` (Executes arbitrary JSON-defined operations on the agent)
* **Risk**: Since the agent system can run on the `System` bus (callable by any system peer), any unprivileged local user could potentially call the `execute` method to trigger agent tasks.

---

### Security Vulnerabilities & Quality Findings

#### 1. Command Injection via Weak Whitelist Tokenization [CRITICAL]
* **Location**: `crates/op-tools/src/builtin_old.rs:125` and `crates/op-tools/src/builtin_old.rs:169`
* **Impact**: Directly exploitable. The `ShellTool` validates input commands by splitting the input string on whitespace and matching the first token against `allowed_commands` (e.g., `ls`, `cat`). 
* **Exploit Scenario**: If an attacker provides `"ls ; rm -rf /"`, the split-whitespace token is `"ls"`. The validation passes because `"ls"` is in the whitelist. The unsanitized string is then formatted directly into `Command::new("sh").arg("-c")`, executing the secondary payload.
* **Remediation**: Avoid launching shell interpreters (`sh`, `bash`). Run the target binary directly with parameterized arguments.

#### 2. Arbitrary File Read and Exfiltration [CRITICAL]
* **Location**: `crates/op-tools/src/builtin_old.rs:197`
* **Impact**: Directly exploitable. `FileReadTool` executes a raw file read on `tokio::fs::read(path)` with zero path traversal protection, sandboxing, or permission checks.
* **Exploit Scenario**: A peer or agent can request the tool with `path: "/etc/shadow"` or `path: "../../../etc/passwd"` to read sensitive system files.
* **Remediation**: Enforce canonicalized path resolution against a strict base directory whitelist.

#### 3. Bypassed Input Validation Layer [HIGH]
* **Location**: `crates/op-tools/src/validation.rs`
* **Impact**: While `validation.rs` implements a robust `InputValidator` with blacklist checking and path containment logic, **no execution tool** in `builtin/shell.rs`, `builtin_old.rs`, or `builtin/shell_tool.rs` actually instantiates or calls this validator. The tools query `get_security_validator()` from `security.rs`, which completely lacks input argument and shell metacharacter scrubbing.
* **Remediation**: Integrate `InputValidator` into the execution path of all shell and filesystem tools.

#### 4. Flag Injection on Self-Repository Code Search [HIGH]
* **Location**: `crates/op-tools/src/builtin/self_tools.rs:291` and `crates/op-tools/src/builtin/self_tools.rs:301`
* **Impact**: The pattern and path parameters are passed directly to `Command::new("rg")` and `Command::new("grep")`. If `pattern` starts with a hyphen (e.g., `-f`), it will be interpreted as an options flag rather than a search string, leading to unexpected behavior or arbitrary file reading by the search tools.
* **Remediation**: Insert `--` before the pattern in the arguments array to signal the end of command options.

#### 5. Arbitrary Service Restart [HIGH]
* **Location**: `crates/op-tools/src/builtin/self_tools.rs:562`
* **Impact**: The `SelfDeployTool` restarts services via `systemctl restart [service]`. The `service` argument is directly user-controlled and is not validated. An attacker can restart arbitrary system services (e.g., `sshd`, `dbus`), resulting in denial of service.
* **Remediation**: Restrict the `service` argument to a hardcoded whitelist of allowed self-services.