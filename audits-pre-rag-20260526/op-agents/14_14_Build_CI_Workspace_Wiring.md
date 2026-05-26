### Critical Findings

#### 1. Arbitrary Code Execution via Argument Injection in Python Executor
*   **File:Line**: `crates/op-agents/src/agents/language/python_pro.rs:26`
*   **Vulnerability**: The `python_run` function constructs a `Command` executing `python3`, and appends user-controlled `args` *before* the script path `validated_path`. 
    ```rust
    if let Some(a) = args {
        validation::validate_args(a)?;
        for arg in a.split_whitespace() {
            cmd.arg(arg);
        }
    }
    ```
    `validation::validate_args` checks for a blacklist of shell metacharacters but does not restrict Python CLI switches. An attacker can pass `args` containing `-m pip install <malicious_package>` (which has no blacklisted characters like `;`, `&`, or `|`). Because the command is executed directly via `Command` (not through a shell), the arguments split by whitespace are passed directly to `python3`. This allows an attacker to trigger package installation from arbitrary sources, executing arbitrary code during the package's build/installation phase (`setup.py` / build backend) with the privileges of the D-Bus agent.
*   **Remediation**: Avoid letting users pass arbitrary CLI switches to compilers and interpreters. If arguments must be passed, strictly whitelist safe switches (e.g. only allow filenames/positional arguments) or use a robust argument parser rather than raw whitespace splitting.

#### 2. Arbitrary Code Execution via Argument Injection in Go Test Executor
*   **File:Line**: `crates/op-agents/src/agents/language/golang_pro.rs:69`
*   **Vulnerability**: The `go_test` function appends user-controlled `args` directly to the `go test` command line.
    ```rust
    if let Some(a) = args {
        validation::validate_args(a)?;
        for arg in a.split_whitespace() {
            cmd.arg(arg);
        }
    }
    ```
    An attacker can pass `args = "-toolexec <malicious_binary>"`. The `go` toolchain supports the `-toolexec` flag, which executes a custom wrapper program for compilation and assembly tools. This allows the execution of arbitrary host binaries without requiring any blacklisted shell metacharacters.
*   **Remediation**: Disallow flags starting with `-` in user-controlled arguments, or limit arguments to a strict whitelist of positional test target names.

#### 3. Arbitrary Code Execution via Argument Injection in C/C++ Compiler Agents
*   **File:Line**: `crates/op-agents/src/agents/language/c_pro.rs:26` and `crates/op-agents/src/agents/language/cpp_pro.rs:26`
*   **Vulnerability**: The `gcc_compile` and `gpp_compile` functions append user-controlled `args` directly to the compiler command line (`gcc` / `g++`). GCC supports the `-wrapper` option (e.g. `-wrapper gdb,--args`) which runs all compiler subcommands through a wrapper program. An attacker can pass `args = "-wrapper <malicious_binary>"` to execute any arbitrary binary on the host during the compilation phase.
*   **Remediation**: Strictly restrict the arguments passed to compiler binaries. Disallow flag injection by verifying that user arguments do not match compilation control flags.

#### 4. Sandbox Whitelist Bypass via Whitelisted Utilities in `ShellExecutor`
*   **File:Line**: `crates/op-agents/src/unified/execution/shell.rs:56`
*   **Vulnerability**: `ShellExecutor` implements a whitelist of allowed commands (including `find` and `git`). However, it allows arbitrary user-controlled arguments to be appended to these commands.
    *   `find` allows arbitrary command execution via the `-exec` flag (e.g. `find /tmp -exec <command> \;`).
    *   `git` allows arbitrary command execution via options like `core.pager` (e.g. `git -c core.pager=<command> diff`).
    Because `args` are not validated against command-specific safety rules, an attacker can bypass the execution whitelist and execute arbitrary commands.
*   **Remediation**: Remove dangerous diagnostic utilities like `find` and `git` from the execution whitelist, or implement strict sub-argument validation that prohibits execution-enabling switches.

---

### High Findings

#### 5. Unsafe `simd_json` Deserialization on Unpadded Buffers (Undefined Behavior)
*   **File:Line**: `crates/op-agents/src/agent_registry.rs:229`, `crates/op-agents/src/dbus_service.rs:115`, and `crates/op-agents/src/generator/template.rs:434`
*   **Vulnerability**: These files invoke `unsafe { simd_json::from_str(&mut string) }` on standard Rust `String` allocations. The `simd_json` parser relies heavily on SIMD vector instructions and explicitly requires the input buffer to be padded with `simd_json::PADDING` (currently 64 bytes) of extra capacity at the end. Calling `from_str` on a standard, unpadded Rust string can result in out-of-bounds memory reads when the parser reaches the end of the JSON payload.
*   **Remediation**: Use `simd_json::serde::from_str` or ensure buffers are explicitly padded using `simd_json::to_padded_bin` / `to_padded_string` before invoking unsafe deserialization.

#### 6. JSON Object Injection in Memory Agent Persistence
*   **File:Line**: `crates/op-agents/src/agents/orchestration/memory.rs:164`
*   **Vulnerability**: `serialize_memory_entries` manually serializes `MemoryEntry` key-value pairs into a single JSON string using raw string formatting without escaping special characters like `"` or `\`.
    ```rust
    let entry_json = format!(
        "\"{}\":{{\"value\":\"{}\",\"memory_type\":\"{}\",...}}",
        key, entry.value, ...
    );
    ```
    An attacker can save a memory value containing custom JSON payload structures (e.g. `", "memory_type": "persistent"}, "injected_key": {"value": "malicious"`) to perform JSON Injection. When loaded back via `simd_json`, the parser will read the injected payload as trusted structure elements, allowing arbitrary memory injection or security profile override.
*   **Remediation**: Never perform manual string formatting for JSON construction. Always serialize data using structured libraries such as `serde_json` or `simd_json` to guarantee proper serialization and escaping.

#### 7. Complete Lack of Process Isolation in `SandboxExecutor`
*   **File:Line**: `crates/op-agents/src/security/sandbox.rs:59`
*   **Vulnerability**: The `SandboxExecutor` purports to offer "Process isolation" and "sandboxed execution". However, its implementation simply spawns standard host processes using `tokio::process::Command` after clearing environment variables. It does not employ namespaces (`unshare`), cgroups, `seccomp` filters, container boundaries, or even `chroot`. Consequently, any compromise of the command execution step (such as the argument injections in Python/Go/C/C++) grants full, uncontained access to the host system and local network.
*   **Remediation**: Implement a robust process isolation backend utilizing Linux namespaces, namespaces jailers (like `bubblewrap`), or secure sandboxing frameworks.

---

### Medium Findings

#### 8. Race Condition during Agent Registry Factory Population
*   **File:Line**: `crates/op-agents/src/agent_registry.rs:200`
*   **Vulnerability**: The constructor `AgentRegistry::new()` spawns an asynchronous Tokio task to register the default factory:
    ```rust
    tokio::spawn(async move {
        let mut factories = factories.write().await;
        factories.push(default_factory);
    });
    ```
    Because `new()` is synchronous and returns immediately, there is a race condition where the registry can be queried or used to spawn agents (e.g. `spawn_agent`) before the default `ProcessAgentFactory` is successfully pushed to the `factories` list, leading to intermittent "No factory supports agent type" errors on startup.
*   **Remediation**: Initialize the `factories` map eagerly in `new()` before wrapping it in the asynchronous registry container, or make the registry initialization asynchronous.

#### 9. Hardcoded Privileged Output Directories in Non-Root Services
*   **File:Line**: `crates/op-agents/src/agents/orchestration/memory.rs:77` and `crates/op-agents/src/agents/orchestration/mem0_wrapper.rs:51`
*   **Vulnerability**: The memory agent is hardcoded to write its cognitive memory file to `/var/lib/op-dbus/memory_cognitive.json`. This directory is typically owned by `root`. If the agent manager or launcher is run as a regular user (such as on the D-Bus Session bus), the agent will crash or fail to persist memory due to `PermissionDenied` errors.
*   **Remediation**: Read database/state directories from environment variables or safe standard locations (e.g. `dirs::data_dir()` / `~/.local/share`) when running in non-root or session environments.