# D-Bus & IPC Attack Surface Audit

## Registered D-Bus & IPC Catalog

### 1. D-Bus Interfaces, Methods, and Signals
No native D-Bus interfaces, methods, or signals are registered or implemented in the provided source files. The codebase references `zbus` in its dependencies but does not implement any `#[dbus_interface]` or proxy registration within the evaluated `op-jsonrpc` crate.

### 2. JSON-RPC (IPC) Services & Methods
The crate exposes a JSON-RPC 2.0 interface over Unix domain sockets and TCP sockets via `crates/op-jsonrpc/src/server.rs` and `crates/op-jsonrpc/src/nonnet.rs`.

| Method | Endpoint / Transport | Caller Identity Checked? | Mutates State / Spawns Processes? | Description |
| :--- | :--- | :--- | :--- | :--- |
| `list_dbs` | Unix Socket / TCP | No | No | Lists active database names. |
| `get_schema` | Unix Socket / TCP | No | No | Retrieves database schema information. |
| `transact` | Unix Socket / TCP | No | Yes (OVSDB Proxy) / No (NonNet) | Forwards transaction operations to OVSDB or queries read-only NonNet state. |
| `echo` | Unix Socket / TCP | No | No | Echoes back parameters for diagnostics. |
| `server.info` | Unix Socket / TCP | No | No | Returns JSON-RPC server name and version metadata. |
| `ovsdb.list_dbs` | Unix Socket / TCP | No | No | Proxy call listing databases on the local OVSDB daemon. |
| `ovsdb.get_schema`| Unix Socket / TCP | No | No | Proxy call retrieving schemas from the local OVSDB daemon. |
| `ovsdb.transact` | Unix Socket / TCP | No | **Yes** (Arbitrary Mutation) | Proxy call executing arbitrary mutations on Open vSwitch. |

---

## Security Findings

### [Finding 1] CRITICAL: Unauthenticated Proxy Mutation of Host Network State via `ovsdb.transact`
*   **File**: `crates/op-jsonrpc/src/server.rs:313-334` (invoked via `crates/op-jsonrpc/src/server.rs:163` and `crates/op-jsonrpc/src/server.rs:181`)
*   **Exploitability**: Directly Exploitable. Any local process that can write to `/var/run/op-dbus/jsonrpc.sock` (or network host that can reach the TCP endpoint if enabled) can execute arbitrary OVSDB transaction queries with write permissions.
*   **Description**:
    The JSON-RPC service exposes the `ovsdb.transact` proxy method. This method accepts arbitrary parameters, extracts the list of operations (`ops`), and forwards them directly to `/var/run/openvswitch/db.sock` via `OvsdbClient::transact`:
    ```rust
    "ovsdb.transact" => {
        let params = request.params.as_array();
        if let Some(params) = params {
            ...
            let db = params[0].as_str().unwrap_or("Open_vSwitch");
            let ops = json!(params[1..].to_vec());
            match client.transact(db, ops).await {
                Ok(result) => result,
                Err(e) => { ... }
            }
        }
    }
    ```
    There is no check of client credentials (`SO_PEERCRED` on Unix domain sockets), nor is there any application-layer authorization or validation on the mutations array. Because Open vSwitch operations run with high privilege (typically root / `CAP_NET_ADMIN`), this allows unprivileged callers to manipulate system-wide bridge configurations, delete interfaces, or redirect network traffic.
*   **Remediation**: 
    1. Implement peer credential checks using `tokio::net::UnixStream::peer_cred` to restrict connections to trusted users (e.g., `root` or a dedicated system group).
    2. Restrict proxy execution of `transact` to a hardcoded allowlist of safe operations (e.g., only `"select"` queries), blocking arbitrary write/mutate actions.

---

### [Finding 2] HIGH: Peer Identity Verification Bypass on IPC Transport Listeners
*   **File**: `crates/op-jsonrpc/src/server.rs:104-118` (Unix listener) and `crates/op-jsonrpc/src/server.rs:123-136` (TCP listener)
*   **Exploitability**: Directly Exploitable. Any client establishing a connection to either listener can execute requests with the full privileges of the `op-jsonrpc` daemon.
*   **Description**:
    When accepting connections, the server spawns a connection handler immediately without performing peer authentication or validating credentials:
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
    There is no check on the socket file permissions of `/var/run/op-dbus/jsonrpc.sock` during creation, meaning default permissions could allow any local user to access host network control features.
*   **Remediation**:
    Explicitly set permissions on `/var/run/op-dbus/jsonrpc.sock` to `0660` or `0600` after binding. Verify that only authorized system services can open the socket.

---

### [Finding 3] MEDIUM: Memory Safety Risks from Unsafe Deserialization of Unvalidated Input
*   **File**: `crates/op-jsonrpc/src/server.rs:201` and `crates/op-jsonrpc/src/nonnet.rs:260`
*   **Exploitability**: Hard to exploit directly for remote code execution, but can cause denial of service or undefined behavior if malformed UTF-8/misaligned inputs are processed.
*   **Description**:
    The system uses an unsafe block to deserialize JSON payloads received directly over socket streams:
    ```rust
    while reader.read_line(&mut line).await? > 0 {
        let response = match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) } {
            ...
        }
    }
    ```
    The `simd_json` crate warns that its unsafe interface has strict alignment, padding, and UTF-8 validation requirements. Mutating raw lines read from an untrusted socket and passing them directly into `simd_json::from_str` under an `unsafe` block bypasses compile-time safety guarantees.
*   **Remediation**:
    Use the safe, validated parser interfaces of `simd_json` or switch to `serde_json::from_str`, which handles arbitrary untrusted buffers safely.

---

### [Finding 4] LOW: Lack of Versioned Schema-as-Code Contracts
*   **File**: `crates/op-jsonrpc/src/protocol.rs:13-18` and `crates/op-jsonrpc/src/nonnet.rs:88-124`
*   **Exploitability**: Not directly exploitable; leads to architectural fragility and API drift.
*   **Description**:
    The JSON-RPC service processes data contracts using ad-hoc, untyped, dynamic JSON values rather than versioned, statically defined schemas (e.g., Protocol Buffers or versioned JSON Schema files):
    ```rust
    pub struct JsonRpcRequest {
        pub jsonrpc: String,
        pub method: String,
        #[serde(default)]
        pub params: Value, // Ad-hoc dynamic Value
        pub id: Value,
    }
    ```
    Database updates and schema operations dynamically infer column structures at runtime (e.g., `infer_columns` / `infer_type` in `crates/op-jsonrpc/src/nonnet.rs:104`):
    ```rust
    fn infer_type(value: &Value) -> &'static str {
        if value.is_null() { return "null"; }
        if value.is_bool() { return "boolean"; }
        ...
    }
    ```
    This breaks the Schema-as-Code discipline by allowing arbitrary, dynamically inferred structure shapes to dictate the system interface.
*   **Remediation**:
    Declare versioned Protocol Buffer schema files for the IPC boundary and compile them to typed Rust structures using `prost` to enforce contract compliance at compilation time.