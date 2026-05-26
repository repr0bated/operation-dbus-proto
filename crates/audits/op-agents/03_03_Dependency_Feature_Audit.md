# Production Security & Quality Audit: op-agents

---

## 1. Dependencies & Feature Inventory

### Direct Dependencies (from `crates/op-agents/Cargo.toml`)

| Dependency | Version Specifier | Features Enabled (Explicit vs Default) | Security/Quality Flags |
|---|---|---|---|
| `op-core` | `workspace = true` | Inherited from workspace (Default) | Internal Crate |
| `op-http` | `workspace = true` | Inherited from workspace (Default) | Internal Crate |
| `tokio` | `workspace = true` | Workspace standard: `full` | **Anyhow/Tokio Flag**: Full async runtime enabled |
| `async-trait` | `workspace = true` | Default | Safe helper macro |
| `futures` | `workspace = true` | Default | Workspace standard |
| `serde` | `workspace = true` | Workspace standard: `derive` | Safe serialization |
| `simd-json` | `workspace = true` | Workspace standard: `serde`, `serde_impl` | High performance JSON parser |
| `serde_yaml` | `workspace = true` | Default | **Crate Deprecated**: Maintainers archived `serde_yaml`. Risks unpatched flaws. |
| `toml` | `workspace = true` | Default | Configuration parser |
| `anyhow` | `workspace = true` | Default | **Anyhow/Tokio Flag**: Error boxing library |
| `thiserror` | `workspace = true` | Default | **Anyhow/Tokio Flag**: Error macro crate |
| `zbus` | `workspace = true` | Workspace standard: `tokio` (v4.0) | IPC framework |
| `uuid` | `workspace = true` | Workspace standard: `v4`, `serde` | UUID generation |
| `chrono` | `workspace = true` | Workspace standard: `serde` | Time manipulation |
| `tracing` | `workspace = true` | Default | Instrumentation framework |
| `tracing-subscriber` | `workspace = true` | Workspace standard: `env-filter`, `json` | Logging output |
| `regex` | `workspace = true` | Default | Pattern matching |
| `shell-words` | `1.1` | Default | **Unpinned Version (`1.1`)**: Allows arbitrary minor updates |
| `axum` | `workspace = true` | Workspace standard: `ws`, `macros`, `tokio` | HTTP framework |

---

## 2. Storage Backend Check

The codebase declares storage interactions both in library modules (for agent state/memory) and inside domain-specific agents that wrap database utilities.

### Storage Backend Inventory

| Backend / Store | Found at File:Line | Role | Architectural Violation? |
|---|---|---|---|
| Ad-hoc JSON File (`memory_cognitive.json`) | `crates/op-agents/src/agents/orchestration/memory.rs:98` | Cognitive Key-Value State / Memory | **Yes**. Writes raw JSON text file instead of using `cozo` / `sled` which are available in workspace dependencies. |
| Ad-hoc JSON File (`memory.json`) | `crates/op-agents/src/agents/orchestration/memory.rs:104` | Legacy State Migration | **Yes**. Direct flat-file database emulation without consistency guarantees. |
| `sqlite3` Subprocess | `crates/op-agents/src/agents/database/database_architect.rs:32` | Schema Inspection | No (Analysis tool wrapper) |
| `sqlite3` Subprocess | `crates/op-agents/src/agents/database/database_optimizer.rs:33` | Query Plan Analyzer | No (Analysis tool wrapper) |
| `sqlite3` Subprocess | `crates/op-agents/src/agents/database/sql_pro.rs:31` | Query Execution | No (Execution sandbox wrapper) |

---

## 3. Schema-as-Code Compliance Gap

The codebase exhibits a structural architectural deviation from the "schema-as-code" discipline. Instead of declaring inter-agent data contracts as versioned, strictly typed schemas (such as Protocol Buffers or JSON Schemas compiled from build scripts), data payloads are expressed as ad-hoc, loosely typed Rust structs or raw unvalidated string blobs.

### Schema-as-Code Violations

*   **Ad-hoc Agent Task Definition**:
    `crates/op-agents/src/agents/base.rs:13-37` declares `AgentTask` as an ad-hoc struct with a raw `HashMap<String, simd_json::OwnedValue>` configuration bag. This lacks a formal schema contract.
*   **Ad-hoc Agent Specification Configuration**:
    `crates/op-agents/src/agent_registry.rs:17-57` defines `AgentSpec` using unstructured `HashMap<String, String>` and unversioned struct fields for external commands and health check mechanisms, rather than a codified OSCAL profile.
*   **Ad-hoc String-Serialized D-Bus Payloads**:
    `crates/op-agents/src/dbus_service.rs:141-150` performs unchecked unsafe deserialization of raw `String` D-Bus method parameters into `AgentTask` instances. This entirely bypasses serialization-layer type safety.

---

## 4. Critical Directly Exploitable Vulnerabilities

### Critical Finding 1: Arbitrary File Read and Sandbox Escape via Directory Traversal in Legacy Agent Path Validation
*   **File:Line Citation**: `crates/op-agents/src/agents/base.rs:341-360`
*   **Vulnerability Type**: Path Traversal (CWE-22)
*   **Exploitability**: Directly Exploitable

#### Technical Analysis
The legacy validation module defines path checks for all core language, documentation, and database agents as follows:

```rust
pub fn validate_path(path: &str, allowed_dirs: &[&str]) -> Result<String, String> {
    if path.len() > MAX_PATH_LENGTH {
        return Err("Path exceeds maximum length".to_string());
    }

    for c in FORBIDDEN_CHARS {
        if path.contains(*c) {
            return Err(format!("Path contains forbidden character: {:?}", c));
        }
    }

    let is_allowed = allowed_dirs.iter().any(|dir| path.starts_with(dir));
    if !is_allowed {
        return Err(format!(
            "Path must be within allowed directories: {:?}",
            allowed_dirs
        ));
    }

    Ok(path.to_string())
}
```

The validation fails in two ways:
1.  It does not resolve or canonicalize the path. It merely takes a raw string slice input.
2.  It relies on `path.starts_with(dir)`.

Because dot `.` is not present in `FORBIDDEN_CHARS` (`crates/op-agents/src/agents/base.rs:337`), an attacker can pass a path such as `/home/../etc/passwd` or `/home/../etc/shadow`. Since this string begins with `/home`, it evaluates `path.starts_with("/home")` to `true`.

When this validated string is passed directly to file read or shell execution functions, it escapes `/home` and reads arbitrary files from the system. 

#### Affected Agents
*   `DocsArchitectAgent` (`crates/op-agents/src/agents/content/docs_architect.rs:31-33` via `std::fs::read_to_string`)
*   `DebuggerAgent` (`crates/op-agents/src/agents/analysis/debugger.rs:33-35` via `tail` command execution)
*   `SqlProAgent` (`crates/op-agents/src/agents/database/sql_pro.rs:34` via database path mounting)

#### Remediation
Replace the legacy string prefix check with canonicalized absolute path evaluation:
```rust
let canonical = std::fs::canonicalize(Path::new(path))
    .map_err(|e| e.to_string())?;
let is_allowed = allowed_dirs.iter()
    .map(PathBuf::from)
    .filter_map(|d| std::fs::canonicalize(d).ok())
    .any(|allowed| canonical.starts_with(allowed));
```

---

### Critical Finding 2: Remote Code Execution / Host Privilege Escalation via Argument Splitting & Flag Injection
*   **File:Line Citation**: `crates/op-agents/src/agents/language/golang_pro.rs:31-37` (also in `c_pro.rs:32`, `cpp_pro.rs:31`, `python_pro.rs:31`, and the generator template `crates/op-agents/src/generator/template.rs:597-601`)
*   **Vulnerability Type**: Argument Injection (CWE-88)
*   **Exploitability**: Directly Exploitable

#### Technical Analysis
The validation of arguments in the agent execution blocks checks for bad characters but completely allows the separation of safe tokens into multi-argument structures via whitespace splitting:

```rust
fn go_build(&self, path: Option<&str>, args: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new("go");
    cmd.arg("build");

    if let Some(a) = args {
        validation::validate_args(a)?;
        for arg in a.split_whitespace() {
            cmd.arg(arg);
        }
    }
```

By passing a string such as `-toolexec /bin/sh` to `go_build` (or `-wrapper /bin/sh` to `gcc_compile` / `gpp_compile`), no blacklisted character in `FORBIDDEN_CHARS` is matched. The string splits cleanly on space boundaries, and is pushed to `Command::new` as:
1.  `cmd.arg("-toolexec")`
2.  `cmd.arg("/bin/sh")`

When `go build` is spawned by the agent manager, it executes `/bin/sh` as the compiler tool wrapper. If the agent is running on the D-Bus system bus as root (configured in `dbus-agent-manager.rs:43`), this yields instant, interactive host root privilege escalation.

#### Remediation
Never split arguments via arbitrary whitespace loops on raw strings. Pass arguments strictly as structured arrays validated against safe regex patterns, or avoid compiling/executing arbitrary unchecked subprocesses on the host using plain `Command::new`.

---

### Critical Finding 3: Arbitrary Command Whitelist Bypass via Path Stripping in `validate_command`
*   **File:Line Citation**: `crates/op-agents/src/security/validation.rs:163-181`
*   **Vulnerability Type**: Path Traversal / Validation Bypass (CWE-426)
*   **Exploitability**: Directly Exploitable

#### Technical Analysis
The function `validate_command` attempts to verify if a binary is whitelisted before execution:

```rust
pub fn validate_command<'a>(
    command: &'a str,
    whitelist: &[String],
) -> Result<&'a str, ValidationError> {
    ...
    // Extract the base command (first component)
    let base_command = command.split_whitespace().next().unwrap_or(command);

    // Extract just the command name without path
    let cmd_name = Path::new(base_command)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(base_command);

    if !whitelist
        .iter()
        .any(|allowed| allowed == cmd_name || allowed == base_command)
    {
        return Err(ValidationError::CommandNotAllowed(command.to_string()));
    }

    Ok(command)
}
```

Because `Path::new(base_command).file_name()` extracts just the trailing filename segment, an input command of `"/tmp/python3"` resolves to `"python3"`. Since `"python3"` exists in the preset whitelist, validation succeeds, and the function returns the raw, unmodified string `/tmp/python3`.

Since `/tmp` is a writable directory in execution profiles, an attacker can write a custom malicious script or binary to `/tmp/python3` and invoke `/tmp/python3` through the sandbox executor, successfully bypassing the binary whitelist completely.

#### Remediation
Verify that the `command` does not contain absolute paths or directory separators, or enforce that any path prefix strictly points to a read-only root system path (such as `/usr/bin/`).

```rust
if Path::new(command).is_absolute() || command.contains('/') {
    return Err(ValidationError::InvalidPath("Absolute or relative paths not allowed in command names".into()));
}
```

---

## 5. High Risk & Security Smells

### Smell 1: JSON Injection via Manual String Formatting in Memory Agent
*   **File:Line Citation**: `crates/op-agents/src/agents/orchestration/memory.rs:242-267`
*   **Vulnerability Type**: Injection / Serialization Flaw (CWE-74)

#### Technical Analysis
The memory agent serializes cache entries back to disk using ad-hoc manual string interpolation:

```rust
let entry_json = format!(
    "\"{}\":{{\"value\":\"{}\",\"memory_type\":\"{}\",\"tags\":[{}],\"created_at\":{},\"updated_at\":{},\"access_count\":{},\"last_accessed\":{}{}}}",
    key, entry.value, memory_type_str, tags_json, entry.created_at, entry.updated_at, 
    entry.access_count, entry.last_accessed, expires_json
);
```

Neither `key` nor `entry.value` are sanitized or escaped. If a user feeds input containing backslashes, double-quotes, or nested JSON brackets to the agent, the output file `/var/lib/op-dbus/memory_cognitive.json` becomes malformed or structurally altered. When the service restarts, `simd_json` will fail to parse the file (resulting in denial-of-service) or parse an injected key-value payload representing poisoned system settings.

#### Remediation
Always use standard structural serializers like `serde_json` or `simd_json` to marshal structured data:
```rust
serde_json::to_string(&self.cache.read().unwrap())
```

---

### Smell 2: Insecure Symlink Traversal due to Lack of Path Canonicalization
*   **File:Line Citation**: `crates/op-agents/src/security/validation.rs:114-138`
*   **Vulnerability Type**: Symlink Traversal (CWE-59)

#### Technical Analysis
`validate_path` constructs a `PathBuf` directly from a raw string without resolving symlinks on the filesystem:

```rust
// Parse and canonicalize the path
let path_buf = PathBuf::from(path);

// Check for path traversal attempts
if path.contains("..") { ... }
```

If an attacker creates a symbolic link inside an allowed directory (such as `/home/user/log_link` pointing to `/etc/passwd`), `validate_path` evaluates the path as starting with `/home`, checking out as valid. Upon access, the operating system traverses the symlink, exposing forbidden system configuration files.

#### Remediation
Ensure you canonicalize the path with `std::fs::canonicalize` prior to matching base prefixes:
```rust
let canonical_path = std::fs::canonicalize(Path::new(path))
    .map_err(|_| ValidationError::InvalidPath("Unresolvable path".into()))?;
```

---

### Smell 3: Denial of Service via Memory Exhaustion in D-Bus Message Deserialization
*   **File:Line Citation**: `crates/op-agents/src/dbus_service.rs:141-150`
*   **Vulnerability Type**: Resource Exhaustion (CWE-400)

#### Technical Analysis
The D-Bus execution interface accepts a raw string `task_json: String` without verifying its length before calling the deserializer:

```rust
async fn execute(&self, task_json: String) -> Result<String, zbus::fdo::Error> {
    ...
    let mut task_json_mut = task_json.to_string();
    let task: AgentTask = unsafe { simd_json::from_str(&mut task_json_mut) }
```

A malicious local process communicating over D-Bus can send an extremely large string containing millions of blank spaces or array fields. Because the service does not enforce payload length restrictions, this causes excessive memory allocation and thread panic inside `simd_json`, crashing the agent manager.

#### Remediation
Enforce a hard limit on D-Bus message parameters prior to deserialization:
```rust
if task_json.len() > MAX_INPUT_LENGTH {
    return Err(zbus::fdo::Error::InvalidArgs("Payload too large".into()));
}
```

---
## ⚠ Citation Warnings
- `crates/op-agents/src/agents/base.rs:341`: file has 255 lines
- `crates/op-agents/src/agents/base.rs:337`: file has 255 lines
