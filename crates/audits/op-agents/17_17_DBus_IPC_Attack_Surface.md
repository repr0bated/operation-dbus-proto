# D-Bus & IPC Attack Surface Audit Report

---

### 1. D-Bus & IPC Attack Surface Registry

The D-Bus API surface is exposed via the `DbusAgentService` struct, which implements the standard `org.dbusmcp.Agent` D-Bus interface.

#### Exposed D-Bus Interface: `org.dbusmcp.Agent`
* **Object Paths**: `/org/dbusmcp/Agent/{AgentType}` or `/org/dbusmcp/Agent/{AgentType}/{InstanceSuffix}`
* **Well-known Names**: `org.dbusmcp.Agent.{AgentType}` or `org.dbusmcp.Agent.{AgentType}.{InstanceSuffix}`

#### Methods
* **`execute`**
  * **Signature**: `execute(task_json: String) -> Result<String, zbus::fdo::Error>`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:107`
  * **Caller Identity Check**: None
* **`run_operation`**
  * **Signature**: `run_operation(operation: String, path: String, args: String) -> Result<String, zbus::fdo::Error>`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:141`
  * **Caller Identity Check**: None
* **`agent_type`**
  * **Signature**: `agent_type() -> &str`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:163`
  * **Caller Identity Check**: None
* **`agent_id`**
  * **Signature**: `agent_id() -> &str`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:168`
  * **Caller Identity Check**: None
* **`name`**
  * **Signature**: `name() -> String`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:173`
  * **Caller Identity Check**: None
* **`description`**
  * **Signature**: `description() -> String`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:179`
  * **Caller Identity Check**: None
* **`operations`**
  * **Signature**: `operations() -> Vec<String>`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:185`
  * **Caller Identity Check**: None
* **`supports_operation`**
  * **Signature**: `supports_operation(operation: String) -> bool`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:191`
  * **Caller Identity Check**: None
* **`status`**
  * **Signature**: `status() -> String`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:197`
  * **Caller Identity Check**: None
* **`security_profile`**
  * **Signature**: `security_profile() -> String`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:203`
  * **Caller Identity Check**: None
* **`metadata`**
  * **Signature**: `metadata() -> String`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:210`
  * **Caller Identity Check**: None
* **`ping`**
  * **Signature**: `ping() -> bool`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:231`
  * **Caller Identity Check**: None

#### Signals
* **`task_completed`**
  * **Signature**: `task_completed(signal_ctxt: &SignalContext<'_>, task_id: &str, success: bool, result_json: &str) -> zbus::Result<()>`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:242`
* **`status_changed`**
  * **Signature**: `status_changed(signal_ctxt: &SignalContext<'_>, new_status: &str) -> zbus::Result<()>`
  * **File Citation**: `crates/op-agents/src/dbus_service.rs:252`

---

### 2. Caller Identity & Authentication Gaps

#### Absence of Identity/Authentication Verification (Critical)
* **Finding**: The D-Bus methods `execute` (`crates/op-agents/src/dbus_service.rs:107`) and `run_operation` (`crates/op-agents/src/dbus_service.rs:141`) allow callers to execute arbitrary backend commands, compile code, and read system metrics. No checks are made against the caller's credentials (such as UID, GID, or process ID) during method invocation.
* **Exploitation**: Any unprivileged local process on the system bus can send method calls to a registered D-Bus service, bypassing standard Linux user boundaries and executing commands with the privileges of the D-Bus service process (which defaults to running as `root` for system-related agents, as seen in `dbus-agent-manager.rs` for agents with `requires_root: true`).

#### Bus Connection Types
* **Session vs. System Bus**: The binary `dbus-agent-manager.rs` defaults to connecting to the **System Bus** unless the environment variable `DBUS_AGENT_SESSION` is explicitly defined:
  ```rust
  let bus_type = if std::env::var("DBUS_AGENT_SESSION").is_ok() {
      info!("Using session bus");
      BusType::Session
  } else {
      info!("Using system bus");
      BusType::System
  };
  ```
  *(See `crates/op-agents/src/bin/dbus-agent-manager.rs:191-205`)*
* **D-Bus Policy Gap**: There are no D-Bus policy files (e.g., XML security files placed in `/etc/dbus-1/system.d/`) provided in the codebase. Without these policies explicitly configured on the host system to deny unprivileged sender access, the system bus connection exposes privileged system commands (such as executing shell scripts, altering database states, and running network probes) to any local user.

---

### 3. Deserialization and Input Validation Vulnerabilities

#### `unsafe` Deserialization of Unvalidated Caller Input (Critical)
* **Finding**: The `execute` method deserializes raw, caller-supplied JSON string parameters using `unsafe` blocks in `simd_json`:
  ```rust
  let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }.map_err(|e| { ... })
  ```
  *(See `crates/op-agents/src/dbus_service.rs:114-115`)*
* **Analysis**: `simd-json`'s `from_str` relies on destructive in-place mutation of the input buffer. Using `unsafe { simd_json::from_str(...) }` on raw strings directly supplied by an untrusted IPC peer bypasses critical length, structure, and padding safety checks. If the IPC caller passes maliciously malformed JSON, concurrent mutations or boundary issues in the underlying memory can trigger memory corruption, buffer overflows, or undefined behavior.
* **Registry Code Parsing**: A similar pattern is found in `agent_registry.rs` during config parsing:
  ```rust
  let specs: Vec<AgentSpec> = unsafe { simd_json::from_str(&mut content) }
  ```
  *(See `crates/op-agents/src/agent_registry.rs:434-436`)*

#### Argument Injection Leading to Arbitrary Code Execution (Critical)
* **Finding**: The validation mechanism in `validation::validate_args` checks arguments against `FORBIDDEN_CHARS` but allows hyphens (`-`).
  ```rust
  pub const FORBIDDEN_CHARS: &[char] = &[
      '$', '`', ';', '&', '|', '>', '<', '(', ')', '{', '}', '\n', '\r', '\0',
  ];
  ```
  *(See `crates/op-agents/src/security/validation.rs:10-12`)*
* **Vulnerable Implementation (`code_reviewer.rs`)**:
  ```rust
  fn git_diff(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
      let mut cmd = Command::new("git");
      cmd.arg("diff");

      if let Some(a) = args {
          validation::validate_args(a)?;
          for arg in a.split_whitespace() {
              cmd.arg(arg);
          }
      }
  ```
  *(See `crates/op-agents/src/agents/analysis/code_reviewer.rs:74-85`)*
* **Exploitation Path**: 
  1. An attacker sends a D-Bus message to the `CodeReviewer` agent's `git_diff` method (exposed via `execute` with `operation: "diff"`).
  2. The attacker supplies `args` containing a flag injection payload: `--ext-diff=sh`.
  3. `validation::validate_args` does not reject `--ext-diff=sh` because none of those characters are in `FORBIDDEN_CHARS` (no semicolons, parentheses, or dollars are needed).
  4. The code splits `args` on whitespace and appends each as an argument to the process.
  5. The generated command executes `git diff --ext-diff=sh`. When Git runs the diff, it executes `sh` as an external diff engine. This bypasses the command whitelist and spawns an arbitrary interactive shell with the privileges of the agent manager process.

---

### 4. Schema-as-Code Violations

The codebase frequently bypasses schema-as-code discipline, expressing core data contracts as ad-hoc Rust structs, manually serializing them to raw JSON strings, or manipulating them as untyped JSON values.

#### Ad-hoc JSON Serialization & Construction
* **Agent Metadata**: The `metadata` method manually formats JSON data contracts using the untyped `simd_json::json!` macro:
  ```rust
  simd_json::json!({
      "agent_type": self.agent_type,
      "agent_id": self.agent_id,
      "name": agent.name(),
      "description": agent.description(),
      "operations": agent.operations(),
      "status": agent.get_status(),
      "security": {
          "category": format!("{:?}", profile.config.category),
          "timeout_secs": profile.config.timeout_secs,
          "requires_root": profile.config.requires_root,
      }
  })
  .to_string()
  ```
  *(See `crates/op-agents/src/dbus_service.rs:210-227`)*
* **Cognitive Memory Entries**: The `MemoryAgent` manually builds serialized JSON strings using string formatting instead of structured schema serializers:
  ```rust
  let entry_json = format!(
      "\"{}\":{{\"value\":\"{}\",\"memory_type\":\"{}\",\"tags\":[{}],\"created_at\":{},\"updated_at\":{},\"access_count\":{},\"last_accessed\":{}{}}}",
      key, entry.value, memory_type_str, tags_json, entry.created_at, entry.updated_at, 
      entry.access_count, entry.last_accessed, expires_json
  );
  ```
  *(See `crates/op-agents/src/agents/orchestration/memory.rs:204-209`)*

#### Ad-hoc Data Structures (Non-Versioned)
Instead of utilizing versioned Protocol Buffers or structured OSCAL schemas for policy/task description, the system relies on ad-hoc structs:
* **`AgentTask`**: Defined as an ad-hoc JSON structure with untyped configuration blocks.
  ```rust
  pub struct AgentTask {
      #[serde(rename = "type")]
      pub task_type: String,
      pub operation: String,
      #[serde(default)]
      pub path: Option<String>,
      #[serde(default)]
      pub args: Option<String>,
      #[serde(default)]
      pub config: HashMap<String, simd_json::OwnedValue>,
  }
  ```
  *(See `crates/op-agents/src/agents/base.rs:14-31`)*
* **`AgentSpec`**: Configured as an ad-hoc struct loaded from unversioned directory configurations.
  ```rust
  pub struct AgentSpec {
      pub agent_type: String,
      pub name: String,
      pub description: String,
      pub command: String,
      pub args: Vec<String>,
      pub env: HashMap<String, String>,
      pub working_dir: Option<PathBuf>,
      pub capabilities: Vec<String>,
      pub requires_root: bool,
      pub max_instances: usize,
      pub restart_policy: RestartPolicy,
      pub health_check: Option<HealthCheck>,
  }
  ```
  *(See `crates/op-agents/src/agent_registry.rs:21-52`)*
* **`AgentRequest` & `AgentResponse`**: Relies on `simd_json::OwnedValue` to pass dynamically typed, unvalidated parameters between agents.
  ```rust
  pub struct AgentRequest {
      pub operation: String,
      pub args: Value,
      pub context: Option<String>,
      pub files: Vec<FileContext>,
  }
  ```
  *(See `crates/op-agents/src/unified/agent_trait.rs:50-55`)*