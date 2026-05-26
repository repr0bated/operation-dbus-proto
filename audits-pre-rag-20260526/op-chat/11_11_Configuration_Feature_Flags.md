### `std::env::var` Reads

| File | Line | Environment Variable | Purpose / Fallback Behavior |
| :--- | :--- | :--- | :--- |
| `crates/op-chat/src/grpc_client.rs` | 44 | `OP_DBUS_GRPC_ADDR` | Specifies the op-dbus gRPC server address. Falls back to `"http://10.200.0.2:50051"`. |
| `crates/op-chat/src/grpc_client.rs` | 245 | `OP_RUN_ON_CONNECTION_AGENTS` | Specifies agents launched at session connection. Falls back to `"rust_pro,backend_architect,sequential_thinking,memory,context_manager"`. |
| `crates/op-chat/src/main.rs` | 14 | `OP_CHAT_LISTEN` | Specifies the TCP address and port to bind to. Falls back to `"0.0.0.0:50052"`. |
| `crates/op-chat/src/system_prompt.rs` | 406 | `CUSTOM_SYSTEM_PROMPT` | Retrieves dynamic system prompt overrides. If absent, falls back to looking up file-based overrides. |
| `crates/op-chat/src/system_prompt.rs` | 493 | `OP_SELF_REPO_PATH` | Checks for existence of the self-repository path to decide whether to append self-repository context to the system prompt. |
| `crates/op-chat/src/system_prompt.rs` | 511 | `OP_SELF_REPO_PATH` | Checks if self-repository path exists to populate metadata properties. |
| `crates/op-chat/src/system_prompt.rs` | 512 | `OP_SELF_REPO_PATH` | Retrieves the path of the self-repository if present. |
| `crates/op-chat/src/orchestration/grpc_pool.rs` | 81 | `OP_AGENT_POOL_ADDRESS` | Sets the base address for agent gRPC services. If absent, uses the default `base_address` configuration. |
| `crates/op-chat/src/orchestration/grpc_pool.rs` | 85 | `OP_AGENT_CONNECT_TIMEOUT_MS` | Configures the timeout in milliseconds for establishing connection to agents. Fallback is the default `connect_timeout`. |
| `crates/op-chat/src/orchestration/grpc_pool.rs` | 91 | `OP_AGENT_REQUEST_TIMEOUT_MS` | Configures the default timeout in milliseconds for operations. Fallback is the default `request_timeout`. |
| `crates/op-chat/src/orchestration/grpc_pool.rs` | 97 | `OP_RUN_ON_CONNECTION_AGENTS` | Sets the list of agents initiated at session connection. Fallback is default list. |

---

### Environment Variables Lacking Default Values or Error Handling

There are **no unhandled environment variable reads** that lack fallback default values or proper error handling in the provided code. Every `std::env::var` call is either protected by safe conditional matching (`if let Ok(...)` / `is_ok()` / `ok()`) or provides fallback default behaviors via `unwrap_or_else` or `unwrap_or`.

---

### Cargo Features

*   **Workspace Level (`Cargo.toml`)**:
    *   `default`: Enabled by default. Includes the `"grpc"` feature.
    *   `grpc`: Exposes the gRPC endpoints and enables related tonic dependencies.
*   **Crate Level (`crates/op-chat/Cargo.toml`)**:
    *   No custom features are defined.

#### Feature Additivity
All Cargo features in Rust are naturally **additive**. If any crate in the dependency graph enables a feature (including the `grpc` feature from the workspace), it is enabled globally across the build graph. Default features can be disabled using `default-features = false`.

---

### Hardcoded Paths, Ports, and Addresses

| File | Line | Hardcoded Parameter | Description |
| :--- | :--- | :--- | :--- |
| `crates/op-chat/src/grpc_client.rs` | 46 | `"http://10.200.0.2:50051"` | Fallback target IP and port for the op-dbus gRPC server. |
| `crates/op-chat/src/grpc_client.rs` | 276 | `"/org/opdbus/agents/{}"` | Hardcoded D-Bus object path prefix. |
| `crates/op-chat/src/grpc_client.rs` | 277 | `"org.opdbus.AgentV1"` | Hardcoded D-Bus interface string. |
| `crates/op-chat/src/grpc_client.rs` | 328 | `"/agents/{}/{}"` | Hardcoded subscription path filter pattern. |
| `crates/op-chat/src/chat_loop.rs` | 37 | `"deepseek-ai/DeepSeek-V2.5"` | Hardcoded default LLM model identifier. |
| `crates/op-chat/src/main.rs` | 15 | `"0.0.0.0:50052"` | Fallback listen socket binding address for the MCP server. |
| `crates/op-chat/src/system_prompt.rs` | 13 | `"/etc/op-dbus/custom-prompt.txt"` | Primary fallback path for production prompt overrides. |
| `crates/op-chat/src/system_prompt.rs` | 14 | `"./custom-prompt.txt"` | Secondary fallback path for local development prompt overrides. |
| `crates/op-chat/src/system_prompt.rs` | 15 | `"../custom-prompt.txt"` | Tertiary fallback path for local development prompt overrides. |
| `crates/op-chat/src/system_prompt.rs` | 45 | `"/var/run/openvswitch/db.sock"` | Socket path for native OVSDB JSON-RPC protocol communication. |
| `crates/op-chat/src/system_prompt.rs` | 114 | `"/etc/hosts"` | Hardcoded path example within system prompt. |
| `crates/op-chat/src/system_prompt.rs` | 200 | `"/var/run/dbus/system_bus_socket"` | Socket path for System D-Bus access. |
| `crates/op-chat/src/system_prompt.rs` | 201 | `"/var/run/netclient/netclient.sock"` | Socket path for Netmaker overlay networking. |
| `crates/op-chat/src/system_prompt.rs` | 446 | `"/etc/op-dbus/custom-prompt.txt"` | Output target path when saving dynamic prompt modifications. |
| `crates/op-chat/src/tool_loader.rs` | 309 | `"/etc/shadow"`, `"/etc/sudoers"` | Hardcoded blacklist check inside `ReadFileTool`. |
| `crates/op-chat/src/tool_loader.rs` | 360 | `"/etc/"`, `"/boot/"`, `"/sys/"`, `"/proc/"` | Hardcoded restricted prefixes blacklist inside `WriteFileTool`. |
| `crates/op-chat/src/tool_loader.rs` | 434 | `"/sys/class/net"` | Hardcoded sysfs interface path. |
| `crates/op-chat/src/tool_loader.rs` | 443 | `"/sys/class/net/{}/operstate"` | Hardcoded sysfs operational state path. |
| `crates/op-chat/src/tool_loader.rs` | 451 | `"/sys/class/net/{}/address"` | Hardcoded sysfs MAC address path. |
| `crates/op-chat/src/tool_loader.rs` | 521 | `"/org/freedesktop/systemd1"` | Hardcoded object path for Systemd D-Bus interface. |
| `crates/op-chat/src/orchestration/dbus_orchestrator.rs` | 30 | `"/com/system/orchestrator/Manager"` | Hardcoded object path for the system orchestrator D-Bus service. |
| `crates/op-chat/src/orchestration/grpc_pool.rs` | 60 | `"http://127.0.0.1"` | Fallback base address for agent gRPC networking. |
| `crates/op-chat/src/orchestration/grpc_pool.rs` | 114-123 | Ports `50051` to `50060` | Hardcoded static port allocations mapped to individual agent services. |

---

### Critical Security Findings

#### CRITICAL: Path Traversal Security Bypass in `ReadFileTool` and `WriteFileTool`
*   **Citations**:
    *   `crates/op-chat/src/tool_loader.rs:309-314`
    *   `crates/op-chat/src/tool_loader.rs:360-365`
*   **Vulnerability Description**:
    The `ReadFileTool` and `WriteFileTool` implementations perform basic path validation to block access to sensitive files (such as `/etc/shadow`) and directories (such as `/etc/`) by using a simple `path.starts_with(prefix)` check. However, the input `path` is passed directly to the filesystem (`tokio::fs::read_to_string` and `tokio::fs::write`) **without canonicalization** (resolving symlinks, relative directories, and dot segments).
*   **Exploitation Vector**:
    An attacker who can make tool calls via gRPC/MCP or guide the LLM to invoke these tools can completely bypass the path validation checks. By utilizing relative directory traversal (e.g., `"/tmp/../../etc/shadow"` or `"/tmp/../../etc/cron.d/payload"`), the `starts_with` check will fail to match the forbidden prefixes (since the string starts with `"/tmp/"`), yet the underlying operating system will resolve the path to the sensitive destination. This results in **arbitrary file read** (leaking password hashes) and **arbitrary file write** (enabling privilege escalation or remote code execution via cron directories).