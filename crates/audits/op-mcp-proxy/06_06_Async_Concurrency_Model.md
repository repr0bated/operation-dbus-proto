# Production Security and Quality Audit: `op-mcp-proxy`

## Section 1: Async & Concurrency Audit

### 1.1 Concurrency Metrics

* **`async fn` Count**: 39
* **`tokio::spawn` Count**: 3
* **`tokio::task::spawn_blocking` Count**: 0

---

### 1.2 Reactor-Blocking Operations Inside Asynchronous Contexts

A critical architectural issue in this crate is the pervasive execution of synchronous, blocking I/O and OS process invocations directly on the Tokio threadpool without offloading them to a blocking-friendly context (such as `tokio::task::spawn_blocking` or `tokio::runtime::Handle::spawn_blocking`). This causes reactor starvation, delays timer execution, and degrades proxy throughput.

#### 1. Synchronous Process Command Execution Inside Async Functions
In `crates/op-mcp-proxy/src/session.rs:162`, `get_or_create_session` executes `Self::get_local_wireguard_pubkey()?` which performs a synchronous, blocking shell execution of the `wg` binary:
```rust
// crates/op-mcp-proxy/src/session.rs:98-105
let output = Command::new("wg")
    .args(["show", "wg0", "public-key"])
    .output();
```
This halts the calling thread's worker loop completely while waiting for the OS to fork, execute, and yield the child process output. The same pattern is present in:
* `crates/op-mcp-proxy/src/gcloud_auth.rs:309-322` (`run_gcloud_access_token`)
* `crates/op-mcp-proxy/src/gcloud_auth.rs:325-337` (`run_gcloud_access_token_no_scopes`)

#### 2. Synchronous Database Operations Inside Async Functions
The `SessionManager` holds a synchronous SQLite connection via `Arc<Mutex<rusqlite::Connection>>`. Within several asynchronous methods, blocking database queries are run on the active executor thread:
* `crates/op-mcp-proxy/src/session.rs:167-187` (`db.query_row`)
* `crates/op-mcp-proxy/src/session.rs:197-200` (`db.execute`)
* `crates/op-mcp-proxy/src/session.rs:217-222` (`db.query_row`)
* `crates/op-mcp-proxy/src/session.rs:228-237` (`db.execute`)
* `crates/op-mcp-proxy/src/session.rs:259-262` (`db.execute`)
* `crates/op-mcp-proxy/src/session.rs:277-283` (`db.query_row`)
* `crates/op-mcp-proxy/src/session.rs:294-298` (`db.execute`)

#### 3. Synchronous File System I/O Inside Async Functions
Standard synchronous file reads are carried out directly inside async execution contexts:
* `crates/op-mcp-proxy/src/gcloud_auth.rs:158` (`std::fs::read_to_string(path)`)
* `crates/op-mcp-proxy/src/gcloud_auth.rs:264` (`std::fs::read_to_string(&paths.credentials)`)
* `crates/op-mcp-proxy/src/gcloud_auth.rs:288` (`std::fs::read_to_string(&paths.credentials)`)
* `crates/op-mcp-proxy/src/gcloud_auth.rs:300` (`std::fs::read_to_string(&paths.adc)`)
* `crates/op-mcp-proxy/src/cloudaicompanion.rs:547` (`std::fs::read_to_string(&path)`)
* `crates/op-mcp-proxy/src/cloudaicompanion.rs:573` (`std::fs::read_to_string(&path)`)
* `crates/op-mcp-proxy/src/cloudaicompanion.rs:625` (`std::fs::write(&path, ...)`)

#### 4. Synchronous Stdin and Stdout Locking Inside `main`
In `crates/op-mcp-proxy/src/main.rs:117-246`, the asynchronous entrypoint loops over `stdin.lock().lines()` and flushes output via `stdout.flush()` directly on the main thread:
```rust
for line in stdin.lock().lines() { ... }
```
Because the standard input is synchronized and blocks until external input is physically ready, this suspends the entire async task runtime if `HTTP_ONLY` is not set.

---

### 1.3 Unhandled Background Task Futures and Dropped JoinHandles

When spawning detached background tasks, the returned `JoinHandle` futures are discarded. This prevents proper panic propagation, error logging, and cleanup tracking during shutdown:

* **`crates/op-mcp-proxy/src/direct_llm.rs:44`**:
  ```rust
  tokio::spawn(async move { ... });
  ```
  The auto-refresh loop runs indefinitely. If this background task panics (for example, due to a DNS resolution failure inside a TLS connection setup), the panic is swallowed, silently halting token refreshes for the entire application lifetime.

* **`crates/op-mcp-proxy/src/vertex_grpc.rs:69`**:
  ```rust
  tokio::spawn(async move { ... });
  ```
  Prefetches and caches the initial Vertex token. If this task fails, the error is logged as a warning, but there is no mechanism to track whether the task has finalized before processing inbound HTTP requests.

* **`crates/op-mcp-proxy/src/main.rs:63`**:
  ```rust
  tokio::spawn(async move { ... });
  ```
  Launches the background HTTP server without retaining a handle. This makes it impossible to implement a coordinated graceful shutdown flow.

---

## Section 2: Schema-As-Code Discipline Violations

This codebase bypasses versioned schema engines (e.g., Protocol Buffers or OpenAPI YAML definitions) in favor of ad-hoc JSON parsing, raw string matching, and brittle memory-mapping patterns.

### 2.1 Raw-Byte Memory Mapping Layout Interoperability
In `crates/op-mcp-proxy/src/sled.rs:13-25`, the binary representation of `plugin_schema.dat` is parsed using hardcoded byte slices:
```rust
let wg_pubkey     = &bytes[0..32];
let mutation_index = u64::from_le_bytes(bytes[32..40].try_into().ok()?);
let is_valid       = bytes[40] != 0;
let footprint      = &bytes[48..80];
let nextdns_profile = fixed_str(&bytes[192..208]);
let subid           = fixed_str(&bytes[96..160]);
let control_source  = fixed_str(&bytes[160..192]);
```
This zero-copy layout is brittle. If the upstream `op-identity` crate modifies its fields, alters its compilation padding, or uses a compiler version that inserts padding bytes to satisfy alignment, this parser will read corrupt data or crash. The interface must be governed by a structured, versioned serialization contract such as Protocol Buffers or FlatBuffers.

### 2.2 Unstructured JSON Parsing and Ad-Hoc Parameter Retrieval
* **`crates/op-mcp-proxy/src/direct_llm.rs:148-195` (`handle`)**:
  Accepts a generic `simd_json::OwnedValue` as an input parameter and performs ad-hoc key extractions like `req.get("id")` and `params.get("model")`.
* **`crates/op-mcp-proxy/src/direct_llm.rs:197-223` (`extract_prompt`)**:
  Navigates unstructured nested arrays manually, making ad-hoc string comparisons to identify content types (`"role"`, `"text"`, etc.).
* **`crates/op-mcp-proxy/src/cloudaicompanion.rs:247-268` (`send_generate_request`)**:
  Builds nested request structures using raw JSON interpolation macros (`serde_json::json!`) instead of strongly typed, schema-generated structs.

---

## Section 3: Production Security & Quality Findings

### Finding 1: Local Privilege Escalation & Denial of Service via `/dev/shm` Shared Memory Identity Sled Hijacking
* **Citations**: 
  * `crates/op-mcp-proxy/src/sled.rs:13-58`
  * `crates/op-mcp-proxy/src/main.rs:34-45`
  * `crates/op-mcp-proxy/src/main.rs:155-168`
* **Severity**: **Critical**
* **Threat Model**:
  The identity sled file is read directly from `/dev/shm/plugin_schema.dat`. On Linux systems, `/dev/shm` is a world-writable (`1777`) shared-memory directory. Since no validation of the file owner, inode metadata, or directory permissions is performed, any local user can pre-create, overwrite, truncate, or inject malicious payloads into `/dev/shm/plugin_schema.dat`.
* **Impact**:
  1. **Identity Spoofing**: An attacker can overwrite `wireguard_pubkey` and `footprint_hex` to impersonate other clients. These values are directly extracted and injected into the outgoing DBus/gRPC control plane headers (`x-ghostbridge-footprint` and `x-ghostbridge-trace-id`) in `main.rs:155-168`, bypassing authorization.
  2. **Denial of Service (SIGBUS/Crash)**: The implementation performs unsafe memory-mapping (`MmapOptions::new().len(SLED_SIZE).map(&file)`) and attempts to access 208 bytes of memory without verifying that the backing file size is actually at least 208 bytes. If a local attacker truncates the file to 0 bytes or writes a smaller file, any read access to the mmapped slice (`bytes[..SLED_SIZE]`) will instantly trigger a `SIGBUS` signal, terminating the proxy process.
* **Remediation**:
  1. Relocate the identity sled to a restricted runtime path (such as `/run/mcp-proxy/` or a user-specific runtime directory path `/run/user/<UID>/`) and restrict directory permissions to `0700`.
  2. Explicitly verify the metadata size and owner UID of `/dev/shm/plugin_schema.dat` using `std::fs::metadata` before establishing the memory mapping.

---

### Finding 2: Plaintext Storage of Sensitive Google Cloud OAuth Tokens with Insecure Permissions
* **Citations**: 
  * `crates/op-mcp-proxy/src/session.rs:46-81`
* **Severity**: **High**
* **Threat Model**:
  The database manager initializes and writes to a SQLite database (`sessions.db`) located in the home directory (`dirs::data_dir()`). The database structure contains an `oauth_token` field:
  ```sql
  CREATE TABLE IF NOT EXISTS sessions (
      session_id TEXT PRIMARY KEY,
      pubkey TEXT NOT NULL,
      user_email TEXT,
      oauth_token TEXT,
      token_expires_at INTEGER,
      created_at INTEGER NOT NULL,
      last_seen_at INTEGER NOT NULL
  );
  ```
  Sensitive Google Cloud OAuth access tokens are stored in plain text. Neither the database directory creation (`std::fs::create_dir_all`) nor the SQLite connection setup enforces strict owner-only file permissions (umask / POSIX mode `0600`).
* **Impact**:
  In a multi-user environment, or if the home directory has default read access permissions (e.g. `0755` or umask `0022`), any other local user on the machine can read `sessions.db` directly, extract the active Google Cloud OAuth tokens, and impersonate the developer or gain unauthorized access to Google Cloud resources.
* **Remediation**:
  1. Explicitly adjust the file permissions of both the target folder and `sessions.db` to `0700` / `0600` on Unix systems before opening the SQLite database.
  2. Encrypt the OAuth token column at rest using an authenticated encryption scheme (such as AES-GCM or ChaCha20-Poly1305) with a key sourced from the secure OS system keyring.

---

### Finding 3: Database Lock Starvation via Async Network Calls Held Across Mutex Guards
* **Citations**: 
  * `crates/op-mcp-proxy/src/session.rs:161-248`
* **Severity**: **High**
* **Threat Model**:
  The method `get_or_create_session` acquires a Tokio asynchronous lock on the SQLite connection:
  ```rust
  let db = self.db.lock().await;
  ```
  While holding this lock, it invokes `self.gcloud_auth.get_token().await`:
  ```rust
  let (oauth_token, token_expires_at) = match self.gcloud_auth.get_token().await { ... }
  ```
  The helper `get_token().await` triggers potentially slow network connections, including refreshing extension auth caches via HTTP POST, reading local system configuration parameters, or running external command processes (e.g., `gcloud` or `wg`).
* **Impact**:
  During the entire duration of the network request and shell subprocess execution (which can take several seconds if there are transient timeouts or network lag), the SQLite database remains locked. All other incoming proxy transactions attempting to call `touch_session`, `get_valid_token`, or `get_or_create_session` are forced to wait, causing immediate request queues and complete proxy lockups.
* **Remediation**:
  Release the database lock before making asynchronous HTTP or command calls. Resolve the token first, and then acquire the database connection lock only to execute the fast, local SQLite transactions:

```rust
// Proposed structural change:
let token_result = self.gcloud_auth.get_token().await;
let db = self.db.lock().await;
// perform DB insertions with token_result here...
```

---

### Finding 4: CWE-426: Untrusted Search Path Vulnerability in Process Executions
* **Citations**: 
  * `crates/op-mcp-proxy/src/session.rs:101-103` (`Command::new("wg")`)
  * `crates/op-mcp-proxy/src/session.rs:123-125` (`Command::new("wg")`)
  * `crates/op-mcp-proxy/src/gcloud_auth.rs:312` (`Command::new("gcloud")`)
  * `crates/op-mcp-proxy/src/gcloud_auth.rs:327` (`Command::new("gcloud")`)
* **Severity**: **Medium**
* **Threat Model**:
  The application launches external utilities (`wg` and `gcloud`) using relative command lookups. It relies entirely on the executing process's `PATH` environment variable to resolve the path to these binaries.
* **Impact**:
  If the proxy binary runs in an environment where the `PATH` variable can be manipulated by local users or service wrappers (especially when run with elevated permissions needed for WireGuard management), an attacker can place a malicious executable named `wg` or `gcloud` in a higher-priority directory to execute arbitrary code with the permissions of the proxy process.
* **Remediation**:
  Enforce absolute paths for binary resolution (e.g., `/usr/bin/wg` or `/usr/bin/gcloud`) or use safe defaults and look up paths using a secure configuration template.

---

### Finding 5: Silent Unproxied Connection Fallback Leaking Client Identity
* **Citations**: 
  * `crates/op-mcp-proxy/src/cloudaicompanion.rs:118-128`
* **Severity**: **Medium**
* **Threat Model**:
  The application is designed to route LLM queries through an Xray SOCKS5 proxy to enforce privacy boundaries and NextDNS profiles. During HTTP client construction:
  ```rust
  if let Some(proxy_url) = socks_proxy {
      match reqwest::Proxy::all(proxy_url) {
          Ok(proxy) => {
              client_builder = client_builder.proxy(proxy);
              info!(proxy = %proxy_url, "LLM HTTP calls routed through Xray SOCKS5");
          }
          Err(e) => warn!("Invalid SOCKS proxy URL {}: {}", proxy_url, e),
      }
  }
  ```
* **Impact**:
  If the configured proxy URL is invalid, or if the initialization encounters an error, the client logs a warning but proceeds to build the `reqwest::Client` anyway. The client will then silently fall back to making unproxied, direct connections to Google Cloud and Google APIs. This leaks the user's raw client IP address and metadata, bypassing the privacy routing boundary.
* **Remediation**:
  If a proxy is requested or if the identity sled is valid, make proxy configuration errors fatal (e.g., return an `Err` instead of warning and continuing) to prevent silent fallback and identity leakage.

---

### Finding 6: Resource Overhead and Port Exhaustion via Repetitive HTTP Client Initialization
* **Citations**: 
  * `crates/op-mcp-proxy/src/gcloud_auth.rs:360-366`
  * `crates/op-mcp-proxy/src/cloudaicompanion.rs:596-602`
* **Severity**: **Low**
* **Threat Model**:
  Within the async token refresh utilities (`refresh_extension_access_token` and `refresh_gemini_cli_token`), a brand-new `reqwest::Client` instance is instantiated inside the helper function on each call:
  ```rust
  let resp = reqwest::Client::new()
      .post("https://oauth2.googleapis.com/token")
      ...
  ```
* **Impact**:
  Creating a new `reqwest::Client` on each token refresh destroys connection pooling. Each invocation allocates fresh sockets, performs new TCP and TLS handshakes, and drops them immediately after, leading to high latency and potential local ephemeral port exhaustion under peak traffic conditions.
* **Remediation**:
  Store and reuse a single `reqwest::Client` instance within the `GCloudAuth` struct, or pass a reference to a shared client down to the helper functions.