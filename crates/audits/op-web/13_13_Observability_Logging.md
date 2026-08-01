# Production Security & Quality Audit: `op-web`

---

### 1. Observability: Tracing Macros vs. `println!`

The following table summarizes the occurrences of `tracing::` macros (`info!`, `warn!`, `error!`, `debug!`) and `println!` across the codebase.

| File | `info!` | `warn!` | `error!` | `debug!` | `println!` |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `crates/op-web/src/email.rs` | 5 | 0 | 0 | 0 | 4 |
| `crates/op-web/src/groups_admin.rs` | 3 | 0 | 1 | 0 | 0 |
| `crates/op-web/src/main.rs` | 5 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/mcp.rs` | 3 | 0 | 1 | 1 | 0 |
| `crates/op-web/src/mcp_agents.rs` | 7 | 4 | 2 | 1 | 0 |
| `crates/op-web/src/mcp_compact.rs` | 6 | 1 | 6 | 1 | 0 |
| `crates/op-web/src/mcp_discovery.rs` | 1 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/privacy_container.rs` | 1 | 0 | 0 | 1 | 0 |
| `crates/op-web/src/server.rs` | 2 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/state.rs` | 22 | 11 | 0 | 1 | 0 |
| `crates/op-web/src/system_prompt_loader.rs` | 1 | 2 | 0 | 0 | 0 |
| `crates/op-web/src/websocket.rs` | 3 | 0 | 1 | 1 | 0 |
| `crates/op-web/src/privacy_network.rs` | 8 | 2 | 0 | 0 | 0 |
| `crates/op-web/src/users.rs` | 7 | 2 | 0 | 0 | 0 |
| `crates/op-web/src/bin/op-dbus.rs` | 1 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/agents.rs` | 2 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/auth_bridge.rs` | 3 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/chat.rs` | 1 | 0 | 1 | 0 | 0 |
| `crates/op-web/src/handlers/logs.rs` | 0 | 0 | 1 | 0 | 0 |
| `crates/op-web/src/handlers/tools.rs` | 2 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/vpn.rs` | 0 | 1 | 1 | 0 | 0 |
| `crates/op-web/src/handlers/websocket.rs` | 3 | 0 | 1 | 0 | 0 |
| `crates/op-web/src/handlers/openclaw.rs` | 0 | 0 | 1 | 5 | 0 |
| `crates/op-web/src/handlers/privacy.rs` | 6 | 1 | 17 | 0 | 0 |
| `crates/op-web/src/middleware/security.rs` | 1 | 0 | 0 | 1 | 0 |
| `crates/op-web/src/orchestrator/anti_hallucination.rs` | 0 | 1 | 0 | 0 | 0 |
| `crates/op-web/src/orchestrator/execution.rs` | 0 | 0 | 2 | 0 | 0 |
| `crates/op-web/src/orchestrator/parsing.rs` | 2 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/orchestrator/process.rs` | 9 | 5 | 2 | 1 | 0 |
| `crates/op-web/src/routes/admin.rs` | 3 | 0 | 1 | 0 | 0 |
| `crates/op-web/src/routes/chat.rs` | 2 | 0 | 1 | 0 | 0 |
| `crates/op-web/src/routes/llm.rs` | 2 | 0 | 3 | 0 | 0 |
| **Total** | **114** | **29** | **42** | **13** | **4** |

#### Metrics Instrumentation
Standard telemetry instrumentation via the `prometheus` or `metrics` crates is **entirely missing** from `op-web`. Instead, the application relies on ad-hoc metrics collection:
1. `crates/op-web/src/state.rs:261` spawns a background thread polling `sysinfo` data (CPU/memory) and pushes it via SSE as JSON text.
2. `crates/op-web/src/handlers/dashboard.rs:58` reads system load and memory allocations by manually parsing `/proc/loadavg` and `/proc/meminfo`.

---

### 2. Swallowed Errors

#### Critical: Silent DB Overwrite on Load Failure (Potential Data Loss)
* **Citation:** `crates/op-web/src/users.rs:106`
* **Description:** Inside `UserStore::new`, the call to `store.load().await` is suffixed with `.ok()`, completely discarding any errors during deserialization, file access, or decryption:
  ```rust
  store.load().await.ok();
  ```
  If `/var/lib/op-dbus/privacy-users.json` becomes corrupted, contains invalid JSON, or experiences a read error, `users` is initialized as empty. When a subsequent write/signup occurs, `self.save().await` is triggered, which overwrites the entire database file with a single user record, resulting in **complete silent data loss** of all other registered accounts and their configurations.

#### Minor: Swallowed File Configuration Failures
* **Citation:** `crates/op-web/src/routes/llm.rs:144` and `crates/op-web/src/routes/llm.rs:173`
* **Description:** The endpoints `/api/llm/model` and `/api/llm/provider` swallow the filesystem persist results using `let _ =`:
  ```rust
  let _ = persist_model(&request.model).await;
  ```
  If the host system has a read-only filesystem or insufficient permissions on `/etc/op-dbus/`, the server silently fails to write the configuration to disk while falsely reporting success (`"success": true`) to the client.

#### Minor: Ignored Lagged Stream Packets
* **Citation:** `crates/op-web/src/mcp.rs:158`
* **Description:** Lagged notifications from the broadcast channel are silently discarded without telemetry increment or log warning:
  ```rust
  Err(_) => None, // Skip lagged messages
  ```

---

### 3. PII & Secrets Leaks in Log Output

#### Critical: Magic Token and User Email Leaked to Console & Logs
* **Citation:** `crates/op-web/src/email.rs:69-76`
* **Description:** When SMTP email is not fully configured, the application dumps the private magic link, verification token, and user email address to the standard tracing logs at `info!` level, as well as printing them to stdout/stderr:
  ```rust
  info!("🔗 MAGIC LINK (no SMTP configured):");
  info!("   To: {}", to_email);
  info!("   URL: {}", magic_url);
  info!("   Token: {}", token);
  println!("\n=== MAGIC LINK FOR TESTING ===");
  println!("Email: {}", to_email);
  println!("Link: {}", magic_url);
  ```
  Even when SMTP is configured, user emails are logged at `info!` level on successful delivery (`crates/op-web/src/email.rs:125` and `crates/op-web/src/handlers/privacy.rs:188`). Under production configurations, these tokens will leak directly to systemd journal logs, granting any system observer the ability to hijack arbitrary VPN accounts.

#### Major: Raw WebSocket Input Logged
* **Citation:** `crates/op-web/src/websocket.rs:89`
* **Description:** The WebSocket chat server logs the entire message payload received from clients at `debug!` level. This raw payload can contain full user prompts (PII) and potentially custom API keys sent in JSON structures.

#### Major: MCP Tool Arguments Logged at `info!` Level
* **Citation:** `crates/op-web/src/mcp_compact.rs:423`
* **Description:** Compact and agent meta-tools log the execution name and their full arguments directly to `info!`:
  ```rust
  info!("Executing underlying tool: {} with args: {}", tool_name, arguments);
  ```
  If tools are called with sensitive file paths, environment variables, or secrets, they are written to the main server logs in plaintext.

---

### 4. Non-Compliance with Schema-as-Code Discipline

This repository violates the project's schema-as-code discipline (Protocol Buffers / OSCAL) by declaring dozens of ad-hoc JSON structs annotated with `#[derive(Serialize, Deserialize)]` to manage core configuration state and API interactions.

#### Incus State Contracts
* **Citation:** `crates/op-web/src/privacy_container.rs:33-65`
* **Description:** State tracking of container allocations (`IncusState` and `IncusInstance`) are represented as ad-hoc, unversioned Rust structs serialized directly to the state coordinator instead of being defined as a versioned Protobuf schema.

#### OpenFlow Flow Policies
* **Citation:** `crates/op-web/src/privacy_openflow.rs:10-53`
* **Description:** The complete SDN configuration contract (`OpenFlowConfig`, `BridgeFlowConfig`, `FlowEntry`, and `FlowAction`) is managed entirely via ad-hoc JSON serialization structs, risking runtime incompatibility with the state manager.

#### Privacy Route Identifiers
* **Citation:** `crates/op-web/src/privacy_routes.rs:12-32`
* **Description:** Router definitions (`PrivacyRoutesState` and `PrivacyRoute`) are represented as local Rust structs lacking versioning fields or standardized OSCAL representation.

---

### 5. Critical & Directly Exploitable Security Vulnerabilities

#### Critical: Unauthenticated Retrieval of VPN Private Keys and Configs
* **Citation:** `crates/op-web/src/handlers/privacy.rs:307-402`
* **Description:** The route `/privacy/access` is mapped to the `privacy_access_message` handler in `crates/op-web/src/routes/mod.rs` without any IP-restriction check or token validation. The handler receives `user_id` as a raw query parameter:
  ```rust
  pub async fn privacy_access_message(
      Extension(state): Extension<Arc<AppState>>,
      Query(query): Query<AccessQuery>,
  ) -> Html<String> {
      ...
      } else if let Some(user_id) = &query.user_id {
          match state.user_store.get_user(user_id).await {
          Some(user) if user.email_verified => {
              let config = generate_client_config(
                  &user.wg_private_key_encrypted,
                  &user.assigned_ip,
                  &state.server_config,
              );
  ```
  If a valid `user_id` is supplied, the endpoint immediately yields the complete WireGuard configuration—**including the unencrypted client private key** (`wg_private_key_encrypted` is stored in plaintext despite its name)—to any requester.

#### Critical: Public Exposure of User Directory (Aiding Account Harvest)
* **Citation:** `crates/op-web/src/handlers/users.rs:20-42`
* **Description:** The route `/api/users` maps to `list_users_handler` with **zero authentication checks**. Although `security::ip_security_middleware` is active on the route layer, it only *attaches* an `AccessZone` extension and never rejects requests. Any anonymous client on the internet can call `/api/users` and harvest a JSON array of all registered users, containing their `id` (the UUID required for the leak above), `email`, and `wireguard_public_key`.
* **Exploit Vector:**
  1. An attacker queries `GET http://<host>:8080/api/users` anonymously to obtain a list of all user `id`s.
  2. The attacker loops through the ids, calling `GET http://<host>:8080/privacy/access?user_id=<id>`.
  3. The attacker steals the unencrypted private keys and IP profiles for every single VPN client.