# Integration & Security Audit: `op-http`

## 1. Workspace Crates Depending on `op-http`
Based on the workspace configuration in `Cargo.toml` and the dependency tree in `Cargo.lock`, the following internal crates explicitly depend on `op-http`:
* **`op-agents`** (declared in `Cargo.lock` package dependencies)
* **`op-tools`** (declared in `Cargo.lock` package dependencies)

---

## 2. Registered D-Bus Service Names and Object Paths
No D-Bus services or object paths are registered in the provided source files for `op-http`. Although `zbus` is defined in the workspace dependencies (`Cargo.toml`), the actual registration of D-Bus paths and control-plane interfaces is decoupled from this HTTP server layer.

---

## 3. Exposed HTTP and gRPC Endpoints

### Explicit HTTP Routes (Handlers Defined in `crates/op-http`)
* **Health Check Endpoints** (`crates/op-http/src/health.rs:114-142`):
  * `health_check` (returns `"OK"`)
  * `detailed_health_check` (returns a JSON payload matching the `HealthResponse` schema)
  * `readiness_check` (returns `200 OK` or `503 SERVICE UNAVAILABLE` with a JSON payload)
  * `liveness_check` (returns `"OK"`)
* **Metrics & Dashboard Endpoints** (`crates/op-http/src/metrics.rs:175-236`):
  * `prometheus_metrics` (exposes Prometheus exposition format data)
  * `json_metrics` (exposes serialized JSON metrics via `simd-json`)
  * `metrics_dashboard` (serves a basic frontend monitoring page)

### Abstract Router Composition (Defined in Comments / Trait Structures)
`crates/op-http/src/lib.rs:9-19` details a composition model where routes from external crates are nested under the central server:
* `/api/mcp/*` (delegated to `op_mcp::create_router()`)
* `/api/chat/*` (delegated to `op_chat::create_router()`)
* `/api/web/*` (delegated to `op_web::create_router()`)
* `/api/tools/*` (delegated to `op_tools::create_router()`)
* `/api/agents/*` (delegated to `op_agents::create_router()`)
* `/ws/*` (WebSocket connection upgrades)
* `/*` (static assets via `ServeDir` in `crates/op-http/src/router.rs:109-117`)

---

## 4. Cross-Crate Circular Dependency Analysis
A significant circular dependency risk exists between `op-http` and the crates that register endpoints through it:
* **The Cycle:** `op-agents` and `op-tools` declare explicit dependencies on `op-http` within their `Cargo.toml` configurations. However, the architectural documentation in `crates/op-http/src/lib.rs:9-19` indicates that the central `op-http` server is responsible for composing and routing traffic to these very crates (e.g., `op_agents::create_router()`).
* **The Mitigation:** If `op-http` directly imports `op-agents` or `op-tools` to compose the router, compilation will fail due to cyclic dependencies. To prevent this, the system defines the `ServiceRouter` trait (`crates/op-http/src/router.rs:42-52`). The root orchestration crate (such as `op-dbus`) or a separate binary crate must perform the actual import and router composition, keeping `op-http` strictly agnostic of the downstream crates that depend on it.

---

## 5. Schema-as-Code Violations
The system's design principles require versioned schemas (such as Protocol Buffers or OSCAL) for all external contracts. The following locations violate this rule by expressing data contracts as ad-hoc Rust structs or raw strings:

### Violation 1: Ad-hoc Health Check Schema
* **File:** `crates/op-http/src/health.rs`
* **Lines:** 12-28
* **Details:** `HealthResponse` and `ServiceHealth` are defined as ad-hoc Rust structs with serialized JSON representations. These structures are not derived from versioned `.proto` files or OSCAL document schemas, leading to potential integration drift when modified.

### Violation 2: Ad-hoc JSON Metrics Serialization
* **File:** `crates/op-http/src/metrics.rs`
* **Lines:** 201-218
* **Details:** The `json_metrics` endpoint constructs an unversioned, arbitrary JSON structure on the fly using `simd_json::json!`. Changes to this internal representation can silently break external monitoring consumers.

---

## 6. Security and Quality Audit Findings

### [CRITICAL] Finding 1: Denial of Service (DoS) via Dynamic Prometheus Metric Registration Panic
* **File:** `crates/op-http/src/metrics.rs`
* **Lines:** 61-66, 105-125, 147-157
* **Vulnerability Type:** Improper Input Validation / Unhandled Exception (Panic)
* **Description:** 
  The metrics middleware (`metrics_middleware`) intercepts every HTTP request and calls `record_request`. This function extracts the service name from the path via `extract_service_name` (e.g., taking the third segment of paths starting with `/api/`). If the extracted service name is not present in the internal metrics registry, `record_request` dynamically instantiates a new metrics tracking struct via `ServiceMetrics::new`.
  
  Within `ServiceMetrics::new`, the unvalidated string is formatted directly into a Prometheus metric name:
  ```rust
  let request_count = register_counter!(
      format!("{}_requests_total", name),
      format!("Total requests for {} service", name)
  ).unwrap();
  ```
  Prometheus enforces strict naming conventions on metric names: `[a-zA-Z_:][a-zA-Z0-9_:]*`. If a request contains characters that do not match this regex (such as a hyphen `-` or space, or if the segment begins with a number), the `register_counter!` macro returns an `Err`.
* **Exploit Scenario:** 
  An attacker sends an unauthenticated request to:
  `GET /api/service-name/` or `GET /api/123service/`
  
  The service name is extracted as `"service-name"`. The server attempts to register a counter named `"service-name_requests_total"`. The registration fails, the `.unwrap()` panics, and the entire web server process immediately crashes. This provides an extremely cheap, 100% reliable remote Denial of Service exploit.

---

### [HIGH] Finding 2: Pseudo-Compression Middleware (Body/Header Mismatch)
* **File:** `crates/op-http/src/request_filters.rs`
* **Lines:** 125-136
* **Vulnerability Type:** Protocol / Data Corruption
* **Description:** 
  The `compression` filter acts as a middleware that forces the `Content-Encoding` header to `"gzip"` without actually applying Gzip compression to the response body:
  ```rust
  let mut response = next.run(request).await;
  let headers = response.headers_mut();
  headers.insert("Content-Encoding", "gzip".parse().unwrap());
  ```
* **Impact:** 
  Any browser or HTTP client receiving this response will attempt to decompress the raw, uncompressed payload as Gzip data. This results in immediate protocol/decoding errors, causing complete communication failure on endpoints wrapped by this middleware.

---

### [HIGH] Finding 3: Placeholder Authentication Bypasses Access Controls
* **File:** `crates/op-http/src/request_filters.rs`
* **Lines:** 73-102
* **Vulnerability Type:** Broken Authentication
* **Description:** 
  The `api_key_auth` middleware parses key headers (`x-api-key`, `authorization`, `x-password`) but fails to validate them. It contains a placeholder bypass:
  ```rust
  // For now, allow all requests (authentication is optional)
  // In production, you would validate the API key here
  ...
  next.run(request).await
  ```
* **Impact:** 
  Access controls are entirely absent. Unauthorized users can access internal system commands, state APIs, and private endpoints.

---

### [MEDIUM] Finding 4: Integer Underflow Panic on System Clock Backward Adjustments
* **File:** `crates/op-http/src/health.rs`
* **Lines:** 81-85
* **Vulnerability Type:** Numeric Overflow/Underflow
* **Description:** 
  The health checker calculates system uptime by subtracting start times in epoch seconds:
  ```rust
  let uptime = now - self.start_time
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();
  ```
* **Impact:** 
  If the host system's clock is synchronized backwards (e.g., via NTP drift correction or manual configuration) such that the current system time falls before the server's initialization time, the subtraction will overflow/underflow. Because Rust compiles with overflow checks in many profiles (and panics on unsigned underflow), invoking the detailed health check endpoint will trigger a server panic.

---

### [MEDIUM] Finding 5: Hardcoded Local Paths & Domain Names in TLS Auto-Detection
* **File:** `crates/op-http/src/tls.rs`
* **Lines:** 168-171, 194-198
* **Vulnerability Type:** Code Quality / Information Exposure
* **Description:** 
  The TLS auto-detection routine contains hardcoded paths belonging to a specific developer's local home folder and external production domains:
  * `/home/jeremy/certs/cloudflare_origin.pem`
  * `/etc/ssl/cloudflare/ghostbridge.tech/cert.pem`
* **Impact:** 
  This leaks local environment names to the binary, creates environmental instability if non-standard domains or folders change, and represents poor configuration hygiene for a shared control-plane library.

---

### [MEDIUM] Finding 6: Permissive Default Wildcard CORS Configuration
* **File:** `crates/op-http/src/middleware.rs`
* **Lines:** 181-196
* **Vulnerability Type:** Security Misconfiguration
* **Description:** 
  If `cors_origins` is omitted from `MiddlewareConfig` (which is the default state), the CORS layer defaults to allowing any origin, any method, and any header:
  ```rust
  CorsLayer::new()
      .allow_origin(Any)
      .allow_methods(Any)
      .allow_headers(Any)
  ```
* **Impact:** 
  Exposing the control plane with a wildcard CORS policy allows malicious sites visited by an authenticated operator to execute cross-origin actions against the local D-Bus/HTTP control-plane interface.