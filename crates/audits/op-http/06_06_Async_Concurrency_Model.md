# Production Security & Quality Audit Report: op-http

## 1. Async & Concurrency Audit Metrics

This section quantifies the asynchronous components and concurrency primitives used in `op-http`.

### Exact Metric Counts
*   **Total `async fn` Count:** **29**
*   **Total `tokio::spawn` Count:** **2**
*   **Total `spawn_blocking` Count:** **0**

### Blocking Code in Asynchronous Contexts
*   **Reactor-Blocking File System Calls:**
    *   **Citation:** `crates/op-http/src/health.rs:232`
    *   **Vulnerability:** Within the asynchronous function `check_filesystem_health`, the code directly invokes `std::path::Path::new(path).exists()`. This call performs synchronous disk I/O, which blocks the thread of the Tokio reactor. In environments with high disk latency or unresponsive network-mounted filesystems, this can cause reactor starvation, severely degrading server performance and latency.
    *   **Remediation:** Wrap the filesystem check inside `tokio::task::spawn_blocking` or use `tokio::fs::metadata`.

*   **Synchronous Subprocess Execution:**
    *   **Citation:** `crates/op-http/src/tls.rs:293-339`
    *   **Vulnerability:** The utilities `validate_cert_key_match`, `get_cert_expiry`, and `is_cloudflare_cert` are written as synchronous functions executing external `openssl` processes via `std::process::Command`. Although they are not currently declared as `async fn` in this crate, if they are called inside any handler or background task, they will block the thread execution pool.
    *   **Remediation:** Standardize on safe Rust native parsers (such as `rustls` or `x509-parser`) rather than shelling out to external processes. If subprocesses must be used, they must be spawned asynchronously via `tokio::process::Command` or dispatched using `tokio::task::spawn_blocking`.

### Unhandled and Dropped JoinHandles
*   **Silent Background Task Failures:**
    *   **Citation:** `crates/op-http/src/server.rs:94` and `crates/op-http/src/server.rs:111`
    *   **Vulnerability:** `tokio::spawn` is invoked to run the secondary HTTP listener and individual client connections. The returned `JoinHandle` values are ignored. If the HTTP server fails to bind (e.g. port already in use) or terminates abruptly due to a panic, the failure will go completely unnoticed by the main thread. This leads to a silently degraded state where only the HTTPS listener is active, or connections are terminated with no fail-fast recovery.
    *   **Remediation:** Keep track of the `JoinHandle`s and use a task selector or coordinator (such as `tokio::select!` or a supervisor task) to ensure that if any critical listener exits, the parent process is notified or restarts appropriately.

---

## 2. Critical Security Findings

### Dynamic Prometheus Metric Registration Panic & Denial of Service (DoS)
*   **Severity:** Critical
*   **Citation:** `crates/op-http/src/metrics.rs:114` (called via line 58)
*   **Vulnerability:** 
    The `metrics_middleware` uses `record_request` to register and update performance metrics. Under the hood, `extract_service_name` parses the 3rd segment of the HTTP request path starting with `/api/` or `/ws/` to use as the service name. For any request to a path like `/api/some-service`, `some-service` is extracted.
    
    This extracted string is formatted dynamically into metric names like `{}_requests_total` and passed directly into `register_counter!` or `register_histogram!`, followed by `.unwrap()`. 
    
    1. **Character Validation Panic:** Prometheus metric names must strictly conform to the regular expression `^[a-zA-Z_:][a-zA-Z0-9_:]*$`. If a client sends an HTTP request containing a hyphen, space, or other non-alphanumeric characters in the path (e.g., `/api/user-login/`), `extract_service_name` extracts `user-login`. When `ServiceMetrics::new` attempts to register `user-login_requests_total`, the Prometheus registry returns an `Err`. Due to the unconditioned `.unwrap()`, this immediately panics the active thread/connection task.
    2. **Memory Exhaustion via Metric Cardinality Explosion:** Because service names are registered dynamically based on arbitrary client-supplied paths, a remote attacker can send millions of requests with random paths (e.g., `/api/rand1`, `/api/rand2`, etc.). This allocates and registers three new global Prometheus metrics on every request. Since the global registry is static, this memory cannot be reclaimed, leading to immediate heap exhaustion and an Out-Of-Memory (OOM) crash.
*   **Exploitation:** An unauthenticated attacker can crash the service by sending a single request to `/api/trigger-crash/`, or repeatedly call varying valid-looking API endpoints to trigger a slow or rapid OOM.
*   **Remediation:**
    *   Validate the extracted service name against a strict whitelist before dynamically formatting and registering it.
    *   Avoid dynamic global metric registration on untrusted inputs. Instead, use static metrics with dynamic labels (e.g., a label `service="user-login"` on a pre-registered generic counter `http_service_requests_total`).

---

## 3. High/Medium Severity Vulnerabilities and Bugs

### Cryptographic Validation Bypass on OpenSSL Failures
*   **Severity:** High
*   **Citation:** `crates/op-http/src/tls.rs:293-310`
*   **Vulnerability:** 
    `validate_cert_key_match` executes external `openssl` commands to extract the modulus from both the certificate and the key, then compares their standard output.
    
    `Command::output()` only returns an `Err` if the binary itself cannot be run. If `openssl` fails at runtime (due to permissions, malformed file structure, or invalid arguments), it prints an error to standard error and exits with a non-zero code. In this case, `cert_output.stdout` and `key_output.stdout` will both be empty `Vec`s.
    
    Because the code compares `cert_output.stdout == key_output.stdout` without checking the process exit status, two empty vectors compare as `true`. This causes the validator to falsely confirm that a mismatched, corrupted, or completely invalid certificate and key are a valid match.
*   **Remediation:** Validate the exit status of both subprocesses before comparing stdout:
    ```rust
    if !cert_output.status.success() || !key_output.status.success() {
        return Err(ServerError::CertificateError("OpenSSL execution failed".to_string()));
    }
    ```

### Unconditional Header Insertion Corrupting Client Responses
*   **Severity:** High
*   **Citation:** `crates/op-http/src/request_filters.rs:110-121`
*   **Vulnerability:** 
    The `compression` middleware unconditionally inserts the `Content-Encoding: gzip` header on any HTTP response that flows through it. However, the middleware does not perform any compression on the response body bytes.
    
    When an HTTP client (such as a web browser) receives the response, it reads the `Content-Encoding: gzip` header and attempts to decompress the uncompressed plain-text or JSON body. This will lead to a parsing failure, raw binary decoding crash, or broken UI page load for the end-user.
*   **Remediation:** Remove the ad-hoc `compression` middleware. Utilize standard, fully-featured middlewares like `tower_http::compression::CompressionLayer` which correctly manage body writers and write compression envelopes.

### Incomplete/Security Bypass Auth and Rate-Limiter Placeholders
*   **Severity:** Medium
*   **Citation:** `crates/op-http/src/request_filters.rs:64-90` and `crates/op-http/src/request_filters.rs:91-108`
*   **Vulnerability:** 
    The `api_key_auth` and `rate_limit` filters are exposed as middleware but behave as no-ops. They parse credentials and real client IPs, but ultimately log them and unconditionally permit the execution via `next.run(request).await`. 
    
    If these filters are mistakenly relied upon in configuration or deployment to protect internal admin/mcp boundaries, the application will remain wide open to unauthenticated API abuse and brute-force resource starvation attacks.
*   **Remediation:** Implement actual key validation and state-based rate limiting (using e.g., Redis or an in-memory sliding window), or fail-fast and reject compilation if configured in production mode without real filters.

---

## 4. Schema-as-Code Compliance Findings

According to project discipline, all data contracts, configurations, and API interfaces must be explicitly declared as versioned schemas (such as Protocol Buffers or OSCAL compliance documents) rather than dynamic types or ad-hoc language-native definitions.

### Ad-hoc Serialization Structs instead of Versioned Schemas
*   **Citation:** `crates/op-http/src/health.rs:10` and `crates/op-http/src/health.rs:20`
*   **Violation:** The data payloads `HealthResponse` and `ServiceHealth` are defined as ad-hoc Rust structs with generic serialization attributes (`#[derive(Serialize, Deserialize)]`). They are not mapped to versioned Protobuf models or OSCAL schema definitions.
*   **Remediation:** Port these structures to a Protocol Buffer specification (e.g. `health.proto`) or map them directly to a standard compliance schema, then compile them into Rust using `prost`.

### Unstructured Dynamic JSON Response Generation
*   **Citation:** `crates/op-http/src/metrics.rs:195-212`
*   **Violation:** In the `json_metrics` endpoint, the returned payload contract is dynamically constructed via `simd_json::json!`. It lacks a defined, versioned, compile-time enforced contract, making it prone to breaking upstream clients if changed or modified.
*   **Remediation:** Define a dedicated metrics response contract in a versioned schema file, compile it, and return that strongly-typed struct instead of a dynamically constructed raw JSON object.