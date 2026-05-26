# Production Security and Quality Audit: op-agents

---

## 1. Build & Codegen Security Assessment

### Edition & Rust Version
* **Edition:** Crate `crates/op-agents/Cargo.toml` inherits the workspace edition via `edition.workspace = true`. The workspace `Cargo.toml` specifies `edition = "2021"`.
* **Rust Version:** Neither `crates/op-agents/Cargo.toml` nor the workspace `Cargo.toml` defines a minimum supported Rust version (`rust-version`). 

### Binaries and Examples
* **Binaries:** The crate defines two binaries:
  * `dbus-agent` (path: `src/bin/dbus-agent.rs`)
  * `op-agent-manager` (path: `src/bin/dbus-agent-manager.rs`)
* **Examples:** No examples are defined in the workspace or crate-level `Cargo.toml`.

### Workspace Inheritance vs. Local Overrides
Workspace inheritance is extensively utilized to maintain consistency across theControl Plane. Key fields such as `version`, `edition`, `authors`, and `license` are inherited. Dependencies like `tokio`, `zbus`, `axum`, `serde`, `simd-json`, `anyhow`, and `thiserror` are declared in the workspace root and inherited via `{ workspace = true }`. The only crate-local dependency overrides are:
* `shell-words = "1.1"` in `crates/op-agents/Cargo.toml`

### Code Generation & Build Script Risks
* **`build.rs` Analysis:** There is **no `build.rs`** file declared or provided for the `op-agents` crate. Thus, there are no immediate risks of arbitrary shell execution during compilation inside this specific crate.

### Schema-as-Code Build Check
* **gRPC/Protobuf Compilation:** `op-agents` does not invoke `prost-build` or `tonic-build` directly. However, the workspace dependencies manifest references to `prost` and `tonic-build` (for other crates like `op-grpc-bridge`, `op-chat`, and `op-cognitive-mcp`). 
* **Source of Truth Check:** No `.proto` schemas are checked into `crates/op-agents`.
* **Committed Generated Files:** No generated Rust files from Protobuf are committed in `crates/op-agents`.
* **Runtime Code Generation Risk:** Code generation from Markdown definitions is supported at runtime. The parser `crates/op-agents/src/generator/md_parser.rs` and the template engine `crates/op-agents/src/generator/template.rs` are compiled into the runtime targets, allowing dynamic parsing of arbitrary markdown files and code generation on the fly. This introduces security risks if agent markdown definitions are modifiable by unprivileged users.

---

## 2. Critical Security Findings

### CRITICAL: Path Traversal Vulnerability in Legacy Validation Module Allows Arbitrary Host File Read and Hijacking
* **File:** `crates/op-agents/src/agents/base.rs:274-293`
* **Other Affected Files:** 
  * `crates/op-agents/src/agents/analysis/code_reviewer.rs:44,57,78`
  * `crates/op-agents/src/agents/analysis/debugger.rs:34`
  * `crates/op-agents/src/agents/database/database_architect.rs:32,50,68`
  * `crates/op-agents/src/agents/database/database_optimizer.rs:32,53,71`
  * `crates/op-agents/src/agents/database/sql_pro.rs:32,55,71`
  * `crates/op-agents/src/agents/infrastructure/deployment.rs:39,53,72`
  * `crates/op-agents/src/agents/infrastructure/terraform.rs:30,48,66,84`
  * `crates/op-agents/src/agents/language/bash_pro.rs:30,52,74`
  * `crates/op-agents/src/agents/language/python_pro.rs:38` (and other language agents using `base::validation`)

#### Technical Analysis
The legacy path validation function defined in `crates/op-agents/src/agents/base.rs` checks only if the user-supplied string starts with an allowed directory (such as `/tmp`), but fails to perform canonicalization or block path traversal patterns (`..` sequences):

```rust
    pub fn validate_path(path: &str, allowed_dirs: &[&str]) -> Result<String, String> {
        ...
        let is_allowed = allowed_dirs.iter().any(|dir| path.starts_with(dir));
        if !is_allowed {
            return Err(format!("Path must be within allowed directories: {:?}", allowed_dirs));
        }
        Ok(path.to_string())
    }
```

Because `"/tmp/../etc/passwd"` starts with `"/tmp"`, this validation succeeds. Because the legacy agents in `crates/op-agents/src/agents/` import this specific validator (`use crate::agents::base::{validation, ...}`) instead of the secure one in `crates/op-agents/src/security/validation.rs` (which explicitly rejects `..`), any user or LLM with access to the agent endpoints can read any file on the system.

#### Exploit Proof of Concept
An attacker invokes the `DebuggerAgent` (which runs outside the sandbox with read-only host privileges) via the D-Bus or HTTP API with the following task payload:
```json
{
  "type": "debugger",
  "operation": "logs",
  "path": "/tmp/../etc/passwd"
}
```
The agent executes:
```rust
let validated_path = validation::validate_path(file, ALLOWED_DIRS)?; // Resolves to "/tmp/../etc/passwd"
let mut cmd = Command::new("tail");
cmd.arg("-n").arg("100").arg(validated_path);
```
This prints the contents of `/etc/passwd`. If the agent manager is running as root (as is common for D-Bus system bus services), `/etc/shadow` can be read similarly.

---

### CRITICAL: Arbitrary Command Execution on Host via `-exec` Argument Injection in `GoExecutor` and `GolangProAgent`
* **File:** `crates/op-agents/src/unified/execution/golang.rs:51-78`
* **Other Affected Files:** `crates/op-agents/src/agents/language/golang_pro.rs:163-195`

#### Technical Analysis
The `GoExecutor` and `GolangProAgent` commands split the user-controlled `args` string on whitespace and append the individual parts as separate arguments to the `go` command without ensuring that flags starting with `-` are rejected:

```rust
        if let Some(a) = args {
            validation::validate_args(a)?;
            for arg in a.split_whitespace() {
                cmd.arg(arg);
            }
        }
```

The validation function `validate_args` checks against `FORBIDDEN_CHARS`, which consists of `['$', '` ', ';', '&', '|', '>', '<', '(', ')', '{', '}', '\n', '\r']`. It does not restrict `-` or flags. 
The standard `go run` utility supports the `-exec` option, which specifies a wrapper program to invoke the binary. For example, `go run . -exec id` instructs the Go utility to execute the command `id` on the host system. Since none of the characters in `-exec id` are forbidden, the validation succeeds and the command is executed outside the sandbox on the host system.

#### Exploit Proof of Concept
An attacker triggers the `run` operation on the `GoExecutor` with:
* `path`: `"."`
* `args`: `"-exec id"`

The command executed on the host is:
```bash
go run . -exec id
```
This triggers the immediate execution of `id` with the privileges of the `dbus-agent` service on the host, completely bypassing the sandbox.

---

### CRITICAL: Arbitrary Command Execution on Host via `-wrapper` Argument Injection in C/C++ Agents
* **File:** `crates/op-agents/src/agents/language/c_pro.rs:29-57`
* **Other Affected Files:** `crates/op-agents/src/agents/language/cpp_pro.rs:29-54`

#### Technical Analysis
Similar to the Go executor, the `CProAgent` (`gcc_compile`) and `CppProAgent` (`gpp_compile`) append user-supplied compilation arguments directly to `gcc` or `g++` after whitespace-splitting. 
The `gcc` driver supports a `-wrapper` flag, which directs the compiler to run all subprocesses (preprocessor, compiler, assembler, linker) through a specified wrapper program. By passing `-wrapper id`, the user can force `gcc` to execute `id`. This flag contains no characters in `FORBIDDEN_CHARS` and easily bypasses all security constraints.

#### Exploit Proof of Concept
An attacker calls the `compile` operation of `c-pro` with:
* `path`: `"/tmp/test.c"`
* `args`: `"-wrapper id"`

The final executed command is:
```bash
gcc -Wall -Wextra /tmp/test.c -wrapper id
```
This executes `id` directly on the host, escaping the sandbox entirely.

---

### CRITICAL: Arbitrary Command Execution on Host via `--ext-diff` Argument Injection in `CodeReviewerAgent`
* **File:** `crates/op-agents/src/agents/analysis/code_reviewer.rs:64-83`

#### Technical Analysis
The `git_diff` function in `CodeReviewerAgent` appends user-controlled arguments to `git diff`:

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

The `git diff` command supports the `--ext-diff=<command>` (or `-x <command>`) flag to use an external program to generate the diff. Since no flags starting with `-` are restricted by `validate_args`, an attacker can pass `--ext-diff=id`.

#### Exploit Proof of Concept
An attacker calls the `diff` operation of `code-reviewer` with:
* `args`: `"--ext-diff=id"`

`git` will execute the `id` command on the host.

---

## 3. Schema-as-Code & Quality Findings

### SCHEMA-AS-CODE: Ad-Hoc Structs and Raw JSON Passing Over IPC/D-Bus
* **File:** `crates/op-agents/src/dbus_service.rs:118-124`
* **Other Affected Files:** 
  * `crates/op-agents/src/agents/base.rs:14-29`
  * `crates/op-agents/src/unified/agent_trait.rs:56-62`
  * `crates/op-agents/src/agent_registry.rs:24-64`

#### Technical Analysis
Rather than enforcing strongly typed, versioned schemas (such as Protocol Buffers or OSCAL profiles) at the interface boundary, the D-Bus service exposes an `execute` method that accepts and returns raw, unversioned `String` payloads containing ad-hoc JSON:

```rust
    async fn execute(&self, task_json: String) -> Result<String, zbus::fdo::Error> {
```

These payloads are parsed into ad-hoc Rust structs (`AgentTask`, `AgentRequest`, `AgentSpec`) at runtime using `simd_json`. This approach lacks compile-time safety across IPC boundaries, lacks schema versioning, and makes API contracts fragile and prone to deserialization mismatches. It directly violates the schema-as-code discipline.

#### Recommendation
Define all IPC structures, tasks, and agents specs as versioned Protocol Buffers inside `.proto` files, compiled via `prost-build`/`tonic-build` at build time. Expose these typed parameters over the D-Bus/gRPC interface instead of passing generic JSON strings.

---

### SECURITY & QUALITY: Risky Use of `unsafe` Deserialization in simd-json
* **File:** `crates/op-agents/src/agent_registry.rs:242`
* **Other Affected Files:** 
  * `crates/op-agents/src/dbus_service.rs:125`
  * `crates/op-agents/src/generator/template.rs:258`

#### Technical Analysis
The codebase frequently deserializes JSON string payloads using `unsafe { simd_json::from_str(&mut content) }`. While `simd_json` is highly efficient, its `from_str` method is marked `unsafe` because it mutates the input buffer in-place. If any references borrow data from the mutated string and outlive it, it results in undefined behavior (use-after-free or dangling pointers). 

In `agent_registry.rs:242`, the deserialized `AgentSpec` struct contains fully owned `String` and `Vec<String>` types, which prevents immediate borrowing issues. However, utilizing `unsafe` deserialization globally without clear safety comments, invariants, or compiler checks represents a major memory safety risk for a security-critical control plane.

#### Recommendation
Transition to the safe API wrapper `simd_json::serde::from_slice` or document the safety invariants of every `unsafe` deserialization block explicitly.

---
## ⚠ Citation Warnings
- `crates/op-agents/src/agents/base.rs:274`: file has 255 lines
