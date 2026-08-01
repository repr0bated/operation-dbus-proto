# Security and Quality Audit: op-mcp-proxy

## 1. Memory Map & Memory Mapping Analysis

The codebase utilizes direct memory-mapped file access to read system state and depends on the embedded `sled` database engine transitively through `cozo`. 

### Memory Map Table

| Site | file:line | Type | Risk |
| :--- | :--- | :--- | :--- |
| `SledSnapshot::read()` maps `/dev/shm/plugin_schema.dat` | `crates/op-mcp-proxy/src/sled.rs:35` | `ro` | **SIGBUS Crash**: Maps a file on `tmpfs`. If the file is truncated concurrently, accessing the mapped slice will trigger a SIGBUS and crash the process. Lacks validation of structural bounds. |
| `cozo` embedded storage dependency (`storage-sled`) | `Cargo.toml:44` | `sled` | **Data Corruption / I/O Blocking**: If the database file is placed on a memory-mapped mount (like a `tmpfs` or `noexec` directory), memory flushing can fail or execute insecurely. |
| `PredictionServiceClient` decoding allocation | `crates/op-mcp-proxy/src/vertex_grpc.rs:54` | Heap | **Out-Of-Memory (OOM)**: Overrides max decoding message size to 64 MiB on the heap, allowing a massive heap allocation for incoming gRPC responses. |

---

## 2. Critical Vulnerabilities

### Unsafe SIMD-JSON Deserialization on Unpadded Buffers
* **File & Lines**: 
  * `crates/op-mcp-proxy/src/main.rs:110`
  * `crates/op-mcp-proxy/src/cloudaicompanion.rs:533`
  * `crates/op-mcp-proxy/src/cloudaicompanion.rs:551`
  * `crates/op-mcp-proxy/src/cloudaicompanion.rs:616`
  * `crates/op-mcp-proxy/src/cloudaicompanion.rs:651`
* **Severity**: **Critical**
* **Exploitability**: Directly exploitable via standard input or local configuration directories.
* **Description**:
  The application utilizes `simd_json::from_str` and `simd_json::from_slice` via `unsafe` markers on standard library strings and files. For example:
  ```rust
  // crates/op-mcp-proxy/src/main.rs:110
  let req: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut line) }?;
  ```
  The safety contract of `simd-json` specifies that the input buffer *must* be padded with additional bytes (at least `simd_json::PADDING_SIZE`, typically 32 bytes) beyond the end of the payload. Standard library `String` allocations returned by `std::io::BufRead::lines()` or `std::fs::read_to_string` do not guarantee this padding. When parsing maliciously short or structured inputs, the underlying vector instructions will read out-of-bounds memory. This can lead to page faults, segmentation faults (crashing the proxy), or information disclosure (leaking heap remnants).

---

## 3. Schema-as-Code Compliance

### Direct Byte-Offset Memory Structure Modeling
* **File & Lines**: `crates/op-mcp-proxy/src/sled.rs:14-48`
* **Severity**: Medium
* **Description**:
  The database layout and configuration properties are modeled using raw, hardcoded offsets inside a memory-mapped buffer:
  ```rust
  let wg_pubkey     = &bytes[0..32];
  let mutation_index = u64::from_le_bytes(bytes[32..40].try_into().ok()?);
  let is_valid       = bytes[40] != 0;
  let footprint      = &bytes[48..80];
  let nextdns_profile = fixed_str(&bytes[192..208]);
  ```
  This violates the Schema-as-Code discipline. Rather than defining these schemas using declarative, versioned contracts (such as Protocol Buffers or OSCAL), the code relies on fragile, ad-hoc `#[repr(C)]` structure matching. Any misalignment, compiler-enforced padding change, or layout shift in the writer crate will silently corrupt identity, routing, and NextDNS credentials.

### Ad-hoc String Database Migrations
* **File & Lines**: `crates/op-mcp-proxy/src/session.rs:44-66`
* **Severity**: Low
* **Description**:
  The relational schemas for SQLite `sessions` and `wireguard_users` tables are constructed using ad-hoc, inline SQL multi-line strings inside `execute_batch`. These should be externalized to formal migration files or versioned schema management entities.

---

## 4. Performance & Allocation Anomalies

### High-Frequency Cloning of Large JSON Payloads
* **File & Lines**: `crates/op-mcp-proxy/src/direct_llm.rs:172-173`
* **Severity**: Medium
* **Description**:
  ```rust
  let id = req.get("id").cloned().unwrap_or_else(Value::null);
  let params = req.get("params").cloned().unwrap_or_else(Value::null);
  ```
  `req` is a JSON-RPC value parsed from incoming network sessions. Cloning `params` performs a deep heap allocation clone of the entire parameters structure. In LLM interactions, `params` routinely contains extensive conversation histories (the `messages` array) and code context. This results in heavy, short-lived heap allocations, putting severe pressure on the memory allocator and garbage collection cycles under concurrent proxy load.

### Unallocated Vector Collections Inside Parsing Loops
* **File & Lines**: `crates/op-mcp-proxy/src/session.rs:121`
* **Severity**: Low
* **Description**:
  Inside a line processing loop (`for line in stdout.lines()`), the code performs `line.split('\t').collect();` to instantiate a new `Vec<&str>` without any capacity hints or pre-allocation. For long WireGuard peer tables, this triggers multiple re-allocations of the vector.

### High-Frequency String Formats in Prediction Paths
* **File & Lines**:
  * `crates/op-mcp-proxy/src/vertex_grpc.rs:248` (Called on every single streaming token or chunk interaction)
  * `crates/op-mcp-proxy/src/vertex_grpc.rs:106` (gRPC model identifier formatting)
  * `crates/op-mcp-proxy/src/http_server.rs:242` (Formatting unique chat completion UUIDs)
* **Severity**: Low
* **Description**:
  The code repeatedly formats strings in the prediction and response streams:
  ```rust
  let routing = format!("model={}", model_resource.replace('/', "%2F"));
  ```
  This creates high-frequency allocations. Pre-formatting and caching known metadata strings would avoid the CPU allocation overhead.

---

## 5. Security & Quality Concerns

### Local TOCTOU File Verification
* **File & Lines**: `crates/op-mcp-proxy/src/gcloud_auth.rs:72-74`
* **Severity**: Medium
* **Description**:
  The code checks for the existence of `MCP_PROXY_TOKEN_FILE` using `.exists()` and then subsequently opens and reads the file using `std::fs::read_to_string` inside `try_cached_token_file`. This is a classic Time-of-Check to Time-of-Use (TOCTOU) vulnerability. A local attacker who can manipulate symlinks or write to the configuration folder can swap the target file between the verification block and the actual read operation.

### Over-privileged SOCKS5 Network Routing Fallbacks
* **File & Lines**: `crates/op-mcp-proxy/src/main.rs:56-58`
* **Severity**: Medium
* **Description**:
  Routing determinations for Xray SOCKS5 are based on whether the identity file `is_valid` is flagged as true. If the configuration fails or is invalid, the proxy falls back silently to un-proxied, standard local WAN connections. If a tenant mandates traffic segmentation or compliance auditing, silent fallbacks bypass crucial next-hop inspection points, causing compliance violations.