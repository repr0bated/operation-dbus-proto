# Production Security & Quality Audit Report

---

## 1. Vulnerability & Safety Audit

### CRITICAL: Authentication Bypass via Cosmetic Bearer Token Validation
- **Citations**: 
  - `crates/op-mcp/src/http_server.rs:173-214`
  - `crates/op-mcp/src/transport/http.rs:31-77`
- **Impact**: Full Authentication Bypass leading to Remote Code Execution (RCE).
- **Description**: 
  The authentication middleware (`auth_middleware` in `http_server.rs` and `wireguard_auth_middleware` in `transport/http.rs`) intercepts incoming HTTP/SSE requests and verifies the `Authorization` header. However, the validation mechanism relies entirely on structural checks defined in `is_wireguard_auth_token`:
  ```rust
  fn is_wireguard_pubkey(token: &str) -> bool {
      token.len() == 44
          && token.ends_with('=')
          && token
              .chars()
              .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
  }

  fn is_wireguard_session_id(token: &str) -> bool {
      Uuid::parse_str(token).is_ok()
  }

  fn is_wireguard_auth_token(token: &str) -> bool {
      is_wireguard_pubkey(token) || is_wireguard_session_id(token)
  }
  ```
  This is a purely cosmetic check. There is no cryptographic signature validation, no session store query, and no database validation of the token. Any HTTP request supplying a structurally valid bearer token (such as any arbitrary UUID or any random 44-character string ending in `=`) will be successfully authenticated and permitted full administrative access to the MCP endpoints.

---

### CRITICAL: Remote Code Execution (RCE) via Whitelisted Python interpreter in `shell_execute`
- **Citations**:
  - `crates/op-mcp/src/tools/shell.rs:35-90`
  - `crates/op-mcp/src/request_handler.rs:192-230`
  - `crates/op-mcp/src/request_context.rs:224-237`
- **Impact**: Full system compromise and execution of arbitrary code with the privileges of the MCP process.
- **Description**: 
  While `ToolAdapter` (in `tool_adapter.rs`) implements a restriction pattern to block dangerous mutations and shell execution tools from MCP exposure, `RequestHandler` (used in compact server modes and SSE handlers) bypasses `ToolAdapter` altogether and directly registers the underlying tools in the request's context:
  ```rust
  // Shell tools
  ctx.load_tool(Arc::new(tools::shell::ShellExecuteTool::new()));
  ```
  The `ShellExecuteTool` whitelist includes `"python"`, `"python3"`, and `"node"`. An attacker who bypasses authentication (using the cosmetic token validation above) can submit a request calling the `execute_tool` meta-tool with the argument `tool_name = "shell_execute"`. Since `RequestContext::execute_tool` performs no blocklist checks, it invokes the tool directly. By specifying `command = "python"` and passing arbitrary Python scripts in `args` (such as calling `os.system()` or launching a reverse shell), an attacker can execute arbitrary code on the target system.

---

### HIGH: Undefined Behavior & Information Disclosure via Unsafe `simd_json` on Unpadded Buffers
- **Citations**:
  - `crates/op-mcp/src/agents_main.rs:484`
  - `crates/op-mcp/src/agents_server.rs:253`
  - `crates/op-mcp/src/external_client.rs:434`
  - `crates/op-mcp/src/external_client.rs:521`
  - `crates/op-mcp/src/transport/stdio.rs:49`
  - `crates/op-mcp/src/transport/websocket.rs:113`
- **Impact**: Out-of-bounds reads, segmentation faults (Denial of Service), or potential heap memory disclosure.
- **Description**: 
  The codebase extensively invokes `simd_json::from_str` within `unsafe` blocks. For example, in `transport/stdio.rs:49`:
  ```rust
  let response = match unsafe { simd_json::from_str::<McpRequest>(&mut line_mut) }
  ```
  `simd-json` explicitly requires that the input buffer must be padded with `simd_json::PADDING` (typically 32 or 64 bytes depending on the vector architecture) of addressable memory at the end. Standard Rust `String` instances returned by `BufRead::lines()`, `stdout.read_line()`, or `tokio::fs::read_to_string()` do not guarantee this padding. When SIMD vector instructions parse near page boundaries, they can read unaddressable memory, triggering SIGSEGV faults or pulling adjacent heap data into the JSON object representation.

---

### MEDIUM: Performance Pitfall & Allocation Spikes via Deep Cloning of Large JSON Trees
- **Citations**:
  - `crates/op-mcp/src/compact.rs:133`
  - `crates/op-mcp/src/server.rs:348`
  - `crates/op-mcp/src/server.rs:527`
- **Impact**: Excessive heap allocations, CPU spikes, and lock contention on high-throughput request paths.
- **Description**: 
  During tool call handling, the server extracts parameters and executes deep clones of `simd_json::OwnedValue` objects:
  ```rust
  let arguments = params
      .as_object()
      .and_then(|obj| obj.get("arguments"))
      .cloned()
      .unwrap_or(json!({}));
  ```
  Deep-cloning complex JSON trees on every request rather than passing references or using shared data structures like `Arc` completely negates the performance benefits of using a SIMD-accelerated parser.

---

## 2. Performance & Allocation Analysis

### Vector/String Re-allocations in Loops
1. **pascal_to_snake** (`crates/op-mcp/src/agents_server.rs:279-293`): 
   Initializes an empty string (`String::new()`) and dynamically pushes characters inside a loop. Since the input is a PascalCase identifier, pre-allocating with `String::with_capacity(s.len() + 4)` would prevent multiple re-allocations as underscores are inserted.
2. **allowed_tools collector** (`crates/op-mcp/src/tool_adapter.rs:203`): 
   Initializes `let mut allowed_tools = Vec::new();` and populates it via iteration. Pre-allocating using `Vec::with_capacity(local_tools.len() + external_tools.len())` is recommended to avoid costly heap resizes.
3. **convert_json_schema_to_tool_schema** (`crates/op-mcp/src/grpc/service.rs:777`):
   Initializes an un-allocated `let mut parameters = Vec::new();` inside schema conversions, which run during tool discovery and listing. Capacity should be reserved based on `props.len()`.

### Hot-Path `format!` Allocations
- **`crates/op-mcp/src/grpc/service.rs:748`**: Formats the streaming tool progress message on every execution tick: `content: format!("Executing tool: {}", tool_name)`.
- **`crates/op-mcp/src/agents_main.rs:290`**: Nested formatting inside the hot execution path of sequential thinking:
  ```rust
  "message": format!("Thinking step {} recorded: {}", step_number,
      if thought.len() > 50 { format!("{}...", &thought[..50]) } else { thought.to_string() })
  ```

---

## 3. Memory Mapping & Storage Analysis

### Raw Memory Mapping Sites
There are no raw memory-mapping calls (`memmap2`, `MmapMut`, etc.) directly invoked in the provided `op-mcp` source files. However, the workspace dependency tree reveals mapping vectors:
- **`cozo` Database Sled Storage Engine** (`Cargo.toml`): The workspace depends on `cozo` with the `storage-sled` feature. Sled internally establishes memory mappings to persist state. If the cache or database files (such as `/var/lib/op-dbus/state/grpc.db`) are situated on a `tmpfs` or a mount marked `noexec`, writing to mapped files can trigger bus errors or database corruption on system reboots or page faults.

### Large Heap Allocations (>1MB)
- **gRPC Message Sizing (`crates/op-mcp/src/grpc/server.rs:44`)**: 
  The default configuration allocates a maximum message size of `16 * 1024 * 1024` (16 MB) on the heap for gRPC decoders:
  ```rust
  max_message_size: 16 * 1024 * 1024
  ```
  When combined with multiple concurrent streams, this facilitates memory exhaustion attacks (OOM DoS) if clients feed large chunks of data.

### Memory Map Table

| Site | file:line | Type | Risk |
| :--- | :--- | :--- | :--- |
| **Sled via Cozo** | `Cargo.toml` | `sled` (Internal mmaps) | Database corruption if database paths are placed on `tmpfs` or `noexec` mounts. |
| **gRPC Message Decoder** | `crates/op-mcp/src/grpc/server.rs:44` | Heap | Up to 16MB heap allocation per message; exposed to denial of service (OOM). |

---

## 4. Schema-As-Code Compliance

This repository mandates the use of versioned schemas (such as Protocol Buffers and OSCAL) for establishing data contracts. The following locations violate this discipline by resorting to ad-hoc structures or raw string manipulation:

### Ad-hoc JSON-RPC & MCP Structures
- **`crates/op-mcp/src/agents_main.rs:30-58`**: Defines local, ad-hoc `JsonRpcRequest`, `JsonRpcResponse`, and `JsonRpcError` structs. These contracts should be derived directly from the unified Protobuf schema.
- **`crates/op-mcp/src/protocol.rs:10-53`**: Re-implements `McpRequest` and `McpResponse` as local Rust types with `simd_json::OwnedValue` fields, deviating from the versioned gRPC proto contracts.

### Raw JSON/OSCAL Schema Definitions as String Literals
- **`crates/op-mcp/src/compact.rs:421-490`**: Defines the compact meta-tool schemas as inline, un-versioned JSON literals rather than compiling them from schema definitions.
- **`crates/op-mcp/src/agents_server.rs:194-210`**: Hardcodes a generic input schema as an ad-hoc `simd_json` literal inside `get_operation_schema`.
- **`crates/op-mcp/src/tools/ovs.rs` & `crates/op-mcp/src/tools/systemd.rs`**: Hand-craft the JSON schemas for the system tools as raw JSON structures (e.g. `json!({"type": "object", "properties": {...}})`). These input contracts should be defined inside a versioned schema file.

---
## ⚠ Citation Warnings
- `crates/op-mcp/src/external_client.rs:521`: file has 486 lines
- `crates/op-mcp/src/grpc/service.rs:777`: file has 707 lines
- `crates/op-mcp/src/grpc/service.rs:748`: file has 707 lines
