# Production Security & Quality Audit: `op-web`

## 1. Documentation & Quality Checklist

### Crate-Level Documentation
* **Status**: **Pass**
* **Location**: `crates/op-web/src/lib.rs:1-19`
* **Comment**: The crate contains detailed crate-level `//!` documentation explaining the unified web server architecture and mapping out all the main routes (`/api/health`, `/api/status`, `/api/tools`, `/mcp`, etc.).

### README.md Presence
* **Status**: **Fail / Absent**
* **Comment**: No `README.md` file was provided in the source files of the `op-web` crate.

### Public Unsafe Functions
* **Status**: **Pass / None Found**
* **Comment**: No `pub unsafe fn` declarations were found in the provided codebase. There are only `unsafe` blocks wrapping safe-interface libraries (e.g., `simd_json::from_str`).

---

## 2. Public Item Documentation Sampling (10 Items)

Below is a sample of 10 public items across the codebase checked for the presence of `///` rustdoc comments:

1. **`pub struct EmailConfig`** (`crates/op-web/src/email.rs:13`)
   * **Status**: **Pass** (documented with `/// Email configuration from environment`)
2. **`pub fn from_env`** (`crates/op-web/src/email.rs:25`)
   * **Status**: **Pass** (documented with `/// Load from environment variables`)
3. **`pub fn is_configured`** (`crates/op-web/src/email.rs:41`)
   * **Status**: **Pass** (documented with `/// Check if email is configured`)
4. **`pub struct EmailSender`** (`crates/op-web/src/email.rs:47`)
   * **Status**: **Pass** (documented with `/// Email sender`)
5. **`pub fn new`** (`crates/op-web/src/email.rs:52`)
   * **Status**: **Fail** (missing `///` rustdoc)
6. **`pub async fn send_magic_link`** (`crates/op-web/src/email.rs:57`)
   * **Status**: **Pass** (documented with `/// Send a magic link email`)
7. **`pub struct GroupsConfig`** (`crates/op-web/src/groups_admin.rs:24`)
   * **Status**: **Pass** (documented with `/// Tool groups configuration storage`)
8. **`pub struct EnabledGroups`** (`crates/op-web/src/groups_admin.rs:31`)
   * **Status**: **Fail** (missing `///` rustdoc)
9. **`pub fn new`** (`crates/op-web/src/groups_admin.rs:38`)
   * **Status**: **Fail** (missing `///` rustdoc)
10. **`pub async fn get_profile`** (`crates/op-web/src/groups_admin.rs:72`)
    * **Status**: **Fail** (missing `///` rustdoc)

---

## 3. Critical Security Findings

### [CRITICAL] Path Traversal and Arbitrary File Write via Chat Transcript Endpoint
* **Location**: `crates/op-web/src/handlers/chat.rs:212` and `crates/op-web/src/handlers/chat.rs:271`
* **Vulnerability Type**: Path Traversal leading to Arbitrary File Write / Remote Code Execution (RCE)
* **Exploitability**: Directly Exploitable

#### Description
The `/api/chat/transcript` endpoint is mapped to `save_transcript_handler` without any form of authentication or path sanitization. 

The `filename` parameter is retrieved directly from the untrusted JSON payload:
```rust
let filename = params
    .get("filename")
    .and_then(|v| v.as_str())
    .map(str::to_string)
    .unwrap_or_else(|| format!("chat-transcript-{}.txt", chrono::Utc::now().timestamp()));
```

It is then formatted directly into `/tmp/{}`:
```rust
let filepath = format!("/tmp/{}", filename);
match tokio::fs::write(&filepath, &transcript).await {
```

An attacker can pass a path-traversal filename (e.g., `../../../etc/cron.d/malicious_job`). 

Additionally, because the `messages` array can be supplied directly in the payload:
```rust
if let Some(messages) = params.get("messages").and_then(|v| v.as_array()) {
```
The contents of `transcript` can be arbitrarily controlled. An attacker can write raw text containing newlines to bypass the prefix formatting. This allows writing a fully-functioning cron job or replacing `/etc/passwd`. Because `op-web` runs with highly privileged access (to control Open vSwitch, systemd units via D-Bus, and networking interfaces), this leads directly to root-level Remote Code Execution.

---

### [CRITICAL] Plaintext WireGuard Private Key and Credential Harvest via Unauthenticated Endpoints
* **Location**: `crates/op-web/src/handlers/users.rs:21` and `crates/op-web/src/handlers/privacy.rs:309`
* **Vulnerability Type**: Authentication Bypass & Critical Information Disclosure
* **Exploitability**: Directly Exploitable

#### Description
The web application provides an endpoint `/api/users` (`list_users_handler`) which outputs all registered users on the system, including their unique user UUIDs:
```rust
pub async fn list_users_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> Json<Vec<UserResponse>> {
    let users = state.user_store.list_users().await;
    Json(
        users
            .into_iter()
            .map(|u| UserResponse {
                id: u.id, // Exposed UUID
                ...
```

The user-facing access endpoint `/privacy/access` (`privacy_access_message`) renders the full plaintext WireGuard configuration—**including the plaintext client private key**—when queried with a `user_id`:
```rust
let config = generate_client_config(
    &user.wg_private_key_encrypted, // Plaintext private key stored here
    &user.assigned_ip,
    &state.server_config,
);
```

Because both `/api/users` and `/privacy/access` have **no authentication checks** (the security middleware only assigns an `AccessZone` extension but does not block any request), a remote attacker can:
1. Fetch all user IDs from `/api/users`.
2. Request `/privacy/access?user_id=<UUID>` for every user.
3. Retrieve every user's plaintext WireGuard private key and immediately compromise/impersonate them on the VPN network.

---

### [CRITICAL] Unauthenticated Direct Tool Execution Interface
* **Location**: `crates/op-web/src/handlers/tools.rs:77` and `crates/op-web/src/routes/mod.rs:118`
* **Vulnerability Type**: Missing Authentication for Critical Functionality
* **Exploitability**: Directly Exploitable

#### Description
The `/api/tool` and `/api/tools/:name/execute` endpoints allow direct, unauthenticated execution of registered tools:
```rust
pub async fn execute_tool_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(request): Json<DirectToolRequest>,
) -> Json<DirectToolResponse> {
    info!("Direct tool execution: {}", request.tool_name);
    execute_tool_internal(state, &request.tool_name, request.arguments).await
}
```

The application registers a massive suite of system administration tools (16k+ tools, including `shell_exec` and file writes). Because this router has no access control or authorization checks in place, any attacker who can reach the HTTP port (bound to `0.0.0.0` by default) can POST directly to `/api/tool` to execute `shell_exec` with arbitrary bash commands, achieving instant Remote Code Execution as the service user.