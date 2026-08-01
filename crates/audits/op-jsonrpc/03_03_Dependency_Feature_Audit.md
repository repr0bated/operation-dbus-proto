# Security and Quality Audit Report: `op-jsonrpc`

This document provides a production-grade security, quality, and architectural audit of the `op-jsonrpc` crate, as represented by the files provided in the FILES section.

---

## 1. Scope & Audited Files
The scope of this audit is strictly restricted to the following source files:
* `crates/op-jsonrpc/Cargo.toml` — Crate configuration and dependencies
* `crates/op-jsonrpc/src/lib.rs` — Library entrypoint
* `crates/op-jsonrpc/src/nonnet.rs` — NonNet database implementation
* `crates/op-jsonrpc/src/protocol.rs` — JSON-RPC 2.0 protocol specifications
* `crates/op-jsonrpc/src/server.rs` — Unified JSON-RPC server
* `crates/op-jsonrpc/src/nonnet_staging.rs` — Read-only staging database interface
* `crates/op-jsonrpc/src/ovsdb.rs` — OVSDB client and bridge manager
* `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs` — OVSDB protocol communication
* `crates/op-jsonrpc/src/ovsdb_rpc_call.rs` — Low-level RPC socket connection handler

---

## 2. Dependencies & Feature Inventory

Based on `crates/op-jsonrpc/Cargo.toml` and the workspace `Cargo.toml`, here is the inventory of direct dependencies and their configuration:

| Dependency | Version Spec | Resolved Version (Lock) | Enabled Features | Security / Operational Notes |
| :--- | :--- | :--- | :--- | :--- |
| `op-core` | Workspace | `0.1.0` (Internal) | Inherited from workspace | Internal core library dependency |
| `tokio` | Workspace | `1.49.0` | `full` (Workspace default) | Large attack surface; includes async file, socket, and thread management |
| `serde` | Workspace | `1.0.228` | `derive` | Default serialization engine |
| `simd-json` | Workspace | `0.13.11` | `serde`, `serde_impl` | Zero-copy SIMD parser; contains significant unsafe surface area |
| `anyhow` | Workspace | `1.0.100` | None | General error wrapper |
| `thiserror` | Workspace | `1.0.69` | None | Strongly-typed library errors |
| `tracing` | Workspace | `0.1.44` | None | Internal structured telemetry |
| `uuid` | Workspace | `1.20.0` | `v4`, `serde` | Used for bridge/port generation |

### Features Gating
* **`op-jsonrpc` Crate Features**: None defined (`none defined` in `crates/op-jsonrpc/Cargo.toml`).
* **Critical Dependencies Checked**:
  * **`tokio`**: Standard features are broad. Heavy network socket dependency on Unix listeners (`UnixListener`) and TCP listeners (`TcpListener`).
  * **`anyhow` / `thiserror`**: Correctly split between library-level diagnostics and general server error-wrapping.
  * **Yanked/Unpinned Crates**: None directly specified with `*`, but workspace dependencies use minor-version pinning (e.g., `simd-json = "0.13"`).

---

## 3. Storage Backend Inventory

`op-jsonrpc` acts as an integration and proxy plane rather than a persistent transactional database. Below is the inventory of the storage mechanics used inside the crate:

| Backend | Found at File:Line | Role | Architecture Alignment Check |
| :--- | :--- | :--- | :--- |
| **In-Memory HashMap** (`NonNetState`) | `crates/op-jsonrpc/src/nonnet.rs:43` | Ephemeral table storage for non-network plugin states | **Aligned**: Suitable for read-only ephemeral states mirrored from loaded plugins. |
| **Unix Domain Socket** (`OVSDB`) | `crates/op-jsonrpc/src/ovsdb.rs:65` | Remote proxy target (`/var/run/openvswitch/db.sock`) | **Aligned**: Directly delegates persistent transactional states to the host OVSDB engine. |

### Note on Global Workspace Storage
The workspace includes `cozo` (relational graph-vector database) and `sqlx` (SQLite backend). However, `op-jsonrpc` does not directly use these engines, maintaining strict separation of concerns by serving as a lightweight RPC gateway.

---

## 4. Schema-As-Code Compliance Critique

This codebase enforces a schema-as-code discipline through Protobuf (`prost`) and structured parsing in other parts of the workspace, but **`op-jsonrpc` contains significant schema-as-code gaps**:

1. **Ad-Hoc JSON Parameter Traversal**:
   Instead of mapping JSON-RPC method parameters to strongly-typed structs via versioned schemas, the codebase relies on raw JSON destructuring:
   * `crates/op-jsonrpc/src/nonnet.rs:290` parses the database target out of a raw array slice:
     ```rust
     let db = request.params.as_array().and_then(|p| p.first()).and_then(|v| v.as_str()).unwrap_or(NONNET_DB_NAME);
     ```
   * `crates/op-jsonrpc/src/nonnet.rs:307` implements ad-hoc manual walking of transact operations:
     ```rust
     let ops = &params[1..];
     for op in ops {
         let op_type = op.get("op").and_then(|v| v.as_str()).unwrap_or("");
         ...
     }
     ```
2. **Brittle Dynamic Schema Inference**:
   Instead of using static schemas to enforce data types, the NonNet DB attempts to *infer* schemas at runtime based on whatever values the plugins supply:
   * `crates/op-jsonrpc/src/nonnet.rs:388` dynamically infers field types (`infer_type` returns `"string"`, `"integer"`, `"set"`, etc.) on the fly. If a plugin updates its state with a different data structure, the schema dynamically shifts, breaking downstream client assumptions.
3. **Absence of OpenAPI/JSONSchema Checks on Ingress**:
   The incoming payloads on the JSON-RPC TCP and Unix socket listeners are parsed as raw, unrestricted `Value` types. There is no verification against a schema file before processing, leaving the server vulnerable to structural logic bugs.

---

## 5. Critical Security Findings

### Finding 1: Unsafe Buffer Manipulation and Memory Safety Violation in `simd_json::from_str`
* **Vulnerability Type**: Out-of-Bounds Memory Read / Undefined Behavior / Denial of Service
* **File & Line Citations**:
  * `crates/op-jsonrpc/src/nonnet.rs:245`
  * `crates/op-jsonrpc/src/server.rs:244`
  * `crates/op-jsonrpc/src/ovsdb.rs:136`
  * `crates/op-jsonrpc/src/ovsdb.rs:149`
  * `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:28`

#### Analysis
`simd-json` achieves its high performance by performing zero-copy parsing using optimized SIMD instructions (AVX2/Neon). However, this introduces strict memory safety requirements. The `simd-json` documentation specifies:
> **The input buffer must be padded with `simd_json::SIMDJSON_PADDING` bytes (typically 32 or 64 bytes) past the end of the logical string.**

If this padding is absent, the SIMD vector registers will perform unaligned reads past the allocated memory boundary of the slice. If the memory allocation lands near a memory page boundary, this unaligned vector read crosses into unmapped memory, resulting in an immediate **Segmentation Fault (`SIGSEGV`)** and crashing the daemon.

In the audited files, `simd_json::from_str` is invoked inside `unsafe` blocks directly on standard strings that contain no padding:
```rust
// crates/op-jsonrpc/src/nonnet.rs:245
let response = match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) } {
```
The string `line` is populated by `BufReader::read_line`. The returned `&mut str` from `line.as_mut_str()` represents exactly the length of the string data, lacking any padded allocation room. This guarantees that `simd-json` will perform an out-of-bounds read.

This same pattern is repeated:
* In `crates/op-jsonrpc/src/server.rs:244` on network connection inputs.
* In `crates/op-jsonrpc/src/ovsdb.rs:136` and `149` where incoming OVSDB data is coerced into string slices (`payload.as_mut_str()`) and passed to `simd_json::from_str` without padding.
* In `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:28` where the socket-read buffer is parsed directly with no padding.

#### Exploit Scenario
An attacker connects to the JSON-RPC socket (either over TCP or the Unix socket) and sends a specifically structured request of variable length. If the payload is parsed when the string's allocation is near the end of a heap boundary, the SIMD instruction reads into unmapped memory. This reliably crashes the entire `op-dbus` control plane, resulting in a persistent Denial of Service (DoS) of the system’s network provisioning state.

#### Remediation
Replace the `unsafe simd_json::from_str` calls with safe equivalents that manage padding automatically, or allocate a padded vector explicitly.
```rust
// Safe alternative using standard serde_json (non-SIMD) for unpadded network inputs:
let request: JsonRpcRequest = serde_json::from_str(&line)?;

// Or, if simd-json must be used, allocate a padded Vec<u8> buffer:
let mut padded_bytes = line.into_bytes();
padded_bytes.resize(padded_bytes.len() + simd_json::SIMDJSON_PADDING, 0);
let response = simd_json::to_owned_value(&mut padded_bytes)?;
```

---

## 6. High & Medium Severity Findings

### Finding 2: Unbounded Connection Line Buffer Leading to Memory Exhaustion (HIGH)
* **Vulnerability Type**: Denial of Service (DoS) via Memory Exhaustion
* **File & Line Citations**:
  * `crates/op-jsonrpc/src/nonnet.rs:242`
  * `crates/op-jsonrpc/src/server.rs:200`
  * `crates/op-jsonrpc/src/server.rs:216`

#### Analysis
On every incoming connection (both Unix socket and TCP), the server splits the stream and reads line-by-line:
```rust
// crates/op-jsonrpc/src/server.rs:199
let mut reader = BufReader::new(reader);
let mut line = String::new();

while reader.read_line(&mut line).await? > 0 {
    ...
```
`tokio::io::BufReadExt::read_line` reads bytes until it encounters a newline character (`\n`). There is no limit enforced on how many bytes can be read into `line` before a newline is encountered.

#### Exploit Scenario
An unauthenticated socket client opens a connection and streams a continuous sequence of random bytes without ever sending a newline character (`\n`). The server continues allocating memory in the heap for the `line` buffer. Because there are no resource limits, the process will consume all available host RAM until the Linux kernel Out-Of-Memory (OOM) Killer terminates the `op-dbus` process.

#### Remediation
Enforce a hard limit on the maximum allowable line size using a custom adapter or a restricted reader stream (e.g., wrapping the reader in `tokio::io::AsyncReadExt::take` before passing it to `BufReader`).
```rust
// Restrict each read operation to a maximum of 10MB to prevent heap exhaustion
let limited_reader = reader.take(10 * 1024 * 1024);
```

---

### Finding 3: Time-of-Check to Time-of-Use (TOCTOU) Race Condition in Port Provisioning (MEDIUM)
* **Vulnerability Type**: Logic Flaw / State Desynchronization
* **File & Line Citations**:
  * `crates/op-jsonrpc/src/ovsdb.rs:271`

#### Analysis
In `OvsdbClient::add_port`, the function checks if the port is already attached and queries the database for existing ports:
```rust
let existing_ports = self.list_ports(bridge).await.unwrap_or_default();
if existing_ports.iter().any(|p| p == port) {
    info!("Port {} already attached to bridge {}", port, bridge);
    return Ok(());
}

let existing_port_uuid = self.find_named_row_uuid("Port", port).await.ok();
```
These check operations are executed as independent, sequential JSON-RPC round-trips over the Unix socket. Only after these requests complete does it execute the mutation transaction.

Because these operations are decoupled and non-atomic, a concurrent provisioning task can delete or create the port/interface between the time `list_ports` is called and the transaction `transact` is sent. This results in transaction errors or duplicate attachments.

#### Remediation
Combine the check and mutation states into a single atomic OVSDB transaction using conditional OVSDB `where` assertions, or orchestrate high-level serialization locks around interface mutations.

---

### Finding 4: Incomplete Input Sanitization in OVSDB Reference Generation (MEDIUM)
* **Vulnerability Type**: Command injection / Configuration Corruption
* **File & Line Citations**:
  * `crates/op-jsonrpc/src/ovsdb.rs:219`
  * `crates/op-jsonrpc/src/ovsdb.rs:222`

#### Analysis
When creating bridges, ports, or interfaces, the `OvsdbClient` generates UUID references from the user-provided interface name:
```rust
// crates/op-jsonrpc/src/ovsdb.rs:219
let safe_name = Self::sanitize_ref(name);
let bridge_uuid = format!("bridge_{}", safe_name);
```
However, the sanitization is only applied to the *internal reference UUIDs* (`bridge_uuid`, `port_uuid`, etc.). The actual name written to the OVSDB database row is passed completely unsanitized:
```rust
"row": {
    "name": name, // <--- Passed completely unsanitized
    "ports": ["set", [["named-uuid", port_uuid]]]
}
```
If `name` contains control characters, extremely long sequences, or illegal OVSDB symbols, Open vSwitch may reject the transaction or enter an inconsistent runtime state.

#### Remediation
Enforce validation of the `name` parameter on entry to guarantee it matches a strict regular expression (e.g., `^[a-zA-Z0-9_\-\.]{1,15}$`) before passing it to any OVSDB database rows.

---

## 7. Refactoring & Code Quality Recommendations

### Quality Issue 1: Redundant OVSDB Client Implementations
* **Files**:
  * `crates/op-jsonrpc/src/ovsdb.rs`
  * `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs`
The crate contains two separate definitions of `OvsdbClient`, both performing identical lower-level transactions and bridge management. This leads to maintenance drift, duplication of safety vulnerabilities, and operational confusion.
* **Recommendation**: Consolidate `ovsdb_jsonrpc.rs` and `ovsdb.rs` into a single, unified client struct.

### Quality Issue 2: Hardcoded Socket Paths
* **Files**:
  * `crates/op-jsonrpc/src/server.rs:49`
  * `crates/op-jsonrpc/src/ovsdb.rs:22`
The path `/var/run/op-dbus/jsonrpc.sock` and `/var/run/openvswitch/db.sock` are hardcoded directly into the implementation files. This makes containerized deployment and non-root execution testing difficult.
* **Recommendation**: Extract all Unix socket path defaults to a central configuration struct or environment variables.