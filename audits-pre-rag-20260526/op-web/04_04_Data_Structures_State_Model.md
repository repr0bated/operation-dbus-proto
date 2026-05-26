### Data Structures Audit

| File | Arc | Rc | RefCell | RwLock | Mutex | OnceCell | .clone() Calls | Large Structs (>5 public fields) | Globally Mutable State |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- | :--- |
| `crates/op-web/src/email.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 2 | `EmailConfig` (7) | None |
| `crates/op-web/src/groups_admin.rs` | 0 | 0 | 0 | 2 | 0 | 0 | 6 | None | `GROUPS_CONFIG` (`lazy_static`) |
| `crates/op-web/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/main.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 2 | None | None |
| `crates/op-web/src/mcp.rs` | 2 | 0 | 0 | 0 | 0 | 0 | 6 | None | `GLOBAL_BROADCASTER` (`lazy_static`) |
| `crates/op-web/src/mcp_agents.rs` | 5 | 0 | 0 | 1 | 0 | 0 | **34** ⚠️ | None | `GLOBAL_AGENTS_STATE` (`lazy_static`) |
| `crates/op-web/src/mcp_compact.rs` | 2 | 0 | 0 | 0 | 0 | 0 | 10 | None | None |
| `crates/op-web/src/mcp_discovery.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/mcp_smart_router.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/privacy_container.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 6 | None | None |
| `crates/op-web/src/privacy_openflow.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 3 | `OpenFlowConfig` (6) | None |
| `crates/op-web/src/privacy_routes.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 8 | `PrivacyRoute` (13) | None |
| `crates/op-web/src/router.rs` | 2 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/server.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 | None | None |
| `crates/op-web/src/sse.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/state.rs` | 12 | 0 | 0 | 2 | 0 | 0 | 10 | `AppState` (17) | None |
| `crates/op-web/src/state_manager_client.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 | None | None |
| `crates/op-web/src/system_prompt_loader.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/websocket.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 6 | None | None |
| `crates/op-web/src/embedded_ui.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/privacy_network.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | `PrivacyNetworkHostConfig` (8) | None |
| `crates/op-web/src/users.rs` | 0 | 0 | 0 | 5 | 0 | 0 | 15 | `PrivacyUser` (15) | None |
| `crates/op-web/src/wireguard.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/bin/op-dbus.rs` | 4 | 0 | 0 | 1 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/handlers/agents.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/handlers/auth_bridge.rs` | 1 | 0 | 0 | 1 | 0 | 0 | 3 | `PendingAuth` (8) | None |
| `crates/op-web/src/handlers/chat.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 10 | None | None |
| `crates/op-web/src/handlers/dashboard.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | `DashboardMetrics` (7) | None |
| `crates/op-web/src/handlers/health.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/handlers/llm.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 1 | None | None |
| `crates/op-web/src/handlers/logs.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 1 | None | None |
| `crates/op-web/src/handlers/mcp.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | `McpServer` (7), `Agent` (7) | None |
| `crates/op-web/src/handlers/status.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | `SystemInfo` (9), `StatusResponse` (6) | None |
| `crates/op-web/src/handlers/tools.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/handlers/users.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | `UserResponse` (9) | None |
| `crates/op-web/src/handlers/vpn.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | `VpnConnection` (8) | None |
| `crates/op-web/src/handlers/websocket.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 5 | None | None |
| `crates/op-web/src/handlers/mail.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | `MailQueueItem` (7) | None |
| `crates/op-web/src/handlers/mod.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | None | None |
| `crates/op-web/src/handlers/openclaw.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 1 | `OpenClawStatusResponse` (6) | None |
| `crates/op-web/src/handlers/privacy.rs` | 1 | 0 | 0 | 0 | 1 | 0 | 6 | None | `LAST_SIGNUP` (`static Mutex`) |

---

### Security Findings

#### CRITICAL: Unsanitized Path Traversal in Chat Transcript Saver (Arbitrary File Write / RCE)
* **File:Line**: `crates/op-web/src/handlers/chat.rs:326`
* **Exploitability**: High.
* **Mechanism**: The endpoint `/api/chat/transcript` accepts user-supplied JSON payload containing a `filename` string parameter. In `save_transcript_handler`, this filename is formatted directly as `format!("/tmp/{}", filename)` (line 407) and written with `tokio::fs::write`. Because there is absolutely no input validation or directory-traversal prevention (e.g. checking for `..`), a remote authenticated (or unauthenticated, see below) attacker can pass a path like `../../etc/cron.d/exploit` and write arbitrary contents to any location on the system. Since the written content corresponds to the chat transcript, the attacker can format the transcript to contain a valid cron format (or shell script payload), obtaining arbitrary remote code execution (RCE) with the privileges of the parent process (which runs with high privileges/root to communicate with Open vSwitch, rtnetlink, and `doas`).

#### CRITICAL: Complete Authentication & Authorization Bypass on Core System Action Routes
* **File:Line**: `crates/op-web/src/middleware/security.rs:123`
* **Exploitability**: High.
* **Mechanism**: The `ip_security_middleware` applied to the router determines the `AccessZone` based on the client IP address and attaches it as an Axum request extension. However, **this middleware never actually returns an error or rejects unauthorized requests**. It only logs diagnostic information and pushes the `AccessZone` into the request extensions.
When inspecting critical administrative and system endpoints, such as:
  * Direct tool execution: `POST /api/tool` (`handlers::tools::execute_tool_handler`)
  * Named tool execution: `POST /api/tools/:name/execute` (`handlers::tools::execute_named_tool_handler`)
  * JSON-RPC MCP handlers: `POST /mcp` (`mcp::mcp_handler`), `POST /mcp/message` (`mcp::mcp_message_handler`), and `POST /mcp/compact/message` (`mcp_compact::mcp_compact_message_handler`)
  * Profile modifications: `POST /groups-admin/api/profiles/:name` (`groups_admin::save_profile`)

  None of these handlers extract the `AccessZone` extension or validate the user's privilege level. Consequently, any remote attacker from any IP address on the public internet can directly execute highly privileged native system tools (e.g., `shell_exec`, Open vSwitch database mutations, systemd service lifecycle control) without credentials.

#### CRITICAL: Unauthenticated Tool Profile Overwrite & ACL Bypass
* **File:Line**: `crates/op-web/src/groups_admin.rs:242`
* **Exploitability**: High.
* **Mechanism**: The `save_profile` endpoint `/groups-admin/api/profiles/:name` allows any client to submit a payload that overrides existing tool profiles on disk (specifically writing to `/var/lib/op-dbus/tool-groups.json`). By overriding the `"default"` profile, an attacker can enable dangerous tool groups, ensuring that the legacy / legacy-fallback MCP clients execute dangerous native tools without restriction.

#### HIGH: Hardcoded Security Bypass Backdoor Keys
* **File:Line**: `crates/op-web/src/middleware/security.rs:16`
* **Exploitability**: High.
* **Mechanism**: The security middleware defines a static array of hardcoded bypass keys:
  ```rust
  const BYPASS_API_KEYS: &[&str] = &[
      "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
      "test-key-huggingface-2024",            // Hugging Face test key
  ];
  ```
  Any request containing these keys in `x-api-key`, `Authorization: Bearer`, or `x-op-mcp-token` headers automatically bypasses all diagnostic IP checks and elevates the request's diagnostic status to `AccessZone::TrustedMesh` (full access). Hardcoding bypass keys in production crates exposes the deployment to trivial authentication bypass if the binary or source repository is public.

#### HIGH: Plaintext Storage of Sensitive Private Keys
* **File:Line**: `crates/op-web/src/users.rs:265`
* **Exploitability**: Medium (requires local filesystem read or arbitrary file read vulnerability).
* **Mechanism**: When a new user registers or links their Google OAuth identity, their newly generated WireGuard private key is stored directly in the `wg_private_key_encrypted` database field of the `PrivacyUser` struct. Despite the "encrypted" suffix in the field name, the code does not perform any encryption on the key before storing it. The raw, plaintext private key is written directly to `/var/lib/op-dbus/privacy-users.json`, exposing all users' WireGuard traffic to any attacker with local read access.

#### MEDIUM: Unsafe Deserialization from Disk via `simd_json::from_str`
* **File:Line**: `crates/op-web/src/groups_admin.rs:51` and `crates/op-web/src/users.rs:84`
* **Exploitability**: Low (requires file corruption or file write privileges).
* **Mechanism**: Both files deserialize JSON files from disk using `unsafe { simd_json::from_str(...) }`. If an attacker or a concurrent process corrupts these JSON databases (or changes their encoding), `simd_json`'s unchecked parsing constraints can trigger undefined behavior (UB), buffer overflows, or segmentation faults in the server binary.

#### MEDIUM: Predictable Temporary File Creation during State Persistence
* **File:Line**: `crates/op-web/src/users.rs:118`
* **Exploitability**: Medium.
* **Mechanism**: The user storage manager saves states by formatting a predictable temporary path: `format!("{}.tmp", self.storage_path)` (yielding `/var/lib/op-dbus/privacy-users.json.tmp`). If the parent directory is world-writable or shared, a local attacker can create a symlink at `/var/lib/op-dbus/privacy-users.json.tmp` pointing to a sensitive system file (e.g. `/etc/passwd` or `/etc/shadow`), causing `op-web` to overwrite the target file with user database JSON data upon the next write operation.

#### MEDIUM: Sensitive Testing Magic Link Leak to Public Output
* **File:Line**: `crates/op-web/src/email.rs:72`
* **Exploitability**: Medium.
* **Mechanism**: When SMTP is not configured, the email sender prints magic link authentication tokens directly to standard output (`println!`). In staging or misconfigured production environments, these authentication links (valid for 15 minutes) will be written to public-facing application logs, allowing anyone with log-read access to compromise user accounts.