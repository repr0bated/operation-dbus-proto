# Code Audit Report: `op-http`

---

## 1. Security & Quality Audit Findings

### Critical Risk Findings

No directly exploitable *Critical* vulnerabilities were identified that could lead to immediate remote code execution (RCE) or complete system compromise based on the isolated static analysis of `op-http` *alone*. However, several severe architectural issues present high-risk exposure if the code is linked to public interfaces.

---

### High Risk Findings

#### [H-01] Command Injection & Denial of Service via Unsanitized OpenSSL Subprocess Spawning
*   **File Citation**: `crates/op-http/src/tls.rs:224`, `crates/op-http/src/tls.rs:242`, `crates/op-http/src/tls.rs:258`
*   **Description**: The utility functions `validate_cert_key_match`, `get_cert_expiry`, and `is_cloudflare_cert` invoke the system's `openssl` binary using `std::process::Command`. While `Command::new` in Rust does not invoke a shell directly (thus preventing basic shell-token splitting injection), passing arbitrary and potentially user-controlled file paths (`cert_path` and `key_path`) to the system process is highly hazardous. A malicious actor able to manipulate these configuration parameters can specify system device files (e.g., `/dev/zero` or `/dev/random`), causing `openssl` to block indefinitely, exhausting system processes and triggering a Denial of Service (DoS). Additionally, if the host environment does not have `openssl` installed or path-accessible, these functions will panic/fail, disrupting certificate initialization.
*   **Remediation**: Avoid executing external system binaries. Use native Rust libraries to parse and validate X.509 certificates and RSA keys (e.g., `rustls-pemfile`, `x509-parser`, or `ring`).

```rust
// Insecure implementation in crates/op-http/src/tls.rs:224
let cert_output = Command::new("openssl")
    .args(["x509", "-in", cert_path, "-noout", "-modulus"])
    .output()
    .map_err(|e| ServerError::CertificateError(format!("Failed to run openssl: {}", e)))?;
```

#### [H-02] Hardcoded Developer and Local System Paths in Certificate Auto-Detection
*   **File Citation**: `crates/op-http/src/tls.rs:136-139`
*   **Description**: The auto-detection logic in `detect_certificates` contains hardcoded system paths belonging to a specific developer's environment (e.g., `"/home/jeremy/certs/cloudflare_origin.pem"` and `"/home/jeremy/certs/cloudflare_origin.key"`). This exposes a developer's private home directory path in the compiled production binary, presenting an information leak. More critically, it creates a brittle, insecure configuration mechanism where production binaries look for keys in non-standard user space.
*   **Remediation**: Remove developer-specific home directory paths. Ensure configuration is strictly managed through standard OS-level directories, secure environment variables (which are checked first), or an explicit configuration file.

#### [H-03] Insecure Incomplete Authentication Bypass Middleware
*   **File Citation**: `crates/op-http/src/request_filters.rs:65-94`
*   **Description**: The `api_key_auth` middleware is fundamentally incomplete. It parses the API key and logs its presence, but does not perform any validation. It ends with an unconditional `next.run(request).await`, effectively bypassing authentication entirely for all endpoints using this filter. If this filter is registered in future router releases, it will present a silent authentication bypass.
*   **Remediation**: Implement a cryptographically secure, constant-time validation of the parsed API keys against a trusted state store, and reject unauthenticated requests with a `401 Unauthorized` status.

---

### Medium Risk Findings

#### [M-01] Wildcard Insecure CORS Configuration
*   **File Citation**: `crates/op-http/src/middleware.rs:188-193`, `crates/op-http/src/request_filters.rs:152-156`
*   **Description**: In both the default middleware builder stack and the native request filters, the CORS configuration is hardcoded to allow *any* origin, *any* method, and *any* header:
    ```rust
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
    ```
    This configuration permits arbitrary third-party websites to make cross-origin requests to this service, exposing the API to Cross-Origin Resource Sharing (CORS) attacks if sensitive data is returned by these endpoints.
*   **Remediation**: Explicitly validate and restrict allowed origins, methods, and headers through the configuration block (`MiddlewareConfig::cors_origins`). Do not default to `Any` in production.

#### [M-02] TLS Handshake Denial of Service (Slowloris Exposure)
*   **File Citation**: `crates/op-http/src/server.rs:109-130`
*   **Description**: The HTTPS connection loop spawns a background Tokio task to accept connections. However, the TLS handshake (`acceptor.accept(stream).await`) is executed without any timeout wrapping. If a malicious client establishes a TCP connection but refuses to complete the TLS cryptographic handshake, the task remains suspended indefinitely. This allows a remote attacker to easily exhaust the server's available file descriptors and system memory (Slowloris/resource exhaustion).
*   **Remediation**: Wrap the TLS handshake in a reasonable timeout using `tokio::time::timeout`.

```rust
// Remediation Example
tokio::spawn(async move {
    match tokio::time::timeout(std::time::Duration::from_secs(5), acceptor.accept(stream)).await {
        Ok(Ok(tls_stream)) => { /* handle connection */ }
        _ => { /* handle timeout/error */ }
    }
});
```

---

### Low Risk Findings

#### [L-01] Panic Potential on Clock Drift in Health Check Endpoint
*   **File Citation**: `crates/op-http/src/health.rs:55`, `crates/op-http/src/health.rs:77`, `crates/op-http/src/health.rs:82`
*   **Description**: The health checker computes uptime and durations by invoking `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()`. If the host system's clock drifts backwards (due to an NTP adjustment, virtualization sleep state, or manual adjustment), `duration_since` returns an `Err`. Calling `.unwrap()` on this result will crash the active worker thread or task immediately.
*   **Remediation**: Use non-panicking code or fallback to a duration of `0`. Alternatively, use `Instant::now()` for measuring relative durations like uptime, as it guarantees monotonic behavior.

```rust
// Remediation Example
let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_else(|_| std::time::Duration::from_secs(0))
    .as_secs();
```

---

## 2. Public API Surface & Dead Code Analysis

### Public API Surface Enumeration
Within the provided codebase, the following public elements are exported. 

*   **Total Public Items**: 64 (Reachable) | 37 (Unreachable/Dead)
*   **Glob Re-exports**: None. Re-exports in `prelude` (`crates/op-http/src/lib.rs:60`) are explicitly targeted imports.

#### Top 10 Most Impactful Public Items
1.  **`HttpServer`** (`crates/op-http/src/server.rs:37`): Main engine handling unified HTTP/HTTPS traffic.
2.  **`HttpServerBuilder`** (`crates/op-http/src/server.rs:159`): Builder implementation for server setup.
3.  **`ServerConfig`** (`crates/op-http/src/server.rs:20`): Structure holding execution parameters (ports, hosts, TLS).
4.  **`MiddlewareConfig`** (`crates/op-http/src/middleware.rs:22`): Struct defining execution configurations (CORS, tracing, timeout).
5.  **`MiddlewareStack`** (`crates/op-http/src/middleware.rs:94`): Aggregator that builds the hyper/tower middle layer.
6.  **`RouterBuilder`** (`crates/op-http/src/router.rs:56`): Structural composability manager for Axum routers.
7.  **`ServiceRouter`** (`crates/op-http/src/router.rs:40`): Core trait used by child modules to publish endpoints.
8.  **`TlsConfig`** (`crates/op-http/src/tls.rs:28`): Data structure managing TLS path assignments and operating mode.
9.  **`TlsMode`** (`crates/op-http/src/tls.rs:16`): Enumeration defining TLS detection modes (`Disabled`, `Enabled`, `Auto`).
10. **`Result`** (`crates/op-http/src/lib.rs:57`): Unified error propagation type.

---

### Dead Code Identification

A significant structural issue exists in the crate's entry point (`crates/op-http/src/lib.rs`). Entire source files are compiled but are completely unreachable because their module declarations are omitted from `lib.rs`.

*   **Unreachable Modules**:
    *   `crates/op-http/src/health.rs` (No `pub mod health;` in `lib.rs`)
    *   `crates/op-http/src/metrics.rs` (No `pub mod metrics;` in `lib.rs`)
    *   `crates/op-http/src/request_filters.rs` (No `pub mod request_filters;` in `lib.rs`)

Due to these omissions, all functions, structs, and modules inside these files are unreachable outside their respective modules. No compiler warnings are triggered because the modules are not declared in the module tree.

#### Public Fields That Should Be Private
Encapsulation is compromised in several configuration structs. Public fields allow callers to bypass the validation logic provided by the builder patterns:
1.  **`MiddlewareConfig`** (`crates/op-http/src/middleware.rs:22`): Fields `cors_enabled`, `cors_origins`, `tracing_enabled`, etc., should be private.
2.  **`ServerConfig`** (`crates/op-http/src/server.rs:20`): Fields `http_port`, `https_port`, `bind_host`, `tls` should be private to prevent modification after instantiation.
3.  **`TlsConfig`** (`crates/op-http/src/tls.rs:28`): Fields `mode`, `cert_path`, `key_path` should be private.

#### Unused Imports
*   `crates/op-http/src/request_filters.rs:12`: `use tower_http::trace::TraceLayer;` (Unused within the module, dead import).

---

### Dead Code Table

| Item | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `health.rs` | Module | `crates/op-http/src/health.rs:1` | Declare `pub mod health;` in `lib.rs` |
| `HealthResponse` | Struct | `crates/op-http/src/health.rs:12` | Expose to allow system monitoring |
| `ServiceHealth` | Struct | `crates/op-http/src/health.rs:22` | Expose to allow system monitoring |
| `HealthChecker` | Struct | `crates/op-http/src/health.rs:28` | Expose to allow system monitoring |
| `metrics.rs` | Module | `crates/op-http/src/metrics.rs:1` | Declare `pub mod metrics;` in `lib.rs` |
| `Metrics` | Struct | `crates/op-http/src/metrics.rs:18` | Register middleware and expose JSON endpoints |
| `ServiceMetrics` | Struct | `crates/op-http/src/metrics.rs:118` | Expose or incorporate into system telemetry |
| `metrics_middleware` | Function | `crates/op-http/src/metrics.rs:194` | Attach as a layer in `MiddlewareStack` |
| `GLOBAL_METRICS` | Static | `crates/op-http/src/metrics.rs:279` | Integrate with Prometheus metrics handler |
| `request_filters.rs` | Module | `crates/op-http/src/request_filters.rs:1` | Declare `pub mod request_filters;` in `lib.rs` |
| `api_key_auth` | Function | `crates/op-http/src/request_filters.rs:65` | Implement key lookup and expose to router |
| `rate_limit` | Function | `crates/op-http/src/request_filters.rs:96` | Complete implementation with state and expose |

---

## 3. Schema-as-Code & Architecture Violations

This codebase has a strict **schema-as-code** discipline using Protocol Buffers and OSCAL for all serialized data structures and configurations. Ad-hoc serializations bypass this validation layer, making integration fragile and preventing automated security assessments.

### Schema-as-Code Infractions

#### [S-01] Ad-Hoc Serialization of Health Payload
*   **File Citation**: `crates/op-http/src/health.rs:12-25`
*   **Description**: `HealthResponse` and `ServiceHealth` are defined as ad-hoc Rust structs with generic serialization attributes:
    ```rust
    #[derive(Serialize, Deserialize)]
    pub struct HealthResponse { ... }
    ```
    This bypasses versioned Protocol Buffers schemas. Any consumer expecting a strict contract has no schema definitions to validate against.
*   **Remediation**: Transition the health status payload to a versioned Protocol Buffers contract. Compile it with `prost` and re-export the generated types.

#### [S-02] Ad-Hoc Manual JSON Construction for Metrics Endpoint
*   **File Citation**: `crates/op-http/src/metrics.rs:238-251`
*   **Description**: The `json_metrics` endpoint constructs unstructured JSON values on the fly using `simd_json::json!`:
    ```rust
    let response = json!({
        "services": service_metrics.into_iter()
            .map(|(name, metrics)| { ... })
    });
    ```
    This lacks contract validation. It should be backed by a protobuf definition.
*   **Remediation**: Declare a versioned Protobuf schema for telemetry reports, populate the generated structs, and serialize using `pb` JSON formatting.

#### [S-03] Ad-Hoc Unvalidated Middleware Configuration
*   **File Citation**: `crates/op-http/src/middleware.rs:22-30`
*   **Description**: `MiddlewareConfig` represents an unvalidated, ad-hoc struct for critical security configurations like CORS and security headers. It lacks compliance validation with standard policy formats such as OSCAL.
*   **Remediation**: Define server and middleware configuration contracts via versioned schemas, allowing centralized policy compliance validation.