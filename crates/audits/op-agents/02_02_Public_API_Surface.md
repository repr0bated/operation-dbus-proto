### Public API Surface Count

A systematic search of the codebase using the regex `^\s*pub\s+(fn|struct|enum|trait|const|static|mod|type|use)` yields **198 matching public items**. This includes module declarations, re-exports, structs, traits, and associated public functions across the core registry, security, and unified agent layers.

---

### Key Exports

| # | Public Item | File:Line | Architectural Significance |
|---|---|---|---|
| 1 | `pub trait AgentTrait: Send + Sync` | `crates/op-agents/src/agents/base.rs:159` | The core traditional agent contract that governs metadata extraction, supported operations, and sandboxed task execution. |
| 2 | `pub trait UnifiedAgent: Send + Sync` | `crates/op-agents/src/unified/agent_trait.rs:116` | The modernized unified agent interface that merges system prompt personas directly into Rust code, eliminating external markdown definitions. |
| 3 | `pub struct AgentRegistry` | `crates/op-agents/src/agent_registry.rs:180` | Thread-safe, dynamic manager governing agent specifications, process spawns, and lifetime handles. |
| 4 | `pub struct SecurityProfile` | `crates/op-agents/src/security/profiles.rs:105` | Crucial security envelope defining path whitelists, binary command access, and operational risk classifications. |
| 5 | `pub struct SandboxExecutor` | `crates/op-agents/src/security/sandbox.rs:72` | Restricts shell execution parameters utilizing custom resource limits, timeouts, and process-level isolation. |
| 6 | `pub async fn start_agent(...)` | `crates/op-agents/src/dbus_service.rs:234` | Registers and exposes standard `AgentTrait` implementations to the system/session D-Bus via the `zbus` library. |
| 7 | `pub fn create_agent(...)` | `crates/op-agents/src/lib.rs:30` | The primary factory routing string identifiers to boxed, static, thread-safe agent implementations. |
| 8 | `pub struct AgentTask` | `crates/op-agents/src/agents/base.rs:12` | Serialization structure representing target actions, arguments, and paths crossing the D-Bus interface boundary. |
| 9 | `pub struct UnifiedAgentRegistry` | `crates/op-agents/src/unified/registry.rs:15` | Coordinates lazy-loading and runtime access to the new generation of `UnifiedAgent` implementations. |
| 10 | `pub fn create_router(...)` | `crates/op-agents/src/router.rs:54` | Exposes the agent management endpoints as a mountable Axum HTTP router. |

---

### Glob Re-exports

*   **File:Line**: `crates/op-agents/src/lib.rs:17`
*   **Statement**: `pub use agents::*;`
*   **Architectural Risk**: This glob re-export exposes all submodules of the `agents` directory (over 70 domain-specific agent structs) directly at the root of the `op-agents` crate. It pollutes the public namespace, leaks implementation details of specialized experts, and severely complicates public API stability. Any minor refactoring or removal of a specialized agent will break downstream consumer builds.

---

### Risks & Audit Findings

#### 1. JSON Injection Vulnerability via Manual String Formatting
*   **Citations**: `crates/op-agents/src/agents/orchestration/memory.rs:211`
*   **Vulnerability**: The `serialize_memory_entries` function manually formats raw strings into a JSON structure:
    ```rust
    let entry_json = format!(
        "\"{}\":{{\"value\":\"{}\",\"memory_type\":\"{}\",\"tags\":[{}],\"created_at\":...}}",
        key, entry.value, memory_type_str, tags_json, ...
    );
    ```
*   **Impact**: If `entry.value` or any tag contains unescaped double quotes (`"`) or control characters, the resulting serialized JSON becomes corrupted. A malicious payload can exploit this to inject arbitrary JSON keys or modify other stored parameters (JSON injection). When the agent restarts and loads the memory cache, it can trigger deserialization failures (denial of service) or privilege escalation within the memory state.
*   **Remediation**: Replace manual string interpolation with safe, robust serialization using `serde_json::to_string` or `simd_json::to_string`.

#### 2. Critical Security Gap: Total Bypass of Sandboxing in Traditional Agents
*   **Citations**: `crates/op-agents/src/agents/analysis/code_reviewer.rs:25`, `crates/op-agents/src/agents/analysis/debugger.rs:32`, `crates/op-agents/src/agents/infrastructure/network.rs:31`, etc.
*   **Vulnerability**: Although the crate implements a `SandboxExecutor` (in `security/sandbox.rs`) with strict resource limits, *none* of the traditional domain-specific agents use it. Instead, they directly import `std::process::Command` and execute binaries directly on the host system:
    ```rust
    let mut cmd = Command::new("rg"); // std::process::Command
    ```
*   **Impact**: The security boundaries defined by `SecurityProfile` (such as `max_memory_mb` and `timeout_secs`) are completely ignored. Highly sensitive agents (some of which run as `root`) execute unmitigated host-level processes, rendering the sandboxing framework completely useless.
*   **Remediation**: Refactor all traditional agents to run binaries exclusively through `SandboxExecutor` or the async-aware `tokio::process::Command` decorated with the target `SecurityProfile`.

#### 3. Command & Flag Injection in Shell Execution
*   **Citations**: `crates/op-agents/src/agents/analysis/code_reviewer.rs:64`, `crates/op-agents/src/agents/analysis/code_reviewer.rs:25`
*   **Vulnerability**: User-controlled argument strings are validated only for basic forbidden characters (via `validation::validate_args` checking for `;`, `&`, etc.) before being passed directly as distinct arguments to binaries:
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
*   **Impact**: This allows flag injection. An attacker can supply flags like `--ext-diff` or `-c core.pager=sh` to Git. This completely bypasses the forbidden character check and executes arbitrary host-level shell commands.
*   **Remediation**: Implement strict argument whitelisting instead of character-exclusion blacklists. Never allow arbitrary, unvalidated flags to be passed directly to VCS binaries.

#### 4. Unsafe Raw Pointer Manipulation without Safety Documentation
*   **Citations**: 
    *   `crates/op-agents/src/agent_registry.rs:239`
    *   `crates/op-agents/src/dbus_service.rs:136`
    *   `crates/op-agents/src/agents/orchestration/memory.rs:159`
    *   `crates/op-agents/src/security/validation.rs:184`
*   **Vulnerability**: The codebase frequently uses `unsafe` blocks to perform zero-copy deserialization using `simd_json::from_str` without any `# Safety` documentation:
    ```rust
    let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }
    ```
*   **Impact**: `simd-json`'s zero-copy parser destructively mutates the underlying buffer. If the compiler cannot guarantee that the string's lifetime exceeds that of the parsed structures, or if the string buffer is concurrently modified or accessed, this leads to undefined behavior (use-after-free or memory corruption).
*   **Remediation**: Add explicit `# Safety` comments explaining why the buffer is guaranteed to live long enough, or switch to `simd_json::from_str_owned` to perform safe, owned allocations.

#### 5. Blocking I/O in Async Contexts
*   **Citations**: `crates/op-agents/src/agents/orchestration/memory.rs:77`
*   **Vulnerability**: The `persist` method on `MemoryAgent` is executed inside an async context but performs blocking filesystem operations:
    ```rust
    fs::write(&self.memory_path, content).map_err(|e| e.to_string())?;
    ```
*   **Impact**: When called under high write frequency, this blocks the cooperative Tokio worker thread, causing latency spikes and potential thread starvation across the entire runtime.
*   **Remediation**: Use `tokio::fs::write` to execute file writing asynchronously without blocking the executor threads.

#### 6. Race Condition on Default Factory Registry Startup
*   **Citations**: `crates/op-agents/src/agent_registry.rs:207`
*   **Vulnerability**: The constructor `AgentRegistry::new()` spawns an un-awaited background task to register the default process agent factory:
    ```rust
    tokio::spawn(async move {
        let mut factories = factories.write().await;
        factories.push(default_factory);
    });
    ```
*   **Impact**: If a consumer immediately attempts to spawn an agent right after creating the registry, `spawn_agent` can fail because the default factory has not yet been registered due to thread scheduling delays.
*   **Remediation**: Expose an `async fn new()` constructor or perform registration synchronously during initialization.

#### 7. Deadlock Potential: Synchronous Mutexes Held Across Async Boundaries
*   **Citations**: `crates/op-agents/src/agents/orchestration/context_manager.rs:27`, `crates/op-agents/src/agents/orchestration/context_manager.rs:37`
*   **Vulnerability**: Async execution functions acquire blocking write locks on a synchronous `std::sync::RwLock`:
    ```rust
    let mut ctx = self.context.write().map_err(|_| "Failed to acquire lock")?;
    ```
*   **Impact**: Holding a synchronous lock across asynchronous suspension points (`.await`) can lead to severe deadlocks. If a thread is blocked waiting for the synchronous write lock to release, it cannot yield, starving the executor.
*   **Remediation**: Switch to `tokio::sync::RwLock` for safe, non-blocking locking across async yields.

#### 8. Extreme Clone Abuse in Catalogs and Registries
*   **Citations**: `crates/op-agents/src/agent_catalog.rs:59-130`, `crates/op-agents/src/agent_registry.rs:279`
*   **Vulnerability**: The catalog loops over 70+ agents, performing an expensive heap allocation clone of `agent_id` for every instantiation:
    ```rust
    Box::new(BashProAgent::new(agent_id.clone())),
    Box::new(CProAgent::new(agent_id.clone())),
    // ... cloned 70+ times
    ```
*   **Impact**: Unnecessary memory allocation and CPU overhead during bootstrap.
*   **Remediation**: Pass a reference `&str` or use an `Arc<str>` to share the agent identifier without cloning.

#### 9. Information Leakage in API Endpoints
*   **Citations**: `crates/op-agents/src/router.rs:115`
*   **Vulnerability**: Internal runtime errors are directly mapped to HTTP responses:
    ```rust
    Err(e) => Json(json!({ "error": e.to_string() })),
    ```
*   **Impact**: Leaks internal system configuration details, directory layouts, and execution errors directly to the client, providing valuable reconnaissance data for attackers.
*   **Remediation**: Log the detailed error internally and return a generic, sanitized error message to the client.