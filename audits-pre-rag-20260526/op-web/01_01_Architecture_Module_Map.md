# ROLE: Architecture & Module Map

### Overview
`op-web` is a unified web server designed to consolidate HTTP services, WebSocket handling, Model Context Protocol (MCP) endpoints, and privacy router provisioning into a single service. It acts as the gateway to the `op-dbus-v2` control plane, facilitating both system administrator interactions (via an LLM-orchestrated chat and direct tool executions) and client VPN connections (via WireGuard/Incus provisioning).

### Module Tree
```text
crates/op-web/src/
├── bin/
│   └── op-dbus.rs (Binary)
├── handlers/
│   ├── agents.rs
│   ├── auth_bridge.rs
│   ├── chat.rs
│   ├── dashboard.rs
│   ├── health.rs
│   ├── llm.rs
│   ├── logs.rs
│   ├── mail.rs
│   ├── mcp.rs
│   ├── openclaw.rs
│   ├── privacy.rs
│   ├── status.rs
│   ├── tools.rs
│   ├── users.rs
│   └── vpn.rs
├── middleware/
│   └── security.rs
├── orchestrator/
│   ├── anti_hallucination.rs
│   ├── execution.rs
│   ├── formatting.rs
│   ├── mod.rs
│   ├── parsing.rs
│   ├── process.rs
│   └── tools.rs
├── routes/
│   ├── admin.rs
│   ├── chat.rs
│   ├── llm.rs
│   └── mod.rs
├── email.rs
├── embedded_ui.rs
├── groups_admin.rs
├── lib.rs
├── main.rs (Binary Entry Point)
├── mcp.rs
├── mcp_agents.rs
├── mcp_compact.rs
├── mcp_discovery.rs
├── mcp_smart_router.rs
├── privacy_container.rs
├── privacy_network.rs
├── privacy_openflow.rs
├── privacy_routes.rs
├── router.rs
├── server.rs
├── sse.rs
├── state.rs
├── state_manager_client.rs
├── system_prompt_loader.rs
├── users.rs
└── wireguard.rs
```

### Entry Points
*   **Library Entry Point**: `crates/op-web/src/lib.rs:1`
*   **Web Server Daemon Entry Point**: `crates/op-web/src/main.rs:1` (compiled as `op-web-server`)
*   **gRPC Control Plane Daemon Entry Point**: `crates/op-web/src/bin/op-dbus.rs:1` (compiled as `op-dbus`)

### Notes
*   **Web Framework**: Built on `axum` with async networking powered by `tokio`.
*   **State Sharing**: Maintained globally via `Arc<AppState>` and injected using Axum's state/extension mechanisms.
*   **D-Bus Integration**: Interfaces with system services (NetworkManager, Open vSwitch, StateManager, dinit) using `zbus` and custom gRPC-to-D-Bus bridging.

---

# Production Security & Quality Audit

## Critical Vulnerabilities

### 1. Pre-Authentication Remote Code Execution (RCE) via WebSocket
*   **File**: `crates/op-web/src/routes/mod.rs:334`
*   **File**: `crates/op-web/src/websocket.rs:141`
*   **Description**: The WebSocket endpoint `/ws` is mapped directly to `websocket::websocket_handler` in the main router without any authentication, session validation, or IP-restriction middleware. Upon connection, the handler starts an unbounded loop reading raw text messages from the client and directly passing them to the `orchestrator.process()` pipeline. Because the orchestrator holds a registry of system tools (including `shell_exec`), any unauthenticated remote client can connect to `/ws` and execute arbitrary system shell commands with the privileges of the running daemon.
*   **Remediation**: Wrap the `/ws` route in an authentication filter (e.g., verifying a signed JWT or session cookie) and restrict the connection's `AccessZone` extension before permitting the WebSocket upgrade.

### 2. Public Exposure of Direct Tool Execution Endpoints (RCE)
*   **File**: `crates/op-web/src/routes/mod.rs:146`
*   **File**: `crates/op-web/src/handlers/tools.rs:94`
*   **Description**: The REST endpoints `/api/tool` and `/api/tools/:name/execute` execute registered system tools directly. These endpoints are mapped in `create_router` without any validation of headers, access tokens, or `AccessZone` extensions. An unauthenticated attacker can execute any tool (e.g., `shell_exec`, `file_write`, or D-Bus system calls) by sending a HTTP `POST` request, resulting in direct, pre-auth remote code execution.
*   **Remediation**: Apply a strict authorization middleware (e.g., bearer tokens or session validation) to all endpoints nested under `/api`. Ensure that sensitive actions are only allowed when the request's `AccessZone` is validated as `Localhost` or `TrustedMesh`.

### 3. Public Exposure of the `/admin` Nested Router
*   **File**: `crates/op-web/src/routes/mod.rs:341`
*   **File**: `crates/op-web/src/routes/admin.rs:40`
*   **Description**: The `/admin` router is registered at the root of the main application without authentication. This exposes administrative utilities to the public network, enabling unauthenticated attackers to:
    *   Overwrite the LLM's custom system prompt via `POST /admin/prompt/custom`, facilitating prompt-injection and behavioral hijacking.
    *   Leak internal configuration states via `GET /admin/config` (including system file paths and tool counts).
    *   Read the complete system prompt context using `GET /admin/prompt`.
*   **Remediation**: Protect the `/admin` route namespace using session authentication, and enforce a rule that only requests with `AccessZone::Localhost` can access admin-level routes.

### 4. Broken IP Access Control via Untrusted Headers
*   **File**: `crates/op-web/src/middleware/security.rs:75`
*   **Description**: The `extract_ip` utility extracts the client's IP address by prioritizing the `X-Forwarded-For` and `X-Real-IP` HTTP headers. Because these headers are trusted blindly without verifying whether the request originated from a trusted reverse proxy, an external attacker can easily spoof these headers (e.g., sending `X-Forwarded-For: 127.0.0.1`). This elevates their request's security context to `AccessZone::Localhost` or `AccessZone::TrustedMesh`, bypassing downstream client-side or server-side security checks.
*   **Remediation**: Do not trust proxy headers unless the connection's direct peer IP (`ConnectInfo<SocketAddr>`) belongs to a explicitly configured, trusted upstream proxy list.

### 5. Plaintext Client Private Key Exposure and Global Hardcoded Bypass Keys
*   **File**: `crates/op-web/src/middleware/security.rs:16`
*   **File**: `crates/op-web/src/handlers/privacy.rs:125`
*   **File**: `crates/op-web/src/handlers/privacy.rs:408`
*   **Description**: 
    *   `BYPASS_API_KEYS` contains hardcoded static API keys (e.g., `"test-key-huggingface-2024"`) compiled directly into the binary. Any request containing this header is granted `AccessZone::TrustedMesh` privileges.
    *   During user registration, the newly generated client WireGuard private key is saved in **plaintext** to `/var/lib/op-dbus/privacy-users.json` under the key `wg_private_key_encrypted` (contrary to the name suggesting encryption).
    *   The `get_config` handler allows anyone presenting a bypass key to retrieve any user's WireGuard configuration, exposing plaintext private keys across the network.
*   **Remediation**: Remove all hardcoded security bypass tokens. Implement a secure, server-side asymmetric key derivation strategy where the server never stores or transmits the client's private key in plaintext after generation.

### 6. Arbitrary File Write / Directory Traversal via Chat Transcripts
*   **File**: `crates/op-web/src/handlers/chat.rs:260`
*   **Description**: The `/api/chat/transcript` endpoint takes a user-supplied `filename` parameter and constructs a path using `format!("/tmp/{}", filename)`. This path is then passed to `tokio::fs::write` to write the chat transcript. Because `filename` is not sanitized, an attacker can perform a directory traversal attack (e.g., `filename: "../etc/cron.d/malicious_job"` or `filename: "../root/.ssh/authorized_keys"`) and write arbitrary content to files anywhere on the host filesystem. Since the user controls the chat message contents, this allows arbitrary file creation/overwriting and privilege escalation.
*   **Remediation**: Strip path-traversal segments (e.g., `..`, `/`) from the `filename` parameter, or strictly validate that the resolved canonical path lies within a designated safe directory.

---

## High & Medium Vulnerabilities

### 1. Broken Google OAuth CSRF State Validation
*   **File**: `crates/op-web/src/handlers/privacy.rs:599`
*   **Description**: The `google_callback` handler validates the OAuth `state` parameter by checking if it exists in the global `state.csrf_tokens` map and removing it. However, the token is not bound to the specific user session that initiated the authorization request (e.g., via an encrypted session cookie). An attacker can initiate an OAuth flow, capture their own valid state token, and force a victim's browser to complete the callback using the attacker's authorization code. This allows OAuth session fixation/hijacking.
*   **Remediation**: Bind OAuth state tokens to the initiating client's browser session using a secure, HTTP-only, ephemeral session cookie, and assert that the state returned by Google matches the state stored in the cookie.

### 2. State Mutation Race Conditions (TOCTOU) in D-Bus Integration
*   **File**: `crates/op-web/src/privacy_container.rs:95`
*   **File**: `crates/op-web/src/privacy_openflow.rs:90`
*   **Description**: Both container and OpenFlow provisioning follow a non-atomic "Read-Modify-Write" pattern: they query current plugin state from the system D-Bus StateManager, mutate the state vector locally, and apply it back. Under high concurrency (e.g., multiple concurrent signups or rapid route additions), this leads to Time-of-Check to Time-of-Use (TOCTOU) race conditions where parallel writes overwrite or discard each other's state changes.
*   **Remediation**: Implement distributed locking or utilize a transactional, atomic state mutation interface on the StateManager D-Bus service.

---

## Quality, Safety, & Maintainability Findings

### 1. Undefined Behavior via Unpadded Buffers in `simd_json`
*   **File**: `crates/op-web/src/handlers/websocket.rs:88`
*   **File**: `crates/op-web/src/users.rs:103`
*   **File**: `crates/op-web/src/orchestrator/parsing.rs:32`
*   **File**: `crates/op-web/src/state_manager_client.rs:40`
*   **Description**: The codebase repeatedly parses JSON using `unsafe { simd_json::from_str(&mut raw) }` where `raw` is a standard `String` or slice. `simd-json` requires its input buffer to have trailing padding bytes (`simd_json::SIMD_JSON_PADDING` - typically 32 or 64 bytes) to safely run SIMD vector instructions. Standard Rust `String` allocations do not guarantee this padding. If the SIMD parser reads past the end of the unpadded allocation, it can trigger segmentation faults or leak adjacent heap memory.
*   **Remediation**: Ensure buffers are padded using `simd_json::to_owned_value` or resize the string capacity to include padding bytes before invoking the unsafe string parser.

### 2. Global Tool Administration without Server-Side Verification
*   **File**: `crates/op-web/src/groups_admin.rs:218`
*   **Description**: In `groups_admin.rs`, the client-side JavaScript implements UI checks to gray out high-privilege tools if the client is not in a safe network zone. However, the corresponding backend handler `save_profile` does not validate the client's actual privilege level. It blindly processes saves, allowing anyone to modify tool groupings and elevate privileges.
*   **Remediation**: Re-verify security policies and `AccessZone` restrictions on the server side for every REST request. Do not rely on UI-level locking.

---
## ⚠ Citation Warnings
- `crates/op-web/src/routes/mod.rs:334`: file has 248 lines
- `crates/op-web/src/routes/mod.rs:341`: file has 248 lines
