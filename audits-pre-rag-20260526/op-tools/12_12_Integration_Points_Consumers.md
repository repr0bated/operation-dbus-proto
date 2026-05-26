### Integration & Routing Analysis

#### 1. Workspace Dependencies on `op-tools`
Based on the provided workspace `Cargo.toml`, the following crates depend on `op-tools`:
* **`op-dbus`** (the root workspace package, as defined in `Cargo.toml` under the `[dependencies]` section).

#### 2. Registered D-Bus Service Names & Object Paths
The following D-Bus services and object paths are registered or utilized as clients within `op-tools`:

* **Agent D-Bus Service Registration** (`crates/op-tools/src/builtin/agent_tool.rs`):
  * **Service Name**: `org.dbusmcp.Agent.{AgentNamePascalCase}` (e.g., `org.dbusmcp.Agent.RustPro`)
  * **Object Path**: `/org/dbusmcp/Agent/{AgentNamePascalCase}` (e.g., `/org/dbusmcp/Agent/RustPro`)
  * **Interface**: `org.dbusmcp.Agent`

* **Projected Plugins Service Registration** (`crates/op-tools/src/builtin/plugin_projection.rs`):
  * **Service Name**: `org.opdbus.v1`
  * **Object Path**: `/org/opdbus/v1/plugins/{PluginName}/{ChildPaths}`
  * **Interface**: `org.opdbus.ProjectedObjectV1`

* **PackageKit Client Operations** (`crates/op-tools/src/bin/op-packagekit-install.rs`, `crates/op-tools/src/builtin/packagekit.rs`):
  * **Service Name**: `org.freedesktop.PackageKit`
  * **Object Path**: `/org/freedesktop/PackageKit` and dynamically generated transaction paths (e.g., `/org/freedesktop/PackageKit/transactions/{id}`)
  * **Interface**: `org.freedesktop.PackageKit` and `org.freedesktop.PackageKit.Transaction`

* **Systemd Client Operations** (`crates/op-tools/src/builtin/dbus.rs`, `crates/op-tools/src/builtin/dbus_hybrid.rs`):
  * **Service Name**: `org.freedesktop.systemd1`
  * **Object Path**: `/org/freedesktop/systemd1` and dynamically retrieved unit paths
  * **Interface**: `org.freedesktop.systemd1.Manager` and `org.freedesktop.systemd1.Unit`

* **NetworkManager Client Operations** (`crates/op-tools/src/builtin/dbus_hybrid.rs`):
  * **Service Name**: `org.freedesktop.NetworkManager`
  * **Object Path**: `/org/freedesktop/NetworkManager`
  * **Interface**: `org.freedesktop.NetworkManager`

* **Dinit Client Operations** (`crates/op-tools/src/builtin/dinit.rs`):
  * **Service Name**: `org.chimera.dinit`
  * **Object Path**: `/org/chimera/dinit`
  * **Interface**: `org.chimera.dinit.Manager`

#### 3. Exposed HTTP/gRPC Endpoints
The crate exposes only HTTP REST endpoints via the Axum router defined in `crates/op-tools/src/router.rs`. No gRPC endpoints are exposed directly within this crate.

* **HTTP Endpoints**:
  * `GET /api/tools` — Lists all registered tool definitions.
  * `GET /api/tools/health` — Returns service health status.
  * `GET /api/tools/:name` — Gets detailed metadata/schema for a specific tool.
  * `POST /api/tools/:name/execute` — Executes a specific tool with a JSON body payload.

#### 4. Cross-Crate Circular Dependency Risks
* **`op-tools` $\leftrightarrow$ `op-http`**: `op-tools` lists `op-http = { path = "../op-http" }` in its dependencies. The Axum router implements the `ServiceRouter` trait from `op_http::router`. If `op-http` references `op-tools` (for routing compilation or unified state management), a circular dependency cycle will prevent compilation.
* **`op-tools` $\leftrightarrow$ `op-state`**: `op-tools` depends on `op-state` to query, diff, and apply state configurations. If `op-state` depends on any core tool execution registry or validation primitives from `op-tools` (rather than defining its own trait contracts), a dependency loop will occur.

---

### Security & Quality Audit Findings

#### CRITICAL: Unauthenticated Remote Code Execution (RCE) via HTTP Router
* **Reference**: `crates/op-tools/src/router.rs:114-131`
* **Impact**: Direct, unauthenticated compromise of the host system.
* **Details**: The POST route handler `execute_tool_handler` extracts the tool name from the path and the parameters from the JSON body, then immediately invokes `tool.execute(params)`. No authentication checks, session validation, token verification, or rate-limiting guards are performed at the router level. Any caller on the network can call `/api/tools/shell_execute/execute` (registered via `register_shell_tools`) to execute arbitrary shell commands with root privileges.
* **Remediation**: Implement a robust Axum authentication layer/middleware (e.g., bearer tokens, session verification) to wrap the router, and pass the caller's verified identities and sessions down to the execution engine.

#### CRITICAL: Shell Command Injection in ShellTool
* **Reference**: `crates/op-tools/src/builtin_old.rs:193-200`
* **Impact**: Arbitrary command execution with host privileges.
* **Details**: The validation logic for `ShellTool` only splits the first whitespace-separated segment of the command input to match it against an allowlist (e.g., `ls`). Because the remainder of the command string is formatted directly into a shell environment invocation via `sh -c`, an attacker can pass command separators such as `&&`, `;`, or `|` (e.g., `ls && rm -rf /`) to execute unvetted commands. Furthermore, command arguments `args` are joined with spaces and appended directly without shell escaping or sanitization.
* **Remediation**: Completely avoid running user input inside shell interpreters like `sh -c`. Use direct process spawning (`tokio::process::Command::new(base_cmd).args(clean_args)`) to pass arguments securely as structured array elements.

#### HIGH: Double-Parsing Memory Corruption and UB via `simd_json::from_str`
* **Reference**: `crates/op-tools/src/mcptools.rs:191-193`
* **Impact**: Potential undefined behavior, memory corruption, or segmentation faults.
* **Details**: `simd_json` uses destructive, in-place parsing that mutates the input string slice (e.g., null-terminating keys, unescaping strings). At line 186, `simd_json::from_str` is called on `raw_mut`. At line 192, if the first parse failed or succeeded, `raw_mut2` (which shares the backing allocation of `raw_mut`) is passed into `simd_json::from_str` again. Re-parsing a corrupted, mutated byte buffer leads directly to undefined behavior in `simd_json`'s native parser.
* **Remediation**: Always pass a freshly allocated or cloned, unmodified string/buffer to `simd_json::from_str` for separate parsing attempts.

#### HIGH: Path Traversal and Arbitrary File Disclosure
* **Reference**: `crates/op-tools/src/builtin_old.rs:248-262`
* **Impact**: Unauthorized access to sensitive system secrets (e.g., private keys, database credentials, `/etc/shadow`).
* **Details**: The `FileReadTool` reads file paths directly from user input arguments and calls `tokio::fs::read` without verifying that the paths lie within a restricted directory sandbox.
* **Remediation**: Restrict file access using a canonicalized path check. Ensure all resolved paths start with a trusted, sandboxed root directory.

#### HIGH: Lexical Path Blacklist Bypass (Symlinks and Path Traversal)
* **Reference**: `crates/op-tools/src/validation.rs:472-477`
* **Impact**: Access bypass to highly sensitive system files.
* **Details**: Path validation uses simple lexical prefix checks (`path_buf.starts_with(forbidden)`). If an attacker uses symbolic links pointing to forbidden files (such as a symlink inside `/tmp` pointing to `/etc/shadow`) or inputs redundant path separators, the prefix match fails to identify the target, but the OS resolves the target when performing the file operation.
* **Remediation**: Always canonicalize the paths (`std::fs::canonicalize`) to resolve symlinks and relative segments (`..`) before checking them against allowlists or blocklists.

#### HIGH: Insufficient Path Traversal Validation in Sanitizer
* **Reference**: `crates/op-tools/src/validation.rs:389-403`
* **Impact**: Bypass of malicious input detection.
* **Details**: The path traversal blocklist check inside `sanitize_input` only looks for literal `"../../../"` and `"..\\"`. An attacker can easily bypass this with custom relative depths (e.g. `../../etc/passwd` or `..//..//..//`), escaping the directory checks entirely on systems that normalize multiple slashes.
* **Remediation**: Replace the blacklist check with a strict check for any occurrences of `..` or use a standard path canonicalization step.

#### HIGH: Resource Exhaustion / Denial of Service via Introspection Recursion
* **Reference**: `crates/op-tools/src/builtin/dbus_introspection.rs:330-340`
* **Impact**: Crash or total freeze of the control plane.
* **Details**: The D-Bus introspection tool allows the caller to set excessively high execution limits (e.g., `max_depth = 128` and `max_objects_per_service = 200000`). Walking the D-Bus object graph to this depth will result in millions of asynchronous requests, causing massive memory consumption, file descriptor exhaustion, and eventual process crash.
* **Remediation**: Clamp the maximum allowed input boundaries for depth and objects to safe, conservative values (e.g., max depth of 5, max objects of 1000).

#### MEDIUM: Insecure Hardcoded Paths in Shared Temporary Directory
* **Reference**: `crates/op-tools/src/builtin/file.rs:327`, `builtin/file.rs:339`
* **Impact**: Local symlink races, potential data overwrite, or deletion of local user files.
* **Details**: Test cases and certain default profiles write to hardcoded paths in the shared `/tmp` directory (e.g., `/tmp/test_read_file.txt`). On multi-user systems, another local user can create a symlink at that location pointing to a file owned by the test runner, hijacking file writes.
* **Remediation**: Utilize a secure temporary directory crate like `tempfile` to generate random, securely isolated temporary file paths.