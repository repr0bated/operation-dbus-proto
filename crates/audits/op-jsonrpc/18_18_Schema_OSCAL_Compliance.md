### 1. Schema-as-Code Audit

The following table details data contracts, models, and interfaces in the `op-jsonrpc` crate that are expressed as ad-hoc Rust structs, dynamically inferred formats, or untyped JSON structures instead of versioned schemas (such as Protocol Buffers or official OSCAL component definitions).

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `JsonRpcRequest` | Rust Struct | `crates/op-jsonrpc/src/protocol.rs:8` | No | Expressed as an ad-hoc Rust struct. Uses untyped `simd_json::OwnedValue` for `params` and `id`, bypassing strict schemas. |
| `JsonRpcResponse` | Rust Struct | `crates/op-jsonrpc/src/protocol.rs:36` | No | Expressed as an ad-hoc Rust struct. Uses untyped `simd_json::OwnedValue` for `result` and `id`, making response payloads schema-less. |
| `JsonRpcError` | Rust Struct | `crates/op-jsonrpc/src/protocol.rs:84` | No | Expressed as an ad-hoc Rust struct. Uses untyped `simd_json::OwnedValue` for context `data`. |
| `NonNetUpdate` | Rust Struct | `crates/op-jsonrpc/src/nonnet.rs:21` | No | Internal database update event containing untyped rows (`Vec<simd_json::OwnedValue>`) instead of versioned structures. |
| `NonNetChanged` | Rust Struct | `crates/op-jsonrpc/src/nonnet.rs:29` | No | Event notification representing table-level modifications without a versioned schema. |
| `Plugins State Schema` | Schema Inference | `crates/op-jsonrpc/src/nonnet.rs:109` | No | Table schemas are dynamically inferred at runtime via `infer_columns` from arbitrary plugin value shapes rather than referencing versioned schema definitions. |
| `OVSDB Mutations` | Ad-Hoc JSON Arrays | `crates/op-jsonrpc/src/ovsdb.rs:214` | No | Operations like `create_bridge` and `add_port` (also at `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs:136`) construct ad-hoc OVSDB JSON arrays manually via `simd_json::json!` instead of using typed, compiled message schemas. |

---

### 2. OSCAL Coverage Audit

The following table maps implemented security-relevant operations, interfaces, and hardcoded logic to NIST SP 800-53 security control areas and identifies gaps where these controls are implemented in code but lack associated machine-readable OSCAL compliance mappings.

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **AC-3: Access Enforcement** (UNIX Sockets) | `crates/op-jsonrpc/src/server.rs:136`, `crates/op-jsonrpc/src/nonnet.rs:244` | None | UNIX sockets are created and bound (including `crates/op-jsonrpc/src/nonnet_staging.rs:23`) without explicit permissions setting (`chmod`). This inherits default umask settings, potentially allowing local privilege escalation if non-root processes can access the socket. There is no OSCAL Component Definition mapping the system boundary permissions of these sockets. |
| **IA-2: Identification and Authentication** (TCP Sockets) | `crates/op-jsonrpc/src/server.rs:163` | None | The TCP listener accepts connections on a configured network port and routes request processing directly to `handle_request`. This exposes OVSDB write capability and core state queries *completely unauthenticated* over the network. There is no OSCAL mapping or requirement documentation mandating network-level authentication on this boundary. |
| **SC-8: Transmission Confidentiality and Integrity** | `crates/op-jsonrpc/src/server.rs:163` | None | The TCP JSON-RPC server handles system configurations and proxies sensitive state over raw, unencrypted TCP streams with no transport encryption (TLS) configuration. No OSCAL Component Definition documents or enforces transport confidentiality for this communication path. |
| **AC-3: Access Enforcement** (Hardcoded Policy) | `crates/op-jsonrpc/src/nonnet.rs:434` | None | NonNet database modification transactions ("insert", "update", "delete", "mutate") are rejected via hardcoded handler logic that returns a static read-only error. This hardcoded rule bypasses centralized, machine-readable OSCAL policies (such as OPA/Rego constraints) and cannot be verified dynamically by automated compliance scanners. |
| **AU-2: Event Logging** (Mutations) | `crates/op-jsonrpc/src/ovsdb.rs:260` | None | System configuration modifications (such as OVS bridge creations and port updates) are logged as unstructured application tracing messages via `tracing::info!`. There is no structured audit logging schema mapped to security controls inside an OSCAL System Security Plan (SSP). |

---

### 3. Recommendations for Major and Critical Gaps

#### [CRITICAL] Memory Safety & Undefined Behavior: Out-of-Bounds Reads in `simd_json::from_str`
* **Finding Citation**: `crates/op-jsonrpc/src/nonnet.rs:290`, `crates/op-jsonrpc/src/server.rs:271`, `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:25`, `crates/op-jsonrpc/src/nonnet_staging.rs:40`
* **Vulnerability Details**: `simd-json` is optimized to perform high-speed SIMD vector instructions (e.g., reading 32 or 64 bytes at a time). To prevent reading past allocated boundaries, `simd-json` strictly requires that input byte slices contain at least `simd_json::SIMDJSON_PADDING` (typically 32 bytes) of allocated trailing padding. 
  In the cited code blocks, the server reads data from TCP or UNIX socket streams via standard IO (`read_line` or `read_to_end`) and passes the mutable slice of standard `String` straight to `unsafe { simd_json::from_str(...) }` or `simd_json::from_slice(...)`. Because a standard Rust `String` populated via network buffers does not guarantee the required trailing padding space, these unsafe parsing operations can cause the SIMD vector registers to perform **out-of-bounds memory reads**. This leads to undefined behavior, memory leaks of adjacent heap chunks, or segmentation faults (Denial of Service).
* **Remediation**:
  To safely parse JSON payloads using `simd-json`, you must copy or resize the input buffer to guarantee trailing padding bytes before passing it to the deserializer:
  ```rust
  // Safe padding remediation pattern:
  let mut bytes = line.into_bytes();
  bytes.resize(bytes.len() + simd_json::SIMDJSON_PADDING, 0);
  let response: Value = unsafe { simd_json::from_slice(&mut bytes)? };
  ```
  Alternatively, replace the unsafe `simd-json` calls with the safe standard library `serde_json::from_str`, which does not require raw block alignment or buffer padding.

---

#### [CRITICAL] Performance Denial-of-Service (DoS) / Thread Blocking in `rpc_call`
* **Finding Citation**: `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:18`
* **Vulnerability Details**: The direct JSON-RPC client function `rpc_call` performs standard network reads using `tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut response_buf)`. 
  Because OVSDB is a persistent server, it holds the UNIX socket connection open across multiple requests and does not signal EOF (End of File). As a result, `read_to_end` will **block indefinitely** waiting for the peer to close the connection, only resolving when the outer `self.timeout` (30 seconds) triggers. This forces every single synchronous JSON-RPC request to take exactly 30 seconds to complete, rendering the client unusable and causing localized Denial of Service for any dependent services.
* **Remediation**:
  OVSDB JSON-RPC requests are newline-terminated payloads (RFC 7047). Modify the read implementation to read exactly one line or frame instead of attempting to read until connection EOF:
  ```rust
  let mut reader = BufReader::new(stream);
  let mut response_line = String::new();
  tokio::time::timeout(self.timeout, reader.read_line(&mut response_line))
      .await
      .context("OVSDB response timeout")??;
  ```

---

#### [MAJOR] Missing Authorization & Transport Confidentiality on Network Bindings (OSCAL AC-3 / SC-8)
* **Finding Citation**: `crates/op-jsonrpc/src/server.rs:163`
* **Vulnerability Details**: When configured with a `tcp_addr`, the JSON-RPC server instantiates an unencrypted `TcpListener` that accepts incoming traffic and forwards commands directly to the core state handlers. Because the protocol has zero mechanisms for identity assertion, authorization checks, or cryptographic transport security (TLS), any network-adjacent attacker can perform arbitrary state reads or execute database write operations (e.g. OVSDB mutations), causing immediate control plane compromise.
* **Remediation**:
  1. **Enforce Mutual TLS (mTLS)**: If network communication is required, wrap the `TcpStream` using a secure TLS connection layer (using `tokio-rustls` and a validated PKI CA structure) to enforce mandatory client certificate validation before request processing begins.
  2. **Access Enforcement**: Implement authentication tokens (e.g. signed cryptographically bound local tokens) or mandate that network listeners only bind to localhost (`127.0.0.1`) by default.
  3. **OSCAL Mapping**: Author an OSCAL System Security Plan (SSP) component definition mapping these interfaces to NIST SP 800-53 SC-8 (Transmission Confidentiality) and IA-2 (Identification and Authentication) controls.

---

#### [MAJOR] Local Sockets Created with World-Writable Permissions (OSCAL AC-3)
* **Finding Citation**: `crates/op-jsonrpc/src/server.rs:136`, `crates/op-jsonrpc/src/nonnet.rs:244`
* **Vulnerability Details**: The UNIX sockets are initialized by calling `UnixListener::bind(path)`. In Linux environments, UNIX sockets are governed by standard file system permissions and inherit permissions based on the active process's `umask` (e.g. `0022` or `0002`). Since this daemon runs as a highly privileged user (`root`) to manage OVSDB bridges and physical interfaces, local unprivileged users can read/write to these sockets if directory or file-level permissions are too broad, leading to local privilege escalation.
* **Remediation**:
  Enforce strict permission flags immediately after binding the socket files:
  ```rust
  use std::fs::set_permissions;
  use std::os::unix::fs::PermissionsExt;

  let listener = UnixListener::bind(path)?;
  // Limit access to owner and group only (mode 0660 / rw-rw----)
  set_permissions(path, std::fs::Permissions::from_mode(0o660))?;
  ```

---

#### [MAJOR] Lack of Versioned Schema Definition for JSON-RPC Messages (Schema-as-Code Compliance)
* **Finding Citation**: `crates/op-jsonrpc/src/protocol.rs:8`, `crates/op-jsonrpc/src/protocol.rs:36`
* **Vulnerability Details**: The control plane interfaces rely on manually typed, dynamic structures utilizing `simd_json::OwnedValue` to pass internal states. Changes to the underlying network plugin data schemas or database structures cannot be structurally checked at build-time, introducing potential runtime deserialization crashes and preventing standard schema evolution checks.
* **Remediation**:
  1. Migrate all RPC data structures and system events to schema-defined Protobuf (`.proto`) models.
  2. Implement build-time generation using `prost-build` and compile-time API validation.
  3. Document all service APIs in an OSCAL component-definition file specifying structural API request models to ensure compliance.