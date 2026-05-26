# Performance, Allocation & Memory Map Audit

## 1. Memory Map Analysis

### Memory Map Table

| Site | file:line | Type (ro/rw/sled) | Risk |
| :--- | :--- | :--- | :--- |
| N/A | None | N/A | No direct memory-mapping or `sled` calls are present in the provided `op-web` source files. |

### Memory Mapping and Sled Details
* **Direct Memory Mapping (`memmap2`, `mmap`, `MmapMut`, `MmapOptions`)**: None of the provided files within the `op-web` crate directly invoke or configure memory maps. While `memmap2` is defined as a workspace dependency in `Cargo.toml`, no direct imports or usages of memory-mapping structures exist in the audited files.
* **Sled Database Usage**: Sled is not directly opened, initialized, or configured in any of the audited `op-web` files. (Sled is referenced in `cozo` workspace dependencies, but no internal or external sled instances are initialized in the visible codebase). Therefore, no tmpfs or noexec mount violations can be evaluated.
* **Large Heap Allocations**: No large heap allocations (e.g., `Vec::with_capacity` > 1MB, or large pre-allocations of `BytesMut`/`Bytes`) are explicitly defined in the provided `op-web` files.

---

## 2. Performance & Allocation Audit

### Finding 1: Critical — Remote Denial of Service (DoS) via Unsafe `simd_json::from_str` on Non-Padded Buffers
* **File:Line**:
  * `crates/op-web/src/websocket.rs:79`
  * `crates/op-web/src/handlers/websocket.rs:52`
  * `crates/op-web/src/groups_admin.rs:55`
  * `crates/op-web/src/state_manager_client.rs:34`
  * `crates/op-web/src/orchestrator/parsing.rs:25`
  * `crates/op-web/src/orchestrator/parsing.rs:65`
  * `crates/op-web/src/orchestrator/parsing.rs:92`
  * `crates/op-web/src/orchestrator/parsing.rs:103`
  * `crates/op-web/src/orchestrator/parsing.rs:124`
  * `crates/op-web/src/orchestrator/parsing.rs:139`
  * `crates/op-web/src/orchestrator/execution.rs:73`
* **Vulnerability Type**: Memory Safety Violation / Undefined Behavior / Out-of-Bounds Read
* **Severity**: **Critical**
* **Description**:
  The `simd-json` crate requires that all input buffers passed to its parser be padded with `simd_json::SIMDJSON_PADDING` bytes. This padding is necessary because the underlying SIMD instructions perform vectorized chunk reads (often 32 or 64 bytes at a time) and will read past the end of the logical string. If the buffer is not padded, this read will cross a memory boundary into unallocated or unmapped pages, resulting in a segmentation fault or memory leak.
  
  In the files cited above, standard Rust `String` instances (which are unpadded) are cloned and passed directly to `unsafe { simd_json::from_str(&mut raw) }`.
  
  For example, in `crates/op-web/src/websocket.rs:79`:
  ```rust
  let mut raw = text.clone();
  let ws_msg: Result<WsMessage, _> = unsafe { simd_json::from_str(&mut raw) };
  ```
  Here, `text` is a standard `String` populated directly from incoming WebSocket frames. An attacker can send a carefully sized WebSocket message to trigger an out-of-bounds heap read, crashing the entire web server (Remote DoS).
* **Remediation**:
  Use `simd_json::to_owned_value` on a padded vector, or use safe JSON parsing (such as `serde_json::from_str`) for buffers where padding cannot be guaranteed. Alternatively, use `simd_json::padded_free::PaddedBytes` to safely construct padded buffers from input strings before parsing.

---

### Finding 2: Medium — Excessive `OwnedValue.clone()` (Deep Copying) on Request/Tool Call Payloads
* **File:Line**:
  * `crates/op-web/src/mcp.rs:324`
  * `crates/op-web/src/mcp_agents.rs:475`
  * `crates/op-web/src/mcp_compact.rs:253`
  * `crates/op-web/src/mcp_compact.rs:377`
  * `crates/op-web/src/orchestrator/execution.rs:12`
* **Vulnerability Type**: Performance Degredation / Unnecessary Heap Allocation
* **Severity**: **Medium**
* **Description**:
  Throughout the MCP handlers and the orchestrator, `params.get("arguments").cloned()` is called on every incoming tool execution or meta-tool query. Because `arguments` is represented as a `simd_json::OwnedValue` (an AST representation of JSON), calling `.cloned()` triggers a deep clone of the entire JSON AST tree, recursively copying maps, arrays, and strings.
  
  In high-throughput environments or when handling large JSON payloads (such as large configuration maps or agent states), this results in severe heap allocation churn and CPU overhead.
* **Remediation**:
  Pass arguments by reference (`&Value`) or move the ownership of the parsed AST rather than performing a deep copy. Where possible, refactor the orchestrator to process references to the parsed request object instead of repeatedly copying payload sub-elements.

---

### Finding 3: Low — `Vec::new` and `String::new` Inside Loops without Pre-allocation
* **File:Line**:
  * `crates/op-web/src/groups_admin.rs:140`
  * `crates/op-web/src/mcp_agents.rs:235`
  * `crates/op-web/src/handlers/logs.rs:29`
  * `crates/op-web/src/handlers/status.rs:185`
  * `crates/op-web/src/handlers/status.rs:224`
* **Vulnerability Type**: Non-optimal Allocation Pattern
* **Severity**: **Low**
* **Description**:
  Vectors and strings are instantiated using `Vec::new()` inside loops or dynamic iteration blocks without reserving capacity. This causes the vectors to repeatedly re-allocate and copy their contents as elements are pushed.
  
  In `mcp_agents.rs:235`:
  ```rust
  let mut tools = Vec::new();
  ...
  for (agent_type, entry) in entries {
      for operation in &entry.descriptor.operations {
          tools.push(...)
      }
  }
  ```
  The size of `tools` is determinable beforehand by summing the lengths of operations.
* **Remediation**:
  Use `Vec::with_capacity` when the number of elements is known or can be estimated prior to loop execution.
  ```rust
  let capacity = entries.values().map(|e| e.descriptor.operations.len()).sum();
  let mut tools = Vec::with_capacity(capacity);
  ```

---

### Finding 4: Low — `format!()` in Hot Paths and Iterators
* **File:Line**:
  * `crates/op-web/src/mcp_agents.rs:241` (Called twice per inner loop element)
  * `crates/op-web/src/handlers/status.rs:230` (Called inside system class directory iterator)
  * `crates/op-web/src/handlers/status.rs:236` (Called inside system class directory iterator)
  * `crates/op-web/src/mcp.rs:164` (Called on every connection setup)
* **Vulnerability Type**: Performance Overhead
* **Severity**: **Low**
* **Description**:
  `format!()` dynamically parses format strings at runtime, allocating a new `String` on the heap every time. When used inside hot paths—such as SSE connection loops, D-Bus polling routines, or network interface directories—this adds noticeable overhead.
  
  In `mcp_agents.rs:241`:
  ```rust
  tools.push(json!({
      "name": Self::tool_name(agent_type, operation), // formatting inside inner loop
      "description": format!("{} ({})", entry.descriptor.description, operation), // formatting inside inner loop
  ...
  ```
* **Remediation**:
  For static or predictable concatenations, use standard string slicing, `inplace_it` formatting, or pre-allocate strings. For logging or error conditions, defer formatting unless the error path is actually taken.