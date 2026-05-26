# Production Quality & Security Audit: `op-jsonrpc`

This document details the quality, documentation, structural compliance, and security findings identified in the `op-jsonrpc` crate.

---

## 1. Docs and API Quality Assessment

### Crate-Level Documentation
The crate contains well-structured crate-level documentation `//!` at the root of `crates/op-jsonrpc/src/lib.rs:1-5`, which clearly outlines the responsibilities of the crate (JSON-RPC 2.0 server over Unix sockets, OVSDB client, and NonNet database).

### README.md Presence
No `README.md` file was provided in the source file list for the `op-jsonrpc` crate. A dedicated `README.md` must be added to the crate root to document build dependencies, system pre-requisites (such as Open vSwitch), and socket permissions.

### Public Unsafe Functions
No public `unsafe fn` items are declared in the codebase. The codebase only utilizes `unsafe` blocks internally within safe private/public helper functions. Thus, no public invariants documentation (`/// # Safety`) was omitted.

### Public API Documentation Sample (10 Items)

The following table summarizes the documentation status of 10 sampled public items across the crate:

| # | Item Name | File & Line | Status | Notes |
|---|---|---|---|---|
| 1 | `pub mod nonnet;` | `crates/op-jsonrpc/src/lib.rs:7` | **Lacks Rustdoc** | Missing `///` module re-export documentation. |
| 2 | `pub mod ovsdb;` | `crates/op-jsonrpc/src/lib.rs:8` | **Lacks Rustdoc** | Missing `///` module re-export documentation. |
| 3 | `pub mod protocol;` | `crates/op-jsonrpc/src/lib.rs:9` | **Lacks Rustdoc** | Missing `///` module re-export documentation. |
| 4 | `pub mod server;` | `crates/op-jsonrpc/src/lib.rs:10` | **Lacks Rustdoc** | Missing `///` module re-export documentation. |
| 5 | `pub use nonnet::NonNetDb;` | `crates/op-jsonrpc/src/lib.rs:12` | **Lacks Rustdoc** | Missing documentation on re-export. |
| 6 | `pub use ovsdb::OvsdbClient;` | `crates/op-jsonrpc/src/lib.rs:13` | **Lacks Rustdoc** | Missing documentation on re-export. |
| 7 | `pub use server::JsonRpcServer;` | `crates/op-jsonrpc/src/lib.rs:14` | **Lacks Rustdoc** | Missing documentation on re-export. |
| 8 | `pub async fn run_unix_jsonrpc` | `crates/op-jsonrpc/src/nonnet_staging.rs:13` | **Lacks Rustdoc** | Missing structural documentation for staging entry point. |
| 9 | `pub struct NonNetUpdate` | `crates/op-jsonrpc/src/nonnet.rs:19` | **Has Rustdoc** | Documented: "NonNet update event". Field level docs are missing. |
| 10 | `pub struct JsonRpcRequest` | `crates/op-jsonrpc/src/protocol.rs:10` | **Has Rustdoc** | Documented: "JSON-RPC 2.0 request". |

---

## 2. Schema-as-Code Violations

The system design relies heavily on ad-hoc, untyped JSON schemas and raw string parsing rather than structured, versioned schemas like Protocol Buffers or OSCAL.

### Key Structural Violations

*   **Runtime Column Type Inference instead of Versioned Schema Definition**
    In `crates/op-jsonrpc/src/nonnet.rs:95` (within `load_from_plugins`), the NonNet database infers database table columns dynamically at runtime using `infer_columns` (`crates/op-jsonrpc/src/nonnet.rs:356`):
    ```rust
    let columns = infer_columns(value);
    schema_tables.insert(name.clone(), json!({"columns": columns}));
    ```
    This completely bypasses schema-as-code principles. Changes in the structure of the plugin values will silently alter the generated schema, breaking downstream clients.

*   **Untyped Dynamic JSON Fields**
    In `crates/op-jsonrpc/src/protocol.rs:13` and `crates/op-jsonrpc/src/protocol.rs:36`, the parameters (`params`) and results (`result`) fields of the core JSON-RPC protocol types are typed as `simd_json::OwnedValue` (essentially raw `serde_json::Value`). These represent completely untyped, unstructured data contracts. These must be replaced with versioned, strongly typed protobuf messages.

*   **Inlined OVSDB Transaction Structures**
    In `crates/op-jsonrpc/src/ovsdb.rs:188` (inside `create_bridge`) and `crates/op-jsonrpc/src/ovsdb.rs:253` (inside `add_port`), the database transaction commands are built dynamically as ad-hoc arrays of raw JSON objects using the `json!` macro:
    ```rust
    let operations = json!([
        {
            "op": "insert",
            "table": "Bridge",
            ...
        }
    ]);
    ```
    Any modifications to the Open vSwitch schema will cause runtime transaction failures instead of compile-time schema validation errors.

---

## 3. Security & Reliability Audit Findings

### Finding 1: Undefined Behavior & Heap OOB Reads via Unpadded `simd_json::from_str` (CRITICAL)

#### Description
The `simd-json` crate is a highly optimized JSON parser that utilizes SIMD instructions (such as AVX2 or SSE) to parse JSON chunks in parallel. Because of this, its `from_str` API carries a strict safety contract: **the input string buffer must be padded with `simd_json::SIMDJSON_PADDING` bytes** (usually 32 or 64 bytes) of nulls or whitespace past its actual logical length. 

In `crates/op-jsonrpc/src/nonnet.rs:277`, `crates/op-jsonrpc/src/server.rs:301`, and `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:21`, the codebase calls `unsafe { simd_json::from_str(...) }` on string slices that are obtained directly from standard I/O reader lines (`reader.read_line(&mut line)`) or standard `String` vectors (`String::from_utf8(response_buf)`). These strings are **not** padded with `SIMDJSON_PADDING` bytes.

#### Exploitation Vector
When an attacker sends a JSON-RPC message over the Unix domain socket or TCP socket, the line is read into `line`. When `simd_json::from_str` is called on `line.as_mut_str()`, the AVX2/SSE parser performs vector reads past the logical end of the string's allocation. This triggers undefined behavior, resulting in process segmentation faults (Denial of Service) or potential Out-of-Bounds memory exposure.

#### Proof of Concept Code Citations
1.  **`crates/op-jsonrpc/src/nonnet.rs:271-277`**:
    ```rust
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let response = match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) } {
    ```
2.  **`crates/op-jsonrpc/src/server.rs:291-301`**:
    ```rust
    while reader.read_line(&mut line).await? > 0 {
        let response = self.process_line(&mut line).await;
    ...
    async fn process_line(&self, line: &mut String) -> JsonRpcResponse {
        match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) } {
    ```
3.  **`crates/op-jsonrpc/src/ovsdb_rpc_call.rs:14-21`**:
    ```rust
    let mut response_buf = Vec::new();
    tokio::time::timeout(self.timeout, tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut response_buf))
        ...
    let mut response_str = String::from_utf8(response_buf)?;
    debug!("OVSDB response: {}", response_str.trim());

    let response: Value = unsafe { simd_json::from_str(&mut response_str)? };
    ```

#### Remediation
Either:
1.  Use `simd_json::to_padded_bin` or reserve additional capacity up to `simd_json::SIMDJSON_PADDING` on the mutated string before passing it to `from_str`.
2.  Switch to the safe `simd_json::from_slice` interface on a vector that has been padded with `simd_json::SIMDJSON_PADDING` empty bytes.
3.  Use `serde_json::from_str` instead of `simd-json` if padding cannot be structurally guaranteed.

---

### Finding 2: Unauthenticated Exposed OVSDB TCP Proxy (HIGH)

#### Description
The JSON-RPC server is capable of binding to a TCP port and exposing an OVSDB proxy when `ovsdb_enabled` is set to `true` (which is enabled by default in `JsonRpcServerConfig`).

In `crates/op-jsonrpc/src/server.rs:180-192`, `run_tcp` binds directly to the configured TCP address and accepts incoming connections:
```rust
loop {
    let (stream, _) = listener.accept().await?;
    let server = self.clone_for_connection();

    tokio::spawn(async move {
        if let Err(e) = server.handle_tcp_connection(stream).await {
            debug!("Connection error: {}", e);
        }
    });
}
```
No form of authentication (such as mTLS, tokens, or SASL) or authorization checks are performed on accepted TCP connections.

#### Vulnerability Impact
If the server's TCP socket is exposed to a network segment or configured with a wildcard address (`0.0.0.0`), any network attacker can connect to the port and send `ovsdb.transact` JSON-RPC calls. This grants the attacker full write access to the host's Open vSwitch state, permitting them to delete bridges, inject rogue interfaces, and modify routing parameters.

#### Code Citations
*   **`crates/op-jsonrpc/src/server.rs:43`** (OVSDB TCP proxy is enabled by default):
    ```rust
    impl Default for JsonRpcServerConfig {
        fn default() -> Self {
            Self {
                unix_socket: Some("/var/run/op-dbus/jsonrpc.sock".to_string()),
                tcp_addr: None,
                ovsdb_enabled: true,
                nonnet_enabled: true,
            }
        }
    }
    ```
*   **`crates/op-jsonrpc/src/server.rs:335-337`** (Handling OVSDB proxy requests):
    ```rust
    "ovsdb.list_dbs" | "ovsdb.get_schema" | "ovsdb.transact"
        if self.config.ovsdb_enabled =>
    ```

#### Remediation
1.  Enforce mutual TLS (mTLS) on the TCP socket using `rustls` to restrict connectivity to authorized control plane services.
2.  Add token-based authorization to JSON-RPC headers or parameters for any remote client connections.