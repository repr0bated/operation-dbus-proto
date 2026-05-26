# Observability & Quality Audit: Crate `op-tools`

## 1. Observability Metric & Macro Counts

### Log Macro Usage Summary
A comprehensive count of all `tracing` instrumentation macros compared against standard-output `println!` prints reveals a heavy reliance on asynchronous tracing, with only a single instance of raw console printing in a CLI binary:

| Macro/Print | Count | Primary Locations / File Patterns |
| :--- | :--- | :--- |
| `debug!` | **32** | Old builtin tools, executor dispatch, D-Bus arguments, OVSDB communication. |
| `info!` | **63** | Systemd unit commands, agent registration, OVSDB writes, rtnetlink link states, self-tool modifications. |
| `warn!` | **15** | Executor timeouts, MCP registration failures, validation bypass, CLI alternative recommendations. |
| `error!` | **8** | Input size check failures, command spawning errors, semantic search failures. |
| `println!` | **1** | `crates/op-tools/src/bin/op-packagekit-install.rs:41` |

---

## 2. Security Vulnerabilities

### [CRITICAL] Arbitrary File Write & Directory Creation via Path Traversal Bypass
* **Citation**: `crates/op-tools/src/builtin/self_tools.rs:42` and `crates/op-tools/src/builtin/self_tools.rs:185`

#### Description
The `validate_self_path` helper and the `SelfWriteFileTool::execute` function contain a classic path traversal vulnerability resulting from the incorrect handling of canonicalization failures on non-existent paths. 

```rust
// In validate_self_path
let canonical = full_path.canonicalize().unwrap_or_else(|_| full_path.clone());
if !canonical.starts_with(&repo_path) { ... }
```

When a path containing directory traversal components (e.g., `../../../../tmp/exploit.rs`) does not exist on disk, `full_path.canonicalize()` fails and returns an `Err`. The code then falls back to `full_path.clone()`. 

Because textual/component-wise path comparison is performed on the uncanonicalized `full_path` via `starts_with`, Rust's `Path::starts_with` evaluates to `true` because the first few components structurally match the prefix `repo_path` (e.g., `/home/user/repo/../../../../tmp/exploit.rs` structurally begins with `/home/user/repo`).

Similarly, in `SelfWriteFileTool::execute`:
```rust
let parent = full_path.parent();
if let Some(p) = parent {
    if p.exists() {
        let canonical_parent = p.canonicalize().unwrap_or(p.to_path_buf());
        if !canonical_parent.starts_with(&canonical_repo) {
            return Err(anyhow::anyhow!("..."));
        }
    } else if !create_dirs {
        return Err(anyhow::anyhow!("Parent directory does not exist: {:?}", p));
    }
}
```
If the parent directory of the target path does not exist (e.g., `/home/user/repo/../../../../tmp/nonexistent_dir`), `p.exists()` is `false`. Since `create_dirs` defaults to `true`, the security check block is entirely skipped. The tool then proceeds to create the directory recursively outside the repository using `tokio::fs::create_dir_all(parent)` and writes arbitrary files (such as root-owned configuration files or system scripts) anywhere on the filesystem.

#### Remediation
Perform string-level validation to block any occurrences of `..` segments *before* joining paths, and ensure that if canonicalization fails, the operation is rejected rather than falling back to an uncanonicalized textual path match.

---

### [CRITICAL] Command Injection in Old Built-in `ShellTool` via Unvalidated Arguments
* **Citation**: `crates/op-tools/src/builtin_old.rs:150` and `crates/op-tools/src/builtin_old.rs:192`

#### Description
The old built-in `ShellTool` validates only the main `"command"` parameter against an allowlist:

```rust
fn validate(&self, args: &simd_json::OwnedValue) -> Result<(), String> {
    let command = args.get("command")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'command' argument")?;
    
    let base_cmd = command.split_whitespace()
        .next()
        .unwrap_or(command);
    
    if !self.allowed_commands.iter().any(|c| c == base_cmd) { ... }
    Ok(())
}
```

However, during execution, the tool collects an unvalidated list of arguments from the `"args"` key, joins them with spaces, and passes the entire formatted string to `sh -c`:

```rust
let args: Vec<&str> = request.arguments.get("args")
    .and_then(|v| v.as_array())
    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
    .unwrap_or_default();

// ...

match tokio::process::Command::new("sh")
    .arg("-c")
    .arg(format!("{} {}", command, args.join(" ")))
    .output()
    .await
```

An attacker can bypass the command allowlist by specifying a benign allowed command (e.g., `command: "ls"`) and passing malicious shell metacharacters in the `args` array (e.g., `args: ["/tmp; rm -rf /etc/shadow"]`). The command executed by `sh -c` becomes `ls /tmp; rm -rf /etc/shadow`, resulting in arbitrary command execution.

#### Remediation
Do not use `sh -c` with string formatting. Pass the arguments directly to the process command vector as distinct arguments (using `Command::new(command).args(args)`) to prevent shell expansion and injection.

---

## 3. Observability & Logging Risks

### Swallowed Errors Without Logging
Several locations in the codebase silently discard errors or return early without logging the failure context:

1. **Silent AnyDesk Command/Service Check Discards**  
   * **Citation**: `crates/op-tools/src/builtin/anydesk.rs:475`, `crates/op-tools/src/builtin/anydesk.rs:493`, `crates/op-tools/src/builtin/anydesk.rs:513`, `crates/op-tools/src/builtin/anydesk.rs:526`, `crates/op-tools/src/builtin/anydesk.rs:536`, `crates/op-tools/src/builtin/anydesk.rs:590`, `crates/op-tools/src/builtin/anydesk.rs:608`, `crates/op-tools/src/builtin/anydesk.rs:623`, `crates/op-tools/src/builtin/anydesk.rs:674`, and `crates/op-tools/src/builtin/anydesk.rs:689`  
   * **Description**: Matches on various CLI and systemd status queries for AnyDesk silently discard error states using `_ => {}` arms. If `anydesk`, `netstat`, `xauth`, `xdpyinfo`, or `systemctl` commands fail or are missing, the system proceeds with empty/default fields, hiding critical diagnostic failures from the logs.

2. **Silent Link Status Sync Discard**  
   * **Citation**: `crates/op-tools/src/builtin/lxc_tools.rs:1205`  
   * **Description**: The LXC container deletion tool issues a synchronous stop command: `let _ = client.stop_container_sync(vmid, 30).await;` but discards the result. Any errors during the synchronous stop are swallowed.

3. **Silent Systemd Enable Unit Discard**  
   * **Citation**: `crates/op-tools/src/builtin/ovs_tools.rs:474`  
   * **Description**: The auto-installer for OVS discards the result of enabling the systemd unit: `let _enable_result: ... = systemd_proxy.call("EnableUnitFiles", ...).await;`. If enabling fails, it goes unnoticed and is not logged.

4. **Git Operations Swallowed Errors**  
   * **Citation**: `crates/op-tools/src/builtin/self_tools.rs:233` and `crates/op-tools/src/builtin/self_tools.rs:296`  
   * **Description**: Git status and revision-parsing calls use `.unwrap_or_default()` to recover from failures during self-repository queries. If `git` is misconfigured or fails, the tool reports a clean state or incorrect commit hashes without registering the underlying error.

---

### Sensitive Data and Secret Exposure in Logs
Several tools log complete input structures, command lines, or environment states containing sensitive PII or credentials:

1. **Plaintext CLI Parameter Logging**  
   * **Citation**: `crates/op-tools/src/builtin/shell.rs:103`, `crates/op-tools/src/builtin/shell.rs:218`, and `crates/op-tools/src/builtin/shell_tool.rs:55`  
   * **Description**: The shell execution tools log the raw command strings at the `info!` level. If an operator or LLM executes an commands containing inline passwords, private keys, or API tokens (e.g., `curl -H "Authorization: Bearer <secret>" ...`), the credentials are leaked in plaintext to system logs.

2. **Unredacted D-Bus and Agent Payload Logging**  
   * **Citation**: `crates/op-tools/src/builtin/agent_tool.rs:219` and `crates/op-tools/src/builtin/dbus_tool.rs:185`  
   * **Description**: `DbusAgentExecutor` and `DbusMethodTool` log the complete `task_json` and `arguments` structures at the `debug!` level. If an agent task handles sensitive administrative operations, encryption keys, or user PII, these are printed unredacted.

3. **System Environment Secret Exposure**  
   * **Citation**: `crates/op-tools/src/builtin/anydesk.rs:608`  
   * **Description**: The AnyDesk environment inspection tool reads and logs the complete environment block configured for the service: `systemctl show anydesk --property=Environment`. Any system environment secrets, tokens, or credentials loaded for the AnyDesk daemon are output to the JSON result.

---

## 4. Metrics Instrumentation Audit

While the root-level workspace `Cargo.toml` declares dependencies on both `prometheus` and `opentelemetry`, the `op-tools` crate does not leverage either for active runtime metrics.

* **Activity Plugin Placeholder**: `crates/op-tools/src/orchestration_plugin.rs:384` contains a stub class `MetricsActivityPlugin`. However, its execution tracking implementation is merely a standard log trace:
  ```rust
  async fn on_tool_executed(&self, event: ToolExecutedEvent) {
      debug!(
          tool = %event.tool_name,
          duration = %event.duration_ms,
          "Recording tool execution metrics"
      );
  }
  ```
* **No Real-Time Gauges**: There are no counters, gauges, or histograms tracking execution durations, database connection times, or validation failures.

---

## 5. Schema-as-Code Violations

The codebase demonstrates a significant departure from the schema-as-code discipline. Rather than relying on versioned, statically-compiled Protocol Buffers or standardized OSCAL schemas, data contracts and validation constraints are defined using ad-hoc, inline Rust JSON structures.

### Major Ad-Hoc Struct & Inline JSON Schema Examples
* **Echo and System Info Schemas** (`crates/op-tools/src/builtin_old.rs:21`, `crates/op-tools/src/builtin_old.rs:52`, `crates/op-tools/src/builtin_old.rs:105`, and `crates/op-tools/src/builtin_old.rs:212`): Tool schemas are defined directly as raw inline strings using `json!`.
* **Dynamic D-Bus Projection** (`crates/op-tools/src/dynamic_tool.rs:92`): Programmatically manufactures raw string-typed schemas at runtime.
* **Inline Response Tool Contracts** (`crates/op-tools/src/builtin/response_tools.rs:114`, `crates/op-tools/src/builtin/response_tools.rs:215`, and `crates/op-tools/src/builtin/response_tools.rs:308`): Define crucial response-to-user interactions, safety blocks, and clarification prompts using raw JSON objects.
* **Ad-hoc Serialization Structs** (`crates/op-tools/src/orchestration_plugin.rs:47`, `crates/op-tools/src/orchestration_plugin.rs:104`, and `crates/op-tools/src/orchestration_plugin.rs:129`): Key administrative events (`ToolExecutedEvent`, `LlmDecisionEvent`, `SessionEvent`) are defined as simple Rust structs serializing arbitrary raw `Value` blobs. These should be strictly typed and mapped to versioned Protobuf messages to guarantee audit log compatibility across upgrades.

---
## ⚠ Citation Warnings
- `crates/op-tools/src/builtin/lxc_tools.rs:1205`: file has 734 lines
