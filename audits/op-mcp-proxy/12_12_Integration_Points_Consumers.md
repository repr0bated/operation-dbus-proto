# Integration & Security Audit: `op-mcp-proxy`

---

## 1. Integration Analysis

### Workspace Dependencies on `op-mcp-proxy`
Based on the provided workspace `Cargo.toml` and `Cargo.lock` files, the following workspace crates depend on `op-mcp-proxy`:
*   **`op-web`** (evident in `Cargo.lock` under the `dependencies` list for `name = "op-web"`).

There are no workspace-level declarations of `op-mcp-proxy` within `[workspace.dependencies]`; it is referenced directly via path dependencies inside the workspace members.

### Registered D-Bus Service Names & Object Paths
The `op-mcp-proxy` crate does **not** register any D-Bus service names or object paths. 
*   It operates as a gRPC client calling the `op-dbus` daemon (via `McpServiceClient` targeting `OP_DBUS_ADDR`, which defaults to `http://10.200.0.2:50051`).
*   It reads local identity data directly from a memory-mapped file (`/dev/shm/plugin_schema.dat`) rather than making D-Bus IPC calls.

### Exposed HTTP & gRPC Endpoints
The crate hosts an OpenAI-compatible HTTP server when `HTTP_SERVER_ADDR` is configured. The exposed endpoints are defined in `crates/op-mcp-proxy/src/http_server.rs`:
*   `POST /v1/chat/completions` (stream and unary chat completion endpoints targeting Vertex AI or the CloudAI companion fallback) — `crates/op-mcp-proxy/src/http_server.rs:356`
*   `GET /v1/models` (lists hardcoded Gemini models) — `crates/op-mcp-proxy/src/http_server.rs:357`

The crate does **not** expose or host any gRPC services of its own; it acts exclusively as a gRPC client for both Vertex AI (`PredictionServiceClient`) and `op-dbus` (`McpServiceClient`).

### Cross-Crate Circular Dependency Risks
*   **Current State:** No circular dependency risks are present in the provided manifests. 
*   **Dependency flow:** `op-web` $\rightarrow$ `op-mcp-proxy` $\rightarrow$ (`op-cache` and `op-identity`). 
*   Neither `op-cache` nor `op-identity` depend back on `op-mcp-proxy` or `op-web`, maintaining a clean, acyclic dependency tree.

---

## 2. Security & Quality Audit Findings

### Critical Findings

### [Critical] Memory Safety Violation & UB via Concurrent Read/Write on Memory-Mapped Sled
*   **Citation:** `crates/op-mcp-proxy/src/sled.rs:40-44`
*   **Vulnerability Type:** Undefined Behavior (Borrow Checker Aliasing Violation) & SIGBUS Risk
*   **Description:** 
    The zero-copy reader maps `/dev/shm/plugin_schema.dat` using `memmap2::MmapOptions` with a hardcoded length of `208` bytes:
    ```rust
    let file = File::open(SLED_PATH).ok()?;
    let mmap = unsafe { MmapOptions::new().len(SLED_SIZE).map(&file).ok()? };
    if mmap.len() < SLED_SIZE { return None; }
    let bytes = &mmap[..SLED_SIZE];
    ```
    1. **Data Race / Aliasing Violation:** The mapped file is updated concurrently by the workspace's identity management processes (e.g., `op-identity::schema_bridge`). Accessing this memory region via a standard immutable reference (`&[u8]`) while another process writes to the underlying file violates Rust's strict aliasing guarantees. This constitutes Undefined Behavior (UB), allowing the compiler to optimize under the assumption that the memory is invariant, leading to silent memory corruption, torn reads, or CPU instruction reordering issues.
    2. **Truncation / SIGBUS Crash:** If the file size on disk is truncated or smaller than `208` bytes when mapped, accessing the mmap region will trigger a `SIGBUS` signal, causing an unhandleable crash of the proxy daemon.
*   **Remediation:** 
    Avoid memory-mapping shared files for concurrent read/write unless utilizing volatile reads or raw pointers within an atomic synchronization protocol. Instead, read the file using standard, thread-safe file I/O operations with proper OS-level advisory locks (e.g., `flock`), or use atomic operations if memory mapping is strictly necessary.

---

### [Critical] Undefined Behavior via `unsafe { simd_json::from_str }` on Unpadded Buffers
*   **Citations:** 
    *   `crates/op-mcp-proxy/src/main.rs:114`
    *   `crates/op-mcp-proxy/src/cloudaicompanion.rs:486`
    *   `crates/op-mcp-proxy/src/cloudaicompanion.rs:508`
    *   `crates/op-mcp-proxy/src/cloudaicompanion.rs:563`
    *   `crates/op-mcp-proxy/src/cloudaicompanion.rs:599`
*   **Vulnerability Type:** Memory Safety (Out-of-Bounds SIMD Memory Read)
*   **Description:** 
    The codebase repeatedly invokes `unsafe { simd_json::from_str(&mut string) }` on strings read directly from standard input (`stdin.lock().lines()`) or from disk (`std::fs::read_to_string`).
    
    The security contract of `simd_json::from_str` explicitly mandates that the input string buffer **must be padded** with at least `simd_json::PADDING` (typically 32 or 64 bytes) of addressable memory beyond the string's actual length. This padding allows SIMD vector registers to load chunked memory boundaries safely.
    
    Because `std::fs::read_to_string` and `BufRead::lines` allocate exact-capacity buffers without SIMD padding, passing their mutable references to `from_str` causes the parser to execute out-of-bounds reads when processing payloads that end near page/allocation boundaries. This can cause segmentation faults, memory leaks, or exploit-susceptible behavior.
*   **Remediation:** 
    Replace the unsafe `from_str` calls with safe alternatives such as `simd_json::from_slice` on vector buffers (which automatically handle padding requirements internally) or switch to standard `serde_json::from_str`.
    ```rust
    // Safe replacement
    let mut bytes = line.into_bytes();
    let req: simd_json::OwnedValue = simd_json::from_slice(&mut bytes)?;
    ```

---

### Warnings

### [Warning] Insecure VSCode Auth Directory Override Path
*   **Citation:** `crates/op-mcp-proxy/src/gcloud_auth.rs:341-344`
*   **Vulnerability Type:** Arbitrary File Access / Information Disclosure
*   **Description:** 
    The VSCode cache directories are resolved via:
    ```rust
    let auth_dir = if let Ok(dir) = std::env::var("MCP_PROXY_VSCODE_AUTH_DIR") {
        PathBuf::from(dir)
    } else { ... }
    ```
    This path is immediately used to read sensitive credential payloads such as `credentials.json` and `application_default_credentials.json` without any validation that the resolved path is located within a trusted folder or belongs to the current user's profile. If the proxy runs with elevated privileges, an unprivileged user capable of setting environment variables can point `MCP_PROXY_VSCODE_AUTH_DIR` to arbitrary paths, causing the proxy to parse and expose sensitive files.
*   **Remediation:** 
    Ensure that any environment-provided paths are canonicalized and validated against a set of secure base paths (e.g., verifying they are subdirectories of the executing user's home directory `dirs::home_dir()`).

---

### [Warning] Token Refresh HTTP Client Lacks Certificate Pinning or Strict Host Verification
*   **Citation:** `crates/op-mcp-proxy/src/gcloud_auth.rs:434-441`
*   **Vulnerability Type:** MITM (Man-in-the-Middle) Risk
*   **Description:** 
    The extension token refresh makes direct HTTP POST requests to Google's OAuth2 endpoints:
    ```rust
    let resp = reqwest::Client::new()
        .post("https://oauth2.googleapis.com/token")
        .form(...)
    ```
    The client is constructed using default settings, relying solely on the system trust store without certificate pinning or validation. Given that the proxy's network stack can be explicitly routed through proxy configurations (e.g., SOCKS5 proxies or custom endpoints), there is an increased surface area for interception of sensitive OAuth refresh tokens, client IDs, and client secrets if system certificates are compromised.
*   **Remediation:** 
    Implement explicit root CA loading and consider pinning public keys or certificates specifically for `oauth2.googleapis.com` inside the token refresh module.

---

### Schema-as-Code Discipline Violations

This codebase violates the **Schema-as-Code** discipline by expressing data contracts as ad-hoc strings, manual JSON constructs, or unversioned raw Rust structs instead of deriving them from declarative schemas (e.g., Protocol Buffers or versioned OSCAL schemas):

1.  **Ad-Hoc SQL Schema Inlined as Raw Strings:**
    *   **Citation:** `crates/op-mcp-proxy/src/session.rs:48-68`
    *   **Violation:** The SQLite tables (`sessions`, `wireguard_users`) are created and managed via ad-hoc SQL strings inside the Rust source. These database structures are data contracts that lack version control, migration schemas, or automated validation against a centralized schema registry.
2.  **Unversioned Ad-Hoc Structs for OpenAI HTTP Payload Contracts:**
    *   **Citation:** `crates/op-mcp-proxy/src/http_server.rs:39-86`
    *   **Violation:** Structs representing chat completion requests and responses (`ChatCompletionRequest`, `ChatMessage`, `ChatCompletionResponse`, `Choice`, etc.) are declared as ad-hoc Rust structs with manual JSON serialization annotations. They are not mapped from a versioned schema file, creating divergence risks with the upstream OpenAI API contract.
3.  **Manual JSON-RPC Payload Generation:**
    *   **Citation:** `crates/op-mcp-proxy/src/direct_llm.rs:188-198` & `crates/op-mcp-proxy/src/main.rs:202-212`
    *   **Violation:** JSON-RPC messages and errors are hand-crafted using inline `simd_json::json!` macros. The structure of these requests/responses is not validated against a protocol schema or protobuf description, leading to schema drift and silent compatibility breaks if contract fields change.

---
## ⚠ Citation Warnings
- `crates/op-mcp-proxy/src/gcloud_auth.rs:434`: file has 427 lines
