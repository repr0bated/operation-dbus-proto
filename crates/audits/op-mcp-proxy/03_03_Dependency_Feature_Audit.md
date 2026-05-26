# PRODUCTION SECURITY & QUALITY AUDIT
**Crate:** `op-mcp-proxy`  
**Date:** March 30, 2025  

---

## 1. Dependencies & Feature Inventory

### Direct Dependencies (from `crates/op-mcp-proxy/Cargo.toml`)

| Dependency | Version Specification | Resolved / Workspace Version | Enabled Features | Explicit vs Default Features | Check / Flags |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `op-cache` | `path = "../op-cache"` | Internal | N/A | Default | Safe (Internal Crate) |
| `op-identity` | `path = "../op-identity"` | Internal | N/A | Default | Safe (Internal Crate) |
| `tokio` | `version = "1"` | `1.49.0` | `["full"]` | Explicitly enabled `["full"]` | Safe (Standard Async Runtime) |
| `tonic` | `workspace = true` | `0.12` | `["tls", "tls-roots", "tls-webpki-roots"]` | Pulled from workspace features | Safe (Standard gRPC) |
| `prost` | `workspace = true` | `0.13` | Default | Workspace defaults | Schema-As-Code Dependency |
| `tokio-stream` | `workspace = true` | `0.1` | Default | Workspace defaults | Safe |
| `futures` | `workspace = true` | `0.3` | Default | Workspace defaults | Safe |
| `serde` | `version = "1"` | `1.0.228` | `["derive"]` | Explicitly enabled `["derive"]` | Safe |
| `simd-json` | `workspace = true` | `0.13` | `["serde", "serde_impl"]` | Workspace defaults | **Risk**: Unpadded buffers (See Finding 2) |
| `reqwest` | `version = "0.12"` | `0.12.28` | `["json", "rustls-tls", "socks"]` | `default-features = false`, explicit features | Safe (Rustls for TLS, SOCKS5 support) |
| `tracing` | `"0.1"` | `0.1.44` | Default | Default | Safe |
| `tracing-subscriber` | `"0.3"` | `0.3.22` | Default | Default | Safe |
| `serde_json` | `"1"` | `1.0.149` | Default | Default | Safe |
| `anyhow` | `"1"` | `1.0.100` | Default | Default | Standard Error Crate |
| `dirs` | `"5"` | `5.0.1` | Default | Default | Safe |
| `hostname` | `"0.4"` | `0.4.2` | Default | Default | Safe |
| `rusqlite` | `workspace = true` | `0.32.1` | `["bundled"]` | Explicitly enabled `["bundled"]` | Storage Backend |
| `chrono` | `version = "0.4"` | `0.4.43` | `["serde"]` | Explicitly enabled `["serde"]` | Safe |
| `uuid` | `version = "1.6"` | `1.20.0` | `["v4", "serde"]` | Explicitly enabled `["v4", "serde"]` | Safe |
| `memmap2` | `workspace = true` | `0.9.10` | Default | Workspace defaults | **Critical**: Read UB (See Finding 1) |
| `hex` | `workspace = true` | `0.4.3` | Default | Workspace defaults | Safe |
| `axum` | `workspace = true` | `0.7.9` | Default | Workspace defaults | HTTP Server Component |

### Build Dependencies
*   `tonic-build`: Workspace dependency (`0.12.3`)

### Own Crate [features]
*   **None defined** in `crates/op-mcp-proxy/Cargo.toml`.

---

## 2. Storage Backend Check

| Backend | Found at file:line | Role (KV/Graph/Cache/Queue) | Audit Check |
| :--- | :--- | :--- | :--- |
| `rusqlite` | `crates/op-mcp-proxy/src/session.rs:12` | **Relational / KV**: Manages local SQLite connection to `sessions.db` to persist user sessions and allowed WireGuard IP mappings. | **Conforms**: Uses parameterized statements. No ad-hoc string formatting found. |
| `memmap2` ("sled") | `crates/op-mcp-proxy/src/sled.rs:25` | **Shared Memory (Zero-copy IPC)**: Reads `/dev/shm/plugin_schema.dat` representing the identity memory structure. | **Violation**: Directly maps raw shared-memory bytes. Highly vulnerable to UB and SIGBUS crashes. (See Finding 1). |

---

## 3. Schema-As-Code Check

`op-mcp-proxy` violates the schema-as-code discipline in multiple places by representing internal state, external communication contracts, and token exchanges as ad-hoc Rust structs or raw dynamic JSON structures instead of using versioned Protocol Buffers or OSCAL-compliant schemas:

1.  **Ad-hoc Session Contract**:
    *   `crates/op-mcp-proxy/src/session.rs:20`: The database schema and local domain model for `Session` are declared as an ad-hoc Rust struct and hardcoded SQL strings instead of a versioned schema.
2.  **Ad-hoc Third-Party API Deserialization**:
    *   `crates/op-mcp-proxy/src/gcloud_auth.rs:43`: `ExtensionCredentialsNested`
    *   `crates/op-mcp-proxy/src/gcloud_auth.rs:52`: `ExtensionCredentials`
    *   `crates/op-mcp-proxy/src/gcloud_auth.rs:62`: `ExtensionAdc`
    *   *These parse Google VSCode extension configurations and authorization tokens using unversioned JSON structures.*
3.  **Ad-hoc OpenAI-compatible API Structures**:
    *   `crates/op-mcp-proxy/src/http_server.rs:53`: `ChatCompletionRequest`
    *   `crates/op-mcp-proxy/src/http_server.rs:62`: `ChatMessage`
    *   `crates/op-mcp-proxy/src/http_server.rs:69`: `ChatCompletionResponse`
    *   `crates/op-mcp-proxy/src/http_server.rs:78`: `Choice`
    *   `crates/op-mcp-proxy/src/http_server.rs:84`: `Usage`
    *   `crates/op-mcp-proxy/src/http_server.rs:91`: `ModelObject`
    *   `crates/op-mcp-proxy/src/http_server.rs:98`: `ModelList`
    *   *These structures redefine standard OpenAI REST schemas in an ad-hoc fashion rather than generating models from OpenAPI specifications.*
4.  **Ad-hoc JSON-RPC Dynamic Parsing**:
    *   `crates/op-mcp-proxy/src/main.rs:109`: MCP JSON-RPC requests are parsed directly into raw `simd_json::OwnedValue` instead of structured Rust representations of the MCP protocol specification.
    *   `crates/op-mcp-proxy/src/main.rs:113`: JSON-RPC method dispatch checks are done via string matches (`"completion/complete"`, `"sampling/createMessage"`, etc.) rather than a versioned schema definition.

---

## 4. Findings

### [Finding 1] Local Denial of Service (DoS) & Undefined Behavior via `memmap2` Aliasing Violation on Shared Memory
*   **Severity:** Critical
*   **File:line:** `crates/op-mcp-proxy/src/sled.rs:42`

#### Description
The proxy maps a local file in shared memory (`/dev/shm/plugin_schema.dat`) into its memory space via `memmap2` and casts it to a Rust byte slice (`&[u8]`):
```rust
let file = File::open(SLED_PATH).ok()?;
let mmap = unsafe { MmapOptions::new().len(SLED_SIZE).map(&file).ok()? };
if mmap.len() < SLED_SIZE { return None; }

let bytes = &mmap[..SLED_SIZE];
```
This is a direct violation of Rust's aliasing guarantees and memory safety contracts:
1.  **Aliasing Violation (Undefined Behavior)**: `bytes` is a shared immutable reference (`&[u8]`). Rust compiler optimizations assume that the memory pointed to by `&[u8]` cannot change. However, because `/dev/shm/plugin_schema.dat` can be mutated concurrently by other processes (e.g., `op-identity`), the memory can change, leading to compiler-induced undefined behavior (UB), data races, and memory corruption.
2.  **SIGBUS Crash (Exploitable Denial of Service)**: If any local process (even an unprivileged one sharing the `/dev/shm` namespace) truncates the `/dev/shm/plugin_schema.dat` file to a size smaller than `SLED_SIZE` after mapping, any future read access to `bytes` will raise a `SIGBUS` signal. Because `SIGBUS` is not caught by the Rust runtime, the `op-mcp-proxy` process will crash instantly.

#### Remediation
Avoid memory-mapping world-writable or multi-process files where concurrent truncation can occur. Instead, read the file synchronously into a standard, heap-allocated buffer:
```rust
use std::io::Read;
let mut file = File::open(SLED_PATH).ok()?;
let mut buffer = vec![0u8; SLED_SIZE];
file.read_exact(&mut buffer).ok()?;
```

---

### [Finding 2] Undefined Behavior / Out-of-Bounds Memory Reads via Misuse of `simd_json::from_str` without Padding
*   **Severity:** Critical
*   **File:line:** `crates/op-mcp-proxy/src/main.rs:109`
*   **Other impacted lines:** `crates/op-mcp-proxy/src/cloudaicompanion.rs:433`, `crates/op-mcp-proxy/src/cloudaicompanion.rs:452`, `crates/op-mcp-proxy/src/cloudaicompanion.rs:525`, `crates/op-mcp-proxy/src/cloudaicompanion.rs:543`

#### Description
The codebase calls `simd_json::from_str` directly on unpadded strings read from `stdin` or file reads:
```rust
// crates/op-mcp-proxy/src/main.rs:109
let req: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut line) }?;
```
And:
```rust
// crates/op-mcp-proxy/src/cloudaicompanion.rs:433
let creds: OwnedValue = unsafe { simd_json::from_str(&mut text) }
```
According to `simd-json` requirements, the parsed input string *must* be padded with at least `simd_json::SIMDJSON_PADDING` (typically 32 or 64 bytes) beyond the string length. `simd-json` uses vectorized SIMD instructions that load chunks of 32 or 64 bytes at a time. If the target string from `stdin` or a standard file read is not padded, the SIMD read instructions can scan past the allocated bounds of the buffer. 

This leads to:
1.  **Segmentation Faults**: Process crashes when the SIMD read crosses a memory page boundary.
2.  **Information Disclosure**: Potential exposure of adjacent heap secrets in memory if they are processed as part of the JSON input.

#### Remediation
Always allocate padding before invoking `simd_json` deserialization, or utilize the safe `serde_json::from_str` or `simd_json::from_slice` with an explicitly padded `Vec<u8>`:
```rust
// Standard, safe fallback
let req: serde_json::Value = serde_json::from_str(&line)?;
```

---

### [Finding 3] Concurrency Starvation & Denial of Service (DoS) in HTTP Server Rate Limiter Throttling
*   **Severity:** High
*   **File:line:** `crates/op-mcp-proxy/src/http_server.rs:125`

#### Description
The HTTP server implements a token-bucket rate limiter that is globally shared as an `Arc<Mutex<TokenBucket>>` in `AppState`. When a request is processed, the rate limiter lock is acquired:
```rust
// crates/op-mcp-proxy/src/http_server.rs:123-135
{
    let wait = state.rate_limiter.lock().await.try_consume().err();
    if let Some(delay) = wait {
        if delay.as_secs() > 5 {
            warn!(wait_ms = delay.as_millis(), "rate limit: rejecting request");
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({ "error": { "message": "rate limit exceeded", "type": "rate_limit_error" } })),
            ).into_response();
        }
        warn!(wait_ms = delay.as_millis(), "rate limit: throttling request");
        tokio::time::sleep(delay).await; // <-- Mutex lock held during await!
    }
}
```
Because the `MutexGuard` is dropped only at the end of the enclosing block, calling `tokio::time::sleep(delay).await` on line 133 keeps the entire rate limiter locked across the asynchronous sleep boundary. 

This causes **complete starvation** of the server: during the sleep interval (up to 5 seconds), no other concurrent connection or thread can acquire `state.rate_limiter.lock()`. A single user triggering a throttle blocks *all other requests* to the proxy server from even checking their rate limits, leading to an instant, easy-to-trigger Denial of Service.

#### Remediation
Release the lock guard before executing the asynchronous sleep:
```rust
let wait = {
    let mut limiter = state.rate_limiter.lock().await;
    limiter.try_consume().err()
};

if let Some(delay) = wait {
    if delay.as_secs() > 5 {
        return (StatusCode::TOO_MANY_REQUESTS, ...).into_response();
    }
    // Lock is now released, sleep safely
    tokio::time::sleep(delay).await;
}
```

---

### [Finding 4] Unauthenticated Identity Spoofing Fallback in WireGuard Public Key Resolution
*   **Severity:** High
*   **File:line:** `crates/op-mcp-proxy/src/session.rs:114`

#### Description
The proxy relies on a WireGuard public key to verify user sessions and identify clients. However, if the command `wg show wg0 public-key` fails (which routinely occurs if the proxy runs as a non-root user or inside a container without administrative network capabilities), the method silently falls back to a deterministic ID based on the system hostname:
```rust
// crates/op-mcp-proxy/src/session.rs:110-117
_ => {
    // Fallback: generate a deterministic ID from hostname
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    warn!("Could not get WireGuard pubkey, using hostname-based ID");
    Ok(format!("local:{}", hostname))
}
```
Hostnames are non-cryptographic, predictable, and often easily modified or guessed. Because session mapping correlates this key with privileged resources (such as user email mappings in `wireguard_users`), any attacker able to guess or set the hostname can masquerade as a legitimate WireGuard peer and hijack proxy sessions.

#### Remediation
Never substitute a cryptographic key identifier with a deterministic or easily spoofed fallback. Fail immediately with an error if the WireGuard interface public key cannot be determined:
```rust
_ => {
    anyhow::bail!("Could not retrieve WireGuard public key; hostname fallbacks are disabled for security.");
}
```

---

### [Finding 5] Insecure Token File Traversal and Scanning
*   **Severity:** Medium
*   **File:line:** `crates/op-mcp-proxy/src/gcloud_auth.rs:328`

#### Description
The `find_token_file_in_dir` helper traverses directories in home path structures looking for any file ending with the `.token` extension:
```rust
fn find_token_file_in_dir(dir: PathBuf) -> Option<PathBuf> {
    std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|ext| ext == "token").unwrap_or(false))
}
```
This design is problematic:
1.  **Lack of Path/Filename Constraints**: It resolves whatever file happens to be read first by the file system explorer. If a user or malicious local process drops an arbitrary token file named `malicious.token` in the target directory, the application will load it without further verification.
2.  **No Permissions Enforcement**: The code does not check file ownership or permissions (e.g., ensuring `0600` on Unix), making it susceptible to loading tokens that are readable/writable by unauthorized users on the same machine.

#### Remediation
Specify an explicit filename (e.g., `session.token`) rather than walking the directory for any matches, and verify on Unix-like systems that the file's metadata permissions are restricted to the owner (`0600` or `0400`):
```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(&path)?;
    if metadata.permissions().mode() & 0o077 != 0 {
        anyhow::bail!("Insecure token file permissions. Must be restricted to owner (0600).");
    }
}
```