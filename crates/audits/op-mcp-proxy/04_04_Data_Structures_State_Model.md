# Data Structures Audit & Quality Metrics

## Target Type & Method Counts by File

| File | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell` | `.clone()` Count |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `crates/op-mcp-proxy/src/session.rs` | 4 | 0 | 0 | 0 | 5 | 0 | 5 |
| `crates/op-mcp-proxy/src/gcloud_auth.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 3 |
| `crates/op-mcp-proxy/src/sled.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-mcp-proxy/src/direct_llm.rs` | 4 | 0 | 0 | 0 | 3 | 0 | 2 |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs` | 0 | 0 | 0 | 0 | 3 | 0 | 7 |
| `crates/op-mcp-proxy/src/vertex_grpc.rs` | 4 | 0 | 0 | 0 | 3 | 0 | 6 |
| `crates/op-mcp-proxy/src/main.rs` | 5 | 0 | 0 | 0 | 0 | 0 | 7 |
| `crates/op-mcp-proxy/src/http_server.rs` | 7 | 0 | 0 | 0 | 3 | 0 | 2 |

## Quality Metrics & Flags

* **Globally Mutable State**: No globally mutable state (`static mut` or `lazy_static`) was detected in the audited files.
* **Clone Thresholds**: No file exceeded the limit of 20 `.clone()` calls. The highest count was 7 `.clone()` calls in both `cloudaicompanion.rs` and `main.rs`.
* **Large Structs (> 5 public fields)**:
  * **`Session`** in `crates/op-mcp-proxy/src/session.rs:25-33` contains 7 public fields:
    * `session_id: String`
    * `pubkey: String`
    * `user_email: Option<String>`
    * `oauth_token: Option<String>`
    * `token_expires_at: Option<DateTime<Utc>>`
    * `created_at: DateTime<Utc>`
    * `last_seen_at: DateTime<Utc>`
  * **`SledSnapshot`** in `crates/op-mcp-proxy/src/sled.rs:20-28` contains 7 public fields:
    * `is_valid: bool`
    * `mutation_index: u64`
    * `footprint_hex: String`
    * `trace_id: String`
    * `nextdns_profile: String`
    * `subid: String`
    * `control_source: String`

---

# Schema-as-Code Compliance Audit

This codebase implements a "schema-as-code" discipline. The following locations violate this rule by declaring data contracts via ad-hoc Rust structs, database schema strings, or unstructured JSON parsing instead of Protocol Buffers or versioned OSCAL schemas.

### 1. Ad-Hoc Database Table & Entity Definition
* **File**: `crates/op-mcp-proxy/src/session.rs`
* **Lines**: 25-33 and 48-69
* **Violation**: The SQL database schema is defined using raw, unversioned DDL strings inside `execute_batch` (e.g., `CREATE TABLE IF NOT EXISTS sessions ...`). The corresponding `Session` Rust struct represents an ad-hoc state contract that should be driven by a structured schema.

### 2. Ad-Hoc OpenAI Chat API Model Structs
* **File**: `crates/op-mcp-proxy/src/http_server.rs`
* **Lines**: 45-81
* **Violation**: Data contracts for HTTP server request/response payloads (`ChatCompletionRequest`, `ChatMessage`, `ChatCompletionResponse`, `Choice`, and `Usage`) are hand-coded as ad-hoc serialized/deserialized Rust structs instead of generation from shared, versioned schema definitions.

### 3. Untyped JSON-RPC Response Assembly
* **File**: `crates/op-mcp-proxy/src/direct_llm.rs`
* **Lines**: 222-230 and 335-343
* **Violation**: Structured API responses (JSON-RPC results and errors) are manually assembled using the `simd_json::json!` macro rather than using strongly typed, versioned Protocol Buffer schemas.

### 4. Raw JSON Dispatching and Parameter Extraction
* **File**: `crates/op-mcp-proxy/src/main.rs`
* **Lines**: 182-184 and 219-253
* **Violation**: Raw JSON-RPC requests are parsed and routed by matching against arbitrary string keys (`req["method"]`, `"completion/complete"`, `"tools/call"`). Parameters are manually deserialized instead of using a typed schema registry.

---

# Security & Vulnerability Findings

### CRITICAL: Fail-Open Privacy Leak — Silent Bypass of SOCKS5 Privacy Proxy
* **File**: `crates/op-mcp-proxy/src/main.rs`
* **Lines**: 44-61
* **Description**: The proxy is designed to route LLM HTTP traffic through an Xray SOCKS5 proxy (configured via `XRAY_SOCKS_ADDR`) to enforce privacy routing through NextDNS. However, `use_xray` evaluates to `false` if `snapshot` is `None` (meaning the shared memory sled was missing or failed to parse) or if `s.is_valid` is `false`. When this occurs, the proxy silently falls back to direct, cleartext internet routing:
  ```rust
  let llm = Arc::new(DirectLLM::new_with_proxy(if use_xray { Some(xray_socks) } else { None }).await?);
  ```
* **Exploitability**: Because `/dev/shm` is a standard, temporary, world-writable memory directory on Linux, any local unprivileged process can write, overwrite, or delete `/dev/shm/plugin_schema.dat`. By truncating or deleting this file, an attacker can reliably trigger the proxy to silently bypass the SOCKS5 proxy and leak the user's IP address, DNS requests, and sensitive queries over the plain public internet. This constitutes a severe, exploitable fail-open design flaw.
* **Remediation**: Fail closed. If `XRAY_SOCKS_ADDR` is provided but the identity sled is missing or invalid, the proxy should abort initialization or refuse to process LLM requests rather than fallback to plain-text internet transport.

---

### CRITICAL: Unsafe Memory Mapping of Shared Memory susceptible to SIGBUS DoS
* **File**: `crates/op-mcp-proxy/src/sled.rs`
* **Lines**: 33-47
* **Description**: The binary reads `/dev/shm/plugin_schema.dat` by memory mapping a fixed 208-byte segment inside `unsafe`:
  ```rust
  let mmap = unsafe { MmapOptions::new().len(SLED_SIZE).map(&file).ok()? };
  if mmap.len() < SLED_SIZE { return None; }
  let bytes = &mmap[..SLED_SIZE];
  ```
  In Linux, if a file backed by a memory map is truncated to a size smaller than the mapped window (e.g., 0 bytes), any subsequent dereference of the mapped memory (such as reading `bytes[32..40]`) raises a `SIGBUS` signal. 
* **Exploitability**: Since `/dev/shm/plugin_schema.dat` is located in the world-writable `/dev/shm` directory, any unprivileged local process can truncate the file. Once truncated, the next time the proxy executes `SledSnapshot::read()` (which happens on every incoming stdin loop iteration), dereferencing `bytes` will immediately crash the daemon with `SIGBUS`. There are no signal handlers or safety boundaries preventing this crash.
* **Remediation**: Avoid raw memory-mapping of unprivileged shared-memory files. Instead, use safe, standard file-system read primitives (`std::fs::read` or `std::io::Read::read_exact`) to read the fixed 208-byte structure into a stack or heap-allocated buffer.

---

### HIGH: Local Privilege Escalation via Unsanitized Environment Variables
* **File**: `crates/op-mcp-proxy/src/gcloud_auth.rs`
* **Lines**: 92-108, 114-118, and 281-285
* **Description**: The authentication subsystem relies on environment variables such as `MCP_PROXY_TOKEN_FILE` and `MCP_PROXY_VSCODE_AUTH_DIR` to locate credentials on the filesystem. However, this proxy daemon interacts with the network configuration interface by executing the `wg` command, which typically requires running as `root` or with `CAP_NET_ADMIN` privileges. 
* **Exploitability**: If the proxy is running as a privileged system service but inherits environment variables from unprivileged calling users, a local attacker can set `MCP_PROXY_TOKEN_FILE` or `MCP_PROXY_VSCODE_AUTH_DIR` to point to privileged system files (such as `/etc/shadow` or other user files). The privileged daemon will then read and parse these target files. Although standard files might not look like JSON, the error responses (e.g., `cannot parse credentials.json: ...`) can leak partial contents of the files, or the daemon may load keys and persist them into the `sessions.db` where the attacker has read permissions.
* **Remediation**: If the binary runs with elevated privileges, it must sanitize and discard unprivileged environment variables (e.g., `HOME`, `MCP_PROXY_TOKEN_FILE`, and `MCP_PROXY_VSCODE_AUTH_DIR`) or explicitly refuse to execute if environment variables point to paths outside of strict root-owned directory spaces.

---

### HIGH: Insecure SQLite Database Location and Weak Directory Permissions
* **File**: `crates/op-mcp-proxy/src/session.rs`
* **Lines**: 38-46 and 88-92
* **Description**: `db_path()` constructs the session storage path under `dirs::data_dir()`, which defaults to user-local directories (e.g., `~/.local/share` on Linux). The parent directories are created using `std::fs::create_dir_all(parent)` with the default system `umask`, and the database is opened with `Connection::open(&db_path)`.
* **Exploitability**: When the database is created, it inherits the caller's standard `umask` (often `0002` or `0022`), which makes `sessions.db` group-readable or world-readable. Because this database stores active OAuth tokens (`oauth_token`) and user identities (`user_email`), any local user with read access to the directory can steal these tokens and compromise the user's entire Google Cloud account. Additionally, if the daemon is run globally, placing the database in a user-writable directory permits low-privilege users to modify or delete the DB.
* **Remediation**: Force strict permissions of `0700` for directories and `0600` for files when creating the database directory and file, or enforce a strict `umask` programmatically before database creation. Ensure the database is stored in a root-controlled, secure location (such as `/var/run` or `/var/lib`) if executed as a system daemon.

---

### MEDIUM: PATH Hijacking via Relative Command Invocation
* **File**: `crates/op-mcp-proxy/src/session.rs` (Lines 96-98, 128-130) and `crates/op-mcp-proxy/src/gcloud_auth.rs` (Lines 312-315, 327-329)
* **Description**: The application invokes external helper utilities (`wg` and `gcloud`) using relative path lookups:
  ```rust
  let output = Command::new("wg").args(["show", ...])
  ```
  and:
  ```rust
  let output = Command::new("gcloud").args(base_args).output().ok()?;
  ```
* **Exploitability**: Relative command invocations look up the executable name across directories specified in the caller's `PATH` environment variable. If the proxy runs with elevated privileges, an attacker can modify the `PATH` environment variable of the parent shell before invocation to point to a directory containing a malicious payload named `wg` or `gcloud`, achieving arbitrary root command execution.
* **Remediation**: Specify absolute paths to all system binaries (e.g., `/usr/bin/wg` or `/usr/bin/gcloud`) or resolve them securely by sanitizing the `PATH` environment variable before invoking `Command::new`.