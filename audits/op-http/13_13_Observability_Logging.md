# Production Security and Quality Audit: `op-http`

## 1. Observability Instrumentation Overview

### Macro Counts
Across the `op-http` crate, the usage of structured logging macros (`tracing::*`) versus standard output macros (`println!`) is distributed as follows:

*   **`tracing::` / `log` macros**: **29**
    *   `crates/op-http/src/middleware.rs`: 3 (`tracing::error!`, `tracing::warn!`, `tracing::info!`)
    *   `crates/op-http/src/request_filters.rs`: 4 (`tracing::info!`, `tracing::debug!` x2, `tracing::error!`)
    *   `crates/op-http/src/router.rs`: 3 (`info!` x2, `tracing::warn!`)
    *   `crates/op-http/src/server.rs`: 9 (`tracing::error!`, `info!` x6, `tracing::debug!` x2)
    *   `crates/op-http/src/tls.rs`: 10 (`info!` x8, `warn!` x2)
    *   `crates/op-http/src/health.rs`: 0
    *   `crates/op-http/src/metrics.rs`: 0
*   **`println!` macros**: **0**

### Metrics Instrumentation
The crate uses the `prometheus` crate to instrument HTTP performance metrics:
*   **Global Metrics**: Exposes `http_requests_total` (Counter), `http_request_duration_seconds` (Histogram), and `http_active_connections` (Gauge) in `crates/op-http/src/metrics.rs:27-39`.
*   **Service-Specific Metrics**: Registers dynamic counters and histograms on demand based on extracted path segments in `crates/op-http/src/metrics.rs:109-121`.

---

## 2. Critical Vulnerabilities & Exploits

### CRITICAL: Remote-Triggerable Server Panic via Prometheus Metric Injection (DoS)
*   **Citation**: `crates/op-http/src/metrics.rs:109-112`, `crates/op-http/src/metrics.rs:144-150`
*   **Threat Model**: An unauthenticated remote attacker can trigger a process-wide panic (Denial of Service) by sending a request to a specially crafted URI path.
*   **Mechanism**:
    1. In `metrics_middleware` (line 157), the request's path is evaluated by `extract_service_name` (line 144).
    2. If the request path begins with `/api/`, `extract_service_name` returns the second path segment directly, e.g., `/api/invalid-name` returns `"invalid-name"`.
    3. `Metrics::record_request` attempts to retrieve or insert the dynamic `ServiceMetrics` struct for this segment (line 56).
    4. If it is a new segment, `ServiceMetrics::new` is executed, calling `register_counter!` with the formatted name: `format!("{}_requests_total", name)`.
    5. The `prometheus` crate enforces a strict regular expression for metric names: `[a-zA-Z_:][a-zA-Z0-9_:]*`. It explicitly forbids hyphens (`-`), slashes, spaces, and other special characters.
    6. Passing `"invalid-name"` results in an invalid Prometheus metric name (`"invalid-name_requests_total"`), causing `register_counter!` to return an `Err`.
    7. Because the code immediately invokes `.unwrap()` on the registry result (line 112), the thread panics. Since the middleware executes within the connection handler, this triggers an unhandled task panic, facilitating resource exhaustion and service disruption.
*   **Exploit Vector**:
    ```bash
    curl http://<server>:8080/api/exploit-payload-causes-panic
    ```

### CRITICAL: Server Socket Loop Self-Termination upon Transient Connection Errors (DoS)
*   **Citation**: `crates/op-http/src/server.rs:104-105`
*   **Threat Model**: A transient network failure, client reset, or file descriptor limit exhaustion (`EMFILE`) will cause the HTTPS server to permanently stop accepting new connections.
*   **Mechanism**:
    In the main HTTPS listener loop:
    ```rust
    loop {
        let (stream, peer_addr) =
            listener.accept().await.map_err(ServerError::BindError)?;
    ```
    If `listener.accept()` returns any `std::io::Error` (e.g., due to firewall connection cuts, transient OS socket errors, or resource limits), the `map_err(ServerError::BindError)?` statement converts it and bubbles the error out of the entire `serve` method. This immediately terminates the execution loop of the HTTPS server, leaving the process running but completely unresponsive to subsequent TLS traffic.
*   **Exploit Vector**: Trigger file descriptor exhaustion on the host or flood the server with rapid TCP connection resets (`RST`) during the handshaking phase to cause `accept()` to return an error, terminating the daemon.

---

## 3. High & Medium Observability Risks

### HIGH: Leakage of High-Entropy Credentials (API Keys/Passwords) to Debug Logs
*   **Citation**: `crates/op-http/src/request_filters.rs:81-89`
*   **Risk**: The `api_key_auth` middleware checks for secrets across multiple headers, including `x-password` (line 78). If a secret is provided and its length is strictly greater than 8, the following sanitization is applied:
    ```rust
    format!("{}...{}", &key[..4], &key[key.len()-4..])
    ```
    If an operator or user sets an 8-character password, it is safely masked as `"***"`. However, if they set a 9-character password (e.g., `"secret123"`), the system logs: `secr...t123`. This exposes **8 out of 9 characters (88%) of the plaintext password** directly to the system logs, rendering brute-forcing trivial.

### HIGH: Sensitive Data Exposure in Request Logging (Plaintext Query String Logs)
*   **Citation**: `crates/op-http/src/middleware.rs:228-256`, `crates/op-http/src/request_filters.rs:54`
*   **Risk**: Both `request_logging_middleware` and `request_logger` write the raw request `uri` directly to logs at `info` or `warn` levels. If clients send sensitive session tokens, transient credentials, or personally identifiable information (PII) within query parameters or path segments (e.g., `/api/users/foo@bar.com/reset?token=abc123xyz`), these parameters are saved in plaintext logs without redacting potential secrets.

### MEDIUM: Silently Swallowed Initialization and Connectivity Errors
*   **Citation**: `crates/op-http/src/health.rs:149-166`, `crates/op-http/src/server.rs:117`, `crates/op-http/src/server.rs:121`
*   **Risk**: 
    *   In `check_service_health` (health check client), if `reqwest::Client::builder().build()` or `client.get().send()` fails, the resulting errors are formatted into the returned `ServiceHealth` JSON struct, but **never** logged to the central logging system via `tracing`. If health checks are failing due to a system misconfiguration, the logs will remain entirely blank.
    *   TLS handshake errors (line 121) and connection handling errors (line 117) are captured and logged strictly at the `debug!` level. In standard production environments (typically run at `info` or `warn` level), configuration issues such as expired certs, invalid cipher negotiations, or handshake failures are completely hidden from operators.

---

## 4. Schema-as-Code Compliance Gaps

This codebase implements a custom, ad-hoc, unversioned serialization strategy for core data contracts rather than expressing schemas in versioned formats like Protocol Buffers or OSCAL:

*   **Ad-hoc Health Check Schema**: `crates/op-http/src/health.rs:13-31`
    `HealthResponse` and `ServiceHealth` use ad-hoc JSON structures mapping arbitrary string hash maps to inner models. This violates schema-as-code principles; service-level state reports should be codified as versioned Protobuf payloads.
*   **Ad-hoc Metrics Serialization**: `crates/op-http/src/metrics.rs:175-188`
    The JSON metrics handler relies on unchecked, unstructured `simd_json::json!` documents containing dynamically nested string maps. There are no static schemas validating the keys, types, or structure of the returned payload.