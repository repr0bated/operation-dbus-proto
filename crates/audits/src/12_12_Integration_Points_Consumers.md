# Workspace Integration & Security Audit Report

This report presents an integration and quality audit of the workspace configurations defined in the provided `Cargo.toml` and `Cargo.lock` files. 

---

## 1. Crates Exposing or Depending on `src` (`op-dbus`)

The root package corresponding to the `src` directory is named **`op-dbus`** (`Cargo.toml:42`). Based on the workspace configurations and the generated dependency lockfile:

*   **Crates in the Workspace depending on `op-dbus`:**
    *   No internal workspace member depends on `op-dbus`. 
    *   `op-dbus` serves as the **root orchestration binary and umbrella library** for the entire workspace. It depends on and aggregates the functionality of 19 internal workspace crates (`Cargo.toml:53-83`):
        *   `op-cache`
        *   `op-core`
        *   `op-tools`
        *   `op-network`
        *   `op-introspection`
        *   `op-dbus-model`
        *   `op-execution-tracker`
        *   `op-jsonrpc`
        *   `op-state`
        *   `op-state-store`
        *   `op-plugins`
        *   `op-workflows`
        *   `op-blockchain`
        *   `op-inspector`
        *   `op-web`
        *   `op-grpc-bridge`
        *   `op-dbus-mirror`
        *   `op-mcp`
        *   `op-cognitive-mcp` (declared locally via path in `Cargo.toml:83`)

---

## 2. Registered D-Bus Service Names and Object Paths

Because this audit is strictly limited to the provided configuration files (`Cargo.toml` and `Cargo.lock`), the concrete string literals for D-Bus service names (e.g., `org.freedesktop...`) and object paths (e.g., `/org/freedesktop/...`) defined in the Rust source code are not directly visible.

However, the configuration reveals how D-Bus capabilities are integrated and structured across the workspace:
*   **D-Bus Framework:** The workspace standardizes on `zbus` (`Cargo.toml:104-105`) and `zbus_xml` (`Cargo.toml:106`) for D-Bus connection management, code-generation, and introspection.
*   **Crates Registering/Accessing D-Bus Interfaces:** Based on their dependency declarations in `Cargo.lock`, the following sub-crates integrate with D-Bus:
    *   `op-core`: Acts as the common abstraction layers for D-Bus communication.
    *   `op-dbus-mirror`: Directly depends on `zbus_xml` to inspect, parse, and mirror active D-Bus interfaces.
    *   `op-introspection`: Queries dynamic D-Bus interface structures.
    *   `op-identity`: Connects to system keyrings and credentials over D-Bus interfaces.
    *   `op-agents`, `op-chat`, `op-cognitive-mcp`, `op-mcp`, `op-plugins`, `op-projection`, `op-services`, `op-state`, `op-state-store`, `op-tools`, and `op-web`: All link against the `zbus` runtime to expose or consume system services.

---

## 3. HTTP and gRPC Endpoints Exposed

While exact route paths (e.g., `/api/v1/...`) are declared within `.rs` files outside the scope of this audit, the network footprint of each sub-crate can be mapped through their server framework dependencies:

### HTTP Endpoints (Axum & Hyper Stack)
The following crates import the HTTP server engine (`axum`, `tower`, and `tower-http`) to expose REST, WebSocket, or Server-Sent Events (SSE) endpoints:
*   **`op-web`**: Integrates Axum, `rust-embed` (for embedded static files), and `tower_governor` for client rate-limiting.
*   **`op-http`**: Exposes utility HTTP transport services.
*   **`op-mcp` & `op-cognitive-mcp`**: Expose Model Context Protocol endpoints over HTTP/JSON-RPC.
*   **`op-mcp-proxy` & `op-projection`**: Run local Axum routing instances to proxy or project state updates.

### gRPC Endpoints (Tonic & Prost Stack)
The following crates compile Protocol Buffers and run Tonic gRPC servers to handle high-performance control-plane messages:
*   **`op-grpc-bridge`**: Exposes Tonic-based endpoints, with `tonic-web` enabled to allow browser-based gRPC-web clients.
*   **`op-services`**: Generates and hosts gRPC service interfaces for system-level execution control.
*   **`op-cognitive-mcp`**: Exposes gRPC endpoints paired with `tonic-health` and `tonic-reflection` for automated service discovery and health monitoring.

---

## 4. Cross-Crate Dependency and Deep-Coupling Risks

Cargo's lockfile compilation verifies that no cyclic dependencies exist that would block compilation. However, the workspace architecture presents significant **deep-coupling and fragility risks**:

```
[op-web] ──> [op-chat] ──> [op-grpc-bridge] ──> [op-cognitive-mcp] ──> [op-mcp] ──> [op-plugins] ──> [op-state] ──> [op-network] ──> [op-core]
```

### Risks Identified:
1.  **Massive Fan-Out / Convergence Hubs:** `op-web` (`Cargo.lock`) and `op-dbus` (`Cargo.toml:53-83`) depend directly on almost the entire workspace. Any modification to low-level crates (such as `op-core` or `op-network`) triggers a cascading recompilation of almost 20 crates, severely impacting compilation performance and integration testing pipelines.
2.  **Long Linear Coupling Paths:** The path from `op-web` down to `op-core` spans more than 8 crate boundaries. This deep architectural hierarchy increases the risk of interface rigidity—making it difficult to modify low-level types without propagating breaking changes through multiple intermediate layers.

---

## 5. Schema-as-Code Compliance Review

The workspace enforces a hybrid approach, leading to several **Schema-as-Code violations** where data contracts are expressed using ad-hoc serialization format structures rather than versioned, centralized schemas.

### Violating Crates (Ad-hoc Structs/Strings via Serde)
The following crates exchange structured system state but do not compile versioned Protocol Buffers or schemas, relying instead on ad-hoc YAML, TOML, XML, or JSON formats:
*   **`op-agents`**: Exposes agent interfaces over ad-hoc serialization formats using `simd-json`, `serde_yaml`, and `toml`.
*   **`op-compliance`**: Validates rules dynamically using `jsonschema` over unstructured JSON objects instead of statically compiled, versioned schema files.
*   **`op-dbus-model` & `op-dbus-mirror`**: Translate database states and dynamic D-Bus payloads into ad-hoc JSON formats via `serde_json` and `simd-json`.
*   **`op-inspector` & `op-introspection`**: Parse system properties and XML descriptors using `quick-xml` and `serde_yaml` in an ad-hoc manner.
*   **`op-jsonrpc`**: Manages JSON-RPC payloads directly on top of raw `simd-json` mappings.
*   **`op-state-store`**: Validates storage payloads using runtime `jsonschema` checks rather than compiled versioned contracts.

### Compliant Crates (Versioned Protocol Buffers)
The following crates natively implement versioned Protobuf contracts using `prost` and `prost-types` paired with compile-time code generation via `tonic-build`:
*   `op-cache`
*   `op-cognitive-mcp`
*   `op-grpc-bridge`
*   `op-mcp`
*   `op-projection`
*   `op-services`

---

## 6. Workspace Integrity & Dependency Split Analysis

The `Cargo.lock` file reveals severe **dependency split anomalies** that pose critical security, performance, and stability risks to the workspace integration.

### FINDING 1: Multi-Version `zbus` Duplication (High Risk)
*   **Citations:** `Cargo.lock`
*   **Impact:** 
    *   The workspace compiles and links **`zbus 4.4.0`** (used by `op-core`, `op-dbus-mirror`, `op-services`, etc.) AND **`zbus 5.13.2`** (used exclusively by `op-identity`).
    *   `zbus` underwent major API rewrites between version 4 and version 5. This split means that `op-identity` cannot share active D-Bus connections, proxy configurations, or serialization types with `op-core` or any other workspace crate at compile time. Passing D-Bus handle types across these boundaries will result in compiler type-mismatches.

### FINDING 2: Duplicate HTTP and TLS Network Stacks (High Risk)
*   **Citations:** `Cargo.lock`
*   **Impact:**
    *   The workspace compiles two entirely separate HTTP client and TLS stacks due to a split in `reqwest` versions:
        *   **`reqwest 0.11.27`** (used by `op-web`, `op-tools`, `op-plugins`, etc.) maps to `hyper 0.14.32`, `tokio-rustls 0.24.1`, and `rustls 0.21.12`.
        *   **`reqwest 0.12.28`** (used by `hf-hub` and `op-mcp-proxy`) maps to `hyper 1.8.1`, `tokio-rustls 0.26.4`, and `rustls 0.23.36`.
    *   This dual linkage causes **severe binary bloat** and introduces substantial risk. Different parts of the control plane will initialize and execute completely separate cryptography engines (`rustls 0.21` vs `rustls 0.23`). Connection pools, root trust anchors, and TLS session states cannot be shared across these crates, resulting in degraded performance and higher memory usage.

### FINDING 3: Protobuf Generator & Runtime Discrepancy (Medium Risk)
*   **Citations:** `Cargo.lock`
*   **Impact:**
    *   `op-chat` compiles using **`prost-build 0.12.6`** and **`tonic-build 0.11.0`**, but runs on top of the **`prost 0.13.5`** runtime.
    *   The remaining Protobuf-dependent crates utilize `prost-build 0.13.5` and `tonic-build 0.12.3`. This discrepancy in code-generation tools can result in silent output divergence or build failures when structures generated by older tools are loaded by newer runtimes.