# Production Security and Quality Audit: op-mcp-proxy

---

## 1. Documentation Audit

### Crate-Level Documentation Checklist
*   **Crate-level `//!` docs in `lib.rs`**: No `lib.rs` exists in the analyzed repository files. `op-mcp-proxy` is compiled strictly as a binary crate (`crates/op-mcp-proxy/src/main.rs`). The binary entry point `main.rs` contains crate-level `//!` docs, but any modular API boundaries are missing their corresponding `lib.rs` documentation.
*   **`README.md` presence**: No `README.md` file is present in the provided files.
*   **Public unsafe functions with missing invariant documentation**: There are **no** public unsafe functions (`pub unsafe fn`) defined in this codebase. (Internal `unsafe` blocks are utilized, but none are exposed as public function APIs).

### Sample of 10 Public Items Missing `/// rustdoc` Comments
The codebase lacks structured public API documentation. Below is a sample of 10 public items completely missing `/// rustdoc` comments:

1.  **`crates/op-mcp-proxy/src/session.rs:19`**:
    ```rust
    pub struct Session {
    ```
2.  **`crates/op-mcp-proxy/src/session.rs:29`**:
    ```rust
    pub struct SessionManager {
    ```
3.  **`crates/op-mcp-proxy/src/session.rs:36`**:
    ```rust
    pub fn new() -> anyhow::Result<Self> {
    ```
4.  **`crates/op-mcp-proxy/src/gcloud_auth.rs:32`**:
    ```rust
    pub struct GCloudAuth {
    ```
5.  **`crates/op-mcp-proxy/src/gcloud_auth.rs:53`**:
    ```rust
    pub fn new() -> Self {
    ```
6.  **`crates/op-mcp-proxy/src/gcloud_auth.rs:232`**:
    ```rust
    pub async fn refresh_extension_token(&self) -> anyhow::Result<(String, DateTime<Utc>)> {
    ```
7.  **`crates/op-mcp-proxy/src/sled.rs:23`**:
    ```rust
    pub struct SledSnapshot {
    ```
8.  **`crates/op-mcp-proxy/src/sled.rs:31`**:
    ```rust
    pub fn read() -> Option<Self> {
    ```
9.  **`crates/op-mcp-proxy/src/direct_llm.rs:22`**:
    ```rust
    pub struct DirectLLM {
    ```
10. **`crates/op-mcp-proxy/src/vertex_grpc.rs:27`**:
    ```rust
    pub struct VertexGrpcClient {
    ```

---

## 2. Schema-As-Code Violations

The codebase frequently relies on fragile, ad-hoc struct layouts, hardcoded memory offsets, and manual string representation conversions instead of formal, versioned schemas (such as Protocol Buffers or OSCAL-compliant definitions).

### Memory-Mapped C-Struct Shadowing (Fragile Alignment & Layout)
*   **Citation**: `crates/op-mcp-proxy/src/sled.rs:3-16` and `23-30`
*   **Vulnerability**: The structure of `SledSnapshot` is hardcoded as an ad-hoc Rust struct mirroring a C-layout struct from a completely separate crate (`op-identity::schema_bridge`). Raw byte slicing is done manually via magic number offsets (e.g., `&bytes[0..32]`, `bytes[32..40]`, `bytes[40]`, `&bytes[48..80]`).
*   **Risk**: Any alignment, padding, or struct modifications in `op-identity` will silently break this parser, causing catastrophic runtime memory corruption, reading garbled data, or generating invalid keys without compilation errors. 

### Ad-hoc JSON REST API Contracts
*   **Citation**: `crates/op-mcp-proxy/src/http_server.rs:49-106`
*   **Vulnerability**: The OpenAI-compatible endpoint models (`ChatCompletionRequest`, `ChatMessage`, `ChatCompletionResponse`, `Choice`, `Usage`, `ModelObject`, `ModelList`) are defined as ad-hoc, internal Rust structs decorated with Serde attributes.
*   **Risk**: Changes to the API contracts require manual Rust code modifications. These APIs are not bound to versioned schemas or formal Interface Definition Languages (IDLs), risking unnoticed contract drifts and breaking external client integrations.

---

## 3. Technical & Security Findings

### CRITICAL: Out-of-Bounds Memory Read / SEGFAULT via Unpadded Inputs to `simd_json`
*   **Citation**: 
    *   `crates/op-mcp-proxy/src/cloudaicompanion.rs:483`
    *   `crates/op-mcp-proxy/src/cloudaicompanion.rs:511`
    *   `crates/op-mcp-proxy/src/main.rs:109`
*   **Vulnerability**: The code uses `unsafe { simd_json::from_str(&mut text) }` on strings read directly from disk or stdin.
    ```rust
    // crates/op-mcp-proxy/src/cloudaicompanion.rs:483
    let mut text = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    let creds: OwnedValue = unsafe { simd_json::from_str(&mut text) }
    ```
    ```rust
    // crates/op-mcp-proxy/src/main.rs:109
    let req: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut line) }?;
    ```
    `simd_json` relies on the input string having `simd_json::SIMDJSON_PADDING` (typically 64 bytes) of addressable padding allocated at the end of the input buffer. Standard `String` buffers populated by `std::fs::read_to_string` or `BufRead::lines()` are *not* padded. 
*   **Exploitation / Impact**: If a configuration file or a line received via stdin ends near a page boundary, the SIMD vector load instructions (which read up to 32 or 64 bytes at once) will read past the allocated page boundary, immediately triggering a segmentation fault (`SIGSEGV`) and causing a complete denial of service (DoS). A malicious peer could exploit this by feeding unpadded payloads via standard input or altering credentials files.
*   **Remediation**: Copy the data into a `simd_json::to_padded_bin` or construct a padded vector, or use the safe API variant `simd_json::from_slice` which handles internal copying when required.

---

### HIGH: Unencrypted SQLite Storage of Sensitive OAuth Bearer Tokens
*   **Citation**: `crates/op-mcp-proxy/src/session.rs:43-73`
*   **Vulnerability**: The local session SQLite database is initialized with an unencrypted `sessions` table containing plain-text sensitive active OAuth credentials:
    ```rust
    CREATE TABLE IF NOT EXISTS sessions (
        session_id TEXT PRIMARY KEY,
        pubkey TEXT NOT NULL,
        user_email TEXT,
        oauth_token TEXT, -- Plain text sensitive token
        token_expires_at INTEGER,
        ...
    ```
    The path to this database is resolved to `dirs::data_dir()`:
    ```rust
    fn db_path() -> anyhow::Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("mcp-proxy");
        Ok(data_dir.join("sessions.db"))
    }
    ```
*   **Impact**: On Linux, `dirs::data_dir()` usually translates to `~/.local/share/mcp-proxy/`. The application does not explicitly set restrictive directory permissions (e.g. `0700` / `S_IRWXU`) during folder creation (`std::fs::create_dir_all`). If the parent directory is created with a default permissive umask, any other local user on the machine can read `sessions.db` and steal active OAuth tokens to access the cloud environment.
*   **Remediation**: 
    1. Force directory permissions to `0700` during creation on Unix systems using `std::os::unix::fs::DirBuilderExt`.
    2. Encrypt the database or store sensitive session bearer tokens inside a secure OS keyring (e.g., via the `keyring` crate) instead of SQLite plaintext columns.

---

### MEDIUM: Local Denial of Service via Concurrent Truncation of Shared Memory Memory-Map
*   **Citation**: `crates/op-mcp-proxy/src/sled.rs:33`
*   **Vulnerability**: The snapshot reader maps a file residing in shared memory:
    ```rust
    let file = File::open(SLED_PATH).ok()?;
    let mmap = unsafe { MmapOptions::new().len(SLED_SIZE).map(&file).ok()? };
    ```
*   **Impact**: The file mapping occurs over `/dev/shm/plugin_schema.dat`. Because memory-mapping a file does not prevent truncation by other processes on the system, any local unprivileged process that can write to `/dev/shm` can truncate this file to 0 bytes. If the file is truncated while `op-mcp-proxy` reads from the `mmap` reference, the OS will raise a `SIGBUS` signal, resulting in an uncatchable immediate crash of the proxy.
*   **Remediation**: Use standard file system read operations with locking (`flock`) rather than memory mapping (`mmap`) for small config files (208 bytes is extremely small and does not benefit from mmap's performance advantages).

---

### MEDIUM: Rate Limiter Thundering Herd & Synchronization Issues
*   **Citation**: `crates/op-mcp-proxy/src/http_server.rs:134-148`
*   **Vulnerability**: When the API limit is exceeded, the rate limiter does not queue requests or decrement a reserved ticket; instead, it calculates a sleep duration and sleeps asynchronously *after* releasing the mutex lock:
    ```rust
    let wait = state.rate_limiter.lock().await.try_consume().err();
    if let Some(delay) = wait {
        if delay.as_secs() > 5 { ... }
        // Lock is released here
        tokio::time::sleep(delay).await;
    }
    ```
*   **Impact**: If a massive burst of requests hits the server while the bucket is empty, all concurrent tasks will read `try_consume().err()`, calculate roughly the exact same sleep delay (e.g. `0.3` seconds), drop the lock, and sleep. When they wake up, they will all simultaneously attempt to acquire the lock and call `try_consume()` again. This leads to a severe "thundering herd" synchronization wave, overloading the underlying APIs and resulting in uneven rate-limit distribution.
*   **Remediation**: Implement a fair Semaphore-backed queue or a reservation system where a request decrements the bucket (even into negative values) and sleeps for its reserved slot before proceeding, preventing thundering herds.