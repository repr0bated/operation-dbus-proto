# Production Security & Quality Audit: op-agents

---

## 1. Public API Surface & Dead Code

### Enumeration of `pub` Items
The `op-agents` crate exposes **206** public items. They are categorized below:

*   **Structs (57)**: `AgentDescriptor`, `AgentSpec`, `HealthCheck`, `AgentInstance`, `AgentHandle`, `ProcessAgentFactory`, `AgentRegistry`, `DbusAgentService`, `AgentsState`, `AgentsServiceRouter`, `AgentTask`, `TaskResult`, `AgentContext`, `AgentDefinition`, `ParsedCapabilities`, `DetectedOperation`, `AgentTemplate`, `AgentOperation`, `ResourceLimits`, `SandboxResult`, `SandboxExecutor`, `SandboxBuilder`, `UnifiedAgentRegistry`, `ExecutionAgent`, `GoExecutor`, `JavaScriptExecutor`, `PythonExecutor`, `RustExecutor`, `ShellExecutor`, `WorkflowStep`, `OrchestrationAgent`, `CodeReviewOrchestrator`, `TddOrchestrator`, `BackendArchitect`, `SecurityAuditor`, `CodeReviewer`, `PersonaAgent`, `DjangoExpert`, `FastAPIExpert`, `ReactExpert`, `KubernetesExpert`, `SystemdExpert`, `DbusExpert`, plus 14 legacy category modules and 45+ specialty agent structs.
*   **Enums (7)**: `RestartPolicy`, `AgentStatus`, `DbusAgentError`, `ProfileCategory`, `RiskLevel`, `AgentCategory`, `AgentCapability`, `MemoryType`.
*   **Traits (4)**: `AgentFactory`, `AgentTrait`, `UnifiedAgent`, `AgentMetadata`.
*   **Modules (31)**: `agent_catalog`, `agent_registry`, `agents`, `dbus_service`, `router`, `security`, `base`, `validation`, `aiml`, `analysis`, `architecture`, `business`, `content`, `database`, `infrastructure`, `language`, `mobile`, `operations`, `orchestration`, `security`, `seo`, `specialty`, `webframeworks`, `profiles`, `sandbox`, `md_parser`, `template`, `prompts`, `templates`, `languages`, `frameworks`.
*   **Functions (15)**: `builtin_agent_descriptors`, `load_default_specs`, `start_agent`, `start_agent_instance`, `generate_agent_id`, `is_agent_service`, `service_name_to_agent_type`, `create_agent`, `list_agent_types`, `create_router`, `validate_input`, `validate_path`, `validate_command`, `validate_args`, `validate_json_input`.
*   **Constants/Statics (8)**: `FORBIDDEN_CHARS`, `MAX_PATH_LENGTH`, `MAX_COMMAND_LENGTH`, `MAX_ARGS_LENGTH`, `MAX_INPUT_LENGTH`, `GLOBAL_REGISTRY`, `EXECUTION_AGENTS`, `PERSONA_AGENTS`, `ORCHESTRATION_AGENTS`.
*   **Re-exports (pub use) (11)**: Flat re-exports in `lib.rs` and `unified/mod.rs`.

---

### Top 10 Most Impactful Public APIs
The following table highlights the most critical public-facing APIs:

| Rank | Item | Type | file:line | Impact Description |
| :--- | :--- | :--- | :--- | :--- |
| **1** | `create_agent` | `fn` | `lib.rs:28` | Factory constructing legacy specialty agents. Bypasses next-gen traits. |
| **2** | `AgentRegistry` | `struct` | `agent_registry.rs:166` | Central host management of processes, lifecycle, and specs. |
| **3** | `start_agent` | `fn` | `dbus_service.rs:228` | Binds any specialty agent to the D-Bus system/session bus. |
| **4** | `AgentTrait` | `trait` | `agents/base.rs:139` | Standard interface defining legacy specialty agent behaviors. |
| **5** | `UnifiedAgent` | `trait` | `unified/agent_trait.rs:136` | Next-gen unified trait intended to merge static prompts into Rust types. |
| **6** | `SandboxExecutor` | `struct` | `security/sandbox.rs:78` | Executes host commands inside isolated boundaries with resource limits. |
| **7** | `SecurityProfile` | `struct` | `security/profiles.rs:125` | Defines commands, paths, and timeouts whitelisted for sandbox execution. |
| **8** | `AgentSpec` | `struct` | `agent_registry.rs:15` | Parameterization for spawning untrusted host binaries. |
| **9** | `create_router` | `fn` | `router.rs:53` | Generates axum REST endpoints for monitoring and lifecycle control. |
| **10** | `UnifiedAgentRegistry` | `struct` | `unified/registry.rs:13` | Lazily instantiates refactored Execution, Persona, and Orchestration agents. |

---

### Glob Re-exports
*   **`crates/op-agents/src/lib.rs:17`**: `pub use agents::*;`
    *   *Security Concern*: Pollution of the crate root namespace with all legacy specialty agent structs (over 50 structs). Changes to internal agent types can break downstream consumers implicitly.

---

### Public Fields on Structs that Should Be Private
Exposing fields on configuration or process handlers breaks structural invariants. The following fields must be encapsulated:

1.  **`crates/op-agents/src/agent_registry.rs:125`**: `pub process: tokio::process::Child` in `AgentHandle`.
    *   *Symptom*: Callers can wait, kill, or steal the handle of a spawned process without updating `AgentInstance` status.
2.  **`crates/op-agents/src/unified/execution/base.rs:22`**: `pub security_profile: SecurityProfile` in `ExecutionAgent`.
    *   *Symptom*: Highly dangerous mutability. Callers can replace the security profile of sandboxed agents at runtime, bypassing restrictions.
3.  **`crates/op-agents/src/agents/base.rs:119`**: `pub profile: SecurityProfile` in `AgentContext`.
    *   *Symptom*: Let callers alter allowed read/write paths during host execution.
4.  **`crates/op-agents/src/agents/base.rs:120`**: `pub executor: SandboxExecutor` in `AgentContext`.
    *   *Symptom*: Exposes execution machinery directly.
5.  **`crates/op-agents/src/agent_registry.rs:90`**: `pub status: AgentStatus` in `AgentInstance`.
    *   *Symptom*: State variables can be mutated directly, lying about the actual PID status.

---

## 2. Dead Code Audit

### Compiler Warnings Suppression (`#[allow(dead_code)]`)
*   **`crates/op-agents/src/agents/mod.rs:1`**: `#![allow(dead_code)]`
    *   *Risk*: This crate-level attribute suppresses unused warnings for *all* code written under the `agents/` tree. Consequently, over 50 legacy specialty agents are silently compiled but may never be used.

---

### Unused Imports (Prefixed with `_` or Compiler Hints)
*   **`crates/op-agents/src/unified/execution/base.rs:6`**: `use simd_json::{json, OwnedValue as Value};`
    *   Neither `json` nor `Value` is used anywhere in the file.
*   **`crates/op-agents/src/unified/persona/base.rs:6`**: `use simd_json::{json, OwnedValue as Value};`
    *   `Value` is imported but never referenced (only the `json!` macro is used).

---

### Empty or TODO Module Declarations
*   **`crates/op-agents/src/agents/system/mod.rs`**: This file contains declarations for:
    ```rust
    pub mod executor;
    pub mod file;
    pub mod monitor;
    ...
    ```
    However, the corresponding files under `crates/op-agents/src/agents/system/` are completely absent or not declared within `crates/op-agents/src/agents/mod.rs`.
*   There is no `pub mod system;` inside `crates/op-agents/src/agents/mod.rs`. This makes the entire `system` module orphaned and dead code.

---

### Dead Code Table

| Item | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `unified` | `directory / mod` | `src/unified/mod.rs` | **Remove / Integrate**: The entire next-gen refactored unified module is never declared in `src/lib.rs`. It is completely dead code. |
| `generator` | `directory / mod` | `src/generator/mod.rs` | **Remove / Integrate**: The markdown definition parser and template generator are never declared in `src/lib.rs`. |
| `system` | `mod` | `src/agents/system/mod.rs` | **Remove**: Unreachable from `src/agents/mod.rs`. All system agents (`Executor`, `File`, `Monitor`, `Network`, `PackageKit`, `Systemd`) are dead. |
| `Mem0WrapperAgent` | `struct` | `src/agents/orchestration/mem0_wrapper.rs:37` | **Remove**: "Temporarily Disabled" stub. Not instantiated in any catalog or factory. |
| `send_task_handler` | `fn` | `src/router.rs:130` | **Remove**: Commented-out HTTP endpoint handler. |

---

## 3. Directly Exploitable Vulnerabilities

### [CRITICAL] Path Traversal Validation Bypass via Dot-Dot Segments
*   **Vulnerability Type**: Path Traversal (CWE-22)
*   **File:Line**: `crates/op-agents/src/agents/base.rs:173-194` (and all calling legacy agents, e.g., `debugger.rs:33`)

#### Description
Legacy specialty agents (such as the `DebuggerAgent` or `CodeReviewerAgent`) validate paths using `validation::validate_path` from `crates/op-agents/src/agents/base.rs`:

```rust
pub fn validate_path(path: &str, allowed_dirs: &[&str]) -> Result<String, String> {
    if path.len() > MAX_PATH_LENGTH { return Err(...); }
    for c in FORBIDDEN_CHARS {
        if path.contains(*c) { return Err(...); }
    }
    let is_allowed = allowed_dirs.iter().any(|dir| path.starts_with(dir));
    if !is_allowed { return Err(...); }
    Ok(path.to_string())
}
```

This validation is fundamentally broken:
1.  `FORBIDDEN_CHARS` does **not** contain the dot (`.`) character.
2.  `path.starts_with(dir)` only checks the prefix.
3.  The string is never canonicalized or resolved prior to prefix validation.

As a result, a path string like `"/tmp/../etc/passwd"` successfully passes validation because it starts with `"/tmp"`. When passed directly to `std::process::Command` execution vectors, the OS resolves the relative path traversal, reading outside the sandbox boundaries.

#### Proof of Concept (PoC)
An attacker issues a D-Bus task call to the `Debugger` agent requesting `read_logs` with:
```json
{
  "type": "debugger",
  "operation": "logs",
  "path": "/tmp/../etc/passwd",
  "args": "10"
}
```
*   `validated_path` resolves to `"/tmp/../etc/passwd"`.
*   The agent runs: `tail -n 10 /tmp/../etc/passwd`.
*   The system files are read and returned directly in the `TaskResult` string.

---

### [CRITICAL] JSON Injection via Manual Serialization Formatting
*   **Vulnerability Type**: Injection / Broken Serialization (CWE-74)
*   **File:Line**: `crates/op-agents/src/agents/orchestration/memory.rs:218-243`

#### Description
The `MemoryAgent` persists cognitive state to `/var/lib/op-dbus/memory_cognitive.json`. However, instead of using a standard JSON library to serialize the `HashMap<String, MemoryEntry>`, it manual-constructs a JSON string:

```rust
let entry_json = format!(
    "\"{}\":{{\"value\":\"{}\",\"memory_type\":\"{}\",\"tags\":[{}],\"created_at\":{},\"updated_at\":{},\"access_count\":{},\"last_accessed\":{}{}}}",
    key, entry.value, memory_type_str, tags_json, entry.created_at, entry.updated_at, 
    entry.access_count, entry.last_accessed, expires_json
);
```

Since the `key` and `entry.value` strings are inserted into formatting variables without escaping quotes or backslashes, a malicious user or LLM can perform **JSON Injection** by saving a memory key/value containing structured JSON elements.

#### Proof of Concept (PoC)
1.  An attacker invokes the memory agent's `remember` operation with:
    *   `key`: `"attacker_key"`
    *   `value`: `_payload_` where:
        ```text
        "}, "injected_key": { "value": "hacked", "memory_type": "persistent", "tags": [], "created_at": 1700000000 }, "dummy": { "value": "
        ```
2.  The resulting string is formatted on disk into:
    ```json
    {"attacker_key":{"value":""}, "injected_key": { "value": "hacked", "memory_type": "persistent" }, "dummy": { "value": ""}}
    ```
3.  On next boot, `parse_memory_entries` parses the file. The parser registers `"injected_key"` as a real, persistent memory state entry.

---

### [HIGH] Bypassing Sandbox Executor to Spawn Unsandboxed Host Commands
*   **Vulnerability Type**: Privilege Escalation / Sandbox Escape (CWE-250)
*   **File:Line**: All files in `crates/op-agents/src/agents/` (e.g. `c_pro.rs:33`, `rust_pro.rs:35`)

#### Description
`op-agents` defines a safe, resource-controlled, environment-scrubbed sandbox execution framework in `src/security/sandbox.rs` (`SandboxExecutor`). 

However, **no specialty agent in production uses it**. Every specialty agent spawns command processes directly via the raw standard library:

```rust
// In rust_pro.rs:35
let mut cmd = Command::new("cargo");
// Bypasses the sandbox completely, inherits the host process context and environment
let output = cmd.output(); 
```

Because of this:
1.  **Full Environment Inheritance**: Host environment variables (potentially containing secrets, DB credentials, D-Bus session variables) are fully leaked to spawned compiler tools.
2.  **No Resource Constraints**: `SandboxExecutor` limits memory and sets timeouts. The direct standard library `Command` has no memory limits and no timeouts, allowing a compiler bomb (DoS) or hang-forever thread to freeze the system.

---

### [HIGH] Unauthenticated Remote process-spawning HTTP Interface
*   **Vulnerability Type**: Missing Authentication for Critical Function (CWE-306)
*   **File:Line**: `crates/op-agents/src/router.rs:94-110`

#### Description
The axum endpoint `spawn_agent_handler` (`POST /api/agents`) triggers the `AgentRegistry`'s `spawn_agent` method.
Spawning an agent spawns host binaries (e.g., `dbus-agent-executor` or `dbus-agent-network`) which can run as root (`requires_root: true` in the spec).

Since the router contains no authentication, token verification, or local socket restriction, any network-adjacent or local unauthenticated HTTP user can spawn high-privilege agent processes at will, leading to trivial host resource exhaustion and potential privilege escalation.