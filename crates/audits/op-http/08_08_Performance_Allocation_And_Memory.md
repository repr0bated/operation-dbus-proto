# Production Security & Quality Audit: `op-http`

---

## 1. Executive Summary

This audit evaluates the quality, performance, and security posture of the `op-http` crate. The crate serves as the centralized HTTP and TLS ingress for the control plane. 

During the audit, **one Critical vulnerability** was identified that allows unauthenticated, remote attackers to instantly crash the server via a validation panic in the metrics middleware. Additionally, several architectural and performance issues regarding clock drift panics, insecure HSTS implementation, and ad-hoc data contract definitions were found.

---

## 2. Memory Map & Large Allocations

Per the codebase audit constraints, we tracked all memory mapping utilities (`memmap2`, `mmap`, `MmapMut`, `MmapOptions`), `sled` database instances, and large heap allocations across the audited source files.

### Memory Map Table

| Site | file:line | Type (ro/rw/sled) | Risk |
| :--- | :--- | :--- | :--- |
| **N/A** | N/A | N/A | No active memory maps are created within the audited `op-http` source files. (Note: `memmap2` is defined in the workspace dependencies but is not imported or used here). |
| **Sled Usage** | N/A | N/A | Sled is referenced in workspace dependencies but is not directly instantiated or used in the audited `op-http` files. |

### Large Heap Allocations

*   **Prometheus Metric Serialization Buffer**  
    **Location**: `crates/op-http/src/metrics.rs:77`  
    **Allocation**: `let mut buffer = Vec::new();`  
    **Description**: The metrics collection buffer is instantiated with a default capacity of `0` and grows dynamically without bounds as the `TextEncoder` serializes the metrics. Under heavy load with numerous metrics families, this causes multiple reallocations and high heap churn.

---

## 3. Performance & Allocation Hot-Path Analysis

### String Allocation and Formatting in Hot Paths

*   **Conditional Debug Formatting in API Key Middleware**  
    **Location**: `crates/op-http/src/request_filters.rs:94-98`  
    **Description**: In the `api_key_auth` filter, which executes on every incoming request, string slicing and formatting occur inside `format!("{}...{}", &key[..4], &key[key.len()-4..])`. This allocation occurs on every authenticated request even if it is only logged at the `debug` level.
*   **Dynamic Metric Path Formatting**  
    **Location**: `crates/op-http/src/metrics.rs:134-145`  
    **Description**: Every time a new service endpoint is registered dynamically upon receiving a request, `format!("{}_requests_total", name)` and two other formatted strings are constructed on the heap. This behaves as an allocation hot path if service names are highly dynamic.

### Unpadded `simd_json` Usage

*   **`simd_json` Macros**  
    **Location**: `crates/op-http/src/metrics.rs:230-244`  
    **Description**: The codebase uses the safe macro helper `simd_json::json!` to construct JSON values on the fly. No instances of direct, unsafe parsing of unpadded buffers (`simd_json::to_borrowed_value` or `simd_json::from_slice`) are present in this crate.

---

## 4. Schema-as-Code & Data Contract Discipline

The codebase uses a "schema-as-code" discipline based on Protocol Buffers and OSCAL. Ad-hoc structs or dynamically constructed string-based JSON responses are considered violations.

### Violations Identified

1.  **Ad-Hoc Health Status Contracts**  
    **Location**: `crates/op-http/src/health.rs:11-25`  
    **Description**: The `HealthResponse` and `ServiceHealth` structures are defined as ad-hoc Rust structs serialized directly to JSON via `serde`. They are not generated from versioned Protocol Buffer schemas or OSCAL-compliant system component declarations, violating schema-as-code discipline.
2.  **Ad-Hoc JSON Serialization of Metrics**  
    **Location**: `crates/op-http/src/metrics.rs:230-244`  
    **Description**: The `json_metrics` endpoint constructs a dynamic JSON object containing nested service metrics using the `simd_json::json!` macro. The structure of this JSON payload is entirely ad-hoc and not validated against a central versioned schema.

---

## 5. Security & Quality Vulnerabilities

### Finding 1: Unauthenticated Remote Denial of Service via Metric Name Validation Panic (CRITICAL)

*   **Location**: `crates/op-http/src/metrics.rs:134-145` (called via `crates/op-http/src/metrics.rs:58-69`)
*   **Impact**: Direct, unauthenticated remote crash of the entire HTTP server (Denial of Service).
*   **Description**:  
    When a request is processed, the metrics middleware extracts the service name from the path:
    ```rust
    let service_name = extract_service_name(path);
    ```
    If the service name is not yet present in the `services` tracking map, it initializes a new `ServiceMetrics`:
    ```rust
    let service_metrics = services.entry(service_name.to_string())
        .or_insert_with(|| ServiceMetrics::new(&service_name));
    ```
    Inside `ServiceMetrics::new()`, it dynamically registers three Prometheus metrics using the service name:
    ```rust
    let request_count = register_counter!(
        format!("{}_requests_total", name),
        format!("Total requests for {} service", name)
    ).unwrap();
    ```
    The `prometheus` crate has strict naming requirements for metric names, enforced by the regex `^[a-zA-Z_:][a-zA-Z0-9_:]*$`. Characters such as hyphens (`-`), spaces, or special characters are invalid.
    
    If a client sends an HTTP request to an endpoint with an invalid character in the service position (for example, `GET /api/my-service-name/v1`), `extract_service_name` returns `"my-service-name"`. 
    
    `ServiceMetrics::new` is invoked, attempting to register `my-service-name_requests_total`. The `prometheus` library validation fails and returns an `Err`. The code calls `.unwrap()` on the registration attempt, triggering an unhandled panic and immediately crashing the entire runtime.
*   **Remediation**:  
    1. Sanitize the extracted `service_name` to replace any invalid characters with underscores before passing it to metric registration.
    2. Replace the panicking `register_counter!...unwrap()` logic with safe registration or retrieve pre-existing metrics from a static pool. Do not register metrics dynamically from user-supplied URL paths.

---

### Finding 2: Security Header Invalidation / HSTS Spoofing via Client-Controlled Host Headers (HIGH)

*   **Location**: `crates/op-http/src/request_filters.rs:30-36`
*   **Impact**: Potential bypass of HTTPS strict transport security (HSTS) validation, allowing MITM attacks or unexpected browser redirection states.
*   **Description**:  
    The security headers middleware attempts to inject the `Strict-Transport-Security` header conditionally based on checking the client-provided `Host` header:
    ```rust
    if let Some(host) = headers.get("host") {
        if let Ok(host_str) = host.to_str() {
            if host_str.contains(":443") || host_str.starts_with("https://") {
                headers.insert("Strict-Transport-Security",
                    "max-age=31536000; includeSubDomains".parse().unwrap());
            }
        }
    }
    ```
    The `Host` header is completely controlled by the client. An attacker can transmit a `Host` header containing `:443` over an unencrypted, plain HTTP connection. This triggers the server to return an HSTS header over an insecure channel, which is a violation of the HSTS specification (RFC 6797) and can be exploited by local network interceptors to manipulate the client browser's security states.
*   **Remediation**:  
    Only inject the `Strict-Transport-Security` header when the connection is securely established via TLS (e.g., terminated natively at the hyper level) or when verifying a trusted, securely injected upstream header (like `X-Forwarded-Proto: https`) from a loopback-bound reverse proxy.

---

### Finding 3: Denial of Service via SystemTime NTP Backwards-Drift Panics (MEDIUM)

*   **Location**:  
    *   `crates/op-http/src/health.rs:56`
    *   `crates/op-http/src/health.rs:66-71`
    *   `crates/op-http/src/health.rs:170`
    *   `crates/op-http/src/health.rs:199`
    *   `crates/op-http/src/health.rs:217`
*   **Impact**: Process panic and crash during period clock adjustments or drift.
*   **Description**:  
    The health-checking module computes elapsed times and stamps timestamps using `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()`.
    Unlike monotonic clocks, `SystemTime` is subject to clock adjustments (e.g., NTP adjustments, leap seconds, manual time synchronization). If the system time is synchronized backwards even by a millisecond, `duration_since` returns an `Err(SystemTimeError)`. Calling `.unwrap()` on this error causes the application thread to panic and terminate.
*   **Remediation**:  
    Avoid using `.unwrap()` on `SystemTime::duration_since`. Use `.unwrap_or_default()` or fallback mechanisms:
    ```rust
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_else(|e| e.duration().as_secs());
    ```
    For interval timing and uptime calculations, always use monotonic clocks (`Instant::now()`).

---

### Finding 4: Fragile OpenSSL CLI Execution Dependency (LOW)

*   **Location**: `crates/op-http/src/tls.rs:252-259`, `266-271`, and `281-286`
*   **Impact**: Dependency on system binaries, platform-specific command failure, and overhead.
*   **Description**:  
    The functions `validate_cert_key_match`, `get_cert_expiry`, and `is_cloudflare_cert` invoke the system's `openssl` binary via `Command::new("openssl")` to parse certificates. If the host system does not have `openssl` installed, has an incompatible version, or runs in a minimal scratch container environment, these calls will fail. Additionally, invoking an external process on the critical path of initialization or management is inefficient.
*   **Remediation**:  
    Use native Rust parsing libraries like `x509-parser` or `rustls-pemfile` combined with `ring` to extract certificate metadata and check cryptographic key/modulus pairings safely in-process.