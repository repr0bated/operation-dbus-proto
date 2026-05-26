# Architecture & Module Map

## Overview
The `op-agents` crate serves as the agent coordination and execution registry for the larger control plane ecosystem. It defines domain-specific AI assistants (categorized into languages, infrastructure, database, operations, and orchestration), wraps them in D-Bus interfaces to allow discoverability via system services, and implements both a legacy-style agent execution model and a newer, but currently uncompiled, "unified" agent model. 

## Module Tree
The physical module hierarchy of the crate (excluding dead code subdirectories not declared in `src/lib.rs`):
```text
op-agents (src/lib.rs)
├── agent_catalog
├── agent_registry
├── agents
│   ├── base (traits & validations)
│   ├── aiml
│   │   ├── ai_engineer
│   │   ├── data_engineer
│   │   ├── data_scientist
│   │   ├── ml_engineer
│   │   ├── mlops_engineer
│   │   └── prompt_engineer
│   ├── analysis
│   │   ├── code_reviewer
│   │   ├── debugger
│   │   ├── performance
│   │   └── security_auditor
│   ├── architecture
│   │   ├── backend_architect
│   │   ├── frontend_developer
│   │   └── graphql_architect
│   ├── business
│   │   ├── business_analyst
│   │   ├── customer_support
│   │   ├── hr_pro
│   │   ├── legal_advisor
│   │   ├── payment_integration
│   │   └── sales_automator
│   ├── content
│   │   ├── api_documenter
│   │   ├── docs_architect
│   │   ├── mermaid_expert
│   │   └── tutorial_engineer
│   ├── database
│   │   ├── database_architect
│   │   ├── database_optimizer
│   │   └── sql_pro
│   ├── infrastructure
│   │   ├── cloud
│   │   ├── deployment
│   │   ├── kubernetes
│   │   ├── network
│   │   └── terraform
│   ├── language
│   │   ├── bash_pro
│   │   ├── c_pro
│   │   ├── cpp_pro
│   │   ├── csharp_pro
│   │   ├── elixir_pro
│   │   ├── golang_pro
│   │   ├── java_pro
│   │   ├── javascript_pro
│   │   ├── julia_pro
│   │   ├── php_pro
│   │   ├── python_pro
│   │   ├── ruby_pro
│   │   ├── rust_pro
│   │   ├── scala_pro
│   │   └── typescript_pro
│   ├── mobile
│   │   ├── flutter_expert
│   │   ├── ios_developer
│   │   └── mobile_developer
│   ├── operations
│   │   ├── devops_troubleshooter
│   │   ├── incident_responder
│   │   └── test_automator
│   ├── orchestration
│   │   ├── context_manager
│   │   ├── dx_optimizer
│   │   ├── mem0_wrapper
│   │   ├── memory
│   │   ├── sequential_thinking
│   │   └── tdd_orchestrator
│   ├── security
│   │   ├── backend_security_coder
│   │   ├── frontend_security_coder
│   │   └── mobile_security_coder
│   ├── seo
│   │   ├── content_marketer
│   │   ├── search_specialist
│   │   ├── seo_content_writer
│   │   ├── seo_keyword_strategist
│   │   └── seo_meta_optimizer
│   ├── specialty
│   │   ├── arm_cortex_expert
│   │   ├── blockchain_developer
│   │   ├── error_detective
│   │   ├── hybrid_cloud_architect
│   │   ├── legacy_modernizer
│   │   ├── observability_engineer
│   │   ├── quant_analyst
│   │   ├── ui_ux_designer
│   │   └── unity_developer
│   ├── system (D-Bus system operations)
│   └── webframeworks
│       ├── django_pro
│       ├── fastapi_pro
│       └── temporal_python_pro
├── dbus_service
├── router
└── security
    ├── profiles
    ├── sandbox
    └── validation
```

## Entry Points
*   **Library Entry Point**: `crates/op-agents/src/lib.rs:1`
*   **Binary - D-Bus Agent Launcher**: `crates/op-agents/src/bin/dbus-agent.rs:1` (launches individual agents on system or session buses).
*   **Binary - D-Bus Agent Manager**: `crates/op-agents/src/bin/dbus-agent-manager.rs:1` (orchestrates starting and monitoring the core set of auto-started agents).

## Notes
*   The codebase includes two modules, `unified` (`crates/op-agents/src/unified/mod.rs`) and `generator` (`crates/op-agents/src/generator/mod.rs`), which are physically present on disk but never declared via `pub mod` inside `src/lib.rs`. Consequently, they are not compiled into the crate, representing significant dead-code bloat.

---

# Security & Quality Audit Findings

## Critical Severity

### 1. Host RCE via Python Pro Argument Order Manipulation
*   **Reference**: `crates/op-agents/src/agents/language/python_pro.rs:32`
*   **Impact**: Arbitrary Command Execution / Privilege Escalation.
*   **Description**: In `PythonProAgent::python_run`, the command-building logic pushes the user-controlled `args` string *before* the script `path` parameter:
    ```rust
    let mut cmd = Command::new("python3");
    if let Some(a) = args {
        validation::validate_args(a)?;
        for arg in a.split_whitespace() {
            cmd.arg(arg);
        }
    }
    if let Some(p) = path {
        let validated_path = validation::validate_path(p, ALLOWED_DIRS)?;
        cmd.arg(validated_path);
    }
    ```
    Because argument validation (`validate_args`) only bans character injections like `;`, `$`, and `&`, it does not restrict standard alphanumeric flags. A malicious caller can set `args` to `-m pip install <malicious-package>` or use python options (e.g. `-c "..."` with syntax bypasses). Because these flags precede the path, the resulting execution becomes:
    `python3 -m pip install <malicious-package> /tmp/script.py`
    During installation, the malicious package will execute arbitrary shell commands inside its `setup.py` on the host with the permissions of the parent agent process.

### 2. Path Traversal Bypass via Missing Validation in Legacy Validation
*   **Reference**: `crates/op-agents/src/agents/base.rs:175`
*   **Impact**: Arbitrary File Read and Write.
*   **Description**: The validation module implemented in the legacy base module (`crates/op-agents/src/agents/base.rs`) checks if a path is allowed by verifying only if the path string *starts with* an approved prefix:
    ```rust
    let is_allowed = allowed_dirs.iter().any(|dir| path.starts_with(dir));
    ```
    No path normalization, canonicalization, or dot-dot (`..`) traversal checking is done in this legacy validation module (unlike the uncompiled `security/validation.rs`).
    Since `Path::starts_with` evaluates component prefixes sequentially, a path like `/tmp/../etc/passwd` starts with `/tmp` component-wise, evaluating as `true`. When spawned under commands (such as `gcc`, `sqlite3`, or `python3`), the operating system resolves the traversal, allowing full read and write access to files outside of `/tmp`, `/home`, and `/opt`.

### 3. Argument Injection to Shell Command Execution in Cloud Architect Agent
*   **Reference**: `crates/op-agents/src/agents/infrastructure/cloud.rs:31`
*   **Impact**: Remote Code Execution (RCE).
*   **Description**: In the `CloudArchitectAgent`, `aws_describe` takes a user-supplied `args` string, validates it via `validate_args`, and splits it into whitespace tokens to append directly to the `aws` process invocation.
    Because there is no parameter filtering beyond basic character blacklisting, a user can supply command-line arguments that hijack the `aws` process execution flow. Specifically, the AWS CLI supports a `--cli-pager` argument that specifies the binary to shell out to for output rendering. By passing:
    `args = "--cli-pager \"touch /tmp/compromised\""`
    The AWS CLI will run the malicious string as a pager binary, resulting in unsandboxed arbitrary command execution.

### 4. Predictable Temporary File / Race Condition in Python Executor
*   **Reference**: `crates/op-agents/src/unified/execution/python.rs:32`
*   **Impact**: Local Privilege Escalation / Arbitrary File Write / Information Disclosure.
*   **Description**: The `PythonExecutor::run_python` method writes user-provided Python code directly to a static, predictable path in the shared temporary directory:
    ```rust
    let temp_file = "/tmp/python_exec.py";
    if let Err(e) = tokio::fs::write(temp_file, code).await { ... }
    ```
    This allows concurrent requests to overwrite each other's execution buffers, causing race conditions. More critically, any unprivileged local user can create `/tmp/python_exec.py` as a symlink pointing to a sensitive system file (e.g., `/etc/shadow` or systemd configuration files). When the agent process (potentially running as root) writes the Python code, it will follow the symlink and overwrite the target file's content.

### 5. Memory Safety Violations via Unpadded `simd_json` Parsing
*   **Reference**: `crates/op-agents/src/dbus_service.rs:114`, `crates/op-agents/src/agent_registry.rs:252`, `crates/op-agents/src/agents/orchestration/memory.rs:136`, `crates/op-agents/src/generator/template.rs:524`
*   **Impact**: Undefined Behavior / Segmentation Fault / Out-of-bounds Read.
*   **Description**: The codebase frequently calls the unsafe method `simd_json::from_str` on standard `String` and `&mut str` references that lack the padding required by `simd_json` internals:
    ```rust
    let mut task_json_mut = task_json.to_string();
    let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }...
    ```
    `simd_json` processes strings in 32-byte SIMD vector chunks and explicitly documents that inputs *must* have `simd_json::SIMDJSON_PADDING` extra bytes of allocated memory beyond the string length to prevent out-of-bounds reads. Calling `unsafe { simd_json::from_str }` on standard Rust `String` allocations passed from D-Bus violates this invariant, causing the parser to read past allocated bounds, leading to undefined behavior or immediate process crashes.

---

## High Severity

### 6. Arbitrary File Read/Write via Symbolic Link Traversal
*   **Reference**: `crates/op-agents/src/security/validation.rs:113`
*   **Impact**: Security Sandbox Bypass / Host File Access.
*   **Description**: While `validate_path` in `security/validation.rs` attempts to verify path structures and blocks parent traversal (`..`), it completely omits symbolic link resolution. 
    An attacker can create a symbolic link inside an allowed directory (such as `/tmp/evil_link` pointing to `/etc/shadow`) and supply the symlink path. Since `/tmp/evil_link` begins with `/tmp`, it passes all prefix checks. When the underlying process opens the path, the OS traverses the symlink, completely bypassing the path-restriction sandbox. To fix this, paths must be resolved via `fs::canonicalize` prior to evaluating prefix bounds.

### 7. Global Privilege Escalation via ProcessAgentFactory
*   **Reference**: `crates/op-agents/src/agent_registry.rs:141`
*   **Impact**: Unauthorized Privilege Escalation.
*   **Description**: In `ProcessAgentFactory::create_agent`, processes are spawned using `tokio::process::Command::new(&spec.command)`. 
    There is no code to drop privileges (e.g. `pre_exec` to call `setuid`/`setgid`) or constrain permissions when spawning agents that do *not* require root privileges (`requires_root: false`). If the main agent manager binary (`op-agent-manager`) runs as root (which is required to manage root services and network interfaces), then *all* launched agent binaries inherit and run with full root privileges.

---

## Medium Severity

### 8. Denial of Service via Uncontrolled File Reading in Docs Architect
*   **Reference**: `crates/op-agents/src/agents/content/docs_architect.rs:24`
*   **Impact**: Process Denial of Service (Infinite Memory Allocation).
*   **Description**: In `DocsArchitectAgent::read_file`, a user can trigger reading of arbitrary files. By exploiting the legacy path-traversal bypass, a caller can instruct the agent to read `/dev/zero` or `/dev/urandom`. 
    The call to `std::fs::read_to_string` will continuously read input until the system runs out of physical memory and swap, causing a kernel Out-Of-Memory (OOM) panic or crashing the agent process.

### 9. Race Condition in AgentRegistry Initialization
*   **Reference**: `crates/op-agents/src/agent_registry.rs:194`
*   **Impact**: Runtime Error on Startup.
*   **Description**: In `AgentRegistry::new`, the default process factory is registered by spawning an asynchronous Tokio task:
    ```rust
    let factories = registry.factories.clone();
    tokio::spawn(async move {
        let mut factories = factories.write().await;
        factories.push(default_factory);
    });
    ```
    If `spawn_agent` is called immediately after registry instantiation, the background initialization task may not have completed, resulting in an empty `factories` list and a failure to spawn any agents due to the `"No factory supports agent type"` error. The default factories must be initialized synchronously during creation.

---

## Low Severity / Code Quality

### 10. Uncompiled Dead Code Modules (`generator` and `unified`)
*   **Reference**: `crates/op-agents/src/lib.rs:1`
*   **Impact**: Dead Code Bloat.
*   **Description**: The directories `crates/op-agents/src/generator` and `crates/op-agents/src/unified` contain extensive implementations of newer, safer agent frameworks. However, these modules are never declared as active `mod` targets in `src/lib.rs`. Consequently, this code is not compiled, verified, or utilized, which hides dead code and leaves the system relying on vulnerable legacy agents.

### 11. Stiff-coded Fallback in Mem0 Agent Wrapper
*   **Reference**: `crates/op-agents/src/agents/orchestration/mem0_wrapper.rs:65`
*   **Impact**: Functional Defect / Stubbed Code.
*   **Description**: The `Mem0WrapperAgent` is completely dummy code that hardcodes a failure result for every single operation:
    ```rust
    Ok(TaskResult {
        success: false,
        operation: task.operation,
        data: json!({ ... "status": "disabled" }).to_string()
    })
    ```
    If any orchestration step expects Mem0-based memory storage or retrieval, it will always fail.