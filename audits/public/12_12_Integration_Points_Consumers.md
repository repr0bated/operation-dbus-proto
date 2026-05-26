# Workspace Integration & Dependency Audit Report

## 1. Crates Depending on the Public Package (`op-dbus`)
Based on the workspace configuration in `Cargo.toml` and the dependency resolution graph in `Cargo.lock`, the public package **`op-dbus`** (defined in `Cargo.toml:103-109`) acts as the top-level orchestrator and system daemon.

* **Workspace Crates depending on `op-dbus`**: None. 
* **Role in Workspace**: `op-dbus` is the final consumer. It aggregates and links almost all workspace sub-crates as dependencies (`Cargo.toml:122-159`), serving as the unified entry point for the Linux control plane.

---

## 2. Registered D-Bus Service Names & Object Paths
Because the actual Rust source implementation files (`.rs`) are not provided in the audited FILES section, exact string registrations of D-Bus well-known names (e.g., `org.op.control...`) and object paths (e.g., `/org/op/control/...`) are not directly visible in the code. 

However, D-Bus integration points are clearly established via manifest configurations:
* **Library Runtime**: The workspace standardizes on `zbus` (`Cargo.toml:47`) and `zbus_xml` (`Cargo.toml:48`).
* **Active D-Bus Crates**: The following sub-crates reference D-Bus message bus abstractions:
  * `op-dbus-mirror`: Performs D-Bus introspection and XML-based mirroring (`zbus_xml` dependency).
  * `op-introspection`: Directly queries and exposes system interfaces.
  * `op-identity`: Implements D-Bus credential caching and keyring/secret integrations (`zbus 5.13.2` dependency).

---

## 3. Exposed HTTP & gRPC Endpoints
The control plane exposes external network boundaries through a mix of HTTP/WebSockets and gRPC endpoints:

### HTTP/WebSocket Interfaces
* **Crate `op-web`**: Exposes user-facing dashboard interfaces, WebSockets, and static assets utilizing `axum` (`Cargo.toml:51`) and `tower-http` (`Cargo.toml:53`).
* **Crate `op-http`**: Provides the shared HTTP service chassis and middleware stack.
* **Crate `op-mcp` / `op-cognitive-mcp`**: Exposes Model Context Protocol (MCP) server endpoints via JSON-RPC over HTTP/SSE.

### gRPC Interfaces
* **Crate `op-grpc-bridge`**: Translates incoming external gRPC commands into internal loopback IPC/D-Bus signals. It compiles Protobuf definitions using `tonic-build` and `prost-build` workspace rules.
* **Crate `op-services`**: Spawns gRPC-based microservices for sandboxed execution and process supervision.

---

## 4. Integration Risks & Architectural Vulnerabilities

### Finding 1: Major Version Fragmentation of the D-Bus Runtime (`zbus` v4 vs v5)
* **Severity**: High
* **Citations**: 
  * `Cargo.toml:47` (Workspace `zbus` dependency: `"5.12"`)
  * `Cargo.lock` (Under `[[package]] name = "op-identity"` showing dependency on `zbus 5.13.2`)
  * `Cargo.lock` (Under `[[package]] name = "op-chat"`, `op-grpc-bridge`, etc., showing dependencies on `zbus 4.4.0`)
* **Description**: The workspace is experiencing dependency duplication. While `op-identity` uses the modern `zbus` v5.x series, other core crates (like `op-chat`, `op-grpc-bridge`, and `op-mcp`) are pinned to the older `zbus` v4.4.0 release.
* **Impact**: Linking two different major versions of `zbus` into the single compiled binary `op-dbus` results in code bloat, double allocation of D-Bus connection resources, and potential runtime crashes if D-Bus message types or connection handles are passed across crate boundaries.

### Finding 2: Dynamic Runtime Circular Loop Risk (gRPC <-> D-Bus Translation)
* **Severity**: Medium
* **Citations**: 
  * `Cargo.toml:138-158` (Dependency definitions of `op-dbus`)
  * `Cargo.lock` (Dependency chains of `op-grpc-bridge` and `op-dbus-mirror`)
* **Description**: `op-grpc-bridge` translates gRPC commands to D-Bus signals, whereas `op-dbus-mirror` monitors D-Bus signals and exposes them back to the gRPC/JSON-RPC layer. 
* **Impact**: Since compile-time cyclic analysis cannot detect runtime loop paths across distinct IPC buses, a D-Bus signal mirrored by `op-dbus-mirror` could trigger an automated task that sends a request to `op-grpc-bridge`, causing a cascading, infinite loop of IPC messages that could exhaust system file descriptors and CPU cycles.

---

## 5. Schema-as-Code Compliance Audit

The workspace enforces a rigorous approach to schema-driven contracts in some sub-systems but permits ad-hoc JSON-schema translation in others:

| Crate | Contract Expression | Discipline | Citation |
| :--- | :--- | :--- | :--- |
| **`op-cache`** | Protobuf (`prost`) | **Schema-as-Code** (Versioned) | `Cargo.lock` (`op-cache` depends on `prost`) |
| **`op-grpc-bridge`**| Protobuf (`prost`/`tonic`) | **Schema-as-Code** (Versioned) | `Cargo.lock` (`op-grpc-bridge` depends on `prost`) |
| **`op-compliance`** | Ad-hoc JSON-Schema | *Dynamic Validation* (No local schema compiler) | `Cargo.toml:44` (`jsonschema` workspace dependency) |
| **`op-state-store`**| Ad-hoc JSON-Schema | *Dynamic Validation* (No local schema compiler) | `Cargo.toml:136` (`jsonschema` workspace dependency) |

### Flagged Schema-as-Code Non-Compliance
* **Ad-hoc JSON Contracts**: Crates like `op-compliance` and `op-state-store` validate data payloads dynamically at runtime using `jsonschema` instead of compile-time versioned serialization schemas (such as Protobuf / gRPC definitions). This approach introduces serialization divergence and validation performance overhead compared to the unified schemas used in `op-grpc-bridge`.