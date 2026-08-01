# Production Security & Quality Audit Report: `op-tools`

## 1. Vulnerability & Quality Audit

### 1.1 Critical Vulnerabilities (Directly Exploitable)

#### 1.1.1 Unauthenticated Arbitrary Remote Code Execution (RCE) via HTTP Router
*   **Citation:** `crates/op-tools/src/router.rs:88` (`execute_tool_handler`), `crates/op-tools/src/builtin_old.rs:141` (`ShellTool::execute`), and `crates/op-tools/src/builtin/shell_tool.rs:51` (`ShellExecuteTool::execute`)
*   **Vulnerability Type:** Command Injection / Broken Access Control
*   **Description:** The HTTP router endpoint `/:name/execute` mapped to `execute_tool_handler` takes arbitrary `Value` parameters directly from the JSON body of a POST request and calls `tool.execute(params).await` without enforcing authentication, authorization, or input validation.
    *   The `ShellExecuteTool` in `shell_tool.rs` has **zero** validation or security verification and spawns commands directly under `bash -c`.
    *   The legacy `ShellTool` in `builtin_old.rs` similarly processes command execution without validation when triggered via this route.
    *   Even if using the newer `ShellExecuteTool` in `shell.rs`, if no `session_id` is supplied in the request body, the session defaults to `"default"`, which resolves to an unrestricted system administrator profile under default configurations.
*   **Remediation:** 
    1. Implement a robust authentication and authorization layer (e.g., JWT validation or local policy checks) on the HTTP router before mounting any execution paths.
    2. Completely remove the unsecured legacy `shell_tool.rs` and `builtin_old.rs` files.
    3. Ensure that every execution pathway through the HTTP handler explicitly routes arguments through `InputValidator::validate_input` before execution.

#### 1.1.2 Command Injection Validation Bypass via Split-Whitespace Parsing
*   **Citation:** `crates/op-tools/src/builtin_old.rs:125` (`ShellTool::validate`)
*   **Vulnerability Type:** Command Injection
*   **Description:** The legacy `ShellTool` implements a custom `validate` function that extracts the "base command" by splitting the input string on whitespace:
    ```rust
    let base_cmd = command.split_whitespace()
        .next()
        .unwrap_or(command);
    ```
    If `base_cmd` is allowed (e.g., `"ls"`), the validation succeeds. An attacker can craft a payload such as `command = "ls; rm -rf /"`. The validator processes `"ls;"` or `"ls"` (which is whitelisted), passes the validation, and then passes the entire raw string to `tokio::process::Command::new("sh").arg("-c").arg(...)`. This permits trivial shell command chaining and complete compromise.
*   **Remediation:** Do not rely on ad-hoc shell string parsing. Avoid `sh -c` or `bash -c` string execution completely. Instead, accept structured arrays of arguments and execute target binaries directly without shell interpolation.

#### 1.1.3 Path Traversal & Arbitrary File Read
*   **Citation:** `crates/op-tools/src/builtin_old.rs:200` (`FileReadTool::execute`)
*   **Vulnerability Type:** Arbitrary File Read / Path Traversal
*   **Description:** The legacy `FileReadTool` accepts a `path` parameter directly from request arguments and passes it to `tokio::fs::read` without performing any directory validation, canonicalization, or path cleaning. Any anonymous user hitting the axum endpoint `/api/tools/file_read/execute` can read arbitrary files (including `/etc/passwd` or `/etc/shadow`) by supplying absolute paths or relative traversals (e.g., `../../etc/shadow`).
*   **Remediation:** Remove `builtin_old.rs` entirely. In all secure file handlers, consistently enforce `validator.validate_read_path(path)` and verify that paths resolve within safe, isolated directories.

---

### 1.2 Schema-As-Code Violations
The codebase extensively defines data contracts (tool definitions, inputs, and outputs) via ad-hoc, in-line JSON structures using the `json!` macro or manual `Value` builder logic. This violates the schema-as-code discipline, which mandates versioned, centralized schemas (such as Protocol Buffers or OSCAL documents) for all API and data contracts.

#### 1.2.1 Ad-Hoc Dynamic Schema Construction
*   **Citation:** `crates/op-tools/src/dynamic_tool.rs:95` (`input_schema`), `crates/op-tools/src/builtin/dbus_hybrid.rs:94` (`generate_schema_from_signature`)
*   **Violation:** Input schemas for dynamic D-Bus projected tools are generated at runtime by programmatically populating JSON objects. This lacks a deterministic, build-time compilation hook or versioned schema contract.

#### 1.2.2 Hardcoded Inline Schemas in Code
*   **Citations:**
    *   `crates/op-tools/src/builtin_old.rs:20` (Echo tool schema)
    *   `crates/op-tools/src/builtin_old.rs:59` (System info schema)
    *   `crates/op-tools/src/builtin_old.rs:101` (Shell tool schema)
    *   `crates/op-tools/src/builtin_old.rs:184` (File read schema)
    *   `crates/op-tools/src/builtin/file.rs:108` (SecureFileTool nested JSON schema matches)
    *   `crates/op-tools/src/builtin/anydesk.rs:51` (AnyDesk tool schema)
    *   `crates/op-tools/src/builtin/dbus.rs:32` (D-Bus restart schema)
    *   `crates/op-tools/src/builtin/openflow_tools.rs:50` (OpenFlow Add Flow schema)
*   **Violation:** Data contracts are declared as unstructured inline literals. These definitions are prone to drifting from target system properties, bypass central OSCAL/Protobuf registries, and cannot be statically audited.
*   **Remediation:** Define all tool definitions and inputs in a declarative schema registry using Protocol Buffers or versioned OSCAL models, generating the Rust tool bindings at build time.

---

### 1.3 General Security & Quality Code Smells

#### 1.3.1 Insecure Default Profile for Security Validator
*   **Citation:** `crates/op-tools/src/security.rs:309` (`with_admin_profile`) and `crates/op-tools/src/security.rs:434` (`Default` implementation)
*   **Vulnerability Type:** Insecure Defaults
*   **Description:** The `SecurityValidator` defaults to the `admin()` profile containing `AccessLevel::Unrestricted`. If any component instantiates a validator without explicitly loading a restricted profile, it will fall back to full unrestricted system administration privileges, facilitating privilege escalation.
*   **Remediation:** Enforce the principle of least privilege. The `Default` implementation must fall back to `with_restricted_profile()` rather than an administrative profile.

#### 1.3.2 Sensitive Parameter Leakage in Log Messages
*   **Citation:** `crates/op-tools/src/builtin/agent_tool.rs:260` (`AgentDbusService::execute`)
*   **Vulnerability Type:** Sensitive Logging
*   **Description:** The D-Bus interface debug log prints the raw, unredacted JSON task string:
    ```rust
    debug!(agent = %self.agent_type, task = %task_json, "Executing");
    ```
    If an agent processes passwords, private keys, or personal identifiable information (PII) within its arguments, this sensitive data is written directly to the system log in plain text.
*   **Remediation:** Parse the JSON structure and strip or mask any keys containing sensitive information (e.g., `"token"`, `"password"`, `"args"`) before emitting log events.

#### 1.3.3 Unsafe Mutation of String Buffers in `simd_json` Deserialization
*   **Citation:** `crates/op-tools/src/mcptools.rs:218` (`load_mcp_config`), `crates/op-tools/src/builtin/agent_tool.rs:263` (`AgentDbusService::execute`)
*   **Vulnerability Type:** Potential Undefined Behavior / Unsafe Code Misuse
*   **Description:** The codebase uses `unsafe { simd_json::from_str(...) }` to perform in-place deserialization of string buffers. While `simd_json` requires mutable access because it modifies the string buffer during parsing, wrapping this in `unsafe` blocks without documented safety guarantees or validation of buffer alignment/ownership poses risks of undefined behavior if the buffers originate from shared memory or concurrent threads.
*   **Remediation:** Document safe preconditions for every `unsafe` block or transition to safe deserialization methods.

---

## 2. Public API Surface & Encapsulation

### 2.1 Public API Surface Enumeration
A search of the audited files reveals a public API surface consisting of approximately **324 public items** (including structs, enums, traits, functions, types, and modules).

The top 10 most impactful public items are listed below:

| # | Item | Type | Location | Impact Description |
|---|---|---|---|---|
| 1 | `Tool` | `trait` | `crates/op-tools/src/tool.rs:43` | Fundamental abstraction for all system capabilities and tool execution. |
| 2 | `ToolRegistry` | `struct` | `crates/op-tools/src/registry.rs:36` | Central registry coordinating dynamic execution and registration of tools. |
| 3 | `InputValidator` | `struct` | `crates/op-tools/src/validation.rs:163` | Engine validating and sanitizing JSON inputs to prevent injection attacks. |
| 4 | `SecurityValidator` | `struct` | `crates/op-tools/src/security.rs:293` | Manages global and session-specific access controls and rate limiting. |
| 5 | `ToolExecutor` | `struct` | `crates/op-tools/src/executor.rs:35` | Controls concurrency, semaphore allocation, and execution timeouts. |
| 6 | `OrchestrationPluginRegistry` | `struct` | `crates/op-tools/src/orchestration_plugin.rs:159` | Directs session, decision, and execution events to auditing backends. |
| 7 | `ProjectionEngine` | `struct` | `crates/op-tools/src/discovery/projection_engine.rs:48` | Auto-discovers and projects live D-Bus APIs directly into executable tools. |
| 8 | `ToolDiscoverySystem` | `struct` | `crates/op-tools/src/discovery/mod.rs:113` | Manages caching, scheduling, and searching of discovered capability schemas. |
| 9 | `DynamicDbusTool` | `struct` | `crates/op-tools/src/dynamic_tool.rs:8` | Facilitates direct, structured method calls over system/session buses. |
| 10 | `ToolsServiceRouter` | `struct` | `crates/op-tools/src/router.rs:59` | Mounts axum API endpoints, exposing system operations over HTTP. |

---

### 2.2 Struct Encapsulation Violations (Public Fields)
Multiple structs expose public fields directly, breaking encapsulation, bypassing validation invariants, and allowing callers to modify internal configuration properties unsafely:

*   **`DynamicDbusTool`** (`crates/op-tools/src/dynamic_tool.rs:9`)
    ```rust
    pub name: String,
    pub service: String,
    pub path: String,
    pub interface: String,
    pub method: String,
    pub signature: String,
    pub arg_names: Vec<String>,
    ```
    *Correction:* Field states should be immutable and exposed strictly through read-only getters.
*   **`ExecutorConfig`** (`crates/op-tools/src/executor.rs:14`)
    ```rust
    pub max_concurrent: usize,
    pub default_timeout_ms: u64,
    pub max_timeout_ms: u64,
    ```
    *Correction:* Keep fields private and provide a builder pattern for configuration.
*   **`ToolsState`** (`crates/op-tools/src/router.rs:17`)
    ```rust
    pub registry: Arc<ToolRegistry>,
    ```
    *Correction:* Encapsulate the registry reference to prevent arbitrary state modification.
*   **`ValidationConfig`** (`crates/op-tools/src/validation.rs:27`)
    *   Exposes `trusted_sessions`, `command_whitelist`, `allowed_dirs`, and `forbidden_dirs` as public mutable fields. This allows any consumer to alter the validation rules and completely bypass directory blocks.
*   **`ToolSecurityProfile`** (`crates/op-tools/src/security.rs:126`)
    *   All security attributes (e.g., `access_level`, `critical_forbidden_paths`, `rate_limit_per_minute`) are public. Any component can modify these on the fly, rendering safety validations unreliable.

---

## 3. Dead Code Analysis

### 3.1 Unused & Commented-Out Modules
Several files are defined but remain completely unreferenced, disabled, or redundant, introducing dead code that increases the attack surface:

1.  **`builtin_old.rs`** is completely unreferenced by the current entry point. It represents an abandoned, insecure implementation of tools containing command injection vulnerabilities.
2.  **`self_tools.rs`** is commented out in `builtin/mod.rs` (lines 20-21). It contains dangerous self-modification tools that can overwrite codebase files.
3.  **`shell_tool.rs`** is dead code; it has been replaced and duplicated by `builtin/shell.rs`.
4.  **`system.rs`** is defined but never registered or referenced in `builtin/mod.rs`, making the process and memory monitoring commands unusable.
5.  **`packagekit.rs`** is unreferenced dead code; its native D-Bus install functions are never registered.
6.  **`ovs.rs`** is dead code superseded by `builtin/ovs_tools.rs`.
7.  **`openflow_tools.rs`** defines multiple OpenFlow routing rules but is never mounted, making its tools unregisterable.
8.  **`indexer_tools.rs`** is left unreferenced by the main builtin registration sequence.
9.  **`plugin.rs`** is dead code; the tool registration has transitioned to `plugin_state_tool.rs`.
10. **`dbus_tool.rs`** is dead code replaced by the introspection and projection modules.

---

### 3.2 Dead Code Enumeration Table

| Item | Type | Location | Recommendation |
|---|---|---|---|
| `builtin_old.rs` | Module | `crates/op-tools/src/builtin_old.rs:1` | Delete entire file. Obsolete and highly vulnerable to RCE. |
| `self_tools.rs` | Module | `crates/op-tools/src/builtin/self_tools.rs:1` | Remove completely to prevent self-modification security risks. |
| `shell_tool.rs` | Module | `crates/op-tools/src/builtin/shell_tool.rs:1` | Delete. Superseded by `builtin/shell.rs`. |
| `system.rs` | Module | `crates/op-tools/src/builtin/system.rs:1` | Expose and register if system statistics are required. |
| `packagekit.rs` | Module | `crates/op-tools/src/builtin/packagekit.rs:1` | Integrate package management tools or delete. |
| `ovs.rs` | Module | `crates/op-tools/src/builtin/ovs.rs:1` | Delete. Superseded by `ovs_tools.rs`. |
| `openflow_tools.rs` | Module | `crates/op-tools/src/builtin/openflow_tools.rs:1` | Register the OpenFlow tools or delete. |
| `indexer_tools.rs` | Module | `crates/op-tools/src/builtin/indexer_tools.rs:1` | Register the semantic search capability tool or delete. |
| `plugin.rs` | Module | `crates/op-tools/src/builtin/plugin.rs:1` | Delete. Superseded by `plugin_state_tool.rs`. |
| `dbus_tool.rs` | Module | `crates/op-tools/src/builtin/dbus_tool.rs:1` | Delete. Superseded by dynamic introspection. |