# LICENSE AUDIT

## 1. Extracted License Field
- **Workspace License**: `Apache-2.0` (defined in the root `Cargo.toml` under `[workspace.package]` and inherited by workspace members via `license.workspace = true`).
- **Crate License (`crates/op-jsonrpc`)**: Inherits `Apache-2.0` from the workspace package definition.

## 2. Cargo.lock License Compatibility Scan
A scan of `Cargo.lock` was performed for copyleft/restrictive licenses (such as GPL, AGPL, or SSPL) that could conflict with the permissive `Apache-2.0` license of this workspace:
- **Copyleft/GPL/AGPL/SSPL Crates**: None found.
- **Notable Licenses**:
  - `cozo` is licensed under MPL-2.0 (Mozilla Public License 2.0), which is a weak copyleft license. It is file-level copyleft and compatible with permissive Apache-2.0 licensing, provided any modifications to Cozo source files themselves are made public under MPL-2.0.
  - All other transitive and direct dependencies list permissive licenses (e.g., MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, or ISC).

## 3. Crates with Missing License Fields
- All analyzed crates inside this workspace specify `license.workspace = true` or inherit the workspace's permissive `Apache-2.0` license. No crate within the workspace has a missing license field.

---

# PRODUCTION SECURITY & QUALITY AUDIT

## 1. Summary of Findings

| Finding ID | Severity | Category | Description | File & Line Reference |
| :--- | :--- | :--- | :--- | :--- |
| **SEC-01** | **Critical** | Memory Safety | SIMD-JSON Padding Safety Invariant Violation (OOB Read / DoS) | `crates/op-jsonrpc/src/nonnet.rs:252`<br>`crates/op-jsonrpc/src/server.rs:242`<br>`crates/op-jsonrpc/src/ovsdb.rs:520`<br>`crates/op-jsonrpc/src/ovsdb_rpc_call.rs:20` |
| **SEC-02** | **High** | Availability | Listener Loop Crash (Accept Error Denial of Service) | `crates/op-jsonrpc/src/server.rs:172`<br>`crates/op-jsonrpc/src/server.rs:194`<br>`crates/op-jsonrpc/src/nonnet.rs:199` |
| **QUAL-01** | **High** | Code Quality | Uncompilable and Dangling Staging Artifacts | `crates/op-jsonrpc/src/nonnet_staging.rs:8`<br>`crates/op-jsonrpc/src/ovsdb_rpc_call.rs:1`<br>`crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:41` |
| **ARCH-01** | **Medium** | Architectural | Dynamic Ad-Hoc Data Contracts and Type Inference | `crates/op-jsonrpc/src/nonnet.rs:114`<br>`crates/op-jsonrpc/src/nonnet.rs:388`<br>`crates/op-jsonrpc/src/protocol.rs:10` |
| **PERF-01** | **Medium** | Performance | N+1 Connection Allocation Bug in OVSDB Query Loop | `crates/op-jsonrpc/src/ovsdb.rs:431` |

---

## 2. Detailed Findings

### SEC-01: SIMD-JSON Padding Safety Invariant Violation (OOB Read / DoS)
- **Severity**: **Critical** (Directly exploitable)
- **Citations**:
  - `crates/op-jsonrpc/src/nonnet.rs:252`
  - `crates/op-jsonrpc/src/server.rs:242`
  - `crates/op-jsonrpc/src/ovsdb.rs:520`
  - `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:20`
- **Description**:
  The `simd-json` parser optimizes performance by reading memory in 32-byte or 64-byte chunks using SIMD vector instructions (AVX2/SSE/NEON). Because of this, its low-level parsing APIs—including `simd_json::from_str`—require that the input slice/string buffer is padded with at least `simd_json::SIMDJSON_PADDING` extra bytes of allocated memory beyond the active slice length. This is a strict safety invariant of the `unsafe` block.
  
  In the codebase, standard `String` buffers populated directly by standard network stream utilities (such as `BufReader::read_line`) are mutated with `.as_mut_str()` and passed directly to `unsafe { simd_json::from_str }`. 
  
  Since there is no guarantee of trailing padding in these buffers, the SIMD vector reads will overrun the allocated boundary of the `String`. This is directly exploitable by sending a crafted JSON string that ends precisely at an allocation boundary, triggering a segmentation fault (Denial of Service) or potentially leaking adjacent heap memory.
- **Remediation**:
  Use `simd_json::to_owned_value` or `simd_json::serde::from_slice` which automatically manage copy-on-write allocation padding, or ensure the input buffer is resized to include `simd_json::SIMDJSON_PADDING` trailing bytes. Alternatively, replace `simd-json` with `serde_json` for network boundaries where input buffer allocation padding cannot be tightly controlled.

---

### SEC-02: Listener Loop Crash (Accept Error Denial of Service)
- **Severity**: **High**
- **Citations**:
  - `crates/op-jsonrpc/src/server.rs:172`
  - `crates/op-jsonrpc/src/server.rs:194`
  - `crates/op-jsonrpc/src/nonnet.rs:199`
- **Description**:
  The server loop in both the Unix socket and TCP socket listener implementations propagates acceptance errors immediately using the `?` operator:
  ```rust
  loop {
      let (stream, _) = listener.accept().await?;
      ...
  }
  ```
  If the application runs out of file descriptors (triggering `EMFILE` or `ENFILE` errors) or encounters transient network stack failures, the call to `accept()` will return an `Err`. Using the `?` operator on this error causes the entire listener task to terminate permanently. This leads to a persistent Denial-of-Service (DoS) state, as the server will stop accepting any further connections, even after file descriptors or resources are freed.
- **Remediation**:
  Handle errors from `accept()` gracefully inside the loop. Log the error and incorporate a retry strategy (optionally with an exponential back-off) rather than propagating the error and crashing the listener task:
  ```rust
  loop {
      match listener.accept().await {
          Ok((stream, _)) => {
              // Spawn connection handler
          }
          Err(e) => {
              tracing::error!("Accept failed: {}. Retrying...", e);
              tokio::time::sleep(std::time::Duration::from_millis(50)).await;
          }
      }
  }
  ```

---

### QUAL-01: Uncompilable and Dangling Staging Artifacts
- **Severity**: **High** (Quality / Build Blockers)
- **Citations**:
  - `crates/op-jsonrpc/src/nonnet_staging.rs:8`
  - `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:1`
  - `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:41`
- **Description**:
  Several files left inside the `src` directory of `op-jsonrpc` are structurally broken and fail to compile:
  1. `nonnet_staging.rs:8` imports a non-existent state manager: `use crate::state::StateManager;`. There is no `state` module defined in `lib.rs` or this crate, rendering this file permanently broken.
  2. `nonnet_staging.rs:40` calls `simd_json::from_str::<Value>(&line)` without `unsafe` and with an immutable borrow `&line`. This violates the API signature of `simd_json::from_str` which requires `&mut str`.
  3. `ovsdb_jsonrpc.rs:41` similarly attempts `simd_json::from_str(&response_line)` on an immutable string borrow, which is a compilation error.
  4. `ovsdb_rpc_call.rs` is not valid Rust code; it is a raw function snippet with indentation and no enclosing structure, imports, or declarations.
- **Remediation**:
  Remove these dangling files from the version control system or exclude them explicitly. If they are intended for staging, move them to a separate test/staging directory outside `src/` to prevent toolchain and static analysis failures.

---

### ARCH-01: Dynamic Ad-Hoc Data Contracts and Type Inference (Schema-as-Code Violation)
- **Severity**: **Medium** (Architectural Compliance)
- **Citations**:
  - `crates/op-jsonrpc/src/nonnet.rs:114`
  - `crates/op-jsonrpc/src/nonnet.rs:388`
  - `crates/op-jsonrpc/src/protocol.rs:10`
- **Description**:
  The system architecture mandates strict schema-as-code discipline using versioned schemas (such as Protocol Buffers or OSCAL profiles). However, the `op-jsonrpc` crate implements ad-hoc data contracts and runtime schema generation:
  1. In `nonnet.rs:114`, database tables and columns are dynamically inferred at runtime from raw JSON structures using `infer_columns` (`nonnet.rs:388`) and `infer_type`.
  2. In `protocol.rs:10`, the `JsonRpcRequest` and `JsonRpcResponse` structures use untyped, arbitrary `simd_json::OwnedValue` elements.
  
  This allows dynamic data structures to bypass schema constraints at the crate boundary. It introduces schema drift, lack of deterministic validation, and possible serialization incompatibilities between client and server.
- **Remediation**:
  Define all incoming and outgoing schemas as compiled Protocol Buffers or versioned JSON schemas. Validate raw incoming payloads against these versioned schemas rather than dynamically generating schemas based on the structural attributes of incoming data at runtime.

---

### PERF-01: N+1 Connection Allocation Bug in OVSDB Query Loop
- **Severity**: **Medium** (Performance)
- **Citations**:
  - `crates/op-jsonrpc/src/ovsdb.rs:431`
- **Description**:
  The method `list_ports` in `ovsdb.rs` retrieves port names for a bridge by executing a separate query in a loop for every single port UUID:
  ```rust
  for port_uuid in port_uuids {
      let ops = json!([{
          "op": "select",
          "table": "Port",
          ...
      }]);
      let result = self.transact("Open_vSwitch", ops).await?;
  ```
  Since `OvsdbClient::transact` invokes `rpc_call()`, and `rpc_call()` initiates a brand new Unix socket connection (`UnixStream::connect`) and shuts it down for every single invocation, this logic manifests an **N+1 connection allocation pattern**. 
  
  If a bridge contains 100 ports, querying `list_ports` will sequentially spawn and destroy 101 separate Unix connections. This results in severe latency overhead, high CPU consumption, and potential socket/file-descriptor exhaustion on high-throughput control planes.
- **Remediation**:
  - Optimize the OVSDB transaction to execute a single multi-row `select` statement that retrieves all port names matching the set of UUIDs in a single query.
  - Refactor `OvsdbClient` to maintain a persistent, re-usable connection pool or a keep-alive connection rather than reconnecting on every single transaction.