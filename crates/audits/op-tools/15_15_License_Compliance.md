# Production Security, Quality, and License Audit: `op-tools`

## 1. Executive Summary

This codebase implements a high-performance tool registry and execution engine designed for the `op-dbus-v2` control plane. It exposes deep system administrative functions (such as D-Bus method execution, Open vSwitch configuration, systemd service management, and self-repository modification) to an orchestrator system.

While the engineering intent is to support native system APIs over insecure CLI wrappers, the execution of this model contains multiple **critical, directly exploitable security vulnerabilities**. These vulnerabilities bypass access level controls, permit arbitrary shell command execution even in `Restricted` mode, allow arbitrary file reading/writing outside of sandbox directories, and expose the host system to memory corruption and undefined behavior via improper use of `unsafe` parsing APIs.

---

## 2. License Auditing

### 2.1. Cargo.toml License Extraction
*   **Workspace License**: The root `Cargo.toml` defines `license = "Apache-2.0"` under its `[workspace.package]` section.
*   **Crate License**: `crates/op-tools/Cargo.toml` defines `license.workspace = true` (inheriting `Apache-2.0`).

### 2.2. GPL/AGPL/SSPL Crate Scan
A scan of `Cargo.lock` reveals no dependencies licensed under copyleft GPL, AGPL, or SSPL licenses. Standard permissive dependencies (`MIT`, `Apache-2.0`, `BSD-3-Clause`) predominate. 

### 2.3. Crates with No License Field
All internal crates defined in the workspace (`crates/*`) correctly utilize the inherited workspace package metadata `license.workspace = true`, meaning no internal crates are missing license fields.

---

## 3. Critical Security Vulnerabilities

### 3.1. Arbitrary File Write Outside Self-Repo via Non-Existent Path Traversal
*   **Vulnerability Type**: Path Traversal (Arbitrary File Write / Code Execution)
*   **Location**: `crates/op-tools/src/builtin/self_tools.rs:45-48`
*   **Impact**: Critical (Remote Code Execution / Privilege Escalation)

#### Description
The helper function `validate_self_path` is designed to prevent path traversal outside of the self-repository directory (`OP_SELF_REPO_PATH`). It attempts to canonicalize the path first, but falls back to the uncanonicalized path if the file does not yet exist:

```rust
// crates/op-tools/src/builtin/self_tools.rs:45-48
let canonical = full_path.canonicalize().unwrap_or_else(|_| full_path.clone());

// Ensure it's still within the repo
if !canonical.starts_with(&repo_path) {
```

When creating a *new* file via the `self_write_file` tool, the file does not yet exist on disk, so `canonicalize()` fails and returns `full_path.clone()`. 

Rust’s `Path::starts_with` does a component-by-component match and does **not** resolve `..` (parent directory) components semantically. Therefore, an input path like `crates/../../../../etc/cron.d/malicious_job` will resolve to:
`[repo_path]/crates/../../../../etc/cron.d/malicious_job`

Since the first components match `repo_path`, `starts_with` evaluates to `true`. When `tokio::fs::write` is subsequently executed, the OS resolves the `..` components, writing the file entirely outside the repository.

#### Proof of Concept
An attacker sends a payload to `SelfWriteFileTool`:
```json
{
  "path": "crates/../../../etc/cron.d/exploit",
  "content": "* * * * * root /usr/bin/reverse_shell\n"
}
```
`canonicalize()` fails (as `/etc/cron.d/exploit` does not yet exist), `starts_with` passes, and the cron job is written, executing arbitrary commands as root.

---

### 3.2. Sandbox Escape via Symlink Traversal
*   **Vulnerability Type**: Symlink Traversal (Arbitrary File Read/Write)
*   **Location**: `crates/op-tools/src/security.rs:320-330` & `351-360`
*   **Impact**: High/Critical (Data Leakage, Sandbox Escape)

#### Description
The `validate_read_path` and `validate_write_path` functions in the `SecurityValidator` attempt to restrict restricted/untrusted users to specific safe directories (such as `/tmp`):

```rust
// crates/op-tools/src/security.rs:320-324
pub async fn validate_read_path(&self, path: &str) -> Result<PathBuf, SecurityError> {
    let profile = self.profile.read().await;
    let path_buf = PathBuf::from(path);

    // Check for path traversal
    if path.contains("..") {
        return Err(SecurityError::PathTraversal(path.to_string()));
    }
```

This validation relies exclusively on string checks (ensuring the input doesn't contain `..` and starts with `/tmp`). It completely fails to canonicalize symlinks.

Because `validate_write_path` permits unrestricted writing within `/tmp` (line 369), a restricted user or compromised agent can write a symbolic link pointing to a sensitive system file (e.g., `/etc/shadow`) and then read it through the `file_read` tool.

#### Proof of Concept
1.  User writes a symbolic link:
    `Command: ln -s /etc/shadow /tmp/victim_link`
2.  User calls `file_read` with `path: "/tmp/victim_link"`.
3.  The validator checks `/tmp/victim_link`:
    *   Does not contain `..` (Passed)
    *   Starts with `/tmp` (Passed)
4.  `tokio::fs::read_to_string("/tmp/victim_link")` executes, resolving the symlink and returning `/etc/shadow` contents.

---

### 3.3. Command Injection and Sandbox Bypass in Restricted Mode
*   **Vulnerability Type**: Command Injection / Access Control Bypass
*   **Location**: `crates/op-tools/src/security.rs:260-281`
*   **Impact**: Critical (Arbitrary Command Execution as Root)

#### Description
When the `SecurityValidator` is configured in `Restricted` or `Custom` mode, it validates shell commands against a whitelist using the following logic:

```rust
// crates/op-tools/src/security.rs:267-270
let base_cmd = command
    .split_whitespace()
    .next()
    .ok_or_else(|| SecurityError::ValidationFailed("Empty command".to_string()))?;
```

It only extracts and validates the first word of the command string against the whitelist. The entire unvalidated `command` string is then executed directly via `bash -c` in `crates/op-tools/src/builtin/shell.rs:136`.

This allows restricted users to execute arbitrary commands by appending shell metacharacters (such as `;`, `|`, `&`, or backticks) after a whitelisted command.

#### Proof of Concept
A restricted session executes the following payload via the `shell_execute` tool:
```json
{
  "command": "cat /dev/null; rm -rf /etc/shadow",
  "session_id": "untrusted_user"
}
```
*   `base_cmd` is extracted as `"cat"` (which is whitelisted).
*   The validator approves the execution.
*   Bash executes: `bash -c "cat /dev/null; rm -rf /etc/shadow"`, successfully deleting the target system file.

---

### 3.4. Insecure Arguments Formatting in Legacy ShellTool
*   **Vulnerability Type**: OS Command Injection
*   **Location**: `crates/op-tools/src/builtin_old.rs:204`
*   **Impact**: Critical (Remote Code Execution)

#### Description
The legacy `ShellTool` validates the `command` argument but completely fails to sanitize the companion `args` array:

```rust
// crates/op-tools/src/builtin_old.rs:201-204
match tokio::process::Command::new("sh")
    .arg("-c")
    .arg(format!("{} {}", command, args.join(" ")))
```

An attacker can provide a valid whitelisted command (e.g. `cat`) and inject arbitrary shell operators inside the `args` array. Because they are concatenated into a single string executed by `sh -c`, the shell interprets the metacharacters, permitting arbitrary system command execution.

#### Proof of Concept
```json
{
  "command": "cat",
  "args": ["/dev/null", ";", "id", ";", "whoami"]
}
```
This formats to: `sh -c "cat /dev/null ; id ; whoami"`, resulting in arbitrary command execution.

---

### 3.5. Undefined Behavior and Memory Safety Violations in `simd_json`
*   **Vulnerability Type**: Undefined Behavior / Out-of-Bounds Read
*   **Location**: Multiple files (e.g., `crates/op-tools/src/mcptools.rs:168`, `175`, `189`, `228`, `281`)
*   **Impact**: Medium (Process Crash / Heap Information Disclosure)

#### Description
The codebase contains numerous calls to `unsafe { simd_json::from_str(...) }` on standard Rust `String` buffers loaded from the environment, system files, or process stdout:

```rust
// crates/op-tools/src/mcptools.rs:189
let mut config: McpToolsConfig = unsafe { simd_json::from_str(&mut raw) }
```

The `simd-json` crate is optimized for high-performance parsing using SIMD instructions. As a strict precondition, **the input buffer must contain padding bytes** (specifically `simd_json::SIMDJSON_PADDING`, which is typically 32 or 64 bytes) beyond the end of the logical string. 

Passing standard unpadded Rust `String` allocations (such as those returned by `std::fs::read_to_string` or `std::env::var`) directly into `from_str` violates this invariant. The SIMD operations will read past the allocated buffer bounds, resulting in undefined behavior, segment faults, or heap memory leaks.

---

## 4. Schema-As-Code Violations

The codebase is built on an ad-hoc, untyped contract model. Instead of relying on compiled, versioned schemas (such as Protocol Buffers or OSCAL schemas), the tool registry defines input schemas and communications using dynamic inline JSON constructs (`simd_json::json!`).

Specific occurrences include:

*   **`crates/op-tools/src/builtin_old.rs:19`**: `EchoTool` input schema defined as an ad-hoc inline JSON structure.
*   **`crates/op-tools/src/builtin_old.rs:58`**: `SystemInfoTool` ad-hoc schema.
*   **`crates/op-tools/src/builtin_old.rs:144`**: `ShellTool` ad-hoc schema.
*   **`crates/op-tools/src/builtin_old.rs:235`**: `FileReadTool` ad-hoc schema.
*   **`crates/op-tools/src/dynamic_tool.rs:81`**: `DynamicDbusTool` dynamically constructs an ad-hoc schema using string inserts.
*   **`crates/op-tools/src/builtin/dbus.rs:26, 85, 144, 201, 260`**: Standard systemd D-Bus tool configurations rely entirely on untyped, inline schemas.
*   **`crates/op-tools/src/builtin/incus_tools.rs:114, 161, 214, 281, 331, 381, 467`**: Instance management tools define their contracts as inline JSON structures rather than importing versioned schema definitions.

---

## 5. Security & Quality Audit Findings

### 5.1. [Critical] Arbitrary File Write via Self-Repo Validation Bypass
*   **File**: `crates/op-tools/src/builtin/self_tools.rs`
*   **Lines**: 45-48
*   **CWE**: CWE-22 (Path Traversal)
*   **Remediation**: Never use `starts_with` to validate paths containing `..` without first successfully resolving them. If the path does not exist, use a dedicated path normalization function (e.g. `lexical_core` or custom resolution) to resolve `..` components in memory before performing prefix checks, or verify that the parent directory can be canonicalized.

### 5.2. [Critical] Sandbox Escape via Symlink Traversal
*   **File**: `crates/op-tools/src/security.rs`
*   **Lines**: 320-330, 351-360
*   **CWE**: CWE-59 (Link Resolution before File Access)
*   **Remediation**: Before checking path containment, canonicalize the path with `std::fs::canonicalize`. If verifying a write path where the file does not yet exist, canonicalize its parent directory.

### 5.3. [Critical] Shell Command Injection via Restricted Semicolon/Pipe Traversal
*   **File**: `crates/op-tools/src/security.rs`
*   **Lines**: 267-270
*   **CWE**: CWE-78 (OS Command Injection)
*   **Remediation**: Avoid passing unstructured strings to `bash -c`. If shell command execution is required, parse the command string into structured arguments, or strictly validate the *entire* string for shell metacharacters (`;`, `&`, `|`, `` ` ``, `$`, `\n`) before execution.

### 5.4. [Critical] OS Command Injection in Legacy ShellTool
*   **File**: `crates/op-tools/src/builtin_old.rs`
*   **Lines**: 204
*   **CWE**: CWE-88 (Argument Injection)
*   **Remediation**: Do not concatenate user arguments into a formatted shell string. Execute commands by passing arguments as discrete array slices (`Command::new(cmd).args(args)`) rather than running `sh -c`.

### 5.5. [High] Undefined Behavior via Unpadded `simd_json::from_str`
*   **File**: `crates/op-tools/src/mcptools.rs` & `crates/op-tools/src/builtin/agent_tool.rs`
*   **Lines**: `mcptools.rs:168`, `mcptools.rs:189`, `agent_tool.rs:341`, `agent_tool.rs:556`
*   **CWE**: CWE-125 (Out-of-bounds Read)
*   **Remediation**: Convert strings to `Vec<u8>`, append padding using `simd_json::padded_free` or use `simd_json::to_owned_value` / `simd_json::serde::from_slice` which automatically manage safe internal buffers.

### 5.6. [Medium] Insecure Parallel Shell Execute Implementations (Security Bypass)
*   **File**: `crates/op-tools/src/builtin/shell_tool.rs`
*   **Lines**: 34-45
*   **CWE**: CWE-1038 (Bypass of Security Stage)
*   **Remediation**: `crates/op-tools/src/builtin/shell_tool.rs` contains a redundant, unprotected implementation of `ShellExecuteTool` that ignores `SecurityValidator` entirely. Delete this file and reference the validated implementation in `crates/op-tools/src/builtin/shell.rs`.

### 5.7. [Low] Ad-Hoc Dynamic Data Contracts (Schema-As-Code Violations)
*   **File**: Throughout entire codebase (e.g. `crates/op-tools/src/builtin/dbus.rs`, `crates/op-tools/src/builtin/incus_tools.rs`)
*   **Lines**: Various
*   **CWE**: CWE-1153 (Ad-hoc Schema/Contract Usage)
*   **Remediation**: Align with schema-as-code principles. Define all tool interaction schemas as versioned Protobuf or JSON schema documents managed within a central repository schema workspace. Load these schemas statically at compilation or dynamically via verified runtime registries rather than constructing dynamic schemas inline using untyped JSON macros.