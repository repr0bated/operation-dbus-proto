# Production Security and Quality Audit: op-mcp-proxy

---

### 1. ROLE: Build Analysis

#### Cargo.toml Analysis
* **Edition**: `crates/op-mcp-proxy/Cargo.toml` specifies `edition = "2021"`.
* **Rust Version**: No minimum Rust version (`rust-version`) is specified in `crates/op-mcp-proxy/Cargo.toml` or the workspace `Cargo.toml`.
* **Bins & Examples**: No custom `[[bin]]` or `[[example]]` targets are defined in `crates/op-mcp-proxy/Cargo.toml`. Standard Cargo layout rules apply, meaning `src/main.rs` is compiled as the primary binary `op-mcp-proxy`.
* **Build Script (`build.rs`)**: No `build.rs` source code is present in the provided files section. However, `crates/op-mcp-proxy/Cargo.toml` defines `tonic-build` under `[build-dependencies]`, indicating a codegen stage is active during build time.

#### Workspace Inheritance vs. Local Overrides
The codebase exhibits several anomalies regarding dependency inheritance:
1. **Version Mismatches**: `crates/op-mcp-proxy/Cargo.toml` overrides `reqwest` to version `0.12` (with `json`, `rustls-tls`, and `socks` features), whereas the workspace root `Cargo.toml` specifies `reqwest = { version = "0.11", features = ["json", "stream"] }`. This forces the compiler to build both versions, bloats the binary size, and introduces potential runtime incompatibilities.
2. **Bypassed Workspace Inheritance**: Multiple dependencies (including `tokio`, `serde`, `tracing`, `tracing-subscriber`, `serde_json`, `anyhow`, `dirs`, `chrono`, and `uuid`) are specified locally with explicit versions instead of leveraging `{ workspace = true }`. This defeats centralized dependency management and increases maintenance overhead.

---

### 2. Schema-As-Code Build Check

#### Protobuf Compilation
* `crates/op-mcp-proxy/Cargo.toml` declares `tonic-build` under `[build-dependencies]`.
* `crates/op-mcp-proxy/src/vertex_grpc.rs:18` uses the macro `tonic::include_proto!("google.cloud.aiplatform.v1");`.
* No generated Rust files are committed in `crates/op-mcp-proxy/src/`. All Protobuf code is compiled at build time, confirming compilation happens strictly at build time rather than runtime.

#### Schema-As-Code Flagged Violations
The codebase violates the strict schema-as-code discipline in two critical areas:

1. **IdentitySled Layout (Ad-hoc Binary Struct representation)**:
   * **Location**: `crates/op-mcp-proxy/src/sled.rs:25-33` and `43-52`
   * **Description**: The zero-copy memory reader maps `/dev/shm/plugin_schema.dat` using hardcoded byte slice offsets to extract fields like `wg_pubkey`, `mutation_index`, `is_valid`, `footprint`, and `nextdns_profile`. This is a highly brittle, unversioned binary contract. Any modification to the `IdentitySled` struct in the upstream `op-identity` crate will silently corrupt data read by this proxy without compiler diagnostics. This should be represented as a serialized versioned Protocol Buffer schema.

2. **OpenAI Compatibility Layer HTTP Structs**:
   * **Location**: `crates/op-mcp-proxy/src/http_server.rs:36-79`
   * **Description**: Multiple ad-hoc Rust structs (`ChatCompletionRequest`, `ChatCompletionResponse`, `Choice`, etc.) are manually declared in code to model the JSON data structures. These external endpoints should be defined using an OpenAPI / JSON Schema specification and compiled into type-safe code at build time.

---

### 3. Production Security & Quality Audit Findings

#### [CRITICAL] Finding 1: Session Hijacking & Privilege Escalation via Deterministic Hostname Fallback
* **Path**: `crates/op-mcp-proxy/src/session.rs:82-99` and `136-173`
* **Vulnerability Type**: Authentication Bypass / Identity Confusion
* **Description**:
  If the proxy runs in a context where executing `wg show wg0 public-key` fails (e.g., inside restricted containers, rootless namespaces, or environments where the `wg` utility is missing), the function `get_local_wireguard_pubkey` falls back to a deterministic string:
  ```rust
  let hostname = hostname::get()
      .map(|h| h.to_string_lossy().to_string())
      .unwrap_or_else(|_| "unknown".to_string());
  Ok(format!("local:{}", hostname))
  ```
  When `get_or_create_session` is called, it queries the SQLite database for a valid session using this fallback value as the primary key:
  ```rust
  "SELECT session_id, pubkey, ... FROM sessions WHERE pubkey = ? AND last_seen_at > ?"
  ```
  If multiple distinct containers or clients share a generic hostname (e.g., `unknown`, `localhost`, `mcp-proxy-pod`), the database will match the fallback key of the *first* authenticated session. The second process is immediately returned the pre-existing session ID, cached Google Cloud OAuth tokens, and email address of the original user. This allows instant privilege escalation and credential theft between isolated workloads.
* **Remediation**:
  Never fall back to a shared/deterministic identifier for session matching. If a cryptographic peer identity cannot be securely fetched via WireGuard, the proxy must fail-closed, abort initialization, and log a critical error rather than generating a fallback ID.

---

#### [CRITICAL] Finding 2: Unpadded Buffer Memory Safety Violation (AVX/SIMD Buffer Overread)
* **Paths**:
  * `crates/op-mcp-proxy/src/cloudaicompanion.rs:488`
  * `crates/op-mcp-proxy/src/cloudaicompanion.rs:509`
  * `crates/op-mcp-proxy/src/cloudaicompanion.rs:613`
  * `crates/op-mcp-proxy/src/cloudaicompanion.rs:641`
  * `crates/op-mcp-proxy/src/main.rs:114`
* **Vulnerability Type**: Out-of-bounds Read / Undefined Behavior
* **Description**:
  The code reads JSON files from disk (or lines from `stdin`) into a standard heap-allocated string via `std::fs::read_to_string` or `std::io::stdin().read_line` and immediately passes them to `unsafe { simd_json::from_str(&mut text) }`. 
  
  `simd-json`'s string-parsing algorithm utilizes high-performance SIMD instructions (AVX2/SSE4.2) that read memory in large vectorized chunks (up to 32 or 64 bytes at a time). To prevent out-of-bounds reads, `simd-json` strictly requires that input buffers contain trailing padding equal to `simd_json::PADDING` bytes of addressable memory. Passing unpadded strings directly from standard files or lines into the unsafe parser triggers an out-of-bounds read if the JSON structure ends near a memory page boundary. This results in undefined behavior and segmentation faults (DoS), or potentially leaks adjacent heap memory.
* **Remediation**:
  Avoid using the highly dangerous `unsafe { simd_json::from_str }` API directly on unpadded strings. Use the safe `simd_json::from_slice` API or `simd_json::to_owned_value` on mutable byte vectors, which automatically allocate and handle the required SIMD padding. Alternatively, use standard `serde_json` for processing small config files where high-throughput SIMD parsing is unnecessary.

---

#### [HIGH] Finding 3: Undefined Behavior & SIGBUS Crash via Unsafe Memory Mapping of Shared Memory
* **Path**: `crates/op-mcp-proxy/src/sled.rs:37-41`
* **Vulnerability Type**: Concurrency / Memory Safety
* **Description**:
  The proxy reads binary layout state by raw memory mapping the shared memory file `/dev/shm/plugin_schema.dat`:
  ```rust
  let mmap = unsafe { MmapOptions::new().len(SLED_SIZE).map(&file).ok()? };
  let bytes = &mmap[..SLED_SIZE];
  ```
  This implementation exhibits two severe flaws:
  1. **Data Races / Undefined Behavior**: The upstream daemon (e.g., `op-identity`) writes to this file concurrently. Because there are no cross-process locking primitives or memory barriers used here, accessing `bytes` concurrently with writes is a data race, which constitutes instant Undefined Behavior (UB) in Rust due to reference aliasing violations of the read slice `&[u8]`.
  2. **SIGBUS Crash**: If the upstream daemon truncates, overwrites, or removes the shared memory file to a size below `SLED_SIZE` while the map is active, any read access to the slice will trigger a `SIGBUS` signal, immediately crashing the proxy process without any possibility of Rust panic recovery.
* **Remediation**:
  Because the metadata is extremely small (`SLED_SIZE = 208` bytes), memory mapping is entirely unnecessary. Read the file synchronously into a heap-allocated buffer using `File::read_exact`. This eliminates the risk of `SIGBUS` signals and avoids `unsafe` blocks entirely.

---

#### [HIGH] Finding 4: Privacy Leak / Fail-Open SOCKS5 Proxy Bypass
* **Path**: `crates/op-mcp-proxy/src/main.rs:52-67`
* **Vulnerability Type**: Security Control Bypass (Fail-Open)
* **Description**:
  The application is designed to route all external LLM API requests through an Xray SOCKS5 proxy to enforce NextDNS filtering and preserve user anonymity:
  ```rust
  let use_xray = !xray_socks.is_empty() && snapshot.as_ref().map(|s| s.is_valid).unwrap_or(false);
  ```
  If the `SledSnapshot` is invalid (e.g., missing `/dev/shm/plugin_schema.dat` or initialized as `is_valid = false`), `use_xray` resolves to `false`. Instead of failing safely and refusing to start, the program initiates the `DirectLLM` companion using `None` as the proxy:
  ```rust
  let llm = Arc::new(DirectLLM::new_with_proxy(if use_xray { Some(xray_socks) } else { None }).await?);
  ```
  As a consequence, the proxy silently leaks all user queries, metadata, and raw public IP addresses directly to Google's public endpoints over the unproxied internet, bypassing the NextDNS privacy stack.
* **Remediation**:
  Implement a fail-closed architecture. If SOCKS5 routing is configured but the identity state prevents safe initialization of the proxy tunnel, the proxy must panic/exit immediately instead of falling back to direct internet exposure.

---

#### [MEDIUM] Finding 5: Cache Stampede (Thundering Herd) on Token Expiry
* **Path**: `crates/op-mcp-proxy/src/direct_llm.rs:168-185`
* **Vulnerability Type**: Concurrency / Resource Exhaustion
* **Description**:
  In `DirectLLM::get_token()`, the lock over `cached_token` is released prior to executing the asynchronous API request to fetch/refresh the OAuth token:
  ```rust
  // Check cached token first.
  {
      let guard = self.cached_token.lock().await;
      if let Some(ref ct) = *guard {
          if ct.expiry > Utc::now() + chrono::Duration::minutes(2) {
              return Ok(ct.token.clone());
          }
      }
  }

  // Lock is released here
  let (token, expiry) = self.fetch_fresh_token().await?;
  ```
  If a burst of concurrent HTTP requests (such as client prompts) hits the proxy while the token is expired, all concurrent requests will read the expired token state, release the lock, and issue simultaneous external requests to refresh the credentials. This triggers a thundering herd (cache stampede) of identical OAuth requests, causing severe performance degradation and risking immediate rate-limiting or API bans by the upstream identity provider.
* **Remediation**:
  Maintain a single-flight mechanism. Ensure that only one asynchronous request can fetch a fresh token at a time, forcing subsequent concurrent tasks to wait for the result of the active refresh call.

---

#### [MEDIUM] Finding 6: Global Rate Limiter Denial of Service
* **Path**: `crates/op-mcp-proxy/src/http_server.rs:21-34` and `116-133`
* **Vulnerability Type**: Denial of Service (DoS)
* **Description**:
  The OpenAI-compatible server implements a token-bucket rate limiter that acts on a global, shared state:
  ```rust
  pub struct AppState {
      ...
      pub rate_limiter: Arc<Mutex<TokenBucket>>,
  }
  ```
  Because the rate limiter does not distinguish between client IP addresses, peer public keys, or API tokens, a single abusive, misconfigured, or malicious user can easily deplete the token bucket. This causes legitimate users to be throttled or rejected with a `429 Too Many Requests` error, creating a straightforward Denial of Service vector.
* **Remediation**:
  Incorporate client-specific identifiers (such as the peer's WireGuard public key from the session context, or the client's source IP address) into a keyed rate-limiting structure (e.g., using a thread-safe map or dashmap of token buckets).