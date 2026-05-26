### D-Bus & IPC Attack Surface Audit

---

### 1. D-Bus Interface, Method, and Signal Inventory

The `op-agents` crate registers D-Bus interfaces to expose agent functionalities. These are defined statically in the wrapper service and dynamically via the code generator.

#### Static Interface: `org.dbusmcp.Agent`
*Registered in:* `crates/op-agents/src/dbus_service.rs:111-112`

| Interface | Name | Type | Caller Identity Checked? | Deserialization Validation? | Spawns Processes / Mutates State? |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `org.dbusmcp.Agent` | `execute` | Method | **No** | **No** (Directly parses via `unsafe` JSON parser) | **Yes** (Dispatches tasks that spawn shell/tool commands) |
| `org.dbusmcp.Agent` | `run_operation` | Method | **No** | **No** (Proxies parameters into `execute`) | **Yes** (Spawns commands) |
| `org.dbusmcp.Agent` | `agent_type` | Method/Prop | **No** | N/A | No |
| `org.dbusmcp.Agent` | `agent_id` | Method/Prop | **No** | N/A | No |
| `org.dbusmcp.Agent` | `name` | Method | **No** | N/A | No |
| `org.dbusmcp.Agent` | `description` | Method | **No** | N/A | No |
| `org.dbusmcp.Agent` | `operations` | Method | **No** | N/A | No |
| `org.dbusmcp.Agent` | `supports_operation` | Method | **No** | N/A | No |
| `org.dbusmcp.Agent` | `status` | Method | **No** | N/A | No |
| `org.dbusmcp.Agent` | `security_profile` | Method | **No** | N/A | No |
| `org.dbusmcp.Agent` | `metadata` | Method | **No** | N/A | No |
| `org.dbusmcp.Agent` | `ping` | Method | **No** | N/A | No |
| `org.dbusmcp.Agent` | `task_completed` | Signal | N/A | N/A | No |
| `org.dbusmcp.Agent` | `status_changed` | Signal | N/A | N/A | No |

---

#### Dynamically Generated Interfaces: `org.dbusmcp.Agent.{AgentType}`
*Generated in:* `crates/op-agents/src/generator/template.rs:368`

| Interface | Name | Type | Caller Identity Checked? | Deserialization Validation? | Spawns Processes / Mutates State? |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `org.dbusmcp.Agent.{AgentType}` | `execute` | Method | **No** | **No** (Uses `unsafe` parsing) | **Yes** (Spawns native compiler/interpreter commands) |
| `org.dbusmcp.Agent.{AgentType}` | `get_status` | Method | **No** | N/A | No |
| `org.dbusmcp.Agent.{AgentType}` | `list_operations` | Method | **No** | N/A | No |
| `org.dbusmcp.Agent.{AgentType}` | `task_completed` | Signal | N/A | N/A | No |

---

### 2. Bus Connection Configuration

The service is configured to connect to both the **System Bus** and **Session Bus**, depending on deployment and flags:

1. **Manager Default:** `crates/op-agents/src/bin/dbus-agent-manager.rs:201-209` defaults to the **System Bus** (`BusType::System`) unless the environment variable `DBUS_AGENT_SESSION` is explicitly defined.
2. **CLI Launcher Default:** `crates/op-agents/src/bin/dbus-agent.rs:173-177` registers on the **System Bus** if `--system` is provided, otherwise falls back to the **Session Bus**.
3. **Hardcoded Generator Bus:** `crates/op-agents/src/generator/template.rs:482` forces the auto-generated code to register on the **System Bus** (`Builder::system()`).

*Note on System Bus Policy:* No system bus policy (e.g., `/etc/dbus-1/system.d/` XML file) was provided in the source files. Because these services connect to the system bus by default and have no in-code authorization checks, any unprivileged local user can invoke their methods if the default D-Bus daemon configuration permits connection.

---

### 3. Safety & Quality Findings

#### [CRITICAL] Local Privilege Escalation via Unauthenticated Process Spawning
*File: Line* `crates/op-agents/src/dbus_service.rs:145-190`
*File: Line* `crates/op-agents/src/generator/template.rs:370-412`

**Description:**  
Both the static D-Bus service and the generated templates register an `execute` method that executes raw operating system commands or launches native scripts (e.g. `bash`, `python3`, `cargo`, `g++`).
No caller identity checks, credential passing (`zbus::Connection::peer_credentials`), or polkit authorizations are performed before dispatching execution commands. 

An unprivileged local user can call the `execute` method of an agent running as a system daemon (which potentially runs as `root` for operations such as `network`, `systemd`, or `packagekit` as declared in `crates/op-agents/src/agent_registry.rs:433-547`). 

**Exploit Scenario (LPE to root):**
1. An unprivileged local user writes a malicious shell script to `/tmp/evil.sh` (e.g., `chmod +s /bin/bash`).
2. The user calls `execute` on the `org.dbusmcp.Agent.BashPro` system bus service with the payload:
   ```json
   {
     "type": "bash-pro",
     "operation": "run",
     "path": "/tmp/evil.sh"
   }
   ```
3. `BashProAgent::execute` (invoked via `crates/op-agents/src/agents/language/bash_pro.rs:100`) validates `/tmp/evil.sh` against the `ALLOWED_DIRS` check (which permits `/tmp` at `bash_pro.rs:11`), and executes it via `std::process::Command` under the high-privilege context of the daemon.

---

#### [CRITICAL] Complete Sandbox Bypass in Agent Implementations
*File: Line* `crates/op-agents/src/agents/language/bash_pro.rs:23` (And all other language/analysis agents)

**Description:**  
The `crates/op-agents/src/security/sandbox.rs` module provides a comprehensive `SandboxExecutor` (lines 142-181) with timeouts, memory constraints, and environment isolation. However, **none** of the compiled language agents or system management agents utilize the `SandboxExecutor`. 

Instead, they invoke `std::process::Command::new(...)` directly on the host operating system context, allowing executed code to run natively without memory or process limits. This renders the sandbox configuration in `crates/op-agents/src/security/profiles.rs` entirely ineffective for built-in agents.

---

#### [HIGH] Unsafe Deserialization of Untrusted JSON
*File: Line* `crates/op-agents/src/dbus_service.rs:153`
*File: Line* `crates/op-agents/src/generator/template.rs:373`

**Description:**  
Raw, untrusted JSON payload bytes (`task_json` / `task_json_mut`) supplied directly by D-Bus clients are parsed using `unsafe { simd_json::from_str(...) }`. 

Because `simd-json` uses structural parsing requiring specific padding and alignment invariants, passing raw, unchecked mutable strings directly from D-Bus into `unsafe { simd_json::from_str }` without prior structure or length validation could trigger undefined behavior or memory safety issues under malformed payloads.

---

#### [HIGH] Parameter Argument Injection via whitespace splitting
*File: Line* `crates/op-agents/src/agents/language/bash_pro.rs:31-36`

**Description:**  
Arguments are split using `a.split_whitespace()`. While `validation::validate_args` checks for special shell characters like `;`, `&`, and `$`, splitting arguments on whitespace and appending them directly as individual arguments allows flag injection (e.g., passing `--preserve-root`, `-rf`, or arbitrary flags to native commands like `rm` or `git` if they are in the allowed command path).