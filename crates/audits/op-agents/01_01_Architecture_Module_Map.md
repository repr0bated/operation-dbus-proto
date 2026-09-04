# Architecture & Module Map

## Overview
The `op-agents` crate is a native control-plane agent registry and D-Bus implementation. It manages domain-specific AI assistants (agents) that can perform localized operations, run diagnostics, execute code in sandboxed subprocesses, and expose capabilities to D-Bus and HTTP interfaces. The module structure separates legacy category-specific agents, modern unified execution and persona agents, security/sandboxing logic, and automatic code-generation capabilities.

## Module Tree
```text
crates/op-agents/
├── Cargo.toml
└── src/
    ├── lib.rs                  # Library root and agent factory entrypoint
    ├── agent_catalog.rs        # Listing of built-in legacy agents
    ├── agent_registry.rs       # Dynamic agent lifecycle & process factory
    ├── dbus_service.rs         # D-Bus service wrappers for AgentTrait
    ├── router.rs               # Axum HTTP routes for remote agent control
    ├── agents/                 # Legacy agents organized by category
    │   ├── mod.rs              # Category-specific imports
    │   ├── base.rs             # Base AgentTrait & path/arg validation
    │   ├── aiml/               # AI/ML expert persona agents
    │   ├── analysis/           # Debuggers, reviews, & audit agents
    │   ├── architecture/       # Code architecture expertise
    │   ├── business/           # CRM & payment integration agents
    │   ├── content/            # Doc builders & visualizer agents
    │   ├── database/           # SQLite/Postgres analysis & linting
    │   ├── infrastructure/     # Cloud, K8s, Terraform, network diagnostics
    │   ├── language/           # Multi-language compiler/interpreter environments
    │   ├── mobile/             # Flutter, iOS native developers
    │   ├── operations/         # DevOps troubleshooting & incident response
    │   ├── orchestration/      # Context managers, TDD, persistent memory
    │   ├── security/           # Application security development helpers
    │   ├── seo/                # Keyword strategy & copy writers
    │   ├── specialty/          # Niche domains (snowball, finance, ARMCortex)
    │   └── system/             # D-Bus based system-level controllers
    ├── bin/                    # Executables
    │   ├── dbus-agent.rs       # Individual agent process launcher
    │   └── dbus-agent-manager.rs # Main orchestrator systemd service
    ├── generator/              # Automatic code generation from Markdown definitions
    │   ├── mod.rs
    │   ├── md_parser.rs        # YAML Frontmatter and Markdown parser
    │   └── template.rs         # Safe Rust D-Bus agent code generation templates
    ├── security/               # Security enforcement engines
    │   ├── mod.rs
    │   ├── profiles.rs         # Preconfigured capability configurations
    │   ├── sandbox.rs          # Subprocess executor with timeouts & memory limits
    │   └── validation.rs       # Path canonicalization & char blacklist
    └── unified/                # Modern single-source-of-truth architectures
        ├── mod.rs
        ├── agent_trait.rs      # Decoupled UnifiedAgent interface
        ├── prompts.rs          # Compiled-in domain prompts
        ├── registry.rs         # Thread-safe unified registry
        ├── execution/          # Whitelisted execution engines (Python, Rust, etc.)
        ├── orchestration/      # Workflow step delegators
        └── persona/            # Expert personas without execution rights
```

## Entry Points
- **Library**: `crates/op-agents/src/lib.rs`
- **Executables**:
  - `crates/op-agents/src/bin/dbus-agent.rs`: Exposes any specified agent as an independent D-Bus endpoint.
  - `crates/op-agents/src/bin/dbus-agent-manager.rs`: Automates starting and monitoring vital system services on D-Bus.

---

# Production Security & Quality Audit

## Critical Severity

### 1. Arbitrary Code Execution via Git Argument Injection in Code Reviewer Agent
- **File & Line**: `crates/op-agents/src/agents/analysis/code_reviewer.rs:61-71`
- **Description**: The `git_diff` function takes user-supplied `args` and splits them by whitespace into arguments passed to `Command::new("git")`. Because arguments starting with hyphens are not blocked or validated, an attacker can pass option flags such as `--ext-diff` or `--output`. If the agent is exposed to the HTTP router or a D-Bus endpoint, an attacker can supply `args` like `--ext-diff=id` or `--output=/tmp/malicious_file`, forcing the host to execute the `id` command or write files outside the sandbox. This completely bypasses the configured `allowed_commands` restriction.
- **Remediation**: Avoid splitting raw strings into command arguments. Restrict the input format to safe revision identifiers or check that no parameters starting with `-` are passed. Alternatively, use a safe Rust Git implementation (e.g., `git2-rs`).

### 2. Sandbox Escape via Path Traversal in Path Validation Check
- **File & Line**: `crates/op-agents/src/agents/base.rs:405-425`
- **Description**: The legacy `validate_path` function attempts to limit directory access by checking if the supplied path starts with an allowed directory prefix: `allowed_dirs.iter().any(|dir| path.starts_with(dir))`. However, this is a naive string prefix match. If `allowed_dirs` includes `"/home"`, an attacker can pass `/home/../etc/passwd`. The string starts with `/home`, bypassing the check, but standard OS file APIs resolve `..` and traverse out of the sandbox. This enables arbitrary file read/write across all agents using `validate_path`.
- **Remediation**: Convert paths to `Path` objects, canonicalize them using `std::fs::canonicalize` to resolve all symlinks and parent directory components (`..`), and then check if the canonicalized path starts with the allowed root directory.

### 3. Privilege Escalation & Host File Access via Generated D-Bus System Agent Traversal
- **File & Line**: `crates/op-agents/src/generator/template.rs:415-440` (and `crates/op-agents/src/generator/template.rs:445-470`)
- **Description**: The agent code-generation template duplicates the flawed naive string-prefix path traversal vulnerability in its generated `validate_path` function. Crucially, the generated agent registers itself on the **System Bus** (`Builder::system()`). Since system D-Bus agents run with system-level privileges (often root), any unprivileged local user on the host system can invoke the exposed D-Bus methods with traversed paths (e.g., `/home/../etc/shadow`) to read or overwrite restricted system configuration files, leading directly to local privilege escalation.
- **Remediation**: Use canonicalized path validation in the generated template. Additionally, ensure generated agents drop privileges, run as a dedicated unprivileged user, and default to the session bus unless system-level integration is explicitly required.

---

## High Severity

### 4. Local File Disclosure via ripgrep (`rg`) Argument Injection
- **File & Line**: `crates/op-agents/src/agents/analysis/code_reviewer.rs:21-25`
- **Description**: The `search_code` function accepts a user-provided `pattern` and adds it as a direct argument to `rg`: `cmd.arg(p)`. Because ripgrep parses arguments starting with a hyphen as option flags, an attacker can pass `-f/etc/passwd` to force ripgrep to read files from outside the intended working directory, or inject command-execution options like `--preprocessor`.
- **Remediation**: Insert a `--` parameter before user-controlled search patterns in `Command` builders to indicate the end of command options.

### 5. Undefined Behavior & Memory Corruption via Unpadded `simd_json::from_str`
- **File & Line**: `crates/op-agents/src/agent_registry.rs:191-197` and `crates/op-agents/src/dbus_service.rs:122`
- **Description**: The codebase invokes `unsafe { simd_json::from_str(&mut content) }` on standard Rust `String` instances returned directly by file reads or D-Bus inputs. `simd-json` requires that input buffers have at least `simd_json::SIMDJSON_PADDING` extra bytes of padding at the end of the allocation. Passing a standard heap-allocated `String` without padding causes the SIMD engine to perform out-of-bounds reads/writes, leading to undefined behavior, memory corruption, or segmentation faults.
- **Remediation**: Avoid raw `unsafe` deserialization unless the buffers have been allocated with the necessary padding. Use the safe, standard `simd_json::to_padded_container` or use a safe parsing wrapper that handles alignment internally.

---

## Medium Severity

### 6. Thread Pool Starvation via Synchronous File I/O in Async Contexts
- **File & Line**: `crates/op-agents/src/agents/orchestration/memory.rs:120-125`
- **Description**: The `MemoryAgent::persist` method executes a synchronous file write: `fs::write(&self.memory_path, content)`. Since `MemoryAgent` runs inside an async context as part of the unified HTTP/D-Bus actor network, executing blocking synchronous file operations on the main thread pool can starve the tokio runtime, increasing request latencies and potentially timing out D-Bus heartbeats.
- **Remediation**: Use `tokio::fs::write` to execute the file operation asynchronously.

### 7. Non-functional Privilege Separation in Agent Specification Launcher
- **File & Line**: `crates/op-agents/src/agent_registry.rs:114-142`
- **Description**: The `AgentSpec` defines a `requires_root` boolean flag. However, the `ProcessAgentFactory::create_agent` launcher simply spawns the command directly without performing any privilege checks, drops, or elevations. If the `dbus-agent-manager` is running as root, *all* agents are executed as root, regardless of whether `requires_root` is set to `false`. This violates the Principle of Least Privilege.
- **Remediation**: Implement a privilege dropping mechanism (e.g., using `setuid`/`setgid` on Unix systems) when `requires_root` is false.

---

## Low Severity / Quality / Code Style

### 8. Hardcoded Memory and Timeout Limits
- **File & Line**: `crates/op-agents/src/security/profiles.rs:111-119`
- **Description**: Resource limits such as `timeout_secs`, `max_memory_mb`, and `max_output_size` are hardcoded inside profiles instead of being configurable via runtime settings, preventing dynamic host-level optimization.
- **Remediation**: Read default values from a global configuration file or environment variables.

---

# Schema-As-Code Compliance Report

## Violations Found
This codebase contains several violations of the schema-as-code discipline, where structured payload formats and service boundaries are defined using ad-hoc manually mirrored Rust structs rather than unified, versioned definitions.

### 1. Ad-Hoc Structs for Agent Specifications
- **File & Line**: `crates/op-agents/src/agent_registry.rs:21-65`
- **Structs**: `AgentSpec`, `RestartPolicy`, `HealthCheck`
- **Violation**: The registry defines configuration file mappings as ad-hoc Serde JSON structs. There is no versioning or schema schema validation (such as JSON Schema, Protocol Buffers, or OSCAL Component Definitions) associated with these definitions, complicating system-wide interoperability and backward compatibility.

### 2. Ad-Hoc Structs for Agent Tasking Contracts
- **File & Line**: `crates/op-agents/src/agents/base.rs:13-39` and `crates/op-agents/src/agents/base.rs:60-76`
- **Structs**: `AgentTask`, `TaskResult`
- **Violation**: Agent tasks and execution outputs are modeled via ad-hoc Rust structs. The `TaskResult` serialized payload contains a generic `HashMap<String, simd_json::OwnedValue>` metadata field. These free-form key-value structures bypass protocol validation and make static contract auditing impossible.

### 3. Unified Agent Message Formatting as Unversioned Payloads
- **File & Line**: `crates/op-agents/src/unified/agent_trait.rs:52-87`
- **Structs**: `AgentRequest`, `AgentResponse`
- **Violation**: The modern unified agent interface utilizes standard Serde JSON representation with unversioned structs. In decentralized system architectures where agents run as detached microservices on different versions, lack of protocol buffer versioning is highly prone to deserialization errors.

## Remediation Strategy
1. **Define Protocol Buffers**: Replace ad-hoc JSON messages with versioned Protobuf `.proto` schemas for all agent invocation messages (`AgentTask`/`AgentRequest`) and responses (`TaskResult`/`AgentResponse`).
2. **Implement JSON Schema / OSCAL**: Generate versioned JSON schemas for `AgentSpec` and host-level system configurations to enable validation at startup. Integrate OSCAL Component definitions to programmatically verify compliance profiles for sandbox boundaries.

---
## ⚠ Citation Warnings
- `crates/op-agents/src/agents/base.rs:405`: file has 255 lines
