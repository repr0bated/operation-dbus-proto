# Integration Analysis & Security Audit: `op-tools`

## 1. Workspace Integration & Dependencies

### Crates Depending on `op-tools`
Based on the provided workspace `Cargo.toml`, the following crates declare direct dependencies on `op-tools`:
*   **`op-dbus`** (Root package) — `Cargo.toml` under `[dependencies]`: `op-tools.workspace = true`.

---

## 2. D-Bus Service Registrations & Object Paths

The `op-tools` crate registers and exposes several native D-Bus services and object paths:

### Capable/Runtime Agent Services
*   **Service Name:** `org.dbusmcp.Agent.[PascalCaseAgentName]` (derived from the normalized agent type, e.g., `org.dbusmcp.Agent.RustPro` or `org.dbusmcp.Agent.PythonPro`).
*   **Object Path:** `/org/dbusmcp/Agent/[PascalCaseAgentName]` (e.g., `/org/dbusmcp/Agent/RustPro`).
*   **Interface:** `org.dbusmcp.Agent`
*   **Citation:** `crates/op-tools/src/builtin/agent_tool.rs:107-119`

### Dinit Service Manager Proxy
*   **Default Service:** `org.chimera.dinit`
*   **Default Path:** `/org/chimera/dinit`
*   **Interface:** `org.chimera.dinit.Manager`
*   **Citation:** `crates/op-tools/src/builtin/dinit.rs:34-36`

### Plugin Projection Services
*   **Default Service:** `org.opdbus.v1`
*   **Object Path Prefix:** `/org/opdbus/v1/plugins/[plugin_name]` (e.g., `/org/opdbus/v1/plugins/systemd` or `/org/opdbus/v1/plugins/net`).
*   **Interface:** `org.opdbus.ProjectedObjectV1`
*   **Citation:** `crates/op-tools/src/builtin/plugin_projection.rs:13-15`

---

## 3. Exposed HTTP & gRPC Endpoints

The `op-tools` crate exposes a set of REST endpoints via Axum, intended to be mounted under a unified HTTP router:

### REST Endpoints
*   **Prefix:** `/api/tools`
*   **Endpoints:**
    *   `GET /api/tools` — Lists all registered tool definitions (`list_tools_handler`).
    *   `GET /api/tools/health` — Reports tools service status (`health_handler`).
    *   `GET /api/tools/:name` — Retrieves detailed schema and metadata for a specific tool (`get_tool_handler`).
    *   `POST /api/tools/:name/execute` — Executes the specified tool using a JSON body payload (`execute_tool_handler`).
*   **Citation:** `crates/op-tools/src/router.rs:38-41`, `crates/op-tools/src/router.rs:59`

---

## 4. Cross-Crate Circular Dependency Risks

A structural circular dependency hazard exists between `op-tools`, `op-state`, and `op-agents`:

1.  **`op-tools` $\rightarrow$ `op-state` / `op-agents`:**
    *   `op-tools` depends on `op-state` in its `Cargo.toml` to query and mutate system state through the state plugin adapter (`crates/op-tools/src/builtin/plugin_state_tool.rs`).
    *   `op-tools` depends on `op-agents` in `Cargo.toml` to access `builtin_agent_descriptors` and discover active agent configurations (`crates/op-tools/src/builtin/agent_tool.rs:13`).
2.  **`op-state` / `op-agents` $\rightarrow$ `op-tools`:**
    *   If any state plugin inside `op-state` or any agent runner inside `op-agents` attempts to statically implement or reference the `Tool` trait or the unified `ToolRegistry` defined in `op-tools`, a compilation-blocking circular dependency (`op-tools` $\rightarrow$ `op-state` $\rightarrow$ `op-tools`) is triggered.
3.  **Mitigation:**
    *   The `Tool` trait (`crates/op-tools/src/tool.rs:43`) and the base `ToolRegistry` should be migrated to a minimal, dependency-free leaf crate (e.g., `op-core`), allowing both `op-state` and `op-tools` to depend on the traits without depending on each other.

---

## 5. Schema-as-Code Discipline Violations

The codebase bypasses the workspace's schema-as-code discipline (which mandates Protocol Buffers and OSCAL versioned schemas) in several locations, opting instead for ad-hoc JSON structures and runtime-parsed string signatures:

### Ad-hoc JSON Schema Construction
Instead of importing versioned schemas from static files or compiled Protobuf structures, tool schemas are constructed dynamically in-memory using `simd_json::json!`:
*   `crates/op-tools/src/builtin_old.rs:24-37` (EchoTool schema)
*   `crates/op-tools/src/builtin/file.rs:114-177` (Ad-hoc filesystem schemas)
*   `crates/op-tools/src/builtin/response_tools.rs:115-136` (Ad-hoc communication schemas)

### Ad-hoc D-Bus Signature to JSON Schema Translation
D-Bus method arguments are translated into JSON schemas on the fly by parsing raw D-Bus signature characters (`s`, `o`, `g`, `b`, etc.):
*   `crates/op-tools/src/builtin/dbus_hybrid.rs:77-123`
*   `crates/op-tools/src/builtin/dbus_tool.rs:66-93`

### Rust Tuples as Untyped D-Bus Contracts
Unversioned, positional Rust tuples are mapped directly to native D-Bus records without structured schemas:
*   `crates/op-tools/src/builtin/dinit.rs:15-16`

### Unstructured Serde Events
Audit events and LLM decisions are defined as ad-hoc Serde-serializable structs containing untyped `Value` blobs, rather than being modeled as rigid Protocol Buffers or OSCAL-compliant audit trail objects:
*   `crates/op-tools/src/orchestration_plugin.rs:35-50` (`ToolExecutedEvent` contains untyped `arguments` and `metadata` JSON `Value` fields).

---

## 6. Production Security & Quality Audit Findings

### [CRITICAL] Memory Corruption & UB via Re-parsing Mutated Buffer
*   **File:** `crates/op-tools/src/mcptools.rs:270-279`
*   **Vulnerability Type:** Memory Safety / Undefined Behavior
*   **Description:** 
    `simd_json` is an in-place parser. When `simd_json::from_str` or `simd_json::from_slice` is invoked, it mutates the source buffer (e.g., performing in-place string unescaping and inserting null-terminators).
    
    At line 270, `mcptools` attempts to parse the environment variable `OP_MCPTOOLS_SERVERS` as a JSON array:
    ```rust
    let mut raw_mut = raw;
    if let Ok(list) = unsafe { simd_json::from_str::<Vec<McpToolsServerConfig>>(&mut raw_mut) } { ... }
    ```
    If this parse fails (for example, if the environment variable contains a single JSON object instead of an array), `raw_mut` has already been mutated and corrupted by the failed parse. 
    
    The code then clones the corrupted string buffer into `raw_mut2` and immediately passes it back into `simd_json::from_str`:
    ```rust
    let mut raw_mut2 = raw_mut;
    let single = unsafe { simd_json::from_str::<McpToolsServerConfig>(&mut raw_mut2) } ...
    ```
    Passing a mutated/corrupted buffer into `simd_json` violates the parser's memory-safety preconditions. This leads to undefined behavior, out-of-bounds reads, memory corruption, or segmentation faults.
*   **Remediation:** Do not reuse or copy `raw_mut` after a failed `simd_json` parsing attempt. Obtain a fresh copy of the original `raw` string for the fallback parse:
    ```rust
    let mut raw_mut2 = raw.clone();
    ```

### [CRITICAL] Arbitrary Command Injection Bypass in `ShellTool`
*   **File:** `crates/op-tools/src/builtin_old.rs:142-178`
*   **Vulnerability Type:** Command Injection / Authorization Bypass
*   **Description:**
    The `ShellTool` validation logic (lines 142-159) attempts to restrict execution to whitelisted commands by extracting the first word of the command string:
    ```rust
    let base_cmd = command.split_whitespace()
        .next()
        .unwrap_or(command);
    
    if !self.allowed_commands.iter().any(|c| c == base_cmd) { ... }
    ```
    If `base_cmd` is in the allowed list (e.g., `"ls"`), validation succeeds.
    
    However, the tool executes the command by passing the *entire* raw `command` string directly to `sh -c`:
    ```rust
    match tokio::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{} {}", command, args.join(" ")))
    ```
    An attacker can supply a command string containing shell metacharacters such as `ls; rm -rf /` or `ls && cat /etc/shadow`. The validation logic extracts `"ls"` as the `base_cmd`, passes the check, and then executes both commands with system privileges.
*   **Remediation:** Avoid executing raw strings via `sh -c`. If shell execution is required, strictly execute only the validated executable directly (via `Command::new(base_cmd)`) and pass arguments as separate elements of an argument vector, bypassing the shell interpreter completely.

### [MEDIUM] Path Traversal Bypass via Lack of Canonicalization
*   **File:** `crates/op-tools/src/security.rs:337-360`
*   **Vulnerability Type:** Path Traversal / Authorization Bypass
*   **Description:**
    The `validate_read_path` and `validate_write_path` functions check if a path belongs to an allowed list (e.g., starting with `/tmp` or `/home`) using simple string matching or `path_buf.starts_with`:
    ```rust
    let path_buf = PathBuf::from(path);
    ...
    let is_allowed = allowed_read.iter().any(|p| path_buf.starts_with(p));
    ```
    Because the path is not canonicalized (i.e., resolving symlinks and redundant parent directory references `..`), a restricted user can bypass this check. For example, if a symbolic link `/tmp/bypass` points to `/root`, a request to read `/tmp/bypass/secret` will pass the `starts_with("/tmp")` check, allowing unauthorized access to sensitive files.
*   **Remediation:** Canonicalize all input paths before performing directory validation checks:
    ```rust
    let path_buf = std::fs::canonicalize(path)?;
    ```