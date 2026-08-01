# Production Security and Quality Audit: op-grpc-bridge

## 1. Executive Summary

This codebase is a bidirectional D-Bus $\leftrightarrow$ gRPC bridge serving as the deterministic control plane for system operations. An audit of the provided files reveals critical security and memory-safety vulnerabilities in the gRPC interceptor, severe Denial of Service vectors in server-streaming endpoints, complete bypass of configured TLS settings in client connection pooling, and structural schema-as-code deviations.

---

## 2. Dependencies & Feature Inventory

### Direct Dependencies (from `crates/op-grpc-bridge/Cargo.toml`)

| Crate | Specified Version | Workspace / Local Path | Explicitly Enabled Features | Pulls in Default | Warnings / Quality Flags |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `op-core` | Workspace | `crates/op-core` | None | Yes | Internal Dependency |
| `tonic` | Workspace | Workspace | None | Yes | Uses workspace-configured `tls`, `tls-roots` |
| `tonic-web` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `prost` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `prost-types`| Workspace | Workspace | None | Yes | Workspace Dependency |
| `tonic-reflection` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `tonic-health` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `tokio` | Workspace | Workspace | `["full", "sync"]` | Yes | Explicit features duplicate workspace features |
| `tokio-stream` | `0.1` | Registry | `["sync"]` | Yes | Unpinned minor version |
| `zbus` | Workspace | Workspace | None | Yes | Version mismatch risk (Workspace: 5.12, lockfile has 4.4.0 and 5.13.2) |
| `serde` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `serde_json` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `simd-json` | Workspace | Workspace | None | Yes | Uses workspace-configured `serde`, `serde_impl` |
| `op-state-store` | Path | `../op-state-store` | None | Yes | Internal Dependency |
| `op-identity` | Path | `../op-identity` | None | Yes | Internal Dependency |
| `op-network` | Path | `../op-network` | None | Yes | Internal Dependency |
| `op-jsonrpc` | Path | `../op-jsonrpc` | None | Yes | Internal Dependency |
| `op-cognitive-mcp` | Path | `../op-cognitive-mcp` | None | Yes | Internal Dependency |
| `op-cache` | Path | `../op-cache` | None | Yes | Internal Dependency |
| `memmap2` | Workspace | Workspace | None | Yes | Critical: Zero-copy pointer manipulation |
| `hex` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `sha2` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `tracing` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `anyhow` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `thiserror` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `async-trait` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `uuid` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `chrono` | Workspace | Workspace | None | Yes | Workspace Dependency |
| `futures` | `0.3` | Registry | None | Yes | Unpinned minor version |
| `async-stream` | `0.3` | Registry | None | Yes | Unpinned minor version |
| `base64` | `0.21` | Registry | None | Yes | Unpinned minor version |
| `qdrant-client` | `1.17` | Registry | None | Yes | Mismatched with workspace `qdrant-client = "1.7"` |

### Crate Features (`crates/op-grpc-bridge/Cargo.toml`)
*   **None defined**: The bridge crate does not export any conditional compilation gates.

### Build Dependencies
*   `tonic-build` (Workspace-defined)

---

## 3. Storage Backend Inventory

The following storage backends and local databases are accessed directly or managed transitively by the audited bridge:

| Backend | Found at File:Line | Type / Role | Architecture Violation? |
| :--- | :--- | :--- | :--- |
| `/dev/shm/plugin_schema.dat` | `crates/op-grpc-bridge/src/interceptor.rs:49` | Shared memory mmap file for zero-copy state lookup | **Yes**. Bypasses unified state abstraction; directly accesses host tempfs without lock coordination. |
| `NonNetDb` | `crates/op-grpc-bridge/src/schema_engine.rs:81` | Relational/JSON-RPC database for non-network state | No. Serves as local configuration store. |
| `OvsdbClient` | `crates/op-grpc-bridge/src/schema_engine.rs:80` | Local network state database (RFC 7047) | No. Coordinates with OVS system database. |

---

## 4. Critical Security Findings

### CRITICAL: Shared Memory Data Races & Undefined Behavior in Interceptor
*   **Location**: `crates/op-grpc-bridge/src/interceptor.rs:53-62`
*   **Impact**: Memory corruption, information disclosure, or daemon crash (Segmentation Fault).
*   **Description**: 
    The gRPC gatekeeper interceptor performs a raw pointer cast on a memory-mapped file `/dev/shm/plugin_schema.dat` to access `IdentitySled`:
    ```rust
    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|_| Status::internal("Mmap failed"))?
    };
    let sled_ptr = mmap.as_ptr() as *const IdentitySled;

    let is_valid = unsafe { (*sled_ptr).is_valid };
    let current_footprint = unsafe { (*sled_ptr).hashed_footprint };
    ```
    This implementation contains multiple critical defects:
    1.  **Alignment Violation**: `mmap.as_ptr()` returns an unaligned pointer (`*const u8`). Casting this directly to `*const IdentitySled` and dereferencing it violates Rust's alignment guarantees for `u64` fields within `IdentitySled`, triggering undefined behavior (or CPU traps on non-x86 architectures).
    2.  **Size Validation Failure**: The code fails to verify that the file size matches or exceeds `std::mem::size_of::<IdentitySled>()`. If the file is truncated, dereferencing `sled_ptr` triggers an out-of-bounds memory read (SIGSEGV).
    3.  **Data Races**: The shared memory file is accessed concurrently by the Schema Engine and the interceptor without atomic operations or synchronization barriers. A concurrent write during interceptor evaluation will read torn or invalid memory. Reading an invalid bit pattern for `is_valid` (which is a Rust `bool` requiring `0` or `1`) is instantaneous undefined behavior.
*   **Remediation**: Use the `bytemuck` crate for safe casting with size validation, and implement file locking or use atomic primitives (`std::sync::atomic`) within the shared memory layout to prevent data races.

---

### CRITICAL: Exploitable Access Control Bypass via Shared Memory Injection
*   **Location**: `crates/op-grpc-bridge/src/interceptor.rs:49`
*   **Impact**: Total authentication bypass of the gRPC ingress control plane.
*   **Description**: 
    The interceptor validates inbound request footprints against `/dev/shm/plugin_schema.dat`. Because `/dev/shm` is a world-writable directory on standard Linux installations, any unprivileged local user, container, or compromised process with access to the host namespace can modify, overwrite, or recreate `plugin_schema.dat`.
    By overwriting this file, an attacker can mark arbitrary invalid sessions as valid (`is_valid = true`) or replace `hashed_footprint` with their own cryptographic footprint, fully bypassing the A.N.N.A. Scribe gatekeeper.
*   **Remediation**: Move the backing file from the world-writable `/dev/shm/` namespace to a restricted runtime directory owned exclusively by the root/daemon user (e.g., `/run/op-dbus/plugin_schema.dat`) with `0600` permissions.

---

### HIGH: Unbounded Audit Event Retrieval (Denial of Service)
*   **Location**: `crates/op-grpc-bridge/src/grpc_server.rs:620-625`
*   **Impact**: Out-Of-Memory (OOM) crash of the control plane daemon.
*   **Description**: 
    In `EventChainService::get_events`, the server retrieves audit entries from the memory-backed `event_chain`. If the client sets `limit = 0` (or fails to provide a value, resolving to `0`), the server collects all historical events:
    ```rust
    .take(if req.limit == 0 {
        usize::MAX
    } else {
        req.limit as usize
    })
    .map(proto_chain_event)
    .collect();
    ```
    On a production node running for a long duration, the audit ledger can contain millions of events. Attempting to allocate and serialize the entire chain into memory without pagination limits will exhaust server memory and trigger an immediate OOM crash.
*   **Remediation**: Enforce a strict maximum cap on the limit field (e.g., `std::cmp::min(req.limit, 1000)`) if `limit` is `0` or exceeds the threshold.

---

### HIGH: Client Transport Layer Security (TLS) Configuration Bypass
*   **Location**: `crates/op-grpc-bridge/src/grpc_client.rs:65-98`
*   **Impact**: Cleartext transmission of control plane mutations over the network.
*   **Description**: 
    The `RemoteEndpoint` configuration defines `tls_enabled: bool` at line 28, but `GrpcClientPool::get_channel` completely ignores this setting when instantiating single-endpoint connections:
    ```rust
    let endpoint = Endpoint::from_shared(address.to_string())
        .map_err(|e| GrpcClientError::ConnectionFailed(e.to_string()))?
        .connect_timeout(self.default_config.connect_timeout)
        .timeout(self.default_config.request_timeout);

    endpoint
        .connect()
        .await
        ...
    ```
    Regardless of whether `tls_enabled` is set to `true`, the client pool attempts to connect using standard cleartext HTTP/2 unless the address string explicitly prepends `https://`. If a user configures a target address as `10.0.0.5:50051` with `tls_enabled: true`, the connection remains unencrypted, leaking all operational payloads.
*   **Remediation**: Check `self.default_config.tls_enabled` in `get_channel`. If enabled, invoke `.tls_config(...)` on the `Endpoint` instance before calling `.connect()`.

---

## 5. Schema-As-Code Gaps

The codebase implements a unified schema engine but exhibits multiple security-relevant violations of the "schema-as-code" discipline:

### Violation 1: Ad-hoc Environment Variable Extractions for OSCAL Validation
*   **Location**: `crates/op-grpc-bridge/src/schema_engine.rs:565-572`
*   **Description**: The bridge injects compliance information into the `op-identity` Sled at runtime by pulling metadata directly from unstructured system environment variables:
    ```rust
    let uuid          = std::env::var("SCHEMA_UUID").unwrap_or_default();
    let subid         = std::env::var("SCHEMA_SUBID").unwrap_or_default();
    let ctrl          = std::env::var("SCHEMA_CONTROL_SOURCE")
                            .unwrap_or_else(|_| "NIST_SP_800_53_R5".into());
    ```
    Rather than consuming versioned, compile-time validated OSCAL control structures, the component relies on ad-hoc configurations. This introduces risks of mismatched control definitions and silent runtime failures if variable values are corrupted.
*   **Remediation**: Define an explicit Protocol Buffer message inside the `operation.v1` package to represent compliance metadata (including UUID, Control Source, and References) to enforce contract structure.

### Violation 2: Dynamic Protocol Buffer Synthesis at Runtime
*   **Location**: `crates/op-grpc-bridge/src/proto_gen.rs:48-111`
*   **Description**: The dynamic conversion of local catalog objects into raw Protocol Buffer string structures (using `writeln!(output, ...)` string interpolation) bypasses the Rust compiler's type and schema checks. 
*   **Remediation**: Leverage a static, version-controlled schema library or utilize code generation steps via `prost-build` within `build.rs` to validate all message fields.

### Violation 3: Raw JSON / Struct Mapping for Remote Mutations
*   **Location**: `crates/op-grpc-bridge/src/grpc_server.rs:1005-1033`
*   **Description**: High-level RPC services such as the `MailService` bypass protobuf structures, serializing arguments to unstructured JSON objects (`simd_json::json!({...})`) before executing D-Bus operations. This violates the contract-first architecture of gRPC networks.
*   **Remediation**: Use typed Protobuf structs instead of generic JSON payloads for all inner transport boundaries.

---

## 6. Code Quality & Workspace Integration Gaps

### Issue 1: Severe Workspace Crate Version Mismatch (zbus)
*   **Location**: `Cargo.toml` and `crates/op-grpc-bridge/Cargo.toml`
*   **Description**: The root `Cargo.toml` specifies a workspace-wide dependency on `zbus = { version = "5.12", features = ["tokio"] }`. However, internal modules such as `op-identity` declare dependency on `zbus 5.13.2`, while `op-core`, `op-agents`, and `op-chat` depend on `zbus 4.4.0`.
    This version pollution leads to multiple versions of the zbus library compiling into the target binary. It creates severe risk of type-mismatch panic states when casting variant objects (e.g., `OwnedValue` from zbus v4 vs zbus v5) across internal API boundaries.
*   **Remediation**: Force all workspace members to use `{ workspace = true }` for the `zbus` crate dependency and eliminate the explicit version overrides.

### Issue 2: Command Argument Vulnerability in dinitctl Calls
*   **Location**: `crates/op-grpc-bridge/src/grpc_server.rs:1141-1146`
*   **Description**: 
    The `get_service` RPC takes user-supplied `service_name` strings and passes them directly to the `dinitctl status` command execution loop:
    ```rust
    let name = &request.get_ref().service_name;
    let output = tokio::process::Command::new("dinitctl")
        .args(["status", name])
        .output()
    ```
    While safe from typical shell injection (since shell parsing is bypassed), passing unvalidated service names allows an attacker to inject arguments (such as parameters beginning with `-`) directly to `dinitctl`, modifying runtime behavior or causing information leaks depending on the parameters parsed by the binary.
*   **Remediation**: Validate `service_name` against a strict regex (e.g., `^[a-zA-Z0-9_-]+$`) before spawning subprocesses. Ensure dinitctl is called via its absolute system path (e.g., `/sbin/dinitctl`) to prevent PATH-hijacking exploits.