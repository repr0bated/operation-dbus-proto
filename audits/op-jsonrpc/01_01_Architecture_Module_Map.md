# Production Security and Quality Audit

## Architecture & Module Map

### Overview
The `op-jsonrpc` crate provides a lightweight JSON-RPC 2.0 server over Unix sockets and TCP ports. It acts as an integration interface for state management, offering:
1. **NonNet Database**: An OVSDB-compatible, read-only dynamic database representing non-network plugin states.
2. **OVSDB Proxy**: A client wrapper that proxies structured database requests to a local Open vSwitch database socket.

### Module Tree
```
crates/op-jsonrpc/src/lib.rs (Crate Root)
 ├── nonnet (pub mod) -> crates/op-jsonrpc/src/nonnet.rs
 ├── ovsdb (pub mod) -> crates/op-jsonrpc/src/ovsdb.rs
 ├── protocol (pub mod) -> crates/op-jsonrpc/src/protocol.rs
 └── server (pub mod) -> crates/op-jsonrpc/src/server.rs
```

### Entry Points
- **Library Entry Point**: `crates/op-jsonrpc/src/lib.rs`
- **JSON-RPC Server Socket Bindings**: `JsonRpcServer::run` in `crates/op-jsonrpc/src/server.rs`
- **NonNet Specialized Server**: `NonNetDb::run_server` in `crates/op-jsonrpc/src/nonnet.rs`

### Auxiliary & Orphaned Files
The following files exist in the `src` directory but are not registered as modules in the crate's `lib.rs`:
- `crates/op-jsonrpc/src/nonnet_staging.rs`
- `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs`
- `crates/op-jsonrpc/src/ovsdb_rpc_call.rs`

---

## Findings

### Critical Vulnerabilities

#### [CRITICAL] Undefined Behavior and Heap Buffer Over-read via Unsafe `simd-json` Parsing Without Buffer Padding
- **Citations**: 
  - `crates/op-jsonrpc/src/nonnet.rs:260`
  - `crates/op-jsonrpc/src/server.rs:281`
  - `crates/op-jsonrpc/src/ovsdb.rs:108`
  - `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:28`
- **Impact**: Memory Safety Violation / Out-Of-Bounds Read / Denial of Service / Information Disclosure.
- **Description**: 
  `simd-json` is optimized for vectorized instruction execution. Its in-place parsing functions, such as `from_str`, require the input buffer to be padded with a trailing block (such as `simd_json::PADDING`) to prevent vector loaders (like AVX2/SSE) from reading past the end of the buffer into unallocated or adjacent memory.
  
  In several locations, standard, un-padded `String` allocations populated by `read_line` or converted from raw byte slices are parsed using `unsafe simd_json::from_str`:
  ```rust
  // crates/op-jsonrpc/src/nonnet.rs:260
  let response = match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) } {
  ```
  Calling `as_mut_str()` on an un-padded `String` slice violates `simd-json` safety guarantees. This triggers undefined behavior (UB), resulting in segmentation faults (denial of service) or potential heap memory disclosure if adjacent heap data is read as part of the JSON structure.

---

#### [CRITICAL] Unauthenticated Arbitrary Database Transaction Execution via Exposed TCP Socket Proxy
- **Citations**: 
  - `crates/op-jsonrpc/src/server.rs:188`
  - `crates/op-jsonrpc/src/server.rs:324-326`
  - `crates/op-jsonrpc/src/server.rs:388-392`
- **Impact**: Full Host / Network Compromise.
- **Description**: 
  The `JsonRpcServer` can be configured to bind to a TCP address via `run_tcp` (`crates/op-jsonrpc/src/server.rs:188`). When active, this socket exposes the OVSDB proxy methods `ovsdb.transact` without requiring any authentication, authorization, or TLS/SSL encryption:
  ```rust
  "ovsdb.list_dbs" | "ovsdb.get_schema" | "ovsdb.transact"
      if self.config.ovsdb_enabled =>
  {
      return self.handle_ovsdb_request(request).await;
  }
  ```
  Because Open vSwitch (OVS) runs with root privileges and dictates physical/virtual networking layers (bridges, interfaces, routing tables), any network-level attacker who can reach the TCP port can execute arbitrary OVSDB mutation or deletion transactions. This allows attackers to redirect host traffic, inject network interfaces, or intercept sensitive control plane payloads.

---

#### [CRITICAL] Permanent Connection Deadlock and Hangs in OVSDB Client RPC Calls
- **Citations**: 
  - `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:21-24`
- **Impact**: Denial of Service (Component Non-Functionality).
- **Description**: 
  In the OVSDB client helper `rpc_call` in `ovsdb_rpc_call.rs`, queries are written to the Unix stream, and then `tokio::io::AsyncReadExt::read_to_end` is invoked to read the response:
  ```rust
  let mut response_buf = Vec::new();
  tokio::time::timeout(self.timeout, tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut response_buf))
      .await
      .context("OVSDB response timeout")??;
  ```
  Because `read_to_end` reads continuously until receiving an EOF (connection close), and because the persistent OVSDB database socket does not close the connection after responding, `read_to_end` will block indefinitely. No half-close (`stream.shutdown()`) is performed prior to reading. Consequently, every call to this module will timeout after 30 seconds and return a failure, completely disabling database communication.

---

### Insecure Data Contracts & Ad-hoc Schemas

#### 1. Ad-hoc and Untyped Data Contracts in Protocol Definitions
- **Citations**: 
  - `crates/op-jsonrpc/src/protocol.rs:9`
  - `crates/op-jsonrpc/src/protocol.rs:34`
  - `crates/op-jsonrpc/src/protocol.rs:81`
- **Impact**: High maintenance overhead, schema-drift, parsing vulnerabilities.
- **Description**: 
  The primary JSON-RPC types are represented using dynamically typed, ad-hoc structures rather than versioned, validated schemas:
  ```rust
  pub struct JsonRpcRequest {
      pub jsonrpc: String,
      pub method: String,
      #[serde(default)]
      pub params: Value, // Dynamic unstructured json value
      pub id: Value,
  }
  ```
  Instead of utilizing compiled Protocol Buffers or JSON Schemas to validate JSON-RPC request and response payloads, the system consumes arbitrary `simd_json::OwnedValue` objects. This bypasses the schema-as-code discipline, opening up the system to type mismatch panics and logical bypasses.

---

#### 2. Ad-hoc Runtime Schema Inference and Inconsistent Coercions
- **Citations**: 
  - `crates/op-jsonrpc/src/nonnet.rs:335`
  - `crates/op-jsonrpc/src/nonnet.rs:356`
  - `crates/op-jsonrpc/src/nonnet_staging.rs:77`
- **Impact**: Schema mismatch, runtime typing errors, and data degradation.
- **Description**: 
  The NonNet component automatically infers database table column types at runtime based on plugin state values:
  ```rust
  fn infer_type(value: &Value) -> &'static str {
      ...
      if value.is_number() {
          return "integer";
      }
      ...
  ```
  This is a critical violation of schema-as-code principles. The schema is mutable and inferred on-the-fly rather than compiled from a single versioned schema source of truth. Additionally, `value.is_number()` matches floats, meaning floating-point values are coerced into the `"integer"` type definition, leading to schema drift. Furthermore, the staging module in `nonnet_staging.rs` maps number values to `"number"`, causing inconsistencies between staging and production.

---

#### 3. Hardcoded Database Operation Structures and Lack of Versioned Schemas
- **Citations**: 
  - `crates/op-jsonrpc/src/ovsdb.rs:175-210`
  - `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:118-150`
- **Impact**: Logic errors, unvalidated OVSDB command structures.
- **Description**: 
  Commands executed on OVSDB (such as creating bridges or setting up ports) bypass structured API layers and instead dynamically construct raw JSON arrays using the `json!` macro:
  ```rust
  let operations = json!([
      {
          "op": "insert",
          "table": "Bridge",
          "row": {
              "name": name,
              "ports": ["set", [["named-uuid", port_uuid]]]
          },
          "uuid-name": bridge_uuid
      },
      ...
  ```
  These raw database structural definitions should be defined as formal schemas. Generating raw database structures using unstructured dynamic maps increases the risk of structure mismatches with OVSDB versions.

---

### Other Security & Quality Findings

#### 1. Unbounded Reader Allocation (Denial of Service via OOM)
- **Citations**: 
  - `crates/op-jsonrpc/src/nonnet.rs:257`
  - `crates/op-jsonrpc/src/server.rs:218`
  - `crates/op-jsonrpc/src/server.rs:234`
  - `crates/op-jsonrpc/src/nonnet_staging.rs:47`
- **Impact**: Denial of Service (Out-Of-Memory Crash).
- **Description**: 
  The connection loop processes streams line-by-line using `read_line(&mut line)`:
  ```rust
  while reader.read_line(&mut line).await? > 0 {
  ```
  `read_line` allocates memory dynamically until a newline character (`\n`) is encountered. If a malicious client initiates a connection and streams an endless sequence of bytes without a newline, the process will consume memory boundlessly until it is terminated by the operating system's OOM killer.
- **Remediation**: Use `tokio::io::AsyncBufReadExt::take` or wrap the reader in a custom decoder that enforces a strict maximum line length limit (e.g., 64KB).

---

#### 2. Time-of-Check to Time-of-Use (TOCTOU) Socket Bind Race Condition
- **Citations**: 
  - `crates/op-jsonrpc/src/nonnet.rs:222-225`
  - `crates/op-jsonrpc/src/server.rs:163-166`
  - `crates/op-jsonrpc/src/nonnet_staging.rs:21-23`
- **Impact**: Medium (Local DoS / Race condition during startup).
- **Description**: 
  When binding Unix domain sockets, the code verifies file existence prior to deleting and binding:
  ```rust
  if path.exists() {
      tokio::fs::remove_file(path).await.ok();
  }
  let listener = UnixListener::bind(path)...
  ```
  This creates a race window. An attacker with write permissions in the target directory could create a socket, link, or file after `exists()` returns but before `remove_file` or `bind` executes.
- **Remediation**: Eliminate the `exists()` check. Unconditionally try to bind, and if it fails with `AddrInUse`, attempt to delete the stale socket file and bind again.

---

#### 3. Orphaned, Non-Compilable Staging Files
- **Citations**: 
  - `crates/op-jsonrpc/src/nonnet_staging.rs:1`
- **Impact**: Codebase pollution, build quality degradation.
- **Description**: 
  The `nonnet_staging.rs` file references a non-existent internal state module:
  ```rust
  use crate::state::StateManager;
  ```
  This module is not defined in `crates/op-jsonrpc/src/lib.rs`. If `nonnet_staging.rs` were registered as a module, the compilation would immediately fail. It exists as dead, broken code in the production codebase.
- **Remediation**: Remove `nonnet_staging.rs` and other unregistered staging source files (`ovsdb_jsonrpc.rs`, `ovsdb_rpc_call.rs`) from the production workspace directory.

---
## ⚠ Citation Warnings
- `crates/op-jsonrpc/src/server.rs:388`: file has 374 lines
