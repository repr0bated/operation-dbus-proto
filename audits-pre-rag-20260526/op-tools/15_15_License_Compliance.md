# LICENSE AUDIT

### License Field Extraction
* **crates/op-tools/Cargo.toml**: Inherits license from the workspace via `license.workspace = true`.
* **Cargo.toml (Workspace)**: Declares `license = "Apache-2.0"` under `[workspace.package]`.
* **Extracted License**: **Apache-2.0**

### Cargo.lock GPL/AGPL/SSPL Scan
A complete scan of the `Cargo.lock` dependencies reveals **no** GPL, AGPL, or SSPL licensed crates. The dependency tree is fully compliant with commercial-friendly licensing policies.
* *Note on Cozo*: The `cozo` crate (`version = "0.7.6"`) is licensed under the **Mozilla Public License 2.0 (MPL-2.0)**, which is a weak copyleft license. It is commercially compatible and does not trigger copyleft contamination of the surrounding Apache-2.0 source code.

### Crates with No License Field
No crates within the workspace are missing a license field. `crates/op-tools` explicitly inherits the workspace license.

---

# SECURITY & QUALITY AUDIT FINDINGS

## CRITICAL FINDINGS

### 1. crates/op-tools/src/builtin/self_tools.rs:214-225
#### Path Traversal Bypass to Arbitrary File Write in `SelfWriteFileTool`
The path canonicalization and confinement checks can be entirely bypassed by providing a path pointing to a non-existent subdirectory:

```rust
let parent = full_path.parent();
if let Some(p) = parent {
    if p.exists() { // <--- VULNERABILITY: If parent doesn't exist, block is skipped
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

If an attacker supplies a path containing directory traversal components that lead to a non-existent directory (e.g., `../../../../tmp/new_dir/payload.rs`), `p.exists()` returns `false`, causing the validation logic to skip the prefix check completely. Because the default configuration of the tool has `create_dirs` enabled, the tool will subsequently run `tokio::fs::create_dir_all` to create the target directory outside of the repository and write the arbitrary payload file.

#### Remediation
Perform path canonicalization on the prospective target path prior to validating its presence or existence on disk, and ensure it always resolves within the `canonical_repo` boundary:

```rust
let clean_path = path.trim_start_matches('/');
let full_path = repo_path.join(clean_path);
let canonical_repo = repo_path.canonicalize()?;

// Resolve parent boundaries even if they do not yet exist
let mut ancestor = full_path.clone();
while let Some(parent) = ancestor.parent() {
    if parent.exists() {
        let canonical_parent = parent.canonicalize()?;
        if !canonical_parent.starts_with(&canonical_repo) {
            return Err(anyhow::anyhow!("Access denied: Path escapes repository boundary."));
        }
        break;
    }
    ancestor = parent.to_path_buf();
}
```

---

### 2. crates/op-tools/src/router.rs:49-55 & crates/op-tools/src/router.rs:133-149
#### Unauthenticated Remote Code Execution and D-Bus Hijacking
The HTTP API endpoints exposed by the tools router do not implement any authentication, session validation, or access level control checks:

```rust
pub fn create_router(state: ToolsState) -> Router {
    Router::new()
        .route("/", get(list_tools_handler))
        .route("/health", get(health_handler))
        .route("/:name", get(get_tool_handler))
        .route("/:name/execute", post(execute_tool_handler)) // <--- Unauthenticated execution endpoint
        .with_state(state)
}
```

Any network-adjacent actor can post to `/api/tools/:name/execute` and trigger powerful administrative tools, including `shell_execute` and `dbus_call_method`. Additionally, since the default configuration runs with full administrator privileges, this enables remote unauthenticated attackers to execute arbitrary bash commands or perform sensitive D-Bus method invocations (such as stopping/restarting critical systemd units).

#### Remediation
Integrate authentication and authorization middleware (e.g., JWT validation or API token gating) into the router setup, and enforce session-scoped security profiles on tool execution:

```rust
pub fn create_router(state: ToolsState) -> Router {
    Router::new()
        .route("/:name/execute", post(execute_tool_handler))
        .route_layer(axum::middleware::from_fn(auth_middleware)) // Implement strict auth gating
        .with_state(state)
}
```

---

### 3. crates/op-tools/src/builtin/file.rs:163-188 & crates/op-tools/src/security.rs:241-275
#### File Exfiltration and Corruption via Unrestricted Default Admin Profile
The filesystem tools `file_read` and `file_write` delegate authorization checks to `validate_read_path` and `validate_write_path` within `SecurityValidator`. By default, the global `SecurityValidator` is initialized with the `admin` profile:

```rust
pub fn admin() -> Self {
    Self {
        name: "admin".to_string(),
        access_level: AccessLevel::Unrestricted,
        custom_allowed_commands: None,
        critical_forbidden_paths: vec![
            // Only truly critical paths that could break the system
        ],
        ...
    }
}
```

Since `critical_forbidden_paths` is empty in the `admin` profile, the security validator allows the execution of read/write operations on any absolute path. Combined with the lack of authentication in the HTTP router, any remote attacker can read and overwrite sensitive files (such as `/etc/shadow`, ssh keys, or configuration files) by sending JSON payloads to the unauthenticated HTTP execute endpoints.

#### Remediation
Enforce a "deny-by-default" policy for unauthenticated or non-admin sessions. Do not default the global validator to unrestricted root access; instead, force explicit authentication and session initialization to elevate the active profile to `Unrestricted`.

---

## HIGH FINDINGS

### 4. crates/op-tools/src/builtin_old.rs:124 & crates/op-tools/src/builtin_old.rs:139
#### Complete Validation Bypass in Legacy `ShellTool`
The `ShellTool` in `builtin_old.rs` defines a validation helper (`validate`) to restrict executed commands to an allowed command list:

```rust
fn validate(&self, args: &simd_json::OwnedValue) -> Result<(), String> { ... }
```

However, the corresponding `execute` function for `ShellTool` completely fails to call `self.validate` before running the command:

```rust
async fn execute(&self, request: ToolRequest) -> ToolResult {
    let start = std::time::Instant::now();
    
    let command = match request.arguments.get("command").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => { ... }
    };
    // ... command is directly executed without validating allowed_commands!
```

This renders the validation configuration useless, permitting execution of any arbitrary command passed through this tool.

#### Remediation
Invoke `self.validate` as the very first step in the `execute` body of `ShellTool`:

```rust
async fn execute(&self, request: ToolRequest) -> ToolResult {
    let start = std::time::Instant::now();
    if let Err(err) = self.validate(&request.arguments) {
        return ToolResult::error(&request.id, &err, start.elapsed().as_millis() as u64);
    }
    // ... proceed with execution
}
```

---

### 5. crates/op-tools/src/executor.rs:92
#### Compilation Failure: Missing Method on `ToolRegistry`
The `ToolExecutor::execute` method attempts to call `self.registry.execute(...)`:

```rust
let timeout_result = timeout(duration, self.registry.execute(request.clone())).await;
```

However, the `ToolRegistry` struct defined in `crates/op-tools/src/registry.rs` does not implement any method named `execute`. This triggers a compilation failure when trying to build the `op-tools` crate.

#### Remediation
Implement the dispatch/execution logic inside `ToolRegistry` or update the `ToolExecutor` to fetch the tool from the registry first and then execute it directly:

```rust
if let Some(tool) = self.registry.get(&request.tool_name).await {
    let timeout_result = timeout(duration, tool.execute(request.arguments.clone())).await;
    // ...
}
```

---

## LOW / QUALITY FINDINGS

### 6. crates/op-tools/src/builtin/anydesk.rs:524 & crates/op-tools/src/discovery/sources/agent.rs:21
#### Hardcoded Home Paths
Multiple files contain hardcoded system home paths (e.g., `/home/jeremy/`) to find configuration files and directory sources:

* **anydesk.rs:524**: `"/home/jeremy/.anydesk/anydesk.conf"`
* **agent.rs:21**: `dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/jeremy"))`

This will result in configuration errors or incorrect fallbacks when deployed on production servers with different user accounts.

#### Remediation
Dynamically locate the system home directory via environment variables or the `dirs` crate, and bubble up an error rather than resorting to a hardcoded developer username fallback.

---

### 7. crates/op-tools/src/builtin_old.rs:258-287
#### Insecure Custom Base64 Encoding
The legacy builtin tools define a custom Base64 encoder module `base64`. Custom implementations of standard codecs are more error-prone and miss target-specific optimization and auditing of mainstream libraries.

#### Remediation
Replace the custom `base64` module with the robust and universally utilized `base64` workspace dependency already declared in the Cargo manifest.

---
## ⚠ Citation Warnings
- `crates/op-tools/src/router.rs:133`: file has 130 lines
