### Observability Audit

#### 1. Tracing vs. `println!` Macro Counts
A comprehensive search was performed across all audited files to compare structured logging against raw console outputs:

*   **`tracing::debug!`**: 7 occurrences
    *   `crates/op-jsonrpc/src/nonnet.rs:126`
    *   `crates/op-jsonrpc/src/server.rs:150`
    *   `crates/op-jsonrpc/src/server.rs:174`
    *   `crates/op-jsonrpc/src/ovsdb.rs:53`
    *   `crates/op-jsonrpc/src/ovsdb.rs:70`
    *   `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:13`
    *   `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:23`
*   **`tracing::info!`**: 8 occurrences
    *   `crates/op-jsonrpc/src/nonnet.rs:223`
    *   `crates/op-jsonrpc/src/server.rs:144`
    *   `crates/op-jsonrpc/src/server.rs:168`
    *   `crates/op-jsonrpc/src/ovsdb.rs:183`
    *   `crates/op-jsonrpc/src/ovsdb.rs:208`
    *   `crates/op-jsonrpc/src/ovsdb.rs:216`
    *   `crates/op-jsonrpc/src/ovsdb.rs:294`
*   **`tracing::warn!`**: 2 occurrences
    *   `crates/op-jsonrpc/src/nonnet.rs:229`
    *   `crates/op-jsonrpc/src/ovsdb.rs:368`
*   **`tracing::error!`**: 2 occurrences
    *   `crates/op-jsonrpc/src/server.rs:98`
    *   `crates/op-jsonrpc/src/server.rs:109`
*   **`println!` / `eprintln!`**: 0 occurrences

#### 2. Swallowed Errors Without Logging
Several errors are suppressed or completely silenced without being recorded in the trace diagnostics:

*   **Connection Error Silencing**: In `crates/op-jsonrpc/src/nonnet_staging.rs:30`, the spawned connection handler drops the result of `handle_connection` via a blind binding:
    ```rust
    tokio::spawn(async move {
        let _ = handle_connection(st, stream).await;
    });
    ```
    If a staging Unix socket connection fails or errors during stream split/parse, the exception is swallowed without any warning or error logging.
*   **Filesystem Suppression**: Across multiple initialization routines, errors from directory creation and file deletion are suppressed using `.ok()` or blank discards instead of matching on errors and logging system issues:
    *   `crates/op-jsonrpc/src/nonnet.rs:144`: `tokio::fs::create_dir_all(dir).await.ok();`
    *   `crates/op-jsonrpc/src/nonnet.rs:149`: `tokio::fs::remove_file(path).await.ok();`
    *   `crates/op-jsonrpc/src/server.rs:132`: `tokio::fs::create_dir_all(dir).await.ok();`
    *   `crates/op-jsonrpc/src/server.rs:136`: `tokio::fs::remove_file(path).await.ok();`
    *   `crates/op-jsonrpc/src/nonnet_staging.rs:20`: `fs::create_dir_all(dir).await.ok();`
    *   `crates/op-jsonrpc/src/nonnet_staging.rs:23`: `let _ = fs::remove_file(p).await;`

#### 3. Log Leakage of Sensitive Data or Secrets
Debug-level trace logs output raw request and response buffers containing unfiltered payload contents:

*   **OVSDB Full Payload Logging**: In `crates/op-jsonrpc/src/ovsdb.rs:53`, `crates/op-jsonrpc/src/ovsdb.rs:70`, `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:13`, and `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:23`:
    ```rust
    debug!("OVSDB request: {}", request_str);
    debug!("OVSDB response: {}", response_text.trim());
    ```
    If network plugins, bridge configuration database records, SSL certificates, key paths, or user credentials reside within the target database, they are recorded in plain-text output at the `DEBUG` log level.

#### 4. Metrics Instrumentation
*   **No Active Metrics**: The `op-jsonrpc` crate does not currently implement any runtime performance metrics or statistics instrumentation. There are no calls to the `metrics` or `prometheus` crates in the provided source files, despite `prometheus` being present as a workspace dependency in `Cargo.toml`.

---

### Schema-as-Code Violations

The codebase contains ad-hoc data contracts and dynamic structure representations, violating strict schema-as-code principles:

*   **Ad-Hoc Rust Serialization Structures**: In `crates/op-jsonrpc/src/protocol.rs:10` and `crates/op-jsonrpc/src/protocol.rs:39`, the critical client-server exchange frames are modeled using manually coded Rust structures:
    ```rust
    pub struct JsonRpcRequest {
        pub jsonrpc: String,
        pub method: String,
        pub params: Value,
        pub id: Value,
    }
    ```
    These structures are defined locally instead of being derived from a central source-of-truth, such as a Protocol Buffers (`.proto`) schema or an OSCAL metadata declaration.
*   **Runtime Schema Inference**: In `crates/op-jsonrpc/src/nonnet.rs:373` and `crates/op-jsonrpc/src/nonnet.rs:398`, table definitions and database structures are dynamically deduced at runtime from arbitrary `simd_json::OwnedValue` shapes:
    ```rust
    fn infer_columns(value: &Value) -> Value { ... }
    fn infer_type(value: &Value) -> &'static str { ... }
    ```
    This approach bypasses the compile-time guarantees of static contracts, allowing structure drift without explicit version-controlled updates. Similar logic is found in `crates/op-jsonrpc/src/nonnet_staging.rs:88` and `crates/op-jsonrpc/src/nonnet_staging.rs:101`.

---

### Security and Quality Findings

#### CRITICAL: Unbounded Line Allocation leading to OOM Denial of Service
*   **Location**: `crates/op-jsonrpc/src/server.rs:200`, `crates/op-jsonrpc/src/server.rs:215`, `crates/op-jsonrpc/src/nonnet.rs:247`, and `crates/op-jsonrpc/src/nonnet_staging.rs:28`
*   **Impact**: Direct process termination via Out-Of-Memory (OOM) crash.
*   **Description**: The TCP and Unix socket connection loops handle requests by reading input lines using `tokio::io::BufReader::read_line`:
    ```rust
    while reader.read_line(&mut line).await? > 0 {
    ```
    `read_line` allocates memory to the target `String` buffer continuously until a newline character (`\n`) is encountered. Because there is no configuration or programmatic limit on the line size, a remote network actor can stream an infinite sequence of non-newline bytes. This forces the server to continually expand its memory allocation until the host system exhausts available memory and the OS kernel terminates the daemon process.

#### HIGH: RFC 7047 Protocol Violation and OVSDB Transaction Failures
*   **Location**: `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:114-116` and `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:150-151`
*   **Impact**: Predictable transactional failures when adding interfaces or bridges.
*   **Description**: In `ovsdb_jsonrpc.rs`, OVSDB dynamic reference IDs are formatted with hyphens:
    ```rust
    let bridge_uuid = format!("bridge-{}", bridge_name);
    let port_uuid = format!("port-{}", bridge_name);
    let iface_uuid = format!("iface-{}", bridge_name);
    ```
    According to **RFC 7047 Section 5.1**, a `named-uuid` must match the exact regular expression format `[a-zA-Z_][a-zA-Z0-9_]*`. Using hyphens (`-`) inside the UUID reference strings causes the target OVSDB server parser to reject the transaction as invalid.
    Furthermore, if `bridge_name` or `port_name` includes unsanitized whitespace or non-alphanumeric characters, they are embedded directly into the transaction `uuid-name` fields, causing the transaction to fail.

#### LOW: Missing Safety Comments on Unsafe `simd_json` Deserialization
*   **Location**: `crates/op-jsonrpc/src/nonnet.rs:252`, `crates/op-jsonrpc/src/server.rs:242`, `crates/op-jsonrpc/src/ovsdb.rs:88`, `crates/op-jsonrpc/src/ovsdb.rs:99`, `crates/op-jsonrpc/src/ovsdb.rs:411`, and `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:26`
*   **Impact**: Quality degradation and maintainability risk.
*   **Description**: The codebase invokes `simd_json::from_str` within `unsafe` blocks to bypass typical safety checks for performance reasons. However, there are no `# Safety` comments explaining the structural invariants or lifetime constraints of the raw buffers. Under Rust's standard safety guidelines, every `unsafe` block must be documented to justify why the mutation or raw pointer boundary dereference is secure.