# Production Security and Quality Audit: op-mcp-proxy

---

## 1. License & Dependency Compliance

### Workspace License Extraction
* **Workspace Package License**: `Apache-2.0`
  * **Source Citation**: `Cargo.toml:46` (`license = "Apache-2.0"` under `[workspace.package]`)

### Missing License Field Flag
* **Crate**: `op-mcp-proxy`
  * **Source Citation**: `crates/op-mcp-proxy/Cargo.toml`
  * **Status**: **FAILED**. The crate does not explicitly specify its own license, nor does it inherit from the workspace package using `license.workspace = true`. It should be updated to contain:
    ```toml
    [package]
    name = "op-mcp-proxy"
    ...
    license.workspace = true
    ```

### GPL/AGPL/SSPL Scanner
* **Source Citation**: `Cargo.lock`
* **Status**: **PASS**. No GPL, AGPL, or SSPL licensed crates were found in the dependency tree. `cozo` is included as a workspace dependency, which is licensed under the Mozilla Public License 2.0 (`MPL-2.0`), which is compatible with `Apache-2.0` when distributed as a separate component.

---

## 2. Schema-as-Code Violations

The codebase has several instances where data contracts and network/API payloads are represented as ad-hoc, manual Rust structs or raw byte offsets instead of versioned Protocol Buffers or OSCAL schemas.

### Finding 1: Ad-hoc Session State Serialization
* **Source Citation**: `crates/op-mcp-proxy/src/session.rs:21-29`
* **Violating Code**:
  ```rust
  pub struct Session {
      pub session_id: String,
      pub pubkey: String,
      pub user_email: Option<String>,
      pub oauth_token: Option<String>,
      pub token_expires_at: Option<DateTime<Utc>>,
      pub created_at: DateTime<Utc>,
      pub last_seen_at: DateTime<Utc>,
  }
  ```
* **Impact**: The database and session synchronization relies on ad-hoc field mappings. Changing the schema requires manually updating multiple raw SQLite queries across `crates/op-mcp-proxy/src/session.rs`. These objects should be generated from versioned Protocol Buffer definitions.

### Finding 2: Brittle Memory-Mapped Binary Layout
* **Source Citation**: `crates/op-mcp-proxy/src/sled.rs:21-29`
* **Violating Code**:
  ```rust
  pub const SLED_SIZE: usize = 208;
  pub const SLED_PATH: &str = "/dev/shm/plugin_schema.dat";
  ```
* **Impact**: The proxy relies on a strict, binary memory layout with manual byte slicing (`&bytes[192..208]`, `&bytes[96..160]`) to read fields. This bypasses structured serialization completely. Any compiler optimization or change in alignment in the writing process (`op-identity`) will cause silent corruption or memory faults.

### Finding 3: Ad-hoc JSON-RPC & OpenAI Request/Response Models
* **Source Citation**: `crates/op-mcp-proxy/src/http_server.rs:53-108`
* **Violating Code**:
  ```rust
  pub struct ChatCompletionRequest { ... }
  pub struct ChatMessage { ... }
  struct ChatCompletionResponse { ... }
  ```
* **Impact**: The HTTP emulation layer defines custom API structures for the OpenAI format rather than referencing a single schema-as-code file. Any external model changes require manual refactoring of serializable structs.

### Finding 4: Ad-hoc External Credentials Modeling
* **Source Citation**: `crates/op-mcp-proxy/src/gcloud_auth.rs:52-78`
* **Violating Code**:
  ```rust
  struct ExtensionCredentials { ... }
  struct ExtensionAdc { ... }
  ```
* **Impact**: Hand-crafted JSON deserialization structs for third-party cache formats are prone to breakage if the extension updates its cache schema.

---

## 3. Production Security Vulnerabilities

### [CRITICAL] Finding 1: Unauthenticated Open Proxy to Google Cloud / Vertex AI API
* **Source Citation**: `crates/op-mcp-proxy/src/http_server.rs:128-199`
* **Vulnerability Type**: Authenticated Service Impersonation / Credential Theft
* **Exploitability**: **Directly Exploitable**.
* **Description**:
  The HTTP server's `/v1/chat/completions` endpoint does not authenticate incoming requests. However, when executing a request via either the Vertex AI gRPC path or the CloudAI companion fallback, it fetches the server host's *private* GCP / Google Cloud credentials:
  ```rust
  let token = self.get_token().await?; // Injects host Google Cloud token
  ```
  If `HTTP_SERVER_ADDR` is bound to any accessible interface (such as `0.0.0.0`), any attacker on the local network or internet can make requests to `/v1/chat/completions`, and the server will execute them using its own high-privilege Google Cloud Service Account or User tokens. This allows complete billing theft, API quota exhaustion, and unauthorized model execution.
* **Remediation**:
  Implement API key or token validation in `http_server.rs` before handling completions. Ensure `axum` routes validate authorization headers against a cryptographically verified key.

---

### [HIGH] Finding 2: Local Denial of Service (SIGBUS) and Data Race via Unsafe Memory Mapping
* **Source Citation**: `crates/op-mcp-proxy/src/sled.rs:36-41`
* **Vulnerability Type**: Memory Unsafety / SIGBUS Page Fault Crash
* **Exploitability**: **Exploitable by any local process**.
* **Description**:
  The proxy reads memory-mapped configuration data from shared memory (`/dev/shm/plugin_schema.dat`):
  ```rust
  let file = File::open(SLED_PATH).ok()?;
  let mmap = unsafe { MmapOptions::new().len(SLED_SIZE).map(&file).ok()? };
  if mmap.len() < SLED_SIZE { return None; }
  let bytes = &mmap[..SLED_SIZE];
  ```
  If a local attacker or concurrent process truncates or changes the size of `/dev/shm/plugin_schema.dat` after the mapping is established, reading from the `bytes` slice triggers a kernel page fault, sending a `SIGBUS` signal to the process and crashing it instantly.
  Furthermore, casting a writable shared memory file directly into a Rust immutable reference (`&[u8]`) while other processes can concurrently mutate it violates Rust's aliasing guarantees, leading to undefined behavior and potential data races.
* **Remediation**:
  Avoid raw memory-mapping of files in shared memory locations (`/dev/shm`). Instead, read the file into an owned heap-allocated buffer using `std::fs::read` or use thread-safe inter-process communication (IPC) libraries.

---

### [MEDIUM] Finding 3: Global Rate Limiter Denial of Service (No Client Partitioning)
* **Source Citation**: `crates/op-mcp-proxy/src/http_server.rs:133-145`
* **Vulnerability Type**: Global Resource Exhaustion (DoS)
* **Exploitability**: **Exploitable**.
* **Description**:
  The server implements a token-bucket rate limiter that is globally shared across all requests:
  ```rust
  let wait = state.rate_limiter.lock().await.try_consume().err();
  ```
  Because the rate limiter is global and does not track client identifiers (like IP addresses or session keys), a single abusive client can make rapid requests to `/v1/chat/completions`, exhausting the entire bucket. This causes the server to immediately reject legitimate requests from *all other clients* with `TOO_MANY_REQUESTS` or block their request threads in `tokio::time::sleep(delay).await`.
* **Remediation**:
  Partition the rate-limiter bucket by client IP address or user ID (e.g., using a hash map of token buckets per identity) rather than applying a single global limit.