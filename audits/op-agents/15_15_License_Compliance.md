# License Audit & Compliance

### 1. Extracted Workspace License
* **Workspace License**: `Apache-2.0` (declared in `Cargo.toml:43`)
* **`op-agents` License**: Inherits `Apache-2.0` from the workspace (declared in `crates/op-agents/Cargo.toml:6` via `license.workspace = true`)

### 2. GPL/AGPL/SSPL Crates Scan
* A scan of the provided `Cargo.lock` shows **no** GPL, AGPL, or SSPL licensed crates. 
* *Note on Copyleft Dependencies*: The workspace utilizes `cozo` version `0.7.6` (`Cargo.lock:327`), which is licensed under the **MPL-2.0** (Mozilla Public License 2.0). MPL-2.0 is a weak copyleft license and is compatible with Apache-2.0, meaning it does not force the rest of the proprietary or Apache-licensed codebase to be licensed under MPL-2.0, provided any modifications to Cozo source files themselves remain MPL-2.0.

### 3. Crates with No License Field
* All workspace-level and internal dependency definitions within the provided files contain correct licensing attributes. No external crates listed in the visible portions of `Cargo.toml` or `Cargo.lock` are missing license metadata.

---

# Security & Quality Vulnerability Audit

## [CRITICAL] Complete Host Sandbox Bypass via direct `std::process::Command` usage
* **File**: `crates/op-agents/src/agents/analysis/code_reviewer.rs:27` (and globally across all execution/analysis agents including `debugger.rs`, `performance.rs`, `security_auditor.rs`, and language-specific agents in `crates/op-agents/src/agents/language/`)
* **Vulnerability Type**: Security Sandbox Bypass
* **Exploitability**: Directly Exploitable

### Description
The codebase implements a robust `SandboxExecutor` in `crates/op-agents/src/security/sandbox.rs` designed to enforce memory limits, CPU timeouts, and process isolation. However, **almost all built-in agents completely bypass this security container**. 

Instead of routing execution tasks through the `SandboxExecutor` or using the `AgentContext` execution wrapper, the agents directly instantiate and execute commands on the host via `std::process::Command::new(...)`. For example, in `crates/op-agents/src/agents/analysis/code_reviewer.rs`:

```rust
fn search_code(&self, path: Option<&str>, pattern: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new("rg"); // Directly spawns on host!
    ...
    let output = cmd.output().map_err(|e| format!("Failed: {}", e))?;
```

Because these commands run directly under the privilege level of the D-Bus Agent Manager process on the host system without any container boundaries, resource constraints, or environment sanitization, this completely invalidates the security posture of the platform.

### Remediation
Refactor all built-in agent implementations to strictly execute host commands via `SandboxExecutor::execute` instead of directly invoking raw `std::process::Command`.

---

## [CRITICAL] Arbitrary Command Execution via Git Diff Argument Injection
* **File**: `crates/op-agents/src/agents/analysis/code_reviewer.rs:48`
* **Vulnerability Type**: Argument Injection / Remote Code Execution (RCE)
* **Exploitability**: Directly Exploitable

### Description
The `git_diff` operation in `CodeReviewerAgent` accepts arbitrary user-controlled arguments, parses them using a simple whitespace-split, and passes them as arguments to `git diff`:

```rust
fn git_diff(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("diff");

    if let Some(a) = args {
        validation::validate_args(a)?;
        for arg in a.split_whitespace() {
            cmd.arg(arg);
        }
    }
```

The validation layer (`crates/op-agents/src/security/validation.rs:136`) checks for shell metacharacters (such as `;`, `&`, `|`, `` ` ``, etc.), but does **not** validate or restrict argument flags. 

By passing the argument `--ext-diff=<executable>`, an attacker can instruct `git diff` to invoke an arbitrary host program (e.g. `--ext-diff=uname`) to process the diff. Because this argument contains none of the forbidden characters, it completely bypasses `validation::validate_args` and achieves arbitrary command execution on the host machine.

### Remediation
Do not allow users to pass arbitrary raw arguments (`args`) directly to complex CLI tools like `git`. Whitelist only safe, specific flags (such as `--stat` or `--staged`), or parse the arguments using a strict, secure parser that rejects flags starting with `--ext-diff` or other unsafe options.

---

## [HIGH] Undefined Behavior / Out-of-Bounds Memory Read via Unpadded `simd_json` Parsing
* **Files**: 
  * `crates/op-agents/src/agent_registry.rs:260`
  * `crates/op-agents/src/dbus_service.rs:136`
  * `crates/op-agents/src/agents/orchestration/memory.rs:188`
  * `crates/op-agents/src/security/validation.rs:172`
* **Vulnerability Type**: Undefined Behavior / Memory Corruption

### Description
The `simd-json` crate relies on high-performance SIMD instructions to parse JSON. For safety, `simd-json` strictly requires that the input buffer has trailing padding of at least `simd_json::SIMD_JSON_PADDING` bytes (typically 64 bytes) so that vectorized 32-byte or 64-byte reads do not read past the allocated buffer. 

Across multiple files in `op-agents`, the code performs unsafe JSON deserialization on unpadded, standard `String` structures that are converted directly from raw text or D-Bus inputs:

```rust
// crates/op-agents/src/dbus_service.rs:136
let mut task_json_mut = task_json.to_string(); // Standard unpadded String
let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }...
```

Since the standard `String` allocator does not guarantee the necessary trailing padding, vectorized instructions during parsing can read out-of-bounds. This results in **undefined behavior**, memory disclosure, or intermittent segmentation faults.

### Remediation
Ensure that all strings parsed with `simd_json` are correctly padded before calling the parser. Use `simd_json::to_padded_bin` or pad the input vector manually:

```rust
let mut padded = task_json.into_bytes();
padded.resize(padded.len() + simd_json::SIMD_JSON_PADDING, 0);
let task: AgentTask = unsafe { simd_json::from_slice(&mut padded) }?;
```

---

# Schema-As-Code Violations

The architecture enforces a strict schema-as-code discipline using Protocol Buffers and OSCAL. The following data contracts are expressed as ad-hoc Rust structs and unstructured JSON strings rather than versioned Protobuf schemas:

| File | Line Reference | Ad-Hoc Data Contract | Recommended Remediation |
| :--- | :--- | :--- | :--- |
| `crates/op-agents/src/agent_registry.rs` | `18` | `AgentSpec` Struct | Re-define as a versioned Protobuf message or an OSCAL Component Definition. |
| `crates/op-agents/src/agents/base.rs` | `12` | `AgentTask` Struct | Re-define as a versioned Protobuf schema for task transport. |
| `crates/op-agents/src/agents/base.rs` | `52` | `TaskResult` Struct | Re-define as a versioned Protobuf schema to enforce structured outputs. |
| `crates/op-agents/src/unified/agent_trait.rs` | `53` | `AgentRequest` Struct | Enforce serialization to/from a common schema model. |
| `crates/op-agents/src/unified/agent_trait.rs` | `77` | `AgentResponse` Struct | Enforce serialization to/from a common schema model. |