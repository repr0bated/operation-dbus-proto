# Production Security and Quality Audit: op-jsonrpc

This document contains a comprehensive security and quality audit of the `op-jsonrpc` crate. The audit was conducted with a strict focus on memory safety, system control plane security, resource exhaustion, and adherence to workspace architectural standards (specifically "schema-as-code" disciplines and OSCAL alignment).

---

## 1. Prioritised Risk Register

| Severity | Issue | Evidence (file:line) | Recommendation |
| :--- | :--- | :--- | :--- |
| **Critical** | Undefined Behavior (UB) and Memory Corruption via Unpadded String Mutation in `simd_json` | `crates/op-jsonrpc/src/nonnet.rs:271`<br>`crates/op-jsonrpc/src/server.rs:254`<br>`crates/op-jsonrpc/src/ovsdb.rs:103`<br>`crates/op-jsonrpc/src/ovsdb.rs:114`<br>`crates/op-jsonrpc/src/ovsdb.rs:449`<br>`crates/op-jsonrpc/src/ovsdb_rpc_call.rs:23` | Replace unsafe in-place mutation of unpadded strings with a padded buffer (`simd_json::to_vec`), or migrate response parsing to `serde_json`. |
| **High** | Unauthenticated Remote Control Plane Access via TCP Listener | `crates/op-jsonrpc/src/server.rs:173`<br>`crates/op-jsonrpc/src/server.rs:229` | Enforce TLS wrapping, mutual authentication (mTLS), or restrict socket bindings exclusively to local loopback/Unix sockets. |
| **High** | Unbounded Buffer Allocation / Denial of Service via Line-Oriented TCP/Unix Readers | `crates/op-jsonrpc/src/nonnet.rs:276`<br>`crates/op-jsonrpc/src/server.rs:219`<br>`crates/op-jsonrpc/src/server.rs:234`<br>`crates/op-jsonrpc/src/nonnet_staging.rs:34` | Implement a custom token/line limit layer or use `tokio_util::codec::LengthDelimitedCodec` to enforce maximum frame sizes. |
| **High** | Insecure Default Unix Socket Permissions and Lack of Access Controls | `crates/op-jsonrpc/src/server.rs:48`<br>`crates/op-jsonrpc/src/nonnet.rs:234`<br>`crates/op-jsonrpc/src/nonnet_staging.rs:19` | Explicitly restrict Unix socket file permissions to `0600` or `0660` using `chmod` after binding, and configure socket owner/group. |
| **High** | Protocol/Structure Injection in Native OVSDB Client | `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:157-159`<br>`crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:198-199` | Sanitize all inputs integrated into identifier fields (such as `uuid-name`) or enforce strict validation matching `[a-zA-Z_][a-zA-Z0-9_]*`. |
| **High** | File Descriptor Exhaustion and Socket Churn under High Load | `crates/op-jsonrpc/src/server.rs:290`<br>`crates/op-jsonrpc/src/ovsdb.rs:44` | Refactor the server connection pipeline to share a long-lived, pooled OVSDB connection rather than instantiating a client per request. |
| **High** | Ad-hoc Data Contracts and Lack of Versioned Schema-as-Code | `crates/op-jsonrpc/src/nonnet.rs:105`<br>`crates/op-jsonrpc/src/nonnet.rs:439`<br>`crates/op-jsonrpc/src/nonnet_staging.rs:105` | Migrate unstructured `simd_json::OwnedValue` to versioned, compile-time Protocol Buffers structures with strictly defined schemas. |
| **Medium** | Deadlock and Request Stalling via Event Broadcast inside Write Lock Guard | `crates/op-jsonrpc/src/nonnet.rs:99`<br>`crates/op-jsonrpc/src/nonnet.rs:143` | Release the `state` write lock before triggering synchronous broadcast sends to prevent subscriber locks from blocking the writer. |

---

## 2. In-Depth Vulnerability Analysis

### 2.1. Critical: Undefined Behavior & Memory Corruption via Unpadded `simd_json` Mutation

#### Description
`simd-json` is a high-performance JSON parser that relies on SIMD hardware vector instructions (AVX2, Neon, etc.) to perform destructive parsing directly inside the input byte slice. To avoid segfaults and buffer overreads during vector loads, **`simd-json` strictly requires its input buffers to be mutable and padded with at least `simd_json::SIMDJSON_PADDING` bytes (typically 32 or 64 bytes) past the end of the logical payload.**

Throughout `op-jsonrpc`, input strings read from sockets are converted into mutable slices via `.as_mut_str()` and passed directly to `simd_json::from_str` within `unsafe` blocks. Standard Rust `String` instances and slices returned by `as_mut_str()` are allocated exactly to their length/capacity boundaries without guaranteed padding. 

When `simd_json` attempts to parse these unpadded buffers, the SIMD load instructions read past the allocation boundary. This leads to:
1. **Segmentation Faults:** If the string boundary coincides with a page boundary (4KB aligned), reading into the next unmapped page causes an immediate crash.
2. **Information Disclosure:** If the subsequent memory contains sensitive keys, tokens, or system configurations, these bytes may be ingested by the parser or cause invalid parsing logic.
3. **Memory Corruption:** Destructive writing by `simd_json` can overwrite adjacent heap memory.

#### Evidence
- **`crates/op-jsonrpc/src/nonnet.rs:271`**:
  ```rust
  let response = match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) }
  ```
- **`crates/op-jsonrpc/src/server.rs:254`**:
  ```rust
  match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) }
  ```
- **`crates/op-jsonrpc/src/ovsdb.rs:103` & `ovsdb.rs:114`**:
  ```rust
  let value = unsafe { simd_json::from_str::<Value>(payload.as_mut_str()) }
  ```
- **`crates/op-jsonrpc/src/ovsdb.rs:449`**:
  ```rust
  if let Ok(update) = unsafe { simd_json::from_str::<Value>(line_clone.as_mut_str()) }
  ```
- **`crates/op-jsonrpc/src/ovsdb_rpc_call.rs:23`**:
  ```rust
  let response: Value = unsafe { simd_json::from_str(&mut response_str)? };
  ```

#### Remediation
Either migrate the network parsing layer to safe, standard parsers such as `serde_json`, or explicitly construct a padded buffer using `simd_json::to_vec` before invoking parsing methods.

*Example remediation for `server.rs`:*
```rust
// Replace unsafe parsing with safe parsing:
match serde_json::from_str::<JsonRpcRequest>(&line) {
    Ok(request) => self.handle_request(request).await,
    Err(e) => { ... }
}
```

---

### 2.2. High: Unauthenticated Remote Control Plane Access via TCP Listener

#### Description
The JSON-RPC server is designed to orchestrate deep-level control plane operations, including OVSDB transaction proxying and bridge/port configuration. However, when configured with `tcp_addr`, the server binds a TCP port and exposes these administrative interfaces without any transport-layer security (TLS/mTLS) or application-layer authentication.

An attacker with network routing access to the bound IP address can send raw JSON-RPC strings over raw TCP sockets to execute arbitrary operations. For example, they can issue `ovsdb.transact` commands or write data, leading to full control over Open vSwitch and local networking state.

#### Evidence
- **`crates/op-jsonrpc/src/server.rs:173`**:
  ```rust
  async fn run_tcp(&self, addr: &str) -> Result<()> {
      let listener = TcpListener::bind(addr)
          .await
          .context("Failed to bind TCP socket")?;
      ...
  ```
- **`crates/op-jsonrpc/src/server.rs:229`**:
  ```rust
  async fn handle_tcp_connection(&self, stream: TcpStream) -> Result<()> {
      // Processes lines and handles requests with no auth checks
  ```

#### Remediation
1. **Disable TCP by Default:** Enforce binding to local Unix Domain Sockets or loopback interfaces (`127.0.0.1`) only.
2. **Mutual TLS (mTLS):** If remote access is strictly required, require connections to be wrapped via `tokio-rustls` and validate client certificates against a trusted CA.
3. **Authorization Headers:** Implement token-based authorization (e.g., Bearer tokens) within the JSON-RPC handling logic.

---

### 2.3. High: Unbounded Buffer Allocation / Denial of Service (DoS)

#### Description
The Unix socket and TCP socket connection handlers read incoming commands using `tokio::io::BufReader::read_line`. This method reads bytes from the stream into an internal buffer (`line`) continuously until a newline character (`\n`) is encountered.

If a malicious client establishes a connection and streams bytes infinitely without sending a newline, the server will continue to allocate memory on the heap to grow the string. This will rapidly exhaust host memory, triggering the Linux Out-Of-Memory (OOM) killer and crashing the system.

#### Evidence
- **`crates/op-jsonrpc/src/nonnet.rs:276`**:
  ```rust
  while reader.read_line(&mut line).await? > 0 {
  ```
- **`crates/op-jsonrpc/src/server.rs:219`**:
  ```rust
  while reader.read_line(&mut line).await? > 0 {
  ```
- **`crates/op-jsonrpc/src/server.rs:234`**:
  ```rust
  while reader.read_line(&mut line).await? > 0 {
  ```
- **`crates/op-jsonrpc/src/nonnet_staging.rs:34`**:
  ```rust
  while reader.read_line(&mut line).await? > 0 {
  ```

#### Remediation
Replace `read_line` with a length-limited reader. This can be accomplished using `tokio_util::codec` with a configured maximum frame size.

*Example:*
```rust
use tokio_util::codec::{Decoder, LinesCodec};

let mut reader = LinesCodec::new_with_max_length(65536).framed(reader);
while let Some(line_result) = reader.next().await {
    let line = line_result?;
    // Process line safely
}
```

---

### 2.4. High: Insecure Default Unix Socket Permissions

#### Description
When Unix sockets are initialized, they are created in the filesystem. By default, unless restricted by parent directory structures or specific permission configuration, these sockets may inherit loose permissions (such as `0777` or `0666` modified by umask).

Because `/var/run/op-dbus/jsonrpc.sock` allows full administration of system networking interfaces (e.g., modifying bridges), allowing unprivileged local users to write to this socket exposes the system to local privilege escalation (LPE) and local denial of service.

#### Evidence
- **`crates/op-jsonrpc/src/server.rs:48`**:
  ```rust
  unix_socket: Some("/var/run/op-dbus/jsonrpc.sock".to_string()),
  ```
- **`crates/op-jsonrpc/src/nonnet.rs:234`**:
  ```rust
  pub async fn run_server(&self, socket_path: &str) -> Result<()> {
  ```

#### Remediation
After binding a Unix Domain Socket, explicitly restrict file permissions using `std::fs::set_permissions` or standard `libc` calls to ensure only the designated running user (such as `root` or `op-dbus`) has read/write privileges.

```rust
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;

let listener = UnixListener::bind(path)?;
std::fs::set_permissions(path, Permissions::from_mode(0o600))?;
```

---

### 2.5. High: Protocol/Structure Injection in Native OVSDB Client

#### Description
In the `ovsdb_jsonrpc` client, user-provided inputs such as `bridge_name` and `port_name` are used directly to format dynamic identifier keys like `bridge_uuid`, `port_uuid`, and `iface_uuid`. These generated keys are subsequently inserted into OVSDB JSON-RPC transaction payloads as `uuid-name` parameters.

According to RFC 7047, OVSDB `uuid-name` variables must be valid RFC-compliant identifiers. In `ovsdb_jsonrpc.rs`, no sanitization is applied. An attacker-controlled bridge name or port name containing whitespace, quotation marks, or JSON-RPC delimiters can break the structured transaction payload, causing critical parsing failure in the Open vSwitch daemon, corrupting transactions, or inducing unpredictable state changes.

#### Evidence
- **`crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:157-159`**:
  ```rust
  let bridge_uuid = format!("bridge-{}", bridge_name);
  let port_uuid = format!("port-{}", bridge_name);
  let iface_uuid = format!("iface-{}", bridge_name);
  ```
- **`crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:198-199`**:
  ```rust
  let port_uuid = format!("port-{}", port_name);
  let iface_uuid = format!("iface-{}", port_name);
  ```

#### Remediation
Ensure all dynamic naming identifiers are sanitized to contain only alphanumeric characters or underscores, analogous to the `sanitize_ref` function defined in `ovsdb.rs`.

```rust
fn sanitize_ref(input: &str) -> String {
    input
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
```

---

### 2.6. High: File Descriptor Exhaustion via High-Frequency Socket Churn

#### Description
When proxying incoming requests for OVSDB operations, the JSON-RPC server handles connection requests on a per-invocation basis. For every request processed by `handle_ovsdb_request` (including standard commands like listing databases, retrieving schemas, or executing transactions), a fresh instance of `OvsdbClient` is initialized, which establishes a synchronous Unix socket connection to `/var/run/openvswitch/db.sock` and tears it down immediately after.

Under normal control plane traffic or network configuration synchronization loops, this pattern induces massive socket churn, consumes system ephemerals, and can exhaust the process's file descriptor table limit (`EMFILE`).

#### Evidence
- **`crates/op-jsonrpc/src/server.rs:290-291`**:
  ```rust
  async fn handle_ovsdb_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
      let client = OvsdbClient::new();
  ```
- **`crates/op-jsonrpc/src/ovsdb.rs:44-46`**:
  ```rust
  async fn rpc_call(&self, method: &str, params: Value) -> Result<Value> {
      let mut stream = UnixStream::connect(&self.socket_path)
          .await
          .context("Failed to connect to OVSDB socket")?;
  ```

#### Remediation
Refactor the architecture to utilize a persistent, shared, and thread-safe connection pool or a long-lived multiplexed `UnixStream` connection managed across requests.

---

### 2.7. Medium: Deadlocks and Request Stalling via Event Broadcast inside Write Lock

#### Description
Within the `NonNetDb` state manager, structural mutations such as loading plugins, updating tables, or inserting tables lock the global state with a write guard (`self.state.write().await`). While this lock is active, the functions execute synchronous event broadcasts to `update_tx` and `watch_tx`.

If any of the active event subscribers try to synchronously inspect the database state upon receiving an event (by calling a function that requests `self.state.read()`), they will block indefinitely. This is because the write lock is still held by the mutation task, resulting in a thread/task deadlock.

#### Evidence
- **`crates/op-jsonrpc/src/nonnet.rs:99-138`**:
  ```rust
  pub async fn load_from_plugins(&self, plugins: &HashMap<String, Value>) {
      let mut state = self.state.write().await;
      ...
      // Broadcast events inside write lock scope:
      let _ = self.update_tx.send(NonNetUpdate { ... });
      let _ = self.watch_tx.send(NonNetChanged { ... });
  }
  ```
- **`crates/op-jsonrpc/src/nonnet.rs:143-162`**:
  ```rust
  pub async fn update_table(&self, name: &str, rows: Vec<Value>) {
      let mut state = self.state.write().await;
      ...
      let _ = self.update_tx.send(NonNetUpdate { ... });
  ```

#### Remediation
Isolate state mutation from notification dispatch. Complete modifications, drop the write guard, and then dispatch broadcast events.

*Example:*
```rust
pub async fn update_table(&self, name: &str, rows: Vec<Value>) {
    let update = {
        let mut state = self.state.write().await;
        state.tables.insert(name.to_string(), rows.clone());
        // build schema...
        NonNetUpdate {
            db_name: NONNET_DB_NAME.to_string(),
            table: name.to_string(),
            rows,
        }
    }; // Lock drops here

    let _ = self.update_tx.send(update);
}
```

---

## 3. Schema-as-Code & Compliance Review

The architectural specification of the parent workspace defines a structured, schema-driven approach to data contracts (Schema-as-Code discipline using Protocol Buffers and OSCAL). 

### 3.1. Ad-hoc Dynamic Schemas
`op-jsonrpc` deviates heavily from this discipline by performing **dynamic, ad-hoc, run-time column and type inference on untyped data payload blobs.**

- **`crates/op-jsonrpc/src/nonnet.rs:105`**:
  ```rust
  let columns = infer_columns(value);
  schema_tables.insert(name.clone(), json!({"columns": columns}));
  ```
- **`crates/op-jsonrpc/src/nonnet.rs:439`**:
  ```rust
  fn infer_columns(value: &Value) -> Value {
      match value {
          Value::Object(map) => { ... }
  ```
- **`crates/op-jsonrpc/src/nonnet_staging.rs:105`**:
  ```rust
  fn build_tables_schema(plugins: &HashMap<String, Value>) -> Value { ... }
  ```

By relying on dynamic inference over raw `simd_json::OwnedValue` objects rather than parsing against compiled, versioned Protocol Buffers or defined schemas, the service introduces several structural risks:
1. **Schema Drift:** Structural shifts in third-party or local plugins silently alter the inferred columns database types, leading to broken downstream integration.
2. **Type Confusion:** The fallback types returned by `infer_type` (e.g., returning string representation "set" or "map") are fragile and prone to false-positive classification when empty objects/arrays are encountered.
3. **Compliance Violations:** There is zero formal OSCAL compliance metadata or structural verification of schemas at boundary ingress points.

### 3.2. Recommendations for Schema Compliance
- Replace unstructured type-inference loops with native Protobuf definitions representing the non-network plugins state structure.
- Validate all incoming database transactions against versioned schema definitions (using standard serialization contracts) instead of raw JSON manipulation.