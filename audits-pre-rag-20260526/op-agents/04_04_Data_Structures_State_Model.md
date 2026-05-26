### Data Structures Audit

#### Data Structure Counts per File

| File | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell`/`Lazy` | `.clone()` Count |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `crates/op-agents/src/agent_catalog.rs` | 0 | 0 | 0 | 0 | 0 | 0 | **70** *(Flagged: >20)* |
| `crates/op-agents/src/agent_registry.rs` | 8 | 0 | 0 | 8 | 0 | 0 | 7 |
| `crates/op-agents/src/dbus_service.rs` | 2 | 0 | 0 | 2 | 0 | 0 | 1 |
| `crates/op-agents/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-agents/src/router.rs` | 3 | 0 | 0 | 3 | 0 | 0 | 1 |
| `crates/op-agents/src/agents/base.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-agents/src/agents/orchestration/context_manager.rs` | 2 | 0 | 0 | 2 | 0 | 0 | 0 |
| `crates/op-agents/src/agents/orchestration/mem0_wrapper.rs` | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| `crates/op-agents/src/agents/orchestration/memory.rs` | 2 | 0 | 0 | 2 | 0 | 0 | 12 |
| `crates/op-agents/src/unified/registry.rs` | 2 | 0 | 0 | 1 | 0 | 1 | 2 |

*Note: Other files not listed contain 0 instances of the target data structures.*

---

#### Large Structs Flagged (> 5 Public Fields)

*   **`AgentSpec`** (`crates/op-agents/src/agent_registry.rs:19`)
    *   **12 public fields**: `agent_type`, `name`, `description`, `command`, `args`, `env`, `working_dir`, `capabilities`, `requires_root`, `max_instances`, `restart_policy`, `health_check`.
*   **`AgentInstance`** (`crates/op-agents/src/agent_registry.rs:114`)
    *   **7 public fields**: `id`, `agent_type`, `pid`, `status`, `started_at`, `last_health_check`, `restart_count`.
*   **`MemoryEntry`** (`crates/op-agents/src/agents/orchestration/memory.rs:17`)
    *   **10 public fields**: `key`, `value`, `vector`, `memory_type`, `tags`, `created_at`, `updated_at`, `expires_at`, `access_count`, `last_accessed`.
*   **`SecurityConfig`** (`crates/op-agents/src/security/profiles.rs:31`)
    *   **13 public fields**: `category`, `allowed_commands`, `allowed_read_paths`, `allowed_write_paths`, `forbidden_paths`, `allowed_tools`, `allowed_subagents`, `timeout_secs`, `max_memory_mb`, `max_output_size`, `max_concurrent`, `requires_approval`, `requires_root`.
*   **`ExecutionAgent`** (`crates/op-agents/src/unified/execution/base.rs:16`)
    *   **8 public fields**: `id`, `name`, `description`, `language`, `system_prompt`, `knowledge`, `security_profile`, `operations`.
*   **`OrchestrationAgent`** (`crates/op-agents/src/unified/orchestration/base.rs:21`)
    *   **6 public fields**: `id`, `name`, `description`, `system_prompt`, `allowed_agents`, `workflow_steps`.
*   **`PersonaAgent`** (`crates/op-agents/src/unified/persona/base.rs:13`)
    *   **8 public fields**: `id`, `name`, `description`, `domain`, `system_prompt`, `knowledge`, `capabilities`, `examples`.

---

#### Globally Mutable / Shared Static State

*   **`GLOBAL_REGISTRY`** (`crates/op-agents/src/unified/registry.rs:113`)
    *   `once_cell::sync::Lazy` static instance of `UnifiedAgentRegistry` wrapping a `RwLock` protected map of instantiated agents.
*   **`EXECUTION_AGENTS`** (`crates/op-agents/src/unified/execution/mod.rs:22`)
    *   `once_cell::sync::Lazy` static factory map.
*   **`ORCHESTRATION_AGENTS`** (`crates/op-agents/src/unified/orchestration/mod.rs:19`)
    *   `once_cell::sync::Lazy` static factory map.
*   **`PERSONA_AGENTS`** (`crates/op-agents/src/unified/persona/mod.rs:18`)
    *   `once_cell::sync::Lazy` static factory map.

---

### Production Security & Quality Audit

#### [CRITICAL] Remote Code Execution via Cargo Compiler Build Script Hijacking
*   **Location**: `crates/op-agents/src/agents/language/rust_pro.rs:105` (also affects Go Compiler Flags at `crates/op-agents/src/agents/language/golang_pro.rs:27`)
*   **Impact**: Direct arbitrary code execution on the host machine as the agent process user (which is frequently root).
*   **Description**: 
    The `rust-pro` agent allows a caller to invoke the `check`, `build`, or `test` operations on an arbitrary directory via `Command::new("cargo")`. Cargo automatically resolves, compiles, and executes `build.rs` build scripts found within the target project directory during compilation/checking.
    An attacker can write an arbitrary malicious Rust payload inside a `build.rs` script in `/tmp/malicious/build.rs`, and request the `rust-pro` agent to run `check` or `build` on `/tmp/malicious`. Cargo will execute the compiled `build.rs` binary outside of any container isolation on the host system.
    Similarly, the `golang-pro` agent splits arguments via `a.split_whitespace()` and appends them directly to `go build` (e.g., `crates/op-agents/src/agents/language/golang_pro.rs:27`). If an attacker passes the `-toolexec` compiler flag via `args`, the Go compiler will execute an arbitrary host binary specified by the attacker during build execution.

---

#### [CRITICAL] Privilege Escalation to Root via Undropped Spawning Privileges
*   **Location**: `crates/op-agents/src/agent_registry.rs:170`
*   **Impact**: Host-level root takeover.
*   **Description**:
    The systemd controller (`dbus-agent-systemd`), network manager (`dbus-agent-network`), and package manager (`dbus-agent-packagekit`) agents require root privileges (declared via `requires_root: true` in `AgentSpec` on lines 472, 497, 542). Because of this design, the main launcher `dbus-agent-manager` must run as `root` to successfully bind and spawn these services.
    However, the `ProcessAgentFactory::create_agent` implementation at `agent_registry.rs:170` spawns *all* configured agents (including compilers like `golang-pro`, `rust-pro`, `python-pro` etc.) using `tokio::process::Command` without dropping privileges (no `uid`/`gid` switching, no cgroups, no namespaces). 
    As a result, compilation/execution agents inherit root privileges, allowing compiler-based payloads (such as `build.rs` scripts) to execute directly as root on the host machine.

---

#### [HIGH] Symlink Directory Traversal/LFI via Uncanonicalized Path Validation
*   **Location**: `crates/op-agents/src/security/validation.rs:113`
*   **Impact**: Arbitrary file read/write outside of the allowed directory boundaries.
*   **Description**:
    The `validate_path` function validates that user-provided paths are restricted to allowed directories (e.g., `/home`, `/tmp`, `/opt`). However, the validation is performed on a raw `PathBuf` from the user-input string *before* resolving symbolic links:
    ```rust
    let path_buf = PathBuf::from(path);
    // ... Checks starts_with on path_buf
    ```
    If an attacker creates a symbolic link at `/tmp/exploit` pointing to `/etc`, the string `/tmp/exploit` passes the check because it literally starts with `/tmp` (which is in `allowed_dirs`) and does not contain `..`.
    When the agent subsequently calls `std::fs::read_to_string` or executes a command using this path, the OS resolves the symbolic link, allowing the attacker to read `/etc/passwd` or write files to forbidden system paths.
*   **Remediation**: Use `fs::canonicalize` on the target path to resolve all symbolic links and `..` segments *before* checking against the allowed prefix list.

---

#### [HIGH] Total Bypass of Sandbox Resource & Execution Constraints
*   **Location**: `crates/op-agents/src/agents/analysis/debugger.rs:24` (and all other domain-specific agent implementations)
*   **Impact**: Denial of Service (DoS), infinite hangs, and memory exhaustion.
*   **Description**:
    The codebase defines a secure `SandboxExecutor` (`crates/op-agents/src/security/sandbox.rs`) designed to limit memory, restrict execution timeouts, truncate excessive output, and enforce strict whitelists. However, **none** of the domain-specific agents actually use `SandboxExecutor` to execute commands.
    Instead, every agent manually constructs and executes commands using `std::process::Command` directly (e.g., `Command::new("tail")` at `debugger.rs:24`). This bypasses all sandbox constraints:
    1.  **Infinite Hangs**: Running a command that blocks indefinitely (e.g., `tail -f /dev/stdin`) will hang the agent thread forever, locking the `RwLock` and causing a permanent Denial of Service for that agent.
    2.  **No Memory/Output Limits**: A process can run out of memory or dump gigabytes of text into stdout, crashing the agent when it attempts to buffer the entire output via `String::from_utf8_lossy`.

---

#### [HIGH] Memory Safety Violation in `simd_json` Parsing
*   **Location**: `crates/op-agents/src/dbus_service.rs:99` (and `crates/op-agents/src/agents/orchestration/memory.rs:108`)
*   **Impact**: Undefined behavior, memory corruption, or memory disclosure under malformed JSON payloads.
*   **Description**:
    The codebase uses `unsafe { simd_json::from_str(&mut string) }` directly on unpadded Rust `String` buffers:
    ```rust
    let mut task_json_mut = task_json.to_string();
    let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }
    ```
    The `simd-json` parser explicitly requires the input string slice to have at least `simd_json::PADDING` (usually 32 bytes) of allocated padding at the end. Passing a standard `String` directly to `simd_json::from_str` without padding is undefined behavior, as the parser may execute SIMD vector reads past the allocated buffer bounds.
*   **Remediation**: Either use safe `simd_json::from_slice` on a `Vec<u8>` padded using `to_address_padded()` / `to_padded_bytes()`, or use `serde_json` for unpadded buffers.

---

#### [MEDIUM] PATH Environment Variable Hijacking
*   **Location**: `crates/op-agents/src/agents/analysis/code_reviewer.rs:32` (and other domain-specific agents)
*   **Impact**: Execution of malicious binaries via path manipulation.
*   **Description**:
    Because the agents execute raw commands using `std::process::Command` rather than through the `SandboxExecutor` (which is bypassed), they do not clear or sanitize the environment variables. If the environment of the parent process is compromised or contains an untrusted directory in `PATH` (such as `/tmp` or a writable directory), the agent will execute the hijacked binary instead of the system utility (e.g., executing `/tmp/rg` instead of `/usr/bin/rg`).