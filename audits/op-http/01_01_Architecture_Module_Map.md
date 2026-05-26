# Section 1: Architecture & Module Map

### Overview
The `op-http` crate serves as the centralized HTTP/TLS web engine for the `op-dbus` workspace. It coordinates TLS termination (via `rustls`), middleware execution (CORS, tracing, security headers), and router composition, allowing external crates to register sub-routers (e.g., MCP, Chat, Agents, and Websockets) under a single unified web server.

### Module Tree
```text
crates/op-http/src/lib.rs (Crate Root)
├── middleware (crates/op-http/src/middleware.rs)
├── router (crates/op-http/src/router.rs)
├── server (crates/op-http/src/server.rs)
└── tls (crates/op-http/src/tls.rs)

[Unlinked/Dangling Modules - Missing from crates/op-http/src/lib.rs]:
├── health (crates/op-http/src/health.rs)
├── metrics (crates/op-http/src/metrics.rs)
└── request_filters (crates/op-http/src/request_filters.rs)
```

### Entry Points
*   **Library Entry Point**: `crates/op-http/src/lib.rs` - Declares the public API, re-exports main types (server config, router builders), and exposes Axum/Tower dependencies to the rest of the workspace.
*   **Server Lifecycle**: `crates/op-http/src/server.rs` (`HttpServer::serve`) - Orchestrates TCP listeners, TLS handshakes, and spawning connections into the Tokio executor.

### Key Architectural Notes
*   **Dangling Files**: The compilation unit completely ignores `health.rs`, `metrics.rs`, and `request_filters.rs` because they are omitted from the module declarations in `lib.rs`. These files currently act as dead code, meaning crucial security features (like `api_key_auth` and HSTS insertion) are not compiled.
*   **Server Lifecycles**: When TLS is active, `HttpServer` spawns a background task to handle HTTP traffic on port 8080 (for redirection or simple fallback) while driving the HTTPS loop on port 8443 on the primary thread.

---

# Section 2: Security Findings

## [HIGH] Arbitrary CORS Wildcard Permits Cross-Origin Control Plane Exploit
### Code Citation
*   `crates/op-http/src/middleware.rs:181-185`

```rust
// Any origin
CorsLayer::new()
    .allow_origin(Any)
    .allow_methods(Any)
    .allow_headers(Any)
```

### Threat Model & Vulnerability Analysis
The `op-dbus` agent functions as a highly privileged system control plane capable of interacting with system DBus interfaces, executing system actions, and driving network configuration. 

By defining an unrestricted CORS policy (`Any` origin), the server allows any external website visited by an administrator on the same system (or a network-adjacent system) to execute arbitrary AJAX requests targeting `http://localhost:8080` or `https://localhost:8443`. This bypasses the browser's Same-Origin Policy entirely.

### Remediation
Restrict the CORS configuration to specifically validated origins or local trusted clients. If wildcards are necessary for developer environments, ensure they are strictly disabled in production builds via a compile-time feature or configuration variable.

```rust
let cors = if let Some(ref origins) = self.config.cors_origins {
    let origins: Vec<_> = origins.iter().filter_map(|o| o.parse().ok()).collect();
    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(Any)
        .allow_headers(Any)
} else {
    // Fail-secure: Do not fall back to `Any` in production
    CorsLayer::new()
};
```

---

## [HIGH] Unauthenticated Control Plane Exposure on `0.0.0.0`
### Code Citation
*   `crates/op-http/src/server.rs:34`
*   `crates/op-http/src/request_filters.rs:86-90`

```rust
// In crates/op-http/src/server.rs:
bind_host: "0.0.0.0".to_string(),
```
```rust
// In crates/op-http/src/request_filters.rs:
// For now, allow all requests (authentication is optional)
// In production, you would validate the API key here
if let Some(key) = api_key {
```

### Threat Model & Vulnerability Analysis
The HTTP server binds globally to `0.0.0.0` by default, making the control plane APIs accessible over all network interfaces. Simultaneously, the authentication middleware (`api_key_auth`) is a non-functional stub that merely logs the presence of an API key but allows all requests through unconditionally.

An unauthenticated remote attacker on the same network can access the system control APIs, allowing them to invoke agents, extract configurations, or manipulate DBus mirrors.

### Remediation
1. Change the default `bind_host` to `127.0.0.1`.
2. Fully implement the `api_key_auth` filter and enforce token validation, rejecting requests that fail to present a valid credential.

```rust
if let Some(key) = api_key {
    if !validate_secure_token(key) {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
} else {
    return (StatusCode::UNAUTHORIZED, "Missing credentials").into_response();
}
```

---

## [MEDIUM] Unbounded TLS Connection Spawns Enable Handshake Denial of Service (DoS)
### Code Citation
*   `crates/op-http/src/server.rs:102-126`

```rust
loop {
    let (stream, peer_addr) =
        listener.accept().await.map_err(ServerError::BindError)?;
    let acceptor = acceptor.clone();
    let router = self.router.clone();

    tokio::spawn(async move {
        match acceptor.accept(stream).await {
            Ok(tls_stream) => {
                let io = TokioIo::new(tls_stream);
                let service = TowerToHyperService::new(router);

                if let Err(e) =
                    http1::Builder::new().serve_connection(io, service).await
                {
                    tracing::debug!("Connection error from {}: {}", peer_addr, e);
                }
            }
            Err(e) => {
                tracing::debug!("TLS handshake error from {}: {}", peer_addr, e);
            }
        }
    });
}
```

### Threat Model & Vulnerability Analysis
The HTTPS server accepts new TCP connections in an unthrottled loop and immediately spawns a task to perform the TLS handshake. There is no active semaphore, connection limit, or TLS handshake timeout in place.

An attacker can exhaust the system's file descriptors or memory resources by opening thousands of concurrent TCP connections and stalling them mid-handshake. This blocks the main thread from accepting legitimate requests.

### Remediation
Wrap the connection accept loop in a concurrency limiter (e.g., using `tokio::sync::Semaphore`) and apply a strict timeout to the `acceptor.accept(stream)` handshake process.

```rust
use tokio::time::{timeout, Duration};

tokio::spawn(async move {
    match timeout(Duration::from_secs(5), acceptor.accept(stream)).await {
        Ok(Ok(tls_stream)) => {
            // Serve connection
        }
        Ok(Err(e)) => {
            tracing::debug!("TLS handshake error: {}", e);
        }
        Err(_) => {
            tracing::debug!("TLS handshake timed out");
        }
    }
});
```

---

## [MEDIUM] Insecure Silent Plaintext Fallback upon Auto-Detection Failure
### Code Citation
*   `crates/op-http/src/server.rs:65-74`
*   `crates/op-http/src/tls.rs:115-125`

```rust
// In crates/op-http/src/tls.rs:
TlsMode::Auto => {
    if let Some((cert_path, key_path)) = detect_certificates()? {
        ...
    } else {
        warn!("No TLS certificates found, falling back to HTTP");
        Ok(None)
    }
}
```

### Threat Model & Vulnerability Analysis
When `TlsMode::Auto` is enabled, the system tries to locate certificates in common directories. If it fails to find them, it logs a warning and returns `Ok(None)`. The server then silently boots in plaintext HTTP mode on port `8080`.

Operational configurations that depend on Auto-TLS to secure administrative traffic over local networks will silently downgrade to an unencrypted, insecure state if file permissions are misconfigured or paths change.

### Remediation
Fail securely. If Auto-TLS is requested but no valid certificates are found, the server must abort execution and return a configuration error instead of silently falling back to insecure plaintext HTTP.

```rust
TlsMode::Auto => {
    if let Some((cert_path, key_path)) = detect_certificates()? {
        let acceptor = create_tls_acceptor(&cert_path, &key_path)?;
        Ok(Some(acceptor))
    } else {
        return Err(ServerError::CertificateError(
            "Auto-TLS enabled but no certificates could be detected on the system".to_string()
        ));
    }
}
```

---

## [LOW] Shell Execution of External Modulus Verification
### Code Citation
*   `crates/op-http/src/tls.rs:300-316`

```rust
pub fn validate_cert_key_match(cert_path: &str, key_path: &str) -> Result<bool> {
    use std::process::Command;

    // Get certificate modulus
    let cert_output = Command::new("openssl")
        .args(["x509", "-in", cert_path, "-noout", "-modulus"])
        .output()
        ...
```

### Threat Model & Vulnerability Analysis
The function invokes the host's `openssl` CLI tool as an external process to compare moduli. While the arguments are passed safely without spawning a shell, spawning external processes is non-portable, slow, and introduces a dependency on host binary paths. 

If the server runs in a minimal environment (such as a distroless or scratch Docker container), the command execution will fail, resulting in a denial-of-service condition for TLS setup.

### Remediation
Utilize native Rust cryptography crates (such as `rustls-pemfile` or `x509-parser` which are already listed in dependencies) to parse and validate public/private key moduli in-memory rather than relying on external command execution.

---

# Section 3: Schema-As-Code Flagged Issues

The codebase violates the Schema-as-Code discipline by defining critical system data structures using ad-hoc, unversioned Rust structs and dynamic JSON macros, rather than versioned Protocol Buffers or standardized OSCAL schemas.

## 1. Unversioned Health and Monitoring Contracts
*   **File Citation**: `crates/op-http/src/health.rs:11-28`

```rust
pub struct HealthResponse {
    pub status: String,
    pub timestamp: u64,
    pub uptime: u64,
    pub version: String,
    pub services: HashMap<String, ServiceHealth>,
}

pub struct ServiceHealth {
    pub status: String,
    pub message: Option<String>,
    pub last_check: u64,
}
```

### Flagged Issue
The service health endpoints define their payload contracts using ad-hoc `serde` structs. Changes to health checks (such as status enums or metric formatting) are unversioned.

### Remediation
Represent `HealthResponse` and `ServiceHealth` inside a Proto schema (e.g., in `op-dbus-model` or under `op-mcp`'s schema definitions) to establish a versioned interface contract.

---

## 2. Dynamic JSON Construction for Core Metrics
*   **File Citation**: `crates/op-http/src/metrics.rs:259-270`

```rust
let response = json!({
    "services": service_metrics.into_iter()
        .map(|(name, metrics)| {
            (name, simd_json::json!({
                "name": metrics.name,
            }))
        })
        .collect::<simd_json::value::owned::Object<String, simd_json::OwnedValue>>()
});
```

### Flagged Issue
The JSON metrics API serializes service states using dynamic, ad-hoc `simd_json::json!` structures constructed directly inside the endpoint handler. This makes the payload schema invisible to static schema checkers and code generation tools.

### Remediation
Define a formal Protocol Buffer message `ServiceMetricsResponse` to compile the serialized output structure statically, ensuring schemas remain consistent across different versions of the control plane.

---

# Section 4: Operational & Quality Findings

## [HIGH] Dead Code: Dangling Modules Excluded from Compilation Unit
### Code Citation
*   `crates/op-http/src/lib.rs:20-23`

```rust
pub mod middleware;
pub mod router;
pub mod server;
pub mod tls;
```

### Problem Description
The files `health.rs`, `metrics.rs`, and `request_filters.rs` exist in the crate directory but are never declared inside `lib.rs`. 

Because of this, these modules are completely excluded from compilation. Any external workspace crate attempting to register health checks or fetch metric definitions from `op-http` will experience compilation failures. Consequently, dead code remains in the crate and cannot be executed.

### Remediation
Add the missing module declarations to `crates/op-http/src/lib.rs` and expose them as public modules:

```rust
pub mod health;
pub mod metrics;
pub mod middleware;
pub mod request_filters;
pub mod router;
pub mod server;
pub mod tls;
```

---

## [MEDIUM] Clock-Drift Panic via SystemTime Assertion
### Code Citation
*   `crates/op-http/src/health.rs:59-62`
*   `crates/op-http/src/health.rs:75-78`

```rust
service.last_check = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_secs();
```

### Problem Description
The `unwrap()` call on `duration_since(UNIX_EPOCH)` assumes that the host's system clock will always read a value after the UNIX epoch. 

However, if the system clock drifts backwards or is synchronized via NTP (causing the clock to set back by even a fraction of a second), `SystemTime::now()` will return an earlier timestamp, causing `duration_since` to return an `Err` and panicking the entire application.

### Remediation
Avoid `unwrap()` on clock checks. Fall back safely to `0` or use `unwrap_or_default()`:

```rust
service.last_check = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs())
    .unwrap_or(0);
```

---

## [LOW] Hardcoded Local Host Developer Paths in TLS Search Chain
### Code Citation
*   `crates/op-http/src/tls.rs:198-202`

```rust
// User directory
(
    "/home/jeremy/certs/cloudflare_origin.pem",
    "/home/jeremy/certs/cloudflare_origin.key",
),
```

### Problem Description
The certificate search path prioritizes a hardcoded home directory path belonging to a specific developer (`/home/jeremy/...`). 

This creates an operational dependency on local developer setups and poses a minor security risk by leaking username details and system paths within the compiled binary.

### Remediation
Remove hardcoded personal home directories from search arrays. Any user-specific or custom paths must be supplied exclusively through environment variables (such as `SSL_CERT_PATH`) rather than being hardcoded in code.