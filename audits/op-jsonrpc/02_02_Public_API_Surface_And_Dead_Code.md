# Production Security and Quality Audit: `op-jsonrpc`

## 1. Executive Summary

This crate provides a control plane JSON-RPC 2.0 server and Open vSwitch Database (OVSDB) client. This audit evaluated the security posture, schema discipline, and code quality of the provided source files. 

### Key Findings
* **No Critical Exploitable Vulnerabilities (RCE/Auth Bypass) Identified**: There are no directly exploitable Remote Code Execution or privilege escalation vectors inside the provided source files.
* **High-Severity Denial of Service (DoS)**: Multiple instances of unbounded stream reading in TCP and Unix socket handlers allow local or remote attackers to crash the control plane daemon via memory exhaustion.
* **Severe Quality Issues (Orphaned Files & Duplicate Code)**: Three files—`nonnet_staging.rs`, `ovsdb_jsonrpc.rs`, and `ovsdb_rpc_call.rs`—are entirely orphaned and omitted from the module tree. They contain broken dependencies (e.g., referencing a non-existent state manager) and duplicate OVSDB client logic.
* **Schema-as-Code Violations**: The crate heavily relies on dynamic JSON structures and runtime type inference to synthesize database schemas rather than consuming versioned, statically defined schemas (e.g., Protobuf or OSCAL).

---

## 2. Public API Surface & Struct Field Analysis

### Public Items Summary
The `op-jsonrpc` library exposes **62** public items within its active module tree (`lib.rs`, `nonnet.rs`, `protocol.rs`, `server.rs`, `ovsdb.rs`).

### Top 10 Most Impactful Public Items
| Item | Type | Location | Impact |
| :--- | :--- | :--- | :--- |
| `JsonRpcServer` | `struct` | `crates/op-jsonrpc/src/server.rs:46` | Main orchestrator managing the JSON-RPC TCP/Unix socket server lifecycle. |
| `NonNetDb` | `struct` | `crates/op-jsonrpc/src/nonnet.rs:44` | Interface and internal state manager for the read-only plugin database. |
| `OvsdbClient` | `struct` | `crates/op-jsonrpc/src/ovsdb.rs:13` | High-level client coordinating Open vSwitch configurations. |
| `JsonRpcRequest` | `struct` | `crates/op-jsonrpc/src/protocol.rs:9` | Strongly typed model representing incoming RPC payloads. |
| `JsonRpcResponse` | `struct` | `crates/op-jsonrpc/src/protocol.rs:41` | Strongly typed model representing outgoing RPC payloads. |
| `JsonRpcServerConfig` | `struct` | `crates/op-jsonrpc/src/server.rs:27` | Defines connection parameters and active backends. |
| `OvsdbClient::transact` | `fn` | `crates/op-jsonrpc/src/ovsdb.rs:149` | Core transaction gateway allowing raw operations on OVS. |
| `NonNetDb::run_server` | `fn` | `crates/op-jsonrpc/src/nonnet.rs:211` | Spawns and manages the underlying Unix socket event loop. |
| `OvsdbClient::monitor_db` | `fn` | `crates/op-jsonrpc/src/ovsdb.rs:449` | Spawns task to stream database updates from OVSDB. |
| `NonNetDb::load_from_plugins` | `fn` | `crates/op-jsonrpc/src/nonnet.rs:91` | Populates database tables dynamically from external plugins. |

### Struct Field Exposure Risks
Several public structs expose their fields directly. While common in data-transfer objects, it presents structural API stability risks and bypasses input validation:

1. **`protocol::JsonRpcRequest` (`crates/op-jsonrpc/src/protocol.rs:9`)**:
   * Fields: `jsonrpc`, `method`, `params`, `id`.
   * **Risk**: Exposing `jsonrpc` as a public mutable `String` allows callers to construct non-compliant payloads (e.g. not matching `"2.0"`). These fields should be read-only via getters, with creation forced through `JsonRpcRequest::new`.
2. **`protocol::JsonRpcError` (`crates/op-jsonrpc/src/protocol.rs:82`)**:
   * Fields: `code`, `message`, `data`.
   * **Risk**: Modifying error fields directly from the calling code can lead to non-standardized error propagation across the system.
3. **`server::JsonRpcServerConfig` (`crates/op-jsonrpc/src/server.rs:27`)**:
   * Fields: `unix_socket`, `tcp_addr`, `ovsdb_enabled`, `nonnet_enabled`.
   * **Risk**: Public fields prevent the introduction of internal validation logic or default fallbacks when config fields are altered post-initialization.

---

## 3. Dead Code & Orphaned Files Audit

### Orphaned Modules Analysis
Three entire files are left in the source directory but are **never declared** as modules in `lib.rs`:
1. `crates/op-jsonrpc/src/nonnet_staging.rs`
2. `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs`
3. `crates/op-jsonrpc/src/ovsdb_rpc_call.rs`

Because they are omitted from the module tree, they are treated as dead code by the compiler and are never compiled into the final binary. Furthermore, `nonnet_staging.rs` imports `crate::state::StateManager`, which does not exist in this crate and would trigger a compilation failure if active.

### Dead Code Table
| Item / File | Type | Location | Recommendation |
| :--- | :--- | :--- | :--- |
| `nonnet_staging.rs` | `File` | `crates/op-jsonrpc/src/nonnet_staging.rs` | **Remove**. Completely redundant staging file containing uncompilable imports and duplicate logic. |
| `ovsdb_jsonrpc.rs` | `File` | `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs` | **Remove**. Duplicate `OvsdbClient` implementation that lacks critical sanitization and connection timeout controls present in `ovsdb.rs`. |
| `ovsdb_rpc_call.rs` | `File` | `crates/op-jsonrpc/src/ovsdb_rpc_call.rs` | **Remove**. Contains a single dangling `rpc_call` function block defined outside of any struct implementation. |
| `#[allow(dead_code)]` on `get_schema` | `Attribute` | `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:44` | **Remove**. Suppresses dead-code warning in an orphaned file. |
| `#[allow(dead_code)]` on `dump_open_vswitch` | `Attribute` | `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:50` | **Remove**. Suppresses dead-code warning in an orphaned file. |

---

## 4. Schema-as-Code Violations

The codebase violates a schema-as-code discipline by expressing data contracts as **ad-hoc structures, untyped JSON values, and dynamic runtime-inferred models**:

1. **Ad-Hoc JSON Schema Generation**:
   * **Location**: `crates/op-jsonrpc/src/nonnet.rs:46` (`empty_nonnet_schema`) and `crates/op-jsonrpc/src/nonnet.rs:105` (`load_from_plugins`).
   * **Violation**: Instead of consuming a versioned Protobuf or OSCAL component definition schema, the database schemas are dynamically constructed as ad-hoc nested objects using `simd_json::json!` macros.
2. **Runtime Type Guessing**:
   * **Location**: `crates/op-jsonrpc/src/nonnet.rs:252` (`infer_type`) and `crates/op-jsonrpc/src/nonnet.rs:232` (`infer_columns`).
   * **Violation**: Schema definitions are dynamically guessed at runtime based on the JSON types of the current row values. If a plugin restarts or changes its payload format, the schema changes implicitly. This dynamic typing bypasses statically checked, versioned contract safety.
3. **Untyped Protocol Envelopes**:
   * **Location**: `crates/op-jsonrpc/src/protocol.rs:11` (`JsonRpcRequest::params`) and `crates/op-jsonrpc/src/protocol.rs:44` (`JsonRpcResponse::result`).
   * **Violation**: Request parameters and response results are transported using `simd_json::OwnedValue` (essentially untyped raw bytes/maps). There is no enforcement of strongly typed data models via versioned serializable schemas.

---

## 5. Security & Quality Vulnerabilities

### [VULN-01] Denial of Service via Unbounded Stream Reading
* **Severity**: **High**
* **Citations**: 
  * `crates/op-jsonrpc/src/nonnet.rs:291`
  * `crates/op-jsonrpc/src/server.rs:215`
  * `crates/op-jsonrpc/src/server.rs:231`
* **Description**:
  The JSON-RPC server handles both Unix socket and TCP streams by reading lines using `BufReader::read_line(&mut line)`. Because there is no constraint on the maximum line length, a malicious client can continuously send bytes without a newline (`\n`). The server will keep allocating memory in the `String` buffer until the system triggers an Out-of-Memory (OOM) crash, disabling the system-level control plane.
* **Remediation**:
  Wrap the incoming stream in a helper that limits the maximum bytes read per line, or use `tokio_util::codec` with a `LengthDelimitedCodec` to set hard limits on frame sizes:
  ```rust
  let mut reader = BufReader::new(reader).take(MAX_ALLOWED_LINE_BYTES);
  ```

---

### [VULN-02] control Plane FD Exhaustion and High Socket Churn
* **Severity**: **Medium**
* **Citations**: 
  * `crates/op-jsonrpc/src/ovsdb.rs:88`
* **Description**:
  The `rpc_call` function connects to the OVSDB socket, transmits a single query, and immediately invokes `stream.shutdown().await` to signal request completion. This design makes the Unix connection single-use, forcing a full socket re-connection for *every* OVSDB query. Under high synchronization loads, this design will cause extreme CPU overhead, socket churn, and rapid File Descriptor (FD) exhaustion, dropping the control plane's connection to Open vSwitch.
* **Remediation**:
  Maintain a single long-lived, multiplexed `UnixStream` connection to the OVSDB socket. Use unique JSON-RPC `id` values to correlate concurrent requests and responses without tearing down the underlying socket connection.

---

### [VULN-03] Implicit Schema Dynamic Drift
* **Severity**: **Medium**
* **Citations**:
  * `crates/op-jsonrpc/src/nonnet.rs:232`
  * `crates/op-jsonrpc/src/nonnet.rs:252`
* **Description**:
  The `infer_columns` implementation dynamically maps raw JSON structures into column schemas. If a plugin yields an empty list or `null` for a key during initialization, `infer_type` returns `"null"` or maps to a blank object. Once the plugin subsequently populates this field with an array or string, the dynamic schema definition shifts. Any running consumers tracking the schema will read incompatible type definitions, causing crash loops or malformed queries.
* **Remediation**:
  Migrate the schema generation away from dynamic type guessing. Force all plugins to register their database contracts statically using Protocol Buffers or defined structural models with specific JSON Schema drafts. Ensure type information is explicitly declared, not dynamically guessed.