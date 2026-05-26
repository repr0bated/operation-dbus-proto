# Production Security and Quality Audit: `op-agents`

This document contains the production security, quality, and architectural compliance audit for the `op-agents` crate based on the provided source code.

---

## 1. Executive Summary

A comprehensive security audit of the `op-agents` codebase was conducted. The system implements an agent-based architectural pattern where domain-specific AI assistants are registered and exposed via D-Bus and HTTP interfaces. 

While the codebase implements active sandboxing, path validation, and command whitelisting, several **Critical** and **High** severity vulnerabilities have been identified. Specifically, the lack of D-Bus peer credential checking enables **local privilege escalation to root** on systems utilizing the default System D-Bus configuration. Additionally, multiple unsafe blocks lack safety documentation, and there are architectural deviations from the required **Schema-as-Code** discipline.

---

## 2. Security & Unsafe Analysis

### Unsafe Blocks Audit
There are **6** instances of `unsafe` blocks in the audited code. All 6 instances are used for in-place JSON parsing via `simd_json::from_str`. Every single instance **lacks a `// SAFETY:` comment**, which violates safe Rust development standards.

Below is the exhaustive list of `unsafe` blocks with line context:

1. **`crates/op-agents/src/agent_registry.rs:252`**
   ```rust
   let specs: Vec<AgentSpec> = unsafe { simd_json::from_str(&mut content) }
       .context("Failed to parse agent specifications")?;
   ```
   * *Finding*: Missing `// SAFETY:` comment. Mutating a string in-place assumes exclusive ownership and valid UTF-8, which must be guaranteed by the caller.

2. **`crates/op-agents/src/dbus_service.rs:136`**
   ```rust
   let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }.map_err(|e| {
   ```
   * *Finding*: Missing `// SAFETY:` comment. The input `task_json_mut` is constructed from a D-Bus payload. Its structure and safety guarantees are undocumented.

3. **`crates/op-agents/src/agents/orchestration/memory.rs:141`**
   ```rust
   let value: simd_json::OwnedValue =
       unsafe { simd_json::from_str(&mut content_mut).unwrap_or_default() };
   ```
   * *Finding*: Missing `// SAFETY:` comment. Deserializing dynamic persistent cognitive memory in-place without safety documentation.

4. **`crates/op-agents/src/agents/orchestration/memory.rs:239`**
   ```rust
   let old_cache: HashMap<String, String> =
       unsafe { simd_json::from_str(&mut content_mut).unwrap_or_default() };
   ```
   * *Finding*: Missing `// SAFETY:` comment. Migrating legacy data files with unsafe in-place mutation.

5. **`crates/op-agents/src/generator/template.rs:555`**
   ```rust
   let task: {struct_name}Task = match unsafe {{ simd_json::from_str(&mut task_json) }} {{
   ```
   * *Finding*: Missing `// SAFETY:` comment. Code generator templates produce unsafe blocks in target files without generating the corresponding safety comments.

6. **`crates/op-agents/src/security/validation.rs:171`**
   ```rust
   unsafe {
       simd_json::from_str(&mut json_mut)
           .map_err(|_| ValidationError::InvalidPath("Invalid JSON".to_string()))
   }
   ```
   * *Finding*: Missing `// SAFETY:` comment. Low-level JSON input validation does not justify its memory safety invariants.

---

### Command Execution Analysis
A total of **103** command execution sites (`Command::new` or `tokio::process::Command::new`) were identified across the codebase. 

* **Registry Spawn Site**: `crates/op-agents/src/agent_registry.rs:151` uses `tokio::process::Command::new(&spec.command)` to start agent processes dynamically.
* **Domain Agent Spawn Sites**: There are **100** individual `Command::new` calls located within language, database, content, infrastructure, and diagnostic agents (e.g., `git`, `docker`, `terraform`, `gofmt`, `gcc`, etc.).
* **Sandbox & Unified Executors**: **2** instances exist in generic executors (`crates/op-agents/src/security/sandbox.rs:160` and `crates/op-agents/src/unified/execution/base.rs:59`).

**Argument Validation Assessment**: 
While most domain-specific agents pass arguments through `validation::validate_args` or `validation::validate_path` to verify character constraints (preventing shell metacharacter injection), some executing agents split arguments on whitespace without robust context-aware checks.

---

### Forbidden Commands Whitelist

The following forbidden command invocations and whitelist entries were identified:

1. **`crates/op-agents/src/agents/language/bash_pro.rs:22`**
   * *Command*: `Command::new("bash")`
   * *Severity*: **High**
   * *Context*: Invokes `bash` directly with user-controlled script path and split arguments.

2. **`crates/op-agents/src/agents/language/bash_pro.rs:77`**
   * *Command*: `Command::new("bash")`
   * *Severity*: **High**
   * *Context*: Invokes `bash` with syntax-checking arguments (`-n`), presenting a bypass window if paths are manipulated.

3. **`crates/op-agents/src/security/profiles.rs:290`**
   * *Forbidden Whitelist*: `commands.extend(["bash", "sh", "shellcheck"].map(String::from));`
   * *Severity*: **High**
   * *Context*: Explicitly permits raw shell execution engines in the preset security profile.

4. **`crates/op-agents/src/generator/template.rs:188`**
   * *Forbidden Whitelist*: `commands.extend(["bash", "sh", "shellcheck"].map(String::from));`
   * *Severity*: **High**
   * *Context*: Automatically injects forbidden shells into generated agent code whitelist presets.

*Note: No references to `ovs-*` or raw OpenFlow tools were found in the audited files.*

---

### Credentials Scan
* No hardcoded API keys, tokens, or passwords were found in the source code.
* Environment variables `OPENAI_API_KEY` are referenced as configuration requirements but are not hardcoded.

---

### D-Bus Method Exposure Analysis
The D-Bus service wrappers (`crates/op-agents/src/dbus_service.rs`) register on either the Session or System bus. Under the default configuration in `dbus-agent-manager.rs:252`, **System Bus** registration is preferred.

The following methods are exposed on the `org.dbusmcp.Agent` interface:

* **`execute(task_json: String) -> String`**
* **`run_operation(operation: String, path: String, args: String) -> String`**
* `agent_type() -> String`
* `agent_id() -> String`
* `name() -> String`
* `description() -> String`
* `operations() -> Vec<String>`
* `supports_operation(operation: String) -> bool`
* `status() -> String`
* `security_profile() -> String`
* `metadata() -> String`
* `ping() -> bool`

**Security Concern**: These methods are exposed to **any** local system-bus peer without any authorization checks, `polkit` integration, or UID validation.

---

## 3. Vulnerability Findings

### Finding 1: Local Privilege Escalation via Unauthorized D-Bus Method Invocation
* **File & Line**: `crates/op-agents/src/dbus_service.rs:128-186`
* **Severity**: **Critical** (Directly exploitable)
* **Root Cause**: The D-Bus interface `org.dbusmcp.Agent` exposes the `execute` and `run_operation` methods to any peer on the System Bus. No validation of the sender's credentials (such as verifying peer UID via `connection.peer_credentials()`) is performed. In `agent_registry.rs:342-411` (`load_default_specs`), agents like `network` (`dbus-agent-network`), `systemd` (`dbus-agent-systemd`), and `packagekit` (`dbus-agent-packagekit`) are configured with `requires_root: true` and run with elevated system permissions.
* **Exploit Scenario**: An unprivileged local user on the host system sends a D-Bus message to the `org.dbusmcp.Agent.Systemd` service, calling the `run_operation` or `execute` method. Because the systemd agent runs as root to control system services, the unprivileged user can stop, start, or modify system services, fully bypassing local security policies.
* **Remediation**:
  1. Retrieve the connection's sender and query peer credentials using `zbus::Message_Header` or `zbus::Connection::peer_credentials`.
  2. Implement an access control check requiring the calling peer's UID to match `0` (root) or the service's owner UID.
  3. Integrates with `polkit` for granular authorization of high-risk actions.

---

### Finding 2: Path Traversal Bypass via Canonicalization Omission
* **File & Line**: `crates/op-agents/src/security/validation.rs:114-131`
* **Severity**: **High**
* **Root Cause**: The function `validate_path` attempts to prevent path traversal by checking `if path.contains("..")` and rejecting it. However, it does not canonicalize (resolve symlinks or relative references) the path on the actual filesystem before checking prefixes.
* **Exploit Scenario**: If an attacker sets up a symlink inside an allowed directory (e.g., `/tmp/exploit` pointing to `/etc`), passing `/tmp/exploit/passwd` does not contain `..` and starts with the allowed prefix `/tmp`. The function returns `Ok(PathBuf)`, allowing an execution agent to read or write forbidden system configuration files.
* **Remediation**:
  Use `std::fs::canonicalize` on the path before comparing it to the `allowed_dirs` and `forbidden_dirs` prefixes:
  ```rust
  let canonical_path = std::fs::canonicalize(&path_buf)
      .map_err(|e| ValidationError::PathNotAllowed(path_buf.clone()))?;
  ```

---

### Finding 3: Arbitrary Process Spawning via Unvalidated Agent Specifications
* **File & Line**: `crates/op-agents/src/agent_registry.rs:141-171`
* **Severity**: **High**
* **Root Cause**: The `ProcessAgentFactory` instantiates process commands directly from `spec.command` and `spec.args`. These parameters are parsed from configuration files loaded via `load_specs_from_directory` and `load_specs_from_file` without validation against a secure, immutable executable whitelist.
* **Exploit Scenario**: If an attacker gains write access to the directory where agent specifications are stored, they can define a malicious spec with `command: "/bin/bash"` and `args: ["-c", "malicious_payload"]`. The registry will execute this command with the privileges of the Agent Manager upon a `spawn_agent` invocation.
* **Remediation**:
  1. Enforce strict filesystem permissions (e.g., owned by `root`, write-only by `root`) on the specifications directory.
  2. Implement a strict, hardcoded command whitelist in `ProcessAgentFactory` that rejects any binary path outside of authorized utility directories (such as `/usr/libexec/dbus-mcp/`).

---

## 4. Schema-as-Code Compliance Findings

This codebase enforces a strict **Schema-as-Code** discipline utilizing Protocol Buffers and OSCAL to define and version API contracts. Ad-hoc data serialization and hand-rolled structs violate this standard.

### Schema Deviation 1: Hand-Rolled Serialization of Dynamic Specifications
* **File & Line**: `crates/op-agents/src/agent_registry.rs:19-91`
* **Finding**: `AgentSpec` is represented as an ad-hoc Rust struct with custom `serde` attributes. Because these specifications are stored on disk and dynamically loaded, they should be defined as a versioned OSCAL Component Definition or a strict Protobuf schema to ensure backward compatibility and machine-readable compliance parsing.

### Schema Deviation 2: Unversioned JSON Execution Tasks and Results
* **File & Line**: `crates/op-agents/src/agents/base.rs:12-96`
* **Finding**: The `AgentTask` and `TaskResult` structs define the core input/output contract for the entire agent subsystem. However, they use an ad-hoc schema with a loosely typed `config: HashMap<String, simd_json::OwnedValue>` and `data: String` payload. These should be defined as formal, versioned Protobuf messages to prevent interface drift across the D-Bus boundary.

### Schema Deviation 3: Loosely-Typed Unified Request/Response Payloads
* **File & Line**: `crates/op-agents/src/unified/agent_trait.rs:43-85`
* **Finding**: The unified agents API utilizes `args: Value` and `data: Value` (untyped JSON objects) for operations. This shifts schema validation from compile-time contracts to runtime handling, violating the safety guarantees of a schema-as-code discipline.

---

## 5. Code Quality & Maintainability Suggestions

### Deviation 1: Blocking Code in Asynchronous Context
* **File & Line**: `crates/op-agents/src/agents/orchestration/memory.rs:88-93`
* **Context**: `fs::read_to_string` and `fs::write` are called synchronously inside `MemoryAgent::new` and `MemoryAgent::persist`. 
* **Impact**: Since these methods are executed within an asynchronous tokio runtime context, synchronous I/O blocks the thread executor, degrading overall system concurrency and increasing latency for unrelated tasks.
* **Correction**: Refactor all synchronous filesystem calls inside `MemoryAgent` to use `tokio::fs::read_to_string` and `tokio::fs::write`.

### Deviation 2: Missing Error Propagation during Memory Persistence
* **File & Line**: `crates/op-agents/src/agents/orchestration/memory.rs:320`
* **Context**: In `MemoryAgent::recall`, the persistence operation is called and its result is ignored: `let _ = self.persist();`.
* **Impact**: If disk write operations fail (e.g., due to out-of-space or permission errors), access counts and timestamps will get out of sync silently without triggering any alerts or diagnostics.
* **Correction**: Log persistence failures using `tracing::error!` rather than discarding them with `let _`.