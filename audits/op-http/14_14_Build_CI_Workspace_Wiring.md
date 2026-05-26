# Build Check & Schema-As-Code Audit

### Build Configuration Analysis
* **Edition**: The workspace inherits the Rust `2021` edition (`Cargo.toml` workspace package definition). Individual crates like `op-http` use `edition.workspace = true`.
* **Rust Version**: No explicit `rust-version` is specified in either `Cargo.toml` or the crate-specific `Cargo.toml`.
* **Crate Type & Entrypoints**: `op-http` is a library crate (`crates/op-http/src/lib.rs`). It defines no individual binary entrypoints or examples in its `Cargo.toml`.
* **Codegen Risks**: No `build.rs` is present in `crates/op-http`. There is no arbitrary code execution or shell execution occurring at build time within this crate.
* **Workspace Inheritance**: The workspace `Cargo.toml` centralizes dependency management, defining versions for critical crates (such as `axum`, `tokio`, `rustls`, etc.) under `[workspace.dependencies]`. `crates/op-http/Cargo.toml` correctly uses `workspace = true` to inherit these dependency configurations without local overrides.

### Schema-as-Code Build Check
* **Proto Compilation**: `op-http` does not invoke `prost-build` or `tonic-build` directly. However, other workspace members (such as `op-chat` and `op-cognitive-mcp`) depend on `prost-build` and `tonic-build` to compile `.proto` files during build time.
* **Source of Truth**: No Protocol Buffer (`.proto`) files are committed within `crates/op-http`. 
* **Runtime Proto Compilation**: There is no dynamic proto compilation occurring at runtime within `op-http`.
* **Schema-as-Code Violations**: 
  * `crates/op-http/src/health.rs:14`: The `HealthResponse` and `ServiceHealth` data contracts are defined as ad-hoc Rust structs with Serde serialization rather than being derived from versioned, language-neutral Protobuf or OSCAL schemas.
  * `crates/op-http/src/metrics.rs:188`: The `json_metrics` endpoint constructs a highly unstructured, ad-hoc JSON payload dynamically via `simd_json::json!`, violating the schema-as-code discipline.

---

# Security & Quality Audit Findings

## Critical Severity

### 1. Dynamic Prometheus Metric Registration Panic (Denial of Service)
* **File & Line**: `crates/op-http/src/metrics.rs:114` (invoked via `crates/op-http/src/metrics.rs:54-58`)
* **Vulnerability Description**: When an HTTP request is made to the server, the `metrics_middleware` intercepts it, extracts the service name using `extract_service_name(path)`, and attempts to record service-specific metrics. If the path has not been seen before, it dynamically initializes `ServiceMetrics::new(&service_name)`, which executes:
  ```rust
  let request_count = register_counter!(
      format!("{}_requests_total", name),
      format!("Total requests for {} service", name)
  ).unwrap();
  ```
  Prometheus metric names must strictly conform to `[a-zA-Z_:][a-zA-Z0-9_:]*`. If an attacker sends an HTTP request to a path containing characters outside this range (such as a hyphen `/api/foo-bar` or starting with a digit `/api/123`), the resulting name will be invalid (e.g., `foo-bar_requests_total`). The `register_counter!` macro returns an `Err`, which is immediately unwrapped. This triggers a thread panic. If the server is compiled with `panic = "abort"` (standard for production profiles to prevent memory leakage), the entire server process terminates instantly.
* **Exploitation Vector**: An external attacker can crash the central HTTP server by sending a single HTTP request to `https://<target>/api/invalid-char-here/`.
* **Resource Exhaustion Vector**: Because any arbitrary `/api/<unseen-path>` results in a new `ServiceMetrics` struct being dynamically registered in Prometheus, an attacker can send millions of unique paths (e.g., `/api/rand1`, `/api/rand2`) to exhaust server memory and saturate the Prometheus registry, leading to an Out-Of-Memory (OOM) crash.
* **Remediation**: 
  1. Sanitize the extracted `service_name` to ensure only alphanumeric and underscore characters are present.
  2. Implement an allowlist of valid, pre-registered service names instead of dynamically creating metric definitions from untrusted HTTP request paths.
  3. Avoid using `.unwrap()` on global Prometheus metric registration at runtime.

### 2. Broken Compression Middleware Lie (Protocol Failure)
* **File & Line**: `crates/op-http/src/request_filters.rs:115`
* **Vulnerability Description**: The `compression` middleware intercepts responses and forcefully appends the `"Content-Encoding": "gzip"` header to the response:
  ```rust
  let mut response = next.run(request).await;
  let headers = response.headers_mut();
  headers.insert("Content-Encoding", "gzip").parse().unwrap();
  ```
  However, the middleware *never* actually compresses the response body. It passes the raw uncompressed body to the client with the gzip header.
* **Exploitation/Impact**: When a client (browser, reqwest, curl) receives this response, it expects a gzip-compressed stream. Since the payload is raw text/JSON, the client's decompression library fails immediately with a corruption/decoding error (e.g., "invalid gzip header"), breaking all API communication.
* **Remediation**: Delete this manual `compression` filter. Instead, utilize `tower_http::compression::CompressionLayer`, which is already imported and correctly applied via the `MiddlewareStack` in `crates/op-http/src/middleware.rs:125`.

---

## High Severity

### 3. Placeholders Bypass Authentication & Rate Limiting
* **File & Line**: `crates/op-http/src/request_filters.rs:65` (API Auth) and `crates/op-http/src/request_filters.rs:88` (Rate Limiting)
* **Vulnerability Description**: The `api_key_auth` and `rate_limit` middleware functions are non-functional placeholders. They log debug statements but never return an error or block the request. They always call `next.run(request).await`:
  ```rust
  // For now, allow all requests (authentication is optional)
  // In production, you would validate the API key here
  ...
  next.run(request).await
  ```
  If this HTTP server is deployed to expose sensitive system interfaces, such as the DBus control plane, database access, or file-system utilities, anyone on the network can access these endpoints without a valid API key, completely bypassing access controls.
* **Remediation**: Implement actual cryptographic validation of keys/tokens or reject requests returning `StatusCode::UNAUTHORIZED` if a key is missing or invalid. Integrate a functional rate-limiting store (such as a token bucket) instead of allowing all IPs unconditionally.

---

## Medium Severity

### 4. Permissive Wildcard CORS Configuration by Default
* **File & Line**: `crates/op-http/src/middleware.rs:141-153`
* **Vulnerability Description**: When no specific origins are defined in the `MiddlewareConfig`, the system defaults to allowing `Any` origin, `Any` method, and `Any` header:
  ```rust
  } else {
      // Any origin
      CorsLayer::new()
          .allow_origin(Any)
          .allow_methods(Any)
          .allow_headers(Any)
  };
  ```
  Since this application interfaces directly with local Linux host system controllers over DBus, exposing a fully open CORS policy on all routes allows malicious websites visited by a user on the same machine to conduct cross-origin requests against local administrative APIs.
* **Remediation**: Force CORS to default to strict local origins (e.g., `127.0.0.1` or specific authorized domains) rather than falling back to `Any`.

### 5. Potential Panic on Backward System Clock Shifting
* **File & Line**: `crates/op-http/src/health.rs:68` and `crates/op-http/src/health.rs:73`
* **Vulnerability Description**: The `check_health` function calculates timestamps using:
  ```rust
  let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
  ```
  If the host system clock shifts backwards (due to NTP sync adjustments, virtualization pauses, or manual correction) to a time prior to the Unix epoch, `duration_since(UNIX_EPOCH)` returns an `Err`. Calling `.unwrap()` on this result immediately panics the active task.
* **Remediation**: Use `.unwrap_or_default()` or handle the error gracefully to prevent system clock anomalies from crashing the health status routine.

---

## Low / Quality Severity

### 6. Subprocess Spawning of `openssl` CLI for Cryptographic Validation
* **File & Line**: `crates/op-http/src/tls.rs:276`, `crates/op-http/src/tls.rs:291`, and `crates/op-http/src/tls.rs:305`
* **Defect Description**: The TLS helper functions `validate_cert_key_match`, `get_cert_expiry`, and `is_cloudflare_cert` use `std::process::Command` to invoke the external `openssl` binary on the host platform. Spawning subprocesses is highly inefficient, introduces a critical runtime dependency on the presence of the `openssl` executable (which may not exist in minimal distroless Docker environments), and risks process exhaustion bugs.
* **Remediation**: Perform these validations programmatically in Rust using standard cryptographic crates (e.g., `rustls`, `x509-parser`, or `openssl` FFI bindings) rather than invoking raw shell commands.

### 7. Hardcoded Developer Paths & Domain Names
* **File & Line**: `crates/op-http/src/tls.rs:188` and `crates/op-http/src/tls.rs:219`
* **Defect Description**: The automated TLS certificate discovery routine contains hardcoded paths belonging to a developer's private workstation (`/home/jeremy/certs/cloudflare_origin.pem`) and specific proprietary domains (`ghostbridge.tech`, `proxmox.ghostbridge.tech`). This represents a significant configuration leak and violates portability standards.
* **Remediation**: Remove proprietary domains and explicit home directory paths. Expose certificate paths solely via dynamic configuration files, CLI arguments, or standard environment variables (`SSL_CERT_PATH`, `SSL_KEY_PATH`).

---
## ⚠ Citation Warnings
- `crates/op-http/src/tls.rs:305`: file has 303 lines
