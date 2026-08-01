# Configuration Analysis

## 1. Environment Variable Reads

Below is a complete list of all `std::env::var` reads within the `op-mcp` crate, including their locations and fallback/error-handling behaviors.

| Environment Variable | File & Line Citation | Has Default / Handling | Description / Behavior |
| :--- | :--- | :--- | :--- |
| `DBUS_AGENT_SESSION` | `crates/op-mcp/src/main.rs:175` | **Yes (Partial)** | Checked using `.is_ok()`. Falls back to `BusType::System` if the variable is not set. However, there is no configuration profile-driven default value. |
| `MCP_TOOL_FILTER` | `crates/op-mcp/src/tool_adapter.rs:60` | **Yes (Partial)** | Read via `.ok().as_deref()`. If not matched, it defaults to a warning and returns `true` (enabling all tools). No configuration profile-driven default exists in code. |
| `MCP_TOOL_FILTER` | `crates/op-mcp/src/tool_adapter.rs:222` | **Yes (Partial)** | Checked using `if let Ok(filter)`. Triggers logging. |
| `MCP_TOOL_FILTER` | `crates/op-mcp/src/tool_adapter_orchestrated.rs:41` | **Yes (Partial)** | Read via `.ok().as_deref()`. Defaults to allowing all tools if unset. |
| `CHAT_CONTROL_MCP_BASE_URL` | `crates/op-mcp/src/http_server.rs:534` | **Yes** | Loaded via `.ok()`. Used as a fallback base url for SSE and POST endpoints. |
| `CHAT_CONTROL_MCP_SSE_URL` | `crates/op-mcp/src/http_server.rs:536` | **Yes** | Loaded via `.ok()`. Falls back to the base url. |
| `CHAT_CONTROL_MCP_POST_URL` | `crates/op-mcp/src/http_server.rs:541` | **Yes** | Loaded via `.ok()`. Falls back to base url or `"/api/chat/mcp"`. |
| `CHAT_CONTROL_MCP_NAME` | `crates/op-mcp/src/http_server.rs:546` | **Yes** | Read using `.unwrap_or_else(|_| "chat-control".to_string())`. |
| `CHAT_CONTROL_MCP_DESCRIPTION` | `crates/op-mcp/src/http_server.rs:548` | **Yes** | Read using `.unwrap_or_else(|_| "Chat Control MCP (op-web) coordinator".to_string())`. |
| `QDRANT_URL` | `crates/op-mcp/src/tools/qdrant.rs:80` | **Yes** | Read via `.ok()`. Falls back to `"http://localhost:6333"`. |

### Flagged Environment Variables with Weak or No Configuration Fallbacks
*   **`MCP_TOOL_FILTER`** (`crates/op-mcp/src/tool_adapter.rs:60`): This variable controls which toolsets (such as D-Bus interfaces, agents, skills) are active. Lacking a configuration-managed default schema means changes in filters rely entirely on manual shell configurations rather than robust file-based configuration profiles.
*   **`DBUS_AGENT_SESSION`** (`crates/op-mcp/src/main.rs:175`): Lacks configuration contract defaults.

---

## 2. Cargo Features Analysis

Features declared in `crates/op-mcp/Cargo.toml`:

*   `default = []`
*   `op-chat = []`
*   `code_search = []`
*   `grpc = ["tonic", "prost", "tonic-build"]`

### Additive Check
All features are **additive**. The `default` feature list is empty, ensuring compilation remains minimal by default, and optional integrations (such as gRPC support and code search context-injection) can be enabled progressively.

---

## 3. Hardcoded Ports, IP Addresses, and Paths

The following static endpoints, ports, directories, and paths are hardcoded within the codebase:

### Network Interfaces & Bind Addresses
*   **Default HTTP Bind Address**: `"0.0.0.0:3001"` — `crates/op-mcp/src/main.rs:160`
*   **Default WebSocket Bind Address**: `"0.0.0.0:3002"` — `crates/op-mcp/src/main.rs:165`
*   **Default gRPC Bind Address**: `"0.0.0.0:50051"` — `crates/op-mcp/src/main.rs:170`
*   **Hardcoded Port Assignment**: `format!("0.0.0.0:{}", port)` — `crates/op-mcp/src/main.rs:132`
*   **gRPC Loopback Server default**: `"[::1]:50051"` — `crates/op-mcp/src/grpc/server.rs:41`
*   **gRPC Loopback Client default**: `"http://[::1]:50051"` — `crates/op-mcp/src/grpc/client.rs:31`
*   **Qdrant Vector Database Host**: `"http://localhost:6333"` — `crates/op-mcp/src/tools/qdrant.rs:31` and `82`
*   **Static VPN/Mesh Subnets**: Hardcoded IP subnets for IP-based bypass checks are present in `crates/op-mcp/src/http_server.rs`:
    *   Netmaker: `"10.101."`, `"10.102."`, `"10.103."` (Line 47)
    *   ZeroTier: `"10.147."`, `"10.244."` (Line 69)
    *   WireGuard: `"10.0.0."`, `"10.200."`, `"10.66.66."` (Line 74)
    *   Nebula: `"10.42."` (Line 79)

### System Paths & Directories
*   **gRPC Cache Path**: `"/var/lib/op-dbus/cache/grpc"` — `crates/op-mcp/src/grpc/server.rs:52`
*   **gRPC SQLite State Path**: `"/var/lib/op-dbus/state/grpc.db"` — `crates/op-mcp/src/grpc/server.rs:53`
*   **gRPC Blockchain Trail Path**: `"/var/lib/op-dbus/blockchain/grpc"` — `crates/op-mcp/src/grpc/server.rs:54`
*   **Local Network Sysfs Path**: `"/sys/class/net"` — `crates/op-mcp/src/tools/system.rs:24`
*   **System Filesystem Access Validations**: Absolute paths check in `crates/op-mcp/src/tools/filesystem.rs`:
    *   `"/etc/shadow"` & `"/etc/sudoers"` (Line 33)
    *   `"/etc/"` & `"/boot/"` (Line 64)

### D-Bus Interface Signatures
*   **Systemd D-Bus interface definitions**:
    *   `"org.freedesktop.systemd1"` — `crates/op-mcp/src/tools/systemd.rs:26` and `49`
    *   `"/org/freedesktop/systemd1"` — `crates/op-mcp/src/tools/systemd.rs:27`
    *   `"org.freedesktop.systemd1.Manager"` — `crates/op-mcp/src/tools/systemd.rs:28`
    *   `"org.freedesktop.systemd1.Unit"` — `crates/op-mcp/src/tools/systemd.rs:51`

---

# Schema-as-Code Audit

The architecture is designed to enforce structural data contracts. However, the codebase deviates from this in several areas, relying on ad-hoc structs, dynamic `simd_json::json!` definitions, or unstructured strings instead of versioned Protobuf schemas or OSCAL templates:

*   **Ad-Hoc JSON-RPC Request/Response Structs**:
    *   `crates/op-mcp/src/agents_main.rs:27-56`: Structs like `JsonRpcRequest`, `JsonRpcResponse`, and `JsonRpcError` are manually maintained serde models rather than built from a central schema file.
    *   `crates/op-mcp/src/protocol.rs:10-85`: Hand-coded protocol wrappers (`McpRequest`, `McpResponse`, `JsonRpcError`) repeat these schema-less structures.
    *   `crates/op-mcp/src/http_server.rs:191-205`: Hand-coded `McpRequest` and `McpResponse` variants are declared locally for the proxy interface, bypassing the protocol module completely.
*   **Unversioned D-Bus Agent Mapping Metadata**:
    *   `crates/op-mcp/src/agents_server.rs:35-55`: `DiscoveredAgent` and `AgentTool` are declared as ad-hoc Rust structs parsing dynamic introspected parameters with no underlying versioned schema validator.
*   **Inline Tool Input Schemas**:
    The system tools define input schemas dynamically in the code using raw JSON construction instead of referring to pre-compiled static schemas:
    *   `crates/op-mcp/src/tools/filesystem.rs:24`: Input parameters for `read_file`.
    *   `crates/op-mcp/src/tools/filesystem.rs:52`: Input parameters for `write_file`.
    *   `crates/op-mcp/src/tools/filesystem.rs:80`: Input parameters for `list_directory`.
    *   `crates/op-mcp/src/tools/systemd.rs:43`: Input validation parameters for `systemd_unit_status`.
    *   `crates/op-mcp/src/tools/systemd.rs:72`: Input validation parameters for `systemd_list_units`.
    *   `crates/op-mcp/src/tools/ovs.rs:43`, `56`, `68`, etc.: Inline schema matrices for Open vSwitch bridges, port creation, and flow allocations.

---

# Production Security & Quality Findings

### [CRITICAL] Request Handler Completely Bypasses Tool Security Blocklist
**File**: `crates/op-mcp/src/request_handler.rs:175` (also line 114)

#### Impact
The `McpServer` has an explicit security safeguard (`crates/op-mcp/src/server.rs:36` & `330`) preventing execution of dangerous commands via `BLOCKED_PATTERNS`, including `"shell_execute"`. 

However, `RequestHandler::load_tools` directly registers the raw `ShellExecuteTool` (line 175) and handles tool calls in `handle_tools_call` (line 114) via a private `RequestContext` without checking any blocked patterns. 

An unauthenticated stdio or loopback connection using `RequestHandler` (e.g., via the compact mode meta-tools) can invoke `execute_tool` to run the `shell_execute` tool. Since the `ShellExecuteTool` whitelist includes high-level interpreters like `python`, `node`, `npm`, and `cargo`, an attacker can easily execute arbitrary system commands on the host by passing arguments such as `python -c "import os; os.system(...)"`.

#### Mitigation
Ensure `RequestHandler` and `RequestContext::execute_tool` reject execution of any tool matching `BLOCKED_PATTERNS`. Alternatively, remove dangerous tools from the registry or delegate tool execution validation exclusively to the secure `McpServer` structure.

---

### [MEDIUM] Unsafe In-Place Parser Instability
**Files**:
*   `crates/op-mcp/src/agents_main.rs:434`
*   `crates/op-mcp/src/agents_server.rs:252`
*   `crates/op-mcp/src/transport/stdio.rs:47`
*   `crates/op-mcp/src/transport/websocket.rs:114`

#### Impact
In multiple locations, JSON deserialization is performed via `unsafe { simd_json::from_str(&mut line) }`. The `simd-json` crate requires that the input string buffer is mutable because it performs in-place modifications (destructive parsing). 

If a multi-threaded transport or concurrent worker thread accesses or shares the same memory address of the parsed string buffer during a stream operation, this will trigger a data race and undefined behavior.

#### Mitigation
Ensure input buffers are strictly isolated and thread-local, or switch to a safe parsing interface (like `simd_json::serde::from_str` or `serde_json::from_str`) for structures exposed to untrusted external streams.

---

### [LOW] Dynamic D-Bus Introspection Property Parsing Failure Risks
**File**: `crates/op-mcp/src/agents_server.rs:131-143`

#### Impact
The agent introspection logic dynamically retrieves string properties (`name`, `description`, `operations`) from a running D-Bus service:
```rust
let name: String = proxy.call("name", &()).await.unwrap_or_else(|_| agent_type_pascal.to_string());
```
If a D-Bus agent changes its signature, hangs, or returns a malformed data type, the service handler falls back but continues execution with unstructured fallback names. Since `operations` returns a `Vec<String>`, failures in retrieving this property fall back to a hardcoded `"execute"` string, which may not align with the actual agent implementation and can cause runtime command mismatch failures.

#### Mitigation
Implement validation on introspected properties, and return explicit D-Bus communication errors instead of falling back to default operations.