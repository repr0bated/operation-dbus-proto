# Production Security and Quality Audit: op-mcp-proxy

---

## 1. Test Coverage & Quality Audit

### Test Verification
*   **Total Test Functions**: 0
*   **Property-Based Testing / Fuzzing**: None found.
*   **Representative Test Instances**: No tests found.

### Security & Quality Risk: No Tests Found
*   **Risk Level**: **High Risk**
*   **Description**: The codebase has absolutely zero test coverage. Security-critical and highly complex operations—such as custom token-bucket rate limiting, session mapping, SQLite state mutations, multi-layered Google Cloud OAuth authentication fallbacks, and unsafe memory mapping—are completely untested. The absence of regression testing makes the system highly vulnerable to silent auth failures, rate-limiter bypasses, and memory corruption bugs during subsequent refactors.

---

## 2. Schema-as-Code Compliance Audit

This codebase bypasses versioned schema enforcement (such as Protocol Buffers or OSCAL) in multiple critical boundaries, opting instead for ad-hoc structs and unstructured raw JSON manipulations:

*   **Ad-hoc HTTP Data Contracts**: 
    *   `crates/op-mcp-proxy/src/http_server.rs:53-90`: Request and response types (`ChatCompletionRequest`, `ChatMessage`, `ChatCompletionResponse`, `Usage`, `ModelList`) are defined as ad-hoc Rust structs serialized directly to/from JSON. They are not mapped to versioned API schemas.
*   **Ad-hoc OAuth JSON Structures**:
    *   `crates/op-mcp-proxy/src/gcloud_auth.rs:44-67`: Deserialization structures (`ExtensionCredentials`, `ExtensionAdc`) are hand-rolled to parse VSCode configuration files instead of using versioned client configuration schemas.
*   **Unstructured JSON-RPC & Prompt Extraction**:
    *   `crates/op-mcp-proxy/src/direct_llm.rs:114-191`: The JSON-RPC routing mechanism operates on untyped `simd_json::OwnedValue` elements. Prompts are parsed ad-hoc through manual string key lookups (`messages`, `prompt`, `ref`) in `extract_prompt` (line 197).
*   **Ad-hoc DB Schemas via Hardcoded SQL Strings**:
    *   `crates/op-mcp-proxy/src/session.rs:48-69`: SQLite tables are initialized using unversioned raw SQL string batches inside the application code rather than migrating schemas through declarative, versioned migration files.
*   **Unversioned Shared-Memory Binary Sled Structure**:
    *   `crates/op-mcp-proxy/src/sled.rs:21-75`: The layout of `plugin_schema.dat` is parsed via raw byte offset slices (e.g., `&bytes[96..160]`). This layout mirrors an external struct from another crate without any schema version negotiation or validation, risking severe memory misalignment if either crate changes layout.

---

## 3. Detailed Security Findings

### [High] Unsafe `simd_json::from_str` with Unpadded String Buffers
*   **Reference**: `crates/op-mcp-proxy/src/main.rs:92`, `crates/op-mcp-proxy/src/cloudaicompanion.rs:517`, `crates/op-mcp-proxy/src/cloudaicompanion.rs:536`
*   **Description**: The application processes stdin inputs and files by passing standard `String` references directly to `unsafe { simd_json::from_str(&mut line) }`. The `simd_json` parser requires input string buffers to be padded with `simd_json::SIMD_JSON_PADDING` (32 bytes) of extra capacity. Passing standard unpadded buffers obtained from `std::io::stdin` or `std::fs::read_to_string` causes `simd_json` to perform out-of-bounds reads up to 32 bytes past the allocated heap buffer boundary.
*   **Impact**: Heap memory disclosure or application crash (Denial of Service) if parsing unpadded input close to allocation boundaries.
*   **Remediation**: 
    1. Use the safe, non-destructive parsing alternatives provided by `serde_json::from_str`.
    2. Alternatively, copy the string slice into a padded buffer using `simd_json::to_padded_bin` before using unsafe fast-parsing methods.

---

### [Medium] Complete Rate-Limiter Bypass in HTTP Chat Completions
*   **Reference**: `crates/op-mcp-proxy/src/http_server.rs:147-163`
*   **Description**: The rate-limiting mechanism checks the token bucket and retrieves a retry delay:
    ```rust
    let wait = state.rate_limiter.lock().await.try_consume().err();
    if let Some(delay) = wait {
        ...
        tokio::time::sleep(delay).await;
    }
    ```
    When a request exceeds the configured RPM capacity, `try_consume` returns `Err(delay)` and *does not* decrement `self.tokens`. However, the caller thread simply sleeps for the calculated duration and then proceeds to dispatch the request to Vertex AI *without re-evaluating the token bucket or decrementing tokens*. 
    
    If multiple concurrent requests arrive when the bucket is empty, they will all receive similar delays, sleep concurrently, and then execute simultaneously. 
*   **Impact**: The rate limiter fails to restrict concurrency spikes, leading to backend gRPC server overload and potential API suspension by upstream providers due to unthrottled concurrent requests.
*   **Remediation**: Modify `chat_completions` to recursively re-try token consumption after waking up, or block requests inside the lock using an async queue-based rate limiter (such as `governor`) instead of letting throttled threads proceed without consuming tokens.

---

### [Medium] Silent Public Internet Traffic Leaks on SOCKS Proxy Misconfiguration
*   **Reference**: `crates/op-mcp-proxy/src/main.rs:54`
*   **Description**: LLM calls are routed through an Xray SOCKS5 proxy *only* if the host's identity sled snapshot is found and successfully validated:
    ```rust
    let use_xray = !xray_socks.is_empty() && snapshot.as_ref().map(|s| s.is_valid).unwrap_or(false);
    ```
    If `/dev/shm/plugin_schema.dat` is missing, corrupted, or has `is_valid = false`, `use_xray` silently becomes `false`. The application then executes HTTP requests to Google Cloud services directly over the host's public network interface without routing through the expected privacy tunnel (NextDNS/Xray).
*   **Impact**: Silent exposure of sensitive enterprise prompts and host traffic to the public internet, violating compliance requirements and exposing network topology.
*   **Remediation**: Implement a fail-closed mechanism. If `XRAY_SOCKS_ADDR` is configured, fail immediately with an error if the identity sled cannot be loaded or is invalid, instead of silently falling back to insecure, direct internet routing.

---

### [Medium] Plaintext Storage of OAuth Access Tokens in World-Readable SQLite DB
*   **Reference**: `crates/op-mcp-proxy/src/session.rs:41-43`
*   **Description**: The session database containing high-privilege cached Google Cloud OAuth tokens (`oauth_token TEXT`) is opened using `Connection::open(&db_path)` in the home directory. The parent directory and database file are created using standard umask settings (typically `0644`), which permits other local users on the system to read the database file directly.
*   **Impact**: Local privilege escalation. Any local user on the host system can read the session database and hijack active Google Cloud authentication sessions to access cloud APIs.
*   **Remediation**: Set restrictive permissions on the directory and database file during creation. On Unix systems, restrict access exclusively to the owner (permissions `0700` for directories and `0600` for files) using `std::os::unix::fs::DirBuilderExt` or `std::fs::Permissions`.

---

### [Low] SIGBUS Risk on Sled Memory-Map Reading
*   **Reference**: `crates/op-mcp-proxy/src/sled.rs:30-33`
*   **Description**: The zero-copy reader uses an unsafe memory map (`memmap2`) to read parameters from `/dev/shm/plugin_schema.dat`. Because memory mapping represents the file directly in the virtual memory space of the process, if another process truncates or changes the file size of `plugin_schema.dat` while `op-mcp-proxy` is running, any read of the slice will immediately trigger a `SIGBUS` signal, terminating the proxy process.
*   **Impact**: Unexpected process termination (Denial of Service) during concurrent file operations on the shared memory device.
*   **Remediation**: Instead of using memory mapping for small binary structs (the sled size is only 208 bytes), read the file directly into a stack-allocated buffer using `std::fs::File::read_exact`. This completely bypasses unsafe memory mapping and guarantees standard Rust error handling instead of raw signal crashes.