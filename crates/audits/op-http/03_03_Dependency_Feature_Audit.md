# SECTION 1: SYSTEM ARCHITECTURE & INTEGRITY AUDIT

`op-http` serves as the centralized HTTP/TLS server and middleware orchestrator for the `op-dbus` workspace control plane. Its architectural role is to compose routers exported by different feature crates (`op-mcp`, `op-chat`, `op-web`, `op-tools`, `op-agents`), apply global security and telemetry middleware, and manage TLS termination.

```text
unified-server binary
    └── op-http (central server)
        ├── TLS Termination (rustls/tokio-rustls)
        ├── Global Middleware Stack (CORS, tracing, compression)
        └── Route Composition
            ├── /api/mcp/*    → op_mcp::create_router()
            ├── /api/chat/*   → op_chat::create_router()
            ├── /api/web/*    → op_web::create_router()
            ├── /api/tools/*  → op_tools::create_router()
            ├── /api/agents/* → op_agents::create_router()
            └── /ws/*         → websocket handlers
```

---

# SECTION 2: DEPENDENCIES & FEATURE INVENTORY

An audit of the dependencies declared in `crates/op-http/Cargo.toml` and the workspace `Cargo.toml` highlights the following runtime library inventory:

| Dependency Crate | Version | Features Enabled | Resolving Source | Risk Profile / Notes |
| :--- | :--- | :--- | :--- | :--- |
| **tokio** | `1.49.0` | `full` | Workspace | Standard runtime. Risk: Medium (complex feature surface area). |
| **futures** | `0.3.31` | Default | Workspace | Asynchronous utility. Standard. |
| **serde** | `1.0.228` | `derive` | Workspace | Serialization framework. Standard. |
| **simd-json** | `0.13.11` | `serde`, `serde_impl` | Workspace | High-performance JSON parser. Risk: Medium (native code SIMD optimizations). |
| **anyhow** | `1.0.100` | Default | Workspace | Error context utility. Standard. |
| **thiserror** | `1.0.69` | Default | Workspace | Struct-based errors. Standard. |
| **tracing** | `0.1.44` | Default | Workspace | Structured diagnostics. Standard. |
| **axum** | `0.7.9` | `ws`, `macros`, `tokio` | Workspace | HTTP/WS web framework. Risk: Low-Medium. |
| **tower** | `0.4.13` | Default | Workspace | Middleware interface. Standard. |
| **tower-http** | `0.5.2` | `cors`, `fs`, `trace`, `compression-gzip`, `compression-br`, `timeout` | Local / Override | HTTP utilities. Contains CORS and compression filters. |
| **hyper** | `1.8.1` | `full` | Workspace | Low-level HTTP engine. Standard. |
| **hyper-util** | `0.1.19` | `full` | Workspace | Hyper server/client integration. Standard. |
| **rustls** | `0.23.36` | `aws-lc-rs` (via dep defaults) | Workspace | Modern TLS library. Risk: Low (memory-safe). |
| **rustls-pemfile**| `2.2.0` | Default | Workspace | PEM parsing. Standard. |
| **tokio-rustls** | `0.26.4` | Default | Workspace | Async TLS streams. Standard. |
| **chrono** | `0.4.43` | `serde` | Workspace | Date/Time handling. Standard. |
| **gethostname** | `0.5.0` | Default | Workspace | System hostname resolution. Standard. |

### Workspace Feature Gaps & Crates of Concern:
- **`serde_yaml` (v0.9.34)**: Declared in workspace `Cargo.toml`. This crate is unmaintained and deprecated. It should be replaced with `unsafe-libyaml` or another active parser if YAML parsing is a hard constraint.
- **No Native features defined**: `op-http/Cargo.toml` defines no custom `[features]`. 

---

# SECTION 3: STORAGE BACKEND ANALYSIS

An audit of all database engine dependencies and connection invocations across the workspace configuration reveals the following storage backend distribution:

| Backend Engine | Found at Location | Role (KV/Graph/Cache/Queue) | Architectural Alignment & Potential Violations |
| :--- | :--- | :--- | :--- |
| **CozoDB (Sled)** | `Cargo.toml:68` | Graph & Vector Relational Storage | **Aligned**: CozoDB with pure-Rust `storage-sled` is embedded directly into the workspace to avoid SQLite3 dynamic linkage clashes. Used for graph/knowledge storage in `op-cozo-store` and `op-cognitive-mcp`. |
| **SQLx (SQLite)** | `Cargo.toml:116` | Relational SQL Storage | **Aligned**: Utilized in `op-state-store`, `op-services`, and `op-dbus-model` for structured state logging. No graph or vector knowledge is bypassed into SQLite. |
| **Rusqlite** | `Cargo.toml:117` | Local Embedded DB | **Aligned**: Used in helper/sidecar modules such as `op-cache` and `op-mcp-proxy` for cache storage. |
| **Redis** | `Cargo.toml:118` | Distributed KV / Cache | **Aligned**: Configured as an optional distributed key-value store for state and session propagation. |

---

# SECTION 4: SCHEMA-AS-CODE COMPLIANCE REVIEW

The system implements a mixed data discipline. While the workspace imports gRPC/Protocol Buffer crates (`prost`, `prost-types`, `tonic`, `tonic-build`) to enforce schema-driven RPC validation, the `op-http` crate contains major **Schema-as-Code gaps**:

1. **Ad-Hoc Health Status Contract**:
   In `crates/op-http/src/health.rs:12-27`, `HealthResponse` and `ServiceHealth` are defined as ad-hoc Rust structs with serializable decorators:
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
   Instead of pulling these definitions from a versioned OpenAPI schema, JSON schema, or Protocol Buffer contract, they are hardcoded directly as Rust structures. Any downstream consumer (e.g., monitoring agents or client dashboards) must manually replicate these structures or rely on rust-specific imports.

2. **Ad-Hoc JSON Metrics Output**:
   In `crates/op-http/src/metrics.rs:230-247`, the `json_metrics` endpoint constructs a dynamic JSON object using `simd_json::json!`:
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
   This untyped, unstructured representation bypasses schema validation. Changes to metric structures are untracked by any contract, risking silent integration breakage on downstream API consumers.

---

# SECTION 5: SECURITY & QUALITY FINDINGS

## 1. Remote Denial of Service (DoS) and Process Crash via Dynamic Prometheus Metric Registration (CRITICAL)
- **File**: `crates/op-http/src/metrics.rs`
- **Lines**: 173-183, 122-139, 44-55
- **Description**: 
  The HTTP server's metrics recording system parses the incoming request path to determine the service name and dynamically registers Prometheus metrics based on this value.
  Specifically, `extract_service_name` (lines 173-183) splits the URL path and extracts the third path segment:
  ```rust
  fn extract_service_name(path: &str) -> &str {
      if path.starts_with("/api/") {
          path.split('/').nth(2).unwrap_or("unknown")
      ...
  ```
  If an API request is made to `/api/foo-bar`, the extracted service name is `foo-bar`. In `Metrics::record_request`, a new `ServiceMetrics` struct is instantiated (lines 44-55):
  ```rust
  let service_name = extract_service_name(path);
  let service_metrics = services.entry(service_name.to_string())
      .or_insert_with(|| ServiceMetrics::new(&service_name));
  ```
  Inside `ServiceMetrics::new` (lines 122-139), metric counters and histograms are dynamically initialized using the raw service name:
  ```rust
  let request_count = register_counter!(
      format!("{}_requests_total", name),
      format!("Total requests for {} service", name)
  ).unwrap();
  ```
  Prometheus enforces strict naming conventions on metric names. Only alphanumeric characters, underscores, and colons are permitted (`[a-zA-Z_:][a-zA-Z0-9_:]*`). If a name contains illegal characters (such as hyphens `-` or dots `.`), `register_counter!` returns an `Err(prometheus::Error::Msg)`. The code calls `.unwrap()` on this result, causing the thread to panic.
- **Exploit / Proof of Concept**:
  An attacker can send a single HTTP request to the server:
  ```bash
  curl -i http://<target>:8080/api/dos-crash/
  ```
  1. The path `/api/dos-crash/` is matched.
  2. `extract_service_name` extracts `"dos-crash"`.
  3. `ServiceMetrics::new("dos-crash")` is invoked.
  4. `register_counter!("dos-crash_requests_total")` is executed.
  5. Prometheus returns a registration error because hyphens are illegal characters.
  6. `.unwrap()` is called on the `Err` variant, triggering an instant panic within the HTTP connection thread. Depending on the runtime thread state and panic settings, this results in an immediate crash of the handler or potential lock poisoning.
- **Remediation**:
  Never register Prometheus metrics dynamically from raw, untrusted HTTP request parameters at runtime. 
  1. Use labels to distinguish services instead of dynamic metric names (e.g., `http_requests_total{service="dos-crash"}`).
  2. Remove all `.unwrap()` invocations from runtime metric registration.
  3. Validate any path input against a static whitelist of valid mounted service routes before performing map insertion.

---

## 2. Unbounded Memory Exhaustion (DDoS) via Dynamic Metrics Allocation (HIGH)
- **File**: `crates/op-http/src/metrics.rs`
- **Lines**: 48-52
- **Description**: 
  The global metrics registry tracks service statistics in a `HashMap` within `Metrics`:
  ```rust
  let mut services = self.services.write().await;
  let service_name = extract_service_name(path);
  let service_metrics = services.entry(service_name.to_string())
      .or_insert_with(|| ServiceMetrics::new(&service_name));
  ```
  Because the server dynamically registers three metrics (a request counter, a duration histogram, and an error counter) for *any* arbitrary string path segment, an attacker can continuously send requests with randomized service names (e.g., `/api/r1`, `/api/r2`, `/api/r3`, ...).
- **Risk**: 
  This permits an unauthenticated remote attacker to cause unbounded allocation of metrics objects in heap memory, triggering an Out-Of-Memory (OOM) panic that halts the control plane.
- **Remediation**: 
  Pre-register metrics for a fixed set of known service paths (e.g., `/api/mcp`, `/api/chat`, `/api/web`) at application startup and reject any dynamic registration attempts for unrecognized routes.

---

## 3. Broken Compression Middleware Sending Fake Content-Encoding Headers (HIGH)
- **File**: `crates/op-http/src/request_filters.rs`
- **Lines**: 118-130
- **Description**: 
  The local `compression` middleware unconditionally injects the `Content-Encoding: gzip` header into HTTP responses without performing any actual payload compression on the response body:
  ```rust
  pub async fn compression(
      request: Request,
      next: Next,
  ) -> Response {
      let mut response = next.run(request).await;

      let headers = response.headers_mut();
      headers.insert("Content-Encoding", "gzip".parse().unwrap());

      response
  }
  ```
  It also fails to inspect the incoming request's `Accept-Encoding` header to verify if the client supports gzip compression.
- **Risk**: 
  All HTTP clients receiving payloads from endpoints where this middleware is applied will attempt to parse raw, uncompressed bytes as gzip-encoded data. This results in standard browser and client-side decoding crashes (e.g., `ERR_CONTENT_DECODING_FAILED`), corrupting all outgoing API data.
- **Remediation**: 
  Delete this custom `compression` filter from `request_filters.rs` entirely. Rely exclusively on Tower-HTTP's native `CompressionLayer` which is already correctly configured in `crates/op-http/src/middleware.rs:136`.

---

## 4. Shell Execution of External Binary (openssl) for Certificate Checks (HIGH)
- **File**: `crates/op-http/src/tls.rs`
- **Lines**: 245-288
- **Description**: 
  The certificate validation routines shell out to the system's `openssl` command-line utility via `std::process::Command` to compare moduli, determine expiration dates, and check issuer names:
  ```rust
  let cert_output = Command::new("openssl")
      .args(["x509", "-in", cert_path, "-noout", "-modulus"])
      .output()
      .map_err(|e| ServerError::CertificateError(format!("Failed to run openssl: {}", e)))?;
  ```
  This creates several fatal defects:
  1. **Deployment Portability Failure**: If the control plane binary is executed within a slim, distroless, or minimal Alpine container, the system lacks the `openssl` CLI. The server will fail to startup or check certificate health.
  2. **Performance Penalities**: Spawning processes at runtime is a heavy operation that wastes CPU cycles and memory.
  3. **Argument Injection**: If the `cert_path` variable is configured dynamically or read from an untrusted system parameter, an attacker can input arguments (such as files starting with hyphens) to modify command-line behavior.
- **Risk**: 
  Runtime platform crashes, poor performance under automated renewal operations, and vulnerability to argument-injection attacks.
- **Remediation**: 
  Parse PEM certificates natively using Rust-native libraries such as `rustls-pki-types` or `x509-parser` instead of spawning external shell commands.

---

## 5. Potential Process Panic on Negative Epoch Clock Drift (MEDIUM)
- **File**: `crates/op-http/src/health.rs`
- **Lines**: 92-95, 185, 203, 219, 237
- **Description**: 
  The health module calculates UNIX timestamps by querying `SystemTime::now()` and unwrapping the duration since epoch:
  ```rust
  let last_check = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();
  ```
  If the host system's hardware clock drifts backwards (due to NTP sync, hypervisor clock corrections, or poor virtualization clock setup) such that `SystemTime::now()` is prior to `UNIX_EPOCH` (1970-01-01 00:00:00 UTC), `duration_since` returns an `Err`.
- **Risk**: 
  The server will crash during runtime due to a clock modification or NTP correction.
- **Remediation**: 
  Avoid calling `.unwrap()` on `duration_since(UNIX_EPOCH)`. Use a safe wrapper:
  ```rust
  SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|d| d.as_secs())
      .unwrap_or(0)
  ```

---

## 6. Denial of Service via Unbounded Connection Concurrency (MEDIUM)
- **File**: `crates/op-http/src/server.rs`
- **Lines**: 111-131
- **Description**: 
  In the custom HTTPS connection loop, the server continuously accepts TCP connections and spawns a tokio task to perform the TLS handshake and HTTP parsing without limiting the maximum number of concurrent sessions:
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
                  ...
  ```
- **Risk**: 
  A simple concurrent connection attack (such as a Slowloris attack or a connection flood) can exhaust the server's available file descriptors (EMFILE) and memory resources, causing the application to drop valid traffic.
- **Remediation**: 
  Protect the connection loop using a `tokio::sync::Semaphore` to restrict active concurrent handshakes, or configure connection-limit layers.

---

## 7. Performance Degradation via Per-Request Reqwest Client Building (LOW)
- **File**: `crates/op-http/src/health.rs`
- **Lines**: 205-207
- **Description**: 
  The health checking utility instantiates a brand new `reqwest::Client` on every request to check service health:
  ```rust
  let client = reqwest::Client::builder()
      .timeout(timeout)
      .build();
  ```
  `reqwest::Client` holds an internal connection pool designed to be reused. Rebuilding the client on each invocation forces a complete socket allocation and handshake cycle, completely bypassing HTTP connection pooling.
- **Risk**: 
  Excessive socket allocation leading to local port exhaustion (TIME_WAIT states) and high checking latency.
- **Remediation**: 
  Store a single, persistent `reqwest::Client` inside the `HealthChecker` struct and reuse it across checking intervals.

---

## 8. Hardcoded Personal Developer Directory Path (LOW)
- **File**: `crates/op-http/src/tls.rs`
- **Lines**: 147-150
- **Description**: 
  The TLS auto-detection routine includes a hardcoded absolute file path pointing to a specific user's home directory:
  ```rust
  // User directory
  (
      "/home/jeremy/certs/cloudflare_origin.pem",
      "/home/jeremy/certs/cloudflare_origin.key",
  ),
  ```
- **Risk**: 
  Information exposure of the developer's system name ("jeremy") and failure of certificate auto-detection in standard target deployment environments.
- **Remediation**: 
  Remove home-directory paths from compiled production code. Load certificates only through structured environment variables or default system directories (such as `/etc/ssl/certs`).

---

## 9. Redundant Security Headers Middleware Definition (LOW)
- **File**: `crates/op-http/src/middleware.rs`, `crates/op-http/src/request_filters.rs`
- **Lines**: `middleware.rs:186`, `request_filters.rs:14`
- **Description**: 
  The codebase contains duplicate implementations for setting security headers. `middleware.rs` registers `security_headers_middleware`, while `request_filters.rs` defines a matching `security_headers` filter. Both insert identical options such as `X-Content-Type-Options: nosniff`.
- **Risk**: 
  Unnecessary code bloat and potential policy drift if headers are updated in one file but not the other.
- **Remediation**: 
  Consolidate all security headers within `middleware.rs` and delete the redundant filters in `request_filters.rs`.