# Production Security & Quality Audit: `op-tools`

This document details the production security, compliance, and quality audit of the `op-tools` crate. Five critical, directly exploitable vulnerabilities have been identified along with significant violations of the schema-as-code discipline.

---

## 1. Direct Security Vulnerabilities

### CRITICAL: Arbitrary Command Injection via `args` Array in `builtin_old.rs`
* **File:** `crates/op-tools/src/builtin_old.rs`
* **Line:** 207-210

#### Vulnerability Mechanism
In `ShellTool::execute`, the command and its arguments are processed and executed using:
```rust
match tokio::process::Command::new("sh")
    .arg("-c")
    .arg(format!("{} {}", command, args.join(" ")))
    .output()
    .await
```
The associated `validate` function (line 153) only extracts the first token of the `command` argument to check if it matches the `allowed_commands` list. However, it completely ignores the contents of the `args` array. 

#### Exploit Scenario
An attacker can pass `"command": "ls"` (which is whitelisted) and `"args": ["; rm -rf /"]`. Because the validator only inspects the first word of the `"command"` field, validation succeeds. The formatted string becomes `ls ; rm -rf /`, which is passed directly to `sh -c`, executing the destructive payload with the daemon's privileges.

---

### CRITICAL: Path Traversal and Arbitrary File Overwrite in `self_tools.rs`
* **File:** `crates/op-tools/src/builtin/self_tools.rs`
* **Line:** 42, 183

#### Vulnerability Mechanism
The path validation function `validate_self_path` attempts to restrict file system operations to the self-repository path:
```rust
let full_path = repo_path.join(clean_path);
let canonical = full_path.canonicalize().unwrap_or_else(|_| full_path.clone());
if !canonical.starts_with(&repo_path) { ... }
```
When creating a **new** file via `SelfWriteFileTool::execute` (line 183), `full_path.canonicalize()` fails because the file does not yet exist. It falls back to `full_path.clone()`. 

Because Rust's `Path::starts_with` performs a literal components-based prefix check without resolving relative path segments (`..`), a path such as `/home/user/repo/../etc/cron.d/exploit` textually "starts with" the prefix `/home/user/repo`. The traversal bypasses the validation check and is written via `tokio::fs::write(&full_path, content)`, leading to arbitrary system file writes (e.g., cron jobs, `/etc/shadow`) with root privileges.

---

### CRITICAL: Client-Controlled Privilege Escalation (Session Spoofing) in `shell.rs`
* **File:** `crates/op-tools/src/builtin/shell.rs`
* **Line:** 52, 197

#### Vulnerability Mechanism
`ShellExecuteTool::execute` (line 52) and `ShellExecuteBatchTool::execute` (line 197) extract the `session_id` directly from the user-controlled JSON payload:
```rust
let session_id = input
    .get("session_id")
    .and_then(|v| v.as_str())
    .unwrap_or("default");
```
This parsed `session_id` is then passed directly to the validator:
```rust
validator.check_rate_limit(session_id).await...
```
In `crates/op-tools/src/validation.rs` (lines 53-56), the validation bypass explicitly whitelists certain sessions:
```rust
let mut trusted_sessions = HashSet::new();
trusted_sessions.insert("chatbot".to_string());
trusted_sessions.insert("orchestrator".to_string());
trusted_sessions.insert("system".to_string());
```

#### Exploit Scenario
An untrusted client or an external attacker calling the tool execution endpoint can explicitly include `"session_id": "chatbot"` or `"session_id": "system"` in their JSON payload. This causes the validation engine to classify the session as trusted, bypassing all shell command whitelists, forbidden directory checks, and input sanitization filters.

---

### CRITICAL: Authentication and Authorization Bypass on Tool Execution Router
* **File:** `crates/op-tools/src/router.rs`
* **Line:** 46, 117

#### Vulnerability Mechanism
The Axum router exposes the direct execution of system tools via HTTP POST at `/api/tools/:name/execute` (line 46):
```rust
pub fn create_router(state: ToolsState) -> Router {
    Router::new()
        ...
        .route("/:name/execute", post(execute_tool_handler))
        .with_state(state)
}
```
The associated handler `execute_tool_handler` (line 117) parses the JSON parameters and executes the requested tool directly:
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
There is no authentication middleware, API key check, or authorization handshake on this router. Any entity capable of reaching the HTTP port can invoke powerful administrative tools (such as `shell_execute`, `file_write`, or `dbus_systemd_restart_unit`) and gain full system compromise.

---

### HIGH: Command Validation Bypass via Command Chaining in `security.rs`
* **File:** `crates/op-tools/src/security.rs`
* **Line:** 527

#### Vulnerability Mechanism
To enforce restricted command whitelists, `SecurityValidator::check_command` parses the "base command" using:
```rust
let base_cmd = command
    .split_whitespace()
    .next()
    .ok_or_else(...)?;
```
It then checks whether `base_cmd` exists in the allowed command set. 

If the input string is `ls -la ; rm -rf /`, `split_whitespace().next()` returns `ls`. Since `ls` is whitelisted, the validation checks pass successfully. However, when the command is subsequently executed via `bash -c`, the shell interprets the semicolon `;` (or other control characters like `&&`, `||`, `|`) as a command separator and runs the entire payload, completely defeating the restricted profile restriction.

---

## 2. Dependencies & Feature Inventory

### Direct Dependencies (`crates/op-tools/Cargo.toml`)

| Dependency | Version | Explicitly Enabled Features | Pulled by Default / Indirectly | Security/Quality Flags |
| :--- | :--- | :--- | :--- | :--- |
| `tokio` | Workspace | `full`, `sync` | None | None |
| `async-trait` | Workspace | None | Standard | None |
| `serde` | Workspace | None | Standard | None |
| `simd-json` | Workspace | None | Standard | Uses `unsafe` in-place parsing |
| `serde_json` | Workspace | None | Standard | None |
| `anyhow` | Workspace | None | Standard | None |
| `thiserror` | Workspace | None | Standard | None |
| `tracing` | Workspace | None | Standard | None |
| `clap` | Workspace | None | Standard | None |
| `futures` | Workspace | None | Standard | None |
| `chrono` | Workspace | None | Standard | None |
| `uuid` | Workspace | None | Standard | None |
| `zbus` | Workspace | None | Standard | None |
| `axum` | Workspace | None | Standard | None |
| `reqwest` | Workspace | None | Standard | None |
| `op-state` | Workspace | None | Standard | Internal Crate Dependency |
| `lazy_static` | Workspace | None | Standard | Legacy lazy-initialization |
| `async-recursion` | `1.0` | None | Standard | Unpinned version |
| `dirs` | `5` | None | Standard | Unpinned version |
| `jsonschema` | `0.18` | None | Standard | Version mismatch with workspace (`0.29`) |

### Crate [features] Section
Stated in `crates/op-tools/Cargo.toml`: **None defined**.

---

## 3. Schema-as-Code Compliance Gap Analysis

The codebase operates a hybrid configuration utilizing strongly-typed schemas (Protocol Buffers via `prost` / `tonic` in other parts of the workspace) but fails to utilize versioned schema disciplines within the audited `op-tools` crate.

### Gap 1: Ad-hoc JSON Value Tool Input Schemas
* **File:** `crates/op-tools/src/registry.rs`
* **Line:** 19, 44

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value, // Dynamic simd_json::OwnedValue
    ...
}
```
Instead of compiling schema definitions into static Rust types via code generation (e.g., Protobuf message definitions), data contracts for tool inputs are represented as dynamic, ad-hoc JSON values. Validation is done dynamically at runtime via the `jsonschema` library. This bypasses compile-time type safety and prevents declarative audit trail validation.

### Gap 2: Ad-hoc Serialization of Orchestration Audit Events
* **File:** `crates/op-tools/src/orchestration_plugin.rs`
* **Lines:** 52, 99, 130

Important orchestration logging objects—such as `ToolExecutedEvent`, `LlmDecisionEvent`, and `SessionEvent`—are defined directly as hand-coded Rust structs with ad-hoc JSON fields:
```rust
pub struct ToolExecutedEvent {
    ...
    pub arguments: Value, // Ad-hoc untyped payload
    pub result: ToolExecutionResult,
    ...
    pub metadata: Value,  // Ad-hoc untyped payload
}
```
Because these events are designed to be committed to an immutable ledger (such as a blockchain or audit database), they must be declared in strongly-versioned schemas (e.g., Protobuf `.proto` or OSCAL JSON schemas) to prevent data corruption or serialization mismatch errors as the code evolves.

### Gap 3: Untyped Dynamic D-Bus Projection and JSON Text-Passing
* **File:** `crates/op-tools/src/builtin/plugin_projection.rs`
* **Line:** 95-102

`PluginProjectionTool::execute` directly fetches raw JSON data over D-Bus as a string and parses it into an untyped JSON tree:
```rust
let json_text: String = proxy.get_property::<String>("json_data").await?;
let mut buf = json_text.into_bytes();
let data = simd_json::from_slice::<Value>(&mut buf)...
```
This bypasses versioned RPC serialization. Data contracts are represented as arbitrary string-serialized blobs, violating native protocol parsing structures.

---

## 4. Storage Backend Inventory

The following storage engines are found within the workspace and dependency declarations of the audited files:

| Backend | Found at file:line | Role (KV/Graph/Cache/Queue) |
| :--- | :--- | :--- |
| `cozo` | `Cargo.toml:54` | Relational-Graph-Vector database utilizing `storage-sled` backend. |
| `sqlx` | `Cargo.toml:104` | Relational SQL adapter configured with `sqlite` feature. |
| `rusqlite` | `Cargo.toml:105` | Local embedded SQLite store used by FTS Indexer. |
| `redis` | `Cargo.toml:106` | In-memory key-value state and session store. |

### Architectural Compliance Violations

1. **Local Embedded SQLite for FTS Search Indexing (`crates/op-tools/src/builtin/dbus_search_tool.rs`)**:
   The indexer uses `rusqlite` to build a local FTS5 database. However, the system architecture mandates `cozo` (with `storage-sled`) for semantic metadata and relational knowledge storage. Storing DBus capabilities in a standalone SQLite index fragments the state and bypasses the graph-relational engine of the mandated CozoDB.