| Severity | Issue | Evidence (file:line) | Recommendation |
| :--- | :--- | :--- | :--- |
| **Critical** | Local Privilege Escalation & Arbitrary File Overwrite via Insecure Temporary File (Symlink Attack) | `crates/op-agents/src/unified/execution/python.rs:37` | Use the `tempfile` crate to generate secure, uniquely named, randomly generated temporary files. |
| **Critical** | Sandbox Escape & Arbitrary Host Command Execution in Shell Executor | `crates/op-agents/src/unified/execution/shell.rs:56` | Avoid whitelisting complex utilities with execution capabilities (`find -exec`, `git core.pager`). Implement sub-argument whitelisting. |
| **Critical** | SQLite Multi-statement Injection & Arbitrary File Write | `crates/op-agents/src/agents/database/sql_pro.rs:26` | Reject queries containing semicolons (`;`) or use a structured SQLite driver with parameterized bindings rather than spawning raw command-line processes. |
| **Critical** | Host Command Execution via Argument Injection in Git/Gcc/Go CLI Agents | `crates/op-agents/src/agents/analysis/code_reviewer.rs:72` | Restrict user-provided arguments to a strict whitelist of safe flags, and completely reject arbitrary flags starting with `--` (e.g., `--ext-cmd`). |
| **High** | Undefined Behavior & Out-of-Bounds Memory Reads via Unpadded SIMD Parsing | `crates/op-agents/src/dbus_service.rs:114` | Never use `unsafe { simd_json::from_str }` on standard unpadded strings. Use safe parsing wrappers or pad inputs with `simd_json::SIMDJSON_PADDING`. |
| **High** | Permanent Memory Database Corruption & Data Loss via Manual JSON Construction | `crates/op-agents/src/agents/orchestration/memory.rs:249` | Serialize the cache map using a robust serialization library (e.g., `serde_json` or `simd_json`) rather than manual string formatting. |
| **High** | Complete Bypass of Sandboxing Policies in Standard Built-in Agents | `crates/op-agents/src/agents/language/golang_pro.rs:15` | Refactor all built-in agents in `src/agents/` to leverage `SandboxExecutor` instead of spawning raw host processes via `std::process::Command`. |
| **Medium** | Registry Startup Race Condition (No Default Factory Found) | `crates/op-agents/src/agent_registry.rs:172` | Initialize the `default_factory` synchronously during registry creation. Use standard sync locks like `parking_lot::RwLock` for registry setup. |
| **Medium** | Coarse-grained Async Write Locking causing Denial of Service (DoS) | `crates/op-agents/src/router.rs:141` | Do not hold the write lock of the entire registry across slow asynchronous boundaries (such as spawning system processes). |

---

### Deep Dive & Technical Analysis

#### 1. Local Privilege Escalation & Arbitrary File Overwrite via Insecure Temporary File
* **File:Line**: `crates/op-agents/src/unified/execution/python.rs:37`
* **Vulnerability Type**: CWE-377: Insecure Temporary File, CWE-59: Improper Link Resolution
* **Exploitability**: **Directly Exploitable**.
* **Analysis**: 
  The `PythonExecutor` writes user-provided Python code directly to `/tmp/python_exec.py` before executing it:
  ```rust
  let temp_file = "/tmp/python_exec.py";
  if let Err(e) = tokio::fs::write(temp_file, code).await { ... }
  ```
  Since `/tmp` is a shared directory on Linux, a local attacker can create a symlink from `/tmp/python_exec.py` pointing to any critical file (e.g., `/home/user/.ssh/authorized_keys` or `/etc/passwd`). When the privileged D-Bus agent service executes Python code, it will follow the symlink and write the user-controlled code to the target file. This permits arbitrary file modification and immediate privilege escalation on the host.

---

#### 2. Sandbox Escape & Arbitrary Host Command Execution in Shell Executor
* **File:Line**: `crates/op-agents/src/unified/execution/shell.rs:56`
* **Vulnerability Type**: CWE-78: Command Injection / Sandbox Escape
* **Exploitability**: **Directly Exploitable**.
* **Analysis**: 
  The `ShellExecutor` attempts to restrict command execution by splitting the user-provided command into whitespace-separated parts and verifying that the first word (program) is in `ALLOWED_COMMANDS`. 
  However, many of the whitelisted commands natively support arbitrary code execution flags. For example, `find` is whitelisted:
  ```rust
  "ls", "cat", "head", "tail", "find", "grep", "wc", "file", "stat"
  ```
  An attacker can execute arbitrary unsandboxed host commands by passing:
  ```bash
  find . -exec rm -rf / \;
  ```
  Because the parsed program name is `find`, the whitelist validation succeeds, and the entire set of arguments is passed directly to the host OS. This completely breaks the security sandbox.

---

#### 3. SQLite Multi-statement Injection & Arbitrary File Write
* **File:Line**: `crates/op-agents/src/agents/database/sql_pro.rs:26`
* **Vulnerability Type**: CWE-89: SQL Injection
* **Exploitability**: **Directly Exploitable**.
* **Analysis**: 
  The SQL Pro agent allows arbitrary raw SQL execution if the query starts with `SELECT`.
  ```rust
  let q_upper = q.to_uppercase();
  if !q_upper.trim().starts_with("SELECT") ... { return Err(...); }
  cmd.arg(q);
  ```
  Because SQLite natively supports multiple queries separated by semicolons (`;`) inside a single statement, an attacker can bypass this validation to run arbitrary modifying commands:
  ```sql
  SELECT 1; ATTACH DATABASE '/home/target/.ssh/authorized_keys' AS keys; CREATE TABLE keys.bar (val text); INSERT INTO keys.bar VALUES ('ssh-rsa AAAAB3...');
  ```
  No validation is performed on the remaining query string, allowing arbitrary file creation/overwrites and database modification.

---

#### 4. Host Command Execution via Argument Injection in Git/Gcc/Go CLI Agents
* **File:Line**: `crates/op-agents/src/agents/analysis/code_reviewer.rs:72`
* **Vulnerability Type**: CWE-88: Argument Injection
* **Exploitability**: **Directly Exploitable**.
* **Analysis**: 
  The `git_diff` method (and similar methods in `golang_pro.rs:36` and `c_pro.rs:35`) takes user-supplied arguments, splits them solely by whitespace, and appends them to a raw `std::process::Command` block:
  ```rust
  if let Some(a) = args {
      validation::validate_args(a)?;
      for arg in a.split_whitespace() {
          cmd.arg(arg);
      }
  }
  ```
  Since `validation::validate_args` only checks for forbidden shell meta-characters (such as `;`, `&`, `\|`), it does not prevent argument injection. An attacker can pass Git options such as `--ext-cmd` to spawn arbitrary shells:
  ```bash
  --ext-cmd=/tmp/malicious_payload.sh
  ```
  When `git diff` runs, it calls the specified executable directly, facilitating unsandboxed arbitrary command execution.

---

#### 5. Undefined Behavior & Out-of-Bounds Memory Reads via Unpadded SIMD Parsing
* **File:Line**: `crates/op-agents/src/dbus_service.rs:114`
* **Vulnerability Type**: CWE-125: Out-of-Bounds Read
* **Exploitability**: **Directly Exploitable** (can trigger process crashes or memory exposure).
* **Analysis**: 
  The service uses `simd_json::from_str` wrapped in an `unsafe` block on unpadded buffers:
  ```rust
  let mut task_json_mut = task_json.to_string();
  let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }
  ```
  `simd_json` relies on AVX2/SSE vector instructions that read memory in 32-byte or 64-byte chunks. The crate explicitly requires that any buffer parsed with unpadded methods have `simd_json::SIMDJSON_PADDING` extra bytes allocated at the end. Calling `unsafe { simd_json::from_str }` on standard `String` allocations will lead to out-of-bounds heap memory reads, causing segmentation faults, server instability, or information leaks.

---

#### 6. Permanent Memory Database Corruption & Data Loss via Manual JSON Construction
* **File:Line**: `crates/op-agents/src/agents/orchestration/memory.rs:249`
* **Vulnerability Type**: CWE-703: Improper Control of Generation of Code/Markup
* **Exploitability**: **Directly Exploitable** (leads to Denial of Service).
* **Analysis**: 
  The `MemoryAgent` writes its cognitive memory database to disk by manual formatting:
  ```rust
  let entry_json = format!(
      "\"{}\":{{\"value\":\"{}\",...}}",
      key, entry.value, ...
  );
  ```
  If `key` or `value` contains unescaped double quotes (`"`) or backslashes (`\`), the serialized output becomes invalid JSON. On the subsequent service reboot, `simd_json::from_str` fails to parse the corrupted file and defaults to an empty database:
  ```rust
  let cache = if let Ok(content) = fs::read_to_string(&memory_path) {
      Self::parse_memory_entries(&content) // Defaults to empty HashMap if parsing fails
  ...
  ```
  This destroys the entire persistent memory store, resulting in complete data loss.

---

#### 7. Complete Bypass of Sandboxing Policies in Standard Built-in Agents
* **File:Line**: `crates/op-agents/src/agents/language/golang_pro.rs:15`
* **Vulnerability Type**: CWE-265: Privileged / Sandbox Bypass
* **Exploitability**: **High** (renders all agent security profiles useless).
* **Analysis**: 
  The codebase defines an extensive `SandboxExecutor` in `src/security/sandbox.rs` designed to enforce timeouts, CPU limits, memory limits, and command whitelists. However, **none of the standard language or analysis agents** (such as `GolangProAgent`, `PythonProAgent`, or `RustProAgent`) actually use it. Instead, they all spawn raw processes with the full privileges of the supervisor via:
  ```rust
  use std::process::Command;
  ```
  As a result, even if an agent's `SecurityProfile` specifies that `requires_root` is false or that its sandbox commands are limited, the actual execution bypasses all security constraints.

---

#### 8. Registry Startup Race Condition (No Default Factory Found)
* **File:Line**: `crates/op-agents/src/agent_registry.rs:172`
* **Vulnerability Type**: CWE-362: Concurrency Race Condition
* **Exploitability**: **Medium** (causes intermittent initialization failures).
* **Analysis**: 
  In the `AgentRegistry` constructor, the default `ProcessAgentFactory` is registered asynchronously using a tokio spawn task:
  ```rust
  let factories = registry.factories.clone();
  tokio::spawn(async move {
      let mut factories = factories.write().await;
      factories.push(default_factory);
  });
  ```
  Since `new()` is a synchronous function, if a caller attempts to use the registry to spawn an agent immediately after creation, the async task may not have run yet. This causes the lookup for `factories.iter().find(...)` to fail with `"No factory supports agent type"`.

---

#### 9. Coarse-grained Async Write Locking causing Denial of Service (DoS)
* **File:Line**: `crates/op-agents/src/router.rs:141`
* **Vulnerability Type**: CWE-400: Resource Exhaustion
* **Exploitability**: **Medium**.
* **Analysis**: 
  In `spawn_agent_handler`, the write lock of the registry state is held across an `await` point:
  ```rust
  let registry = state.registry.write().await;
  match registry.spawn_agent(agent_type, config).await { ... }
  ```
  The `spawn_agent` function performs slow asynchronous I/O, such as resolving factory support and invoking `tokio::process::Command::spawn`. While this write lock is held, all other concurrent endpoint requests (such as `/api/agents` health checks, listing agents, or getting statuses) are completely blocked. Under moderate load, this blocks the entire service worker pool.