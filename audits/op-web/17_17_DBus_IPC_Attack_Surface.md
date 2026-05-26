# D-Bus & IPC Attack Surface & Quality Audit

## 1. D-Bus & IPC Attack Surface Map

This section maps all D-Bus interfaces, methods, and components interacting with the system bus or session bus, as implemented in the provided source files.

### D-Bus Interface Table
| D-Bus Interface | Object Path | Method / Signal | Session vs. System Bus | Caller Identity / Auth Check | Impact of Execution |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `org.opdbus.StateManager` | `/org/opdbus/v1/state` | `QueryState` (Method Call) | System Bus | **None** (Triggered directly by unauthenticated HTTP API endpoints) | Reads unified system configuration state |
| `org.opdbus.StateManager` | `/org/opdbus/v1/state` | `ApplyContractMutation` (Method Call) | System Bus | **None** (Indirectly triggered via unauthenticated endpoints) | Modifies system architecture (OpenFlow, Incus containers, route profiles) |

### D-Bus Connection Mechanics
*   **System Bus Utilization**: The `op-web` service connects exclusively to the system bus via `zbus::Connection::system()` in `crates/op-web/src/state_manager_client.rs:24` and `crates/op-web/src/state_manager_client.rs:49`.
*   **Authorization Deficiencies**: The `state_manager_client` library makes high-privilege method calls to the `org.opdbus.StateManager` service on the system bus. No caller identity verification or permission checks are performed prior to passing mutations to the D-Bus daemon. 

---

## 2. Security & Code Quality Findings

### [CRITICAL] Unauthenticated Arbitrary Tool Execution & Host System Compromise
*   **File Citation**: `crates/op-web/src/routes/mod.rs:40-205` / `crates/op-web/src/handlers/tools.rs:65-104`
*   **Vulnerability Type**: Privilege Escalation / Unauthenticated Remote Code Execution (RCE)
*   **Impact**: Direct host system compromise. An unauthenticated attacker can execute arbitrary system commands, manipulate networking, restart services, and read/write arbitrary files as `root` (or the privileged user running the control plane).
*   **Description**: 
    The HTTP router registers several highly sensitive endpoints, including `POST /api/tool` (mapping to `execute_tool_handler`) and `POST /api/tools/:name/execute` (mapping to `execute_named_tool_handler`). 
    Although a security middleware (`security::ip_security_middleware`) is applied to the router in `crates/op-web/src/routes/mod.rs:201`, this middleware is a non-blocking placeholder. It calculates the client's `AccessZone` and inserts it into the request extensions, but **never rejects or denies any requests** (see `crates/op-web/src/middleware/security.rs:114-142`). 
    The tool execution handlers in `crates/op-web/src/handlers/tools.rs` fail to extract this `AccessZone` extension or perform any API token validation. Any remote attacker with access to the HTTP port (default `0.0.0.0:8080`) can execute arbitrary tools from the registry (including `shell_exec`, `file_write`, and `dbus_systemd_restart_unit`) with zero authentication.
*   **Remediation**:
    Enforce strict access control inside the `ip_security_middleware` or within the handler functions. Rejects requests from unauthorized IP address ranges or those lacking a valid bypass API token.
    ```rust
    // Example remediation inside security.rs or handler:
    let zone = request.extensions().get::<AccessZone>().ok_or(StatusCode::FORBIDDEN)?;
    if !zone.can_access(SecurityLevel::Elevated) {
        return Err(StatusCode::FORBIDDEN);
    }
    ```

---

### [HIGH] Memory Unsoundness / Parsing Flaws via Unvalidated Unsafe Deserialization
*   **File Citation**: `crates/op-web/src/orchestrator/parsing.rs:30`, `crates/op-web/src/orchestrator/parsing.rs:69`, `crates/op-web/src/groups_admin.rs:44`
*   **Vulnerability Type**: Memory Safety / Deserialization Vulnerability
*   **Impact**: Potential Denial of Service (DoS), crash, or memory corruption.
*   **Description**:
    The codebase repeatedly utilizes `unsafe { simd_json::from_str(...) }` on raw, mutable strings derived directly from HTTP/WebSocket inputs. The `unsafe` variant of `simd_json::from_str` skips critical structural alignment and lifetime checks to perform in-place mutations. Processing unvalidated remote strings via an unsafe SIMD-accelerated parser can result in memory corruption or crashes if malicious JSON payloads bypass expected UTF-8 or structural invariants.
*   **Remediation**:
    Use the safe, standard `simd_json::from_str` or `serde_json::from_str` APIs when deserializing untrusted strings. The marginal performance loss of safe parsing is negligible compared to the memory safety risks of `unsafe simd_json` on public network endpoints.

---

### [HIGH] Denial of Service via Trivial CSRF Collection Eviction
*   **File Citation**: `crates/op-web/src/handlers/privacy.rs:533-541`
*   **Vulnerability Type**: Denial of Service (DoS) / State Manipulation
*   **Impact**: Legitimate users are locked out of the Google OAuth login flow.
*   **Description**:
    When initiating a Google OAuth flow, the server generates a random CSRF token and registers it in `state.csrf_tokens`. To prevent memory exhaustion, the handler implements a primitive cleanup heuristic:
    ```rust
    if tokens.len() > 1000 {
        tokens.clear();
    }
    ```
    This completely wipes the entire CSRF token registry. An attacker can repeatedly trigger the `/api/privacy/google/auth` endpoint 1,001 times, causing the server to clear all legitimate users' pending OAuth tokens. Consequently, all active users completing their callback will fail authentication with `403 Forbidden` ("Invalid CSRF state").
*   **Remediation**:
    Implement an LRU cache or a timed eviction strategy (e.g., storing tokens alongside their expiration timestamp) rather than wiping the entire cache when it fills up.

---

### [MEDIUM] Host Privilege Escalation via Wildcard Passwordless Sudo Alternative (`doas`)
*   **File Citation**: `crates/op-web/src/handlers/status.rs:192-196`
*   **Vulnerability Type**: Privilege Escalation Dependency
*   **Impact**: Host takeover if the `op-web` system user is compromised.
*   **Description**:
    The system health monitoring endpoint executes a system shell-out using `doas dinitctl list` to check service statuses. This design pattern implies that the `op-web` service user is configured with passwordless privileges to run `/sbin/dinitctl` via `doas`. If an attacker compromises the web server (e.g., via the unauthenticated tool execution vulnerability), they can leverage this `doas` configuration to manipulate host services.
*   **Remediation**:
    Rather than calling command-line wrappers like `doas dinitctl`, monitor service status programmatically using native IPC or local socket queries, or restrict the system service permissions using strict systemd/dinit capabilities and sandboxing.

---

## 3. Schema-as-Code Compliance Audit

The system mandates a Schema-as-Code discipline using Protocol Buffers and OSCAL versioned schemas. Ad-hoc structs, raw JSON maps, and unschematized strings are strictly discouraged.

### Schema-as-Code Violations
1.  **Tool Groups Profile Contracts**:
    *   **File**: `crates/op-web/src/groups_admin.rs:30-36`, `crates/op-web/src/groups_admin.rs:218-222`
    *   **Violation**: The serialization contract for user-enabled tools (`EnabledGroups`) and requests to change profiles (`SaveProfileRequest`) are implemented as ad-hoc, unversioned Rust structs instead of Protobuf messages.
2.  **Cognitive Agent Metadata & Runtimes**:
    *   **File**: `crates/op-web/src/mcp_agents.rs:74-95`
    *   **Violation**: State selection (`AgentSelectionConfig`) and runtime snapshots (`CognitiveRuntimeSnapshot`) are formulated as ad-hoc structs. This configuration is stored as unversioned JSON in `/var/lib/op-dbus/cognitive-mcp-agents.json`.
3.  **Meta-Tool Definitions (JSON-RPC)**:
    *   **File**: `crates/op-web/src/mcp_compact.rs:66-160`
    *   **Violation**: The meta-tools schema descriptions (`list_tools`, `search_tools`, `get_tool_schema`, `execute_tool`) are constructed as raw JSON literals inside `json!` macros, entirely bypassing compile-time versioned schema definitions.
4.  **Incus State Representation**:
    *   **File**: `crates/op-web/src/privacy_container.rs:24-43`
    *   **Violation**: The container virtualization state contracts (`IncusState` and `IncusInstance`) are represented as local ad-hoc structures mirroring external virtual machine configurations.
5.  **OpenFlow Policy Structure**:
    *   **File**: `crates/op-web/src/privacy_openflow.rs:12-67`
    *   **Violation**: `OpenFlowConfig` and its inner routing actions are defined using ad-hoc enums and structs rather than versioned networking schema formats.
6.  **Privacy Route Specifications**:
    *   **File**: `crates/op-web/src/privacy_routes.rs:11-35`
    *   **Violation**: `PrivacyRoute` maps networking details across system components using unversioned Rust models.
7.  **Unstructured System Status Contracts**:
    *   **File**: `crates/op-web/src/handlers/status.rs:39-99`
    *   **Violation**: System resources, LLM, network interface states, and active agent metrics are packaged inside a monolithic, unversioned `StatusResponse` struct.

### Remediation Guidance
To achieve full compliance with the control plane's architecture:
1.  Define all API requests, responses, state files, and inter-service payloads using Protocol Buffers (`.proto`).
2.  Use the `op-grpc-bridge` or `prost` to compile these definitions into type-safe, versioned Rust structures.
3.  Incorporate OSCAL schemas for system authorization and validation rules instead of maintaining manual JSON structure representations.