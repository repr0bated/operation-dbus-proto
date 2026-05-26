### Critical Findings

#### 1. Sandbox Bypass & Remote Code Execution via Command Chaining
* **File**: `crates/op-tools/src/security.rs:509`
* **File**: `crates/op-tools/src/builtin/shell.rs:88`
* **Details**: The `SecurityValidator` allows commands under `AccessLevel::Restricted` by checking only the first token of the command string using `.split_whitespace().next()`. This allowlist check is trivially bypassed by chaining commands using shell metacharacters (e.g., `;`, `&&`, `||`, or `|`). 
* **Exploitability**: An attacker with restricted access can pass `ls ; rm -rf /` or `ls && <malicious command>` to `ShellExecuteTool`. The `base_cmd` evaluates to `ls` (which is allowed), passing the security check. The tool then passes the full unescaped command string directly to `bash -c`, executing arbitrary commands with host privileges.

---

#### 2. Arbitrary Code Execution and Sandbox Bypass in Old `ShellTool`
* **File**: `crates/op-tools/src/builtin_old.rs:131`
* **File**: `crates/op-tools/src/builtin_old.rs:109`
* **Details**: The `ShellTool` implementation in `builtin_old.rs` defines a custom `validate` function to enforce command limits. However, the `execute` method does not actually invoke `validate`, allowing unvalidated command execution. Furthermore, the `validate` function suffers from the same `.split_whitespace().next()` parsing bug, and the tool joins and executes `args` directly inside `sh -c` without any validation, creating multiple direct command injection vectors.
* **Exploitability**: Directly exploitable by calling `execute` with any command or arguments containing shell metacharacters.

---

#### 3. Host File Read & Write Sandbox Bypass via `/proc` Symbolic Links
* **File**: `crates/op-tools/src/builtin/procfs.rs:13`
* **File**: `crates/op-tools/src/builtin/procfs.rs:163`
* **File**: `crates/op-tools/src/builtin/procfs.rs:293`
* **Details**: The `validate_relative_path` helper restricts paths to `/proc` and `/sys` by checking `path.contains("..")` and rejecting leading slashes. However, it does not resolve symbolic links. Inside `/proc`, `self/root` and `1/root` are symbolic links pointing directly to the root directory `/` of the host system.
* **Exploitability**: A restricted user can read or write any file on the host system (e.g., `/etc/shadow`) by invoking `procfs_read` or `procfs_write` with a path such as `1/root/etc/shadow`. This completely bypasses the `/proc` filesystem sandbox boundary.

---

#### 4. Path Traversal Sandbox Escape in `validate_self_path` on Missing Files
* **File**: `crates/op-tools/src/builtin/self_tools.rs:43`
* **Details**: `validate_self_path` canonicalizes paths using `canonicalize().unwrap_or_else(|_| full_path.clone())` and then checks if the path starts with the repository path using `starts_with()`. When writing a new file, the file does not exist yet, causing `canonicalize()` to fail and return the un-normalized `full_path.clone()`. In Rust, `Path::starts_with` only compares syntactic path components and does not normalize them.
* **Exploitability**: If a path like `/home/user/repo/../../../etc/shadow` is used for a new file, `starts_with` returns `true` because `/home/user/repo` is a syntactic prefix of the un-normalized path. When the file is subsequently written by the OS, the relative segments are resolved, overwriting host files outside the repository.

---

#### 5. Denial of Service (Out-Of-Memory Crash) via Large Pseudo-Files
* **File**: `crates/op-tools/src/builtin/procfs.rs:29`
* **File**: `crates/op-tools/src/builtin_old.rs:201`
* **File**: `crates/op-tools/src/builtin/file.rs:204`
* **Details**: Multiple file-reading tools (e.g., `procfs_read`, `FileReadTool`, `SecureFileTool`) read the entire target file into memory using `fs::read_to_string` or `fs::read` before enforcing capacity truncation or limits.
* **Exploitability**: Any user can trigger an Out-Of-Memory (OOM) crash of the entire system process by pointing these tools to infinitely streaming or massive pseudo-files such as `/proc/kcore` (which represents the system's virtual memory up to 128TB) or `/dev/urandom`.

---

#### 6. Privilege Escalation via Unauthorized Write Actions Marked as `ReadOnly`
* **File**: `crates/op-tools/src/builtin/dbus_introspection.rs:893`
* **File**: `crates/op-tools/src/builtin/dbus_introspection.rs:1007`
* **File**: `crates/op-tools/src/builtin/dbus.rs:25`
* **File**: `crates/op-tools/src/builtin/incus_tools.rs:232`
* **File**: `crates/op-tools/src/builtin/lxc_tools.rs:141`
* **File**: `crates/op-tools/src/builtin/packagekit.rs:16`
* **Details**: Multiple highly privileged, state-modifying tools (such as calling arbitrary system D-Bus methods, writing system properties, restarting systemd units, creating/deleting container instances, and installing packages via PackageKit) do not override the default `Tool::security_level` implementation. As a result, they default to `SecurityLevel::ReadOnly`.
* **Exploitability**: Under the permission system, restricted users who are only authorized for read-only actions can execute these tools, allowing them to delete containers, install arbitrary system packages, and perform unauthorized D-Bus calls.

---

### Major Findings

#### 7. Architectural Defect: Unused `InputValidator` and Unenforced `SecurityLevel`
* **File**: `crates/op-tools/src/tool.rs:48`
* **File**: `crates/op-tools/src/executor.rs:52`
* **Details**: The codebase implements a robust input validation layer (`InputValidator` in `validation.rs`) that sanitizes forbidden characters and prevents shell injections. However, this validator is completely unused by the tool executors and HTTP router handlers. Similarly, `SecurityLevel` is declared on the `Tool` trait but never queried or enforced in `ToolExecutor` or the axum handlers, rendering these access controls ineffective.

---

#### 8. Undefined Behavior & Memory Safety Violations via Unsafe `simd_json::from_str`
* **File**: `crates/op-tools/src/builtin/agent_tool.rs:341`
* **File**: `crates/op-tools/src/builtin/dbus_tool.rs:408`
* **File**: `crates/op-tools/src/builtin/rtnetlink_tools.rs:105`
* **File**: `crates/op-tools/src/mcptools.rs:252`
* **Details**: The codebase frequently calls `unsafe { simd_json::from_str(&mut string) }` on standard Rust `String` instances returned by process stdout or environment variables. `simd-json` requires the input buffer to be allocated with `simd_json::PADDING` bytes of trailing capacity. Parsing unpadded standard `String` structures can result in out-of-bounds reads and undefined behavior (including segfaults or memory disclosure).

---

#### 9. Client-Controlled `session_id` Bypasses Rate Limiting
* **File**: `crates/op-tools/src/builtin/shell.rs:59`
* **Details**: The `session_id` used to enforce rate-limiting in `ShellExecuteTool` is fetched directly from the JSON execution parameters provided by the client. An attacker can completely bypass rate limits by rotating the `session_id` field on every request or by passing a trusted ID such as `"chatbot"`.

---

#### 10. Broken Semantic Code Search Implementation
* **File**: `crates/op-tools/src/builtin/code_search.rs:172`
* **Details**: The `embed_text` function in `code_search.rs` is a hardcoded mock that always returns a vector of 384 zeros. As a result, any semantic queries sent to Qdrant will use the zero vector, retrieving random or completely unrelated code chunks.

---

### Minor Findings

#### 11. High Latency sequentially blocking D-Bus Introspection in `ProjectionEngine`
* **File**: `crates/op-tools/src/discovery/projection_engine.rs:189`
* **Details**: The `discover_paths` method traverses the D-Bus object tree sequentially. On systems with many D-Bus objects (such as UDisks2 or NetworkManager), performing hundreds of synchronous-like sequential round-trips over D-Bus can take several minutes and block startup or trigger timeouts.

---

#### 12. Panicking Static Initializers on Missing Setup
* **File**: `crates/op-tools/src/builtin/response_tools.rs:77`
* **File**: `crates/op-tools/src/orchestration_plugin.rs:260`
* **Details**: `get_response_accumulator()` and `get_orchestration_registry()` panics on uninitialized access. Implementing lazy initialization via `OnceLock::get_or_init` would be much more robust, as is done correctly in `AgentConnectionRegistry::global()`.

---

#### 13. Parameter Mapping Failures in `DbusCallMethodTool`
* **File**: `crates/op-tools/src/builtin/dbus_introspection.rs:918`
* **Details**: `json_to_owned_value` automatically maps all JSON integers to `i64` or `u64`. This causes immediate signature mismatches when calling target D-Bus methods that expect narrower types (such as `i32` or `u32`), as the tool lacks signature-aware casting before serialization.