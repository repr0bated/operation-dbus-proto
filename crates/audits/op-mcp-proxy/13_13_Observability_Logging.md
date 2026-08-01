### 1. Observability Profile & Statistics

#### Tracing Macros vs. Standard Print Macros
The `op-mcp-proxy` crate uses the structured `tracing` ecosystem exclusively. There are **zero** instances of standard printing macros (`println!`, `eprintln!`) in any of the production files. 

##### Statistics Table

| File | `debug!` | `info!` | `warn!` | `error!` | `println!` / `eprintln!` |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `crates/op-mcp-proxy/src/session.rs` | 1 | 3 | 2 | 0 | 0 |
| `crates/op-mcp-proxy/src/gcloud_auth.rs` | 5 | 6 | 4 | 0 | 0 |
| `crates/op-mcp-proxy/src/sled.rs` | 0 | 0 | 0 | 0 | 0 |
| `crates/op-mcp-proxy/src/direct_llm.rs` | 4 | 2 | 5 | 0 | 0 |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs` | 1 | 7 | 5 | 0 | 0 |
| `crates/op-mcp-proxy/src/vertex_grpc.rs` | 1 | 3 | 2 | 0 | 0 |
| `crates/op-mcp-proxy/src/main.rs` | 0 | 3 | 3 | 2 | 0 |
| `crates/op-mcp-proxy/src/http_server.rs` | 0 | 6 | 7 | 0 | 0 |
| **Total** | **12** | **30** | **28** | **2** | **0** |

---

#### Swallowed Errors (Without Logging)
Several errors throughout the proxy are silently discarded using `.ok()` or `.ok()?` transforms, leaving no diagnostic trace in the event of failure:

1. **Database Query Corruption & Lock Failures**
   * **`crates/op-mcp-proxy/src/session.rs:158`**: `db.query_row(...).ok()` discards SQLite query failures when looking up existing sessions.
   * **`crates/op-mcp-proxy/src/session.rs:193`**: `db.query_row("SELECT user_email FROM wireguard_users...", ...).ok()` silences database query errors when mapping WireGuard public keys to user emails.
   * **`crates/op-mcp-proxy/src/session.rs:245`**: `db.query_row("SELECT oauth_token...", ...).ok()` silently swallows token retrieval failures.

2. **Launcher / Process Launch Failures**
   * **`crates/op-mcp-proxy/src/gcloud_auth.rs:329`**: `Command::new("gcloud").args(args).output().ok()?` silently swallows the launch error if the `gcloud` binary is missing from the system path, permissions are denied, or execution fails.
   * **`crates/op-mcp-proxy/src/gcloud_auth.rs:343`**: Same silent swallow for `run_gcloud_access_token_no_scopes`.

3. **Silent Configuration and File Parse Failures**
   * **`crates/op-mcp-proxy/src/cloudaicompanion.rs:538`**: `read_gcloud_adc_quota_project()` swallows filesystem and parsing errors using `.ok()?`.
   * **`crates/op-mcp-proxy/src/cloudaicompanion.rs:548`**: `read_extension_adc_quota_project()` silences all authorization directory read and parsing errors.
   * **`crates/op-mcp-proxy/src/cloudaicompanion.rs:561`**: `read_antigravity_project()` silently discards parse errors for VSCode settings files.
   * **`crates/op-mcp-proxy/src/cloudaicompanion.rs:577`**: `read_adc_oauth_client()` silently discards credential parsing failures.

4. **Sled Memory Mapping / State Read Failures**
   * **`crates/op-mcp-proxy/src/sled.rs:34-35`**: `File::open(SLED_PATH).ok()?` and `MmapOptions::new()...map(&file).ok()?` silently exit the function with `None` if the shared-memory descriptor is missing or cannot be memory-mapped.

---

#### PII and Secrets Exposure Risks in Logs
Sensitive identity data and cryptographic secrets are exposed directly in logs:

1. **WireGuard Public Key Logging**
   * **`crates/op-mcp-proxy/src/session.rs:190`**: `info!("Creating new session: {} for pubkey: {}", session_id, pubkey);` logs the WireGuard public key of incoming peers at `info` level.

2. **Plaintext Email Disclosure (High PII Leak)**
   * **`crates/op-mcp-proxy/src/session.rs:296`**: `info!("Registered WireGuard user: {} -> {}", pubkey, user_email);` prints the raw, unhashed user email address in the application log at `info` level.

3. **Implicit Secrets Disclosure via Parse Errors**
   * **`crates/op-mcp-proxy/src/gcloud_auth.rs:253`**: `warn!("Extension auth cache unusable: {}", e);` prints the `serde_json::Error` resulting from parsing `credentials.json`. When parsing syntax errors occur, `serde_json` prints the context surrounding the error, which may contain plaintext `accessToken` or `refreshToken` credentials.

---

#### Metrics Instrumentation Evaluation
There is **no formal metrics instrumentation** in `op-mcp-proxy`. Although `prometheus` and `opentelemetry` are included in the workspace dependencies (`Cargo.toml`), none of the files in `op-mcp-proxy` import or record metrics.

* **Rate Throttling Observability Gap**: The Token-Bucket rate limiter in `crates/op-mcp-proxy/src/http_server.rs:155` logs dropped and throttled requests using ad-hoc `warn!` statements instead of exporting standard counter/histogram metrics. This prevents automated rate-limit alert generation via systems like Prometheus.

---

### 2. Schema-As-Code Discipline Violations

The codebase contains several occurrences where data contracts are defined as ad-hoc Rust structs, raw JSON objects, or raw strings instead of versioned, compiled schemas (e.g. Protocol Buffers or OpenAPI/OSCAL structures):

1. **Raw Database Schemas Defined as Ad-Hoc Strings**
   * **`crates/op-mcp-proxy/src/session.rs:48-70`**: SQLite table schemas (`sessions`, `wireguard_users`) are declared inline inside raw SQL strings.

2. **Ad-Hoc JSON REST Request/Response Structs**
   * **`crates/op-mcp-proxy/src/http_server.rs:58-112`**: OpenAI-compatible request/response contracts (`ChatCompletionRequest`, `ChatMessage`, `ChatCompletionResponse`, `Choice`, `Usage`, `ModelObject`, `ModelList`) are defined as ad-hoc Rust structs decorated with local `serde` annotations rather than utilizing a schema-generated type library.

3. **Dynamic Object Construction (JSON-RPC)**
   * **`crates/op-mcp-proxy/src/direct_llm.rs:185`**: Dynamic construction of the JSON-RPC completion response utilizing the `simd_json::json!` macro.
   * **`crates/op-mcp-proxy/src/cloudaicompanion.rs:214`**: Constructs the Google Cloud Code generate request body on the fly via `serde_json::json!`.
   * **`crates/op-mcp-proxy/src/main.rs:201`**: Dynamically creates the outbound JSON-RPC container structure.

4. **Ad-Hoc Extension Credentials Parsing**
   * **`crates/op-mcp-proxy/src/gcloud_auth.rs:50-74`**: Ad-hoc deserialization structs `ExtensionCredentials` and `ExtensionAdc` are constructed manually to match VSCode settings without formal external schema definitions.

* **Praise**: **`crates/op-mcp-proxy/src/vertex_grpc.rs:20-25`** correctly follows schema-as-code discipline. It builds its gRPC payload types (`GenerateContentRequest`, `GenerationConfig`, `Content`, `Part`) using Rust structures compiled from centralized Protobuf schemas via `tonic::include_proto!("google.cloud.aiplatform.v1")`.

---

### 3. Security and Quality Audit Findings

#### CRITICAL: Secrets Database Created with World-Readable Permissions (Local Privilege Escalation)
* **File & Line**: `crates/op-mcp-proxy/src/session.rs:40-45` (and lines 80-82)
* **Impact**: The SQLite database file storing active Google Cloud OAuth access tokens (`oauth_token`) and user email addresses (`user_email`) is created at `~/.local/share/mcp-proxy/sessions.db` without enforcing restricted file permissions.
* **Mechanism**: On standard Linux/Unix installations, `Connection::open(&db_path)` creates files with permissions dictated by the ambient process `umask` (commonly `0022` or `0002`). This leaves the database file readable by other local users on shared systems, enabling local credential theft and privilege escalation.
* **Remediation**: Apply restricted permissions (`0700` on the parent directory, `0600` on the database file) before creating or opening the database. For example, use Unix-specific FS extensions to set permissions:
  ```rust
  use std::fs::DirBuilder;
  use std::os::unix::fs::DirBuilderExt;
  
  let mut builder = DirBuilder::new();
  builder.recursive(true).mode(0o700);
  builder.create(parent)?;
  ```

---

#### HIGH: Undefined Behavior via Unsynchronized Memory Map of Shared-Memory File
* **File & Line**: `crates/op-mcp-proxy/src/sled.rs:34-45`
* **Impact**: Unsynchronized access to the shared-memory file `/dev/shm/plugin_schema.dat` via `Mmap` results in undefined behavior (UB) under Rust's aliasing rules.
* **Mechanism**: `SledSnapshot::read()` memory-maps a file using `memmap2` and casts the raw pointer directly into a standard Rust immutable byte slice `&[u8]`. Because this file is located in `/dev/shm` (shared memory) and is updated dynamically by other processes, the mapped bytes can be mutated concurrently while the Rust runtime holds an immutable reference `&[u8]`. Rust relies on the guarantee that the data behind a shared reference `&T` is not mutated concurrently without interior mutability wrapper types (e.g. `Atomic` or `UnsafeCell`). Additionally, if the writing process truncates the file while mapped, a read will trigger a `SIGBUS` panic and crash the proxy.
* **Remediation**: Do not use standard `Mmap` for shared-memory structures subject to concurrent mutations without synchronization primitives. Instead, read the file bytes into a heap-allocated buffer using standard filesystem reads:
  ```rust
  let mut file = File::open(SLED_PATH).ok()?;
  let mut buf = vec![0u8; SLED_SIZE];
  file.read_exact(&mut buf).ok()?;
  ```

---

#### MEDIUM: Thread Lock Blocking via Synchronous Database Calls in Async Task
* **File & Line**: `crates/op-mcp-proxy/src/session.rs:153-155`
* **Impact**: The `SessionManager` utilizes a `tokio::sync::Mutex` wrapping a synchronous `rusqlite::Connection`. While holding this async lock, synchronous SQLite queries (`db.query_row`, `db.execute`) are executed directly on the thread. This blocks the underlying Tokio worker thread, severely limiting throughput and causing latency spikes under high concurrent connection loads.
* **Remediation**: Wrap all synchronous database operations inside `tokio::task::spawn_blocking` to offload blocking I/O to a dedicated thread pool:
  ```rust
  let db = self.db.clone();
  let session = tokio::task::spawn_blocking(move || {
      let conn = db.blocking_lock();
      // Execute synchronous rusqlite queries
  }).await??;
  ```