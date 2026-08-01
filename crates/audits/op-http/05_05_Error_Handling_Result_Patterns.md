### Section 1: Error Handling API Counts

| Operation / Macro | Count |
| :--- | :--- |
| `.unwrap()` | 24 |
| `.expect()` | 0 |
| `.unwrap_or()` | 3 |
| `?` operator | 17 |
| `todo!()` | 0 |
| `unimplemented!()` | 0 |
| `panic!()` | 0 |

---

### Section 2: Initial `.unwrap()` Site Audit

The first 5 `.unwrap()` calls detected in the codebase are detailed below, along with risk ratings and structural recommendations:

#### 1. `crates/op-http/src/health.rs:74`
```rust
service.last_check = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
```
* **Risk**: Low. Fails only if the host system's real-time clock is set to a date prior to Jan 1, 1970.
* **Recommendation**: Replace with a safe fallback: `SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)`.

#### 2. `crates/op-http/src/health.rs:83`
```rust
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
```
* **Risk**: Low. Same clock-skew condition as above.
* **Recommendation**: Replace with a fallback: `SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)`.

#### 3. `crates/op-http/src/health.rs:88`
```rust
let uptime = now - self.start_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
```
* **Risk**: Low. Could panic if `self.start_time` is somehow in the future relative to the system clock.
* **Recommendation**: Use `self.start_time.elapsed().map(|d| d.as_secs()).unwrap_or(0)`.

#### 4. `crates/op-http/src/health.rs:158`
```rust
let start_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
```
* **Risk**: Low. Fails only under pre-epoch system times.
* **Recommendation**: Replace with a fallback: `SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)`.

#### 5. `crates/op-http/src/metrics.rs:31`
```rust
let request_count = register_counter!("http_requests_total", "Total number of HTTP requests").unwrap();
```
* **Risk**: Low-Medium (Startup Panic). Fails if a metric with the same name is already registered in the global collector.
* **Recommendation**: Handle the registration error at initialization, or use a custom lazy registry setup that safely logs registration failure instead of calling `.unwrap()` inside `Metrics::new`.

---

### Section 3: Lock Poisoning Risk Audit

The codebase uses `tokio::sync::RwLock` for state management in the `Metrics` struct:
* **`crates/op-http/src/metrics.rs:15`**: `use tokio::sync::RwLock;`
* **`crates/op-http/src/metrics.rs:25`**: `services: Arc<RwLock<HashMap<String, ServiceMetrics>>>`
* **`crates/op-http/src/metrics.rs:55`**: `let mut services = self.services.write().await;`
* **`crates/op-http/src/metrics.rs:90`**: `self.services.read().await.clone()`

#### Lock Poisoning Assessment:
There is **no lock poisoning risk** present from these operations. Tokio’s `RwLock` and `Mutex` implementations do not implement standard lock poisoning. Unlike `std::sync::Mutex` or `std::sync::RwLock`, which return a poison `Result` if a thread panics while holding the lock, Tokio's async synchronization primitives do not propagate poisoning across tasks. Furthermore, no standard-library locks (`std::sync::Mutex` or `std::sync::RwLock`) are unwrapped anywhere in the provided codebase.

---

### Section 4: Production Vulnerability Analysis

#### CRITICAL: Unauthenticated Remote Denial of Service (DoS) via Invalid Metric Registration Names
* **Location**: `crates/op-http/src/metrics.rs:55-61` and `crates/op-http/src/metrics.rs:107-120`
* **Vulnerability Description**: 
  The HTTP server logs metrics per service using paths extracted directly from incoming unauthenticated HTTP request URIs. 
  ```rust
  // metrics.rs:58
  let service_name = extract_service_name(path);
  let service_metrics = services.entry(service_name.to_string())
      .or_insert_with(|| ServiceMetrics::new(&service_name));
  ```
  Inside `ServiceMetrics::new`, the extracted `name` string is directly formatted into the Prometheus metric names:
  ```rust
  // metrics.rs:108
  let request_count = register_counter!(
      format!("{}_requests_total", name),
      format!("Total requests for {} service", name)
  ).unwrap();
  ```
  Prometheus metric names have a strict format constraint: they must match the regular expression `^[a-zA-Z_:][a-zA-Z0-9_:]*$`. Characters such as hyphens (`-`), spaces, dots (`.`), and percent-encodings are illegal.
  If a remote attacker sends an unauthenticated request to an endpoint such as `GET /api/bad-name/` or `GET /api/bad.name/`, `extract_service_name` will return `"bad-name"` or `"bad.name"`. Since this key does not exist in `services`, the server calls `ServiceMetrics::new("bad-name")`, which attempts to execute `register_counter!("bad-name_requests_total")`.
  The Prometheus registration fails due to the illegal character and returns an `Err`. Because `register_counter!(...).unwrap()` is executed, **the entire worker thread/task will panic, causing an immediate crash or state corruption.** An attacker can continuously send requests containing invalid path characters to rapidly crash the server.
* **Remediation**:
  1. **Strictly validate** or sanitize the extracted service name to ensure it matches the allowed Prometheus regex before attempting registration.
  2. Map any invalid character to an underscore (`_`) or fall back to an `"unknown"` metric bucket.
  3. Never use `.unwrap()` inside `ServiceMetrics::new`. Propagate the error using `Result` or log a warning and return a dummy metric collector.

#### HIGH: Hardcoded Authentication Bypass in API Key Middleware
* **Location**: `crates/op-http/src/request_filters.rs:67-85`
* **Vulnerability Description**:
  The `api_key_auth` middleware parses standard authorization and API key headers (`x-api-key`, `authorization`, `x-password`), but explicitly contains a hardcoded bypass that allows all requests to pass through regardless of validity:
  ```rust
  // For now, allow all requests (authentication is optional)
  // In production, you would validate the API key here
  if let Some(key) = api_key {
      ...
  }
  next.run(request).await
  ```
  Any route that is configured to use `api_key_auth` is entirely unprotected, exposing internal APIs to the public without authentication.
* **Remediation**:
  Remove the bypass immediately. Implement actual verification of `api_key` against a state-managed store or configuration parameters, returning `StatusCode::UNAUTHORIZED` if validation fails.

#### HIGH: Arbitrary Command Execution and File Processing Risk during Certificate Validation
* **Location**: `crates/op-http/src/tls.rs:309-346`
* **Vulnerability Description**:
  The function `validate_cert_key_match` runs external shell commands to extract moduli using `openssl` directly on the provided file paths:
  ```rust
  let cert_output = Command::new("openssl")
      .args(["x509", "-in", cert_path, "-noout", "-modulus"])
      .output()
      ...
  ```
  If `cert_path` or `key_path` is ever read from user-supplied parameters, API queries, or unvalidated configuration uploads, this constitutes an arbitrary path validation risk or command execution. Additionally, calling external shell binaries instead of using native Rust TLS crate validation introduces major portability, execution latency, and security surface vulnerabilities.
* **Remediation**:
  Avoid spawning a shell to execute `openssl`. Validate certificates and keys natively using the `rustls` or `x509-parser` libraries.

#### MEDIUM: Fragile Host-Based HTTPS Validation for HSTS Headers
* **Location**: `crates/op-http/src/request_filters.rs:27-33`
* **Vulnerability Description**:
  The security middleware applies HSTS headers based entirely on the `Host` header:
  ```rust
  if let Some(host) = headers.get("host") {
      if let Ok(host_str) = host.to_str() {
          if host_str.contains(":443") || host_str.starts_with("https://") {
              headers.insert("Strict-Transport-Security", ...);
          }
      }
  }
  ```
  The HTTP `Host` header is completely controlled by the client and is easily manipulated. If a reverse proxy handles TLS termination and forwards traffic over port 80 to this Axum application, the `Host` header will not contain `:443` or `https://`. Consequently, HSTS will be stripped from connections that are actually HTTPS, exposing downstream users to SSL-stripping vectors.
* **Remediation**:
  Rely on reliable proxy headers such as `X-Forwarded-Proto` (checking if it equals `https`) to determine HSTS deployment, or configure the HSTS rules strictly via global static server configuration.

---

### Section 5: Schema-as-Code Violations

This codebase uses ad-hoc structs and unstructured formats rather than versioned serialization schemas (such as Protocol Buffers or JSON Schemas) to communicate with external interfaces:

1. **Ad-hoc Health Models (`crates/op-http/src/health.rs:12-28`)**:
   `HealthResponse` and `ServiceHealth` are defined as ad-hoc Rust structs serialized directly to JSON via Serde. This violates schema-as-code discipline. These structures should be derived from versioned `.proto` (Protobuf) or JSON Schema files to ensure backwards compatibility as health checks evolve across the workspace.
2. **Unstructured JSON Metrics (`crates/op-http/src/metrics.rs:206-218`)**:
   The `json_metrics` endpoint constructs JSON dynamically using the `simd_json::json!` macro:
   ```rust
   let response = json!({
       "services": service_metrics.into_iter()
           .map(|(name, metrics)| { ... })
   });
   ```
   No schema contract is established, creating potential ingestion failures for monitoring dashboards if fields are added or modified.
3. **Plain-Text Error Responses (`crates/op-http/src/request_filters.rs:170-176`)**:
   The global `error_handler` returns a plain-text string `"Internal Server Error"`. To maintain consistent service contracts, errors should return structured, schema-validated payloads (e.g., an OSCAL-aligned incident structure or a schema-compliant error JSON envelope).

---
## ⚠ Citation Warnings
- `crates/op-http/src/tls.rs:309`: file has 303 lines
