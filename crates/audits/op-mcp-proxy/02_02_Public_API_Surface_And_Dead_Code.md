# Production Security & Quality Audit: op-mcp-proxy

---

## 1. Public API Surface

### 1.1 Public Item Enumeration & Analysis
The `op-mcp-proxy` crate exposes several public types, functions, and modules, primarily designed to facilitate session management, OAuth discovery, direct LLM proxying, and local Axum-based server execution.

**Total Explicit Public Items Count**: 62

### 1.2 Top 10 Most Impactful Public Items

| Item Name | Type | File Path & Line Citation | Impact Description |
| :--- | :--- | :--- | :--- |
| `SessionManager` | `struct` | `crates/op-mcp-proxy/src/session.rs:32` | Coordinates local state storage, session lifecycle, and token resolution. |
| `GCloudAuth` | `struct` | `crates/op-mcp-proxy/src/gcloud_auth.rs:32` | Discovers, refreshes, and caches Google Cloud platform credentials. |
| `DirectLLM` | `struct` | `crates/op-mcp-proxy/src/direct_llm.rs:21` | Orchestrates local Gemini API dispatching and background token refreshes. |
| `CloudAICompanion` | `struct` | `crates/op-mcp-proxy/src/cloudaicompanion.rs:32` | Mimics Google Cloud Code extension endpoint handshakes and bootstrap logic. |
| `VertexGrpcClient` | `struct` | `crates/op-mcp-proxy/src/vertex_grpc.rs:35` | Manages high-performance Tonic gRPC streaming connections to Vertex AI. |
| `AppState` | `struct` | `crates/op-mcp-proxy/src/http_server.rs:49` | Core shared state extractor used across local REST endpoint handlers. |
| `run` | `fn` | `crates/op-mcp-proxy/src/http_server.rs:301` | Spawns and configures the local multi-backend Axum HTTP engine. |
| `SledSnapshot` | `struct` | `crates/op-mcp-proxy/src/sled.rs:21` | Contains deserialized parameters from the hardware memory map. |
| `get_token` | `fn` | `crates/op-mcp-proxy/src/gcloud_auth.rs:83` | Fallback routing function executing credential generation across 5 layers. |
| `handle` | `fn` | `crates/op-mcp-proxy/src/direct_llm.rs:147` | Handles standard JSON-RPC inputs and executes model completions. |

### 1.3 Structural Encapsulation Flags

#### Glob Re-exports (`pub use *`)
No glob re-exports were detected in any of the audited source files.

#### Exposed Public Fields on Structs
*   **Struct `Session`** (`crates/op-mcp-proxy/src/session.rs:21`): All fields (`session_id`, `pubkey`, `user_email`, `oauth_token`, `token_expires_at`, `created_at`, `last_seen_at`) are declared `pub`. This allows external modules to write directly to these fields, risking state incoherency.
*   **Struct `SledSnapshot`** (`crates/op-mcp-proxy/src/sled.rs:21`): All fields are `pub`. Since this is designed as a read-only snapshot, this is acceptable, but getters would better protect mutation boundaries.
*   **Struct `AppState`** (`crates/op-mcp-proxy/src/http_server.rs:49`): Fields `llm`, `vertex`, and `rate_limiter` are exposed as `pub`. Axum routers do not require these fields to be public to consume state.
*   **Structs `ChatCompletionRequest` and `ChatMessage`** (`crates/op-mcp-proxy/src/http_server.rs:58`, `crates/op-mcp-proxy/src/http_server.rs:70`): All deserialized payload fields are `pub`.

---

## 2. Dead Code

### 2.1 Dead Code Suppressions (`#[allow(dead_code)]`)
The codebase explicitly suppresses compiler dead-code analysis in several locations:
*   `crates/op-mcp-proxy/src/session.rs:118` — `get_pubkey_for_ip`
*   `crates/op-mcp-proxy/src/session.rs:295` — `register_wireguard_user`
*   `crates/op-mcp-proxy/src/session.rs:315` — `cleanup_expired_sessions`
*   `crates/op-mcp-proxy/src/gcloud_auth.rs:186` — `refresh_token`

### 2.2 Unreferenced and Unused Elements
The following elements are compiled but never invoked by any execution path within the source files:

| Item | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `get_pubkey_for_ip` | Private Function | `crates/op-mcp-proxy/src/session.rs:119` | Remove if peer discovery is fully shifted to DBus/control plane. |
| `register_wireguard_user` | Public Function | `crates/op-mcp-proxy/src/session.rs:296` | Expose to administrative CLI commands, or remove. |
| `cleanup_expired_sessions` | Public Function | `crates/op-mcp-proxy/src/session.rs:316` | Call from a periodic background tokio thread in `main.rs`. |
| `touch_session` | Public Function | `crates/op-mcp-proxy/src/session.rs:242` | Call on active direct LLM traffic to dynamically extend session expiry. |
| `get_valid_token` | Public Function | `crates/op-mcp-proxy/src/session.rs:259` | Integrate with Vertex AI credential pre-warming to bypass redundant refreshes. |
| `refresh_token` | Public Function | `crates/op-mcp-proxy/src/gcloud_auth.rs:187` | Use in direct manual refresh RPCs, or remove. |

---

## 3. Audit Findings

### 3.1 Security Vulnerabilities

#### [CRITICAL] Memory Safety Violation: Unpadded Buffer Parsing via `simd_json::from_str`
*   **File:Line**: `crates/op-mcp-proxy/src/main.rs:125`, `crates/op-mcp-proxy/src/cloudaicompanion.rs:416`, `crates/op-mcp-proxy/src/cloudaicompanion.rs:435`
*   **Impact**: Execution crash (Segmentation Fault) or potential Out-of-Bounds Memory Read.
*   **Description**: The codebase invokes `unsafe { simd_json::from_str(&mut str) }` to parse JSON blocks on standard mutable strings (`line` and `text`). The `simd-json` specification mandates that the parsed string must have `simd_json::SIMDJSON_PADDING` (typically 64 bytes) of uninitialized or null-padded space allocated *beyond* the end of the string's logical content. If the string is unpadded, the SIMD vector load instructions (which read 32 or 64 bytes at a time) will read past the heap allocation boundary. This can cause an immediate crash (segmentation fault) if the read crosses a virtual memory page boundary.
*   **Exploit Vector**: An attacker can feed a JSON payload over `stdin` that terminates exactly at the edge of a page block, forcing a process termination.
*   **Remediation**: Use `simd_json::to_owned_value` on the byte buffer or explicitly pad the string buffer using:
    ```rust
    let mut padded_bytes = line.into_bytes();
    padded_bytes.resize(padded_bytes.len() + simd_json::SIMDJSON_PADDING, 0);
    let req = unsafe { simd_json::from_slice(&mut padded_bytes) }?;
    ```

#### [HIGH] Authorization Bypass: Substring Allowed-IP Matching in Peer Authentication
*   **File:Line**: `crates/op-mcp-proxy/src/session.rs:136`
*   **Impact**: Peer authentication bypass or session hijacking.
*   **Description**: The logic inside `get_pubkey_for_ip` retrieves the WireGuard allowed IP configuration and tests for membership using `ips.contains(peer_ip)`. This is a weak substring match. If a target peer has an allowed IP of `"10.200.0.10"`, a query from `peer_ip = "10.200.0.1"` will evaluate as a match (`"10.200.0.10".contains("10.200.0.1")` is `true`). An unauthorized peer can therefore falsely acquire the public key mapping of another host on the network.
*   **Exploit Vector**: A peer with IP `10.200.0.1` can successfully map to the public key configuration of any peer matching `10.200.0.1X`.
*   **Remediation**: Parse the allowed IP block into proper structured CIDR network/host ranges or perform full-token comma/space token splitting before comparison:
    ```rust
    for ip_subnet in ips.split(',') {
        if ip_subnet.trim().split('/').next() == Some(peer_ip) {
            return Ok(Some(pubkey.to_string()));
        }
    }
    ```

#### [HIGH] Denial of Service: Unchecked Memory-Mapped File Shrinkage (SIGBUS)
*   **File:Line**: `crates/op-mcp-proxy/src/sled.rs:35-36`
*   **Impact**: Immediate uncatchable crash (SIGBUS) of the proxy daemon.
*   **Description**: `SledSnapshot::read` accesses `/dev/shm/plugin_schema.dat` via a memory mapping (`memmap2::MmapOptions`). Shared memory files are highly volatile. If `op-identity` (or any other service writing to `/dev/shm`) truncates or shrinks the file while `op-mcp-proxy` is reading, any subsequent read operation on the slice `&bytes` will trigger a hardware page fault that translates into a `SIGBUS` signal. Rust applications cannot catch `SIGBUS` gracefully, resulting in instant crash of the proxy.
*   **Exploit Vector**: An unprivileged user or system process truncates `/dev/shm/plugin_schema.dat` to 0 bytes, permanently crashing the proxy.
*   **Remediation**: Use standard file system operations (`std::fs::read` or `File::read_exact`) instead of memory-mapping, since the structure size is a small static size of 208 bytes. If memory mapping is mandatory, trap `SIGBUS` using a custom signal handler, or use `memmap2`'s safe API wrappers.

#### [MEDIUM] Thundering Herd Rate-Limiter Bypass
*   **File:Line**: `crates/op-mcp-proxy/src/http_server.rs:175-188`
*   **Impact**: Upstream API quota exhaustion and client burst execution.
*   **Description**: In the rate-limiting block for the `/v1/chat/completions` endpoint, if the rate limiter bucket is exhausted, the handler calculates a delay, releases the rate-limiter lock, and calls `tokio::time::sleep(delay).await;`. Crucially, **after waking up, the task does not re-acquire the rate-limiter lock and does not re-evaluate the bucket state**. It directly forwards the payload. If 50 requests arrive concurrently when the bucket is empty, they will all receive roughly the same delay, sleep, and then proceed concurrently to make Google Cloud API calls, completely bypassing the rate-limiting contract.
*   **Exploit Vector**: Coordinated batching of requests allows clients to force concurrent API bursts, causing rate-limiting failures on the remote backend.
*   **Remediation**: Keep throttled requests in a queue or re-verify the token state after waking up before initiating the request:
    ```rust
    loop {
        let wait = state.rate_limiter.lock().await.try_consume().err();
        match wait {
            Some(delay) if delay.as_secs() > 5 => {
                return (StatusCode::TOO_MANY_REQUESTS, ...).into_response();
            }
            Some(delay) => {
                tokio::time::sleep(delay).await;
            }
            None => break, // Successfully acquired a slot
        }
    }
    ```

#### [MEDIUM] Weak Path Resolution: Unsanitized Binary Execution
*   **File:Line**: `crates/op-mcp-proxy/src/session.rs:91`, `crates/op-mcp-proxy/src/session.rs:120`
*   **Impact**: Privilege escalation or unauthorized command execution.
*   **Description**: `Command::new("wg")` relies on searching the user's `PATH` environment variable to locate the WireGuard command binary. If the daemon runs with root/system privileges and the `PATH` variable is contaminated or modified by an attacker, they can hijack execution.
*   **Exploit Vector**: Placing a malicious executable named `wg` in a high-priority path directory.
*   **Remediation**: Explicitly call absolute paths (e.g., `/usr/bin/wg` or `/usr/sbin/wg`), or fallback to PATH searches only in safe, system-administered directories.

#### [MEDIUM] Plaintext Storage of Sensitive Google Cloud OAuth Access Tokens
*   **File:Line**: `crates/op-mcp-proxy/src/session.rs:58`
*   **Impact**: Local privilege escalation and token extraction.
*   **Description**: The database schema creates the `sessions` table storing `oauth_token TEXT` in plaintext on disk (`~/.local/share/mcp-proxy/sessions.db`). If local file permissions are not explicitly managed, any unprivileged local process can read this SQLite database and extract valid Google Cloud OAuth access tokens (`ya29...`), which carry extensive platform permissions.
*   **Remediation**: Enforce strict `0600` file creation masks (`libc::umask(0077)`) on directory/file setup, or keep session tokens transient in memory only (e.g., in a secure non-persisted structure).

---

### 3.2 Schema-as-Code Violations
This repository strictly enforces a Schema-as-Code discipline.

#### [LOW/STYLE] Ad-hoc Data Contracts in HTTP Server & Main Proxy Loop
*   **File:Line**: `crates/op-mcp-proxy/src/http_server.rs:58-73`, `crates/op-mcp-proxy/src/main.rs:127`
*   **Violation**: Ad-hoc JSON-RPC parsing and local struct deserialization.
*   **Description**: The Axum HTTP server declares ad-hoc Rust structs (`ChatCompletionRequest` and `ChatMessage`) to handle OpenAI/REST JSON translation. Similarly, `main.rs` processes incoming messages on stdin by manually querying JSON fields (e.g., `req["method"]`, `req["params"]["name"]`). This bypasses versioned Protocol Buffers or structured OSCAL representations of data contracts.
*   **Remediation**: Use generated types from versioned schemas (e.g., MCP protobuf definitions) to serialize/deserialize all interface boundaries.

---

### 3.3 Quality & Maintainability Issues

#### [LOW] Memory-unsafe Cast in Epoch Parsing
*   **File:Line**: `crates/op-mcp-proxy/src/gcloud_auth.rs:360-366`
*   **Impact**: Logic errors on 32-bit platforms or invalid timestamp parsing.
*   **Description**: In `parse_expiry_epoch`, the code converts epoch times:
    ```rust
    let seconds = if raw > 10_000_000_000 { raw / 1000 } else { raw };
    ```
    This heuristic detects millisecond vs. second epoch values. However, passing unvalidated high integer ranges to `from_timestamp` can return `None` or out-of-bounds timestamps.
*   **Remediation**: Validate range bounds explicitly and handle the conversion with proper error-checked safe abstractions.

#### [LOW] Unbounded Retry Backoff in Generate Loop
*   **File:Line**: `crates/op-mcp-proxy/src/direct_llm.rs:197`
*   **Impact**: Increased API contention and proxy latency under heavy load.
*   **Description**: The retry delay inside the Direct LLM logic scales linearly: `500u64.saturating_mul(attempt as u64)`.
*   **Remediation**: Apply an exponential backoff strategy with jitter to prevent cascading retry storms on Google Cloud's upstream servers.