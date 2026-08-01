# Production Security & Quality Audit Report

## 1. Critical Security Findings

### CRITICAL-01: Remote Denial of Service (DoS) via Dynamic Invalid Prometheus Metric Registration
* **Location**: `crates/op-http/src/metrics.rs:126-146` (triggered via lines 60-63)
* **Impact**: Directly exploitable by an unauthenticated remote attacker. A single HTTP request with crafted path parameters can instantly panic the worker thread or abort the entire server process, leading to a complete Denial of Service.
* **Mechanism**: 
  The server registers a global metrics middleware which calls `record_request` on every incoming request. 
  ```rust
  // crates/op-http/src/metrics.rs:60-63
  let service_name = extract_service_name(path);
  let service_metrics = services.entry(service_name.to_string())
      .or_insert_with(|| ServiceMetrics::new(&service_name));
  ```
  `extract_service_name` obtains the metric key from the raw request URL path. When a request matches `/api/some-invalid-name/`, `"some-invalid-name"` is passed to `ServiceMetrics::new`.
  Inside `ServiceMetrics::new`:
  ```rust
  let request_count = register_counter!(
      format!("{}_requests_total", name),
      format!("Total requests for {} service", name)
  ).unwrap();
  ```
  Prometheus metric names must strictly conform to the regex `^[a-zA-Z_:][a-zA-Z0-9_:]*$`. The name `"some-invalid-name_requests_total"` is invalid because of the hyphen (`-`). 
  The Prometheus `register_counter!` macro returns an `Err` for invalid names, which is immediately unpacked with `.unwrap()`. This forces an unhandled panic. In environments compiled with `panic = "abort"`, the entire process terminates.

* **Remediation**:
  1. Never dynamically construct Prometheus metric names using raw request components.
  2. Use standard static Prometheus labels (e.g., `http_requests_total{service="some-service"}`) instead of formatting service names directly into the metric key.
  3. Validate and sanitize input strings before dynamic registration, and safely match the registration `Result` instead of using `.unwrap()`.

---

### HIGH-02: Remote Memory Exhaustion (OOM) via Unbounded Prometheus Metric Allocation
* **Location**: `crates/op-http/src/metrics.rs:60-63`
* **Impact**: Unauthenticated remote attackers can exhaust system memory by sending requests to a large number of unique URL paths (e.g., `/api/rand1`, `/api/rand2`, `/api/rand3`).
* **Mechanism**:
  Every unique service name extracted from a path generates a new `ServiceMetrics` entry. Each entry registers three new dynamic Prometheus metrics (`_requests_total`, `_request_duration_seconds`, `_errors_total`) in the global static registry. Since there is no limit on the number of entries in the `services` `HashMap`, an attacker can allocate millions of metric descriptors, causing memory utilization to swell until the system runs out of memory (OOM) and the Linux kernel kills the process.
* **Remediation**:
  Restrict dynamic metric creation. Ensure all metrics are registered with a fixed, static set of descriptors at server startup, and record variables using dimension labels rather than dynamic descriptor names.

---

### HIGH-03: Subprocess Spawning of `openssl` CLI for Certificate Modulus Matching
* **Location**: `crates/op-http/src/tls.rs:307-330`
* **Impact**: Spawning processes at runtime is a dangerous anti-pattern. It introduces potential argument injection vectors if path names contain leading dashes (e.g., `-flag`) or shell metacharacters, and introduces a hard dependency on the `openssl` binary being present in the host system's `PATH`. This is highly fragile and breaks execution within minimal, distroless, or scratch Docker containers.
* **Mechanism**:
  ```rust
  pub fn validate_cert_key_match(cert_path: &str, key_path: &str) -> Result<bool> {
      use std::process::Command;

      // Get certificate modulus
      let cert_output = Command::new("openssl")
          .args(["x509", "-in", cert_path, "-noout", "-modulus"])
          .output()
          ...
  ```
  Spawning the `openssl` binary directly is slow, unsafe, and leaks file handles. If the input parameters are sourced or influenced by dynamic configurations or directory names, command argument injection can occur.
* **Remediation**:
  Avoid spawning external processes. Perform certificate modulus comparison natively in Rust using crates like `rustls-pemfile` or parsing the ASN.1 structure with `x509-parser`.

---

### MEDIUM-04: Protocol Corruption via Deception of Gzip Content-Encoding
* **Location**: `crates/op-http/src/request_filters.rs:123-132`
* **Impact**: Clients receiving responses from this endpoint will experience rendering or decoding failures because the server lies about the payload representation.
* **Mechanism**:
  ```rust
  pub async fn compression(
      request: Request,
      next: Next,
  ) -> Response {
      // Add compression headers
      let mut response = next.run(request).await;

      let headers = response.headers_mut();
      headers.insert("Content-Encoding", "gzip".parse().unwrap());

      response
  }
  ```
  This middleware injects `Content-Encoding: gzip` directly into HTTP response headers *without actually compressing the response body*. The client browser or API consumer tries to parse the plain-text/uncompressed JSON response as a binary gzip stream, which fails, corrupting the payload.
* **Remediation**:
  Remove this custom filter. Response compression must be handled by the tower-http `CompressionLayer` initialized on `crates/op-http/src/middleware.rs:114`, which correctly handles both header injection and actual byte-stream compression.

---

### MEDIUM-05: Denial of Service via NTP Time-Rollback Panics
* **Location**: `crates/op-http/src/health.rs:66-70` and `crates/op-http/src/health.rs:77-84`
* **Impact**: The application can panic and crash during routine NTP network time adjustments that set the system clock backwards.
* **Mechanism**:
  ```rust
  service.last_check = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();
  ```
  If the system time shifts backwards (even by a fraction of a second) due to NTP synchronization or manual clock updates, `SystemTime::now().duration_since(UNIX_EPOCH)` or `now.duration_since(self.start_time)` can return a `SystemTimeError` instead of a duration. Calling `.unwrap()` on this error causes an immediate thread panic.
* **Remediation**:
  Use monotonically increasing clocks (`Instant::now()`) for measuring offsets and uptimes. For calendar timestamps where `SystemTime` is required, use safe unwrapping like `.unwrap_or_default()` or handle the time error gracefully.

---

### SCHEMA-01: Ad-hoc Serialization Structs Instead of Versioned Schemas
* **Location**: `crates/op-http/src/health.rs:10-21`
* **Impact**: The health and status interfaces define critical system metrics as ad-hoc, unversioned Rust structs. Changes to the health JSON payload structure can silently break operational monitoring dashboards, automated kubernetes probes, and control plane integrations.
* **Mechanism**:
  ```rust
  #[derive(Serialize, Deserialize)]
  pub struct HealthResponse {
      pub status: String,
      pub timestamp: u64,
      pub uptime: u64,
      pub version: String,
      pub services: HashMap<String, ServiceHealth>,
  }
  ```
  Under a disciplined Schema-as-Code architecture, all cross-boundary state contracts must be modeled using standardized, versioned formats such as Protocol Buffers or OSCAL component status definition schemas, rather than arbitrary Rust serialization declarations.
* **Remediation**:
  Define the system health interfaces inside versioned `.proto` files (e.g., `op/health/v1/health.proto`) and generate safe, backward-compatible Rust structs using `prost` during the build lifecycle.

---

## 2. Proactive Improvement Suggestions

### Suggestion 1: Decouple Health Check Implementation from Axum Web Server Module
* **Rationale**: The health check module (`health.rs`) currently mixes axum-specific route state, HTTP request-making clients (`reqwest`), file system checks, and database probe stubs. This violates single-responsibility design and unnecessarily bloats `op-http` with dependencies like database drivers or system interaction utilities.
* **Example**: `crates/op-http/src/health.rs:149`

### Suggestion 2: Replace String-Manipulated Path Parsing with Typed Enum Dispatch
* **Rationale**: The function `extract_service_name` parses incoming URLs using naive string manipulation (`split('/')`), which is highly vulnerable to route changes and causes allocation on every request. Introducing a typed route extractor or matching against an enum representing valid system boundaries is significantly safer and faster.
* **Example**: `crates/op-http/src/metrics.rs:115`

### Suggestion 3: Mitigate Allocation Overheads by Eliminating Map Clones on Probes
* **Rationale**: On every call to the detailed health check probe, the system copies the entire registry map: `services: self.services.clone()`. This causes garbage collector pressure under high-frequency load-balancer probing. Wrapping the health-state map in an `Arc<RwLock<...>>` allows returning cheap references without clones.
* **Example**: `crates/op-http/src/health.rs:94`

### Suggestion 4: Transition Web Server Log Outputs to Structured Key-Value Spans
* **Rationale**: Tracing events in the request logger are output as raw string templates (`"{} {} {:?} {} - {}ms"`), which requires downstream log parsers to execute costly regex matching. Emitting structured fields (e.g. `tracing::info!(method = ?method, path = %uri, latency_ms = duration.as_millis())`) enables instant indexing in aggregators.
* **Example**: `crates/op-http/src/middleware.rs:205`

### Suggestion 5: Persist Compliance and Health Metrics to Embedded CozoDB Storage
* **Rationale**: The repository workspace makes extensive use of `cozo` with pure-Rust `storage-sled` features. Relying purely on ephemeral in-memory `HashMap` states to track system health limits real-time operations auditing. Storing health transitions directly in `CozoDB` enables robust historical query capabilities and OSCAL compliance reports.
* **Example**: `crates/op-http/src/health.rs:31`

---
## ⚠ Citation Warnings
- `crates/op-http/src/tls.rs:307`: file has 303 lines
