# Production Security & Quality Audit Report

## Section 1: Rustdoc and Quality Audit

### 1. Crate-Level Documentation
* **Status**: **Present**
* **File Location**: `crates/op-web/src/lib.rs:1-19`
* **Details**: Crate-level `//!` documentation is present and provides a high-level architecture overview, including an ASCII diagram mapping HTTP routes to their functionalities.

### 2. Sample of 10 Public Items and Doc Comment Status
We sampled 10 public items across the codebase to check for `///` rustdoc presence:

1. **`EnabledGroups` (struct)** 
   * **File & Line**: `crates/op-web/src/groups_admin.rs:33`
   * **Status**: **Missing rustdoc**
2. **`GroupsConfig::new` (associated function)**
   * **File & Line**: `crates/op-web/src/groups_admin.rs:41`
   * **Status**: **Missing rustdoc**
3. **`create_groups_admin_router` (function)**
   * **File & Line**: `crates/op-web/src/groups_admin.rs:118`
   * **Status**: **Missing rustdoc**
4. **`McpRequest` (struct)**
   * **File & Line**: `crates/op-web/src/mcp.rs:50`
   * **Status**: **Missing rustdoc**
5. **`McpResponse` (struct)**
   * **File & Line**: `crates/op-web/src/mcp.rs:59`
   * **Status**: **Missing rustdoc**
6. **`McpError` (struct)**
   * **File & Line**: `crates/op-web/src/mcp.rs:70`
   * **Status**: **Missing rustdoc**
7. **`create_mcp_router` (function)**
   * **File & Line**: `crates/op-web/src/mcp.rs:92`
   * **Status**: **Missing rustdoc**
8. **`jsonrpc_handler` (function)**
   * **File & Line**: `crates/op-web/src/mcp.rs:118`
   * **Status**: **Missing rustdoc**
9. **`CriticalAgentsState` (struct)**
   * **File & Line**: `crates/op-web/src/mcp_agents.rs:125`
   * **Status**: **Missing rustdoc**
10. **`WebServiceRouter` (struct)**
    * **File & Line**: `crates/op-web/src/router.rs:14`
    * **Status**: **Missing rustdoc**

### 3. README.md Presence
* **Status**: **Absent**
* **Details**: No `README.md` is present in the `FILES` section for this crate.

### 4. Public Unsafe Functions and Invariant Documentation
* **Status**: **No Public Unsafe Functions**
* **Details**: There are no public `unsafe fn` declarations defined within the audited files. All unsafe code is encapsulated within safe functions using `unsafe {}` blocks (predominantly for `simd_json::from_str` operations).

---

## Section 2: Schema-as-Code Discipline Audit

The codebase uses ad-hoc structs and unstructured formats for data contracts instead of formal Protocol Buffer schemas or OSCAL templates. Below are the occurrences where data contracts are defined as ad-hoc structs or strings:

### 1. Ad-Hoc REST Request/Response Models
The REST endpoint payloads are defined as native Rust structs with `serde::Deserialize` or `serde::Serialize` instead of being generated from a single-source-of-truth Protobuf schema:
* `SaveProfileRequest` — `crates/op-web/src/groups_admin.rs:198`
* `AddNetworkRequest` — `crates/op-web/src/groups_admin.rs:241`
* `SpawnAgentRequest` — `crates/op-web/src/handlers/agents.rs:37`
* `ChatRequest` — `crates/op-web/src/handlers/chat.rs:24`
* `CreateSessionRequest` — `crates/op-web/src/handlers/chat.rs:34`
* `ChatResponse` — `crates/op-web/src/handlers/chat.rs:39`
* `DashboardMetrics` — `crates/op-web/src/handlers/dashboard.rs:13`
* `HealthResponse` — `crates/op-web/src/handlers/health.rs:10`
* `LlmStatusResponse` — `crates/op-web/src/handlers/llm.rs:14`
* `LlmProvidersResponse` — `crates/op-web/src/handlers/llm.rs:20`
* `SwitchModelRequest` — `crates/op-web/src/handlers/llm.rs:74`
* `SwitchProviderRequest` — `crates/op-web/src/handlers/llm.rs:100`
* `LogEntry` — `crates/op-web/src/handlers/logs.rs:22`
* `SetAgentsRequest` — `crates/op-web/src/handlers/mcp.rs:37`
* `MemoryQuery` — `crates/op-web/src/handlers/mcp.rs:46`
* `OpenClawChatRequest` — `crates/op-web/src/handlers/openclaw.rs:136`
* `SignupRequest` — `crates/op-web/src/handlers/privacy.rs:22`
* `SignupResponse` — `crates/op-web/src/handlers/privacy.rs:27`
* `VerifyResponse` — `crates/op-web/src/handlers/privacy.rs:43`
* `StatusResponse` — `crates/op-web/src/handlers/privacy.rs:52`
* `SetCredentialsRequest` — `crates/op-web/src/handlers/privacy.rs:60`
* `SetCredentialsResponse` — `crates/op-web/src/handlers/privacy.rs:69`
* `UserResponse` — `crates/op-web/src/handlers/users.rs:13`
* `VpnStatus` — `crates/op-web/src/handlers/vpn.rs:13`
* `VpnConnection` — `crates/op-web/src/handlers/vpn.rs:28`
* `VpnConfig` — `crates/op-web/src/handlers/vpn.rs:40`
* `MailStatus` — `crates/op-web/src/handlers/mail.rs:11`
* `MailQueueItem` — `crates/op-web/src/handlers/mail.rs:19`

### 2. Ad-Hoc JSON-RPC and MCP Protocols
The Model Context Protocol (MCP) data contracts are represented via ad-hoc serialization structs rather than compiled protobuf models:
* `McpRequest` — `crates/op-web/src/mcp.rs:50`
* `McpResponse` — `crates/op-web/src/mcp.rs:59`
* `McpError` — `crates/op-web/src/mcp.rs:70`
* `JsonRpcRequest` — `crates/op-web/src/mcp_agents.rs:35`
* `JsonRpcResponse` — `crates/op-web/src/mcp_agents.rs:44`
* `JsonRpcError` — `crates/op-web/src/mcp_agents.rs:54`
* `JsonRpcRequest` — `crates/op-web/src/mcp_compact.rs:32`
* `JsonRpcResponse` — `crates/op-web/src/mcp_compact.rs:41`
* `JsonRpcError` — `crates/op-web/src/mcp_compact.rs:51`

### 3. State and Configuration Storage Contracts
External tool state is serialized using unstructured schemas and direct filesystem read/write operations:
* `IncusState` & `IncusInstance` — `crates/op-web/src/privacy_container.rs:28-49`
* `OpenFlowConfig`, `BridgeFlowConfig`, and `FlowEntry` — `crates/op-web/src/privacy_openflow.rs:12-40`
* `PrivacyRoutesState` & `PrivacyRoute` — `crates/op-web/src/privacy_routes.rs:12-32`

---

## Section 3: Security & Quality Vulnerabilities

### 1. [CRITICAL] Unauthenticated Direct/Named Tool Execution Endpoints (Remote Code Execution)
* **File & Line**: `crates/op-web/src/routes/mod.rs:135-139`, `crates/op-web/src/handlers/tools.rs:71-92`
* **Vulnerability Type**: CWE-306: Missing Authentication for Critical Function
* **Impact**: Remote Code Execution (RCE) / Full System Compromise.
* **Description**: The endpoints `/api/tool` and `/api/tools/:name/execute` execute tool calls on the system. Although `ip_security_middleware` is applied to the router layer (`crates/op-web/src/routes/mod.rs:248`), this middleware *only* inserts the identified `AccessZone` extension into the request extensions; it *never* denies access to unauthorized zones. The tool execution handlers in `tools.rs` never check the request's `AccessZone` extension. Consequently, anyone on the public internet can send a POST request to `/api/tool` to run arbitrary registered system tools (e.g., `shell_exec`, `file_write`) as `root` without authentication.
* **Exploitation Proof of Concept**:
  An external attacker can execute arbitrary bash commands by calling:
  ```http
  POST /api/tool HTTP/1.1
  Host: <vulnerable-ip>:8080
  Content-Type: application/json

  {
    "tool_name": "shell_exec",
    "arguments": {
      "command": "rm -rf /"
    }
  }
  ```
* **Remediation**: Extract the `AccessZone` extension inside `execute_tool_handler` and `execute_named_tool_handler` and reject the request if the client is not in `AccessZone::Localhost` or `AccessZone::TrustedMesh`.

---

### 2. [CRITICAL] Hardcoded Bypass API Keys (Static Credentials Backdoor)
* **File & Line**: `crates/op-web/src/middleware/security.rs:13-16`
* **Vulnerability Type**: CWE-798: Use of Hardcoded Credentials
* **Impact**: Total security zone bypass.
* **Description**: There are hardcoded cryptographic keys within `security.rs` that bypass all IP-based security zone restrictions:
  ```rust
  const BYPASS_API_KEYS: &[&str] = &[
      "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
      "test-key-huggingface-2024",            // Hugging Face test key
  ];
  ```
  Any request containing the `x-api-key`, `Authorization`, or `x-op-mcp-token` headers matching these keys is immediately elevated to `AccessZone::TrustedMesh`, bypassing security constraints.
* **Exploitation Proof of Concept**:
  An external attacker can gain `TrustedMesh` privileges by sending:
  ```http
  GET /api/status HTTP/1.1
  Host: <vulnerable-ip>:8080
  X-API-Key: test-key-huggingface-2024
  ```
* **Remediation**: Remove hardcoded API keys. Load permitted API keys dynamically from environment variables, cryptographically hashed config files, or a secure secrets manager.

---

### 3. [HIGH] CSRF Map Flushing Vulnerability (Denial of Service)
* **File & Line**: `crates/op-web/src/handlers/privacy.rs:647-653`
* **Vulnerability Type**: CWE-400: Uncontrolled Resource Consumption
* **Impact**: Denial of Service (DoS) for all concurrent OAuth users.
* **Description**: When storing CSRF tokens during Google OAuth initiation, the application uses an in-memory `HashMap` protected by a write lock. To prevent memory exhaustion, it performs a naive flush:
  ```rust
  // Cleanup old tokens (simple heuristic)
  if tokens.len() > 1000 {
      tokens.clear();
  }
  ```
  An attacker can trigger this condition by hitting `/api/privacy/google/auth` 1001 times. This wipes the *entire* token map, invalidating every active OAuth flow for all concurrent legitimate users.
* **Exploitation Proof of Concept**:
  1. Attackers script 1001 HTTP requests to `GET /api/privacy/google/auth`.
  2. The server's `csrf_tokens` map gets completely cleared.
  3. Legitimate users who were completing Google redirects will have their callbacks rejected with `403 Forbidden: Invalid CSRF state`.
* **Remediation**: Use an LRU cache or store tokens with individual expiration timestamps (e.g., using a thread-safe crate or cleaning expired elements periodically instead of wiping the entire database).

---

### 4. [HIGH] IP Spoofing via Untrusted Header Parsing
* **File & Line**: `crates/op-web/src/middleware/security.rs:59-67`
* **Vulnerability Type**: CWE-340: Generation of Predictable Numbers / IP Spoofing
* **Impact**: Privilege escalation to local IP zones.
* **Description**: The `extract_ip` function parses `X-Forwarded-For` and `X-Real-IP` headers directly from the incoming request without verifying if the request originated from a trusted reverse proxy:
  ```rust
  if let Some(forwarded) = headers.get("x-forwarded-for") {
      if let Ok(s) = forwarded.to_str() {
          if let Some(client_ip) = s.split(',').next() {
              return client_ip.trim().to_string();
          }
      }
  }
  ```
  If the application is exposed directly to the internet, any client can spoof their IP address to `127.0.0.1` by appending this header to bypass zone-based logic.
* **Exploitation Proof of Concept**:
  An external attacker can access `Localhost` or `PrivateNetwork` zones by sending:
  ```http
  GET /groups-admin/api/access-zone HTTP/1.1
  Host: <vulnerable-ip>:8080
  X-Forwarded-For: 127.0.0.1
  ```
* **Remediation**: Configure `extract_ip` to ignore proxy headers unless they come from verified upstream gateway addresses (e.g., Caddy or Nginx loopback IPs).

---

### 5. [MEDIUM] Plaintext WireGuard Private Key Storage
* **File & Line**: `crates/op-web/src/users.rs:31`, `crates/op-web/src/handlers/privacy.rs:188-193`, `crates/op-web/src/handlers/privacy.rs:242-248`
* **Vulnerability Type**: CWE-312: Cleartext Storage of Sensitive Information
* **Impact**: Exposure of user WireGuard cryptographic identities.
* **Description**: The user struct field is named `wg_private_key_encrypted`, but the signup and configuration generation handlers store and process the key in plaintext:
  ```rust
  // Create user (we'll encrypt the private key later, for now just store it)
  match state
      .user_store
      .create_user(&email, keypair.public_key, keypair.private_key)
  ```
  This raw private key is saved directly to `/var/lib/op-dbus/privacy-users.json` without encryption. Any compromise of the filesystem results in immediate key theft.
* **Remediation**: Encrypt WireGuard private keys before persisting them to `/var/lib/op-dbus/privacy-users.json` using a system-level key (e.g., via AEAD with a key stored in a secure location).

---

### 6. [MEDIUM] Raw `unsafe` Parsing with `simd_json::from_str`
* **File & Line**: `crates/op-web/src/websocket.rs:88`
* **Vulnerability Type**: Insecure Deserialization / Undefined Behavior risk
* **Impact**: Potential memory safety violations on malformed payloads.
* **Description**: In multiple locations, raw `unsafe simd_json::from_str` is used on user-controlled input:
  ```rust
  let mut raw = text.clone();
  let ws_msg: Result<WsMessage, _> = unsafe { simd_json::from_str(&mut raw) };
  ```
  `simd_json::from_str` is marked `unsafe` because it mutates the input slice. In this context, `raw` is a newly allocated owned string, so mutating it is safe. However, bypassing compiler safety checks with raw `unsafe` blocks introduces long-term maintainability risks if the underlying struct `WsMessage` is refactored to borrow from the parsed string (`&'a str`), which can lead to lifetime-related Undefined Behavior.
* **Remediation**: Use `simd_json::serde::from_slice` or safe parsed alternatives to eliminate unnecessary `unsafe` blocks.