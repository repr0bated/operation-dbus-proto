### Critical Vulnerabilities

#### Path Traversal via Unvalidated Paths in `ReadFileTool`
*   **File & Line**: `crates/op-chat/src/tool_loader.rs:290-302`
*   **Description**: `ReadFileTool::execute` attempts to prevent reading sensitive files by checking if `path.starts_with(p)` against a blacklist of forbidden paths (e.g., `["/etc/shadow", "/etc/sudoers"]`). However, it fails to canonicalize the path first. An attacker or a compromised LLM can completely bypass this check using standard directory traversal sequences (e.g., `path = "/tmp/../etc/shadow"` or `path = "/etc/./shadow"`), allowing arbitrary system file disclosure.
*   **Impact**: Compromise of system credentials and configuration files.

#### Path Traversal and Arbitrary File Write in `WriteFileTool`
*   **File & Line**: `crates/op-chat/src/tool_loader.rs:349-364`
*   **Description**: Similar to the read tool, `WriteFileTool::execute` performs a simple `path.starts_with(p)` validation against system directories like `/etc/`. Without path canonicalization, an attacker can write arbitrary files to restricted directories (e.g., writing to `/tmp/../etc/cron.d/malicious_job`).
*   **Impact**: Direct Remote Code Execution (RCE) with the privileges of the control plane process.

#### Command Execution via Dangerous Whitelisted Binaries in `ShellExecuteTool`
*   **File & Line**: `crates/op-chat/src/tool_loader.rs:442-491`
*   **Description**: `ShellExecuteTool` defines a command whitelist designed to allow only "safe, read-mostly commands." However, this whitelist contains highly dangerous executables such as `"python"`, `"python3"`, `"pip"`, `"npm"`, `"git"`, and `"find"`. These binaries natively support executing arbitrary code/processes (e.g., `python -c "import os; os.system(...)"` or `find . -exec ...`). The whitelist provides no real security isolation.
*   **Impact**: Trivial bypass of execution restrictions, allowing arbitrary command execution on the host.

#### JSON Injection via Raw String Manipulation in Workflow Templating
*   **File & Line**: `crates/op-chat/src/orchestrated_executor.rs:524-533`
*   **Description**: The template resolution logic serializes the input JSON `Value` into a raw string, performs a raw substring replacement for placeholders such as `$context.key`, and then deserializes the modified string back into JSON. If a variable contains structural JSON characters (e.g., `value = "foo\", \"injected_key\": \"injected_value\""`), it mutates the JSON object structure during deserialization.
*   **Impact**: Injection of arbitrary parameters and keys into subsequent tool execution steps, bypassing schema validation and leading to unauthorized operations.

---

### High Severity (Compilation Blockers)

#### Duplicate Function Definition of `register_tool`
*   **File & Line**: `crates/op-chat/src/tool_loader.rs:45`
*   **Description**: The helper function `register_tool` is defined twice in the same scope (lines 28-39 and lines 45-56) with identical signatures. This results in a duplicate definition compilation error.

#### Unresolved Identifier `args` in `parse_explicit_tool_invocation`
*   **File & Line**: `crates/op-chat/src/hybrid_executor.rs:124`
*   **Description**: In `parse_explicit_tool_invocation`, the expression inside the `if` statement evaluates the JSON but is never assigned to a variable named `args`. The return statement on line 124 attempts to reference `args` which does not exist in scope.

#### Attempt to Borrow Temporary Value as Mutable
*   **File & Lines**: 
    *   `crates/op-chat/src/hybrid_executor.rs:133`
    *   `crates/op-chat/src/nl_admin.rs:166`
    *   `crates/op-chat/src/nl_admin.rs:206`
    *   `crates/op-chat/src/forced_execution.rs:310`
*   **Description**: The code attempts to pass mutable references of temporary values (e.g., `&mut parts[1].to_string()` and `&mut args_str.to_string()`) to `simd_json::from_str`. Rust does not allow borrowing temporary Rvalues as mutable. The values must be bound to a local `mut` variable before taking a mutable reference.

#### Type Mismatch in `Value::Object` Instantiation
*   **File & Line**: `crates/op-chat/src/intent_executor.rs:431`
*   **Description**: The `Value::Object` variant in the used `simd-json` version expects a boxed `Object` (`Box<Object>`). The code attempts to instantiate it directly as `Value::Object(intent.params.clone().into_iter().collect())` without wrapping it in a `Box::new()`.

#### Missing Scope for `PluginServiceClient`
*   **File & Line**: `crates/op-chat/src/grpc_client.rs:140`
*   **Description**: `PluginServiceClient` is instantiated in the `connect` method but is only imported locally inside the `execute` method (line 266). It is not in scope at the class level or inside `connect`, causing a compilation failure.

---

### Medium Severity

#### Dead/Unlinked Source Files
*   **File & Line**: `crates/op-chat/src/lib.rs:1`
*   **Description**: Multiple source files (`chat_loop.rs`, `grpc_client.rs`, `hybrid_executor.rs`, `intent_executor.rs`, `router.rs`, `tool_loader.rs`) exist in the crate directory but are not declared as modules via `mod` in `lib.rs` or `main.rs`. These files are completely ignored during compilation, hiding syntax and safety errors from `cargo check`.

---

### Low Severity & Code Quality

#### Duplicate Session and Message Models
*   **File & Line**: `crates/op-chat/src/router.rs:17`
*   **Description**: `router.rs` declares a duplicate definition of the `ChatSession` and `ChatMessage` structs rather than importing and reusing the canonical models defined in `crates/op-chat/src/session.rs`.

#### Expensive Re-Compilation of Regexes
*   **File & Line**: `crates/op-chat/src/nl_admin.rs:476-488`
*   **Description**: Multiple regex patterns are compiled inside the `clean_llm_response` function on every execution. This introduces a significant performance overhead. These patterns should be compiled once using a lazy static initializer or stored as fields.