# Production Security and Quality Audit: op-tools Crate

This document contains the production security and quality audit of the `op-tools` crate, analyzing its configuration, security parameters, and data contract compliance.

---

## 1. Environment Variable (`std::env::var`) Reads

Below is the complete inventory of all `std::env::var` reads identified within the provided source files.

| File Path | Line Number | Environment Variable | Default Value / Error Handling | Status |
| :--- | :--- | :--- | :--- | :--- |
| `crates/op-tools/src/mcptools.rs` | 81 | `OP_MCPTOOLS_BIN` | Default: `"mcp"` | Safe |
| `crates/op-tools/src/mcptools.rs` | 175 | `OP_MCPTOOLS_SERVERS` | Fallback logic via `if let Ok` | Safe |
| `crates/op-tools/src/mcptools.rs` | 196 | `OP_MCPTOOLS_SERVER` | Fallback logic via `if let Ok` | Safe |
| `crates/op-tools/src/mcptools.rs` | 206 | `OP_MCPTOOLS_SERVER_NAME` | Default: `"default"` | Safe |
| `crates/op-tools/src/mcptools.rs` | 318 | `OP_MCPTOOLS_CONFIG` | Default: `"mcptools.json"` | Safe |
| `crates/op-tools/src/mcptools.rs` | 345 | `OP_MCPTOOLS_ALLOW_UNPREFIXED` | Safe parsing via `.ok().map(...)` | Safe |
| `crates/op-tools/src/builtin/agent_tool.rs` | 190 | `OP_AGENT_INCLUDE` | Handled via `parse_agent_set` -> `.ok()` | Safe |
| `crates/op-tools/src/builtin/agent_tool.rs` | 191 | `OP_AGENT_AUTOSTART` | Handled via `parse_agent_set` -> `.ok()` | Safe |
| `crates/op-tools/src/builtin/agent_tool.rs` | 192 | `OP_AGENT_AUTOSTART_ALL` | Safe parsing via `.ok().map(...)` | Safe |
| `crates/op-tools/src/builtin/agent_tool.rs` | 214 | `var_name` (dynamic) | Handled via `.ok()?` | Safe |
| `crates/op-tools/src/builtin/agent_tool.rs` | 229 | `OP_AGENT_BUS` | Handled via `.ok().and_then(...)` | Safe |
| `crates/op-tools/src/builtin/agent_tool.rs` | 236 | `DBUS_SESSION_BUS_ADDRESS` | Handled via `.is_ok()` | Safe |
| `crates/op-tools/src/builtin/anydesk.rs` | 534 | `DISPLAY` | Handled via `if let Ok` | Safe |
| `crates/op-tools/src/builtin/anydesk.rs` | 539 | `XAUTHORITY` | Handled via `if let Ok` | Safe |
| `crates/op-tools/src/builtin/anydesk.rs` | 545 | `DISPLAY` | Handled via `if let Ok` | Safe |
| `crates/op-tools/src/builtin/anydesk.rs` | 577 | `DISPLAY` | Handled via `if let Ok` | Safe |
| `crates/op-tools/src/builtin/anydesk.rs` | 647 | `DISPLAY` | Handled via `if let Ok` | Safe |
| `crates/op-tools/src/builtin/anydesk.rs` | 657 | `DISPLAY` | Handled via `if let Ok` | Safe |
| `crates/op-tools/src/builtin/code_search.rs` | 115 | `QDRANT_URL` | Default: `"http://127.0.0.1:6333"` | Safe |
| `crates/op-tools/src/builtin/plugin_projection.rs` | 42 | `DBUS_SESSION_BUS_ADDRESS` | Handled via `.is_ok()` | Safe |
| `crates/op-tools/src/builtin/self_tools.rs` | 19 | `OP_SELF_REPO_PATH` | Checked with `.ok()` | Safe |
| `crates/op-tools/src/discovery/sources/dbus.rs` | 132 | `DBUS_SESSION_BUS_ADDRESS` | Handled via `.is_ok()` | Safe |

### Environment Variables Flagged with No Default or No Error Handling
*None.* Every environment variable read identified in the codebase utilizes safe fallback mechanisms (such as `.ok()`, `unwrap_or_else`, or `if let Ok(...)` bindings) rather than throwing panics or failing ungracefully.

---

## 2. Cargo Features Analysis

*   **`op-tools` Features (`crates/op-tools/Cargo.toml`)**:
    *   No local features are defined in this crate's manifest.
*   **Workspace Features (`Cargo.toml`)**:
    *   `default = ["grpc"]`
    *   `grpc = []`

### Additive Behavior
Cargo features are **additive**. If different workspace packages or external crates depend on `op-tools` or other workspace members with different features, the dependency graph compiles with the union of all active features. In this workspace, enabling the `grpc` feature on the root package activates related compilation paths across workspace-wide dependencies.

---

## 3. Hardcoded Paths, Ports, and Addresses

The following static properties bypass runtime configuration structures and are hardcoded into the source code:

### Hardcoded Filesystem Paths
*   `crates/op-tools/src/mcptools.rs:47`: `"mcptools.json"` used as a fallback MCP configuration path.
*   `crates/op-tools/src/mcptools.rs:81`: `"mcp"` used as the default command path for the MCP tool binary.
*   `crates/op-tools/src/validation.rs:89`: Hardcoded allowed paths: `"/tmp"`, `"/var/tmp"`, and `"/home"`.
*   `crates/op-tools/src/validation.rs:96`: Hardcoded blocked system paths: `"/boot"`, `"/dev"`, `"/proc/sys"`, `"/sys"`, `"/root"`, `"/etc/shadow"`, and `"/etc/passwd"`.
*   `crates/op-tools/src/security.rs:196`: Hardcoded restricted paths: `"/etc/shadow"`, `"/etc/sudoers"`, and `"/root"`.
*   `crates/op-tools/src/security.rs:333`: Hardcoded allowed read paths for restricted profiles: `"/tmp"`, `"/var/log"`, `"/home"`, and `"/opt"`.
*   `crates/op-tools/src/security.rs:359`: Hardcoded write path fallback: `"/tmp"`.
*   `crates/op-tools/src/builtin/anydesk.rs:444`: Hardcoded configurations: `"/etc/anydesk/anydesk.conf"`, `"/home/jeremy/.anydesk/anydesk.conf"`, and `"/home/jeremy/.anydesk/user.conf"`.
*   `crates/op-tools/src/builtin/anydesk.rs:608, 615`: Hardcoded Xauthority paths: `"/root/.Xauthority"`, `"/home/jeremy/.Xauthority"`, and `"/home/user/.Xauthority"`.
*   `crates/op-tools/src/builtin/ovs_tools.rs:539`: Hardcoded OVSDB socket: `"/var/run/openvswitch/db.sock"`.
*   `crates/op-tools/src/builtin/ovsdb.rs:22`: Hardcoded OVSDB socket: `"/var/run/openvswitch/db.sock"`.

### Hardcoded Ports
*   `crates/op-tools/src/builtin/anydesk.rs:509`: AnyDesk network port list: `"7070"`, `"6568"`, `"80"`, and `"443"`.
*   `crates/op-tools/src/builtin/code_search.rs:116`: Qdrant port: `6333` inside `"http://127.0.0.1:6333"`.
*   `crates/op-tools/src/builtin/ovs_tools.rs:674`: Wireguard obfuscation ports: `"51820"` and `"443"`.

### Hardcoded Addresses
*   `crates/op-tools/src/builtin/code_search.rs:116`: Qdrant default loopback: `"127.0.0.1"`.
*   `crates/op-tools/src/builtin/rtnetlink_tools.rs:165`: Hardcoded schema IP: `"10.0.0.1"`.
*   `crates/op-tools/src/builtin/rtnetlink_tools.rs:307`: Hardcoded schema gateway IP: `"148.113.204.1"`.

---

## 4. Schema-as-Code Compliance

This codebase utilizes a **schema-as-code** methodology to define contract boundaries. However, several modules define ad-hoc interfaces or inline schemas directly in the source as strings/unversioned structures rather than consuming compiled, versioned schema files (such as Protocol Buffers or structured OSCAL documents).

### Non-Compliant Ad-Hoc Schemas
Virtually all tools implement the `input_schema` function by defining ad-hoc schemas using the `simd_json::json!` or `serde_json::json!` macros. The contracts are not tied to a single source of truth or versioned schemas:

*   `crates/op-tools/src/builtin_old.rs:20, 62, 115, 204`
*   `crates/op-tools/src/dynamic_tool.rs:89`
*   `crates/op-tools/src/builtin/agent_tool.rs:384`
*   `crates/op-tools/src/builtin/anydesk.rs:49, 93, 137, 187, 231, 275`
*   `crates/op-tools/src/builtin/dbus.rs:26, 80, 134, 188, 240`
*   `crates/op-tools/src/builtin/dbus_hybrid.rs:98`
*   `crates/op-tools/src/builtin/dbus_search_tool.rs:28, 143, 227`
*   `crates/op-tools/src/builtin/dbus_tool.rs:115`
*   `crates/op-tools/src/builtin/dinit.rs:111, 153, 195, 232`
*   `crates/op-tools/src/builtin/error_reporting_tool.rs:19`
*   `crates/op-tools/src/builtin/file.rs:113`
*   `crates/op-tools/src/builtin/gcloud_tools.rs:151, 207, 273, 335`
*   `crates/op-tools/src/builtin/incus_tools.rs:61, 102, 161, 203, 271, 313, 370, 429`
*   `crates/op-tools/src/builtin/lxc_tools.rs:20, 59, 101, 143, 239, 297, 363, 442`
*   `crates/op-tools/src/builtin/ovs.rs:31`
*   `crates/op-tools/src/builtin/ovs_tools.rs:25, 59, 99, 137, 185, 227, 283, 321, 371, 417, 467, 539`
*   `crates/op-tools/src/builtin/ovsdb.rs:391, 441, 483, 513, 563, 613, 655`
*   `crates/op-tools/src/builtin/packagekit.rs:19, 75`
*   `crates/op-tools/src/builtin/plugin.rs:31`
*   `crates/op-tools/src/builtin/plugin_state_tool.rs:75`
*   `crates/op-tools/src/builtin/procfs.rs:176, 226, 277, 340`
*   `crates/op-tools/src/builtin/respond_tool.rs:21, 65`
*   `crates/op-tools/src/builtin/response_tools.rs:111, 219, 313`
*   `crates/op-tools/src/builtin/rtnetlink_tools.rs:23, 103, 141, 199, 241, 283, 331, 373, 411`
*   `crates/op-tools/src/builtin/self_tools.rs:81, 151, 219, 283, 347, 393, 441, 497, 551, 607`
*   `crates/op-tools/src/builtin/shell_tool.rs:21, 143, 227`
*   `crates/op-tools/src/builtin/indexer_tools.rs:17`
*   `crates/op-tools/src/builtin/openflow_tools.rs:24, 91, 137, 185, 249`

---

## 5. Security & Quality Audit Findings

### Finding 1: Validation Bypass and Command Injection in `builtin_old.rs` [CRITICAL]
*   **Location**: `crates/op-tools/src/builtin_old.rs:120-136` (Validation) and `crates/op-tools/src/builtin_old.rs:167-171` (Execution).
*   **Vulnerability Description**:
    The `ShellTool` validates shell commands using the following logic:
    ```rust
    let base_cmd = command.split_whitespace()
        .next()
        .unwrap_or(command);
    
    if !self.allowed_commands.iter().any(|c| c == base_cmd) { ... }
    ```
    This approach only parses the very first whitespace-separated token of the command string. However, because the command is later spawned using the system shell:
    ```rust
    match tokio::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{} {}", command, args.join(" ")))
    ```
    an attacker can bypass validation entirely by injecting shell metacharacters (e.g. `;`, `&&`, `||`, or `|`) after the first word.
*   **Proof of Concept**:
    If `"ls"` is on the allowed list, the payload `"ls; rm -rf /"` results in a `base_cmd` of `"ls"`. The validator returns `Ok(())`. The execution block then runs `sh -c "ls; rm -rf / "`, resulting in arbitrary system command execution.
*   **Remediation**:
    Avoid raw execution via shell wrapper environments (`sh -c`). Execute binary arguments directly as distinct elements inside `Command::new()`. If shell scripting is required, enforce strict character whitelist verification to prevent command chaining.

### Finding 2: Unauthenticated Remote Code Execution in Tools Router [CRITICAL]
*   **Location**: `crates/op-tools/src/router.rs:88-142` (specifically `execute_tool_handler` at line 125).
*   **Vulnerability Description**:
    The HTTP endpoint `POST /api/tools/:name/execute` is exposed publicly via Axum without any session validation, authentication tokens, or authorization checks:
    ```rust
    async fn execute_tool_handler(
        State(state): State<ToolsState>,
        axum::extract::Path(name): axum::extract::Path<String>,
        Json(params): Json<Value>,
    ) -> impl IntoResponse {
        if let Some(tool) = state.registry.get(&name).await {
            match tool.execute(params).await { ... }
        }
    }
    ```
    This completely bypasses the security philosophy stated in `lib.rs` (security at the access level). Any remote actor with network visibility of `/api/tools/` can execute any registered tool directly.
*   **Impact**:
    Because highly privilege-invasive tools (such as `ShellExecuteTool`, `FileWriteTool`, and `SelfWriteFileTool`) are registered, any unauthenticated attacker can execute arbitrary commands as the host service user (often `root`), read/write sensitive system files, or overwrite the application binary itself.
*   **Remediation**:
    Apply authentication middleware (such as OAuth2 bearer checks or local API tokens) on the entire tools router. Enforce role-based access control inside `execute_tool_handler` prior to invoking `tool.execute()`.

### Finding 3: Unprotected Shell Execution in `builtin/shell_tool.rs` [CRITICAL]
*   **Location**: `crates/op-tools/src/builtin/shell_tool.rs:43-98`.
*   **Vulnerability Description**:
    There are two parallel implementations of `ShellExecuteTool` in this codebase: one in `builtin/shell.rs` and another in `builtin/shell_tool.rs`. The implementation inside `builtin/shell_tool.rs` completely omits validation via `SecurityValidator`.
    ```rust
    async fn execute(&self, request: ToolRequest) -> ToolResult {
        ...
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            execute_command(command, working_dir),
        )
        .await;
    ```
    This executes commands directly via bash without checking `check_command` or validating the user's privilege profile, introducing an immediate privilege escalation vector.
*   **Remediation**:
    Remove duplicate or old implementations. Ensure that every shell/filesystem tool uniformly delegates authorization to the global `SecurityValidator` instance.