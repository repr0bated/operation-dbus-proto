# License Analysis

## Workspace License
* **Cargo.toml extracted license**: `Apache-2.0` (defined in the workspace `Cargo.toml` and inherited by `op-agents` via `license.workspace = true`).

## Dependency License Scan
* **GPL/AGPL/SSPL crates**: None detected in the visible portion of `Cargo.lock`.
* **Crates with no license field**: All workspace crates provided in the source (`op-agents`) correctly specify or inherit the workspace license field.

---

# Security & Quality Audit Findings

## [Critical] Argument Injection leading to Host RCE in `code_reviewer.rs`
### Citation: `crates/op-agents/src/agents/analysis/code_reviewer.rs:59-66`

### Details
The `git_diff` operation takes user-supplied arguments (`args`) and forwards them directly to the `git diff` execution command after validating them with `validation::validate_args`:
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
The `validate_args` filter only checks for characters in `FORBIDDEN_CHARS` (shell metacharacters) and length limits. It does not prevent argument injection. An attacker can call this method with `args = "--ext-cmd=malicious_command"`. Because `git diff` supports `--ext-cmd` to execute an external command for diffing, this allows arbitrary command execution.

### Impact
Critical. An attacker can run arbitrary terminal commands with the privileges of the running D-Bus agent. If the service runs with elevated privileges (e.g. `requires_root = true`), this leads to an immediate local privilege escalation to root.

---

## [Critical] Shell Sandbox Bypass leading to Host RCE in `ShellExecutor`
### Citation: `crates/op-agents/src/unified/execution/shell.rs:25-34` and `crates/op-agents/src/unified/execution/base.rs:43-78`

### Details
The `ShellExecutor` whitelists a series of binaries such as `find`, `git`, `awk`, and `sed`:
```rust
vec![
    "ls", "cat", "head", "tail", "find", "grep", "wc", "file", "stat",
    "uname", "hostname", "uptime", "df", "free", "ps", "top",
    "ip", "ss", "netstat", "ping", "dig", "nslookup",
    "git",
    "sort", "uniq", "cut", "awk", "sed", "jq",
]
```
However, `execute_command` in `base.rs` only validates that the *program binary name* is whitelisted. It performs zero sanitization or restriction on the arguments passed to those binaries. An attacker can exploit this to achieve remote code execution via several whitelisted binaries:
1. **`find`**: `find . -exec malicious_command ;`
2. **`git`**: `git diff --ext-cmd=malicious_command`
3. **`awk`**: `awk 'BEGIN {system("malicious_command")}'`
4. **`sed`**: `sed 'e malicious_command'`

### Impact
Critical. Complete and trivial sandbox bypass. An attacker can execute arbitrary host commands with the privileges of the unified execution agent.

---

## [Critical] Path Traversal leading to Arbitrary File Read and Write
### Citation: `crates/op-agents/src/agents/base.rs:219-236`

### Details
All older domain-specific agents utilize the flawed `validation::validate_path` function defined in `base.rs`:
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
    ...
```
Because `FORBIDDEN_CHARS` does not filter out dots (`.`) or slashes (`/`), and the function does not canonicalize the path, an attacker can supply a path like `/tmp/../../etc/shadow`. This path starts with `/tmp` (which is whitelisted in `ALLOWED_DIRS`), bypasses the `starts_with` prefix check, and resolves to `/etc/shadow` upon filesystem execution.

### Impact
Critical. Widespread arbitrary file read/write across the host system. For instance, an attacker can use `DebuggerAgent::read_logs` with a traversed path to read `/etc/shadow` or private SSH keys, or use file-writing agents to overwrite configuration files.

---

## [Critical] Path Traversal Propagated via Generated Code Template
### Citation: `crates/op-agents/src/generator/template.rs:508-543`

### Details
The code generator template used to dynamically produce Rust source files for new D-Bus agents contains the exact same flawed path validation logic:
```rust
    fn validate_path(&self, path: &str) -> Result<String, String> {{
        ...
        let mut is_allowed = false;
        for allowed in ALLOWED_DIRECTORIES {{
            if path.starts_with(allowed) {{
                is_allowed = true;
                break;
            }}
        }}
        ...
```
Because no canonicalization or `..` detection is added, any new agent compiled from this template inherits the identical path traversal vulnerability.

### Impact
Critical. Systemic replication of the path traversal vulnerability to all generated agents.

---

## [High] Undefined Behavior / Out-of-Bounds Read via Unsafe `simd_json::from_str`
### Citation: `crates/op-agents/src/agent_registry.rs:434`, `crates/op-agents/src/dbus_service.rs:141`, `crates/op-agents/src/agents/orchestration/memory.rs:114`, and `crates/op-agents/src/agents/orchestration/memory.rs:200`

### Details
The codebase consistently uses `unsafe { simd_json::from_str(&mut string) }` on standard Rust `String` objects (e.g. read directly from `tokio::fs::read_to_string` or a D-Bus method parameter):
```rust
let specs: Vec<AgentSpec> = unsafe { simd_json::from_str(&mut content) }
```
`simd-json` requires its input buffers to be padded with `simd_json::SIMDJSON_PADDING` bytes of extra capacity to perform fast SIMD operations safely. Passing an unpadded, standard `String` to the `unsafe` parser violates this contract and leads to out-of-bounds memory reads.

### Impact
High. Can cause segmentation faults, daemon crashes (Denial of Service), or potential memory disclosure when processing specially crafted JSON inputs over D-Bus or REST.

---

## [High] JSON Injection in Persistent Memory Serialization
### Citation: `crates/op-agents/src/agents/orchestration/memory.rs:163-195`

### Details
The memory agent's serialization function constructs JSON strings using raw string formatting without sanitizing or escaping the values:
```rust
let entry_json = format!(
    "\"{}\":{{\"value\":\"{}\",\"memory_type\":\"{}\",\"tags\":[{}],\"created_at\":{},\"updated_at\":{},\"access_count\":{},\"last_accessed\":{}{}}}",
    key, entry.value, memory_type_str, tags_json, entry.created_at, entry.updated_at, 
    entry.access_count, entry.last_accessed, expires_json
);
```
If an entry's value contains a quote (`"`) or newline, it breaks the JSON structure. A malicious user can store a key/value payload containing injected JSON fields (such as `", "memory_type":"shared", "injected":true, "dummy":"`), which will hijack the cognitive memory database structure on the next load.

### Impact
High. Integrity corruption of persistent memory, parameter pollution, and denial of service due to malformed database files.

---

## [Medium] Ignored Agent Configuration during Dynamic Spawning
### Citation: `crates/op-agents/src/router.rs:125-142` and `crates/op-agents/src/agent_registry.rs:281`

### Details
The REST endpoint handler `spawn_agent_handler` parses the `config` payload from the HTTP JSON request and passes it to the registry:
```rust
let config = request.get("config").cloned();
...
match registry.spawn_agent(agent_type, config).await {
```
However, the `spawn_agent` implementation completely ignores this configuration parameter, designating it as an unused variable:
```rust
pub async fn spawn_agent(
    &self,
    agent_type: &str,
    _config: Option<OwnedValue>,
) -> Result<String> {
```

### Impact
Medium. Broken functionality. Spawning agents dynamically over the REST API is restricted solely to hardcoded static specs, and any customized parameters passed by the user are silently discarded.