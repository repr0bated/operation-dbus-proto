# Production Security and Quality Audit: `op-mcp` Crate

## 1. Schema-as-Code Audit

The following table catalogs data contracts and communication payloads that are defined as ad-hoc Rust structs, untyped structures (`simd_json::OwnedValue`), or strings instead of versioned Protocol Buffer definitions:

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `JsonRpcRequest` / `JsonRpcResponse` / `JsonRpcError` | Struct | `crates/op-mcp/src/agents_main.rs:26` | No | Stdio-transport JSON-RPC envelope parsed into ad-hoc Rust Serde structs. |
| `ServerInfo` / `ServerCapabilities` / `ToolDefinition` | Struct | `crates/op-mcp/src/agents_main.rs:80` | No | Tool definition details and capabilities utilize ad-hoc mapping. Input schemas rely on untyped raw JSON `Value` types. |
| `DiscoveredAgent` / `AgentTool` | Struct | `crates/op-mcp/src/agents_server.rs:30` | No | D-Bus agent introspection metadata is modeled using unversioned ad-hoc structs. |
| D-Bus Task Payload | Serialized String | `crates/op-mcp/src/agents_server.rs:302` | No | Task configurations (`task_type`, `operation`, etc.) are serialized as raw JSON strings over the D-Bus IPC boundary instead of utilizing strongly-typed D-Bus structures or a Protocol Buffer schema. |
| `SessionContext` | Struct | `crates/op-mcp/src/compact.rs:18` | No | Gatekeeping session and authentication context modeled in an ad-hoc Rust struct. |
| `Settings` / `ToolConfig` | Struct | `crates/op-mcp/src/config.rs:6` | No | Configuration settings are manually deserialized from unstructured configuration files. |
| `ExternalMcpConfig` / `ExternalTool` | Struct | `crates/op-mcp/src/external_client.rs:14` | No | external MCP clients and subprocess configurations represented as ad-hoc structures. |
| `McpRequest` / `McpResponse` | Struct | `crates/op-mcp/src/protocol.rs:10` | No | The HTTP/SSE protocol transport relies on ad-hoc structs with manual Serde mapping rather than reusing the versioned Protobuf models defined for the gRPC transport. |
| `McpRequest` / `McpResponse` (Redefined) | Struct | `crates/op-mcp/src/router.rs:77` | No | Complete redeclaration of protocol envelope types inside the router module for Axum, creating drift risks with `protocol::McpRequest`. |
| `ResourceInfo` / `ResourceTemplateInfo` | Struct | `crates/op-mcp/src/resources.rs:8` | No | Served template metadata represented using unversioned ad-hoc structs. |
| `QdrantSearchRequest` / `QdrantSearchPayload` | Struct | `crates/op-mcp/src/tools/qdrant.rs:6` | No | Vector database interface structures represented as ad-hoc structs. |

---

## 2. OSCAL Coverage Audit

The following table maps the security controls implemented in the source code to the corresponding NIST SP 800-53 security control areas and identifies gaps in OSCAL compliance (specifically, lack of machine-readable SSP or Component Definition representation):

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **Identification and Authentication (IA-2 / IA-8)** | `crates/op-mcp/src/http_server.rs:145` | SSP / Component Definition | WireGuard-based public key and session ID Bearer authentication is hardcoded in proxy middleware, with no OSCAL mapping documenting this identity verification control. |
| **Identification and Authentication (IA-2 / IA-8)** | `crates/op-mcp/src/transport/http.rs:87` | SSP / Component Definition | Duplicate implementation of WireGuard auth token extraction and verification for generic HTTP transport. |
| **Access Control (AC-3 / AC-6)** | `crates/op-mcp/src/compact.rs:56`<br>`crates/op-mcp/src/request_context.rs:119` | SSP / Component Definition | Chatbot controller checks (`can_execute_controller_tools`) are represented as hardcoded boolean checks, with no OSCAL policy describing the authorization boundaries between regular clients and controllers. |
| **Access Control / Guardrails (SC-7 / SC-18)** | `crates/op-mcp/src/tool_adapter.rs:26` | Component Definition | Boundary guardrails (blocking mutations like `shell_execute`, `write_file`, `systemd_*` and `ovs_*` modifications) are hardcoded as static lists of blacklisted patterns rather than using machine-readable schemas or policy engines. |
| **Access Control / Duplicate Guardrails (SC-7)** | `crates/op-mcp/src/tool_adapter_orchestrated.rs:18` | Component Definition | Duplicate copy of `BLOCKED_PATTERNS` list in orchestrated executor. This duplicate guardrail creates drift risks if one list is modified but the other is forgotten. |
| **Information Flow Enforcement (AC-4)** | `crates/op-mcp/src/request_context.rs:37` | SSP | Request-scoped isolation (loading/unloading tools on request boundary to prevent memory leaks and turn-limit enforcement) is implemented directly in code logic with no architectural control description. |
| **API Endpoint Cataloging (CA-2)** | `crates/op-mcp/src/router.rs:52` | Component Definition | REST endpoints (`/sse`, `/tools`, `/tools/:name`, `/initialize`) are exposed to networks but are not registered in a machine-readable OSCAL Component Definition documenting the system's external interface boundary. |

---

## 3. Detailed Findings & Recommendations

### CRITICAL: Authentication Bypass via Client-Controlled `Host` Header Spoofing

#### Finding Description
In `crates/op-mcp/src/transport/http.rs`, the HTTP/SSE transport middleware implements authentication via a WireGuard-derived bearer token. However, it attempts to bypass authentication for loopback connections to allow convenient local development and internal health checks. 

The mechanism utilized to check for loopback connections is insecure:
```rust
// crates/op-mcp/src/transport/http.rs:80
fn is_localhost_host(headers: &HeaderMap) -> bool {
    headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .map(|h| {
            let host = h.split(':').next().unwrap_or(h);
            host == "127.0.0.1" || host == "localhost" || host == "::1"
        })
        .unwrap_or(false)
}
```

At line 91, this function is used to decide whether to completely bypass authentication:
```rust
// crates/op-mcp/src/transport/http.rs:91
if request.uri().path() == "/health" || is_localhost_host(&headers) {
    return Ok(next.run(request).await);
}
```

Because the `Host` HTTP header is fully controlled by the client, a remote attacker on the network can easily bypass the authentication requirements by supplying a spoofed `Host: localhost` or `Host: 127.0.0.1` header with their request. Once bypassed, the attacker gains full access to execute high-privilege system tools (such as reading arbitrary files or running agent tasks) on the system.

#### Remediation Plan
Modify the authentication middleware to determine if a connection is local by checking the *connection source IP address* (peer address) provided by the web server socket, rather than relying on untrusted, client-supplied HTTP headers.

A secure implementation in Axum using `ConnectInfo` is shown below:

```rust
use axum::extract::ConnectInfo;
use std::net::SocketAddr;

async fn wireguard_auth_middleware(
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    let ip = addr.ip();
    let is_loopback = ip.is_loopback();

    // Allow health check and loopback without auth
    if request.uri().path() == "/health" || is_loopback {
        return Ok(next.run(request).await);
    }

    let Some(token) = extract_bearer_token(&headers) else {
        warn!("Rejected HTTP MCP request without bearer token");
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !is_wireguard_auth_token(token) {
        warn!("Rejected HTTP MCP request with non-WireGuard bearer token");
        return Err(StatusCode::UNAUTHORIZED);
    }

    debug!("Accepted HTTP MCP request with WireGuard bearer token");
    request.extensions_mut().insert(token.to_string());
    Ok(next.run(request).await)
}
```
*Note: Ensure Axum is configured with `Router::into_make_service_with_connect_info::<SocketAddr>()` when binding the transport listener.*

---

### HIGH: Schema-as-Code Drift and Untyped JSON Contracts

#### Finding Description
There is significant architectural drift in how data contracts are handled within the codebase. The gRPC transport layer implements a formal schema utilizing Protocol Buffers (compiled via `prost` and `tonic` into `crates/op-mcp/src/grpc/generated/op.mcp.v1.rs`). However, the HTTP/SSE transport layer and internal agents (`agents_main.rs`, `protocol.rs`, `router.rs`) redefine duplicates of `McpRequest` and `McpResponse` as ad-hoc Rust structs with manual Serde serialization. 

Furthermore, tool arguments, schemas, and return values are consistently mapped using untyped JSON `simd_json::OwnedValue` objects rather than strongly-typed, schema-validated contracts. This violates the core schema-as-code discipline and increases the risk of interface drift, runtime parsing failures, and security validation bypasses.

#### Remediation Plan
1. Consolidate the multiple declarations of `McpRequest`, `McpResponse`, and `JsonRpcError` across `protocol.rs`, `router.rs`, and `agents_main.rs` into a single, unified codebase module.
2. Standardize on the Protobuf types defined in `op.mcp.v1.proto` (and generated in `crates/op-mcp/src/grpc/generated/op.mcp.v1.rs`) as the single source of truth.
3. Replace the untyped JSON `simd_json::OwnedValue` fields for tool schemas with compiled, validated JSON-schema objects generated programmatically from the centralized Protobuf schema descriptors.

---

### MEDIUM: Hardcoded Security Guardrails and Code Duplication

#### Finding Description
In `crates/op-mcp/src/tool_adapter.rs` and `crates/op-mcp/src/tool_adapter_orchestrated.rs`, security guardrails designed to prevent the execution of high-privilege system-altering commands (such as shell execution, BTRFS snapshots, or systemd mutations) are enforced via hardcoded string matchers (`BLOCKED_PATTERNS` list).

```rust
// crates/op-mcp/src/tool_adapter.rs:26
const BLOCKED_PATTERNS: &[&str] = &[
    "shell_execute",
    "write_file",
    "systemd_start",
    "systemd_stop",
    // ...
];
```

This implementation suffers from two major compliance and quality gaps:
1. **Control Duplication**: The `BLOCKED_PATTERNS` array is entirely duplicated between `tool_adapter.rs` and `tool_adapter_orchestrated.rs`, inviting security regressions if a developer updates one whitelist but fails to update the other.
2. **OSCAL Non-Compliance**: This security guardrail represents an access control boundary (NIST SP 800-53 SC-7). Hardcoding this policy in code makes it impossible to dynamically audit or assess the system's posture using machine-readable compliance tooling.

#### Remediation Plan
1. Centralize the `BLOCKED_PATTERNS` logic and whitelists into a single configuration structure in a common library.
2. Externalize the tool execution blocklist to a policy profile file (e.g., an OSCAL Component Definition document or a structured JSON schema configuration) loaded dynamically at startup.
3. Validate the loaded policy file against a cryptographic signature to prevent local tampering of security guardrails.

---

### LOW: Undocumented Authorization Boundaries and Exposed Endpoints

#### Finding Description
Multiple network endpoints and REST routes are defined directly in code within `crates/op-mcp/src/router.rs` and `crates/op-mcp/src/http_server.rs` without any corresponding OSCAL control documentation. Similarly, access control checks such as `can_execute_controller_tools` in `crates/op-mcp/src/compact.rs` establish hardcoded privilege levels (regular client vs. chatbot controller) with no machine-readable compliance tracking. This makes it difficult for automated regulatory audit tools (such as FedRAMP validation suites) to trace network attack surfaces and privilege escalation vectors.

#### Remediation Plan
Produce an OSCAL Component Definition JSON/YAML artifact that officially registers the `op-mcp` service component. This document must:
1. Formally map the authentication checks in `http.rs` to the NIST SP 800-53 **IA-2 (Identification and Authentication)** control family.
2. Map the tool-blocking guardrails and `is_controller` validation checks to the **AC-3 (Access Enforcement)** control family.
3. Explicitly catalog the REST and gRPC service ports and endpoint routes in the component's interface documentation.