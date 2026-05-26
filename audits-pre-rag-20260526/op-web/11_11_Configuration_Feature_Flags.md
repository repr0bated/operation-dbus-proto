### 1. `std::env::var` Reads

| File | Line | Environment Variable | Fallback / Handling |
| :--- | :--- | :--- | :--- |
| `crates/op-web/src/email.rs` | 31 | `SMTP_HOST` | Defaults to `"localhost"` |
| `crates/op-web/src/email.rs` | 32 | `SMTP_PORT` | Defaults to `587` |
| `crates/op-web/src/email.rs` | 36 | `SMTP_USER` | Defaults to `String::new()` via `unwrap_or_default()` |
| `crates/op-web/src/email.rs` | 37 | `SMTP_PASS` | Defaults to `String::new()` via `unwrap_or_default()` |
| `crates/op-web/src/email.rs` | 38 | `SMTP_FROM_EMAIL` | Defaults to `"noreply@example.com"` |
| `crates/op-web/src/email.rs` | 40 | `SMTP_FROM_NAME` | Defaults to `"Privacy Router"` |
| `crates/op-web/src/email.rs` | 42 | `BASE_URL` | Defaults to `"http://localhost:8080"` |
| `crates/op-web/src/main.rs` | 33 | `PORT` | Defaults to `8080` |
| `crates/op-web/src/mcp_agents.rs` | 534 | `OP_COGNITIVE_MCP_AGENT_CONFIG` | Defaults to `/var/lib/op-dbus/cognitive-mcp-agents.json` |
| `crates/op-web/src/privacy_container.rs` | 53 | `PRIVACY_CONTAINER_IMAGE` | Defaults to `"images:alpine/3.19"` |
| `crates/op-web/src/privacy_container.rs` | 55 | `PRIVACY_CONTAINER_PREFIX` | Defaults to `"privacy-user-"` |
| `crates/op-web/src/privacy_container.rs` | 57 | `PRIVACY_CONTAINER_DEVICE` | Defaults to `"privacy0"` |
| `crates/op-web/src/privacy_container.rs` | 59 | `PRIVACY_CONTAINER_STORAGE_POOL` | Defaults to `None` |
| `crates/op-web/src/privacy_container.rs` | 63 | `PRIVACY_CONTAINER_ATTACH_BRIDGED_NIC`| Defaults to `true` |
| `crates/op-web/src/privacy_container.rs` | 126 | `PRIVACY_CONTAINER_BRIDGE` | Defaults to `"ovsbr0"` |
| `crates/op-web/src/privacy_routes.rs` | 58 | `PRIVACY_ROUTE_SHARED_SECRET` | Required; throws descriptive error on failure via `?` |
| `crates/op-web/src/privacy_routes.rs` | 104 | `PRIVACY_ROUTE_INGRESS_PORT` | Defaults to `"ovsbr0-sock"` |
| `crates/op-web/src/privacy_routes.rs` | 106 | `PRIVACY_ROUTE_NEXT_HOP` | Defaults to `"priv_wg"` |
| `crates/op-web/src/state.rs` | 45 | `GOOGLE_OAUTH_CLIENT_ID` | Optional; returns `None` safely via `?` |
| `crates/op-web/src/state.rs` | 46 | `GOOGLE_OAUTH_CLIENT_SECRET` | Optional; returns `None` safely via `?` |
| `crates/op-web/src/state.rs` | 47 | `GOOGLE_OAUTH_REDIRECT_URL` | Defaults to `"http://localhost:8080/api/privacy/google/callback"` |
| `crates/op-web/src/state.rs` | 241 | `OP_DBUS_GRPC_ADDR` | Defaults to `"http://10.200.0.2:50051"` |
| `crates/op-web/src/state.rs` | 368 | `OP_WEB_TOOL_SOURCE` | Defaults to standalone lookup (local discovery) |
| `crates/op-web/src/state.rs` | 371 | `OP_WEB_PULL_TOOLS_FROM_OP_DBUS` | Defaults to `false` |
| `crates/op-web/src/state.rs` | 377 | `OP_WEB_REMOTE_TOOL_URL` | Defaults to `http://{OP_DBUS_WEB_HOST}:{OP_DBUS_WEB_PORT}` |
| `crates/op-web/src/state.rs` | 384 | `OP_DBUS_WEB_HOST` | Defaults to `"127.0.0.1"` |
| `crates/op-web/src/state.rs` | 385 | `OP_DBUS_WEB_PORT` | Defaults to `"8081"` |
| `crates/op-web/src/privacy_network.rs` | 39 | `PRIVACY_BRIDGE_NAME` | Defaults to `"ovsbr0"` |
| `crates/op-web/src/privacy_network.rs` | 41 | `PRIVACY_WGCF_TUNNEL` | Defaults to `"wgcf"` |
| `crates/op-web/src/privacy_network.rs` | 43 | `PRIVACY_PORTS` | Defaults to `["priv_xray", "priv_warp", "priv_wg", "ovsbr0-mgmt", "ovsbr0-sock"]` |
| `crates/op-web/src/privacy_network.rs` | 51 | `PRIVACY_MGMT_CIDR` | Defaults to `"10.200.0.1/24"` |
| `crates/op-web/src/privacy_network.rs` | 53 | `PRIVACY_OPENFLOW_CONTROLLER` | Defaults to `"10.200.0.1:6653"` |
| `crates/op-web/src/privacy_network.rs` | 55 | `XRAY_INGRESS_IP` | Defaults to `"10.200.0.1"` |
| `crates/op-web/src/privacy_network.rs` | 57 | `PRIVACY_DATAPATH_TYPE` | Defaults to `"system"` |
| `crates/op-web/src/privacy_network.rs` | 59 | `PRIVACY_FAIL_MODE` | Defaults to `"standalone"` |
| `crates/op-web/src/handlers/vpn.rs` | 132 | `VPN_ENDPOINT` | Defaults to `"148.113.204.83:51820"` |
| `crates/op-web/src/handlers/openclaw.rs`| 23 | `OPENCLAW_BASE_URL` | Defaults to `"http://127.0.0.1:18789"` |
| `crates/op-web/src/handlers/openclaw.rs`| 29 | `OPENCLAW_DEFAULT_MODEL` | Defaults to `"openclaw:main"` |
| `crates/op-web/src/routes/mod.rs` | 207 | `OP_WEB_STATIC_DIR` | Checked via `.ok()`; falls back to embedded UI |
| `crates/op-web/src/routes/mod.rs` | 213 | `OP_WEB_STATIC_DIR` | Defaults to `"static"` |
| `crates/op-web/src/routes/admin.rs` | 240 | `OP_SELF_REPO_PATH` | Checked via `.is_ok()`; defaults to `None` |
| `crates/op-web/src/routes/admin.rs` | 241 | `OP_SELF_REPO_PATH` | Checked via `.ok()`; defaults to `None` |
| `crates/op-web/src/bin/op-dbus.rs` | 28 | `OP_DBUS_GRPC_LISTEN` | Defaults to `"10.200.0.2:50051"` |
| `crates/op-web/src/handlers/privacy.rs` | 580 | `WG_INTERFACE` | Defaults to `"wg0"` |
| `crates/op-web/src/handlers/privacy.rs` | 582 | `WG_SERVER_PUBKEY` | Defaults to active interface detection |
| `crates/op-web/src/handlers/privacy.rs` | 583 | `WG_SERVER_PUBLIC_KEY` | Defaults to active interface detection |
| `crates/op-web/src/handlers/privacy.rs` | 584 | `WIREGUARD_PUBLIC_KEY` | Defaults to active interface detection |
| `crates/op-web/src/handlers/privacy.rs` | 588 | `WG_SERVER_ENDPOINT` | Defaults to `"148.113.204.83:51820"` via fallback |
| `crates/op-web/src/handlers/privacy.rs` | 589 | `VPN_ENDPOINT` | Defaults to `"148.113.204.83:51820"` via fallback |
| `crates/op-web/src/handlers/privacy.rs` | 591 | `WG_ALLOWED_IPS` | Defaults to `"0.0.0.0/0, ::/0"` |
| `crates/op-web/src/handlers/privacy.rs` | 592 | `VPN_ALLOWED_IPS` | Defaults to `"0.0.0.0/0, ::/0"` |
| `crates/op-web/src/handlers/privacy.rs` | 594 | `WG_DNS` | Defaults to `"10.200.0.1"` |
| `crates/op-web/src/handlers/privacy.rs` | 595 | `VPN_DNS` | Defaults to `"10.200.0.1"` |
| `crates/op-web/src/handlers/privacy.rs` | 699 | `VPN_ENDPOINT` | Defaults to `"148.113.204.83:51820"` |

---

### 2. Environment Variables with No Default and No Error Handling

All parsed environment variables utilize either:
1. Native fallback defaults (`unwrap_or_else`, `unwrap_or`, `unwrap_or_default`).
2. Optional checks with graceful termination or propagation (`.ok()`, `.ok()?`).
3. Explicit error propagation through `Result` matching (`PRIVACY_ROUTE_SHARED_SECRET`).

No active panic or unsafe `.unwrap()` call was identified on raw `std::env::var` returns.

---

### 3. Cargo Features

#### Workspace Root (`Cargo.toml`)
*   **Features Defined:**
    *   `default = ["grpc"]`
    *   `grpc = []`

#### `op-web` Crate (`crates/op-web/Cargo.toml`)
*   No explicit custom features are defined.

#### Feature Additive Analysis
*   In Cargo, features are strictly **additive**. If any crate in the dependency resolution tree compiles `op-web` or `op-dbus` with default features enabled, those features will be merged globally across the workspace build.

---

### 4. Hardcoded Paths, Ports, and IP Addresses

#### Hardcoded File & Database Paths
*   `crates/op-web/src/groups_admin.rs:30` - `/var/lib/op-dbus/tool-groups.json` (Configuration path)
*   `crates/op-web/src/mcp_agents.rs:33` - `/var/lib/op-dbus/cognitive-mcp-agents.json` (Agent selection path)
*   `crates/op-web/src/state.rs:144` - `/var/lib/op-dbus/privacy-users.json` (User registry path)
*   `crates/op-web/src/state.rs:172` - `/var/lib/op-dbus/state.db` (Persistent SQLite path)
*   `crates/op-web/src/state.rs:261` - `/etc/op-dbus/llm-model` (LLM configuration path)
*   `crates/op-web/src/state.rs:262` - `/etc/op-dbus/llm-provider` (LLM provider configuration path)
*   `crates/op-web/src/users.rs:70` - `/var/lib/op-dbus/privacy-users.json` (User storage path)
*   `crates/op-web/src/handlers/llm.rs:157` - `/etc/op-dbus/llm-model` (LLM write path)
*   `crates/op-web/src/handlers/llm.rs:158` - `/etc/op-dbus/llm-provider` (LLM provider write path)
*   `crates/op-web/src/handlers/logs.rs:33` - `/var/log/op-web.log` (Tail read target)
*   `crates/op-web/src/handlers/logs.rs:34` - `/var/log/op-dbus.log` (Tail read target)
*   `crates/op-web/src/handlers/logs.rs:35` - `/tmp/op-web.log` (Tail read target)
*   `crates/op-web/src/routes/admin.rs:243` - `/etc/op-dbus/custom-prompt.txt` (Custom system prompt configuration path)
*   `crates/op-web/src/system_prompt_loader.rs:19` - `/home/jeremy/git/gemini-op-dbus/LLM-SYSTEM-PROMPT-COMPLETE.txt` (Absolute debug path)
*   `crates/op-web/src/system_prompt_loader.rs:20` - `/home/jeremy/op-dbus-v2/LLM-SYSTEM-PROMPT-COMPLETE.txt` (Absolute debug path)

#### Hardcoded Ports & IP Addresses
*   `crates/op-web/src/bin/op-dbus.rs:27` - `10.200.0.2:50051` (Default listen/gRPC gateway address)
*   `crates/op-web/src/state.rs:242` - `http://10.200.0.2:50051` (gRPC client routing fallback)
*   `crates/op-web/src/privacy_network.rs:15` - `10.200.0.1/24` (Xray routing/OVS bridge management network segment)
*   `crates/op-web/src/privacy_network.rs:16` - `10.200.0.1:6653` (OpenFlow controller fallback address)
*   `crates/op-web/src/privacy_network.rs:17` - `10.200.0.1` (Xray default ingress IP)
*   `crates/op-web/src/handlers/vpn.rs:133` - `148.113.204.83:51820` (WireGuard endpoint address fallback)
*   `crates/op-web/src/handlers/vpn.rs:138` - `10.100.0.0/24` (Client subnet allocation space)
*   `crates/op-web/src/handlers/vpn.rs:139` - `1.1.1.1` (DNS resolver fallback)
*   `crates/op-web/src/handlers/openclaw.rs:16` - `http://127.0.0.1:18789` (Local OpenClaw gateway connection)
*   `crates/op-web/src/handlers/openclaw.rs:18` - `127.0.0.1` (OpenClaw local host fallback)
*   `crates/op-web/src/handlers/openclaw.rs:115` - `18789` (Local OpenClaw service target port)
*   `crates/op-web/src/server.rs:53` - `127.0.0.1:3000` (Default service address configuration)

---

### 5. Production Security & Quality Audit Findings

#### CRITICAL: Path Traversal and Arbitrary File Write via User-Controlled Filename
*   **Location:** `crates/op-web/src/handlers/chat.rs:442`
*   **Vulnerability Type:** Path Traversal / Arbitrary File Write (CWE-22)
*   **Description:** The POST handler `save_transcript_handler` extracts the `filename` parameter directly from a user-supplied JSON payload without sanitization or directory-traversal prevention (e.g., matching `..` or path separators). It passes this directly to `format!("/tmp/{}", filename)`. This value is then written directly using `tokio::fs::write`.
*   **Impact:** A remote, unauthenticated attacker can execute a directory traversal attack (e.g., passing `filename: "../etc/cron.d/exploit"`), writing arbitrary content (the conversation logs/payload) to sensitive system paths. This allows immediate Remote Code Execution (RCE) via cron or other system configuration files.

#### CRITICAL: Unauthenticated Remote Tool Execution (Complete Authorization Bypass)
*   **Location:** `crates/op-web/src/routes/mod.rs:218`
*   **Vulnerability Type:** Missing Authorization / Authentication (CWE-306)
*   **Description:** The complete API router mounts administrative tool-execution routes `/api/tool` and `/api/tools/:name/execute` without enforcing authorization. While `ip_security_middleware` resolves the client IP and appends an `AccessZone` request extension, neither the middleware nor the handlers (`execute_tool_handler`, `execute_named_tool_handler`) ever check the zone or reject unauthorized requests.
*   **Impact:** Any unauthenticated remote client can POST to `/api/tool` or `/api/tools/:name/execute` and execute arbitrary system-level commands, alter Open vSwitch bridges, inspect directory contents, or manipulate dinit services, resulting in a total compromise of the host system.

#### CRITICAL: Hardcoded API Security Bypass Tokens
*   **Location:** `crates/op-web/src/middleware/security.rs:13-14`
*   **Vulnerability Type:** Hardcoded Credentials (CWE-798)
*   **Description:** The security middleware implements a hardcoded static array of `BYPASS_API_KEYS`:
    ```rust
    const BYPASS_API_KEYS: &[&str] = &[
        "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
        "test-key-huggingface-2024",            // Hugging Face test key
    ];
    ```
    If present in the headers, these keys bypass the default IP access zones and promote the connection's access level to `TrustedMesh`.
*   **Impact:** Because these keys are statically compiled into the binary, they cannot be rotated. Any party with source-code access can exploit these tokens to bypass all zone-based protections.

#### HIGH: Unauthenticated System Prompt Manipulation
*   **Location:** `crates/op-web/src/routes/mod.rs:114`
*   **Vulnerability Type:** Privilege Escalation / Unchecked PUT Handlers (CWE-285)
*   **Description:** The PUT route `/api/chat/system-prompt` points to `handlers::chat::update_system_prompt_handler`. No authorization checks are applied to this route.
*   **Impact:** Attackers can dynamically rewrite the LLM system instructions, inserting malicious formatting, deleting strict boundaries (such as "NEVER suggest CLI commands"), or altering tool constraints.