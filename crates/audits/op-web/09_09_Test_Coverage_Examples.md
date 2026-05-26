# op-web Production Security and Quality Audit

## ROLE: Tests

### Test Suite Summary
- **Total Test Functions**: 17
- **Property Testing & Fuzzing**: None found. All tests are standard manual unit tests.

### Representative Tests
1. **Container Name Generation Test**: `crates/op-web/src/privacy_container.rs:210` (`generates_container_name_from_uuid`) - Verifies deterministic container naming from user UUIDs.
2. **Deterministic Route ID Derivation Test**: `crates/op-web/src/privacy_routes.rs:128` (`test_route_id_is_deterministic`) - Assures that the HKDF-based route IDs remain stable and deterministic.
3. **Forbidden CLI Detection Test**: `crates/op-web/src/orchestrator/anti_hallucination.rs:199` (`test_detects_ovs_vsctl`) - Validates the regex-based detection of banned CLI commands in LLM generations.

---

## Schema-as-Code Discipline Audit

The codebase exhibits multiple violations of the schema-as-code discipline. Rather than utilizing versioned, standardized schema descriptions (such as Protocol Buffers, JSON Schema documents, or OSCAL formats), key data contracts are declared as ad-hoc Rust structs, and some are dynamically queried and manipulated using raw JSON values.

### Schema-as-Code Violations
1. **Incus Instance and State Contracts**: 
   - *Location*: `crates/op-web/src/privacy_container.rs:43-46`
   - *Violation*: `IncusState` and `IncusInstance` represent external infrastructure resource models but are declared as ad-hoc Rust structs with generic string-to-string maps for configuration and devices.
2. **OpenFlow Bridge and Flow Policies**:
   - *Location*: `crates/op-web/src/privacy_openflow.rs:14-23`
   - *Violation*: OpenFlow configurations, flow matches, and action representations (`OpenFlowConfig`, `BridgeFlowConfig`, `FlowEntry`) are declared as ad-hoc internal Rust structures instead of being generated from standard, versioned network control-plane schemas.
3. **Privacy Routing State**:
   - *Location*: `crates/op-web/src/privacy_routes.rs:14-19`
   - *Violation*: `PrivacyRoutesState` and `PrivacyRoute` represent critical system routing contracts but are modeled as ad-hoc structures without schema versioning attributes.
4. **Model Context Protocol (MCP) Message Frames**:
   - *Location*: `crates/op-web/src/mcp.rs:51-71`
   - *Violation*: Frame structures for the MCP JSON-RPC spec (`McpRequest`, `McpResponse`, and `McpError`) are implemented as custom local structures rather than utilizing shared, formally versioned schema packages.
5. **Authentication Bridge Contracts**:
   - *Location*: `crates/op-web/src/handlers/auth_bridge.rs:23`
   - *Violation*: `PendingAuth` represents sensitive authorization requests in an unversioned ad-hoc structure.

---

## Security & Quality Audit Findings

### [CRITICAL] Remote Code Execution (RCE) via Unauthenticated Direct Tool Execution API
- **Location**: `crates/op-web/src/handlers/tools.rs:79` (`execute_tool_handler`) and `crates/op-web/src/handlers/tools.rs:88` (`execute_named_tool_handler`)
- **Impact**: Any remote client can execute arbitrary tools on the host. This includes high-privilege operations such as shell command execution (`shell_exec` / `shell_execute`), reading raw files (`file_read`), and writing files (`file_write`).
- **Description**: The endpoints `/api/tool` and `/api/tools/:name/execute` execute tools directly from the `tool_registry` using user-supplied parameters. There is no authentication validation, session verification, or signature checking performed within either handler. Because the web server manages network namespaces and interfaces, this process runs with highly elevated privileges (e.g. using `doas`), enabling full host compromise.
- **Remediation**: Implement strict cryptographic authentication (such as session tokens or API keys validated against the `user_store`) on all tool-execution endpoints. Ensure that only authenticated clients mapped to the `TrustedMesh` or `Localhost` security zones can access direct execution APIs.

### [CRITICAL] Path Traversal and Arbitrary File Overwrite via Unsanitized Transcript Filename
- **Location**: `crates/op-web/src/handlers/chat.rs:304` (`save_transcript_handler`) and `crates/op-web/src/handlers/chat.rs:377` (`save_transcript_to_file`)
- **Impact**: Arbitrary file write. An attacker can write arbitrary contents to any path on the filesystem (such as `/etc/cron.d/malicious_job` or `/root/.ssh/authorized_keys`), leading directly to remote code execution and host takeover.
- **Description**: `save_transcript_handler` extracts the `filename` parameter from a user-supplied JSON payload without sanitizing it for path traversal sequences (e.g., `../`). This parameter is then formatted directly into `/tmp/` in `save_transcript_to_file`:
  ```rust
  let filepath = format!("/tmp/{}", filename);
  ```
  Because the string is passed unchecked to `tokio::fs::write`, directory traversal is fully trivial.
- **Remediation**: Sanitize the `filename` by extracting only the base name (using `std::path::Path::file_name`) or generate a secure, random UUID-based filename on the server-side instead of trusting user input.

### [CRITICAL] Broken Access Control via Ineffective Security Middleware
- **Location**: `crates/op-web/src/middleware/security.rs:98` (`ip_security_middleware`) and `crates/op-web/src/routes/mod.rs:188`
- **Impact**: Complete bypass of the IP security zone model. All sensitive administrator APIs, registered user details, and system diagnostic logs are exposed to the public internet without authorization.
- **Description**: The `ip_security_middleware` is designed to enforce access control based on client IP addresses and bypass keys. However, the middleware always calls `next.run(request).await` regardless of the resolved `AccessZone`. It never rejects requests. Furthermore, individual endpoint handlers (such as `list_users_handler` or `execute_tool_handler`) do not extract or check the `AccessZone` extension from the request before carrying out sensitive actions.
- **Remediation**: Update `ip_security_middleware` to actively reject requests that resolve to unauthorized zones for a given path prefix, or enforce zone checks programmatically in every route handler.

### [HIGH] Denial of Service (DoS) via Naive CSRF Token Cache Purging
- **Location**: `crates/op-web/src/handlers/privacy.rs:629` (`google_auth`)
- **Impact**: All pending Google OAuth login sessions can be instantly invalidated by a remote attacker, resulting in a persistent Denial of Service for authentication.
- **Description**: When storing a newly generated CSRF token, the system employs a primitive cleanup mechanism:
  ```rust
  // Cleanup old tokens (simple heuristic)
  if tokens.len() > 1000 {
      tokens.clear();
  }
  ```
  If an attacker sends 1001 rapid requests to the `/api/privacy/google/auth` endpoint, the `tokens` cache is completely cleared. This drops all active, legitimate CSRF states for users currently undergoing the login flow.
- **Remediation**: Replace the global cache purge with an LRU (Least Recently Used) cache or time-to-live (TTL) expiration mechanism to prune expired tokens individually without affecting valid ones.