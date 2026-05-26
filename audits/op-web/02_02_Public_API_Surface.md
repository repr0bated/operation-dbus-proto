# Public API Surface Audit: `op-web`

## 1. Public Item Count & Statistics
* **Total Public Items (`pub`):** 214
* **Glob Re-exports:** 1 (Flagged below)

---

## 2. Top 10 Most Important Public Exports

| # | Public Item | File & Line | Description |
|---|---|---|---|
| 1 | `pub struct AppState` | `crates/op-web/src/state.rs:59` | Central shared state engine holding gRPC pools, agent registries, databases, and encryption configs. |
| 2 | `pub struct UnifiedOrchestrator` | `crates/op-web/src/orchestrator/mod.rs:18` | Core multi-turn cognitive execution loop mediating between LLMs and native system tools. |
| 3 | `pub fn create_router(...) -> Router` | `crates/op-web/src/routes/mod.rs:30` | Main server router assembly mapping all REST endpoints, SSE streams, and MCP endpoints. |
| 4 | `pub struct WebServer` | `crates/op-web/src/server.rs:79` | Server driver running the underlying Axum / hyper binding loop and applying rate limit layers. |
| 5 | `pub struct UserStore` | `crates/op-web/src/users.rs:73` | Persistent user directory for magic links, WireGuard allocations, and third-party API keys. |
| 6 | `pub struct AgentsMcpState` | `crates/op-web/src/mcp_agents.rs:434` | State coordinator for streaming and pre-warming cognitive agents under MCP tool context. |
| 7 | `pub async fn query_plugin_state<T>(...) -> Result<Option<T>>` | `crates/op-web/src/state_manager_client.rs:21` | D-Bus system bus client for querying raw plugin state directly from `org.opdbus.StateManager`. |
| 8 | `pub async fn publish_user_privacy_route(...) -> Result<String>` | `crates/op-web/src/privacy_routes.rs:42` | Dynamically updates system-level routing tables via state-manager serialization. |
| 9 | `pub async fn ip_security_middleware(...) -> Response` | `crates/op-web/src/middleware/security.rs:106` | Edge isolation middleware classifying inbound connections into isolated access zones. |
| 10 | `pub static ref GROUPS_CONFIG: GroupsConfig` | `crates/op-web/src/groups_admin.rs:114` | Global lazy-initialized tool group mapping configuration. |

---

## 3. Glob Re-Exports Flagged
* **File:** `crates/op-web/src/orchestrator/mod.rs:7`  
  **Re-export:** `pub use types::*;`  
  *Risk:* Pulls the entire internal types layout (events, configurations, response objects, constants) into the outer orchestrator namespace, increasing potential for namespace collisions and preventing structural encapsulation of orchestration internals.

---

## 4. Key Security & Implementation Risks

### A. Unsafe Code Analysis
* **`crates/op-web/src/handlers/websocket.rs:88`**
  ```rust
  let ws_msg: Result<WsMessage, _> = unsafe { simd_json::from_str(&mut raw) };
  ```
  *Risk:* Invoking `simd_json::from_str` with raw, mutable string buffers received directly from untrusted WebSockets. If the WebSocket payload is structured maliciously or contains invalid UTF-8 (due to lack of strict sanitation before mutating), it can violate `simd-json`'s structural assumptions, resulting in memory corruption or segmentation faults.
* **`crates/op-web/src/state_manager_client.rs:31`**
  ```rust
  let query_state: QueryStateResponse = unsafe { simd_json::from_str(&mut state_json) };
  ```
  *Risk:* Parses external payload received from D-Bus system bus using `unsafe` JSON deserialization. If the D-Bus provider is compromised or spoofed, it can yield arbitrarily structured strings that trigger undefined behavior during parsing.
* **`crates/op-web/src/groups_admin.rs:51`**
  ```rust
  if let Ok(saved) = unsafe { simd_json::from_str::<HashMap<String, EnabledGroups>>(&mut raw) }
  ```
  *Risk:* Performs unsafe serialization of local files on disk. If the target file `/var/lib/op-dbus/tool-groups.json` is modified or truncated by an unprivileged system actor, loading the application state will trigger memory corruption.

### B. Panics & Unchecked `unwrap` Usage
* **`crates/op-web/src/server.rs:138`**
  ```rust
  let governor_conf = GovernorConfigBuilder::default()
      .per_second(rate_limit_per_sec)
      .burst_size(self.config.rate_limit.burst_size as u32)
      .finish()
      .unwrap();
  ```
  *Risk:* If `rate_limit_per_sec` or `burst_size` evaluates to zero or overflows, `finish()` returns an `Err`. Calling `.unwrap()` will panic during server initialization, causing a denial of service.
* **`crates/op-web/src/handlers/openclaw.rs:135`**
  ```rust
  let client = Client::builder()
      .timeout(Duration::from_secs(60))
      .build()
      .unwrap();
  ```
  *Risk:* Panics if the underlying system's TLS backend fails to initialize during client build. Use `?` instead of `.unwrap()`.

### C. Deep Copy / Clone Abuse
* **`crates/op-web/src/orchestrator/process.rs:188`**
  ```rust
  let request = ChatRequest {
      messages: messages.clone(),
      ...
  ```
  *Risk:* Clones the entire conversation history vector (`messages`) on every turn inside a loop. Since conversation histories grow with each turn, this results in $O(N^2)$ allocation complexity, stressing the allocator and opening vectors to memory exhaustion attacks.
* **`crates/op-web/src/mcp_compact.rs:408`**
  ```rust
  output: Some(res.clone()),
  ```
  *Risk:* Clones the massive JSON payloads returned from generic tools directly into auditing structures. These values should be moved or managed using reference-counted pointers (`Arc<Value>`) rather than deep copying the entire DOM tree.

### D. Public API Leakage / Internal Exposure
* **`crates/op-web/src/state.rs:93`**
  ```rust
  pub grpc_client: Arc<RemoteOperationClient>,
  ```
  *Risk:* Publicly exposes the gRPC bridge client, permitting arbitrary external consumers of `AppState` to invoke untrusted network operations on internal services without going through the web gatekeepers.
* **`crates/op-web/src/users.rs:16`**
  ```rust
  pub struct PrivacyUser {
      ...
      pub wg_private_key_encrypted: String,
      pub api_credentials: Option<UserApiCredentials>,
  }
  ```
  *Risk:* Exposes raw encrypted private keys and sensitive LLM API keys directly on a public model structure across crate boundaries. This data should be strictly encapsulated and accessible only via bounded, secure accessor methods.
* **`crates/op-web/src/groups_admin.rs:30`**
  ```rust
  pub struct GroupsConfig {
      profiles: RwLock<HashMap<String, EnabledGroups>>,
      trusted_networks: RwLock<Vec<String>>,
  }
  ```
  *Risk:* Exposes the inner locking mechanisms of `GroupsConfig` publicly, enabling external modules to hold locks indefinitely and trigger thread-level deadlocks across the server.