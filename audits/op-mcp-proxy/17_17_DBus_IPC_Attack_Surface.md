## D-Bus & IPC Attack Surface Analysis

### 1. D-Bus Registration & Policy Analysis
* **D-Bus Interfaces, Methods, and Signals**: No D-Bus interfaces, methods, or signals are registered in the audited files (`op-mcp-proxy`). Although other workspace crates depend on `zbus`, this specific crate (`op-mcp-proxy`) acts as a network-to-gRPC proxy and does not register with any D-Bus system or session bus.
* **Caller Identity & Verification**: Not applicable for D-Bus. However, when forwarding gRPC requests to `op-dbus`, caller identity is extracted from shared memory (`/dev/shm/plugin_schema.dat`) and blindly trusted without local cryptographic validation.
* **System/Session Bus Connections**: Neither. The proxy communicates via localhost/local-network HTTP and gRPC.
* **Over-Permissioned System Bus Policies**: No system bus policy was provided in the files list.

### 2. IPC/Network State Mutation & Process Spawning
The following methods and entry points trigger external process execution or invoke billing-heavy external state modifications without authorization:
* **WireGuard Process Spawning**:
  * `crates/op-mcp-proxy/src/session.rs:100` spawns `wg show wg0 public-key` to identify the host machine.
  * `crates/op-mcp-proxy/src/session.rs:118` spawns `wg show wg0 allowed-ips` to parse peer IP associations.
* **Google Cloud CLI Process Spawning**:
  * `crates/op-mcp-proxy/src/gcloud_auth.rs:230` and `crates/op-mcp-proxy/src/gcloud_auth.rs:247` spawn `gcloud auth print-access-token` and `gcloud auth application-default print-access-token` respectively.
* **Unauthenticated HTTP State Mutation**:
  * `crates/op-mcp-proxy/src/http_server.rs:141` (`chat_completions`) exposes LLM endpoints to arbitrary callers over the network without API key validation or authentication. These endpoints invoke costly Vertex AI or CloudAI APIs.

### 3. Unvalidated Deserialization
The following endpoints parse caller-supplied bytes without prior validation:
* **JSON-RPC Deserialization**:
  * `crates/op-mcp-proxy/src/main.rs:113` parses standard input directly into a mutable `simd_json::OwnedValue` using `unsafe { simd_json::from_str(&mut line) }` without verifying the payload against any JSON schema.
* **HTTP Body Deserialization**:
  * `crates/op-mcp-proxy/src/http_server.rs:136` uses axum's `Json<ChatCompletionRequest>` extractor to deserialize incoming HTTP POST requests without limiting request size or verifying payload structures.
* **Shared Memory Deserialization**:
  * `crates/op-mcp-proxy/src/sled.rs:35` maps and reads a raw binary struct from `/dev/shm/plugin_schema.dat` via `memmap2` without validating that the mapped memory conforms to the expected structural constraints or schema version.

---

## Security Vulnerability Findings

### [CRITICAL] Complete Lack of Authentication on HTTP Server Gateway
* **Reference**: `crates/op-mcp-proxy/src/http_server.rs:141` (`chat_completions`) and `crates/op-mcp-proxy/src/http_server.rs:326` (`axum::serve`)
* **Severity**: Critical
* **Description**: The OpenAI-compatible HTTP server exposed via `http_server::run` binds to a configured address and processes `/v1/chat/completions` requests. It utilizes the server's ambient, cached Google Cloud OAuth tokens (or Gemini credentials) to authorize requests to Vertex AI or CloudAI. However, the `chat_completions` handler performs **no authentication** of incoming requests.
* **Exploitation Scenario**: An attacker with network access to the port bound by `HTTP_SERVER_ADDR` (or another local user on a multi-user machine if bound to `127.0.0.1`) can send arbitrary chat completion payloads. The server will process them and forward them to Vertex AI, consuming the host's GCP API quotas and incurring financial charges under the host's GCP billing account.
* **Recommendation**: Implement token-based authentication (e.g., Bearer API keys) within `http_server.rs` or enforce strict client certificate/firewall rules to prevent unauthorized network or local access.

---

### [HIGH] Shared Memory Identity Tampering Vulnerability
* **Reference**: `crates/op-mcp-proxy/src/sled.rs:24` (`SLED_PATH`) and `crates/op-mcp-proxy/src/main.rs:164`
* **Severity**: High
* **Description**: `op-mcp-proxy` parses `/dev/shm/plugin_schema.dat` to extract the host identity (`wg_pubkey`, `mutation_index`, `footprint_hex`, `trace_id`). In `main.rs:164`, these values are parsed and injected into forwarded gRPC requests as standard `x-ghostbridge-footprint` and `x-ghostbridge-trace-id` headers. Because `/dev/shm` is typically world-writable or writable by any local process, the file `plugin_schema.dat` can be modified or created by unprivileged local processes.
* **Exploitation Scenario**: A malicious local user or compromised local application on the host machine overwrites `/dev/shm/plugin_schema.dat` with a forged WireGuard public key and trace ID. The proxy reads this file, sets `is_valid` to `true`, and injects the forged identity headers into all subsequent control plane gRPC requests forwarded to `op-dbus`, effectively spoofing the identity of other authorized network nodes or peers.
* **Recommendation**: Avoid storing control-plane trust assets in public directories like `/dev/shm`. Secure the identity file with `0600` permissions in a protected runtime directory (e.g., `/run/op-mcp-proxy/`) owned strictly by the proxy user.

---

### [HIGH] Unsafe mmap Execution and SIGBUS Vulnerability
* **Reference**: `crates/op-mcp-proxy/src/sled.rs:35`
* **Severity**: High
* **Description**: In `sled.rs`, `SledSnapshot::read()` uses `memmap2` to map `/dev/shm/plugin_schema.dat` with a hardcoded length of `SLED_SIZE` (208 bytes). However, the mapping is created and read from without pre-validating that the physical file size is at least `SLED_SIZE` bytes. Furthermore, the file is not locked.
* **Exploitation Scenario**: An attacker truncates `/dev/shm/plugin_schema.dat` to 0 bytes. When the proxy attempts to read `bytes[40]` or construct fields, a read operation beyond the physical file size boundaries is performed on the memory map, instantly triggering a `SIGBUS` signal. Because `SIGBUS` is unhandled, this results in a sudden, unrecoverable denial-of-service crash of the proxy process.
* **Recommendation**: Query the file metadata using `file.metadata()?.len()` to guarantee that the file on disk is at least `SLED_SIZE` bytes before calling `MmapOptions::map`. Additionally, lock the file via advisory file locking (e.g., `fs2` or raw `flock`) to prevent concurrent truncation during reads.

---

### [HIGH] Plaintext Credential Exposure via Insecure Database Permissions
* **Reference**: `crates/op-mcp-proxy/src/session.rs:44` and `crates/op-mcp-proxy/src/session.rs:77`
* **Severity**: High
* **Description**: The SQLite database `sessions.db` is initialized using `std::fs::create_dir_all(parent)` and `Connection::open(&db_path)`. This database stores highly sensitive plaintext `oauth_token` values extracted from Google Cloud auth chains. Because the directory and file are created with standard umask permissions (often leaving them world- or group-readable), the database file is exposed to unauthorized local users.
* **Exploitation Scenario**: A local user on the host system reads `/home/<user>/.local/share/mcp-proxy/sessions.db` or the current working directory's `./mcp-proxy/sessions.db` database. They execute `SELECT oauth_token FROM sessions` to extract valid, plaintext Google Cloud OAuth access tokens and hijack the proxy's GCP identity.
* **Recommendation**: Set a strict umask or explicitly restrict directory and file permissions to `0700` and `0600` using Unix-specific `std::os::unix::fs::DirBuilderExt` or `OpenOptionsExt` before creating the database file.

---

### [MEDIUM] Global Rate Limiting Denial of Service
* **Reference**: `crates/op-mcp-proxy/src/http_server.rs:32` (`TokenBucket`) and `crates/op-mcp-proxy/src/http_server.rs:145`
* **Severity**: Medium
* **Description**: The rate-limiting mechanism implemented in `http_server.rs` uses a single global `TokenBucket` rate limiter across all request handlers. This lock is acquired globally via `state.rate_limiter.lock().await` on every chat completions request.
* **Exploitation Scenario**: A single client floods `/v1/chat/completions` with rapid requests. The global rate limiter depletes its tokens within seconds. All subsequent legitimate requests from other independent clients or sessions are rejected with a `429 Too Many Requests` status, resulting in a global denial-of-service state for the entire system.
* **Recommendation**: Refactor the rate limiter to use a keyed strategy (e.g., rate limiting by Client IP, peer ID, or Session ID) instead of applying a single global token pool.

---

### [MEDIUM] Ambient PATH Command Hijacking
* **Reference**: `crates/op-mcp-proxy/src/session.rs:100` and `crates/op-mcp-proxy/src/gcloud_auth.rs:230`
* **Severity**: Medium
* **Description**: The commands `wg` and `gcloud` are executed via `Command::new("wg")` and `Command::new("gcloud")` without using absolute executable paths. This causes the host's standard `Command` API to search the ambient environment's `PATH` variable to resolve the location of these binaries.
* **Exploitation Scenario**: If the proxy process runs with elevated privileges (e.g., to query WireGuard metrics) or is launched from an environment where the `PATH` variable can be manipulated by unprivileged users, an attacker can place a malicious executable named `wg` or `gcloud` in a custom directory, prepend it to the `PATH`, and trigger arbitrary code execution under the security context of the proxy process.
* **Recommendation**: Resolve executables using absolute paths (e.g., `/usr/bin/wg` and `/usr/bin/gcloud`) or use configuration overrides to specify validated paths.

---

## Schema-as-Code Compliance Audit

The system design relies on ad-hoc structs and unstructured JSON strings rather than strictly versioned Protocol Buffers or formal schema contracts to manage data representation and serialization.

### Identified Ad-hoc Contracts
1. **Ad-hoc Session Representation**:
   * `crates/op-mcp-proxy/src/session.rs:20` defines the `Session` struct:
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
     This structure represents a core data contract that should be modeled as a versioned Protobuf or SQLite schema definition rather than a mutable, code-first SQLite struct.
2. **Untracked Google Credentials Deserialization**:
   * `crates/op-mcp-proxy/src/gcloud_auth.rs:46` (`ExtensionCredentials`), `crates/op-mcp-proxy/src/gcloud_auth.rs:56` (`ExtensionAdc`) model Google/VSCode auth cache files using ad-hoc `serde` structs rather than versioned schema definitions.
3. **Implicit Shared Memory Memory Mapping**:
   * `crates/op-mcp-proxy/src/sled.rs:21` implements `SledSnapshot` to mirror raw C-struct structures located in `/dev/shm/plugin_schema.dat` without formal protobuf schemas or versioning indicators:
     ```rust
     pub struct SledSnapshot {
         pub is_valid:         bool,
         pub mutation_index:   u64,
         pub footprint_hex:    String,
         pub trace_id:         String,
         pub nextdns_profile:  String,
         pub subid:            String,
         pub control_source:   String,
     }
     ```
4. **Ad-hoc OpenAI HTTP Payloads**:
   * `crates/op-mcp-proxy/src/http_server.rs:43` (`ChatCompletionRequest`), `crates/op-mcp-proxy/src/http_server.rs:55` (`ChatMessage`), and `crates/op-mcp-proxy/src/http_server.rs:61` (`ChatCompletionResponse`) represent ad-hoc internal representations of public API contracts. These contracts are constructed dynamically in memory instead of being generated from official OpenAPI specifications or versioned schemas.
5. **Dynamic JSON Construction**:
   * `crates/op-mcp-proxy/src/main.rs:125` manually generates dynamic JSON objects on the fly:
     ```rust
     let generated_req = simd_json::json!({
         "jsonrpc": "2.0",
         "id": req["id"].clone(),
         "method": "generate",
         "params": { ... }
     });
     ```
     This violates schema-as-code principles by relying on unvalidated, raw nested JSON dictionaries rather than strongly-typed, schema-compiled messages.