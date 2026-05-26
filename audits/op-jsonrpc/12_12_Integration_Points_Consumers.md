# Production Security & Quality Audit: `op-jsonrpc`

## 1. Workspace Integration Audit

### 1.1 Crates Depending on `op-jsonrpc`
Based on the workspace `Cargo.toml` and `Cargo.lock` files, the following internal crates depend on `op-jsonrpc`:
* **`op-dbus`** (defined in `Cargo.toml:132`, depends on `op-jsonrpc` at `Cargo.toml:166`)
* **`op-dbus-mirror`** (depends on `op-jsonrpc` in `Cargo.lock` under `[[package]] name = "op-dbus-mirror"`)
* **`op-grpc-bridge`** (depends on `op-jsonrpc` in `Cargo.lock` under `[[package]] name = "op-grpc-bridge"`)
* **`op-state`** (depends on `op-jsonrpc` in `Cargo.lock` under `[[package]] name = "op-state"`)
* **`op-web`** (depends on `op-jsonrpc` in `Cargo.lock` under `[[package]] name = "op-web"`)

### 1.2 D-Bus Service Names and Object Paths Registered
No native D-Bus services or object paths are directly registered within the `op-jsonrpc` crate codebase. However, the default Unix domain socket path configured for the JSON-RPC server is:
* **Socket Path**: `/var/run/op-dbus/jsonrpc.sock` (defined in `crates/op-jsonrpc/src/server.rs:42`)

This directory structure indicates integration with the broader `op-dbus` environment.

### 1.3 HTTP / gRPC Endpoints Exposed
The crate does not expose standard HTTP/REST or gRPC endpoints. It exposes JSON-RPC 2.0 endpoints over:
* **Unix Domain Socket**: Configured via `unix_socket` (default: `/var/run/op-dbus/jsonrpc.sock`, `crates/op-jsonrpc/src/server.rs:42`)
* **TCP Socket**: Configured via `tcp_addr` (optional, `crates/op-jsonrpc/src/server.rs:43`)

These transports accept line-delimited JSON-RPC 2.0 requests and expose the following method endpoints:
* **`list_dbs`**: Lists available databases (returns `["OpNonNet"]`).
* **`get_schema`**: Retrieves the schema for a given database.
* **`transact`**: Performs read-only transactions (queries) against the database.
* **`ovsdb.list_dbs`**: Proxies database listing to the local OVSDB socket.
* **`ovsdb.get_schema`**: Proxies schema requests to OVSDB.
* **`ovsdb.transact`**: Proxies transact/mutation operations directly to OVSDB.
* **`server.info`**: Exposes the server's packet/version information.
* **`echo`**: Echoes input parameters for testing.

### 1.4 Cross-Crate Circular Dependency Risk
* **Identified Risk**: `crates/op-jsonrpc/src/nonnet_staging.rs:8` attempts to import `StateManager` using the statement `use crate::state::StateManager;`. 
* **Dependency Loop**: `StateManager` resides in the `op-state` crate (which is verified by `Cargo.toml` workspace members). `op-state` depends directly on `op-jsonrpc` (evidenced in `Cargo.lock`). If `op-jsonrpc` imports `op-state`'s `StateManager` to compile `nonnet_staging.rs`, a direct circular dependency (`op-state` -> `op-jsonrpc` -> `op-state`) is introduced, which prevents compilation.
* **Mitigation**: `nonnet_staging.rs` is currently omitted from `crates/op-jsonrpc/src/lib.rs` modules, but the source file remains in the tree and represents a high quality and refactoring risk.

---

## 2. Schema-As-Code Compliance

This codebase violates the **schema-as-code** discipline by expressing data contracts as ad-hoc structs and unstructured dynamically-inferred JSON values rather than formal, versioned schemas (such as Protocol Buffers or OSCAL).

* **Unstructured Protocol Objects**: `JsonRpcRequest` and `JsonRpcResponse` in `crates/op-jsonrpc/src/protocol.rs:10-53` use raw, untyped `simd_json::OwnedValue` (generic Value type) for their `params` and `result` payloads. This allows arbitrary JSON values to bypass contract validation.
* **On-The-Fly Schema Inference**: Instead of relying on a pre-compiled versioned schema, the database schemas for `OpNonNet` are constructed on the fly by dynamically scanning and guessing types from raw JSON plugin values. This occurs in:
  * `crates/op-jsonrpc/src/nonnet.rs:360-394` (`infer_columns` and `infer_type` map Rust primitives to raw string type descriptors like `"null"`, `"boolean"`, `"integer"`, `"string"`, `"set"`, `"map"`).
  * `crates/op-jsonrpc/src/nonnet_staging.rs:83-113` (duplicate ad-hoc inference mapping).
* **Ad-Hoc JSON Operations**: Database transactions and OVSDB configurations are constructed as inline raw JSON arrays/objects using the `json!` macro (e.g., inside `create_bridge`, `delete_bridge`, and `add_port` in `crates/op-jsonrpc/src/ovsdb.rs:197-340`). No type-safe schemas or validated models exist for these messages.

---

## 3. Production Quality & Security Audit

### 3.1 Definite Critical Vulnerability: Out-of-Bounds Read & Memory Corruption in JSON-RPC Parsing

#### Finding: Memory Corruption / Out-of-Bounds Read in `simd_json::from_str` Unpadded Buffer Parsing
* **Location**: `crates/op-jsonrpc/src/nonnet.rs:256` and `crates/op-jsonrpc/src/server.rs:219`
* **Severity**: **Critical** (Directly exploitable)
* **Code Reference**:
  ```rust
  let response = match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) } {
  ```

#### Analysis
`simd-json` relies on SIMD hardware vector instructions (AVX2/SSE) to parse JSON in parallel. This design strictly requires the input buffer to be padded with `simd_json::PADDING` (typically 32 or 64 bytes) of extra addressable memory beyond the end of the payload string. 

In both the `NonNetDb` connection handler (`nonnet.rs:256`) and the `JsonRpcServerConnection` (`server.rs:219`), the input buffer `line` is populated directly via a tokio `BufReader::read_line` call:
```rust
while reader.read_line(&mut line).await? > 0 {
```
The resulting `line` is a standard `std::string::String` with an arbitrary capacity that matches its exact character content. No padding bytes are appended to the string before calling the unsafe `simd_json::from_str::<Value>(line.as_mut_str())`.

#### Exploitability
An attacker connected to the JSON-RPC Unix socket or TCP port can send a malformed JSON request that lies exactly on the boundary of the allocated string page. When `simd-json` attempts to read 32-byte vectors past the end of the string slice:
1. **Denial of Service**: If the unpadded read crosses a page boundary into unmapped virtual memory, the operating system kernel immediately raises a segmentation fault, crashing the entire control plane process.
2. **Information Disclosure / Heap Memory Leak**: The parser reads adjacent memory on the heap. If the read memory happens to contain sensitive configurations or key material, and the parsed elements are reflected back to the client (such as using the standard `"echo"` JSON-RPC method exposed at `server.rs:260`), adjacent heap contents can be disclosed.

---

### 3.2 High Security & Quality Findings

#### Finding 1: Command Injection & Insufficient Validation on Bridge and Port Names
* **Location**: `crates/op-jsonrpc/src/ovsdb.rs:197`, `crates/op-jsonrpc/src/ovsdb.rs:253`, and `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:114`
* **Severity**: **High**
* **Code Reference**:
  ```rust
  pub async fn create_bridge(&self, name: &str) -> Result<()> {
      let safe_name = Self::sanitize_ref(name);
      let bridge_uuid = format!("bridge_{}", safe_name);
      ...
      let operations = json!([
          {
              "op": "insert",
              "table": "Bridge",
              "row": {
                  "name": name, // <--- Raw unsanitized name inserted
  ```

#### Analysis
The `sanitize_ref` function is called on bridge and port names, but *only* to generate the internal transactional `named-uuid` references (e.g. `bridge_uuid`, `port_uuid`). The raw, unsanitized `name` and `port` inputs are inserted directly into the OVSDB `Bridge`, `Port`, and `Interface` tables. 

If an attacker gets a malicious string (e.g. containing spaces, quotes, newlines, or shell metacharacters like `; command ;`) written to the OVSDB, and downstream processes or scripts retrieve these names and run them inside shell commands (such as `ovs-vsctl` or `ip link`), shell command injection or local privilege escalation will occur. Bridge names must be strictly validated to be alphanumeric.

---

#### Finding 2: Unconditional OVSDB Client Deadlock / Absolute Response Timeout
* **Location**: `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:17-21` and `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:39-42`
* **Severity**: **High**
* **Code Reference**:
  ```rust
  // Inside ovsdb_rpc_call.rs:
  let mut response_buf = Vec::new();
  tokio::time::timeout(self.timeout, tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut response_buf))
      .await
  ```

#### Analysis
In `ovsdb_rpc_call.rs`, the OVSDB client sends a command over `UnixStream` and immediately reads the response using `read_to_end`. However, it does *not* close the write half of the socket first. OVSDB keeps database connections open for persistent JSON-RPC monitoring and transactions. Consequently, the stream never hits EOF. 

The `read_to_end` call will block indefinitely until the 30-second `self.timeout` expires. Every database call using this module will take exactly 30 seconds to run and then fail with a timeout error, entirely breaking database integration.

Similarly, in `ovsdb_jsonrpc.rs:39`, `read_line` is used on the socket. As noted in the comments of `ovsdb.rs:62`, OVSDB does not guarantee newline-terminated responses. If OVSDB returns a response without a trailing newline, `read_line` blocks until it times out.

---

#### Finding 3: Read-Only Transaction Bypass in Staging Handler
* **Location**: `crates/op-jsonrpc/src/nonnet_staging.rs:116-132`
* **Severity**: **High**
* **Code Reference**:
  ```rust
  async fn handle_transact_select(state: &Arc<StateManager>, ops: Value) -> Result<Value> {
      let mut out = Vec::new();
      ...
      if let Some(arr) = ops.as_array() {
          for op in arr {
              let table = op.get("table").and_then(|v| v.as_str()).unwrap_or("");
              ...
              let val = plugins.get(table).cloned().unwrap_or(json!(null));
              let rows = rows_from_plugin_value(&val);
              out.push(json!({"rows": rows}));
          }
      }
      Ok(json!(out))
  }
  ```

#### Analysis
The staging JSON-RPC server handles the OVSDB `transact` protocol. However, the transact select handler never checks if the operation is actually a `"select"` command (`op.get("op") == Some("select")`). It processes *any* transaction operation containing a `table` parameter as a select query and returns the table contents. This bypasses protocol safety invariants, returning read-only data for operations intended to write, delete, or mutate.

---

### 3.3 Medium Security & Quality Findings

#### Finding 4: Unbounded Thread Spawning / Denial of Service (DoS)
* **Location**: `crates/op-jsonrpc/src/nonnet.rs:191-197` and `crates/op-jsonrpc/src/server.rs:125-132`
* **Severity**: **Medium**
* **Code Reference**:
  ```rust
  loop {
      let (stream, _) = listener.accept().await?;
      let server = self.clone_for_connection();

      tokio::spawn(async move {
          if let Err(e) = server.handle_unix_connection(stream).await {
              debug!("Connection error: {}", e);
          }
      });
  }
  ```

#### Analysis
Incoming socket connections are accepted and spawned using `tokio::spawn` immediately without any connection limits, rate-limiting, or semaphores. An attacker can exhaust available file descriptors (EMFILE) and memory by opening thousands of concurrent TCP or UNIX socket connections, easily knocking the control plane offline.

---

#### Finding 5: Defective Imports and Non-Compiling Code in Tree
* **Location**: `crates/op-jsonrpc/src/nonnet_staging.rs:8`
* **Severity**: **Medium**
* **Code Reference**:
  ```rust
  use crate::state::StateManager;
  ```

#### Analysis
The source file `nonnet_staging.rs` is present in the crate's `src` folder but is not registered in `lib.rs`. It contains a broken module import pointing to a non-existent internal path (`crate::state`). This code is broken and cannot compile as-is, representing a severe regression in repository code cleanliness and maintainability.

---

### 3.4 Low Security & Quality Findings

#### Finding 6: Missing Socket File Cleanup on Shutdown
* **Location**: `crates/op-jsonrpc/src/nonnet.rs:184` and `crates/op-jsonrpc/src/server.rs:118`
* **Severity**: **Low**
* **Code Reference**:
  ```rust
  if path.exists() {
      tokio::fs::remove_file(path).await.ok();
  }
  ```

#### Analysis
While the code correctly cleans up existing socket files *before* binding, there is no cleanup mechanism (such as implementing the `Drop` trait or listening for termination signals) to delete the sockets on shutdown. This leaves dead UNIX domain sockets on the host filesystem when the process terminates.