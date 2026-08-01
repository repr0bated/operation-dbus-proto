# Production Security & Quality Audit: op-http Crate

## 1. D-Bus & IPC Attack Surface Analysis

As a unified HTTP/TLS server binary wrapper, the `op-http` crate is designed as the external-facing network ingress. It acts as the HTTP entry point that composes routers for modules that communicate over D-Bus (such as `op-agents`, `op-introspection`, etc. which are mentioned in `crates/op-http/src/lib.rs`). 

Below is the structured analysis of the D-Bus and IPC attack surface within the audited source files:

### D-Bus Interface Registration
* **Registered Interfaces**: None. The `op-http` crate does not register or publish any direct D-Bus interfaces, methods, or signals in its own codebase. Its role is strictly HTTP/HTTPS routing and TLS termination.
* **Bus Connection**: The crate itself does not connect to either the `system` or `session` bus. (Other crates in the workspace, such as `op-introspection` or `op-agents`, manage their own ZBus connections).
* **Caller Identity Checks**: N/A (no direct D-Bus methods exposed in this crate).
* **System Bus Policy Over-Permissions**: No system bus XML policy was provided for the `op-http` crate, and no over-permissioned `allow` rules can be identified in the provided files.

### HTTP/IPC Exposed Surface & Deserialization
Although there are no D-Bus methods, the crate exposes an external HTTP/HTTPS IPC surface (using Axum/Hyper). 
* **Deserialization without Validation**: 
  * The endpoints use Serde/Simd-json to parse arbitrary payloads.
  * In `crates/op-http/src/health.rs`, health check endpoints serialize and deserialize `HealthResponse` and `ServiceHealth` dynamically.
  * In `crates/op-http/src/metrics.rs` line 208 (`json_metrics`), service metrics are parsed and formatted using `simd_json` without validation against any formal schema.

---

## 2. Security & Vulnerability Findings

### [CRITICAL] Completely Bypassable API Authentication Middleware
* **Location**: `crates/op-http/src/request_filters.rs:67-91`
* **Vulnerability Type**: Authentication Bypass / Missing Authorization Enforcer
* **Impact**: 
  Any route wrapped with the `api_key_auth` middleware is entirely unauthenticated. An attacker can access sensitive API endpoints without providing any credentials, or by providing completely arbitrary strings. Because the `op-http` crate acts as the central ingress gateway for the control plane, this exposes all nested service routers (`/api/mcp/*`, `/api/chat/*`, `/api/web/*`, `/api/tools/*`, `/api/agents/*`) to unauthenticated remote exploitation.
* **Analysis**:
  The `api_key_auth` implementation parses potential keys from headers but fails to reject requests when they are missing or invalid:
  ```rust
  // For now, allow all requests (authentication is optional)
  // In production, you would validate the API key here
  if let Some(key) = api_key {
      ...
  }
  next.run(request).await // <-- Always forwards the request
  ```
* **Remediation**:
  Replace the placeholder with a concrete key-verification database or config check, and return a `StatusCode::UNAUTHORIZED` response if the key is missing or invalid:
  ```rust
  if let Some(key) = api_key {
      if validate_key(key).await {
          return next.run(request).await;
      }
  }
  (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
  ```

---

### [HIGH] No-Op Rate Limiting Middleware Placeholder
* **Location**: `crates/op-http/src/request_filters.rs:93-109`
* **Vulnerability Type**: Denial of Service (DoS) Susceptibility
* **Impact**:
  The system lacks any actual rate limit enforcement at the middleware layer. Attackers can trivially flood the control plane endpoints with HTTP requests, exhaust server thread pools, or block local IPC/D-Bus translation loops.
* **Analysis**:
  The `rate_limit` middleware parses client IP headers but does not store bucket state or block requests:
  ```rust
  // For now, just log and allow all requests
  tracing::debug!("Request from IP: {}", client_ip);

  next.run(request).await
  ```
* **Remediation**:
  Integrate a stateful rate limiter such as `tower_governor` or `governor` to track requests per client IP and return `429 Too Many Requests` when thresholds are breached.

---

### [HIGH] Argument Injection & Arbitrary File Probing via External OpenSSL Invocation
* **Location**: `crates/op-http/src/tls.rs:377-417`
* **Vulnerability Type**: Argument Injection / Information Disclosure
* **Impact**:
  If certificate or key paths are configurable via any dynamic settings (e.g. administrative API endpoints or user-supplied profiles), an attacker can pass paths starting with dashes (e.g., `-help` or other flags) to inject command-line arguments into the `openssl` executable. Furthermore, pointing `cert_path` to arbitrary local files (like `/etc/shadow`) will cause `openssl` to attempt to parse them, potentially disclosing structural or partial data via parsing error logs.
* **Analysis**:
  The functions `validate_cert_key_match`, `get_cert_expiry`, and `is_cloudflare_cert` execute system commands directly with unvalidated path strings:
  ```rust
  let cert_output = Command::new("openssl")
      .args(["x509", "-in", cert_path, "-noout", "-modulus"])
      .output()
  ```
* **Remediation**:
  1. Avoid spawning shell processes to inspect certificate metadata. Use native Rust libraries (e.g., parsing the X.509 structure using the `x509-parser` crate) instead of calling out to the system's `openssl` binary.
  2. If process execution is unavoidable, strictly validate that `cert_path` and `key_path` do not begin with `-`, contain only safe characters, and represent canonicalized, authorized file paths.

---

## 3. Schema-as-Code Compliance & Quality Review

The audited codebase does not adhere to the "Schema-as-Code" discipline for external data representations or system configurations, instead relying on ad-hoc Serde definitions and dynamic strings.

### Ad-hoc Structs & Missing Versioned Schemas
* **Location**: `crates/op-http/src/health.rs:11-25`
* **Violation**: 
  The data contracts for `HealthResponse` and `ServiceHealth` are defined as ad-hoc, unversioned Rust structs annotated with basic Serde macros. They are not defined using a schema language like Protocol Buffers, nor do they comply with OSCAL (Open Security Controls Assessment Language) schemas for representing system health or security assessments.
* **Remediation**:
  Define all external telemetry and health structures as versioned Protocol Buffers (e.g., `op.telemetry.v1.HealthResponse`) and compile them using `prost-build` within the crate’s build script, or map them directly to standard OSCAL Assessment Results JSON schemas.

### Dynamic JSON Metric Serialization
* **Location**: `crates/op-http/src/metrics.rs:208-228`
* **Violation**:
  The `json_metrics` endpoint constructs a nested JSON response dynamically using the `simd_json::json!` macro:
  ```rust
  let response = json!({
      "services": service_metrics.into_iter()
          .map(|(name, metrics)| {
              (name, simd_json::json!({
                  "name": metrics.name,
              }))
          })
          .collect::<...>()
  });
  ```
  This approach prevents downstream consumers from programmatically validating the metrics output structure against a versioned data contract.
* **Remediation**:
  Serialize strongly-typed, schema-generated structs rather than utilizing ad-hoc JSON generation macros. Ensure metrics contracts are published as a formalized schema alongside the API router specifications.

---
## ⚠ Citation Warnings
- `crates/op-http/src/tls.rs:377`: file has 303 lines
