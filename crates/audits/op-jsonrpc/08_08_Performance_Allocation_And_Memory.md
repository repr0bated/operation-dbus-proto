# PRODUCTION SECURITY & QUALITY AUDIT REPORT

## 1. Executive Summary
This audit evaluated the safety, performance, memory allocation patterns, and schema architecture of the `op-jsonrpc` crate. The audit revealed a systemic security flaw involving the improper use of `unsafe` parsing APIs within `simd-json` on non-padded buffers, which constitutes a critical memory safety risk. Furthermore, significant allocation and cloning overhead on JSON payloads was detected in high-throughput hot paths. Lastly, multiple ad-hoc data contracts exist, violating the project’s strict schema-as-code and OSCAL compliance guidelines.

---

## 2. Critical Security Findings

### 2.1. Critical: Out-of-Bounds Read/Write via Unsafe `simd_json::from_str` on Unpadded Buffers
* **File & Line Citations:**
  * `crates/op-jsonrpc/src/nonnet.rs:285`
  * `crates/op-jsonrpc/src/server.rs:260`
  * `crates/op-jsonrpc/src/ovsdb.rs:114`
  * `crates/op-jsonrpc/src/ovsdb.rs:128`
  * `crates/op-jsonrpc/src/ovsdb.rs:431`
  * `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:27`

* **Vulnerability Description:**
  The `simd-json` crate relies on highly optimized SIMD vector registers to parse JSON. For these operations to be safe, the input slice MUST contain at least `simd_json::PADDING_SIZE` (typically 32 bytes) of allocated, addressable memory beyond the end of the JSON string. Without this padding, SIMD instructions can read or write out of bounds, leading to segmentation faults, memory corruption, or information disclosure.
  
  In the cited code locations, the server reads data from standard Unix or TCP sockets using standard `String::new()` and `BufReader::read_line(&mut line)`. It then invokes `unsafe { simd_json::from_str(...) }` directly on `line.as_mut_str()` or `payload.as_mut_str()`. None of these strings contain the mandatory end-of-buffer padding.

* **Exploitability Analysis:**
  An unauthenticated client sending JSON payloads over the network or local sockets can trigger an out-of-bounds read if the payload aligns near a memory page boundary. Because this is executed inside spawn blocks handling network connections, this vulnerability is directly exploitable from the network/IPC boundaries.

* **Remediation:**
  Do not use `unsafe simd_json::from_str` on unpadded standard `String` or `&mut str` buffers. Use `simd_json::to_padded_container` or migrate to the safe, padded API wrappers provided by the `simd-json` crate. Alternatively, parse into a pre-allocated padded buffer such as `simd_json::AlignedBuf`.

---

## 3. Performance & Allocation Audit

### 3.1. Non-Preallocated Collections (`Vec` and `String`) Inside Loops
* **File & Line Citations:**
  * `crates/op-jsonrpc/src/nonnet.rs:282` (Instantiates `String::new()` in connection read loop)
  * `crates/op-jsonrpc/src/nonnet.rs:339` (Instantiates `let mut results = Vec::new()` without pre-allocation in transaction loop)
  * `crates/op-jsonrpc/src/server.rs:111` (Instantiates `let mut handles = Vec::new()` inside `run` without pre-allocation)
  * `crates/op-jsonrpc/src/server.rs:205` (Instantiates `String::new()` inside socket connection handler loop)
  * `crates/op-jsonrpc/src/server.rs:221` (Instantiates `String::new()` inside TCP connection handler loop)
  * `crates/op-jsonrpc/src/ovsdb.rs:81` (Instantiates `let mut response_bytes = Vec::new()` for every OVSDB RPC call)
  * `crates/op-jsonrpc/src/ovsdb.rs:198` (Instantiates `let mut bridges = Vec::new()` without capacity)
  * `crates/op-jsonrpc/src/ovsdb.rs:223` (Instantiates `let mut port_uuids = Vec::new()` without capacity)
  * `crates/op-jsonrpc/src/ovsdb.rs:229` (Instantiates `let mut port_names = Vec::new()` without capacity)

* **Impact:**
  Spawning new `String` or `Vec` containers without capacity limits inside request loops causes frequent allocator churn, memory fragmentation, and latency spikes under heavy JSON-RPC transaction loads.

* **Remediation:**
  Utilize `Vec::with_capacity(capacity)` and reuse buffer lines across read loops by clearing them via `line.clear()` instead of reallocating on every iteration.

---

### 3.2. Performance-Degrading `Value.clone()` on Large JSON Payloads
* **File & Line Citations:**
  * `crates/op-jsonrpc/src/nonnet.rs:115` (`rows.clone()`)
  * `crates/op-jsonrpc/src/nonnet.rs:116` (`rows.clone()`)
  * `crates/op-jsonrpc/src/nonnet.rs:136` (`rows.clone()`)
  * `crates/op-jsonrpc/src/nonnet.rs:140` (`table_rows.clone()`)
  * `crates/op-jsonrpc/src/nonnet.rs:144` (`schema.clone()`)
  * `crates/op-jsonrpc/src/nonnet.rs:158` (`table_rows.clone()`)
  * `crates/op-jsonrpc/src/nonnet.rs:163` (`rows.clone()`)
  * `crates/op-jsonrpc/src/nonnet.rs:167` (`table_rows.clone()`)
  * `crates/op-jsonrpc/src/nonnet.rs:288` (`value.clone()`)
  * `crates/op-jsonrpc/src/nonnet.rs:324` (`state.schema.clone()`)
  * `crates/op-jsonrpc/src/nonnet.rs:420` (`val.clone()`)
  * `crates/op-jsonrpc/src/nonnet_staging.rs:136` (`val.clone()`)

* **Impact:**
  Cloning a `simd_json::OwnedValue` (Value) forces deep copy recursive traversals of heap-allocated arrays and object maps. When processed on larger state snapshots (e.g., loaded plugin states), this triggers extensive heap replication overhead and CPU cycles spent on allocations.

* **Remediation:**
  Store structures within thread-safe references (`Arc`) or modify processing logic to borrow references `&Value` rather than consuming ownership.

---

### 3.3. Multi-Allocation `format!` Call Overhead in Hot Paths
* **File & Line Citations:**
  * `crates/op-jsonrpc/src/nonnet.rs:290` (Formats error within read loop)
  * `crates/op-jsonrpc/src/nonnet.rs:297` (Formats error within read loop)
  * `crates/op-jsonrpc/src/nonnet.rs:319` (Formats string key lookup failure)
  * `crates/op-jsonrpc/src/nonnet.rs:348` (Formats string on transaction operation type mismatch)
  * `crates/op-jsonrpc/src/nonnet.rs:388` (Formats unknown method execution error)
  * `crates/op-jsonrpc/src/server.rs:266` (Formats invalid request inside connection handler)
  * `crates/op-jsonrpc/src/server.rs:273` (Formats parse error inside connection handler)
  * `crates/op-jsonrpc/src/server.rs:333` (Formats error message for every unknown request method)
  * `crates/op-jsonrpc/src/ovsdb.rs:163-165` (Formats multiple naming UUIDs inside create-bridge)
  * `crates/op-jsonrpc/src/ovsdb.rs:272-273` (Formats safe naming strings dynamically)

* **Impact:**
  The `format!` macro produces a temporary `String` allocation. Performing this inside loops and error paths—even those logged at low tracing levels or returned directly in RPC responses—creates unnecessary heap allocation overhead.

* **Remediation:**
  Replace dynamic formatted strings in hot loops with static string references where possible, or use lazy evaluation with zero-allocation formats.

---

## 4. Schema-as-Code & OSCAL Compliance Analysis

### 4.1. Ad-Hoc Data Contracts and Untyped Map Schemas
* **File & Line Citations:**
  * `crates/op-jsonrpc/src/protocol.rs:10-38`
  * `crates/op-jsonrpc/src/nonnet.rs:65-68`
  * `crates/op-jsonrpc/src/nonnet.rs:92-130`
  * `crates/op-jsonrpc/src/nonnet_staging.rs:101-125`

* **Compliance Deviation:**
  The project enforces a schema-as-code discipline using Protocol Buffers (protobuf) and OSCAL component structures. However, the JSON-RPC interface ignores these compile-time safety and regulatory structures:
  
  1. `JsonRpcRequest` and `JsonRpcResponse` define their parameters and results using an untyped and unversioned `simd_json::OwnedValue` (Value) structure.
  2. Database tables, columns, and types inside the `NonNetDb` are inferred dynamically on-the-fly from unstructured JSON structures via `infer_columns` and `infer_type`.
  3. Dynamic schema-generation code (`empty_nonnet_schema`) builds ad-hoc object definitions directly as unversioned runtime JSON objects.

* **Remediation:**
  Compile the JSON-RPC models and schema definitions directly from Protocol Buffer definitions (`.proto` files). Ensure OSCAL security control profiles map back to explicitly typed schemas representing the plugin components instead of treating table schemas as dynamic maps.

---

## 5. Memory Mapping & Large Allocation Mapping

### 5.1. Memory Mapping and Large Heap Allocation Analysis
The following table details the memory allocation analysis for the evaluated `op-jsonrpc` codebase. Note that despite `memmap2` being defined in the root workspace `Cargo.toml`, there are no memory-mapped file handles, `memmap2` usages, or `sled` databases initialized inside the evaluated library files of the `op-jsonrpc` crate.

### 5.2. Memory Map Table

| Site | File : Line | Type | Risk | Mitigation |
| :--- | :--- | :--- | :--- | :--- |
| `UnixStream` Buffer Reads | `crates/op-jsonrpc/src/nonnet.rs:283` | Socket I/O BufReader | Medium | High buffer volume without explicit bounds could trigger high memory usage. Limit socket stream size. |
| OVSDB RPC Response Buffer | `crates/op-jsonrpc/src/ovsdb.rs:81` | Dynamic `Vec<u8>` Heap Alloc | Medium | `read_to_end` allocates arbitrarily large vector arrays based on target OVSDB responses. |
| OVSDB RPC Response Buffer | `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:19` | Dynamic `Vec<u8>` Heap Alloc | Medium | Re-allocates response buffers per call without reuse. |